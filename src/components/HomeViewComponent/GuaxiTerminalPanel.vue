<template>
  <div class="guaxi-terminal-panel">
    <div class="guaxi-terminal-panel__toolbar">
      <div class="guaxi-terminal-panel__tabs">
        <button type="button" class="guaxi-terminal-panel__tool-tab active">终端</button>
      </div>

      <div class="guaxi-terminal-panel__toolbar-right">
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <select
              v-model="selectedProfileId"
              class="guaxi-terminal-panel__profile-select"
              title="选择终端类型"
            >
              <option
                v-for="profile in availableProfiles"
                :key="profile.id"
                :value="profile.id"
              >
                {{ profile.label }}
              </option>
            </select>
          </template>
          选择终端类型
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button type="button" class="guaxi-terminal-panel__icon-btn" title="新建终端" @click="createSession()">
              <Icon :icon="plusIcon" width="14" height="14" />
            </button>
          </template>
          新建终端
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button type="button" class="guaxi-terminal-panel__icon-btn" title="刷新终端配置" @click="reloadProfiles">
              <Icon :icon="refreshIcon" width="14" height="14" />
            </button>
          </template>
          刷新终端配置
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button
              type="button"
              class="guaxi-terminal-panel__icon-btn"
              title="清空当前终端输出"
              @click="clearActiveSessionBuffer"
            >
              <Icon :icon="clearIcon" width="14" height="14" />
            </button>
          </template>
          清空当前终端输出
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button
              type="button"
              class="guaxi-terminal-panel__icon-btn"
              title="结束当前终端会话"
              :disabled="!activeSession"
              @click="killActiveSession"
            >
              <Icon :icon="stopIcon" width="14" height="14" />
            </button>
          </template>
          结束当前终端会话
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button
              type="button"
              class="guaxi-terminal-panel__icon-btn"
              title="搜索终端输出"
              @click="openSearchPanel"
            >
              <Icon :icon="searchIcon" width="14" height="14" />
            </button>
          </template>
          搜索终端输出
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button type="button" class="guaxi-terminal-panel__icon-btn" title="收起终端" @click="$emit('close')">
              <Icon :icon="chevronDownIcon" width="14" height="14" />
            </button>
          </template>
          收起终端
        </n-tooltip>
      </div>
    </div>

    <div class="guaxi-terminal-panel__content" :class="{ 'without-sidebar': !showSidebar }">
      <div class="guaxi-terminal-panel__main">
        <div class="guaxi-terminal-panel__body">
          <div v-if="searchVisible" class="guaxi-terminal-panel__search" @mousedown.stop @keydown.stop>
            <input
              ref="searchInputRef"
              v-model="searchKeyword"
              class="guaxi-terminal-panel__search-input"
              type="text"
              placeholder="搜索终端输出"
              @keydown.stop
              @keydown.enter.prevent="focusNextSearchMatch"
              @keydown.shift.enter.prevent="focusPrevSearchMatch"
              @keydown.esc.prevent="closeSearchPanel"
            />
            <span class="guaxi-terminal-panel__search-count">{{ searchMatchCountText }}</span>
            <button type="button" class="guaxi-terminal-panel__search-btn" @click="focusPrevSearchMatch">上一个</button>
            <button type="button" class="guaxi-terminal-panel__search-btn" @click="focusNextSearchMatch">下一个</button>
            <button type="button" class="guaxi-terminal-panel__search-btn" @click="copySelectionToClipboard">复制</button>
            <button type="button" class="guaxi-terminal-panel__search-btn" @click="pasteClipboardToTerminal">粘贴</button>
            <button type="button" class="guaxi-terminal-panel__search-btn guaxi-terminal-panel__search-btn--close" @click="closeSearchPanel">关闭</button>
          </div>
          <div ref="terminalHostRef" class="guaxi-terminal-panel__terminal"></div>
          <div v-if="panelBusy && !sessions.length" class="guaxi-terminal-panel__status-overlay">
            <div class="guaxi-terminal-panel__status-title">正在准备终端面板</div>
            <div class="guaxi-terminal-panel__status-subtitle">{{ loadingStateText }}</div>
          </div>
        </div>
      </div>

      <aside v-if="showSidebar" class="guaxi-terminal-panel__sidebar">
        <div class="guaxi-terminal-panel__sidebar-title">会话</div>
        <div class="guaxi-terminal-panel__session-list">
          <button
            v-for="session in sessions"
            :key="session.session_id"
            type="button"
            class="guaxi-terminal-panel__session-item"
            :class="{ active: session.session_id === activeSessionId }"
            @click="activateSession(session.session_id)"
          >
            <span class="guaxi-terminal-panel__session-label">{{ session.label }}</span>
            <span v-if="session.exited" class="guaxi-terminal-panel__session-state">已退出</span>
            <n-tooltip placement="left" trigger="hover">
              <template #trigger>
                <span
                  class="guaxi-terminal-panel__session-close"
                  title="关闭终端"
                  @click.stop="closeSession(session.session_id)"
                >
                  <Icon :icon="closeIcon" width="14" height="14" />
                </span>
              </template>
              关闭终端
            </n-tooltip>
          </button>
          <div v-if="!sessions.length" class="guaxi-terminal-panel__session-empty">
            {{ loadingStateText }}
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup>
import 'xterm/css/xterm.css'

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'
import plusIcon from '@iconify-icons/lucide/plus'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import clearIcon from '@iconify-icons/lucide/eraser'
import stopIcon from '@iconify-icons/lucide/square'
import searchIcon from '@iconify-icons/lucide/search'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import closeIcon from '@iconify-icons/lucide/x'

