<template>
  <div class="page">
    <div class="header">
      <div class="title">知识源管理</div>

      <n-space align="center" :wrap="true">
        <n-select v-model:value="filterCategory" size="small" clearable placeholder="筛选分类" :options="categoryOptions"
          :disabled="loading" style="width: 160px" />

        <n-input v-model:value="keyword" size="small" clearable placeholder="搜索名称 / URL" :disabled="loading"
          style="width: 240px">
          <template #prefix>
            <n-icon :component="SearchIcon" />
          </template>
        </n-input>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" circle @click="refresh" :loading="loading">
              <n-icon :component="RefreshIcon" />
            </n-button>
          </template>
          刷新列表
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" circle type="error" secondary @click="stopAll" :loading="stoppingAll">
              <n-icon :component="StopIcon" />
            </n-button>
          </template>
          终止全部抓取任务
        </n-tooltip>

        <n-button type="primary" size="small" @click="openSimpleDialog" :disabled="loading">
          <template #icon><n-icon :component="AddIcon" /></template>
          快速添加
        </n-button>

        <n-button type="primary" size="small" ghost @click="openCrawlDialog" :disabled="loading">
          <template #icon><n-icon :component="CrawlIcon" /></template>
          整站深抓
        </n-button>

        <n-button size="small" @click="openAddDialog" :disabled="loading">
          <template #icon><n-icon :component="ConfigIcon" /></template>
          高级配置
        </n-button>
      </n-space>
    </div>

    <!-- 批量操作条：选中条目后出现 -->
    <transition name="fade">
      <div v-if="checkedRowKeys.length" class="batch-bar">
        <span class="batch-count">已选 {{ checkedRowKeys.length }} 项</span>
        <n-space :size="8">
          <n-button size="small" secondary @click="batchSetEnabled(true)" :loading="batchLoading">
            <template #icon><n-icon :component="EnableIcon" /></template>
            批量启用
          </n-button>
          <n-button size="small" secondary @click="batchSetEnabled(false)" :loading="batchLoading">
            <template #icon><n-icon :component="DisableIcon" /></template>
            批量禁用
          </n-button>
          <n-button size="small" type="error" @click="batchDelete" :loading="batchLoading">
            <template #icon><n-icon :component="TrashIcon" /></template>
            批量删除
          </n-button>
          <n-button size="small" quaternary @click="checkedRowKeys = []">取消选择</n-button>
        </n-space>
      </div>
    </transition>

    <n-data-table class="table" :columns="columns" :data="filteredFeeds" :row-key="rowKey" :loading="loading"
      :pagination="pagination" :scroll-x="scrollX" :bordered="false" :striped="true"
      v-model:checked-row-keys="checkedRowKeys" />

    <FeedSourceConfigDialog :show="showDialog" :categories="categories" :default-category="filterCategory || 'all'"
      :initial-feed="editingFeed" @update:show="val => (showDialog = val)" @submit="handleDialogSubmit"
      @cancel="handleDialogCancel" />

    <n-modal v-model:show="showSimpleDialog" preset="dialog" title="快速添加知识源" :mask-closable="false"
      :close-on-esc="false" :style="{ width: '520px' }">
      <n-form :model="simpleForm">
        <n-form-item label="主站 URL">
          <n-input v-model:value="simpleForm.seedUrl" placeholder="例如 https://medium.com 或 https://www.zhihu.com" />
        </n-form-item>
        <n-form-item label="归属分类">
          <n-select v-model:value="simpleForm.categoryId" :options="categoryOptions" placeholder="选择归属分类" />
        </n-form-item>
      </n-form>
      <template #action>
        <n-space justify="end">
          <n-button @click="handleSimpleCancel" :disabled="simpleLoading">
            取消
          </n-button>
          <n-button type="primary" @click="handleSimpleSubmit" :loading="simpleLoading">
            自动发现并添加
          </n-button>
        </n-space>
      </template>
    </n-modal>

    <n-modal v-model:show="showCrawlDialog" preset="dialog" title="整站深度抓取" :mask-closable="false"
      :close-on-esc="false" :style="{ width: '560px' }">
      <n-form :model="crawlForm">
        <n-form-item label="主站 / 入口 URL">
          <n-input v-model:value="crawlForm.seedUrl" placeholder="例如 https://example.com 或某个专栏主页" />
        </n-form-item>
        <n-form-item label="归属分类">
          <n-select v-model:value="crawlForm.categoryId" :options="categoryOptions" placeholder="选择归属分类" />
        </n-form-item>
        <n-form-item label="兴趣描述（相关性筛选）">
          <n-input v-model:value="crawlForm.interest" type="textarea"
            placeholder="描述你关注的主题，系统据此为每篇内容打相关性分并排序"
            :autosize="{ minRows: 2, maxRows: 4 }" />
        </n-form-item>
        <n-grid :cols="2" :x-gap="16">
          <n-form-item-gi label="抓取深度">
            <n-input-number v-model:value="crawlForm.maxDepth" :min="1" :max="8" />
          </n-form-item-gi>
          <n-form-item-gi label="每次抓取页数">
            <n-input-number v-model:value="crawlForm.maxPagesPerTick" :min="1" :max="64" />
          </n-form-item-gi>
        </n-grid>
      </n-form>
      <template #action>
        <n-space justify="end">
          <n-button @click="showCrawlDialog = false" :disabled="crawlLoading">
            取消
          </n-button>
          <n-button type="primary" @click="handleCrawlSubmit" :loading="crawlLoading">
            创建并开始抓取
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
/**
 * FlipboardFeedManager
 * - 订阅源独立管理窗口页面（配合 /flipboard-feeds 路由 + tauri create_flipboard_feed_window 命令）
 * - 功能：查看列表、启用/禁用、编辑、删除、搜索/筛选、刷新
 *
 * 说明：
 * - 目前后端 FeedSource 字段为：id/name/url/category/enabled
 * - FeedSourceConfigDialog 收集了更多字段（sourceType/fetchInterval/keywords/remark），当前仅使用 name/url/categoryId 参与后端调用
 */
