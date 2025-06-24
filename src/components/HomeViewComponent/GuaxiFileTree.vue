<template>
  <div class="file-tree-container">
    <!-- 工具栏 -->
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title-header">知识库编辑</n-text>
      </div>
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-icon-wrapper :size="18" :border-radius="6" class="refresh-icon" @click="refreshAll">
            <n-icon :component="RefreshIcon" size="16" />
          </n-icon-wrapper>
        </template>
        刷新所有目录
      </n-tooltip>
    </div>

    <n-scrollbar class="repo-scroll-container">
      <n-list bordered class="repo-list">
        <n-list-item v-for="path in fileTreeStore.repopathlist" :key="path"
          :class="{ active: path === fileTreeStore.activeRepoPath }" class="repo-list-item">
          <div class="repo-item" @click="toggleRepo(path)">
            <div class="repo-title">
              <n-icon size="15" class="repo-icon">
                <FolderIcon />
              </n-icon>
              {{ getRepoName(path) }}
            </div>
            <div class="repo-actions">
              <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFile(path)">
                <n-icon :component="FileAddIcon" size="14" />
              </n-icon-wrapper>
              <n-icon-wrapper :size="20" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(path)">
                <n-icon :component="FolderAddIcon" size="14" />
              </n-icon-wrapper>
              <n-icon size="15" class="expand-icon">
                <chevron-down-icon v-if="fileTreeStore.isRepoExpanded(path) && path === fileTreeStore.activeRepoPath" />
                <chevron-forward-icon v-else />
              </n-icon>
            </div>
          </div>

          <!-- 将n-collapse-transition移到n-list-item内部，但在repo-item外部 -->
          <n-collapse-transition>
            <div v-if="path === fileTreeStore.activeRepoPath && fileTreeStore.isRepoExpanded(path)"
              class="tree-container">
              <n-tree ref="treeRef" :data="fileTree" key-field="id" :render-label="renderLabel"
                :render-prefix="renderPrefix" :expand-on-click="true" :show-line="true"
                @update:selected-keys="handleSelect" :node-props="nodeProps" virtual-scroll
                :virtual-scroll-item-height="28" class="tree-component" />
            </div>
          </n-collapse-transition>
        </n-list-item>
      </n-list>
    </n-scrollbar>

    <!-- 右键菜单 -->
    <n-dropdown v-model:show="isContextMenuVisible" :options="contextMenuOptions" placement="bottom-start"
      :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleContextMenuSelect"
      @clickoutside="handleClickoutside" />
  </div>
</template>

<script setup>
import { NTree, NIcon, useMessage } from 'naive-ui'
import { DocumentTextOutline, FolderOutline, AddOutline, TrashOutline, CreateOutline, MoveOutline, FolderOpenOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { h, ref, onMounted, onUnmounted, watch, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { NList, NListItem, NCollapseTransition, NText, NTooltip, NIconWrapper } from 'naive-ui'
import { RefreshOutline as RefreshIcon, ChevronDownOutline as ChevronDownIcon, ChevronForwardOutline as ChevronForwardIcon } from '@vicons/ionicons5'
import { Folder20Regular as FolderIcon, ChannelAdd20Regular as FileAddIcon, FolderAdd20Regular as FolderAddIcon } from '@vicons/fluent'


const message = useMessage();

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const lastSelectedKey = ref(null);

// 文件树数据
const fileTree = ref([])

// 右键菜单状态
const isContextMenuVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const selectedNode = ref(null) // 当前右键点击的节点

// 已展开的目录
const expandedKeys = ref([])
const unlistenRef = ref(null)

const selectedKeys = ref([]); // 新增：存储当前选中的节点keys

// 右键菜单选项
const contextMenuOptions = ref([
  {
    label: '新建文件',
    key: 'addFile',
    icon: () => h(NIcon, null, { default: () => h(AddOutline) })
  },
  {
    label: '新建文件夹',
    key: 'addFolder',
    icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) })
  },
  {
    label: '重命名',
    key: 'rename',
    icon: () => h(NIcon, null, { default: () => h(CreateOutline) })
  },
  {
    label: '删除',
    key: 'delete',
    icon: () => h(NIcon, null, { default: () => h(TrashOutline) })
  },
  {
    label: '移动到',
    key: 'move',
    icon: () => h(NIcon, null, { default: () => h(MoveOutline) })
  },
])


// 修改后的handleAddFile函数
const handleAddFile = async (repoPath) => {
  try {
    let parentPath = repoPath; // 默认使用仓库路径

    // 如果有选中的节点，获取其路径
    if (selectedKeys.value.length > 0) {
      const selectedKey = selectedKeys.value[0];
      const selectedNode = findNodeById(fileTree.value, selectedKey);

      // 如果选中的是目录，则使用该目录路径
      if (selectedNode?.is_directory) {
        parentPath = selectedNode.id;
      }
    }

    await addNode(parentPath, false);
  } catch (error) {
    message.error(`添加文件失败: ${error.message}`);
  }
};

