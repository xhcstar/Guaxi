import { ref } from 'vue'

/**
 * 有声书页面「已打开文件」的本次运行暂存。
 *
 * 为什么不用组件内的 ref：切到别的应用页会把这个页面整个卸载（路由层是直接
 * `h(Comp)`，没有 keep-alive），列表随之清空，每次回来都得重新导入一遍。
 *
 * 为什么也不用 localStorage：这份列表是**工作态**而不是设置。重启之后源文件很可能
 * 已经挪走或改名，把旧路径原样还原只会让人困惑。放模块作用域里，切走再切回还在、
 * 重启应用即清空，正好卡在这条线上；路径是否还有效，交给真正转换时的检查去判断。
 *
 * 设置类参数（模型、音色、引擎等）另走 utils/uiState 的 localStorage，那些应当跨重启保留。
 */
const openedFiles = ref([])

export function useTtsBookFiles() {
  return openedFiles
}