import { computed, h, onMounted, onUnmounted, ref } from 'vue'
import { NButton, NSwitch, NTag, NModal, NForm, NFormItem, NSelect, NIcon, NTooltip, darkTheme, createDiscreteApi } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import {
  SearchOutline as SearchIcon,
  RefreshOutline as RefreshIcon,
  AddCircleOutline as AddIcon,
  GlobeOutline as CrawlIcon,
  OptionsOutline as ConfigIcon,
  CreateOutline as EditIcon,
  LogInOutline as LoginIcon,
  ReloadOutline as RecrawlIcon,
  TrashOutline as TrashIcon,
  CheckmarkCircleOutline as EnableIcon,
  CloseCircleOutline as DisableIcon,
  StopCircleOutline as StopIcon
} from '@vicons/ionicons5'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import FeedSourceConfigDialog from './FeedSourceConfigDialog.vue'

const logger = createLogger('FlipboardFeedManager')

const props = defineProps({
  // 路由 query 透传：/flipboard-feeds?category=xxx
  category: {
    type: String,
    default: ''
  }
})

const store = useFlipboardStore()
const settingsStore = useSettingsStore()
// 让独立的 message/dialog 跟随应用主题（暗色模式下提示文字清晰可读）。
const discreteConfigProviderProps = computed(() => ({
  theme: settingsStore.appearanceTheme === 'dark' ? darkTheme : null
}))
const { message, dialog } = createDiscreteApi(['message', 'dialog'], {
  configProviderProps: discreteConfigProviderProps
})

const loading = ref(false)
const keyword = ref('')
const filterCategory = ref('')
const showDialog = ref(false)
const editingFeed = ref(null)

const showSimpleDialog = ref(false)
const simpleLoading = ref(false)
const simpleForm = ref({
  categoryId: 'all',
  seedUrl: ''
})

const showCrawlDialog = ref(false)
const crawlLoading = ref(false)
const crawlForm = ref({
  seedUrl: '',
  categoryId: 'all',
  interest: '',
  maxDepth: 3,
  maxPagesPerTick: 8
})

const openCrawlDialog = () => {
  crawlForm.value = {
    seedUrl: '',
    categoryId: filterCategory.value || 'all',
    interest: '',
    maxDepth: 3,
    maxPagesPerTick: 8
  }
  showCrawlDialog.value = true
}

