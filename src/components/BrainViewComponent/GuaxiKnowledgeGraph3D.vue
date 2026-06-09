<template>
  <div class="kg3d-root">
    <!-- 顶部容器:工具条 + 过滤,纵向排布,过滤始终在工具条下方(工具条换行也不重叠) -->
    <div class="kg3d-top">
      <div class="kg3d-toolbar">
        <div class="tb-left">
          <span class="tb-title">知识图谱</span>
          <span class="tb-crumb">
            <a v-for="(c, i) in breadcrumb" :key="c.id" @click="drillTo(i)">{{ c.label }}</a>
          </span>
        </div>
        <div class="tb-right">
          <input
            v-model="searchText"
            class="tb-search"
            placeholder="搜索 / 语义检索…"
            @keyup.enter="runSearch"
          />
          <select v-model="view" class="tb-sel" @change="switchView">
            <option value="relation">关系视角</option>
            <option value="tag">分类视角</option>
            <option value="time">时间视角</option>
            <option value="importance">重要度视角</option>
          </select>
          <button class="tb-btn" :class="{ active: pathMode }" @click="togglePathMode" title="两点连线:依次点两个节点">路径</button>
          <button class="tb-btn" :class="{ active: collapsed }" @click="toggleCollapse" title="按社区折叠为超级节点">折叠</button>
          <button class="tb-btn" @click="rebuildIndexAndReload" title="重建图索引">重建</button>
          <button class="tb-btn" @click="runCommunities" title="社区检测着色">社区</button>
          <button class="tb-btn" @click="runEmbed" title="生成/更新向量(首次会自动下载模型)">向量</button>
          <button class="tb-btn" @click="runExtract" title="用 AI 抽取概念与关系">概念</button>
        </div>
      </div>

      <!-- 类型过滤 -->
      <div class="kg3d-filters">
        <label v-for="t in allTypes" :key="t.key" class="flt">
          <input type="checkbox" :checked="visibleTypes.has(t.key)" @change="toggleType(t.key)" />
          <span class="flt-dot" :style="{ background: t.color }"></span>{{ t.label }}
        </label>
      </div>
    </div>

    <div ref="canvasWrap" class="kg3d-canvas"></div>

    <!-- 小地图:俯视投影 + 当前焦点(可点击定位) -->
    <canvas ref="minimap" class="kg3d-minimap" width="180" height="140" @click="onMinimapClick"></canvas>

    <!-- 时间轴:拖动按修改时间过滤,回放知识增长 -->
    <div class="kg3d-timeline">
      <button class="tl-play" @click="toggleTimelinePlay">{{ timelinePlaying ? "⏸" : "▶" }}</button>
      <input
        type="range"
        min="0"
        max="100"
        v-model.number="timelineValue"
        class="tl-range"
        @input="applyTimeline"
      />
      <span class="tl-label">{{ timelineLabel }}</span>
    </div>

    <!-- 选中信息 / 建议面板 -->
    <div v-if="selected" class="kg3d-panel">
      <div class="pn-head">
        <span class="pn-type" :style="{ background: typeColor(selected.type) }">{{ selected.type }}</span>
        <span class="pn-name">{{ selected.name }}</span>
        <span class="pn-x" @click="clearSelection">×</span>
      </div>
      <div class="pn-meta">
        <span v-if="selected.rel_path">{{ selected.rel_path }}</span>
        <span>度 {{ selected.degree }} · PR {{ (selected.pagerank || 0).toFixed(4) }}</span>
      </div>
      <div v-if="selected.tags && selected.tags.length" class="pn-tags">
        <span v-for="tg in selected.tags" :key="tg" class="pn-tag">#{{ tg }}</span>
      </div>
      <div class="pn-actions">
        <button @click="suggestRelations">推荐关联</button>
        <button @click="suggestTags">推荐标签</button>
        <button @click="focusEgo">聚焦邻居</button>
      </div>
      <div v-if="suggestions.length" class="pn-suggest">
        <div class="pn-sub">关联建议</div>
        <div v-for="s in suggestions" :key="s.to" class="pn-srow">
          <span class="pn-sname" :title="s.to_name">{{ s.to_name }}</span>
          <span class="pn-sscore">{{ s.score.toFixed(2) }}</span>
          <button @click="confirmRelation(s)">＋</button>
        </div>
      </div>
      <div v-if="tagSuggestions.length" class="pn-suggest">
        <div class="pn-sub">标签建议</div>
        <div v-for="s in tagSuggestions" :key="s.tag" class="pn-srow">
          <span class="pn-sname">#{{ s.tag }}</span>
          <span class="pn-sscore">{{ s.score.toFixed(2) }}</span>
          <button @click="confirmTag(s)">＋</button>
        </div>
      </div>
    </div>

    <div v-if="statusText" class="kg3d-status">{{ statusText }}</div>
  </div>
</template>

<script setup>
import { onMounted, onBeforeUnmount, ref, shallowRef } from "vue";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { CSS2DRenderer, CSS2DObject } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import gsap from "gsap";
import { useKnowledgeBaseStore } from "../../stores/knowledgeBaseStore";
import { resolveLayout } from "./kgLayout";

const props = defineProps({
  kbName: { type: String, default: "" },
});

