<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="themeOverrides">
    <n-message-provider placement="bottom">
      <RouterView />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted } from 'vue'
import { RouterView } from 'vue-router'
import { NConfigProvider, NMessageProvider, darkTheme } from 'naive-ui'
import { useSettingsStore } from './stores/settingsStore'
import { mainNavigationItems } from './router/index.js'
import { GUAXI_SHORTCUTS, shortcutText, shortcutTooltip, useGuaxiHotkeys } from './composables/useGuaxiHotkeys'
import { createLogger } from './utils/logger'

const settingsStore = useSettingsStore()
const logger = createLogger('GuaxiApp')

const globalNavigationItems = mainNavigationItems.map((item) => ({
  ...item,
  shortcut: GUAXI_SHORTCUTS[item.shortcutId],
}))

let navIconObserver = null
let navIconDecorationFrame = 0

const currentHashPath = () => (window.location.hash || '#/').slice(1) || '/'

const navigateByShortcut = (item) => {
  const from = currentHashPath()
  logger.info('[GlobalNavigationShortcut] navigate requested', {
    from,
    to: item.path,
    label: item.label,
    shortcut: shortcutText(item.shortcut),
  })
  window.location.hash = item.path
}

useGuaxiHotkeys(
  globalNavigationItems.map((item) => ({
    id: `nav:${item.path}`,
    label: item.label,
    shortcut: item.shortcut,
    handler: () => navigateByShortcut(item),
  })),
  {
    logger,
    scope: 'GuaxiGlobalNavigationShortcuts',
  },
)

const decorateNavIcon = (icon, item) => {
  if (!icon || !item) return

  const title = shortcutTooltip(item.label, item.shortcut)
  icon.setAttribute('title', title)
  icon.setAttribute('aria-label', title)
  icon.setAttribute('data-guaxi-nav-shortcut', shortcutText(item.shortcut))
}

const decorateGlobalNavIcons = () => {
  cancelAnimationFrame(navIconDecorationFrame)
  navIconDecorationFrame = requestAnimationFrame(() => {
    const topItems = globalNavigationItems.slice(0, -1)
    const settingItem = globalNavigationItems[globalNavigationItems.length - 1]

    document.querySelectorAll('.sider-icon-container').forEach((container) => {
      container.querySelectorAll('.sider-icon-top .nav-icon').forEach((icon, index) => {
        decorateNavIcon(icon, topItems[index])
      })
      decorateNavIcon(container.querySelector('.sider-icon-bottom .nav-icon'), settingItem)
    })
  })
}

onMounted(async () => {
  await settingsStore.loadAppearanceConfig()
  // Markdown 格式化选项与外观配置一起在启动时读回：首帧就要用，不能等用户按下格式化才加载。
  await settingsStore.loadMarkdownFormatConfig()
  // 主编辑器的字体/字号/小地图同理：编辑器是按需懒建的，配置必须先到位，
  // 否则首个打开的标签页会先按默认字号渲染一次再跳变。
  await settingsStore.loadEditorDisplayConfig()
  // 图片宽度/圆角走 CSS 变量下发，必须在首帧前落到根节点，
  // 否则预览会先按默认宽度渲染一次再跳变。
  await settingsStore.loadPreviewImageConfig()
  decorateGlobalNavIcons()
  navIconObserver = new MutationObserver(decorateGlobalNavIcons)
  navIconObserver.observe(document.body, {
    childList: true,
    subtree: true,
  })
  logger.debug('[GlobalNavigationShortcut] icon tooltip decorator mounted', {
    count: globalNavigationItems.length,
  })
})

onBeforeUnmount(() => {
  navIconObserver?.disconnect?.()
  navIconObserver = null
  cancelAnimationFrame(navIconDecorationFrame)
})

const ONE_DARK = {
  bg: '#282c34',
  panel: '#21252b',
  header: '#21252b',
  border: '#3e4451',
  text1: '#abb2bf',
  text2: '#9da5b4',
  primary: '#61afef',
  info: '#56b6c2',
  success: '#98c379',
  warning: '#e5c07b',
  error: '#e06c75',
}

