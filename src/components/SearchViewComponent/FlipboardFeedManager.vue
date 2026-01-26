<template>
  <div class="page">
    <div class="header">
      <div class="title">订阅源管理</div>

      <n-space align="center" :wrap="true">
        <n-select v-model:value="filterCategory" size="small" clearable placeholder="筛选分类" :options="categoryOptions"
          :disabled="loading" style="width: 180px" />

        <n-input v-model:value="keyword" size="small" clearable placeholder="搜索名称 / URL" :disabled="loading"
          style="width: 260px" />

        <n-button size="small" @click="refresh" :loading="loading">
          刷新
        </n-button>

        <n-button type="primary" size="small" @click="openSimpleDialog" :disabled="loading">
          快速添加信息源
        </n-button>

        <n-button size="small" @click="openAddDialog" :disabled="loading">
          高级配置
        </n-button>
      </n-space>
    </div>

    <n-data-table class="table" :columns="columns" :data="filteredFeeds" :row-key="rowKey" :loading="loading"
      :pagination="pagination" :scroll-x="scrollX" :bordered="false" :striped="true" />

    <FeedSourceConfigDialog :show="showDialog" :categories="categories" :default-category="filterCategory || 'all'"
      :initial-feed="editingFeed" @update:show="val => (showDialog = val)" @submit="handleDialogSubmit"
      @cancel="handleDialogCancel" />

    <n-modal v-model:show="showSimpleDialog" preset="dialog" title="快速添加信息源" :mask-closable="false"
      :close-on-esc="false" :style="{ width: '520px' }">
      <n-form :model="simpleForm">
        <n-form-item label="归属分类">
          <n-select v-model:value="simpleForm.categoryId" :options="categoryOptions" placeholder="选择归属分类" />
        </n-form-item>
        <n-form-item label="平台">
          <n-select v-model:value="simpleForm.platforms" :options="platformOptions" multiple
            placeholder="请选择至少一个平台" />
        </n-form-item>
      </n-form>
      <template #action>
        <n-space justify="end">
          <n-button @click="handleSimpleCancel" :disabled="simpleLoading">
            取消
          </n-button>
          <n-button type="primary" @click="handleSimpleSubmit" :loading="simpleLoading">
            生成信息源
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
/**
 * FlipboardFeedManager
 * - 订阅源独立管理窗口页面（配合 /flipboard-feeds 路由 + tauri create_flipboard_feed_window 命令）
 * - 功能：查看列表、启用/禁用、编辑、删除、搜索/筛选、刷新
 *
 * 说明：
 * - 目前后端 FeedSource 字段为：id/name/url/category/enabled
 * - FeedSourceConfigDialog 收集了更多字段（sourceType/fetchInterval/keywords/remark），当前仅使用 name/url/categoryId 参与后端调用
 */
import { computed, h, onMounted, ref } from 'vue'
import { NButton, NSwitch, NTag, NModal, NForm, NFormItem, NSelect, createDiscreteApi } from 'naive-ui'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import FeedSourceConfigDialog from './FeedSourceConfigDialog.vue'

const props = defineProps({
  // 路由 query 透传：/flipboard-feeds?category=xxx
  category: {
    type: String,
    default: ''
  }
})

const store = useFlipboardStore()
const { message, dialog } = createDiscreteApi(['message', 'dialog'])

const loading = ref(false)
const keyword = ref('')
const filterCategory = ref('')
const showDialog = ref(false)
const editingFeed = ref(null)

const showSimpleDialog = ref(false)
const simpleLoading = ref(false)
const simpleForm = ref({
  categoryId: 'all',
  platforms: []
})

const platformOptions = [
  { label: '知乎', value: 'zhihu' },
  { label: 'B 站', value: 'bilibili' }
]

// 行级别的“忙碌”集合：避免用户连续点击导致重复请求
const busyIds = ref(new Set())

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
            console.log('FlipboardFeedManager: toggle enabled', { id, enabled: val })
            busyIds.value.add(id)
            try {
              await store.setFeedEnabled(id, !!val)
              message.success(val ? '已启用' : '已禁用')
            } catch (e) {
              console.error('FlipboardFeedManager: setFeedEnabled failed', e)
              message.error(String(e || '操作失败'))
            } finally {
              busyIds.value.delete(id)
            }
          }
        })
      }
    },
    {
      title: () => h('span', { style: titleStyle }, '操作'),
      key: 'actions',
      width: 180,
      render: (row) => {
        const id = String(row?.id || '')
        const isBusy = busyIds.value.has(id)

        return h(
          'div',
          { style: { display: 'flex', gap: '8px' } },
          [
            h(
              NButton,
              {
                size: 'tiny',
                secondary: true,
                disabled: isBusy,
                onClick: () => openEditDialog(row)
              },
              { default: () => '编辑' }
            ),
            h(
              NButton,
              {
                size: 'tiny',
                type: 'error',
                secondary: true,
                disabled: isBusy,
                onClick: () => confirmDelete(row)
              },
              { default: () => '删除' }
            )
          ]
        )
      }
    }
  ]
})

