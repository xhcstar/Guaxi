<template>
  <!--
    面板自己带一层 NConfigProvider：
    · 主窗口里它嵌在 App 的 provider 内，这里只补颜色变量；
    · 独立阅读器窗口（doc-reader.html）没有 App 那层 provider，naive 的 Select /
      弹层会退回亮色主题，深色模式下出现白底下拉 —— 这层 provider 让两处一致。
  -->
  <n-config-provider :theme="naiveTheme" :theme-overrides="naiveThemeOverrides">
    <div ref="panelRef" class="pot-selection-translate-panel" :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
      @mousedown.stop="onPanelMouseDown" @click.stop @wheel.stop>
    <div class="panel-header" @mousedown.prevent="onHeaderMouseDown">
      <div class="panel-header-left">
        <n-button text size="tiny" :title="pinned ? '取消固定' : '固定'" @click.stop="emit('toggle-pin')">
          <Icon :icon="pinned ? pinOffIcon : pinIcon" width="16" height="16" />
        </n-button>
        <div class="panel-title">划词翻译</div>
      </div>

      <div class="panel-header-right">
        <n-button text size="tiny" title="重新翻译" :disabled="busy" @click.stop="emit('translate')">
          <Icon :icon="rotateCwIcon" width="16" height="16" />
        </n-button>
        <n-button text size="tiny" title="复制译文" :disabled="busy || !primaryText" @click.stop="emit('copy', { text: primaryText, engine: primaryEngine })">
          <Icon :icon="copyIcon" width="16" height="16" />
        </n-button>
        <n-button text size="tiny" title="关闭" @click.stop="emit('close')">
          <Icon :icon="xIcon" width="16" height="16" />
        </n-button>
      </div>
    </div>

    <div ref="panelBodyRef" class="panel-body" tabindex="0" @keydown.stop.prevent="onPanelBodyKeydown">
      <div class="panel-langs" @mousedown.stop @click.stop>
        <n-select v-model:value="fromLangProxy" size="tiny" :options="langOptions" :disabled="busy" style="width: 130px" />
        <n-button text size="tiny" title="交换语言" :disabled="busy" @click.stop="emit('swap-langs')">
          <Icon :icon="swapIcon" width="16" height="16" />
        </n-button>
        <n-select v-model:value="toLangProxy" size="tiny" :options="langOptions" :disabled="busy" style="width: 130px" />
      </div>

      <div v-if="busy && !isWordPanel" class="section-state">
        <n-spin size="small" />
        <span>正在翻译...</span>
      </div>

      <div v-else-if="error" class="section-content section-error">{{ error }}</div>

      <div v-else-if="isWordPanel" class="word-panel">
          <div class="word-main-card">
            <div class="word-main-head">
              <div class="word-head">
                <div class="word-text">{{ wordMetaWord }}</div>
                <div class="word-meta-badges">
                  <div v-if="phoneticUkText" class="phonetic-row">
                    <n-tag size="small" round>
                      <template #icon>
                        <Icon :icon="speechIcon" width="13" height="13" />
                      </template>
                      英 {{ phoneticUkText }}
                    </n-tag>
                    <n-button text size="tiny" title="英式发音" @click.stop="playDictPronunciation('uk')">
                      <Icon :icon="volumeUkIcon" width="16" height="16" />
                    </n-button>
                  </div>
                  <div v-if="phoneticUsText" class="phonetic-row">
                    <n-tag size="small" round>
                      <template #icon>
                        <Icon :icon="speechIcon" width="13" height="13" />
                      </template>
                      美 {{ phoneticUsText }}
                    </n-tag>
                    <n-button text size="tiny" title="美式发音" @click.stop="playDictPronunciation('us')">
                      <Icon :icon="volumeUsIcon" width="16" height="16" />
                    </n-button>
                  </div>
                  <n-tag v-if="wordMeta?.part_of_speech" size="small" round type="info">
                    <template #icon>
                      <Icon :icon="badgeInfoIcon" width="13" height="13" />
                    </template>
                    词性: {{ wordMeta.part_of_speech }}
                  </n-tag>
                </div>
              </div>
              <n-button text size="tiny" title="复制词条内容" :disabled="!wordMainCopyText" @click.stop="copyText(wordMainCopyText)">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div class="word-main-translation">{{ combinedWordTranslationText || '—' }}</div>
          </div>
          <div v-if="phrasePairs.length" class="word-sub-section">
            <div class="panel-section-head">
              <div class="word-sub-title">常用短语</div>
              <n-button text size="tiny" title="复制本区域" :disabled="!phrasesCopyText" @click.stop="copyText(phrasesCopyText)">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div class="word-lines">
              <div v-for="(it, i) in phrasePairs" :key="`ph-${i}`" class="word-example-item">
                <div class="word-line"><span class="word-line-index">{{ i + 1 }}.</span>{{ it.source }}</div>
                <div v-if="it.target" class="word-line word-line-target">{{ it.target }}</div>
              </div>
            </div>
          </div>
          <div v-if="synonymsList.length || antonymsList.length || associatesList.length" class="word-sub-section">
            <div class="panel-section-head">
              <div class="word-sub-title">近义 / 反义 / 联想词</div>
              <n-button text size="tiny" title="复制本区域" :disabled="!lexCopyText" @click.stop="copyText(lexCopyText)">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div class="word-lex-group" v-if="synonymsList.length">
              <div class="word-lex-label">近义词</div>
              <div class="word-chip-wrap">
                <n-tag v-for="(x, i) in synonymsPairs" :key="`syn-${i}`" size="small">{{ x.w }}<span v-if="x.t"> · {{ x.t }}</span></n-tag>
              </div>
            </div>
            <div class="word-lex-group" v-if="antonymsList.length">
              <div class="word-lex-label">反义词</div>
              <div class="word-chip-wrap">
                <n-tag v-for="(x, i) in antonymsPairs" :key="`ant-${i}`" size="small" type="warning">{{ x.w }}<span v-if="x.t"> · {{ x.t }}</span></n-tag>
              </div>
            </div>
            <div class="word-lex-group" v-if="associatesList.length">
              <div class="word-lex-label">联想词</div>
              <div class="word-chip-wrap">
                <n-tag v-for="(x, i) in associatesPairs" :key="`asc-${i}`" size="small" type="info">{{ x.w }}<span v-if="x.t"> · {{ x.t }}</span></n-tag>
              </div>
            </div>
          </div>
          <div v-if="examplePairs.length" class="word-sub-section">
            <div class="panel-section-head">
              <div class="word-sub-title">常用例句库</div>
              <n-button text size="tiny" title="复制本区域" :disabled="!examplesCopyText" @click.stop="copyText(examplesCopyText)">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div class="word-lines">
              <div v-for="(it, i) in examplePairs" :key="`ex-${i}`" class="word-example-item">
                <div class="word-line"><span class="word-line-index">{{ i + 1 }}.</span>{{ it.source }}</div>
                <div v-if="it.target" class="word-line word-line-target">{{ it.target }}</div>
              </div>
            </div>
          </div>
          <div v-if="busy" class="section-state">
            <n-spin size="small" />
            <span>词条模块继续加载中...</span>
          </div>
      </div>

      <div v-else class="engine-list">
          <div class="section-content section-raw generic-source">
            <div>{{ raw || '—' }}</div>
          </div>
          <div v-for="it in items" :key="it.engine" class="engine-item">
            <div class="engine-item-header">
              <div class="engine-name">{{ it.engine }}</div>
              <n-button text size="tiny" title="复制" :disabled="!it.success || !it.text"
                @click.stop="emit('copy', { text: it.text || '', engine: it.engine })">
                <Icon :icon="copyIcon" width="16" height="16" />
              </n-button>
            </div>
            <div v-if="!it.success" class="engine-body engine-error">{{ it.error || '翻译失败' }}</div>
            <div v-else class="engine-body engine-text">{{ it.text || '—' }}</div>
          </div>

          <div v-if="!items || items.length === 0" class="section-content section-result">{{ result || '—' }}</div>
      </div>
    </div>
    </div>
  </n-config-provider>
