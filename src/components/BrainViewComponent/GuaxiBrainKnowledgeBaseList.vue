<template>
  <div class="knowledge-base-list">
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title">知识图谱</n-text>
      </div>
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-icon-wrapper :size="18" :border-radius="6" class="refresh-icon" @click="refreshList">
            <n-icon :component="RefreshIcon" size="16" />
          </n-icon-wrapper>
        </template>
        刷新知识库
      </n-tooltip>
    </div>

    <div class="kb-list">
      <div
        v-for="kb in orderedKnowledgeBases"
        :key="kb.id"
        v-show="!isSelectedKnowledgeBaseExpanded || kb.id === selectedId"
        class="kb-list-item"
        :class="{ active: kb.id === selectedId, expanded: kb.id === selectedId && expanded }"
      >
        <div class="kb-item" @click="toggleKnowledgeBase(kb)">
          <div class="kb-title">
            <n-icon size="18" class="kb-icon">
              <book-icon />
            </n-icon>
            {{ kb.knowledge_base_name }}
            <n-tag v-if="kb.tags" size="small" type="info" round style="margin-left: 8px;">
              {{ kb.tags.split(',')[0] }}
            </n-tag>
          </div>
          <n-icon size="18" class="expand-icon">
            <chevron-down-icon v-if="kb.id === selectedId && expanded" />
            <chevron-forward-icon v-else />
          </n-icon>
        </div>

        <div
          v-if="kb.id === selectedId && expanded"
          ref="treeContainerRef"
          class="tree-container"
          tabindex="0"
          @keydown.capture="handleTreeKeydown"
          @wheel.stop
        >
          <n-tree
            :key="`tree-${selectedId}`"
            :data="treeData"
            :block-line="true"
            :expand-on-click="true"
            :expanded-keys="expandedKeys"
            :selected-keys="selectedKeys"
            :render-prefix="renderTreePrefix"
            class="kb-tree"
            @update:expanded-keys="handleTreeExpand"
            @update:selected-keys="handleTreeSelect"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted, h, shallowRef, onUnmounted, computed, nextTick } from 'vue';
import { storeToRefs } from 'pinia';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { invoke } from '@tauri-apps/api/core';
import { useMessage } from 'naive-ui';
import {
  NTree, NTooltip, NButton, NIcon, NTag
} from 'naive-ui';

import {
  RefreshOutline as RefreshIcon,
  ChevronDownOutline as ChevronDownIcon,
  ChevronForwardOutline as ChevronForwardIcon,
} from '@vicons/ionicons5';

import {
  Book24Regular as BookIcon,
} from '@vicons/fluent';

// 与主编辑器知识编辑模式文件树共用同一套精致彩色图标
import { Icon } from '@iconify/vue';
import { getFileTreeIcon } from '../HomeViewComponent/fileTreeIcons';
import { onKgIndexChanged } from '../../utils/kgIndexEvents';

const logger = createLogger('GuaxiBrainKnowledgeBaseList')
const TREE_KEYBOARD_ROW_HEIGHT = 28;
const TREE_SCROLL_PADDING = 6;
const store = useKnowledgeBaseStore();
const message = useMessage();
const { knowledgeBases, selectedKnowledgeBase } = storeToRefs(store);

// 使用 shallowRef 替代 ref 避免不必要的深度响应式
const selectedId = computed(() => selectedKnowledgeBase.value?.id || null);

// 激活的知识库置顶,其余保持原序
const orderedKnowledgeBases = computed(() => {
  const list = Array.isArray(knowledgeBases.value) ? knowledgeBases.value : [];
  const sid = selectedId.value;
  if (!sid) return list;
  const active = list.filter((k) => k.id === sid);
  const rest = list.filter((k) => k.id !== sid);
  return [...active, ...rest];
});
const treeData = shallowRef([]);
const expanded = ref(false);
const expandedKeys = ref([]);
const selectedKeys = ref([]);
const hiddenRootKey = ref('');
const treeContainerRef = ref(null);
let abortController = new AbortController();
// 收起态收到索引变更时记脏,重新展开时补刷一次文件树
let treeStale = false;