// 修改后的handleAddFolder函数
const handleAddFolder = async (repoPath) => {
  try {
    let parentPath = repoPath; // 默认使用仓库路径

    // 如果有选中的节点，获取其路径
    if (selectedKeys.value.length > 0) {
      const selectedKey = selectedKeys.value[0];
      const selectedNode = findNodeById(fileTree.value, selectedKey);

      // 如果选中的是目录，则使用该目录路径
      if (selectedNode?.is_directory) {
        parentPath = selectedNode.id;
      }
    }

    await addNode(parentPath, true);
  } catch (error) {
    message.error(`添加文件夹失败: ${error.message}`);
  }
};

// 获取仓库显示名称
const getRepoName = (path) => {
  const name = path.split(/[\\/]/).pop() || path
  console.log('[Util] 获取仓库名称:', { path, name })
  return name
}

const toggleRepo = async (path) => {
  console.log('目标路径:', path)
  console.log('当前激活路径:', fileTreeStore.activeRepoPath)

  if (fileTreeStore.activeRepoPath === path) {
    // 如果点击的是当前已激活的仓库，则切换展开状态
    fileTreeStore.toggleRepoExpand(path)
  } else {
    // 如果是新仓库，则设置激活并展开
    fileTreeStore.setActiveRepoPath(path)
    fileTreeStore.expandedRepos.set(path, true) // 确保新仓库展开
    await loadRepoTree(path)
  }
}

const loadRepoTree = async (path) => {
  console.log('[API] 加载仓库树')
  try {
    // 无论数据是否为空都重新加载
    await loadFileTree()
    // 确保仓库在加载后保持展开状态
    fileTreeStore.expandedRepos.set(path, true)
  } catch (error) {
    console.error('[API] 加载仓库树失败:', error)
  }
}

const refreshAll = async () => {
  console.log('[Action] 刷新所有仓库')
  for (const path of fileTreeStore.repopathlist) {
    console.log('[Refresh] 刷新仓库:', path)
    await loadRepoTree(path)
  }
}

const normalizeTreeIds = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];
  return tree.map(item => {
    const newItem = { ...item, id: normalizePath(item.id) };
    if (item.children) {
      newItem.children = normalizeTreeIds(item.children);
    }
    return newItem;
  });
};

// 加载文件树数据
const loadFileTree = async () => {
  console.log('[API] 加载文件树')
  try {
    fileTree.value = []
    console.log('当前仓库路径:', fileTreeStore.activeRepoPath)
    await fileTreeStore.fetchFileTree(fileTreeStore.activeRepoPath)
    // 先 normalize，再排序
    fileTree.value = [...sortFileTree(normalizeTreeIds(fileTreeStore.fileTree))]
  } catch (error) {
    console.error('获取文件树数据失败', error)
    fileTree.value = []
  }
}

// 对文件树数据进行排序（先文件后目录）
const sortFileTree = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];  // Add null check

  return tree.map(item => {
    if (item.children) {
      item.children = sortFileTree(item.children);
    }
    return item;
  }).sort((a, b) => {
    // Directories first, then files
    if (a.type === 'directory' && b.type !== 'directory') return -1;
    if (a.type !== 'directory' && b.type === 'directory') return 1;

    // Alphabetical sorting
    return a.name.localeCompare(b.name);
  });
};

// 渲染文件树图标
const renderPrefix = ({ option }) => {
  const icon = option.is_directory ? FolderOutline : DocumentTextOutline
  return h(NIcon, null, { default: () => h(icon) })
}

