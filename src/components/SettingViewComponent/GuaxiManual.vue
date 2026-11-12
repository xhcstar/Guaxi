<template>
  <div
    ref="shellRef"
    class="manual-shell"
    :class="[`layout-${layoutMode}`, { 'is-dark': isDarkTheme, 'is-searching': !!query.trim() }]"
  >
    <div class="m-progress" :style="{ transform: `scaleX(${scrollProgress / 100})` }" aria-hidden="true"></div>

    <header class="m-hero">
      <div class="m-hero-grid">
        <div class="m-hero-copy">
          <div class="m-eyebrow">
            <span class="m-eyebrow-dot"></span>
            GUAXI · FIELD MANUAL
            <span class="m-eyebrow-version">v0.1</span>
          </div>
          <h1 class="m-title">把知识，做成<em>可用的东西</em></h1>
          <p class="m-hero-lead">
            从导入资料到沉淀笔记，从 Agent 动手到图谱与大脑替你读材料。这是一份按页面、按功能拆开的使用手册。
          </p>

          <div class="m-search" role="search">
            <svg class="m-search-icon" viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="11" cy="11" r="7"></circle>
              <path d="m16.2 16.2 4.3 4.3"></path>
            </svg>
            <input
              ref="searchInput"
              v-model="query"
              class="m-search-input"
              type="search"
              placeholder="搜索功能、页面或用法，例如：全词匹配、RSS、同步、插件"
              aria-label="搜索手册"
              @keydown="onSearchKeydown"
            />
            <kbd v-if="!query" class="m-search-kbd">Ctrl K</kbd>
            <button v-else class="m-search-clear" type="button" @click="clearSearch">清除</button>
          </div>

          <div class="m-hot">
            <span class="m-hot-label">热门</span>
            <button
              v-for="chip in hotQueries"
              :key="chip"
              type="button"
              class="m-hot-chip"
              @click="applyQuery(chip)"
            >
              {{ chip }}
            </button>
          </div>
        </div>

        <aside class="m-hero-aside">
          <div class="m-route-card">
            <div class="m-route-title">阅读路径</div>
            <button type="button" class="m-route-item" @click="goChapter('start-quick')">
              <span class="m-route-no">01</span>
              <span class="m-route-main">
                <b>第一次使用</b>
                <small>目录、模型、导入</small>
              </span>
              <span class="m-route-arrow">→</span>
            </button>
            <button type="button" class="m-route-item" @click="goChapter('home-layout')">
              <span class="m-route-no">02</span>
              <span class="m-route-main">
                <b>日常写作</b>
                <small>编辑器、搜索、图表</small>
              </span>
              <span class="m-route-arrow">→</span>
            </button>
            <button type="button" class="m-route-item" @click="goChapter('agent-layout')">
              <span class="m-route-no">03</span>
              <span class="m-route-main">
                <b>让 Agent 干活</b>
                <small>任务、审批、diff</small>
              </span>
              <span class="m-route-arrow">→</span>
            </button>
            <button type="button" class="m-route-item" @click="goChapter('shortcuts-toolbar')">
              <span class="m-route-no">04</span>
              <span class="m-route-main">
                <b>查快捷键</b>
                <small>顶栏、导航、导出</small>
              </span>
              <span class="m-route-arrow">→</span>
            </button>
          </div>
        </aside>
      </div>

      <div class="m-hero-foot">
        <span><b>{{ manualChapters.length }}</b> 章</span>
        <span><b>{{ manualStats.sections }}</b> 个小节</span>
        <span><b>{{ totalFigures }}</b> 张界面图</span>
        <span><b>{{ totalShortcuts }}</b> 组快捷键</span>
        <span class="m-hero-foot-hint">Ctrl+K 搜索 · Esc 关闭</span>
      </div>
    </header>

    <div class="m-body">
      <div class="m-content">
        <div v-if="showChapterStrip && !query.trim()" class="m-chapter-strip">
          <button
            v-for="chapter in manualChapters"
            :key="chapter.id"
            type="button"
            class="m-strip-item"
            :class="{ active: activeChapter === chapter.id && !query.trim() }"
            @click="goChapter(chapter.id)"
          >
            <span>{{ chapter.number }}</span>{{ chapter.title }}
          </button>
        </div>

        <section v-if="query.trim()" class="m-search-panel" aria-live="polite">
          <header class="m-search-head">
            <div>
              <div class="m-search-kicker">SEARCH</div>
              <h2 class="m-search-title">
                找到 <b>{{ searchResults.length }}</b> 处相关内容
              </h2>
              <p class="m-search-query">关键词：{{ query.trim() }}</p>
            </div>
            <button type="button" class="m-btn-ghost" @click="clearSearch">清除搜索</button>
          </header>

          <div v-if="searchResults.length" class="m-result-list">
            <button
              v-for="(result, index) in visibleResults"
              :key="result.sectionId"
              type="button"
              class="m-result"
              :class="{ active: index === activeResult }"
              @click="openResult(result)"
              @mouseenter="activeResult = index"
            >
              <span class="m-result-meta">
                <b>{{ result.chapterNumber }}</b>
                {{ result.chapterTitle }}
              </span>
              <span class="m-result-title" v-html="highlight(result.sectionTitle, searchTerms)"></span>
              <span class="m-result-snippet" v-html="highlight(result.snippet, searchTerms)"></span>
            </button>

            <button
              v-if="searchResults.length > resultLimit"
              type="button"
              class="m-btn-ghost m-result-more"
              @click="resultLimit += 30"
            >
              继续显示（还剩 {{ searchResults.length - resultLimit }} 条）
            </button>
          </div>

          <div v-else class="m-empty">
            <div class="m-empty-mark">∅</div>
            <p>没有找到“{{ query.trim() }}”。试试更短的关键词，或者从下面这些入口开始：</p>
            <div class="m-hot m-hot-center">
              <button
                v-for="chip in hotQueries"
                :key="chip"
                type="button"
                class="m-hot-chip"
                @click="applyQuery(chip)"
              >
                {{ chip }}
              </button>
            </div>
          </div>
        </section>

        <template v-else>
          <article v-for="chapter in manualChapters" :id="chapter.id" :key="chapter.id" class="m-chapter">
            <header class="m-chapter-head">
              <div class="m-chapter-no">{{ chapter.number }}</div>
              <div class="m-chapter-copy">
                <div class="m-chapter-eyebrow">{{ chapter.eyebrow }}</div>
                <h2 class="m-chapter-title">{{ chapter.title }}</h2>
                <p class="m-chapter-sub">{{ chapter.subtitle }}</p>
                <p class="m-chapter-summary">{{ chapter.summary }}</p>
              </div>
            </header>

            <section
              v-for="(section, sectionIndex) in chapter.sections"
              :id="section.id"
              :key="section.id"
              class="m-section"
              :class="{ 'is-flash': flashId === section.id }"
            >
              <h3 class="m-section-title">
                <span class="m-section-no">{{ sectionNumber(chapter, sectionIndex) }}</span>
                <span>{{ section.title }}</span>
              </h3>

              <template v-for="(block, blockIndex) in section.blocks" :key="`${section.id}-${blockIndex}`">
                <p v-if="block.type === 'lead'" class="m-lead" v-html="block.text"></p>
                <p v-else-if="block.type === 'p'" class="m-p" v-html="block.text"></p>
                <h4 v-else-if="block.type === 'h4'" class="m-h4" v-html="block.text"></h4>

                <ul v-else-if="block.type === 'list'" class="m-list">
                  <li v-for="(item, itemIndex) in block.items" :key="itemIndex" v-html="item"></li>
                </ul>

                <ol v-else-if="block.type === 'steps'" class="m-steps">
                  <li v-for="(item, itemIndex) in block.items" :key="itemIndex" class="m-step">
                    <span class="m-step-no">{{ pad2(itemIndex + 1) }}</span>
                    <div class="m-step-body">
                      <div class="m-step-title">{{ item.title }}</div>
                      <p class="m-step-text" v-html="item.text"></p>
                    </div>
                  </li>
                </ol>

                <div v-else-if="block.type === 'cards'" class="m-card-grid">
                  <component
                    :is="item.go ? 'button' : 'div'"
                    v-for="(item, itemIndex) in block.items"
                    :key="itemIndex"
                    class="m-card"
                    :class="{ 'is-link': item.go }"
                    :type="item.go ? 'button' : undefined"
                    @click="item.go ? goChapter(item.go) : undefined"
                  >
                    <span class="m-card-index">{{ item.index || pad2(itemIndex + 1) }}</span>
                    <span class="m-card-title">{{ item.title }}</span>
                    <span class="m-card-text" v-html="item.text"></span>
                    <span v-if="item.go" class="m-card-go">打开章节 →</span>
                  </component>
                </div>

                <div v-else-if="block.type === 'table'" class="m-table-wrap">
                  <table class="m-table">
                    <thead>
                      <tr>
                        <th v-for="(head, headIndex) in block.head" :key="headIndex">{{ head }}</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="(row, rowIndex) in block.rows" :key="rowIndex">
                        <td v-for="(cell, cellIndex) in row" :key="cellIndex" v-html="cell"></td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <div v-else-if="block.type === 'code'" class="m-code">
                  <div class="m-code-head">
                    <span>{{ block.lang || 'text' }}</span>
                    <button type="button" class="m-code-copy" @click="copyCode(block.text, `${section.id}-${blockIndex}`)">
                      {{ copiedKey === `${section.id}-${blockIndex}` ? '已复制' : '复制' }}
                    </button>
                  </div>
                  <pre><code>{{ block.text }}</code></pre>
                </div>

                <div v-else-if="block.type === 'callout'" class="m-callout" :class="`tone-${block.tone || 'note'}`">
                  <div class="m-callout-label">{{ calloutLabel(block.tone) }}</div>
                  <div class="m-callout-body">
                    <b v-if="block.title">{{ block.title }}</b>
                    <p v-html="block.text"></p>
                  </div>
                </div>

                <dl v-else-if="block.type === 'defs'" class="m-defs">
                  <div v-for="(item, itemIndex) in block.items" :key="itemIndex" class="m-def">
                    <dt>{{ item.term }}</dt>
                    <dd v-html="item.text"></dd>
                  </div>
                </dl>

                <div v-else-if="block.type === 'flow'" class="m-flow">
                  <div v-for="(item, itemIndex) in block.items" :key="itemIndex" class="m-flow-item">
                    <span class="m-flow-no">{{ pad2(itemIndex + 1) }}</span>
                    <span>{{ item }}</span>
                  </div>
                </div>

                <div v-else-if="block.type === 'shortcuts'" class="m-shortcuts">
                  <div v-for="(group, groupIndex) in block.groups" :key="groupIndex" class="m-shortcut-group">
                    <div class="m-shortcut-title">{{ group.title }}</div>
                    <div v-for="(row, rowIndex) in group.rows" :key="rowIndex" class="m-shortcut-row">
                      <span>{{ row[0] }}</span>
                      <kbd>{{ row[1] }}</kbd>
                    </div>
                  </div>
                </div>

                <figure v-else-if="block.type === 'figure'" class="m-figure">
                  <button
                    type="button"
                    class="m-figure-frame"
                    :class="{ 'is-missing': !figureUrl(block.image) || isFigureBroken(block.image) }"
                    @click="openPreview(block)"
                  >
                    <img
                      v-if="figureUrl(block.image) && !isFigureBroken(block.image)"
                      :src="figureUrl(block.image)"
                      :alt="block.caption"
                      loading="lazy"
                      @error="markFigureBroken(block.image)"
                    />
                    <span v-else class="m-figure-missing">
                      <b>截图待补</b>
                      <small>src/assets/manual/{{ block.image }}.png</small>
                    </span>
                    <span v-if="figureUrl(block.image) && !isFigureBroken(block.image)" class="m-figure-zoom">点击放大</span>
                  </button>
                  <figcaption>
                    <span class="m-figure-no">图 {{ figureNo(chapter, section, blockIndex) }}</span>
                    <span>{{ block.caption }}</span>
                  </figcaption>
                </figure>

                <div v-else-if="block.type === 'qa'" class="m-qa">
                  <div v-for="(item, itemIndex) in block.items" :key="itemIndex" class="m-qa-item">
                    <div class="m-q"><span>Q</span>{{ item.q }}</div>
                    <p class="m-a" v-html="item.a"></p>
                  </div>
                </div>
              </template>
            </section>
          </article>

          <footer class="m-foot">
            <div class="m-foot-rule"></div>
            <p>Guaxi 用户手册 · 按当前代码中的页面与入口整理</p>
            <p class="m-foot-meta">
              {{ manualChapters.length }} 章 / {{ manualStats.sections }} 节 · 界面截图随版本更新 · 最后整理 2026-11
            </p>
            <button type="button" class="m-btn-ghost" @click="scrollToTop">回到顶部 ↑</button>
          </footer>
        </template>
      </div>

    </div>

    <Teleport to="body">
      <div v-if="preview" class="m-lightbox" @click.self="closePreview">
        <button type="button" class="m-lightbox-close" @click="closePreview">关闭 · Esc</button>
        <img :src="preview.src" :alt="preview.caption" />
        <div class="m-lightbox-caption">{{ preview.caption }}</div>
      </div>
    </Teleport>
  </div>
