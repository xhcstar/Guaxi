<template>
  <div class="knowledge-graph-container" :class="{ 'dark-mode': darkMode }">
    <!-- 工具栏 -->
    <div class="toolbar">
      <n-space>
        <n-button size="small" @click="resetCamera">
          <template #icon>
            <n-icon>
              <CompassOutline />
            </n-icon>
          </template>
          重置视角
        </n-button>
        <n-button size="small" @click="toggleAutoRotate">
          <template #icon>
            <n-icon>
              <SyncOutline />
            </n-icon>
          </template>
          {{ autoRotate ? '停止旋转' : '自动旋转' }}
        </n-button>
        <n-button size="small" @click="toggleDarkMode">
          <template #icon>
            <n-icon>
              <MoonOutline />
            </n-icon>
          </template>
          {{ darkMode ? '亮色模式' : '暗色模式' }}
        </n-button>
      </n-space>
    </div>

    <!-- Three.js 渲染容器 -->
    <div ref="container" class="graph-container"></div>

    <!-- 加载状态 -->
    <div v-if="isLoading" class="loading-overlay">
      <n-spin size="large" />
    </div>

    <!-- 节点信息面板 -->
    <div v-if="selectedNode" class="node-info-panel">
      <n-card :title="selectedNode.name || '连接线'" size="small">
        <n-space vertical>
          <n-text v-if="selectedNode.type === 'knowledgeBase'">知识库</n-text>
          <n-text v-if="selectedNode.type === 'file'">文件</n-text>
          <n-text v-if="selectedNode.type === 'directory'">目录</n-text>
          <n-text v-if="selectedNode.type === 'edge'">连接线</n-text>
          <n-text v-if="selectedNode.path">路径: {{ selectedNode.path }}</n-text>
          <n-text v-if="selectedNode.from">从: {{ selectedNode.from.name }}</n-text>
          <n-text v-if="selectedNode.to">到: {{ selectedNode.to.name }}</n-text>
          <n-button size="small" @click="focusOnNode(selectedNode.type === 'edge' ? selectedNode.from : selectedNode)">
            聚焦此节点
          </n-button>
        </n-space>
      </n-card>
    </div>

    <!-- 错误提示面板 -->
    <div v-if="showError" class="error-panel">
      <n-alert type="error" :title="errorMessage" closable @close="showError = false" />
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount, watch, nextTick, computed } from 'vue'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls'
import { ArrowHelper } from 'three'
import { LineDashedMaterial } from 'three'
import { invoke } from '@tauri-apps/api/core'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { storeToRefs } from 'pinia'
import { NButton, NSpace, NIcon, NCard, NText, NAlert, NSpin } from 'naive-ui'
import {
  CompassOutline,
  SyncOutline,
  MoonOutline,
} from '@vicons/ionicons5'
import { CSS2DRenderer, CSS2DObject } from 'three/examples/jsm/renderers/CSS2DRenderer'

// ============== 调试配置 ==============
const DEBUG = true
const log = (...args) => DEBUG && console.log('[KnowledgeGraph]', ...args)
const warn = (...args) => DEBUG && console.warn('[KnowledgeGraph]', ...args)
const error = (...args) => DEBUG && console.error('[KnowledgeGraph]', ...args)

// ============== 响应式状态 ==============
const container = ref(null)
const autoRotate = ref(true)
const darkMode = ref(false)
const selectedNode = ref(null)
const graphData = ref([])
const isLoading = ref(false)
const showError = ref(false)
const errorMessage = ref('')
const pendingKnowledgeBaseChange = ref(null)

// ============== Three.js 变量 ==============
let scene = null
let camera = null
let renderer = null
let controls = null
let raycaster = null
const mouse = new THREE.Vector2()
let nodes = []
let edges = []

// ============== Store 相关 ==============
const store = useKnowledgeBaseStore()
log('Store 初始化完成:', store.$state)

const { knowledgeBases, selectedKnowledgeBase } = storeToRefs(store)
const selectedKB = computed(() => {
  return selectedKnowledgeBase.value ||
    (knowledgeBases.value.length > 0 ? knowledgeBases.value[0] : {})
})


