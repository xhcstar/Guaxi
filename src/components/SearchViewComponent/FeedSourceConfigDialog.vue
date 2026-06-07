<template>
  <n-modal v-model:show="innerVisible" preset="dialog" title="配置知识源" :mask-closable="false" :close-on-esc="false"
    :style="{ width: '640px' }">
    <n-form ref="formRef" :model="form" :rules="rules" label-placement="top" label-width="auto">
      <n-grid :cols="2" :x-gap="16">
        <n-form-item-gi label="名称" path="name">
          <n-input v-model:value="form.name" placeholder="为知识源起一个名字，便于识别" />
        </n-form-item-gi>
        <n-form-item-gi label="平台类型" path="sourceType">
          <n-select v-model:value="form.sourceType" :options="sourceTypeOptions" placeholder="选择平台类型" />
        </n-form-item-gi>
      </n-grid>

      <n-form-item label="入口 URL" path="url">
        <n-input v-model:value="form.url" placeholder="请输入要抓取的知识入口 URL，例如 https://example.com" />
      </n-form-item>

      <n-grid :cols="2" :x-gap="16">
        <n-form-item-gi label="归属分类" path="categoryId">
          <n-select v-model:value="form.categoryId" :options="categoryOptions" placeholder="选择归属分类" />
        </n-form-item-gi>
        <n-form-item-gi label="抓取频率" path="fetchInterval">
          <n-select v-model:value="form.fetchInterval" :options="fetchIntervalOptions" placeholder="选择抓取频率" />
        </n-form-item-gi>
      </n-grid>

      <n-form-item label="包含关键字" path="includeKeywords">
        <n-input v-model:value="form.includeKeywords" placeholder="多个关键字使用逗号分隔，将优先抓取包含这些词的内容" />
      </n-form-item>

      <n-form-item label="排除关键字" path="excludeKeywords">
        <n-input v-model:value="form.excludeKeywords" placeholder="多个关键字使用逗号分隔，将过滤掉包含这些词的内容" />
      </n-form-item>

      <n-form-item label="兴趣描述（相关性筛选）" path="interestProfile">
        <n-input v-model:value="form.interestProfile" type="textarea"
          placeholder="描述你感兴趣的主题，例如：大模型推理优化、RAG、向量数据库。系统会据此为每篇内容打相关性分。"
          :autosize="{ minRows: 2, maxRows: 4 }" />
      </n-form-item>

      <n-grid :cols="2" :x-gap="16">
        <n-form-item-gi label="相关度阈值（低于则丢弃，0=不过滤）" path="relevanceThreshold">
          <n-slider v-model:value="form.relevanceThreshold" :min="0" :max="100" :step="5" />
        </n-form-item-gi>
        <n-form-item-gi label="LLM 增强（摘要 / 标签 / 精筛）" path="llmEnhance">
          <n-switch v-model:value="form.llmEnhance" />
        </n-form-item-gi>
      </n-grid>

      <n-form-item label="备注" path="remark">
        <n-input v-model:value="form.remark" type="textarea" placeholder="记录来源说明、登录方式、抓取规则等补充信息"
          :autosize="{ minRows: 2, maxRows: 4 }" />
      </n-form-item>
    </n-form>

    <template #action>
      <n-space justify="end">
        <n-button @click="handleCancel" :disabled="saving">
          取消
        </n-button>
        <n-button type="primary" @click="handleSubmit" :loading="saving">
          保存
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'

const logger = createLogger('FeedSourceConfigDialog')
const props = defineProps({
  show: {
    type: Boolean,
    default: false
  },
  categories: {
    type: Array,
    default: () => []
  },
  defaultCategory: {
    type: String,
    default: 'all'
  },
  initialFeed: {
    type: Object,
    default: null
  }
})

const emit = defineEmits(['update:show', 'submit', 'cancel'])

const message = useMessage()
const innerVisible = ref(props.show)
const saving = ref(false)

watch(
  () => props.show,
  (value) => {
    innerVisible.value = value
    if (value) {
      resetForm()
    }
  }
)

watch(innerVisible, (value) => {
  emit('update:show', value)
})

const formRef = ref(null)
const form = ref({
  id: '',
  name: '',
  sourceType: 'rss',
  url: '',
  categoryId: '',
  fetchInterval: 60,
  includeKeywords: '',
  excludeKeywords: '',
  interestProfile: '',
  relevanceThreshold: 0,
  llmEnhance: false,
  remark: ''
})