import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('GuaxiTerminalPanel')
const settingsStore = useSettingsStore()

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  contextPath: {
    type: String,
    default: '',
  },
})

const emit = defineEmits(['close'])

const terminalHostRef = ref(null)
const sessions = ref([])
const activeSessionId = ref('')
const profiles = ref([])
const defaultProfileId = ref('')
const selectedProfileId = ref('')
const panelBusy = ref(false)
const panelActivated = ref(false)
const loadingStateText = ref('等待创建终端会话')
const searchVisible = ref(false)
const searchKeyword = ref('')
const searchInputRef = ref(null)
const searchMatches = ref([])
const searchActiveIndex = ref(-1)
const terminalCommandDrafts = ref({})
const terminalExitProbeTimers = ref({})

let terminal = null
let fitAddon = null
let unlistenOutput = null
let unlistenExit = null
let terminalResizeObserver = null
let pendingFitFrame = 0

const isWindows = typeof navigator !== 'undefined'
  ? /Windows/i.test(navigator.userAgent)
  : true

const availableProfiles = computed(() => profiles.value.filter((profile) => profile.available))
const activeSession = computed(() => sessions.value.find((session) => session.session_id === activeSessionId.value) || null)
const showSidebar = computed(() => sessions.value.length > 1)
const isSwitchingProfile = ref(false)
const sessionCreateToken = ref(0)
const searchMatchCountText = computed(() => {
  if (!searchKeyword.value.trim()) return '0 / 0'
  const total = searchMatches.value.length
  if (!total) return '0 / 0'
  return `${Math.max(0, searchActiveIndex.value) + 1} / ${total}`
})

const buildFallbackProfiles = () => {
  if (isWindows) {
    return [
      { id: 'ipython', label: 'IPython', available: true, program: '', args: [], source: 'fallback', detail: 'Guaxi Python 环境' },
      { id: 'powershell', label: 'PowerShell', available: true, program: '', args: [], source: 'fallback', detail: 'Windows PowerShell' },
      { id: 'cmd', label: 'Command Prompt', available: true, program: '', args: [], source: 'fallback', detail: 'Windows 命令提示符' },
      { id: 'git-bash', label: 'Git Bash', available: true, program: '', args: ['-i', '-l'], source: 'fallback', detail: 'Git Bash' },
    ]
  }

  return [
    { id: 'ipython', label: 'IPython', available: true, program: '', args: [], source: 'fallback', detail: 'Guaxi Python 环境' },
    { id: 'bash', label: 'Bash', available: true, program: '', args: ['-i'], source: 'fallback', detail: 'Bash' },
    { id: 'zsh', label: 'Zsh', available: true, program: '', args: ['-i'], source: 'fallback', detail: 'Zsh' },
    { id: 'powershell', label: 'PowerShell', available: true, program: '', args: [], source: 'fallback', detail: 'PowerShell' },
  ]
}

const ensureSelectedProfile = () => {
  selectedProfileId.value =
    availableProfiles.value.find((profile) => profile.id === selectedProfileId.value)?.id
    || availableProfiles.value.find((profile) => profile.id === defaultProfileId.value)?.id
    || availableProfiles.value[0]?.id
    || ''
}