</template>

<script setup>
import {
  computed,
  nextTick,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  reactive,
  ref,
  watch,
} from 'vue'
import { useSettingsStore } from '../../stores/settingsStore'
import { useSiderStore } from '../../stores/useSiderStore'
import { useManualStore } from '../../stores/manualStore.js'
import { createLogger } from '../../utils/logger'
import { manualChapters, manualSearchEntries, manualStats } from './manual/manualContent.js'

const logger = createLogger('GuaxiManual')
const settingsStore = useSettingsStore()
const siderStore = useSiderStore()
const manualStore = useManualStore()

const shellRef = ref(null)
const searchInput = ref(null)
const query = ref('')
const activeResult = ref(0)
const resultLimit = ref(30)
const layoutMode = ref('mid')
const flashId = ref('')
const copiedKey = ref('')
const preview = ref(null)
const brokenFigures = reactive(new Set())

const activeChapter = computed(() => manualStore.activeChapterId)
const scrollProgress = computed(() => manualStore.progress)
const showChapterStrip = computed(() => siderStore.isRightSiderCollapsed['setting'] !== false)

let scrollRoot = null
let resizeObserver = null
let flashTimer = 0
let copiedTimer = 0
let listenersAttached = false

const manualImages = import.meta.glob('../../assets/manual/*.{png,gif}', {
  eager: true,
  query: '?url',
  import: 'default',
})