const ONE_DARK_LIGHT = {
  bg: '#ffffff',
  panel: '#ffffff',
  header: '#ffffff',
  border: '#d0d7de',
  text1: '#1f2328',
  text2: '#57606a',
  primary: '#4078f2',
  info: '#0184bc',
  success: '#50a14f',
  warning: '#c18401',
  error: '#e45649',
}

const naiveTheme = computed(() => {
  return settingsStore.appearanceTheme === 'dark' ? darkTheme : null
})

const themeOverrides = computed(() => {
  const p = settingsStore.appearanceTheme === 'dark' ? ONE_DARK : ONE_DARK_LIGHT
  return {
    common: {
      // 全局统一 UI 字体:与主界面 / 编辑器同一份（--guaxi-editor-font-family，Maple Mono NF CN）。
      // naive-ui 的下拉菜单、抽屉、弹窗等会 teleport 到 body,只有通过 config-provider 的
      // common.fontFamily 才能让这些浮层也跟随同一字体,否则会回退到 naive-ui 默认无衬线字体,
      // 造成「表格是等宽、右键菜单/抽屉却是另一种字体」的不一致。
      fontFamily:
        "'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
      primaryColor: p.primary,
      primaryColorHover: p.primary,
      primaryColorPressed: p.primary,
      infoColor: p.info,
      successColor: p.success,
      warningColor: p.warning,
      errorColor: p.error,
      bodyColor: p.bg,
      cardColor: p.panel,
      modalColor: p.panel,
      popoverColor: p.panel,
      borderColor: p.border,
      textColorBase: p.text1,
      textColor1: p.text1,
      textColor2: p.text2,
    },
  }
})
</script>