const store = useKnowledgeBaseStore();

const TYPE_COLORS = {
  "knowledge-base": 0xf39c12,
  directory: 0x3498db,
  file: 0x2ecc71,
  section: 0x8e44ad,
  paragraph: 0xd35400,
  concept: 0xe74c3c,
  unknown: 0x95a5a6,
};
const allTypes = [
  { key: "directory", label: "目录", color: "#3498db" },
  { key: "file", label: "文件", color: "#2ecc71" },
  { key: "section", label: "章节", color: "#8e44ad" },
  { key: "paragraph", label: "段落", color: "#d35400" },
  { key: "concept", label: "概念", color: "#e74c3c" },
];

const canvasWrap = ref(null);
const minimap = ref(null);
const searchText = ref("");
const view = ref("relation");
const pathMode = ref(false);
const collapsed = ref(false);
const selected = ref(null);
const suggestions = ref([]);
const tagSuggestions = ref([]);
const statusText = ref("");
const visibleTypes = shallowRef(new Set(["directory", "file", "section", "paragraph", "concept"]));
const breadcrumb = ref([{ id: "kb_root", label: "全库" }]);
const timelineValue = ref(100);
const timelineLabel = ref("全部");
const timelinePlaying = ref(false);
let timelinePlayTimer = null;
let timeBounds = { min: 0, max: 1 };
// 折叠前保留的完整图,便于展开/恢复
let fullNodeList = null;
let fullEdgeList = null;
let minimapTick = 0;

// three 运行时对象(非响应式)
let scene, camera, renderer, labelRenderer, controls, raf;
let typeMeshes = {}; // type -> InstancedMesh
let edgeLines = null;
let nodeList = []; // [{...node, _pos:{x,y,z}, _type, _inst}]
let nodeById = new Map();
let edgeList = [];
let communityColors = new Map(); // community_id -> hex string
let labelObjs = [];
let pathPick = [];
let curPositions = new Map(); // node_id -> {x,y,z}
let themeColors = { bg: 0x0b0e14, edge: 0x4a5568, isDark: true };
let themeObserver = null;
let lastProj = null; // 小地图投影参数,用于点击反投影
const tmpMat = new THREE.Matrix4();
const tmpColor = new THREE.Color();
const tmpVec = new THREE.Vector3();
const tmpQuat = new THREE.Quaternion();
const tmpScale = new THREE.Vector3();

function typeColor(t) {
  return "#" + (TYPE_COLORS[t] ?? TYPE_COLORS.unknown).toString(16).padStart(6, "0");
}

// 读取主题色:根据 --guaxi-content-bg 亮度判断深/浅色,适配亮色模式
function readThemeColors() {
  let bgHex = "#0b0e14";
  try {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue("--guaxi-content-bg")
      .trim();
    if (v) bgHex = v;
  } catch (e) {
    /* ignore */
  }
  const c = new THREE.Color(bgHex);
  const lum = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
  const isDark = lum < 0.5;
  return {
    bg: c.getHex(),
    edge: isDark ? 0x4a5568 : 0xb8c0cc,
    isDark,
  };
}

function applyTheme() {
  themeColors = readThemeColors();
  if (scene) scene.background = new THREE.Color(themeColors.bg);
  if (edgeLines) edgeLines.material.color = new THREE.Color(themeColors.edge);
  // 切换浅/深色根类,驱动 CSS 覆盖层配色
  const root = canvasWrap.value?.closest(".kg3d-root");
  if (root) root.classList.toggle("kg3d-light", !themeColors.isDark);
}

function initThree() {
  const wrap = canvasWrap.value;
  const w = wrap.clientWidth || 800;
  const h = wrap.clientHeight || 600;

  scene = new THREE.Scene();
  themeColors = readThemeColors();
  scene.background = new THREE.Color(themeColors.bg);

  camera = new THREE.PerspectiveCamera(60, w / h, 0.1, 100000);
  camera.position.set(0, 80, 320);

  renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(w, h);
  wrap.appendChild(renderer.domElement);

  labelRenderer = new CSS2DRenderer();
  labelRenderer.setSize(w, h);
  labelRenderer.domElement.style.position = "absolute";
  labelRenderer.domElement.style.top = "0";
  labelRenderer.domElement.style.pointerEvents = "none";
  wrap.appendChild(labelRenderer.domElement);

  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;

  scene.add(new THREE.AmbientLight(0xffffff, 0.85));
  const dir = new THREE.DirectionalLight(0xffffff, 0.6);
  dir.position.set(120, 200, 160);
  scene.add(dir);

  renderer.domElement.addEventListener("click", onClick);
  renderer.domElement.addEventListener("dblclick", onDblClick);
  window.addEventListener("resize", onResize);

  // 监听根节点 class/属性变化,主题切换时重新配色
  applyTheme();
  themeObserver = new MutationObserver(() => applyTheme());
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "data-theme", "style"],
  });

  const loop = () => {
    raf = requestAnimationFrame(loop);
    controls.update();
    updateLabelLod();
    if (++minimapTick % 12 === 0) drawMinimap();
    renderer.render(scene, camera);
    labelRenderer.render(scene, camera);
  };
  loop();
}

