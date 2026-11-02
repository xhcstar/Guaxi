// 知识图谱布局器:优先使用后端固化坐标(kg_load_layout),缺失的节点用 d3-force-3d
// 现场补算,然后把结果回存(kg_save_layout)。支持多视角(relation/tag/time/importance)
// 各自独立的坐标缓存,使视角切换可以做平滑 GSAP 转场。
//
// 力导向(relation 视角)计算成本最高,采用三级回退,任一层失败自动降级,保证零功能损失:
//   1. 后端 Rust kg_compute_layout(rayon 并行,最快、零主线程开销)
//   2. 前端 Web Worker(d3-force-3d,不阻塞 UI)
//   3. 主线程同步(原实现,最终兜底)
import {
  forceSimulation,
  forceManyBody,
  forceLink,
  forceCenter,
  forceCollide,
} from "d3-force-3d";

// ─── 力导向 Worker(单例 + 任务队列 + 失败回退)───
let layoutWorker = null;
let workerUsable = true;
let workerSeq = 0;
const workerPending = new Map();

function getLayoutWorker() {
  if (!workerUsable) return null;
  if (layoutWorker) return layoutWorker;
  try {
    layoutWorker = new Worker(new URL("../../workers/kgForceLayoutWorker.js", import.meta.url), {
      type: "module",
    });
    layoutWorker.onmessage = (event) => {
      const { id, ok, points, error } = event.data || {};
      const task = workerPending.get(id);
      if (!task) return;
      workerPending.delete(id);
      if (ok) task.resolve(points);
      else task.reject(new Error(error || "layout worker failed"));
    };
    layoutWorker.onerror = () => {
      workerUsable = false;
      for (const t of workerPending.values()) t.reject(new Error("layout worker error"));
      workerPending.clear();
    };
    return layoutWorker;
  } catch (e) {
    workerUsable = false;
    return null;
  }
}

// 用 Worker 跑力导向;失败抛错由上层回退主线程
function computeForceLayoutInWorker(nodes, edges, pos) {
  const worker = getLayoutWorker();
  if (!worker) return Promise.reject(new Error("no worker"));
  const id = ++workerSeq;
  const wnodes = nodes.map((n) => {
    const seed = pos.get(n.id);
    return seed ? { id: n.id, x: seed.x, y: seed.y, z: seed.z } : { id: n.id };
  });
  const links = edges.map((e) => ({ source: e.from, target: e.to, strength: e.strength ?? 0.3 }));
  return new Promise((resolve, reject) => {
    workerPending.set(id, {
      resolve: (points) => {
        for (const p of points) pos.set(p.id, { x: p.x, y: p.y, z: p.z });
        resolve(pos);
      },
      reject,
    });
    worker.postMessage({ id, nodes: wnodes, links });
  });
}

// 组件卸载时调用,释放 Worker
export function disposeLayoutWorker() {
  if (layoutWorker) {
    try { layoutWorker.terminate(); } catch (e) { /* ignore */ }
    layoutWorker = null;
  }
  workerPending.clear();
}

/**
 * 立刻可渲染的坐标,用于首屏秒开:
 * - relation:只读缓存(不跑力导向),缺失节点本地贴邻居补位;needsForce 表示需要后台细化。
 * - tag/time/importance:布点本身很轻,直接本地算完并回存,无需等待后端。
 * 与 resolveLayout 不同,这里绝不触发昂贵的力导向,调用方负责在后台补算。
 * @returns {Promise<{pos: Map, needsForce: boolean}>}
 */
export async function resolveLayoutImmediate(store, kbName, view, graph) {
  const nodes = graph.nodes || [];
  const edges = graph.edges || [];
  const pos = new Map();

  if (view === "relation") {
    try {
      const cached = await store.kgLoadLayout(kbName, view);
      for (const p of cached || []) {
        pos.set(p.node_id, { x: p.x, y: p.y, z: p.z });
      }
    } catch (e) {
      /* 无缓存:全部本地补位 */
    }
    const seeded = seedMissingPositions(nodes, edges, pos);
    return { pos, needsForce: seeded > 0 };
  }

  if (view === "importance") {
    computeRadialByMetric(nodes, pos, (n) => n.pagerank ?? 0);
  } else if (view === "time") {
    computeAxisByTime(nodes, pos, { timeOf: (n) => n.modified_at || n.created_at });
  } else {
    computeClusterByKey(nodes, pos, (n) => {
      if (n.tags && n.tags.length) return `tag:${n.tags[0]}`;
      if (n.community_id != null) return `c:${n.community_id}`;
      return `type:${n.type}`;
    });
  }
  seedMissingPositions(nodes, edges, pos);
  // 这些视角是纯确定性计算(毫秒级),不走数据库回存,避免首屏多一次批量写入。
  return { pos, needsForce: false };
}

