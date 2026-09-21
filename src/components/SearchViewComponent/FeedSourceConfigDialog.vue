<template>
  <n-modal v-model:show="innerVisible" preset="dialog" title="配置知识源" :mask-closable="false" :close-on-esc="false"
    :style="{ width: '640px' }">
    <n-collapse class="rsshub-quick" style="margin-bottom: 16px;">
      <n-collapse-item title="RSSHub 全网订阅（任意网页 / 全部路由）" name="rsshub">
        <n-tabs type="segment" size="small" v-model:value="rsshubTab">
          <!-- 方式一：粘贴任意网页 URL，Radar 自动探测可用路由（浏览器插件同款能力） -->
          <n-tab-pane name="radar" tab="网页探测">
            <n-input-group>
              <n-input v-model:value="rsshubRadar.pageUrl" placeholder="粘贴任意网页地址，如 https://space.bilibili.com/2"
                @keyup.enter="handleRadarDetect" />
              <n-button type="primary" :loading="rsshubRadar.loading" @click="handleRadarDetect">探测</n-button>
            </n-input-group>
            <div class="rsshub-note" style="margin-top:6px;">
              自动识别该网站在 RSSHub 中可订阅的内容（类似 RSSHub Radar 浏览器插件）。
            </div>
            <div v-if="rsshubRadar.matches.length" class="radar-results">
              <div v-for="(m, i) in rsshubRadar.matches" :key="i" class="radar-item"
                :class="{ active: rsshubRadar.selected === i }" @click="selectRadarMatch(i)">
                <div class="radar-item-title">{{ m.title }}</div>
                <div class="radar-item-path">{{ m.filledTarget || m.target }}</div>
                <div class="radar-item-missing" v-if="m.missingParams.length">
                  需补参数：{{ m.missingParams.join('、') }}
                </div>
              </div>
            </div>
            <div v-else-if="rsshubRadar.searched && !rsshubRadar.loading" class="rsshub-note" style="margin-top:8px;">
              未在该域名找到可用的 RSSHub 路由。可试试「自定义路由」，或换一个页面地址。
            </div>
          </n-tab-pane>

          <!-- 方式二：精选常用平台 -->
          <n-tab-pane name="preset" tab="常用平台">
            <n-grid :cols="2" :x-gap="12">
              <n-form-item-gi label="平台 / 路由">
                <n-select v-model:value="rsshub.route" :options="rsshubRouteOptions" placeholder="选择平台"
                  @update:value="onRsshubRouteChange" />
              </n-form-item-gi>
              <n-form-item-gi :label="rsshubParamLabel" v-if="rsshubSelected && rsshubSelected.param">
                <n-input v-model:value="rsshub.param" :placeholder="rsshubSelected.paramHint || '填写参数'" />
              </n-form-item-gi>
            </n-grid>
          </n-tab-pane>

          <!-- 方式三：自定义路由（充分使用 RSSHub 全部 1900+ 路由） -->
          <n-tab-pane name="custom" tab="自定义路由">
            <n-input v-model:value="rsshubCustom.path"
              placeholder="填 RSSHub 路由路径或完整地址，如 /github/issue/DIYgod/RSSHub 或 https://rsshub.app/..." />
            <div class="rsshub-note" style="margin-top:6px;">
              支持粘贴 RSSHub 文档里的路由路径。路由大全见 docs.rsshub.app（本地已内置全部路由）。
            </div>
          </n-tab-pane>
        </n-tabs>

        <!-- 通用参数（对所有 RSSHub 路由生效） -->
        <div class="rsshub-params">
          <n-space align="center" :size="12">
            <span class="rsshub-params-label">通用参数：</span>
            <n-tooltip trigger="hover"><template #trigger>
              <n-input-number v-model:value="rsshubParams.limit" :min="0" :max="200" size="small" placeholder="条数"
                style="width:110px" />
            </template>limit：限制返回条目数（0=不限）</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <n-switch v-model:value="rsshubParams.fulltext" size="small">
                <template #checked>全文</template><template #unchecked>全文</template>
              </n-switch>
            </template>mode=fulltext：抓取全文（部分路由支持）</n-tooltip>
            <n-input v-model:value="rsshubParams.filter" size="small" placeholder="filter 关键词（可选）"
              style="width:180px" />
          </n-space>
        </div>

        <div class="rsshub-tip" v-if="rsshubPreviewUrl">
          <span>将生成：{{ rsshubPreviewUrl }}</span>
        </div>
        <n-space justify="end" style="margin-top:8px;">
          <n-button size="small" :loading="rsshubRefreshing" @click="handleRefreshRsshubCookies">
            刷新登录态
          </n-button>
          <n-button size="small" type="primary" :disabled="!rsshubCanApply" @click="applyRsshubRoute">
            填入下方表单
          </n-button>
        </n-space>
        <div class="rsshub-note">
          登录墙平台（知乎/小红书/微博）需先在阅读页完成一次登录，再点「刷新登录态」让 RSSHub 读取 cookie。
        </div>
      </n-collapse-item>
    </n-collapse>

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
import { useFlipboardStore } from '../../stores/flipboardStore.js'