// 设计说明: 知识图谱侧边栏仍使用 Naive Tree 负责渲染，但键盘导航由本组件接管。
// 这样 selectedKeys、图谱定位和滚动容器始终来自同一份可见节点序列，避免焦点到底部后只移动内部焦点、不推动外层滚动条。
const visibleTreeNodes = computed(() => {
  const result = [];
  const expandedSet = new Set(expandedKeys.value);

  const walk = (nodes, depth = 0) => {
    if (!Array.isArray(nodes)) return;
    for (const node of nodes) {
      result.push({ node, depth });
      if (node?.isDir && expandedSet.has(node.key)) {
        walk(node.children, depth + 1);
      }
    }
  };

  walk(treeData.value);
  return result;
});

// 设计说明：与主界面知识编辑模式保持一致。
// 选中的知识库展开后，其它知识库条目暂时隐藏，让当前知识库文件树独占列表高度，避免被下方条目挤压。
const isSelectedKnowledgeBaseExpanded = computed(() => Boolean(selectedId.value && expanded.value));

// 确保在切换知识库时正确处理树形数据
const toggleKnowledgeBase = async (kb) => {
  try {
    // 取消之前的请求
    abortController.abort();
    abortController = new AbortController();

    if (selectedId.value === kb.id) {
      // 同一个知识库，切换展开状态
      expanded.value = !expanded.value;
      logger.info('[BrainKnowledgeBaseList] 切换当前知识库展开状态', {
        knowledgeBase: kb.knowledge_base_name,
        expanded: expanded.value
      });
      if (!expanded.value) {
        expandedKeys.value = [];
      } else {
        // 收起期间若磁盘有变更(索引变更事件已记脏),重新展开时补一次刷新
        if (treeStale) {
          treeStale = false;
          await reloadTreeFromDisk('reexpand-stale');
        }
        ensureTreeKeyboardStart('toggle-current-expanded');
      }
    } else {
      // 不同的知识库，重置状态
      expanded.value = true;
      // selectedId.value = kb.id;
      store.setSelectedKnowledgeBase(kb);
      treeData.value = []; // 先清空数据

      // 异步加载树形数据
      const root = await invoke("get_knowledge_base_tree",
        { rootPath: kb.location },
        { signal: abortController.signal }
      );

      applyLoadedTree(root, 'toggle', kb);
    }
  } catch (error) {
    if (error.name !== 'AbortError') {
      logger.error("切换知识库失败:", error);
      message.error("加载知识库内容失败");
    }
  }
};

// 处理树节点展开状态变化
const handleTreeExpand = (keys) => {
  expandedKeys.value = keys;
  scrollSelectedTreeNodeIntoView('expand');
};

function getTreeScrollContainer() {
  const rawContainer = treeContainerRef.value;
  return Array.isArray(rawContainer) ? rawContainer.find(Boolean) : rawContainer;
}

function getSelectedTreeKey() {
  return Array.isArray(selectedKeys.value) ? selectedKeys.value[0] : null;
}

function scrollSelectedTreeNodeIntoView(reason = 'select') {
  nextTick(() => {
    const container = getTreeScrollContainer();
    if (!container) return;
    const selectedNode = container.querySelector(
      '.n-tree-node--selected .n-tree-node-content, .n-tree-node-content--selected'
    );
    if (!selectedNode) return;
    const containerRect = container.getBoundingClientRect();
    const selectedRect = selectedNode.getBoundingClientRect();
    const maxScrollTop = Math.max(0, container.scrollHeight - container.clientHeight);
    let nextScrollTop = container.scrollTop;

    if (selectedRect.top < containerRect.top + TREE_SCROLL_PADDING) {
      nextScrollTop -= containerRect.top + TREE_SCROLL_PADDING - selectedRect.top;
    } else if (selectedRect.bottom > containerRect.bottom - TREE_SCROLL_PADDING) {
      nextScrollTop += selectedRect.bottom - (containerRect.bottom - TREE_SCROLL_PADDING);
    }

    const clampedScrollTop = Math.max(0, Math.min(maxScrollTop, nextScrollTop));
    if (clampedScrollTop !== container.scrollTop) {
      container.scrollTop = clampedScrollTop;
    }

    logger.debug('[BrainKnowledgeBaseList] 文件树键盘焦点滚动同步', {
      reason,
      selectedKey: getSelectedTreeKey(),
      scrollTop: container.scrollTop,
      maxScrollTop
    });
  });
}

function findTreeOptionByKey(nodes, key) {
  if (!key || !Array.isArray(nodes)) return null;
  for (const node of nodes) {
    if (node?.key === key) return node;
    const found = findTreeOptionByKey(node?.children, key);
    if (found) return found;
  }
  return null;
}

