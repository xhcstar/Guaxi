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
          <n-text v-if="selectedNode.from">从: {{ selectedNode.from.name }}</n-text>
          <n-text v-if="selectedNode.to">到: {{ selectedNode.to.name }}</n-text>
          <n-button-group size="small">
            <n-button @click="focusOnNode(selectedNode.type === 'edge' ? selectedNode.from : selectedNode)">
              聚焦
            </n-button>
            <n-button @click="highlightConnectedNodes(selectedNode)">
              高亮连接
            </n-button>
            <n-button v-if="selectedNode.type === 'file'" @click="openFile(selectedNode.path)">
              打开文件
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

    <!-- Search panel for finding nodes -->
    <div class="search-panel">
      <n-card title="搜索节点" size="small">
        <n-input v-model:value="searchQuery" placeholder="搜索节点..." clearable @update:value="searchNodes">
          <template #prefix>
            <n-icon>
              <SearchOutline />
            </n-icon>
          </template>
        </n-input>
        <n-list v-if="searchResults.length > 0" class="search-results">
          <n-list-item v-for="result in searchResults" :key="result.id" @click="focusOnNode(result)">
            <n-thing :title="result.name" :description="result.path" />
          </n-list-item>
        </n-list>
      </n-card>
    </div>
  </div>
</template>

<script setup>
// Vue composition API imports
import { ref, onMounted, onBeforeUnmount, watch, computed, nextTick } from 'vue';

// Three.js core and extensions
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass';
import { CSS2DRenderer, CSS2DObject } from 'three/examples/jsm/renderers/CSS2DRenderer';
import { Octree } from 'three/examples/jsm/math/Octree';
import Stats from 'three/examples/jsm/libs/stats.module';
import * as d3 from 'd3-force-3d';

// Tauri API for backend communication
import { invoke } from '@tauri-apps/api/core';

// Pinia store for state management
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { storeToRefs } from 'pinia';

// UI components from Naive UI
import {
  NButton, NSpace, NIcon, NCard, NText, NAlert, NSpin,
  NSelect, NSlider, NProgress, NTag, NButtonGroup,
  NInput, NList, NListItem, NThing
} from 'naive-ui';

// Icons from Ionicons
import { CompassOutline, SyncOutline, MoonOutline, SearchOutline } from '@vicons/ionicons5';

// Initialize Pinia store
const store = useKnowledgeBaseStore();

// Destructure store properties with reactivity
const { knowledgeBases, selectedKnowledgeBase } = storeToRefs(store);

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
const searchQuery = ref(''); // Search query text
const searchResults = ref([]); // Search results array
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
const mouse = new THREE.Vector2(); // Mouse position tracker

/**
 * Initialize Three.js scene, renderer, camera and controls
 */
const initThreeJS = () => {
  // 1. Create scene with background and fog
  scene = new THREE.Scene();
  scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xf0f0f0);
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

  // Update scene background
  if (scene) {
    scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xf0f0f0);
    scene.fog.color.copy(scene.background);
  }

  // Update lighting for new mode
  setupLighting();

  log(`已切换至${darkMode.value ? '暗黑' : '亮色'}模式`);
};

/**
 * Search nodes by name or path
 * @param {string} query - Search term
 */
const searchNodes = (query) => {
  if (!query || !nodes.length) {
    searchResults.value = [];
    return;
  }

  const lowerQuery = query.toLowerCase();
  searchResults.value = nodes.filter(node =>
    node.userData?.name?.toLowerCase().includes(lowerQuery) ||
    node.userData?.path?.toLowerCase().includes(lowerQuery)
  );

  log(`搜索"${query}"找到${searchResults.value.length}个结果`);
};

/**
 * Find node by its position in 3D space
 * @param {Array} positionArray - [x, y, z] coordinates
 * @returns {Object|null} Found node or null
 */
const findNodeByPosition = (positionArray) => {
  if (!positionArray || positionArray.length < 3) return null;

  const position = new THREE.Vector3(
    positionArray[0],
    positionArray[1],
    positionArray[2]
  );

  return nodes.find(node =>
    node.mesh &&
    node.mesh.position.distanceTo(position) < 0.1
  )?.userData || null;
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
    case 'edge': return '连接';
    default: return '未知';
  }
};

/**
 * Focus camera on specific node
 * @param {Object} node - Node to focus on
 */
