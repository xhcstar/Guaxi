import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

let store

export const useFlipboardStore = () => {
  if (store) return store
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

  // 获取文章列表（基于订阅源的真实抓取）
  const fetchArticles = async (category = null, forceRefresh = false) => {
    if (forceRefresh) {
      isRefreshing.value = true
    } else {
      isLoading.value = true
    }

    selectedCategory.value = category

    try {
      console.log('FlipboardStore: fetchArticles from feeds', { category })
      const data = await invoke('refresh_articles_from_feeds', {
        category,
        limit: 50
      })
      articles.value = Array.isArray(data) ? data : []

      try {
        await fetchFeedHealth()
      } catch (e) {
        console.warn('FlipboardStore: fetchFeedHealth failed after fetchArticles', e)
      }

      // 保存到本地存储
      localStorage.setItem('flipboard_last_fetch', new Date().toISOString())
    } catch (error) {
      console.error('Failed to fetch articles:', error)
    } finally {
      isLoading.value = false
      isRefreshing.value = false
    }
  }

  // 获取单个文章详情
  const fetchArticleDetail = async (articleId) => {
    isLoading.value = true
    try {
      const data = await invoke('get_article_detail', { id: articleId })
      currentArticle.value = data

      // 添加到阅读历史
      addToReadingHistory(data)
    } catch (error) {
      console.error('Failed to fetch article detail:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  // 搜索文章
  const searchArticles = async (query) => {
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

  // 添加书签
  const addBookmark = (article) => {
    if (!bookmarks.value.some(b => b.id === article.id)) {
      bookmarks.value.push({
        ...article,
        bookmarkedAt: new Date().toISOString()
      })
      saveBookmarks()
    }
  }

  // 移除书签
  const removeBookmark = (articleId) => {
    const index = bookmarks.value.findIndex(b => b.id === articleId)
    if (index !== -1) {
      bookmarks.value.splice(index, 1)
      saveBookmarks()
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
    await fetchArticles(selectedCategory.value, true)
  }

  // 计算属性
  const filteredArticles = computed(() => {
    if (!searchQuery.value.trim()) {
      return articles.value
    }

    const query = searchQuery.value.toLowerCase()
    return articles.value.filter(article =>
      article.title.toLowerCase().includes(query) ||
      article.description.toLowerCase().includes(query) ||
      article.content.toLowerCase().includes(query) ||
      article.source.toLowerCase().includes(query)
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

  // 初始化
  const init = async () => {
    loadLocalData()
    await fetchCategories()
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

    // 计算属性
    filteredArticles,
    categoryArticles,
    featuredArticles,
    recentArticles,
    popularArticles,

    // 方法
    fetchArticles,
    fetchArticleDetail,
    searchArticles,
    fetchCategories,
    fetchFeeds,
    fetchFeedHealth,
    addFeed,
    generateFeedsByPlatform,
    setFeedEnabled,
    updateFeed,
    deleteFeed,
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