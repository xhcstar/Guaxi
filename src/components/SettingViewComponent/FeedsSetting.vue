<template>
  <div class="feeds-set">
    <div class="feeds-set-head">
      <div>
        <div class="feeds-set-title">
          <n-icon :component="FeedIcon" />知识采集
        </div>
        <div class="feeds-set-sub">
          抓取、清洗、Markdown 落盘与保留策略。这些设置决定"存什么、存多久、占多少磁盘"。
        </div>
      </div>
      <n-button size="small" :loading="loading" @click="reload">刷新</n-button>
    </div>

    <!-- 存储 -->
    <n-collapse :default-expanded-names="['storage', 'retention', 'pipeline']">
      <n-collapse-item name="storage">
        <template #header>
          <span class="feeds-sec"><n-icon :component="FolderIcon" />存储</span>
        </template>
        <div class="feeds-rows">
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">Markdown 双写</div>
              <div class="feeds-row-hint">
                开启后正文会写成 Markdown 文件，作为知识库的一部分被索引与备份。
                关闭则只保留数据库缓存。
              </div>
            </div>
            <n-switch
              size="small"
              :value="settings.markdown_enabled"
              @update:value="(v) => patch('settings', 'markdown_enabled', v)"
            />
          </div>

          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">库目录名</div>
              <div class="feeds-row-hint">知识库根下的目录名，默认 Feeds。</div>
            </div>
            <n-input
              size="small"
              style="width: 160px"
              :value="settings.dir_name"
              @update:value="(v) => patchLocal('settings', 'dir_name', v)"
              @blur="flushLocal"
            />
          </div>

          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">图片本地化</div>
              <div class="feeds-row-hint">
                把正文图片下载到本地 <code>_assets/</code>。离线可读、源站删图不残缺，
                但会占用磁盘。
              </div>
            </div>
            <n-switch
              size="small"
              :value="settings.localize_images"
              @update:value="(v) => patch('settings', 'localize_images', v)"
            />
          </div>

          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">单图上限</div>
              <div class="feeds-row-hint">超过此大小的图片不下载。</div>
            </div>
            <n-input-number
              size="small"
              style="width: 130px"
              :min="1"
              :max="200"
              :value="Math.round(settings.max_image_bytes / 1024 / 1024)"
              @update:value="(v) => patch('settings', 'max_image_bytes', Math.round(v || 0) * 1024 * 1024)"
            >
              <template #suffix>MB</template>
            </n-input-number>
          </div>

          <div class="feeds-stat-line" v-if="storage">
            当前库：<code>{{ storage.root }}</code><br />
            文档 <b>{{ storage.documents }}</b> 篇 · 图片 <b>{{ storage.images }}</b> 张 ·
            占用 <b>{{ formatBytes(storage.bytes) }}</b>
          </div>
        </div>
      </n-collapse-item>

      <!-- 保留策略 -->
      <n-collapse-item name="retention">
        <template #header>
          <span class="feeds-sec"><n-icon :component="TimeIcon" />保留策略</span>
        </template>
        <div class="feeds-note">
          这是<b>不囤积</b>的最后一道闸：前两道（质量门、每日上限）限制新增，这一道限制存量。<br />
          回收按<b>"读没读过"</b>判断，不按质量分——质量分是启发式判断，按它删会误删冷门好文。
          删除先进回收站，可在宽限期内撤销。
        </div>
        <div class="feeds-rows">
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">未读保留</div>
              <div class="feeds-row-hint">从未打开的文章，超过这个天数进入回收站。</div>
            </div>
            <n-select
              size="small"
              style="width: 130px"
              :value="retention.unread_days"
              :options="dayOptions"
              @update:value="(v) => patch('retention', 'unread_days', v)"
            />
          </div>
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">已读保留</div>
              <div class="feeds-row-hint">打开过的文章保留多久。</div>
            </div>
            <n-select
              size="small"
              style="width: 130px"
              :value="retention.read_days"
              :options="readDayOptions"
              @update:value="(v) => patch('retention', 'read_days', v)"
            />
          </div>
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">撤销宽限期</div>
              <div class="feeds-row-hint">回收站内容保留多久后物理删除。</div>
            </div>
            <n-input-number
              size="small"
              style="width: 130px"
              :min="1"
              :max="168"
              :value="retention.undo_hours"
              @update:value="(v) => patch('retention', 'undo_hours', v)"
            >
              <template #suffix>小时</template>
            </n-input-number>
          </div>
        </div>
      </n-collapse-item>

      <!-- 抓取管线 -->
      <n-collapse-item name="pipeline">
        <template #header>
          <span class="feeds-sec"><n-icon :component="FilterIcon" />质量门与吞吐</span>
        </template>
        <div class="feeds-rows">
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">入盘质量门</div>
              <div class="feeds-row-hint">
                低于此分的文章不写文件。质量分由正文完整度、干净度、元数据、标题一致四项加权，
                并受页面类型封顶（商品页最高 15 分）。
              </div>
            </div>
            <n-input-number
              size="small"
              style="width: 100px"
              :min="0"
              :max="100"
              :value="settings.min_quality"
              @update:value="(v) => patch('settings', 'min_quality', v)"
            />
          </div>
          <div class="feeds-row">
            <div class="feeds-row-main">
              <div class="feeds-row-label">每日入盘上限</div>
              <div class="feeds-row-hint">每个主题每天最多写入多少篇。0 表示不限。</div>
            </div>
            <n-input-number
              size="small"
              style="width: 120px"
              :min="0"
              :max="5000"
              :value="settings.daily_admit_cap"
              @update:value="(v) => patch('settings', 'daily_admit_cap', v)"
            />
          </div>
        </div>

        <div class="feeds-stat-line" v-if="stats.length">
          管线状态：
          <span v-for="(s, i) in stats" :key="s[0]" class="feeds-stat-chip">
            {{ stateLabel(s[0]) }} <b>{{ s[1] }}</b><span v-if="i < stats.length - 1"> · </span>
          </span>
        </div>
      </n-collapse-item>

      <!-- 回收站 -->
      <n-collapse-item name="trash">
        <template #header>
          <span class="feeds-sec"><n-icon :component="TrashIcon" />回收站（{{ trashed.length }}）</span>
        </template>
        <div class="feeds-note">
          被保留策略回收的文章会先放在这里，宽限期内可以撤销。
        </div>
        <div v-if="!trashed.length" class="feeds-empty">回收站是空的。</div>
        <div v-else class="feeds-trash-list">
          <div v-for="t in trashed" :key="t.url_norm" class="feeds-trash-row">
            <div class="feeds-trash-main">
              <div class="feeds-trash-title">{{ t.title || '(无标题)' }}</div>
              <div class="feeds-trash-meta">{{ t.source }} · {{ formatWhen(t.purged_at) }}</div>
            </div>
            <n-button size="tiny" :loading="restoring === t.url_norm" @click="restore(t)">撤销</n-button>
          </div>
        </div>
        <div class="feeds-actions">
          <n-button size="small" :loading="purging" @click="runPurge">
            立即执行清理
          </n-button>
          <span class="feeds-hint">按当前保留策略回收过期文章（进入回收站，可撤销）。</span>
        </div>
      </n-collapse-item>
    </n-collapse>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { NIcon, useMessage } from 'naive-ui'
