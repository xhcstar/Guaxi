<template>
  <!-- Main container with dark mode support -->
  <div class="knowledge-graph-container" :class="{ 'dark-mode': darkMode }">
    <!-- Toolbar section with graph controls -->
    <div class="toolbar">
    <n-space align="center" size="small">
      <!-- Reset camera view button -->
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button size="small" secondary circle @click="resetCamera">
            <template #icon><n-icon>
                <CompassOutline />
              </n-icon></template>
          </n-button>
        </template>
        重置视角
      </n-tooltip>

      <!-- Toggle auto-rotation button -->
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button size="small" secondary circle :type="autoRotate ? 'primary' : 'default'" @click="toggleAutoRotate">
            <template #icon><n-icon>
                <SyncOutline />
              </n-icon></template>
          </n-button>
        </template>
        {{ autoRotate ? '停止旋转' : '自动旋转' }}
      </n-tooltip>

      <!-- Toggle dark mode button -->
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button size="small" secondary circle @click="toggleDarkMode">
            <template #icon><n-icon>
                <MoonOutline />
              </n-icon></template>
          </n-button>
        </template>
        {{ darkMode ? '亮色模式' : '暗色模式' }}
      </n-tooltip>

      <!-- Layout selection popover -->
      <n-popover trigger="click" placement="bottom">
        <template #trigger>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary circle>
                <template #icon><n-icon>
                    <LayersOutline />
                  </n-icon></template>
              </n-button>
            </template>
            布局设置
          </n-tooltip>
        </template>
        <div style="width: 160px; padding: 4px;">
          <div style="margin-bottom: 8px; font-weight: 500; font-size: 12px;">布局模式</div>
          <n-select v-model:value="currentLayout" :options="layoutOptions" size="small" />
        </div>
      </n-popover>

      <!-- Performance level popover -->
      <n-popover trigger="click" placement="bottom">
        <template #trigger>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary circle>
                <template #icon><n-icon>
                    <SpeedometerOutline />
                  </n-icon></template>
              </n-button>
            </template>
            性能设置
          </n-tooltip>
        </template>
        <div style="width: 200px; padding: 4px;">
          <div style="margin-bottom: 8px; font-weight: 500; font-size: 12px;">渲染质量</div>
          <n-slider v-model:value="performanceLevel" :step="1" :min="0" :max="2" :marks="performanceMarks" />
        </div>
      </n-popover>

      <!-- Glow effect popover -->
      <n-popover trigger="click" placement="bottom">
        <template #trigger>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary circle :type="glowEnabled ? 'primary' : 'default'">
                <template #icon><n-icon>
                    <SparklesOutline />
                  </n-icon></template>
              </n-button>
            </template>
            视觉特效
          </n-tooltip>
        </template>
        <div style="width: 220px; padding: 4px;">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
            <span style="font-weight: 500; font-size: 12px;">星光特效</span>
            <n-switch v-model:value="glowEnabled" size="small" />
          </div>
          <div v-if="glowEnabled">
            <div style="font-size: 11px; color: var(--guaxi-text-secondary); margin-bottom: 4px;">强度</div>
            <n-slider v-model:value="glowStrength" :min="0" :max="2.5" :step="0.05" />
            <div style="font-size: 11px; color: var(--guaxi-text-secondary); margin: 12px 0 4px;">光晕大小</div>
            <n-slider v-model:value="glowSize" :min="6" :max="44" :step="1" />
          </div>
        </div>
      </n-popover>
    </n-space>
  </div>

    <!-- Three.js rendering container -->
    <div ref="container" class="graph-container"></div>

    <!-- Loading overlay shown during data loading -->
    <div v-if="isLoading" class="loading-overlay">
      <n-spin size="large" />
      <n-progress type="circle" :percentage="loadingProgress" :indicator-placement="'inside'" :show-indicator="false" />
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

      <div class="navigation-tools">
        <n-button size="tiny" @click="resetCamera">
          <template #icon><n-icon>
              <CompassOutline />
            </n-icon></template>
          重置视角
        </n-button>
        <n-button size="tiny" @click="toggleAutoRotate">
          <template #icon><n-icon>
              <SyncOutline />
            </n-icon></template>
          {{ autoRotate ? '停止旋转' : '自动旋转' }}
        </n-button>
      </div>
    </div>


  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
// Vue composition API imports
import { ref, onMounted, onBeforeUnmount, watch, computed, nextTick } from 'vue';

// Three.js core and extensions
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass';
import { CSS2DRenderer, CSS2DObject } from 'three/examples/jsm/renderers/CSS2DRenderer';
import { gsap } from "gsap";
import Stats from 'three/examples/jsm/libs/stats.module';
import * as d3 from 'd3-force-3d';

// Tauri API for backend communication
import { invoke } from '@tauri-apps/api/core';

// Pinia store for state management
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { useEditorStore } from '../../stores/editorStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { storeToRefs } from 'pinia';

// UI components from Naive UI
import {
  NButton, NSpace, NIcon, NCard, NText, NAlert, NSpin,
  NSelect, NSlider, NProgress, NSwitch, NTooltip, NPopover
} from 'naive-ui';

// Icons from Ionicons
import { 
  CompassOutline, 
  SyncOutline, 
  MoonOutline, 
  EyeOutline,
  LayersOutline,
  SpeedometerOutline,
  SparklesOutline
} from '@vicons/ionicons5';

const logger = createLogger('GuaxiKnowledgeBaseGraph')
// Initialize Pinia store
const store = useKnowledgeBaseStore();
const fileTreeStore = useFileTreeStore();
const editorStore = useEditorStore();
const settingsStore = useSettingsStore();

// Destructure store properties with reactivity
const { knowledgeBases, selectedKnowledgeBase, visibleTypes, selectedTags, searchQuery, focusTarget, selectedGraphNode } = storeToRefs(store);

// Computed property for currently selected knowledge base
const selectedKB = computed(() => {
  return selectedKnowledgeBase.value ||
    (knowledgeBases.value.length > 0 ? knowledgeBases.value[0] : {});
});

// Debug configuration
const DEBUG = true;
const log = (...args) => DEBUG && logger.info('[KnowledgeGraph]', ...args);
const warn = (...args) => DEBUG && logger.warn('[KnowledgeGraph]', ...args);
const error = (...args) => DEBUG && logger.error('[KnowledgeGraph]', ...args);

// Reactive state variables
const container = ref(null); // Reference to DOM container
const autoRotate = ref(true); // Auto-rotation toggle state
const darkMode = ref(document.documentElement?.dataset?.guaxiTheme === 'dark'); // Dark mode toggle state
const selectedNode = ref(null); // Currently selected node
const isLoading = ref(false); // Loading state
const loadingProgress = ref(0); // Loading progress percentage
const showError = ref(false); // Error display state
const errorMessage = ref(''); // Error message content
const currentLayout = ref('force'); // Current graph layout type
const performanceLevel = ref(1); // Performance quality level
const pendingKnowledgeBaseChange = ref(null); // Pending KB change
const isUserInteracting = ref(false);
const glowEnabled = ref(true);
const glowStrength = ref(0.9);
const glowSize = ref(16);
const isFocusMode = ref(false);
const focusContext = ref(null);

let activeCameraTween = null;
let initialViewSnapshot = null;

const killActiveCameraTween = () => {
  if (activeCameraTween) {
    activeCameraTween.kill();
    activeCameraTween = null;
  }
};

const captureInitialSnapshot = (pose) => {
  if (!camera || !controls || !nodes.length || !pose) return;
  const cam = pose.cameraPosition;
  const target = pose.target;
  if (!cam || !target || !isFiniteVec3(cam) || !isFiniteVec3(target)) return;

  initialViewSnapshot = {
    cameraPosition: cam.clone(),
    target: target.clone(),
    minDistance: Number.isFinite(controls.minDistance) ? controls.minDistance : undefined,
    maxDistance: Number.isFinite(controls.maxDistance) ? controls.maxDistance : undefined,
    layout: currentLayout.value
  };

  for (const n of nodes) {
    const tp = n?.targetPosition;
    if (tp && isFiniteVec3(tp)) {
      n.__initialTargetPosition = tp.clone();
    } else if (n?.mesh?.position && isFiniteVec3(n.mesh.position)) {
      n.__initialTargetPosition = n.mesh.position.clone();
    } else {
      n.__initialTargetPosition = null;
    }
  }
};