const handleWindowResize = () => {
  if (!camera || !container.value || !renderer) return;

  camera.aspect = container.value.clientWidth / container.value.clientHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(container.value.clientWidth, container.value.clientHeight);

  if (DEBUG) console.log('窗口大小调整:', {
    width: container.value.clientWidth,
    height: container.value.clientHeight
  });
};

const handleCanvasClick = (event) => {
  if (!raycaster || !camera || !scene) return;

  // 计算鼠标位置归一化坐标
  mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
  mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

  // 设置射线
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

    if (DEBUG) console.log('点击对象:', selectedNode.value);
  } else {
    selectedNode.value = null;
  }
};

// ============== 知识库验证 ==============
/**
 * 验证知识库是否有效
 * @param {Object} kb - 知识库对象
 * @returns {boolean} 是否有效
 */
const isValidKnowledgeBase = (kb) => {
  if (!kb || typeof kb !== 'object') {
    warn('知识库对象无效');
    return false;
  }

  // 检查必要字段
  const requiredFields = ['id', 'knowledge_base_name', 'location'];
  if (requiredFields.some(field => !(field in kb))) {
    warn('知识库缺少必要字段');
    return false;
  }

  // 检查路径格式
  if (typeof kb.location !== 'string') {
    warn('location必须为字符串');
    return false;
  }

  const normalizedPath = kb.location.replace(/\\/g, '/');
  const isWindows = navigator.platform.toLowerCase().includes('win');

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

// 计算属性：当前知识库是否有效
const hasValidKnowledgeBase = computed(() => {
  return isValidKnowledgeBase(selectedKB.value);
});

// ============== 核心函数 ==============
const initThreeJS = () => {
  log('开始初始化 Three.js 场景');

  if (!container.value) {
    error('容器元素未找到');
    showError.value = true;
    errorMessage.value = '渲染容器未找到';
    return;
  }

  try {
    // 1. 创建场景
    scene = new THREE.Scene();
    scene.background = darkMode.value ? new THREE.Color(0x121212) : new THREE.Color(0xf0f0f0);
    scene.fog = new THREE.FogExp2(darkMode.value ? 0x050505 : 0xf0f0f0, 0.002);

    // 2. 创建相机
    camera = new THREE.PerspectiveCamera(
      60, // 更宽的视野
      container.value.clientWidth / container.value.clientHeight,
      0.1,
      10000
    );
    camera.position.set(0, 0, 100);

    // 3. 创建渲染器
    renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true
    });
    renderer.setSize(container.value.clientWidth, container.value.clientHeight);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    container.value.appendChild(renderer.domElement);

    // 4. 添加控制器
    controls = new OrbitControls(camera, renderer.domElement);
    controls.autoRotate = autoRotate.value;
    controls.autoRotateSpeed = 0.5; // 更慢的旋转速度
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 20;
    controls.maxDistance = 500;

    // 5. 添加光照
    const ambientLight = new THREE.AmbientLight(0x404040, darkMode.value ? 0.5 : 1.0);
    scene.add(ambientLight);

    // 主光源
    const directionalLight = new THREE.DirectionalLight(0xffffff, 1);
    directionalLight.position.set(1, 1, 1);
    directionalLight.castShadow = true;
    directionalLight.shadow.mapSize.width = 2048;
    directionalLight.shadow.mapSize.height = 2048;
    scene.add(directionalLight);

    // 辅助光源
    const hemisphereLight = new THREE.HemisphereLight(
      0xffffbb, 0x080820, 0.5
    );
    scene.add(hemisphereLight);

    // 6. 初始化射线检测
    raycaster = new THREE.Raycaster();

    // 7. 设置事件监听
    window.addEventListener('resize', () => {
      camera.aspect = container.value.clientWidth / container.value.clientHeight
      camera.updateProjectionMatrix()
      renderer.setSize(container.value.clientWidth, container.value.clientHeight)
    })

    renderer.domElement.addEventListener('click', (event) => {
      mouse.x = (event.clientX / window.innerWidth) * 2 - 1
      mouse.y = -(event.clientY / window.innerHeight) * 2 + 1
      raycaster.setFromCamera(mouse, camera)
      const intersects = raycaster.intersectObjects(scene.children, true)

      if (intersects.length > 0) {
        const clickedObject = intersects[0].object
        if (clickedObject.userData?.type === 'edge') {
          selectedNode.value = {
            type: 'edge',
            name: '连接线',
            from: findNodeByPosition(clickedObject.geometry.attributes.position.array.slice(0, 3)),
            to: findNodeByPosition(clickedObject.geometry.attributes.position.array.slice(3, 6))
          }
        } else {
          selectedNode.value = clickedObject.userData
        }
      }
    })

    log('Three.js 初始化完成')

    // 处理延迟的知识库变化
    if (pendingKnowledgeBaseChange.value) {
      log('处理延迟的知识库变化:', pendingKnowledgeBaseChange.value)
      const kb = pendingKnowledgeBaseChange.value
      pendingKnowledgeBaseChange.value = null
      nextTick(() => {
        if (isValidKnowledgeBase(kb)) {
          buildGraph()
        }
      })
    }
  } catch (err) {
    error('Three.js 初始化失败:', err)
    showError.value = true
    errorMessage.value = `Three.js 初始化失败: ${err.message}`
  }
}