</template>

<script setup>
import { ref, onUnmounted, computed, onMounted, nextTick } from 'vue'
import { NButton, NConfigProvider, NSelect, NSpin, NTag, darkTheme } from 'naive-ui'
import { Icon } from '@iconify/vue'
import pinIcon from '@iconify-icons/lucide/pin'
import pinOffIcon from '@iconify-icons/lucide/pin-off'
import copyIcon from '@iconify-icons/lucide/copy'
import rotateCwIcon from '@iconify-icons/lucide/rotate-cw'
import xIcon from '@iconify-icons/lucide/x'
import swapIcon from '@iconify-icons/lucide/arrow-left-right'
import badgeInfoIcon from '@iconify-icons/lucide/badge-info'
import speechIcon from '@iconify-icons/lucide/messages-square'
import volumeUsIcon from '@iconify-icons/lucide/volume-2'
import volumeUkIcon from '@iconify-icons/lucide/volume-1'

const props = defineProps({
  pos: { type: Object, default: () => ({ left: 8, top: 8 }) },
  raw: { type: String, default: '' },
  result: { type: String, default: '' },
  error: { type: String, default: '' },
  busy: { type: Boolean, default: false },
  pinned: { type: Boolean, default: false },
  items: { type: Array, default: () => [] },
  wordMeta: { type: Object, default: null },
  fromLang: { type: String, default: 'auto' },
  toLang: { type: String, default: 'zh' },
  langOptions: { type: Array, default: () => [] }
})

