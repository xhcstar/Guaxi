<template>
  <!-- 外层包裹：用于承载文章内容插槽 + 选择工具条 + 笔记面板 -->
  <div class="selection-notes-wrapper">
    <!-- 文章内容插槽区域
         注意：外层页面需在插槽里的根元素上挂 @mouseup="selectionNotesRef.handleSelectionMouseUp()" -->
    <div class="selection-content-slot">
      <slot />
    </div>

    <!-- 选择工具条：基于 PDF 阅读器的简化版本（只保留“插入笔记”逻辑） -->
    <div v-if="selectionToolbarVisible" class="selection-toolbar"
      :style="{ top: `${selectionToolbarTop}px`, left: `${selectionToolbarLeft}px` }" @mousedown.stop @click.stop
      @dblclick.stop>
      <div class="selection-toolbar-actions">
        <!-- 插入笔记按钮：将当前 selectionInsertText 插入到笔记中 -->
        <button class="selection-tool-btn" type="button" title="插入笔记" :disabled="selectionBusy || !selectionInsertText"
          @click="insertSelectionResult">
          + 笔记
        </button>
      </div>
    </div>

    <!-- 笔记面板：与阅读器一致的“我的笔记”布局 -->
    <div class="notes-panel" :class="{ 'notes-panel-visible': showNotes }">
      <div class="notes-header">
        <h3>我的笔记</h3>
        <button @click="toggleNotesPanel" class="close-btn" type="button">
          关闭
        </button>
      </div>

      <div class="notes-content">
        <n-card v-for="(card, index) in noteCards" :key="card.id" class="note-card" size="small" :title="card.title"
          :bordered="true" :segmented="{ content: true, footer: 'soft' }" header-style="padding: 12px 16px;"
          content-style="padding: 0;">
          <template #header-extra>
            <div class="card-actions" v-if="activeCardIndex === index">
              <n-button size="tiny" type="primary" @click.stop="saveCardEdit(index)" title="保存" :disabled="selectionBusy">
                <template #icon>
                  <n-icon :component="Save" />
                </template>
              </n-button>
              <n-button size="tiny" tertiary @click.stop="exitEditMode(index)" title="退出编辑">
                <template #icon>
                  <n-icon :component="Close" />
                </template>
              </n-button>
              <n-button size="tiny" type="error" tertiary @click.stop="deleteCard(index)" title="删除">
                <template #icon>
                  <n-icon :component="Trash" />
                </template>
              </n-button>
            </div>
          </template>

          <!-- 非复合笔记：展示模式 -->
          <div v-if="card.type !== 'composite' && activeCardIndex !== index" class="note-content"
            :contenteditable="false" v-html="card.content" @click="enterEditMode(index)"></div>

          <!-- 非复合笔记：编辑模式 -->
          <div v-else-if="card.type !== 'composite' && activeCardIndex === index" class="note-content">
            <textarea class="note-editor" v-model="card.content" @keydown.meta.s.prevent="saveCardEdit(index)"
              @keydown.ctrl.s.prevent="saveCardEdit(index)"></textarea>
          </div>

          <!-- 复合笔记渲染：这里简化为直接显示 content 字段 -->
          <div v-else class="note-content" @click="enterEditMode(index)">
            <pre class="composite-note-content">{{ card.content }}</pre>
          </div>

          <template #footer>
            <n-text depth="3" style="font-size: 12px;">
              {{ formatTime(card.timestamp) }}
            </n-text>
          </template>
        </n-card>

        <div class="add-card-container">
          <n-button type="primary" dashed block @click="addNewCard">
            <template #icon>
              <n-icon :component="Add" />
            </template>
            添加新卡片
          </n-button>
        </div>
      </div>
    </div>

    <!-- 添加笔记的提示气泡 -->
    <transition name="fade">
      <div v-if="showNoteBubble" class="note-added-bubble">
        <div class="bubble-content">
          <span>已添加到笔记</span>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