const getThemeCssVar = (name, fallback) => {
  if (typeof window === 'undefined') return fallback
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

const getTerminalTheme = () => {
  const dark = settingsStore.appearanceTheme === 'dark'
  return dark
    ? {
        // 终端内部颜色跟随 Guaxi 的暗色主题变量，避免看起来像另一套独立皮肤。
        background: getThemeCssVar('--guaxi-panel-bg', '#1e1e1e'),
        foreground: getThemeCssVar('--guaxi-text-primary', '#d4d4d4'),
        cursor: getThemeCssVar('--primary-color', '#aeafad'),
        selectionBackground: 'rgba(74, 202, 219, 0.22)',
      }
    : {
        background: getThemeCssVar('--guaxi-panel-bg', '#ffffff'),
        foreground: getThemeCssVar('--guaxi-text-primary', '#1f2328'),
        cursor: getThemeCssVar('--primary-color', '#0969da'),
        selectionBackground: 'rgba(9, 105, 218, 0.18)',
      }
}

const getSessionIndex = (sessionId) => sessions.value.findIndex((session) => session.session_id === sessionId)

const applyTerminalTheme = () => {
  if (!terminal) return
  terminal.options.theme = getTerminalTheme()
}

const renderActiveSession = () => {
  if (!terminal) return
  terminal.reset()
  terminal.clear()
  if (activeSession.value?.buffer) {
    terminal.write(activeSession.value.buffer)
  }
  updateSearchMatches('render-active-session')
}

const writeClipboardText = async (value) => {
  const text = String(value || '')
  if (!text) return
  if (navigator?.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return
  }
  const mod = await import('@tauri-apps/plugin-clipboard-manager')
  await mod.writeText(text)
}

const readClipboardText = async () => {
  if (navigator?.clipboard?.readText) {
    return String(await navigator.clipboard.readText() || '')
  }
  const mod = await import('@tauri-apps/plugin-clipboard-manager')
  return String(await mod.readText() || '')
}

const scrollToSearchMatch = (index) => {
  if (!terminal) return
  const match = searchMatches.value[index]
  if (!match) return
  searchActiveIndex.value = index
  terminal.select(match.col, match.row, match.length)
  terminal.scrollToLine(Math.max(match.row - 3, 0))
  terminal.focus()
}

const updateSearchMatches = (reason = 'manual-search-update') => {
  const keyword = String(searchKeyword.value || '').trim()
  const buffer = String(activeSession.value?.buffer || '')
  if (!keyword || !buffer) {
    searchMatches.value = []
    searchActiveIndex.value = -1
    return
  }

  const rows = buffer.split(/\r?\n/)
  const matches = []
  rows.forEach((line, row) => {
    let startIndex = 0
    while (startIndex <= line.length) {
      const col = line.toLowerCase().indexOf(keyword.toLowerCase(), startIndex)
      if (col < 0) break
      matches.push({ row, col, length: keyword.length })
      startIndex = col + Math.max(keyword.length, 1)
    }
  })
  searchMatches.value = matches
  searchActiveIndex.value = matches.length ? 0 : -1
  logger.debug('[TerminalPanel] search matches updated', {
    reason,
    keyword,
    matchCount: matches.length,
  })
}

const openSearchPanel = async () => {
  searchVisible.value = true
  await nextTick()
  searchInputRef.value?.focus?.()
  searchInputRef.value?.select?.()
  updateSearchMatches('open-search')
}

const closeSearchPanel = () => {
  searchVisible.value = false
  searchKeyword.value = ''
  searchMatches.value = []
  searchActiveIndex.value = -1
  terminal?.clearSelection?.()
  terminal?.focus?.()
}

const TERMINAL_EXIT_COMMANDS = new Set([
  'exit',
  'quit',
  'logout',
  'exit()',
  'quit()',
])

const clearExitProbeTimer = (sessionId) => {
  const normalizedSessionId = String(sessionId || '').trim()
  const timerId = terminalExitProbeTimers.value[normalizedSessionId]
  if (timerId) {
    window.clearTimeout(timerId)
  }
  if (!(normalizedSessionId in terminalExitProbeTimers.value)) return
  const nextTimers = { ...terminalExitProbeTimers.value }
  delete nextTimers[normalizedSessionId]
  terminalExitProbeTimers.value = nextTimers
}

const scheduleExitProbe = (sessionId, commandText) => {
  const normalizedSessionId = String(sessionId || '').trim()
  const normalizedCommand = String(commandText || '').trim().toLowerCase()
  if (!normalizedSessionId || !TERMINAL_EXIT_COMMANDS.has(normalizedCommand)) return

  logger.info('[TerminalPanel] schedule exit probe', {
    sessionId: normalizedSessionId,
    command: normalizedCommand,
  })

  clearExitProbeTimer(normalizedSessionId)

  const maxAttempts = 12
  const probeDelayMs = 280
  let attempt = 0

  const runProbe = () => {
    attempt += 1
    void invoke('terminal_probe_session_exit', {
      request: {
        session_id: normalizedSessionId,
      },
    }).then((response) => {
      if (response?.exited) {
        clearExitProbeTimer(normalizedSessionId)
        syncExitedSession(normalizedSessionId, Number(response?.exit_code))
        return
      }
      if (attempt >= maxAttempts || getSessionIndex(normalizedSessionId) < 0) {
        clearExitProbeTimer(normalizedSessionId)
        return
      }
      const timerId = window.setTimeout(runProbe, probeDelayMs)
      terminalExitProbeTimers.value = {
        ...terminalExitProbeTimers.value,
        [normalizedSessionId]: timerId,
      }
    }).catch((error) => {
      clearExitProbeTimer(normalizedSessionId)
      logger.warn('[TerminalPanel] probe session exit failed', {
        sessionId: normalizedSessionId,
        command: normalizedCommand,
        attempt,
        error,
      })
    })
  }

  const timerId = window.setTimeout(runProbe, 180)
  terminalExitProbeTimers.value = {
    ...terminalExitProbeTimers.value,
    [normalizedSessionId]: timerId,
  }
}

const trackTerminalInput = (sessionId, data) => {
  const normalizedSessionId = String(sessionId || '').trim()
  if (!normalizedSessionId) return

  const chunks = Array.from(String(data || ''))
  let draft = String(terminalCommandDrafts.value[normalizedSessionId] || '')

  chunks.forEach((char) => {
    if (char === '\r' || char === '\n') {
      const completedCommand = draft.trim()
      terminalCommandDrafts.value = {
        ...terminalCommandDrafts.value,
        [normalizedSessionId]: '',
      }
      scheduleExitProbe(normalizedSessionId, completedCommand)
      draft = ''
      return
    }

    if (char === '\u0008' || char === '\u007f') {
      draft = draft.slice(0, -1)
      return
    }

    if (char >= ' ') {
      draft += char
    }
  })

  terminalCommandDrafts.value = {
    ...terminalCommandDrafts.value,
    [normalizedSessionId]: draft,
  }
}

const focusNextSearchMatch = () => {
  if (!searchMatches.value.length) return
  const nextIndex = searchActiveIndex.value < 0
    ? 0
    : (searchActiveIndex.value + 1) % searchMatches.value.length
  scrollToSearchMatch(nextIndex)
}

const focusPrevSearchMatch = () => {
  if (!searchMatches.value.length) return
  const prevIndex = searchActiveIndex.value < 0
    ? searchMatches.value.length - 1
    : (searchActiveIndex.value - 1 + searchMatches.value.length) % searchMatches.value.length
  scrollToSearchMatch(prevIndex)
}

const copySelectionToClipboard = async () => {
  const selected = String(terminal?.getSelection?.() || '').trim()
  if (!selected) return
  await writeClipboardText(selected)
}

const pasteClipboardToTerminal = async () => {
  const sessionId = activeSessionId.value
  if (!sessionId) return
  const text = await readClipboardText()
  if (!text) return
  await invoke('terminal_write_session', {
    request: {
      session_id: sessionId,
      data: text,
    },
  })
}

const scheduleFitTerminal = (reason = 'scheduled-fit') => {
  if (typeof window === 'undefined') {
    void fitTerminal(reason)
    return
  }
  if (pendingFitFrame) {
    window.cancelAnimationFrame(pendingFitFrame)
  }
  pendingFitFrame = window.requestAnimationFrame(() => {
    pendingFitFrame = 0
    void fitTerminal(reason)
  })
}

const fitTerminal = async (reason = 'manual-fit') => {
  if (!terminal || !fitAddon || !props.visible) return
  await nextTick()
  try {
    fitAddon.fit()

    // 这里显式修正 viewport 高度，确保内容较多时会出现垂直滚动条。
    const viewport = terminalHostRef.value?.querySelector?.('.xterm-viewport')
    if (viewport) {
      viewport.style.height = '100%'
      viewport.style.maxHeight = '100%'
      viewport.style.overflowY = 'auto'
    }

    const xtermElement = terminalHostRef.value?.querySelector?.('.xterm')
    if (xtermElement) {
      xtermElement.style.height = '100%'
    }

    terminal.refresh?.(0, Math.max(terminal.rows - 1, 0))

    const sessionId = activeSessionId.value
    if (sessionId) {
      await invoke('terminal_resize_session', {
        request: {
          session_id: sessionId,
          cols: terminal.cols,
          rows: terminal.rows,
        },
      })
    }

    logger.info('[TerminalPanel] fit terminal', {
      reason,
      cols: terminal.cols,
      rows: terminal.rows,
      sessionId,
    })
  } catch (error) {
    logger.warn('[TerminalPanel] fit terminal failed', { reason, error })
  }
}

const activateSession = (sessionId) => {
  if (!sessionId || sessionId === activeSessionId.value) return
  activeSessionId.value = sessionId
  const session = sessions.value.find((item) => item.session_id === sessionId)
  if (session?.profile_id) {
    selectedProfileId.value = session.profile_id
  }
  renderActiveSession()
  void fitTerminal('activate-session')
}

const appendSessionOutput = (sessionId, data) => {
  const index = getSessionIndex(sessionId)
  if (index < 0) return
  const session = sessions.value[index]
  const nextBuffer = `${session.buffer || ''}${String(data || '')}`
  sessions.value[index] = {
    ...session,
    buffer: nextBuffer,
  }
  if (activeSessionId.value === sessionId && terminal) {
    terminal.write(String(data || ''))
  }
  if (activeSessionId.value === sessionId && searchVisible.value && searchKeyword.value.trim()) {
    updateSearchMatches('append-session-output')
  }
}

const markSessionExited = (sessionId, exitCode) => {
  const index = getSessionIndex(sessionId)
  if (index < 0) return
  const session = sessions.value[index]
  const footer = `\r\n[Guaxi] 终端已退出${Number.isFinite(exitCode) ? `，退出码 ${exitCode}` : ''}\r\n`
  sessions.value[index] = {
    ...session,
    exited: true,
    buffer: `${session.buffer || ''}${footer}`,
  }
  if (activeSessionId.value === sessionId && terminal) {
    terminal.write(footer)
  }
}

const handleSessionExit = (sessionId, exitCode) => {
  const index = getSessionIndex(sessionId)
  if (index < 0) return
  const session = sessions.value[index]
  const normalizedExitCode = Number.isFinite(exitCode) ? Number(exitCode) : null
  const footer = `\r\n[Guaxi] 终端已退出${normalizedExitCode !== null ? `，退出码 ${normalizedExitCode}` : ''}\r\n`
  const nextSessions = sessions.value
    .map((item, itemIndex) => (itemIndex === index
      ? {
          ...item,
          exited: true,
          buffer: `${item.buffer || ''}${footer}`,
        }
      : item))
    .filter((item) => item.session_id !== sessionId)

  // 终端进程已经自行退出时，前端同步移除会话，避免留下一个无法继续交互的空标签。
  sessions.value = nextSessions

  logger.info('[TerminalPanel] session exited and auto closed', {
    sessionId,
    profileId: session?.profile_id || '',
    exitCode: normalizedExitCode,
    remainingSessionCount: nextSessions.length,
  })

  if (activeSessionId.value === sessionId) {
    const nextActiveSession = nextSessions[0] || null
    activeSessionId.value = nextActiveSession?.session_id || ''
    selectedProfileId.value = nextActiveSession?.profile_id || selectedProfileId.value
    renderActiveSession()
    if (!nextActiveSession) {
      closeSearchPanel()
    }
    void fitTerminal('session-exited-auto-close')
    return
  }

  if (!nextSessions.length) {
    closeSearchPanel()
  }
}

const syncExitedSession = (sessionId, exitCode) => {
  const index = getSessionIndex(sessionId)
  if (index < 0) return
  const session = sessions.value[index]
  const normalizedExitCode = Number.isFinite(exitCode) ? Number(exitCode) : null
  const footer = `\r\n[Guaxi] terminal exited${normalizedExitCode !== null ? `, code ${normalizedExitCode}` : ''}\r\n`
  const nextSessions = sessions.value
    .map((item, itemIndex) => (itemIndex === index
      ? {
          ...item,
          exited: true,
          buffer: `${item.buffer || ''}${footer}`,
        }
      : item))
    .filter((item) => item.session_id !== sessionId)

  // Keep terminal tabs synchronized with the PTY lifecycle so an exited process
  // never leaves behind a dead session entry that the user has to close manually.
  delete terminalCommandDrafts.value[sessionId]
  clearExitProbeTimer(sessionId)
  sessions.value = nextSessions

  logger.info('[TerminalPanel] session exited and auto closed', {
    sessionId,
    profileId: session?.profile_id || '',
    exitCode: normalizedExitCode,
    remainingSessionCount: nextSessions.length,
  })

  if (activeSessionId.value === sessionId) {
    const nextActiveSession = nextSessions[0] || null
    activeSessionId.value = nextActiveSession?.session_id || ''
    selectedProfileId.value = nextActiveSession?.profile_id || selectedProfileId.value
    renderActiveSession()
    if (!nextActiveSession) {
      closeSearchPanel()
      emit('close')
    } else {
      void fitTerminal('session-exited-auto-close')
    }
    return
  }

  if (!nextSessions.length) {
    closeSearchPanel()
    emit('close')
  }
}

const loadProfilesWithTimeout = async () => {
  const fallbackProfiles = buildFallbackProfiles()
  const timeoutMs = 1200

  return await Promise.race([
    invoke('terminal_detect_profiles'),
    new Promise((resolve) => {
      window.setTimeout(() => {
        resolve({
          default_profile: isWindows ? 'powershell' : 'bash',
          profiles: fallbackProfiles,
          __fallback: true,
        })
      }, timeoutMs)
    }),
  ])
}

const reloadProfiles = async () => {
  loadingStateText.value = '正在检测可用终端配置'
  try {
    const response = await loadProfilesWithTimeout()
    const nextProfiles = Array.isArray(response?.profiles) ? response.profiles : buildFallbackProfiles()
    profiles.value = nextProfiles
    defaultProfileId.value = String(response?.default_profile || '').trim() || (isWindows ? 'powershell' : 'bash')
    ensureSelectedProfile()

    logger.info('[TerminalPanel] profiles loaded', {
      defaultProfileId: defaultProfileId.value,
      profileCount: profiles.value.length,
      availableCount: availableProfiles.value.length,
      fallback: !!response?.__fallback,
    })

    if (response?.__fallback) {
      loadingStateText.value = '终端配置检测较慢，已先使用默认配置'
    }
  } catch (error) {
    profiles.value = buildFallbackProfiles()
    defaultProfileId.value = isWindows ? 'powershell' : 'bash'
    ensureSelectedProfile()
    loadingStateText.value = '终端配置加载失败，已回退默认配置'
    logger.error('[TerminalPanel] load terminal profiles failed', error)
  }
}

const createSession = async (options = {}) => {
  const requestedProfileId = String(options?.profileId || selectedProfileId.value || defaultProfileId.value || '').trim()
  if (!requestedProfileId) return null
  const createToken = sessionCreateToken.value + 1
  sessionCreateToken.value = createToken
  try {
    panelBusy.value = true
    loadingStateText.value = `正在启动 ${requestedProfileId} 终端`
    if (terminal) {
      terminal.reset()
      terminal.clear()
      terminal.writeln(`[Guaxi] 正在启动 ${requestedProfileId} 终端...`)
    }
    const snapshot = await invoke('terminal_create_session', {
      request: {
        profile_id: requestedProfileId,
        cwd: String(options?.cwd || props.contextPath || '').trim(),
        cols: terminal?.cols || 120,
        rows: terminal?.rows || 32,
      },
    })
    const session = {
      ...snapshot,
      buffer: '',
      exited: false,
    }
    sessions.value = [...sessions.value, session]
    activeSessionId.value = session.session_id
    renderActiveSession()
    if (createToken === sessionCreateToken.value) {
      void fitTerminal('create-session')
    }

    logger.info('[TerminalPanel] session created', {
      sessionId: session.session_id,
      profileId: session.profile_id,
      cwd: session.cwd,
    })

    return session
  } catch (error) {
    loadingStateText.value = `启动 ${requestedProfileId} 终端失败`
    logger.error('[TerminalPanel] create session failed', {
      requestedProfileId,
      error,
    })

    if (terminal) {
      terminal.writeln('')
      terminal.writeln(`[Guaxi] 启动 ${requestedProfileId} 终端失败`)
      terminal.writeln(String(error?.message || error))
    }
    return null
  } finally {
    panelBusy.value = false
  }
}

const switchProfileSession = async (profileId) => {
  const normalizedProfileId = String(profileId || '').trim()
  if (!normalizedProfileId || isSwitchingProfile.value) return
  if (activeSession.value?.profile_id === normalizedProfileId) return

  const existingSession = sessions.value.find(
    (session) => session.profile_id === normalizedProfileId && !session.exited
  )
  if (existingSession) {
    activateSession(existingSession.session_id)
    return
  }

  isSwitchingProfile.value = true
  try {
    if (terminal) {
      terminal.reset()
      terminal.clear()
      terminal.writeln(`[Guaxi] 正在切换到 ${normalizedProfileId}...`)
    }
    await createSession({
      profileId: normalizedProfileId,
      cwd: props.contextPath,
    })
  } finally {
    isSwitchingProfile.value = false
  }
}

const closeSession = async (sessionId, options = {}) => {
  const normalizedSessionId = String(sessionId || '').trim()
  if (!normalizedSessionId) return
  try {
    clearExitProbeTimer(normalizedSessionId)
    delete terminalCommandDrafts.value[normalizedSessionId]
    await invoke('terminal_close_session', {
      request: {
        session_id: normalizedSessionId,
      },
    })
  } catch (error) {
    logger.warn('[TerminalPanel] close session command failed', {
      sessionId: normalizedSessionId,
      error,
    })
  }

  const nextSessions = sessions.value.filter((session) => session.session_id !== normalizedSessionId)
  sessions.value = nextSessions
  logger.info('[TerminalPanel] session closed', {
    sessionId: normalizedSessionId,
    reason: String(options?.reason || 'manual-close'),
    remainingSessionCount: nextSessions.length,
  })
  if (activeSessionId.value === normalizedSessionId) {
    activeSessionId.value = nextSessions[0]?.session_id || ''
    renderActiveSession()
    await fitTerminal('close-session')
  }
  if (!nextSessions.length && options?.closePanelWhenEmpty) {
    emit('close')
  }
}

const killActiveSession = async () => {
  const session = activeSession.value
  if (!session) return

  // 这里显式区分“结束当前终端进程”和“仅关闭一个标签”。
  // 当最后一个会话被 kill 后，同步收起整个 terminal 面板，符合 VS Code 风格的终端体验。
  await closeSession(session.session_id, {
    reason: 'kill-active-session',
    closePanelWhenEmpty: sessions.value.length <= 1,
  })
}

const clearActiveSessionBuffer = () => {
  if (!activeSession.value) return
  const index = getSessionIndex(activeSession.value.session_id)
  if (index < 0) return
  sessions.value[index] = {
    ...sessions.value[index],
    buffer: '',
  }
  renderActiveSession()
  updateSearchMatches('clear-active-buffer')
}

const ensureSession = async (options = {}) => {
  if (sessions.value.length) {
    if (options?.profileId && activeSession.value?.profile_id !== String(options.profileId)) {
      return createSession(options)
    }
    await fitTerminal('ensure-existing-session')
    return activeSession.value
  }
  return createSession(options)
}

const activatePanel = async (reason = 'visible-change', options = {}) => {
  if (panelActivated.value) {
    return ensureSession(options)
  }

  panelBusy.value = true
  loadingStateText.value = '正在准备终端面板'
  try {
    await reloadProfiles()
    const existingSessions = await invoke('terminal_list_sessions')
    sessions.value = Array.isArray(existingSessions)
      ? existingSessions.map((session) => ({ ...session, buffer: '', exited: false }))
      : []
    activeSessionId.value = sessions.value[0]?.session_id || ''
    panelActivated.value = true
    renderActiveSession()

    logger.info('[TerminalPanel] panel activated', {
      reason,
      existingSessionCount: sessions.value.length,
    })

    return await ensureSession(options)
  } catch (error) {
    logger.error('[TerminalPanel] activate panel failed', { reason, error })
    loadingStateText.value = '终端面板初始化失败'
    return null
  } finally {
    panelBusy.value = false
  }
}

const initTerminal = async () => {
  if (terminal || !terminalHostRef.value) return

  fitAddon = new FitAddon()
  terminal = new Terminal({
    allowProposedApi: false,
    convertEol: false,
    cursorBlink: true,
    // 终端字体和主编辑器共用同一套变量，避免出现独立字体风格。
    fontFamily: getComputedStyle(document.documentElement)
      .getPropertyValue('--editor-font-family')
      .trim() || "'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace",
    // 适当放大终端字号，提升控制台输出的可读性。
    fontSize: 15,
    lineHeight: 1.35,
    scrollback: 10000,
    theme: getTerminalTheme(),
  })
  terminal.loadAddon(fitAddon)
  terminal.open(terminalHostRef.value)
  terminal.attachCustomKeyEventHandler((event) => {
    const isMod = event.ctrlKey || event.metaKey
    if (!isMod) {
      if (event.key === 'Escape' && searchVisible.value) {
        closeSearchPanel()
        return false
      }
      if (event.key === 'F3') {
        if (event.shiftKey) focusPrevSearchMatch()
        else focusNextSearchMatch()
        return false
      }
      return true
    }

    const normalizedKey = String(event.key || '').toLowerCase()
    if (event.shiftKey && normalizedKey === 'c') {
      void copySelectionToClipboard()
      return false
    }
    if (event.shiftKey && normalizedKey === 'v') {
      void pasteClipboardToTerminal()
      return false
    }
    if (normalizedKey === 'f') {
      void openSearchPanel()
      return false
    }
    if (normalizedKey === 'a') {
      terminal.selectAll()
      return false
    }
    return true
  })
  if (typeof ResizeObserver !== 'undefined') {
    terminalResizeObserver = new ResizeObserver(() => {
      scheduleFitTerminal('host-resize-observer')
    })
    if (terminalHostRef.value) {
      terminalResizeObserver.observe(terminalHostRef.value)
    }
    const hostParent = terminalHostRef.value?.parentElement
    if (hostParent && hostParent !== terminalHostRef.value) {
      terminalResizeObserver.observe(hostParent)
    }
  }

  // 这里把输入严格转发到当前活动会话，避免终端视图和后端 PTY 脱节。
  terminal.onData((data) => {
    const sessionId = activeSessionId.value
    if (!sessionId) return
    trackTerminalInput(sessionId, data)
    void invoke('terminal_write_session', {
      request: {
        session_id: sessionId,
        data,
      },
    }).catch((error) => {
      logger.error('[TerminalPanel] write input failed', {
        sessionId,
        error,
      })
    })
  })

  renderActiveSession()
  await fitTerminal('init-terminal')
}

const handleResize = () => {
  void fitTerminal('window-resize')
}

onMounted(async () => {
  await initTerminal()
  window.addEventListener('resize', handleResize)

  unlistenOutput = await listen('terminal-session-output', (event) => {
    const sessionId = String(event?.payload?.session_id || '').trim()
    if (!sessionId) return
    appendSessionOutput(sessionId, String(event?.payload?.data || ''))
  })

  unlistenExit = await listen('terminal-session-exit', (event) => {
    const sessionId = String(event?.payload?.session_id || '').trim()
    if (!sessionId) return
    syncExitedSession(sessionId, Number(event?.payload?.exit_code))
  })
})

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return
    await initTerminal()
    window.setTimeout(() => {
      void activatePanel('panel-visible')
        .then(() => fitTerminal('panel-visible'))
        .then(() => terminal?.focus())
    }, 0)
  },
  { immediate: true }
)

