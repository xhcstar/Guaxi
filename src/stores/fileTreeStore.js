import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'

export const useFileTreeStore = defineStore('fileTreeStore', {
  state: () => ({
    fileTree: [],
    selectedFile: null,
  }),
  actions: {
    // 获取文件树数据
    async fetchFileTree(path) {
    //   console.log('开始获取文件树数据，路径:', path)
      try {
        const result = await invoke('get_file_tree', { path })
        // console.log('获取文件树数据成功:', result)
        this.fileTree = result
      } catch (error) {
        console.error('获取文件树数据失败:', error)
      }
    },

    // 获取文件内容
    async fetchFileContent(path) {
    //   console.log('开始获取文件内容，路径:', path)
      try {
        const content = await invoke('read_file_content', { path })
        // console.log('获取文件内容成功:', content)
        this.selectedFile = { path, content }
      } catch (error) {
        console.error('获取文件内容失败:', error)
      }
    },

    // 更新文件内容
    async updateFileContent(path, content) {
    //   console.log('开始更新文件内容，路径:', path)
      try {
        await invoke('write_file_content', { path, content })
        // console.log('更新文件内容成功')
        if (this.selectedFile && this.selectedFile.path === path) {
          this.selectedFile.content = content
        //   console.log('更新 selectedFile 内容成功')
        } else {
          console.warn('当前选中的文件与更新的文件不匹配，未更新 selectedFile')
        }
      } catch (error) {
        console.error('更新文件内容失败:', error)
      }
    },
  },
})