const scrollX = computed(() => {
  // 与 columns.width 对齐；留一点余量避免布局抖动
  return 180 + 360 + 120 + 90 + 180 + 24
})

const refresh = async () => {
  try {
    loading.value = true
    console.log('FlipboardFeedManager: refresh start')
    await store.fetchCategories()
    await store.fetchFeeds()
    console.log('FlipboardFeedManager: refresh done', {
      categories: store.categories?.value?.length,
      feeds: store.feeds?.value?.length
    })
  } catch (e) {
    console.error('FlipboardFeedManager: refresh failed', e)
    message.error(String(e || '刷新失败'))
  } finally {
    loading.value = false
  }
}

const handleSimpleSubmit = async () => {
  const categoryId = String(simpleForm.value.categoryId || 'all').trim() || 'all'
  const platforms = Array.isArray(simpleForm.value.platforms) ? simpleForm.value.platforms.slice() : []

  if (!platforms.length) {
    message.error('请至少选择一个平台')
    return
  }

  try {
    simpleLoading.value = true
    console.log('FlipboardFeedManager: simple submit', { categoryId, platforms })
    const created = await store.generateFeedsByPlatform(platforms, categoryId)
    if (created && created.length) {
      message.success(`已为所选平台生成 ${created.length} 个信息源`)
    } else {
      message.warning('未生成新的信息源，请尝试使用高级配置')
    }
    showSimpleDialog.value = false
  } catch (e) {
    console.error('FlipboardFeedManager: simple submit failed', e)
    message.error(String(e || '生成信息源失败'))
  } finally {
    simpleLoading.value = false
  }
}

const openAddDialog = () => {
  console.log('FlipboardFeedManager: open add dialog')
  editingFeed.value = null
  showDialog.value = true
}

const openSimpleDialog = () => {
  console.log('FlipboardFeedManager: open simple dialog')
  simpleForm.value = {
    categoryId: filterCategory.value || 'all',
    platforms: []
  }
  showSimpleDialog.value = true
}

const openEditDialog = (row) => {
  console.log('FlipboardFeedManager: open edit dialog', row)
  editingFeed.value = row ? { ...row } : null
  showDialog.value = true
}

const handleDialogCancel = () => {
  console.log('FlipboardFeedManager: dialog cancel')
}

const handleSimpleCancel = () => {
  console.log('FlipboardFeedManager: simple dialog cancel')
  showSimpleDialog.value = false
}

const handleDialogSubmit = async (payload) => {
  console.log('FlipboardFeedManager: dialog submit', payload)

  const id = String(payload?.id || '').trim()
  const name = String(payload?.name || '').trim()
  const url = String(payload?.url || '').trim()
  const category = String(payload?.categoryId || 'all').trim() || 'all'
  const sourceType = String(payload?.sourceType || 'rss').trim() || 'rss'
  const fetchInterval = Number(payload?.fetchInterval || 60)
  const includeKeywords = String(payload?.includeKeywords || '').trim()
  const excludeKeywords = String(payload?.excludeKeywords || '').trim()
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
    remark
  }

  try {
    loading.value = true

    if (id) {
      const original = feeds.value.find(f => String(f?.id || '') === id)
      const enabled = original ? !!original.enabled : true
      const merged = { ...source, enabled }

      console.log('FlipboardFeedManager: update feed', merged)
      await store.updateFeed(merged)
      message.success('订阅源已更新')
    } else {
      const merged = { ...source, enabled: true }
      console.log('FlipboardFeedManager: add feed', merged)
      await store.addFeed(merged)
      message.success('订阅源已添加')
    }

    showDialog.value = false
    editingFeed.value = null
  } catch (e) {
    console.error('FlipboardFeedManager: add/update failed', e)
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
        console.log('FlipboardFeedManager: delete feed', { id })
        await store.deleteFeed(id)
        message.success('已删除')
      } catch (e) {
        console.error('FlipboardFeedManager: delete failed', e)
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
  console.log('FlipboardFeedManager: mounted', { category: props.category })
  await refresh()
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

.table {
  flex: 1 1 auto;
  min-height: 0;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 8px;
}
</style>