const resetCamera = () => {
  if (camera && controls) {
    camera.position.set(0, 0, 50)
    controls.reset()
    log('相机视角已重置')
  }
}

const toggleAutoRotate = () => {
  autoRotate.value = !autoRotate.value
  if (controls) {
    controls.autoRotate = autoRotate.value
    log(`自动旋转已${autoRotate.value ? '开启' : '关闭'}`)
  }
}

const toggleDarkMode = () => {
  darkMode.value = !darkMode.value
  if (scene) {
    scene.background = darkMode.value
      ? new THREE.Color(0x121212)
      : new THREE.Color(0xf0f0f0)
  }
  log(`已切换至${darkMode.value ? '暗色' : '亮色'}模式`)
}

/**
 * 清理场景资源
 */
const clearScene = () => {
  log(`清理场景，当前节点数: ${nodes.length}, 边数: ${edges.length}`)

  // 移除所有节点
  nodes.forEach(node => {
    if (node.mesh && scene) {
      scene.remove(node.mesh)
      if (node.mesh.geometry) node.mesh.geometry.dispose()
      if (node.mesh.material) node.mesh.material.dispose()
    }
  })

  // 移除所有边和箭头
  edges.forEach(edge => {
    if (edge && scene) {
      scene.remove(edge)
      if (edge.userData?.arrow) {
        scene.remove(edge.userData.arrow)
      }
      if (edge.geometry) edge.geometry.dispose()
      if (edge.material) edge.material.dispose()
    }
  })

  // 重置数组
  nodes = []
  edges = []
}

/**
 * 递归处理树形数据
 * @param {Object} treeData - 原始树形数据
 * 1. 创建根节点
 * 2. 递归创建子节点和连接边
 * 3. 确保所有节点和边正确添加到场景
 */
const processTree = (node, parentMesh = null, depth = 0) => {
  log(`处理节点[深度${depth}]: ${node.name} (${node.is_dir ? '目录' : '文件'})`);

  // 创建节点对象（传递depth参数）
  const newNode = {
    id: node.path || `node_${depth}_${Date.now()}`,
    name: node.name,
    type: node.is_dir ? 'directory' : 'file',
    path: node.path,
    depth: depth,
    mesh: createNodeMesh(
      node.name,
      node.is_dir ? 0x3498db : 0x2ecc71, // 目录蓝色，文件绿色
      Math.max(1.5, 5 - depth * 0.8), // 基础大小调整
      depth // 传递深度信息
    )
  };

  // 添加到节点列表和场景
  nodes.push(newNode);
  scene.add(newNode.mesh);
  log(`节点创建完成: ${newNode.name} (ID: ${newNode.id})`);

  // 如果不是根节点，创建连接线
  if (parentMesh) {
    log(`创建连接线: ${parentMesh.userData.name} -> ${newNode.name}`);
    const edge = createEdge(parentMesh, newNode.mesh);
    edges.push(edge);
    scene.add(edge);
  }

  // 递归处理子节点
  if (node.children && node.is_dir) {
    log(`发现${node.children.length}个子节点，开始递归处理...`);
    node.children.forEach((child, index) => {
      log(`处理子节点[${index + 1}/${node.children.length}]: ${child.name}`);
      processTree(child, newNode.mesh, depth + 1);
    });
  }

  return newNode;
};