const restoreInitialNodes = () => {
  if (!nodes.length) return false;
  let restored = false;
  for (const n of nodes) {
    const p = n?.__initialTargetPosition;
    const mesh = n?.mesh;
    if (!p || !isFiniteVec3(p) || !mesh) continue;
    if (n?.targetPosition) n.targetPosition.copy(p);
    if (mesh?.position) mesh.position.copy(p);
    restored = true;
  }
  if (restored) {
    for (const e of edges) e?.userData?.update?.();
  }
  return restored;
};

// Track active D3 simulation
let activeSimulation = null;

const stopActiveSimulation = () => {
  if (activeSimulation) {
    activeSimulation.stop();
    activeSimulation = null;
  }
};

// Computed property to check if current KB is valid
const hasValidKnowledgeBase = computed(() => {
  return isValidKnowledgeBase(selectedKB.value);
});

// Layout options for dropdown
const layoutOptions = [
  { label: '力导向布局', value: 'force' },
  { label: '层次布局', value: 'hierarchical' },
  { label: '球形布局', value: 'spherical' }
];

// Performance level marks for slider
const performanceMarks = {
  0: '性能优先',
  1: '平衡',
  2: '质量优先'
};

const layoutViewConfig = {
  force: {
    biasY: -0.75,
    minDistanceFactor: 0.08,
    maxDistanceFactor: 20,
    maxRadius: 800
  },
  spherical: {
    biasY: -0.6,
    minDistanceFactor: 0.12,
    maxDistanceFactor: 14,
    maxRadius: 600
  }
};

// Three.js variables (non-reactive)
let scene, camera, renderer, controls, raycaster, composer, labelRenderer;
let nodes = [], edges = [], octree;
let buildGraphRunId = 0;
let ensureViewTimer = null;

class GraphOctree {
  constructor(cellSize = 8) {
    this.cellSize = Math.max(1, Number(cellSize) || 8);
    this.cells = new Map();
  }

  _cellKeyFromVec3(v) {
    const cs = this.cellSize;
    const ix = Math.round(v.x / cs);
    const iy = Math.round(v.y / cs);
    const iz = Math.round(v.z / cs);
    return `${ix}|${iy}|${iz}`;
  }

  addGraphNode(mesh) {
    if (!mesh?.position) return;
    const key = this._cellKeyFromVec3(mesh.position);
    let arr = this.cells.get(key);
    if (!arr) {
      arr = [];
      this.cells.set(key, arr);
    }
    arr.push(mesh);
  }

  clear() {
    this.cells.clear();
  }

  search(position, radius = 5) {
    if (!position) return [];
    const r = Math.max(0, Number(radius) || 0);
    const cs = this.cellSize;
    const steps = Math.max(0, Math.ceil(r / cs));

    const ix0 = Math.round(position.x / cs);
    const iy0 = Math.round(position.y / cs);
    const iz0 = Math.round(position.z / cs);

    const hits = [];
    const r2 = r * r;

    for (let dx = -steps; dx <= steps; dx++) {
      for (let dy = -steps; dy <= steps; dy++) {
        for (let dz = -steps; dz <= steps; dz++) {
          const key = `${ix0 + dx}|${iy0 + dy}|${iz0 + dz}`;
          const arr = this.cells.get(key);
          if (!arr) continue;
          for (const mesh of arr) {
            const p = mesh?.position;
            if (!p) continue;
            const d2 = position.distanceToSquared(p);
            if (d2 <= r2) hits.push({ mesh, d2 });
          }
        }
      }
    }

    hits.sort((a, b) => a.d2 - b.d2);
    return hits.map(h => h.mesh);
  }
}

let containerResizeObserver = null;
const mouse = new THREE.Vector2(); // Mouse position tracker

const followPointer = new THREE.Vector2(0, 0);
const followCurrentOffset = new THREE.Vector3(0, 0, 0);
let followEnabled = true;

// 漫游控制（键盘）
// - WASD: 前后左右
// - Q/E: 下/上
// - Shift: 加速
// - F: 聚焦选中节点
// - R: 重置视角
// - Space: 切换自动旋转
// - Esc: 取消选择
// - G: 切换漫游开关
const roamEnabled = ref(true);
const roamBaseSpeed = ref(220);
const roamBoostMultiplier = ref(3);
const roamKeysDown = new Set();
let roamLastTs = 0;

const shouldIgnoreKeyEvent = (event) => {
  const el = event?.target;
  if (!el) return false;
  const tag = String(el.tagName || '').toUpperCase();
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return !!el.isContentEditable;
};

const handleKeyDown = (event) => {
  if (!event || shouldIgnoreKeyEvent(event)) return;

  if (event.code === 'Escape') {
    selectedNode.value = null;
    isFocusMode.value = false;
    store.setSelectedGraphNode(null);
    resetNodeHighlights();
    log('键盘: 取消选择');
    return;
  }

  if (event.code === 'Space') {
    event.preventDefault();
    toggleAutoRotate();
    return;
  }

  if (event.code === 'KeyR') {
    resetCamera();
    log('键盘: 重置视角');
    return;
  }

  if (event.code === 'KeyG') {
    roamEnabled.value = !roamEnabled.value;
    roamKeysDown.clear();
    log(`键盘: 漫游已${roamEnabled.value ? '开启' : '关闭'}`);
    return;
  }

  if (event.code === 'KeyF') {
    if (selectedNode.value) {
      focusOnNode(selectedNode.value);
      log('键盘: 聚焦到选中节点');
    }
    return;
  }

  if (event.repeat) return;
  roamKeysDown.add(event.code);
};

const handleKeyUp = (event) => {
  if (!event) return;
  roamKeysDown.delete(event.code);
};

const applyRoamMovement = () => {
  if (!roamEnabled.value || !camera || !controls) return;

  const now = performance.now();
  if (!roamLastTs) {
    roamLastTs = now;
    return;
  }

  const dt = Math.min(0.05, Math.max(0, (now - roamLastTs) / 1000));
  roamLastTs = now;
  if (dt <= 0 || roamKeysDown.size === 0) return;

  const forward = new THREE.Vector3();
  camera.getWorldDirection(forward);
  forward.normalize();

  const right = new THREE.Vector3().crossVectors(forward, camera.up).normalize();
  const up = camera.up.clone().normalize();

  const move = new THREE.Vector3(0, 0, 0);
  if (roamKeysDown.has('KeyW')) move.add(forward);
  if (roamKeysDown.has('KeyS')) move.sub(forward);
  if (roamKeysDown.has('KeyD')) move.add(right);
  if (roamKeysDown.has('KeyA')) move.sub(right);
  if (roamKeysDown.has('KeyE')) move.add(up);
  if (roamKeysDown.has('KeyQ')) move.sub(up);

  if (move.lengthSq() < 1e-8) return;
  move.normalize();

  const dist = camera.position.distanceTo(controls.target);
  const base = Math.max(10, Number(roamBaseSpeed.value) || 220);
  const scaled = base * THREE.MathUtils.clamp(dist / 300, 0.25, 4);
  const boost = (roamKeysDown.has('ShiftLeft') || roamKeysDown.has('ShiftRight'))
    ? Math.max(1, Number(roamBoostMultiplier.value) || 3)
    : 1;

  const step = scaled * boost * dt;
  move.multiplyScalar(step);

  camera.position.add(move);
  controls.target.add(move);
  controls.update();

  ensureViewInBounds({ mode: 'graph', recenter: false, useTargetPositions: true });
  clampViewToGraphBounds({ useTargetPositions: true, soft: true });
};

const _followForward = new THREE.Vector3();
const _followRight = new THREE.Vector3();
const _followUp = new THREE.Vector3();
const _followDesired = new THREE.Vector3();
const _followPrev = new THREE.Vector3();
const _followDelta = new THREE.Vector3();

const _keepOff = new THREE.Vector3();
const _keepDesiredTarget = new THREE.Vector3();
const _keepDelta = new THREE.Vector3();

const isFiniteVec3 = (v) => {
  const x = v?.x, y = v?.y, z = v?.z;
  return Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z);
};

