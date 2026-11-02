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
          <select v-model="levelFilter" class="tb-sel" @change="onLevelChange" title="只看某一层级的节点">
            <option value="all">全部层级</option>
            <option value="dir">仅目录</option>
            <option value="file">到文件</option>
            <option value="section">到章节</option>
          </select>
          <select v-model="view" class="tb-sel" @change="switchView" title="换一种方式摆放知识点">
            <option value="relation">按关联摆放</option>
            <option value="tag">按主题分组</option>
            <option value="time">按时间排列</option>
            <option value="importance">按重要程度</option>
          </select>
          <button
            v-for="b in toolButtons"
            :key="b.key"
            class="tb-iconbtn"
            :class="{ active: b.active() }"
            :title="b.tip"
            :aria-label="b.label"
            @click="b.run"
          >
            <component :is="b.icon" class="tb-ico" />
          </button>
        </div>
      </div>

      <!-- 类型过滤 + 标签过滤 -->
      <div class="kg3d-filters">
        <label v-for="t in allTypes" :key="t.key" class="flt">
          <input type="checkbox" :checked="visibleTypes.has(t.key)" @change="toggleType(t.key)" />
          <span class="flt-dot" :style="{ background: t.color }"></span>{{ t.label }}
        </label>
        <div v-if="tagFilterOptions.length" class="tag-filter">
          <button class="tag-filter-btn" :class="{ active: selectedTagFilter.length }" @click="tagDropdownOpen = !tagDropdownOpen">
            标签筛选{{ selectedTagFilter.length ? `(${selectedTagFilter.length})` : "" }} ▾
          </button>
          <div v-if="tagDropdownOpen" class="tag-filter-menu">
            <div class="tag-filter-head">
              <span>按标签筛选(可多选)</span>
              <a v-if="selectedTagFilter.length" @click="selectedTagFilter = []; onTagFilterChange()">清除</a>
            </div>
            <label v-for="o in tagFilterOptions" :key="o.value" class="tag-filter-item">
              <input type="checkbox" :value="o.value" v-model="selectedTagFilter" @change="onTagFilterChange" />
              <span>{{ o.label }}</span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <div ref="canvasWrap" class="kg3d-canvas"></div>

    <!-- 小地图:俯视投影 + 当前焦点(可点击定位) -->
    <canvas ref="minimap" class="kg3d-minimap" width="180" height="140" @click="onMinimapClick"></canvas>

    <!-- 时间轴:拖动按修改时间过滤,回放知识增长 -->
    <div class="kg3d-timeline">
      <button class="tl-play" @click="toggleTimelinePlay">{{ timelinePlaying ? "⏸" : "▶" }}</button>
      <span class="tl-edge" :title="timelineRangeLabel.min">{{ shortDate(timelineRangeLabel.min) }}</span>
      <input
        type="range"
        min="0"
        max="100"
        v-model.number="timelineValue"
        class="tl-range"
        @input="applyTimeline"
      />
      <span class="tl-edge" :title="timelineRangeLabel.max">{{ shortDate(timelineRangeLabel.max) }}</span>
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
        <span v-if="selected.created_at">创建 {{ formatDate(selected.created_at) }}</span>
        <span v-if="selected.modified_at">修改 {{ formatDate(selected.modified_at) }}</span>
      </div>
      <div v-if="selected.tags && selected.tags.length" class="pn-tags">
        <span v-for="tg in selected.tags" :key="tg" class="pn-tag">
          #{{ tg }}<i class="pn-tagx" title="删除标签" @click="removeTag(tg)">×</i>
        </span>
      </div>
      <div class="pn-actions">
        <button @click="suggestRelations">推荐关联</button>
        <button @click="suggestTags">推荐标签</button>
        <button @click="focusEgo">聚焦邻居</button>
      </div>
      <div class="pn-actions" style="margin-top:6px;">
        <button v-if="canOpenNode(selected)" @click="openNodeInEditor(selected)">打开</button>
        <button @click="toggleBookmark(selected)">{{ isBookmarked(selected) ? '取消收藏' : '收藏' }}</button>
        <button @click="loadNodeRelations">关联列表</button>
      </div>
      <div v-if="nodeRelations.length" class="pn-suggest">
        <div class="pn-sub">已有关联(点名称高亮 · ×删除)</div>
        <div v-for="r in nodeRelations" :key="r.to + r.type" class="pn-srow">
          <span class="pn-sname pn-link" :title="r.to_name || r.to" @click="highlightPair(r.toId ?? r.to)">{{ r.to_name || r.to }}</span>
          <span class="pn-sscore">{{ r.type }}</span>
          <button v-if="r.deletable" class="pn-del" title="删除关系" @click="removeRelation(r)">×</button>
        </div>
      </div>
      <div v-if="suggestions.length" class="pn-suggest">
        <div class="pn-sub">关联建议(点名称预览高亮)</div>
        <div v-for="s in suggestions" :key="s.to" class="pn-srow">
          <span class="pn-sname pn-link" :title="s.to_name" @click="highlightPair(s.to)">{{ s.to_name }}</span>
          <span class="pn-sscore">{{ s.score.toFixed(2) }}</span>
          <button title="建立关联" @click="confirmRelation(s)">＋</button>
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
import { onMounted, onBeforeUnmount, onActivated, onDeactivated, ref, shallowRef, watch, markRaw, computed } from "vue";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { CSS2DRenderer, CSS2DObject } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import gsap from "gsap";
import { storeToRefs } from "pinia";
import {
  Airplane24Regular,
  Flowchart24Regular,
  Group24Regular,
  Sparkle24Regular,
  BrainCircuit24Regular,
  ArrowSync24Regular,
} from "@vicons/fluent";
import { useKnowledgeBaseStore } from "../../stores/knowledgeBaseStore";
import { useFileTreeStore } from "../../stores/fileTreeStore";
import { useEditorStore } from "../../stores/editorStore";
import { createLogger } from "../../utils/logger";
import { onKgIndexChanged } from "../../utils/kgIndexEvents";
import { resolveLayout, resolveLayoutImmediate, disposeLayoutWorker } from "./kgLayout";

const props = defineProps({
  kbName: { type: String, default: "" },
});

const store = useKnowledgeBaseStore();
const fileTreeStore = useFileTreeStore();
const editorStore = useEditorStore();
const { kg3dActionRequest } = storeToRefs(store);
const logger = createLogger("GuaxiKnowledgeGraph3D");