function getSelectedVisibleTreeIndex() {
  const key = getSelectedTreeKey();
  if (!key) return -1;
  return visibleTreeNodes.value.findIndex((item) => item.node?.key === key);
}

function ensureTreeKeyboardStart(reason = 'tree-open') {
  nextTick(() => {
    const container = getTreeScrollContainer();
    if (container && document.activeElement !== container) {
      container.focus({ preventScroll: true });
    }

    const nodes = visibleTreeNodes.value;
    if (!nodes.length) return;

    const previousKey = getSelectedTreeKey();
    const target = nodes[0]?.node;
    if (!target?.key) return;

    selectedKeys.value = [target.key];
    logger.debug('[BrainKnowledgeBaseList] 初始化文件树键盘起始条目', {
      reason,
      key: target.key,
      previousKey,
      focusedTreeContainer: document.activeElement === container
    });
    scrollSelectedTreeNodeIntoView(reason);
  });
}

function applyTreeSelection(key, reason = 'select') {
  if (!key) return false;
  selectedKeys.value = [key];
  store.setSelectedTreeNodeKey(key);

  if (store.graphViewMode === 'kg3d') {
    store.requestKg3dAction('selectByPath', { path: key });
  }

  logger.debug('[BrainKnowledgeBaseList] 文件树键盘/鼠标选择同步', {
    reason,
    key
  });
  scrollSelectedTreeNodeIntoView(reason);
  return true;
}

function selectVisibleTreeNodeByIndex(index, reason = 'keyboard') {
  const nodes = visibleTreeNodes.value;
  if (!nodes.length) return false;
  const boundedIndex = Math.max(0, Math.min(nodes.length - 1, index));
  const target = nodes[boundedIndex]?.node;
  if (!target?.key) return false;
  return applyTreeSelection(target.key, reason);
}

function setDirectoryExpanded(key, expandedState, reason) {
  const set = new Set(expandedKeys.value);
  const wasExpanded = set.has(key);
  if (expandedState) set.add(key);
  else set.delete(key);
  expandedKeys.value = Array.from(set);

  logger.info('[BrainKnowledgeBaseList] 键盘切换目录展开状态', {
    reason,
    key,
    wasExpanded,
    expanded: expandedState
  });
}

function findParentVisibleIndex(currentIndex) {
  const nodes = visibleTreeNodes.value;
  const current = nodes[currentIndex];
  if (!current || current.depth <= 0) return -1;
  for (let i = currentIndex - 1; i >= 0; i -= 1) {
    if (nodes[i].depth === current.depth - 1) return i;
  }
  return -1;
}

function toggleSelectedDirectoryByEnter() {
  const key = getSelectedTreeKey();
  const node = findTreeOptionByKey(treeData.value, key);
  if (!node || node.isLeaf || !node.isDir) return false;

  const willExpand = !expandedKeys.value.includes(key);
  setDirectoryExpanded(key, willExpand, 'enter');
  scrollSelectedTreeNodeIntoView('enter-toggle');
  return true;
}

const handleTreeKeydown = (event) => {
  const navigationKeys = new Set([
    'ArrowUp',
    'ArrowDown',
    'ArrowLeft',
    'ArrowRight',
    'Home',
    'End',
    'PageUp',
    'PageDown',
    'Enter'
  ]);
  if (!navigationKeys.has(event.key)) return;

  event.preventDefault();
  event.stopPropagation();

  if (event.key === 'Enter') {
    toggleSelectedDirectoryByEnter();
    return;
  }

  const nodes = visibleTreeNodes.value;
  if (!nodes.length) return;

  const container = getTreeScrollContainer();
  const pageStep = Math.max(1, Math.floor((container?.clientHeight || TREE_KEYBOARD_ROW_HEIGHT) / TREE_KEYBOARD_ROW_HEIGHT) - 1);
  const currentIndex = getSelectedVisibleTreeIndex();
  const normalizedIndex = currentIndex === -1 ? 0 : currentIndex;
  const current = nodes[normalizedIndex];
  let targetIndex = currentIndex === -1 ? 0 : currentIndex;

  if (event.key === 'ArrowDown') {
    targetIndex = currentIndex === -1 ? 0 : normalizedIndex + 1;
  } else if (event.key === 'ArrowUp') {
    targetIndex = currentIndex === -1 ? nodes.length - 1 : normalizedIndex - 1;
  } else if (event.key === 'Home') {
    targetIndex = 0;
  } else if (event.key === 'End') {
    targetIndex = nodes.length - 1;
  } else if (event.key === 'PageDown') {
    targetIndex = normalizedIndex + pageStep;
  } else if (event.key === 'PageUp') {
    targetIndex = normalizedIndex - pageStep;
  } else if (event.key === 'ArrowRight') {
    const key = current?.node?.key;
    if (current?.node?.isDir && key && !expandedKeys.value.includes(key)) {
      setDirectoryExpanded(key, true, 'arrow-right');
      scrollSelectedTreeNodeIntoView('arrow-right-expand');
      return;
    }
    if (nodes[normalizedIndex + 1]?.depth > current?.depth) {
      targetIndex = normalizedIndex + 1;
    }
  } else if (event.key === 'ArrowLeft') {
    const key = current?.node?.key;
    if (current?.node?.isDir && key && expandedKeys.value.includes(key)) {
      setDirectoryExpanded(key, false, 'arrow-left');
      scrollSelectedTreeNodeIntoView('arrow-left-collapse');
      return;
    }
    const parentIndex = findParentVisibleIndex(normalizedIndex);
    if (parentIndex !== -1) targetIndex = parentIndex;
  }

  selectVisibleTreeNodeByIndex(targetIndex, `keyboard-${event.key}`);
};