function onResize() {
  if (!renderer || !canvasWrap.value) return;
  const w = canvasWrap.value.clientWidth;
  const h = canvasWrap.value.clientHeight;
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
  renderer.setSize(w, h);
  labelRenderer.setSize(w, h);
}

function disposeGraphObjects() {
  for (const m of Object.values(typeMeshes)) {
    scene.remove(m);
    m.geometry.dispose();
    m.material.dispose();
  }
  typeMeshes = {};
  if (edgeLines) {
    scene.remove(edgeLines);
    edgeLines.geometry.dispose();
    edgeLines.material.dispose();
    edgeLines = null;
  }
  for (const l of labelObjs) scene.remove(l);
  labelObjs = [];
}

function colorForNode(n) {
  if (n.community_id != null && communityColors.has(n.community_id)) {
    return new THREE.Color(communityColors.get(n.community_id));
  }
  return new THREE.Color(TYPE_COLORS[n.type] ?? TYPE_COLORS.unknown);
}

function buildMeshes() {
  disposeGraphObjects();

  // 按类型分组
  const byType = {};
  for (const n of nodeList) {
    (byType[n.type] = byType[n.type] || []).push(n);
  }

  const sphere = new THREE.SphereGeometry(1, 16, 16);
  for (const [type, arr] of Object.entries(byType)) {
    const mat = new THREE.MeshStandardMaterial({
      metalness: 0.25,
      roughness: 0.55,
      emissive: new THREE.Color(TYPE_COLORS[type] ?? TYPE_COLORS.unknown),
      emissiveIntensity: 0.12,
    });
    const mesh = new THREE.InstancedMesh(sphere, mat, arr.length);
    mesh.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(arr.length * 3), 3);
    mesh.userData.type = type;
    mesh.userData.nodes = arr;
    arr.forEach((n, i) => {
      n._type = type;
      n._inst = i;
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = baseRadius(n);
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
      mesh.setColorAt(i, colorForNode(n));
    });
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    scene.add(mesh);
    typeMeshes[type] = mesh;
  }

  buildEdges();
  applyTypeVisibility();
  rebuildLabels();
}

function baseRadius(n) {
  const s = n.size ?? 0.2;
  return 2 + s * 6;
}

function buildEdges() {
  const positions = [];
  for (const e of edgeList) {
    const a = curPositions.get(e.from);
    const b = curPositions.get(e.to);
    if (!a || !b) continue;
    positions.push(a.x, a.y, a.z, b.x, b.y, b.z);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const mat = new THREE.LineBasicMaterial({ color: themeColors.edge, transparent: true, opacity: 0.28, depthWrite: false });
  edgeLines = new THREE.LineSegments(geo, mat);
  scene.add(edgeLines);
}

// 仅给“重要 + 选中 + 近距”的节点显示标签,避免上万 DOM
function rebuildLabels() {
  for (const l of labelObjs) scene.remove(l);
  labelObjs = [];
  const important = [...nodeList]
    .sort((a, b) => (b.pagerank || 0) - (a.pagerank || 0))
    .slice(0, Math.min(60, nodeList.length));
  const set = new Set(important.map((n) => n.id));
  if (selected.value) set.add(selected.value.id);
  for (const n of nodeList) {
    if (!set.has(n.id)) continue;
    const p = curPositions.get(n.id);
    if (!p) continue;
    const div = document.createElement("div");
    div.className = "kg3d-label";
    div.textContent = n.name?.length > 18 ? n.name.slice(0, 18) + "…" : n.name;
    const obj = new CSS2DObject(div);
    obj.position.set(p.x, p.y + baseRadius(n) + 2, p.z);
    obj.userData.nodeId = n.id;
    scene.add(obj);
    labelObjs.push(obj);
  }
}

function updateLabelLod() {
  // 距离过远的标签隐藏
  for (const l of labelObjs) {
    const d = camera.position.distanceTo(l.position);
    l.element.style.opacity = d > 600 ? "0" : "0.92";
  }
}

function applyTypeVisibility() {
  for (const [type, mesh] of Object.entries(typeMeshes)) {
    const vis = visibleTypes.value.has(type);
    mesh.visible = vis;
  }
}

// ───────── 数据加载 ─────────
async function loadGraph(rootId = "kb_root", maxDepth = 99) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  if (!name) return;
  statusText.value = "加载图数据…";
  try {
    await store.kgEnsureIndex(name).catch(() => {});
    const opts = { root_id: rootId, max_depth: maxDepth, limit: 6000 };
    const data = await store.kgQueryGraph(name, opts);

    // 社区色
    communityColors = new Map();
    try {
      const comms = await store.kgCommunities(name);
      for (const c of comms || []) {
        if (c.color) communityColors.set(c.community_id, c.color);
      }
    } catch (e) {
      /* 无社区数据 */
    }

    nodeList = (data.nodes || []).map((n) => ({ ...n }));
    nodeById = new Map(nodeList.map((n) => [n.id, n]));
    edgeList = (data.edges || []).filter((e) => nodeById.has(e.from) && nodeById.has(e.to));

    curPositions = await resolveLayout(store, name, view.value, { nodes: nodeList, edges: edgeList });
    // 折叠态在重新加载时复位
    collapsed.value = false;
    fullNodeList = null;
    fullEdgeList = null;
    computeTimeBounds();
    buildMeshes();
    statusText.value = `节点 ${nodeList.length} · 边 ${edgeList.length}${data.truncated ? " (已截断)" : ""}`;
    fitCamera();
    drawMinimap();
  } catch (e) {
    statusText.value = "加载失败: " + (e?.message || e);
  }
}