const isDarkTheme = computed(() => {
  const storeTheme = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (storeTheme === 'dark') return true
  if (storeTheme === 'light') return false
  const dataTheme = document?.documentElement?.dataset?.guaxiTheme
  return dataTheme === 'dark'
})

const figureUrl = (key) => {
  const base = `../../assets/manual/${key}`
  const candidates = isDarkTheme.value
    ? [`${base}-dark.gif`, `${base}.gif`, `${base}-dark.png`, `${base}.png`, `${base}-light.png`]
    : [`${base}.gif`, `${base}.png`, `${base}-light.png`]
  for (const candidate of candidates) {
    if (manualImages[candidate]) return manualImages[candidate]
  }
  return ''
}

const figureSlot = (key) => `${key}:${isDarkTheme.value ? 'dark' : 'light'}`
const isFigureBroken = (key) => brokenFigures.has(figureSlot(key))
const markFigureBroken = (key) => {
  brokenFigures.add(figureSlot(key))
  logger.warn('[Manual] figure missing', { key, dark: isDarkTheme.value })
}

const figureNumbers = new Map()
let figureCount = 0
manualChapters.forEach((chapter) => {
  chapter.sections.forEach((section) => {
    section.blocks.forEach((block, blockIndex) => {
      if (block.type === 'figure') {
        figureCount += 1
        figureNumbers.set(`${chapter.id}/${section.id}/${blockIndex}`, figureCount)
      }
    })
  })
})

const totalFigures = figureCount
const totalShortcuts = manualChapters.reduce(
  (total, chapter) =>
    total +
    chapter.sections.reduce(
      (sectionTotal, section) =>
        sectionTotal +
        section.blocks.reduce(
          (blockTotal, block) =>
            blockTotal +
            (block.type === 'shortcuts'
              ? block.groups.reduce((groupTotal, group) => groupTotal + group.rows.length, 0)
              : 0),
          0,
        ),
      0,
    ),
  0,
)

const hotQueries = ['全词匹配', 'RSS', '同步', '插件', '快捷键', 'PDF']

const searchTerms = computed(() =>
  String(query.value || '')
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean),
)

const searchResults = computed(() => {
  const terms = searchTerms.value
  if (!terms.length) return []

  const results = []
  for (const entry of manualSearchEntries) {
    const haystack = entry.haystack.toLowerCase()
    if (!terms.every((term) => haystack.includes(term))) continue

    const title = entry.sectionTitle.toLowerCase()
    const chapterTitle = entry.chapterTitle.toLowerCase()
    const body = entry.body.toLowerCase()
    let score = 0
    if (terms.every((term) => title.includes(term))) score += 6
    if (terms.every((term) => chapterTitle.includes(term))) score += 3
    if (terms.every((term) => body.includes(term))) score += 2
    score += terms.reduce((total, term) => total + (title.includes(term) ? 1 : 0), 0)

    results.push({
      ...entry,
      score,
      snippet: buildSnippet(entry.body, terms),
    })
  }

  return results.sort((a, b) => {
    if (b.score !== a.score) return b.score - a.score
    if (a.chapterNumber !== b.chapterNumber) return a.chapterNumber.localeCompare(b.chapterNumber)
    return a.sectionTitle.localeCompare(b.sectionTitle, 'zh-CN')
  })
})

const visibleResults = computed(() => searchResults.value.slice(0, resultLimit.value))

const sectionNumber = (chapter, index) => `${chapter.number}.${pad2(index + 1)}`

const figureNo = (chapter, section, blockIndex) =>
  figureNumbers.get(`${chapter.id}/${section.id}/${blockIndex}`) || '—'

function pad2(value) {
  return String(value).padStart(2, '0')
}

function calloutLabel(tone) {
  if (tone === 'tip') return 'TIP'
  if (tone === 'warn') return 'WARN'
  return 'NOTE'
}

function buildSnippet(text, terms) {
  if (!text) return ''
  const lower = text.toLowerCase()
  let index = -1
  for (const term of terms) {
    const found = lower.indexOf(term)
    if (found >= 0 && (index < 0 || found < index)) index = found
  }
  if (index < 0) return text.slice(0, 130)
  const start = Math.max(0, index - 44)
  const end = Math.min(text.length, index + 104)
  return `${start > 0 ? '…' : ''}${text.slice(start, end)}${end < text.length ? '…' : ''}`
}