const focusOnNode = (node) => {
  if (!node || !camera || !controls) return;

  const targetPosition = node.mesh
    ? node.mesh.position.clone()
    : new THREE.Vector3(0, 0, 0);

  // Calculate camera position
  const direction = new THREE.Vector3(0, 0, 1);
  const distance = 30;
  const cameraPosition = targetPosition.clone()
    .add(direction.multiplyScalar(distance));

  // Animate camera movement
  gsap.to(camera.position, {
    x: cameraPosition.x,
    y: cameraPosition.y,
    z: cameraPosition.z,
    duration: 1,
    ease: "power2.inOut"
  });

  // Animate controls target
  gsap.to(controls.target, {
    x: targetPosition.x,
    y: targetPosition.y,
    z: targetPosition.z,
    duration: 1,
    ease: "power2.inOut"
  });

  log(`已聚焦到节点: ${node.name}`);
};

/**
 * Highlight connected nodes and edges
 * @param {Object} node - Node to highlight connections for
 */
const highlightConnectedNodes = (node) => {
  if (!node) return;

  resetNodeHighlights();

  // Highlight current node
  if (node.mesh) {
    node.mesh.material.color.setHex(0xff0000);
    node.mesh.material.emissive.setHex(0x333333);
  }

  // Highlight connected edges and nodes
  edges.forEach(edge => {
    if (edge.userData.from === node || edge.userData.to === node) {
      edge.material.color.setHex(0xff0000);
      edge.material.opacity = 1;

      const otherNode = edge.userData.from === node
        ? edge.userData.to
        : edge.userData.from;

      if (otherNode.mesh) {
        otherNode.mesh.material.color.setHex(0xff9900);
        otherNode.mesh.material.emissive.setHex(0x222222);
      }
    }
  });

  log(`已高亮${node.name}的连接节点`);
};

/**
 * Open file associated with node
 * @param {string} path - File path to open
 */
const openFile = (path) => {
  if (!path) return;

  log(`尝试打开文件: ${path}`);
  // Implementation would use Tauri or Electron APIs here
};

/**
 * Clear the Three.js scene and reset node/edge arrays
 */
