/**
 * notebookReadingStore.js
 * 模块级单例：保存 GuaxiNotebookView 的阅读位置历史栈。
 * 独立于组件生命周期，确保探索者模式在卸载/重挂载后仍能恢复阅读位置。
 */
import { ref } from 'vue'

export const notebookReadingLocationBackStack = ref([])
export const notebookReadingLocationForwardStack = ref([])
