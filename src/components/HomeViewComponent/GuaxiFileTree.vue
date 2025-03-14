<template>
  <n-tree :data="fileTree" key-field="id" :render-label="renderLabel" :render-prefix="renderPrefix"
    :expand-on-click="true" @update:selected-keys="handleSelect" />

  <!-- 右键菜单 -->
  <n-dropdown v-model:show="isContextMenuVisible" :options="contextMenuOptions" placement="bottom-start"
    :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleContextMenuSelect" />
</template>

<script setup>
import { NTree, NIcon } from 'naive-ui'
import { DocumentTextOutline, FolderOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { h, ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()

// 文件树数据
const fileTree = ref([])

// 右键菜单状态
const isContextMenuVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const selectedNode = ref(null) // 当前右键点击的节点

// 右键菜单选项
const contextMenuOptions = ref([
  {
    label: '新建文件',
    key: 'addFile',
  },
  {
    label: '新建文件夹',
    key: 'addFolder',
  },
  {
    label: '重命名',
    key: 'rename',
  },
  {
    label: '删除',
    key: 'delete',
  },
])


// 初始化时获取文件树数据
onMounted(async () => {
  // console.log('初始化：开始获取文件树数据')
  try {
    await fileTreeStore.fetchFileTree('E:\\Record\\source\\ripgrep')
    // await fileTreeStore.fetchFileTree('D:\\OpenSource\\ripgrep')
    // await fileTreeStore.fetchFileTree('D:\\Mine\\I\\Note')
    fileTree.value = fileTreeStore.fileTree
    // console.log('初始化：文件树数据加载成功', fileTree.value)
  } catch (error) {
    console.error('初始化：获取文件树数据失败', error)
  }
})

// 渲染文件树节点
const renderLabel = ({ option }) => {
  return option.name
}

// 渲染文件树图标
const renderPrefix = ({ option }) => {
  const icon = option.is_directory ? FolderOutline : DocumentTextOutline
  return h(NIcon, null, { default: () => h(icon) })
}

// 处理文件选中事件
const handleSelect = async (keys) => {
  console.log('文件选中事件触发，选中的 keys:', keys)
  const selectedKey = keys[0]
  if (selectedKey) {
    console.log('查找选中节点，key:', selectedKey)
    const selectedNode = findNodeById(fileTree.value, selectedKey)
    if (selectedNode) {
      console.log('找到选中节点:', selectedNode)
      if (!selectedNode.is_directory) {
        console.log('选中节点是文件，开始获取文件内容')
        fileTreeStore.fetchFileContent(selectedNode.id)
      } else {
        console.log('选中节点是目录，开始加载子目录内容')
        try {
          // 获取子目录内容
          await fileTreeStore.fetchFileTree(selectedNode.id)
          // 更新子目录内容
          selectedNode.children = [...fileTreeStore.fileTree]
          console.log('子目录内容加载成功:', selectedNode.children)
        } catch (error) {
          console.error('加载子目录内容失败:', error)
        }
      }
    } else {
      console.warn('未找到选中节点')
    }
  } else {
    console.warn('未选中任何节点')
  }
}

// // 处理目录展开事件
// const handleExpand = async (keys) => {
//   console.log('目录展开事件触发，展开的 keys:', keys)
//   const expandedKey = keys[0]
//   if (expandedKey) {
//     const expandedNode = findNodeById(fileTree.value, expandedKey)
//     if (expandedNode) {
//       if (expandedNode.is_directory) {
//         try {
//           // 获取子目录内容
//           await fileTreeStore.fetchFileTree(expandedNode.id)
//           // 更新子目录内容
//           expandedNode.children = [...fileTreeStore.fileTree]
//           console.log('子目录内容加载成功:', expandedNode.children)
//         } catch (error) {
//           console.error('加载子目录内容失败:', error)
//         }
//       } else {
//         // 如果不是目录，更新当前目录下的文件
//         try {
//           await fileTreeStore.fetchFileTree(expandedNode.id)
//           // 更新当前目录下的文件
//           expandedNode.children = [...fileTreeStore.fileTree]
//           console.log('当前目录下的文件加载成功:', expandedNode.children)
//         } catch (error) {
//           console.error('加载当前目录下的文件失败:', error)
//         }
//       }
//     } else {
//       console.warn('未找到展开节点')
//     }
//   } else {
//     console.warn('未展开任何节点')
//   }
// }

// 递归查找节点
const findNodeById = (tree, id) => {
  console.log('递归查找节点，当前树:', tree, '查找的 id:', id)
  for (const node of tree) {
    if (node.id === id) {
      console.log('找到节点:', node)
      return node
    }
    if (node.children) {
      console.log('当前节点有子节点，继续递归查找')
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
const handleContextMenu = (e) => {
  e.preventDefault() // 阻止默认右键菜单

  // 获取当前点击的节点
  const target = e.target.closest('.n-tree-node')
  if (target) {
    const nodeId = target.dataset.key
    const node = findNodeById(fileTree.value, nodeId)
    if (node) {
      selectedNode.value = node
      isContextMenuVisible.value = true
      contextMenuPosition.value = { x: e.clientX, y: e.clientY }
    }
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
    default:
      break
  }
}

// 添加文件或目录
const addNode = async (parentId, isDirectory = false) => {
  const parentNode = findNodeById(fileTree.value, parentId)
  if (parentNode && parentNode.is_directory) {
    const newNodeName = prompt(`请输入新${isDirectory ? '目录' : '文件'}的名称:`)
    if (newNodeName) {
      const newNode = {
        id: `${parentId}/${newNodeName}`,
        name: newNodeName,
        is_directory: isDirectory,
        children: isDirectory ? [] : undefined,
      }
      parentNode.children.push(newNode)
      // 调用 Tauri 后端 API 创建文件或目录
      if (isDirectory) {
        await invoke('create_directory', { path: newNode.id })
      } else {
        await invoke('create_file', { path: newNode.id })
      }
      console.log(`添加${isDirectory ? '目录' : '文件'}成功:`, newNode)
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
        console.log('删除成功:', node)
      }
    }
  }
}

// 重命名文件或目录
const renameNode = async (nodeId) => {
  const node = findNodeById(fileTree.value, nodeId)
  if (node) {
    const newName = prompt('请输入新的名称:', node.name)
    if (newName) {
      const oldId = node.id
      const parentNode = findParentNode(fileTree.value, nodeId)
      if (parentNode) {
        node.name = newName
        node.id = `${parentNode.id}/${newName}`
        // 调用 Tauri 后端 API 重命名文件或目录
        await invoke('rename_path', { oldPath: oldId, newPath: node.id })
        console.log('重命名成功:', node)
      }
    }
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

</script>