const logger = createLogger('FeedSourceConfigDialog')
const flipboardStore = useFlipboardStore()
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

// RSSHub 本地 sidecar 基址（与 rsshub_server.rs 的 RSSHUB_PORT=1200 对齐）。
const RSSHUB_BASE = 'http://127.0.0.1:1200'

// 精选常用 RSSHub 路由。{param} 为占位；param 为空的路由无需额外输入。
// label 展示名，name 生成的订阅源默认名，param 参数占位键，paramHint 输入提示。
// 路由已对照实际构建的 RSSHub（commit f99e982）route 定义逐条核实。
const RSSHUB_ROUTES = [
  { label: '知乎 · 热榜', name: '知乎热榜', route: '/zhihu/hot', param: '' },
  { label: '知乎 · 用户动态', name: '知乎用户', route: '/zhihu/people/activities/{param}', param: 'param', paramHint: '用户 ID（url_token），如 zhang-san' },
  { label: '微博 · 用户', name: '微博用户', route: '/weibo/user/{param}', param: 'param', paramHint: '用户 UID（纯数字）' },
  { label: 'B站 · UP主投稿', name: 'B站UP主', route: '/bilibili/user/video/{param}', param: 'param', paramHint: 'UP主 UID（纯数字）' },
  { label: 'B站 · 综合热门', name: 'B站热门', route: '/bilibili/popular/all', param: '' },
  { label: '小红书 · 用户笔记', name: '小红书用户', route: '/xiaohongshu/user/{param}/notes', param: 'param', paramHint: '用户 ID' },
  { label: '少数派 · 首页', name: '少数派', route: '/sspai/index', param: '' },
  { label: 'V2EX · 最热', name: 'V2EX最热', route: '/v2ex/topics/hot', param: '' },
  { label: 'GitHub · Trending', name: 'GitHub Trending', route: '/github/trending/daily/any', param: '' },
  { label: 'Twitter/X · 用户', name: 'X用户', route: '/twitter/user/{param}', param: 'param', paramHint: '用户名（不带 @）' },
  { label: 'YouTube · 频道', name: 'YouTube频道', route: '/youtube/channel/{param}', param: 'param', paramHint: '频道 ID（UC 开头）' },
]

// 当前 RSSHub 子方式：radar 网页探测 / preset 常用平台 / custom 自定义路由。
const rsshubTab = ref('radar')

const rsshub = ref({ route: null, param: '' })
const rsshubCustom = ref({ path: '' })
const rsshubRadar = ref({ pageUrl: '', loading: false, searched: false, matches: [], selected: -1 })

// 通用参数（对所有 RSSHub 路由生效）。
const rsshubParams = ref({ limit: null, fulltext: false, filter: '' })

const rsshubRouteOptions = RSSHUB_ROUTES.map((r) => ({ label: r.label, value: r.route }))

const rsshubSelected = computed(() =>
  RSSHUB_ROUTES.find((r) => r.route === rsshub.value.route) || null
)

const rsshubParamLabel = computed(() => rsshubSelected.value?.paramHint ? '参数' : '参数')