const handleSelect = async (keys) => {
  console.log('触发选中事件, keys 参数:', keys);
  selectedKeys.value = keys; // 更新选中的keys

  // 防抖处理
  if (lastSelectedKey.value === keys[0]) {
    console.log('忽略重复选中:', keys[0]);
    return;
  }
  lastSelectedKey.value = keys[0];

  const selectedKey = keys[0];
  if (!selectedKey) {
    console.warn('未选中任何节点');
    return;
  }

  console.log('查找选中节点, key:', selectedKey);
  const selectedNode = findNodeById(fileTree.value, selectedKey);
  if (!selectedNode) {
    console.warn('未找到选中节点');
    return;
  }

  console.log('找到选中节点:', selectedNode);

  if (!selectedNode.is_directory) {
    try {
      console.log('选中节点是文件，开始获取文件内容');

      // 检查是否已存在标签页
      const existingTab = fileTreeStore.openTabs.find(tab => tab.path === selectedNode.id);
      if (existingTab) {
        console.log('文件已在标签页中打开，激活标签页');
        fileTreeStore.activeTab = selectedNode.id;
        return;
      }

      // 获取文件内容
      const content = await fileTreeStore.fetchFileContent(selectedNode.id);
      if (content !== null) {
        console.log('文件内容获取成功，打开标签页');
        await fileTreeStore.openFileInTab(selectedNode.id, content);
      } else {
        console.error('文件内容获取失败');
      }
    } catch (error) {
      console.error('处理文件选中时出错:', error);
    }
  } else {
    try {
      console.log('选中节点是目录，开始加载子目录内容');
      await fileTreeStore.fetchFileTree(selectedNode.id);
      selectedNode.children = [...fileTreeStore.fileTree];
      console.log('子目录内容加载成功:', selectedNode.children);
      sortFileTree(selectedNode.children);
    } catch (error) {
      console.error('加载子目录内容失败:', error);
    }
  }
};

// 递归查找节点
const findNodeById = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  console.log('查找节点:', normalizedTargetId);

  // 深度优先搜索
  const dfs = (nodes) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);
      // console.log('检查节点:', nodePath);

      if (nodePath === normalizedTargetId) {
        return node;
      }

      if (node.children) {
        const found = dfs(node.children);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree);
};

// 处理文件树容器的右键点击事件
const treeRef = ref(null)
const treeContainer = ref(null)

const nodeProps = ({ option }) => ({
  // 处理左键点击
  onClick: (e) => {
    e.stopPropagation()
    console.log('节点左键点击:', option)
    handleSelect([option.id]) // 手动触发选中
  },
  // 处理右键点击
  onContextmenu: (e) => {
    e.preventDefault()
    e.stopPropagation()
    selectedNode.value = option
    isContextMenuVisible.value = true
    contextMenuPosition.value = { x: e.clientX, y: e.clientY }
    console.log('节点右键菜单触发:', option.name)
  }
})

// 修改容器右键处理
const handleTreeContextMenu = (e) => {
  // 阻止默认行为（包括Tauri调试菜单）
  e.preventDefault()
  e.stopPropagation()

  // 检查是否点击在空白区域
  const isBlankArea = !e.target.closest('.n-tree-node') &&
    !e.target.closest('.n-tree-node-content') &&
    treeContainer.value.contains(e.target)

  if (isBlankArea) {
    isContextMenuVisible.value = true
    contextMenuPosition.value = { x: e.clientX, y: e.clientY }
    selectedNode.value = null
    console.log('空白区域右键菜单触发')
  }
}

// 处理右键菜单选择
const handleContextMenuSelect = async (key) => {
  isContextMenuVisible.value = false;
  try {
    switch (key) {
      case 'addFile':
        console.log('添加文件操作, 选中节点:', selectedNode.value);
        await addNode(selectedNode.value?.id, false);
        break;
      case 'addFolder':
        await addNode(selectedNode.value?.id, true);
        break;
      case 'rename':
        if (selectedNode.value) {
          renameNode(selectedNode.value.id);
        }
        break;
      case 'delete':
        if (selectedNode.value) {
          deleteNode(selectedNode.value.id);
        }
        break;
      case 'move':
        if (selectedNode.value) {
          moveNode(selectedNode.value);
        }
        break;
    }
  } catch (error) {
    message.error(`操作失败: ${error.message}`);
  }
};

// 处理点击外部关闭菜单
const handleClickoutside = () => {
  isContextMenuVisible.value = false
}

// 添加一个 ref 来存储当前正在编辑的节点
const editingNode = ref(null)
// 添加一个 ref 来存储编辑中的名称
const editName = ref('')

