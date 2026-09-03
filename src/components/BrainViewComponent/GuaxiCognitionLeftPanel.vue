<template>
  <div class="cog-left">
    <div class="cl-head">
      <span class="cl-title">大脑</span>
      <n-tag :bordered="false" size="small" :type="cog.enabled ? 'success' : 'default'">
        {{ cog.enabled ? '运行中' : '已关闭' }}
      </n-tag>
      <n-button size="tiny" quaternary circle :loading="cog.statusLoading" @click="reload">
        <template #icon><n-icon :component="ArrowSync20Regular" /></template>
      </n-button>
    </div>

    <n-collapse :default-expanded-names="['nudge', 'source', 'brief']">
      <n-collapse-item name="nudge" title="待处理">
        <div v-if="!cog.openNudgeCount" class="cl-empty">
          没有待处理的提醒。系统仍在后台观察感知收件箱与知识库的变化。
        </div>
        <div v-else class="cl-rows">
          <div
            class="cl-row"
            :class="{ on: cog.nudgeFilterKind === '' }"
            @click="pickKind('')"
          >
            <span class="cl-row-label">全部</span>
            <span class="cl-row-num">{{ cog.openNudgeCount }}</span>
          </div>
          <div
            v-for="k in kindRows"
            :key="k.kind"
            class="cl-row"
            :class="{ on: cog.nudgeFilterKind === k.kind }"
            :title="k.hint"
            @click="pickKind(k.kind)"
          >
            <span class="cl-dot" :style="{ background: k.color }"></span>
            <span class="cl-row-label">{{ k.label }}</span>
            <span class="cl-row-num">{{ k.count }}</span>
          </div>
        </div>
        <div class="cl-note">
          今日已打断 {{ st?.nudge?.deliveredToday ?? 0 }} 次，还剩
          {{ st?.nudge?.interruptsLeftToday ?? 0 }} 次额度。
          够不上门槛的提醒不会消失，只会降级进简报。
        </div>
      </n-collapse-item>

      <n-collapse-item name="source" title="感知来源">
        <div class="cl-rows">
          <div
            class="cl-row"
            :class="{ on: !cog.perceptFilter.sourceKind }"
            @click="pickSource('')"
          >
            <span class="cl-row-label">全部</span>
            <span class="cl-row-num">{{ st?.percept?.newCount ?? 0 }}</span>
          </div>
          <div
            v-for="s in sourceRows"
            :key="s.kind"
            class="cl-row"
            :class="{ on: cog.perceptFilter.sourceKind === s.kind }"
            @click="pickSource(s.kind)"
          >
            <span class="cl-row-label">{{ s.label }}</span>
            <span class="cl-row-num">{{ s.count }}</span>
          </div>
        </div>

        <!-- 今日自动采集额度：这一层的目标是让阅读清单变短，所以额度必须可见。 -->
        <div class="cl-quota">
          <div class="cl-quota-top">
            <span>今日自动采集</span>
            <b>{{ st?.percept?.todayAuto ?? 0 }} / {{ capText }}</b>
          </div>
          <div class="cl-quota-bar"><i :style="{ width: quotaPct }"></i></div>
          <div class="cl-note">
            超出额度的抓取内容不会入库。未读的抓取内容会在
            {{ st?.percept?.unreadKeepDays ?? 1 }} 天后自动清除，
            被引用过的条目不会被清。
          </div>
        </div>
      </n-collapse-item>

      <n-collapse-item name="brief" title="简报">
        <div v-if="!cog.briefHistory.length" class="cl-empty">
          还没有简报。它会在画像里设定的时刻自动生成。
        </div>
        <div v-else class="cl-rows">
          <div
            v-for="b in cog.briefHistory"
            :key="b.day + b.kind"
            class="cl-row"
            :class="{ on: cog.brief?.day === b.day }"
            @click="openBrief(b.day)"
          >
            <span class="cl-row-label">{{ b.day }}</span>
            <span v-if="!b.opened" class="cl-unread">新</span>
          </div>
        </div>
      </n-collapse-item>

      <n-collapse-item name="memory" title="记忆与知识">
        <div class="cl-stats">
          <div class="cl-stat">
            <span>活跃主张</span><b>{{ st?.belief?.active ?? 0 }}</b>
          </div>
          <div class="cl-stat">
            <span>已被反驳</span><b>{{ st?.belief?.contradicted ?? 0 }}</b>
          </div>
          <div class="cl-stat">
            <span>行为轨迹</span><b>{{ st?.episode?.total ?? 0 }}</b>
          </div>
          <div class="cl-stat">
            <span>引证被用上</span>
            <b>{{ st?.episode?.citationUsed ?? 0 }} / {{ st?.episode?.citationTotal ?? 0 }}</b>
          </div>
        </div>
        <div v-if="!cog.embeddingReady" class="cl-warn">
          本地语义服务未就绪。召回、新颖度打分、矛盾检测都依赖它——
          请先在图谱视图里为知识库生成一次向量。
        </div>
      </n-collapse-item>

      <n-collapse-item name="actions" title="快捷操作">
        <div class="cl-actions">
          <n-button size="small" block :loading="busy.deliberate" @click="deliberate">
            立即审议
          </n-button>
          <n-button size="small" block :loading="cog.briefLoading" @click="makeBrief">
            生成今日简报
          </n-button>
          <n-button
            v-if="cog.pendingScoreCount"
            size="small"
            block
            :loading="busy.score"
            @click="scoreNow"
          >
            打分 {{ cog.pendingScoreCount }} 条待处理
          </n-button>
          <n-button size="small" block quaternary :loading="busy.purge" @click="purge">
            清理未读抓取内容
          </n-button>
        </div>
      </n-collapse-item>
    </n-collapse>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { ArrowSync20Regular } from '@vicons/fluent'