const handleTreeSelect = (keys) => {
  selectedKeys.value = keys;
  const key = Array.isArray(keys) ? keys[0] : null;
  if (key) {
    applyTreeSelection(key, 'select');
  } else {
    logger.debug('[BrainKnowledgeBaseList] 文件树选择已清空', {
      hiddenRootKey: hiddenRootKey.value
    });
  }
};

// 刷新列表
const refreshList = async () => {
  try {
    await store.fetchKnowledgeBases();
    if (selectedId.value) {
      const kb = knowledgeBases.value.find(k => k.id === selectedId.value);
      if (kb) {
        const root = await invoke("get_knowledge_base_tree", { rootPath: kb.location });
        applyLoadedTree(root, 'refresh', kb);
      }
    }
  } catch (error) {
    logger.error("刷新知识库列表失败:", error);
    message.error("刷新知识库列表失败");
  }
};

// 不在知识图谱文件树中展示的内部/元数据文件
function isHiddenTreeNode(node) {
  const name = String(node?.name || '');
  if (!name) return false;
  const lower = name.toLowerCase();
  // 隐藏:.meta / .cellmeta / 其它元数据与隐藏文件
  if (name.startsWith('.')) return true;
  if (lower.endsWith('.cellmeta') || lower.endsWith('.meta')) return true;
  if (lower.endsWith('_meta.json')) return true;
  if (name === '.meta' || name === '.git') return true;
  return false;
}

// 转换树形数据结构
function convertToTreeNode(node) {
  const key = String(node.path || '').replace(/\\/g, '/');
  const kids = Array.isArray(node.children)
    ? node.children.filter((c) => !isHiddenTreeNode(c)).map(convertToTreeNode)
    : undefined;
  return {
    label: node.name,
    key,
    isLeaf: !node.is_dir,
    isDir: !!node.is_dir,
    children: kids
  };
}

// 设计说明：后端返回的 root 是知识库目录本身，而标题行已经展示了该知识库。
// 因此这里把 root 作为隐藏容器，只渲染它的 children，避免左侧树出现重复的顶级根节点。
function buildVisibleTreeData(root) {
  const rootKey = String(root?.path || '').replace(/\\/g, '/');
  const nodes = Array.isArray(root?.children)
    ? root.children.filter((child) => !isHiddenTreeNode(child)).map(convertToTreeNode)
    : [];
  return { rootKey, nodes };
}

// 收集树中全部 key,供刷新时剔除已消失的展开/选中项。
function collectTreeKeys(nodes, out = new Set()) {
  if (!Array.isArray(nodes)) return out;
  for (const node of nodes) {
    if (node?.key) out.add(node.key);
    collectTreeKeys(node?.children, out);
  }
  return out;
}

