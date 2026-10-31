<template>
  <div
    class="composer-root"
    @dragover.prevent="onHtmlDragOver"
    @dragleave="onHtmlDragLeave"
    @drop.prevent="onHtmlDrop"
  >
    <div v-if="dragging" class="drop-veil">
      <Icon :icon="paperclipIcon" width="16" height="16" />
      <span>松开以添加附件</span>
    </div>

    <div class="composer-shell" :class="{ 'composer-shell--running': pending }">
      <div v-if="attachments.length" class="attach-row">
        <div
          v-for="(a, i) in attachments"
          :key="a.id"
          class="attach-chip"
          :class="{ 'attach-chip--image': a.preview }"
          :title="a.path || a.name"
        >
          <img v-if="a.preview" class="attach-thumb" :src="a.preview" :alt="a.name" />
          <Icon v-else :icon="a.type === 'localAudio' || a.type === 'audio' ? micIcon : fileIcon" width="13" height="13" />
          <span class="attach-name">{{ a.name || baseName(a.path) }}</span>
          <button type="button" class="attach-remove" title="移除" @click="removeAttachment(i)">
            <Icon :icon="xIcon" width="10" height="10" />
          </button>
        </div>
        <span class="attach-count">{{ attachments.length }} 个附件</span>
      </div>

      <div class="input-wrap">
        <textarea
          ref="inputRef"
          v-model="text"
          class="composer-input"
          :placeholder="placeholder"
          rows="1"
          spellcheck="false"
          @keydown="onKeydown"
          @compositionstart="composing = true"
          @compositionend="composing = false"
          @input="onInput"
          @paste="onPaste"
          @blur="onBlur"
        />

        <div v-if="popup.open" class="popup" @mousedown.prevent>
          <div class="popup-head">
            <span>{{ popup.title }}</span>
            <span class="popup-hint">↑↓ 选择 · Enter 确认 · Esc 关闭</span>
          </div>
          <button
            v-for="(item, index) in popup.items"
            :key="item.key"
            type="button"
            class="popup-item"
            :class="{ 'popup-item--on': index === popup.active }"
            @mouseenter="popup.active = index"
            @click="chooseItem(item)"
          >
            <Icon :icon="item.icon || (popup.kind === 'file' ? fileIcon : commandIcon)" width="13" height="13" />
            <span class="popup-main">
              <span class="popup-label">{{ item.label }}</span>
              <span v-if="item.hint" class="popup-sub">{{ item.hint }}</span>
            </span>
            <kbd v-if="item.keyLabel" class="popup-key">{{ item.keyLabel }}</kbd>
          </button>
          <div v-if="!popup.items.length" class="popup-empty">
            {{ popup.kind === 'file' ? '没有匹配的文件' : '没有匹配的命令' }}
          </div>
        </div>
      </div>

      <div class="composer-bar">
        <div class="bar-left">
          <button type="button" class="icon-btn" title="添加文件或图片" @click="pickFiles">
            <Icon :icon="paperclipIcon" width="15" height="15" />
          </button>
          <button type="button" class="icon-btn" title="提及文件（@）" @click="startMention">
            <Icon :icon="atIcon" width="15" height="15" />
          </button>

          <n-dropdown :options="modeOptions" trigger="click" placement="top-start" @select="onModeSelect">
            <button type="button" class="bar-chip" :title="modeHint">
              <Icon :icon="sparklesIcon" width="12" height="12" />
              <span>{{ modeLabel }}</span>
            </button>
          </n-dropdown>
        </div>

        <div class="bar-right">
          <n-dropdown :options="permissionOptions" trigger="click" placement="top" @select="onPermissionSelect">
            <button type="button" class="bar-chip bar-chip--perm" :title="permissionHint">
              <Icon :icon="shieldIcon" width="12" height="12" />
              <span>{{ permission.label }}</span>
            </button>
          </n-dropdown>

          <n-dropdown :options="cwdOptions" trigger="click" placement="top" @select="onCwdSelect">
            <button type="button" class="bar-chip" :title="currentCwd || '未选择工作目录'">
              <Icon :icon="folderIcon" width="12" height="12" />
              <span class="bar-chip-text">{{ workspaceLabel }}</span>
            </button>
          </n-dropdown>

          <n-dropdown :options="modelOptions" trigger="click" placement="top" @select="onModelSelect">
            <button type="button" class="bar-chip" :title="modelHint">
              <Icon :icon="botIcon" width="12" height="12" />
              <span class="bar-chip-text">{{ modelLabel }}</span>
            </button>
          </n-dropdown>

          <n-dropdown :options="effortOptions" trigger="click" placement="top" @select="onEffortSelect">
            <button type="button" class="bar-chip bar-chip--effort" :title="'推理强度'">
              <Icon :icon="gaugeIcon" width="12" height="12" />
              <span>{{ effortLabel }}</span>
            </button>
          </n-dropdown>

          <button
            v-if="pending"
            type="button"
            class="send-btn send-btn--stop"
            title="停止（Esc）"
            @click="$emit('stop')"
          >
            <Icon :icon="squareIcon" width="13" height="13" />
          </button>
          <button
            type="button"
            class="send-btn"
            :class="{ 'send-btn--steer': pending }"
            :disabled="!canSend"
            :title="pending ? '转向（Enter）' : '发送（Enter）'"
            @click="submit"
          >
            <Icon :icon="arrowUpIcon" width="15" height="15" />
          </button>
        </div>
      </div>
    </div>

    <div class="composer-foot">
      <span class="foot-hint">{{ hint }}</span>
      <span class="foot-right">
        <span v-if="contextPercent !== null" class="foot-ctx" :title="`上下文已用 ${contextPercent}%`">
          上下文 {{ contextPercent }}%
        </span>
        <span class="foot-mode">{{ modeLabel }}</span>
      </span>
    </div>
  </div>
