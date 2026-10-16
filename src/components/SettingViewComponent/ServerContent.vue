<template>
  <div class="srv">
    <div class="srv-head">
      <div class="srv-head__text">
        <div class="srv-title">服务器</div>
        <div class="srv-sub">
          应用在后台运行的服务会列在这里。端口冲突、文档里的本地图片渲染不出来、或想按网络管控要求关掉某个端口时，
          可以在这里看状态、停掉或重启它们。
        </div>
      </div>
      <n-space align="center" :size="10">
        <n-text depth="3" class="srv-poll">每1小时自动刷新</n-text>
        <n-button size="small" tertiary :loading="refreshing" @click="refresh">刷新</n-button>
      </n-space>
    </div>

    <div v-if="!services.length" class="srv-empty">正在读取服务列表…</div>

    <template v-for="group in groupedServices" :key="group.name">
      <div class="srv-group">{{ group.name }}</div>

      <div v-for="svc in group.items" :key="svc.id" class="srv-row">
        <div class="srv-row__main">
          <div class="srv-row__line">
            <span class="srv-dot" :class="`is-${svc.status}`"></span>
            <span class="srv-name">{{ svc.label }}</span>
            <n-tag size="small" :type="statusTagType(svc.status)" :bordered="false">
              {{ statusText(svc.status) }}
            </n-tag>
            <span v-if="svc.detail" class="srv-detail">{{ svc.detail }}</span>
            <span v-if="svc.address" class="srv-addr">{{ svc.address }}</span>
          </div>
          <div class="srv-desc">{{ svc.description }}</div>
        </div>

        <div class="srv-row__actions">
          <!-- 会话池没有"自启"一说，别给它一个没有意义的开关。 -->
          <n-tooltip v-if="svc.kind !== 'session'" trigger="hover">
            <template #trigger>
              <n-switch size="small" :value="svc.autostart" :disabled="!!busy[svc.id]"
                @update:value="(value) => toggleAutostart(svc, value)">
                <template #checked>自启</template>
                <template #unchecked>停用</template>
              </n-switch>
            </template>
            关掉后，重启应用也不会自动拉起它
          </n-tooltip>
          <n-button v-if="svc.can_start" size="small" type="primary" tertiary :loading="!!busy[svc.id]"
            :disabled="svc.status === 'running'" @click="act('start', svc)">启动</n-button>
          <n-button v-if="svc.can_stop" size="small" type="error" tertiary :disabled="svc.status !== 'running'"
            @click="act('stop', svc)">{{ svc.kind === 'session' ? '全部关闭' : '停止' }}</n-button>
          <n-button v-if="svc.can_start && svc.can_stop" size="small" tertiary :disabled="!!busy[svc.id]"
            @click="act('restart', svc)">重启</n-button>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NButton, NSpace, NSwitch, NTag, NText, NTooltip, useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'

const logger = createLogger('ServerContent')
const message = useMessage()

const services = ref([])
// 按服务 id 记录"正在执行动作"，只锁住这一行，别让整页跟着转。
const busy = reactive({})
const refreshing = ref(false)

const POLL_INTERVAL_MS = 3600000
let pollTimer = null

const statusText = (status) => ({
  running: '运行中',
  stopped: '未启动',
  disabled: '已停用',
}[status] || status)

const statusTagType = (status) => ({
  running: 'success',
  stopped: 'default',
  disabled: 'warning',
}[status] || 'default')

// 按后端给的分组归拢，并保持后端返回的顺序（常驻 → 按需 → 会话），不在这里再排一遍。
const groupedServices = computed(() => {
  const order = []
  const buckets = new Map()
  for (const svc of services.value) {
    const name = svc.group || '其他'
    if (!buckets.has(name)) {
      buckets.set(name, [])
      order.push(name)
    }
    buckets.get(name).push(svc)
  }
  return order.map((name) => ({ name, items: buckets.get(name) }))
})

const refresh = async () => {
  if (refreshing.value) return
  refreshing.value = true
  try {
    services.value = await invoke('service_list')
  } catch (error) {
    logger.warn('[ServerContent] 读取服务列表失败', { error: String(error?.message || error) })
  } finally {
    refreshing.value = false
  }
}

const act = async (kind, svc) => {
  if (busy[svc.id]) return
  busy[svc.id] = true
  try {
    await invoke(`service_${kind}`, { id: svc.id })
    message.success(`已${kind === 'start' ? '启动' : kind === 'stop' ? '停止' : '重启'} ${svc.label}`)
  } catch (error) {
    // 起不来的原因（端口被占、绑定失败）后端会带出来，原样显示给用户，别吞掉。
    const text = String(error?.message || error || '操作失败')
    logger.warn(`[ServerContent] ${kind} ${svc.id} 失败`, { error: text })
    message.error(`${svc.label}：${text}`)
  } finally {
    busy[svc.id] = false
    // 动作后立刻回读，别等下一次轮询 —— 状态要跟手。
    await refresh()
  }
}

const toggleAutostart = async (svc, enabled) => {
  if (busy[svc.id]) return
  busy[svc.id] = true
  try {
    await invoke('service_set_autostart', { id: svc.id, enabled })
  } catch (error) {
    message.error(`设置自启失败：${String(error?.message || error)}`)
  } finally {
    busy[svc.id] = false
    await refresh()
  }
}

onMounted(() => {
  refresh()
  pollTimer = window.setInterval(refresh, POLL_INTERVAL_MS)
})

onBeforeUnmount(() => {
  if (pollTimer) {
    window.clearInterval(pollTimer)
    pollTimer = null
  }
})
</script>

<style scoped>
.srv {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px 2px 12px;
}

.srv-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 0 14px;
  border-bottom: 1px solid var(--guaxi-border);
}

.srv-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.srv-sub {
  margin-top: 6px;
  max-width: 720px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
}

.srv-poll {
  font-size: 11px;
  white-space: nowrap;
}

.srv-empty {
  padding: 18px 2px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.srv-group {
  margin: 14px 0 2px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  color: var(--guaxi-text-secondary);
}

.srv-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 2px;
  border-bottom: 1px solid var(--guaxi-border);
}

.srv-row:last-child {
  border-bottom: 0;
}

.srv-row__main {
  min-width: 0;
  flex: 1 1 auto;
}

.srv-row__line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

/* 状态点比整块变色克制：一眼扫到状态，又不抢文字的注意力。 */
.srv-dot {
  flex: 0 0 auto;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #94a3b8;
}

.srv-dot.is-running {
  background: #22c55e;
  box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.16);
}

.srv-dot.is-disabled {
  background: #f59e0b;
  box-shadow: 0 0 0 3px rgba(245, 158, 11, 0.16);
}

.srv-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.srv-addr {
  font-size: 11px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', monospace;
  color: var(--guaxi-text-secondary);
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 会话数量这类补充信息：给点底色，和端口/地址区分开，一眼能看出是"动态值"。 */
.srv-detail {
  flex: 0 0 auto;
  padding: 1px 7px;
  border-radius: 999px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  background: var(--guaxi-header-bg);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.srv-desc {
  margin-top: 5px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
}

.srv-row__actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