const emit = defineEmits([
  'close',
  'translate',
  'copy',
  'toggle-pin',
  'swap-langs',
  'update:pos',
  'update:fromLang',
  'update:toLang'
])

const fromLangProxy = computed({
  get: () => props.fromLang,
  set: (v) => emit('update:fromLang', v)
})

const toLangProxy = computed({
  get: () => props.toLang,
  set: (v) => emit('update:toLang', v)
})

const primaryItem = computed(() => {
  const list = Array.isArray(props.items) ? props.items : []
  return list.find((it) => it && it.success && String(it.text || '').trim()) || null
})

const primaryText = computed(() => {
  const it = primaryItem.value
  if (it && it.success) return String(it.text || '')
  return String(props.result || '')
})

const primaryEngine = computed(() => {
  const it = primaryItem.value
  if (it && it.engine) return String(it.engine || '')
  return ''
})

const isWordLikeRaw = computed(() => {
  const raw = String(props.raw || '').trim()
  if (!raw || /\s/.test(raw)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(raw)
})

const isWordPanel = computed(() => {
  return !!(props.wordMeta && isWordLikeRaw.value)
})

const wordMetaWord = computed(() => {
  const raw = String(props.raw || '').trim()
  const w = String(props.wordMeta?.word || '').trim()
  return w || raw || '—'
})

const phoneticUkText = computed(() => {
  const direct = String(props.wordMeta?.phonetic_uk || '').trim()
  if (direct) return direct
  return String(props.wordMeta?.phonetic || '').trim()
})

const phoneticUsText = computed(() => {
  const direct = String(props.wordMeta?.phonetic_us || '').trim()
  if (direct) return direct
  return String(props.wordMeta?.phonetic || '').trim()
})

const copyText = (text, engine = primaryEngine.value) => {
  const v = String(text || '').trim()
  if (!v) return
  emit('copy', { text: v, engine: engine || 'SelectionTranslatePanel' })
}

let panelPronounceAudio = null
const playDictPronunciation = (accent = 'us') => {
  const word = String(wordMetaWord.value || '').trim()
  if (!word) return
  const type = accent === 'uk' ? '1' : '0'
  const url = `https://dict.youdao.com/dictvoice?audio=${encodeURIComponent(word)}&type=${type}`
  try {
    if (panelPronounceAudio) {
      panelPronounceAudio.pause()
      panelPronounceAudio = null
    }
    panelPronounceAudio = new Audio(url)
    panelPronounceAudio.play().catch(() => {})
  } catch {}
}

const normalizeList = (arr) => {
  const list = Array.isArray(arr) ? arr : []
  const out = []
  list.forEach((x) => {
    const t = String(x || '').trim()
    if (!t) return
    if (!out.some((y) => y.toLowerCase() === t.toLowerCase())) {
      out.push(t)
    }
  })
  return out
}

const splitSimpleTranslations = (text) => {
  const src = String(text || '').trim()
  if (!src) return []
  const parts = src.split(/[;；,，、/|｜]/g).map((x) => String(x || '').trim()).filter(Boolean)
  const out = []
  parts.forEach((p) => {
    if (p.length > 16) return
    if (!out.some((x) => x === p)) out.push(p)
  })
  return out.slice(0, 5)
}

const synonymsAll = computed(() => normalizeList(props.wordMeta?.synonyms))
const antonymsList = computed(() => normalizeList(props.wordMeta?.antonyms))
const synonymsTranslations = computed(() => (Array.isArray(props.wordMeta?.synonyms_translations) ? props.wordMeta.synonyms_translations : []).map((x) => String(x || '').trim()))
const antonymsTranslations = computed(() => (Array.isArray(props.wordMeta?.antonyms_translations) ? props.wordMeta.antonyms_translations : []).map((x) => String(x || '').trim()))
const associatesTranslations = computed(() => (Array.isArray(props.wordMeta?.associates_translations) ? props.wordMeta.associates_translations : []).map((x) => String(x || '').trim()))

const associatesList = computed(() => {
  const direct = normalizeList(props.wordMeta?.associates || props.wordMeta?.associations || props.wordMeta?.related)
  if (direct.length) return direct.slice(0, 24)
  const all = synonymsAll.value
  if (all.length <= 12) return []
  return all.slice(12, 24)
})

const synonymsList = computed(() => {
  const all = synonymsAll.value
  if (all.length <= 12) return all
  return all.slice(0, 12)
})

const synonymsPairs = computed(() => {
  return synonymsList.value.map((w, i) => ({ w, t: String(synonymsTranslations.value[i] || '').trim() }))
})

const antonymsPairs = computed(() => {
  return antonymsList.value.map((w, i) => ({ w, t: String(antonymsTranslations.value[i] || '').trim() }))
})

const associatesPairs = computed(() => {
  return associatesList.value.map((w, i) => ({ w, t: String(associatesTranslations.value[i] || '').trim() }))
})

const combinedWordTranslationText = computed(() => {
  const defs = normalizeList(props.wordMeta?.meanings)
  if (defs.length) return defs.slice(0, 5).join('；')
  const lead = splitSimpleTranslations(primaryText.value)
  return lead.slice(0, 5).join('；')
})

const examplePairs = computed(() => {
  const sources = normalizeList(props.wordMeta?.examples)
  const targets = Array.isArray(props.wordMeta?.example_translations)
    ? props.wordMeta.example_translations.map((x) => String(x || '').trim())
    : []
  const pairs = sources.slice(0, 18).map((source, i) => ({
    source,
    target: String(targets[i] || '').trim()
  }))
  if (pairs.length) return pairs
  const fallbackSource = String(phrasePairs.value[0]?.source || '').trim() || `This is a ${wordMetaWord.value}.`
  const fallbackTarget = String(phrasePairs.value[0]?.target || '').trim() || combinedWordTranslationText.value
  return [{ source: fallbackSource, target: String(fallbackTarget || '').trim() }]
})

const phrasePairs = computed(() => {
  const sources = normalizeList(props.wordMeta?.phrases)
  const targets = Array.isArray(props.wordMeta?.phrase_translations)
    ? props.wordMeta.phrase_translations.map((x) => String(x || '').trim())
    : []
  return sources.slice(0, 12).map((source, i) => ({
    source,
    target: String(targets[i] || '').trim()
  }))
})

const wordMainCopyText = computed(() => {
  const top = String(wordMetaWord.value || '').trim()
  const meta = [String(props.wordMeta?.phonetic || '').trim(), String(props.wordMeta?.part_of_speech || '').trim()]
    .filter(Boolean)
    .join(' ')
  const body = String(combinedWordTranslationText.value || '').trim()
  return [top, meta, body].filter(Boolean).join('\n')
})

const lexCopyText = computed(() => {
  const formatPair = (x) => x.t ? `${x.w}（${x.t}）` : x.w
  const chunks = []
  if (synonymsPairs.value.length) chunks.push(`近义词：${synonymsPairs.value.map(formatPair).join('；')}`)
  if (antonymsPairs.value.length) chunks.push(`反义词：${antonymsPairs.value.map(formatPair).join('；')}`)
  if (associatesPairs.value.length) chunks.push(`联想词：${associatesPairs.value.map(formatPair).join('；')}`)
  return chunks.join('\n')
})

const examplesCopyText = computed(() => {
  return examplePairs.value
    .map((it, i) => `${i + 1}. ${it.source}${it.target ? `\n${it.target}` : ''}`)
    .join('\n')
})

const phrasesCopyText = computed(() => {
  return phrasePairs.value
    .map((it, i) => `${i + 1}. ${it.source}${it.target ? `\n${it.target}` : ''}`)
    .join('\n')
})

const panelRef = ref(null)
const panelBodyRef = ref(null)
let panelResizeObserver = null

let dragging = false
let startX = 0
let startY = 0
let startLeft = 0
let startTop = 0

const clampToParent = (left, top) => {
  const el = panelRef.value
  const parent = el?.offsetParent
  const parentRect = typeof parent?.getBoundingClientRect === 'function'
    ? parent.getBoundingClientRect()
    : null
  const pw = parent?.clientWidth ?? window.innerWidth
  const ph = parent?.clientHeight ?? window.innerHeight
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight

  const w = el?.offsetWidth ?? 360
  const h = el?.offsetHeight ?? 220

  const pad = 8
  const visibleLeft = parentRect ? Math.max(pad, -parentRect.left + pad) : pad
  const visibleTop = parentRect ? Math.max(pad, -parentRect.top + pad) : pad
  const visibleRight = parentRect
    ? Math.min(pw - pad, viewportWidth - parentRect.left - pad)
    : viewportWidth - pad
  const visibleBottom = parentRect
    ? Math.min(ph - pad, viewportHeight - parentRect.top - pad)
    : viewportHeight - pad
  const maxLeft = Math.max(visibleLeft, visibleRight - w)
  const maxTop = Math.max(visibleTop, visibleBottom - h)

  return {
    left: Math.min(Math.max(visibleLeft, left), maxLeft),
    top: Math.min(Math.max(visibleTop, top), maxTop)
  }
}

const updatePos = (left, top) => {
  const p = clampToParent(left, top)
  emit('update:pos', p)
}

const onMouseMove = (e) => {
  if (!dragging) return
  const dx = e.clientX - startX
  const dy = e.clientY - startY
  updatePos(startLeft + dx, startTop + dy)
}

const onMouseUp = () => {
  if (!dragging) return
  dragging = false
  window.removeEventListener('mousemove', onMouseMove, true)
  window.removeEventListener('mouseup', onMouseUp, true)
}

const startDrag = (e) => {
  dragging = true
  startX = e.clientX
  startY = e.clientY
  startLeft = Number(props.pos?.left ?? 0) || 0
  startTop = Number(props.pos?.top ?? 0) || 0
  window.addEventListener('mousemove', onMouseMove, true)
  window.addEventListener('mouseup', onMouseUp, true)
}

const isInteractiveTarget = (target) => {
  const el = target instanceof Element ? target : null
  if (!el) return false
  return Boolean(el.closest('button,input,textarea,select,option,a,[role="button"],.n-button,.n-input,.n-select'))
}

const onPanelMouseDown = (e) => {
  if (e.button !== 0) return
  if (isInteractiveTarget(e.target)) return
  startDrag(e)
}

const onHeaderMouseDown = (e) => {
  if (e.button !== 0) return
  startDrag(e)
}

const ensureClampedPos = () => {
  const left = Number(props.pos?.left ?? 0) || 0
  const top = Number(props.pos?.top ?? 0) || 0
  const p = clampToParent(left, top)
  if (Number(props.pos?.left) === p.left && Number(props.pos?.top) === p.top) return
  emit('update:pos', p)
}

const onPanelBodyKeydown = (e) => {
  const el = panelBodyRef.value
  if (!el) return
  const step = 42
  const page = Math.max(120, Math.floor(el.clientHeight * 0.72))
  if (e.key === 'ArrowDown') el.scrollTop += step
  else if (e.key === 'ArrowUp') el.scrollTop -= step
  else if (e.key === 'PageDown') el.scrollTop += page
  else if (e.key === 'PageUp') el.scrollTop -= page
  else if (e.key === 'Home') el.scrollTop = 0
  else if (e.key === 'End') el.scrollTop = el.scrollHeight
}

// ── 面板主题 ───────────────────────────────────────────────────────────
// 面板外壳用的是 --guaxi-* 变量，但 naive 的 Select / 下拉弹层不吃这些变量。
// 独立阅读器窗口没有 App.vue 那层 NConfigProvider，所以这里补一层：跟随
// <html data-guaxi-theme> 切换亮/暗，并把弹层底色对齐面板变量。
const panelDark = ref(false)
const naiveTheme = computed(() => (panelDark.value ? darkTheme : null))
const naiveThemeOverrides = ref(undefined)
let panelThemeObserver = null

const syncPanelTheme = () => {
  const el = typeof document !== 'undefined' ? document.documentElement : null
  if (!el) return
  panelDark.value = el.dataset?.guaxiTheme === 'dark'
  try {
    const cs = getComputedStyle(el)
    const read = (name) => cs.getPropertyValue(name).trim()
    const panelBg = read('--guaxi-panel-bg')
    const border = read('--guaxi-border')
    const text = read('--guaxi-text-primary')
    const font = read('--guaxi-editor-font-family')
    naiveThemeOverrides.value = {
      common: {
        ...(panelBg ? { popoverColor: panelBg, cardColor: panelBg, modalColor: panelBg } : {}),
        ...(border ? { borderColor: border, dividerColor: border } : {}),
        ...(text ? { textColorBase: text, textColor1: text } : {}),
        ...(font ? { fontFamily: font } : {}),
      },
    }
  } catch {
    naiveThemeOverrides.value = undefined
  }
}

onMounted(async () => {
  syncPanelTheme()
  if (typeof MutationObserver !== 'undefined') {
    panelThemeObserver = new MutationObserver(syncPanelTheme)
    panelThemeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-guaxi-theme', 'class'],
    })
  }
  await nextTick()
  ensureClampedPos()
  if (typeof ResizeObserver !== 'undefined' && panelRef.value) {
    panelResizeObserver = new ResizeObserver(() => {
      ensureClampedPos()
    })
    panelResizeObserver.observe(panelRef.value)
  }
  window.addEventListener('resize', ensureClampedPos, true)
})

