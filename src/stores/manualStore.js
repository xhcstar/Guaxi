import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { manualChapters } from '../components/SettingViewComponent/manual/manualContent.js'

/**
 * 手册与设置页右侧栏之间的共享状态。
 *
 * 手册正文和右侧导航是两棵组件树，滚动位置、当前章节和跳转请求都放在这里：
 * 正文负责上报进度，右侧栏只负责发起跳转，不直接操作 DOM。
 */
export const useManualStore = defineStore('manual', () => {
  const activeChapterId = ref(manualChapters[0]?.id || '')
  const activeSectionId = ref(manualChapters[0]?.sections?.[0]?.id || '')
  const progress = ref(0)
  const scrollRequest = ref(null)
  const searchFocusTick = ref(0)

  const activeChapter = computed(
    () => manualChapters.find((chapter) => chapter.id === activeChapterId.value) || manualChapters[0],
  )

  function setActiveChapter(chapterId) {
    if (chapterId && chapterId !== activeChapterId.value) {
      activeChapterId.value = chapterId
    }
  }

  function setActiveSection(sectionId) {
    if (sectionId && sectionId !== activeSectionId.value) {
      activeSectionId.value = sectionId
    }
  }

  function setProgress(value) {
    progress.value = Math.min(100, Math.max(0, Number(value) || 0))
  }

  function requestScroll(sectionId, options = {}) {
    if (!sectionId) return
    scrollRequest.value = {
      sectionId,
      smooth: options.smooth !== false,
      at: Date.now(),
    }
  }

  function focusSearch() {
    searchFocusTick.value += 1
  }

  return {
    activeChapterId,
    activeChapter,
    activeSectionId,
    progress,
    scrollRequest,
    searchFocusTick,
    setActiveChapter,
    setActiveSection,
    setProgress,
    requestScroll,
    focusSearch,
  }
})