watch(
  selectedProfileId,
  async (nextProfileId, previousProfileId) => {
    if (!panelActivated.value || !props.visible) return
    if (!nextProfileId || nextProfileId === previousProfileId) return
    await switchProfileSession(nextProfileId)
  }
)

watch(
  () => settingsStore.appearanceTheme,
  () => {
    applyTerminalTheme()
    renderActiveSession()
  }
)

watch(searchKeyword, () => {
  updateSearchMatches('search-keyword-change')
})

onBeforeUnmount(async () => {
  window.removeEventListener('resize', handleResize)
  if (typeof unlistenOutput === 'function') {
    await unlistenOutput()
  }
  if (typeof unlistenExit === 'function') {
    await unlistenExit()
  }
  Object.keys(terminalExitProbeTimers.value).forEach((sessionId) => {
    clearExitProbeTimer(sessionId)
  })
  if (pendingFitFrame && typeof window !== 'undefined') {
    window.cancelAnimationFrame(pendingFitFrame)
    pendingFitFrame = 0
  }
  terminalResizeObserver?.disconnect?.()
  terminalResizeObserver = null
  terminal?.dispose?.()
  terminal = null
  fitAddon = null
})

defineExpose({
  ensureSession,
  createSession,
  fitTerminal,
  openSearch: openSearchPanel,
  focusTerminal: () => terminal?.focus?.(),
})
</script>