const handleCrawlSubmit = async () => {
  const seedUrl = String(crawlForm.value.seedUrl || '').trim()
  if (!seedUrl) {
    message.error('请输入主站 / 入口 URL')
    return
  }
  try {
    crawlLoading.value = true
    const crawlCfg = JSON.stringify({
      max_depth: Number(crawlForm.value.maxDepth) || 3,
      max_pages_per_tick: Number(crawlForm.value.maxPagesPerTick) || 8
    })
    await store.startSiteCrawl({
      seedUrl,
      category: crawlForm.value.categoryId || 'all',
      interest: String(crawlForm.value.interest || '').trim(),
      crawlCfg
    })
    message.success('整站抓取源已创建，将在后台逐步抓取')
    showCrawlDialog.value = false
    await refresh()
  } catch (e) {
    logger.error('FlipboardFeedManager: start site crawl failed', e)
    message.error(String(e || '创建失败'))
  } finally {
    crawlLoading.value = false
  }
}

// 行级别的“忙碌”集合：避免用户连续点击导致重复请求
const busyIds = ref(new Set())

// 批量选择 / 批量操作
const checkedRowKeys = ref([])
const batchLoading = ref(false)
const stoppingAll = ref(false)

const stopAll = async () => {
  stoppingAll.value = true
  try {
    await store.stopAllFeedTasks()
    message.success('已请求终止全部抓取任务')
  } catch (e) {
    logger.error('FlipboardFeedManager: stop all failed', e)
    message.error(String(e || '终止失败'))
  } finally {
    stoppingAll.value = false
  }
}

const stopFeed = async (row) => {
  const id = String(row?.id || '')
  if (!id) return
  try {
    await store.stopFeedTask(id)
    message.success('已请求终止该源的抓取')
  } catch (e) {
    logger.error('FlipboardFeedManager: stop feed failed', e)
    message.error(String(e || '终止失败'))
  }
}

const batchDelete = () => {
  const ids = checkedRowKeys.value.map(String).filter(Boolean)
  if (!ids.length) return
  dialog.warning({
    title: '批量删除',
    content: `确定删除选中的 ${ids.length} 个知识源吗？此操作不可撤销。`,
    positiveText: '删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      batchLoading.value = true
      try {
        const removed = await store.deleteFeeds(ids)
        message.success(`已删除 ${removed} 个知识源`)
        checkedRowKeys.value = []
      } catch (e) {
        logger.error('FlipboardFeedManager: batch delete failed', e)
        message.error(String(e || '批量删除失败'))
      } finally {
        batchLoading.value = false
      }
    }
  })
}

const batchSetEnabled = async (enabled) => {
  const ids = checkedRowKeys.value.map(String).filter(Boolean)
  if (!ids.length) return
  batchLoading.value = true
  try {
    const changed = await store.setFeedsEnabled(ids, enabled)
    message.success(`已${enabled ? '启用' : '禁用'} ${changed} 个知识源`)
  } catch (e) {
    logger.error('FlipboardFeedManager: batch set enabled failed', e)
    message.error(String(e || '批量操作失败'))
  } finally {
    batchLoading.value = false
  }
}

// 整站抓取进度：feedId -> { pending, done, error, total }
const crawlProgress = ref({})
let crawlPollTimer = null

const pollCrawlProgress = async () => {
  const crawlFeeds = feeds.value.filter(f => String(f?.source_type || '') === 'site_crawl')
  if (!crawlFeeds.length) return
  const next = { ...crawlProgress.value }
  for (const f of crawlFeeds) {
    const id = String(f?.id || '')
    if (!id) continue
    try {
      next[id] = await store.getCrawlProgress(id)
    } catch (e) {
      // 忽略单个查询失败
    }
  }
  crawlProgress.value = next
}

const categories = computed(() => {
  const list = store.categories?.value
  return Array.isArray(list) ? list : []
})

const feeds = computed(() => {
  const list = store.feeds?.value
  return Array.isArray(list) ? list : []
})

const categoryOptions = computed(() => {
  const options = categories.value.map(x => ({
    label: x?.name || '',
    value: x?.id || ''
  })).filter(x => x.value)

  // 确保包含“全部”
  if (!options.some(o => o.value === 'all')) {
    options.unshift({ label: '全部', value: 'all' })
  }
  return options
})