// 修改后的 addNode 函数
const addNode = async (parentId, isDirectory = false) => {
  parentId = normalizePath(parentId);
  console.log('添加节点，父节点 ID:', parentId, '是否目录:', isDirectory);

  let parentNode;
  let isRoot = false;
  if (!parentId || parentId === normalizePath(fileTreeStore.activeRepoPath)) {
    // 如果没有parentId，使用当前激活的仓库路径作为根目录
    parentNode = {
      id: normalizePath(fileTreeStore.activeRepoPath),
      is_directory: true,
      children: fileTree.value
    };
    isRoot = true;
  } else {
    parentNode = findNodeById(fileTree.value, parentId);
    if (!parentNode) {
      // 如果找不到父节点，可能是根目录
      parentNode = {
        id: normalizePath(fileTreeStore.activeRepoPath),
        is_directory: true,
        children: fileTree.value
      };
      isRoot = true;
      console.warn('未找到父节点，使用根目录作为父节点');
    }
  }

  // 2. 创建临时名称和路径
  const tempName = isDirectory ? '新建文件夹' : '新建文件';
  const tempId = `${normalizePath(parentNode.id)}/${tempName}`;

  // 3. 创建临时节点
  const newNode = {
    id: tempId,
    name: tempName,
    is_directory: isDirectory,
    children: isDirectory ? [] : undefined,
    isNew: true // 标记为新节点
  };

  if (isRoot) {
    // 直接操作 fileTree.value
    fileTree.value = [...fileTree.value, newNode];
  } else {
    parentNode.children = [...(parentNode.children || []), newNode];
    parentNode.children.sort((a, b) =>
      a.is_directory === b.is_directory ? 0 : a.is_directory ? -1 : 1
    );
  }

  // ★ 自动展开父节点，确保新节点可见
  if (!expandedKeys.value.includes(parentNode.id)) {
    expandedKeys.value.push(parentNode.id);
  }

  // 5. 进入编辑模式
  startEditing(newNode);

  selectedKeys.value = [];

  // 6. 返回Promise，在编辑完成后处理
  return new Promise((resolve) => {
    const unwatch = watch(editingNode, (newVal) => {
      if (!newVal) {
        unwatch();
        handleEditComplete(newNode, parentNode, isDirectory, resolve);
      }
    });
  });
};

// 处理编辑完成的函数
const handleEditComplete = async (node, parentNode, isDirectory, resolve) => {
  // 如果名称未改变或为空，删除临时节点
  if (node.name === '新建文件夹' || node.name === '新建文件' || !node.name.trim()) {
    parentNode.children = parentNode.children.filter(n => n.id !== node.id);
    resolve(null);
    return;
  }

  try {
    // 创建实际文件/目录
    if (isDirectory) {
      await invoke('create_directory', { path: node.id });
    } else {
      await invoke('create_file_with_content', {
        path: node.id,
        content: '' // 初始为空内容
      });
    }

    // 移除临时标记
    delete node.isNew;
    resolve(node);
  } catch (error) {
    console.error('创建失败:', error);
    // 回滚：删除临时节点
    parentNode.children = parentNode.children.filter(n => n.id !== node.id);
    message.error(`创建失败: ${error.message}`);
    resolve(null);
  } finally {
    // 重置选中路径
    selectedKeys.value = [];
  }
};

// 修改后的 handleBlur 函数
const handleBlur = async () => {
  if (!editingNode.value) return;

  const node = editingNode.value;
  const oldId = node.id;
  const parentNode = findParentNode(fileTree.value, oldId);
  const parentPath = parentNode ? parentNode.id : fileTreeStore.activeRepoPath;

  if (!editName.value.trim()) {
    // 名称为空，如果是新节点则删除
    if (node.isNew) {
      parentNode.children = parentNode.children.filter(n => n.id !== oldId);
    }
    editingNode.value = null;
    editName.value = '';
    return;
  }

  // 构建新路径
  const newId = `${parentPath}/${editName.value}`;

  // 更新节点信息
  node.name = editName.value;
  node.id = newId;

  // 如果是新节点，结束编辑（实际文件会在handleEditComplete中创建）
  if (node.isNew) {
    editingNode.value = null;
    editName.value = '';
    return;
  }

  // 以下是原有重命名逻辑
  try {
    let oldContent = '';
    if (!node.is_directory) {
      try {
        oldContent = await fileTreeStore.fetchFileContent(oldId);
      } catch (error) {
        console.error('获取旧文件内容失败:', error);
      }
    }

    if (node.is_directory) {
      await invoke('rename_directory', { oldPath: oldId, newPath: newId });
    } else {
      await invoke('create_file_with_content', {
        path: newId,
        content: oldContent
      });
      await invoke('delete_path', { path: oldId });
    }

    // 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === oldId);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = newId;
      fileTreeStore.openTabs[tabIndex].fileName = editName.value;
      if (fileTreeStore.activeTab === oldId) {
        fileTreeStore.activeTab = newId;
      }
    }
  } catch (error) {
    console.error('重命名操作失败:', error);
    node.name = oldId.split(/[\\/]/).pop();
    node.id = oldId;
    message.error(`重命名失败: ${error.message}`);
  } finally {
    editingNode.value = null;
    editName.value = '';
    await refreshDirectory(parentPath);
  }
};

// 删除文件或目录
const deleteNode = async (nodeId) => {
  const node = findNodeById(fileTree.value, nodeId)
  if (node) {
    if (confirm(`确定要删除 ${node.name} 吗？`)) {
      const parentNode = findParentNode(fileTree.value, nodeId)
      if (parentNode) {
        parentNode.children = parentNode.children.filter((child) => child.id !== nodeId)
        // 调用 Tauri 后端 API 删除文件或目录
        await invoke('delete_path', { path: nodeId })
        await refreshDirectory(nodeId);
      } else {
        await invoke('delete_path', { path: nodeId })
        await refreshDirectory(fileTreeStore.activeRepoPath);
      }
    }
  }
}

