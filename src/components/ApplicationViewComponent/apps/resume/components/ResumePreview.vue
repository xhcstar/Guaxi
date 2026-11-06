<template>
  <div class="rs-preview">
    <div class="rs-preview-bar">
      <span class="rs-eyebrow">实时预览 · A4</span>
      <span class="rs-preview-template">{{ store.activeTemplate.name }}</span>
      <div class="rs-zoom">
        <button type="button" title="缩小" aria-label="缩小" @click="zoomBy(-0.08)">
          <Icon :icon="minusIcon" width="12" height="12" />
        </button>
        <span>{{ Math.round(scale * 100) }}%</span>
        <button type="button" title="放大" aria-label="放大" @click="zoomBy(0.08)">
          <Icon :icon="plusIcon" width="12" height="12" />
        </button>
      </div>
      <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" :class="{ 'is-on': autoFit }" title="适应宽度" aria-label="适应宽度" @click="fitToWidth">
        <Icon :icon="fitIcon" width="13" height="13" />
      </button>
    </div>

    <div ref="stage" class="rs-stage">
      <div
        v-if="store.doc"
        class="rs-paper"
        :style="{ width: `${794 * scale}px`, minHeight: `${1123 * scale}px` }"
      >
        <iframe
          :key="frameKey"
          :srcdoc="html"
          sandbox="allow-same-origin"
          title="简历预览"
        />
      </div>
      <div v-else class="rs-preview-empty">打开一份简历后，这里会实时预览排版</div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import minusIcon from '@iconify-icons/lucide/minus'
import plusIcon from '@iconify-icons/lucide/plus'
import fitIcon from '@iconify-icons/lucide/scan'
import { useResumeStore } from '../store/resumeStore'
import { renderResumeHtml, stripScriptsForPreview } from '../render/renderResumeHtml'

const store = useResumeStore()
const stage = ref(null)
const scale = ref(0.8)
const autoFit = ref(true)
const renderedHtml = ref('')
let renderTimer = null
let observer = null

const html = computed(() => renderedHtml.value)
const frameKey = computed(() => `${store.activeDocId}-${store.activeTemplate.id}`)

function computeHtml() {
  if (!store.doc) {
    renderedHtml.value = ''
    return
  }
  renderedHtml.value = stripScriptsForPreview(
    renderResumeHtml(store.doc, store.activeTemplateHtml),
  )
}

watch(
  () => [store.doc?.data, store.doc?.title, store.activeTemplateHtml],
  () => {
    if (renderTimer) clearTimeout(renderTimer)
    renderTimer = setTimeout(computeHtml, 180)
  },
  { deep: true },
)

function measure() {
  if (!autoFit.value) return
  const width = stage.value?.clientWidth || 720
  scale.value = Math.min(1.1, Math.max(0.32, (width - 48) / 794))
}

function fitToWidth() {
  autoFit.value = true
  measure()
}

function zoomBy(delta) {
  autoFit.value = false
  scale.value = Math.min(1.6, Math.max(0.25, scale.value + delta))
}

onMounted(async () => {
  computeHtml()
  await nextTick()
  measure()
  observer = new ResizeObserver(() => measure())
  if (stage.value) observer.observe(stage.value)
})

onBeforeUnmount(() => {
  if (renderTimer) clearTimeout(renderTimer)
  observer?.disconnect()
})
</script>

<style scoped>
.rs-preview-template {
  font-family: var(--rs-label);
  font-size: 9.5px;
  letter-spacing: .1em;
  text-transform: uppercase;
  color: var(--rs-muted);
  border: 1px solid var(--rs-line);
  border-radius: 2px;
  padding: 2px 6px;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