<style>
:root {
  --guaxi-editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  --guaxi-app-bg: #ffffff;
  --guaxi-header-bg: #ffffff;
  --guaxi-content-bg: #ffffff;
  --guaxi-panel-bg: #ffffff;
  --guaxi-border: #d0d7de;
  /* 别名：部分组件误用了 --guaxi-border-color（带 -color 后缀），此前从未定义，
     导致回退到亮色 #e0e0e6，在暗色模式下形成亮边。这里补齐别名。 */
  --guaxi-border-color: var(--guaxi-border);
  /* 弱分隔线（表格行、面板底边等）。此前从未定义，所有用它的地方都回退到
     rgba(0,0,0,.06)——在暗色下等于不可见，于是表格行边界整片消失。 */
  --guaxi-border-weak: rgba(0, 0, 0, 0.08);
  /* Markdown 图片外框：圆角卡片式缩略图的描边与底纹。
     不用纯白底，避免透明 PNG / 浅色截图在亮色主题下「看不见边界」。 */
  --guaxi-figure-border: rgba(15, 23, 42, 0.08);
  --guaxi-figure-bg: transparent;
  /* Markdown 表格配色：预览、阅读器窗口、语法手册三处共用这一组，改一处全局生效。
     亮色走「白底 + 极浅表头 + 发丝线」，暗色不用纯黑，跟随面板底色提亮两档。 */
  --guaxi-table-bg: #ffffff;
  --guaxi-table-border: #e4e8ee;
  --guaxi-table-border-strong: #d5dbe3;
  --guaxi-table-head-bg: #f6f8fa;
  --guaxi-table-head-text: #24292f;
  --guaxi-table-row-alt-bg: #fafbfc;
  --guaxi-table-row-hover-bg: #f2f6fb;
  --guaxi-table-text: #24292f;
  --guaxi-text-primary: #1f2328;
  --guaxi-text-secondary: #57606a;
  /* 别名：部分组件（统计看板图表、影像工坊、首页面板等）误用了 --guaxi-text-color
     与 --guaxi-hover-bg，这两个此前从未定义，于是回退到硬编码亮色，暗色模式下
     文字几乎看不清、悬停/进度槽发白。这里补齐别名，统一跟随主题。 */
  --guaxi-text-color: var(--guaxi-text-primary);
  --guaxi-hover-bg: rgba(15, 23, 42, 0.05);
  /* 全局窄滚动条（8px）：滑块用半透明中性色，亮/暗主题各一档。 */
  --guaxi-scrollbar-thumb: rgba(15, 23, 42, 0.24);
  --guaxi-scrollbar-thumb-hover: rgba(15, 23, 42, 0.4);
  --guaxi-header-shadow: 0px 1px 2px rgba(0, 0, 0, 0.06);
  /* Markdown 渲染出来的代码块字号；与 styles/guaxiBase.css 的值保持一致。 */
  --guaxi-markdown-code-font-size: 17px;
  --guaxi-syntax-base: #383a42;
  --guaxi-syntax-comment: #6a737d;
  --guaxi-syntax-string: #22863a;
  --guaxi-syntax-keyword: #a626a4;
  --guaxi-syntax-number: #986801;
  --guaxi-syntax-boolean: #986801;
  --guaxi-syntax-regexp: #007c89;
  --guaxi-syntax-operator: #007c89;
  --guaxi-syntax-punctuation: #383a42;
  --guaxi-syntax-type: #6f42c1;
  --guaxi-syntax-class: #6f42c1;
  --guaxi-syntax-function: #0969da;
  --guaxi-syntax-method: #007c89;
  --guaxi-syntax-property: #007c89;
  --guaxi-syntax-parameter: #9a6700;
  --guaxi-syntax-variable: #0550ae;
  --guaxi-syntax-constant: #986801;
  --guaxi-syntax-namespace: #6f42c1;
  --guaxi-syntax-macro: #a626a4;
  --guaxi-syntax-tag: #e45649;
  --guaxi-syntax-attribute: #9a6700;
  --guaxi-syntax-builtin: #0969da;
  --guaxi-syntax-decorator: #a626a4;

  /* 设计说明（统一选中与搜索命中的观感）：
     预览、分屏、探索者三个模式此前各写一套颜色，和编辑模式的 Monaco 也不一致。
     这里集中定义，四处共用；改一处就能全局调。
     - selection 只改背景、保留前景色，代码块的语法着色不会被抹平；
     - 搜索命中沿用 Monaco 的橙色系（findMatchHighlight / findMatch）。 */
  --guaxi-selection-bg: rgba(0, 122, 204, 0.26);
  --guaxi-selection-strong-bg: rgba(0, 122, 204, 0.4);
  --guaxi-search-hit-bg: rgba(234, 92, 0, 0.28);
  --guaxi-search-hit-active-bg: rgba(234, 92, 0, 0.5);
  --guaxi-search-hit-active-border: rgba(246, 154, 60, 0.9);

  /* 设计说明（搜索面板三个标签页共用的一套行样式）：
     文件 / 内容 / 摘录 以前各写一套悬停和选中效果——三种圆角、三种阴影、三种位移，
     同一个手势在三个标签页里长得不一样。这里把"行"的状态收成一组变量，三处引用。

     不能再用 --guaxi-header-bg 当悬停底色：亮色主题下它和 --guaxi-panel-bg 都是 #ffffff，
     等于悬停和选中都没有底色，只能靠那道左侧竖条认。所以这里给的是半透明叠加色，
     压在任何主题的底色上都留得下痕迹。
     - hover 只改明度（灰），selected 改色相（主题蓝）：两种状态不会被看成同一件事；
     - rail 是层级引导线，替代"再套一层带边框的盒子"。 */
  --sr-row-hover: rgba(15, 23, 42, 0.05);
  --sr-row-active: rgba(24, 144, 255, 0.12);
  --sr-row-accent: #1890ff;
  --sr-rail: rgba(148, 163, 184, 0.28);
  --sr-hairline: rgba(148, 163, 184, 0.18);

  /* 设计说明（三个标签页共用的一条字号梯级）：
     以前 内容 标签页的文件名是 13px、文件 标签页 14px、摘录 里那层 n-card 标题被 naive 定到 ~18px，
     同一个层级三个字号，最需要读的正文反而最小。这里只留三档，每档差 1px：
     - group：文件名这类组头，三个标签页统一，也是全panel最大的字；
     - body：真正要读的东西（命中行、摘录正文），只比组头小一档；
     - meta：行号、相对路径、时间戳这类附属信息，不参与阅读。
     行高单独给一档，26px 装 15px 的字会顶到边。 */
  --sr-fs-group: 15px;
  --sr-fs-body: 14px;
  --sr-fs-meta: 12px;

  /* 设计说明（三个标签页共用的边距和滚动条）：
     行是通栏的，左右这一档内边距就是面板唯一的横向边距——容器不再另外留边，
     否则窄侧栏里那点宽度全花在留白上。
     滚动条也必须是同一根：以前 文件/内容 用的是原生滚动条（占 15px 布局宽度），
     摘录 用 naive 的 n-scrollbar（悬浮在内容上、不占宽度），于是右边距一边宽一边窄，
     而且悬浮那根直接压在时间戳上。现在三处都是原生细滚动条，槽位是真实宽度，压不到内容。 */
  --sr-pad-x: 8px;
  --sr-scrollbar: 8px;
  --sr-scrollbar-thumb: rgba(15, 23, 42, 0.26);
  --sr-scrollbar-thumb-hover: rgba(15, 23, 42, 0.42);
}

