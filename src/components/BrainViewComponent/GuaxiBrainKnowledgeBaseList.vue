<template>
  <div class="knowledge-base-list">
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title">第二大脑</n-text>
      </div>
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button type="primary" size="small" circle @click="refreshList" class="refresh-btn">
            <template #icon>
              <n-icon :component="RefreshIcon" />
            </template>
          </n-button>
        </template>
        刷新知识库列表
      </n-tooltip>
    </div>

    <n-list bordered class="kb-list">
      <n-list-item v-for="kb in knowledgeBases" :key="kb.id" :class="{ active: kb.id === selectedId }"
        class="kb-list-item">
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

        <n-collapse-transition>
          <div v-if="kb.id === selectedId && expanded" class="tree-container">
            <n-tree :key="`tree-${selectedId}`" :data="treeData" :block-line="true" :expand-on-click="true"
              :expanded-keys="expandedKeys" class="kb-tree" @update:expanded-keys="handleTreeExpand" />
          </div>
        </n-collapse-transition>
      </n-list-item>
    </n-list>
  </div>
</template>

<script setup>
import { ref, onMounted, h, shallowRef, onUnmounted } from 'vue';
import { storeToRefs } from 'pinia';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { invoke } from '@tauri-apps/api/core';
import { useMessage } from 'naive-ui';
import {
  NList, NListItem, NTree, NTooltip, NButton, NIcon, NTag, NCollapseTransition
} from 'naive-ui';
import {
  BookOutline as BookIcon,
  RefreshOutline as RefreshIcon,
  ChevronDownOutline as ChevronDownIcon,
  ChevronForwardOutline as ChevronForwardIcon,
  DocumentTextOutline as DocumentIcon,
  FolderOutline as FolderIcon,
} from '@vicons/ionicons5';


const store = useKnowledgeBaseStore();
const message = useMessage();
const { knowledgeBases } = storeToRefs(store);

// 使用 shallowRef 替代 ref 避免不必要的深度响应式
const selectedId = ref(null);
const treeData = shallowRef([]);
const expanded = ref(false);
const expandedKeys = ref([]);
let abortController = new AbortController();

// 确保在切换知识库时正确处理树形数据
const toggleKnowledgeBase = async (kb) => {
  try {
    // 取消之前的请求
    abortController.abort();
    abortController = new AbortController();

    if (selectedId.value === kb.id) {
      // 同一个知识库，切换展开状态
      expanded.value = !expanded.value;
      if (!expanded.value) {
        expandedKeys.value = [];
      }
    } else {
      // 不同的知识库，重置状态
      expanded.value = true;
      selectedId.value = kb.id;
      treeData.value = []; // 先清空数据

      // 异步加载树形数据
      const root = await invoke("get_knowledge_base_tree",
        { rootPath: kb.location },
        { signal: abortController.signal }
      );

      const newTreeData = [convertToTreeNode(root)];

      // 使用 shallowRef 的 .value 赋值确保响应式更新
      treeData.value = newTreeData;
      expandedKeys.value = [newTreeData[0].key];
    }
  } catch (error) {
    if (error.name !== 'AbortError') {
      console.error("切换知识库失败:", error);
      message.error("加载知识库内容失败");
    }
  }
};

// 处理树节点展开状态变化
const handleTreeExpand = (keys) => {
  expandedKeys.value = keys;
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
    console.error("刷新知识库列表失败:", error);
    message.error("刷新知识库列表失败");
  }
};

// 转换树形数据结构
function convertToTreeNode(node) {
  return {
    label: node.name,
    key: node.path,
    isLeaf: !node.is_dir,
    prefix: () => h(
      NIcon,
      { size: 16 },
      { default: () => h(node.is_dir ? FolderIcon : DocumentIcon) }
    ),
    children: node.children ? node.children.map(convertToTreeNode) : undefined
  };
}

onMounted(() => {
  store.fetchKnowledgeBases();
});

onUnmounted(() => {
  // 组件卸载时取消所有请求
  abortController.abort();
  selectedId.value = null;
  treeData.value = [];
});
</script>

<style scoped>
/* 添加 tree-container 确保树形结构有稳定的容器 */
.tree-container {
  width: 100%;
  overflow: hidden;
  min-height: 20px;
  /* 确保有最小高度 */
  padding-left: 8px;
}

.knowledge-base-list {
  padding: 0px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  margin-bottom: 8px;
  background-color: #ffffff;
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
  color: #191b1b;
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
  background-color: #191b1b;
  margin-right: 8px;
  border-radius: 2px;
}

/* 刷新按钮样式调整 */
.refresh-btn {
  background-color: #f0f0f0;
  color: #5f6368;
  border: none;
  transition: all 0.3s ease;
}

.refresh-btn:hover {
  background-color: #e0e0e0;
  color: #191b1b;
  transform: rotate(90deg);
}

.kb-list {
  flex: 1;
  overflow-y: auto;
  border-radius: 8px;
  padding-left: 0;
  padding-right: 0;
}

.kb-list-item {
  padding-left: 12px;
  /* 从16px减少到12px */
  padding-right: 12px;
  /* 从16px减少到12px */
}


.kb-item {
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
  margin: 8px 0 12px 16px;
  border-left: 1px dashed var(--n-border-color);
  padding-left: 12px;
}

.n-tree-node {
  padding: 4px 0;
}

.n-tree-node-content {
  padding: 4px 0;
}
</style>