</template>

<script setup>
/**
 * 输入区。
 *
 * 这里是整个工作台唯一「你说话」的地方，所以承担三类东西：
 * 1. 发送：文字、图片、文件、@ 提及、/ 命令、上下键翻历史；
 * 2. 参数：模式、权限、工作目录、模型、推理强度（都随这一轮下发）；
 * 3. 中断：运行中 Enter 是「转向」，Esc 或方块按钮是停止。
 *
 * 参数不塞进输入框，而是压在底部一条 chip 上：它们描述「怎么跑」，
 * 和「要做什么」混在同一段文字里，两边都会看不清。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import paperclipIcon from '@iconify-icons/lucide/paperclip'
import atIcon from '@iconify-icons/lucide/at-sign'
import xIcon from '@iconify-icons/lucide/x'
import fileIcon from '@iconify-icons/lucide/file-text'
import micIcon from '@iconify-icons/lucide/mic'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import squareIcon from '@iconify-icons/lucide/square'
import shieldIcon from '@iconify-icons/lucide/shield'
import folderIcon from '@iconify-icons/lucide/folder'
import botIcon from '@iconify-icons/lucide/bot'
import gaugeIcon from '@iconify-icons/lucide/gauge'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import commandIcon from '@iconify-icons/lucide/command'
import { AGENT_MODES, PERMISSION_PRESETS, useAgentWorkspaceStore } from '../../stores/agentWorkspaceStore'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  pending: { type: Boolean, default: false },
})

const emit = defineEmits(['send', 'steer', 'stop', 'new', 'open-diff'])

const logger = createLogger('AgentComposer')
const message = useMessage()
const store = useAgentWorkspaceStore()

const text = ref('')
const composing = ref(false)
const inputRef = ref(null)
const attachments = ref([])
const dragging = ref(false)
const historyIndex = ref(-1)
const historyDraft = ref('')

const popup = reactive({
  open: false,
  kind: 'file',
  title: '',
  items: [],
  active: 0,
  from: 0,
  to: 0,
  query: '',
})

let draftTimer = null
let popupTimer = null
let offNativeDrag = null

const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg']
const AUDIO_EXT = ['mp3', 'wav', 'm4a', 'ogg', 'flac']

const mode = computed(() => AGENT_MODES.find((m) => m.id === store.settings.mode) || AGENT_MODES[0])
const modeLabel = computed(() => mode.value.label)
const modeHint = computed(() => mode.value.hint)
const permission = computed(() => store.permissionPreset)
const permissionHint = computed(() => permission.value.hint)

const currentCwd = computed(() => store.currentState?.cwd || store.settings.cwd || '')
const workspaceLabel = computed(() => {
  const parts = String(currentCwd.value || '').split(/[\\/]/).filter(Boolean)
  if (!parts.length) return '选择目录'
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : parts.join('/')
})

const modelLabel = computed(() => {
  const m = store.currentModel
  return m?.displayName || m?.model || store.settings.model || '默认模型'
})
const modelHint = computed(() => (store.models.length ? `共 ${store.models.length} 个模型` : '尚未读取模型清单'))

const effortLabel = computed(() => {
  const value = store.settings.effort
  return { minimal: '推理·极低', low: '推理·低', medium: '推理·中', high: '推理·高' }[value] || '推理·自动'
})

const contextPercent = computed(() => {
  const percent = store.contextUsage?.percent || 0
  return percent > 0 ? percent : null
})

const canSend = computed(() => Boolean(text.value.trim() || attachments.value.length))

const placeholder = computed(() => {
  if (props.pending) return '运行中：Enter 把你的话插进当前回合'
  if (mode.value.id === 'computer') return '说明要在这台电脑上做什么…'
  return '描述任务，@ 提及文件，/ 打开命令'
})

const hint = computed(() => {
  if (props.pending) return 'Enter 转向 · Esc 停止'
  return 'Enter 发送 · Shift+Enter 换行 · ↑↓ 调历史'
})

const modeOptions = computed(() => AGENT_MODES.map((m) => ({ key: m.id, label: `${m.label} · ${m.hint}` })))

const permissionOptions = computed(() =>
  PERMISSION_PRESETS.map((p) => ({ key: p.id, label: `${p.label} · ${p.hint}` })),
)

const cwdOptions = computed(() => {
  const items = [{ key: '__pick__', label: '选择工作目录…' }]
  if (currentCwd.value) items.push({ key: '__current__', label: currentCwd.value, disabled: true })
  return items
})

const modelOptions = computed(() => {
  const list = store.models.filter((m) => !m.hidden)
  if (!list.length) return [{ key: '__none__', label: '还没有模型清单', disabled: true }]
  return list.map((m) => ({ key: String(m.model || m.id), label: m.displayName || m.model || m.id }))
})

const effortOptions = [
  { key: '', label: '自动（跟随模型）' },
  { key: 'minimal', label: '极低' },
  { key: 'low', label: '低' },
  { key: 'medium', label: '中' },
  { key: 'high', label: '高' },
]

// ───────────── 输入框

const uid = () => `att-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`
const baseName = (p) => String(p || '').split(/[\\/]/).filter(Boolean).pop() || ''

const autosize = () => {
  const el = inputRef.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 200)}px`
}

const scheduleDraft = () => {
  if (draftTimer) clearTimeout(draftTimer)
  draftTimer = setTimeout(() => store.setDraft(text.value), 260)
}

const resetHistory = () => {
  historyIndex.value = -1
  historyDraft.value = ''
}

// ───────────── 附件

const addPath = (path) => {
  const value = String(path || '').trim()
  if (!value) return
  if (attachments.value.some((a) => a.path === value)) return
  const ext = value.split('.').pop().toLowerCase()
  const name = baseName(value)
  if (IMAGE_EXT.includes(ext)) {
    attachments.value.push({
      id: uid(),
      type: 'localImage',
      path: value,
      name,
      preview: convertFileSrc(value),
    })
  } else if (AUDIO_EXT.includes(ext)) {
    attachments.value.push({ id: uid(), type: 'localAudio', path: value, name })
  } else {
    attachments.value.push({ id: uid(), type: 'mention', path: value, name })
  }
}

const addFile = (file) => {
  if (!file) return
  const path = file.path || ''
  if (path) {
    addPath(path)
    return
  }
  if (String(file.type || '').startsWith('image/')) {
    const reader = new FileReader()
    reader.onload = () => {
      const url = String(reader.result || '')
      if (!url) return
      attachments.value.push({
        id: uid(),
        type: 'image',
        url,
        name: file.name || '粘贴的图片',
        preview: url,
      })
    }
    reader.readAsDataURL(file)
    return
  }
  message.warning('拿不到这个文件的路径，请用回形针选择文件')
}

const removeAttachment = (index) => {
  attachments.value.splice(index, 1)
}

const pickFiles = async () => {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({
      multiple: true,
      directory: false,
      title: '选择文件或图片',
    })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    for (const path of list) addPath(path)
  } catch (err) {
    logger.warn('[composer] pick files failed', { error: String(err?.message || err) })
    message.error('打开文件选择器失败')
  }
}

const onPaste = (event) => {
  const files = Array.from(event.clipboardData?.files || [])
  if (!files.length) return
  event.preventDefault()
  for (const file of files) addFile(file)
}

// HTML5 拖放：桌面端多数情况走 Tauri 原生事件，这里兜住浏览器内拖拽。
const onHtmlDragOver = () => {
  dragging.value = true
}

const onHtmlDragLeave = (event) => {
  if (event.relatedTarget) return
  dragging.value = false
}

const onHtmlDrop = (event) => {
  dragging.value = false
  for (const file of Array.from(event.dataTransfer?.files || [])) addFile(file)
}

// ───────────── @ 提及 / 命令

const commands = [
  { key: 'compact', label: '/compact', hint: '压缩上下文，保留结论', icon: commandIcon },
  { key: 'review', label: '/review', hint: '审查未提交的改动', icon: commandIcon },
  { key: 'fork', label: '/fork', hint: '从当前回合分叉', icon: commandIcon },
  { key: 'diff', label: '/diff', hint: '打开本次改动', icon: commandIcon },
  { key: 'new', label: '/new', hint: '新建任务', icon: commandIcon },
]

const closePopup = () => {
  popup.open = false
  popup.items = []
  popup.active = 0
}

const tokenBeforeCaret = () => {
  const el = inputRef.value
  const pos = typeof el?.selectionStart === 'number' ? el.selectionStart : text.value.length
  const before = text.value.slice(0, pos)
  const at = before.match(/(?:^|\s)@([^\s@]*)$/)
  if (at) {
    return { kind: 'file', query: at[1], from: pos - at[1].length - 1, to: pos }
  }
  const slash = before.match(/(?:^|\s)\/([a-z-]*)$/)
  if (slash) {
    return { kind: 'command', query: slash[1], from: pos - slash[1].length - 1, to: pos }
  }
  return null
}

const fetchFileItems = async (query) => {
  if (!query) {
    popup.items = []
    return
  }
  const files = await store.fuzzyFiles(query)
  if (!popup.open || popup.kind !== 'file' || popup.query !== query) return
  const root = currentCwd.value
  popup.items = (files || []).slice(0, 8).map((file) => {
    const path = String(file?.path || '')
    const label = path.startsWith(root)
      ? path.slice(root.length).replace(/^[\\/]/, '')
      : baseName(path)
    return {
      key: path || label,
      label,
      hint: path,
      path,
      icon: fileIcon,
    }
  })
}

const updatePopup = () => {
  const token = tokenBeforeCaret()
  if (!token) {
    closePopup()
    return
  }
  if (token.kind === 'file' && !currentCwd.value) {
    closePopup()
    return
  }
  popup.open = true
  popup.kind = token.kind
  popup.query = token.query
  popup.from = token.from
  popup.to = token.to
  popup.active = 0

  if (token.kind === 'command') {
    popup.title = '命令'
    popup.items = commands.filter((c) => c.key.startsWith(token.query))
    return
  }

  popup.title = token.query ? `文件 · ${token.query}` : '文件'
  popup.items = []
  if (popupTimer) clearTimeout(popupTimer)
  popupTimer = setTimeout(() => fetchFileItems(token.query), 150)
}

const startMention = () => {
  const el = inputRef.value
  const pos = typeof el?.selectionStart === 'number' ? el.selectionStart : text.value.length
  const next = `${text.value.slice(0, pos)}@${text.value.slice(pos)}`
  text.value = next
  nextTick(() => {
    el?.focus()
    el?.setSelectionRange(pos + 1, pos + 1)
    autosize()
    updatePopup()
  })
}

const chooseItem = async (item) => {
  if (!item) return
  if (popup.kind === 'command') {
    closePopup()
    text.value = ''
    store.setDraft('')
    await runCommand(item.key)
    nextTick(autosize)
    return
  }
  const rel = String(item.label || '')
  const next = `${text.value.slice(0, popup.from)}@${rel} ${text.value.slice(popup.to)}`
  text.value = next
  closePopup()
  nextTick(() => {
    const el = inputRef.value
    el?.focus()
    const caret = popup.from + rel.length + 2
    el?.setSelectionRange(caret, caret)
    autosize()
  })
}

const runCommand = async (key) => {
  const needsThread = ['compact', 'review', 'fork'].includes(key)
  if (needsThread && !store.currentThreadId) {
    message.info('先发一条消息建立会话，才能用这个命令')
    return
  }
  try {
    if (key === 'compact') {
      await store.compactThread()
      message.success('已开始压缩上下文')
    } else if (key === 'review') {
      await store.startReview()
      message.success('已开始审查未提交改动')
    } else if (key === 'fork') {
      await store.forkThread(store.currentThreadId)
      message.success('已分叉出新会话')
    } else if (key === 'diff') {
      emit('open-diff')
    } else if (key === 'new') {
      emit('new')
    }
  } catch (err) {
    logger.warn('[composer] command failed', { key, error: String(err?.message || err) })
    message.error(String(err?.message || err))
  }
}

// ───────────── 历史（↑↓）

const historyUp = () => {
  const list = store.currentInputHistory || []
  if (!list.length) return false
  if (historyIndex.value === -1) {
    historyDraft.value = text.value
    historyIndex.value = list.length - 1
  } else {
    historyIndex.value = Math.max(0, historyIndex.value - 1)
  }
  text.value = list[historyIndex.value] || ''
  nextTick(() => {
    const el = inputRef.value
    el?.setSelectionRange(text.value.length, text.value.length)
    autosize()
  })
  return true
}

const historyDown = () => {
  const list = store.currentInputHistory || []
  if (historyIndex.value === -1) return false
  historyIndex.value += 1
  if (historyIndex.value >= list.length) {
    historyIndex.value = -1
    text.value = historyDraft.value
  } else {
    text.value = list[historyIndex.value] || ''
  }
  nextTick(() => {
    const el = inputRef.value
    el?.setSelectionRange(text.value.length, text.value.length)
    autosize()
  })
  return true
}

// ───────────── 键盘

const onInput = () => {
  autosize()
  resetHistory()
  scheduleDraft()
  updatePopup()
}

const movePopup = (delta) => {
  const count = popup.items.length
  if (!count) return
  popup.active = (popup.active + delta + count) % count
}

const onKeydown = (event) => {
  // 中文输入法组合期间的 Enter / 方向键是选词，不是操作。
  if (composing.value || event.isComposing) return

  if (popup.open) {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      movePopup(1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      movePopup(-1)
      return
    }
    if (event.key === 'Enter' || event.key === 'Tab') {
      event.preventDefault()
      chooseItem(popup.items[popup.active])
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      closePopup()
      return
    }
  }

  if (event.key === 'Escape' && props.pending) {
    event.preventDefault()
    emit('stop')
    return
  }

  if (event.key === 'ArrowUp' && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
    const el = inputRef.value
    const caret = typeof el?.selectionStart === 'number' ? el.selectionStart : 0
    if ((historyIndex.value >= 0 || !text.value || caret === 0) && historyUp()) {
      event.preventDefault()
    }
    return
  }

  if (event.key === 'ArrowDown' && !event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
    if (historyIndex.value >= 0 && historyDown()) event.preventDefault()
    return
  }

  if (event.key === 'Enter') {
    if (event.shiftKey) return
    event.preventDefault()
    submit()
  }
}

// ───────────── 发送

const submit = () => {
  const value = text.value.trim()
  if (!value && !attachments.value.length) return
  if (attachments.value.length > 40) {
    message.warning('一次最多带 40 个附件')
    return
  }
  const payload = attachments.value.map((a) => ({
    type: a.type,
    path: a.path || '',
    url: a.url || '',
    name: a.name || '',
  }))
  if (props.pending) emit('steer', value, payload)
  else emit('send', value, payload)
  text.value = ''
  attachments.value = []
  store.setDraft('')
  resetHistory()
  closePopup()
  nextTick(autosize)
}

const onBlur = () => {
  setTimeout(() => closePopup(), 120)
}

// ───────────── 参数选择

const onModeSelect = (key) => store.setMode(key)

const onPermissionSelect = (key) => {
  store.setPermission(key)
  if (key === 'bypass') message.warning('已跳过审批：命令会直接执行，不再逐次询问')
}

const onCwdSelect = (key) => {
  if (key === '__pick__') pickWorkspace()
}

const onModelSelect = (key) => {
  if (key === '__none__') return
  store.setModel(key)
}

const onEffortSelect = (key) => store.setEffort(key)

const pickWorkspace = async () => {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, multiple: false, title: '选择工作目录' })
    if (typeof picked === 'string' && picked) {
      store.setCwd(picked)
      if (store.currentThreadId) message.info('工作目录会从下一轮开始生效')
    }
  } catch (err) {
    logger.warn('[composer] pick workspace failed', { error: String(err?.message || err) })
  }
}

// ───────────── 生命周期

watch(
  () => store.currentThreadId,
  () => {
    text.value = store.getDraft()
    attachments.value = []
    resetHistory()
    closePopup()
    nextTick(autosize)
  },
)

onMounted(async () => {
  text.value = store.getDraft()
  nextTick(autosize)
  try {
    const webview = getCurrentWebview()
    offNativeDrag = await webview.onDragDropEvent((event) => {
      const type = String(event?.payload?.type || '')
      if (type === 'enter' || type === 'over') {
        dragging.value = true
      } else if (type === 'leave') {
        dragging.value = false
      } else if (type === 'drop') {
        dragging.value = false
        for (const path of event?.payload?.paths || []) addPath(path)
      }
    })
  } catch (err) {
    logger.warn('[composer] native drag-drop unavailable', { error: String(err?.message || err) })
  }
})

onBeforeUnmount(() => {
  if (draftTimer) clearTimeout(draftTimer)
  if (popupTimer) clearTimeout(popupTimer)
  if (typeof offNativeDrag === 'function') offNativeDrag()
})
</script>

<style scoped>
.composer-root {
  position: relative;
  flex: 0 0 auto;
  /* 宽度由外层给 100%，横向 padding 必须算进宽度里，否则整块会向右溢出约 56px，
     被 workspace-main 的 overflow:hidden 在右侧栏边界处截断（发送按钮被切掉）。 */
  box-sizing: border-box;
  padding: 8px 28px 12px;
  background: var(--guaxi-content-bg);
}