onUnmounted(() => {
  if (panelThemeObserver) {
    panelThemeObserver.disconnect()
    panelThemeObserver = null
  }
  if (panelPronounceAudio) {
    panelPronounceAudio.pause()
    panelPronounceAudio = null
  }
  if (panelResizeObserver) {
    panelResizeObserver.disconnect()
    panelResizeObserver = null
  }
  window.removeEventListener('resize', ensureClampedPos, true)
  onMouseUp()
})
</script>

<style scoped>
/* NConfigProvider 在我们的模板里只是主题容器，不参与布局：
   display:contents 让面板的绝对定位仍以阅读器容器为参照。 */
.n-config-provider {
  display: contents;
}

.pot-selection-translate-panel {
  position: absolute;
  z-index: 20;
  width: min(460px, calc(100% - 16px));
  min-width: min(320px, calc(100% - 16px));
  min-height: 220px;
  max-width: calc(100% - 16px);
  max-height: min(calc(100dvh - 24px), 70vh);
  display: flex;
  flex-direction: column;
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 10px 26px rgba(0, 0, 0, 0.16);
  overflow: hidden;
  resize: both;
  user-select: none;
  font-family: var(--guaxi-editor-font-family, var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'));
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  cursor: grab;
}

.panel-header:active {
  cursor: grabbing;
}

.panel-header-left,
.panel-header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.panel-header-left {
  min-width: 0;
}

.panel-header-right {
  flex-shrink: 0;
}

.panel-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  opacity: 0.95;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.panel-body {
  padding: 12px 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--guaxi-border) 70%, transparent) transparent;
}