// preserveState:索引变更引发的静默刷新用。保留用户已展开的目录与当前选中项(只剔除已删除的 key),
// 否则每次外部新建文件都会把整棵树收起、焦点跳回顶部。
function applyLoadedTree(root, reason, kb, { preserveState = false } = {}) {
  const { rootKey, nodes } = buildVisibleTreeData(root);
  hiddenRootKey.value = rootKey;
  treeData.value = nodes;

  if (preserveState) {
    const alive = collectTreeKeys(nodes);
    expandedKeys.value = expandedKeys.value.filter((k) => alive.has(k));
    selectedKeys.value = selectedKeys.value.filter((k) => alive.has(k));
  } else {
    expandedKeys.value = [];
    selectedKeys.value = [];
    ensureTreeKeyboardStart(`load-${reason}`);
  }

  if (rootKey && !preserveState) {
    store.setSelectedTreeNodeKey(rootKey);
  }

  logger.info('[BrainKnowledgeBaseList] 已加载知识库文件树并隐藏根节点', {
    reason,
    preserveState,
    knowledgeBase: kb?.knowledge_base_name,
    rootPath: kb?.location,
    hiddenRootKey: rootKey,
    visibleTopLevelCount: nodes.length
  });
}

// 从磁盘重新拉取当前知识库文件树(保留展开/选中态),供索引变更事件调用。
async function reloadTreeFromDisk(reason) {
  const kb = knowledgeBases.value.find((k) => k.id === selectedId.value);
  if (!kb) return;
  try {
    const root = await invoke("get_knowledge_base_tree", { rootPath: kb.location });
    applyLoadedTree(root, reason, kb, { preserveState: true });
  } catch (error) {
    logger.warn('[BrainKnowledgeBaseList] 静默刷新文件树失败', { reason, error });
  }
}

// 树前缀图标:与主界面文件树一致的彩色类型图标,目录随展开态切换开/合。
function renderTreePrefix({ option }) {
  const expanded = Array.isArray(expandedKeys.value) && expandedKeys.value.includes(option.key);
  return h(Icon, {
    icon: getFileTreeIcon(option.label, !!option.isDir, expanded),
    width: 18,
    height: 18,
    class: option.isDir ? 'kb-tree-type-icon is-dir' : 'kb-tree-type-icon',
  });
}

// 主界面在知识库里新建/删除/移动文件后，后端广播 kg_index_changed，这里静默跟随刷新文件树。
let unlistenIndex = null;
const handleIndexChanged = async ({ kbName, reason }) => {
  if (reason === 'removed') {
    await store.fetchKnowledgeBases();
    return;
  }
  const kb = knowledgeBases.value.find((k) => k.id === selectedId.value);
  if (!kb || kb.knowledge_base_name !== kbName) return;
  if (!expanded.value) { treeStale = true; return; }
  treeStale = false;
  await reloadTreeFromDisk(`index-${reason}`);
};

onMounted(async () => {
  await store.fetchKnowledgeBases(); // 会自动选中第一个

  // 如果已有选中项，加载其树数据
  if (selectedKnowledgeBase.value) {
    const root = await invoke("get_knowledge_base_tree", {
      rootPath: selectedKnowledgeBase.value.location
    });
    applyLoadedTree(root, 'mounted', selectedKnowledgeBase.value);
  }

  try {
    unlistenIndex = await onKgIndexChanged(handleIndexChanged);
  } catch (error) {
    logger.warn('[BrainKnowledgeBaseList] 监听索引变更失败', error);
  }
});

onUnmounted(() => {
  if (unlistenIndex) { try { unlistenIndex(); } catch (_) { /* noop */ } }
  abortController.abort();
  treeData.value = [];
  expandedKeys.value = [];
  selectedKeys.value = [];
  hiddenRootKey.value = '';
});
</script>

<style scoped>
/* 刷新图标样式 */
.refresh-icon {
  color: var(--n-text-color-secondary);
  cursor: pointer;
  transition: all 0.3s ease;
  padding: 4px;
}

.refresh-icon:hover {
  color: var(--n-color-primary);
  background-color: var(--n-color-primary-subtle);
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .refresh-icon:hover {
  background-color: rgba(65, 105, 225, 0.2);
}

/* 添加 tree-container 确保树形结构有稳定的容器 */
.tree-container {
  width: 100%;
  /* 设计说明：展开知识库时，仅让树容器内部滚动；外层列表高度保持稳定，避免文件树被其它条目挤压。 */
  flex: 1 1 auto;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding-left: 0;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 150, 0.4) transparent;
}

.tree-container:focus {
  outline: none;
}

.tree-container::-webkit-scrollbar {
  width: 9px;
  height: 9px;
}

.tree-container::-webkit-scrollbar-track {
  background: transparent;
}