function fitCamera() {
  if (!nodeList.length) return;
  const box = new THREE.Box3();
  for (const n of nodeList) {
    const p = curPositions.get(n.id);
    if (p) box.expandByPoint(tmpVec.set(p.x, p.y, p.z));
  }
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3()).length() || 200;
  gsap.to(controls.target, { x: center.x, y: center.y, z: center.z, duration: 0.8 });
  gsap.to(camera.position, {
    x: center.x,
    y: center.y + size * 0.3,
    z: center.z + size * 0.7,
    duration: 0.8,
  });
}

// ───────── 视角切换:坐标 morph ─────────
async function switchView() {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  if (!name) return;
  statusText.value = "切换视角…";
  // force 重算:避免历史缓存的旧布局导致节点挤作一团
  const target = await resolveLayout(store, name, view.value, { nodes: nodeList, edges: edgeList }, {}, { force: true });
  morphTo(target);
}

function morphTo(target) {
  const from = new Map(curPositions);
  const prog = { t: 0 };
  gsap.to(prog, {
    t: 1,
    duration: 1.1,
    ease: "power2.inOut",
    onUpdate() {
      for (const n of nodeList) {
        const a = from.get(n.id) || { x: 0, y: 0, z: 0 };
        const b = target.get(n.id) || a;
        curPositions.set(n.id, {
          x: a.x + (b.x - a.x) * prog.t,
          y: a.y + (b.y - a.y) * prog.t,
          z: a.z + (b.z - a.z) * prog.t,
        });
      }
      syncInstancePositions();
    },
    onComplete() {
      curPositions = target;
      syncInstancePositions();
      rebuildLabels();
      fitCamera(); // 视角切换后重新取景,适配新布局的尺度
      drawMinimap();
      statusText.value = "";
    },
  });
}

function syncInstancePositions() {
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = baseRadius(n) * (n._scaleMul ?? 1);
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
    });
    mesh.instanceMatrix.needsUpdate = true;
  }
  // 重新生成边
  if (edgeLines) {
    const pos = edgeLines.geometry.getAttribute("position");
    let k = 0;
    for (const e of edgeList) {
      const a = curPositions.get(e.from);
      const b = curPositions.get(e.to);
      if (!a || !b) continue;
      pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
      pos.array[k++] = b.x; pos.array[k++] = b.y; pos.array[k++] = b.z;
    }
    pos.needsUpdate = true;
  }
  for (const l of labelObjs) {
    const p = curPositions.get(l.userData.nodeId);
    const n = nodeById.get(l.userData.nodeId);
    if (p && n) l.position.set(p.x, p.y + baseRadius(n) + 2, p.z);
  }
}

// ───────── 交互:拾取 ─────────
function pickNode(ev) {
  const rect = renderer.domElement.getBoundingClientRect();
  const mouse = new THREE.Vector2(
    ((ev.clientX - rect.left) / rect.width) * 2 - 1,
    -((ev.clientY - rect.top) / rect.height) * 2 + 1
  );
  const ray = new THREE.Raycaster();
  ray.setFromCamera(mouse, camera);
  const meshes = Object.values(typeMeshes).filter((m) => m.visible);
  const hits = ray.intersectObjects(meshes, false);
  if (!hits.length) return null;
  const h = hits[0];
  const arr = h.object.userData.nodes;
  return arr[h.instanceId] || null;
}

async function onClick(ev) {
  const node = pickNode(ev);
  if (!node) return;
  if (pathMode.value) {
    pathPick.push(node.id);
    statusText.value = `路径起点已选 (${pathPick.length}/2)`;
    if (pathPick.length === 2) {
      await runShortestPath(pathPick[0], pathPick[1]);
      pathPick = [];
      pathMode.value = false;
    }
    return;
  }
  selectNode(node);
}

function selectNode(node) {
  selected.value = node;
  suggestions.value = [];
  tagSuggestions.value = [];
  highlightEgo(node.id);
  rebuildLabels();
  const p = curPositions.get(node.id);
  if (p) {
    gsap.to(controls.target, { x: p.x, y: p.y, z: p.z, duration: 0.6 });
  }
}

function clearSelection() {
  selected.value = null;
  resetHighlight();
  rebuildLabels();
}

async function onDblClick(ev) {
  const node = pickNode(ev);
  if (!node) return;
  // 折叠态:双击超级节点 → 展开该社区
  if (collapsed.value && typeof node.id === "string" && node.id.startsWith("super_")) {
    expandCommunity(node.community_id);
    return;
  }
  // 目录/概念双击 → 下钻
  if (node.type === "directory" || node.type === "knowledge-base") {
    breadcrumb.value.push({ id: node.id, label: node.name });
    await loadGraph(node.id, 3);
  }
}

async function drillTo(i) {
  breadcrumb.value = breadcrumb.value.slice(0, i + 1);
  const target = breadcrumb.value[i];
  await loadGraph(target.id, target.id === "kb_root" ? 99 : 3);
}