/**
 * 取得某视角下每个节点的目标坐标 Map<node_id,{x,y,z}>。
 * @param {object} store knowledgeBaseStore 实例
 * @param {string} kbName
 * @param {string} view relation|tag|time|importance
 * @param {{nodes:Array, edges:Array}} graph
 * @param {object} ctx 额外上下文(communities, tagOf 等)用于非 relation 视角布点
 */
export async function resolveLayout(store, kbName, view, graph, ctx = {}, opts = {}) {
  const nodes = graph.nodes || [];
  const edges = graph.edges || [];
  const pos = new Map();

  // 1. 读取已固化坐标(force 时跳过,强制按当前算法重算)
  if (!opts.force) {
    try {
      const cached = await store.kgLoadLayout(kbName, view);
      for (const p of cached || []) {
        pos.set(p.node_id, { x: p.x, y: p.y, z: p.z });
      }
    } catch (e) {
      /* 忽略,走现场计算 */
    }
  }

  const missing = nodes.filter((n) => !pos.has(n.id));
  if (missing.length === 0) {
    return pos;
  }

  // 2. 视角决定布局策略
  if (view === "relation") {
    await computeForceLayout(store, kbName, nodes, edges, pos, opts);
  } else if (view === "importance") {
    computeRadialByMetric(nodes, pos, (n) => n.pagerank ?? 0);
  } else if (view === "time") {
    computeAxisByTime(nodes, pos, ctx);
  } else if (view === "tag") {
    // 分类视角:优先按首个标签聚簇,无标签退化到社区,再退化到类型,保证有意义的分散
    computeClusterByKey(nodes, pos, (n) => {
      if (n.tags && n.tags.length) return `tag:${n.tags[0]}`;
      if (n.community_id != null) return `c:${n.community_id}`;
      return `type:${n.type}`;
    });
  } else {
    await computeForceLayout(store, kbName, nodes, edges, pos, opts);
  }

  // 2.5 兜底:仍然缺坐标的节点(例如后端返回了不含新节点的旧缓存)必须补上。
  // 否则渲染层会把它们回退到 (0,0,0) —— 彼此重叠、被大球吞掉、连边被整条跳过,
  // 表现为"新建的文件在 3D 图里根本看不到"。
  const seededCount = seedMissingPositions(nodes, edges, pos);

  // 3. 回存(只存本视角全部节点)。后端 kg_compute_layout 已自行回存时跳过,避免重复写;
  // 但前端补过点就必须落盘,否则下次仍是残缺缓存。
  if (!pos._savedByBackend || seededCount > 0) {
    try {
      const points = nodes.map((n) => {
        const p = pos.get(n.id) || { x: 0, y: 0, z: 0 };
        return { node_id: n.id, x: p.x, y: p.y, z: p.z };
      });
      await store.kgSaveLayout(kbName, view, points);
    } catch (e) {
      /* 存储失败不致命 */
    }
  }

  return pos;
}

// 补点与既有节点的最小间距(最大节点半径 11,留出肉眼可辨的空隙)
const LAYOUT_MIN_SEP = 14;

