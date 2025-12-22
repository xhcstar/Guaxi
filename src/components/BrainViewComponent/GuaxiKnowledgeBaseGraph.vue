<template>
  <!-- Main container with dark mode support -->
  <div class="knowledge-graph-container" :class="{ 'dark-mode': darkMode }">
    <!-- Toolbar section with graph controls -->
    <div class="toolbar">
      <n-space>
        <!-- Reset camera view button -->
        <n-button size="small" @click="resetCamera">
          <template #icon><n-icon>
              <CompassOutline />
            </n-icon></template>
          重置视角
        </n-button>

        <!-- Toggle auto-rotation button -->
        <n-button size="small" @click="toggleAutoRotate">
          <template #icon><n-icon>
              <SyncOutline />
            </n-icon></template>
          {{ autoRotate ? '停止旋转' : '自动旋转' }}
        </n-button>

        <!-- Toggle dark mode button -->
        <n-button size="small" @click="toggleDarkMode">
          <template #icon><n-icon>
              <MoonOutline />
            </n-icon></template>
          {{ darkMode ? '亮色模式' : '暗色模式' }}
        </n-button>

        <!-- Layout selection dropdown -->
        <n-select v-model:value="currentLayout" :options="layoutOptions" size="small" style="width: 120px" />

        <!-- Performance level slider -->
        <n-slider v-model:value="performanceLevel" :step="1" :min="0" :max="2" :marks="performanceMarks"
          style="width: 150px" />
      </n-space>
    </div>

    <!-- Three.js rendering container -->
    <div ref="container" class="graph-container"></div>

    <!-- Loading overlay shown during data loading -->
    <div v-if="isLoading" class="loading-overlay">
      <n-spin size="large" />
      <n-progress type="circle" :percentage="loadingProgress" :indicator-placement="'inside'" :show-indicator="false" />
    </div>

    <!-- Node information panel shown when a node is selected -->
    <div v-if="selectedNode" class="node-info-panel">
      <n-card :title="selectedNode.name || '连接线'" size="small">
        <n-space vertical>
          <n-tag :type="getNodeTagType(selectedNode.type)" size="small">
            {{ getNodeTypeName(selectedNode.type) }}
          </n-tag>
          <n-text v-if="selectedNode.path">路径: {{ selectedNode.path }}</n-text>
          <n-text v-if="selectedNode.stable_id">stable_id: {{ selectedNode.stable_id }}</n-text>
          <n-text v-if="selectedNode.start_line">行: {{ selectedNode.start_line }}</n-text>
          <n-text v-if="selectedNode.from">从: {{ selectedNode.from.name }}</n-text>
          <n-text v-if="selectedNode.to">到: {{ selectedNode.to.name }}</n-text>
          <n-button-group size="small">
            <n-button @click="focusOnNode(selectedNode.type === 'edge' ? selectedNode.from : selectedNode)">
              聚焦
            </n-button>
            <n-button @click="highlightConnectedNodes(selectedNode)">
              高亮连接
            </n-button>
            <n-button
              v-if="selectedNode.path && ['file', 'section', 'paragraph'].includes(selectedNode.type)"
              @click="openNode(selectedNode)"
            >
              打开
            </n-button>
          </n-button-group>
        </n-space>
      </n-card>
    </div>

    <!-- Legend panel explaining node and edge types -->
    <div class="legend-panel">
      <n-card title="图例" size="small">
        <n-space vertical>
          <div class="legend-item">
            <div class="legend-color knowledge-base"></div>
            <n-text>知识库</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-color directory"></div>
            <n-text>目录</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-color file"></div>
            <n-text>文件</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-color section"></div>
            <n-text>标题</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-color paragraph"></div>
            <n-text>段落</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-line strong-link"></div>
            <n-text>强连接</n-text>
          </div>
          <div class="legend-item">
            <div class="legend-line weak-link"></div>
            <n-text>弱连接</n-text>
          </div>
        </n-space>
      </n-card>
    </div>


  </div>
</template>

<script setup>
// Vue composition API imports
import { ref, onMounted, onBeforeUnmount, watch, computed, nextTick } from 'vue';
import { useRouter } from 'vue-router';

// Three.js core and extensions
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass';
import { CSS2DRenderer, CSS2DObject } from 'three/examples/jsm/renderers/CSS2DRenderer';
import { Octree } from 'three/addons/math/Octree.js';
import { gsap } from "gsap";
import Stats from 'three/examples/jsm/libs/stats.module';
import * as d3 from 'd3-force-3d';

// Tauri API for backend communication
import { invoke } from '@tauri-apps/api/core';

// Pinia store for state management
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { useEditorStore } from '../../stores/editorStore';
import { storeToRefs } from 'pinia';

// UI components from Naive UI
import {
  NButton, NSpace, NIcon, NCard, NText, NAlert, NSpin,
  NSelect, NSlider, NProgress, NTag, NButtonGroup
} from 'naive-ui';

// Icons from Ionicons
import { CompassOutline, SyncOutline, MoonOutline } from '@vicons/ionicons5';

// Initialize Pinia store
const store = useKnowledgeBaseStore();
const router = useRouter();
const fileTreeStore = useFileTreeStore();
const editorStore = useEditorStore();

// Destructure store properties with reactivity
const { knowledgeBases, selectedKnowledgeBase, visibleTypes, selectedTags, searchQuery, focusTarget } = storeToRefs(store);

// Computed property for currently selected knowledge base
const selectedKB = computed(() => {
  return selectedKnowledgeBase.value ||
    (knowledgeBases.value.length > 0 ? knowledgeBases.value[0] : {});
});

// Debug configuration
const DEBUG = true;
const log = (...args) => DEBUG && console.log('[KnowledgeGraph]', ...args);
const warn = (...args) => DEBUG && console.warn('[KnowledgeGraph]', ...args);
const error = (...args) => DEBUG && console.error('[KnowledgeGraph]', ...args);

// Reactive state variables
const container = ref(null); // Reference to DOM container
const autoRotate = ref(true); // Auto-rotation toggle state
const darkMode = ref(false); // Dark mode toggle state
const selectedNode = ref(null); // Currently selected node
const isLoading = ref(false); // Loading state
const loadingProgress = ref(0); // Loading progress percentage
const showError = ref(false); // Error display state
const errorMessage = ref(''); // Error message content
const currentLayout = ref('force'); // Current graph layout type
const performanceLevel = ref(1); // Performance quality level
const pendingKnowledgeBaseChange = ref(null); // Pending KB change

// Computed property to check if current KB is valid
const hasValidKnowledgeBase = computed(() => {
  return isValidKnowledgeBase(selectedKB.value);
});

// Layout options for dropdown
const layoutOptions = [
  { label: '力导向布局', value: 'force' },
  { label: '层次布局', value: 'hierarchical' },
  { label: '球形布局', value: 'spherical' },
  { label: '树状布局', value: 'tree' }
];

// Performance level marks for slider
const performanceMarks = {
  0: '性能优先',
  1: '平衡',
  2: '质量优先'
};