// 重命名文件或目录
const renameNode = async (nodeId) => {
  const node = findNodeById(fileTree.value, nodeId)
  if (node) {
    // 设置节点为编辑状态
    startEditing(node)
  }
}

// 修改后的 findParentNode 函数
const findParentNode = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  console.log('查找父节点:', normalizedTargetId);

  // 深度优先搜索
  const dfs = (nodes, parent) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);

      if (nodePath === normalizedTargetId) {
        return parent;
      }

      if (node.children) {
        const found = dfs(node.children, node);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree, null);
};

// 开始编辑节点
const startEditing = (node) => {
  console.log('startEditing 开始编辑节点:', node.name);
  editingNode.value = node
  editName.value = node.name
  console.log('startEditing ==> ', node);
}

const renderLabel = ({ option }) => {
  // 用 id 判断是否为正在编辑的节点
  if (editingNode.value && normalizePath(editingNode.value.id) === normalizePath(option.id)) {
    console.log('EEE ======> renderLabel editingNode.value:', editingNode.value);
    return h('input', {
      class: 'editable-input',
      value: editName.value,
      onInput: (e) => (editName.value = e.target.value),
      onBlur: handleBlur,
      onKeydown: handleKeydown,
    })
  } else {
    // console.log('======> renderLabel option:', option);
    // console.log('======> renderLabel editingNode.value:', editingNode.value);
    return option.name
  }
}

watch(editingNode, (val) => {
  if (val) {
    nextTick(() => {
      setTimeout(() => {
        const input = document.querySelector('.editable-input')
        if (input) {
          input.focus()
          input.select()
        }
      }, 50) // 延迟，确保 input 已渲染
    })
  }
})

// 处理输入框的 keydown 事件
const handleKeydown = (e) => {
  console.log('handleKeydown triggered'); // 添加日志
  if (e.key === 'Enter') {
    e.target.blur(); // 触发 input 失焦事件
  } else if (e.key === 'Escape') {
    // 取消编辑
    editingNode.value = null;
    editName.value = '';
  }
};

const moveNode = async (node) => {
  const targetDirectoryPath = await selectTargetPath();

  if (targetDirectoryPath) {
    try {
      const fileName = node.name; // 获取文件名
      const destinationPath = `${normalizePath(targetDirectoryPath)}/${fileName}`; // 构建目标路径

      console.log('源路径:', node.id);
      console.log('目标路径:', destinationPath);

      await invoke('move_path', { sourcePath: normalizePath(node.id), destinationPath });

      // 更新文件树
      const parentNode = findParentNode(fileTree.value, node.id);
      if (parentNode) {
        parentNode.children = parentNode.children.filter((child) => child.id !== node.id);
      }

      const targetNode = findNodeById(fileTree.value, normalizePath(targetDirectoryPath));
      if (targetNode && targetNode.is_directory) {
        targetNode.children.push(node);
        node.id = destinationPath; // 更新节点路径
      }

      // 刷新源目录和目标目录
      const sourceDirectoryId = findParentNode(fileTree.value, node.id)?.id;
      console.log('源节点 ID:', node.id, '源目录 ID:', fileTree.value);
      if (sourceDirectoryId) {
        await refreshDirectory(sourceDirectoryId);
      }

      if (targetDirectoryPath) {
        await refreshDirectory(targetDirectoryPath);
      }
      console.log('移动成功');
    } catch (error) {
      console.error('移动失败:', error);
    }
  }
};

const selectTargetPath = async () => {
  const result = await open({
    directory: true,
    multiple: false,
    title: '选择目标路径',
  });

  if (result) {
    return result;
  } else {
    return null;
  }
};

const refreshDirectory = async (directoryId) => {
  console.log('刷新目录:', directoryId);

  // 如果是根目录，刷新整个文件树
  if (!directoryId || directoryId === fileTreeStore.activeRepoPath) {
    await loadFileTree();
    return;
  }

  // 查找要刷新的目录节点
  const directoryNode = findNodeById(fileTree.value, directoryId);
  if (directoryNode && directoryNode.is_directory) {
    try {
      // 保存当前展开状态
      const wasExpanded = expandedKeys.value.includes(directoryId);

      // 重新加载目录内容
      await fileTreeStore.fetchFileTree(directoryId);

      // 保留原有节点的展开状态和内容
      const newChildren = fileTreeStore.fileTree.map(newNode => {
        const existingNode = findNodeById([directoryNode], newNode.id);
        return existingNode || newNode;
      });

      directoryNode.children = [...newChildren];

      // 恢复展开状态
      if (wasExpanded && !expandedKeys.value.includes(directoryId)) {
        expandedKeys.value.push(directoryId);
      }

      console.log('目录局部刷新成功:', directoryNode);
    } catch (error) {
      console.error('目录刷新失败:', error);
    }
  }
};

