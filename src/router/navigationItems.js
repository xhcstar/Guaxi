// 全局左侧导航栏的菜单项。
//
// 独立成模块而不是留在 `router/index.js`，是为了让 `GuaxiNavRail` 能直接引用：
// router/index.js 会 import 全部页面组件，导航列 import 它会形成
// 「导航列 → 路由表 → 页面 → 导航列」的循环，Vite 下可能拿到未初始化的绑定。
//
// 这份声明原本就在 router/index.js 里（注释写着「导航项和快捷键 id 统一放在路由层声明」），
// 这里只是搬家，导出名与内容保持不变；router/index.js 会继续 re-export，老引用不用改。
export const mainNavigationItems = Object.freeze([
  { path: '/', routeName: 'main', label: '知识编辑', shortcutId: 'navHome' },
  { path: '/chat', routeName: 'chat', label: '智能助手', shortcutId: 'navChat' },
  { path: '/repository', routeName: 'repository', label: '仓库', shortcutId: 'navRepository' },
  { path: '/brain', routeName: 'brain', label: '知识图谱', shortcutId: 'navBrain' },
  { path: '/application', routeName: 'application', label: '应用', shortcutId: 'navApplication' },
  { path: '/statistics', routeName: 'statistics', label: '统计', shortcutId: 'navStatistics' },
  { path: '/search', routeName: 'search', label: '搜索', shortcutId: 'navSearch' },
  { path: '/excalidraw', routeName: 'excalidraw', label: '白板', shortcutId: 'navExcalidraw' },
  { path: '/setting', routeName: 'setting', label: '设置', shortcutId: 'navSetting' },
])