.tree-container::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 150, 0.38);
  border-radius: 10px;
  border: 2.5px solid transparent;
  background-clip: padding-box;
}

.tree-container::-webkit-scrollbar-thumb:hover {
  background: rgba(120, 120, 130, 0.6);
  background-clip: padding-box;
}

.knowledge-base-list {
  padding: 0px;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 与主界面文件树 / 编辑器同一套字体（--guaxi-editor-font-family）。
     naive 的树组件没有设置 font-family，不显式指定会落回 WebView 默认无衬线字体，
     同一份文件列表在两个页面看起来像两种字体。 */
  font-family: var(
    --guaxi-editor-font-family,
    'Maple Mono NF CN',
    'JetBrainsMono Nerd Font Mono',
    Consolas,
    monospace,
    'Microsoft YaHei',
    'Courier New'
  );
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  margin-bottom: 8px;
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.title-container {
  display: flex;
  align-items: center;
}

.repo-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
}

.repo-title::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 16px;
  background-color: var(--guaxi-text-primary);
  margin-right: 8px;
  border-radius: 2px;
}

/* 刷新按钮样式调整 - 修改为GuaxiFileTree风格 */
.refresh-btn {
  background-color: transparent !important;
  color: var(--n-text-color-secondary) !important;
  border: none;
  transition: all 0.3s ease;
  box-shadow: none !important;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.refresh-btn:hover {
  background-color: var(--n-color-primary-subtle) !important;
  color: var(--n-color-primary) !important;
  transform: rotate(90deg);
}

.refresh-btn .n-icon {
  font-size: 18px;
}

/* 暗色模式适配 */
.dark-mode .refresh-btn {
  color: var(--n-text-color-secondary) !important;
}

.dark-mode .refresh-btn:hover {
  background-color: rgba(65, 105, 225, 0.2) !important;
  color: var(--n-color-primary) !important;
}

/* 修改滚动条样式 - 变细且更现代 */
.kb-list::-webkit-scrollbar {
  width: 9px;
  height: 9px;
}

.kb-list::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 150, 0.38);
  border-radius: 10px;
  border: 2.5px solid transparent;
  background-clip: padding-box;
}

.kb-list::-webkit-scrollbar-thumb:hover {
  background: rgba(120, 120, 130, 0.6);
  background-clip: padding-box;
}

.kb-list::-webkit-scrollbar-track {
  background: transparent;
}

/* Firefox滚动条样式 */
.kb-list {
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 150, 0.4) transparent;
}

.kb-list {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 设计说明：收缩态需要列表自身可滚动；展开态其它条目已 v-show 隐藏，树仍由 .tree-container 内部滚动。 */
  overflow-x: hidden;
  overflow-y: auto;
  border-radius: 8px;
  padding-left: 0;
  padding-right: 0;
}

.kb-list-item {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 8px 12px;
  transition: background 0.2s ease;
  border-radius: 6px;
  margin: 2px 0;
  position: relative;
  /* 为伪元素定位做准备 */
  overflow: hidden;
  /* 防止光圈溢出 */
}

/* 展开的知识库条目:其它条目通过 v-show 隐藏后，当前条目独占列表高度，让内部树铺满并可滚动。 */
.kb-list-item.expanded {
  flex: 1 1 auto;
  min-height: 0;
}

/* 鼠标悬停效果 */
.kb-list-item::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(24, 144, 255, 0.1);
  border-radius: 6px;
  opacity: 0;
  transition: opacity 0.3s ease;
  z-index: -1;
}

.kb-list-item:hover::before {
  opacity: 1;
}

/* 鼠标悬停时的文字和图标效果 */
.kb-list-item:hover .kb-title {
  color: #1890ff;
}

.kb-list-item:hover .kb-icon {
  color: #1890ff;
}

.kb-list-item:hover .expand-icon {
  color: #1890ff;
}

/* 选中状态只保留文字和图标强调，和主界面的知识库标题行保持一致。 */
.kb-list-item.active {
  border-left: none;
}

/* 标题行不再铺蓝色高亮底，避免和主界面文件树视觉不一致。 */
.kb-list-item.active .kb-item {
  background: transparent;
  border-radius: 6px;
}

/* 选中状态下的文字和图标 */
.kb-list-item.active .kb-title,
.kb-list-item.active .kb-icon,
.kb-list-item.active .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