// 更健壮的路径规范化函数
const normalizePath = (path) => {
  if (!path) return '';

  // 替换所有反斜杠为正斜杠
  let normalized = path.replace(/\\/g, '/');

  // 处理Windows盘符 (如 C:/)
  if (normalized.match(/^[a-zA-Z]:\//)) {
    normalized = normalized[0].toUpperCase() + normalized.slice(1);
  }

  // 移除末尾斜杠（目录除外）
  if (normalized.endsWith('/') && normalized.length > 1) {
    normalized = normalized.slice(0, -1);
  }

  return normalized;
};

// 处理文件创建事件
const handleFileCreated = async (filePath) => {
  const parentPath = filePath.split('/').slice(0, -1).join('/')
  await refreshDirectory(parentPath || fileTreeStore.activeRepoPath)
}

// 处理文件删除事件
// const handleFileRemoved = async (filePath) => {
//   const parentPath = filePath.split('/').slice(0, -1).join('/')
//   await refreshDirectory(parentPath || fileTreeStore.activeRepoPath)
// }

// 处理文件重命名事件
const handleFileRenamed = async (oldPath, newPath) => {
  if (!newPath) return;

  try {
    const normalizedNewPath = normalizePath(newPath);
    const normalizedOldPath = oldPath ? normalizePath(oldPath) : null;

    // 1. 更新树中的节点
    if (normalizedOldPath) {
      const nodeToUpdate = findNodeById(fileTree.value, normalizedOldPath);
      if (nodeToUpdate) {
        // 保留原有内容
        const content = nodeToUpdate.content;
        nodeToUpdate.id = normalizedNewPath;
        nodeToUpdate.name = newPath.split(/[\\/]/).pop();
        nodeToUpdate.content = content; // 保留内容
      }
    }

    // 2. 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === normalizedOldPath);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = normalizedNewPath;
      fileTreeStore.openTabs[tabIndex].fileName = newPath.split(/[\\/]/).pop();
      fileTreeStore.openTabs[tabIndex].content = fileTreeStore.openTabs[tabIndex].content; // 保留内容

      if (fileTreeStore.activeTab === normalizedOldPath) {
        fileTreeStore.activeTab = normalizedNewPath;
      }
    }

    // 3. 刷新父目录
    const parentPath = normalizedOldPath
      ? normalizedOldPath.split('/').slice(0, -1).join('/')
      : normalizedNewPath.split('/').slice(0, -1).join('/');

    await refreshDirectory(parentPath || fileTreeStore.activeRepoPath);
  } catch (error) {
    console.error('处理文件重命名时出错:', error);
  }
};

// 文件监控函数
const setupFileWatcher = async () => {
  try {
    console.log('开始监控目录:', fileTreeStore.activeRepoPath)

    // 启动后端监控
    await invoke('start_file_watching', {
      path: fileTreeStore.activeRepoPath
    })

    // 监听文件变化事件
    const unlisten = await listen('file_change', (event) => {
      const payload = event.payload
      console.log('文件系统变化事件:', payload)

      switch (payload.kind) {
        case 'Create':
          console.log('文件创建事件:', payload.path)
          handleFileCreated(payload.path)
          break
        case 'Remove':
          console.log('文件删除事件:', payload.path)
          handleFileRemoved(payload.path)
          break
        case 'Rename':
          // 处理old_path可能为null的情况
          const oldPath = payload.old_path || extractOldPathFromEvent(payload)
          console.log('文件重命名事件:', oldPath, '->', payload.path)
          handleFileRenamed(oldPath, payload.path)
          break
      }
    })

    console.log('文件监控已启动')
  } catch (error) {
    console.error('启动文件监控失败:', error)
  }
}

// 辅助函数：从事件中提取旧路径（如果需要）
const extractOldPathFromEvent = (event) => {
  // 根据实际情况实现从事件中提取旧路径的逻辑
  return null // 如果无法提取则返回null
}

