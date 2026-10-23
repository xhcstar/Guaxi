<template>
  <header class="mail-toolbar">
    <div class="mail-toolbar-left">
      <button
        v-if="sidebarCollapsed"
        class="mail-icon-btn"
        title="展开账户与文件夹栏"
        aria-label="展开账户与文件夹栏"
        @click="$emit('toggle-sidebar')"
      >
        <n-icon :component="MenuOutline" />
      </button>
      <span class="mail-toolbar-count" v-if="unread">{{ unread }} 封未读</span>
      <span class="mail-toolbar-count is-muted" v-else>没有未读邮件</span>
      <span class="mail-toolbar-sync" :class="{ 'is-stale': stale }">
        {{ syncLabel }}
      </span>
    </div>

    <div class="mail-toolbar-actions">
      <button class="mail-btn is-primary" @click="$emit('compose')">
        <n-icon :component="CreateOutline" />
        <span>写邮件</span>
      </button>
      <span class="mail-toolbar-sep"></span>

      <button
        class="mail-icon-btn"
        :disabled="syncing"
        title="同步收件"
        aria-label="同步收件"
        @click="$emit('sync')"
      >
        <n-icon v-if="!syncing" :component="RefreshOutline" />
        <span v-else class="mail-spin"></span>
      </button>
      <button
        class="mail-icon-btn"
        :disabled="syncing"
        title="重新发现文件夹"
        aria-label="重新发现文件夹"
        @click="$emit('refresh')"
      >
        <n-icon :component="FolderOpenOutline" />
      </button>

      <span class="mail-toolbar-sep"></span>

      <button class="mail-icon-btn" title="通讯录" aria-label="通讯录" @click="$emit('contacts')">
        <n-icon :component="PeopleOutline" />
      </button>
      <button class="mail-icon-btn" title="收信规则" aria-label="收信规则" @click="$emit('rules')">
        <n-icon :component="FunnelOutline" />
      </button>
      <button class="mail-icon-btn" title="账户" aria-label="账户" @click="$emit('accounts')">
        <n-icon :component="PersonCircleOutline" />
      </button>
    </div>
  </header>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { NIcon } from 'naive-ui'
import {
  CreateOutline,
  FolderOpenOutline,
  FunnelOutline,
  MenuOutline,
  PeopleOutline,
  PersonCircleOutline,
  RefreshOutline,
} from '@vicons/ionicons5'

const props = defineProps({
  syncing: { type: Boolean, default: false },
  lastSyncAt: { type: Number, default: 0 },
  unread: { type: Number, default: 0 },
  sidebarCollapsed: { type: Boolean, default: false },
})
defineEmits([
  'sync',
  'refresh',
  'compose',
  'accounts',
  'rules',
  'contacts',
  'toggle-sidebar',
])

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
  align-items: center;
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
  gap: 4px;
  margin-left: auto;
}

/* 图标按钮：比文字按钮紧凑得多，也不会因为中英文混排长短不齐。 */
.mail-toolbar-actions .mail-icon-btn {
  width: 30px;
  height: 30px;
  font-size: 17px;
}

.mail-toolbar-actions .mail-btn.is-primary {
  gap: 6px;
  padding: 0 12px;
}

.mail-toolbar-actions .mail-btn.is-primary :deep(.n-icon) {
  font-size: 16px;
}

/* .mail-toolbar-sep 的样式放在 mail.css（全局）：阅读区工具栏也用同一个类，
   写在 scoped 块里另一个组件拿不到。 */
</style>