function escapeHtml(value) {
  return String(value || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

function escapeRegExp(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function highlight(text, terms) {
  const escaped = escapeHtml(text)
  if (!terms.length) return escaped
  const pattern = terms
    .map((term) => escapeRegExp(escapeHtml(term)))
    .filter(Boolean)
    .join('|')
  if (!pattern) return escaped
  return escaped.replace(new RegExp(`(${pattern})`, 'gi'), '<mark>$1</mark>')
}

function applyQuery(value) {
  query.value = value
  focusSearch()
}

function focusSearch() {
  searchInput.value?.focus?.()
  searchInput.value?.select?.()
}

function clearSearch() {
  query.value = ''
  nextTick(() => searchInput.value?.focus?.())
}

function onSearchKeydown(event) {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    activeResult.value = Math.min(activeResult.value + 1, Math.max(0, searchResults.value.length - 1))
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    activeResult.value = Math.max(0, activeResult.value - 1)
    return
  }
  if (event.key === 'Enter') {
    const result = searchResults.value[activeResult.value]
    if (result) {
      event.preventDefault()
      openResult(result)
    }
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    clearSearch()
  }
}

async function openResult(result) {
  logger.info('[Manual] open search result', {
    chapter: result.chapterId,
    section: result.sectionId,
    query: query.value,
  })
  query.value = ''
  await nextTick()
  await scrollToSection(result.sectionId)
}

async function goChapter(chapterId) {
  if (query.value) {
    query.value = ''
    await nextTick()
  }
  const chapter = manualChapters.find((item) => item.id === chapterId)
  logger.info('[Manual] navigate chapter', { chapterId, chapter: chapter?.title || '' })
  // 章节之间可能隔几万像素，平滑滚动会让人等好几秒；章节跳转直接到位，
  // 只有右侧“本章小节”这种近距离跳转才保留平滑动画。
  await scrollToSection(chapterId, { smooth: false })
}

async function scrollToSection(sectionId, options = {}) {
  const element = document.getElementById(sectionId)
  if (!element) {
    logger.warn('[Manual] section not found', { sectionId })
    return
  }
  element.scrollIntoView({ behavior: options.smooth === false ? 'auto' : 'smooth', block: 'start' })
  flashId.value = sectionId
  window.clearTimeout(flashTimer)
  flashTimer = window.setTimeout(() => {
    if (flashId.value === sectionId) flashId.value = ''
  }, 1800)
}

function scrollToTop() {
  if (scrollRoot?.scrollTo) {
    scrollRoot.scrollTo({ top: 0, behavior: 'smooth' })
  } else {
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }
}

function openPreview(block) {
  const src = figureUrl(block.image)
  if (!src || isFigureBroken(block.image)) return
  preview.value = { src, caption: block.caption }
}

function closePreview() {
  preview.value = null
}

async function copyCode(text, key) {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
  }
  copiedKey.value = key
  window.clearTimeout(copiedTimer)
  copiedTimer = window.setTimeout(() => {
    if (copiedKey.value === key) copiedKey.value = ''
  }, 1600)
}

function measureLayout() {
  const width = shellRef.value?.getBoundingClientRect?.().width || 0
  layoutMode.value = width >= 1080 ? 'wide' : width >= 760 ? 'mid' : 'narrow'
}

function updateScrollState() {
  const root = scrollRoot
  if (!root) return
  if (root === window) {
    const max = document.documentElement.scrollHeight - window.innerHeight
    manualStore.setProgress(max > 0 ? (window.scrollY / max) * 100 : 0)
  } else {
    const max = root.scrollHeight - root.clientHeight
    manualStore.setProgress(max > 0 ? (root.scrollTop / max) * 100 : 0)
  }

  if (query.value) return

  const rootTop = root === window ? 0 : root.getBoundingClientRect().top
  let current = manualChapters[0]?.id || ''
  for (const chapter of manualChapters) {
    const element = document.getElementById(chapter.id)
    if (!element) continue
    const top = element.getBoundingClientRect().top - rootTop
    if (top <= 150) current = chapter.id
  }
  manualStore.setActiveChapter(current)

  const currentChapter = manualChapters.find((chapter) => chapter.id === current)
  if (currentChapter) {
    let currentSection = currentChapter.sections[0]?.id || ''
    for (const section of currentChapter.sections) {
      const element = document.getElementById(section.id)
      if (!element) continue
      const top = element.getBoundingClientRect().top - rootTop
      if (top <= 190) currentSection = section.id
    }
    manualStore.setActiveSection(currentSection)
  }
}

function onWindowKeydown(event) {
  const key = String(event.key || '').toLowerCase()
  const withCommand = event.ctrlKey || event.metaKey
  if (withCommand && (key === 'k' || key === 'f')) {
    event.preventDefault()
    focusSearch()
    return
  }
  if (event.key === 'Escape') {
    if (preview.value) {
      event.preventDefault()
      closePreview()
      return
    }
    if (query.value) {
      event.preventDefault()
      clearSearch()
    }
  }
}

watch(query, (value) => {
  activeResult.value = 0
  resultLimit.value = 30
  // 从长文深处按 Ctrl+K 搜索时，手册还停在原来的滚动位置；检索态内容短得多，
  // 先回到顶部，让搜索框和结果标题同时可见。清空搜索不强制回顶，
  // 否则会和“打开某条结果后跳到对应小节”抢滚动位置。
  if (String(value || '').trim()) {
    nextTick(() => scrollRoot?.scrollTo?.({ top: 0, behavior: 'auto' }))
  }
})

watch(
  () => searchResults.value.length,
  (length) => {
    if (activeResult.value > length - 1) activeResult.value = Math.max(0, length - 1)
  },
)

// 右侧栏只发起“跳到某节”的请求，滚动仍由正文负责，避免两边各维护一套定位逻辑。
watch(
  () => manualStore.scrollRequest,
  (request) => {
    if (request?.sectionId) {
      scrollToSection(request.sectionId, { smooth: request.smooth })
    }
  },
)

watch(
  () => manualStore.searchFocusTick,
  () => {
    nextTick(() => {
      scrollRoot?.scrollTo?.({ top: 0, behavior: 'auto' })
      focusSearch()
    })
  },
)

function attachListeners() {
  if (listenersAttached) detachListeners()
  // 手册现在是设置内容区里唯一的滚动视口，不再需要向外找滚动祖先。
  scrollRoot = shellRef.value || window
  scrollRoot.addEventListener?.('scroll', updateScrollState, { passive: true })
  window.addEventListener('keydown', onWindowKeydown)
  window.addEventListener('resize', onViewportResize)
  resizeObserver?.disconnect?.()
  if (resizeObserver && shellRef.value) resizeObserver.observe(shellRef.value)
  listenersAttached = true
  measureLayout()
  updateScrollState()
}

function onViewportResize() {
  measureLayout()
  updateScrollState()
}

function detachListeners() {
  scrollRoot?.removeEventListener?.('scroll', updateScrollState)
  window.removeEventListener('keydown', onWindowKeydown)
  window.removeEventListener('resize', onViewportResize)
  resizeObserver?.disconnect?.()
  listenersAttached = false
}

onMounted(async () => {
  await nextTick()
  resizeObserver = new ResizeObserver(measureLayout)
  attachListeners()
  logger.info('[Manual] mounted', {
    chapters: manualChapters.length,
    sections: manualStats.sections,
    figures: totalFigures,
    theme: isDarkTheme.value ? 'dark' : 'light',
  })
})

// 设置页用 keep-alive 缓存各面板：手册失活后必须摘掉全局 Ctrl+K / Ctrl+F 监听，
// 否则在别的设置页按搜索会被这个隐藏的手册抢走。
onActivated(() => {
  nextTick(() => attachListeners())
})

onDeactivated(() => {
  detachListeners()
})

onBeforeUnmount(() => {
  detachListeners()
  resizeObserver = null
  window.clearTimeout(flashTimer)
  window.clearTimeout(copiedTimer)
})
</script>

<style scoped>
.manual-shell {
  /* 手册不再自带一套暖色纸感配色：直接跟随软件的亮/暗主题变量，
     强调色用 Naive 的主色，避免和顶部栏、左侧导航和表单控件割裂。 */
  --m-paper: var(--guaxi-content-bg);
  --m-surface: var(--guaxi-panel-bg);
  --m-surface-2: color-mix(in srgb, var(--guaxi-panel-bg) 88%, var(--guaxi-text-primary) 12%);
  --m-ink: var(--guaxi-text-primary);
  --m-muted: var(--guaxi-text-secondary);
  --m-line: var(--guaxi-border);
  --m-line-strong: color-mix(in srgb, var(--guaxi-border) 64%, var(--guaxi-text-secondary) 36%);
  --m-accent: #4078f2;
  --m-accent-ink: #4078f2;
  --m-accent-soft: color-mix(in srgb, var(--m-accent) 14%, transparent);
  --m-blue: var(--m-accent);
  --m-blue-soft: color-mix(in srgb, var(--m-blue) 11%, transparent);
  --m-code-bg: color-mix(in srgb, var(--guaxi-panel-bg) 84%, var(--guaxi-text-primary) 8%);
  --m-shadow: 0 12px 28px rgba(0, 0, 0, 0.14);
  --m-body-font: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --m-display: var(--m-body-font);
  --m-mono: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace);

  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--m-ink);
  font-family: var(--m-body-font);
  background-color: var(--m-paper);
  background-image:
    radial-gradient(900px 360px at 4% -8%, var(--m-accent-soft), transparent 60%),
    repeating-linear-gradient(0deg, transparent 0 31px, color-mix(in srgb, var(--m-ink) 4%, transparent) 31px 32px),
    repeating-linear-gradient(90deg, transparent 0 31px, color-mix(in srgb, var(--m-ink) 4%, transparent) 31px 32px);
  box-sizing: border-box;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: var(--m-line-strong) transparent;
}