:root[data-guaxi-theme='dark'] {
  --guaxi-app-bg: #282c34;
  --guaxi-header-bg: #21252b;
  --guaxi-content-bg: #282c34;
  --guaxi-panel-bg: #21252b;
  --guaxi-border: #3e4451;
  --guaxi-border-color: var(--guaxi-border);
  /* 暗色下的弱分隔线要比亮色更亮一档，否则表格行边界完全看不见。 */
  --guaxi-border-weak: rgba(255, 255, 255, 0.11);
  /* 暗色下图片外框要比亮色更明显一点，否则白色截图会「发光」糊在深色底上。 */
  --guaxi-figure-border: rgba(255, 255, 255, 0.12);
  --guaxi-figure-bg: transparent;
  /* 暗色滚动条：浅色半透明滑块，不能继续用亮色主题的深灰。 */
  --guaxi-scrollbar-thumb: rgba(255, 255, 255, 0.18);
  --guaxi-scrollbar-thumb-hover: rgba(255, 255, 255, 0.32);
  /* 表格：底色跟面板走，不压成纯黑；表头比底色亮一档，斑马行只差一点点。 */
  --guaxi-table-bg: #21252b;
  --guaxi-table-border: #353c47;
  --guaxi-table-border-strong: #434b58;
  --guaxi-table-head-bg: #262c35;
  --guaxi-table-head-text: #e6edf3;
  --guaxi-table-row-alt-bg: #232931;
  --guaxi-table-row-hover-bg: #2d343e;
  --guaxi-table-text: #d8e0eb;
  --guaxi-text-primary: #d8e0eb;
  --guaxi-text-secondary: #9da5b4;
  --guaxi-text-color: var(--guaxi-text-primary);
  --guaxi-hover-bg: rgba(255, 255, 255, 0.07);
  --guaxi-header-shadow: 0px 2px 4px rgba(0, 0, 0, 0.35);
  --guaxi-syntax-base: #abb2bf;
  --guaxi-syntax-comment: #5c6370;
  --guaxi-syntax-string: #98c379;
  --guaxi-syntax-keyword: #c678dd;
  --guaxi-syntax-number: #d19a66;
  --guaxi-syntax-boolean: #d19a66;
  --guaxi-syntax-regexp: #56b6c2;
  --guaxi-syntax-operator: #56b6c2;
  --guaxi-syntax-punctuation: #abb2bf;
  --guaxi-syntax-type: #e5c07b;
  --guaxi-syntax-class: #e5c07b;
  --guaxi-syntax-function: #61afef;
  --guaxi-syntax-method: #56b6c2;
  --guaxi-syntax-property: #56b6c2;
  --guaxi-syntax-parameter: #d19a66;
  --guaxi-syntax-variable: #d7deea;
  --guaxi-syntax-constant: #d19a66;
  --guaxi-syntax-namespace: #e5c07b;
  --guaxi-syntax-macro: #c678dd;
  --guaxi-syntax-tag: #e06c75;
  --guaxi-syntax-attribute: #d19a66;
  --guaxi-syntax-builtin: #56b6c2;
  --guaxi-syntax-decorator: #c678dd;

  /* 暗色主题下选区与命中都要更亮一档，否则压在深色底上几乎看不出来。 */
  --guaxi-selection-bg: rgba(58, 150, 221, 0.38);
  --guaxi-selection-strong-bg: rgba(58, 150, 221, 0.52);
  --guaxi-search-hit-bg: rgba(234, 92, 0, 0.36);
  --guaxi-search-hit-active-bg: rgba(234, 92, 0, 0.62);
  --guaxi-search-hit-active-border: rgba(246, 154, 60, 0.95);

  /* 深色底上叠加同样的透明度会看不出来，悬停改用白色微光，选中把蓝调再抬一档。 */
  --sr-row-hover: rgba(255, 255, 255, 0.06);
  --sr-row-active: rgba(24, 144, 255, 0.2);
  --sr-row-accent: #4c9df8;
  --sr-rail: rgba(148, 163, 184, 0.24);
  --sr-hairline: rgba(148, 163, 184, 0.16);
  --sr-scrollbar-thumb: rgba(255, 255, 255, 0.2);
  --sr-scrollbar-thumb-hover: rgba(255, 255, 255, 0.34);
}