const categoryName = (categoryId) => {
  if (!categoryId) return '未分类'
  const hit = categories.value.find(c => c.id === categoryId)
  return hit?.name || categoryId
}

const filteredFeeds = computed(() => {
  const list = feeds.value.slice()

  // 分类筛选：优先使用 UI 选择器，其次使用路由传入 category
  const cat = (filterCategory.value || '').trim()
  const activeCategory = cat || (props.category || '').trim()

  let out = list
  if (activeCategory && activeCategory !== 'all') {
    out = out.filter(f => String(f?.category || '') === activeCategory)
  }

  const kw = (keyword.value || '').trim().toLowerCase()
  if (kw) {
    out = out.filter(f => {
      const name = String(f?.name || '').toLowerCase()
      const url = String(f?.url || '').toLowerCase()
      const id = String(f?.id || '').toLowerCase()
      return name.includes(kw) || url.includes(kw) || id.includes(kw)
    })
  }

  return out
})

const rowKey = (row) => row?.id

const pagination = ref({
  pageSize: 12,
  showSizePicker: true,
  pageSizes: [10, 12, 20, 50, 100],
  showQuickJumper: true,
  prefix: (info) => `共 ${info.itemCount} 条`
})

const columns = computed(() => {
  const titleStyle = {
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--guaxi-text-primary)'
  }

  return [
    {
      type: 'selection',
      width: 44,
      fixed: 'left'
    },
    {
      title: () => h('span', { style: titleStyle }, '名称'),
      key: 'name',
      width: 180,
      render: (row) => h('span', { title: row?.name || '' }, row?.name || '')
    },
    {
      title: () => h('span', { style: titleStyle }, 'URL'),
      key: 'url',
      width: 360,
      render: (row) => {
        const url = row?.url || ''
        return h(
          'a',
          {
            href: url,
            target: '_blank',
            rel: 'noreferrer',
            title: url,
            style: {
              color: 'var(--guaxi-link, #4c8bf5)',
              textDecoration: 'none'
            }
          },
          url
        )
      }
    },
    {
      title: () => h('span', { style: titleStyle }, '分类'),
      key: 'category',
      width: 120,
      render: (row) => h(NTag, { size: 'small', bordered: false }, { default: () => categoryName(row?.category) })
    },
    {
      title: () => h('span', { style: titleStyle }, '启用'),
      key: 'enabled',
      width: 90,
      render: (row) => {
        const id = String(row?.id || '')
        const isBusy = busyIds.value.has(id)
        return h(NSwitch, {
          value: !!row?.enabled,
          loading: isBusy,
          disabled: isBusy,
          async onUpdateValue(val) {
            if (!id) return
            logger.info('FlipboardFeedManager: toggle enabled', { id, enabled: val })
            busyIds.value.add(id)
            try {
              await store.setFeedEnabled(id, !!val)
              message.success(val ? '已启用' : '已禁用')
            } catch (e) {
              logger.error('FlipboardFeedManager: setFeedEnabled failed', e)
              message.error(String(e || '操作失败'))
            } finally {
              busyIds.value.delete(id)
            }
          }
        })
      }
    },
    {
      title: () => h('span', { style: titleStyle }, '抓取进度'),
      key: 'crawl_progress',
      width: 140,
      render: (row) => {
        const isCrawl = String(row?.source_type || '') === 'site_crawl'
        if (!isCrawl) return h('span', { style: { color: 'var(--guaxi-text-secondary)' } }, '—')
        const p = crawlProgress.value[String(row?.id || '')]
        if (!p) return h('span', { style: { color: 'var(--guaxi-text-secondary)' } }, '等待中')
        const errPart = p.error > 0 ? `，失败 ${p.error}` : ''
        return h('span', { style: { fontSize: '12px' } }, `${p.done}/${p.total}${errPart}`)
      }
    },
    {
      title: () => h('span', { style: titleStyle }, '操作'),
      key: 'actions',
      width: 230,
      render: (row) => {
        const id = String(row?.id || '')
        const isBusy = busyIds.value.has(id)
        const isCrawl = String(row?.source_type || '') === 'site_crawl'

        const buttons = [iconAction({ icon: EditIcon, tip: '编辑', disabled: isBusy, onClick: () => openEditDialog(row) })]

        if (isCrawl) {
          buttons.push(
            iconAction({ icon: LoginIcon, tip: '登录该站', disabled: isBusy, onClick: () => handleLogin(row) }),
            iconAction({ icon: RecrawlIcon, tip: '重新抓取', disabled: isBusy, onClick: () => handleRecrawl(row) }),
            iconAction({ icon: StopIcon, tip: '终止抓取', type: 'warning', onClick: () => stopFeed(row) })
          )
        }

        buttons.push(
          iconAction({ icon: TrashIcon, tip: '删除', type: 'error', disabled: isBusy, onClick: () => confirmDelete(row) })
        )

        return h('div', { style: { display: 'flex', gap: '6px', flexWrap: 'nowrap' } }, buttons)
      }
    }
  ]
})