.manual-shell.is-dark {
  --m-accent: #61afef;
  --m-accent-ink: #61afef;
  --m-shadow: 0 16px 38px rgba(0, 0, 0, 0.32);
}

.m-progress {
  position: sticky;
  top: 0;
  z-index: 30;
  height: 2px;
  width: 100%;
  transform-origin: left center;
  background: linear-gradient(90deg, var(--m-accent), var(--m-blue));
  transition: transform 0.08s linear;
}

/* ---------- 封面 ---------- */
.m-hero {
  position: relative;
  padding: 46px 44px 24px;
  border-bottom: 1px solid var(--m-line);
  overflow: hidden;
}

.m-hero::after {
  content: '';
  position: absolute;
  right: -80px;
  top: -120px;
  width: 360px;
  height: 360px;
  border-radius: 50%;
  border: 1px solid var(--m-line);
  opacity: 0.55;
  pointer-events: none;
}

.m-hero-grid {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 286px;
  gap: 38px;
  align-items: end;
}

.m-eyebrow {
  display: flex;
  align-items: center;
  gap: 9px;
  font-family: var(--m-mono);
  font-size: 11.5px;
  letter-spacing: 0.16em;
  color: var(--m-muted);
}

.m-eyebrow-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--m-accent);
  box-shadow: 0 0 0 4px var(--m-accent-soft);
}

.m-eyebrow-version {
  padding: 1px 7px;
  border: 1px solid var(--m-line-strong);
  border-radius: 999px;
  letter-spacing: 0.08em;
  color: var(--m-accent-ink);
}

.m-title {
  margin: 16px 0 0;
  font-family: var(--m-display);
  font-size: clamp(32px, 3.6vw, 52px);
  line-height: 1.14;
  letter-spacing: 0.015em;
  font-weight: 600;
  color: var(--m-ink);
}

.m-title em {
  font-style: normal;
  color: var(--m-accent);
  white-space: nowrap;
  background: linear-gradient(transparent 70%, var(--m-accent-soft) 70%);
}

.m-hero-lead {
  max-width: 640px;
  margin: 14px 0 0;
  font-size: 14.5px;
  line-height: 1.85;
  color: var(--m-muted);
}

.m-search {
  display: flex;
  align-items: center;
  gap: 11px;
  margin-top: 22px;
  padding: 0 14px;
  height: 54px;
  border: 1px solid var(--m-line-strong);
  border-radius: 14px;
  background: var(--m-surface);
  box-shadow: var(--m-shadow);
  transition: border-color 0.18s ease, box-shadow 0.18s ease;
}

.m-search:focus-within {
  border-color: var(--m-accent);
  box-shadow: 0 0 0 4px var(--m-accent-soft), var(--m-shadow);
}

.m-search-icon {
  flex: 0 0 auto;
  width: 19px;
  height: 19px;
  fill: none;
  stroke: var(--m-accent);
  stroke-width: 1.9;
  stroke-linecap: round;
}

.m-search-input {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--m-ink);
  font: inherit;
  font-size: 14px;
}

.m-search-input::placeholder {
  color: var(--m-muted);
  opacity: 0.82;
}

.m-search-input::-webkit-search-cancel-button {
  display: none;
}

.m-search-kbd {
  flex: 0 0 auto;
  padding: 3px 8px;
  border: 1px solid var(--m-line);
  border-radius: 6px;
  background: var(--m-surface-2);
  color: var(--m-muted);
  font-family: var(--m-mono);
  font-size: 11px;
}

.m-search-clear {
  flex: 0 0 auto;
  border: 0;
  background: transparent;
  color: var(--m-accent);
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
}

.m-hot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 7px;
  margin-top: 12px;
}

.m-hot-center {
  justify-content: center;
}

.m-hot-label {
  font-family: var(--m-mono);
  font-size: 11px;
  letter-spacing: 0.12em;
  color: var(--m-muted);
}