import {
  FolderOpen20Regular,
  Clock20Regular,
  Filter20Regular,
  Delete20Regular,
  BroadActivityFeed20Regular,
} from '@vicons/fluent'
import { useFlipboardStore } from '../../stores/flipboardStore'

const FolderIcon = FolderOpen20Regular
const TimeIcon = Clock20Regular
const FilterIcon = Filter20Regular
const TrashIcon = Delete20Regular
const FeedIcon = BroadActivityFeed20Regular

const store = useFlipboardStore()
const message = useMessage()

const loading = ref(false)
const purging = ref(false)
const restoring = ref('')

const settings = reactive({
  markdown_enabled: true,
  dir_name: 'Feeds',
  min_quality: 35,
  daily_admit_cap: 200,
  localize_images: true,
  max_image_bytes: 8 * 1024 * 1024,
  max_images_per_article: 30,
})

const retention = reactive({ unread_days: 21, read_days: 180, undo_hours: 24 })
const stats = ref([])
const storage = ref(null)
const trashed = ref([])

const dayOptions = [
  { label: '14 天', value: 14 },
  { label: '21 天', value: 21 },
  { label: '30 天', value: 30 },
  { label: '60 天', value: 60 },
  { label: '90 天', value: 90 },
  { label: '不清理', value: 0 },
]
const readDayOptions = [
  { label: '90 天', value: 90 },
  { label: '180 天', value: 180 },
  { label: '365 天', value: 365 },
  { label: '永久', value: 0 },
]

const stateLabel = (key) => {
  const m = {
    discovered: '待处理', fetched: '处理中', extracted: '已抽取',
    ready: '就绪', rejected: '已拒收', failed: '失败', needs_login: '等登录',
  }
  return m[key] || key
}