// 渲染一个带 tooltip 的圆形图标操作按钮（供表格 render 复用）。
const iconAction = ({ icon, tip, type, disabled, onClick }) =>
  h(
    NTooltip,
    { trigger: 'hover' },
    {
      trigger: () =>
        h(
          NButton,
          { size: 'small', circle: true, tertiary: true, type, disabled, onClick },
          { icon: () => h(NIcon, null, { default: () => h(icon) }) }
        ),
      default: () => tip
    }
  )

const scrollX = computed(() => {
  // 与 columns.width 对齐；留一点余量避免布局抖动
  return 44 + 180 + 360 + 120 + 90 + 140 + 230 + 24
})

const handleLogin = async (row) => {
  const id = String(row?.id || '')
  const url = String(row?.url || '')
  if (!id || !url) return
  busyIds.value.add(id)
  try {
    message.info('正在打开登录窗口，请在弹出的浏览器中完成登录…')
    await store.openLoginSession(id, url)
    message.success('登录会话已保存，后续抓取将复用该登录态')
  } catch (e) {
    logger.error('FlipboardFeedManager: login failed', e)
    message.error(String(e || '登录失败'))
  } finally {
    busyIds.value.delete(id)
  }
}

const handleRecrawl = async (row) => {
  const id = String(row?.id || '')
  if (!id) return
  busyIds.value.add(id)
  try {
    await store.resetCrawlQueue(id)
    message.success('已重置抓取队列，将重新遍历该站点')
  } catch (e) {
    logger.error('FlipboardFeedManager: recrawl failed', e)
    message.error(String(e || '重爬失败'))
  } finally {
    busyIds.value.delete(id)
  }
}

const refresh = async () => {
  try {
    loading.value = true
    logger.info('FlipboardFeedManager: refresh start')
    await store.fetchCategories()
    await store.fetchFeeds()
    logger.info('FlipboardFeedManager: refresh done', {
      categories: store.categories?.value?.length,
      feeds: store.feeds?.value?.length
    })
  } catch (e) {
    logger.error('FlipboardFeedManager: refresh failed', e)
    message.error(String(e || '刷新失败'))
  } finally {
    loading.value = false
  }
}

const handleSimpleSubmit = async () => {
  const categoryId = String(simpleForm.value.categoryId || 'all').trim() || 'all'
  const seedUrl = String(simpleForm.value.seedUrl || '').trim()

  if (!seedUrl) {
    message.error('请输入主站 URL')
    return
  }

  try {
    simpleLoading.value = true
    logger.info('FlipboardFeedManager: simple submit', { categoryId, seedUrl })
    const created = await store.addSite(seedUrl, categoryId)
    if (created && created.length) {
      message.success(`已自动发现并添加 ${created.length} 个信息源`)
    } else {
      message.warning('未发现可添加的信息源，请尝试其他主站 URL')
    }
    showSimpleDialog.value = false
  } catch (e) {
    logger.error('FlipboardFeedManager: simple submit failed', e)
    message.error(String(e || '自动发现失败'))
  } finally {
    simpleLoading.value = false
  }
}

const openAddDialog = () => {
  logger.info('FlipboardFeedManager: open add dialog')
  editingFeed.value = null
  showDialog.value = true
}

const openSimpleDialog = () => {
  logger.info('FlipboardFeedManager: open simple dialog')
  simpleForm.value = {
    categoryId: filterCategory.value || 'all',
    seedUrl: ''
  }
  showSimpleDialog.value = true
}