const updateCameraClipping = (center, radius) => {
  if (!camera) return;

  const r = Number.isFinite(radius) && radius > 0 ? radius : 1;
  const c = center && isFiniteVec3(center) ? center : new THREE.Vector3(0, 0, 0);

  const dist = camera.position.distanceTo(c);
  // 动态调整远剪裁面，确保能看到整个场景
  const far = THREE.MathUtils.clamp(dist + r * 8, 2000, 5_000_000);
  // 动态调整近剪裁面，避免过近导致闪烁，但也不能太远
  const near = Math.max(0.05, far / 200_000);

  if (Math.abs((camera.near || 0) - near) > 1e-6 || Math.abs((camera.far || 0) - far) > 1e-6) {
    camera.near = near;
    camera.far = far;
    camera.updateProjectionMatrix();
  }
};

/**
 * Initialize Three.js scene, renderer, camera and controls
 */
const initThreeJS = () => {
  // 1. Create scene with background and fog
  scene = new THREE.Scene();
  scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xbbbbbb);
  scene.fog = new THREE.FogExp2(scene.background, 0.002);

  logger.info('[initThreeJS]');
  // 2. Create perspective camera
  camera = new THREE.PerspectiveCamera(
    60, // Field of view
    container.value.clientWidth / container.value.clientHeight, // Aspect ratio
    0.1, // Near clip
    10000 // Far clip
  );
  camera.position.set(120, 140, 160); // Initial camera position (match resetCamera)

  logger.info('[initThreeJS]', 'camera', camera);
  // 3. Create WebGL renderer with performance settings
  renderer = new THREE.WebGLRenderer({
    antialias: true,
    powerPreference: "high-performance",
    logarithmicDepthBuffer: true
  });

  if (!renderer.getContext()) {
    logger.error('WebGL 渲染器初始化失败');
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
  logger.info("renderer", renderer);

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
  logger.info('composer', composer);

  if (camera && renderer?.domElement) {
    controls = new OrbitControls(camera, renderer.domElement);
    controls.autoRotate = false;
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.enableZoom = true;
    controls.zoomSpeed = 0.9;

    // 漫游式交互：左键旋转，右键平移，滚轮缩放
    controls.enablePan = true;
    controls.panSpeed = 0.75;
    controls.mouseButtons = {
      LEFT: THREE.MOUSE.ROTATE,
      MIDDLE: THREE.MOUSE.DOLLY,
      RIGHT: THREE.MOUSE.PAN
    };

    // 键盘漫游由自定义逻辑处理，避免和 OrbitControls 自带键盘互相干扰
    controls.enableKeys = false;

    controls.screenSpacePanning = false;
    controls.minDistance = 10;
    controls.maxDistance = 4000;

    controls.target.set(0, 0, 0);
    controls.update();

    controls.addEventListener('start', () => {
      isUserInteracting.value = true;
    });

    controls.addEventListener('end', () => {
      isUserInteracting.value = false;
      ensureViewInBounds({ mode: 'graph', recenter: false, useTargetPositions: true });
      clampViewToGraphBounds({ useTargetPositions: true, soft: true });
    });

    // 移除 change 事件监听，避免死循环导致的视角漂移
    // controls.addEventListener('change', () => { ... });
  } else {
    logger.error('初始化控制器失败: camera或renderer.domElement未定义');
  }

  // Add bloom effect for glowing highlights
  const bloomPass = new UnrealBloomPass(
    new THREE.Vector2(window.innerWidth, window.innerHeight),
    1.5,
    0.4,
    0.85
  );
  bloomPass.enabled = glowEnabled.value;
  bloomPass.strength = (Number(glowStrength.value) || 0) * 0.5;
  composer.addPass(bloomPass);
  logger.info('bloomPass', bloomPass);

  watch([glowEnabled, glowStrength, glowSize], () => {
    if (!bloomPass) return;
    bloomPass.enabled = !!glowEnabled.value;
    bloomPass.strength = glowEnabled.value ? (Number(glowStrength.value) || 0) * 0.6 : 0;

    nodes.forEach((n) => applyStarGlow(n));
  });

  setupLighting();
  logger.info('setupLighting', setupLighting)
  raycaster = new THREE.Raycaster();

  logger.info('raycaster', raycaster)
  // 9. Set up event listeners
  setupEventListeners();
  logger.info('setupEventListeners', setupEventListeners)
};

const getVisibleBoundingSphere = () => {
  if (!nodes.length) return null;

  const box = new THREE.Box3();
  let hasAny = false;

  for (const n of nodes) {
    const mesh = n?.mesh;
    if (!mesh || !mesh.visible) continue;
    if (!isFiniteVec3(mesh.position)) continue; // Skip invalid positions
    box.expandByObject(mesh);
    hasAny = true;
  }

  if (!hasAny) return null;

  const sphere = new THREE.Sphere();
  box.getBoundingSphere(sphere);

  if (!Number.isFinite(sphere.radius) || sphere.radius <= 0) sphere.radius = 1;
  return sphere;
};

const getVisibleTargetBoundingSphere = () => {
  if (!nodes.length) return null;

  const box = new THREE.Box3();
  let hasAny = false;

  for (const n of nodes) {
    const mesh = n?.mesh;
    const p = n?.targetPosition;
    if (!mesh || !mesh.visible || !p) continue;
    if (!isFiniteVec3(p)) continue; // Skip invalid positions
    box.expandByPoint(p);
    hasAny = true;
  }

  if (!hasAny) return null;

  const pad = 4;
  box.min.addScalar(-pad);
  box.max.addScalar(pad);

  const sphere = new THREE.Sphere();
  box.getBoundingSphere(sphere);

  if (!Number.isFinite(sphere.radius) || sphere.radius <= 0) sphere.radius = 1;
  return sphere;
};

const ensureViewInBounds = (options = {}) => {
  if (!camera || !controls) return;

  const mode = options.mode || 'auto';
  const recenter = options.recenter !== false;
  const useTargetPositions = options.useTargetPositions === true;

  let sphere = useTargetPositions
    ? (getVisibleTargetBoundingSphere() || getVisibleBoundingSphere())
    : getVisibleBoundingSphere();
  if (!sphere) return;

  const cfg = layoutViewConfig[currentLayout?.value] || layoutViewConfig.force;
  const maxRadius = Number(cfg.maxRadius) > 0 ? cfg.maxRadius : Infinity;
  const minRadius = Number(cfg.minRadius) > 0 ? cfg.minRadius : 1;

  let radius = Math.max(1, sphere.radius);
  radius = THREE.MathUtils.clamp(radius, minRadius, maxRadius);

  let center = sphere.center.clone();
  const bias = Number.isFinite(cfg.biasY) ? cfg.biasY : 0;
  const rawBiasOff = radius * bias;
  const biasOff = THREE.MathUtils.clamp(rawBiasOff, -1200, 1200);
  if (Math.abs(rawBiasOff - biasOff) > 1e-6) {
    warn('视野偏置已钳制', { rawBiasOff, biasOff, radius, bias, layout: currentLayout?.value });
  }
  center.y += biasOff;

  if (mode === 'auto' || mode === 'node') {
    const sel = selectedNode.value;
    if (sel && sel.type !== 'edge') {
      const gn = resolveGraphNode(sel);
      const p = gn?.mesh?.position;
      if (p) center = p.clone();
    }
  }

  const minFactor = Number(cfg.minDistanceFactor) > 0 ? cfg.minDistanceFactor : 0.08;
  const maxFactor = Number(cfg.maxDistanceFactor) > 0 ? cfg.maxDistanceFactor : 20;

  const minD = Math.max(2, Math.min(radius * minFactor, 12));
  const maxD = Math.max(minD + 20, radius * maxFactor);
  controls.minDistance = minD;
  controls.maxDistance = maxD;

  if (recenter) {
    const targetDelta = center.clone().sub(controls.target);
    if (targetDelta.lengthSq() > 1e-6) {
      controls.target.add(targetDelta);
      camera.position.add(targetDelta);
    }
  }

  // 更新相机的剪裁面，确保视野内的物体不被剪裁
  updateCameraClipping(center, radius);

  // 7. 让 OrbitControls 自动处理距离约束，不再手动强制设置 camera.position
  // 这样可以避免与 controls.update() 的冲突和死循环
  controls.update();
};