// ───────── 高亮 / ego ─────────
async function highlightEgo(nodeId) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  let egoIds = new Set([nodeId]);
  try {
    const ego = await store.kgNeighbors(name, nodeId, 1, 400);
    for (const n of ego.nodes || []) egoIds.add(n.id);
  } catch (e) {
    /* 退化:只高亮自身 */
  }
  applyHighlight(egoIds);
}

function applyHighlight(idSet) {
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const c = colorForNode(n);
      if (!idSet.has(n.id)) c.multiplyScalar(0.25);
      mesh.setColorAt(i, c);
    });
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }
}

function resetHighlight() {
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => mesh.setColorAt(i, colorForNode(n)));
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }
}

function focusEgo() {
  if (selected.value) highlightEgo(selected.value.id);
}

// ───────── 搜索 / 最短路 ─────────
async function runSearch() {
  const q = searchText.value.trim();
  if (!q) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  // 先尝试语义检索,失败/无 embedding 回退名称匹配
  let hitId = null;
  try {
    const res = await store.kgSemanticSearch(name, q, 5);
    if (res && res.length) hitId = res[0].id;
  } catch (e) {
    /* 回退 */
  }
  if (!hitId) {
    const lc = q.toLowerCase();
    const hit = nodeList.find((n) => (n.name || "").toLowerCase().includes(lc));
    hitId = hit?.id;
  }
  if (hitId && nodeById.has(hitId)) {
    selectNode(nodeById.get(hitId));
    flyTo(hitId);
  } else {
    statusText.value = "未找到匹配节点";
  }
}

function flyTo(nodeId) {
  const p = curPositions.get(nodeId);
  if (!p) return;
  gsap.to(controls.target, { x: p.x, y: p.y, z: p.z, duration: 0.8 });
  gsap.to(camera.position, { x: p.x + 40, y: p.y + 30, z: p.z + 60, duration: 0.8 });
}

async function runShortestPath(a, b) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    const res = await store.kgShortestPath(name, a, b);
    if (!res.found) {
      statusText.value = "两节点之间无连通路径";
      return;
    }
    const ids = new Set((res.nodes || []).map((n) => n.id));
    applyHighlight(ids);
    statusText.value = `最短路径:${res.hops} 跳`;
  } catch (e) {
    statusText.value = "路径查询失败";
  }
}

function togglePathMode() {
  pathMode.value = !pathMode.value;
  pathPick = [];
  statusText.value = pathMode.value ? "路径模式:依次点击两个节点" : "";
}

// ───────── 建议 / 确认 ─────────
async function suggestRelations() {
  if (!selected.value) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    suggestions.value = await store.kgSuggestRelations(name, selected.value.id, 8, 0.55);
    if (!suggestions.value.length) statusText.value = "暂无关联建议(可能未生成 embedding)";
  } catch (e) {
    statusText.value = "需要先生成 embedding:" + (e?.message || e);
  }
}

async function suggestTags() {
  if (!selected.value) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    tagSuggestions.value = await store.kgSuggestTags(name, selected.value.id, null, 6, 0.5);
    if (!tagSuggestions.value.length) statusText.value = "暂无标签建议";
  } catch (e) {
    statusText.value = "标签建议失败:" + (e?.message || e);
  }
}

async function confirmRelation(s) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  await store.kgConfirmRelation(name, s.from, s.to, "related", s.score);
  suggestions.value = suggestions.value.filter((x) => x.to !== s.to);
  statusText.value = "已建立关联";
}

async function confirmTag(s) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  await store.kgAddTag(name, selected.value.id, s.tag, "zeroshot", s.score);
  if (!selected.value.tags) selected.value.tags = [];
  selected.value.tags.push(s.tag);
  tagSuggestions.value = tagSuggestions.value.filter((x) => x.tag !== s.tag);
}

// ───────── 工具条动作 ─────────
function toggleType(t) {
  const next = new Set(visibleTypes.value);
  if (next.has(t)) next.delete(t);
  else next.add(t);
  visibleTypes.value = next;
  applyTypeVisibility();
}

async function rebuildIndexAndReload() {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  statusText.value = "重建索引…";
  try {
    await store.kgEnsureIndex(name);
    await loadGraph(breadcrumb.value[breadcrumb.value.length - 1].id);
  } catch (e) {
    statusText.value = "重建失败:" + (e?.message || e);
  }
}

async function runCommunities() {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  statusText.value = "社区检测中…";
  try {
    await store.kgDetectCommunities(name, 1.0);
    await loadGraph(breadcrumb.value[breadcrumb.value.length - 1].id);
    statusText.value = "社区着色完成";
  } catch (e) {
    statusText.value = "社区检测失败:" + (e?.message || e);
  }
}

async function runEmbed() {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  statusText.value = "生成向量中(首次会自动下载模型,可能较久)…";
  try {
    const r = await store.kgEmbedKb(name);
    statusText.value = `向量完成:新增 ${r.embedded} · 跳过 ${r.skipped} · 模型 ${r.model_key}`;
  } catch (e) {
    statusText.value = "向量生成失败:" + (e?.message || e);
  }
}