/* 暗色模式:保留更亮的蓝字提示当前知识库，不额外铺底色。 */
:is(.dark, .dark-mode) .kb-list-item.active {
  border-left-color: transparent;
}
:is(.dark, .dark-mode) .kb-list-item.active .kb-item {
  background: transparent;
}
:is(.dark, .dark-mode) .kb-list-item.active .kb-title,
:is(.dark, .dark-mode) .kb-list-item.active .kb-icon,
:is(.dark, .dark-mode) .kb-list-item.active .expand-icon {
  color: #58a6ff;
}
:is(.dark, .dark-mode) .kb-list-item:hover .kb-title,
:is(.dark, .dark-mode) .kb-list-item:hover .kb-icon,
:is(.dark, .dark-mode) .kb-list-item:hover .expand-icon {
  color: #58a6ff;
}
:is(.dark, .dark-mode) .kb-list-item:hover::before {
  background: rgba(56, 139, 253, 0.12);
}

.kb-item {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  cursor: pointer;
  padding-left: 4px;
  /* 内容区域左内边距 */
  padding-right: 4px;
  /* 内容区域右内边距 */
}

.kb-title {
  display: flex;
  align-items: center;
  font-weight: 500;
  font-size: 15px;
}

.kb-icon {
  margin-right: 8px;
  color: var(--n-color-primary);
}

.expand-icon {
  color: var(--n-text-color-secondary);
  transition: transform 0.2s ease;
}

.n-list-item {
  padding: 0 16px;
  transition: background-color 0.2s ease;
}

.n-list-item:hover {
  background-color: var(--n-color-embedded);
}

.n-list-item.active {
  background-color: var(--n-color-primary-subtle);
}

.kb-tree {
  height: auto;
  min-height: 0;
  margin: 6px 0 0 0;
  border-left: none;
  padding: 0 0 12px 0;
  box-sizing: border-box;
}

/* ───── 复刻主编辑器文件树行观感:圆角 hover、无额外蓝色选中底、彩色类型图标 ───── */
.kb-tree :deep(.n-tree-node) {
  padding: 1px 0;
}

.kb-tree :deep(.n-tree-node-content) {
  min-height: 28px;
  padding: 2px 6px 2px 2px;
  border-radius: 8px;
  margin: 1px 2px;
  transition: background-color 0.14s ease, box-shadow 0.14s ease;
}

.kb-tree :deep(.n-tree-node-content:hover) {
  background: rgba(120, 120, 128, 0.12);
}

.kb-tree :deep(.n-tree-node--selected .n-tree-node-content),
.kb-tree :deep(.n-tree-node-content--selected) {
  background: transparent;
  box-shadow: none;
}

/* 类型图标:与主树一致的尺寸与柔光质感 */
.kb-tree :deep(.kb-tree-type-icon) {
  display: block;
  filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.18)) saturate(1.04);
  transition: transform 0.14s ease, filter 0.14s ease;
}
/* 文件夹:单色图标统一染成淡蓝高级色,与主树一致 */
.kb-tree :deep(.kb-tree-type-icon.is-dir) {
  color: #5b9bd5;
  filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.10));
}
:is(.dark, .dark-mode) .kb-tree :deep(.kb-tree-type-icon.is-dir) {
  color: #7fb2e8;
}
.kb-tree :deep(.n-tree-node-content:hover .kb-tree-type-icon) {
  transform: scale(1.06);
  filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.22)) saturate(1.12) brightness(1.04);
}

/* 前缀与文本间距,贴近主树排版 */
.kb-tree :deep(.n-tree-node-content__prefix) {
  margin-right: 6px;
  display: inline-flex;
  align-items: center;
}
.kb-tree :deep(.n-tree-node-content__text) {
  font-size: 14px;
  letter-spacing: 0.1px;
  border-bottom: none;
}

/* 展开箭头配色对齐次级文本色 */
.kb-tree :deep(.n-tree-node-switcher) {
  color: var(--n-text-color-secondary);
}

/* 暗色模式 hover/选中微调 */
:is(.dark, .dark-mode) .kb-tree :deep(.n-tree-node-content:hover) {
  background: rgba(120, 120, 128, 0.18);
}
:is(.dark, .dark-mode) .kb-tree :deep(.n-tree-node--selected .n-tree-node-content),
:is(.dark, .dark-mode) .kb-tree :deep(.n-tree-node-content--selected) {
  background: transparent;
  box-shadow: none;
}
</style>