/**
 * SelectionNotesPanel
 * -------------------
 * 复用 PDF 阅读器中的「选择工具条 + 笔记面板 + addToNotes 插入逻辑」，
 * 但针对 Flipboard 文章做了轻量化和本地存储适配。
 *
 * 使用方式：
 *  1. 在父组件中引入并使用 <SelectionNotesPanel> 包裹文章内容
 *  2. 父组件拿到 ref="selectionNotesRef"
 *  3. 在文章内容的 .article-content 上挂：@mouseup="selectionNotesRef.handleSelectionMouseUp()"
 */

import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join } from '@tauri-apps/api/path'
import { NCard, NButton, NIcon, NText, useMessage } from 'naive-ui'
import { AddOutline as Add, SaveOutline as Save, CloseOutline as Close, TrashOutline as Trash } from '@vicons/ionicons5'
import { useFlipboardCollectionsStore } from '../../stores/flipboardCollectionsStore.js'

const logger = createLogger('SelectionNotesPanel')
const collections = useFlipboardCollectionsStore()
/**
 * props.articleId     用于区分每篇文章的笔记存储 key
 * props.articleTitle  用于生成默认卡片标题
 */
const props = defineProps({
  articleId: {
    type: [String, Number],
    required: true
  },
  articleTitle: {
    type: String,
    default: ''
  },
  sourceUrl: {
    type: String,
    default: ''
  },
  category: {
    type: String,
    default: ''
  }
})

const message = useMessage()

// 是否展示右侧笔记面板
const showNotes = ref(false)
// 笔记卡片列表（结构与阅读器保持一致：id/type/title/timestamp/items）
const noteCards = ref([])
// 当前正在编辑的卡片索引，-1 表示没有卡片处于编辑状态
const activeCardIndex = ref(-1)
// 是否展示“已添加到笔记”的气泡提示
const showNoteBubble = ref(false)

// 选择工具条相关状态
const selectionToolbarVisible = ref(false)
const selectionToolbarTop = ref(0)
const selectionToolbarLeft = ref(0)
const selectionText = ref('')
const selectionInsertText = ref('')
const selectionBusy = ref(false)

/**
 * 笔记文件路径：每篇文章单独一个 JSON 文件
 * - 目标位置：可执行文件同级目录 /Fliprepo/flipboard-notes/
 * - 文件名：<articleId>.json
 */
const notesFilePath = ref('')

/**
 * 将 articleId 转成可用作文件名的安全字符串（Windows 兼容）
 * - 禁止字符：<>:"/\\|?* 以及控制字符
 * - 同时避免把 URL 的 / 或 \ 变成目录层级
 */
const toSafeFileName = (value) => {
  const raw = String(value ?? '').trim()
  if (!raw) return ''

  return raw
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, '_')
    .replace(/[\s]+/g, ' ')
    .replace(/[. ]+$/g, '')
    .slice(0, 180)
}

/**
 * 初始化并确保笔记文件所在目录存在
 */
const ensureNotesFilePath = async () => {
  if (notesFilePath.value) return notesFilePath.value

  const rawArticleId = String(props.articleId)
  const safeArticleId = toSafeFileName(rawArticleId)
  const fileBaseName = safeArticleId || `article_${Date.now()}`

  try {
    const exeDir = await invoke('get_executable_dir')
    const dir = await join(exeDir, 'Fliprepo', 'flipboard-notes')

    logger.debug('[SelectionNotesPanel] resolve notes dir', {
      exeDir,
      dir,
      rawArticleId,
      safeArticleId,
      fileBaseName
    })

    await invoke('create_directory', { path: dir })

    notesFilePath.value = await join(dir, `${fileBaseName}.json`)
    logger.debug('[SelectionNotesPanel] notesFilePath ready:', notesFilePath.value)

    return notesFilePath.value
  } catch (error) {
    const msg = error?.message || String(error)
    logger.error('[SelectionNotesPanel] ensureNotesFilePath failed:', msg)
    message.error('初始化笔记目录失败', { duration: 2500 })
    throw error
  }
}

