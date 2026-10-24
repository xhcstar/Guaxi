<template>
  <div class="flipboard-header">
    <div class="header-content">
      <!-- 左侧品牌和菜单 -->
      <div class="header-left">
        <GuaxiSiderToggle view-key="search" side="left" />
        <div class="logo">
          <span class="logo-text">KnowGuaxi</span>
        </div>
      </div>

      <!-- 中间搜索框 -->
      <div class="header-center">
        <n-input round placeholder="搜索知识主题、术语、问题..." clearable class="search-input" :value="searchQuery"
          @update:value="handleSearch">
          <template #prefix>
            <n-icon :component="SearchIcon" />
          </template>
        </n-input>
      </div>

      <!-- 右侧操作按钮 -->
      <div class="header-right">
        <n-button-group>
          <n-tooltip placement="bottom" trigger="hover">
            <template #trigger>
              <n-button circle size="small" @click="setViewMode('magazine')"
                :type="currentView === 'magazine' ? 'primary' : 'default'">
                <n-icon>
                  <MagazineIcon />
                </n-icon>
              </n-button>
            </template>
            杂志视图
          </n-tooltip>

          <n-tooltip placement="bottom" trigger="hover">
            <template #trigger>
              <n-button circle size="small" @click="setViewMode('grid')"
                :type="currentView === 'grid' ? 'primary' : 'default'">
                <n-icon>
                  <GridIcon />
                </n-icon>
              </n-button>
            </template>
            网格视图
          </n-tooltip>

          <n-tooltip placement="bottom" trigger="hover">
            <template #trigger>
              <n-button circle size="small" @click="setViewMode('list')"
                :type="currentView === 'list' ? 'primary' : 'default'">
                <n-icon>
                  <ListIcon />
                </n-icon>
              </n-button>
            </template>
            列表视图
          </n-tooltip>
        </n-button-group>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="header-circle-btn" circle size="small" secondary @click="refreshHealth" :disabled="isLoading" :type="healthType">
              <n-icon>
                <HealthIcon />
              </n-icon>
            </n-button>
          </template>
          <div class="health-tooltip">
            <div class="health-title">抓取健康</div>
            <div class="health-line">总源：{{ healthSummary.total }} / 成功：{{ healthSummary.ok }} / 失败：{{ healthSummary.failed }}</div>
            <div class="health-line">RSS：{{ healthSummary.rss }} / Spider：{{ healthSummary.spider }} / HTML：{{ healthSummary.html }} / 其他：{{ healthSummary.fallback }}</div>
            <div v-if="healthSummary.failedItems.length" class="health-failed">
              <div class="health-line">最近失败：</div>
              <div v-for="x in healthSummary.failedItems" :key="x.feed_id" class="health-failed-item">
                {{ x.feed_name }}：{{ x.last_error }}
              </div>
            </div>
            <div v-else class="health-line">最近无失败记录</div>
          </div>
        </n-tooltip>

        <!-- 后台抓取总开关 -->
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="header-circle-btn" circle size="small" @click="toggleScraping"
              :type="scrapingEnabled ? 'success' : 'default'" :loading="scrapingBusy">
              <n-icon>
                <PauseIcon v-if="scrapingEnabled" />
                <PlayIcon v-else />
              </n-icon>
            </n-button>
          </template>
          {{ scrapingEnabled ? '后台抓取：开（点击关闭）' : '后台抓取：关（点击开启）' }}
        </n-tooltip>

        <!-- 待登录队列 -->
        <n-popover trigger="click" placement="bottom-end" :show="loginPopoverVisible"
          @update:show="onLoginPopoverToggle" :z-index="11000">
          <template #trigger>
            <n-badge :value="pendingLogins.length" :max="99" :show="pendingLogins.length > 0">
              <n-button class="header-circle-btn" circle size="small"
                :type="pendingLogins.length ? 'warning' : 'default'">
                <n-icon>
                  <LoginIcon />
                </n-icon>
              </n-button>
            </n-badge>
          </template>
          <div class="login-queue">
            <div class="login-queue-title">待登录站点</div>
            <div v-if="!pendingLogins.length" class="login-queue-empty">暂无需要登录的站点</div>
            <div v-for="item in pendingLogins" :key="item.host" class="login-queue-item">
              <span class="login-queue-host" :title="item.url">{{ item.host }}</span>
              <div class="login-queue-actions">
                <n-button size="tiny" type="primary" @click="openLogin(item)">打开登录</n-button>
                <n-button size="tiny" tertiary @click="ignoreLogin(item)">忽略</n-button>
              </div>
            </div>
          </div>
        </n-popover>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="header-circle-btn" circle size="small" @click="toggleCollections"
              :type="collectionsVisible ? 'primary' : 'default'">
              <n-icon>
                <CollectionsIcon />
              </n-icon>
            </n-button>
          </template>
          我的收集
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button class="header-circle-btn" circle size="small" @click="refreshArticles" :disabled="isLoading">
              <n-icon :class="{ 'refresh-spin': isLoading }">
                <RefreshIcon />
              </n-icon>
            </n-button>
          </template>
          刷新知识流
        </n-tooltip>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, ref, onMounted, onBeforeUnmount } from 'vue'