/**
 * 构建知识图谱主函数
 * 1. 验证知识库有效性
 * 2. 清理现有场景
 * 3. 获取知识库树形数据
 * 4. 处理并可视化树形结构
 * 5. 布局节点和边
 * 6. 启动动画循环
 */
const buildGraph = async () => {
  // ============== 初始化阶段 ==============
  if (isLoading.value) {
    log('正在加载中，跳过重复请求');
    return;
  }

  log('开始构建知识图谱');
  isLoading.value = true;
  showError.value = false;
  const startTime = Date.now();

  try {
    // ============== 1. 验证知识库有效性 ==============
    log('验证知识库有效性...');
    if (!isValidKnowledgeBase(selectedKB.value)) {
      throw new Error('当前知识库无效，缺少必要字段或路径格式错误');
    }
    log('知识库验证通过:', {
      id: selectedKB.value.id,
      name: selectedKB.value.knowledge_base_name,
      location: selectedKB.value.location
    });

    // ============== 2. 清理现有场景 ==============
    log('清理现有场景...');
    clearScene();
    log('场景清理完成');

    // ============== 3. 获取知识库树形数据 ==============
    log('请求知识库树数据...');
    const normalizedPath = selectedKB.value.location.replace(/\\/g, '/');
    const treeData = await invoke("get_knowledge_base_tree", {
      rootPath: normalizedPath
    }).catch(err => {
      throw new Error(`获取树数据失败: ${err.message}`);
    });

    if (!treeData || !treeData.name) {
      throw new Error('获取到的树数据格式无效');
    }
    log('获取树数据成功:', {
      name: treeData.name,
      childrenCount: treeData.children?.length || 0,
      isDirectory: treeData.is_dir
    });

    // ============== 4. 处理树形数据 ==============
    log('开始处理树形数据...');
    /**
     * 递归处理树形数据
     * @param {Object} node - 当前节点数据
     * @param {THREE.Mesh|null} parentMesh - 父节点网格对象
     * @param {number} depth - 当前递归深度
     * @returns {Object} 处理后的节点对象
     */
    const processTree = (node, parentMesh = null, depth = 0) => {
      log(`处理节点[深度${depth}]: ${node.name} (${node.is_dir ? '目录' : '文件'})`);

      // 创建节点对象
      const newNode = {
        id: node.path || `node_${depth}_${Date.now()}`, // 确保唯一ID
        name: node.name,
        type: node.is_dir ? 'directory' : 'file',
        path: node.path,
        depth: depth,
        mesh: createNodeMesh(
          node.name,
          node.is_dir ? 0x3498db : 0x2ecc71, // 目录蓝色，文件绿色
          Math.max(1, 3 - depth * 0.5) // 随深度减小尺寸，最小为1
        )
      };

      // 添加到节点列表和场景
      nodes.push(newNode);
      scene.add(newNode.mesh);
      log(`节点创建完成: ${newNode.name} (ID: ${newNode.id})`);

      // 如果不是根节点，创建连接线
      if (parentMesh) {
        log(`创建连接线: ${parentMesh.userData.name} -> ${newNode.name}`);
        const edge = createEdge(parentMesh, newNode.mesh);
        edges.push(edge);
        scene.add(edge);
      }

      // 递归处理子节点
      if (node.children && node.is_dir) {
        log(`发现${node.children.length}个子节点，开始递归处理...`);
        node.children.forEach((child, index) => {
          log(`处理子节点[${index + 1}/${node.children.length}]: ${child.name}`);
          processTree(child, newNode.mesh, depth + 1);
        });
      }

      return newNode;
    };

    // 从根节点开始处理
    const rootNode = processTree(treeData);
    log(`树形数据处理完成，共创建 ${nodes.length} 个节点和 ${edges.length} 条边`);

    // ============== 5. 布局节点 ==============
    log('开始布局节点...');
    layoutNodes();
    log('节点布局完成');

    // ============== 6. 布局后处理 ==============
    log('开始布局后处理...');

    // 添加自动旋转初始角度
    if (controls) {
      controls.autoRotate = true;
      controls.autoRotateSpeed = 0.5;
      controls.reset();
      log('初始化自动旋转控制器');
    }

    // 添加场景动画
    scene.traverse(object => {
      if (object.isMesh) {
        object.userData.originalPosition = object.position.clone();
        if (object.userData.depth > 0) { // 非根节点添加轻微浮动
          object.userData.floatSpeed = Math.random() * 0.02 + 0.01;
          object.userData.floatDistance = Math.random() * 0.5 + 0.3;
        }
      }
    });
    log('添加场景动画效果');

    // ============== 7. 启动动画循环 ==============
    animate();
    log(`知识图谱构建完成，总耗时 ${Date.now() - startTime}ms`);

  } catch (err) {
    error("构建知识图谱失败:", err);
    showError.value = true;
    errorMessage.value = `构建失败: ${err.message}`;
    showEmptyState('构建失败: ' + err.message);
  } finally {
    isLoading.value = false;
    log('构建流程结束');
  }
};