html,
body {
  /*
   * 根布局设计说明：
   * 1. 应用内部的文件树、编辑器等面板各自管理滚动，浏览器文档本身不参与滚动。
   * 2. 清除 body 默认的 8px 外边距，避免页面使用满高布局时产生额外高度。
   * 3. 建立 html -> body -> #app 的完整高度链，页面组件可以稳定继承窗口高度。
   */
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
  /* 全局默认字体：与主界面 / 编辑器同一份。
     此前只有 naive-ui 组件经 config-provider 拿到字体，普通 div / 表格行会落回
     WebView 默认的衬线字体，于是设置页、大脑、语言学习这些页面看起来像另一个软件。
     放在 body 上让所有未显式声明字体的节点统一继承。 */
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
}

#app {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

/* WebView 的表单控件不继承页面字体，会固定走系统 UI 字体，这里显式跟随全局。 */
button,
input,
select,
textarea {
  font-family: inherit;
}

/* 统一浮层字体：naive-ui 的下拉菜单 / 抽屉 / 弹窗 / 气泡会 teleport 到 <body>，
   拿不到页面组件内的字体设置，也不总是跟随 config-provider 的 fontFamily，
   这里全局强制让这些浮层与主界面同一份字体（--guaxi-editor-font-family）。 */
.n-dropdown-menu,
.n-dropdown-menu *,
.n-drawer,
.n-drawer *,
.n-modal,
.n-modal *,
.n-popover,
.n-popover *,
.n-tooltip,
.n-tooltip * {
  font-family: var(
    --guaxi-editor-font-family,
    'Maple Mono NF CN',
    'JetBrainsMono Nerd Font Mono',
    Consolas,
    monospace,
    'Microsoft YaHei',
    'Courier New'
  ) !important;
}

/* 全局窄滚动条：所有可滚动区域统一 8px，滑块颜色跟随明暗主题。
   具体组件（代码块 / 阅读器）若有自己的滚动条规则，会以更高优先级覆盖这里。 */
* {
  scrollbar-width: thin;
  scrollbar-color: var(--guaxi-scrollbar-thumb) transparent;
}
*::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
*::-webkit-scrollbar-track {
  background: transparent;
}
*::-webkit-scrollbar-thumb {
  background: var(--guaxi-scrollbar-thumb);
  border: 2px solid transparent;
  border-radius: 999px;
  background-clip: content-box;
}
*::-webkit-scrollbar-thumb:hover {
  background: var(--guaxi-scrollbar-thumb-hover);
  background-clip: content-box;
}
*::-webkit-scrollbar-corner {
  background: transparent;
}

pre,
.hljs,
.code-segment {
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.22) transparent;
}

/* 设计说明：阅读器面板统一使用“滚动时显色、静止时隐藏”的滚动条。
   这样鼠标滚轮、触控板和键盘触发滚动时都能看到滚动位置，同时不影响编辑器/代码块的既有滚动条。 */