async function runExtract() {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  statusText.value = "AI 抽取概念与关系中…";
  try {
    const r = await store.kgExtractConcepts(name, null, 6, 30);
    await loadGraph(breadcrumb.value[breadcrumb.value.length - 1].id);
    statusText.value = `概念抽取完成:文件 ${r.files_processed} · 概念 ${r.concepts} · 关系 ${r.relations}`;
  } catch (e) {
    statusText.value = "概念抽取失败:" + (e?.message || e);
  }
}

// 点击小地图 → 主图相机平移到对应世界坐标
function onMinimapClick(ev) {
  if (!lastProj) return;
  const cv = minimap.value;
  const rect = cv.getBoundingClientRect();
  const cx = ((ev.clientX - rect.left) / rect.width) * lastProj.W;
  const cy = ((ev.clientY - rect.top) / rect.height) * lastProj.H;
  const worldX = lastProj.minX + (cx - lastProj.pad) / lastProj.s;
  const worldZ = lastProj.minZ + (cy - lastProj.pad) / lastProj.s;
  const ty = controls.target.y;
  const offset = camera.position.clone().sub(controls.target);
  gsap.to(controls.target, { x: worldX, y: ty, z: worldZ, duration: 0.6 });
  gsap.to(camera.position, {
    x: worldX + offset.x,
    y: ty + offset.y,
    z: worldZ + offset.z,
    duration: 0.6,
  });
}

// ───────── 小地图(俯视投影)─────────
function drawMinimap() {
  const cv = minimap.value;
  if (!cv || !nodeList.length) return;
  const ctx = cv.getContext("2d");
  const W = cv.width;
  const H = cv.height;
  ctx.clearRect(0, 0, W, H);
  ctx.fillStyle = "rgba(13,17,23,0.6)";
  ctx.fillRect(0, 0, W, H);

  let minX = Infinity, maxX = -Infinity, minZ = Infinity, maxZ = -Infinity;
  for (const n of nodeList) {
    const p = curPositions.get(n.id);
    if (!p) continue;
    if (p.x < minX) minX = p.x;
    if (p.x > maxX) maxX = p.x;
    if (p.z < minZ) minZ = p.z;
    if (p.z > maxZ) maxZ = p.z;
  }
  if (!isFinite(minX)) return;
  const pad = 8;
  const sx = (W - pad * 2) / Math.max(1, maxX - minX);
  const sz = (H - pad * 2) / Math.max(1, maxZ - minZ);
  const s = Math.min(sx, sz);
  lastProj = { minX, minZ, s, pad, W, H };
  const proj = (p) => ({
    x: pad + (p.x - minX) * s,
    y: pad + (p.z - minZ) * s,
  });

  for (const n of nodeList) {
    const p = curPositions.get(n.id);
    if (!p) continue;
    if (!visibleTypes.value.has(n.type) && n.type !== "knowledge-base") continue;
    const q = proj(p);
    const col =
      n.community_id != null && communityColors.has(n.community_id)
        ? communityColors.get(n.community_id)
        : typeColor(n.type);
    ctx.fillStyle = col;
    ctx.fillRect(q.x, q.y, 1.6, 1.6);
  }
  // 当前焦点
  const t = controls?.target;
  if (t) {
    const q = proj({ x: t.x, z: t.z });
    ctx.strokeStyle = "#1f6feb";
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(q.x, q.y, 6, 0, Math.PI * 2);
    ctx.stroke();
  }
}

// ───────── 时间轴回放 ─────────
function computeTimeBounds() {
  let min = Infinity, max = -Infinity;
  for (const n of nodeList) {
    const raw = n.modified_at || n.created_at;
    const t = raw ? Date.parse(raw) : NaN;
    if (Number.isFinite(t)) {
      if (t < min) min = t;
      if (t > max) max = t;
    }
  }
  timeBounds = isFinite(min) && max > min ? { min, max } : { min: 0, max: 1 };
  timelineValue.value = 100;
  timelineLabel.value = "全部";
}

function applyTimeline() {
  const frac = timelineValue.value / 100;
  const cutoff = timeBounds.min + (timeBounds.max - timeBounds.min) * frac;
  timelineLabel.value =
    timelineValue.value >= 100 ? "全部" : new Date(cutoff).toLocaleDateString();
  // 修改时间晚于 cutoff 的节点淡出(scale→0)
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const raw = n.modified_at || n.created_at;
      const t = raw ? Date.parse(raw) : NaN;
      const hidden = Number.isFinite(t) && t > cutoff && timelineValue.value < 100;
      n._scaleMul = hidden ? 0 : 1;
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = baseRadius(n) * n._scaleMul;
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
    });
    mesh.instanceMatrix.needsUpdate = true;
  }
}

function toggleTimelinePlay() {
  timelinePlaying.value = !timelinePlaying.value;
  if (timelinePlaying.value) {
    if (timelineValue.value >= 100) timelineValue.value = 0;
    timelinePlayTimer = setInterval(() => {
      timelineValue.value = Math.min(100, timelineValue.value + 2);
      applyTimeline();
      if (timelineValue.value >= 100) toggleTimelinePlay();
    }, 120);
  } else if (timelinePlayTimer) {
    clearInterval(timelinePlayTimer);
    timelinePlayTimer = null;
  }
}