// Three.js variables (non-reactive)
let scene, camera, renderer, controls, raycaster, composer, labelRenderer;
let nodes = [], edges = [], octree;
let buildGraphRunId = 0;
const mouse = new THREE.Vector2(); // Mouse position tracker

/**
 * Initialize Three.js scene, renderer, camera and controls
 */
const initThreeJS = () => {
  // 1. Create scene with background and fog
  scene = new THREE.Scene();
  scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xbbbbbb);
  scene.fog = new THREE.FogExp2(scene.background, 0.002);

  console.log('[initThreeJS]');
  // 2. Create perspective camera
  camera = new THREE.PerspectiveCamera(
    60, // Field of view
    container.value.clientWidth / container.value.clientHeight, // Aspect ratio
    0.1, // Near clip
    10000 // Far clip
  );
  camera.position.set(0, 0, 100); // Initial camera position

  console.log('[initThreeJS]', 'camera', camera);
  // 3. Create WebGL renderer with performance settings
  renderer = new THREE.WebGLRenderer({
    antialias: true,
    powerPreference: "high-performance",
    logarithmicDepthBuffer: true
  });

  if (!renderer.getContext()) {
    console.error('WebGL 渲染器初始化失败');
    showError.value = true;
    errorMessage.value = '浏览器不支持 WebGL 或初始化失败';
    return;
  }

  // 启用线宽支持
  const gl = renderer.getContext();
  gl.getExtension('EXT_frag_depth');
  gl.getExtension('OES_standard_derivatives');

  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(container.value.clientWidth, container.value.clientHeight);
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  container.value.appendChild(renderer.domElement);
  console.log("renderer", renderer);

  // 4. Create CSS2D renderer for labels
  labelRenderer = new CSS2DRenderer();
  labelRenderer.setSize(container.value.clientWidth, container.value.clientHeight);
  labelRenderer.domElement.style.position = 'absolute';
  labelRenderer.domElement.style.top = '0';
  labelRenderer.domElement.style.pointerEvents = 'none';
  container.value.appendChild(labelRenderer.domElement);

  // 5. Set up post-processing effects
  composer = new EffectComposer(renderer);
  const renderPass = new RenderPass(scene, camera);
  composer.addPass(renderPass);
  console.log('composer', composer);

  if (camera && renderer?.domElement) {
    controls = new OrbitControls(camera, renderer.domElement);
    controls.autoRotate = autoRotate.value;
    controls.autoRotateSpeed = 0.5;
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 20;
    controls.maxDistance = 500;

    // 确保 controls 已创建再设置 target
    controls.target.set(0, 0, 0);
    controls.update();
    console.log('controls', controls);
  } else {
    console.error('初始化控制器失败: camera或renderer.domElement未定义');
  }

  // Add bloom effect for glowing highlights
  const bloomPass = new UnrealBloomPass(
    new THREE.Vector2(window.innerWidth, window.innerHeight),
    1.5, // Strength
    0.4, // Radius
    0.85 // Threshold
  );
  composer.addPass(bloomPass);
  console.log('bloomPass', bloomPass);

  // 6. Add orbit controls for camera interaction
  controls = new OrbitControls(camera, renderer.domElement);
  controls.autoRotate = autoRotate.value;
  controls.autoRotateSpeed = 0.5;
  controls.enableDamping = true;
  controls.dampingFactor = 0.05;
  controls.minDistance = 20;
  controls.maxDistance = 500;

  console.log('OrbitControls controls', controls)
  // 7. Set up scene lighting
  setupLighting();
  console.log('setupLighting', setupLighting)
  // 8. Initialize raycasting for mouse interaction
  raycaster = new THREE.Raycaster();

  console.log('raycaster', raycaster)
  // 9. Set up event listeners
  setupEventListeners();
  console.log('setupEventListeners', setupEventListeners)
};

/**
 * Reset camera to default position and orientation
 */
const resetCamera = () => {
  if (!camera || !controls) return;

  camera.position.set(0, 0, 100);
  controls.target.set(0, 0, 0);
  controls.update();

  log('相机已重置');
};

/**
 * Toggle auto-rotation of the graph
 */
const toggleAutoRotate = () => {
  if (!controls) return;

  autoRotate.value = !autoRotate.value;
  controls.autoRotate = autoRotate.value;

  log(`自动旋转已${autoRotate.value ? '开启' : '关闭'}`);
};

/**
 * Toggle between dark and light mode
 */
const toggleDarkMode = () => {
  darkMode.value = !darkMode.value;

  // 更新场景背景和雾
  if (scene) {
    scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xbbbbbb);
    scene.fog.color.copy(scene.background);
  }

  nodes.forEach(node => {
    if (node.mesh?.material) {
      node.mesh.material.color.setHex(getNodeColor(node.userData.type));
      node.mesh.material.emissive.setHex(darkMode.value ? 0x111111 : 0x000000);
      node.mesh.material.emissiveIntensity = darkMode.value ? 0.2 : 0.15;
      node.mesh.material.needsUpdate = true;
    }
  });

  // 更新边材质
  edges.forEach(edge => {
    if (edge.material) {
      edge.material.color.setHex(darkMode.value ? 0x888888 : 0x1565C0);
      edge.material.linewidth = darkMode.value ? 1 : 2;
      edge.material.opacity = darkMode.value ? 0.8 : 0.9;
      edge.material.needsUpdate = true;
    }
  });

  // 更新光照
  setupLighting();

  log(`已切换至${darkMode.value ? '暗黑' : '亮色'}模式`);
};

const normalize = (v) => String(v || '').toLowerCase();
const normalizePath = (v) => String(v || '').replace(/\\/g, '/');

const MAX_LABEL_LEN = 14;
const truncateLabel = (v) => {
  const s = String(v || '');
  if (s.length <= MAX_LABEL_LEN) return s;
  return `${s.slice(0, MAX_LABEL_LEN)}…`;
};

const resolveGraphNode = (nodeLike) => {
  if (!nodeLike) return null;
  if (nodeLike.mesh && nodeLike.userData) return nodeLike;

  const id = nodeLike.id ?? nodeLike.userData?.id;
  const stableId = nodeLike.stable_id ?? nodeLike.userData?.stable_id;
  const path =
    nodeLike.path ??
    nodeLike.file_path ??
    nodeLike.userData?.path ??
    nodeLike.userData?.file_path;

  const pathNorm = path ? normalizePath(path) : '';

  return (
    nodes.find((n) => {
      const ud = n?.userData || {};
      if (id != null && String(ud.id) === String(id)) return true;
      if (stableId && String(ud.stable_id || '') === String(stableId)) return true;
      if (pathNorm && normalizePath(ud.path || ud.file_path || '') === pathNorm) return true;
      return false;
    }) || null
  );
};