import {
  useCognitionStore,
  NUDGE_KINDS,
  PERCEPT_SOURCE_LABELS,
} from '../../stores/cognitionStore'

const cog = useCognitionStore()
const message = useMessage()
const st = computed(() => cog.status)
const busy = ref({ deliberate: false, score: false, purge: false })

const kindRows = computed(() =>
  Object.entries(NUDGE_KINDS)
    .map(([kind, meta]) => ({
      kind,
      label: meta.label,
      color: meta.color,
      hint: meta.hint,
      count: cog.nudges.filter((n) => n.kind === kind).length,
    }))
    .filter((k) => k.count > 0),
)

const sourceRows = computed(() =>
  (st.value?.percept?.byKind || [])
    .map(([kind, count]) => ({
      kind,
      count,
      label: PERCEPT_SOURCE_LABELS[kind] || kind,
    }))
    .filter((s) => s.count > 0),
)

const capText = computed(() => {
  const cap = st.value?.percept?.dailyCap ?? 0
  return cap > 0 ? String(cap) : '不限'
})
const quotaPct = computed(() => {
  const cap = st.value?.percept?.dailyCap ?? 0
  if (cap <= 0) return '0%'
  const used = st.value?.percept?.todayAuto ?? 0
  return `${Math.min(100, Math.round((used / cap) * 100))}%`
})

function pickKind(kind) {
  cog.nudgeFilterKind = cog.nudgeFilterKind === kind ? '' : kind
  cog.setViewMode('cognition')
}

function pickSource(kind) {
  cog.refreshPercepts({ sourceKind: kind })
  cog.setViewMode('cognition')
}

async function openBrief(day) {
  await cog.loadBrief(day)
  cog.setViewMode('cognition')
}

async function reload() {
  await cog.bootstrap()
  await cog.refreshBriefHistory()
}

async function deliberate() {
  busy.value.deliberate = true
  try {
    const r = await cog.deliberateNow()
    message.success(`审议完成：新增 ${r.produced} 条，投递 ${r.delivered} 条`)
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.deliberate = false
  }
}

async function makeBrief() {
  try {
    await cog.generateBrief('morning')
    await cog.refreshBriefHistory()
    message.success('简报已生成')
  } catch (e) {
    message.error(String(e))
  }
}

async function scoreNow() {
  busy.value.score = true
  try {
    const n = await cog.scorePercepts(24)
    message.success(n ? `已打分 ${n} 条` : '没有待打分的条目')
  } catch (e) {
    message.warning(`打分暂不可用：${e}`)
  } finally {
    busy.value.score = false
  }
}

async function purge() {
  busy.value.purge = true
  try {
    const n = await cog.purgePercepts(0)
    message.success(n ? `已清理 ${n} 条未读抓取内容` : '没有可清理的内容')
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.purge = false
  }
}

onMounted(async () => {
  await cog.refreshStatus()
  if (cog.enabled) {
    await Promise.all([cog.refreshNudges(), cog.refreshBriefHistory()])
  }
})
</script>

<style scoped>
.cog-left {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 10px 12px 20px;
  box-sizing: border-box;
  font-family: var(--guaxi-editor-font-family);
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-primary);
  background: var(--guaxi-panel-bg);
}

.cl-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 10px;
}
.cl-title {
  font-size: 16px;
  font-weight: 600;
  flex: 1 1 auto;
}

.cl-rows {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.cl-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 6px 8px;
  border-radius: 5px;
  cursor: pointer;
  font-size: 14px;
  line-height: 20px;
  transition: background 0.15s ease;
}
.cl-row:hover {
  background: rgba(127, 127, 127, 0.12);
}
.cl-row.on {
  background: rgba(45, 212, 191, 0.14);
  color: var(--n-color-primary, #2dd4bf);
}
.cl-row-label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cl-row-num {
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--guaxi-text-secondary);
}
.cl-row.on .cl-row-num {
  color: inherit;
}

.cl-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: 0 0 auto;
}

.cl-unread {
  font-size: 11px;
  padding: 0 5px;
  border-radius: 8px;
  background: #e04f5f;
  color: #fff;
  line-height: 16px;
}

.cl-empty,
.cl-note {
  font-size: 13px;
  line-height: 1.9;
  color: var(--guaxi-text-secondary);
}
.cl-empty {
  padding: 10px 2px;
}
.cl-note {
  padding-top: 8px;
}

.cl-warn {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 4px;
  border-left: 3px solid rgba(217, 138, 43, 0.85);
  background: rgba(217, 138, 43, 0.1);
  font-size: 13px;
  line-height: 1.9;
}

.cl-quota {
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--guaxi-border);
}
.cl-quota-top {
  display: flex;
  justify-content: space-between;
  font-size: 14px;
  margin-bottom: 6px;
}
.cl-quota-bar {
  height: 5px;
  border-radius: 3px;
  background: rgba(127, 127, 127, 0.22);
  overflow: hidden;
}
.cl-quota-bar i {
  display: block;
  height: 100%;
  background: var(--n-color-primary, #2dd4bf);
}

.cl-stats {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.cl-stat {
  display: flex;
  justify-content: space-between;
  font-size: 14px;
  line-height: 20px;
}
.cl-stat span {
  color: var(--guaxi-text-secondary);
}
.cl-stat b {
  font-variant-numeric: tabular-nums;
}

.cl-actions {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.cog-left :deep(.n-collapse-item__header-main) {
  font-size: 15px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}
.cog-left :deep(.n-button),
.cog-left :deep(.n-tag) {
  font-family: var(--guaxi-editor-font-family);
}
</style>
