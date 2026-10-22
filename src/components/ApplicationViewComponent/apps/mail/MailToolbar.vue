<template>
  <header class="mail-toolbar">
    <div class="mail-toolbar-left">
      <span class="mail-toolbar-count" v-if="unread">{{ unread }} 封未读</span>
      <span class="mail-toolbar-count is-muted" v-else>没有未读邮件</span>
      <span class="mail-toolbar-sync" :class="{ 'is-stale': stale }">
        {{ syncLabel }}
      </span>
    </div>

    <div class="mail-toolbar-actions">
      <button class="mail-btn is-primary" @click="$emit('compose')">✎ 写邮件</button>
      <span class="mail-toolbar-sep"></span>
      <button class="mail-btn" :disabled="syncing" @click="$emit('sync')">
        <span v-if="syncing" class="mail-spin"></span>
        <span v-else>↻</span>
        同步
      </button>
      <button class="mail-btn" :disabled="syncing" @click="$emit('refresh')" title="重新发现文件夹">
        文件夹
      </button>
      <span class="mail-toolbar-sep"></span>
      <button class="mail-btn" @click="$emit('rules')">收信规则</button>
      <button class="mail-btn" @click="$emit('accounts')">账户</button>
      <button class="mail-btn" @click="$emit('settings')">外观</button>
    </div>
  </header>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  syncing: { type: Boolean, default: false },
  lastSyncAt: { type: Number, default: 0 },
  unread: { type: Number, default: 0 },
})
defineEmits(['sync', 'refresh', 'compose', 'accounts', 'rules', 'settings'])

// 相对时间需要每分钟重算一次；用一个本地 tick 驱动，不依赖 store 变化。
const tick = ref(Date.now())
let timer = null
onMounted(() => {
  timer = setInterval(() => (tick.value = Date.now()), 30_000)
})
onBeforeUnmount(() => clearInterval(timer))

const syncLabel = computed(() => {
  if (props.syncing) return '正在同步…'
  if (!props.lastSyncAt) return '尚未同步'
  const seconds = Math.max(0, Math.round((tick.value - props.lastSyncAt) / 1000))
  if (seconds < 60) return '刚刚同步'
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟前同步`
  const hours = Math.round(minutes / 60)
  return `${hours} 小时前同步`
})

const stale = computed(() => props.lastSyncAt && tick.value - props.lastSyncAt > 15 * 60 * 1000)
</script>

<style scoped>
.mail-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 46px;
  padding: 7px 18px;
  border-bottom: 1px solid var(--mail-border);
  background: var(--mail-bg-elevated);
}

.mail-toolbar-left {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.mail-toolbar-count {
  font-size: 13px;
  font-weight: 650;
  color: var(--mail-accent);
}
.mail-toolbar-count.is-muted {
  color: var(--mail-text-muted);
  font-weight: 500;
}

.mail-toolbar-sync {
  font-size: 11.5px;
  color: var(--mail-text-muted);
  font-variant-numeric: tabular-nums;
}
.mail-toolbar-sync.is-stale {
  color: var(--mail-warning);
}

.mail-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

/* .mail-toolbar-sep 的样式放在 mail.css（全局）：阅读区工具栏也用同一个类，
   写在 scoped 块里另一个组件拿不到。 */
</style>