/**
 * 从 JSON 文件加载笔记
 */
const loadNotesFromFile = async () => {
  const articleId = String(props.articleId)
  const fp = await ensureNotesFilePath()

  const isNotFoundError = (msg) => {
    const s = String(msg || '').toLowerCase()
    return (
      s.includes('notfound') ||
      s.includes('no such file') ||
      s.includes('系统找不到指定的文件') ||
      s.includes('the system cannot find the file specified') ||
      s.includes('os error 2')
    )
  }

  const tryMigrateFromLegacyLocalStorage = async () => {
    try {
      const legacyKey = `flipboard_notes_${articleId}`
      const legacyRaw = localStorage.getItem(legacyKey)
      if (!legacyRaw) return false

      const legacyData = JSON.parse(legacyRaw)
      if (Array.isArray(legacyData)) {
        noteCards.value = legacyData
      } else if (Array.isArray(legacyData?.cards)) {
        noteCards.value = legacyData.cards
      } else {
        return false
      }

      logger.info('[SelectionNotesPanel] migrated notes from legacy localStorage', {
        legacyKey,
        filePath: fp,
        count: noteCards.value.length
      })

      await saveNotesToFile()
      return true
    } catch (e) {
      logger.warn('[SelectionNotesPanel] migrate legacy localStorage failed', e)
      return false
    }
  }

  try {
    let raw = ''
    try {
      raw = await invoke('read_file_content', { path: fp })
    } catch (e) {
      const msg = e?.message || String(e)
      if (!isNotFoundError(msg)) throw e

      logger.debug('[SelectionNotesPanel] notes file not found, treat as empty', {
        filePath: fp,
        msg
      })

      noteCards.value = []
      const migrated = await tryMigrateFromLegacyLocalStorage()
      if (!migrated) {
        await invoke('create_file_with_content', { path: fp, content: '' })
      }
      return
    }

    if (!String(raw || '').trim()) {
      logger.debug('[SelectionNotesPanel] notes file empty, treat as empty', { filePath: fp })
      noteCards.value = []

      const migrated = await tryMigrateFromLegacyLocalStorage()
      if (!migrated) {
        await saveNotesToFile()
      }
      return
    }

    let data
    try {
      data = JSON.parse(raw)
    } catch (e) {
      logger.warn('[SelectionNotesPanel] notes file is not valid json, reset to empty', {
        filePath: fp,
        error: e?.message || String(e)
      })
      noteCards.value = []
      await saveNotesToFile()
      return
    }

    if (Array.isArray(data)) {
      noteCards.value = data
    } else if (Array.isArray(data?.cards)) {
      noteCards.value = data.cards
    } else {
      noteCards.value = []
    }

    logger.debug('[SelectionNotesPanel] notes loaded from file', {
      filePath: fp,
      count: noteCards.value.length
    })
  } catch (error) {
    const msg = error?.message || String(error)
    logger.error('[SelectionNotesPanel] loadNotesFromFile failed:', msg)
    message.error(`读取笔记失败: ${msg}`, { duration: 2500 })
  }
}

/**
 * 将当前 noteCards 写回 JSON 文件
 */
const saveNotesToFile = async () => {
  try {
    const fp = await ensureNotesFilePath()

    const payload = JSON.stringify(
      {
        version: 1,
        articleId: String(props.articleId),
        articleTitle: props.articleTitle || '',
        updatedAt: new Date().toISOString(),
        cards: noteCards.value
      },
      null,
      2
    )

    await invoke('write_file_content', { path: fp, content: payload })

    logger.debug('[SelectionNotesPanel] notes saved to file', {
      filePath: fp,
      count: noteCards.value.length
    })
  } catch (error) {
    const msg = error?.message || String(error)
    logger.error('[SelectionNotesPanel] saveNotesToFile failed:', msg)
    message.error('保存笔记失败', { duration: 2500 })
  }
}