const TYPE_COLORS = {
  "knowledge-base": 0xf39c12,
  directory: 0x3498db,
  file: 0x2ecc71,
  section: 0x8e44ad,
  paragraph: 0xd35400,
  concept: 0xe84393,
  unknown: 0x95a5a6,
};
// 自发光强度按层级递减:根/顶层亮,越往叶子越微弱(球体本体的底光,星点光晕另算)
const TYPE_GLOW = {
  "knowledge-base": 0.45,
  directory: 0.3,
  file: 0.14,
  section: 0.1,
  paragraph: 0.09,
  concept: 0.18,
  unknown: 0.1,
};
// 节点基础半径:越靠顶层越大
const TYPE_RADIUS = {
  "knowledge-base": 11,
  directory: 6.5,
  file: 4,
  section: 3,
  paragraph: 2.4,
  concept: 4.5,
  unknown: 2.6,
};
const allTypes = [
  { key: "directory", label: "目录", color: "#3498db" },
  { key: "file", label: "文件", color: "#2ecc71" },
  { key: "section", label: "章节", color: "#8e44ad" },
  { key: "paragraph", label: "段落", color: "#d35400" },
  { key: "concept", label: "概念", color: "#e84393" },
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
const nodeRelations = ref([]);
const statusText = ref("");
const visibleTypes = shallowRef(new Set(["directory", "file", "section", "paragraph", "concept"]));
// 层级过滤:只显示到某一层为止的节点(目录/文件/章节/全部),用于在不同粒度间快速切换
const levelFilter = ref("all");
const LEVEL_TYPES = {
  all: null,
  dir: new Set(["knowledge-base", "directory"]),
  file: new Set(["knowledge-base", "directory", "file"]),
  section: new Set(["knowledge-base", "directory", "file", "section"]),
};
// 标签过滤:为空=不按标签过滤;非空=只显示带任一所选标签的节点
const selectedTagFilter = ref([]);
const tagFilterOptions = ref([]); // [{ label, value }],来自当前图的所有标签
const tagDropdownOpen = ref(false);
const breadcrumb = ref([{ id: "kb_root", label: "全库" }]);
const timelineValue = ref(100);
const timelineLabel = ref("全部");
const timelineRangeLabel = ref({ min: "", max: "" });
const timelinePlaying = ref(false);
let timelinePlayTimer = null;
let timeBounds = { min: 0, max: 1 };
// 折叠前保留的完整图,便于展开/恢复
let fullNodeList = null;
let fullEdgeList = null;
let minimapTick = 0;
// 每次加载递增。知识库快速切换时，旧请求即使后返回也不能覆盖当前图谱。
let graphLoadSeq = 0;
// 索引变更时若当前不宜重载(不可见/漫游中/回放中…),先记脏,恢复后再补一次。
let pendingReload = false;
let viewActive = true;

// three 运行时对象(非响应式)
let scene, camera, renderer, labelRenderer, controls, raf;
let typeMeshes = {}; // type -> InstancedMesh
let edgeLines = null;
let highlightEdges = null; // 高亮/路径边的发光叠加层
let highlightedIds = new Set(); // 当前强调的节点集合(用于 sync 时放大)
let glowGroup = null; // 高亮节点的发光光晕(Sprite,加色混合)
let starPoints = null; // 全体节点的星点光晕(加色混合 Points,营造"星星点点")
let starDust = null; // 远景静态星尘层,营造层次/景深的"繁星点点"氛围(不随节点移动)
let ambientLight = null; // 环境光引用,亮色模式下提升亮度让节点更通透
let glowSprites = []; // [{ sprite, nodeId, base, primary }]
let glowTexture = null; // 共享的径向渐变贴图
let glowPulse = 0; // 呼吸相位
let glowPrimaryId = null; // 当前主高亮节点(发光最强)
let nodeList = []; // [{...node, _pos:{x,y,z}, _type, _inst}]
let nodeById = new Map();
let edgeList = [];
let communityColors = new Map(); // community_id -> hex string
let labelObjs = [];
let pathPick = [];
let curPositions = new Map(); // node_id -> {x,y,z}
let themeColors = { bg: 0x0b0e14, edge: 0x4a5568, isDark: true };
let themeObserver = null;
let resizeObserver = null; // 监听画布容器尺寸变化(边栏展开/收缩)
let lastProj = null; // 小地图投影参数,用于点击反投影
// hover 高亮状态
let hovered = null; // { mesh, inst, node }
const tmpRay = new THREE.Raycaster();
const tmpMouse = new THREE.Vector2();
// 飞行模式(PointerLock 第一人称)
const flyMode = ref(false);
let flyKeys = { w: false, a: false, s: false, d: false, q: false, e: false, shift: false };
let flyYaw = 0;
let flyPitch = 0;
let sceneScale = 200; // 当前图谱包围盒尺度,用于退出飞行时设定缩放支点距离
let lastFrameTime = 0;
// 知识演化:进入演化态时记录"新出现"节点用于波纹高亮
let evolutionActive = false;
let prevVisibleIds = new Set();
const tmpMat = new THREE.Matrix4();
const tmpColor = new THREE.Color();
const tmpVec = new THREE.Vector3();
const tmpQuat = new THREE.Quaternion();
const tmpScale = new THREE.Vector3();

// 工具条按钮:图标 + 通俗提示。函数声明已被提升,这里引用是安全的。
const toolButtons = [
  {
    key: "fly",
    label: "自由漫游",
    tip: "自由漫游:用 W/A/S/D 在知识里穿行,鼠标控制看向哪里(按 Esc 退出)",
    icon: markRaw(Airplane24Regular),
    active: () => flyMode.value,
    run: () => toggleFlyMode(),
  },
  {
    key: "path",
    label: "找两点之间的路",
    tip: "找两点之间的路:依次点两个知识点,自动连出它们之间的最短关联路径并高亮",
    icon: markRaw(Flowchart24Regular),
    active: () => pathMode.value,
    run: () => togglePathMode(),
  },
  {
    key: "collapse",
    label: "按主题收拢",
    tip: "按主题收拢:把同一主题的知识点合成一个大球俯瞰全局(双击大球展开);未分组时会自动先分组",
    icon: markRaw(Group24Regular),
    active: () => collapsed.value,
    run: () => toggleCollapse(),
  },
  {
    key: "community",
    label: "自动分组上色",
    tip: "自动分组上色:把紧密相关的知识点归到同一主题,并用不同颜色区分",
    icon: markRaw(Sparkle24Regular),
    active: () => false,
    run: () => runCommunities(),
  },
  {
    key: "concept",
    label: "AI 提炼概念",
    tip: "AI 提炼概念:让 AI 从文档里找出关键概念和它们之间的关系,补进图里(需要时会自动先生成向量)",
    icon: markRaw(BrainCircuit24Regular),
    active: () => false,
    run: () => runExtract(),
  },
  {
    key: "rebuild",
    label: "重新扫描",
    tip: "重新扫描:重新读取知识库,刷新整张图",
    icon: markRaw(ArrowSync24Regular),
    active: () => false,
    run: () => rebuildIndexAndReload(),
  },
];

function typeColor(t) {
  return "#" + (TYPE_COLORS[t] ?? TYPE_COLORS.unknown).toString(16).padStart(6, "0");
}

// 节点时间展示(时间轴范围、选中面板共用)
function shortDate(v) {
  if (!v) return "";
  const d = new Date(v);
  if (!Number.isFinite(d.getTime())) return "";
  return `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function formatDate(v) {
  if (!v) return "";
  const d = new Date(v);
  if (!Number.isFinite(d.getTime())) return v;
  return d.toLocaleString();
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

// 按主题调整节点外观:修复亮色模式下饱和色 + 金属度在白底发黑发闷的问题。
// 亮色模式:metalness→0(无环境贴图时金属度会让球面发暗)、提高自发光、实例色略微提亮、增强环境光。
const _white = new THREE.Color(0xffffff);
function applyNodeThemeAppearance() {
  const isDark = themeColors.isDark;
  if (ambientLight) ambientLight.intensity = isDark ? 0.85 : 1.18;
  if (starDust) starDust.material.opacity = isDark ? 0.5 : 0.16;
  for (const [type, mesh] of Object.entries(typeMeshes)) {
    const baseGlow = TYPE_GLOW[type] ?? TYPE_GLOW.unknown;
    mesh.material.metalness = isDark ? 0.25 : 0.0;
    mesh.material.roughness = isDark ? 0.6 : 0.5;
    mesh.material.emissiveIntensity = isDark ? baseGlow : Math.min(0.6, baseGlow * 1.7 + 0.12);
    mesh.material.needsUpdate = true;
    const arr = mesh.userData.nodes || [];
    for (const n of arr) {
      const c = colorForNode(n);
      if (!isDark) c.lerp(_white, 0.12); // 向白轻微提亮,白底更通透
      mesh.setColorAt(n._inst, c);
    }
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }
}

function applyTheme() {
  themeColors = readThemeColors();
  if (scene) scene.background = new THREE.Color(themeColors.bg);
  if (edgeLines) edgeLines.material.color = new THREE.Color(themeColors.edge);
  // 切换浅/深色根类,驱动 CSS 覆盖层配色
  const root = canvasWrap.value?.closest(".kg3d-root");
  if (root) root.classList.toggle("kg3d-light", !themeColors.isDark);
  applyNodeThemeAppearance();
  requestRender();
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

  // 高 DPR 屏关抗锯齿省显存/带宽(节点是球体,锯齿不明显);独显优先
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  renderer = new THREE.WebGLRenderer({
    antialias: dpr < 1.5,
    powerPreference: "high-performance",
    stencil: false,
    depth: true,
  });
  renderer.setPixelRatio(dpr);
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
  // 任何相机交互(拖拽/缩放/平移)标脏,触发按需渲染
  controls.addEventListener("change", () => requestRender());
  // gsap 动画期间(相机飞行、morph、pop)持续标脏:只要有活跃 tween 就保持渲染
  gsap.ticker.add(gsapDirtyTick);

  ambientLight = new THREE.AmbientLight(0xffffff, 0.85);
  scene.add(ambientLight);
  const dir = new THREE.DirectionalLight(0xffffff, 0.6);
  dir.position.set(120, 200, 160);
  scene.add(dir);

  // 发光光晕层(加色混合 sprite),用于让选中/高亮节点真正"发光"
  glowTexture = makeGlowTexture();
  glowGroup = new THREE.Group();
  glowGroup.renderOrder = 998;
  scene.add(glowGroup);

  // 远景星尘层(在最底层,营造繁星点点的景深氛围;不随节点移动)
  buildStarDust();

  renderer.domElement.addEventListener("click", onClick);
  renderer.domElement.addEventListener("dblclick", onDblClick);
  renderer.domElement.addEventListener("mousemove", onPointerMove);
  window.addEventListener("resize", onResize);

  // 画布容器尺寸随边栏展开/收缩变化时,window.resize 不会触发,改用 ResizeObserver。
  if (typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => onResize());
    resizeObserver.observe(wrap);
  }

  // 监听根节点 class/属性变化,主题切换时重新配色
  applyTheme();
  themeObserver = new MutationObserver(() => applyTheme());
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "data-theme", "style"],
  });

  startLoop();
}

// ───────── 渲染循环:按需渲染 + 可暂停 ─────────
// dirtyUntil: 在此时间戳前持续渲染(交互/数据变更后给一小段窗口让动画走完);
// 持续需要渲染的状态(飞行/发光脉冲/时间轴播放)直接标脏。静止时只空跑 rAF 检查、不调用昂贵的 render。
let dirtyUntil = 0;
let loopRunning = false;

function requestRender(ms = 260) {
  // 用 performance.now 与循环里的 now 同源
  dirtyUntil = Math.max(dirtyUntil, (typeof performance !== "undefined" ? performance.now() : Date.now()) + ms);
}

// gsap ticker:只要全局时间轴上有活跃动画就持续标脏(覆盖相机飞行/morph/pop 等)
function gsapDirtyTick() {
  if (gsap.globalTimeline.getChildren(true, true, false).length > 0) {
    requestRender(120);
  }
}

function startLoop() {
  if (loopRunning) return;
  loopRunning = true;
  lastFrameTime = 0;
  requestRender(300); // 切回/初始化先渲染一段
  const loop = (now) => {
    if (!loopRunning) return;
    raf = requestAnimationFrame(loop);
    const dt = lastFrameTime ? Math.min(0.05, (now - lastFrameTime) / 1000) : 0.016;
    lastFrameTime = now;

    let dirty = now < dirtyUntil;
    if (flyMode.value) {
      updateFlight(dt);
      dirty = true;
    } else {
      controls.update(); // 阻尼/惯性靠 'change' 事件已标脏的 dirtyUntil 窗口覆盖
    }
    if (glowSprites.length) {
      updateGlow(dt); // 发光脉冲是连续动画
      dirty = true;
    }
    if (timelinePlaying.value) dirty = true;

    if (!dirty) return; // 静止:跳过昂贵的渲染,仅保留极廉价的 rAF 检查

    updateLabelLod();
    if (++minimapTick % 12 === 0) drawMinimap();
    renderer.render(scene, camera);
    labelRenderer.render(scene, camera);
  };
  raf = requestAnimationFrame(loop);
}

function stopLoop() {
  loopRunning = false;
  if (raf) {
    cancelAnimationFrame(raf);
    raf = null;
  }
}

// 径向渐变光晕贴图(中心亮、边缘柔和透明,过渡更顺滑)
function makeGlowTexture() {
  const size = 128;
  const cv = document.createElement("canvas");
  cv.width = cv.height = size;
  const ctx = cv.getContext("2d");
  const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  g.addColorStop(0, "rgba(255,255,255,1)");
  g.addColorStop(0.18, "rgba(255,255,255,0.82)");
  g.addColorStop(0.40, "rgba(255,255,255,0.42)");
  g.addColorStop(0.68, "rgba(255,255,255,0.14)");
  g.addColorStop(1, "rgba(255,255,255,0)");
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(cv);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

// 远景星尘:在大半径球壳内散布的微弱柔光点,提供景深层次的"繁星点点"。
// 静态、不随节点移动;renderOrder 最底、depthWrite=false,绝不遮挡节点与标签。
function buildStarDust() {
  if (starDust || !glowTexture) return;
  const N = 1300;
  const pos = new Float32Array(N * 3);
  const col = new Float32Array(N * 3);
  const siz = new Float32Array(N);
  for (let i = 0; i < N; i++) {
    const r = 700 + Math.random() * 1600;
    const theta = Math.random() * Math.PI * 2;
    const phi = Math.acos(2 * Math.random() - 1);
    pos[i * 3] = r * Math.sin(phi) * Math.cos(theta);
    pos[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
    pos[i * 3 + 2] = r * Math.cos(phi);
    // 冷白到淡蓝的细微色温变化,星点明暗错落
    const tint = 0.45 + Math.random() * 0.55;
    col[i * 3] = tint * 0.78;
    col[i * 3 + 1] = tint * 0.86;
    col[i * 3 + 2] = tint;
    siz[i] = 0.5 + Math.random() * 1.6;
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setAttribute("aSize", new THREE.BufferAttribute(siz, 1));
  const mat = new THREE.PointsMaterial({
    map: glowTexture,
    size: 5,
    sizeAttenuation: true,
    vertexColors: true,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    opacity: themeColors.isDark ? 0.5 : 0.16,
  });
  applyPerPointSize(mat);
  starDust = new THREE.Points(geo, mat);
  starDust.renderOrder = 0; // 最底层
  starDust.frustumCulled = false;
  scene.add(starDust);
}

// 让 PointsMaterial 支持逐点尺寸(读取 aSize 属性,乘到 gl_PointSize 上),实现近大远小的层次。
function applyPerPointSize(mat) {
  mat.onBeforeCompile = (shader) => {
    shader.vertexShader =
      "attribute float aSize;\n" +
      shader.vertexShader.replace("gl_PointSize = size;", "gl_PointSize = size * aSize;");
  };
}

// 为高亮集重建光晕 sprite(主节点用更亮的青色,其余用节点本色)
function rebuildGlow() {
  if (!glowGroup) return;
  for (const g of glowSprites) glowGroup.remove(g.sprite);
  for (const g of glowSprites) g.sprite.material.dispose();
  glowSprites = [];
  if (!highlightedIds.size) return;
  for (const id of highlightedIds) {
    const n = nodeById.get(id);
    const p = curPositions.get(id);
    if (!n || !p) continue;
    const isPrimary = id === glowPrimaryId;
    const col = isPrimary ? new THREE.Color(0x6fe3ff) : colorForNode(n).lerp(new THREE.Color(0xffffff), 0.35);
    const mat = new THREE.SpriteMaterial({
      map: glowTexture,
      color: col,
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      depthTest: false,
      opacity: 0.9,
    });
    const sprite = new THREE.Sprite(mat);
    sprite.position.set(p.x, p.y, p.z);
    const base = baseRadius(n) * (isPrimary ? 9 : 6);
    sprite.scale.set(base, base, 1);
    glowGroup.add(sprite);
    glowSprites.push({ sprite, nodeId: id, base, primary: isPrimary });
  }
}

// 呼吸脉动 + 跟随节点位置(morph 时也对齐)
function updateGlow(dt) {
  if (!glowSprites.length) return;
  glowPulse += dt * 3.2;
  const pulse = 1 + Math.sin(glowPulse) * 0.16;
  for (const g of glowSprites) {
    const p = curPositions.get(g.nodeId);
    if (p) g.sprite.position.set(p.x, p.y, p.z);
    const s = g.base * pulse;
    g.sprite.scale.set(s, s, 1);
    g.sprite.material.opacity = (g.primary ? 0.95 : 0.75) * (0.82 + 0.18 * Math.sin(glowPulse));
  }
}

function onResize() {
  if (!renderer || !canvasWrap.value) return;
  const w = canvasWrap.value.clientWidth;
  const h = canvasWrap.value.clientHeight;
  if (w === 0 || h === 0) return; // 容器被折叠为 0,跳过避免相机比例 NaN
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
  renderer.setSize(w, h);
  labelRenderer.setSize(w, h);
  requestRender();
}

function disposeGraphObjects() {
  if (!scene) return;
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
  if (starPoints) {
    scene.remove(starPoints);
    starPoints.geometry.dispose();
    starPoints.material.dispose();
    starPoints = null;
  }
  if (highlightEdges) {
    scene.remove(highlightEdges);
    highlightEdges.geometry.dispose();
    highlightEdges.material.dispose();
    highlightEdges = null;
  }
  if (glowGroup) {
    for (const g of glowSprites) {
      glowGroup.remove(g.sprite);
      g.sprite.material.dispose();
    }
  }
  glowSprites = [];
  glowPrimaryId = null;
  highlightedIds = new Set();
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
      roughness: 0.6,
      emissive: new THREE.Color(TYPE_COLORS[type] ?? TYPE_COLORS.unknown),
      // 发光强度按层级递减:根/顶层亮,叶子节点只微微发光
      emissiveIntensity: TYPE_GLOW[type] ?? TYPE_GLOW.unknown,
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
  buildStarField();
  applyNodeThemeAppearance(); // 按当前主题校正节点材质/配色(修复亮色发黑)
  applyTypeVisibility(); // 内部会重建标签,无需再单独调用
  requestRender();
}

// 星点光晕:全体节点叠加一层加色混合的柔光点,营造"星星点点";根/顶层更亮,叶子很微弱。
// 体量小且在标签下方,不会盖住标签文字。
function starBrightness(node) {
  if (node._hidden) return 0; // 类型/标签过滤隐藏
  if (node._scaleMul === 0) return 0; // 时间轴隐藏
  return 0.32 + (TYPE_GLOW[node.type] ?? 0.1) * 1.5;
}

// 星点尺寸按层级分档:根/顶层更大更亮,叶子更小,营造层次感。
function starSize(node) {
  return 0.75 + (TYPE_GLOW[node.type] ?? 0.1) * 3.2;
}

function buildStarField() {
  if (starPoints) {
    scene.remove(starPoints);
    starPoints.geometry.dispose();
    starPoints.material.dispose();
    starPoints = null;
  }
  const n = nodeList.length;
  if (!n || !glowTexture) return;
  const pos = new Float32Array(n * 3);
  const col = new Float32Array(n * 3);
  const siz = new Float32Array(n);
  const tmp = new THREE.Color();
  nodeList.forEach((node, i) => {
    node._starIdx = i;
    const p = curPositions.get(node.id) || { x: 0, y: 0, z: 0 };
    pos[i * 3] = p.x; pos[i * 3 + 1] = p.y; pos[i * 3 + 2] = p.z;
    tmp.copy(colorForNode(node));
    const b = starBrightness(node);
    col[i * 3] = tmp.r * b; col[i * 3 + 1] = tmp.g * b; col[i * 3 + 2] = tmp.b * b;
    siz[i] = starSize(node);
  });
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
  geo.setAttribute("aSize", new THREE.BufferAttribute(siz, 1));
  const mat = new THREE.PointsMaterial({
    map: glowTexture,
    size: 11,
    sizeAttenuation: true,
    vertexColors: true,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    opacity: 0.82,
  });
  applyPerPointSize(mat); // 逐点尺寸,实现层级近大远小
  starPoints = new THREE.Points(geo, mat);
  starPoints.renderOrder = 1; // 在球体之上叠加柔光,但 depthWrite=false 不遮挡
  scene.add(starPoints);
}

function updateStarPositions() {
  if (!starPoints) return;
  const attr = starPoints.geometry.getAttribute("position");
  nodeList.forEach((node, i) => {
    const p = curPositions.get(node.id) || { x: 0, y: 0, z: 0 };
    attr.array[i * 3] = p.x; attr.array[i * 3 + 1] = p.y; attr.array[i * 3 + 2] = p.z;
  });
  attr.needsUpdate = true;
}

function updateStarColors() {
  if (!starPoints) return;
  const attr = starPoints.geometry.getAttribute("color");
  const tmp = new THREE.Color();
  nodeList.forEach((node, i) => {
    tmp.copy(colorForNode(node));
    const b = starBrightness(node);
    attr.array[i * 3] = tmp.r * b; attr.array[i * 3 + 1] = tmp.g * b; attr.array[i * 3 + 2] = tmp.b * b;
  });
  attr.needsUpdate = true;
  requestRender();
}

function baseRadius(n) {
  const t = n.type || n._type;
  if (t && TYPE_RADIUS[t] != null) return TYPE_RADIUS[t];
  return 2 + (n.size ?? 0.2) * 6;
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
  // 候选池放大到 300:按 pagerank 排序并记录名次,名次用于"缩放自适应"显隐(updateLabelLod)。
  // 远观只露重要节点,拉近后越来越多节点的标签渐次浮现,兼顾"每个节点都能看到名字"与性能。
  const ranked = [...nodeList].sort((a, b) => (b.pagerank || 0) - (a.pagerank || 0));
  const cap = Math.min(300, ranked.length);
  const rankById = new Map();
  for (let i = 0; i < cap; i++) rankById.set(ranked[i].id, i);
  if (selected.value && !rankById.has(selected.value.id)) rankById.set(selected.value.id, 0);
  for (const id of highlightedIds) if (!rankById.has(id)) rankById.set(id, 0);
  for (const n of nodeList) {
    if (!rankById.has(n.id)) continue;
    if (n._hidden) continue; // 被过滤掉的节点不显示标签
    const p = curPositions.get(n.id);
    if (!p) continue;
    const hl = highlightedIds.has(n.id);
    const div = document.createElement("div");
    div.className = hl ? "kg3d-label kg3d-label-hl" : "kg3d-label";
    // 类型/社区色圆点 + 文本,胶囊样式由 CSS 承担
    const dot = document.createElement("span");
    dot.className = "kg3d-label-dot";
    dot.style.background = colorForNode(n).getStyle();
    const txt = document.createElement("span");
    txt.className = "kg3d-label-txt";
    txt.textContent = n.name?.length > 18 ? n.name.slice(0, 18) + "…" : n.name;
    div.appendChild(dot);
    div.appendChild(txt);
    const obj = new CSS2DObject(div);
    const r = baseRadius(n) * (n._hlMul ?? 1);
    obj.position.set(p.x, p.y + r + 5, p.z);
    obj.userData.nodeId = n.id;
    obj.userData.hl = hl;
    obj.userData.rank = rankById.get(n.id);
    scene.add(obj);
    labelObjs.push(obj);
  }
}

function updateLabelLod() {
  // 缩放自适应:每个标签按"相机距离 + 重要度名次"决定可见度。
  // 重要节点(名次小)可见距离更远;次要节点要相机拉近才浮现。强调标签恒显。
  for (const l of labelObjs) {
    if (l.userData.hl) { l.element.style.opacity = "1"; continue; }
    const d = camera.position.distanceTo(l.position);
    const rank = l.userData.rank ?? 0;
    const threshold = 1500 / (1 + rank * 0.07);
    let op = 0;
    if (d < threshold) {
      // 末段 20% 做渐隐过渡,避免标签硬切闪烁
      const fadeStart = threshold * 0.8;
      op = d <= fadeStart ? 0.95 : 0.95 * (1 - (d - fadeStart) / (threshold * 0.2));
    }
    l.element.style.opacity = op.toFixed(3);
  }
}

// 节点是否通过当前过滤(层级 + 类型 + 标签)。知识库根节点始终保留为骨架。
function nodePassesFilter(n) {
  if (n.type === "knowledge-base") return true;
  const levelTypes = LEVEL_TYPES[levelFilter.value];
  if (levelTypes && !levelTypes.has(n.type)) return false;
  if (!visibleTypes.value.has(n.type)) return false;
  const tags = selectedTagFilter.value;
  if (tags.length) {
    const nt = Array.isArray(n.tags) ? n.tags : [];
    if (!tags.some((t) => nt.includes(t))) return false;
  }
  return true;
}

// 应用过滤:隐藏的节点缩到 0(而非整组 mesh.visible),边只连两端都可见的节点。
function applyTypeVisibility() {
  const visIds = new Set();
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const ok = nodePassesFilter(n);
      n._hidden = !ok;
      if (ok) visIds.add(n.id);
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = ok ? baseRadius(n) * (n._scaleMul ?? 1) * (n._hlMul ?? 1) : 0;
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
    });
    mesh.instanceMatrix.needsUpdate = true;
    mesh.visible = true; // 始终渲染,靠缩放隐藏个别节点
  }
  rebuildEdgesForVisible(visIds);
  updateStarColors();
  rebuildLabels();
  requestRender();
}

// 重建主边集合:跳过任一端点被过滤掉的边
function rebuildEdgesForVisible(visIds) {
  if (!edgeLines) return;
  const pos = edgeLines.geometry.getAttribute("position");
  let k = 0;
  for (const e of edgeList) {
    const a = curPositions.get(e.from);
    const b = curPositions.get(e.to);
    if (!a || !b) continue;
    const show = visIds.has(e.from) && visIds.has(e.to);
    if (show) {
      pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
      pos.array[k++] = b.x; pos.array[k++] = b.y; pos.array[k++] = b.z;
    } else {
      // 收成零长度线段,等价隐藏
      pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
      pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
    }
  }
  pos.needsUpdate = true;
}

// ───────── 数据加载 ─────────
function resetGraphStateForKnowledgeBase(kbName) {
  if (morphTween) {
    morphTween.kill();
    morphTween = null;
  }
  if (timelinePlayTimer) {
    clearInterval(timelinePlayTimer);
    timelinePlayTimer = null;
  }
  if (flyMode.value) exitFlyMode();

  // 设计说明：切换知识库等价进入一张全新的图谱，不能继承旧库的选中节点、折叠态、标签筛选或时间轴状态。
  selected.value = null;
  suggestions.value = [];
  tagSuggestions.value = [];
  nodeRelations.value = [];
  store.setKg3dSelected(null);
  store.setKg3dRelations([]);
  store.setKg3dSuggestions([]);

  searchText.value = "";
  selectedTagFilter.value = [];
  tagFilterOptions.value = [];
  tagDropdownOpen.value = false;
  visibleTypes.value = new Set(["directory", "file", "section", "paragraph", "concept"]);
  levelFilter.value = "all";
  breadcrumb.value = [{ id: "kb_root", label: "全库" }];
  timelineValue.value = 100;
  timelineLabel.value = "全部";
  timelineRangeLabel.value = { min: "", max: "" };
  timelinePlaying.value = false;
  evolutionActive = false;
  prevVisibleIds = new Set();
  pathMode.value = false;
  pathPick = [];
  collapsed.value = false;
  pendingReload = false;
  fullNodeList = null;
  fullEdgeList = null;
  highlightedIds = new Set();
  glowPrimaryId = null;

  nodeList = [];
  nodeById = new Map();
  edgeList = [];
  communityColors = new Map();
  curPositions = new Map();
  disposeGraphObjects();
  drawMinimap();
  statusText.value = kbName ? `切换到知识库: ${kbName}` : "";
  logger.info("[kg3d] reset graph state for knowledge base switch", { kbName });
  requestRender();
}

// opts.ensureIndex:是否在查询前强制全量重建索引(kg_build_index)。
// 默认 false:交给后端 build_if_needed 惰性判断 —— 索引干净时直接命中缓存。
// 只有「重新扫描」按钮才值得无条件全量重建(数千个文件时要好几秒)。
async function loadGraph(rootId = "kb_root", maxDepth = 99, options = {}) {
  const { ensureIndex = false } = options;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  const loadSeq = ++graphLoadSeq;
  if (!name) {
    resetGraphStateForKnowledgeBase("");
    return;
  }
  const t0 = performance.now();
  statusText.value = ensureIndex ? "重新扫描索引…" : "加载图数据…";
  logger.info("[kg3d] start loading graph", { kbName: name, rootId, maxDepth, ensureIndex, loadSeq });
  try {
    if (ensureIndex) {
      await store.kgEnsureIndex(name).catch(() => {});
      if (loadSeq !== graphLoadSeq) return;
      statusText.value = "加载图数据…";
    }
    const opts = { root_id: rootId, max_depth: maxDepth, limit: 6000 };
    // 图数据与社区配色互不依赖,并行请求缩短首屏等待。
    const [data, comms] = await Promise.all([
      store.kgQueryGraph(name, opts),
      store.kgCommunities(name).catch(() => []),
    ]);
    if (loadSeq !== graphLoadSeq) return;

    // 社区色
    const nextCommunityColors = new Map();
    for (const c of comms || []) {
      if (c.color) nextCommunityColors.set(c.community_id, c.color);
    }

    const nextNodeList = (data.nodes || []).map((n) => ({ ...n }));
    const nextNodeById = new Map(nextNodeList.map((n) => [n.id, n]));
    const nextEdgeList = (data.edges || []).filter((e) => nextNodeById.has(e.from) && nextNodeById.has(e.to));

    // 先用缓存/本地补位把图立刻画出来;力导向细化放到后台,首屏不再等布局计算。
    const { pos: nextPositions, needsForce } = await resolveLayoutImmediate(
      store,
      name,
      view.value,
      { nodes: nextNodeList, edges: nextEdgeList }
    );
    if (loadSeq !== graphLoadSeq) return;

    // 新数据即将接管画布:停掉上一张图的布局转场,避免旧 tween 覆盖新坐标。
    if (morphTween) {
      morphTween.kill();
      morphTween = null;
    }
    communityColors = nextCommunityColors;
    nodeList = nextNodeList;
    nodeById = nextNodeById;
    edgeList = nextEdgeList;
    curPositions = nextPositions;
    // 折叠态在重新加载时复位
    collapsed.value = false;
    fullNodeList = null;
    fullEdgeList = null;
    computeTimeBounds();
    refreshTagFilterOptions();
    buildMeshes();
    fitCamera();
    drawMinimap();
    const ms = Math.round(performance.now() - t0);
    statusText.value = `节点 ${nodeList.length} · 边 ${edgeList.length} · ${ms} ms${data.truncated ? " (已截断)" : ""}`;
    logger.info("[kg3d] graph loaded", {
      kbName: name,
      nodes: nodeList.length,
      edges: edgeList.length,
      truncated: Boolean(data.truncated),
      ms,
      needsForce,
      loadSeq
    });
    if (needsForce) optimizeLayoutInBackground(name, loadSeq);
  } catch (e) {
    if (loadSeq !== graphLoadSeq) return;
    statusText.value = "加载失败: " + (e?.message || e);
    logger.error("[kg3d] load graph failed", { kbName: name, error: e });
  }
}

// 首屏已用缓存/补位坐标渲染,这里只把更规整的力导向结果补上并平滑过渡。
function optimizeLayoutInBackground(name, loadSeq) {
  statusText.value += " · 后台优化布局…";
  store.kgComputeLayout(name, "relation", false)
    .then((points) => {
      if (loadSeq !== graphLoadSeq || view.value !== "relation") return;
      if (!Array.isArray(points) || !points.length) return;
      const target = new Map();
      for (const p of points) {
        const id = p.node_id ?? p.id;
        if (id != null) target.set(id, { x: p.x, y: p.y, z: p.z });
      }
      morphTo(target, `布局优化完成 · 节点 ${nodeList.length}`);
    })
    .catch(() => {
      /* 后台优化失败不影响已渲染的图 */
    });
}

// ───────── 索引变更自动跟随 ─────────
// 主界面在知识库里新建/删除/移动文件后,后端广播 kg_index_changed。
// 可见且不在"会被打断"的交互态时防抖重载;否则记脏,等切回/退出该交互时补一次。
//
// 正在漫游、连线选点、演化回放或社区折叠态时重载会打断用户操作,推迟到之后再补。
function autoReloadBlockedReason() {
  if (!viewActive || document.hidden) return "hidden";
  if (flyMode.value) return "fly-mode";
  if (pathMode.value) return "path-mode";
  if (timelinePlaying.value) return "timeline-playing";
  if (collapsed.value) return "collapsed";
  return null;
}

async function autoReloadGraph(reason) {
  const blocked = autoReloadBlockedReason();
  if (blocked) {
    pendingReload = true;
    logger.info("[kg3d] 索引变更,推迟重载", { reason, blocked });
    return;
  }
  pendingReload = false;
  const keepId = selected.value?.id || null;
  const rootId = breadcrumb.value[breadcrumb.value.length - 1]?.id || "kb_root";
  const maxDepth = rootId === "kb_root" ? 99 : 3;
  // 静默重载不该搬动用户正在看的视角:先记下相机与支点,重载后覆盖掉 fitCamera 的取景动画。
  const camPos = camera ? camera.position.clone() : null;
  const camTarget = controls ? controls.target.clone() : null;
  logger.info("[kg3d] 索引变更,自动重载图谱", { reason, rootId, keepId });
  await loadGraph(rootId, maxDepth, { ensureIndex: false });
  // 尽量恢复重载前的选中节点;节点已不存在(被删/改名)则清空面板。
  if (keepId) {
    const again = nodeById.get(keepId);
    if (again) selectNode(again, { frame: false });
    else clearSelection();
  }
  if (camPos && camTarget) {
    gsap.killTweensOf(camera.position);
    gsap.killTweensOf(controls.target);
    camera.position.copy(camPos);
    controls.target.copy(camTarget);
    controls.update();
    requestRender();
  }
}

function flushPendingReload(reason) {
  if (!pendingReload) return;
  if (autoReloadBlockedReason()) return;
  autoReloadGraph(reason);
}

function fitCamera() {
  if (!nodeList.length) return;
  const box = new THREE.Box3();
  for (const n of nodeList) {
    if (n._hidden) continue; // 只看当前层级/过滤后真正可见的节点
    const p = curPositions.get(n.id);
    if (p) box.expandByPoint(tmpVec.set(p.x, p.y, p.z));
  }
  if (box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3()).length() || 200;
  sceneScale = size; // 记录当前图谱尺度,供退出飞行时设定合理的缩放支点距离
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
  if (view.value === "relation") {
    // 按关联视角:先用既有布局/补位立刻换过去,力导向细化交给后台
    const { pos, needsForce } = await resolveLayoutImmediate(store, name, view.value, {
      nodes: nodeList,
      edges: edgeList,
    });
    morphTo(pos);
    if (needsForce) optimizeLayoutInBackground(name, graphLoadSeq);
    return;
  }
  // 其它视角的布点很轻,直接本地算完
  const target = await resolveLayout(store, name, view.value, { nodes: nodeList, edges: edgeList }, {}, { force: true });
  morphTo(target);
}

// 布局转场 tween(后台力导向结果可能在切换视角动画中途返回,先停掉上一段避免两份坐标互相打架)
let morphTween = null;

function morphTo(target, doneText = "") {
  if (morphTween) {
    morphTween.kill();
    morphTween = null;
  }
  const from = new Map(curPositions);
  const prog = { t: 0 };
  morphTween = gsap.to(prog, {
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
      morphTween = null;
      curPositions = target;
      syncInstancePositions();
      rebuildLabels();
      fitCamera(); // 视角切换后重新取景,适配新布局的尺度
      drawMinimap();
      statusText.value = doneText;
    },
  });
}

function syncInstancePositions() {
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      // 被层级/类型/时间过滤掉的节点保持隐藏,不能因为位置同步又冒出来
      const s = n._hidden ? 0 : baseRadius(n) * (n._scaleMul ?? 1) * (n._hlMul ?? 1);
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
    if (p && n) l.position.set(p.x, p.y + baseRadius(n) + 5, p.z);
  }
  updateStarPositions();
  requestRender();
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

// ───────── 交互:hover 高亮 ─────────
function onPointerMove(ev) {
  if (flyMode.value) return; // 飞行模式下鼠标用于转向,不做拾取
  const rect = renderer.domElement.getBoundingClientRect();
  tmpMouse.set(
    ((ev.clientX - rect.left) / rect.width) * 2 - 1,
    -((ev.clientY - rect.top) / rect.height) * 2 + 1
  );
  tmpRay.setFromCamera(tmpMouse, camera);
  const meshes = Object.values(typeMeshes).filter((m) => m.visible);
  const hits = tmpRay.intersectObjects(meshes, false);
  const hit = hits.length ? hits[0] : null;
  const node = hit ? hit.object.userData.nodes[hit.instanceId] : null;

  if (hovered && (!node || hovered.node.id !== node.id)) {
    restoreInstanceScale(hovered.mesh, hovered.inst, hovered.node);
    hovered = null;
    requestRender();
  }
  if (node && hit && (!hovered || hovered.node.id !== node.id)) {
    hovered = { mesh: hit.object, inst: hit.instanceId, node };
    bumpInstanceScale(hit.object, hit.instanceId, node, 1.6);
    requestRender();
  }
  renderer.domElement.style.cursor = node ? "pointer" : flyMode.value ? "none" : "default";
}

function bumpInstanceScale(mesh, inst, node, mul) {
  const p = curPositions.get(node.id) || { x: 0, y: 0, z: 0 };
  const s = baseRadius(node) * (node._scaleMul ?? 1) * mul;
  tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
  mesh.setMatrixAt(inst, tmpMat);
  mesh.instanceMatrix.needsUpdate = true;
  const c = colorForNode(node).multiplyScalar(1.6);
  mesh.setColorAt(inst, c);
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
}

function restoreInstanceScale(mesh, inst, node) {
  const p = curPositions.get(node.id) || { x: 0, y: 0, z: 0 };
  const s = baseRadius(node) * (node._scaleMul ?? 1);
  tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
  mesh.setMatrixAt(inst, tmpMat);
  mesh.instanceMatrix.needsUpdate = true;
  mesh.setColorAt(inst, colorForNode(node));
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
}

// ───────── 飞行模式(PointerLock 第一人称)─────────
function toggleFlyMode() {
  flyMode.value = !flyMode.value;
  if (flyMode.value) {
    // 用当前相机朝向初始化 yaw/pitch
    const dir = new THREE.Vector3();
    camera.getWorldDirection(dir);
    flyYaw = Math.atan2(dir.x, dir.z);
    flyPitch = Math.asin(THREE.MathUtils.clamp(dir.y, -1, 1));
    controls.enabled = false;
    renderer.domElement.requestPointerLock?.();
    document.addEventListener("pointerlockchange", onPointerLockChange);
    window.addEventListener("keydown", onFlyKeyDown);
    window.addEventListener("keyup", onFlyKeyUp);
    document.addEventListener("mousemove", onFlyMouseMove);
    statusText.value = "飞行模式:WASD 平移 · 鼠标转向 · Q/E 升降 · Shift 加速 · Esc 退出";
  } else {
    exitFlyMode();
  }
}

function exitFlyMode() {
  flyMode.value = false;
  controls.enabled = true;
  if (document.pointerLockElement) document.exitPointerLock?.();
  document.removeEventListener("pointerlockchange", onPointerLockChange);
  window.removeEventListener("keydown", onFlyKeyDown);
  window.removeEventListener("keyup", onFlyKeyUp);
  document.removeEventListener("mousemove", onFlyMouseMove);
  flyKeys = { w: false, a: false, s: false, d: false, q: false, e: false, shift: false };
  renderer.domElement.style.cursor = "default";
  // 让 orbit target 落到视线前方的内容上,支点距离按场景尺度,
  // 避免回到轨道模式后缩放支点过近(每格滚轮只移动几单位)导致"缩放特别慢"。
  const dir = new THREE.Vector3();
  camera.getWorldDirection(dir);
  let pivotDist = THREE.MathUtils.clamp(sceneScale * 0.35, 80, 6000);
  try {
    const ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(0, 0), camera);
    const meshes = Object.values(typeMeshes).filter((m) => m.visible);
    const hits = ray.intersectObjects(meshes, false);
    if (hits.length && hits[0].distance > 5) pivotDist = hits[0].distance;
  } catch (e) {
    /* 拾取失败则用场景尺度回退 */
  }
  controls.target.copy(camera.position).addScaledVector(dir, pivotDist);
  statusText.value = "";
  flushPendingReload("exit-fly-mode");
}

function onPointerLockChange() {
  // 用户按 Esc 退出指针锁 → 同步退出飞行模式
  if (!document.pointerLockElement && flyMode.value) exitFlyMode();
}

function onFlyMouseMove(ev) {
  if (!flyMode.value) return;
  const sens = 0.0022;
  flyYaw -= ev.movementX * sens;
  flyPitch -= ev.movementY * sens;
  const lim = Math.PI / 2 - 0.05;
  flyPitch = THREE.MathUtils.clamp(flyPitch, -lim, lim);
}

function onFlyKeyDown(ev) {
  const k = ev.key.toLowerCase();
  if (k === "escape") { exitFlyMode(); return; }
  if (k in flyKeys) { flyKeys[k] = true; ev.preventDefault(); }
  if (k === "shift") flyKeys.shift = true;
}
function onFlyKeyUp(ev) {
  const k = ev.key.toLowerCase();
  if (k in flyKeys) flyKeys[k] = false;
  if (k === "shift") flyKeys.shift = false;
}

function updateFlight(dt) {
  // 由 yaw/pitch 重建朝向
  const forward = new THREE.Vector3(
    Math.sin(flyYaw) * Math.cos(flyPitch),
    Math.sin(flyPitch),
    Math.cos(flyYaw) * Math.cos(flyPitch)
  ).normalize();
  const right = new THREE.Vector3().crossVectors(forward, camera.up).normalize();
  // 飞行速度:常速更快,Shift 冲刺更猛(按场景尺度调高,避免"挪不动")
  const speed = (flyKeys.shift ? 1200 : 460) * dt;
  const move = new THREE.Vector3();
  if (flyKeys.w) move.add(forward);
  if (flyKeys.s) move.sub(forward);
  if (flyKeys.d) move.add(right);
  if (flyKeys.a) move.sub(right);
  if (flyKeys.e) move.y += 1;
  if (flyKeys.q) move.y -= 1;
  if (move.lengthSq() > 0) {
    move.normalize().multiplyScalar(speed);
    camera.position.add(move);
  }
  camera.lookAt(camera.position.clone().add(forward));
}

async function onClick(ev) {
  if (flyMode.value) return; // 飞行模式不触发节点选择
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

// opts.frame:是否把镜头拉近到选中集。索引变更后的静默重载传 false,避免无端搬动用户视角。
function selectNode(node, opts = {}) {
  const { frame = true } = opts;
  selected.value = node;
  suggestions.value = [];
  tagSuggestions.value = [];
  nodeRelations.value = [];
  store.setKg3dRelations([]);
  store.setKg3dSuggestions([]);
  // 高亮选中节点及其邻居(主节点发光),并把镜头拉近取景,飞行模式下不抢镜头
  highlightEgo(node.id, { frame: frame && !flyMode.value });
  rebuildLabels();
  store.setKg3dSelected(node); // 发布到通信总线,右侧边栏据此渲染操作面板
}

function clearSelection() {
  selected.value = null;
  resetHighlight();
  rebuildLabels();
  store.setKg3dSelected(null);
}

// 是否可在编辑器中打开(目录/知识库节点没有可跳转的文件)
function canOpenNode(n) {
  if (!n) return false;
  const path = n.file_path || n.abs_path || n.rel_path || n.path;
  return !!path && ["file", "section", "paragraph", "concept"].includes(n.type);
}

// 按文件路径在当前图中查找对应节点(用于文件树点击定位)。优先匹配 file/directory 节点。
function findNodeByPath(rawPath) {
  const norm = (p) => String(p || "").replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  const target = norm(rawPath);
  if (!target) return null;
  let best = null;
  for (const n of nodeList) {
    const abs = norm(n.abs_path || n.file_path || n.path);
    const rel = norm(n.rel_path);
    // 绝对路径相等,或绝对路径以该节点相对路径结尾
    const hit = (abs && abs === target) || (rel && (target === rel || target.endsWith("/" + rel)));
    if (!hit) continue;
    const isContainer = ["directory", "file", "knowledge-base"].includes(n.type);
    if (!best || isContainer) best = n;
    if (best && ["directory", "file", "knowledge-base"].includes(best.type)) break;
  }
  return best;
}

// 在主界面编辑器中打开节点对应文件并跳转到对应行
async function openNodeInEditor(node) {
  const n = node || selected.value;
  if (!canOpenNode(n)) return;
  const path = n.file_path || n.abs_path || n.rel_path || n.path;
  const lineNumber = Number(n.start_line || 1);
  try {
    await fileTreeStore.openFileInTab(path);
    const content = fileTreeStore.selectedFile?.content || "";
    editorStore.setFile({
      path,
      content,
      highlightInfo: {
        lineNumber,
        matches: [{ start: 0, end: 1000000 }],
        text: n.name || "",
      },
    });
    window.location.hash = "/";
  } catch (e) {
    statusText.value = "打开失败:" + (e?.message || e);
  }
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
async function highlightEgo(nodeId, opts = {}) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  let egoIds = new Set([nodeId]);
  try {
    const ego = await store.kgNeighbors(name, nodeId, 1, 400);
    for (const n of ego.nodes || []) egoIds.add(n.id);
  } catch (e) {
    /* 退化:只高亮自身 */
  }
  applyHighlight(egoIds, { primary: nodeId, frame: opts.frame === true });
}

// 强调一组节点:高亮集放大 + 提亮,其余大幅压暗缩小,并(可选)给定一条/多条"链路"
// 用更亮的发光边连起来;最后把相机取景到强调集的包围盒,让效果直观可见。
// opts: { primary: 主节点id(最亮最大), pathIds: [按顺序的链路节点id], frame: 是否取景 }
function applyHighlight(idSet, opts = {}) {
  highlightedIds = idSet instanceof Set ? idSet : new Set(idSet);
  const primary = opts.primary;
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      const on = highlightedIds.has(n.id);
      const c = colorForNode(n);
      if (on) {
        c.multiplyScalar(n.id === primary ? 2.6 : 1.7);
      } else {
        c.multiplyScalar(0.12);
      }
      mesh.setColorAt(i, c);
      // 放大强调节点(主节点更大),压小背景节点
      const mul = on ? (n.id === primary ? 2.6 : 1.9) : 0.55;
      n._hlMul = mul;
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = n._hidden ? 0 : baseRadius(n) * (n._scaleMul ?? 1) * mul;
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
    });
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }
  buildHighlightEdges(idSet, opts.pathIds);
  glowPrimaryId = primary ?? null;
  rebuildGlow();
  if (opts.frame !== false) frameNodes(highlightedIds);
  requestRender();
}

// 构建发光叠加边:若给了有序 pathIds 则只连相邻路径段,否则连强调集内部的所有原始边。
function buildHighlightEdges(idSet, pathIds) {
  if (highlightEdges) {
    scene.remove(highlightEdges);
    highlightEdges.geometry.dispose();
    highlightEdges.material.dispose();
    highlightEdges = null;
  }
  const positions = [];
  const pushSeg = (aId, bId) => {
    const a = curPositions.get(aId);
    const b = curPositions.get(bId);
    if (a && b) positions.push(a.x, a.y, a.z, b.x, b.y, b.z);
  };
  if (Array.isArray(pathIds) && pathIds.length > 1) {
    for (let i = 0; i < pathIds.length - 1; i++) pushSeg(pathIds[i], pathIds[i + 1]);
  } else {
    for (const e of edgeList) {
      if (idSet.has(e.from) && idSet.has(e.to)) pushSeg(e.from, e.to);
    }
  }
  if (!positions.length) return;
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const mat = new THREE.LineBasicMaterial({
    color: 0x39d0ff,
    transparent: true,
    opacity: 0.95,
    depthTest: false,
    depthWrite: false,
  });
  highlightEdges = new THREE.LineSegments(geo, mat);
  highlightEdges.renderOrder = 999;
  scene.add(highlightEdges);
}

// 相机取景到一组节点:计算包围盒,把 target 移到中心、相机拉到合适距离。
function frameNodes(idSet) {
  const ids = idSet instanceof Set ? idSet : new Set(idSet);
  if (!ids.size) return;
  const box = new THREE.Box3();
  let any = false;
  for (const id of ids) {
    const p = curPositions.get(id);
    if (p) { box.expandByPoint(tmpVec.set(p.x, p.y, p.z)); any = true; }
  }
  if (!any) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3()).length();
  // 拉近镜头:小集合(单点/两点)用很近的固定距离,确保看得清且能看到发光;
  // 大集合按包围盒放大但仍偏紧,避免节点显得很小。
  const dist = THREE.MathUtils.clamp(size * 0.85 + 26, 34, 260);
  const dir = camera.position.clone().sub(controls.target).normalize();
  if (!isFinite(dir.x) || dir.lengthSq() < 1e-6) dir.set(0.3, 0.4, 1).normalize();
  gsap.to(controls.target, { x: center.x, y: center.y, z: center.z, duration: 0.85, ease: "power2.out" });
  gsap.to(camera.position, {
    x: center.x + dir.x * dist,
    y: center.y + dir.y * dist + size * 0.1,
    z: center.z + dir.z * dist,
    duration: 0.85,
    ease: "power2.out",
  });
}

function resetHighlight() {
  highlightedIds = new Set();
  glowPrimaryId = null;
  rebuildGlow();
  if (highlightEdges) {
    scene.remove(highlightEdges);
    highlightEdges.geometry.dispose();
    highlightEdges.material.dispose();
    highlightEdges = null;
  }
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      n._hlMul = 1;
      mesh.setColorAt(i, colorForNode(n));
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const s = n._hidden ? 0 : baseRadius(n) * (n._scaleMul ?? 1);
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
    });
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }
  requestRender();
}

function focusEgo() {
  if (selected.value) highlightEgo(selected.value.id, { frame: true });
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

// 选中节点后聚焦它本身:发光 + 拉近镜头(单点也要看得清)
function focusSingle(nodeId) {
  const ids = new Set([nodeId]);
  // ego 邻居也带上,让上下文一起亮起来
  applyHighlight(ids, { primary: nodeId, frame: true });
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
    const orderedIds = (res.nodes || []).map((n) => n.id);
    const ids = new Set(orderedIds);
    applyHighlight(ids, { primary: orderedIds[0], pathIds: orderedIds, frame: true });
    rebuildLabels();
    statusText.value = `最短路径:${res.hops} 跳 · 已高亮 ${orderedIds.length} 个节点`;
  } catch (e) {
    statusText.value = "路径查询失败";
  }
}

function togglePathMode() {
  pathMode.value = !pathMode.value;
  pathPick = [];
  statusText.value = pathMode.value ? "路径模式:依次点击两个节点" : "";
  if (!pathMode.value) flushPendingReload("exit-path-mode");
}

// ───────── 建议 / 确认 ─────────
// 关联/标签建议都依赖向量。失败大多是没生成向量,这里自动补一次再重试,不麻烦用户。
let embedAutoTried = false;
async function ensureEmbeddingsThen(retryFn) {
  if (embedAutoTried) return false;
  embedAutoTried = true;
  statusText.value = "首次使用,正在自动生成向量(可能较久)…";
  try {
    await store.kgEmbedKb(props.kbName || store.selectedKnowledgeBase?.knowledge_base_name);
    await retryFn();
    return true;
  } catch (e) {
    statusText.value = "自动生成向量失败:" + (e?.message || e);
    return false;
  }
}

async function suggestRelations() {
  if (!selected.value) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    suggestions.value = await store.kgSuggestRelations(name, selected.value.id, 8, 0.55);
    store.setKg3dSuggestions(suggestions.value);
    if (!suggestions.value.length) {
      // 没结果:可能没向量,自动补一次再试
      const ok = await ensureEmbeddingsThen(async () => {
        suggestions.value = await store.kgSuggestRelations(name, selected.value.id, 8, 0.55);
        store.setKg3dSuggestions(suggestions.value);
      });
      if (!suggestions.value.length) statusText.value = ok ? "暂无关联建议" : "暂无关联建议(可能未生成向量)";
    } else {
      // 有建议:把它们一起高亮出来,让用户直观看到候选
      highlightSuggestionSet();
    }
  } catch (e) {
    await ensureEmbeddingsThen(async () => {
      suggestions.value = await store.kgSuggestRelations(name, selected.value.id, 8, 0.55);
      store.setKg3dSuggestions(suggestions.value);
      highlightSuggestionSet();
    });
  }
}

// 把当前节点 + 所有建议关联节点一起高亮(主节点发光),并取景
function highlightSuggestionSet() {
  if (!selected.value || !suggestions.value.length) return;
  const ids = new Set([selected.value.id]);
  for (const s of suggestions.value) ids.add(s.to);
  applyHighlight(ids, { primary: selected.value.id, frame: true });
  rebuildLabels();
}

async function suggestTags() {
  if (!selected.value) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    tagSuggestions.value = await store.kgSuggestTags(name, selected.value.id, null, 6, 0.5);
    if (!tagSuggestions.value.length) {
      const ok = await ensureEmbeddingsThen(async () => {
        tagSuggestions.value = await store.kgSuggestTags(name, selected.value.id, null, 6, 0.5);
      });
      if (!tagSuggestions.value.length) statusText.value = ok ? "暂无标签建议" : "暂无标签建议(可能未生成向量)";
    }
  } catch (e) {
    await ensureEmbeddingsThen(async () => {
      tagSuggestions.value = await store.kgSuggestTags(name, selected.value.id, null, 6, 0.5);
    });
  }
}

async function confirmRelation(s) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  await store.kgConfirmRelation(name, s.from, s.to, "related", s.score);
  suggestions.value = suggestions.value.filter((x) => x.to !== s.to);
  store.setKg3dSuggestions(suggestions.value);
  // 本地补一条边,使高亮/关系列表立即反映,无需整图重建
  if (!edgeList.some((e) => e.from === s.from && e.to === s.to && (e.type || "related") === "related")) {
    edgeList.push({ from: s.from, to: s.to, type: "related", strength: s.score ?? 0.5, source: "manual" });
  }
  statusText.value = "已建立关联";
}

async function confirmTag(s) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  await store.kgAddTag(name, selected.value.id, s.tag, "zeroshot", s.score);
  if (!selected.value.tags) selected.value.tags = [];
  selected.value.tags.push(s.tag);
  tagSuggestions.value = tagSuggestions.value.filter((x) => x.tag !== s.tag);
}

// ───────── 删除标签 / 关系 ─────────
async function removeTag(tag) {
  if (!selected.value) return;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    await store.kgRemoveTag(name, selected.value.id, tag);
    selected.value.tags = (selected.value.tags || []).filter((t) => t !== tag);
    statusText.value = `已删除标签 #${tag}`;
  } catch (e) {
    statusText.value = "删除标签失败:" + (e?.message || e);
  }
}