const layoutNodes = () => {
  log('开始布局节点');

  if (!scene || nodes.length === 0) {
    warn('场景未初始化或没有节点可布局');
    return;
  }

  // ============== 1. 根节点居中 ==============
  const rootNode = nodes[0];
  rootNode.mesh.position.set(0, 0, 0);
  log(`根节点 "${rootNode.name}" 放置在中心位置 (0, 0, 0)`);

  // ============== 2. 第一层级布局（行星） ==============
  const firstLevelNodes = edges
    .filter(edge => edge.userData.from === rootNode.mesh)
    .map(edge => nodes.find(n => n.mesh === edge.userData.to));

  if (firstLevelNodes.length > 0) {
    const firstLevelRadius = 20; // 行星轨道半径
    const firstLevelAngleStep = (Math.PI * 2) / firstLevelNodes.length;
    const firstLevelZOffset = 5; // Z轴偏移量

    firstLevelNodes.forEach((node, index) => {
      const angle = firstLevelAngleStep * index;
      const zPos = index % 2 === 0 ? firstLevelZOffset : -firstLevelZOffset;

      node.mesh.position.set(
        Math.cos(angle) * firstLevelRadius,
        Math.sin(angle) * firstLevelRadius,
        zPos
      );
      log(`第一级节点 "${node.name}" 放置在 (${node.mesh.position.x.toFixed(1)}, ${node.mesh.position.y.toFixed(1)}, ${node.mesh.position.z.toFixed(1)})`);

      // ============== 3. 第二层级布局（卫星） ==============
      const secondLevelNodes = edges
        .filter(edge => edge.userData.from === node.mesh)
        .map(edge => nodes.find(n => n.mesh === edge.userData.to));

      if (secondLevelNodes.length > 0) {
        const secondLevelRadius = 10; // 卫星轨道半径
        const secondLevelAngleStep = (Math.PI * 2) / secondLevelNodes.length;
        const secondLevelZOffset = 3; // Z轴偏移量

        secondLevelNodes.forEach((subNode, subIndex) => {
          const subAngle = secondLevelAngleStep * subIndex;
          const subZPos = subIndex % 2 === 0 ?
            node.mesh.position.z + secondLevelZOffset :
            node.mesh.position.z - secondLevelZOffset;

          subNode.mesh.position.set(
            node.mesh.position.x + Math.cos(subAngle) * secondLevelRadius,
            node.mesh.position.y + Math.sin(subAngle) * secondLevelRadius,
            subZPos
          );
          log(`第二级节点 "${subNode.name}" 放置在 (${subNode.mesh.position.x.toFixed(1)}, ${subNode.mesh.position.y.toFixed(1)}, ${subNode.mesh.position.z.toFixed(1)})`);
        });
      }
    });
  }

  // ============== 4. 更新所有边的几何体 ==============
  log('开始更新所有连接线...');
  edges.forEach((edge, index) => {
    if (!edge.userData.from || !edge.userData.to) {
      warn(`边[${index}]的from/to未定义，跳过更新`);
      return;
    }

    // 更新线几何体
    edge.geometry.dispose();
    edge.geometry = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3().copy(edge.userData.from.position),
      new THREE.Vector3().copy(edge.userData.to.position)
    ]);

    // 更新箭头位置和方向
    if (edge.userData.arrow) {
      const dir = new THREE.Vector3().subVectors(
        edge.userData.to.position,
        edge.userData.from.position
      ).normalize();

      const arrowLength = Math.min(
        15,
        edge.userData.from.position.distanceTo(edge.userData.to.position) * 0.6
      );

      edge.userData.arrow.position.copy(edge.userData.from.position);
      edge.userData.arrow.setDirection(dir);
      edge.userData.arrow.setLength(
        arrowLength,
        Math.max(0.15, arrowLength * 0.15),
        Math.max(0.1, arrowLength * 0.1)
      );
    }
    log(`更新连接线 "${edge.userData.from.userData.name} -> ${edge.userData.to.userData.name}"`);
  });

  // ============== 5. 自动调整相机位置 ==============
  if (camera && controls) {
    const boundingBox = new THREE.Box3().setFromObject(scene);
    const size = boundingBox.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z);
    const fov = camera.fov * (Math.PI / 180);
    const cameraZ = Math.abs(maxDim / Math.sin(fov / 2)) * 1.2;

    camera.position.z = cameraZ;
    controls.target.set(0, 0, 0);
    controls.update();
    log(`自动调整相机位置到 (0, 0, ${cameraZ.toFixed(1)})`);
  }

  log('节点布局完成');
};

