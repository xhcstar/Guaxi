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

        <div v-if="kb.id === selectedId && expanded" class="tree-container" @wheel.stop>
          <n-tree
            :key="`tree-${selectedId}`"
            :data="treeData"
            :block-line="true"
            :expand-on-click="true"
            :expanded-keys="expandedKeys"
            :selected-keys="selectedKeys"
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
import { ref, onMounted, h, shallowRef, onUnmounted, computed } from 'vue';
import { storeToRefs } from 'pinia';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { invoke } from '@tauri-apps/api/core';
import { useMessage } from 'naive-ui';
import {
  NTree, NTooltip, NButton, NIcon, NTag
} from 'naive-ui';

import {
  BookOutline as BookListIcon,
  RefreshOutline as RefreshIcon,
  ChevronDownOutline as ChevronDownIcon,
  ChevronForwardOutline as ChevronForwardIcon,
  DocumentTextOutline as DocumentIcon,
} from '@vicons/ionicons5';

import {
  Book24Regular as BookIcon,
} from '@vicons/fluent';

const logger = createLogger('GuaxiBrainKnowledgeBaseList')
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
let abortController = new AbortController();

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

      const newTreeData = [convertToTreeNode(root)];

      treeData.value = newTreeData;
      expandedKeys.value = [newTreeData[0].key];
      selectedKeys.value = [newTreeData[0].key];
      store.setSelectedTreeNodeKey(newTreeData[0].key);
      logger.info('[BrainKnowledgeBaseList] 展开知识库并加载文件树', {
        knowledgeBase: kb.knowledge_base_name,
        rootPath: kb.location,
        rootKey: newTreeData[0].key
      });
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
};

const handleTreeSelect = (keys) => {
  selectedKeys.value = keys;
  const key = Array.isArray(keys) ? keys[0] : null;
  if (key) {
    store.setSelectedTreeNodeKey(key);
    // 工业图谱模式:请求在中间图中定位并高亮对应节点
    if (store.graphViewMode === 'kg3d') {
      store.requestKg3dAction('selectByPath', { path: key });
    }
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
        treeData.value = [convertToTreeNode(root)];
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
    prefix: () => h(
      NIcon,
      { size: 16 },
      { default: () => h(node.is_dir ? BookListIcon : DocumentIcon) }
    ),
    children: kids
  };
}

onMounted(async () => {
  await store.fetchKnowledgeBases(); // 会自动选中第一个

  // 如果已有选中项，加载其树数据
  if (selectedKnowledgeBase.value) {
    const root = await invoke("get_knowledge_base_tree", {
      rootPath: selectedKnowledgeBase.value.location
    });
    treeData.value = [convertToTreeNode(root)];
    expandedKeys.value = [treeData.value[0].key];
    selectedKeys.value = [treeData.value[0].key];
    store.setSelectedTreeNodeKey(treeData.value[0].key);
    logger.info('[BrainKnowledgeBaseList] 初始化知识库文件树', {
      knowledgeBase: selectedKnowledgeBase.value.knowledge_base_name,
      rootPath: selectedKnowledgeBase.value.location,
      rootKey: treeData.value[0].key
    });
  }
});

onUnmounted(() => {
  abortController.abort();
  treeData.value = [];
  expandedKeys.value = [];
  selectedKeys.value = [];
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
  overflow: auto;
  padding-left: 8px;
  overscroll-behavior: contain;
}

.knowledge-base-list {
  padding: 0px;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
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
  width: 6px;
  height: 6px;
}

.kb-list::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
  transition: background-color 0.3s;
}

.kb-list::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.4);
}

.kb-list::-webkit-scrollbar-track {
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 3px;
}

/* Firefox滚动条样式 */
.kb-list {
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.2) rgba(0, 0, 0, 0.05);
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

/* 选中状态:仅在左侧加细的强调线,不给整条(含展开的树)铺背景,避免影响阅读 */
.kb-list-item.active {
  border-left: 3px solid #1890ff;
}

/* 蓝色底只罩标题行 */
.kb-list-item.active .kb-item {
  background: rgba(24, 144, 255, 0.12);
  border-radius: 6px;
}

/* 选中状态下的文字和图标 */
.kb-list-item.active .kb-title,
.kb-list-item.active .kb-icon,
.kb-list-item.active .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

/* 暗色模式:更亮的蓝字 + 标题行略深的底(兼容 .dark / .dark-mode 两种根类) */
:is(.dark, .dark-mode) .kb-list-item.active {
  border-left-color: #58a6ff;
}
:is(.dark, .dark-mode) .kb-list-item.active .kb-item {
  background: rgba(56, 139, 253, 0.18);
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
  height: 100%;
  margin: 8px 0 12px 16px;
  border-left: 1px dashed var(--n-border-color);
  padding-left: 8px;
}

.n-tree-node {
  padding: 4px 0;
}

.n-tree-node-content {
  padding: 4px 0;
}
</style>