/**
 * 打开 / 关闭 笔记面板
 */
const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value
}

/**
 * 根据类型生成卡片标题
 */
const getCardTitle = (type) => {
  const base = props.articleTitle || 'Flipboard 文章'
  const typeMap = {
    text: '文本摘录',
    image: '图片摘录',
    code: '代码摘录',
    math: '公式摘录',
    diagram: '图表摘录'
  }
  const suffix = typeMap[type] || '笔记'
  return `${base} - ${suffix}`
}

/**
 * 选择结束时的处理函数（暴露给父组件，在 .article-content 上挂 @mouseup 调用）
 *  - 读取当前选中的文本
 *  - 计算工具条位置
 *  - 填充 selectionText / selectionInsertText
 */
const handleSelectionMouseUp = () => {
  try {
    const selection = window.getSelection()
    const text = selection ? selection.toString().trim() : ''
    if (!text) {
      closeSelectionToolbar()
      return
    }

    // 计算工具条位置：基于选区的 boundingRect
    if (!selection || selection.rangeCount === 0) {
      closeSelectionToolbar()
      return
    }

    const range = selection.getRangeAt(0)
    const rect = range.getBoundingClientRect()
    if (!rect || (rect.width === 0 && rect.height === 0)) {
      closeSelectionToolbar()
      return
    }

    selectionText.value = text
    // 这里将选中的原文作为插入内容，后续点击“插入笔记”时使用
    selectionInsertText.value = text

    const pad = 8
    const toolWidth = 140
    const toolHeight = 40

    let top = rect.top - toolHeight - 8
    if (top < pad) top = rect.bottom + 8

    let left = rect.left
    left = Math.max(pad, Math.min(left, window.innerWidth - toolWidth - pad))

    selectionToolbarTop.value = Math.round(top)
    selectionToolbarLeft.value = Math.round(left)
    selectionToolbarVisible.value = true

    logger.debug('[SelectionNotesPanel] selection captured', {
      text: text.slice(0, 50),
      rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height }
    })
  } catch (error) {
    logger.error('[SelectionNotesPanel] handleSelectionMouseUp error:', error)
    closeSelectionToolbar()
  }
}

/**
 * 关闭选择工具条并清空状态
 */
const closeSelectionToolbar = () => {
  selectionToolbarVisible.value = false
  selectionText.value = ''
  selectionInsertText.value = ''
}

/**
 * 将当前 selectionInsertText 写入笔记：
 *  1. 尝试追加到当前编辑卡片（如果有）
 *  2. 否则创建一个新的单条笔记卡片
 */
const insertSelectionResult = async () => {
  const text = selectionInsertText.value.trim()
  if (!text) {
    return
  }

  selectionBusy.value = true
  try {
    // 当前是否有正在编辑的卡片
    const idx = activeCardIndex.value
    if (idx >= 0 && idx < noteCards.value.length) {
      const card = noteCards.value[idx]
      card.content = `${card.content || ''}\n\n${text}`
      card.timestamp = new Date().toISOString()
      noteCards.value.splice(idx, 1, { ...card })
      await persistNotes()
      message.success('已插入当前笔记', { duration: 1200 })
    } else {
      // 没有活动卡片则新建
      await addToNotes({ type: 'text', content: text, originalMarkdown: text })
    }

    showNoteAddedBubble()
  } catch (error) {
    logger.error('[SelectionNotesPanel] insertSelectionResult failed:', error)
    message.error('插入笔记失败', { duration: 2000 })
  } finally {
    selectionBusy.value = false
    closeSelectionToolbar()
  }
}

/**
 * 核心：addToNotes
 *  - 与 PDF 阅读器保持同样的语义：接收一个 element，抽取 originalMarkdown / content
 *  - 创建一张单条笔记卡片（type = 'single'）
 */
