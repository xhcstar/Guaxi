// 知识图谱布局器:优先使用后端固化坐标(kg_load_layout),缺失的节点用 d3-force-3d
// 现场补算,然后把结果回存(kg_save_layout)。支持多视角(relation/tag/time/importance)
// 各自独立的坐标缓存,使视角切换可以做平滑 GSAP 转场。
import {
  forceSimulation,
  forceManyBody,
  forceLink,
  forceCenter,
  forceCollide,
} from "d3-force-3d";

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
    computeForceLayout(nodes, edges, pos);
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
    computeForceLayout(nodes, edges, pos);
  }

  // 3. 回存(只存本视角全部节点)
  try {
    const points = nodes.map((n) => {
      const p = pos.get(n.id) || { x: 0, y: 0, z: 0 };
      return { node_id: n.id, x: p.x, y: p.y, z: p.z };
    });
    await store.kgSaveLayout(kbName, view, points);
  } catch (e) {
    /* 存储失败不致命 */
  }

  return pos;
}

// relation 视角:d3-force-3d 物理布局
function computeForceLayout(nodes, edges, pos) {
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