// 拉取当前节点已有关系(从内存边集,标注哪些可删除:非结构性边)
async function loadNodeRelations() {
  if (!selected.value) return;
  const id = selected.value.id;
  const rels = [];
  const seen = new Set();
  for (const e of edgeList) {
    let other = null;
    if (e.from === id) other = e.to;
    else if (e.to === id) other = e.from;
    else continue;
    const key = other + "|" + (e.type || "");
    if (seen.has(key)) continue;
    seen.add(key);
    const on = nodeById.get(other);
    const deletable = !["contains", "next"].includes(e.type) &&
      (e.from === id); // 仅对从本节点出发的非结构边提供删除
    rels.push({ to: other, to_name: on?.name, type: e.type || "related", deletable, from: e.from, toId: e.to });
  }
  nodeRelations.value = rels;
  store.setKg3dRelations(rels);
  if (!rels.length) statusText.value = "该节点暂无关系边";
}

async function removeRelation(r) {
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  try {
    await store.kgRemoveRelation(name, r.from ?? selected.value.id, r.toId ?? r.to, r.type);
    nodeRelations.value = nodeRelations.value.filter((x) => !(x.to === r.to && x.type === r.type));
    store.setKg3dRelations(nodeRelations.value);
    edgeList = edgeList.filter((e) => !(e.from === (r.from ?? selected.value.id) && e.to === (r.toId ?? r.to) && (e.type || "related") === r.type));
    syncInstancePositions();
    statusText.value = "已删除关系";
  } catch (e) {
    statusText.value = "删除关系失败:" + (e?.message || e);
  }
}