// 优化处理文件删除事件的函数
const handleFileRemoved = async (filePath) => {
  const normalizedPath = normalizePath(filePath)
  const parentPath = normalizedPath.split('/').slice(0, -1).join('/')

  console.log('处理删除事件 - 文件路径:', normalizedPath, '父目录:', parentPath)

  // 查找并删除节点
  const nodeToRemove = findNodeById(fileTree.value, normalizedPath)
  if (nodeToRemove) {
    const parentNode = findParentNode(fileTree.value, normalizedPath)
    if (parentNode) {
      parentNode.children = parentNode.children.filter(child => child.id !== normalizedPath)
      console.log('从父节点删除成功:', parentNode)
    } else {
      // 如果是根节点下的文件
      fileTree.value = fileTree.value.filter(child => child.id !== normalizedPath)
      console.log('从根节点删除成功')
    }
  }

  // 刷新父目录
  await refreshDirectory(parentPath || fileTreeStore.activeRepoPath)
}

// 修改后的清理函数
const cleanupFileWatcher = () => {
  if (unlistenRef.value) {
    // 使用Promise链式调用处理异步操作
    invoke('stop_file_watching')
      .then(() => {
        unlistenRef.value()
        unlistenRef.value = null
        console.log('文件监控已停止')
      })
      .catch(error => {
        console.error('停止文件监控失败:', error)
      })
  }
}

// 新增初始化函数
const initializeRepositories = async () => {
  console.log('[Init] 开始初始化知识库')

  try {
    // 1. 从知识库商店加载知识库列表
    const knowledgeBases = await knowledgeBaseStore.fetchKnowledgeBases()
    console.log('[Load] 获取知识库列表:', knowledgeBases)

    // 2. 同步路径到filetreestore
    const paths = knowledgeBases.map(kb => kb.location)
    fileTreeStore.setRepopathlist(paths)

    // 3. 设置默认激活路径（优先保持当前选中，否则选第一个）
    const currentActive = fileTreeStore.activeRepoPath
    const defaultActive = paths.length > 0 ? paths[0] : null

    if (!currentActive && defaultActive) {
      fileTreeStore.setActiveRepoPath(defaultActive)
      console.log('[Set] 设置默认激活路径:', defaultActive)
    } else if (currentActive && !paths.includes(currentActive)) {
      // 当前选中路径不在新列表中时，重置为第一个
      fileTreeStore.setActiveRepoPath(defaultActive)
      console.warn('[Warn] 重置无效激活路径:', currentActive, '→', defaultActive)
    }

    return paths
  } catch (error) {
    console.error('[Error] 初始化知识库失败:', error)
    throw error
  }
}

// 使用单独的函数处理右键事件
const handleContextMenu = (e) => {
  if (e.target.closest('.n-tree')) {
    handleTreeContextMenu(e)
  }
}

onBeforeUnmount(() => {
  // 清理所有状态
  editingNode.value = null
  editName.value = ''
  selectedNode.value = null
  expandedKeys.value = []
  lastSelectedKey.value = null

  // 清理文件监控
  cleanupFileWatcher()
})

// 初始化时获取文件树数据
onMounted(async () => {
  // 1. 初始化知识库
  await initializeRepositories()

  // 2. 加载当前激活仓库的文件树
  if (fileTreeStore.activeRepoPath) {
    await loadRepoTree(fileTreeStore.activeRepoPath)
  }

  // 添加文件监控
  setupFileWatcher()
})

// 在组件卸载时清理事件监听器
onUnmounted(() => {
  // 清理文件监控
  cleanupFileWatcher()
})

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

/* 新增的标题栏样式 */
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
  height: auto;
  /* 确保高度自适应 */
}

.repo-title-header {
  font-size: 18px !important;
  /* 使用!important确保覆盖其他样式 */
  font-weight: 600;
  color: #191b1b;
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  margin: 0;
  /* 移除可能影响大小的外边距 */
  padding: 0;
  /* 移除可能影响大小的内边距 */
  line-height: 1.2;
  /* 设置合适的行高 */
}