const nodePassesFilters = (n) => {
  const ud = n?.userData || {};

  const allowedTypes = new Set(Array.isArray(visibleTypes.value) ? visibleTypes.value : []);
  if (!allowedTypes.has(ud.type)) return false;

  const selTags = Array.isArray(selectedTags.value) ? selectedTags.value : [];
  if (selTags.length > 0) {
    const ts = Array.isArray(ud.tags) ? ud.tags : [];
    if (!selTags.some((t) => ts.includes(t))) return false;
  }

  const q = normalize(searchQuery.value).trim();
  if (!q) return true;

  const hay = `${ud.name || ''} ${normalizePath(ud.path || '')} ${normalizePath(ud.file_path || '')} ${ud.stable_id || ''} ${(Array.isArray(ud.tags) ? ud.tags.join(',') : '')}`;
  return normalize(hay).includes(q);
};

const applyGraphVisibility = () => {
  nodes.forEach((n) => {
    const ok = nodePassesFilters(n);
    if (n?.mesh) n.mesh.visible = ok;
  });

  edges.forEach((e) => {
    const from = e?.userData?.from;
    const to = e?.userData?.to;
    e.visible = !!from && !!to && !!from.mesh?.visible && !!to.mesh?.visible;
  });
};

watch([visibleTypes, selectedTags, searchQuery], () => applyGraphVisibility(), { deep: true });

/**
 * Find node by its position in 3D space
 * @param {Array} positionArray - [x, y, z] coordinates
 * @returns {Object|null} Found node or null
 */
const findNodeByPosition = (positionArray) => {
  if (!positionArray || positionArray.length < 3 || !octree) return null;

  const position = new THREE.Vector3(
    positionArray[0],
    positionArray[1],
    positionArray[2]
  );

  // 使用八叉树查询
  const result = octree.search(position, 5); // 5是搜索半径
  if (result.length > 0) {
    return result[0].userData || null;
  }

  return null;
};

/**
 * Get tag type for node based on its type
 * @param {string} type - Node type
 * @returns {string} Tag type
 */
const getNodeTagType = (type) => {
  switch (type) {
    case 'knowledge-base': return 'warning';
    case 'directory': return 'info';
    case 'file': return 'success';
    case 'section': return 'primary';
    case 'paragraph': return 'default';
    case 'edge': return 'error';
    default: return 'default';
  }
};

/**
 * Get display name for node type
 * @param {string} type - Node type
 * @returns {string} Display name
 */
const getNodeTypeName = (type) => {
  switch (type) {
    case 'knowledge-base': return '知识库';
    case 'directory': return '目录';
    case 'file': return '文件';
    case 'section': return '标题';
    case 'paragraph': return '段落';
    case 'edge': return '连接';
    default: return '未知';
  }
};

/**
 * Focus camera on specific node
 * @param {Object} node - Node to focus on
 */
const focusOnNode = (nodeLike) => {
  const node = resolveGraphNode(nodeLike);
  if (!node || !camera || !controls) return;

  const targetPosition = node.mesh?.position?.clone?.() || new THREE.Vector3(0, 0, 0);

  const direction = new THREE.Vector3(0, 0, 1);
  const distance = 30;
  const cameraPosition = targetPosition.clone().add(direction.multiplyScalar(distance));

  gsap.to(camera.position, {
    x: cameraPosition.x,
    y: cameraPosition.y,
    z: cameraPosition.z,
    duration: 1,
    ease: "power2.inOut"
  });

  gsap.to(controls.target, {
    x: targetPosition.x,
    y: targetPosition.y,
    z: targetPosition.z,
    duration: 1,
    ease: "power2.inOut"
  });

  log(`已聚焦到节点: ${node.userData?.name || ''}`);
};

/**
 * Highlight connected nodes and edges
 * @param {Object} node - Node to highlight connections for
 */
const highlightConnectedNodes = (nodeLike) => {
  const node = resolveGraphNode(nodeLike);
  if (!node) return;

  resetNodeHighlights();

  if (node.mesh) {
    node.mesh.material.color.setHex(0xff0000);
    node.mesh.material.emissive.setHex(0x333333);
  }

  edges.forEach((edge) => {
    if (edge.userData.from === node || edge.userData.to === node) {
      edge.material.color.setHex(0xff0000);
      edge.material.opacity = 1;

      const otherNode = edge.userData.from === node ? edge.userData.to : edge.userData.from;

      if (otherNode.mesh) {
        otherNode.mesh.material.color.setHex(0xff9900);
        otherNode.mesh.material.emissive.setHex(0x222222);
      }
    }
  });

  log(`已高亮${node.userData?.name || ''}的连接节点`);
};

/**
 * Open file associated with node
 * @param {string} path - File path to open
 */
const openNode = async (node) => {
  const path = node?.file_path || node?.path;
  if (!path) return;

  const lineNumber = Number(node?.start_line || 1);

  try {
    await fileTreeStore.openFileInTab(path);
    const content = fileTreeStore.selectedFile?.content || '';

    editorStore.setFile({
      path,
      content,
      highlightInfo: {
        lineNumber,
        matches: [{ start: 0, end: 1000000 }],
        text: node?.name || ''
      }
    });

    await router.push({ name: 'home' });
  } catch (e) {
    warn('打开节点失败:', e);
  }
};

/**
 * Clear the Three.js scene and reset node/edge arrays
 */
const clearScene = () => {
  if (!scene) return;

  selectedNode.value = null;
  store.setSelectedGraphNode(null);

  if (labelRenderer?.domElement) {
    labelRenderer.domElement.innerHTML = '';
  }

  octree = null;

  while (scene.children.length > 0) {
    const obj = scene.children[0];

    obj.traverse?.((child) => {
      const el = child?.element;
      if (el?.parentNode) el.parentNode.removeChild(el);

      if (child?.geometry) child.geometry.dispose();
      if (child?.material) {
        if (Array.isArray(child.material)) child.material.forEach(m => m.dispose());
        else child.material.dispose();
      }
    });

    if (obj.geometry) obj.geometry.dispose();
    if (obj.material) {
      if (Array.isArray(obj.material)) {
        obj.material.forEach(m => m.dispose());
      } else {
        obj.material.dispose();
      }
    }
    scene.remove(obj);
  }

  nodes = [];
  edges = [];

  log('场景已清理');
};

/**
 * Fetch graph data from backend
 * @returns {Promise<Object|null>} Graph data or null on error
 */
const fetchGraphData = async () => {
  isLoading.value = true;

  try {
    // Validate selected knowledge base
    if (!selectedKB.value?.id) {
      throw new Error("未选择有效的知识库");
    }

    // Call store method to get data
    const data = await store.fetchGraphData(selectedKB.value.id);
    log('获取图形数据成功', data);
    return data;
  } catch (err) {
    error('获取图形数据失败:', err);
    showError.value = true;
    errorMessage.value = '获取图形数据失败';
    return null;
  } finally {
    isLoading.value = false;
  }
};

/**
 * Create Three.js objects from graph data
 * @param {Object} data - Graph data with nodes and edges
 */
