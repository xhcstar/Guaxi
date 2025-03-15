<template>
  <!-- 文件树 -->
  <n-tree :data="fileTree" key-field="id" :render-label="renderLabel" :render-prefix="renderPrefix"
    :expand-on-click="true" :show-line="true" @update:selected-keys="handleSelect" :node-props="RightClickMenu"
    :on-update:expanded-keys="expandedKeys" />

  <!-- 右键菜单 -->
  <n-dropdown v-model:show="isContextMenuVisible" :options="contextMenuOptions" placement="bottom-start"
    :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleContextMenuSelect"
    @clickoutside="handleClickoutside" />
</template>

<script setup>
import { NTree, NIcon } from 'naive-ui'
import { DocumentTextOutline, FolderOutline, AddOutline, TrashOutline, CreateOutline, MoveOutline, FolderOpenOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { h, ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()

// 文件树数据
const fileTree = ref([])

// 右键菜单状态
const isContextMenuVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const selectedNode = ref(null) // 当前右键点击的节点

// 已展开的目录
const expandedKeys = ref([])

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

// 初始化时获取文件树数据
onMounted(async () => {
  await loadFileTree()
})

// 加载文件树数据
const loadFileTree = async () => {
  // console.log('初始化：开始获取文件树数据')
  try {
    // await fileTreeStore.fetchFileTree('E:\\Record\\source\\ripgrep')
    await fileTreeStore.fetchFileTree('D:\\OpenSource\\ripgrep')
    // await fileTreeStore.fetchFileTree('D:\\Mine\\I\\Note')
    fileTree.value = fileTreeStore.fileTree
    // 对文件树数据进行排序
    fileTree.value = sortFileTree(fileTreeStore.fileTree)
    // console.log('初始化：文件树数据加载成功', fileTree.value)
  } catch (error) {
    console.error('初始化：获取文件树数据失败', error)
  }
}

// 对文件树数据进行排序（先文件后目录）
const sortFileTree = (tree) => {
  // 递归排序每个节点的 children
  const sortedTree = tree.map((node) => {
    if (node.children && node.children.length > 0) {
      // 对子节点进行排序
      node.children = sortFileTree(node.children)
    }
    return node
  })

  // 对当前层级的节点进行排序
  const result = sortedTree.sort((a, b) => {
    if (a.is_directory === b.is_directory) return 0 // 如果类型相同，保持原顺序
    return a.is_directory ? 1 : -1 // 目录排在后，文件排在前
  })

  // 输出排序后的结果
  // console.log('对文件树数据进行排序', result)
  return result
}

// 修改 renderLabel，支持编辑模式
const renderLabel = ({ option }) => {
  if (editingNode.value === option) {
    return h('input', {
      class: 'editable-input',
      value: editName.value,
      onInput: (e) => (editName.value = e.target.value),
      onBlur: handleBlur,
      onKeydown: handleKeydown,
    })
  } else {
    return option.name
  }
}

// 渲染文件树图标
const renderPrefix = ({ option }) => {
  const icon = option.is_directory ? FolderOutline : DocumentTextOutline
  return h(NIcon, null, { default: () => h(icon) })
}

// 处理文件选中事件
const handleSelect = async (keys) => {
  // console.log('文件选中事件触发，选中的 keys:', keys)
  const selectedKey = keys[0]
  if (selectedKey) {
    // console.log('查找选中节点，key:', selectedKey)
    const selectedNode = findNodeById(fileTree.value, selectedKey)
    if (selectedNode) {
      console.log('找到选中节点:', selectedNode)
      if (!selectedNode.is_directory) {
        // console.log('选中节点是文件，开始获取文件内容')
        fileTreeStore.fetchFileContent(selectedNode.id)
      } else {
        // console.log('选中节点是目录，开始加载子目录内容')
        try {
          // 获取子目录内容
          await fileTreeStore.fetchFileTree(selectedNode.id)
          // 更新子目录内容
          selectedNode.children = [...fileTreeStore.fileTree]
          // console.log('子目录内容加载成功:', selectedNode.children)
        } catch (error) {
          console.error('加载子目录内容失败:', error)
        }
      }
      sortFileTree(selectedNode.children)
    } else {
      console.warn('未找到选中节点')
    }
  } else {
    console.warn('未选中任何节点')
  }
}

// 递归查找节点
const findNodeById = (tree, id) => {
  // console.log('递归查找节点，当前树:', tree, '查找的 id:', id)
  for (const node of tree) {
    if (node.id === id) {
      // console.log('找到节点:', node)
      return node
    }
    if (node.children) {
      // console.log('当前节点有子节点，继续递归查找')
      const result = findNodeById(node.children, id)
      if (result) {
        return result
      }
    }
  }
  console.log('未找到节点')
  return null
}

// 处理右键点击事件
const RightClickMenu = ({ option }) => {
  return {
    onContextmenu: (e) => {
      e.preventDefault() // 阻止默认右键菜单
      selectedNode.value = option // 设置当前右键点击的节点
      isContextMenuVisible.value = true // 显示右键菜单
      contextMenuPosition.value = { x: e.clientX, y: e.clientY } // 设置菜单位置
    },
  }
}

// 处理右键菜单选择
const handleContextMenuSelect = (key) => {
  isContextMenuVisible.value = false
  switch (key) {
    case 'addFile':
      addNode(selectedNode.value.id, false)
      break
    case 'addFolder':
      addNode(selectedNode.value.id, true)
      break
    case 'rename':
      renameNode(selectedNode.value.id)
      break
    case 'delete':
      deleteNode(selectedNode.value.id)
      break
    case 'move':
      moveNode(selectedNode.value)
      break
    default:
      break
  }
}

// 处理点击外部关闭菜单
const handleClickoutside = () => {
  isContextMenuVisible.value = false
}

// 添加一个 ref 来存储当前正在编辑的节点
const editingNode = ref(null)
// 添加一个 ref 来存储编辑中的名称
const editName = ref('')

// 添加文件或目录
const addNode = async (parentId, isDirectory = false) => {
  const parentNode = findNodeById(fileTree.value, parentId)
  console.log('添加文件或目录，父节点:', parentNode)
  if (parentNode && parentNode.is_directory) {
    // 创建一个新节点
    const newNode = {
      id: `${parentId}\\New${isDirectory ? 'Folder' : 'File'}`,
      name: `New${isDirectory ? 'Folder' : 'File'}`,
      is_directory: isDirectory,
      children: isDirectory ? [] : undefined,
    }
    parentNode.children.push(newNode)

    // 对父节点的 children 进行排序
    parentNode.children.sort((a, b) => {
      if (a.is_directory === b.is_directory) return 0 // 如果类型相同，保持原顺序
      return a.is_directory ? 1 : -1 // 目录排在后，文件排在前
    })
    console.log('添加文件或目录成功:', newNode)

    // 设置新节点为编辑状态
    startEditing(newNode)

    // 保持父节点展开状态
    if (!expandedKeys.value.includes(parentId)) {
      expandedKeys.value.push(parentId)
    }
  } else {
    alert('只能在目录下添加文件或目录')
  }
}

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

// 查找父节点
const findParentNode = (tree, id) => {
  for (const node of tree) {
    if (node.children) {
      if (node.children.some((child) => child.id === id)) {
        return node
      }
      const result = findParentNode(node.children, id)
      if (result) {
        return result
      }
    }
  }
  return null
}

// 开始编辑节点
const startEditing = (node) => {
  editingNode.value = node
  editName.value = node.name

  // 聚焦到输入框
  setTimeout(() => {
    const input = document.querySelector('.editable-input')
    if (input) {
      input.focus()
      input.select()
    }
  }, 0)
}

// 处理输入框的 blur 事件
const handleBlur = async () => {
  if (editingNode.value) {
    const oldId = editingNode.value.id
    const parentNode = findParentNode(fileTree.value, oldId)

    if (editName.value.trim()) {
      // 更新节点名称
      editingNode.value.name = editName.value
      const newId = `${parentNode.id}\\${editName.value}`
      editingNode.value.id = newId

      // 调用 Tauri 后端 API 创建文件或目录
      if (editingNode.value.is_directory) {
        await invoke('create_directory', { path: newId })
      } else {
        await invoke('create_file', { path: newId })
      }

      // 如果是重命名，删除旧文件
      if (oldId !== newId) {
        await invoke('delete_path', { path: oldId })
      }
    } else {
      // 如果名称为空，则删除节点
      parentNode.children = parentNode.children.filter(
        (child) => child.id !== oldId
      )
    }

    // 结束编辑状态
    editingNode.value = null
    editName.value = ''
  }
}

// 处理输入框的 keydown 事件
const handleKeydown = (e) => {
  if (e.key === 'Enter') {
    handleBlur()
  } else if (e.key === 'Escape') {
    // 取消编辑
    editingNode.value = null
    editName.value = ''
  }
}

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
  console.log('刷新目录，directoryId:', directoryId);
  console.log('文件树:', fileTree.value);

  // 如果 directoryId 是根目录，刷新整个文件树
  if (directoryId === 'D:\\OpenSource\\ripgrep') {
    await loadFileTree();
    console.log('刷新整个文件树');
  } else if (fileTree.value.some(node => node.id === directoryId)) {
    await loadFileTree();
    console.log('刷新整个文件树');
  } else {
    // 否则刷新指定目录
    const directoryNode = findNodeById(fileTree.value, directoryId);
    if (directoryNode && directoryNode.is_directory) {
      try {
        // 重新加载目录内容
        await fileTreeStore.fetchFileTree(directoryId);
        directoryNode.children = [...fileTreeStore.fileTree];
        // 对子节点进行排序
        directoryNode.children = sortFileTree(directoryNode.children);
        console.log('目录刷新成功:', directoryNode);
      } catch (error) {
        console.error('目录刷新失败:', error);
      }
    }
  }
};

const normalizePath = (path) => {
  return path.replace(/\\/g, '/'); // 将 Windows 路径分隔符统一为 /
};

</script>

<style scoped></style>