const clearScene = () => {
  if (!scene) return;

  // Remove all objects and dispose resources
  while (scene.children.length > 0) {
    const obj = scene.children[0];
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

  // Reset node and edge collections
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
  // console.log('创建节点和边, data:', data);
  if (!data) return;

  console.log('开始创建节点和边...');

  // 创建节点
  data.nodes.forEach(nodeData => {
    // console.log(`处理节点: ${nodeData.name} (${nodeData.type})`);

    const geometry = new THREE.SphereGeometry(2, 32, 32); // 从 5 改为 2
    const material = new THREE.MeshPhongMaterial({
      color: getNodeColor(nodeData.type),
      emissive: 0x000000,
      shininess: 30
    });

    // console.log(`创建节点: ${nodeData.name} (${nodeData.type})`);

    // Create mesh with random initial position
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(
      Math.random() * 100 - 50,
      Math.random() * 100 - 50,
      Math.random() * 100 - 50
    );

    // Store node data and add to scene
    mesh.userData = nodeData;
    scene.add(mesh);

    // Track node in our collection
    nodes.push({
      mesh,
      userData: nodeData,
      targetPosition: mesh.position.clone()
    });

    // Add label if CSS2D is available
    if (typeof CSS2DObject !== 'undefined') {
      const label = document.createElement('div');
      label.className = 'node-label';
      label.textContent = nodeData.name;
      const labelObj = new CSS2DObject(label);
      labelObj.position.set(0, 10, 0);
      mesh.add(labelObj);
    }
  });
  // console.log(`创建了${nodes.length}个节点`);

  // Create edges between nodes
  data.edges.forEach(edgeData => {
    const fromNode = nodes.find(n => n.userData.id === edgeData.from);
    const toNode = nodes.find(n => n.userData.id === edgeData.to);

    if (fromNode && toNode) {
      // Create curved path between nodes
      const points = [
        fromNode.mesh.position,
        getControlPoint(fromNode.mesh.position, toNode.mesh.position),
        toNode.mesh.position
      ];

      const curve = new THREE.CatmullRomCurve3(points);
      const lineGeometry = new THREE.BufferGeometry().setFromPoints(
        curve.getPoints(10)
      );

      // Style edge based on connection strength
      const lineMaterial = new THREE.LineBasicMaterial({
        color: edgeData.strength > 0.5 ? 0xff5555 : 0x888888,
        opacity: 0.6,
        transparent: true
      });

      // Create line and add to scene
      const line = new THREE.Line(lineGeometry, lineMaterial);
      line.userData = {
        ...edgeData,
        from: fromNode,
        to: toNode
      };

      scene.add(line);
      edges.push(line);
    }
  });

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
  const height = Math.min(distance * 0.3, 30);

  mid.z += height;
  return mid;
};

/**
 * Apply force-directed layout to nodes
 */
const forceDirectedLayout = () => {
  log('应用力导向布局');

  // 构建 d3-force-3d 节点和边数据
  const nodeDatas = nodes.map(n => n.userData);
  const edgeDatas = edges.map(e => ({
    source: e.userData.from.userData.id,
    target: e.userData.to.userData.id
  }));

  const simulation = d3.forceSimulation(nodeDatas)
    .force('charge', d3.forceManyBody().strength(-100))
    .force('link', d3.forceLink(edgeDatas).id(d => d.id).distance(50))
    .force('center', d3.forceCenter(0, 0, 0))
    .force('collision', d3.forceCollide().radius(10));

  simulation.on('tick', () => {
    nodes.forEach((node, i) => {
      const d = nodeDatas[i];
      node.targetPosition.set(
        d.x || 0,
        d.y || 0,
        d.z || 0
      );
    });
  });

  setTimeout(() => simulation.stop(), 1000);
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

  const radius = 50;
  // Position nodes on a sphere using Fibonacci spiral
  nodes.forEach((node, i) => {
    const phi = Math.acos(-1 + (2 * i) / nodes.length);
    const theta = Math.sqrt(nodes.length * Math.PI) * phi;

    node.targetPosition.set(
      radius * Math.cos(theta) * Math.sin(phi),
      radius * Math.sin(theta) * Math.sin(phi),
      radius * Math.cos(phi)
    );
  });
};

/**
 * Apply tree layout to nodes
 */
const treeLayout = () => {
  log('应用树状布局');

  // Simple tree layout with parent-child relationships
  const rootNodes = nodes.filter(n => !n.userData.parentId);
  const childNodes = nodes.filter(n => n.userData.parentId);

  // Position root nodes in a row
  rootNodes.forEach((node, i) => {
    node.targetPosition.set(i * 30 - 50, 0, 0);
  });

  // Position child nodes below their parents
  childNodes.forEach((node, i) => {
    const parent = nodes.find(n => n.userData.id === node.userData.parentId);
    if (parent) {
      node.targetPosition.set(
        parent.targetPosition.x + (i % 3) * 10 - 10,
        parent.targetPosition.y - 20,
        parent.targetPosition.z + Math.floor(i / 3) * 10 - 10
      );
    }
  });
};

/**
 * Set up lighting for the scene
 */
const setupLighting = () => {
  // Ambient light for overall illumination
  const ambientLight = new THREE.AmbientLight(0xffffff, darkMode.value ? 0.8 : 1.0);
  scene.add(ambientLight);

  // Directional light for shadows and highlights
  const directionalLight = new THREE.DirectionalLight(0xffffff, 1);
  directionalLight.position.set(1, 1, 1);
  directionalLight.castShadow = true;
  directionalLight.shadow.mapSize.width = 2048;
  directionalLight.shadow.mapSize.height = 2048;
  scene.add(directionalLight);

  // Hemisphere light for natural outdoor lighting effect
  const hemisphereLight = new THREE.HemisphereLight(
    0xffffbb, // Sky color
    0x080820, // Ground color
    0.5 // Intensity
  );
  scene.add(hemisphereLight);

  // Point light for additional highlights
  const pointLight = new THREE.PointLight(0xffffff, 1, 50);
  pointLight.position.set(0, 0, 20);
  scene.add(pointLight);
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

  // Mouse move for hover effects
  renderer.domElement.addEventListener('mousemove', (event) => {
    mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
    mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

    // 高亮悬停的节点
    raycaster.setFromCamera(mouse, camera);

    // 只检测 mesh 和 line 对象，忽略标签等辅助对象
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

  // Click for node selection
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
    if (node.mesh && node.mesh.material && node.userData.originalColor !== undefined) {
      node.mesh.material.color.setHex(node.userData.originalColor);
      if (node.mesh.material.emissive) {
        node.mesh.material.emissive.setHex(0x000000);
      }
    }
  });

  edges.forEach(edge => {
    if (edge.material) {
      edge.material.color.setHex(0x888888);
      edge.material.opacity = 0.6;
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
  } else {
    selectedNode.value = {
      ...object.userData,
      object: object
    };
  }
};

/**
 * Build the complete knowledge graph
 */
const buildGraph = async () => {
  console.log('开始构建知识图谱...');
  isLoading.value = true;
  loadingProgress.value = 0;

  try {
    // 1. Clear existing scene
    clearScene();

    // 2. Fetch graph data
    const treeData = await fetchGraphData();
    loadingProgress.value = 30;

    console.log('获取知识图谱数据成功！');
    loadingProgress.value = 60;
    // 3. Create nodes and edges
    await createNodesAndEdges(treeData);
    loadingProgress.value = 70;
    console.log('创建节点和边成功！');

    // 4. Apply selected layout
    applyLayout(currentLayout.value);
    loadingProgress.value = 90;
    console.log('应用布局成功！');

    // 5. Initialize octree for spatial queries
    const meshes = nodes.map(n => n.mesh).filter(Boolean);
    console.log('准备初始化八叉树', meshes);
    octree = new Octree();
    octree.fromGraphNode(meshes);
    console.log('初始化八叉树成功！');

    // 添加准备检查
    if (scene.children.length === 0) {
      console.warn('场景中没有可渲染对象');
    }
    // 6. Start animation loop
    startAnimate();
    console.log('开始动画循环！');
    loadingProgress.value = 100;
  } catch (err) {
    showError.value = true;
    errorMessage.value = `构建失败: ${err.message}`;
  } finally {
    isLoading.value = false;
    resetCamera();
  }
};

const getNodeColor = (type) => {
  switch (type) {
    case 'knowledge-base': return 0xf39c12; // 橙色
    case 'directory': return 0x3498db;     // 蓝色
    case 'file': return 0x2ecc71;         // 绿色
    default: return 0xcccccc;             // 默认灰色
  }
}

/**
 * Apply selected layout algorithm
 * @param {string} layoutType - Layout type to apply
 */
const applyLayout = (layoutType) => {
  console.log(`应用布局: ${layoutType}`);
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
};

const startAnimate = () => {
  console.log('startAnimate 开始动画');
  if (animationFrameId) {
    console.warn('startAnimate 动画循环已在运行，取消之前的循环');
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  animate();
};

/**
 * Animation loop for continuous rendering
 */
let animationFrameId = null;

const animate = () => {
  console.log('animate 循环动画');
  if (!renderer || !scene || !camera) {
    animationFrameId = null;
    return;
  }

  animationFrameId = requestAnimationFrame(animate);

  nodes.forEach(node => {
    if (node.mesh && node.targetPosition) {
      node.mesh.position.lerp(node.targetPosition, 0.2);
    }
  });

  if (controls) controls.update();

  renderer.render(scene, camera);
  labelRenderer?.render(scene, camera);

  console.log('animate running, scene.children.length:', scene.children.length);

};


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
  const material = new THREE.MeshBasicMaterial({
    color: 0x3498db,
    wireframe: true,
    transparent: true,
    opacity: 0.6
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
  background-color: rgba(255, 255, 255, 0.9);
  border-bottom: 1px solid #e0e0e0;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Dark mode toolbar variation */
.dark-mode .toolbar {
  background-color: rgba(30, 30, 30, 0.9);
  border-bottom-color: #333;
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
  display: block;
  position: absolute;
  top: 0;
  left: 0;
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
  bottom: 20px;
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

/* Legend panel styling */
.legend-panel {
  position: absolute;
  top: 60px;
  left: 20px;
  width: 150px;
  z-index: 10;
  background-color: rgba(255, 255, 255, 0.9);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Dark mode legend panel */
.dark-mode .legend-panel {
  background-color: rgba(30, 30, 30, 0.9);
}

/* Legend item styling */
.legend-item {
  display: flex;
  align-items: center;
  margin: 4px 0;
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

/* Line indicators in legend */
.legend-line {
  width: 24px;
  height: 2px;
  margin-right: 8px;
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

/* Search panel styling */
.search-panel {
  position: absolute;
  top: 60px;
  right: 20px;
  width: 250px;
  z-index: 10;
  background-color: rgba(255, 255, 255, 0.9);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Dark mode search panel */
.dark-mode .search-panel {
  background-color: rgba(30, 30, 30, 0.9);
}

/* Search results list */
.search-results {
  max-height: 300px;
  overflow-y: auto;
  margin-top: 8px;
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