const createNodesAndEdges = async (data) => {
  if (!data) return;

  console.log('开始创建节点和边...');

  // 1. 创建节点
  data.nodes.forEach(nodeData => {
    const geometry = new THREE.SphereGeometry(2, 32, 32);

    nodeData.originalColor = getNodeColor(nodeData.type);

    const material = new THREE.MeshStandardMaterial({
      color: nodeData.originalColor,
      metalness: 0.2,
      roughness: 0.5,
      emissive: darkMode.value ? 0x111111 : 0x000000,
      emissiveIntensity: darkMode.value ? 0.2 : 0.15
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(0, 0, 0);
    mesh.userData = nodeData;
    scene.add(mesh);

    nodes.push({
      mesh,
      userData: nodeData,
      targetPosition: new THREE.Vector3(0, 0, 0)
    });

    if (typeof CSS2DObject !== 'undefined') {
      const label = document.createElement('div');
      label.className = 'node-label';
      label.textContent = truncateLabel(nodeData.name);
      label.title = String(nodeData.name || '');
      const labelObj = new CSS2DObject(label);
      labelObj.position.set(0, 10, 0);
      mesh.add(labelObj);
    }
  });

  // 2. 先应用布局，确保节点有正确位置
  await applyLayout(currentLayout.value);

  // 3. 创建边（在节点位置确定后）
  // 先清除所有现有边
  edges.forEach(edge => {
    scene.remove(edge);
    if (edge.geometry) edge.geometry.dispose();
    if (edge.material) edge.material.dispose();
  });
  edges = [];

  data.edges.forEach(edgeData => {
    const fromNode = nodes.find(n => n.userData.id === edgeData.from);
    const toNode = nodes.find(n => n.userData.id === edgeData.to);

    if (fromNode && toNode) {
      // 创建直线几何体
      const geometry = new THREE.BufferGeometry();
      const positions = new Float32Array(6); // 两个点，每个点3个坐标
      geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));


      const material = new THREE.LineBasicMaterial({
        color: darkMode.value ? 0x888888 : 0x1565C0,
        linewidth: darkMode.value ? 1 : 2,
        transparent: true,
        opacity: darkMode.value ? 0.8 : 1,
        depthTest: true,
        depthWrite: true
      });

      const line = new THREE.Line(geometry, material);

      // 保存更新函数
      line.userData = {
        ...edgeData,
        from: fromNode,
        to: toNode,
        update: () => {
          if (fromNode.mesh && toNode.mesh) {
            const positions = line.geometry.attributes.position.array;
            positions[0] = fromNode.mesh.position.x;
            positions[1] = fromNode.mesh.position.y;
            positions[2] = fromNode.mesh.position.z;
            positions[3] = toNode.mesh.position.x;
            positions[4] = toNode.mesh.position.y;
            positions[5] = toNode.mesh.position.z;
            line.geometry.attributes.position.needsUpdate = true;
            line.geometry.computeBoundingSphere();
          }
        }
      };

      scene.add(line);
      edges.push(line);
      line.userData.update(); // 初始更新
    }
  });

  applyGraphVisibility();
  log(`创建了${nodes.length}个节点和${edges.length}条边`);
};
/**
 * Calculate control point for curved edges
 * @param {THREE.Vector3} start - Start position
 * @param {THREE.Vector3} end - End position
 * @returns {THREE.Vector3} Control point
 */
const getControlPoint = (start, end) => {
  const mid = new THREE.Vector3().addVectors(start, end).multiplyScalar(0.5);
  const distance = start.distanceTo(end);

  // 根据距离动态调整曲线高度
  const height = Math.min(distance * 0.5, 50);

  // 随机方向避免所有曲线在同一平面
  const dir = new THREE.Vector3(
    Math.random() * 0.2 - 0.1,
    Math.random() * 0.2 - 0.1,
    1
  ).normalize();

  mid.add(dir.multiplyScalar(height));
  return mid;
};

/**
 * Apply force-directed layout to nodes
 */
const forceDirectedLayout = () => {
  log('应用力导向布局');

  if (!nodes.length) return;

  const byId = new Map();
  nodes.forEach((n, idx) => {
    const id = n?.userData?.id ?? `__i_${idx}`;
    byId.set(String(id), n);
  });

  const rootNode =
    nodes.find(n => n?.userData?.type === 'knowledge-base') ||
    nodes.find(n => n?.userData?.parentId == null) ||
    nodes[0];

  const rootId = String(rootNode?.userData?.id ?? '__i_0');

  const ringStep = performanceLevel.value === 0 ? 22 : performanceLevel.value === 2 ? 55 : 38;
  const base = performanceLevel.value === 0 ? 20 : performanceLevel.value === 2 ? 45 : 32;

  const nodeDatas = nodes.map((n, idx) => {
    const id = n?.userData?.id ?? `__i_${idx}`;
    const depth = Number.isFinite(n?.userData?.depth) ? Number(n.userData.depth) : (n?.userData?.parentId ? 1 : 0);
    const d = { ...n.userData, id, depth };

    const r = String(id) === rootId ? 0 : (base + Math.max(1, depth) * ringStep);
    const i = idx + 1;
    const theta = i * 2.399963229728653;
    const u = (i % 997) / 997;
    const phi = Math.acos(1 - 2 * u);

    d.x = r * Math.cos(theta) * Math.sin(phi);
    d.y = r * Math.sin(theta) * Math.sin(phi);
    d.z = r * Math.cos(phi);

    if (String(id) === rootId) {
      d.fx = 0; d.fy = 0; d.fz = 0;
      d.x = 0; d.y = 0; d.z = 0;
    }

    return d;
  });

  const ndById = new Map(nodeDatas.map(d => [String(d.id), d]));

  const edgeDatas = (edges || [])
    .map(e => ({
      source: e?.userData?.from?.userData?.id,
      target: e?.userData?.to?.userData?.id
    }))
    .filter(l => l.source != null && l.target != null)
    .map(l => ({ source: String(l.source), target: String(l.target) }));

  const forceSphere = (radiusFn, strength = 0.35) => {
    let ns = [];
    function f(alpha) {
      for (const d of ns) {
        if (d.fx != null || d.fy != null || d.fz != null) continue;
        const r = radiusFn(d);
        const x = d.x || 0, y = d.y || 0, z = d.z || 0;
        const dist = Math.sqrt(x * x + y * y + z * z) || 1e-6;
        const k = (r - dist) / dist * strength * alpha;
        d.vx = (d.vx || 0) + x * k;
        d.vy = (d.vy || 0) + y * k;
        d.vz = (d.vz || 0) + z * k;
      }
    }
    f.initialize = (_) => { ns = _ || []; };
    return f;
  };

  const simulation = d3.forceSimulation(nodeDatas)
    .numDimensions(3)
    .alpha(1)
    .alphaDecay(0.06)
    .velocityDecay(0.35)
    .force('link', d3.forceLink(edgeDatas)
      .id(d => String(d.id))
      .distance(l => {
        const t = String(l?.target?.id ?? l?.target);
        const td = ndById.get(t);
        const depth = Number.isFinite(td?.depth) ? td.depth : 1;
        return base + Math.max(1, depth) * ringStep;
      })
      .strength(0.9)
    )
    .force('charge', d3.forceManyBody().strength(d => (String(d.id) === rootId ? -120 : -28)))
    .force('collision', d3.forceCollide().radius(d => (String(d.id) === rootId ? 10 : 6)))
    .force('orbit', forceSphere(d => {
      if (String(d.id) === rootId) return 0;
      const depth = Number.isFinite(d.depth) ? d.depth : 1;
      return base + Math.max(1, depth) * ringStep;
    }, 0.35));

  simulation.on('tick', () => {
    for (const d of nodeDatas) {
      const n = byId.get(String(d.id));
      if (!n) continue;
      n.targetPosition.set(d.x || 0, d.y || 0, d.z || 0);
    }
  });

  setTimeout(() => simulation.stop(), 1200);
};

