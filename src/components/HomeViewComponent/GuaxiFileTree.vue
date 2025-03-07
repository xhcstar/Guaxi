<template>
    <n-tree :data="fileTree" key-field="id" :render-label="renderLabel" :render-prefix="renderPrefix"
        :expand-on-click="true" @update:selected-keys="handleSelect" @update:expanded-keys="handleExpand" />
</template>

<script setup>
import { NTree, NIcon } from 'naive-ui'
import { DocumentTextOutline, FolderOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { h, ref, onMounted } from 'vue'

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()

// 文件树数据
const fileTree = ref([])

// 初始化时获取文件树数据
onMounted(async () => {
    // console.log('初始化：开始获取文件树数据')
    try {
        await fileTreeStore.fetchFileTree('D:\\OpenSource\\ripgrep')
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
</script>