// ───────── 社区折叠 / 展开(LOD)─────────
async function toggleCollapse() {
  if (collapsed.value) {
    // 恢复完整图
    if (fullNodeList) {
      nodeList = fullNodeList;
      edgeList = fullEdgeList;
      nodeById = new Map(nodeList.map((n) => [n.id, n]));
      fullNodeList = null;
      fullEdgeList = null;
    }
    collapsed.value = false;
    buildMeshes();
    fitCamera();
    return;
  }
  // 需要社区数据
  const hasComm = nodeList.some((n) => n.community_id != null);
  if (!hasComm) {
    statusText.value = "请先点「社区」做社区检测";
    return;
  }
  buildCollapsedGraph();
}

function buildCollapsedGraph() {
  fullNodeList = nodeList;
  fullEdgeList = edgeList;

  // 聚合:每社区一个超级节点,位置=成员质心,大小∝成员数
  const groups = new Map();
  for (const n of fullNodeList) {
    const c = n.community_id ?? -1;
    if (!groups.has(c)) groups.set(c, []);
    groups.get(c).push(n);
  }
  const superNodes = [];
  const centroid = new Map();
  for (const [c, members] of groups.entries()) {
    let sx = 0, sy = 0, sz = 0, k = 0;
    let label = members[0]?.name || `社区${c}`;
    let bestPr = -1;
    for (const m of members) {
      const p = curPositions.get(m.id);
      if (p) { sx += p.x; sy += p.y; sz += p.z; k++; }
      if ((m.pagerank || 0) > bestPr) { bestPr = m.pagerank || 0; label = m.name; }
    }
    const pos = k ? { x: sx / k, y: sy / k, z: sz / k } : { x: 0, y: 0, z: 0 };
    const id = `super_${c}`;
    centroid.set(id, pos);
    const color = communityColors.get(c);
    superNodes.push({
      id,
      type: "directory",
      name: `${label} (${members.length})`,
      size: Math.min(1.4, 0.3 + Math.log2(members.length + 1) * 0.18),
      community_id: c,
      _superColor: color,
      _members: members.map((m) => m.id),
      degree: members.length,
      pagerank: 0,
      tags: [],
    });
  }

  // 聚合边:社区间连接计数
  const edgeMap = new Map();
  const commOf = new Map(fullNodeList.map((n) => [n.id, n.community_id ?? -1]));
  for (const e of fullEdgeList) {
    const ca = commOf.get(e.from);
    const cb = commOf.get(e.to);
    if (ca == null || cb == null || ca === cb) continue;
    const key = ca < cb ? `${ca}|${cb}` : `${cb}|${ca}`;
    edgeMap.set(key, (edgeMap.get(key) || 0) + 1);
  }
  const superEdges = [];
  for (const [key, cnt] of edgeMap.entries()) {
    const [a, b] = key.split("|");
    superEdges.push({ from: `super_${a}`, to: `super_${b}`, strength: Math.min(1, cnt / 10), type: "aggregate" });
  }

  nodeList = superNodes;
  edgeList = superEdges;
  nodeById = new Map(nodeList.map((n) => [n.id, n]));
  for (const [id, p] of centroid.entries()) curPositions.set(id, p);
  collapsed.value = true;
  buildMeshes();
  fitCamera();
  statusText.value = `已折叠为 ${superNodes.length} 个社区超级节点(双击展开)`;
}

function expandCommunity(cid) {
  if (!fullNodeList) return;
  // 展开:恢复完整图并只保留该社区成员
  const members = fullNodeList.filter((n) => (n.community_id ?? -1) === cid);
  const ids = new Set(members.map((n) => n.id));
  nodeList = members;
  edgeList = fullEdgeList.filter((e) => ids.has(e.from) && ids.has(e.to));
  nodeById = new Map(nodeList.map((n) => [n.id, n]));
  collapsed.value = false;
  buildMeshes();
  fitCamera();
  statusText.value = `展开社区:${members.length} 个节点(点「折叠」返回总览)`;
}

onMounted(async () => {
  initThree();
  await loadGraph();
});

onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  if (timelinePlayTimer) clearInterval(timelinePlayTimer);
  if (themeObserver) themeObserver.disconnect();
  window.removeEventListener("resize", onResize);
  if (renderer) {
    renderer.domElement.removeEventListener("click", onClick);
    renderer.domElement.removeEventListener("dblclick", onDblClick);
    disposeGraphObjects();
    renderer.dispose();
  }
});

defineExpose({ reload: loadGraph });
</script>