// 点击关联/建议节点:把当前节点与目标节点及其连线一起高亮放大并取景。
// 优先用最短路径串起两者(经过的中间节点也高亮),拿不到则只连两点。
async function highlightPair(targetId) {
  if (!selected.value || !targetId) return;
  const fromId = selected.value.id;
  const name = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
  let pathIds = [fromId, targetId];
  const ids = new Set([fromId, targetId]);
  try {
    const res = await store.kgShortestPath(name, fromId, targetId);
    if (res && res.found && Array.isArray(res.nodes) && res.nodes.length) {
      pathIds = res.nodes.map((n) => n.id);
      for (const id of pathIds) ids.add(id);
    }
  } catch (e) {
    /* 退化为直连两点 */
  }
  applyHighlight(ids, { primary: fromId, pathIds, frame: true });
  rebuildLabels();
  const tname = nodeById.get(targetId)?.name || targetId;
  statusText.value = `高亮:${selected.value.name} ↔ ${tname}${pathIds.length > 2 ? ` (${pathIds.length - 1} 跳)` : ""}`;
}

// ───────── 收藏 ─────────
function isBookmarked(n) {
  return !!n && store.kg3dBookmarks.some((x) => x.id === n.id);
}
function toggleBookmark(n) {
  if (!n) return;
  store.toggleKg3dBookmark(n);
}