.m-hot-chip {
  padding: 3px 10px;
  border: 1px solid var(--m-line);
  border-radius: 999px;
  background: transparent;
  color: var(--m-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: color 0.16s ease, border-color 0.16s ease, background 0.16s ease;
}

.m-hot-chip:hover {
  color: var(--m-accent-ink);
  border-color: var(--m-accent);
  background: var(--m-accent-soft);
}

.m-route-card {
  padding: 18px;
  border: 1px solid color-mix(in srgb, var(--m-accent) 32%, var(--m-line));
  border-radius: 14px;
  background: color-mix(in srgb, var(--m-accent) 8%, var(--m-surface));
  color: var(--m-ink);
}

.m-route-title {
  margin-bottom: 10px;
  font-family: var(--m-mono);
  font-size: 11px;
  letter-spacing: 0.16em;
  opacity: 0.62;
}

.m-route-item {
  display: flex;
  align-items: center;
  gap: 11px;
  width: 100%;
  padding: 10px 8px;
  border: 0;
  border-top: 1px solid var(--m-line);
  background: transparent;
  color: inherit;
  text-align: left;
  font: inherit;
  cursor: pointer;
  transition: background 0.16s ease, padding-left 0.16s ease;
}

.m-route-item:hover {
  padding-left: 12px;
  background: var(--m-surface-2);
}

.m-route-no {
  flex: 0 0 auto;
  font-family: var(--m-mono);
  font-size: 10.5px;
  opacity: 0.55;
}

.m-route-main {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.m-route-main b {
  font-size: 13px;
  font-weight: 600;
}

.m-route-main small {
  font-size: 11px;
  opacity: 0.6;
}

.m-route-arrow {
  flex: 0 0 auto;
  color: var(--m-accent);
  transition: transform 0.16s ease;
}

.m-route-item:hover .m-route-arrow {
  transform: translateX(3px);
}

.m-hero-foot {
  display: flex;
  flex-wrap: wrap;
  gap: 22px;
  margin-top: 28px;
  padding-top: 14px;
  border-top: 1px solid var(--m-line);
  font-family: var(--m-mono);
  font-size: 11.5px;
  color: var(--m-muted);
}

.m-hero-foot b {
  margin-right: 3px;
  color: var(--m-ink);
  font-size: 14px;
  font-weight: 600;
}

.m-hero-foot-hint {
  margin-left: auto;
}

/* ---------- 主体两栏 ---------- */
.m-body {
  display: block;
}

/* 检索态：收起封面装饰，让结果区紧贴搜索框。 */
.manual-shell.is-searching .m-hero-grid {
  grid-template-columns: minmax(0, 1fr);
}

.manual-shell.is-searching .m-hero-aside,
.manual-shell.is-searching .m-hero-foot {
  display: none;
}

.manual-shell.is-searching .m-hero {
  padding-bottom: 20px;
}

.m-content {
  --m-content-pad-x: 44px;
  --m-content-pad-y: 32px;
  min-width: 0;
  padding: var(--m-content-pad-y) var(--m-content-pad-x) 64px;
}

.layout-mid .m-content {
  --m-content-pad-x: 30px;
  --m-content-pad-y: 26px;
  padding: var(--m-content-pad-y) var(--m-content-pad-x) 58px;
}

.layout-narrow .m-content {
  --m-content-pad-x: 20px;
  --m-content-pad-y: 20px;
  padding: var(--m-content-pad-y) var(--m-content-pad-x) 52px;
}

.m-chapter-strip {
  position: sticky;
  top: 2px;
  z-index: 12;
  display: flex;
  gap: 6px;
  margin: calc(var(--m-content-pad-y) * -1) calc(var(--m-content-pad-x) * -1) 22px;
  padding: 8px var(--m-content-pad-x);
  overflow-x: auto;
  background: color-mix(in srgb, var(--m-paper) 92%, transparent);
  border-bottom: 1px solid var(--m-line);
  backdrop-filter: blur(8px);
  scrollbar-width: none;
}

.m-chapter-strip::-webkit-scrollbar {
  display: none;
}

.m-strip-item {
  flex: 0 0 auto;
  padding: 4px 9px;
  border: 1px solid var(--m-line);
  border-radius: 999px;
  background: var(--m-surface);
  color: var(--m-muted);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
}

.m-strip-item span {
  margin-right: 4px;
  font-family: var(--m-mono);
  font-size: 9.5px;
  opacity: 0.6;
}

.m-strip-item.active {
  color: var(--m-accent-ink);
  border-color: var(--m-accent);
  background: var(--m-accent-soft);
}

/* ---------- 章 / 节 ---------- */
.m-chapter {
  scroll-margin-top: 14px;
  padding-bottom: 18px;
  margin-bottom: 46px;
}

.m-chapter:last-of-type {
  margin-bottom: 20px;
}

.m-chapter-head {
  display: grid;
  grid-template-columns: 78px minmax(0, 1fr);
  gap: 18px;
  padding-bottom: 20px;
  border-bottom: 2px solid var(--m-ink);
}

.m-chapter-no {
  font-family: var(--m-display);
  font-size: 58px;
  line-height: 0.9;
  color: var(--m-accent);
  opacity: 0.9;
  letter-spacing: -0.04em;
}

.m-chapter-eyebrow {
  font-family: var(--m-mono);
  font-size: 10.5px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--m-muted);
}

.m-chapter-title {
  margin: 6px 0 0;
  font-family: var(--m-display);
  font-size: clamp(26px, 2.5vw, 36px);
  line-height: 1.2;
  font-weight: 600;
  color: var(--m-ink);
}

.m-chapter-sub {
  margin: 8px 0 0;
  font-size: 14px;
  line-height: 1.7;
  color: var(--m-muted);
}

.m-chapter-summary {
  margin: 10px 0 0;
  max-width: 760px;
  font-size: 13px;
  line-height: 1.85;
  color: var(--m-muted);
}

.m-section {
  scroll-margin-top: 14px;
  margin-top: 30px;
  padding-top: 20px;
  border-top: 1px dashed var(--m-line);
  transition: background 0.3s ease, box-shadow 0.3s ease;
}

.m-section.is-flash {
  background: var(--m-accent-soft);
  box-shadow: -14px 0 0 var(--m-accent-soft), 14px 0 0 var(--m-accent-soft);
}

.m-section-title {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin: 0;
  font-family: var(--m-display);
  font-size: 21px;
  line-height: 1.35;
  font-weight: 600;
  color: var(--m-ink);
}

.m-section-no {
  flex: 0 0 auto;
  font-family: var(--m-mono);
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.04em;
  color: var(--m-accent);
}

.m-lead {
  margin: 16px 0 0;
  padding-left: 14px;
  border-left: 3px solid var(--m-accent);
  font-size: 17px;
  line-height: 1.9;
  color: var(--m-ink);
}

.m-p {
  margin: 12px 0 0;
  font-size: 14.5px;
  line-height: 1.9;
  color: var(--m-ink);
}

.m-h4 {
  margin: 22px 0 0;
  font-family: var(--m-display);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.5;
  color: var(--m-ink);
}

/* 正文大量使用 v-html（加粗、行内代码、快捷键），这些子元素没有 scoped 属性，
   必须用 :deep() 才能吃到手册自己的配色，而不是浏览器默认的 mark / code 样式。 */
.m-p :deep(b),
.m-list :deep(b),
.m-lead :deep(b),
.m-step-text :deep(b),
.m-a :deep(b),
.m-callout :deep(b) {
  color: var(--m-accent-ink);
  font-weight: 600;
}

.m-p :deep(code),
.m-list :deep(code),
.m-lead :deep(code),
.m-step-text :deep(code),
.m-a :deep(code),
.m-callout :deep(code),
.m-def :deep(code),
.m-table :deep(code) {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--m-code-bg);
  border: 1px solid var(--m-line);
  font-family: var(--m-mono);
  font-size: 0.92em;
}

.m-p :deep(kbd),
.m-list :deep(kbd),
.m-lead :deep(kbd),
.m-callout :deep(kbd),
.m-step-text :deep(kbd),
.m-a :deep(kbd) {
  display: inline-block;
  padding: 0 5px;
  border: 1px solid var(--m-line-strong);
  border-bottom-width: 2px;
  border-radius: 4px;
  background: var(--m-surface);
  font-family: var(--m-mono);
  font-size: 0.88em;
  line-height: 1.6;
  color: var(--m-ink);
}

.m-list {
  margin: 10px 0 0;
  padding-left: 0;
  list-style: none;
}

.m-list li {
  position: relative;
  margin: 7px 0;
  padding-left: 18px;
  font-size: 14px;
  line-height: 1.85;
  color: var(--m-ink);
}

.m-list li::before {
  content: '';
  position: absolute;
  left: 2px;
  top: 0.78em;
  width: 6px;
  height: 6px;
  border-radius: 1px;
  background: var(--m-accent);
  transform: rotate(45deg);
  opacity: 0.85;
}

/* 步骤 */
.m-steps {
  margin: 14px 0 0;
  padding: 0;
  list-style: none;
}

.m-step {
  position: relative;
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  gap: 12px;
  padding-bottom: 18px;
}

.m-step:not(:last-child)::before {
  content: '';
  position: absolute;
  left: 16px;
  top: 34px;
  bottom: 0;
  width: 1px;
  background: var(--m-line-strong);
}

.m-step-no {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 1px solid var(--m-line-strong);
  background: var(--m-surface);
  color: var(--m-accent);
  font-family: var(--m-mono);
  font-size: 11px;
  font-weight: 600;
}

.m-step-title {
  margin-top: 5px;
  font-size: 14px;
  font-weight: 600;
  color: var(--m-ink);
}

.m-step-text {
  margin: 4px 0 0;
  font-size: 13.5px;
  line-height: 1.85;
  color: var(--m-muted);
}

/* 卡片 */
.m-card-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 14px;
}

.m-card {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 15px;
  border: 1px solid var(--m-line);
  border-radius: 12px;
  background: var(--m-surface);
  color: var(--m-ink);
  text-align: left;
  font: inherit;
  transition: transform 0.18s ease, border-color 0.18s ease, box-shadow 0.18s ease;
}

.m-card.is-link {
  cursor: pointer;
}