const clampViewToGraphBounds = (options = {}) => {
  if (!camera || !controls || !nodes.length) return;

  const useTargetPositions = options.useTargetPositions !== false;
  const soft = options.soft !== false;

  const sphere = useTargetPositions
    ? (getVisibleTargetBoundingSphere() || getVisibleBoundingSphere())
    : getVisibleBoundingSphere();
  if (!sphere) return;

  const cfg = layoutViewConfig[currentLayout?.value] || layoutViewConfig.force;
  const maxRadius = Number(cfg.maxRadius) > 0 ? cfg.maxRadius : Infinity;
  const minRadius = Number(cfg.minRadius) > 0 ? cfg.minRadius : 1;

  const rawRadius = Math.max(1, sphere.radius);
  const radius = THREE.MathUtils.clamp(rawRadius, minRadius, maxRadius);
  const center = sphere.center.clone();

  const maxCamDist = Math.max(80, radius * 6);
  const maxTargetDist = Math.max(30, radius * 2.2);

  let changed = false;

  _keepOff.copy(controls.target).sub(center);
  const tLen = _keepOff.length();
  if (tLen > maxTargetDist && tLen > 1e-6) {
    _keepOff.multiplyScalar(maxTargetDist / tLen);
    _keepDesiredTarget.copy(center).add(_keepOff);
    _keepDelta.copy(_keepDesiredTarget).sub(controls.target);
    if (soft) _keepDelta.multiplyScalar(0.35);
    controls.target.add(_keepDelta);
    camera.position.add(_keepDelta);
    changed = true;
  }

  _followDelta.copy(camera.position).sub(center);
  const cLen = _followDelta.length();
  if (cLen > maxCamDist && cLen > 1e-6) {
    _followDelta.multiplyScalar(maxCamDist / cLen);
    _followDesired.copy(center).add(_followDelta);
    _followPrev.copy(_followDesired).sub(camera.position);
    if (soft) _followPrev.multiplyScalar(0.35);
    camera.position.add(_followPrev);
    controls.target.add(_followPrev);
    changed = true;
  }

  if (changed) {
    updateCameraClipping(center, radius);
    controls.update();
  }
};

/**
 * Frame all nodes in view
 */
const frameAllNodes = (options = {}) => {
  if (!camera || !controls || !nodes.length) return;

  killActiveCameraTween();

  const useTargetPositions = options.useTargetPositions !== false;
  const captureBaseline = options.captureBaseline === true;
  const sphere = useTargetPositions ? getVisibleTargetBoundingSphere() : getVisibleBoundingSphere();
  if (!sphere) return;

  const cfg = layoutViewConfig[currentLayout?.value] || layoutViewConfig.force;
  const maxRadius = Number(cfg.maxRadius) > 0 ? cfg.maxRadius : Infinity;
  const minRadius = Number(cfg.minRadius) > 0 ? cfg.minRadius : 1;

  const rawRadius = Math.max(1, sphere.radius);
  const radius = THREE.MathUtils.clamp(rawRadius, minRadius, maxRadius);

  const center = sphere.center.clone();
  const bias = Number.isFinite(cfg.biasY) ? cfg.biasY : 0;
  const rawBiasOff = radius * bias;
  const biasOff = THREE.MathUtils.clamp(rawBiasOff, -1200, 1200);
  if (Math.abs(rawBiasOff - biasOff) > 1e-6) {
    warn('Frame 视野偏置已钳制', { rawBiasOff, biasOff, radius, bias, layout: currentLayout?.value });
  }
  center.y += biasOff;

  const dir = camera.position.clone().sub(controls.target);
  if (dir.lengthSq() < 1e-6) dir.set(0, 0, 1);
  dir.normalize();

  const vFov = THREE.MathUtils.degToRad(camera.fov || 60);
  const aspect = Number(camera.aspect) > 0 ? Number(camera.aspect) : (container.value?.clientWidth && container.value?.clientHeight ? (container.value.clientWidth / container.value.clientHeight) : 1);
  const hFov = 2 * Math.atan(Math.tan(vFov / 2) * aspect);
  const fitFov = Math.min(vFov, hFov);

  const rawPad = Number(cfg.framePadding) > 0 ? cfg.framePadding : 1.18;
  const padding = Math.max(rawPad, 1.26);
  const distance = (radius / Math.max(1e-6, Math.sin(fitFov / 2))) * padding;
  const cameraPosition = center.clone().add(dir.multiplyScalar(distance));

  updateCameraClipping(center, radius);

  if (captureBaseline) {
    captureInitialSnapshot({ cameraPosition, target: center });
  }

  activeCameraTween = gsap.timeline({
    defaults: { duration: 0.6, ease: 'power2.out' },
    onUpdate: () => {
      controls.update();
      updateCameraClipping(center, radius);
    },
    onComplete: () => {
      activeCameraTween = null;
    }
  })
    .to(camera.position, { x: cameraPosition.x, y: cameraPosition.y, z: cameraPosition.z }, 0)
    .to(controls.target, { x: center.x, y: center.y, z: center.z }, 0);
};

/**
 * Reset camera to default position and orientation (Animated)
 */
const resetCamera = () => {
  if (!camera || !controls) return;

  killActiveCameraTween();
  isFocusMode.value = false;

  if (initialViewSnapshot?.cameraPosition && initialViewSnapshot?.target) {
    restoreInitialNodes();

    if (Number.isFinite(initialViewSnapshot.minDistance)) controls.minDistance = initialViewSnapshot.minDistance;
    if (Number.isFinite(initialViewSnapshot.maxDistance)) controls.maxDistance = initialViewSnapshot.maxDistance;

    const sphere = getVisibleTargetBoundingSphere() || getVisibleBoundingSphere();
    if (sphere) {
      const cfg = layoutViewConfig[currentLayout?.value] || layoutViewConfig.force;
      const maxRadius = Number(cfg.maxRadius) > 0 ? cfg.maxRadius : Infinity;
      const minRadius = Number(cfg.minRadius) > 0 ? cfg.minRadius : 1;

      const radius = THREE.MathUtils.clamp(sphere.radius, minRadius, maxRadius);
      const center = sphere.center.clone();

      const dir = new THREE.Vector3(120, 140, 160).normalize();

      const vFov = THREE.MathUtils.degToRad(camera.fov || 60);
      const aspect = Number(camera.aspect) > 0 ? Number(camera.aspect) : (container.value?.clientWidth / container.value?.clientHeight) || 1;
      const hFov = 2 * Math.atan(Math.tan(vFov / 2) * aspect);
      const fitFov = Math.min(vFov, hFov);

      const rawPad = Number(cfg.framePadding) > 0 ? cfg.framePadding : 1.18;
      const padding = Math.max(rawPad, 1.26);
      const distance = (radius / Math.max(1e-6, Math.sin(fitFov / 2))) * padding;

      const finalCamPos = center.clone().add(dir.multiplyScalar(distance));

      updateCameraClipping(center, radius);

      activeCameraTween = gsap.timeline({
        defaults: { duration: 0.85, ease: 'power2.inOut' },
        onUpdate: () => {
          controls.update();
          updateCameraClipping(center, radius);
        },
        onComplete: () => {
          activeCameraTween = null;
          clampViewToGraphBounds({ useTargetPositions: true, soft: false });
        }
      })
        .to(controls.target, { x: center.x, y: center.y, z: center.z }, 0)
        .to(camera.position, { x: finalCamPos.x, y: finalCamPos.y, z: finalCamPos.z }, 0);

      log('相机已重置 (Snapshot+Center)');
      return;
    }

    const target = initialViewSnapshot.target.clone();
    const camPos = initialViewSnapshot.cameraPosition.clone();

    if (!isFiniteVec3(target)) target.set(0, 0, 0);
    if (!isFiniteVec3(camPos)) camPos.set(120, 140, 160);

    activeCameraTween = gsap.timeline({
      defaults: { duration: 0.8, ease: 'power2.inOut' },
      onUpdate: () => {
        controls.update();
      },
      onComplete: () => {
        activeCameraTween = null;
      }
    })
      .to(controls.target, { x: target.x, y: target.y, z: target.z }, 0)
      .to(camera.position, { x: camPos.x, y: camPos.y, z: camPos.z }, 0);

    log('相机已重置 (Snapshot)');
    return;
  }

  if (!isFiniteVec3(camera.position)) camera.position.set(120, 140, 160);
  if (!isFiniteVec3(controls.target)) controls.target.set(0, 0, 0);

  const sphere = getVisibleTargetBoundingSphere() || getVisibleBoundingSphere();
  if (!sphere) {
    activeCameraTween = gsap.timeline({
      defaults: { duration: 0.8, ease: 'power2.inOut' },
      onUpdate: () => controls.update(),
      onComplete: () => {
        activeCameraTween = null;
      }
    })
      .to(controls.target, { x: 0, y: 0, z: 0 }, 0)
      .to(camera.position, { x: 120, y: 140, z: 160 }, 0);
    return;
  }

  const cfg = layoutViewConfig[currentLayout?.value] || layoutViewConfig.force;
  const maxRadius = Number(cfg.maxRadius) > 0 ? cfg.maxRadius : Infinity;
  const minRadius = Number(cfg.minRadius) > 0 ? cfg.minRadius : 1;

  const radius = THREE.MathUtils.clamp(sphere.radius, minRadius, maxRadius);

  const center = sphere.center.clone();

  const dir = new THREE.Vector3(120, 140, 160).normalize();

  const vFov = THREE.MathUtils.degToRad(camera.fov || 60);
  const aspect = Number(camera.aspect) > 0 ? Number(camera.aspect) : (container.value?.clientWidth / container.value?.clientHeight) || 1;
  const hFov = 2 * Math.atan(Math.tan(vFov / 2) * aspect);
  const fitFov = Math.min(vFov, hFov);

  const padding = Number(cfg.framePadding) > 0 ? cfg.framePadding : 1.18;
  const distance = (radius / Math.max(1e-6, Math.sin(fitFov / 2))) * padding;

  const finalCamPos = center.clone().add(dir.multiplyScalar(distance));

  updateCameraClipping(center, radius);

  activeCameraTween = gsap.timeline({
    defaults: { duration: 0.8, ease: 'power2.inOut' },
    onUpdate: () => {
      controls.update();
      updateCameraClipping(center, radius);
    },
    onComplete: () => {
      activeCameraTween = null;
      clampViewToGraphBounds({ useTargetPositions: true, soft: false });
    }
  })
    .to(controls.target, { x: center.x, y: center.y, z: center.z }, 0)
    .to(camera.position, { x: finalCamPos.x, y: finalCamPos.y, z: finalCamPos.z }, 0);

  log('相机已重置 (Animated)');
};

