import { reactive } from 'vue'

/**
 * 邮件行拖拽的共享状态（列表写入、侧栏读取）。
 *
 * 为什么不用 HTML5 拖放：Windows 上 Tauri 的 webview 自己注册了原生放置目标，
 * 页面里的 dragstart / drop 收不到；就算把 `dragDropEnabled` 关掉，不同 WebView2
 * 版本上的表现也不一致（这条反馈已经反复出现三次）。改成用鼠标事件自己实现：
 * 位移超过阈值才算拖拽，落点用 elementFromPoint 找侧栏上的 `data-drop-folder`，
 * 全程由我们控制，也不受 webview 配置影响。
 */
export const mailDrag = reactive({
  active: false,
  /** 这次拖拽要一起移动的邮件行。 */
  rows: [],
  /** 当前鼠标悬停的文件夹名；为空表示没落在任何文件夹上。 */
  hoverFolder: '',
  x: 0,
  y: 0,
})

export function resetMailDrag() {
  mailDrag.active = false
  mailDrag.rows = []
  mailDrag.hoverFolder = ''
}