/**
 * Apply hierarchical layout to nodes
 */
const hierarchicalLayout = () => {
  log('应用层次布局');

  // Simple hierarchical layout based on depth
  nodes.forEach((node, i) => {
    const depth = node.userData.depth || 1;
    node.targetPosition.set(
      (i % 10) * 20 - 100,
      depth * 30 - 50,
      0
    );
  });
};

/**
 * Apply spherical layout to nodes
 */
const sphericalLayout = () => {
  log('应用球形布局');

  if (!nodes.length) return;

  const rootNode =
    nodes.find(n => n?.userData?.type === 'knowledge-base') ||
    nodes.find(n => n?.userData?.parentId == null) ||
    nodes[0];

  const radius = performanceLevel.value === 0 ? 45 : performanceLevel.value === 2 ? 110 : 75;
  rootNode?.targetPosition?.set(0, 0, 0);

  const others = nodes.filter(n => n !== rootNode);
  const m = others.length;
  if (!m) return;

  const sorted = others.slice().sort((a, b) =>
    String(a?.userData?.id ?? '').localeCompare(String(b?.userData?.id ?? ''), 'en')
  );

  const offset = 2 / m;
  const inc = Math.PI * (3 - Math.sqrt(5));

  sorted.forEach((node, i) => {
    const y = (i * offset - 1) + offset / 2;
    const r = Math.sqrt(1 - y * y);
    const phi = i * inc;

    node.targetPosition.set(
      radius * Math.cos(phi) * r,
      radius * y,
      radius * Math.sin(phi) * r
    );
  });
};

/**
 * Apply tree layout to nodes
 */
const treeLayout = () => {
  log('应用树状布局');

  if (!nodes.length) return;

  const byId = new Map();
  nodes.forEach((n, idx) => {
    const id = n?.userData?.id ?? `__i_${idx}`;
    byId.set(String(id), n);
  });

  const rootNode =
    nodes.find(n => n?.userData?.type === 'knowledge-base') ||
    nodes.find(n => n?.userData?.parentId == null) ||
    nodes[0];

  const rootId = String(rootNode?.userData?.id ?? '__i_0');

  const children = new Map();
  for (const [id] of byId) children.set(id, []);

  for (const [id, n] of byId) {
    const pidRaw = n?.userData?.parentId;
    const pid = pidRaw == null ? null : String(pidRaw);
    if (!pid || pid === id) continue;
    if (children.has(pid)) children.get(pid).push(id);
  }

  for (const [id, n] of byId) {
    if (id === rootId) continue;
    if (n?.userData?.parentId == null) {
      const arr = children.get(rootId);
      if (arr && !arr.includes(id)) arr.push(id);
    }
  }

  const leaf = new Map();
  const visiting = new Set();
  const countLeaves = (id) => {
    if (leaf.has(id)) return leaf.get(id);
    if (visiting.has(id)) return 1;
    visiting.add(id);
    const ch = children.get(id) || [];
    if (!ch.length) {
      leaf.set(id, 1);
      return 1;
    }
    let sum = 0;
    for (const cid of ch) sum += countLeaves(cid);
    sum = Math.max(1, sum);
    leaf.set(id, sum);
    return sum;
  };

  countLeaves(rootId);

  const depthById = new Map();
  const q = [[rootId, 0]];
  while (q.length) {
    const [id, d] = q.shift();
    if (depthById.has(id)) continue;
    depthById.set(id, d);
    const ch = children.get(id) || [];
    for (const cid of ch) q.push([cid, d + 1]);
  }

  const radiusStep = performanceLevel.value === 0 ? 18 : performanceLevel.value === 2 ? 42 : 28;
  const levelGap = performanceLevel.value === 0 ? 14 : performanceLevel.value === 2 ? 32 : 22;

  const place = (id, start, end) => {
    const n = byId.get(id);
    const depth = depthById.get(id) ?? 0;
    const a = (start + end) / 2;
    const r = depth === 0 ? 0 : depth * radiusStep;
    if (n?.targetPosition) {
      n.targetPosition.set(
        r * Math.cos(a),
        depth * levelGap,
        r * Math.sin(a)
      );
    }

    const ch = children.get(id) || [];
    if (!ch.length) return;

    const total = ch.reduce((acc, cid) => acc + (leaf.get(cid) || 1), 0) || 1;
    let cur = start;
    for (const cid of ch) {
      const w = (leaf.get(cid) || 1) / total;
      const next = cur + (end - start) * w;
      place(cid, cur, next);
      cur = next;
    }
  };

  rootNode?.targetPosition?.set(0, 0, 0);
  place(rootId, 0, Math.PI * 2);
};

/**
 * Set up lighting for the scene
 */
const setupLighting = () => {
  // 清除所有现有光源
  scene.children.filter(obj => obj.isLight).forEach(light => scene.remove(light));

  // 亮色模式下使用更强的环境光
  const ambientLight = new THREE.AmbientLight(
    0xFFFFFF,
    darkMode.value ? 0.4 : 1.0  // 亮色模式增加到1.0
  );
  scene.add(ambientLight);

  // 主方向光（模拟太阳光）
  const directionalLight = new THREE.DirectionalLight(
    0xFFFFFF,
    darkMode.value ? 0.8 : 1.5  // 亮色模式增加到1.5
  );
  directionalLight.position.set(1, 1, 1).normalize();
  directionalLight.castShadow = true;
  scene.add(directionalLight);

  // 辅助的半球光
  const hemisphereLight = new THREE.HemisphereLight(
    0xFFFFFF, // 天空颜色
    0x888888, // 地面颜色
    darkMode.value ? 0.5 : 1.0  // 亮色模式增加到1.0
  );
  scene.add(hemisphereLight);
};

/**
 * Handle window resize events
 */
const handleWindowResize = () => {
  if (!camera || !container.value || !renderer) return;

  // Update camera and renderer dimensions
  camera.aspect = container.value.clientWidth / container.value.clientHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(container.value.clientWidth, container.value.clientHeight);
  labelRenderer?.setSize(container.value.clientWidth, container.value.clientHeight);

  // Update post-processing effects
  if (composer) {
    composer.setSize(container.value.clientWidth, container.value.clientHeight);
  }

  if (DEBUG) {
    log('窗口大小调整:', {
      width: container.value.clientWidth,
      height: container.value.clientHeight
    });
  }
};