const openEditDialog = (row) => {
  logger.info('FlipboardFeedManager: open edit dialog', row)
  editingFeed.value = row ? { ...row } : null
  showDialog.value = true
}

const handleDialogCancel = () => {
  logger.info('FlipboardFeedManager: dialog cancel')
}

const handleSimpleCancel = () => {
  logger.info('FlipboardFeedManager: simple dialog cancel')
  showSimpleDialog.value = false
}

const handleDialogSubmit = async (payload) => {
  logger.info('FlipboardFeedManager: dialog submit', payload)

  const id = String(payload?.id || '').trim()
  const name = String(payload?.name || '').trim()
  const url = String(payload?.url || '').trim()
  const category = String(payload?.categoryId || 'all').trim() || 'all'
  const sourceType = String(payload?.sourceType || 'rss').trim() || 'rss'
  const fetchInterval = Number(payload?.fetchInterval || 60)
  const includeKeywords = String(payload?.includeKeywords || '').trim()
  const excludeKeywords = String(payload?.excludeKeywords || '').trim()
  const interestProfile = String(payload?.interestProfile || '').trim()
  const relevanceThreshold = Number(payload?.relevanceThreshold || 0)
  const llmEnhance = !!payload?.llmEnhance
  const remark = String(payload?.remark || '').trim()

  if (!name || !url) {
    message.error('名称和 URL 不能为空')
    return
  }

  const source = {
    id,
    name,
    url,
    category,
    source_type: sourceType,
    fetch_interval: fetchInterval,
    include_keywords: includeKeywords,
    exclude_keywords: excludeKeywords,
    interest_profile: interestProfile,
    relevance_threshold: relevanceThreshold,
    llm_enhance: llmEnhance,
    remark
  }

  try {
    loading.value = true

    if (id) {
      const original = feeds.value.find(f => String(f?.id || '') === id)
      const enabled = original ? !!original.enabled : true
      const merged = { ...source, enabled }

      logger.info('FlipboardFeedManager: update feed', merged)
      await store.updateFeed(merged)
      message.success('订阅源已更新')
    } else {
      const merged = { ...source, enabled: true }
      logger.info('FlipboardFeedManager: add feed', merged)
      await store.addFeed(merged)
      message.success('订阅源已添加')
    }

    showDialog.value = false
    editingFeed.value = null
  } catch (e) {
    logger.error('FlipboardFeedManager: add/update failed', e)
    message.error(String(e || '保存失败'))
  } finally {
    loading.value = false
  }
}

const confirmDelete = (row) => {
  const id = String(row?.id || '')
  const name = String(row?.name || '')
  if (!id) return

  dialog.warning({
    title: '确认删除',
    content: `确定要删除订阅源「${name || id}」吗？删除后将从本地配置中移除。`,
    positiveText: '删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      busyIds.value.add(id)
      try {
        logger.info('FlipboardFeedManager: delete feed', { id })
        await store.deleteFeed(id)
        message.success('已删除')
      } catch (e) {
        logger.error('FlipboardFeedManager: delete failed', e)
        message.error(String(e || '删除失败'))
      } finally {
        busyIds.value.delete(id)
      }
    }
  })
}

onMounted(async () => {
  // 初始筛选：优先使用路由传入
  filterCategory.value = (props.category || '').trim() || ''
  logger.info('FlipboardFeedManager: mounted', { category: props.category })
  await refresh()
  // 整站抓取进度轮询（每 6 秒）。
  await pollCrawlProgress()
  crawlPollTimer = setInterval(pollCrawlProgress, 6000)
})

onUnmounted(() => {
  if (crawlPollTimer) {
    clearInterval(crawlPollTimer)
    crawlPollTimer = null
  }
})
</script>

<style scoped>
.page {
  height: 100vh;
  box-sizing: border-box;
  padding: 16px;
  background: var(--guaxi-app-bg);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.title {
  font-size: 18px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.batch-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 14px;
  border-radius: 10px;
  background: linear-gradient(135deg, rgba(102, 126, 234, 0.12), rgba(118, 75, 162, 0.12));
  border: 1px solid rgba(102, 126, 234, 0.3);
}

.batch-count {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.table {
  flex: 1 1 auto;
  min-height: 0;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 8px;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