const formatBytes = (n) => {
  const b = Number(n) || 0
  if (b < 1024) return `${b} B`
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
  if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`
  return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const formatWhen = (ts) => {
  if (!ts) return ''
  const d = new Date(Number(ts) * 1000)
  return d.toLocaleString()
}

// 本地值先变（输入框要跟手），失焦或变更后再落盘。
const pendingSettings = ref({})
const patchLocal = (group, key, value) => {
  settings[key] = value
  pendingSettings.value = { ...pendingSettings.value, [key]: value }
}
const flushLocal = async () => {
  if (!Object.keys(pendingSettings.value).length) return
  const payload = pendingSettings.value
  pendingSettings.value = {}
  await persist({ ...settings, ...payload }, null)
}

const patch = async (group, key, value) => {
  if (group === 'settings') settings[key] = value
  else retention[key] = value
  await persist(
    group === 'settings' ? settings : null,
    group === 'retention' ? retention : null,
  )
}

const persist = async (s, r) => {
  try {
    await store.saveFeedsSettings(s ? { ...s } : null, r ? { ...r } : null)
    message.success('已保存', { duration: 1200 })
  } catch (e) {
    message.error(`保存失败：${e?.message || e}`, { duration: 2500 })
  }
}

const reload = async () => {
  loading.value = true
  try {
    const res = await store.fetchFeedsSettings()
    Object.assign(settings, res?.settings || {})
    Object.assign(retention, res?.retention || {})
    stats.value = await store.fetchPipelineStats()
    storage.value = await store.fetchFeedsStorage()
    trashed.value = await store.fetchTrashed(50)
  } catch (e) {
    message.error(`读取设置失败：${e?.message || e}`, { duration: 2500 })
  } finally {
    loading.value = false
  }
}

const restore = async (item) => {
  restoring.value = item.url_norm
  try {
    await store.restoreTrashed(item.url_norm)
    trashed.value = await store.fetchTrashed(50)
    storage.value = await store.fetchFeedsStorage()
    message.success('已恢复', { duration: 1500 })
  } catch (e) {
    message.error(`恢复失败：${e?.message || e}`, { duration: 3000 })
  } finally {
    restoring.value = ''
  }
}

const runPurge = async () => {
  purging.value = true
  try {
    const n = await store.purgeExpired()
    trashed.value = await store.fetchTrashed(50)
    storage.value = await store.fetchFeedsStorage()
    message.success(n > 0 ? `已回收 ${n} 篇（可在回收站撤销）` : '没有到期的文章', { duration: 2500 })
  } catch (e) {
    message.error(`清理失败：${e?.message || e}`, { duration: 3000 })
  } finally {
    purging.value = false
  }
}

onMounted(reload)
</script>

<style scoped>
.feeds-set {
  padding: 4px 2px 24px;
}

.feeds-set-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}

.feeds-set-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.feeds-set-sub {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  line-height: 1.6;
}

.feeds-sec {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 600;
}

.feeds-note {
  margin: 4px 0 10px;
  padding: 8px 10px;
  border-left: 2px solid var(--guaxi-border);
  font-size: 12px;
  line-height: 1.65;
  color: var(--guaxi-text-secondary);
  background: var(--guaxi-content-bg);
  border-radius: 0 6px 6px 0;
}

.feeds-rows {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.feeds-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 6px 0;
  border-bottom: 1px dashed var(--guaxi-border);
}

.feeds-row:last-child {
  border-bottom: none;
}

.feeds-row-main {
  min-width: 0;
  flex: 1;
}

.feeds-row-label {
  font-size: 13px;
  color: var(--guaxi-text-primary);
}

.feeds-row-hint {
  margin-top: 2px;
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--guaxi-text-secondary);
}

.feeds-stat-line {
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--guaxi-border);
  font-size: 12px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
}

.feeds-stat-chip b {
  color: var(--guaxi-text-primary);
}

.feeds-empty {
  padding: 12px 0;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.feeds-trash-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 280px;
  overflow: auto;
}

.feeds-trash-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
}

.feeds-trash-main {
  min-width: 0;
}

.feeds-trash-title {
  font-size: 12.5px;
  color: var(--guaxi-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.feeds-trash-meta {
  margin-top: 2px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.feeds-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 10px;
}

.feeds-hint {
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

code {
  padding: 1px 4px;
  border-radius: 4px;
  background: var(--guaxi-content-bg);
  font-size: 11.5px;
}
</style>