.m-card:hover {
  transform: translateY(-2px);
  border-color: var(--m-line-strong);
  box-shadow: var(--m-shadow);
}

.m-card-index {
  font-family: var(--m-display);
  font-size: 20px;
  line-height: 1;
  color: var(--m-accent);
}

.m-card-title {
  margin-top: 4px;
  font-size: 14px;
  font-weight: 700;
  color: var(--m-ink);
}

.m-card-text {
  font-size: 12.5px;
  line-height: 1.75;
  color: var(--m-muted);
}

.m-card-go {
  margin-top: 6px;
  font-family: var(--m-mono);
  font-size: 10.5px;
  color: var(--m-accent);
}

/* 表格 */
.m-table-wrap {
  margin-top: 14px;
  border: 1px solid var(--m-line);
  border-radius: 12px;
  background: var(--m-surface);
  overflow-x: auto;
}

.m-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.8px;
  line-height: 1.7;
}

.m-table th {
  padding: 9px 12px;
  border-bottom: 1px solid var(--m-line-strong);
  background: var(--m-surface-2);
  color: var(--m-muted);
  text-align: left;
  font-weight: 600;
  white-space: nowrap;
}

.m-table td {
  padding: 9px 12px;
  border-bottom: 1px solid var(--m-line);
  color: var(--m-ink);
  vertical-align: top;
}

.m-table tr:last-child td {
  border-bottom: 0;
}

.m-table tbody tr {
  transition: background 0.14s ease;
}

.m-table tbody tr:hover {
  background: var(--m-surface-2);
}

/* 代码 */
.m-code {
  margin-top: 14px;
  border: 1px solid var(--m-line);
  border-radius: 12px;
  background: var(--m-code-bg);
  overflow: hidden;
}

.m-code-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 7px 12px;
  border-bottom: 1px solid var(--m-line);
  font-family: var(--m-mono);
  font-size: 10.5px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--m-muted);
}

.m-code-copy {
  border: 0;
  background: transparent;
  color: var(--m-accent);
  font-family: var(--m-mono);
  font-size: 10.5px;
  cursor: pointer;
}

.m-code pre {
  margin: 0;
  padding: 14px 16px;
  overflow-x: auto;
}

.m-code code {
  font-family: var(--m-mono);
  font-size: 12.4px;
  line-height: 1.75;
  color: var(--m-ink);
  white-space: pre;
}

/* 提示 */
.m-callout {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  gap: 12px;
  margin-top: 16px;
  padding: 13px 15px;
  border: 1px solid var(--m-line);
  border-left: 3px solid var(--m-blue);
  border-radius: 10px;
  background: var(--m-blue-soft);
}

.m-callout.tone-tip {
  border-left-color: var(--m-accent);
  background: var(--m-accent-soft);
}

.m-callout.tone-warn {
  border-left-color: #b45309;
  background: rgba(180, 83, 9, 0.08);
}

.is-dark .m-callout.tone-warn {
  border-left-color: #f0a35e;
  background: rgba(240, 163, 94, 0.1);
}

.m-callout-label {
  padding-top: 2px;
  font-family: var(--m-mono);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.14em;
  color: var(--m-accent);
}

.m-callout.tone-note .m-callout-label {
  color: var(--m-blue);
}

.m-callout.tone-warn .m-callout-label {
  color: #b45309;
}

.is-dark .m-callout.tone-warn .m-callout-label {
  color: #f0a35e;
}

.m-callout-body b {
  display: block;
  margin-bottom: 4px;
  font-size: 13px;
  color: var(--m-ink);
}

.m-callout-body p {
  margin: 0;
  font-size: 12.8px;
  line-height: 1.8;
  color: var(--m-muted);
}

/* 定义 */
.m-defs {
  display: grid;
  gap: 0;
  margin: 14px 0 0;
  border-top: 1px solid var(--m-line);
}

.m-def {
  display: grid;
  grid-template-columns: 144px minmax(0, 1fr);
  gap: 16px;
  padding: 10px 0;
  border-bottom: 1px solid var(--m-line);
}

.m-def dt {
  font-size: 12.8px;
  font-weight: 700;
  color: var(--m-accent-ink);
}

.m-def dd {
  margin: 0;
  font-size: 13px;
  line-height: 1.8;
  color: var(--m-muted);
}

/* 流程 */
.m-flow {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.m-flow-item {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 8px 12px;
  border: 1px solid var(--m-line);
  border-radius: 999px;
  background: var(--m-surface);
  font-size: 12.5px;
  color: var(--m-ink);
}

.m-flow-item:not(:last-child)::after {
  content: '→';
  margin-left: 6px;
  color: var(--m-accent);
}

.m-flow-no {
  font-family: var(--m-mono);
  font-size: 9.5px;
  color: var(--m-accent);
}

/* 快捷键 */
.m-shortcuts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 14px;
}

.m-shortcut-group {
  padding: 12px 14px;
  border: 1px solid var(--m-line);
  border-radius: 12px;
  background: var(--m-surface);
}

.m-shortcut-title {
  margin-bottom: 6px;
  font-size: 12.5px;
  font-weight: 700;
  color: var(--m-ink);
}

.m-shortcut-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  padding: 4px 0;
  border-top: 1px dashed var(--m-line);
  font-size: 12px;
  color: var(--m-muted);
}

.m-shortcut-row kbd {
  flex: 0 0 auto;
  padding: 1px 7px;
  border: 1px solid var(--m-line-strong);
  border-bottom-width: 2px;
  border-radius: 5px;
  background: var(--m-surface-2);
  font-family: var(--m-mono);
  font-size: 11px;
  color: var(--m-ink);
  white-space: nowrap;
}

/* 截图 */
.m-figure {
  margin: 18px 0 0;
}

.m-figure-frame {
  position: relative;
  display: block;
  width: 100%;
  padding: 10px;
  border: 1px solid var(--m-line);
  border-radius: 14px;
  background:
    linear-gradient(135deg, rgba(127, 127, 127, 0.06), transparent 42%),
    var(--m-surface);
  cursor: zoom-in;
  overflow: hidden;
  transition: border-color 0.18s ease, box-shadow 0.18s ease;
}

.m-figure-frame:hover {
  border-color: var(--m-line-strong);
  box-shadow: var(--m-shadow);
}

.m-figure-frame.is-missing {
  cursor: default;
  min-height: 180px;
}

.m-figure-frame img {
  /* 图片宽度跟随图框，高度按比例自然增长。不要给图片加 max-height：
     一旦高度被封顶，contain 会在两侧补出空白，窗口/右侧栏变化时看起来
     像图片区域没有跟着变宽。长截图就让它自然变长，由手册整体滚动。 */
  display: block;
  width: 100%;
  height: auto;
  border-radius: 8px;
  background: var(--m-surface-2);
}

.m-figure-missing {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 160px;
  color: var(--m-muted);
}

.m-figure-missing b {
  font-size: 13px;
  color: var(--m-ink);
}

.m-figure-missing small {
  font-family: var(--m-mono);
  font-size: 10.5px;
  word-break: break-all;
}

.m-figure-zoom {
  position: absolute;
  right: 18px;
  bottom: 18px;
  padding: 3px 9px;
  border-radius: 999px;
  background: rgba(15, 17, 20, 0.62);
  color: #fff;
  font-size: 10.5px;
  opacity: 0;
  transition: opacity 0.18s ease;
}