const sourceTypeOptions = [
  { label: 'RSS / Atom', value: 'rss' },
  { label: '整站深度抓取', value: 'site_crawl' },
  { label: 'Spider 网页探索', value: 'spider' },
  { label: '知乎', value: 'zhihu' },
  { label: 'B 站', value: 'bilibili' },
  { label: 'Quora', value: 'quora' },
  { label: '小红书', value: 'xiaohongshu' },
  { label: '微信公众号', value: 'wechat' },
  { label: '博客 / 专栏', value: 'blog' },
  { label: '论坛 / 社区', value: 'forum' },
  { label: '其他 / 自定义', value: 'custom' }
]

const fetchIntervalOptions = [
  { label: '每 5 分钟', value: 5 },
  { label: '每 15 分钟', value: 15 },
  { label: '每 30 分钟', value: 30 },
  { label: '每 1 小时', value: 60 },
  { label: '每 3 小时', value: 180 },
  { label: '每天一次', value: 1440 }
]

const categoryOptions = computed(() => {
  const list = props.categories && Array.isArray(props.categories)
    ? props.categories
    : []
  const options = list.map((item) => ({
    label: item.name,
    value: item.id
  }))
  if (!options.some((option) => option.value === 'all')) {
    options.unshift({
      label: '全部',
      value: 'all'
    })
  }
  return options
})

const rules = {
  name: [
    {
      required: true,
      message: '请输入订阅源名称',
      trigger: 'blur'
    }
  ],
  sourceType: [
    {
      required: true,
      message: '请选择平台类型',
      trigger: 'change'
    }
  ],
  url: [
    {
      required: true,
      message: '请输入入口 URL',
      trigger: ['blur', 'input']
    },
    {
      validator: (_, value) => {
        if (!value) return true
        const trimmed = String(value).trim()
        if (!trimmed.startsWith('http://') && !trimmed.startsWith('https://')) {
          return false
        }
        return true
      },
      message: 'URL 必须以 http:// 或 https:// 开头',
      trigger: ['blur', 'input']
    }
  ],
  categoryId: [
    {
      required: true,
      message: '请选择归属分类',
      trigger: 'change'
    }
  ],
  fetchInterval: [
    {
      type: 'number',
      required: true,
      message: '请选择抓取频率',
      trigger: 'change'
    }
  ]
}

const resetForm = () => {
  const initial = props.initialFeed || {}
  const sourceType = initial.source_type || initial.sourceType || 'rss'
  const fetchInterval = initial.fetch_interval ?? 60
  const includeKeywords = initial.include_keywords || ''
  const excludeKeywords = initial.exclude_keywords || ''
  const remark = initial.remark || ''

  form.value = {
    id: initial.id || '',
    name: initial.name || '',
    sourceType,
    url: initial.url || '',
    categoryId: initial.category || props.defaultCategory || 'all',
    fetchInterval,
    includeKeywords,
    excludeKeywords,
    interestProfile: initial.interest_profile || '',
    relevanceThreshold: Number.isFinite(initial.relevance_threshold) ? initial.relevance_threshold : 0,
    llmEnhance: !!initial.llm_enhance,
    remark
  }
}

const handleCancel = () => {
  logger.info('FeedSourceConfigDialog: cancel')
  innerVisible.value = false
  emit('cancel')
}

const handleSubmit = async () => {
  if (!formRef.value) {
    return
  }
  try {
    saving.value = true
    await formRef.value.validate()
    const payload = {
      id: form.value.id,
      name: form.value.name.trim(),
      sourceType: form.value.sourceType,
      url: form.value.url.trim(),
      categoryId: form.value.categoryId || props.defaultCategory || 'all',
      fetchInterval: Number(form.value.fetchInterval),
      includeKeywords: form.value.includeKeywords.trim(),
      excludeKeywords: form.value.excludeKeywords.trim(),
      interestProfile: form.value.interestProfile.trim(),
      relevanceThreshold: Number(form.value.relevanceThreshold) || 0,
      llmEnhance: !!form.value.llmEnhance,
      remark: form.value.remark.trim()
    }
    logger.info('FeedSourceConfigDialog: submit', payload)
    emit('submit', payload)
  } catch (error) {
    logger.error('FeedSourceConfigDialog: validate or submit failed', error)
    message.error('表单校验失败，请检查输入')
  } finally {
    saving.value = false
  }
}
</script>