// 稳定哈希(FNV-1a):落点相位由 node_id 决定,而不是"缺失列表里的序号"。
// 用序号会让每轮补点的第一个节点落到同一点上 —— 新目录会精确重叠在上次新增的目录里。
function stableHash(id) {
  let h = 0x811c9dc5;
  const s = String(id);
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

// 空间网格:O(1) 查询"该点附近是否已有节点",避免补点重合
function makeGrid(cell) {
  const map = new Map();
  const key = (x, y, z) =>
    `${Math.floor(x / cell)},${Math.floor(y / cell)},${Math.floor(z / cell)}`;
  return {
    insert(x, y, z) {
      const k = key(x, y, z);
      if (!map.has(k)) map.set(k, []);
      map.get(k).push([x, y, z]);
    },
    hasWithin(x, y, z, r) {
      const r2 = r * r;
      const cx = Math.floor(x / cell), cy = Math.floor(y / cell), cz = Math.floor(z / cell);
      for (let dx = -1; dx <= 1; dx++) {
        for (let dy = -1; dy <= 1; dy++) {
          for (let dz = -1; dz <= 1; dz++) {
            const pts = map.get(`${cx + dx},${cy + dy},${cz + dz}`);
            if (!pts) continue;
            for (const [px, py, pz] of pts) {
              const d2 = (x - px) ** 2 + (y - py) ** 2 + (z - pz) ** 2;
              if (d2 < r2) return true;
            }
          }
        }
      }
      return false;
    },
  };
}

/**
 * 给还没有坐标的节点补位:贴着已定位的邻居(父目录/所属文件)落位,确定性无随机。
 * 与后端 seed_missing_layout 同一套策略,作为跨层兜底。
 * @returns {number} 补了多少个节点
 */
function seedMissingPositions(nodes, edges, pos) {
  const missing = nodes.filter((n) => !pos.has(n.id));
  if (!missing.length) return 0;

  const adj = new Map();
  const link = (a, b) => {
    if (!adj.has(a)) adj.set(a, []);
    adj.get(a).push(b);
  };
  for (const e of edges) {
    link(e.from, e.to);
    link(e.to, e.from);
  }

  // 全局质心与外径:一个已定位邻居都没有时的兜底锚点
  let cx = 0, cy = 0, cz = 0, count = 0, rMax = 0;
  for (const p of pos.values()) { cx += p.x; cy += p.y; cz += p.z; count++; }
  if (count) {
    cx /= count; cy /= count; cz /= count;
    for (const p of pos.values()) {
      rMax = Math.max(rMax, Math.hypot(p.x - cx, p.y - cy, p.z - cz));
    }
  }

  // 浅层先放,父节点先有坐标,子节点才能贴着它
  const ordered = [...missing].sort(
    (a, b) => (a.depth ?? 99) - (b.depth ?? 99) || String(a.id).localeCompare(String(b.id))
  );
  const golden = Math.PI * (3 - Math.sqrt(5));
  const spreadOf = (type) => (type === "directory" ? 46 : type === "file" ? 28 : type === "concept" ? 24 : 18);

  // 既有节点入网格,新点据此避让
  const grid = makeGrid(LAYOUT_MIN_SEP);
  for (const p of pos.values()) grid.insert(p.x, p.y, p.z);

  for (const n of ordered) {
    let ax = 0, ay = 0, az = 0, c = 0;
    for (const id of adj.get(n.id) || []) {
      const p = pos.get(id);
      if (p) { ax += p.x; ay += p.y; az += p.z; c++; }
    }
    const anchor = c ? { x: ax / c, y: ay / c, z: az / c } : { x: cx, y: cy, z: cz };
    const h = stableHash(n.id);
    const t = ((h >>> 9) % 64 + 0.5) / 64;
    const phi = Math.acos(1 - 2 * t);
    let theta = golden * (h % 100003);
    let spread = Math.max(LAYOUT_MIN_SEP, c ? spreadOf(n.type) : Math.max(60, rMax * 0.8));
    let x = 0, y = 0, z = 0;
    for (let attempt = 0; attempt < 24; attempt++) {
      x = anchor.x + spread * Math.sin(phi) * Math.cos(theta);
      y = anchor.y + spread * (1 - 2 * t) * 0.6;
      z = anchor.z + spread * Math.sin(phi) * Math.sin(theta);
      if (!grid.hasWithin(x, y, z, LAYOUT_MIN_SEP)) break;
      theta += golden;
      spread += LAYOUT_MIN_SEP * 0.35;
    }
    grid.insert(x, y, z);
    pos.set(n.id, { x, y, z });
  }
  return ordered.length;
}

// relation 视角:三级回退(后端 Rust → 前端 Worker → 主线程同步)
async function computeForceLayout(store, kbName, nodes, edges, pos, opts) {
  // 2a. 后端 Rust 计算(最优)。后端会直接写入 kg_layout 缓存。
  if (store && typeof store.kgComputeLayout === "function") {
    try {
      const points = await store.kgComputeLayout(kbName, "relation", opts.force === true);
      if (Array.isArray(points) && points.length) {
        for (const p of points) pos.set(p.node_id ?? p.id, { x: p.x, y: p.y, z: p.z });
        // 标记后端已存,避免上层重复回存
        Object.defineProperty(pos, "_savedByBackend", { value: true, enumerable: false });
        return;
      }
    } catch (e) {
      /* 后端不可用 → 回退前端 */
    }
  }
  // 2b. 前端 Worker
  try {
    await computeForceLayoutInWorker(nodes, edges, pos);
    return;
  } catch (e) {
    /* worker 不可用 → 回退主线程同步 */
  }
  // 2c. 主线程同步(兜底)
  computeForceLayoutSync(nodes, edges, pos);
}


// relation 视角:d3-force-3d 物理布局(主线程同步兜底实现)
function computeForceLayoutSync(nodes, edges, pos) {
  const simNodes = nodes.map((n) => {
    const seed = pos.get(n.id);
    return seed ? { id: n.id, x: seed.x, y: seed.y, z: seed.z } : { id: n.id };
  });
  const idset = new Set(simNodes.map((n) => n.id));
  const simLinks = edges
    .filter((e) => idset.has(e.from) && idset.has(e.to))
    .map((e) => ({ source: e.from, target: e.to, strength: e.strength ?? 0.3 }));

  const sim = forceSimulation(simNodes, 3)
    .force("charge", forceManyBody().strength(-30))
    .force(
      "link",
      forceLink(simLinks)
        .id((d) => d.id)
        .distance((l) => 18 + (1 - (l.strength ?? 0.3)) * 30)
        .strength((l) => Math.min(1, (l.strength ?? 0.3)))
    )
    .force("center", forceCenter(0, 0, 0))
    .force("collide", forceCollide(3))
    .stop();

  const ticks = Math.min(300, Math.max(80, Math.round(120 + nodes.length / 40)));
  for (let i = 0; i < ticks; i++) sim.tick();

  for (const n of simNodes) {
    pos.set(n.id, { x: n.x || 0, y: n.y || 0, z: n.z || 0 });
  }
}

// importance 视角:PageRank 越高越靠中心,层层外扩
function computeRadialByMetric(nodes, pos, metric) {
  const sorted = [...nodes].sort((a, b) => (metric(b) || 0) - (metric(a) || 0));
  const golden = Math.PI * (3 - Math.sqrt(5));
  const n = sorted.length || 1;
  const maxR = 120 + Math.sqrt(n) * 26; // 随规模扩展,避免挤一团
  sorted.forEach((node, i) => {
    const t = i / n;
    const radius = 14 + t * maxR;
    const y = (1 - 2 * t) * (maxR * 0.35);
    const theta = i * golden;
    pos.set(node.id, {
      x: radius * Math.cos(theta),
      y,
      z: radius * Math.sin(theta),
    });
  });
}

// time 视角:沿 X 轴按时间排布;无有效时间戳则退化为按索引均匀排布
function computeAxisByTime(nodes, pos, ctx) {
  const tof = (n) => {
    const raw = ctx.timeOf ? ctx.timeOf(n) : n.modified_at || n.created_at;
    const t = raw ? Date.parse(raw) : NaN;
    return Number.isFinite(t) ? t : NaN;
  };
  const n = nodes.length || 1;
  const width = 200 + Math.sqrt(n) * 40;
  const withT = nodes.map((node, i) => ({ node, i, t: tof(node) }));
  const valid = withT.filter((x) => Number.isFinite(x.t)).map((x) => x.t);
  const hasTime = valid.length > 1;
  const min = hasTime ? Math.min(...valid) : 0;
  const max = hasTime ? Math.max(...valid) : 1;
  const span = Math.max(1, max - min);

  withT.forEach(({ node, i, t }) => {
    // 有时间用时间定位,否则用索引均匀铺开,避免全部落在同一 x
    const frac = hasTime && Number.isFinite(t) ? (t - min) / span : i / n;
    const x = frac * (width * 2) - width;
    const angle = i * 2.399;
    const r = 40 + (i % 80) * 1.6;
    pos.set(node.id, { x, y: r * Math.sin(angle) * 0.7, z: r * Math.cos(angle) * 0.7 });
  });
}

// tag/社区 视角:同组聚成簇,簇心环形分布;间距随组数/成员数扩展
function computeClusterByKey(nodes, pos, keyOf) {
  const groups = new Map();
  for (const n of nodes) {
    const k = keyOf(n);
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(n);
  }
  const keys = [...groups.keys()];
  const gn = keys.length || 1;
  const ringR = 140 + gn * 14 + Math.sqrt(nodes.length) * 6;
  keys.forEach((k, gi) => {
    const ga = (gi / gn) * Math.PI * 2;
    const gx = Math.cos(ga) * ringR;
    const gz = Math.sin(ga) * ringR;
    const members = groups.get(k);
    const mn = members.length || 1;
    const clusterR = 12 + Math.sqrt(mn) * 9;
    const golden = Math.PI * (3 - Math.sqrt(5));
    members.forEach((node, mi) => {
      const a = mi * golden;
      const rr = clusterR * Math.sqrt((mi + 0.5) / mn);
      pos.set(node.id, {
        x: gx + Math.cos(a) * rr,
        y: (((mi % 7) - 3) / 3) * (clusterR * 0.5),
        z: gz + Math.sin(a) * rr,
      });
    });
  });
}