.repo-title {
  font-size: 15px !important;
  /* 使用!important确保覆盖其他样式 */
  font-weight: 600;
  color: #191b1b;
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  margin: 0;
  /* 移除可能影响大小的外边距 */
  padding: 0;
  /* 移除可能影响大小的内边距 */
  line-height: 1.2;
  /* 设置合适的行高 */
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

.file-tree-container {
  height: calc(100% - var(--scrollbar-height, 42px));
  display: flex;
  flex-direction: column;
  background-color: var(--n-color-embedded);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.toolbar {
  padding: 12px 16px;
  background-color: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(5px);
  border-bottom: 1px solid var(--n-border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  font-size: 16px;
  font-weight: 600;
  color: var(--n-text-color);
}

.repo-scroll-container {
  flex: 1;
  height: 0;
  background-color: #ffffff;
}

.repo-list {
  border-radius: 0;
  background-color: transparent;
}

.repo-list-item {
  padding: 8px 12px;
  transition: all 0.2s ease;
  border-radius: 6px;
  border-bottom: 1px solid var(--n-border-color);
  position: relative;
  margin: 2px 0;
  overflow: hidden;
}

.repo-list-item:last-child {
  border-bottom: none;
}

/* 鼠标悬停效果 - 添加发光边框和背景 */
.repo-list-item:hover {
  background-color: rgba(24, 144, 255, 0.05);
  box-shadow: inset 0 0 0 1px rgba(24, 144, 255, 0.2);
}

/* 鼠标悬停时的文字和图标效果 */
.repo-list-item:hover .repo-title {
  color: #1890ff;
}

.repo-list-item:hover .repo-icon {
  color: #1890ff;
}

.repo-list-item:hover .expand-icon {
  color: #1890ff;
}

/* 选中状态效果*/
.repo-list-item.active {
  background-color: rgba(24, 144, 255, 0.16); /* 比原来更深一点 */
  border-left: 3px solid #1890ff;
  box-shadow: inset 0 0 8px rgba(24, 144, 255, 0.13);
}

/* 暗色模式下选中状态更明显 */
.dark-mode .repo-list-item.active {
  background-color: rgba(24, 144, 255, 0.22);
  border-left: 3px solid #8ab4f8;
  box-shadow: inset 0 0 8px rgba(138, 180, 248, 0.18);
}

/* 选中状态下的文字和图标 */
.repo-list-item.active .repo-title,
.repo-list-item.active .repo-icon,
.repo-list-item.active .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

.repo-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  cursor: pointer;
  padding: 4px 0;
}

.repo-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: var(--n-text-color);
  transition: color 0.2s ease;
}

.repo-icon {
  color: var(--n-color-primary);
  margin-right: 8px;
  transition: color 0.2s ease;
}

.expand-icon {
  transition: all 0.2s ease;
  color: var(--n-text-color);
}

.expand-icon.rotated {
  transform: rotate(90deg);
}

.tree-container {
  position: relative;
  width: 100%;
  padding: 8px 0 8px 24px;
  border-left: 1px dashed var(--n-border-color);
}

.tree-component {
  width: 100%;
}

/* 节点悬停效果 */
.n-tree-node:hover {
  background: rgba(0, 0, 0, 0.05);
}

/* 编辑输入框样式 */
.editable-input {
  border: 1px solid var(--n-border-color);
  border-radius: 4px;
  padding: 2px 4px;
  font-size: 14px;
  outline: none;
  width: 100%;
}

.editable-input:focus {
  border-color: var(--n-color-primary);
  box-shadow: 0 0 0 2px var(--n-color-primary-hover);
}

/* 刷新按钮动画 */
.refresh-btn {
  transition: transform 0.3s ease;
}

.refresh-btn:hover {
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .file-tree-container {
  background-color: var(--n-color-embedded);
}

.dark-mode .toolbar {
  background-color: rgba(30, 30, 30, 0.7) !important;
  border-bottom: 1px solid #333 !important;
}

.dark-mode .repo-scroll-container {
  background-color: #1a1a1a;
}

/* 选中状态下的 action/refresh 图标 */
.repo-list-item.active .action-icon,
.repo-list-item.active .refresh-icon {
  color: var(--n-color-primary, #1890ff);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 按下状态下的 action/refresh 图标 */
.action-icon:active,
.refresh-icon:active {
  color: var(--n-color-primary-hover, #1765ad);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 暗色模式下 action/refresh 图标 */
.dark-mode .action-icon,
.dark-mode .refresh-icon {
  color: #444a57;
  background-color: transparent;
}

.dark-mode .action-icon:hover,
.dark-mode .action-icon.active,
.dark-mode .refresh-icon:hover,
.dark-mode .refresh-icon.active,
.dark-mode .repo-list-item.active .action-icon,
.dark-mode .repo-list-item.active .refresh-icon {
  color: #8ab4f8;
  background-color: rgba(65, 105, 225, 0.13);
}

/* 新增的样式 */
.repo-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.repo-list-item:hover .repo-actions {
  opacity: 1;
}

/* Action/refresh 图标基础样式（合并） */
.action-icon,
.refresh-icon {
  color: var(--n-text-color-disabled, #bfbfbf);
  /* 平时较淡，与背景接近 */
  background-color: transparent;
  transition: color 0.2s, background-color 0.2s;
  border-radius: 4px;
  padding: 2px;
}

.action-icon:hover,
.action-icon.active,
.refresh-icon:hover,
.refresh-icon.active {
  color: var(--n-color-primary, #1890ff);
  /* 选中时主色 */
  background-color: var(--n-color-primary-suppl, #e6f7ff);
  /* 选中时淡主色背景 */
}
</style>