/**
 * Set up event listeners for interaction
 */
const setupEventListeners = () => {
  window.addEventListener('resize', handleWindowResize);

  renderer.domElement.addEventListener('mousemove', (event) => {
    if (selectedNode.value) return;

    mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
    mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

    raycaster.setFromCamera(mouse, camera);

    const interactableObjects = scene.children.filter(obj =>
      obj.isMesh || obj.isLine
    );

    const intersects = raycaster.intersectObjects(interactableObjects, true);

    if (intersects.length > 0) {
      const object = intersects[0].object;
      highlightNode(object);
    } else {
      resetNodeHighlights();
    }
  });

  renderer.domElement.addEventListener('click', (event) => {
    mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
    mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(scene.children, true);

    if (intersects.length > 0) {
      const clickedObject = intersects[0].object;
      selectNode(clickedObject);
    } else {
      selectedNode.value = null;
      store.setSelectedGraphNode(null);
      resetNodeHighlights();
    }
  });

  // Double click to focus on node
  renderer.domElement.addEventListener('dblclick', (event) => {
    mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
    mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(scene.children, true);

    if (intersects.length > 0) {
      const object = intersects[0].object;
      focusOnNode(object.userData);
    }
  });
};

/**
 * Reset all node highlights to default
 */
const resetNodeHighlights = () => {
  nodes.forEach(node => {
    if (node.mesh && node.mesh.material) {
      const c = node.userData.originalColor ?? getNodeColor(node.userData?.type);
      node.mesh.material.color.setHex(c);
      if (node.mesh.material.emissive) {
        node.mesh.material.emissive.setHex(darkMode.value ? 0x111111 : 0x000000);
      }
      if (typeof node.mesh.material.emissiveIntensity === 'number') {
        node.mesh.material.emissiveIntensity = darkMode.value ? 0.2 : 0.15;
      }
    }
  });

  edges.forEach(edge => {
    if (edge.material) {
      edge.material.color.setHex(darkMode.value ? 0x888888 : 0x1565C0);
      edge.material.opacity = darkMode.value ? 0.8 : 1;
    }
  });
};

/**
 * Highlight a specific node
 * @param {THREE.Object3D} object - Node to highlight
 */
const highlightNode = (object) => {
  // 先重置所有高亮
  resetNodeHighlights();

  // 确保对象有效且有 userData 和 material
  if (!object || !object.userData?.type || !object.material) {
    return;
  }

  try {
    // 保存原始颜色（如果还没有保存）
    if (object.userData.originalColor === undefined) {
      object.userData.originalColor = object.material.color.getHex();
    }

    // 高亮当前节点
    object.material.color.setHex(0xff0000);

    // 如果有 emissive 属性则设置
    if (object.material.emissive) {
      object.material.emissive.setHex(0x333333);
    }

    // 高亮连接的边
    edges.forEach(edge => {
      if (edge.userData.from === object || edge.userData.to === object) {
        edge.material.color.setHex(0xff0000);
        edge.material.opacity = 1;
      }
    });
  } catch (e) {
    console.warn('高亮节点时出错:', e);
  }
};

/**
 * Select a node and update UI
 * @param {THREE.Object3D} object - Node to select
 */
const selectNode = (object) => {
  if (object.userData?.type === 'edge') {
    selectedNode.value = {
      type: 'edge',
      name: '连接线',
      from: findNodeByPosition(object.geometry.attributes.position.array.slice(0, 3)),
      to: findNodeByPosition(object.geometry.attributes.position.array.slice(3, 6)),
      object: object
    };
    store.setSelectedGraphNode(null);
    resetNodeHighlights();
    return;
  }

  selectedNode.value = {
    ...object.userData,
    object: object
  };

  store.setSelectedGraphNode({ ...object.userData });
  highlightConnectedNodes(object.userData);
};

/**
 * Build the complete knowledge graph
 */
const buildGraph = async () => {
  const runId = ++buildGraphRunId;
  const kbId = selectedKB.value?.id;

  console.log('开始构建知识图谱...');
  isLoading.value = true;
  loadingProgress.value = 0;

  try {
    clearScene();

    const treeData = await fetchGraphData();
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 30;

    console.log('获取知识图谱数据成功！');
    loadingProgress.value = 60;

    await createNodesAndEdges(treeData);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 70;
    console.log('创建节点和边成功！');

    await applyLayout(currentLayout.value);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 90;
    console.log('应用布局成功！');

    const meshes = nodes.map(n => n.mesh).filter(Boolean);
    console.log('准备初始化八叉树，网格数量:', meshes.length);
    await initOctree(meshes);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;

    startAnimate();
    console.log('开始动画循环！');
    loadingProgress.value = 100;
  } catch (err) {
    showError.value = true;
    errorMessage.value = `构建失败: ${err.message}`;
  } finally {
    if (runId === buildGraphRunId) {
      isLoading.value = false;
      resetCamera();
    }
  }
};

/**
 * 优化的八叉树初始化方法
 * @param {Array} meshes - 需要加入八叉树的网格数组
 */
const initOctree = async (meshes) => {
  if (!meshes || meshes.length === 0) {
    console.warn('没有网格数据可用于初始化八叉树');
    octree = {
      search: () => [],
      addGraphNode: () => { },
      clear: () => { }
    };
    return;
  }

  try {
    console.time('八叉树初始化耗时');

    // 创建新的八叉树实例
    octree = new Octree();

    // 检查八叉树方法是否可用
    if (typeof octree.addGraphNode === 'function') {
      // 标准初始化流程
      meshes.forEach(mesh => {
        if (mesh.geometry) {
          // 确保几何体有边界框
          if (!mesh.geometry.boundingBox) {
            mesh.geometry.computeBoundingBox();
          }
          octree.addGraphNode(mesh);
        }
      });
    } else {
      // 降级处理
      console.warn('八叉树缺少addGraphNode方法，使用简化版本');
      octree = {
        search: () => [],
        addGraphNode: () => { },
        clear: () => { }
      };
    }

    console.timeEnd('八叉树初始化耗时');
    console.log(`八叉树初始化完成，共处理${meshes.length}个网格`);
  } catch (e) {
    console.error('八叉树初始化失败:', e);
    // 降级处理
    octree = {
      search: () => [],
      addGraphNode: () => console.warn('八叉树功能不可用'),
      clear: () => { }
    };
  }
};

// 确保亮色模式返回明亮的颜色
const getNodeColor = (type) => {
  const lightColors = {
    'knowledge-base': 0xFFA500,
    'directory': 0x1E90FF,
    'file': 0x32CD32,
    'section': 0x9B59B6,
    'paragraph': 0xE67E22,
    'default': 0x777777
  };

  const darkColors = {
    'knowledge-base': 0xF39C12,
    'directory': 0x3498DB,
    'file': 0x2ECC71,
    'section': 0x8E44AD,
    'paragraph': 0xD35400,
    'default': 0xCCCCCC
  };

  return darkMode.value ? darkColors[type] || darkColors.default
    : lightColors[type] || lightColors.default;
}