.panel-body::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.panel-body::-webkit-scrollbar-thumb {
  background: color-mix(in srgb, var(--guaxi-border) 75%, transparent);
  border-radius: 999px;
}

.panel-body::-webkit-scrollbar-track {
  background: transparent;
}

.panel-langs {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.engine-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.word-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.word-main-card {
  padding: 8px 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  min-width: 0;
  max-width: 100%;
  background: linear-gradient(160deg, color-mix(in srgb, var(--guaxi-panel-bg) 96%, #8fb8ff 4%) 0%, color-mix(in srgb, var(--guaxi-panel-bg) 95%, #7ef0cb 5%) 100%);
}

.word-main-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.word-head {
  min-width: 0;
}

.word-text {
  font-size: 30px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
  word-break: break-word;
  overflow-wrap: anywhere;
}

.word-meta-badges {
  margin-top: 8px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.phonetic-row {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.word-main-translation {
  margin-top: 10px;
  font-size: 14px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.generic-source {
  border-radius: 12px;
}

.word-chip-wrap {
  display: flex;
  flex-wrap: wrap;
  /* 横向 6px 是同一行的词间距离；纵向要大一些，否则多行联想词会挤成一片 */
  gap: 8px 6px;
  min-width: 0;
  max-width: 100%;
}

/* n-tag 默认 nowrap + 固定高度 + line-height:1，长释义会把面板撑宽、多行还会挤成一片 */
.word-chip-wrap :deep(.n-tag),
.word-meta-badges :deep(.n-tag) {
  max-width: 100%;
  height: auto;
  min-height: 20px;
  white-space: normal;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.word-chip-wrap :deep(.n-tag .n-tag__content),
.word-meta-badges :deep(.n-tag .n-tag__content) {
  white-space: normal;
  word-break: break-word;
  overflow-wrap: anywhere;
}

/* 联想词这类长文本标签会折行，需要比药丸更大的上下留白和行距，
   否则边框贴着字、行与行糊在一起。徽标区保持原样。 */
.word-chip-wrap :deep(.n-tag) {
  padding: 4px 9px;
}

.word-chip-wrap :deep(.n-tag .n-tag__content) {
  line-height: 1.55;
}

.word-sub-section {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  max-width: 100%;
  background: color-mix(in srgb, var(--guaxi-header-bg) 68%, transparent);
}

.word-sub-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  opacity: 0.88;
}

.panel-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.word-lex-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.word-lex-label {
  font-size: 13px;
  color: var(--guaxi-text-secondary);
}

.word-lines {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.word-line {
  font-size: 15px;
  line-height: 1.55;
  color: var(--guaxi-text-primary);
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.word-example-item {
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 94%, transparent);
}

.word-line-index {
  color: var(--guaxi-text-secondary);
  margin-right: 6px;
  font-size: 13px;
}

.word-line-target {
  margin-top: 4px;
  font-size: 14px;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 70%, var(--guaxi-text-primary));
}

.engine-item {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  overflow: hidden;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.engine-item-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: color-mix(in srgb, var(--guaxi-header-bg) 80%, transparent);
  border-bottom: 1px solid var(--guaxi-border);
}

.engine-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  opacity: 0.9;
}

.engine-body {
  padding: 8px 10px;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.engine-text {
  color: var(--guaxi-text-primary);
}

.engine-error {
  color: #ff6b6b;
  background: rgba(255, 107, 107, 0.06);
}

.section-label {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.75;
}

.section-content {
  font-size: 15px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
}

.section-raw {
  background: color-mix(in srgb, var(--guaxi-header-bg) 85%, transparent);
}

.section-result {
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
}

.section-error {
  background: rgba(255, 107, 107, 0.08);
  border-color: rgba(255, 107, 107, 0.35);
  color: #ff6b6b;
}

.section-state {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.8;
  padding: 2px 2px 0;
}
</style>
