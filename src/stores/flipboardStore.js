import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import { useFlipboardCollectionsStore } from './flipboardCollectionsStore.js'

const logger = createLogger('flipboardStore')
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
}

let store

export const useFlipboardStore = () => {
  if (store) return store
  let activeFetchToken = 0
  let activeDetailToken = 0
  const inMemoryArticleCache = new Map()
  // 状态
  const articles = ref([])
  const categories = ref([])
  const feeds = ref([])
  const bookmarks = ref([])
  const readingHistory = ref([])
  const currentArticle = ref(null)
  const selectedCategory = ref(null)
  const isLoading = ref(false)
  const isRefreshing = ref(false)
  const searchQuery = ref('')
  const currentView = ref('grid') // 'grid' | 'list' | 'magazine'
  const feedHealth = ref([])
  const collectionsPanelVisible = ref(false)
  const isHydratingDetail = ref(false)
  // 后台抓取总开关（用户掌控；持久化，默认关闭）。
  const scrapingEnabled = ref(localStorage.getItem('flipboard_scraping_enabled') === '1')
  // 待登录队列（后台命中登录墙时入队，由用户主动打开，不再自动弹窗）。
  const pendingLogins = ref([])
  // 收集面板点击条目时，请求阅读区打开该文章（ReadingCoordinator 监听）。
  const pendingReaderArticle = ref(null)
  const openReaderForArticle = (articleRef) => {
    pendingReaderArticle.value = articleRef
  }

  const scoreByTopic = (article, topicTokens) => {
    if (!topicTokens.length) return 0
    const title = String(article?.title || '').toLowerCase()
    const description = String(article?.description || '').toLowerCase()
    const content = String(article?.content || '').toLowerCase()
    const source = String(article?.source || '').toLowerCase()
    return topicTokens.reduce((sum, token) => {
      const t = String(token || '').trim().toLowerCase()
      if (!t) return sum
      const titleCount = title.split(t).length - 1
      const descriptionCount = description.split(t).length - 1
      const contentCount = content.split(t).length - 1
      const sourceCount = source.split(t).length - 1
      return sum + titleCount * 4 + descriptionCount * 3 + contentCount + sourceCount * 2
    }, 0)
  }

  const runSchedulerTick = async (topic = '', force = false, limit = 120) => {
    try {
      const category = selectedCategory.value || null
      const data = await invoke('run_feed_scheduler_tick', {
        category,
        topic: String(topic || '').trim() || null,
        force,
        limit
      })
      if (Array.isArray(data?.reports)) {
        await fetchFeedHealth()
      }
      return data
    } catch (error) {
      console.error('FlipboardStore: runSchedulerTick failed:', error)
      throw error
    }
  }

  const fetchFeedSchedule = async () => {
    try {
      return await invoke('get_feed_schedule_status')
    } catch (error) {
      console.error('FlipboardStore: fetchFeedSchedule failed:', error)
      throw error
    }
  }

  const createKbNoteFromTopic = async (payload) => {
    try {
      return await invoke('create_kb_note_from_topic', payload || {})
    } catch (error) {
      console.error('FlipboardStore: createKbNoteFromTopic failed:', error)
      throw error
    }
  }

  const addSite = async (seedUrl, category = 'all') => {
    try {
      console.log('FlipboardStore: addSite', { seedUrl, category })
      const created = await invoke('add_site', { seedUrl, category })
      await fetchFeeds()
      return Array.isArray(created) ? created : []
    } catch (error) {
      console.error('FlipboardStore: addSite failed:', error)
      throw error
    }
  }

  const importFeedsFromWebsiteMarkdown = async (path = null) => {
    try {
      const created = await invoke('import_feeds_from_website_markdown', { path })
      await fetchFeeds()
      return Array.isArray(created) ? created : []
    } catch (error) {
      if (String(error || '').includes('未找到 website markdown 文件')) {
        console.warn('FlipboardStore: website markdown not found, skip importing feeds')
        return []
      }
      console.error('FlipboardStore: importFeedsFromWebsiteMarkdown failed:', error)
      throw error
    }
  }

  const buildSourcePreference = () => {
    const weight = new Map()
    const add = (source, delta) => {
      const key = String(source || '').trim().toLowerCase()
      if (!key) return
      weight.set(key, (weight.get(key) || 0) + delta)
    }
    ;(Array.isArray(readingHistory.value) ? readingHistory.value : []).forEach(item => add(item?.source, 2))
    ;(Array.isArray(bookmarks.value) ? bookmarks.value : []).forEach(item => add(item?.source, 3))
    return weight
  }

  const scoreByPreference = (article, sourceWeight) => {
    const key = String(article?.source || '').trim().toLowerCase()
    const sourceScore = sourceWeight.get(key) || 0
    const hasImageScore = article?.image_url ? 1 : 0
    return sourceScore + hasImageScore
  }

  const mergeAndSortArticles = (current, incoming, topicText) => {
    const map = new Map()
    const add = (item) => {
      const key = `${String(item?.url || '').trim().toLowerCase()}|${String(item?.title || '').trim().toLowerCase()}`
      if (!key || key === '|') return
      if (!map.has(key)) map.set(key, item)
    }
    ;(Array.isArray(current) ? current : []).forEach(add)
    ;(Array.isArray(incoming) ? incoming : []).forEach(add)
    const merged = Array.from(map.values())
    const tokens = String(topicText || '')
      .split(/[,\s]+/)
      .map(x => x.trim().toLowerCase())
      .filter(Boolean)
    const sourceWeight = buildSourcePreference()
    // 相关性优先：已评分(>=0)的文章按相关分降序排在前；未评分(-1)的退回原排序逻辑。
    const relevanceCmp = (a, b) => {
      const ra = Number.isFinite(a?.relevance_score) ? a.relevance_score : -1
      const rb = Number.isFinite(b?.relevance_score) ? b.relevance_score : -1
      if (ra < 0 && rb < 0) return 0
      if (ra < 0) return 1
      if (rb < 0) return -1
      return rb - ra
    }
    if (tokens.length) {
      merged.sort((a, b) => {
        const rc = relevanceCmp(a, b)
        if (rc !== 0) return rc
        const sa = scoreByTopic(a, tokens)
        const sb = scoreByTopic(b, tokens)
        if (sb !== sa) return sb - sa
        const pa = scoreByPreference(a, sourceWeight)
        const pb = scoreByPreference(b, sourceWeight)
        if (pb !== pa) return pb - pa
        return new Date(b?.published_at || 0) - new Date(a?.published_at || 0)
      })
    } else {
      merged.sort((a, b) => {
        const rc = relevanceCmp(a, b)
        if (rc !== 0) return rc
        const pa = scoreByPreference(a, sourceWeight)
        const pb = scoreByPreference(b, sourceWeight)
        if (pb !== pa) return pb - pa
        return new Date(b?.published_at || 0) - new Date(a?.published_at || 0)
      })
    }
    return merged
  }

  const pickRelevantFeedIds = (category) => {
    const list = Array.isArray(feeds.value) ? feeds.value : []
    const cat = String(category || '').trim()
    return list
      .filter(f => {
        if (!f?.enabled) return false
        if (!cat || cat === 'all') return true
        return String(f?.category || '') === cat || String(f?.category || '') === 'all'
      })
      .map(f => String(f?.id || '').trim())
      .filter(Boolean)
  }

  const fetchKnowledgeProgressively = async (topicText, category = null, forceRefresh = false) => {
    const normalizedTopic = String(topicText || '').trim()
    const normalizedCategory = category || null
    const requestToken = ++activeFetchToken
    const isStale = () => requestToken !== activeFetchToken
    const cacheKey = `${String(normalizedCategory || 'all')}|${normalizedTopic.toLowerCase()}`
    let ensuredCategoryFallback = false
    const ensureCategoryFallbackFeeds = async (force = false) => {
      if (ensuredCategoryFallback) return 0
      const categoryId = String(normalizedCategory || '').trim()
      if (!categoryId || categoryId === 'all') return 0
      ensuredCategoryFallback = true
      try {
        const created = await invoke('ensure_category_default_feeds', { category: categoryId, force })
        const createdCount = Number(created || 0)
        if (createdCount > 0) {
          console.log('FlipboardStore: ensured fallback feeds for category', { category: categoryId, createdCount })
          await fetchFeeds()
        }
        return createdCount
      } catch (e) {
        console.warn('FlipboardStore: ensure_category_default_feeds failed', e)
        return 0
      }
    }
    if (forceRefresh) {
      isRefreshing.value = true
    } else {
      isLoading.value = true
    }
    selectedCategory.value = normalizedCategory

    try {
      if (!forceRefresh) {
        const memoryCached = inMemoryArticleCache.get(cacheKey)
        if (Array.isArray(memoryCached) && memoryCached.length) {
          articles.value = memoryCached.slice(0, 80)
          isLoading.value = false
        }
        const persisted = await invoke('get_cached_articles', {
          category: normalizedCategory,
          topic: normalizedTopic || null,
          limit: 80
        })
        if (isStale()) return
        if (Array.isArray(persisted) && persisted.length) {
          articles.value = persisted
          inMemoryArticleCache.set(cacheKey, persisted.slice(0, 120))
          isLoading.value = false
          if (persisted.length >= 40) {
            return
          }
        }
      }

      if (!Array.isArray(feeds.value) || feeds.value.length === 0) {
        await fetchFeeds()
      }
      if (isStale()) return
      let feedIds = pickRelevantFeedIds(normalizedCategory)
      if (!feedIds.length) {
        await ensureCategoryFallbackFeeds()
        if (isStale()) return
        feedIds = pickRelevantFeedIds(normalizedCategory)
      }

      if (!feedIds.length) {
        const fallback = normalizedTopic
          ? await invoke('refresh_topic_knowledge_from_feeds', { topic: normalizedTopic, category: normalizedCategory, limit: 80 })
          : await invoke('refresh_articles_from_feeds', { category: normalizedCategory, limit: 80 })
        if (isStale()) return
        const next = Array.isArray(fallback) ? fallback : []
        articles.value = next
        inMemoryArticleCache.set(cacheKey, next.slice(0, 120))
        return
      }

      const batchSize = 3
      const perBatchLimit = 24
      let merged = []

      for (let i = 0; i < feedIds.length; i += batchSize) {
        const batchIds = feedIds.slice(i, i + batchSize)
        const chunk = await invoke('fetch_knowledge_from_feed_ids', {
          feedIds: batchIds,
          topic: normalizedTopic || null,
          category: normalizedCategory,
          limit: perBatchLimit,
          force: forceRefresh
        })
        if (isStale()) return
        merged = mergeAndSortArticles(merged, Array.isArray(chunk) ? chunk : [], normalizedTopic)
      }
      let next = merged.slice(0, 80)
      if (!next.length) {
        ensuredCategoryFallback = false
        const createdCount = await ensureCategoryFallbackFeeds(true)
        if (isStale()) return
        if (createdCount > 0) {
          const recovered = normalizedTopic
            ? await invoke('refresh_topic_knowledge_from_feeds', { topic: normalizedTopic, category: normalizedCategory, limit: 80 })
            : await invoke('refresh_articles_from_feeds', { category: normalizedCategory, limit: 80 })
          if (isStale()) return
          next = Array.isArray(recovered) ? recovered : []
        }
      }
      articles.value = next
      inMemoryArticleCache.set(cacheKey, next.slice(0, 120))
    } catch (error) {
      console.error('FlipboardStore: fetchKnowledgeProgressively failed:', error)
      throw error
    } finally {
      if (isStale()) return
      try {
        await fetchFeedHealth()
      } catch (e) {
        console.warn('FlipboardStore: fetchFeedHealth failed after progressive fetch', e)
      }
      localStorage.setItem('flipboard_last_fetch', new Date().toISOString())
      isLoading.value = false
      isRefreshing.value = false
    }
  }

  const fetchArticles = async (category = null, forceRefresh = false) => {
    console.log('FlipboardStore: fetchArticles from feeds', { category })
    await fetchKnowledgeProgressively('', category, forceRefresh)
  }

  // 获取单个文章详情
  // 立即用列表已有数据打开面板，正文在后台 hydrate 完成后再替换，避免“点击无反应”。
  const fetchArticleDetail = async (articleRef, force = false) => {
    const articleId = typeof articleRef === 'string' ? articleRef : String(articleRef?.id || '')
    const articleUrl = typeof articleRef === 'string' ? null : String(articleRef?.url || '').trim() || null

    // 1. 先用点击时已有的对象立刻渲染（RSS 已带标题/摘要/部分正文）。
    if (typeof articleRef === 'object' && articleRef) {
      currentArticle.value = articleRef
      addToReadingHistory(articleRef)
    }

    // 2. 后台请求完整正文（可能触发渲染/抽取，较慢），完成后替换。
    isHydratingDetail.value = true
    const fetchToken = ++activeDetailToken
    try {
      const data = await invoke('get_article_detail', { id: articleId, url: articleUrl, force: !!force })
      // 仅当用户仍停留在同一篇文章时才覆盖，避免快速切换时旧请求回填。
      if (fetchToken === activeDetailToken) {
        currentArticle.value = data
        addToReadingHistory(data)
      }
      return data
    } catch (error) {
      console.error('Failed to fetch article detail:', error)
      // 已有 base 渲染则不抛错打断 UI；完全没有才抛。
      if (!currentArticle.value) throw error
    } finally {
      if (fetchToken === activeDetailToken) {
        isHydratingDetail.value = false
      }
    }
  }

  // 重新深解析当前文章（强制 re-hydrate）
  const reparseArticle = async (articleRef) => {
    return fetchArticleDetail(articleRef, true)
  }

  // 在内置浏览器窗口打开原网页
  const openArticleInBrowser = async (url) => {
    try {
      await invoke('open_article_in_browser', { url })
      return true
    } catch (error) {
      console.error('FlipboardStore: openArticleInBrowser failed:', error)
      throw error
    }
  }

  // 重启 RSSHub 以读取最新登录态 cookie（用户登录知乎/小红书等之后调用）。
  // RSSHub 未构建时后端返回错误，这里吞掉不打断（聚合的其余源不受影响）。
  const refreshRsshubCookies = async () => {
    try {
      await invoke('rsshub_refresh_cookies')
      return true
    } catch (error) {
      console.warn('FlipboardStore: refreshRsshubCookies skipped:', error)
      return false
    }
  }

  // 搜索文章
  const searchArticles = async (query) => {
    if (!query.trim()) {
      fetchArticles(selectedCategory.value)
      return
    }

    searchQuery.value = query
    await fetchTopicKnowledge(query, selectedCategory.value)
  }

  const fetchTopicKnowledge = async (topic, category = null, forceRefresh = false) => {
    const topicText = String(topic || '').trim()
    if (!topicText) {
      return fetchArticles(category, forceRefresh)
    }
    console.log('FlipboardStore: fetchTopicKnowledge', { topic: topicText, category })
    searchQuery.value = topicText
    await fetchKnowledgeProgressively(topicText, category, forceRefresh)
  }

  // 搜索文章
  const searchArticlesLocal = async (query) => {
    if (!query.trim()) {
      fetchArticles(selectedCategory.value)
      return
    }

    searchQuery.value = query
    isLoading.value = true

    try {
      const data = await invoke('search_articles', {
        query,
        category: selectedCategory.value
      })
      articles.value = data
    } catch (error) {
      console.error('Search failed:', error)
    } finally {
      isLoading.value = false
    }
  }

  // 获取分类
  const fetchCategories = async () => {
    try {
      const data = await invoke('get_categories')
      categories.value = data
    } catch (error) {
      console.error('Failed to fetch categories:', error)
    }
  }

  // 获取RSS源
  const fetchFeeds = async () => {
    try {
      const data = await invoke('get_feeds')
      feeds.value = data
    } catch (error) {
      console.error('Failed to fetch feeds:', error)
    }
  }

  const fetchFeedHealth = async () => {
    try {
      console.log('FlipboardStore: fetchFeedHealth')
      const data = await invoke('get_feed_health')
      feedHealth.value = Array.isArray(data) ? data : []
      return feedHealth.value
    } catch (error) {
      console.error('FlipboardStore: fetchFeedHealth failed:', error)
      throw error
    }
  }

  // 添加信息源
  const addFeed = async (source) => {
    try {
      console.log('FlipboardStore: addFeed', source)
      await invoke('add_feed', { input: source })
      await fetchFeeds()
      return true
    } catch (error) {
      console.error('Failed to add feed:', error)
      throw error
    }
  }

  // 按平台和分类自动生成信息源（简单模式）
  const generateFeedsByPlatform = async (platforms, category) => {
    try {
      console.log('FlipboardStore: generateFeedsByPlatform', { platforms, category })
      const created = await invoke('generate_feeds_for_platforms', { platforms, category })
      await fetchFeeds()
      return created
    } catch (error) {
      console.error('FlipboardStore: generateFeedsByPlatform failed:', error)
      throw error
    }
  }

  // 启动整站深度抓取源
  const startSiteCrawl = async ({ seedUrl, category, interest, crawlCfg, renderProfile } = {}) => {
    try {
      console.log('FlipboardStore: startSiteCrawl', { seedUrl, category })
      const feed = await invoke('start_site_crawl', {
        seedUrl,
        category: category || null,
        interest: interest || null,
        crawlCfg: crawlCfg || null,
        renderProfile: renderProfile || null
      })
      await fetchFeeds()
      return feed
    } catch (error) {
      console.error('FlipboardStore: startSiteCrawl failed:', error)
      throw error
    }
  }

  // 查询整站抓取进度
  const getCrawlProgress = async (feedId) => {
    try {
      return await invoke('get_crawl_progress', { feedId })
    } catch (error) {
      console.error('FlipboardStore: getCrawlProgress failed:', error)
      return { pending: 0, done: 0, error: 0, total: 0 }
    }
  }

  // 重置爬虫队列（重爬）
  const resetCrawlQueue = async (feedId) => {
    try {
      await invoke('reset_crawl_queue', { feedId })
      return true
    } catch (error) {
      console.error('FlipboardStore: resetCrawlQueue failed:', error)
      throw error
    }
  }

  // 确保无头 Chromium 就绪（首次会下载）
  const ensureChromiumReady = async () => {
    try {
      return await invoke('ensure_chromium_ready')
    } catch (error) {
      console.error('FlipboardStore: ensureChromiumReady failed:', error)
      throw error
    }
  }

  // 打开登录会话窗口（登录墙站点）
  const openLoginSession = async (profile, loginUrl) => {
    try {
      await invoke('open_login_session', { profile, loginUrl })
      return true
    } catch (error) {
      console.error('FlipboardStore: openLoginSession failed:', error)
      throw error
    }
  }

  // 启用/禁用订阅源
  const setFeedEnabled = async (id, enabled) => {
    try {
      console.log('FlipboardStore: setFeedEnabled', { id, enabled })
      await invoke('set_feed_enabled', { id, enabled })
      await fetchFeeds()
      return true
    } catch (error) {
      console.error('FlipboardStore: setFeedEnabled failed:', error)
      throw error
    }
  }

  // 批量删除订阅源
  const deleteFeeds = async (ids) => {
    try {
      const removed = await invoke('delete_feeds', { ids })
      await fetchFeeds()
      return removed
    } catch (error) {
      console.error('FlipboardStore: deleteFeeds failed:', error)
      throw error
    }
  }

  // 后台抓取总开关：开/关后台调度器，并持久化用户选择。
  const setScrapingEnabled = async (enabled) => {
    try {
      await invoke('set_feed_scheduler_enabled', { enabled: !!enabled })
      scrapingEnabled.value = !!enabled
      localStorage.setItem('flipboard_scraping_enabled', enabled ? '1' : '0')
      if (!enabled) {
        // 关闭时顺带终止当前在跑的任务，避免遗留任务继续。
        try { await invoke('stop_all_feed_tasks') } catch (e) { /* ignore */ }
      }
      return scrapingEnabled.value
    } catch (error) {
      console.error('FlipboardStore: setScrapingEnabled failed:', error)
      throw error
    }
  }

  // 拉取待登录队列
  const fetchPendingLogins = async () => {
    try {
      const list = await invoke('get_pending_logins')
      pendingLogins.value = Array.isArray(list) ? list : []
      return pendingLogins.value
    } catch (error) {
      console.error('FlipboardStore: fetchPendingLogins failed:', error)
      return pendingLogins.value
    }
  }

  // 主动打开某个待登录项的浏览器，并从队列移除
  const openPendingLogin = async (item) => {
    const host = String(item?.host || '').trim()
    const url = String(item?.url || '').trim()
    if (!host || !url) return
    try {
      await invoke('open_login_session', { profile: host, loginUrl: url })
    } catch (error) {
      console.error('FlipboardStore: openPendingLogin failed:', error)
      throw error
    } finally {
      await dismissPendingLogin(host)
    }
  }

  // 从待登录队列忽略某项
  const dismissPendingLogin = async (host) => {
    try {
      await invoke('dismiss_pending_login', { host: String(host || '').trim() })
    } catch (error) {
      console.warn('FlipboardStore: dismissPendingLogin failed:', error)
    }
    pendingLogins.value = pendingLogins.value.filter(p => p.host !== host)
  }

  // 终止单个源的抓取任务
  const stopFeedTask = async (feedId) => {
    try {
      await invoke('stop_feed_task', { feedId })
      return true
    } catch (error) {
      console.error('FlipboardStore: stopFeedTask failed:', error)
      throw error
    }
  }

  // 终止全部抓取任务
  const stopAllFeedTasks = async () => {
    try {
      await invoke('stop_all_feed_tasks')
      isLoading.value = false
      isRefreshing.value = false
      return true
    } catch (error) {
      console.error('FlipboardStore: stopAllFeedTasks failed:', error)
      throw error
    }
  }

  // 批量启用/禁用订阅源
  const setFeedsEnabled = async (ids, enabled) => {
    try {
      const changed = await invoke('set_feeds_enabled', { ids, enabled })
      await fetchFeeds()
      return changed
    } catch (error) {
      console.error('FlipboardStore: setFeedsEnabled failed:', error)
      throw error
    }
  }

  // 更新信息源
  const updateFeed = async (source) => {
    try {
      console.log('FlipboardStore: updateFeed', source)
      await invoke('update_feed', { input: source })
      await fetchFeeds()
      return true
    } catch (error) {
      console.error('FlipboardStore: updateFeed failed:', error)
      throw error
    }
  }

  // 删除订阅源
  const deleteFeed = async (id) => {
    try {
      console.log('FlipboardStore: deleteFeed', { id })
      await invoke('delete_feed', { id })
      await fetchFeeds()
      return true
    } catch (error) {
      console.error('FlipboardStore: deleteFeed failed:', error)
      throw error
    }
  }

  // 添加书签（同时写入「我的收集」收藏夹，便于在收集面板中查看/管理）
  const addBookmark = (article) => {
    if (!bookmarks.value.some(b => b.id === article.id)) {
      const entry = { ...article, bookmarkedAt: new Date().toISOString() }
      bookmarks.value.push(entry)
      saveBookmarks()
      try {
        useFlipboardCollectionsStore().upsertBookmarkArticle(entry)
      } catch (e) {
        console.warn('FlipboardStore: sync bookmark to collections failed:', e)
      }
    }
  }

  // 移除书签（同时从收集库移除对应收藏条目）
  const removeBookmark = (articleId) => {
    const index = bookmarks.value.findIndex(b => b.id === articleId)
    if (index !== -1) {
      bookmarks.value.splice(index, 1)
      saveBookmarks()
      try {
        useFlipboardCollectionsStore().removeBookmarkArticle(articleId)
      } catch (e) {
        console.warn('FlipboardStore: remove bookmark from collections failed:', e)
      }
    }
  }

  // 检查是否已收藏
  const isBookmarked = (articleId) => {
    return bookmarks.value.some(b => b.id === articleId)
  }

  // 切换书签状态
  const toggleBookmark = (article) => {
    if (isBookmarked(article.id)) {
      removeBookmark(article.id)
    } else {
      addBookmark(article)
    }
  }

  // 添加到阅读历史
  const addToReadingHistory = (article) => {
    const existingIndex = readingHistory.value.findIndex(h => h.id === article.id)

    if (existingIndex !== -1) {
      // 如果已存在，移到最前面
      readingHistory.value.splice(existingIndex, 1)
    }

    readingHistory.value.unshift({
      ...article,
      readAt: new Date().toISOString()
    })

    // 最多保存100条历史记录
    if (readingHistory.value.length > 100) {
      readingHistory.value = readingHistory.value.slice(0, 100)
    }

    saveReadingHistory()
  }

  // 清空阅读历史
  const clearReadingHistory = () => {
    readingHistory.value = []
    saveReadingHistory()
  }

  // 保存书签到本地存储
  const saveBookmarks = () => {
    try {
      localStorage.setItem('flipboard_bookmarks', JSON.stringify(bookmarks.value))
    } catch (error) {
      console.error('Failed to save bookmarks:', error)
    }
  }

  // 保存阅读历史到本地存储
  const saveReadingHistory = () => {
    try {
      localStorage.setItem('flipboard_history', JSON.stringify(readingHistory.value))
    } catch (error) {
      console.error('Failed to save reading history:', error)
    }
  }

  // 从本地存储加载数据
  const loadLocalData = () => {
    try {
      const savedBookmarks = localStorage.getItem('flipboard_bookmarks')
      const savedHistory = localStorage.getItem('flipboard_history')

      if (savedBookmarks) {
        bookmarks.value = JSON.parse(savedBookmarks)
      }

      if (savedHistory) {
        readingHistory.value = JSON.parse(savedHistory)
      }

      // 启动时把收藏夹与「我的收集」收集库对齐（保证收藏卡片在收集面板可见）。
      try {
        useFlipboardCollectionsStore().syncBookmarks(bookmarks.value)
      } catch (e) {
        console.warn('FlipboardStore: syncBookmarks failed:', e)
      }
    } catch (error) {
      console.error('Failed to load local data:', error)
    }
  }

  // 切换视图模式
  const toggleViewMode = () => {
    const modes = ['grid', 'list', 'magazine']
    const currentIndex = modes.indexOf(currentView.value)
    currentView.value = modes[(currentIndex + 1) % modes.length]
    localStorage.setItem('flipboard_view_mode', currentView.value)
  }

  // 刷新文章（强制从订阅源重新抓取）
  const refreshArticles = async () => {
    console.log('FlipboardStore: manual refresh articles')
    const topic = String(searchQuery.value || '').trim()
    if (topic) {
      await fetchTopicKnowledge(topic, selectedCategory.value, true)
      return
    }
    await fetchArticles(selectedCategory.value, true)
  }

  // 计算属性
  const filteredArticles = computed(() => {
    const base = Array.isArray(articles.value) ? articles.value : []
    const category = String(selectedCategory.value || '').trim()
    const byCategory = !category || category === 'all'
      ? base
      : base.filter(article => String(article?.category || '').trim() === category)

    if (!searchQuery.value.trim()) {
      return byCategory
    }

    const query = searchQuery.value.toLowerCase()
    return byCategory.filter(article =>
      String(article?.title || '').toLowerCase().includes(query) ||
      String(article?.description || '').toLowerCase().includes(query) ||
      String(article?.content || '').toLowerCase().includes(query) ||
      String(article?.source || '').toLowerCase().includes(query)
    )
  })

  const categoryArticles = computed(() => {
    if (!selectedCategory.value) {
      return filteredArticles.value
    }
    return filteredArticles.value.filter(article =>
      article.category === selectedCategory.value
    )
  })

  const featuredArticles = computed(() => {
    return filteredArticles.value
      .filter(article => article.image_url)
      .slice(0, 5)
  })

  const recentArticles = computed(() => {
    return [...articles.value]
      .sort((a, b) => new Date(b.published_at) - new Date(a.published_at))
      .slice(0, 10)
  })

  const popularArticles = computed(() => {
    // 模拟热门文章（基于阅读历史）
    const readCounts = {}
    readingHistory.value.forEach(history => {
      readCounts[history.id] = (readCounts[history.id] || 0) + 1
    })

    return [...articles.value]
      .sort((a, b) => (readCounts[b.id] || 0) - (readCounts[a.id] || 0))
      .slice(0, 10)
  })

  // 启动清理：仅保留收藏文章，其余缓存全部删除（避免磁盘/数据库无限增长）。
  // 每次进程启动只跑一次。
  const purgeUnbookmarkedOnce = async () => {
    if (window.__flipboardPurgedThisSession) return
    window.__flipboardPurgedThisSession = true
    try {
      const keepUrls = (Array.isArray(bookmarks.value) ? bookmarks.value : [])
        .map(b => String(b?.url || '').trim())
        .filter(Boolean)
      const removed = await invoke('purge_unbookmarked_articles', { keepUrls })
      console.log('FlipboardStore: purged unbookmarked cached articles', { removed, kept: keepUrls.length })
    } catch (error) {
      console.warn('FlipboardStore: purge_unbookmarked_articles failed:', error)
    }
  }

  // 初始化
  const init = async () => {
    loadLocalData()
    // 把持久化的"后台抓取总开关"同步到后端（后端默认关闭，由用户掌控）。
    try {
      await invoke('set_feed_scheduler_enabled', { enabled: scrapingEnabled.value })
    } catch (e) {
      console.warn('FlipboardStore: apply scraping pref failed:', e)
    }
    // 后台预热渲染引擎：提前解析/探测可用浏览器（系统 Chrome/Edge 优先，必要时下载），
    // 让首次需要 JS 渲染的抓取无需等待。不阻塞主流程。
    if (!window.__flipboardEngineWarmed) {
      window.__flipboardEngineWarmed = true
      invoke('ensure_chromium_ready').catch(e =>
        console.warn('FlipboardStore: warm render engine failed:', e)
      )
    }
    // 先清理未收藏缓存，再拉取（拉取会重新填充当前分类的新鲜内容）。
    await purgeUnbookmarkedOnce()
    await fetchCategories()
    try {
      await importFeedsFromWebsiteMarkdown(null)
    } catch (error) {
      console.warn('FlipboardStore: importFeedsFromWebsiteMarkdown skipped:', error)
    }
    await fetchFeeds()
    await fetchArticles()

    // 加载保存的视图模式
    const savedViewMode = localStorage.getItem('flipboard_view_mode')
    if (savedViewMode) {
      currentView.value = savedViewMode
    }
  }

  store = {
    // 状态
    articles,
    categories,
    feeds,
    bookmarks,
    readingHistory,
    currentArticle,
    selectedCategory,
    isLoading,
    isRefreshing,
    searchQuery,
    currentView,
    feedHealth,
    collectionsPanelVisible,
    isHydratingDetail,
    pendingReaderArticle,
    openReaderForArticle,
    scrapingEnabled,
    pendingLogins,
    setScrapingEnabled,
    fetchPendingLogins,
    openPendingLogin,
    dismissPendingLogin,

    // 计算属性
    filteredArticles,
    categoryArticles,
    featuredArticles,
    recentArticles,
    popularArticles,

    // 方法
    fetchArticles,
    fetchTopicKnowledge,
    fetchArticleDetail,
    reparseArticle,
    openArticleInBrowser,
    refreshRsshubCookies,
    searchArticles,
    searchArticlesLocal,
    fetchCategories,
    fetchFeeds,
    fetchFeedHealth,
    runSchedulerTick,
    fetchFeedSchedule,
    createKbNoteFromTopic,
    importFeedsFromWebsiteMarkdown,
    addFeed,
    addSite,
    generateFeedsByPlatform,
    startSiteCrawl,
    getCrawlProgress,
    resetCrawlQueue,
    ensureChromiumReady,
    openLoginSession,
    setFeedEnabled,
    setFeedsEnabled,
    stopFeedTask,
    stopAllFeedTasks,
    updateFeed,
    deleteFeed,
    deleteFeeds,
    addBookmark,
    removeBookmark,
    isBookmarked,
    toggleBookmark,
    addToReadingHistory,
    clearReadingHistory,
    toggleViewMode,
    refreshArticles,
    init
  }

  return store
}