const createNodeMesh = (name, color, size = 3, depth = 0) => {
  // 根据深度调整节点属性
  let finalSize, finalColor, wireframe;

  switch (depth) {
    case 0: // 根节点（恒星）
      finalSize = 6;
      finalColor = 0xF39C12; // 橙色
      wireframe = false;
      break;
    case 1: // 第一级节点（行星）
      finalSize = 4;
      finalColor = 0x3498db; // 蓝色
      wireframe = false;
      break;
    case 2: // 第二级节点（卫星）
      finalSize = 2.5;
      finalColor = 0x2ecc71; // 绿色
      wireframe = name.endsWith('.md'); // 文件显示为线框
      break;
    default: // 更深层级
      finalSize = Math.max(1.5, 4 - depth * 0.8);
      finalColor = color;
      wireframe = name.endsWith('.md');
  }

  const geometry = new THREE.SphereGeometry(finalSize, 32, 32);
  const material = new THREE.MeshPhongMaterial({
    color: finalColor,
    transparent: true,
    opacity: wireframe ? 0.7 : 0.9,
    shininess: 100,
    specular: 0x222222,
    wireframe: wireframe,
    wireframeLinewidth: 1
  });

  const mesh = new THREE.Mesh(geometry, material);
  mesh.castShadow = true;
  mesh.receiveShadow = true;

  mesh.userData = {
    name,
    type: depth === 0 ? 'knowledgeBase' : (name.endsWith('.md') ? 'file' : 'directory'),
    depth
  };

  log(`创建 ${depth === 0 ? '根' : depth === 1 ? '第一级' : '第二级'}节点 "${name}" (大小: ${finalSize}, 颜色: ${finalColor.toString(16)}, ${wireframe ? '线框' : '实体'})`);
  return mesh;
};

const createEdge = (fromMesh, toMesh) => {
  // 创建连接线几何体
  const points = [
    new THREE.Vector3().copy(fromMesh.position),
    new THREE.Vector3().copy(toMesh.position)
  ];
  const geometry = new THREE.BufferGeometry().setFromPoints(points);

  // 根据连接类型设置不同样式
  const isFileConnection = toMesh.userData.type === 'file';
  const material = new THREE.LineBasicMaterial({
    color: isFileConnection ? 0x00ff00 : 0x5555ff, // 文件连接绿色，目录连接蓝色
    linewidth: isFileConnection ? 1 : 2,
    transparent: true,
    opacity: 0.8
  });

  const line = new THREE.Line(geometry, material);

  // 添加箭头（仅目录连接显示箭头）
  if (toMesh.userData.type !== 'file') {
    const dir = new THREE.Vector3().subVectors(
      toMesh.position,
      fromMesh.position
    ).normalize();

    const distance = fromMesh.position.distanceTo(toMesh.position);
    const arrowLength = Math.min(10, distance * 0.3);

    const arrow = new THREE.ArrowHelper(
      dir,
      fromMesh.position,
      arrowLength,
      0xff5555,  // 红色箭头
      arrowLength * 0.2,
      arrowLength * 0.1
    );

    line.userData = {
      type: 'edge',
      from: fromMesh,
      to: toMesh,
      arrow: arrow
    };

    scene.add(arrow);
    log(`创建带箭头的连接线 "${fromMesh.userData.name} -> ${toMesh.userData.name}"`);
  } else {
    line.userData = {
      type: 'edge',
      from: fromMesh,
      to: toMesh
    };
    log(`创建普通连接线 "${fromMesh.userData.name} -> ${toMesh.userData.name}"`);
  }

  return line;
};