.drop-veil {
  position: absolute;
  inset: 2px 22px 6px;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px dashed rgba(180, 125, 26, 0.7);
  border-radius: 12px;
  background: rgba(180, 125, 26, 0.07);
  color: #b47d1a;
  font-size: 12.5px;
  pointer-events: none;
  animation: veil-in 120ms ease-out;
}

@keyframes veil-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.composer-shell {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 11px 7px;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  transition: border-color 140ms ease, box-shadow 140ms ease;
}

.composer-shell:focus-within {
  border-color: rgba(180, 125, 26, 0.55);
  box-shadow: 0 0 0 3px rgba(180, 125, 26, 0.08);
}

.composer-shell--running {
  border-color: rgba(180, 125, 26, 0.45);
}

.attach-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding-bottom: 2px;
}

.attach-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 220px;
  height: 26px;
  padding: 0 6px 0 7px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-size: 11.5px;
}

.attach-chip--image {
  padding-left: 3px;
}

.attach-thumb {
  width: 20px;
  height: 20px;
  border-radius: 3px;
  object-fit: cover;
}

.attach-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
}

.attach-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  padding: 0;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}

.attach-remove:hover {
  background: var(--guaxi-hover-bg);
  color: #b42318;
}

.attach-count {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

.input-wrap {
  position: relative;
  display: flex;
}

.composer-input {
  width: 100%;
  min-height: 46px;
  max-height: 200px;
  padding: 2px 2px 0;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 14px;
  line-height: 1.6;
}

.composer-input::placeholder {
  color: var(--guaxi-text-secondary);
  opacity: 0.62;
}

/* ── @ / 命令 浮层 */

.popup {
  position: absolute;
  left: -4px;
  bottom: calc(100% + 10px);
  z-index: 30;
  width: min(480px, 100%);
  padding: 5px;
  border: 1px solid var(--guaxi-border);
  border-radius: 9px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.24);
  animation: popup-in 110ms ease-out;
}

