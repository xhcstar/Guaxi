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
    // create_directory 对已存在目录会报错（KB 场景的重名保护），但这里是"确保存在"语义，
    // 目录已在即视为成功。
    try {
      await invoke('create_directory', { path: dir })
    } catch (e) {
      if (!String(e?.message || e).includes('已存在')) throw e
    }
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

    // 保留该文章旧条目的「收藏夹」标记（按 entry id），避免重新编辑笔记后丢失收藏。
    const prevFavorites = new Map(
      entries.value
        .filter(e => String(e.articleId) === aid)
        .map(e => [String(e.id), !!e.favorite])
    )

    // 先移除该文章的旧条目，再写入当前卡片。
    entries.value = entries.value.filter(e => String(e.articleId) !== aid)

    const list = Array.isArray(cards) ? cards : []
    for (const card of list) {
      const content = String(card?.content || '').trim()
      if (!content || content === '点击编辑内容...') continue
      const entryId = `${aid}::${card.id}`
      entries.value.push({
        id: entryId,
        articleId: aid,
        cardId: String(card.id || ''),
        title: String(card?.title || articleTitle || '未命名笔记'),
        content,
        sourceUrl: String(sourceUrl || ''),
        sourceTitle: String(articleTitle || ''),
        category: String(category || ''),
        favorite: prevFavorites.get(entryId) || false,
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

  /**
   * 直接追加一条采集笔记（用于"浏览器打开原文"窗口双击采集等场景）。
   * payload: { type:'text'|'image'|'video', content, originalMarkdown, sourceUrl, sourceTitle, category }
   */
  let captureSeq = 0
  const addCapturedNote = async (payload = {}) => {
    const content = String(payload.originalMarkdown || payload.content || '').trim()
    if (!content) return
    await load()
    captureSeq += 1
    const ts = new Date().toISOString()
    const sourceUrl = String(payload.sourceUrl || '')
    let sourceTitle = String(payload.sourceTitle || '')
    if (!sourceTitle && sourceUrl) {
      try { sourceTitle = new URL(sourceUrl).host } catch { sourceTitle = sourceUrl }
    }
    const typeLabel = payload.type === 'image' ? '图片' : payload.type === 'video' ? '视频' : '摘录'
    entries.value.push({
      id: `capture::${Date.now()}::${captureSeq}`,
      articleId: '',
      type: 'note',
      title: `${sourceTitle || '网页'} - ${typeLabel}`,
      content,
      sourceUrl,
      sourceTitle,
      category: String(payload.category || ''),
      favorite: false,
      timestamp: ts
    })
    await save()
  }

  /** 把一篇文章作为「收藏」条目写入收集库（卡片点收藏 icon 时调用）。 */
  const bookmarkEntryId = (articleId) => `bookmark::${String(articleId)}`

  const upsertBookmarkArticle = async (article) => {
    const aid = String(article?.id || '').trim()
    if (!aid) return
    await load()
    const entry = {
      id: bookmarkEntryId(aid),
      articleId: aid,
      type: 'bookmark',
      title: String(article?.title || '未命名文章'),
      content: String(article?.description || article?.summary || article?.content || '').trim(),
      sourceUrl: String(article?.url || ''),
      sourceTitle: String(article?.source || article?.title || ''),
      category: String(article?.category || ''),
      favorite: true,
      timestamp: article?.bookmarkedAt || new Date().toISOString()
    }
    const idx = entries.value.findIndex(e => e.id === entry.id)
    if (idx >= 0) entries.value.splice(idx, 1, entry)
    else entries.value.push(entry)
    await save()
  }

  const removeBookmarkArticle = async (articleId) => {
    await removeEntry(bookmarkEntryId(articleId))
  }

  /** 启动时把收藏夹（localStorage 文章）与收集库的 bookmark 条目对齐。 */
  const syncBookmarks = async (bookmarks) => {
    await load()
    const list = Array.isArray(bookmarks) ? bookmarks : []
    const wantIds = new Set(list.map(b => bookmarkEntryId(b?.id)))
    // 移除已取消收藏的 bookmark 条目（保留所有 note 条目）。
    entries.value = entries.value.filter(e => e.type !== 'bookmark' || wantIds.has(e.id))
    for (const b of list) {
      const id = bookmarkEntryId(b?.id)
      const entry = {
        id,
        articleId: String(b?.id || ''),
        type: 'bookmark',
        title: String(b?.title || '未命名文章'),
        content: String(b?.description || b?.summary || b?.content || '').trim(),
        sourceUrl: String(b?.url || ''),
        sourceTitle: String(b?.source || b?.title || ''),
        category: String(b?.category || ''),
        favorite: true,
        timestamp: b?.bookmarkedAt || new Date().toISOString()
      }
      const idx = entries.value.findIndex(e => e.id === id)
      if (idx >= 0) entries.value.splice(idx, 1, entry)
      else entries.value.push(entry)
    }
    await save()
  }

  /** 切换某条笔记的「收藏夹」标记（长久保留）。 */
  const toggleFavorite = async (entryId) => {
    await load()
    const hit = entries.value.find(e => e.id === entryId)
    if (hit) {
      hit.favorite = !hit.favorite
      await save()
    }
    return hit?.favorite || false
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

  /** 「全部笔记」：只含笔记条目（排除卡片收藏的整篇文章）。 */
  const noteEntries = computed(() => filteredEntries.value.filter(e => e.type !== 'bookmark'))

  /** 收藏夹：仅 favorite=true 的条目（在 filteredEntries 基础上再筛）。 */
  const favoriteEntries = computed(() => filteredEntries.value.filter(e => !!e.favorite))

  store = {
    entries,
    searchQuery,
    filteredEntries,
    noteEntries,
    favoriteEntries,
    load,
    save,
    syncArticleNotes,
    addCapturedNote,
    removeEntry,
    toggleFavorite,
    upsertBookmarkArticle,
    removeBookmarkArticle,
    syncBookmarks,
    buildMarkdown
  }
  return store
}