const addToNotes = async (element) => {
  const safeContent = (element?.originalMarkdown || element?.content || '').trim()
  if (!safeContent) {
    message.warning('未选中有效内容', { duration: 1500 })
    return
  }

  const normalizedElement = {
    ...element,
    content: element?.content || safeContent,
    originalMarkdown: element?.originalMarkdown || safeContent
  }

  const now = Date.now()
  const newCard = {
    id: now.toString(),
    type: 'single',
    title: getCardTitle(normalizedElement.type || 'text'),
    timestamp: new Date().toISOString(),
    // 简化：内容直接存到 content 字段，items 保留结构兼容
    content: normalizedElement.originalMarkdown,
    items: [
      {
        id: `${now}-0`,
        type: normalizedElement.type || 'text',
        content: normalizedElement.content,
        originalMarkdown: normalizedElement.originalMarkdown,
        timestamp: new Date().toISOString()
      }
    ]
  }

  noteCards.value.push(newCard)
  await persistNotes()

  logger.debug('[SelectionNotesPanel] note card added', {
    articleId: props.articleId,
    cardId: newCard.id,
    title: newCard.title
  })

  showNoteAddedBubble()
}

/**
 * 显示“已添加到笔记”的气泡提示
 */
const showNoteAddedBubble = () => {
  showNoteBubble.value = true
  setTimeout(() => {
    showNoteBubble.value = false
  }, 1200)
}

/**
 * 将当前 noteCards 持久化到 localStorage
 */
const persistNotes = async () => {
  await saveNotesToFile()
  // 同步到统一收集索引（全局搜索 / 插入主页编辑器）。
  try {
    await collections.syncArticleNotes({
      articleId: props.articleId,
      articleTitle: props.articleTitle,
      sourceUrl: props.sourceUrl,
      category: props.category,
      cards: noteCards.value
    })
  } catch (e) {
    logger.warn('[SelectionNotesPanel] sync collections index failed', e)
  }
}

/**
 * 新建一张空白笔记卡片
 */
const addNewCard = () => {
  const now = Date.now()
  const placeholder = '点击编辑内容...'
  const newCard = {
    id: now.toString(),
    type: 'single',
    title: props.articleTitle ? `${props.articleTitle} - 新笔记` : '新笔记',
    timestamp: new Date().toISOString(),
    content: placeholder,
    items: [
      {
        id: `${now}-0`,
        type: 'text',
        content: placeholder,
        originalMarkdown: placeholder,
        timestamp: new Date().toISOString()
      }
    ]
  }

  noteCards.value.push(newCard)
  persistNotes()

  logger.debug('[SelectionNotesPanel] empty note card created', {
    articleId: props.articleId,
    cardId: newCard.id
  })
}

/**
 * 删除卡片
 */
const deleteCard = async (index) => {
  if (index < 0 || index >= noteCards.value.length) return

  const cardId = noteCards.value[index].id
  noteCards.value.splice(index, 1)

  if (activeCardIndex.value === index) {
    activeCardIndex.value = -1
  } else if (activeCardIndex.value > index) {
    activeCardIndex.value--
  }

  await persistNotes()

  logger.debug('[SelectionNotesPanel] note card deleted', {
    articleId: props.articleId,
    cardId
  })
}

/**
 * 进入编辑模式
 */
const enterEditMode = (index) => {
  if (index < 0 || index >= noteCards.value.length) return
  activeCardIndex.value = index
}

/**
 * 退出编辑模式，不保存
 */
const exitEditMode = (index) => {
  if (index === activeCardIndex.value) {
    activeCardIndex.value = -1
  }
}

/**
 * 保存当前编辑中的卡片
 */
const saveCardEdit = async (index) => {
  if (index < 0 || index >= noteCards.value.length) return

  const card = noteCards.value[index]
  card.timestamp = new Date().toISOString()
  noteCards.value.splice(index, 1, { ...card })
  activeCardIndex.value = -1

  await persistNotes()
  message.success('笔记已保存', { duration: 1500 })

  logger.debug('[SelectionNotesPanel] note card updated', {
    articleId: props.articleId,
    cardId: card.id
  })
}