/**
 * Toggle auto-rotation of the graph
 */
const toggleAutoRotate = () => {
  autoRotate.value = !autoRotate.value;
  log(`节点自转已${autoRotate.value ? '开启' : '关闭'}`);
};

/**
 * Toggle between dark and light mode
 */
const applyDarkModeTheme = () => {
  if (scene) {
    scene.background = new THREE.Color(darkMode.value ? 0x121212 : 0xbbbbbb);
    if (scene.fog?.color) scene.fog.color.copy(scene.background);
  }

  nodes.forEach((node) => {
    if (node.mesh?.material) {
      const c = getNodeColor(node.userData.type);
      node.userData.originalColor = c;
      node.mesh.material.color.setHex(c);

      const baseIntensity = darkMode.value ? 0.1 : 0.08;
      const glowBonus = (Number(glowStrength.value) || 0) * 0.06;
      const intensity = glowEnabled.value ? (baseIntensity + glowBonus) : (darkMode.value ? 0.06 : 0.05);

      node.mesh.material.emissiveIntensity = intensity;
      node.mesh.material.emissive.setHex(glowEnabled.value ? c : (darkMode.value ? 0x111111 : 0x000000));
      node.mesh.material.needsUpdate = true;
    }
    applyStarGlow(node);
  });

  edges.forEach((edge) => {
    if (edge.material) {
      edge.material.color.setHex(darkMode.value ? 0x445566 : 0xB0BEC5);
      edge.material.opacity = darkMode.value ? 0.25 : 0.3;
      edge.material.needsUpdate = true;
    }
  });

  if (scene) setupLighting();
};

const syncDarkModeFromAppTheme = () => {
  const next = (settingsStore.appearanceTheme === 'dark') || (document.documentElement?.dataset?.guaxiTheme === 'dark');
  if (darkMode.value !== next) {
    darkMode.value = next;
  }
  if (scene) applyDarkModeTheme();
};

watch(
  () => settingsStore.appearanceTheme,
  () => {
    syncDarkModeFromAppTheme();
  }
);

