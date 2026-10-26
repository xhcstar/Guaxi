<template>
  <!-- 地址悬停卡片：发件人 / 收件人 / 抄送上的任意地址都可以悬停出这张卡。
       定位用 fixed + 视口坐标，避免被阅读区的滚动容器裁掉。 -->
  <Teleport to="body">
    <div
      v-if="card"
      class="mail-addr-card"
      :style="{ left: `${card.x}px`, top: `${card.y}px` }"
      @mouseenter="cancelHide"
      @mouseleave="scheduleHide"
    >
      <div class="mail-addr-card-head">
        <span
          class="mail-avatar mail-addr-card-avatar"
          :style="{ background: `hsl(${seed.hue} 42% 48%)` }"
        >
          {{ seed.letter }}
        </span>
        <div class="mail-addr-card-who">
          <span class="mail-addr-card-name">{{ name }}</span>
          <span class="mail-addr-card-addr">{{ card.data.email }}</span>
        </div>
        <button class="mail-addr-card-close" title="关闭" @click="hide">×</button>
      </div>

      <div v-if="card.data.organization || card.data.title" class="mail-addr-card-org">
        {{ [card.data.organization, card.data.title].filter(Boolean).join(' · ') }}
      </div>

      <div class="mail-addr-card-stats">
        <span v-if="card.data.messageCount">
          往来 {{ card.data.messageCount }} 封
          <template v-if="card.data.lastSeenAt">
            · 最近 {{ relative(card.data.lastSeenAt) }}
          </template>
        </span>
        <span v-else>还没有往来邮件</span>
        <span v-if="card.data.contactId" class="mail-addr-card-tag">已在通讯录</span>
      </div>

      <div class="mail-addr-card-actions">
        <button class="mail-btn" @click="copyEmail">复制地址</button>
        <button v-if="!card.data.contactId" class="mail-btn is-primary" @click="saveContact">
          加进通讯录
        </button>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { computed, onBeforeUnmount, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { avatarSeed } from './theme'

const HIDE_DELAY = 260

/** 当前悬停的地址：`{ data, x, y }`。`data` 来自后端的一次性批量查询。 */
const card = ref(null)
let hideTimer = null

const seed = computed(() => {
  const data = card.value?.data || {}
  return avatarSeed(data.displayName || data.headerName, data.email)
})

/** 显示名：通讯录优先，其次邮件头里的名字，最后只留地址。 */
const name = computed(() => {
  const data = card.value?.data || {}
  return data.displayName?.trim() || data.headerName?.trim() || data.email || ''
})

function cancelHide() {
  clearTimeout(hideTimer)
}

function scheduleHide() {
  cancelHide()
  hideTimer = setTimeout(() => (card.value = null), HIDE_DELAY)
}

function hide() {
  clearTimeout(hideTimer)
  card.value = null
}

/**
 * 打开某地址的卡片。`rect` 是触发元素的位置：卡片贴在该元素下方，
 * 靠近窗口底部时翻到上方，靠近右缘时向左收，避免被裁掉。
 */
function show(address, data, rect) {
  if (!address) return
  cancelHide()
  const width = 280
  const height = 168
  const x = Math.max(8, Math.min(rect.left, window.innerWidth - width - 12))
  const below = rect.bottom + 6
  const y = below + height > window.innerHeight ? Math.max(8, rect.top - height - 6) : below
  const payload = data || emptyCard(address)
  if (card.value?.data?.email === payload.email) {
    // 同一张卡只挪位置，不重新渲染（鼠标在同一行地址之间移动时不闪）。
    card.value = { data: payload, x, y }
    return
  }
  card.value = { data: payload, x, y }
}

function emptyCard(address) {
  return reactive({
    email: address,
    displayName: '',
    contactId: '',
    organization: '',
    title: '',
    starred: false,
    messageCount: 0,
    lastSeenAt: 0,
    headerName: '',
  })
}

async function copyEmail() {
  const text = card.value?.data?.email
  if (!text) return
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
  } catch {
    navigator.clipboard?.writeText?.(text)
  }
}

async function saveContact() {
  const data = card.value?.data
  if (!data?.email) return
  try {
    const saved = await invoke('mail_save_address_contact', {
      email: data.email,
      displayName: data.headerName || data.email,
    })
    card.value = {
      ...card.value,
      data: {
        ...data,
        contactId: saved.id,
        displayName: saved.displayName || data.displayName,
      },
    }
  } catch {
    /* 保存失败不弹窗：地址还在卡片上，用户重试即可 */
  }
}

function relative(unix) {
  if (!unix) return ''
  const days = Math.floor((Date.now() / 1000 - unix) / 86400)
  if (days <= 0) return '今天'
  if (days === 1) return '昨天'
  if (days < 30) return `${days} 天前`
  if (days < 365) return `${Math.floor(days / 30)} 个月前`
  return `${Math.floor(days / 365)} 年前`
}

onBeforeUnmount(() => clearTimeout(hideTimer))

defineExpose({ show, hide, cancelHide, scheduleHide, card })
</script>