// 把路由路径 + 通用参数拼成完整本地 URL。
const buildRsshubUrl = (routePath) => {
  const path = String(routePath || '').trim()
  if (!path) return ''
  const clean = path.startsWith('/') ? path : `/${path}`
  const qs = []
  const limit = Number(rsshubParams.value.limit)
  if (Number.isFinite(limit) && limit > 0) qs.push(`limit=${limit}`)
  if (rsshubParams.value.fulltext) qs.push('mode=fulltext')
  const filter = String(rsshubParams.value.filter || '').trim()
  if (filter) qs.push(`filter=${encodeURIComponent(filter)}`)
  return `${RSSHUB_BASE}${clean}${qs.length ? `?${qs.join('&')}` : ''}`
}

// 当前选中方式对应的"路由路径"（未加通用参数）。
const currentRoutePath = computed(() => {
  if (rsshubTab.value === 'preset') {
    const sel = rsshubSelected.value
    if (!sel) return ''
    return sel.param
      ? sel.route.replace('{param}', encodeURIComponent(String(rsshub.value.param || '').trim()))
      : sel.route
  }
  if (rsshubTab.value === 'custom') {
    let p = String(rsshubCustom.value.path || '').trim()
    if (!p) return ''
    // 允许粘贴完整 rsshub 地址，取其 path+query。
    if (/^https?:\/\//i.test(p)) {
      try { const u = new URL(p); p = u.pathname + u.search } catch { /* 保持原样 */ }
    }
    return p
  }
  // radar
  const m = rsshubRadar.value.matches[rsshubRadar.value.selected]
  return m ? (m.filledTarget || m.target) : ''
})

const rsshubPreviewUrl = computed(() => buildRsshubUrl(currentRoutePath.value))

const rsshubCanApply = computed(() => {
  const path = currentRoutePath.value
  if (!path) return false
  // radar：选中项若仍有未填参数（:xxx）则不可应用。
  if (rsshubTab.value === 'radar' && /:[A-Za-z]/.test(path)) return false
  // preset：带参数的必须填。
  if (rsshubTab.value === 'preset' && rsshubSelected.value?.param) {
    return String(rsshub.value.param || '').trim().length > 0
  }
  return true
})

const onRsshubRouteChange = () => {
  rsshub.value.param = ''
}

// Radar 探测：调后端代理，把返回的按子域名分组规则拍平成候选列表。
const handleRadarDetect = async () => {
  const pageUrl = String(rsshubRadar.value.pageUrl || '').trim()
  if (!pageUrl) return
  rsshubRadar.value.loading = true
  rsshubRadar.value.searched = false
  rsshubRadar.value.matches = []
  rsshubRadar.value.selected = -1
  try {
    const data = await flipboardStore.rsshubRadarDetect(pageUrl)
    // data 形如 { "_name": "...", "www": [ { title, target, source } ], ".": [...] }
    const urlPath = (() => { try { return new URL(pageUrl).pathname } catch { return '' } })()
    const matches = []
    Object.keys(data || {}).forEach((sub) => {
      if (sub === '_name') return
      const arr = Array.isArray(data[sub]) ? data[sub] : []
      arr.forEach((rule) => {
        // 尝试用当前页面路径回填 target 里的 :参数（source 是带 :param 的模式）。
        const filledTarget = tryFillTarget(rule, urlPath)
        const target = String(rule.target || '')
        const missingParams = (String(filledTarget || target).match(/:[A-Za-z_]+\??/g) || [])
          .filter((s) => !s.endsWith('?'))
          .map((s) => s.replace(/[:?]/g, ''))
        matches.push({
          title: rule.title || '订阅',
          target,
          filledTarget: filledTarget && filledTarget !== target ? filledTarget : '',
          missingParams,
        })
      })
    })
    // 完整可用（无缺参）的排前面。
    matches.sort((a, b) => a.missingParams.length - b.missingParams.length)
    rsshubRadar.value.matches = matches
    // 默认选中第一个完整可用的。
    const firstReady = matches.findIndex((m) => m.missingParams.length === 0)
    rsshubRadar.value.selected = firstReady >= 0 ? firstReady : (matches.length ? 0 : -1)
    if (!matches.length) message.info('未找到该网站的 RSSHub 路由')
  } catch (e) {
    message.warning(`探测失败：${e?.message || e}（RSSHub 可能未构建）`)
  } finally {
    rsshubRadar.value.searched = true
    rsshubRadar.value.loading = false
  }
}