@keyframes popup-in {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}

.popup-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 3px 7px 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  letter-spacing: 0.05em;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}

.popup-hint {
  margin-left: auto;
  letter-spacing: 0;
  opacity: 0.7;
}

.popup-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 6px 8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-primary);
  text-align: left;
  cursor: pointer;
}

.popup-item--on {
  background: var(--guaxi-hover-bg);
}

.popup-main {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  flex: 1 1 auto;
}

.popup-label {
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-sub {
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.popup-key {
  flex: 0 0 auto;
  padding: 1px 5px;
  border: 1px solid var(--guaxi-border);
  border-radius: 4px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  opacity: 0.7;
}

.popup-empty {
  padding: 10px 8px;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.65;
}

.composer-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-top: 6px;
  border-top: 1px solid var(--guaxi-border);
}

.bar-left,
.bar-right {
  display: flex;
  align-items: center;
  gap: 3px;
  min-width: 0;
}

/* 窗口收窄时让 chip 先缩（文字走省略号），发送按钮始终留在可视区内。 */
.bar-left {
  flex: 1 1 auto;
  overflow: hidden;
}

.bar-right {
  flex: 0 1 auto;
  margin-left: auto;
}

.icon-btn {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

.icon-btn:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.bar-chip {
  display: inline-flex;
  flex: 0 1 auto;
  min-width: 0;
  align-items: center;
  gap: 5px;
  max-width: 190px;
  height: 25px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
}

.bar-chip:hover {
  background: var(--guaxi-hover-bg);
  border-color: var(--guaxi-border);
  color: var(--guaxi-text-primary);
}

.bar-chip-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bar-chip--perm {
  color: #b47d1a;
}

.bar-chip--effort {
  opacity: 0.82;
}

.send-btn {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  margin-left: 4px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: #b47d1a;
  color: #fff;
  cursor: pointer;
  transition: opacity 120ms ease, transform 120ms ease, background 120ms ease;
}

.send-btn:hover:not(:disabled) {
  transform: translateY(-1px);
}

.send-btn:disabled {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
  cursor: default;
}

.send-btn--steer {
  background: var(--guaxi-text-primary);
  color: var(--guaxi-content-bg);
}

.send-btn--stop {
  background: transparent;
  border: 1px solid rgba(180, 125, 26, 0.65);
  color: #b47d1a;
}

.send-btn--stop:hover {
  background: rgba(180, 125, 26, 0.1);
}

.composer-foot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 5px 3px 0;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
}

.foot-hint {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.foot-right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.foot-ctx {
  opacity: 0.9;
}

.foot-mode {
  letter-spacing: 0.04em;
}

@media (max-width: 900px) {
  .composer-root {
    padding: 6px 14px 10px;
  }

  .bar-chip--effort,
  .foot-ctx {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .popup,
  .drop-veil {
    animation: none;
  }
}

/* COMPOSER_STYLE_END */
</style>