/**
 * Apply selected layout algorithm
 * @param {string} layoutType - Layout type to apply
 */
const applyLayout = async (layoutType) => {
  if (!nodes.length) return;

  console.log(`应用布局: ${layoutType}`);

  // 停止当前所有动画
  nodes.forEach(node => {
    node.targetPosition = node.mesh.position.clone();
  });

  // 使用Promise确保布局完成
  await new Promise(resolve => {
    setTimeout(() => {
      switch (layoutType) {
        case 'force':
          forceDirectedLayout();
          break;
        case 'hierarchical':
          hierarchicalLayout();
          break;
        case 'spherical':
          sphericalLayout();
          break;
        case 'tree':
          treeLayout();
          break;
        default:
          forceDirectedLayout();
      }
      resolve();
    }, 0);
  });

  // 更新所有边
  edges.forEach(edge => {
    if (edge.userData?.update) {
      edge.userData.update();
    }
  });
};

/**
 * Animation loop for continuous rendering
 */
let animationFrameId = null;

const animate = () => {
  animationFrameId = requestAnimationFrame(animate);

  // 平滑过渡节点位置
  nodes.forEach(node => {
    if (node.mesh && node.targetPosition) {
      node.mesh.position.lerp(node.targetPosition, 0.1);
    }
  });

  // 更新边位置
  edges.forEach(edge => {
    if (edge.userData?.update) {
      edge.userData.update();
    }
  });

  if (controls) controls.update();

  // 使用composer进行渲染（如果存在）
  if (composer) {
    composer.render();
  } else {
    renderer.render(scene, camera);
  }

  labelRenderer?.render(scene, camera);
};

const startAnimate = () => {
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  animate();
};

watch(currentLayout, (newLayout) => {
  if (scene && nodes.length > 0) {
    applyLayout(newLayout);
  }
});

const testRender = () => {
  if (renderer && scene && camera) {
    // 添加测试立方体
    const geometry = new THREE.BoxGeometry(10, 10, 10);
    const material = new THREE.MeshBasicMaterial({ color: 0xff0000 });
    const cube = new THREE.Mesh(geometry, material);
    scene.add(cube);

    // 手动渲染
    renderer.render(scene, camera);
    console.log('测试立方体已渲染，应该看到红色立方体');
  }
};

/**
 * Show empty state with message
 * @param {string} reason - Reason for empty state
 */
const showEmptyState = (reason = '') => {
  clearScene();

  // Create torus geometry as placeholder
  const group = new THREE.Group();
  const geometry = new THREE.TorusGeometry(3, 0.5, 16, 32);
  const material = new THREE.MeshStandardMaterial({
    color: getNodeColor(nodeData.type),
    metalness: 0.2,
    roughness: 0.5,
    emissive: darkMode.value ? 0x111111 : 0x888888, // 亮色模式下使用浅灰色自发光
    emissiveIntensity: darkMode.value ? 0.2 : 0.5   // 亮色模式下增加自发光强度
  });
  const torus = new THREE.Mesh(geometry, material);
  torus.rotation.x = Math.PI / 2;
  group.add(torus);

  // Add label if available
  if (typeof CSS2DObject !== 'undefined') {
    const label = document.createElement('div');
    label.className = 'empty-state-label';
    label.textContent = reason || '请选择知识库';
    const labelObj = new CSS2DObject(label);
    labelObj.position.set(0, -5, 0);
    group.add(labelObj);
  }

  scene.add(group);

  // Simple rotation animation
  const animateGroup = () => {
    group.rotation.y += 0.01;
    requestAnimationFrame(animateGroup);
  };
  animateGroup();
};

// Watch for changes in selected knowledge base
watch(
  () => store.selectedKnowledgeBase,
  (newVal) => {
    if (!scene) {
      console.warn('场景未初始化，将延迟处理知识库变化');
      pendingKnowledgeBaseChange.value = newVal;
      return;
    }

    if (isValidKnowledgeBase(newVal)) {
      console.log('检测到有效知识库:', newVal.knowledge_base_name);
      nextTick(() => buildGraph());
    } else if (knowledgeBases.value.length > 0) {
      store.setSelectedKnowledgeBase(knowledgeBases.value[0]);
    } else {
      showEmptyState('没有可用的知识库');
    }
  },
  { immediate: true, deep: true }
);

watch(
  focusTarget,
  (target) => {
    if (!target || !nodes.length) return;
    const mode = target.mode || 'id';
    const value = String(target.value || '');
    if (!value) return;

    const hit = nodes.find((n) => {
      const ud = n?.userData || {};
      if (mode === 'path') return normalizePath(ud.path || ud.file_path || '') === normalizePath(value);
      if (mode === 'stable_id') return String(ud.stable_id || '') === value;
      return String(ud.id) === value;
    });

    if (hit) {
      focusOnNode(hit);
      selectedNode.value = { ...hit.userData, object: hit.mesh };
      store.setSelectedGraphNode({ ...hit.userData });
      highlightConnectedNodes(hit);
    }
  },
  { deep: true }
);

/**
 * Handle canvas click events
 * @param {MouseEvent} event - Click event
 */
const handleCanvasClick = (event) => {
  if (!raycaster || !camera) return;

  // Calculate normalized mouse coordinates
  mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
  mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

  // Find intersected objects
  raycaster.setFromCamera(mouse, camera);
  const intersects = raycaster.intersectObjects(scene.children, true);

  if (intersects.length > 0) {
    const clickedObject = intersects[0].object;
    if (clickedObject.userData?.type === 'edge') {
      selectedNode.value = {
        type: 'edge',
        name: '连接线',
        from: findNodeByPosition(clickedObject.geometry.attributes.position.array.slice(0, 3)),
        to: findNodeByPosition(clickedObject.geometry.attributes.position.array.slice(3, 6)),
        object: clickedObject
      };
    } else {
      selectedNode.value = {
        ...clickedObject.userData,
        object: clickedObject
      };
    }
  } else {
    selectedNode.value = null;
  }
};

// Component lifecycle hooks
onMounted(async () => {
  log('组件挂载开始');

  // 1. 先初始化Three.js场景
  await nextTick();
  initThreeJS();

  // 2. 确保容器尺寸正确
  handleWindowResize();

  // 3. 然后加载数据
  if (knowledgeBases.value.length === 0) {
    try {
      await store.fetchKnowledgeBases();
    } catch (err) {
      error('加载知识库列表失败:', err);
      showError.value = true;
      errorMessage.value = '加载知识库列表失败';
    }
  }

  // 4. 最后构建图谱
  if (hasValidKnowledgeBase.value) {
    await buildGraph();
  } else {
    showEmptyState('没有可用的知识库');
  }

  // testRender();
});

/**
 * Validate knowledge base object
 * @param {Object} kb - Knowledge base to validate
 * @returns {boolean} True if valid
 */