// 在工具函数部分添加
const disposeMaterial = (material) => {
  if (!material) return;

  if (Array.isArray(material)) {
    material.forEach(m => disposeSingleMaterial(m));
  } else {
    disposeSingleMaterial(material);
  }
};

const disposeSingleMaterial = (material) => {
  material.dispose();
  // 特殊处理需要手动释放的纹理
  for (const key in material) {
    if (material[key]?.isTexture) {
      material[key].dispose();
    }
  }
};

// 增强版findNodeByPosition
const findNodeByPosition = (positionArray) => {
  if (!positionArray || positionArray.length < 3) return null;

  const position = new THREE.Vector3().fromArray(positionArray);
  return nodes.find(node => {
    if (!node.mesh) return false;
    return node.mesh.position.distanceTo(position) < 0.5; // 增加容差范围
  });
};

const animate = () => {
  requestAnimationFrame(animate);

  // 自动旋转时保持视角在中心
  if (controls && autoRotate.value) {
    controls.target.set(0, 0, 0);
  }

  // 更新控制器
  if (controls) controls.update();

  // 渲染场景
  renderer.render(scene, camera);

  // 调试信息
  if (DEBUG && frames % 120 === 0) {
    log('当前场景状态:');
    log(`- 节点数量: ${nodes.length}`);
    log(`- 边数量: ${edges.length}`);
    log(`- 相机位置: (${camera.position.x.toFixed(1)}, ${camera.position.y.toFixed(1)}, ${camera.position.z.toFixed(1)})`);

    nodes.forEach(node => {
      log(`节点 "${node.name}":`,
        `位置: (${node.mesh.position.x.toFixed(1)}, ${node.mesh.position.y.toFixed(1)}, ${node.mesh.position.z.toFixed(1)})`,
        `大小: ${node.mesh.geometry.parameters.radius.toFixed(1)}`,
        `类型: ${node.type}`
      );
    });
  }
  frames++;
};
let frames = 0; // 用于调试计数

const showEmptyState = (reason = '') => {
  clearScene()

  const group = new THREE.Group()
  const geometry = new THREE.TorusGeometry(3, 0.5, 16, 32)
  const material = new THREE.MeshBasicMaterial({
    color: 0x3498db,
    wireframe: true,
    transparent: true,
    opacity: 0.6
  })
  const torus = new THREE.Mesh(geometry, material)
  torus.rotation.x = Math.PI / 2
  group.add(torus)

  if (typeof CSS2DObject !== 'undefined') {
    const label = document.createElement('div')
    label.className = 'empty-state-label'
    label.textContent = reason || '请选择知识库'
    const labelObject = new CSS2DObject(label)
    labelObject.position.set(0, -5, 0)
    group.add(labelObject)
  }

  scene.add(group)

  const animateGroup = () => {
    group.rotation.y += 0.01
    requestAnimationFrame(animateGroup)
  }
  animateGroup()
}

// ============== 生命周期 ==============
watch(
  () => store.selectedKnowledgeBase,
  (newVal) => {
    if (!scene) {
      warn('场景未初始化，将延迟处理知识库变化');
      pendingKnowledgeBaseChange.value = newVal;
      return;
    }

    if (isValidKnowledgeBase(newVal)) {
      log('检测到有效知识库:', newVal.knowledge_base_name);
      nextTick(() => buildGraph());
    } else if (knowledgeBases.value.length > 0) {
      store.setSelectedKnowledgeBase(knowledgeBases.value[0]);
    } else {
      showEmptyState('没有可用的知识库');
    }
  },
  { immediate: true, deep: true }
);

