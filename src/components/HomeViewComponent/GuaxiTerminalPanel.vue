<template>
  <div class="guaxi-terminal-panel">
    <div class="guaxi-terminal-panel__toolbar">
      <div class="guaxi-terminal-panel__tabs">
        <button type="button" class="guaxi-terminal-panel__tool-tab active">TERMINAL</button>
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
              <span class="codicon codicon-plus"></span>
            </button>
          </template>
          新建终端
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button type="button" class="guaxi-terminal-panel__icon-btn" title="刷新终端配置" @click="reloadProfiles">
              <span class="codicon codicon-refresh"></span>
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
              <span class="codicon codicon-clear-all"></span>
            </button>
          </template>
          清空当前终端输出
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <button type="button" class="guaxi-terminal-panel__icon-btn" title="收起终端" @click="$emit('close')">
              <span class="codicon codicon-chevron-down"></span>
            </button>
          </template>
          收起终端
        </n-tooltip>
      </div>
    </div>

    <div class="guaxi-terminal-panel__content" :class="{ 'without-sidebar': !showSidebar }">
      <div class="guaxi-terminal-panel__main">
        <div class="guaxi-terminal-panel__body">
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
                  <span class="codicon codicon-close"></span>
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
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'

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

defineEmits(['close'])

const terminalHostRef = ref(null)
const sessions = ref([])
const activeSessionId = ref('')
const profiles = ref([])
const defaultProfileId = ref('')
const selectedProfileId = ref('')
const panelBusy = ref(false)
const panelActivated = ref(false)
const loadingStateText = ref('等待创建终端会话')

let terminal = null
let fitAddon = null
let unlistenOutput = null
let unlistenExit = null

const isWindows = typeof navigator !== 'undefined'
  ? /Windows/i.test(navigator.userAgent)
  : true

const availableProfiles = computed(() => profiles.value.filter((profile) => profile.available))
const activeSession = computed(() => sessions.value.find((session) => session.session_id === activeSessionId.value) || null)
const showSidebar = computed(() => sessions.value.length > 1)
const isSwitchingProfile = ref(false)

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

const getTerminalTheme = () => {
  const dark = settingsStore.appearanceTheme === 'dark'
  return dark
    ? {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#aeafad',
        selectionBackground: 'rgba(38, 79, 120, 0.45)',
      }
    : {
        background: '#ffffff',
        foreground: '#1f2328',
        cursor: '#0969da',
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
  try {
    panelBusy.value = true
    loadingStateText.value = `正在启动 ${requestedProfileId} 终端`
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
    await fitTerminal('create-session')

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
    await createSession({
      profileId: normalizedProfileId,
      cwd: props.contextPath,
    })
  } finally {
    isSwitchingProfile.value = false
  }
}

const closeSession = async (sessionId) => {
  const normalizedSessionId = String(sessionId || '').trim()
  if (!normalizedSessionId) return
  try {
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
  if (activeSessionId.value === normalizedSessionId) {
    activeSessionId.value = nextSessions[0]?.session_id || ''
    renderActiveSession()
    await fitTerminal('close-session')
  }
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
    fontSize: 14,
    lineHeight: 1.3,
    scrollback: 10000,
    theme: getTerminalTheme(),
  })
  terminal.loadAddon(fitAddon)
  terminal.open(terminalHostRef.value)

  // 这里把输入严格转发到当前活动会话，避免终端视图和后端 PTY 脱节。
  terminal.onData((data) => {
    const sessionId = activeSessionId.value
    if (!sessionId) return
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
    markSessionExited(sessionId, Number(event?.payload?.exit_code))
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

onBeforeUnmount(async () => {
  window.removeEventListener('resize', handleResize)
  if (typeof unlistenOutput === 'function') {
    await unlistenOutput()
  }
  if (typeof unlistenExit === 'function') {
    await unlistenExit()
  }
  terminal?.dispose?.()
  terminal = null
  fitAddon = null
})

defineExpose({
  ensureSession,
  createSession,
  fitTerminal,
  focusTerminal: () => terminal?.focus?.(),
})
</script>

<style scoped>
.guaxi-terminal-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: #1e1e1e;
}

.guaxi-terminal-panel__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 35px;
  padding: 0 8px;
  border-bottom: 1px solid #2b2b2b;
  background: #252526;
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
  border-bottom: 1px solid #ffffff;
  background: transparent;
  color: #ffffff;
  font-size: 12px;
  cursor: default;
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
  border: 1px solid #3a3d41;
  border-radius: 3px;
  background: #2d2d30;
  color: #d4d4d4;
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

.guaxi-terminal-panel__icon-btn .codicon,
.guaxi-terminal-panel__session-close .codicon {
  font-size: 14px;
  line-height: 1;
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

.guaxi-terminal-panel__terminal {
  height: 100%;
  min-height: 0;
  background: #1e1e1e;
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
  background: rgba(30, 30, 30, 0.92);
  color: #d4d4d4;
  pointer-events: none;
}

.guaxi-terminal-panel__status-title,
.guaxi-terminal-panel__status-subtitle {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
}

.guaxi-terminal-panel__status-subtitle {
  color: #9da5b4;
  font-size: 12px;
}

.guaxi-terminal-panel__sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-left: 1px solid #2b2b2b;
  background: #252526;
}

.guaxi-terminal-panel__sidebar-title {
  padding: 8px 10px 6px;
  color: #9da5b4;
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
  color: #c8c8c8;
  cursor: pointer;
}

.guaxi-terminal-panel__session-item.active {
  background: #37373d;
  color: #ffffff;
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
  color: #d7ba7d;
}

.guaxi-terminal-panel__session-close {
  opacity: 0.7;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.guaxi-terminal-panel__session-empty {
  padding: 8px 10px;
  color: #9da5b4;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel {
  background: #ffffff;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__toolbar,
:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__sidebar {
  background: #f3f3f3;
  border-color: #dddddd;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__tool-tab {
  color: #1f2328;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__sidebar-title,
:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__session-empty,
:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__status-subtitle {
  color: #6b7280;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__profile-select,
:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__icon-btn {
  background: #ffffff;
  color: #1f2328;
  border-color: #d0d7de;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__terminal {
  background: #ffffff;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__session-item {
  color: #1f2328;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__session-item.active {
  background: #e7eefb;
}

:global(:root[data-guaxi-theme='light']) .guaxi-terminal-panel__status-overlay {
  background: rgba(255, 255, 255, 0.92);
  color: #1f2328;
}
</style>