import { useMessage } from 'naive-ui'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
import {
  SearchOutline,
  GridOutline,
  ListOutline,
  NewspaperOutline,
  RefreshOutline,
  PulseOutline,
  BookmarksOutline,
  PlayOutline,
  PauseOutline,
  LogInOutline
} from '@vicons/ionicons5'

const logger = createLogger('FlipboardHeader')
const SearchIcon = SearchOutline
const GridIcon = GridOutline
const ListIcon = ListOutline
const MagazineIcon = NewspaperOutline
const RefreshIcon = RefreshOutline
const HealthIcon = PulseOutline
const CollectionsIcon = BookmarksOutline
const PlayIcon = PlayOutline
const PauseIcon = PauseOutline
const LoginIcon = LogInOutline

const store = useFlipboardStore()
const message = useMessage()

// 后台抓取总开关
const scrapingEnabled = computed(() => store.scrapingEnabled.value)
const scrapingBusy = ref(false)
const toggleScraping = async () => {
  scrapingBusy.value = true
  try {
    const next = !store.scrapingEnabled.value
    await store.setScrapingEnabled(next)
    message.success(next ? '已开启后台抓取' : '已关闭后台抓取', { duration: 1500 })
  } catch (e) {
    message.error('切换失败', { duration: 1500 })
  } finally {
    scrapingBusy.value = false
  }
}

// 待登录队列
const pendingLogins = computed(() => store.pendingLogins.value)
const loginPopoverVisible = ref(false)
const onLoginPopoverToggle = (v) => {
  loginPopoverVisible.value = v
  if (v) store.fetchPendingLogins()
}
const openLogin = async (item) => {
  try {
    await store.openPendingLogin(item)
    message.info('已打开登录浏览器，请完成登录后关闭该窗口', { duration: 4000 })
  } catch (e) {
    message.error('打开登录失败', { duration: 2000 })
  }
  loginPopoverVisible.value = false
}
const ignoreLogin = async (item) => {
  await store.dismissPendingLogin(item.host)
}

let loginPollTimer = null
onMounted(() => {
  store.fetchPendingLogins()
  loginPollTimer = setInterval(() => store.fetchPendingLogins(), 8000)
})
onBeforeUnmount(() => {
  if (loginPollTimer) clearInterval(loginPollTimer)
})

const searchQuery = computed(() => store.searchQuery.value)
const currentView = computed(() => store.currentView.value)
const isLoading = computed(() => store.isLoading.value)
const collectionsVisible = computed(() => store.collectionsPanelVisible.value)