<style scoped>
.kg3d-root {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: #0b0e14;
  color: #e6edf3;
  font-size: 13px;
}
.kg3d-canvas {
  position: absolute;
  inset: 0;
}
.kg3d-top {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 5;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 12px;
  background: linear-gradient(rgba(13, 17, 23, 0.78), rgba(13, 17, 23, 0.35));
  backdrop-filter: blur(6px);
}
.kg3d-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  row-gap: 6px;
}
.tb-left { display: flex; align-items: center; gap: 10px; min-width: 0; flex: 1 1 auto; overflow: hidden; }
.tb-title { font-weight: 700; white-space: nowrap; }
.tb-crumb { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tb-crumb a { cursor: pointer; opacity: 0.7; margin-right: 6px; }
.tb-crumb a:hover { opacity: 1; text-decoration: underline; }
.tb-right { display: flex; align-items: center; gap: 8px; flex: 0 0 auto; flex-wrap: wrap; justify-content: flex-end; }
.tb-search { width: 160px; padding: 4px 8px; border-radius: 6px; border: 1px solid #30363d; background: #161b22; color: #e6edf3; }
.tb-sel { padding: 4px 6px; border-radius: 6px; background: #161b22; color: #e6edf3; border: 1px solid #30363d; }
.tb-btn { padding: 4px 10px; border-radius: 6px; background: #161b22; color: #e6edf3; border: 1px solid #30363d; cursor: pointer; white-space: nowrap; }
.tb-btn.active { background: #1f6feb; border-color: #1f6feb; }
.kg3d-filters {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.flt { display: flex; align-items: center; gap: 4px; cursor: pointer; white-space: nowrap; }
.flt-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
.kg3d-panel {
  position: absolute;
  top: 96px;
  right: 12px;
  width: 280px;
  max-height: calc(100% - 120px);
  overflow: auto;
  z-index: 6;
  background: rgba(22, 27, 34, 0.92);
  border: 1px solid #30363d;
  border-radius: 10px;
  padding: 10px;
}
.pn-head { display: flex; align-items: center; gap: 6px; }
.pn-type { font-size: 11px; padding: 1px 6px; border-radius: 4px; color: #0b0e14; }
.pn-name { font-weight: 600; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pn-x { cursor: pointer; opacity: 0.6; }
.pn-meta { font-size: 11px; opacity: 0.7; margin: 6px 0; display: flex; flex-direction: column; gap: 2px; }
.pn-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.pn-tag { font-size: 11px; background: #21262d; padding: 1px 6px; border-radius: 10px; }
.pn-actions { display: flex; gap: 6px; }
.pn-actions button { flex: 1; padding: 4px; border-radius: 6px; background: #21262d; color: #e6edf3; border: 1px solid #30363d; cursor: pointer; }
.pn-suggest { margin-top: 8px; }
.pn-sub { font-size: 11px; opacity: 0.6; margin-bottom: 4px; }
.pn-srow { display: flex; align-items: center; gap: 6px; margin-bottom: 3px; }
.pn-sname { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.pn-sscore { font-size: 11px; opacity: 0.6; }
.pn-srow button { width: 22px; border-radius: 4px; background: #1f6feb; color: #fff; border: none; cursor: pointer; }
.kg3d-status {
  position: absolute;
  bottom: 10px;
  left: 12px;
  z-index: 5;
  font-size: 12px;
  opacity: 0.8;
  background: rgba(13, 17, 23, 0.6);
  padding: 4px 10px;
  border-radius: 6px;
}
.kg3d-minimap {
  position: absolute;
  right: 12px;
  bottom: 48px;
  z-index: 5;
  border: 1px solid #30363d;
  border-radius: 6px;
  background: rgba(13, 17, 23, 0.6);
  cursor: pointer;
}
.kg3d-timeline {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  bottom: 10px;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 8px;
  width: min(46%, 420px);
  background: rgba(13, 17, 23, 0.6);
  padding: 5px 10px;
  border-radius: 8px;
}
.tl-play {
  border: none;
  background: #21262d;
  color: #e6edf3;
  border-radius: 4px;
  width: 24px;
  height: 22px;
  cursor: pointer;
}
.tl-range { flex: 1; }
.tl-label { font-size: 11px; opacity: 0.75; min-width: 64px; text-align: right; }
:deep(.kg3d-label) {
  color: #c9d1d9;
  font-size: 11px;
  text-shadow: 0 1px 3px #000;
  pointer-events: none;
  white-space: nowrap;
}

/* ───────── 亮色模式覆盖 ───────── */
.kg3d-root.kg3d-light {
  color: #1f2328;
}
.kg3d-light .kg3d-top {
  background: linear-gradient(rgba(255, 255, 255, 0.9), rgba(255, 255, 255, 0.5));
}
.kg3d-light .tb-search,
.kg3d-light .tb-sel,
.kg3d-light .tb-btn {
  background: #ffffff;
  color: #1f2328;
  border-color: #d0d7de;
}
.kg3d-light .tb-btn.active {
  background: #1f6feb;
  color: #fff;
  border-color: #1f6feb;
}
.kg3d-light .kg3d-panel {
  background: rgba(255, 255, 255, 0.95);
  border-color: #d0d7de;
  color: #1f2328;
}
.kg3d-light .pn-tag,
.kg3d-light .pn-actions button {
  background: #eef1f4;
  color: #1f2328;
  border-color: #d0d7de;
}
.kg3d-light .kg3d-status,
.kg3d-light .kg3d-timeline,
.kg3d-light .kg3d-minimap {
  background: rgba(255, 255, 255, 0.85);
  color: #1f2328;
}
.kg3d-light .tl-play {
  background: #eef1f4;
  color: #1f2328;
}
.kg3d-light :deep(.kg3d-label) {
  color: #24292f;
  text-shadow: 0 1px 2px rgba(255, 255, 255, 0.8);
}
</style>