onMounted(async () => {
  log('组件挂载开始');

  await nextTick();
  initThreeJS();

  // 添加事件监听（修改后版本）
  window.addEventListener('resize', handleWindowResize, { passive: true });
  if (renderer?.domElement) {
    renderer.domElement.style.cursor = 'pointer';
    renderer.domElement.addEventListener('click', handleCanvasClick);
  }

  // 原有知识库加载逻辑保持不变
  if (knowledgeBases.value.length === 0) {
    try {
      await store.fetchKnowledgeBases();
    } catch (err) {
      error('加载知识库列表失败:', err);
      showError.value = true;
      errorMessage.value = '加载知识库列表失败';
    }
  }

  if (hasValidKnowledgeBase.value) {
    await buildGraph();
  } else if (knowledgeBases.value.length > 0) {
    store.setSelectedKnowledgeBase(knowledgeBases.value[0]);
  } else {
    showEmptyState('没有可用的知识库');
  }
});

onBeforeUnmount(() => {
  log('开始组件卸载清理');

  // 1. 移除事件监听器
  window.removeEventListener('resize', handleWindowResize);
  if (renderer?.domElement) {
    renderer.domElement.removeEventListener('click', handleCanvasClick);
  }

  // 2. 清理Three.js场景
  if (scene) {
    log('开始清理场景对象');

    // 清理所有材质和几何体
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

      // 特殊处理箭头辅助对象
      if (object.isArrowHelper) {
        scene.remove(object);
        log('已移除箭头辅助对象');
      }
    });

    // 清空场景
    scene.children = [];
  }

  // 3. 移除渲染器DOM
  if (container.value && renderer?.domElement) {
    container.value.removeChild(renderer.domElement);
    log('已移除渲染器DOM');
  }

  // 4. 释放控制器
  if (controls) {
    controls.dispose();
    log('已释放控制器');
  }

  // 5. 释放渲染器
  if (renderer) {
    renderer.dispose();
    renderer.forceContextLoss();
    log('已释放渲染器');
  }

  // 6. 清空引用
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
.knowledge-graph-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
}

/* 节点发光效果 */
.knowledge-graph-container:deep(canvas) {
  filter: drop-shadow(0 0 8px rgba(255, 255, 255, 0.3));
}

.toolbar {
  padding: 8px 16px;
  z-index: 10;
  backdrop-filter: blur(5px);
  /* 固定亮色背景 */
  background-color: rgba(255, 255, 255, 0.7) !important;
  /* 固定亮色边框 */
  border-bottom: 1px solid #e0e0e0 !important;
}

.graph-container {
  flex-grow: 1;
  width: 100%;
  height: calc(100% - 40px);
  overflow: hidden;
  background: linear-gradient(to bottom, #1a1a2e, #16213e) !important;
}

/* 图形容器样式 */
.graph-container {
  flex-grow: 1;
  width: 100%;
  height: calc(100% - 40px);
  overflow: hidden;
  /* 初始亮色背景 */
  background: linear-gradient(to bottom, #f0f0f0, #e0e0e0) !important;
}

/* 暗色模式仅影响图形容器 */
.dark-mode .graph-container {
  background: linear-gradient(to bottom, #121212, #0a0a0a) !important;
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.2);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.node-info-panel {
  position: absolute;
  bottom: 20px;
  right: 20px;
  max-width: 300px;
  z-index: 10;
  backdrop-filter: blur(5px);
  background-color: rgba(var(--n-color-modal-rgb), 0.85) !important;
  border: 1px solid var(--n-border-color) !important;
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.error-panel {
  position: absolute;
  top: 50px;
  left: 50%;
  transform: translateX(-50%);
  width: 80%;
  max-width: 600px;
  z-index: 100;
}

/* 增强空状态样式 */
.empty-state-label {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  font-size: 16px;
  padding: 12px 16px;
  border-radius: 6px;
  background: rgba(var(--n-color-embedded-rgb), 0.9);
  backdrop-filter: blur(4px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  transform: translateY(20px);
}

.dark-mode {
  --n-color: #1a1a1a;
  --n-border-color: #333;
  --n-text-color: rgba(255, 255, 255, 0.82);
}

.dark-mode .empty-state-label {
  background: rgba(30, 30, 30, 0.9);
  color: #fff;
}

:not(.dark-mode) {
  --n-color: #f5f5f5;
  --n-border-color: #e0e0e0;
  --n-text-color: rgba(0, 0, 0, 0.82);
}
</style>