.m-figure-frame:hover .m-figure-zoom {
  opacity: 1;
}

.m-figure figcaption {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-top: 8px;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--m-muted);
}

.m-figure-no {
  flex: 0 0 auto;
  font-family: var(--m-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  color: var(--m-accent);
}

/* FAQ */
.m-qa {
  margin-top: 14px;
}

.m-qa-item {
  padding: 14px 0;
  border-bottom: 1px solid var(--m-line);
}

.m-qa-item:first-child {
  border-top: 1px solid var(--m-line);
}

.m-q {
  display: flex;
  gap: 8px;
  font-size: 14px;
  font-weight: 700;
  line-height: 1.6;
  color: var(--m-ink);
}

.m-q span {
  flex: 0 0 auto;
  font-family: var(--m-display);
  color: var(--m-accent);
}

.m-a {
  margin: 6px 0 0 20px;
  font-size: 13.5px;
  line-height: 1.9;
  color: var(--m-muted);
}

/* 搜索结果 */
.m-search-panel {
  min-height: 460px;
}

.m-search-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  padding-bottom: 18px;
  border-bottom: 2px solid var(--m-ink);
}

.m-search-kicker {
  font-family: var(--m-mono);
  font-size: 10.5px;
  letter-spacing: 0.18em;
  color: var(--m-accent);
}

.m-search-title {
  margin: 6px 0 0;
  font-family: var(--m-display);
  font-size: 28px;
  font-weight: 600;
  color: var(--m-ink);
}

.m-search-title b {
  color: var(--m-accent);
}

.m-search-query {
  margin: 6px 0 0;
  font-family: var(--m-mono);
  font-size: 11.5px;
  color: var(--m-muted);
}

.m-btn-ghost {
  flex: 0 0 auto;
  padding: 6px 12px;
  border: 1px solid var(--m-line-strong);
  border-radius: 999px;
  background: var(--m-surface);
  color: var(--m-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: color 0.16s ease, border-color 0.16s ease;
}

.m-btn-ghost:hover {
  color: var(--m-accent-ink);
  border-color: var(--m-accent);
}

.m-result-list {
  margin-top: 8px;
}

.m-result {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 100%;
  padding: 13px 14px;
  border: 0;
  border-left: 2px solid transparent;
  border-bottom: 1px solid var(--m-line);
  background: transparent;
  color: var(--m-ink);
  text-align: left;
  font: inherit;
  cursor: pointer;
  transition: background 0.14s ease, border-color 0.14s ease;
}

.m-result:hover,
.m-result.active {
  border-left-color: var(--m-accent);
  background: var(--m-surface-2);
}

.m-result-meta {
  font-family: var(--m-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  color: var(--m-muted);
}

.m-result-meta b {
  margin-right: 4px;
  color: var(--m-accent);
}

.m-result-title {
  font-family: var(--m-display);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.5;
}

.m-result-snippet {
  font-size: 12.5px;
  line-height: 1.8;
  color: var(--m-muted);
}

.m-result :deep(mark) {
  padding: 0 2px;
  border-radius: 3px;
  background: var(--m-accent-soft);
  color: var(--m-accent-ink);
  box-shadow: inset 0 -1px 0 var(--m-accent);
}

.m-result-more {
  display: block;
  margin: 18px auto 0;
}

.m-empty {
  padding: 56px 20px;
  text-align: center;
  color: var(--m-muted);
}

.m-empty-mark {
  font-family: var(--m-display);
  font-size: 54px;
  line-height: 1;
  color: var(--m-accent);
  opacity: 0.55;
}

.m-empty p {
  max-width: 480px;
  margin: 12px auto 0;
  font-size: 13px;
  line-height: 1.9;
}

/* 页脚 */
.m-foot {
  margin-top: 20px;
  padding-top: 10px;
  text-align: center;
  color: var(--m-muted);
}

.m-foot-rule {
  height: 10px;
  margin-bottom: 16px;
  border-top: 1px solid var(--m-line-strong);
  border-bottom: 1px solid var(--m-line);
}

.m-foot p {
  margin: 4px 0;
  font-size: 12px;
  line-height: 1.8;
}

.m-foot-meta {
  font-family: var(--m-mono);
  font-size: 10.5px;
  opacity: 0.8;
}

.m-foot .m-btn-ghost {
  margin-top: 12px;
}

/* 灯箱 */
.m-lightbox {
  position: fixed;
  inset: 0;
  z-index: 4000;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px;
  background: rgba(10, 11, 13, 0.86);
  backdrop-filter: blur(6px);
}

.m-lightbox img {
  max-width: min(1400px, 92vw);
  max-height: 82vh;
  object-fit: contain;
  border-radius: 10px;
  box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5);
  background: #fff;
}

.m-lightbox-close {
  position: absolute;
  top: 18px;
  right: 22px;
  padding: 6px 12px;
  border: 1px solid rgba(255, 255, 255, 0.28);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.m-lightbox-caption {
  max-width: 80vw;
  color: rgba(255, 255, 255, 0.72);
  font-size: 12.5px;
  line-height: 1.7;
  text-align: center;
}

/* 布局收窄 */
.layout-mid .m-hero-grid,
.layout-narrow .m-hero-grid {
  grid-template-columns: minmax(0, 1fr);
  gap: 22px;
}

.layout-mid .m-route-card,
.layout-narrow .m-route-card {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0 14px;
}

.layout-mid .m-route-title,
.layout-narrow .m-route-title {
  grid-column: 1 / -1;
}

.layout-mid .m-hero,
.layout-narrow .m-hero {
  padding: 32px 26px 20px;
}

.layout-narrow .m-hero {
  padding: 28px 20px 18px;
}

.layout-mid .m-title,
.layout-narrow .m-title {
  font-size: clamp(28px, 5vw, 40px);
}

.layout-mid .m-search,
.layout-narrow .m-search {
  height: 50px;
}

.layout-narrow .m-chapter-head {
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 12px;
}

.layout-narrow .m-chapter-no {
  font-size: 42px;
}

.layout-narrow .m-card-grid,
.layout-narrow .m-shortcuts {
  grid-template-columns: minmax(0, 1fr);
}

.layout-narrow .m-def {
  grid-template-columns: minmax(0, 1fr);
  gap: 3px;
}

.layout-narrow .m-callout {
  grid-template-columns: minmax(0, 1fr);
  gap: 5px;
}

.layout-narrow .m-route-card {
  grid-template-columns: minmax(0, 1fr);
}

.layout-narrow .m-search-head {
  align-items: flex-start;
  flex-direction: column;
  gap: 10px;
}

.layout-narrow .m-search-title {
  font-size: 23px;
}

.layout-narrow .m-figure-frame img {
  height: auto;
}

.layout-narrow .m-lightbox {
  padding: 16px;
}

.layout-narrow .m-hero-foot-hint {
  display: none;
}

@media (max-width: 560px) {
  .m-hero-foot {
    gap: 14px;
  }

  .m-title {
    font-size: 28px;
  }

  .m-content {
    padding-left: 16px;
    padding-right: 16px;
  }
}
</style>
