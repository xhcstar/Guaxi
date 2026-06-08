import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join } from '@tauri-apps/api/path'
import { createLogger } from '../utils/logger'

const logger = createLogger('flipboardCollectionsStore')

let store

/**
 * Flipboard「我的收集」统一笔记库。
 *
 * 与 SelectionNotesPanel 的单篇 JSON 并存：单篇文件仍是每篇文章的笔记真源，
 * 这里维护一个跨文章的统一索引 index.json，用于全局搜索、管理与插入主页编辑器。
 * 每次单篇笔记保存时调用 syncArticleNotes 同步对应条目。
 */
export const useFlipboardCollectionsStore = () => {
  if (store) return store

  const entries = ref([])
  const searchQuery = ref('')
  const loaded = ref(false)
  let indexPath = ''

  const ensureIndexPath = async () => {
    if (indexPath) return indexPath
    const exeDir = await invoke('get_executable_dir')
    const dir = await join(exeDir, 'Fliprepo', 'flipboard-notes')
    await invoke('create_directory', { path: dir })
    indexPath = await join(dir, 'index.json')
    return indexPath
  }

  const load = async (force = false) => {
    if (loaded.value && !force) return entries.value
    try {
      const fp = await ensureIndexPath()
      let raw = ''
      try {
        raw = await invoke('read_file_content', { path: fp })
      } catch {
        raw = ''
      }
      if (String(raw || '').trim()) {
        const data = JSON.parse(raw)
        entries.value = Array.isArray(data?.entries) ? data.entries : []
      } else {
        entries.value = []
      }
      loaded.value = true
    } catch (error) {
      logger.error('load collections index failed:', error?.message || error)
      entries.value = []
    }
    return entries.value
  }

  const save = async () => {
    try {
      const fp = await ensureIndexPath()
      const payload = JSON.stringify(
        { version: 1, updatedAt: new Date().toISOString(), entries: entries.value },
        null,
        2
      )
      await invoke('write_file_content', { path: fp, content: payload })
    } catch (error) {
      logger.error('save collections index failed:', error?.message || error)
    }
  }

  /**
   * 用某篇文章的最新笔记卡片覆盖该文章在索引里的条目。
   * cards: SelectionNotesPanel 的 noteCards（含 id/title/content/timestamp）。
   */
  const syncArticleNotes = async ({ articleId, articleTitle, sourceUrl, category, cards } = {}) => {
    const aid = String(articleId || '').trim()
    if (!aid) return
    await load()

    // 先移除该文章的旧条目，再写入当前卡片。
    entries.value = entries.value.filter(e => String(e.articleId) !== aid)

    const list = Array.isArray(cards) ? cards : []
    for (const card of list) {
      const content = String(card?.content || '').trim()
      if (!content || content === '点击编辑内容...') continue
      entries.value.push({
        id: `${aid}::${card.id}`,
        articleId: aid,
        cardId: String(card.id || ''),
        title: String(card?.title || articleTitle || '未命名笔记'),
        content,
        sourceUrl: String(sourceUrl || ''),
        sourceTitle: String(articleTitle || ''),
        category: String(category || ''),
        timestamp: card?.timestamp || new Date().toISOString()
      })
    }

    await save()
  }

  const removeEntry = async (entryId) => {
    await load()
    entries.value = entries.value.filter(e => e.id !== entryId)
    await save()
  }

  /** 生成带来源引用的 Markdown，供复制/插入主页编辑器。 */
  const buildMarkdown = (entry) => {
    if (!entry) return ''
    const lines = []
    if (entry.title) lines.push(`## ${entry.title}`, '')
    lines.push(entry.content || '', '')
    if (entry.sourceUrl) {
      const label = entry.sourceTitle || entry.sourceUrl
      lines.push(`> 摘自 [${label}](${entry.sourceUrl})`)
    } else if (entry.sourceTitle) {
      lines.push(`> 摘自 ${entry.sourceTitle}`)
    }
    return lines.join('\n').trim() + '\n'
  }

  const filteredEntries = computed(() => {
    const q = String(searchQuery.value || '').trim().toLowerCase()
    const list = [...entries.value].sort(
      (a, b) => new Date(b.timestamp || 0) - new Date(a.timestamp || 0)
    )
    if (!q) return list
    return list.filter(e =>
      String(e.title || '').toLowerCase().includes(q) ||
      String(e.content || '').toLowerCase().includes(q) ||
      String(e.sourceTitle || '').toLowerCase().includes(q) ||
      String(e.sourceUrl || '').toLowerCase().includes(q) ||
      String(e.category || '').toLowerCase().includes(q)
    )
  })

  store = {
    entries,
    searchQuery,
    filteredEntries,
    load,
    save,
    syncArticleNotes,
    removeEntry,
    buildMarkdown
  }
  return store
}