// 用页面路径回填 target 的 :参数：source 模式如 /:uid 对应页面 /12345 → uid=12345。
const tryFillTarget = (rule, urlPath) => {
  try {
    const sources = Array.isArray(rule.source) ? rule.source : [rule.source]
    let target = String(rule.target || '')
    if (!target.includes(':')) return target
    for (const src of sources) {
      const srcStr = String(src || '')
      // 把 source 模式转成正则，捕获各 :param。
      const names = []
      const re = new RegExp('^' + srcStr.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
        .replace(/\\\/:([A-Za-z_]+)\\\?/g, (_, n) => { names.push(n); return '(?:/([^/]+))?' })
        .replace(/:([A-Za-z_]+)/g, (_, n) => { names.push(n); return '([^/]+)' }) + '/?$')
      const mm = urlPath.match(re)
      if (mm) {
        names.forEach((n, i) => {
          const v = mm[i + 1]
          if (v) target = target.replace(new RegExp(`:${n}\\??`), v)
        })
        // 去掉未匹配到的可选参数占位。
        target = target.replace(/\/:[A-Za-z_]+\?/g, '')
        return target
      }
    }
    return target
  } catch {
    return String(rule.target || '')
  }
}

const selectRadarMatch = (i) => {
  const m = rsshubRadar.value.matches[i]
  if (!m) return
  if (m.missingParams.length) {
    message.warning(`该订阅还需补参数：${m.missingParams.join('、')}，可用"自定义路由"手动补全`)
  }
  rsshubRadar.value.selected = i
}

const applyRsshubRoute = () => {
  const path = currentRoutePath.value
  if (!path || !rsshubCanApply.value) return
  form.value.sourceType = 'rss'
  form.value.url = rsshubPreviewUrl.value
  if (!form.value.name.trim()) {
    if (rsshubTab.value === 'preset' && rsshubSelected.value) {
      const sel = rsshubSelected.value
      form.value.name = sel.param ? `${sel.name} · ${String(rsshub.value.param).trim()}` : sel.name
    } else if (rsshubTab.value === 'radar' && rsshubRadar.value.selected >= 0) {
      form.value.name = rsshubRadar.value.matches[rsshubRadar.value.selected]?.title || 'RSSHub 订阅'
    } else {
      form.value.name = `RSSHub · ${path.split('/').filter(Boolean)[0] || '订阅'}`
    }
  }
  message.success('已填入表单，可继续补充分类等信息后保存')
}

const rsshubRefreshing = ref(false)
const handleRefreshRsshubCookies = async () => {
  rsshubRefreshing.value = true
  try {
    const ok = await flipboardStore.refreshRsshubCookies()
    if (ok) {
      message.success('已重启 RSSHub 并读取最新登录态')
    } else {
      message.warning('RSSHub 未就绪（可能未构建）；其余源不受影响')
    }
  } finally {
    rsshubRefreshing.value = false
  }
}

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
  // 每次打开重置 RSSHub 快捷选择器（含三种方式 + 通用参数）。
  rsshubTab.value = 'radar'
  rsshub.value = { route: null, param: '' }
  rsshubCustom.value = { path: '' }
  rsshubRadar.value = { pageUrl: '', loading: false, searched: false, matches: [], selected: -1 }
  rsshubParams.value = { limit: null, fulltext: false, filter: '' }
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

<style scoped>
.rsshub-tip {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
  word-break: break-all;
  margin: 4px 0 8px;
}

.rsshub-note {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #94a3b8);
  margin-top: 8px;
  line-height: 1.5;
}

.rsshub-params {
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px dashed var(--guaxi-border, rgba(0, 0, 0, 0.1));
}

.rsshub-params-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.radar-results {
  margin-top: 10px;
  max-height: 220px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.radar-item {
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border, rgba(0, 0, 0, 0.1));
  cursor: pointer;
  transition: all 0.15s ease;
}

.radar-item:hover {
  border-color: var(--guaxi-primary, #2563eb);
}

.radar-item.active {
  border-color: var(--guaxi-primary, #2563eb);
  background: rgba(37, 99, 235, 0.06);
}

.radar-item-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary, #1f2328);
}

.radar-item-path {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
  word-break: break-all;
  margin-top: 2px;
}

.radar-item-missing {
  font-size: 11px;
  color: #f0a020;
  margin-top: 2px;
}
</style>