<style scoped>
.guaxi-terminal-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--guaxi-panel-bg, #1e1e1e);
}

.guaxi-terminal-panel__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 35px;
  padding: 0 8px;
  border-top: none;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 82%, transparent);
  background: var(--toolbar-bg, var(--guaxi-panel-bg, #252526));
  box-shadow: none;
}

.guaxi-terminal-panel__tabs {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.guaxi-terminal-panel__tool-tab {
  height: 35px;
  padding: 0 8px;
  border: 0;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-text-primary, #ffffff) 92%, transparent);
  background: transparent;
  color: var(--guaxi-text-primary, #ffffff);
  font-size: 12px;
  cursor: default;
  box-shadow: none;
}

.guaxi-terminal-panel__toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.guaxi-terminal-panel__profile-select,
.guaxi-terminal-panel__icon-btn {
  height: 24px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 88%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 92%, transparent);
  color: var(--guaxi-text-primary, #d4d4d4);
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
}

.guaxi-terminal-panel__profile-select {
  min-width: 140px;
  padding: 0 8px;
}

.guaxi-terminal-panel__icon-btn {
  min-width: 24px;
  padding: 0 6px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.guaxi-terminal-panel__content {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 152px;
  flex: 1;
  min-height: 0;
}

.guaxi-terminal-panel__content.without-sidebar {
  grid-template-columns: minmax(0, 1fr);
}

.guaxi-terminal-panel__icon-btn svg,
.guaxi-terminal-panel__session-close svg {
  display: block;
  flex: 0 0 auto;
}

.guaxi-terminal-panel__main,
.guaxi-terminal-panel__body {
  min-width: 0;
  min-height: 0;
  height: 100%;
}

.guaxi-terminal-panel__body {
  position: relative;
}

.guaxi-terminal-panel__search {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 3;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 34px;
  padding: 6px 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 84%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--toolbar-bg, var(--guaxi-panel-bg, #252526)) 96%, transparent);
  box-shadow: none;
}

.guaxi-terminal-panel__search-input {
  width: 180px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 88%, transparent);
  border-radius: 4px;
  background: var(--guaxi-panel-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #d4d4d4);
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 12px;
}

.guaxi-terminal-panel__search-count {
  min-width: 40px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12px;
  text-align: center;
}

.guaxi-terminal-panel__search-btn {
  height: 24px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 88%, transparent);
  border-radius: 4px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 92%, transparent);
  color: var(--guaxi-text-primary, #d4d4d4);
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 12px;
  cursor: pointer;
}

.guaxi-terminal-panel__search-btn--close {
  color: color-mix(in srgb, #b91c1c 78%, var(--guaxi-text-primary));
}

.guaxi-terminal-panel__terminal {
  height: 100%;
  min-height: 0;
  background: var(--guaxi-panel-bg, #1e1e1e);
}

.guaxi-terminal-panel__terminal :deep(.xterm) {
  height: 100%;
  padding: 0;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
}

.guaxi-terminal-panel__terminal :deep(.xterm-screen),
.guaxi-terminal-panel__terminal :deep(.xterm-scroll-area) {
  width: 100% !important;
}

.guaxi-terminal-panel__terminal :deep(.xterm-viewport) {
  width: 100% !important;
  height: 100% !important;
  max-height: 100% !important;
  overflow-y: auto !important;
  scrollbar-gutter: stable;
}

.guaxi-terminal-panel__status-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #1e1e1e) 92%, transparent);
  color: var(--guaxi-text-primary, #d4d4d4);
  pointer-events: none;
}

.guaxi-terminal-panel__status-title,
.guaxi-terminal-panel__status-subtitle {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
}

.guaxi-terminal-panel__status-subtitle {
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12px;
}

.guaxi-terminal-panel__sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-left: 1px solid color-mix(in srgb, var(--guaxi-border, #2b2b2b) 82%, transparent);
  background: var(--toolbar-bg, var(--guaxi-panel-bg, #252526));
}

.guaxi-terminal-panel__sidebar-title {
  padding: 8px 10px 6px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.guaxi-terminal-panel__session-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.guaxi-terminal-panel__session-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-height: 30px;
  padding: 0 8px;
  border: 0;
  background: transparent;
  color: var(--guaxi-text-primary, #c8c8c8);
  cursor: pointer;
}

.guaxi-terminal-panel__session-item.active {
  background: color-mix(in srgb, var(--primary-color, #4acadb) 18%, var(--guaxi-panel-bg, #37373d));
  color: var(--guaxi-text-primary, #ffffff);
}

.guaxi-terminal-panel__session-label,
.guaxi-terminal-panel__session-state,
.guaxi-terminal-panel__session-close,
.guaxi-terminal-panel__session-empty {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 12px;
}

.guaxi-terminal-panel__session-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.guaxi-terminal-panel__session-state {
  color: color-mix(in srgb, #d7ba7d 72%, var(--guaxi-text-primary, #d4d4d4));
}

.guaxi-terminal-panel__session-close {
  opacity: 0.7;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.guaxi-terminal-panel__session-empty {
  padding: 8px 10px;
  color: var(--guaxi-text-secondary, #9da5b4);
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__status-overlay {
  color: #1f2328;
}
</style>