// ───────── 工具条动作 ─────────
function toggleType(t) {
  const next = new Set(visibleTypes.value);
  if (next.has(t)) next.delete(t);
  else next.add(t);
  visibleTypes.value = next;
  applyTypeVisibility();
}

function onLevelChange() {
  applyTypeVisibility();
  fitCamera();
  const labels = { all: "全部层级", dir: "仅目录", file: "到文件", section: "到章节" };
  statusText.value = `层级:${labels[levelFilter.value] || levelFilter.value}`;
}

function onTagFilterChange() {
  applyTypeVisibility();
}

// 从当前图收集所有标签作为下拉选项
function refreshTagFilterOptions() {
  const counts = new Map();
  for (const n of nodeList) {
    for (const t of Array.isArray(n.tags) ? n.tags : []) {
      const s = String(t || "").trim();
      if (s) counts.set(s, (counts.get(s) || 0) + 1);
    }
  }
  tagFilterOptions.value = [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], "zh"))
    .map(([tag, c]) => ({ label: `${tag} (${c})`, value: tag }));
  // 清掉已不存在的已选标签
  const valid = new Set(tagFilterOptions.value.map((o) => o.value));
  selectedTagFilter.value = selectedTagFilter.value.filter((t) => valid.has(t));
}

async function rebuildIndexAndReload() {
  statusText.value = "重建索引…";
  try {
    const root = breadcrumb.value[breadcrumb.value.length - 1]?.id || "kb_root";
    await loadGraph(root, root === "kb_root" ? 99 : 3, { ensureIndex: true });
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
  if (!cv) return;
  const ctx = cv.getContext("2d");
  const W = cv.width;
  const H = cv.height;
  ctx.clearRect(0, 0, W, H);
  ctx.fillStyle = "rgba(13,17,23,0.6)";
  ctx.fillRect(0, 0, W, H);
  if (!nodeList.length) return;

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
    const levelTypes = LEVEL_TYPES[levelFilter.value];
    if (levelTypes && !levelTypes.has(n.type) && n.type !== "knowledge-base") continue;
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
// 演化模式:timeMode=true 用真实时间;否则用结构顺序(深度→重要度)作为"显现顺序",
// 避免所有节点时间戳缺失/雷同时,回放一直空白、最后突然全冒出来。
let evoTimeMode = true;

function computeTimeBounds() {
  let count = 0;
  let min = Infinity;
  let max = -Infinity;
  for (const n of nodeList) {
    const t = rawNodeTimeMs(n);
    if (t == null) continue;
    count++;
    if (t < min) min = t;
    if (t > max) max = t;
  }
  const hasRange = count > 0;
  if (!hasRange) {
    min = 0;
    max = 1;
  }
  // 有效时间需要:足够多节点有时间戳 且 时间跨度不退化(> 1 分钟)
  const coverage = nodeList.length ? count / nodeList.length : 0;
  evoTimeMode = coverage >= 0.4 && max - min > 60_000;
  timeBounds = evoTimeMode ? { min, max } : { min: 0, max: 1 };
  timelineRangeLabel.value = evoTimeMode
    ? { min: new Date(min).toISOString(), max: new Date(max).toISOString() }
    : { min: "", max: "" };

  if (evoTimeMode) {
    for (const n of nodeList) {
      const t = rawNodeTimeMs(n);
      n._evoFrac = t == null ? 0 : (t - min) / Math.max(1, max - min);
    }
  } else {
    // 结构顺序:根/目录先出现,然后按深度、再按重要度逐层展开
    const ordered = [...nodeList].sort((a, b) => {
      const da = a.depth ?? 99, db = b.depth ?? 99;
      if (da !== db) return da - db;
      return (b.pagerank || 0) - (a.pagerank || 0);
    });
    const m = Math.max(1, ordered.length - 1);
    ordered.forEach((n, i) => { n._evoFrac = i / m; });
  }

  timelineValue.value = 100;
  timelineLabel.value = "全部";
}

function rawNodeTimeMs(n) {
  const raw = n.modified_at || n.created_at;
  const t = raw ? Date.parse(raw) : NaN;
  return Number.isFinite(t) ? t : null;
}

function applyTimeline() {
  const frac = timelineValue.value / 100;
  const showAll = timelineValue.value >= 100;
  if (evoTimeMode) {
    const cutoffMs = timeBounds.min + (timeBounds.max - timeBounds.min) * frac;
    timelineLabel.value = showAll ? "全部" : new Date(cutoffMs).toLocaleDateString();
  } else {
    timelineLabel.value = showAll ? "全部" : `进度 ${Math.round(frac * 100)}%`;
  }

  // 计算当前可见集合,识别相对上一帧"新出现"的节点用于波纹高亮
  const nowVisible = new Set();
  let visCount = 0;
  for (const [, mesh] of Object.entries(typeMeshes)) {
    const arr = mesh.userData.nodes;
    arr.forEach((n, i) => {
      // 按 _evoFrac 决定该节点是否已"显现":显现阈值 <= 当前进度;被类型/标签过滤的不显现
      const ef = n._evoFrac ?? 0;
      const visible = !n._hidden && (showAll || ef <= frac);
      if (visible) { nowVisible.add(n.id); visCount++; }
      const justBorn = evolutionActive && visible && !prevVisibleIds.has(n.id) && !showAll;
      n._scaleMul = visible ? 1 : 0;
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      // 新生节点做一次 pop 放大 + 高亮闪光
      const popMul = justBorn ? 2.2 : 1;
      const s = baseRadius(n) * n._scaleMul * popMul;
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
      if (justBorn) {
        mesh.setColorAt(i, colorForNode(n).multiplyScalar(2.4));
        // 回落动画:下一帧用 gsap 缩回正常大小/颜色
        gsapPopBack(mesh, i, n);
      } else if (visible) {
        mesh.setColorAt(i, colorForNode(n));
      }
    });
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }

  // 边随端点可见而出现:不可见端点的边收起
  if (edgeLines) {
    const pos = edgeLines.geometry.getAttribute("position");
    let k = 0;
    for (const e of edgeList) {
      const a = curPositions.get(e.from);
      const b = curPositions.get(e.to);
      const both = nowVisible.has(e.from) && nowVisible.has(e.to);
      if (!a || !b) continue;
      if (both) {
        pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
        pos.array[k++] = b.x; pos.array[k++] = b.y; pos.array[k++] = b.z;
      } else {
        // 折叠到原点外不可见(0 长度线段)
        pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
        pos.array[k++] = a.x; pos.array[k++] = a.y; pos.array[k++] = a.z;
      }
    }
    pos.needsUpdate = true;
  }

  prevVisibleIds = nowVisible;
  if (evolutionActive && !showAll) {
    statusText.value = `知识演化 · ${timelineLabel.value} · 已显现 ${visCount}/${nodeList.length}`;
  }
  updateStarColors();
  requestRender();
}

// 新生节点回落:把放大的实例平滑缩回正常尺寸与颜色
function gsapPopBack(mesh, i, n) {
  const prog = { t: 0 };
  gsap.to(prog, {
    t: 1,
    duration: 0.7,
    ease: "back.out(2)",
    onUpdate() {
      if (n._scaleMul === 0) return; // 已被收起
      const p = curPositions.get(n.id) || { x: 0, y: 0, z: 0 };
      const popMul = 2.2 - 1.2 * prog.t;
      const s = baseRadius(n) * popMul;
      tmpMat.compose(tmpVec.set(p.x, p.y, p.z), tmpQuat.identity(), tmpScale.set(s, s, s));
      mesh.setMatrixAt(i, tmpMat);
      mesh.instanceMatrix.needsUpdate = true;
      mesh.setColorAt(i, colorForNode(n).multiplyScalar(2.4 - 1.4 * prog.t));
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    },
  });
}

function toggleTimelinePlay() {
  timelinePlaying.value = !timelinePlaying.value;
  if (timelinePlaying.value) {
    // 进入演化态:从头开始,逐步显现
    evolutionActive = true;
    prevVisibleIds = new Set();
    if (timelineValue.value >= 100) timelineValue.value = 0;
    applyTimeline();
    timelinePlayTimer = setInterval(() => {
      timelineValue.value = Math.min(100, timelineValue.value + 1.5);
      applyTimeline();
      if (timelineValue.value >= 100) {
        // 播放结束:停在全显并退出演化态
        evolutionActive = false;
        toggleTimelinePlay();
        statusText.value = "知识演化回放完成";
      }
    }, 140);
  } else if (timelinePlayTimer) {
    clearInterval(timelinePlayTimer);
    timelinePlayTimer = null;
    evolutionActive = false;
    flushPendingReload("timeline-stopped");
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
    flushPendingReload("expand-collapsed");
    return;
  }
  // 需要社区数据:没有就自动先做一次分组,不麻烦用户二次操作
  let hasComm = nodeList.some((n) => n.community_id != null);
  if (!hasComm) {
    statusText.value = "未分组,正在自动按主题分组…";
    await runCommunities();
    hasComm = nodeList.some((n) => n.community_id != null);
    if (!hasComm) {
      statusText.value = "自动分组失败,无法收拢";
      return;
    }
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

// 边栏 → kg3d 的动作总线
watch(kg3dActionRequest, (req) => {
  if (!req) return;
  const { type, payload } = req;
  switch (type) {
    case "select": {
      const n = nodeById.get(payload?.id);
      if (n) selectNode(n);
      break;
    }
    case "selectByPath": {
      const n = findNodeByPath(payload?.path);
      if (n) {
        selectNode(n);
        statusText.value = `已定位:${n.name}`;
      } else {
        statusText.value = "图中未找到该文件对应的节点(可能未建索引或已折叠)";
      }
      break;
    }
    case "flyTo":
      if (payload?.id) flyTo(payload.id);
      break;
    case "open":
      openNodeInEditor(payload?.node || selected.value);
      break;
    case "suggestRelations":
      suggestRelations();
      break;
    case "suggestTags":
      suggestTags();
      break;
    case "focusEgo":
      focusEgo();
      break;
    case "addTag":
      if (payload?.tag && selected.value) {
        store.kgAddTag(props.kbName || store.selectedKnowledgeBase?.knowledge_base_name, selected.value.id, payload.tag, "manual", 1.0).then(() => {
          if (!selected.value.tags) selected.value.tags = [];
          if (!selected.value.tags.includes(payload.tag)) selected.value.tags.push(payload.tag);
        });
      }
      break;
    case "removeTag":
      if (payload?.tag) removeTag(payload.tag);
      break;
    case "loadRelations":
      loadNodeRelations();
      break;
    case "removeRelation":
      if (payload) removeRelation(payload);
      break;
    case "highlightPair":
      if (payload?.targetId) highlightPair(payload.targetId);
      break;
    case "switchView":
      if (payload?.view) { view.value = payload.view; switchView(); }
      break;
    case "reload":
      loadGraph(breadcrumb.value[breadcrumb.value.length - 1].id);
      break;
    case "community":
      runCommunities();
      break;
    case "embed":
      runEmbed();
      break;
    case "focusCommunity":
      focusCommunity(payload?.community_id);
      break;
  }
});

watch(
  () => props.kbName,
  async (nextName, prevName) => {
    const next = String(nextName || "").trim();
    const prev = String(prevName || "").trim();
    if (next === prev) return;

    // 切库先让所有未完成的旧加载失效，再清空画布，最后加载新知识库。
    graphLoadSeq += 1;
    resetGraphStateForKnowledgeBase(next);
    logger.info("[kg3d] knowledge base prop changed, reload graph", {
      from: prev,
      to: next
    });

    if (!next) return;
    await loadGraph();
  },
  { flush: "post" }
);

// 聚焦某社区:高亮其成员、连其内部边并取景
function focusCommunity(cid) {
  if (cid == null) return;
  const members = nodeList.filter((n) => (n.community_id ?? -1) === cid);
  const ids = new Set(members.map((n) => n.id));
  if (!ids.size) return;
  // 以社区内 PageRank 最高的节点为主节点
  let primary = members[0]?.id;
  let bestPr = -1;
  for (const m of members) {
    if ((m.pagerank || 0) > bestPr) { bestPr = m.pagerank || 0; primary = m.id; }
  }
  applyHighlight(ids, { primary, frame: true });
  rebuildLabels();
  statusText.value = `聚焦社区:${members.length} 个节点`;
}

// 页面可见性:标签页切到后台时停渲染,切回时恢复
function onVisibilityChange() {
  if (document.hidden) stopLoop();
  else { startLoop(); onResize(); flushPendingReload("visibility-restored"); }
}

let unlistenIndex = null;

onMounted(async () => {
  initThree();
  document.addEventListener("visibilitychange", onVisibilityChange);
  try {
    // 保存/新建文件会连发脏标记,防抖再长一点,避免大库连续重建
    unlistenIndex = await onKgIndexChanged(({ kbName, reason }) => {
      const cur = props.kbName || store.selectedKnowledgeBase?.knowledge_base_name;
      if (!cur || kbName !== cur) return;
      autoReloadGraph(reason);
    }, { debounceMs: 1200 });
  } catch (e) {
    logger.warn("[kg3d] 监听索引变更失败", e);
  }
  // 错峰:先让路由切换那一帧把页面框架画出来,再异步拉数据+布局,首屏不卡
  await new Promise((r) => requestAnimationFrame(() => r()));
  await loadGraph();
});

// KeepAlive:被缓存(切到其它页面)时停掉渲染循环,切回时恢复
onActivated(() => {
  viewActive = true;
  startLoop();
  onResize();
  flushPendingReload("activated");
});
onDeactivated(() => {
  viewActive = false;
  stopLoop();
  if (flyMode.value) exitFlyMode();
});

onBeforeUnmount(() => {
  if (unlistenIndex) { try { unlistenIndex(); } catch (e) { /* noop */ } }
  stopLoop();
  if (timelinePlayTimer) clearInterval(timelinePlayTimer);
  if (themeObserver) themeObserver.disconnect();
  if (resizeObserver) resizeObserver.disconnect();
  if (flyMode.value) exitFlyMode();
  gsap.ticker.remove(gsapDirtyTick);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  window.removeEventListener("resize", onResize);
  if (renderer) {
    renderer.domElement.removeEventListener("click", onClick);
    renderer.domElement.removeEventListener("dblclick", onDblClick);
    renderer.domElement.removeEventListener("mousemove", onPointerMove);
    disposeGraphObjects();
    renderer.dispose();
  }
  if (starDust) {
    scene?.remove(starDust);
    starDust.geometry.dispose();
    starDust.material.dispose();
    starDust = null;
  }
  if (glowTexture) { glowTexture.dispose(); glowTexture = null; }
  disposeLayoutWorker();
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
.tb-iconbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  padding: 0;
  border-radius: 7px;
  background: #161b22;
  color: #e6edf3;
  border: 1px solid #30363d;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, transform 0.12s ease;
}
.tb-iconbtn:hover { background: #21303f; transform: translateY(-1px); }
.tb-iconbtn.active { background: #1f6feb; border-color: #1f6feb; color: #fff; }
.tb-ico { width: 17px; height: 17px; }
.kg3d-filters {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.flt { display: flex; align-items: center; gap: 4px; cursor: pointer; white-space: nowrap; }
.flt-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
.tag-filter { position: relative; }
.tag-filter-btn {
  padding: 2px 10px; border-radius: 6px; cursor: pointer; white-space: nowrap;
  background: #161b22; color: #e6edf3; border: 1px solid #30363d; font-size: 12px;
}
.tag-filter-btn.active { background: #1f6feb; border-color: #1f6feb; color: #fff; }
.tag-filter-menu {
  position: absolute; top: 26px; left: 0; z-index: 20;
  min-width: 180px; max-height: 280px; overflow: auto;
  background: rgba(22, 27, 34, 0.98); border: 1px solid #30363d; border-radius: 8px;
  padding: 6px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}
.tag-filter-head { display: flex; justify-content: space-between; align-items: center; font-size: 11px; opacity: 0.7; margin-bottom: 4px; }
.tag-filter-head a { cursor: pointer; color: #58a6ff; }
.tag-filter-item { display: flex; align-items: center; gap: 6px; padding: 3px 4px; border-radius: 5px; cursor: pointer; font-size: 12px; }
.tag-filter-item:hover { background: rgba(127, 127, 127, 0.14); }
.kg3d-panel {
  position: absolute;
  top: 96px;
  right: 12px;
  width: 280px;
  /* 上下都锚定,内容超高时面板内部滚动,绝不挤出屏幕 */
  max-height: calc(100% - 160px);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  overflow-x: hidden;
  z-index: 6;
  background: rgba(22, 27, 34, 0.92);
  border: 1px solid #30363d;
  border-radius: 10px;
  padding: 10px;
}
/* 关联/标签建议、已有关系列表各自限高,长列表内部滚动 */
.kg3d-panel .pn-suggest {
  max-height: 220px;
  overflow-y: auto;
  flex: 0 0 auto;
}
.pn-head { display: flex; align-items: center; gap: 6px; }
.pn-type { font-size: 11px; padding: 1px 6px; border-radius: 4px; color: #0b0e14; }
.pn-name { font-weight: 600; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pn-x { cursor: pointer; opacity: 0.6; }
.pn-meta { font-size: 11px; opacity: 0.7; margin: 6px 0; display: flex; flex-direction: column; gap: 2px; }
.pn-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.pn-tag { font-size: 11px; background: #21262d; padding: 1px 6px; border-radius: 10px; display: inline-flex; align-items: center; gap: 3px; }
.pn-tagx { cursor: pointer; opacity: 0.55; font-style: normal; font-size: 13px; line-height: 1; }
.pn-tagx:hover { opacity: 1; color: #f85149; }
.pn-del { width: 22px; border-radius: 4px; background: #6e1f1f; color: #fff; border: none; cursor: pointer; }
.pn-del:hover { background: #b62324; }
.pn-actions { display: flex; gap: 6px; }
.pn-actions button { flex: 1; padding: 4px; border-radius: 6px; background: #21262d; color: #e6edf3; border: 1px solid #30363d; cursor: pointer; }
.pn-suggest { margin-top: 8px; }
.pn-sub { font-size: 11px; opacity: 0.6; margin-bottom: 4px; }
.pn-srow { display: flex; align-items: center; gap: 6px; margin-bottom: 3px; }
.pn-sname { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.pn-link { cursor: pointer; }
.pn-link:hover { color: #58a6ff; text-decoration: underline; }
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
  width: min(52%, 520px);
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
.tl-edge { font-size: 10px; opacity: 0.6; white-space: nowrap; min-width: 42px; }
.tl-label { font-size: 11px; opacity: 0.75; min-width: 64px; text-align: right; }
:deep(.kg3d-label) {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: #e6edf3;
  font-size: 11px;
  font-weight: 500;
  line-height: 1.3;
  letter-spacing: 0.2px;
  padding: 2px 8px 2px 6px;
  border-radius: 999px;
  pointer-events: none;
  white-space: nowrap;
  /* 磨砂胶囊:半透明深底 + 细描边 + 轻投影,文字始终清晰可读 */
  background: rgba(13, 17, 23, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.10);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
  transition: opacity 0.18s ease;
}
:deep(.kg3d-label-dot) {
  flex: 0 0 auto;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #58a6ff;
  box-shadow: 0 0 6px currentColor, 0 0 0 1px rgba(255, 255, 255, 0.25) inset;
}
:deep(.kg3d-label-txt) {
  overflow: hidden;
  text-overflow: ellipsis;
}
:deep(.kg3d-label-hl) {
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  padding: 3px 10px 3px 8px;
  background: linear-gradient(135deg, rgba(31, 111, 235, 0.92), rgba(57, 160, 255, 0.92));
  border-color: rgba(120, 200, 255, 0.55);
  box-shadow: 0 0 14px rgba(57, 208, 255, 0.6), 0 2px 10px rgba(0, 0, 0, 0.4);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
}
:deep(.kg3d-label-hl) .kg3d-label-dot {
  background: #d6f0ff !important;
  box-shadow: 0 0 8px #aee3ff;
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
.kg3d-light .tb-btn,
.kg3d-light .tb-iconbtn {
  background: #ffffff;
  color: #1f2328;
  border-color: #d0d7de;
}
.kg3d-light .tb-btn.active,
.kg3d-light .tb-iconbtn.active {
  background: #1f6feb;
  color: #fff;
  border-color: #1f6feb;
}
.kg3d-light .tb-iconbtn:hover { background: #eef2f6; }
.kg3d-light .tag-filter-btn { background: #ffffff; color: #1f2328; border-color: #d0d7de; }
.kg3d-light .tag-filter-btn.active { background: #1f6feb; color: #fff; border-color: #1f6feb; }
.kg3d-light .tag-filter-menu { background: #ffffff; border-color: #d0d7de; color: #1f2328; }
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
  color: #1f2328;
  background: rgba(255, 255, 255, 0.78);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  text-shadow: 0 1px 1px rgba(255, 255, 255, 0.7);
}
.kg3d-light :deep(.kg3d-label-hl) {
  color: #fff;
  background: linear-gradient(135deg, rgba(31, 111, 235, 0.95), rgba(57, 160, 255, 0.95));
  border-color: rgba(31, 111, 235, 0.5);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
}
</style>