const toggleCollections = () => {
  store.collectionsPanelVisible.value = !store.collectionsPanelVisible.value
}

const feedHealth = computed(() => {
  const list = store.feedHealth?.value
  return Array.isArray(list) ? list : []
})

const healthSummary = computed(() => {
  const list = feedHealth.value
  const total = list.length

  let ok = 0
  let failed = 0
  let rss = 0
  let spider = 0
  let html = 0
  let fallback = 0

  const failedItems = []

  for (const h of list) {
    const err = h?.last_error
    if (err) {
      failed += 1
      failedItems.push(h)
    } else {
      ok += 1
    }

    const method = String(h?.last_method || '').trim()
    if (method === 'rss') rss += 1
    else if (method.startsWith('spider')) spider += 1
    else if (method === 'html') html += 1
    else if (method) fallback += 1
  }

  failedItems.sort((a, b) => String(b?.last_attempt_at || '').localeCompare(String(a?.last_attempt_at || '')))

  return {
    total,
    ok,
    failed,
    rss,
    spider,
    html,
    fallback,
    failedItems: failedItems.slice(0, 3)
  }
})

const healthType = computed(() => {
  if (!healthSummary.value.total) return 'default'
  if (healthSummary.value.failed) return 'error'
  if (healthSummary.value.fallback) return 'warning'
  return 'success'
})

const handleSearch = (query) => {
  store.searchQuery.value = query
  if (query.trim()) {
    store.searchArticles(query)
  } else {
    store.fetchArticles(store.selectedCategory.value)
  }
}

const setViewMode = (mode) => {
  store.currentView.value = mode
  localStorage.setItem('flipboard_view_mode', mode)
}

const refreshArticles = () => {
  store.refreshArticles()
}

const refreshHealth = async () => {
  try {
    logger.info('FlipboardHeader: refreshHealth')
    await store.fetchFeedHealth()
  } catch (e) {
    logger.error('FlipboardHeader: refreshHealth failed', e)
  }
}
</script>

<style scoped>
.flipboard-header {
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  height: 64px;
  position: sticky;
  top: 0;
  z-index: 1000;
}

.header-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 100%;
  padding: 0 24px;
  max-width: 100%;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.header-icon {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: var(--guaxi-panel-bg);
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 700;
  font-size: 20px;
}

.logo-icon {
  font-size: 24px;
}

.logo-text {
  background: linear-gradient(135deg, #667eea, #764ba2);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.header-center {
  flex: 1;
  max-width: 500px;
  margin: 0 40px;
}

.search-input {
  width: 100%;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.header-circle-btn {
  width: 32px;
  min-width: 32px;
  height: 32px;
  padding: 0;
}

.header-circle-btn :deep(.n-button__content) {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-circle-btn :deep(.n-button__icon) {
  margin-right: 0 !important;
}

.header-circle-btn :deep(.n-base-loading) {
  margin-right: 0 !important;
}

.refresh-spin {
  animation: guaxi-refresh-spin 0.9s linear infinite;
}

@keyframes guaxi-refresh-spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

.health-tooltip {
  max-width: 420px;
  white-space: normal;
  line-height: 1.35;
}

.health-title {
  font-weight: 700;
  margin-bottom: 6px;
}

.health-line {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.health-failed {
  margin-top: 8px;
}

.health-failed-item {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-right .n-button {
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  transition: all 0.2s ease;
}

.header-right .n-button:hover {
  background: var(--guaxi-content-bg);
  border-color: var(--guaxi-border);
}

.login-queue {
  min-width: 260px;
  max-width: 340px;
}

.login-queue-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--guaxi-text-primary);
}

.login-queue-empty {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 8px 0;
}

.login-queue-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 6px 0;
  border-top: 1px solid var(--guaxi-border);
}

.login-queue-host {
  font-size: 13px;
  color: var(--guaxi-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 160px;
}

.login-queue-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
</style>
