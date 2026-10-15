<template>
  <GuaxiPdfViewer
    ref="viewerRef"
    :path="pdfPath"
    fit-mode="width"
    :source-path="sourcePath"
    :building="building"
    :error-text="errorText"
    :theme="theme"
    :engine="engineKind"
    :title-label="`${titleLabel} 预览`"
    empty-text="暂无预览，开始编辑或点击 ⟳ 构建"
    :features="features"
    @export="$emit('export')"
    @rebuild="$emit('rebuild')"
    @ready="(p) => $emit('ready', p)"
    @reverse-jump="(p) => $emit('reverse-jump', p)"
    @scroll="$emit('scroll')"
  />
</template>

<script setup>
// LaTeX / Typst 预览：现在是统一基础组件 GuaxiPdfViewer 的薄包装。
// 对外保持原有的 props / emits / 暴露的 forwardSync 不变，因此 GuaxiEditor 无需改动。
import { ref, computed } from 'vue'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'

const props = defineProps({
  // 已构建好的 PDF 临时文件路径
  pdfPath: { type: String, default: '' },
  // 源文件路径（.tex / .typ）：双向定位需要
  sourcePath: { type: String, default: '' },
  building: { type: Boolean, default: false },
  errorText: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  // 'LaTeX' | 'Typst'
  titleLabel: { type: String, default: 'LaTeX' },
})

defineEmits(['export', 'rebuild', 'ready', 'reverse-jump', 'scroll'])

const viewerRef = ref(null)

const engineKind = computed(() => (/typst/i.test(props.titleLabel) ? 'typst' : 'latex'))

const features = {
  search: true,
  reverseJump: true,
  outline: false,
  export: true,
  rebuild: true,
}

// 供 GuaxiEditor 调用的正向定位，转发给基础组件。
const forwardSync = (payload) => viewerRef.value?.forwardSync(payload)
// 预留给宿主的大纲/书签页码跳转能力；不改变当前正向定位链路，只把 PdfViewer 已有能力透出。
const goToPage = (page, options) => viewerRef.value?.goToPage(page, options)
// 设计说明：GuaxiEditor 会按源文件记住 PDF 阅读位置；这里继续转发基础阅读器的比例滚动 API，
// 避免编辑器层需要知道 PDF 阅读器内部 DOM 结构。
// null 必须原样透传（容器不可测量），不能兜底成 0：宿主会把它当成“用户就在文档开头”存下来。
const scrollRatio = () => viewerRef.value?.scrollRatio?.() ?? null
const restoreScroll = (ratio) => viewerRef.value?.restoreScroll?.(ratio)
// 跨帧重试到布局稳定的落位：切换文件后恢复阅读位置时用，避免文档刚打开、容器还没有
// 可滚动高度时只落一帧被 clamp 到顶部。
const restoreScrollStable = (ratio) => viewerRef.value?.restoreScrollStable?.(ratio)

defineExpose({ forwardSync, goToPage, scrollRatio, restoreScroll, restoreScrollStable })
</script>