const isValidKnowledgeBase = (kb) => {
  if (!kb || typeof kb !== 'object') {
    warn('知识库对象无效');
    return false;
  }

  // Check required fields
  const requiredFields = ['id', 'knowledge_base_name', 'location'];
  if (requiredFields.some(field => !(field in kb))) {
    warn('知识库缺少必要字段');
    return false;
  }

  // Validate path format
  if (typeof kb.location !== 'string') {
    warn('location必须为字符串');
    return false;
  }

  const normalizedPath = kb.location.replace(/\\/g, '/');
  const isWindows = navigator.platform.toLowerCase().includes('win');

  // Platform-specific path validation
  if (isWindows) {
    if (!normalizedPath.match(/^[a-zA-Z]:\//)) {
      warn('Windows路径格式无效');
      return false;
    }
  } else {
    if (!normalizedPath.startsWith('/')) {
      warn('Unix路径必须以/开头');
      return false;
    }
  }

  return true;
};

// Clean up on component unmount
onBeforeUnmount(() => {
  log('开始组件卸载清理');

  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }

  // 1. Remove event listeners
  window.removeEventListener('resize', handleWindowResize);
  if (renderer?.domElement) {
    renderer.domElement.removeEventListener('click', handleCanvasClick);
  }

  // 2. Clean up Three.js scene
  if (scene) {
    log('开始清理场景对象');

    // Dispose all geometries and materials
    scene.traverse((object) => {
      if (object.isMesh || object.isLine) {
        if (object.geometry) {
          object.geometry.dispose();
          log(`已释放几何体: ${object.userData?.name || '未知对象'}`);
        }

        if (object.material) {
          if (Array.isArray(object.material)) {
            object.material.forEach(m => m.dispose());
            log(`已释放材质数组: ${object.userData?.name || '未知对象'}`);
          } else {
            object.material.dispose();
            log(`已释放材质: ${object.userData?.name || '未知对象'}`);
          }
        }
      }

      // Special handling for helper objects
      if (object.isArrowHelper) {
        scene.remove(object);
        log('已移除箭头辅助对象');
      }
    });

    // Clear scene children
    scene.children = [];
  }

  // 3. Remove renderer DOM
  if (container.value && renderer?.domElement) {
    container.value.removeChild(renderer.domElement);
    log('已移除渲染器DOM');
  }

  // 4. Dispose controls
  if (controls) {
    controls.dispose();
    log('已释放控制器');
  }

  // 5. Dispose renderer
  if (renderer) {
    renderer.dispose();
    renderer.forceContextLoss();
    log('已释放渲染器');
  }

  // 添加八叉树清理
  if (octree) {
    octree = null;
    console.log('已释放八叉树');
  }

  // 6. Clear references
  scene = null;
  camera = null;
  renderer = null;
  controls = null;
  raycaster = null;
  nodes = [];
  edges = [];

  log('组件卸载完成');
});
</script>

<style scoped>
/* Main container styling */
.knowledge-graph-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 400px;
}

/* Toolbar styling */
.toolbar {
  padding: 8px 16px;
  z-index: 10;
  background-color: var(--toolbar-bg, rgba(255, 255, 255, 0.9));
  border-bottom: 1px solid var(--toolbar-border, #e0e0e0);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* 暗色模式下的工具栏 */
.dark-mode .toolbar {
  padding: 8px 16px;
  z-index: 10;
  background-color: var(--toolbar-bg, rgba(255, 255, 255, 0.9));
  border-bottom: 1px solid var(--toolbar-border, #e0e0e0);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Graph container styling */
.graph-container {
  flex-grow: 1;
  width: 100%;
  height: calc(100% - 40px);
  background: linear-gradient(to bottom, #f0f0f0, #e0e0e0);
  background: transparent;
  position: relative;
  z-index: 1;
}

/* 确保渲染器canvas可见 */
.graph-container canvas {
  /* 必须确保这些属性不被覆盖 */
  display: block !important;
  position: absolute !important;
  top: 0 !important;
  left: 0 !important;
  z-index: 1 !important;
  opacity: 1 !important;
  pointer-events: auto !important;
}

/* Dark mode graph container */
.dark-mode .graph-container {
  background: linear-gradient(to bottom, #121212, #0a0a0a);
}

/* Loading overlay styling */
.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: rgba(0, 0, 0, 0.5);
  z-index: 1000;
}

/* Node info panel styling */
.node-info-panel {
  position: absolute;
  bottom: 50px;
  right: 20px;
  max-width: 300px;
  z-index: 10;
  background-color: rgba(255, 255, 255, 0.9);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Dark mode node info panel */
.dark-mode .node-info-panel {
  background-color: rgba(30, 30, 30, 0.9);
}

/* 修改 legend-panel 的样式 */
.legend-panel {
  position: absolute;
  top: 120px;
  /* 下移避免遮挡工具栏 */
  left: 10px;
  width: 95px;
  /* 缩小宽度 */
  z-index: 5;
  /* 确保在工具栏下方 */
  background-color: rgba(255, 255, 255, 0.9);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transform: scale(0.9);
  /* 整体缩小 */
  transform-origin: top left;
}

/* 调整图例项的大小 */
.legend-item {
  display: flex;
  align-items: center;
  margin: 2px 0;
  /* 减小间距 */
  font-size: 12px;
  /* 缩小字体 */
}

.legend-color,
.legend-line {
  transform: scale(0.8);
  /* 缩小颜色标记 */
  margin-right: 4px;
  /* 减小右边距 */
}

/* 暗色模式下的调整 */
.dark-mode .legend-panel {
  background-color: rgba(30, 30, 30, 0.9);
}

/* Color indicators in legend */
.legend-color {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  margin-right: 8px;
}

/* Knowledge base color */
.legend-color.knowledge-base {
  background-color: #f39c12;
}

/* Directory color */
.legend-color.directory {
  background-color: #3498db;
}

/* File color */
.legend-color.file {
  background-color: #2ecc71;
}

.legend-color.section {
  background-color: #8e44ad;
}

.legend-color.paragraph {
  background-color: #d35400;
}

/* Strong connection line */
.legend-line.strong-link {
  background-color: #ff5555;
}

/* Weak connection line */
.legend-line.weak-link {
  background-color: #888888;
  opacity: 0.6;
}


/* Node label styling (deep selector for dynamically created elements) */
:deep(.node-label) {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 12px;
  color: #333;
  background-color: rgba(255, 255, 255, 0.9);
  padding: 4px 8px;
  border-radius: 4px;
  pointer-events: none;
  transform: translateY(15px);
  white-space: nowrap;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

/* Dark mode node labels */
.dark-mode :deep(.node-label) {
  color: #fff;
  background-color: rgba(30, 30, 30, 0.9);
}

/* Root node animation */
:deep(.root-node) {
  animation: pulse 2s infinite;
}

/* Pulse animation keyframes */
@keyframes pulse {
  0% {
    transform: scale(1);
  }

  50% {
    transform: scale(1.05);
  }

  100% {
    transform: scale(1);
  }
}
</style>