const toggleDarkMode = async () => {
  const next = darkMode.value ? 'light' : 'dark';
  try {
    await settingsStore.setAppearanceTheme(next);
  } catch (e) {
  }

  syncDarkModeFromAppTheme();
  if (scene) applyDarkModeTheme();
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

const getNodeKey = (nodeLike) => {
  const node = resolveGraphNode(nodeLike);
  if (!node) return '';
  const ud = node?.userData || {};
  if (ud.id != null) return `id:${String(ud.id)}`;
  if (ud.stable_id) return `stable:${String(ud.stable_id)}`;
  const p = ud.path || ud.file_path || '';
  return p ? `path:${normalizePath(p)}` : '';
};

const nodePassesTypeFilter = (n) => {
  const ud = n?.userData || {};
  const allowedTypes = new Set(Array.isArray(visibleTypes.value) ? visibleTypes.value : []);
  return allowedTypes.has(ud.type);
};

const nodePassesFilters = (n) => {
  const ud = n?.userData || {};

  if (!nodePassesTypeFilter(n)) return false;

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

const buildFocusContext = (nodeLike) => {
  const node = resolveGraphNode(nodeLike);
  if (!node) return null;

  const relatedNodes = new Set();
  const relatedEdges = new Set();
  const nodeKey = getNodeKey(node);

  relatedNodes.add(node);

  const byId = new Map();
  nodes.forEach((n) => {
    const nid = n?.userData?.id;
    if (nid != null) byId.set(String(nid), n);
  });

  const parentId = node.userData?.parentId;
  if (parentId != null) {
    const parent = byId.get(String(parentId));
    if (parent) relatedNodes.add(parent);
  }

  if (node.userData?.id != null) {
    const idStr = String(node.userData.id);
    nodes.forEach((n) => {
      if (String(n?.userData?.parentId ?? '') === idStr) relatedNodes.add(n);
    });
  }

  edges.forEach((edge) => {
    const from = edge?.userData?.from;
    const to = edge?.userData?.to;
    if (!from || !to) return;
    if (from === node || to === node) {
      relatedEdges.add(edge);
      relatedNodes.add(from);
      relatedNodes.add(to);
    }
  });

  return { node, nodeKey, nodes: relatedNodes, edges: relatedEdges };
};

const applyGraphVisibility = (context) => {
  const scope = context || focusContext.value;
  const hasScope = !!(scope && scope.nodes && scope.nodes.size);

  nodes.forEach((n) => {
    const ok = hasScope
      ? nodePassesTypeFilter(n) && scope.nodes.has(n)
      : nodePassesFilters(n);
    if (n?.mesh) n.mesh.visible = ok;
  });

  edges.forEach((e) => {
    const from = e?.userData?.from;
    const to = e?.userData?.to;
    let ok = !!from && !!to && !!from.mesh?.visible && !!to.mesh?.visible;
    if (hasScope) ok = ok && scope.edges.has(e);
    e.visible = ok;
  });
};

const applyFocusContext = (nodeLike) => {
  const context = buildFocusContext(nodeLike);
  if (!context) return;
  focusContext.value = context;
  applyGraphVisibility(context);
  highlightConnectedNodes(context.node);
  log('已进入关联聚焦模式', context.node?.userData?.name || '');
};

const clearFocusContext = () => {
  if (!focusContext.value) return;
  focusContext.value = null;
  applyGraphVisibility();
  resetNodeHighlights();
  log('已退出关联聚焦模式');
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

  killActiveCameraTween();
  isFocusMode.value = true;

  const p = node.mesh?.position?.clone?.() || new THREE.Vector3(0, 0, 0);

  let nodeRadius = 2;
  const geo = node.mesh?.geometry;
  if (geo?.computeBoundingSphere) {
    if (!geo.boundingSphere) geo.computeBoundingSphere();
    const r = geo.boundingSphere?.radius;
    if (Number.isFinite(r) && r > 0) {
      const s = node.mesh?.scale?.x;
      nodeRadius = r * (Number.isFinite(s) && s > 0 ? s : 1);
    }
  }

  ensureViewInBounds({ mode: 'node', recenter: false });

  const dir = camera.position.clone().sub(controls.target);
  if (dir.lengthSq() < 1e-6) dir.set(0, 0, 1);
  dir.normalize();

  const fov = THREE.MathUtils.degToRad(camera.fov || 60);
  const fit = (nodeRadius / Math.max(1e-6, Math.sin(fov / 2))) * 2.4;
  const desired = Math.max(controls.minDistance || 0, 30, fit);
  const distance = THREE.MathUtils.clamp(desired, 2, controls.maxDistance || 100000);

  const cameraPosition = p.clone().add(dir.multiplyScalar(distance));

  updateCameraClipping(p, Math.max(1, nodeRadius));

  isUserInteracting.value = true;

  activeCameraTween = gsap.timeline({
    defaults: { duration: 0.7, ease: 'power2.inOut' },
    onUpdate: () => {
      controls.update();
      updateCameraClipping(p, Math.max(1, nodeRadius));
    },
    onComplete: () => {
      activeCameraTween = null;
      isUserInteracting.value = false;
      ensureViewInBounds({ mode: 'node', recenter: false });
      clampViewToGraphBounds({ useTargetPositions: true, soft: false });
    }
  })
    .to(camera.position, { x: cameraPosition.x, y: cameraPosition.y, z: cameraPosition.z }, 0)
    .to(controls.target, { x: p.x, y: p.y, z: p.z }, 0);

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
      edge.material.color.setHex(0x2196F3);
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

    window.location.hash = '/';
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
  focusContext.value = null;

  if (labelRenderer?.domElement) {
    labelRenderer.domElement.innerHTML = '';
  }

  octree = null;

  const toRemove = scene.children.filter((obj) => !obj?.isLight);
  for (const obj of toRemove) {
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

  logger.info('开始创建节点和边...');

  // 1. 创建节点
  data.nodes.forEach(nodeData => {
    const geometry = new THREE.SphereGeometry(2, 32, 32);

    nodeData.originalColor = getNodeColor(nodeData.type);

    const material = new THREE.MeshStandardMaterial({
      color: nodeData.originalColor,
      metalness: 0.2,
      roughness: 0.5,
      emissive: glowEnabled.value ? nodeData.originalColor : (darkMode.value ? 0x111111 : 0x000000),
      emissiveIntensity: glowEnabled.value
        ? (darkMode.value ? (0.09 + (Number(glowStrength.value) || 0) * 0.09) : (0.07 + (Number(glowStrength.value) || 0) * 0.07))
        : (darkMode.value ? 0.06 : 0.05)
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(0, 0, 0);
    mesh.userData = nodeData;
    scene.add(mesh);

    const graphNode = {
      mesh,
      userData: nodeData,
      targetPosition: new THREE.Vector3(0, 0, 0)
    };

    nodes.push(graphNode);
    applyStarGlow(graphNode);

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
        color: darkMode.value ? 0x888888 : 0x1565C0, // 更暗淡的蓝灰色
        linewidth: darkMode.value ? 1 : 2,
        transparent: true,
        opacity: darkMode.value ? 0.25 : 0.3,
        depthTest: true,
        depthWrite: false, // 避免遮挡节点
        blending: THREE.NormalBlending
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
  stopActiveSimulation();
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

  activeSimulation = simulation;

  simulation.on('tick', () => {
    for (const d of nodeDatas) {
      const n = byId.get(String(d.id));
      if (!n) continue;
      
      const x = Number.isFinite(d.x) ? d.x : 0;
      const y = Number.isFinite(d.y) ? d.y : 0;
      const z = Number.isFinite(d.z) ? d.z : 0;
      
      n.targetPosition.set(x, y, z);
    }
  });

  setTimeout(() => simulation.stop(), 1200);
};

/**
 * Apply hierarchical layout to nodes
 */
const hierarchicalLayout = () => {
  stopActiveSimulation();
  log('应用层次布局(千层饼：同级同平面 + 均匀铺开)');

  if (!nodes.length) return;

  // 规则：
  // - 同一 depth 的节点放在同一平面(y 固定)，像“千层饼”
  // - 在 xz 平面用黄金角(向日葵)均匀铺开，并随该层节点数自适应半径，减少重叠
  const golden = 2.399963229728653;
  const layerY = performanceLevel.value === 0 ? 34 : performanceLevel.value === 2 ? 64 : 46;
  const minDisc = performanceLevel.value === 0 ? 30 : performanceLevel.value === 2 ? 85 : 55;
  const spacing = performanceLevel.value === 0 ? 10 : performanceLevel.value === 2 ? 18 : 13;

  const byDepth = new Map();
  nodes.forEach((node, idx) => {
    const ud = node?.userData || {};
    const depth = Number.isFinite(ud.depth) ? Number(ud.depth) : (ud.parentId ? 1 : 0);
    if (!byDepth.has(depth)) byDepth.set(depth, []);
    byDepth.get(depth).push({ node, idx });
  });

  for (const [depth, arr] of byDepth) {
    const k = arr.length;
    const y = -depth * layerY;

    // 同层节点越多，圆盘半径越大
    const discR = Math.max(minDisc + depth * spacing * 1.1, Math.sqrt(Math.max(1, k)) * spacing * 2.2);

    for (let i = 0; i < k; i++) {
      const { node, idx } = arr[i];
      if (!node?.targetPosition) continue;

      if (depth === 0) {
        node.targetPosition.set(0, 0, 0);
        continue;
      }

      const t = (i + 0.5) / k;
      const r = discR * Math.sqrt(t);
      const a = (idx + 1) * golden + depth * 0.37;

      const x = Math.cos(a) * r;
      const z = Math.sin(a) * r;
      node.targetPosition.set(x, y, z);
    }
  }

  log('层次布局完成', { layers: byDepth.size, nodes: nodes.length });
};

/**
 * Apply spherical layout to nodes
 */
const sphericalLayout = () => {
  stopActiveSimulation();
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

  const forceSphere = (radiusFn, strength = 0.9) => {
    let ns = [];
    function f(alpha) {
      for (const d of ns) {
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

  const thetaInc = 2.399963229728653;
  const nodeDatas = others.map((n, i) => {
    const u = (i + 0.5) / m;
    const phi = Math.acos(1 - 2 * u);
    const theta = (i + 1) * thetaInc;
    return {
      id: String(n?.userData?.id ?? i),
      x: radius * Math.cos(theta) * Math.sin(phi),
      y: radius * Math.sin(theta) * Math.sin(phi),
      z: radius * Math.cos(phi)
    };
  });

  const collisionRadius = performanceLevel.value === 0 ? 5 : performanceLevel.value === 2 ? 8 : 6;
  const chargeStrength = performanceLevel.value === 0 ? -20 : performanceLevel.value === 2 ? -40 : -28;
  const ticks = performanceLevel.value === 0 ? 60 : performanceLevel.value === 2 ? 140 : 100;

  const simulation = d3.forceSimulation(nodeDatas)
    .numDimensions(3)
    .alpha(1)
    .alphaDecay(0.08)
    .velocityDecay(0.45)
    .force('charge', d3.forceManyBody().strength(chargeStrength))
    .force('collision', d3.forceCollide().radius(collisionRadius))
    .force('sphere', forceSphere(() => radius, 0.9));

  for (let k = 0; k < ticks; k++) simulation.tick();
  simulation.stop();

  for (let i = 0; i < m; i++) {
    const n = others[i];
    const d = nodeDatas[i];
    n?.targetPosition?.set(d.x || 0, d.y || 0, d.z || 0);
  }
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

  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('keyup', handleKeyUp);
  log('已绑定键盘事件');

  if (typeof ResizeObserver !== 'undefined' && container.value) {
    containerResizeObserver?.disconnect?.();
    containerResizeObserver = new ResizeObserver(() => {
      handleWindowResize();
    });
    containerResizeObserver.observe(container.value);
  }

  const setMouseFromEvent = (event) => {
    const rect = renderer?.domElement?.getBoundingClientRect?.();
    if (!rect || rect.width <= 0 || rect.height <= 0) return false;
    mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    followPointer.x = mouse.x;
    followPointer.y = mouse.y;
    return true;
  };

  renderer.domElement.addEventListener('mousemove', (event) => {
    if (!setMouseFromEvent(event)) return;
    if (selectedNode.value) return;

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

  renderer.domElement.addEventListener('wheel', (event) => {
    if (!setMouseFromEvent(event) || !camera || !controls) return;

    setTimeout(() => {
      ensureViewInBounds({ mode: 'graph', recenter: false, useTargetPositions: true });
      clampViewToGraphBounds({ useTargetPositions: true, soft: true });
    }, 0);
  }, { passive: true });

  renderer.domElement.addEventListener('click', (event) => {
    if (!setMouseFromEvent(event)) return;

    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(scene.children, true);

    if (intersects.length > 0) {
      const clickedObject = intersects[0].object;
      selectNode(clickedObject);
    } else {
      selectedNode.value = null;
      isFocusMode.value = false;
      store.setSelectedGraphNode(null);
      resetNodeHighlights();
    }
  });

  renderer.domElement.addEventListener('dblclick', (event) => {
    if (!setMouseFromEvent(event)) return;

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
      node.mesh.material.needsUpdate = true;
    }
    applyStarGlow(node);
  });

  edges.forEach(edge => {
    if (edge.material) {
      edge.material.color.setHex(darkMode.value ? 0x445566 : 0xB0BEC5);
      edge.material.opacity = darkMode.value ? 0.25 : 0.3;
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

    if (object.material.emissive) {
      object.material.emissive.setHex(0xff0000);
    }
    if (typeof object.material.emissiveIntensity === 'number') {
      object.material.emissiveIntensity = Math.max(object.material.emissiveIntensity || 0, 1.6);
    }

    const sp = object.userData?.__starGlowSprite;
    if (sp?.material) {
      sp.visible = true;
      sp.material.color.setHex(0xff0000);
      sp.material.opacity = Math.min(1, 0.9);
      sp.material.needsUpdate = true;
    }

    // 高亮连接的边
    edges.forEach(edge => {
      if (edge.userData.from === object || edge.userData.to === object) {
        edge.material.color.setHex(0x2196F3);
        edge.material.opacity = 1;
      }
    });
  } catch (e) {
    logger.warn('高亮节点时出错:', e);
  }
};

/**
 * Select a node and update UI
 * @param {THREE.Object3D} object - Node to select
 */
const selectNode = (object) => {
  if (object.userData?.type === 'edge') {
    const fromNode = object.userData?.from?.userData || findNodeByPosition(object.geometry.attributes.position.array.slice(0, 3));
    const toNode = object.userData?.to?.userData || findNodeByPosition(object.geometry.attributes.position.array.slice(3, 6));

    selectedNode.value = {
      type: 'edge',
      name: '连接线',
      from: fromNode,
      to: toNode,
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

  logger.info('开始构建知识图谱...');
  isLoading.value = true;
  loadingProgress.value = 0;

  try {
    clearScene();
    setupLighting();

    const treeData = await fetchGraphData();
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 30;

    logger.info('获取知识图谱数据成功！');
    loadingProgress.value = 60;

    await createNodesAndEdges(treeData);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 70;
    logger.info('创建节点和边成功！');

    await applyLayout(currentLayout.value);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;
    loadingProgress.value = 90;
    logger.info('应用布局成功！');

    const meshes = nodes.map(n => n.mesh).filter(Boolean);
    logger.info('准备初始化八叉树，网格数量:', meshes.length);
    await initOctree(meshes);
    if (runId !== buildGraphRunId || kbId !== selectedKB.value?.id) return;

    startAnimate();
    logger.info('开始动画循环！');
    loadingProgress.value = 100;
  } catch (err) {
    showError.value = true;
    errorMessage.value = `构建失败: ${err.message}`;
  } finally {
    if (runId === buildGraphRunId) {
      isLoading.value = false;
      frameAllNodes();
    }
  }
};

/**
 * 优化的八叉树初始化方法
 * @param {Array} meshes - 需要加入八叉树的网格数组
 */
const initOctree = async (meshes) => {
  if (!meshes || meshes.length === 0) {
    octree = new GraphOctree();
    return;
  }

  try {
    console.time('八叉树初始化耗时');

    octree = new GraphOctree();
    for (const mesh of meshes) {
      octree.addGraphNode(mesh);
    }

    console.timeEnd('八叉树初始化耗时');
    logger.info(`八叉树初始化完成，共处理${meshes.length}个网格`);
  } catch (e) {
    logger.error('八叉树初始化失败:', e);
    octree = new GraphOctree();
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

let starGlowTexture = null;

const getStarGlowTexture = () => {
  if (starGlowTexture) return starGlowTexture;

  const size = 128;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;

  const ctx = canvas.getContext('2d');
  if (!ctx) {
    starGlowTexture = new THREE.Texture();
    starGlowTexture.needsUpdate = true;
    return starGlowTexture;
  }

  const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  g.addColorStop(0, 'rgba(255,255,255,1)');
  g.addColorStop(0.2, 'rgba(255,255,255,0.9)');
  g.addColorStop(0.5, 'rgba(255,255,255,0.25)');
  g.addColorStop(1, 'rgba(255,255,255,0)');

  ctx.clearRect(0, 0, size, size);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, size, size);

  starGlowTexture = new THREE.CanvasTexture(canvas);
  starGlowTexture.needsUpdate = true;
  return starGlowTexture;
};

const getStarGlowScale = (type) => {
  const base = Math.max(1, Number(glowSize.value) || 16);
  switch (type) {
    case 'knowledge-base': return base * 1.4;
    case 'directory': return base * 1.2;
    case 'file': return base * 1.1;
    case 'section': return base * 1.05;
    case 'paragraph': return base * 1.0;
    default: return base * 1.05;
  }
};

const applyStarGlow = (node) => {
  const mesh = node?.mesh;
  const mat = mesh?.material;
  if (!mesh || !mat) return;

  const ud = node?.userData || mesh.userData || {};
  const type = ud.type;
  const c = getNodeColor(type);

  if (mat.emissive) {
    mat.emissive.setHex(glowEnabled.value ? c : (darkMode.value ? 0x111111 : 0x000000));
  }
  if (typeof mat.emissiveIntensity === 'number') {
    const s = Math.max(0, Number(glowStrength.value) || 0);
    mat.emissiveIntensity = glowEnabled.value
      ? (darkMode.value ? (0.11 + s * 0.11) : (0.08 + s * 0.08))
      : (darkMode.value ? 0.06 : 0.05);
  }
  mat.needsUpdate = true;

  const existing = mesh.userData?.__starGlowSprite || null;
  if (!glowEnabled.value) {
    if (existing) existing.visible = false;
    return;
  }

  let sprite = existing;
  if (!sprite) {
    const sm = new THREE.SpriteMaterial({
      map: getStarGlowTexture(),
      color: c,
      transparent: true,
      opacity: 0.85,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      depthTest: false
    });
    sprite = new THREE.Sprite(sm);
    sprite.renderOrder = 999;
    mesh.add(sprite);
    mesh.userData.__starGlowSprite = sprite;
  }

  sprite.visible = true;
  const strength = Math.max(0, Number(glowStrength.value) || 0);
  if (sprite.material) {
    sprite.material.color.setHex(c);
    sprite.material.opacity = Math.min(1, 0.12 + strength * 0.14);
    sprite.material.needsUpdate = true;
  }

  const s = getStarGlowScale(type);
  sprite.scale.set(s, s, 1);
};

/**
 * Apply selected layout algorithm
 * @param {string} layoutType - Layout type to apply
 */
const applyLayout = async (layoutType) => {
  if (!nodes.length) return;

  logger.info(`应用布局: ${layoutType}`);

  nodes.forEach(node => {
    node.targetPosition = node.mesh.position.clone();
  });

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
        default:
          forceDirectedLayout();
      }
      resolve();
    }, 0);
  });

  edges.forEach(edge => {
    if (edge.userData?.update) {
      edge.userData.update();
    }
  });

  await nextTick();
  frameAllNodes({ useTargetPositions: true });

  setTimeout(() => {
    frameAllNodes({ useTargetPositions: true, captureBaseline: true });
    ensureViewInBounds({ mode: 'graph', recenter: true, useTargetPositions: true });
    clampViewToGraphBounds({ useTargetPositions: true, soft: false });
  }, 120);
};

/**
 * Animation loop for continuous rendering
 */
let animationFrameId = null;
let autoSpinLastTime = performance.now();

const _autoSpinAxis = new THREE.Vector3(0, 1, 0);
const _autoSpinCenter = new THREE.Vector3();
const _autoSpinTmp = new THREE.Vector3();
const _autoSpinQuat = new THREE.Quaternion();

const applyAutoSpin = (dt) => {
  if (!autoRotate.value) return false;
  if (!nodes.length) return false;
  if (isUserInteracting.value) return false;
  if (isFocusMode.value) return false;

  const angle = dt * 0.05;
  if (!Number.isFinite(angle) || Math.abs(angle) < 1e-8) return false;

  // 计算所有节点的几何中心（质心）
  // 使用 targetPosition 计算更稳定，避免动画过程中的抖动
  let cx = 0, cy = 0, cz = 0;
  let count = 0;
  for (const n of nodes) {
    const p = n.targetPosition;
    if (p && isFiniteVec3(p)) {
      cx += p.x;
      cy += p.y;
      cz += p.z;
      count++;
    }
  }

  if (count > 0) {
    _autoSpinCenter.set(cx / count, cy / count, cz / count);
  } else {
    _autoSpinCenter.set(0, 0, 0);
  }

  _autoSpinQuat.setFromAxisAngle(_autoSpinAxis, angle);

  for (const n of nodes) {
    const mesh = n?.mesh;
    const tp = n?.targetPosition;

    if (mesh && isFiniteVec3(mesh.position)) {
      _autoSpinTmp.copy(mesh.position).sub(_autoSpinCenter).applyQuaternion(_autoSpinQuat).add(_autoSpinCenter);
      mesh.position.copy(_autoSpinTmp);
    }

    if (tp && isFiniteVec3(tp)) {
      _autoSpinTmp.copy(tp).sub(_autoSpinCenter).applyQuaternion(_autoSpinQuat).add(_autoSpinCenter);
      tp.copy(_autoSpinTmp);
    }
  }

  return true;
};

const updateNodeLabels = () => {
  if (!camera || !nodes.length) return;
  const isCrowded = nodes.length > 80;
  const camPos = camera.position;
  
  for (const node of nodes) {
    const mesh = node?.mesh;
    if (!mesh || !mesh.visible) continue;
    
    const label = mesh.children.find(c => c.isCSS2DObject);
    if (!label) continue;
    
    // 如果节点很少，总是显示
    if (!isCrowded) {
        if (!label.visible) label.visible = true;
        label.element.style.opacity = '1';
        continue;
    }

    const type = node.userData?.type;
    // 重要节点始终显示 (知识库、目录)
    const isImportant = type === 'knowledge-base' || type === 'directory' || type === 'file';
    // 选中节点始终显示
    const isSelected = selectedNode.value && String(selectedNode.value.id) === String(node.userData.id);
    // 高亮节点(红色)始终显示
    const isHighlighted = mesh.material.color.getHex() === 0xff0000;
    
    if (isImportant || isSelected || isHighlighted) {
      label.visible = true;
      label.element.style.opacity = '1';
      label.element.style.zIndex = isSelected || isHighlighted ? '100' : '10';
    } else {
      const d = mesh.position.distanceTo(camPos);
      const maxDist = 100; // 可见距离阈值
      
      if (d < maxDist) {
        label.visible = true;
        // 随距离渐变
        const op = Math.max(0.1, 1 - (d / maxDist) * 0.9);
        label.element.style.opacity = op.toFixed(2);
        label.element.style.zIndex = '1';
      } else {
        label.visible = false;
      }
    }
  }
};

const animate = () => {
  animationFrameId = requestAnimationFrame(animate);

  if (controls && camera) {
    if (!isFiniteVec3(camera.position) || !isFiniteVec3(controls.target)) {
      warn('相机向量异常，已触发重置', { camera: camera.position, target: controls.target });
      resetCamera();
      return;
    }
  }

  const now = performance.now();
  const dt = Math.min(0.05, Math.max(0, (now - autoSpinLastTime) / 1000));
  autoSpinLastTime = now;

  applyRoamMovement();

  let moved = applyAutoSpin(dt);
  for (const node of nodes) {
    const mesh = node?.mesh;
    const target = node?.targetPosition;
    if (!mesh || !target) continue;

    const d2 = mesh.position.distanceToSquared(target);
    if (d2 > 1e-4) {
      if (isFiniteVec3(target)) {
        mesh.position.lerp(target, 0.12);
        moved = true;
      }
    } else if (d2 > 0) {
      if (isFiniteVec3(target)) {
        mesh.position.copy(target);
      }
    }
  }

  if (moved) {
    for (const edge of edges) {
      edge.userData?.update?.();
    }
  }

  controls?.update?.();

  if (composer) composer.render();
  else renderer.render(scene, camera);

  updateNodeLabels();
  labelRenderer?.render(scene, camera);
};

const startAnimate = () => {
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  autoSpinLastTime = performance.now();
  animate();
};

watch(currentLayout, async (newLayout) => {
  if (scene && nodes.length > 0) {
    await applyLayout(newLayout);

    if (!isUserInteracting.value) {
      ensureViewInBounds({ mode: 'graph', recenter: false, useTargetPositions: true });
      setTimeout(() => ensureViewInBounds({ mode: 'graph', recenter: false, useTargetPositions: true }), 450);
    }
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
    logger.info('测试立方体已渲染，应该看到红色立方体');
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
      logger.warn('场景未初始化，将延迟处理知识库变化');
      pendingKnowledgeBaseChange.value = newVal;
      return;
    }

    if (isValidKnowledgeBase(newVal)) {
      logger.info('检测到有效知识库:', newVal.knowledge_base_name);
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
  selectedGraphNode,
  (n) => {
    if (!n) {
      selectedNode.value = null;
      isFocusMode.value = false;
      clearFocusContext();
      return;
    }

    const hit = resolveGraphNode(n);
    if (!hit) return;

    selectedNode.value = { ...hit.userData, object: hit.mesh };

    if (focusContext.value) {
      const hitKey = getNodeKey(hit);
      if (!focusContext.value.nodeKey || focusContext.value.nodeKey !== hitKey) {
        const nextContext = buildFocusContext(hit);
        if (nextContext) {
          focusContext.value = nextContext;
          applyGraphVisibility(nextContext);
          log('已更新关联聚焦上下文', hit.userData?.name || '');
        }
      }
    }

    highlightConnectedNodes(hit);
  },
  { deep: true }
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
      applyFocusContext(hit);
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
    store.setSelectedGraphNode(null);
  }
};

// Component lifecycle hooks
onMounted(async () => {
  log('组件挂载开始');

  syncDarkModeFromAppTheme();

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
  stopActiveSimulation();
  log('开始组件卸载清理');

  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }

  // 1. Remove event listeners
  window.removeEventListener('resize', handleWindowResize);
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('keyup', handleKeyUp);
  roamKeysDown.clear();
  containerResizeObserver?.disconnect?.();
  containerResizeObserver = null;
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
    logger.info('已释放八叉树');
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
  padding: 8px 12px;
  z-index: 10;
  background-color: var(--guaxi-panel-bg);
  backdrop-filter: blur(8px);
  border-bottom: 1px solid var(--guaxi-border);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
}

.dark-mode .toolbar {
  background-color: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

/* Graph container styling */
.graph-container {
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
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


/* 修改 legend-panel 的样式 */
.legend-panel {
  position: absolute;
  top: 120px;
  left: 10px;
  width: 140px;
  z-index: 5;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transform: scale(0.95);
  transform-origin: top left;
}

.navigation-tools {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px;
}

.navigation-tools :deep(.n-button) {
  width: 100%;
  justify-content: flex-start;
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
  background-color: var(--guaxi-panel-bg);
  border-color: var(--guaxi-border);
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