.guaxi-reader-scrollbar {
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.guaxi-reader-scrollbar:hover,
.guaxi-reader-scrollbar:focus-within,
.guaxi-reader-scrollbar.is-reader-scrolling {
  scrollbar-color: rgba(0, 0, 0, 0.32) transparent;
}

pre::-webkit-scrollbar,
.hljs::-webkit-scrollbar,
.code-segment::-webkit-scrollbar,
.guaxi-reader-scrollbar::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

pre::-webkit-scrollbar-thumb,
.hljs::-webkit-scrollbar-thumb,
.code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.22);
  border-radius: 999px;
}

.guaxi-reader-scrollbar::-webkit-scrollbar-thumb {
  background-color: transparent;
  border-radius: 999px;
}

pre::-webkit-scrollbar-thumb:hover,
.hljs::-webkit-scrollbar-thumb:hover,
.code-segment::-webkit-scrollbar-thumb:hover,
.guaxi-reader-scrollbar:hover::-webkit-scrollbar-thumb,
.guaxi-reader-scrollbar:focus-within::-webkit-scrollbar-thumb,
.guaxi-reader-scrollbar.is-reader-scrolling::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.34);
}

pre::-webkit-scrollbar-track,
.hljs::-webkit-scrollbar-track,
.code-segment::-webkit-scrollbar-track,
.guaxi-reader-scrollbar::-webkit-scrollbar-track {
  background-color: transparent;
}

:root[data-guaxi-theme='dark'] pre,
:root[data-guaxi-theme='dark'] .hljs,
:root[data-guaxi-theme='dark'] .code-segment {
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}

:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:hover,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:focus-within,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar.is-reader-scrolling {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.18);
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:hover::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:focus-within::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar.is-reader-scrolling::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.26);
}

/* 设计说明：普通文本/Markdown Reader 的右侧渲染内容由 HTML 注入，不能依赖单个组件的 scoped CSS 完整命中。
   这里只约束通用文本流和 Markdown 产物，不作用于 PDF/Word/EPUB 左侧专用阅读器的页面渲染。 */
.text-preview-container,
.markdown-panel,
.notes-content {
  overflow-x: hidden;
}

