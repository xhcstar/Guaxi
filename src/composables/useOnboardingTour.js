import { driver } from 'driver.js'
import 'driver.js/dist/driver.css'
import { readUiState, persistUiState } from '../utils/uiState'

/** 「已看过引导」的标记键。放 localStorage：这是界面态，不需要后端参与。 */
const DONE_KEY = 'guaxi_onboarding_done'

/**
 * 目标元素缺失时的落点：一个钉在屏幕中心的零尺寸锚点。
 *
 * 引导里有几步的目标要用户先操作才会出现——知识库表单点了按钮才弹、编辑器要有文件才有。
 * 所以每一步的 `element` 都写成函数：找不到目标就返回这个锚点。气泡于是落在中间继续讲，
 * 而不是被 driver.js 按 `skipMissingElement` 整步跳过、或者因为拿到 null 而报错中断。
 * 这是整个引导最容易翻车的地方。
 */
function centerAnchor() {
  const existing = document.getElementById('guaxi-tour-anchor')
  if (existing) return existing

  const node = document.createElement('div')
  node.id = 'guaxi-tour-anchor'
  Object.assign(node.style, {
    position: 'fixed',
    left: '50%',
    top: '50%',
    width: '0',
    height: '0',
    opacity: '0',
    pointerEvents: 'none',
  })
  document.body.appendChild(node)
  return node
}

const anchorFor = (selector) => () => document.querySelector(selector) || centerAnchor()

/**
 * 引导步骤。没有 `selector` 的步骤由 driver.js 自己的占位元素承载，气泡居中。
 * 目标控件的钩子见各组件里的 `data-tour` 属性。
 */
const STEPS = [
  {
    title: '欢迎使用 Guaxi',
    description:
      '用半分钟走一遍最短路径：新建知识库 → 新建 Markdown 文件 → 切到渲染模式阅读。随时可以点右上角 ✕ 跳过。',
  },
  {
    selector: '[data-tour="create-kb"]',
    title: '第 1 步 · 新建知识库',
    description: '点右上角这个图标新建知识库。知识库是独立的资料空间，文档和摘录都放在里面。',
    side: 'bottom',
  },
  {
    selector: '.kb-create-form .n-input',
    title: '第 2 步 · 起个名字',
    description: '先填知识库名称即可，标签和描述可以以后再补。',
    side: 'bottom',
  },
  {
    selector: '.form-footer .n-button--primary-type',
    title: '第 3 步 · 创建',
    description: '填好名称后点这里完成创建。',
    side: 'top',
  },
  {
    selector: '[data-tour="new-file"]',
    title: '第 4 步 · 新建 Markdown 文件',
    description: '在左侧文件树的知识库标题栏点这个按钮新建文件，默认就是 Markdown。',
    side: 'bottom',
  },
  {
    title: '第 5 步 · 写点内容并保存',
    description: '在编辑区写下内容，按 Ctrl+S 保存。这一步没有固定位置，跟着做就行。',
  },
  {
    selector: '[data-tour="preview-mode"]',
    title: '第 6 步 · 切到渲染模式阅读',
    description: '点这里选择「预览」，保存后的 Markdown 就会以渲染后的样子显示。',
    side: 'bottom',
  },
  {
    title: '就绪',
    description: '这条路径随时可以重来：设置页里有「重新观看新手引导」。',
  },
]

const toDriverSteps = () =>
  STEPS.map((step) => ({
    ...(step.selector ? { element: anchorFor(step.selector) } : {}),
    popover: {
      title: step.title,
      description: step.description,
      side: step.side || 'bottom',
    },
  }))

export function useOnboardingTour() {
  let instance = null

  const hasSeen = () => Boolean(readUiState(DONE_KEY, false))

  /**
   * 启动引导。`force` 用于「重新观看」入口：忽略已看过的标记。
   * 返回是否真的启动了（首次之外直接调用会静默跳过）。
   */
  const start = ({ force = false } = {}) => {
    if (!force && hasSeen()) return false

    instance?.destroy()
    instance = driver({
      showProgress: true,
      progressText: '{{current}} / {{total}}',
      nextBtnText: '下一步',
      prevBtnText: '上一步',
      doneBtnText: '完成',
      // 允许随时关掉：首次启动就强制走完会让人反感。
      allowClose: true,
      // 专属类名，主题样式在 App.vue 的全局 style 里按它写，不去动 driver.js 的默认类。
      popoverClass: 'guaxi-tour-popover',
      overlayColor: 'rgba(8, 12, 18, 0.62)',
      stagePadding: 6,
      stageRadius: 8,
      popoverOffset: 12,
      // 走完或跳过都算看过；下次启动不再打扰。
      onDestroyed: () => {
        persistUiState(DONE_KEY, true)
        instance = null
      },
      steps: toDriverSteps(),
    })
    instance.drive()
    return true
  }

  return { start, hasSeen }
}