/**
 * 时间格式化
 */
const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

// 监听 articleId 变化，切换文章时重新加载对应的笔记数据
watch(
  () => props.articleId,
  async () => {
    notesFilePath.value = ''
    noteCards.value = []
    await loadNotesFromFile()
  },
  { immediate: true }
)

onMounted(async () => {
  await loadNotesFromFile()
})

/**
 * 双击插入：父组件传入一段文本（句子/段落），直接生成一张笔记卡片，无需手动选择。
 * 插入后自动展开笔记面板，让用户看到结果。
 */
const insertBlockText = async (text) => {
  const safe = String(text || '').trim()
  if (!safe) return
  await addToNotes({ type: 'text', content: safe, originalMarkdown: safe })
  if (!showNotes.value) {
    showNotes.value = true
  }
}

// 将 handleSelectionMouseUp / toggleNotesPanel / insertBlockText 暴露给父组件
defineExpose({
  handleSelectionMouseUp,
  toggleNotesPanel,
  insertBlockText
})
</script>

<style scoped>
.selection-notes-wrapper {
  position: relative;
}

/* 文章内容插槽区域 */
.selection-content-slot {
  position: relative;
}

/* 选择工具条 */
.selection-toolbar {
  position: fixed;
  z-index: 9999;
  background: var(--guaxi-panel-bg, #fff);
  border: 1px solid var(--guaxi-border, #ddd);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(15, 23, 42, 0.15);
  padding: 4px 8px;
}

.selection-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.selection-tool-btn {
  border: none;
  background: var(--guaxi-primary, #2563eb);
  color: #fff;
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 12px;
  cursor: pointer;
}

.selection-tool-btn[disabled] {
  opacity: 0.5;
  cursor: default;
}

/* 笔记面板（右侧滑出） */
.notes-panel {
  position: fixed;
  top: 64px;
  right: -360px;
  width: 320px;
  height: calc(100% - 80px);
  background: var(--guaxi-panel-bg, #fff);
  border-left: 1px solid var(--guaxi-border, #e5e7eb);
  box-shadow: -4px 0 24px rgba(15, 23, 42, 0.12);
  display: flex;
  flex-direction: column;
  transition: right 0.25s ease;
  z-index: 9998;
}

.notes-panel-visible {
  right: 0;
}

.notes-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--guaxi-border, #e5e7eb);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.notes-header h3 {
  margin: 0;
  font-size: 14px;
}

.notes-header .close-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.notes-content {
  flex: 1;
  padding: 12px;
  overflow: auto;
}

.note-card {
  margin-bottom: 12px;
}

.card-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
}

.note-content {
  padding: 12px 16px;
  font-size: 13px;
  line-height: 1.6;
  color: var(--guaxi-text-primary, #0f172a);
}

.note-editor {
  width: 100%;
  min-height: 140px;
  padding: 10px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border, #e5e7eb);
  font-size: 13px;
  resize: vertical;
  background: var(--guaxi-panel-bg, #fff);
  color: var(--guaxi-text-primary, #0f172a);
}

.note-editor::placeholder {
  color: var(--guaxi-text-secondary, #64748b);
}

.note-editor:focus {
  outline: none;
  border-color: var(--guaxi-primary, #2563eb);
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.18);
}

.composite-note-content {
  white-space: pre-wrap;
  font-size: 13px;
}

.add-card-container {
  margin-top: 8px;
}

/* 已添加气泡 */
.note-added-bubble {
  position: fixed;
  right: 24px;
  bottom: 24px;
  background: rgba(34, 197, 94, 0.96);
  color: #fff;
  padding: 8px 16px;
  border-radius: 999px;
  box-shadow: 0 8px 24px rgba(22, 163, 74, 0.45);
  font-size: 13px;
  z-index: 10000;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>