.text-preview-content,
.text-preview-content pre,
.markdown-content,
.markdown-content :where(p, li, blockquote, h1, h2, h3, h4, h5, h6, a, span, div) {
  min-width: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.text-preview-content,
.text-preview-content pre {
  white-space: pre-wrap;
}

/* 设计说明（阅读器里的划词选中）：此前各阅读器的正文选区只有浏览器默认的浅色，
   和编辑器预览不统一。这里复用与编辑器同一组 --guaxi-selection-* 变量。
   EPUB 的正文渲染在 iframe 里，全局样式到不了，另由 epub.js 主题注入（见 GuaxiEpubReader）。 */
.markdown-content ::selection,
.text-preview-container ::selection,
.docx ::selection,
.docx-wrapper ::selection {
  background: var(--guaxi-selection-bg, rgba(0, 122, 204, 0.26));
  color: inherit;
}

/* 代码块底色本来就深，选区再半透明就看不出来了，这里单独加深一档。 */
.markdown-content pre ::selection,
.markdown-content pre::selection,
.markdown-content code ::selection,
.markdown-content code::selection {
  background: var(--guaxi-selection-strong-bg, rgba(0, 122, 204, 0.4));
  color: inherit;
}

/* 逐句朗读的当前句高亮。用 CSS Custom Highlight 而非包裹 DOM：正文是 v-html 渲染的，
   插节点会破坏链接/代码块/锚点。整块规则必须放在全局样式里——::highlight() 没有可挂载的
   元素，写在 scoped 里会被加上 data-v 属性而彻底失效。 */
::highlight(guaxi-readaloud) {
  background-color: var(--guaxi-search-hit-active-bg, rgba(234, 92, 0, 0.5));
  color: inherit;
}

.markdown-content {
  width: 100%;
  box-sizing: border-box;
}

.markdown-content img,
.markdown-content svg,
.markdown-content canvas,
.markdown-content video {
  max-width: 100%;
  height: auto;
  box-sizing: border-box;
}

/* ───── Markdown 表格：编辑器预览 / 阅读器 / 语法手册共用一套 ─────
 * 设计说明：
 * 1. 三个渲染面以前各自为政：`.preview` 的规则写错样式块而整条失效，`.markdown-content`
 *    只有布局没有观感，手册窗口干脆没有表格规则 —— 同一张表在三个地方长得不一样。
 *    这里收成一份，颜色全部走 --guaxi-table-* 变量（App.vue 与 guaxiBase.css 各一份，
 *    对应主窗口与独立阅读器窗口两个入口，改配色两处都要动）。
 * 2. 只给「带 thead 的表」画卡片：markdown 管道表格一定有 thead，而 Word/PDF 转换出来的
 *    HTML 里大量是用于排版的嵌套表，给它们加边框和 100% 宽度会把版式搞烂。
 * 3. 圆角外框要求 border-collapse: separate；此时单元格只画右/下边线，末列与末行收口，
 *    否则会是双线。
 * 4. 排除 Python 单元与数据面板注入的表格，它们自带一套无边框紧凑样式。
 */
:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:not(:is(.guaxi-dataframe-table, .guaxi-inline-dataframe__table)):not(.guaxi-notebook__dataframe *) {
  min-width: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  :is(th, td):not(:is(.guaxi-dataframe-table, .guaxi-inline-dataframe__table, .guaxi-notebook__dataframe) *) {
  min-width: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
}

/* 无 thead 的表（Word / PDF 转 HTML 出来的排版表）保持原来的 100% 宽 + 固定布局，
   不套卡片样式也不改它们的列宽算法，否则转换出来的版式会跟着变。 */
:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:not(:has(> thead)):not(:is(.guaxi-dataframe-table, .guaxi-inline-dataframe__table)):not(.guaxi-notebook__dataframe *) {
  width: 100%;
  table-layout: fixed;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead):not(:is(.guaxi-dataframe-table, .guaxi-inline-dataframe__table)) {
  width: 100%;
  max-width: 100%;
  margin: 1em 0;
  border: 1px solid var(--guaxi-table-border);
  border-radius: 10px;
  border-collapse: separate;
  border-spacing: 0;
  background: var(--guaxi-table-bg);
  color: var(--guaxi-table-text);
  line-height: 1.55;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead):not(:is(.guaxi-dataframe-table, .guaxi-inline-dataframe__table))
  :is(th, td) {
  padding: 8px 14px;
  border-right: 1px solid var(--guaxi-table-border);
  border-bottom: 1px solid var(--guaxi-table-border);
  text-align: left;
  vertical-align: top;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) :is(tr > :last-child) {
  border-right: 0;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) tbody tr:last-child > :is(th, td) {
  border-bottom: 0;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) thead th {
  background: var(--guaxi-table-head-bg);
  color: var(--guaxi-table-head-text);
  font-weight: 600;
  border-bottom-color: var(--guaxi-table-border-strong);
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) thead tr:first-child > th:first-child {
  border-top-left-radius: 9px;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) thead tr:first-child > th:last-child {
  border-top-right-radius: 9px;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) tbody tr:last-child > td:first-child {
  border-bottom-left-radius: 9px;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) tbody tr:last-child > td:last-child {
  border-bottom-right-radius: 9px;
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) tbody tr:nth-child(even) > td {
  background: var(--guaxi-table-row-alt-bg);
}

:is(.markdown-content, .markdown-manual-card-body, .markdown-body, .preview)
  table:has(> thead) tbody tr:hover > td {
  background: var(--guaxi-table-row-hover-bg);
}

.markdown-content pre,
.markdown-content .hljs {
  max-width: 100%;
  box-sizing: border-box;
  white-space: pre-wrap;
  overflow-x: hidden;
  overflow-wrap: anywhere;
}

.markdown-content pre code,
.markdown-content .hljs code {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.markdown-content mjx-container,
.markdown-content .math-block,
.markdown-content .math-inline {
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
}

.markdown-content .mermaid {
  max-width: 100%;
  overflow-x: hidden;
}

.markdown-content .mermaid svg {
  max-width: 100%;
  height: auto;
}

:root[data-guaxi-theme='dark'] .fit-width,
:root[data-guaxi-theme='dark'] .zoom-in,
:root[data-guaxi-theme='dark'] .zoom-out {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-primary) !important;
  border: 1px solid var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .fit-width:hover,
:root[data-guaxi-theme='dark'] .zoom-in:hover,
:root[data-guaxi-theme='dark'] .zoom-out:hover {
  background-color: var(--guaxi-header-bg) !important;
}

:root[data-guaxi-theme='dark'] .zoom-in:disabled,
:root[data-guaxi-theme='dark'] .zoom-out:disabled {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .markdown-panel,
:root[data-guaxi-theme='dark'] .markdown-content {
  background-color: var(--guaxi-content-bg) !important;
  color: var(--guaxi-text-primary) !important;
}

:root[data-guaxi-theme='dark'] .original-content {
  background-color: var(--guaxi-panel-bg) !important;
}

:root[data-guaxi-theme='dark'] .translated-content {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .code-segment {
  background: var(--guaxi-header-bg) !important;
}

:root[data-guaxi-theme='dark'] .diagram-segment {
  background: var(--guaxi-panel-bg) !important;
}

:root[data-guaxi-theme='dark'] .note-segment {
  border-bottom: 1px dashed var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .original-fallback {
  border-top: 1px dashed var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .skipped-translation {
  border-left: 3px solid var(--guaxi-border) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .translation-skipped-marker {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .hljs {
  background: var(--guaxi-panel-bg) !important;
  border-color: var(--guaxi-border) !important;
  /* 基础文字色必须跟代码主题走：这里曾写成正文色（近白），
     导致没被 token 命中的标识符在预览里比编辑器里亮一大截。 */
  color: var(--guaxi-syntax-base) !important;
}

:root[data-guaxi-theme='dark'] .mermaid {
  background-color: #ffffff !important;
  color: #1f2328 !important;
  border: 1px solid var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .mermaid svg {
  background-color: #ffffff !important;
}

/* ── 新手引导气泡（driver.js）──────────────────────────────────────
   气泡由 driver.js 挂到 body 上，组件里的 scoped 样式够不着，只能全局定义。
   选择器写成 `.driver-popover.guaxi-tour-popover`：driver.js 的基础规则带 `all: unset`，
   同权重时会按打包顺序把我的样式清掉，提高一级优先级就不依赖顺序了。
   颜色全走 --guaxi-* 变量，明暗主题自动跟随。 */
.driver-popover.guaxi-tour-popover {
  font-family: inherit;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.32);
  max-width: 320px;
  padding: 14px 16px;
}

.driver-popover.guaxi-tour-popover .driver-popover-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.driver-popover.guaxi-tour-popover .driver-popover-description {
  font-size: 13px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
}

.driver-popover.guaxi-tour-popover .driver-popover-progress-text {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.driver-popover.guaxi-tour-popover .driver-popover-close-btn {
  color: var(--guaxi-text-secondary);
}

.driver-popover.guaxi-tour-popover .driver-popover-footer button {
  border-radius: 6px;
  padding: 3px 12px;
  font-size: 12px;
  text-shadow: none;
  background-color: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
}

.driver-popover.guaxi-tour-popover .driver-popover-footer .driver-popover-next-btn {
  background-color: var(--n-color-primary, #1890ff);
  color: #fff;
  border-color: transparent;
}

/* 箭头只改各方向里“不透明”的那一边，免得把另外三边的 transparent 覆盖掉。 */
.driver-popover.guaxi-tour-popover .driver-popover-arrow-side-left {
  border-left-color: var(--guaxi-panel-bg);
}

.driver-popover.guaxi-tour-popover .driver-popover-arrow-side-right {
  border-right-color: var(--guaxi-panel-bg);
}

.driver-popover.guaxi-tour-popover .driver-popover-arrow-side-top {
  border-top-color: var(--guaxi-panel-bg);
}

.driver-popover.guaxi-tour-popover .driver-popover-arrow-side-bottom {
  border-bottom-color: var(--guaxi-panel-bg);
}
</style>
