<template>
  <div class="assistant-root">
    <div class="assistant-topbar">
      <div class="assistant-title-group">
        <div class="assistant-orb" />
        <div class="assistant-title-wrap">
          <div class="assistant-title">智能助手</div>
          <div class="assistant-subtitle">知识库问答 · 联网增强</div>
        </div>
      </div>
      <div class="assistant-topbar-right">
        <n-switch
          :value="webSearchEnabled"
          size="small"
          @update:value="handleWebSearchEnabledChange"
        >
          <template #checked>联网</template>
          <template #unchecked>离线</template>
        </n-switch>
        <n-button quaternary size="small" @click="clearHistory">清空</n-button>
      </div>
    </div>

    <div ref="messagesRef" class="assistant-messages" @scroll="handleMessagesScroll">
      <div v-if="messages.length === 0" class="assistant-empty">
        <div class="assistant-empty-icon">
          <svg viewBox="0 0 24 24" width="24" height="24">
            <path
              fill="currentColor"
              d="M12 2a7 7 0 0 1 7 7v1.1a4.9 4.9 0 0 1 2.34 4.18A4.72 4.72 0 0 1 16.62 19H12.5L9 22v-3H7.38A4.72 4.72 0 0 1 2.66 14.3a4.9 4.9 0 0 1 2.34-4.2V9a7 7 0 0 1 7-7m0 2a5 5 0 0 0-5 5v2.39l-.63.32a2.9 2.9 0 0 0-1.71 2.59A2.73 2.73 0 0 0 7.38 17H11v.66l.83-.66h4.79a2.73 2.73 0 0 0 2.72-2.7a2.9 2.9 0 0 0-1.7-2.59l-.64-.32V9a5 5 0 0 0-5-5"
            />
          </svg>
        </div>
        <div class="assistant-empty-title">开始一次智能问答</div>
        <div class="assistant-empty-desc">支持本地 DeepSeek 与联网搜索增强</div>
      </div>

      <div
        v-for="message in messages"
        :key="message.id"
        class="assistant-message"
        :class="message.role === 'user' ? 'assistant-message-user' : message.role === 'tool_step' ? 'assistant-message-tool' : 'assistant-message-ai'"
      >
        <div v-if="message.role !== 'tool_step'" class="assistant-message-avatar">
          <span v-if="message.role === 'user'">我</span>
          <span v-else-if="message.role === 'web'">网</span>
          <span v-else>AI</span>
        </div>
        <div class="assistant-message-body">
          <div class="assistant-message-meta">
            <span>{{ message.role === 'user' ? '你' : (message.role === 'web' ? '联网检索' : '智能助手') }}</span>
            <span>{{ formatTime(message.createdAt) }}</span>
          </div>
          <div
            v-if="message.role === 'assistant' && message.streaming && !message.html"
            class="assistant-message-content assistant-message-plain"
          >{{ message.content }}</div>
          <div
            v-else-if="message.role === 'assistant'"
            class="assistant-message-content markdown-body"
            v-html="message.html || message.content"
          />
          <div
            v-else-if="message.role === 'web'"
            class="assistant-message-content assistant-message-plain"
          >
            {{ message.content || '联网搜索结果' }}
          </div>
          <div
            v-else
            class="assistant-message-content assistant-message-plain"
          >
            {{ message.content }}
          </div>
          <div
            v-if="message.role === 'user' && getMessageContextItems(message).length"
            class="assistant-message-context-list"
          >
            <div
              v-for="item in getMessageContextItems(message)"
              :key="`${message.id}-${item.kind}-${item.path}`"
              class="assistant-message-context-chip"
              :class="`assistant-message-context-chip--${item.kind}`"
              :title="item.path"
            >
              <span class="assistant-message-context-kind">
                <Icon
                  v-if="item.kind === 'mention'"
                  :icon="atSignIcon"
                  width="11"
                  height="11"
                />
                <Icon
                  v-else
                  :icon="fileIcon"
                  width="11"
                  height="11"
                />
              </span>
              <Icon class="assistant-message-context-icon" :icon="getMessageContextIcon(item)" width="13" height="13" />
              <span class="assistant-message-context-name">{{ item.name }}</span>
            </div>
          </div>
          <!-- Agent 工具调用步骤卡片 -->
          <div
            v-if="message.role === 'tool_step'"
            class="agent-tool-card"
            :class="'agent-tool-card--' + (message.status || 'running')"
          >
            <div class="agent-tool-card-head" @click="toggleToolStepCollapse(message)">
              <span class="agent-tool-card-icon">
                <Icon :icon="getAgentToolIcon(message)" width="15" height="15" />
              </span>
              <span class="agent-tool-card-label">{{ message.toolLabel }}</span>
              <span class="agent-tool-card-status">
                <span v-if="message.status === 'running'" class="agent-tool-spinner" />
                <Icon v-else-if="message.status === 'done'" :icon="checkIcon" width="14" height="14" />
                <Icon v-else-if="message.status === 'error'" :icon="alertTriangleIcon" width="14" height="14" />
              </span>
              <span class="agent-tool-card-toggle">
                <Icon
                  :icon="chevronRightIcon"
                  width="14"
                  height="14"
                  :class="{ 'agent-tool-card-toggle--open': !message.collapsed }"
                />
              </span>
            </div>
            <div v-if="!message.collapsed" class="agent-tool-card-body">
              <div v-if="Object.keys(message.args || {}).length" class="agent-tool-card-section">
                <div class="agent-tool-card-section-title">参数</div>
                <pre class="agent-tool-card-pre">{{ JSON.stringify(message.args, null, 2) }}</pre>
              </div>
              <div v-if="message.result" class="agent-tool-card-section">
                <div class="agent-tool-card-section-title">结果</div>
                <pre class="agent-tool-card-pre agent-tool-card-result">{{ typeof message.result === 'string' ? message.result : JSON.stringify(message.result, null, 2) }}</pre>
              </div>
              <div v-if="message.status === 'error' && !message.result" class="agent-tool-card-section">
                <div class="agent-tool-card-section-title agent-tool-card-error">执行失败</div>
              </div>
            </div>
          </div>
          <div
            v-if="message.role === 'web' && !message.meta?.agentStep"
            class="assistant-web-toggle-wrap"
          >
            <button
              type="button"
              class="assistant-web-toggle-btn"
              @click="toggleWebMessageCollapse(message.id)"
            >
              {{ isWebMessageCollapsed(message.id) ? '展开联网结果' : '收起联网结果' }}
            </button>
          </div>
          <div
            v-if="message.meta?.webContextCount && (message.role !== 'web' || !isWebMessageCollapsed(message.id))"
            class="assistant-message-source"
          >
            参考了 {{ message.meta.webContextCount }} 条联网结果
          </div>
          <div
            v-else-if="message.meta?.webSearchTriggered && (message.role !== 'web' || !isWebMessageCollapsed(message.id))"
            class="assistant-message-source"
          >
            已启用联网搜索，但未检索到可用结果，本次按离线答案回复
          </div>
          <div
            v-if="Array.isArray(message.meta?.webReferences) && message.meta.webReferences.length && (message.role !== 'web' || !isWebMessageCollapsed(message.id))"
            class="assistant-reference-list"
          >
            <div class="assistant-reference-title">参考来源</div>
            <a
              v-for="(refItem, idx) in message.meta.webReferences"
              :key="`${message.id}-ref-${idx}`"
              class="assistant-reference-card"
              :href="refItem.url"
              target="_blank"
              rel="noopener noreferrer"
              :title="refItem.url"
            >
              <div class="assistant-reference-head">
                <img
                  class="assistant-reference-favicon"
                  :src="getFaviconUrl(refItem.url)"
                  alt=""
                  loading="lazy"
                >
                <div class="assistant-reference-host">
                  {{ getHostFromUrl(refItem.url) || '来源站点' }}
                </div>
                <div class="assistant-reference-index">#{{ idx + 1 }}</div>
              </div>
              <div class="assistant-reference-link-title">
                {{ refItem.title || refItem.url }}
              </div>
              <div v-if="refItem.snippet" class="assistant-reference-snippet">
                {{ refItem.snippet }}
              </div>
            </a>
          </div>
          <div v-if="message.role === 'assistant'" class="assistant-message-actions">
            <button type="button" class="assistant-action-btn" title="复制" @click="copyAssistantMessage(message)">
              <Icon :icon="copyIcon" width="14" height="14" />
            </button>
            <button type="button" class="assistant-action-btn" title="插入当前文件" @click="insertAssistantToCurrentFile(message)">
              <Icon :icon="filePenLineIcon" width="14" height="14" />
            </button>
            <button type="button" class="assistant-action-btn" title="创建新文件" @click="createAssistantAsNewFile(message)">
              <Icon :icon="filePlusIcon" width="14" height="14" />
            </button>
          </div>
          <div v-else-if="message.role === 'user'" class="assistant-message-actions">
            <button type="button" class="assistant-action-btn" title="复制提问" @click="copyAssistantMessage(message)">
              <Icon :icon="copyIcon" width="14" height="14" />
            </button>
          </div>
        </div>
      </div>

      <div v-if="pending" class="assistant-thinking-wrap">
        <div class="assistant-thinking-card">
          <div class="assistant-thinking-ring" />
          <div class="assistant-thinking-text">模型思考中</div>
          <div class="assistant-thinking-dots">
            <span />
            <span />
            <span />
          </div>
        </div>
      </div>
    </div>

    <button
      v-if="showJumpToLatest"
      type="button"
      class="assistant-jump-latest"
      title="跳到最新内容"
      @click="scrollToLatestFromButton"
    >
      <Icon :icon="arrowDownIcon" width="16" height="16" />
    </button>

    <div v-if="agentConfirm" class="agent-confirm-card">
      <div class="agent-confirm-head">
        <span class="agent-confirm-badge">需确认</span>
        <span class="agent-confirm-title">AI 请求执行：{{ agentConfirm.toolLabel }}</span>
      </div>
      <div class="agent-confirm-field">
        <label class="agent-confirm-label">路径</label>
        <input v-model="agentConfirm.path" class="agent-confirm-input" type="text" placeholder="文件路径" />
      </div>
      <div v-if="agentConfirm.isWriteFile" class="agent-confirm-field">
        <label class="agent-confirm-label">模式</label>
        <select v-model="agentConfirm.mode" class="agent-confirm-input">
          <option value="overwrite">覆盖</option>
          <option value="append">追加</option>
        </select>
      </div>
      <div v-if="agentConfirm.hasContent" class="agent-confirm-field">
        <label class="agent-confirm-label">内容（可编辑）</label>
        <textarea v-model="agentConfirm.content" class="agent-confirm-textarea" rows="6" />
      </div>
      <div class="agent-confirm-actions">
        <button type="button" class="agent-confirm-btn agent-confirm-reject" @click="resolveAgentConfirm(false)">拒绝</button>
        <button type="button" class="agent-confirm-btn agent-confirm-approve" @click="resolveAgentConfirm(true)">允许并执行</button>
      </div>
    </div>

    <div class="assistant-footer">
      <div class="composer-shell" @click="focusComposerInput">
        <!-- @ 引用：知识库文件模糊搜索弹层 -->
        <div
          v-if="mentionPanel.open"
          class="mention-panel"
          @mousedown.prevent
          @click.stop
        >
          <div class="mention-panel-head">
            <span class="mention-panel-title">引用知识库文件</span>
            <span class="mention-panel-hint">↑↓ 选择 · Enter 确认 · Esc 关闭</span>
          </div>
          <div v-if="mentionPanel.loading" class="mention-empty">搜索中…</div>
          <div v-else-if="!mentionPanel.items.length" class="mention-empty">未找到匹配文件</div>
          <ul v-else ref="mentionListRef" class="mention-list">
            <li
              v-for="(item, idx) in mentionPanel.items"
              :key="item.path"
              class="mention-item"
              :class="{ 'mention-item--active': idx === mentionPanel.activeIndex }"
              @mouseenter="mentionPanel.activeIndex = idx"
              @click="selectMention(item)"
            >
              <span class="mention-item-icon">
                <Icon :icon="getMentionFileIcon(item)" width="15" height="15" />
              </span>
              <span class="mention-item-name">{{ item.name }}</span>
              <span class="mention-item-path" :title="item.path">{{ formatMentionLocation(item) }}</span>
            </li>
          </ul>
        </div>

        <div v-if="pendingAttachments.length" class="attachment-list">
          <div v-for="file in pendingAttachments" :key="file.id" class="attachment-chip">
            <Icon class="attachment-icon" :icon="fileIcon" width="13" height="13" />
            <span class="attachment-name" :title="file.path">{{ file.name }}</span>
            <span class="attachment-size">{{ formatAttachmentSize(file.size) }}</span>
            <button class="attachment-remove" type="button" title="移除附件" @click="removeAttachment(file.id)">
              <Icon :icon="xIcon" width="12" height="12" />
            </button>
          </div>
        </div>

        <div v-if="mentionedFiles.length" class="attachment-list">
          <div v-for="file in mentionedFiles" :key="file.id" class="attachment-chip mention-chip">
            <span class="mention-chip-at">
              <Icon :icon="atSignIcon" width="12" height="12" />
            </span>
            <Icon class="attachment-icon" :icon="getMentionFileIcon(file)" width="13" height="13" />
            <span class="attachment-name" :title="file.path">{{ file.name }}</span>
            <button class="attachment-remove" type="button" title="移除引用" @click="removeMention(file.id)">
              <Icon :icon="xIcon" width="12" height="12" />
            </button>
          </div>
        </div>

        <textarea
          ref="composerInputRef"
          class="composer-input"
          placeholder="输入问题，@ 引用知识库文件，Enter 发送，Shift + Enter 换行"
          v-model="question"
          :disabled="pending"
          @compositionstart="handleCompositionStart"
          @compositionend="handleCompositionEnd"
          @input="handleComposerInput"
          @keydown="handleInputKeydown"
        />

        <div class="composer-toolbar">
          <button
            class="composer-icon-btn"
            type="button"
            title="添加附件"
            :disabled="pending"
            @click="pickAttachments"
          >
            <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
              <path d="M17.5 6.5v9a5.5 5.5 0 0 1-11 0v-10a4 4 0 0 1 8 0v9a2.5 2.5 0 1 1-5 0V8h1.8v6.5a.7.7 0 1 0 1.4 0v-9a2.2 2.2 0 0 0-4.4 0v10a3.7 3.7 0 0 0 7.4 0v-9h1.8z" fill="currentColor" />
            </svg>
          </button>
          <div @mousedown.stop @click.stop>
            <n-select
              class="composer-task-select"
              size="small"
              :value="task"
              :options="taskOptions"
              :consistent-menu-width="false"
              @update:value="handleTaskChange"
            />
          </div>
          <div @mousedown.stop @click.stop>
            <n-select
              class="composer-model-select"
              size="small"
              :value="modelId"
              :options="modelOptions"
              :consistent-menu-width="false"
              @update:value="handleModelChange"
            />
          </div>
          <div class="composer-spacer"></div>
          <button
            class="send-btn"
            type="button"
            :disabled="!canClickSend"
            @click="handleSendButtonClick"
            :title="pending ? '停止生成' : '发送'"
          >
            <svg
              v-if="pending"
              viewBox="0 0 24 24"
              width="18"
              height="18"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path d="M7 7h10v10H7z" fill="currentColor" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
              <path d="M12 4l-7 7 1.4 1.4 4.6-4.6V20h2V7.8l4.6 4.6L19 11z" fill="currentColor" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import copyIcon from '@iconify-icons/lucide/copy'
import filePenLineIcon from '@iconify-icons/lucide/file-edit'
import filePlusIcon from '@iconify-icons/lucide/file-plus'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import fileIcon from '@iconify-icons/lucide/file'
import imageIcon from '@iconify-icons/lucide/image'
import folderIcon from '@iconify-icons/lucide/folder'
import searchIcon from '@iconify-icons/lucide/search'
import globeIcon from '@iconify-icons/lucide/globe'
import terminalIcon from '@iconify-icons/lucide/terminal'
import databaseIcon from '@iconify-icons/lucide/database'
import wrenchIcon from '@iconify-icons/lucide/wrench'
import atSignIcon from '@iconify-icons/lucide/at-sign'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
import checkIcon from '@iconify-icons/lucide/check'
import alertTriangleIcon from '@iconify-icons/lucide/alert-triangle'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import xIcon from '@iconify-icons/lucide/x'
import { addCopyButtons, addMediaViewerButtons, renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useChatStore } from '../../stores/chatStore'
import { useEditorStore } from '../../stores/editorStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiAgentAssistant')
const message = useMessage()
const rightSiderStore = useRightSiderStore()
const chatStore = useChatStore()
const editorStore = useEditorStore()
const fileTreeStore = useFileTreeStore()
const STREAM_EVENT = 'modle-streaming-respond-output'
const WEB_PUSH_EVENT = 'web-model-chat-push'
const WEB_MODEL_IDS = new Set([7, 8])
const AGENT_TOOL_LABELS = {
  kb_semantic_search: '检索知识库',
  read_file: '读取文件',
  list_knowledge_bases: '列出知识库',
  list_directory: '浏览目录',
  create_note: '创建笔记',
  write_file: '写入文件',
  search_files: '搜索文件',
  web_search: '网络搜索',
  fetch_webpage: '抓取网页',
  exec_python: '执行 Python',
  exec_shell: '执行命令',
  agent_stream: 'Agent 流',
}
const AGENT_TOOL_ICONS = {
  read_file: fileTextIcon,
  list_directory: folderIcon,
  search_files: searchIcon,
  write_file: filePenLineIcon,
  create_note: filePlusIcon,
  web_search: globeIcon,
  fetch_webpage: fileTextIcon,
  exec_python: terminalIcon,
  exec_shell: terminalIcon,
  kb_semantic_search: databaseIcon,
  list_knowledge_bases: databaseIcon,
  default: wrenchIcon
}

const getAgentToolIcon = (chatMessage) => {
  const toolName = String(chatMessage?.toolName || chatMessage?.name || chatMessage?.tool || '')
  return AGENT_TOOL_ICONS[toolName] || AGENT_TOOL_ICONS.default
}
const messagesRef = ref(null)
const composerInputRef = ref(null)
const mentionListRef = ref(null)
const question = ref('')
const pendingAttachments = ref([])
// @ 引用：被引用的知识库文件清单（按 path 去重）
const mentionedFiles = ref([])
// @ 引用：模糊搜索弹层状态
const mentionPanel = ref({ open: false, query: '', items: [], activeIndex: 0, loading: false })
// 触发 @ 的起始下标（光标前那个 @ 的位置），用于选中后替换文本片段
const mentionAnchor = ref(-1)
const mentionSearchTimer = ref(null)
const mentionSearchSeq = ref(0)
const streamUnlisten = ref(null)
const webPushUnlisten = ref(null)
const agentStepUnlisten = ref(null)
const agentConfirmUnlisten = ref(null)
const agentConfirm = ref(null)
const streamRenderTimer = ref(null)
const isComposing = ref(false)
const inputHistory = ref([])
const historyIndex = ref(-1)
const collapsedWebMessageIds = ref(new Set())
const disableWebSearchNextSend = ref(false)
const currentStreamText = ref('')
const currentStreamingMessageId = ref('')
const currentRequestModelId = ref(0)
const currentRequestSessionId = ref('')
const isMessagesNearBottom = ref(true)

const messages = computed(() => rightSiderStore.assistant.messages)
const pending = computed(() => rightSiderStore.assistant.pending)
const modelId = computed(() => rightSiderStore.assistant.modelId)
const webSearchEnabled = computed(() => rightSiderStore.assistant.webSearchEnabled)
const task = computed(() => rightSiderStore.assistant.task || 'chat')
const taskOptions = [
  { value: 'chat', label: '对话' },
  { value: 'agent', label: 'Agent' },
  { value: 'write', label: '文档编写' },
  { value: 'translate', label: '翻译' },
  { value: 'organize', label: '知识整理' },
  { value: 'classify', label: '分类' },
  { value: 'tag', label: '打标签' },
]
const hasComposerPayload = computed(() => {
  return question.value.trim().length > 0 ||
    pendingAttachments.value.length > 0 ||
    mentionedFiles.value.length > 0
})
const canSend = computed(() => !pending.value && hasComposerPayload.value)
const canClickSend = computed(() => pending.value || hasComposerPayload.value)
const showJumpToLatest = computed(() => messages.value.length > 0 && !isMessagesNearBottom.value)
const modelOptions = computed(() => {
  const remote = Array.isArray(chatStore.LLMlist?.remoteModel) ? chatStore.LLMlist.remoteModel : []
  const local = Array.isArray(chatStore.LLMlist?.localModel) ? chatStore.LLMlist.localModel : []
  return [...remote, ...local]
    .map((item) => {
      const value = Number(item?.selectedModel)
      const label = String(item?.name || '')
      return { value, label }
    })
    .filter((item) => Number.isFinite(item.value) && item.value !== 7 && item.value !== 8)
})

const enhanceRenderedMessages = () => {
  nextTick(() => {
    const root = messagesRef.value
    if (!root) return
    const nodes = Array.from(root.querySelectorAll('.assistant-message-content.markdown-body'))
    nodes.forEach((node) => {
      addCopyButtons(node)
      addMediaViewerButtons(node)
    })
  })
}

const MESSAGE_BOTTOM_THRESHOLD = 48

const updateMessagesNearBottom = () => {
  const el = messagesRef.value
  if (!el) return true
  const distance = el.scrollHeight - el.scrollTop - el.clientHeight
  const near = distance <= MESSAGE_BOTTOM_THRESHOLD
  if (near !== isMessagesNearBottom.value) {
    logger.info('[assistant] message scroll anchor changed', { nearBottom: near, distance })
  }
  isMessagesNearBottom.value = near
  return near
}

const handleMessagesScroll = () => {
  updateMessagesNearBottom()
}

const scrollToBottom = ({ smooth = false, reason = 'auto' } = {}) => {
  nextTick(() => {
    const el = messagesRef.value
    if (!el) return
    if (typeof el.scrollTo === 'function') {
      el.scrollTo({ top: el.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
    } else {
      el.scrollTop = el.scrollHeight
    }
    isMessagesNearBottom.value = true
    logger.debug('[assistant] scroll to latest', { reason, smooth })
  })
}

const maybeScrollToBottom = (reason) => {
  if (isMessagesNearBottom.value) {
    scrollToBottom({ reason })
    return
  }
  logger.debug('[assistant] keep reader scroll position', { reason })
}

const scrollToLatestFromButton = () => {
  logger.info('[assistant] jump latest clicked')
  scrollToBottom({ smooth: true, reason: 'jump-button' })
}

watch(messages, () => {
  // 设计说明：消息内容、工具卡片展开状态、联网结果折叠状态都会触发 messages 深度变化。
  // 只有用户本来贴近底部时才自动跟随，避免阅读旧内容时被展开/收起操作拉回底部。
  maybeScrollToBottom('messages-change')
  enhanceRenderedMessages()
}, { deep: true })

watch(pending, () => {
  maybeScrollToBottom('pending-change')
})

const applyDraftFromStore = () => {
  const payload = rightSiderStore.consumeAssistantDraftPayload?.() || {}
  const draft = String(payload?.text || '').trim()
  if (!draft) return
  if (payload?.disableWebSearch) {
    disableWebSearchNextSend.value = true
  }
  question.value = question.value.trim() ? `${question.value.trim()}\n${draft}` : draft
  nextTick(focusComposerInput)
  if (payload?.autoSend && !pending.value) {
    sendQuestion()
  }
}

watch(
  () => [
    rightSiderStore.assistant.draftInput,
    rightSiderStore.assistant.draftAutoSend,
    rightSiderStore.assistant.draftDisableWebSearch
  ],
  ([value, autoSend, disableWebSearch]) => {
    if (!value && !autoSend && !disableWebSearch) return
    applyDraftFromStore()
  }
)

watch(modelOptions, (list) => {
  const current = Number(modelId.value)
  const exists = Array.isArray(list) && list.some((x) => Number(x?.value) === current)
  if (!exists) {
    rightSiderStore.setAssistantModelId(5)
  }
}, { immediate: true })

onMounted(() => {
  applyDraftFromStore()
  enhanceRenderedMessages()
  initAssistantEventListeners().catch((err) => {
    logger.error('[assistant] init event listeners failed', err)
  })
})

onBeforeUnmount(() => {
  if (typeof streamUnlisten.value === 'function') {
    streamUnlisten.value()
    streamUnlisten.value = null
  }
  if (typeof webPushUnlisten.value === 'function') {
    webPushUnlisten.value()
    webPushUnlisten.value = null
  }
  if (typeof agentStepUnlisten.value === 'function') {
    agentStepUnlisten.value()
    agentStepUnlisten.value = null
  }
  if (typeof agentConfirmUnlisten.value === 'function') {
    agentConfirmUnlisten.value()
    agentConfirmUnlisten.value = null
  }
  if (streamRenderTimer.value) {
    clearTimeout(streamRenderTimer.value)
    streamRenderTimer.value = null
  }
  if (mentionSearchTimer.value) {
    clearTimeout(mentionSearchTimer.value)
    mentionSearchTimer.value = null
  }
})

const formatTime = (ts) => {
  if (!ts) return ''
  try {
    return new Date(ts).toLocaleTimeString()
  } catch {
    return ''
  }
}

const clearHistory = () => {
  rightSiderStore.clearAssistantMessages()
  collapsedWebMessageIds.value = new Set()
  mentionedFiles.value = []
  closeMentionPanel()
}

const isWebMessageCollapsed = (messageId) => collapsedWebMessageIds.value.has(String(messageId || ''))

const toggleWebMessageCollapse = (messageId) => {
  const id = String(messageId || '')
  if (!id) return
  const next = new Set(collapsedWebMessageIds.value)
  const collapsed = !next.has(id)
  if (next.has(id)) {
    next.delete(id)
  } else {
    next.add(id)
  }
  collapsedWebMessageIds.value = next
  logger.info('[assistant] toggle web result collapse', { messageId: id, collapsed })
}

const toggleToolStepCollapse = (chatMessage) => {
  if (!chatMessage) return
  chatMessage.collapsed = !chatMessage.collapsed
  logger.info('[assistant] toggle tool step collapse', {
    messageId: chatMessage.id,
    toolName: chatMessage.toolName,
    collapsed: chatMessage.collapsed,
  })
}

const handleModelChange = (value) => {
  rightSiderStore.setAssistantModelId(value)
}

const handleTaskChange = (value) => {
  rightSiderStore.setAssistantTask(value)
  // 设计说明：此前非对话类任务（agent/文档编写/分类/整理/打标签）会被强制切到本地
  // LocalQwen(id 9)，相当于限制只能用该模型。后端的 doc_agent_process / agent_chat_stream
  // 实际都按用户所选 modelid 执行，因此不再强制切换，让当前所选模型对所有任务生效。
}

// 取当前编辑器文档：优先选区，其次全文
const getCurrentDocContext = () => {
  const editor = editorStore.editorInstance
  const result = { selection: '', full: '' }
  try {
    const model = editor?.getModel?.()
    if (model) {
      result.full = String(model.getValue() || '')
      const sel = editor?.getSelection?.()
      if (sel && typeof model.getValueInRange === 'function') {
        result.selection = String(model.getValueInRange(sel) || '')
      }
    }
  } catch (err) {
    logger.warn('[assistant] read editor doc failed', err)
  }
  if (!result.full) {
    result.full = String(editorStore.currentFile?.content || '')
  }
  return result
}

// 取 Agent 任务的输入文本：选区 > 全文
const resolveAgentDocText = () => {
  const ctx = getCurrentDocContext()
  return (ctx.selection && ctx.selection.trim()) ? ctx.selection : ctx.full
}

// 构造散文类 Agent 任务（write/translate/organize）的提示词
const buildProseAgentPrompt = (taskName, instruction) => {
  const docText = resolveAgentDocText().trim()
  const userText = String(instruction || '').trim()
  if (taskName === 'translate') {
    const body = userText || docText
    return [
      '你是专业翻译助手。请将下面的内容翻译为目标语言；若用户未指明目标语言，则中文译为英文、其它语言译为中文。',
      '只输出译文本身，保留原有 Markdown 结构，不要添加解释。',
      docText && userText ? `\n[用户补充要求]\n${userText}` : '',
      `\n[待翻译内容]\n${body}`,
    ].filter(Boolean).join('\n')
  }
  if (taskName === 'organize') {
    return [
      '你是知识整理助手。请把下面的内容整理为结构清晰的 Markdown 笔记：提炼要点、合理分节、必要时用列表与表格。',
      '保留关键信息，不要编造内容。',
      userText ? `\n[整理要求]\n${userText}` : '',
      `\n[原始内容]\n${docText}`,
    ].filter(Boolean).join('\n')
  }
  // write
  return [
    '你是文档编写助手。请根据下面的要求撰写或续写 Markdown 文档内容，语言简洁专业。',
    `\n[写作要求]\n${userText}`,
    docText ? `\n[当前文档参考]\n${docText}` : '',
  ].filter(Boolean).join('\n')
}

const handleWebSearchEnabledChange = (value) => {
  rightSiderStore.setAssistantWebSearchEnabled(value)
}

const handleInputKeydown = (event) => {
  if (isComposing.value || event.isComposing || event.keyCode === 229) return
  // @ 引用弹层打开时，优先消费方向键 / Enter / Tab / Esc，避免触发历史与发送
  if (mentionPanel.value.open) {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      moveMentionActive(1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      moveMentionActive(-1)
      return
    }
    if (event.key === 'PageDown') {
      event.preventDefault()
      moveMentionActive(getMentionPageStep())
      return
    }
    if (event.key === 'PageUp') {
      event.preventDefault()
      moveMentionActive(-getMentionPageStep())
      return
    }
    if (event.key === 'Enter' || event.key === 'Tab') {
      const item = mentionPanel.value.items[mentionPanel.value.activeIndex]
      if (item) {
        event.preventDefault()
        selectMention(item)
        return
      }
      closeMentionPanel()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      closeMentionPanel()
      return
    }
  }
  if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && !event.shiftKey) {
    const inputEl = composerInputRef.value
    const hasSelectionApi = inputEl && typeof inputEl.selectionStart === 'number' && typeof inputEl.selectionEnd === 'number'
    if (hasSelectionApi) {
      const value = (inputEl.value ?? '').toString()
      const isEmpty = value.length === 0
      const atStart = isEmpty || (inputEl.selectionStart === 0 && inputEl.selectionEnd === 0)
      const atEnd = isEmpty || (inputEl.selectionStart === value.length && inputEl.selectionEnd === value.length)
      if (event.key === 'ArrowUp' && atStart) {
        event.preventDefault()
        moveHistory(-1)
        return
      }
      if (event.key === 'ArrowDown' && atEnd) {
        event.preventDefault()
        moveHistory(1)
        return
      }
    }
  }
  if (event.key !== 'Enter') return
  if (event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) return
  event.preventDefault()
  handleSendButtonClick()
}

const handleCompositionStart = () => {
  isComposing.value = true
}

const handleCompositionEnd = () => {
  isComposing.value = false
  // 中文输入法提交后再检测一次 @ 触发
  handleComposerInput()
}

const focusComposerInput = () => {
  const el = composerInputRef.value
  if (el && typeof el.focus === 'function') {
    el.focus()
  }
}

const formatAttachmentSize = (size) => {
  const n = Number(size)
  if (!Number.isFinite(n) || n <= 0) return ''
  if (n < 1024) return `${n}B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)}KB`
  return `${(n / (1024 * 1024)).toFixed(1)}MB`
}

const parseAttachmentName = (path) => {
  const p = (path ?? '').toString()
  if (!p) return ''
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
  return i >= 0 ? p.slice(i + 1) : p
}

const pickAttachments = async () => {
  try {
    const picked = await open({ multiple: true, directory: false })
    const paths = Array.isArray(picked) ? picked : (typeof picked === 'string' ? [picked] : [])
    if (!paths.length) return

    const exists = new Set(pendingAttachments.value.map((x) => x.path))
    for (const p of paths) {
      const path = (p ?? '').toString()
      if (!path || exists.has(path)) continue
      exists.add(path)
      pendingAttachments.value.push({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        path,
        name: parseAttachmentName(path),
        size: 0,
      })
    }
  } catch (err) {
    logger.error('[assistant] pick attachments failed', err)
    message.error('选择附件失败')
  }
}

const removeAttachment = (id) => {
  pendingAttachments.value = pendingAttachments.value.filter((x) => x.id !== id)
}

// ───── @ 引用：模糊搜索 / 选择 / 注入 ─────

// 取最后一段路径，用于在弹层里区分同名跨库文件
const lastPathSegment = (p) => {
  const s = String(p || '').replace(/[\\/]+$/, '')
  const i = Math.max(s.lastIndexOf('/'), s.lastIndexOf('\\'))
  return i >= 0 ? s.slice(i + 1) : s
}

// 弹层里每个候选展示的“位置”：相对路径目录 + 所属库名，帮助区分同名文件
const formatMentionLocation = (item) => {
  const rel = String(item?.relative || '')
  const dir = rel.includes('/') ? rel.slice(0, rel.lastIndexOf('/')) : ''
  const repo = lastPathSegment(item?.repo)
  return [repo, dir].filter(Boolean).join(' · ') || repo
}

const getMentionFileIcon = (item) => {
  const name = String(item?.name || item?.path || '').toLowerCase()
  const itemType = String(item?.itemType || item?.type || item?.kind || '').toLowerCase()
  if (item?.isDirectory || itemType === 'directory' || itemType === 'dir') return folderIcon
  if (/\.(png|jpe?g|gif|webp|svg|bmp|ico)$/.test(name)) return imageIcon
  if (/\.(md|markdown|txt|json|ya?ml|toml|rs|js|ts|vue|css|html|py|go|java|c|cpp|h|hpp)$/.test(name)) return fileTextIcon
  return fileIcon
}

const cloneComposerContextItems = (items, kind) => {
  return (Array.isArray(items) ? items : [])
    .map((item) => ({
      id: String(item?.id || `${kind}-${item?.path || item?.name || ''}`),
      kind,
      name: String(item?.name || lastPathSegment(item?.path) || '未命名引用'),
      path: String(item?.path || ''),
      repo: String(item?.repo || ''),
      relative: String(item?.relative || ''),
      size: Number(item?.size || 0),
      itemType: String(item?.itemType || item?.type || (item?.isDirectory ? 'directory' : 'file')),
      isDirectory: Boolean(item?.isDirectory),
    }))
    .filter((item) => item.path)
}

const getMessageContextItems = (chatMessage) => {
  const context = chatMessage?.meta?.composerContext || {}
  return [
    ...cloneComposerContextItems(context.mentions, 'mention'),
    ...cloneComposerContextItems(context.attachments, 'attachment'),
  ]
}

const getMessageContextIcon = (item) => getMentionFileIcon(item)

const closeMentionPanel = () => {
  if (mentionSearchTimer.value) {
    clearTimeout(mentionSearchTimer.value)
    mentionSearchTimer.value = null
  }
  mentionPanel.value = { open: false, query: '', items: [], activeIndex: 0, loading: false }
  mentionAnchor.value = -1
}

// 检测光标前是否处于一个 @ 触发 token（@ 前是行首或空白，@ 后无空白）
const detectMentionTrigger = () => {
  const el = composerInputRef.value
  if (!el || typeof el.selectionStart !== 'number') return null
  if (el.selectionStart !== el.selectionEnd) return null
  const value = String(el.value ?? '')
  const caret = el.selectionStart
  const before = value.slice(0, caret)
  const match = before.match(/(^|\s)@([^\s@]*)$/)
  if (!match) return null
  const query = match[2] || ''
  const anchor = caret - query.length - 1 // 指向 @ 字符
  return { query, anchor }
}

const runMentionSearch = (query) => {
  const roots = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist.filter(Boolean) : []
  if (!roots.length) {
    mentionPanel.value.items = []
    mentionPanel.value.loading = false
    return
  }
  const seq = (mentionSearchSeq.value += 1)
  mentionPanel.value.loading = true
  invoke('fuzzy_search_files', { query, roots, limit: 40 })
    .then((items) => {
      if (seq !== mentionSearchSeq.value || !mentionPanel.value.open) return
      mentionPanel.value.items = Array.isArray(items) ? items : []
      mentionPanel.value.activeIndex = 0
      mentionPanel.value.loading = false
      nextTick(() => {
        if (mentionListRef.value) mentionListRef.value.scrollTop = 0
      })
      logger.info('[assistant] mention search finished', {
        query,
        count: mentionPanel.value.items.length,
      })
    })
    .catch((err) => {
      if (seq !== mentionSearchSeq.value || !mentionPanel.value.open) return
      logger.warn('[assistant] fuzzy_search_files failed', err)
      mentionPanel.value.items = []
      mentionPanel.value.loading = false
    })
}

// textarea 内容变化时检测 @ 触发并驱动弹层
const handleComposerInput = () => {
  if (isComposing.value) return
  const trigger = detectMentionTrigger()
  if (!trigger) {
    if (mentionPanel.value.open) closeMentionPanel()
    return
  }
  mentionAnchor.value = trigger.anchor
  mentionPanel.value.open = true
  mentionPanel.value.query = trigger.query
  if (mentionSearchTimer.value) clearTimeout(mentionSearchTimer.value)
  mentionSearchTimer.value = setTimeout(() => {
    mentionSearchTimer.value = null
    runMentionSearch(trigger.query)
  }, 120)
}

const moveMentionActive = (delta) => {
  const n = mentionPanel.value.items.length
  if (!n) return
  const next = Math.min(n - 1, Math.max(0, mentionPanel.value.activeIndex + delta))
  mentionPanel.value.activeIndex = next
  scrollMentionActiveIntoView()
  logger.debug('[assistant] mention active moved', { activeIndex: next, delta })
}

const getMentionPageStep = () => {
  const list = mentionListRef.value
  if (!list) return 6
  return Math.max(1, Math.floor(list.clientHeight / 34) - 1)
}

const scrollMentionActiveIntoView = () => {
  nextTick(() => {
    const list = mentionListRef.value
    if (!list) return
    const active = list.querySelector('.mention-item--active')
    if (active && typeof active.scrollIntoView === 'function') {
      active.scrollIntoView({ block: 'nearest' })
    }
  })
}

// 选中候选：文件已在输入框顶部渲染为 chip，因此只移除 textarea 里的 @query token，避免重复显示。
const selectMention = (item) => {
  if (!item) return
  const el = composerInputRef.value
  const path = String(item.path || '')
  if (!mentionedFiles.value.some((f) => f.path === path)) {
    mentionedFiles.value.push({
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      name: String(item.name || lastPathSegment(path)),
      path,
      repo: String(item.repo || ''),
    })
  }

  const value = String(question.value ?? '')
  const anchor = mentionAnchor.value
  const caret = el && typeof el.selectionStart === 'number' ? el.selectionStart : value.length
  if (anchor >= 0 && anchor <= value.length) {
    let before = value.slice(0, anchor)
    let after = value.slice(caret)
    if (!before) {
      after = after.replace(/^\s+/, '')
    } else if (!after) {
      before = before
    } else if (/\s$/.test(before) && /^\s/.test(after)) {
      after = after.replace(/^\s+/, '')
    } else if (!/\s$/.test(before) && !/^\s/.test(after)) {
      after = ` ${after}`
    }
    question.value = before + after
    nextTick(() => {
      const node = composerInputRef.value
      if (node && typeof node.setSelectionRange === 'function') {
        const pos = Math.min(anchor, question.value.length)
        node.focus()
        node.setSelectionRange(pos, pos)
      }
    })
  }
  logger.info('[assistant] mention selected', {
    name: item.name,
    path,
    mentionedCount: mentionedFiles.value.length,
    inputLength: question.value.length,
  })
  closeMentionPanel()
}

const removeMention = (id) => {
  const target = mentionedFiles.value.find((x) => x.id === id)
  mentionedFiles.value = mentionedFiles.value.filter((x) => x.id !== id)
  logger.info('[assistant] mention removed', {
    name: target?.name || '',
    path: target?.path || '',
    mentionedCount: mentionedFiles.value.length,
  })
}

// 把被引用文件按当前任务模式注入到要发送的文本中：
// - Agent 模式：仅追加绝对路径，由 Agent 用 read_file 工具自行读取（含跨库）。
// - 其它文本模式：直接读出文件内容拼进 prompt（每文件截断）。
const MENTION_CONTENT_MAX_CHARS = 8000
const appendReferencesByMode = async (text, taskName, referencedFiles = mentionedFiles.value) => {
  const files = Array.isArray(referencedFiles) ? referencedFiles : []
  if (!files.length) return text
  const base = String(text ?? '')

  if (taskName === 'agent') {
    const list = files.map((f, idx) => `${idx + 1}. ${f.name} (${f.path})`).join('\n')
    return `${base}\n\n[引用文件]\n${list}\n[/引用文件]`
  }

  const blocks = []
  for (const f of files) {
    try {
      const raw = await invoke('read_file_content', { path: f.path })
      let content = String(raw ?? '')
      if (content.length > MENTION_CONTENT_MAX_CHARS) {
        content = `${content.slice(0, MENTION_CONTENT_MAX_CHARS)}\n…（内容已截断）`
      }
      blocks.push(`[引用文件: ${f.name}]（${f.path}）\n${content}`)
    } catch (err) {
      logger.warn('[assistant] read mentioned file failed', { path: f.path, err })
      message.warning(`读取引用文件失败：${f.name}`)
      blocks.push(`[引用文件: ${f.name}]（${f.path}）\n（读取失败）`)
    }
  }
  return `${base}\n\n${blocks.join('\n\n')}`
}

const buildQuestionWithAttachments = (text, attachmentFiles = pendingAttachments.value) => {
  const base = (text ?? '').toString().trim()
  const files = Array.isArray(attachmentFiles) ? attachmentFiles : []
  if (!files.length) return base
  const list = files.map((f, idx) => `${idx + 1}. ${f.name} (${f.path})`).join('\n')
  return `${base}\n\n[附件]\n${list}\n[/附件]`
}

// 构造发给模型的多轮对话历史：从已有消息里取最近的 user/assistant 轮次，
// 受最大条数与累计字符限制，避免 prompt 过长拖慢本地模型；与后端 cache_prompt 协同复用 KV 前缀。
const MAX_HISTORY_MESSAGES = 12
const MAX_HISTORY_CHARS = 6000
const buildConversationHistory = () => {
  const all = Array.isArray(messages.value) ? messages.value : []
  const picked = []
  let total = 0
  for (let i = all.length - 1; i >= 0; i -= 1) {
    const m = all[i]
    const role = m?.role
    if (role !== 'user' && role !== 'assistant') continue
    const content = String(m?.content || '').trim()
    if (!content) continue
    if (picked.length >= MAX_HISTORY_MESSAGES) break
    if (picked.length > 0 && total + content.length > MAX_HISTORY_CHARS) break
    picked.push({ role, content })
    total += content.length
  }
  return picked.reverse()
}

const buildWebContextPrompt = (rawQuestion, webItems) => {
  if (!Array.isArray(webItems) || webItems.length === 0) {
    return rawQuestion
  }

  // 控制拼入 prompt 的联网上下文体量：最多取前 5 条，每条摘要截断到 200 字，
  // 避免 prompt 过长拖慢本地模型 prefill。参考来源卡片展示不受影响（走 webReferences 链路）。
  const MAX_WEB_CONTEXT_ITEMS = 5
  const MAX_WEB_SNIPPET_CHARS = 200
  const lines = webItems.slice(0, MAX_WEB_CONTEXT_ITEMS).map((item, idx) => {
    const title = String(item?.title || '').trim()
    let snippet = String(item?.snippet || '').trim()
    if (snippet.length > MAX_WEB_SNIPPET_CHARS) {
      snippet = `${snippet.slice(0, MAX_WEB_SNIPPET_CHARS)}…`
    }
    const link = String(item?.url || '').trim()
    return `[${idx + 1}] ${title}\n${snippet}\n来源: ${link}`
  })

  return `${rawQuestion}\n\n请结合以下联网结果回答，并在回答结尾补充“参考来源”小节：\n${lines.join('\n\n')}`
}

const buildWebSearchPlannerPrompt = (rawQuestion) => {
  const question = String(rawQuestion || '').trim()
  return [
    '你是联网检索规划器。',
    '请先理解用户问题，再生成最少且必要的联网检索需求。',
    '仅输出 JSON，禁止输出额外文本。',
    'JSON 格式：{"need_search":boolean,"queries":string[],"reason":string}',
    '约束：',
    '1) queries 最多 4 条，去重，单条 6~24 字符',
    '2) 每条 query 必须完整表达一个检索意图，不能只取零碎关键词',
    '3) 若问题不需要联网，need_search=false 且 queries=[]',
    '4) query 必须保留用户问题中的核心对象与限定词，禁止扩展到无关领域',
    '',
    `用户问题：${question}`,
  ].join('\n')
}

const extractJsonObject = (raw) => {
  const text = String(raw || '').trim()
  if (!text) return ''
  const fenced = text.match(/```(?:json)?\s*([\s\S]*?)```/i)
  if (fenced?.[1]) return fenced[1].trim()
  const first = text.indexOf('{')
  const last = text.lastIndexOf('}')
  if (first >= 0 && last > first) {
    return text.slice(first, last + 1).trim()
  }
  return ''
}

const parseWebSearchPlan = (rawAnswer) => {
  const jsonText = extractJsonObject(rawAnswer)
  if (!jsonText) return null
  try {
    const parsed = JSON.parse(jsonText)
    const needSearch = Boolean(parsed?.need_search)
    const queries = Array.isArray(parsed?.queries)
      ? parsed.queries
        .map((x) => String(x || '').trim())
        .filter((x) => x.length >= 2 && x.length <= 64)
      : []
    const reason = String(parsed?.reason || '').trim()
    return {
      needSearch,
      queries: Array.from(new Set(queries)).slice(0, 4),
      reason,
    }
  } catch (err) {
    logger.warn('[assistant] parse web search plan failed', err)
    return null
  }
}

const extractCoreQueryTokens = (text) => {
  const source = String(text || '').toLowerCase()
  if (!source) return []
  const tokens = new Set()
  const cjkChunks = source.match(/[\u4e00-\u9fff]{2,}/g) || []
  for (const chunk of cjkChunks) {
    if (chunk.length >= 3) tokens.add(chunk)
    for (let i = 0; i < chunk.length - 2; i += 1) {
      tokens.add(chunk.slice(i, i + 3))
    }
  }
  const asciiChunks = source.match(/[a-z0-9][a-z0-9._:+-]{2,}/g) || []
  for (const chunk of asciiChunks) {
    if (chunk.length >= 4) tokens.add(chunk)
  }
  return Array.from(tokens)
}

const queryHasCoreOverlap = (query, coreTokens) => {
  const q = String(query || '').toLowerCase()
  if (!q) return false
  if (!Array.isArray(coreTokens) || !coreTokens.length) return true
  return coreTokens.some((token) => token.length >= 3 && q.includes(token))
}

const sanitizePlannedQueries = (queries, rawQuestion) => {
  const list = Array.isArray(queries)
    ? queries.map((x) => String(x || '').trim()).filter((x) => x.length > 0)
    : []
  const deduped = Array.from(new Set(list)).slice(0, 4)
  const coreTokens = extractCoreQueryTokens(rawQuestion)
  const filtered = deduped.filter((q) => queryHasCoreOverlap(q, coreTokens))
  if (filtered.length > 0) return filtered
  const fallback = normalizeWebSearchQuery(rawQuestion)
  return fallback ? [fallback] : []
}

const buildWebSearchPlanWithAi = async (rawQuestion, selectedModelId) => {
  const modelForPlanning = WEB_MODEL_IDS.has(Number(selectedModelId)) ? 5 : Number(selectedModelId)
  const plannerRequest = {
    modelid: modelForPlanning,
    question: buildWebSearchPlannerPrompt(rawQuestion),
    topicid: 0,
    sessionid: `assistant-web-plan-${Date.now()}`,
  }
  try {
    const res = await invoke('get_chat_answer', { request: plannerRequest })
    const answer = String(res?.answer || '')
    const parsed = parseWebSearchPlan(answer)
    if (parsed) {
      return parsed
    }
  } catch (err) {
    logger.warn('[assistant] build web search plan with ai failed', err)
  }
  const fallback = normalizeWebSearchQuery(rawQuestion)
  return {
    needSearch: Boolean(fallback),
    queries: fallback ? [fallback] : [],
    reason: 'fallback',
  }
}

const mergeWebSearchItems = (items) => {
  if (!Array.isArray(items)) return []
  const seen = new Set()
  const merged = []
  for (const item of items) {
    const url = String(item?.url || '').trim()
    if (!url || seen.has(url)) continue
    seen.add(url)
    merged.push(item)
    if (merged.length >= 8) break
  }
  return merged
}

const normalizeWebReferences = (webItems) => {
  if (!Array.isArray(webItems)) return []
  return webItems
    .map((item) => ({
      title: String(item?.title || '').trim(),
      snippet: String(item?.snippet || '').trim(),
      url: String(item?.url || '').trim(),
    }))
    .filter((item) => item.url.length > 0)
}

const buildWebRelevanceTokens = (text) => {
  const source = String(text || '').toLowerCase()
  if (!source) return []
  const strongTokens = new Set()
  const weakTokens = new Set()
  const cjkChunks = source.match(/[\u4e00-\u9fff]{2,}/g) || []
  for (const chunk of cjkChunks) {
    if (chunk.length >= 3) strongTokens.add(chunk)
    for (let i = 0; i < chunk.length - 2; i += 1) {
      strongTokens.add(chunk.slice(i, i + 3))
    }
    for (let i = 0; i < chunk.length - 1; i += 1) {
      weakTokens.add(chunk.slice(i, i + 2))
    }
  }
  const asciiChunks = source.match(/[a-z0-9][a-z0-9._:+-]{2,}/g) || []
  for (const chunk of asciiChunks) {
    if (chunk.length >= 4) strongTokens.add(chunk)
    else weakTokens.add(chunk)
  }
  const compact = source.replace(/[^a-z0-9\u4e00-\u9fff]/g, '')
  if (compact.length >= 4) strongTokens.add(compact)
  return {
    strong: Array.from(strongTokens),
    weak: Array.from(weakTokens).filter((x) => !strongTokens.has(x)),
  }
}

const scoreWebItemRelevance = (item, tokens, rawQuestion) => {
  const hay = `${String(item?.title || '')} ${String(item?.snippet || '')} ${String(item?.url || '')}`.toLowerCase()
  const compactQuestion = String(rawQuestion || '').toLowerCase().replace(/[^a-z0-9\u4e00-\u9fff]/g, '')
  let score = 0
  let strongHits = 0
  let weakHits = 0
  if (compactQuestion.length >= 3 && hay.includes(compactQuestion)) {
    score += 12
    strongHits += 1
  }
  for (const token of (tokens?.strong || [])) {
    if (token.length < 2) continue
    if (hay.includes(token)) {
      score += /[\u4e00-\u9fff]/.test(token) ? 3 : 2
      strongHits += 1
    }
  }
  for (const token of (tokens?.weak || [])) {
    if (token.length < 2) continue
    if (hay.includes(token)) {
      score += /[\u4e00-\u9fff]/.test(token) ? 1 : 1
      weakHits += 1
    }
  }
  return { score, strongHits, weakHits }
}

const filterRelevantWebItems = (items, rawQuestion, queries) => {
  if (!Array.isArray(items) || !items.length) return []
  const tokenSource = [String(rawQuestion || ''), ...(Array.isArray(queries) ? queries : [])].join(' ')
  const tokens = buildWebRelevanceTokens(tokenSource)
  if (!(tokens.strong.length || tokens.weak.length)) return items
  const scored = items
    .map((item) => ({ item, metrics: scoreWebItemRelevance(item, tokens, rawQuestion) }))
    .filter((x) => x.metrics.strongHits >= 1 || x.metrics.weakHits >= 3)
    .sort((a, b) => b.metrics.score - a.metrics.score)
  if (!scored.length) {
    return []
  }
  return scored.map((x) => x.item)
}

const getHostFromUrl = (url) => {
  try {
    const host = new URL(String(url || '')).hostname || ''
    return host.replace(/^www\./, '')
  } catch {
    return ''
  }
}

const referenceFaviconPalette = ['#38bdf8', '#22c55e', '#f59e0b', '#a78bfa', '#fb7185', '#14b8a6']

const hashReferenceHost = (host) => {
  let hash = 0
  for (let i = 0; i < host.length; i += 1) {
    hash = (hash * 31 + host.charCodeAt(i)) >>> 0
  }
  return hash
}

const getFaviconUrl = (url) => {
  const host = getHostFromUrl(url)
  if (!host) return ''
  const hash = hashReferenceHost(host)
  const bg = referenceFaviconPalette[hash % referenceFaviconPalette.length]
  const label = (host.match(/[A-Za-z0-9\u4e00-\u9fa5]/)?.[0] || 'G').toUpperCase()
  // 设计说明：这里使用本地生成的 data URL 作为引用来源图标，避免请求 Google/Gstatic favicon 服务。
  // 这样既减少第三方隐私拦截警告，也让离线或弱网场景下的引用卡片保持稳定显示。
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><rect width="64" height="64" rx="16" fill="${bg}"/><text x="32" y="39" text-anchor="middle" font-family="Inter, Arial, sans-serif" font-size="30" font-weight="700" fill="#0f172a">${label}</text></svg>`
  return `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(svg)}`
}

const normalizeWebSearchQuery = (rawQuestion) => {
  const raw = String(rawQuestion || '').trim()
  if (!raw) return ''
  const compact = raw
    .replace(/[“”"‘’'`]/g, ' ')
    .replace(/[，。！？；：,.!?;:()[\]{}<>《》]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  const engTokens = compact.match(/[A-Za-z0-9._:+-]{2,}/g) || []
  const cjk = compact
    .replace(/^(给我|请问|请|帮我|我想|麻烦|能否|可以|告诉我)+/g, '')
    .replace(/(怎么|如何|一下|一下子|的使用方法|使用方法|教程)$/g, '')
    .trim()
  const parts = []
  if (engTokens.length) {
    parts.push(engTokens.join(' '))
  }
  if (cjk) {
    parts.push(cjk)
  }
  const optimized = parts.join(' ').trim()
  return optimized || compact
}

const queryWebSearch = async (rawQuestion, selectedModelId, shouldSearch = true) => {
  if (!shouldSearch) {
    return { items: [], queries: [], reason: '', plannedByAi: false }
  }
  const plan = await buildWebSearchPlanWithAi(rawQuestion, selectedModelId)
  const plannedQueries = Array.isArray(plan?.queries) ? plan.queries : []
  const queries = sanitizePlannedQueries(plannedQueries, rawQuestion).slice(0, 4)
  const needSearch = Boolean(plan?.needSearch) && queries.length > 0
  logger.info('[assistant] web search plan prepared', {
    rawQuestion,
    queries,
    reason: plan?.reason || '',
    needSearch,
  })
  if (!needSearch) {
    return {
      items: [],
      queries,
      reason: String(plan?.reason || ''),
      plannedByAi: true,
    }
  }

  try {
    const settled = await Promise.all(queries.map((query) => invoke('search_web_duckduckgo', { query, limit: 4 })))
    const all = settled.flatMap((items) => (Array.isArray(items) ? items : []))
    const mergedItems = mergeWebSearchItems(all)
    const filteredItems = filterRelevantWebItems(mergedItems, rawQuestion, queries)
    logger.info('[assistant] web search relevance filtered', {
      before: mergedItems.length,
      after: filteredItems.length,
    })
    return {
      items: filteredItems,
      queries,
      reason: String(plan?.reason || ''),
      plannedByAi: true,
    }
  } catch (err) {
    logger.warn('[assistant] web search failed', err)
    message.warning('联网搜索失败，已自动使用离线问答')
    return {
      items: [],
      queries,
      reason: String(plan?.reason || ''),
      plannedByAi: true,
    }
  }
}

const renderAssistantMarkdown = async (content) => {
  try {
    return await renderMarkdown(content, {
      mermaid: false,
      includeNotes: false,
      // 流式阶段启用数学公式稳定模式，避免复杂公式只渲染一半。
      deferMathUntilComplete: true,
      // 把 `\Phi_E`、`I_{enc}` 这类“写在反引号里的数学符号”恢复成行内数学。
      smartInlineMathFromCode: true,
    })
  } catch (err) {
    logger.warn('[assistant] render markdown failed', err)
    return content
  }
}

const ensureStreamingAssistantMessage = () => {
  const id = String(currentStreamingMessageId.value || '')
  if (id) return id
  const createdId = rightSiderStore.addAssistantMessage({
    role: 'assistant',
    content: '',
    html: '',
    meta: {},
  })
  currentStreamingMessageId.value = String(createdId || '')
  return currentStreamingMessageId.value
}

const appendWebSearchMessage = (references, queries = [], plannedByAi = false) => {
  if (!webSearchEnabled.value) return
  const refs = Array.isArray(references) ? references : []
  const queryList = Array.isArray(queries) ? queries : []
  const content = refs.length
    ? (plannedByAi
      ? `AI已规划 ${queryList.length} 个检索需求，检索到 ${refs.length} 条联网结果`
      : `已检索到 ${refs.length} 条联网结果`)
    : (plannedByAi
      ? `AI已规划 ${queryList.length} 个检索需求，未检索到可用结果`
      : '联网搜索已启用')
  rightSiderStore.addAssistantMessage({
    role: 'web',
    content,
    meta: {
      webSearchTriggered: true,
      webContextCount: refs.length,
      webReferences: refs,
      webQueries: queryList,
      webPlannedByAi: plannedByAi,
    },
  })
}

const scheduleStreamingRender = () => {
  if (streamRenderTimer.value) {
    clearTimeout(streamRenderTimer.value)
  }
  streamRenderTimer.value = setTimeout(() => {
    streamRenderTimer.value = null
    const messageId = ensureStreamingAssistantMessage()
    if (!messageId) return
    const raw = String(currentStreamText.value || '')
    logger.info('[assistant] streaming text tick', {
      messageId,
      textLength: raw.length,
    })
    // 流式期间仅做纯文本增量更新（不渲染 Markdown、不触网），
    // 由 finishAssistantStreaming 在结束时做一次完整 Markdown 渲染。
    rightSiderStore.updateAssistantMessage(messageId, {
      content: raw,
      streaming: true,
    })
    maybeScrollToBottom('streaming-render')
  }, 60)
}

const pushInputHistory = (text) => {
  const value = String(text || '').trim()
  if (!value) return
  const list = inputHistory.value
  const last = list[list.length - 1]
  if (last === value) {
    historyIndex.value = list.length
    return
  }
  list.push(value)
  if (list.length > 60) {
    list.splice(0, list.length - 60)
  }
  historyIndex.value = list.length
}

const moveHistory = (delta) => {
  if (!inputHistory.value.length) return
  if (historyIndex.value === -1) {
    historyIndex.value = inputHistory.value.length
  }
  const next = Math.min(inputHistory.value.length, Math.max(0, historyIndex.value + delta))
  historyIndex.value = next
  question.value = next >= inputHistory.value.length ? '' : (inputHistory.value[next] || '')
  requestAnimationFrame(() => {
    const el = composerInputRef.value
    if (!el) return
    const len = (el.value ?? '').length
    el.selectionStart = len
    el.selectionEnd = len
  })
}

const finishAssistantStreaming = async ({ fallbackText = '', keepPending = false } = {}) => {
  if (streamRenderTimer.value) {
    clearTimeout(streamRenderTimer.value)
    streamRenderTimer.value = null
  }

  const messageId = ensureStreamingAssistantMessage()
  const text = String(currentStreamText.value || '').trim() || String(fallbackText || '').trim()
  if (messageId) {
    const finalText = text || '模型未返回内容。'
    const html = await renderMarkdown(finalText, {
      mermaid: false,
      includeNotes: false,
      smartInlineMathFromCode: true,
    })
    logger.info('[assistant] finish streaming render', {
      messageId,
      textLength: finalText.length,
      keepPending,
    })
    rightSiderStore.updateAssistantMessage(messageId, {
      content: finalText,
      html,
      streaming: false,
    })
  }

  if (!keepPending) {
    rightSiderStore.setAssistantPending(false)
  }
}

const resolveWebProvider = (id) => {
  if (Number(id) === 7) return 'yuanbao'
  if (Number(id) === 8) return 'chatgpt'
  return ''
}

const updateAssistantMessageMeta = (metaPatch) => {
  const messageId = ensureStreamingAssistantMessage()
  if (!messageId) return
  const list = rightSiderStore.assistant.messages
  const target = Array.isArray(list) ? list.find((x) => x.id === messageId) : null
  if (!target) return
  rightSiderStore.updateAssistantMessage(messageId, {
    meta: {
      ...(target.meta || {}),
      ...(metaPatch || {}),
    },
  })
}

const resolveAgentConfirm = async (approved) => {
  const c = agentConfirm.value
  if (!c) return
  let args = null
  if (approved) {
    args = { path: c.path, content: c.content }
    if (c.isWriteFile) args.mode = c.mode
  }
  agentConfirm.value = null
  try {
    await invoke('agent_tool_decision', { request: { call_id: c.callId, approved, args } })
  } catch (err) {
    logger.warn('[assistant] agent_tool_decision failed', err)
  }
}

const initAssistantEventListeners = async () => {
  if (typeof streamUnlisten.value !== 'function') {
    streamUnlisten.value = await listen(STREAM_EVENT, (event) => {
      if (!pending.value) return
      if (WEB_MODEL_IDS.has(Number(currentRequestModelId.value))) return
      const chunk = String(event?.payload || '')
      if (!chunk) return
      currentStreamText.value += chunk
      scheduleStreamingRender()
    })
  }

  if (typeof webPushUnlisten.value !== 'function') {
    webPushUnlisten.value = await listen(WEB_PUSH_EVENT, async (event) => {
      if (!pending.value) return
      if (!WEB_MODEL_IDS.has(Number(currentRequestModelId.value))) return

      const payload = event?.payload || {}
      const model = Number(payload?.model_id)
      if (model !== Number(currentRequestModelId.value)) return

      const incomingSessionId = String(payload?.session_id || '').trim()
      const currentSessionId = String(currentRequestSessionId.value || '').trim()
      if (currentSessionId && incomingSessionId && currentSessionId !== incomingSessionId) return

      if (incomingSessionId) {
        currentRequestSessionId.value = incomingSessionId
        rightSiderStore.setAssistantSessionId(incomingSessionId)
      }

      const status = String(payload?.status || '').trim().toLowerCase()
      const webMessages = Array.isArray(payload?.messages) ? payload.messages : []
      const lastAssistant = [...webMessages]
        .reverse()
        .find((x) => String(x?.role || '').toLowerCase() !== 'user' && String(x?.content || '').trim())

      let hasAssistantContent = false
      if (lastAssistant?.content) {
        hasAssistantContent = true
        currentStreamText.value = String(lastAssistant.content)
        await finishAssistantStreaming({ keepPending: status !== 'ready' })
      }

      if (status === 'need_login') {
        updateAssistantMessageMeta({ webLoginRequired: true })
        message.warning('请先在弹出的网页窗口完成登录，登录后会继续回复')
      }

      if (status === 'ready' && !hasAssistantContent) {
        await finishAssistantStreaming({ fallbackText: '网页模型已就绪，但未返回有效内容。' })
      }
    })
  }

  // Agent 工具调用进度：创建可折叠的步骤卡片，running 时更新为 done/error
  if (typeof agentStepUnlisten.value !== 'function') {
    agentStepUnlisten.value = await listen('agent-step', (event) => {
      if (!pending.value) return
      const p = event?.payload || {}
      logger.info('[assistant] agent step event received', {
        type: p.type,
        name: p.name,
        tool: p.tool,
        status: p.status,
      })

      // 兼容新旧格式：新格式有 type/name，旧格式有 tool
      const evType = p.type || ''
      const toolName = p.name || p.tool || ''
      const toolLabel = AGENT_TOOL_LABELS[toolName] || toolName || '工具'
      const status = p.status || 'running'

      if (evType === 'error') {
        // 后端会把 SSE 断流、模型调用失败等内部错误统一转成 error 事件。
        // 单独建卡片可以保留上下文，也避免落入 invoke catch 后只显示一条泛化失败提示。
        const message = p.message || p.result || 'Agent 执行失败'
        logger.warn('[assistant] agent step error received', { toolName, message })
        rightSiderStore.addAssistantMessage({
          role: 'tool_step',
          content: `⚠️ ${toolLabel} 失败：${message}`,
          toolName,
          toolLabel,
          icon: toolName,
          status: 'error',
          result: message,
          collapsed: false,
          createdAt: Date.now(),
        })
        maybeScrollToBottom('agent-tool-error')
        return
      }

      if (evType === 'tool_result') {
        // tool_result: 更新之前 running 的卡片为 done/error
        const allMessages = rightSiderStore.assistant.messages
        for (let i = allMessages.length - 1; i >= 0; i--) {
          const m = allMessages[i]
          if (m.role === 'tool_step' && m.toolName === toolName && m.status === 'running') {
            rightSiderStore.updateAssistantMessage(m.id, {
              status: status === 'done' ? 'done' : 'error',
              result: p.result || m.result,
            })
            break
          }
        }
        return
      }

      if (evType === 'tool_call' || !evType) {
        // tool_call 或旧格式 agent-step：创建 running 卡片
        let args = {}
        try {
          if (typeof p.args === 'string') args = JSON.parse(p.args)
          else if (p.args && typeof p.args === 'object') args = p.args
        } catch {}

        rightSiderStore.addAssistantMessage({
          role: 'tool_step',
          content: '',
          toolName,
          toolLabel,
          icon: toolName,
          status,
          args,
          result: p.result || '',
          collapsed: true,
          createdAt: Date.now(),
        })
        maybeScrollToBottom('agent-tool-call')
        return
      }

      // 兜底：简单状态文本
      let text = ''
      if (status === 'running') text = `🔧 正在调用：${toolLabel}`
      else if (status === 'done') text = `✅ 完成：${toolLabel}`
      else if (status === 'error') text = `⚠️ ${toolLabel} 失败：${p.message || ''}`
      if (text) {
        rightSiderStore.addAssistantMessage({ role: 'tool_step', content: text, toolName, toolLabel, status, createdAt: Date.now() })
        maybeScrollToBottom('agent-tool-status')
      }
    })
  }

  // Agent 写操作确认：弹出内嵌确认卡片（可编辑 path/content/mode），结果回传后端。
  if (typeof agentConfirmUnlisten.value !== 'function') {
    agentConfirmUnlisten.value = await listen('agent-tool-confirm', (event) => {
      const p = event?.payload || {}
      logger.info('[assistant] agent confirm event received', {
        type: p.type,
        name: p.name,
        tool: p.tool,
        hasCallId: Boolean(p.call_id),
      })
      const callId = String(p.call_id || '')
      if (!callId) return
      const args = p.args || {}
      const toolName = p.tool || ''
      agentConfirm.value = {
        callId,
        tool: p.tool || '',
        toolLabel: AGENT_TOOL_LABELS[p.tool] || p.tool || '写操作',
        path: String(args.path || ''),
        content: typeof args.content === 'string' ? args.content : '',
        mode: String(args.mode || 'overwrite'),
        hasContent: typeof args.content === 'string',
        isWriteFile: p.tool === 'write_file',
      }
      logger.info('[assistant] agent confirm card shown', { callId, toolName })
    })
  }
}

const stopQuestion = async () => {
  if (!pending.value) return
  try {
    await invoke('cancel_chat_stream')
  } catch (err) {
    logger.warn('[assistant] cancel stream invoke failed', err)
  }
  // 停止时若有待确认的写操作卡片，一并清理（后端 cancel 已清理 pending）。
  agentConfirm.value = null
  await finishAssistantStreaming({ fallbackText: '已停止本次回答。' })
  message.info('已停止本次回答')
}

const handleSendButtonClick = () => {
  if (pending.value) {
    stopQuestion()
    return
  }
  sendQuestion()
}

const writeClipboard = async (text) => {
  const value = (text ?? '').toString()
  if (!value) return false

  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value)
      return true
    } catch {}
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (err) {
    logger.error('[assistant] write clipboard failed', err)
    return false
  }
}

const copyAssistantMessage = async (chatMessage) => {
  const text = String(chatMessage?.content || '').trim()
  if (!text) return
  const ok = await writeClipboard(text)
  if (ok) {
    message.success('已复制回复内容')
  } else {
    message.error('复制失败')
  }
}

const insertAssistantToCurrentFile = async (chatMessage) => {
  const text = String(chatMessage?.content || '').trim()
  if (!text) {
    message.warning('无可插入内容')
    return
  }

  const editor = editorStore.editorInstance
  if (!editor) {
    message.warning('当前没有可编辑的文件')
    return
  }

  editorStore.insertContent(text)
  message.success('已插入到当前文件')
}

const createAssistantAsNewFile = async (chatMessage) => {
  const text = String(chatMessage?.content || '').trim()
  if (!text) {
    message.warning('无可保存内容')
    return
  }

  const stamp = new Date().toISOString().replace(/[:.]/g, '-')
  const baseDir = String(fileTreeStore.activeRepoPath || '')
  const defaultName = `assistant-${stamp}.md`
  const defaultPath = baseDir ? `${baseDir}/${defaultName}` : defaultName

  try {
    const path = await save({
      defaultPath,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    })
    if (!path) return

    await invoke('create_file_with_content', { path: String(path), content: text })
    if (baseDir) {
      await fileTreeStore.fetchFileTree(baseDir, { force: true })
    }
    await fileTreeStore.openFileInTab(String(path), text)
    editorStore.setFile({
      path: String(path),
      content: text,
      highlightInfo: { lineNumber: 1, matches: [] },
    })
    message.success('已创建新文件')
  } catch (err) {
    logger.error('[assistant] create file from response failed', err)
    message.error('创建新文件失败')
  }
}

const sendQuestion = async () => {
  const typedQuestion = question.value.trim()
  const sentMentions = cloneComposerContextItems(mentionedFiles.value, 'mention')
  const sentAttachments = cloneComposerContextItems(pendingAttachments.value, 'attachment')
  const hasMentions = sentMentions.length > 0
  const hasAttachments = sentAttachments.length > 0
  const rawQuestion = typedQuestion ||
    (hasMentions ? '请基于已引用的文件回答。' : (hasAttachments ? '请基于附件回答。' : ''))
  if (!rawQuestion || pending.value) return

  const selectedModelId = Number(modelId.value)
  const isWebModel = WEB_MODEL_IDS.has(selectedModelId)
  const currentTask = task.value
  const isStructuredTask = currentTask === 'classify' || currentTask === 'tag'
  const isProseAgentTask = currentTask === 'write' || currentTask === 'translate' || currentTask === 'organize'
  const sessionId = rightSiderStore.assistant.sessionId || `${Date.now()}-${Math.random().toString(16).slice(2)}`
  const shouldUseWebSearch = webSearchEnabled.value && !disableWebSearchNextSend.value && currentTask === 'chat'
  disableWebSearchNextSend.value = false

  // 对话与 Agent 任务带多轮上下文（散文/结构化任务是对当前文档的一次性操作，保持纯净）。
  // 必须在 push 当前 user 消息之前读取，确保历史不含本次问题。
  const conversationHistory = (currentTask === 'chat' || currentTask === 'agent') ? buildConversationHistory() : []

  currentRequestModelId.value = selectedModelId
  currentRequestSessionId.value = sessionId
  currentStreamText.value = ''
  currentStreamingMessageId.value = ''
  if (typedQuestion) {
    pushInputHistory(typedQuestion)
  }

  question.value = ''
  pendingAttachments.value = []
  mentionedFiles.value = []
  closeMentionPanel()
  rightSiderStore.setAssistantPending(true)
  logger.info('[assistant] send question prepared', {
    task: currentTask,
    modelId: selectedModelId,
    questionLength: rawQuestion.length,
    typedLength: typedQuestion.length,
    attachmentCount: sentAttachments.length,
    mentionCount: sentMentions.length,
  })
  rightSiderStore.addAssistantMessage({
    role: 'user',
    content: rawQuestion,
    meta: {
      composerContext: {
        mentions: sentMentions,
        attachments: sentAttachments,
      },
    },
  })
  scrollToBottom({ reason: 'send-question' })

  try {
    if (Number(modelId.value) === 6) {
      await invoke('ensure_local_deepseek_service')
    }
    if (Number(modelId.value) === 9) {
      await invoke('ensure_local_qwen_service')
    }

    // 结构化 Agent 任务（分类 / 打标签）：走非流式 doc_agent_process
    if (isStructuredTask) {
      const docText = (resolveAgentDocText().trim() || rawQuestion)
      const res = await invoke('doc_agent_process', {
        request: {
          modelid: selectedModelId,
          task: currentTask,
          doc_text: docText,
          sessionid: sessionId,
        },
      })
      const result = res?.result || {}
      let md = ''
      if (currentTask === 'tag') {
        const tags = Array.isArray(result.tags) ? result.tags : []
        md = `**标签**：${tags.length ? tags.map((t) => `\`${t}\``).join(' ') : '（无）'}`
        if (result.reason) md += `\n\n**依据**：${result.reason}`
      } else {
        const conf = typeof result.confidence === 'number' ? `（置信度 ${result.confidence.toFixed(2)}）` : ''
        md = `**分类**：${result.category || '（未识别）'} ${conf}`
        if (result.reason) md += `\n\n**理由**：${result.reason}`
      }
      const html = await renderAssistantMarkdown(md)
      rightSiderStore.addAssistantMessage({ role: 'assistant', content: md, html })
      rightSiderStore.setAssistantSessionId(res?.sessionid || sessionId)
      pendingAttachments.value = []
      mentionedFiles.value = []
      return
    }

    // Agent 任务：模型可调用工具（检索知识库 / 读文件 / 写文件需确认），多轮自动执行。
    if (currentTask === 'agent') {
      const agentQuestion = await appendReferencesByMode(
        buildQuestionWithAttachments(rawQuestion, sentAttachments),
        'agent',
        sentMentions,
      )
      await invoke('agent_chat_stream', {
        request: {
          modelid: selectedModelId,
          question: agentQuestion,
          topicid: 0,
          sessionid: sessionId,
          history: conversationHistory,
          kb_name: '',
          repo_path: String(fileTreeStore.activeRepoPath || ''),
          repo_roots: Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist.filter(Boolean) : [],
        },
      })
      rightSiderStore.setAssistantSessionId(sessionId)
      pendingAttachments.value = []
      mentionedFiles.value = []
      await finishAssistantStreaming()
      return
    }

    const searchOutcome = await queryWebSearch(rawQuestion, selectedModelId, shouldUseWebSearch)
    const webItems = Array.isArray(searchOutcome?.items) ? searchOutcome.items : []
    const withAttachments = buildQuestionWithAttachments(rawQuestion, sentAttachments)
    // 散文类 Agent 任务（编写 / 翻译 / 整理）：用预设提示词替换原问句，并跳过联网
    const baseQuestion = isProseAgentTask
      ? buildProseAgentPrompt(currentTask, rawQuestion)
      : buildWebContextPrompt(withAttachments, webItems)
    // 非 Agent 文本模式：把被引用的知识库文件内容直接拼进 prompt
    const mergedQuestion = await appendReferencesByMode(baseQuestion, currentTask, sentMentions)
    const webReferences = normalizeWebReferences(webItems)
    if (shouldUseWebSearch) {
      appendWebSearchMessage(
        webReferences,
        Array.isArray(searchOutcome?.queries) ? searchOutcome.queries : [],
        Boolean(searchOutcome?.plannedByAi),
      )
    }

    if (isWebModel) {
      const provider = resolveWebProvider(selectedModelId)
      if (provider) {
        await invoke('prepare_web_model_window', { provider })
      }
    }

    await invoke('get_chat_answer_stream', {
      request: {
        modelid: selectedModelId,
        question: mergedQuestion,
        topicid: 0,
        sessionid: sessionId,
        history: conversationHistory,
      }
    })
    rightSiderStore.setAssistantSessionId(sessionId)
    pendingAttachments.value = []
    mentionedFiles.value = []
    if (!isWebModel) {
      await finishAssistantStreaming()
    }
  } catch (err) {
    const text = String(err?.message || err || '请求失败')
    logger.error('[assistant] send question failed', err)
    message.error(text)
    await finishAssistantStreaming({ fallbackText: `请求失败：${text}` })
  } finally {
    if (!isWebModel) {
      rightSiderStore.setAssistantPending(false)
    }
  }
}
</script>

<style scoped>
.assistant-root {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  background:
    radial-gradient(1200px 300px at 20% -40%, rgba(33, 189, 236, 0.12), transparent 60%),
    radial-gradient(1000px 280px at 90% -45%, rgba(152, 95, 255, 0.12), transparent 60%),
    var(--guaxi-panel-bg);
}

.assistant-topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
  backdrop-filter: blur(12px);
}

.assistant-title-group {
  display: flex;
  align-items: center;
  gap: 10px;
}

.assistant-orb {
  width: 22px;
  height: 22px;
  border-radius: 999px;
  background: linear-gradient(135deg, #21bdec, #8b5cf6);
  box-shadow: 0 0 18px rgba(97, 120, 255, 0.45);
  animation: orb-breathe 2.2s ease-in-out infinite;
}

.assistant-title-wrap {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
  gap: 3px;
}

.assistant-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.assistant-subtitle {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.assistant-topbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.assistant-messages {
  flex: 1 1 0;
  min-height: 0;
  overflow: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--guaxi-text-secondary) 34%, transparent) transparent;
}

.assistant-messages::-webkit-scrollbar,
.mention-list::-webkit-scrollbar,
.agent-tool-card-pre::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.assistant-messages::-webkit-scrollbar-thumb,
.mention-list::-webkit-scrollbar-thumb,
.agent-tool-card-pre::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-text-secondary) 28%, transparent);
}

.assistant-empty {
  margin: auto;
  text-align: center;
  color: var(--guaxi-text-secondary);
}

.assistant-empty-icon {
  width: 44px;
  height: 44px;
  margin: 0 auto 10px auto;
  border-radius: 12px;
  display: grid;
  place-items: center;
  color: #7d61ff;
  background: linear-gradient(145deg, rgba(125, 97, 255, 0.15), rgba(33, 189, 236, 0.15));
}

.assistant-empty-title {
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 4px;
}

.assistant-empty-desc {
  font-size: 12px;
}

.assistant-message {
  display: flex;
  gap: 10px;
}

.assistant-message-avatar {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  border-radius: 999px;
  display: grid;
  place-items: center;
  font-size: 12px;
  font-weight: 700;
  color: #fff;
}

.assistant-message-user .assistant-message-avatar {
  background: linear-gradient(135deg, #21bdec, #1492d8);
}

.assistant-message-ai .assistant-message-avatar {
  background: linear-gradient(135deg, #8b5cf6, #5b7cfa);
}

.assistant-message-body {
  flex: 1 1 auto;
  min-width: 0;
}

.assistant-message-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 5px;
}

.assistant-message-content {
  border-radius: 8px;
  padding: 10px 12px;
  line-height: 1.6;
  /* 设计说明：消息正文是智能助手的主要阅读区域，默认字号略大于辅助信息，提升长回答的可读性。 */
  font-size: 14px;
  border: 1px solid var(--guaxi-border);
  overflow: hidden;
}

.assistant-jump-latest {
  position: absolute;
  right: 16px;
  bottom: 112px;
  z-index: 25;
  width: 32px;
  height: 32px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, #4f7dff 44%, var(--guaxi-border));
  background: color-mix(in srgb, var(--guaxi-panel-bg) 88%, #4f7dff 12%);
  color: var(--guaxi-text-primary);
  display: grid;
  place-items: center;
  cursor: pointer;
  box-shadow: 0 8px 22px rgba(0, 0, 0, 0.18);
  transition: transform 0.16s ease, background 0.16s ease, border-color 0.16s ease;
}

.assistant-jump-latest:hover {
  transform: translateY(-1px);
  border-color: color-mix(in srgb, #4f7dff 70%, var(--guaxi-border));
  background: color-mix(in srgb, var(--guaxi-panel-bg) 78%, #4f7dff 22%);
}

.assistant-message-user .assistant-message-content {
  background: linear-gradient(135deg, rgba(33, 189, 236, 0.12), rgba(33, 189, 236, 0.05));
}

.assistant-message-ai .assistant-message-content {
  background: linear-gradient(135deg, rgba(125, 97, 255, 0.1), rgba(125, 97, 255, 0.04));
}

.assistant-message-plain {
  white-space: pre-wrap;
  word-break: break-word;
}

.assistant-message-context-list {
  margin-top: 6px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.assistant-message-context-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  max-width: 100%;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 4px 7px;
  background: color-mix(in srgb, var(--guaxi-header-bg) 76%, transparent);
  color: var(--guaxi-text-primary);
}

.assistant-message-context-chip--mention {
  border-color: color-mix(in srgb, #4f7dff 38%, var(--guaxi-border));
  background: color-mix(in srgb, #4f7dff 10%, var(--guaxi-header-bg));
}

.assistant-message-context-kind {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  color: #4f7dff;
}

.assistant-message-context-icon {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
}

.assistant-message-context-name {
  min-width: 0;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  line-height: 1.2;
}

.assistant-message-content.markdown-body :deep(a) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
}

:global([data-guaxi-theme="dark"]) .assistant-message-content.markdown-body :deep(a) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.assistant-web-toggle-wrap {
  margin-top: 6px;
}

.assistant-web-toggle-btn {
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  border-radius: 8px;
  padding: 3px 8px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.16s ease;
}

.assistant-web-toggle-btn:hover {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-header-bg) 86%, transparent);
}

.assistant-message-source {
  margin-top: 6px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.assistant-reference-list {
  margin-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.assistant-reference-title {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.assistant-reference-card {
  display: block;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-header-bg) 80%, transparent);
  padding: 8px 10px;
  color: inherit;
  text-decoration: none;
  transition: all 0.16s ease;
}

.assistant-reference-card:hover {
  border-color: color-mix(in srgb, #4f7dff 45%, var(--guaxi-border));
  background: color-mix(in srgb, #4f7dff 12%, var(--guaxi-header-bg));
}

.assistant-reference-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.assistant-reference-favicon {
  width: 14px;
  height: 14px;
  border-radius: 4px;
  flex: 0 0 auto;
}

.assistant-reference-host {
  min-width: 0;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.assistant-reference-index {
  margin-left: auto;
  font-size: 10px;
  color: var(--guaxi-text-secondary);
}

.assistant-reference-link-title {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  font-weight: 600;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.assistant-reference-snippet {
  margin-top: 5px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  line-height: 1.45;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.assistant-message-actions {
  margin-top: 6px;
  display: flex;
  gap: 8px;
  opacity: 0;
  pointer-events: none;
  transform: translateY(2px);
  transition: all 0.16s ease;
}

.assistant-message-ai:hover .assistant-message-actions,
.assistant-message-user:hover .assistant-message-actions {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}

.assistant-action-btn {
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  border-radius: 8px;
  width: 26px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.16s ease;
}

.assistant-action-btn:hover {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-header-bg) 86%, transparent);
}

.assistant-thinking-wrap {
  display: flex;
  justify-content: flex-start;
}

.assistant-thinking-card {
  position: relative;
  min-width: 180px;
  border-radius: 14px;
  padding: 12px 14px;
  background: rgba(123, 97, 255, 0.08);
  border: 1px solid rgba(123, 97, 255, 0.2);
  overflow: hidden;
}

.assistant-thinking-ring {
  position: absolute;
  width: 180px;
  height: 180px;
  right: -70px;
  top: -110px;
  border-radius: 999px;
  border: 2px solid rgba(125, 97, 255, 0.25);
  animation: ring-spin 5s linear infinite;
}

.assistant-thinking-text {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.assistant-thinking-dots {
  margin-top: 8px;
  display: flex;
  gap: 6px;
}

.assistant-thinking-dots span {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: linear-gradient(135deg, #21bdec, #8b5cf6);
  animation: dot-wave 1.1s ease-in-out infinite;
}

.assistant-thinking-dots span:nth-child(2) {
  animation-delay: 0.12s;
}

.assistant-thinking-dots span:nth-child(3) {
  animation-delay: 0.24s;
}

.agent-confirm-card {
  margin: 8px 12px;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid rgba(245, 158, 11, 0.4);
  background:
    radial-gradient(600px 120px at 10% -30%, rgba(245, 158, 11, 0.12), transparent 60%),
    var(--guaxi-panel-bg);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.agent-confirm-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.agent-confirm-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(245, 158, 11, 0.18);
  color: rgb(217, 119, 6);
  border: 1px solid rgba(245, 158, 11, 0.36);
}

.agent-confirm-title {
  font-weight: 600;
  font-size: 13px;
}

.agent-confirm-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.agent-confirm-label {
  font-size: 12px;
  opacity: 0.72;
}

.agent-confirm-input,
.agent-confirm-textarea {
  width: 100%;
  box-sizing: border-box;
  padding: 6px 8px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
  color: inherit;
  font-family: inherit;
  font-size: 12px;
  resize: vertical;
}

.agent-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 4px;
}

.agent-confirm-btn {
  padding: 6px 14px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: inherit;
  cursor: pointer;
  font-size: 12px;
}

.agent-confirm-reject:hover {
  border-color: rgba(239, 68, 68, 0.5);
  color: rgb(239, 68, 68);
}

.agent-confirm-approve {
  background: rgba(16, 185, 129, 0.16);
  border-color: rgba(16, 185, 129, 0.4);
  color: rgb(16, 185, 129);
}

.agent-confirm-approve:hover {
  background: rgba(16, 185, 129, 0.26);
}

.assistant-footer {
  border-top: 1px solid var(--guaxi-border);
  padding: 10px 12px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 84%, transparent);
  backdrop-filter: blur(10px);
}

.composer-shell {
  position: relative;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, #ffffff 10%);
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.06);
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  cursor: text;
}

/* @ 引用：模糊搜索弹层，悬浮在输入框外壳上方 */
.mention-panel {
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 8px);
  max-height: 300px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 96%, var(--guaxi-header-bg) 4%);
  box-shadow: 0 18px 36px rgba(0, 0, 0, 0.20);
  z-index: 30;
}

.mention-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px 10px;
  border-bottom: 1px solid var(--guaxi-border);
}

.mention-panel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.mention-panel-hint {
  font-size: 10px;
  color: var(--guaxi-text-secondary);
}

.mention-empty {
  padding: 14px 12px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  text-align: center;
}

.mention-list {
  flex: 1 1 auto;
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow: auto;
  min-height: 0;
  max-height: 246px;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 34px;
  padding: 6px 8px;
  border-radius: 8px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}

.mention-item:hover {
  background: color-mix(in srgb, var(--guaxi-header-bg) 82%, transparent);
}

.mention-item--active {
  background: color-mix(in srgb, #4f7dff 15%, var(--guaxi-panel-bg));
  border-color: color-mix(in srgb, #4f7dff 34%, var(--guaxi-border));
}

.mention-item-icon {
  flex: 0 0 auto;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  display: grid;
  place-items: center;
  color: #4f7dff;
  background: color-mix(in srgb, #4f7dff 12%, transparent);
}

.mention-item-name {
  flex: 0 1 auto;
  min-width: 0;
  font-size: 12px;
  color: var(--guaxi-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mention-item-path {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: right;
}

.mention-chip {
  background: color-mix(in srgb, #4f7dff 11%, var(--guaxi-header-bg));
  border-color: color-mix(in srgb, #4f7dff 40%, var(--guaxi-border));
}

.mention-chip-at {
  color: #4f7dff;
  display: inline-flex;
  align-items: center;
}

.attachment-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.attachment-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  padding: 4px 7px;
  max-width: 100%;
  background: color-mix(in srgb, var(--guaxi-header-bg) 80%, transparent);
}

.attachment-icon {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
}

.attachment-name {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  color: var(--guaxi-text-primary);
}

.attachment-size {
  font-size: 10px;
  color: var(--guaxi-text-secondary);
}

.attachment-remove {
  border: 1px solid transparent;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  line-height: 0;
  padding: 0;
  width: 18px;
  height: 18px;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.attachment-remove:hover {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-header-bg) 86%, transparent);
  border-color: var(--guaxi-border);
}

.composer-input {
  width: 100%;
  min-height: 72px;
  max-height: 200px;
  resize: none;
  border: none;
  outline: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-size: 13px;
  line-height: 1.6;
  font-family: inherit;
}

.composer-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  /* 设计说明：面板被压窄时允许换行，避免下拉框/发送按钮溢出输入框外壳。 */
  flex-wrap: wrap;
}

/* 设计说明：两个下拉框由定宽改为可收缩弹性项（min-width:0 允许低于内容宽度收缩），
   n-select 内部默认占满容器，因此窄宽下会一起收缩/换行而不撑破父容器。 */
.composer-model-select {
  flex: 2 1 120px;
  min-width: 0;
}

.composer-task-select {
  /* 设计说明：基准从 88px 加大到 104px，保证「文档编写」「知识整理」等 4 字标签
     在触发器里完整可见；配合 :consistent-menu-width="false" 让弹出菜单按内容展开。 */
  flex: 1 1 104px;
  min-width: 0;
}

.composer-task-select :deep(.n-base-selection) {
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 86%, transparent);
}

.composer-model-select :deep(.n-base-selection) {
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 86%, transparent);
}

.composer-icon-btn {
  flex: 0 0 auto;
  border: 1px solid transparent;
  background: transparent;
  color: var(--guaxi-text-secondary);
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  cursor: pointer;
  transition: all 0.15s ease;
}

.composer-icon-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--guaxi-header-bg) 86%, transparent);
  color: var(--guaxi-text-primary);
  border-color: var(--guaxi-border);
}

.composer-icon-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.composer-spacer {
  flex: 1 1 0;
  min-width: 0;
}

.send-btn {
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  border-radius: 10px;
  border: none;
  color: #fff;
  background: linear-gradient(135deg, #20c0ef 0%, #4f7dff 45%, #8d5eff 100%);
  box-shadow: 0 6px 16px rgba(79, 125, 255, 0.35);
  display: grid;
  place-items: center;
  cursor: pointer;
  transition: transform 0.16s ease, box-shadow 0.16s ease, opacity 0.16s ease;
}

.send-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 8px 20px rgba(79, 125, 255, 0.45);
}

.send-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.send-btn svg {
  transform: rotate(90deg);
}

.markdown-body :deep(pre) {
  overflow: auto;
}

@keyframes dot-wave {
  0%, 100% {
    transform: translateY(0);
    opacity: 0.45;
  }
  50% {
    transform: translateY(-4px);
    opacity: 1;
  }
}

@keyframes ring-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes orb-breathe {
  0%, 100% {
    transform: scale(1);
    box-shadow: 0 0 16px rgba(97, 120, 255, 0.35);
  }
  50% {
    transform: scale(1.08);
    box-shadow: 0 0 24px rgba(97, 120, 255, 0.55);
  }
}

/* ====== Agent 工具调用步骤卡片 ====== */
.assistant-message-tool { opacity: 0.92; }

.agent-tool-card {
  margin: 4px 0;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 94%, var(--guaxi-header-bg) 6%);
  overflow: hidden;
  transition: border-color 0.2s, background 0.2s;
}

.agent-tool-card--running {
  border-color: color-mix(in srgb, #3b82f6 60%, var(--guaxi-border));
  background: color-mix(in srgb, var(--guaxi-panel-bg) 91%, #3b82f6 9%);
}

.agent-tool-card--done { border-color: color-mix(in srgb, #22c55e 42%, var(--guaxi-border)); }
.agent-tool-card--error { border-color: color-mix(in srgb, #ef4444 48%, var(--guaxi-border)); }

.agent-tool-card-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  cursor: pointer;
  user-select: none;
  font-size: 12px;
}

.agent-tool-card-head:hover {
  background: color-mix(in srgb, var(--guaxi-header-bg) 95%, transparent);
}

.agent-tool-card-icon {
  flex: 0 0 auto;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  display: grid;
  place-items: center;
  color: #4f7dff;
  background: color-mix(in srgb, #4f7dff 12%, transparent);
}
.agent-tool-card-label { flex: 1 1 auto; color: var(--guaxi-text-primary); font-weight: 500; }
.agent-tool-card-status {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  color: var(--guaxi-text-secondary);
}
.agent-tool-card--done .agent-tool-card-status { color: #22c55e; }
.agent-tool-card--error .agent-tool-card-status { color: #ef4444; }
.agent-tool-card-toggle {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  display: grid;
  place-items: center;
}

.agent-tool-card-toggle svg {
  transition: transform 0.14s ease;
}

.agent-tool-card-toggle--open {
  transform: rotate(90deg);
}

.agent-tool-card-body {
  padding: 0 10px 10px 42px;
}
.agent-tool-card-section { margin-top: 8px; }

.agent-tool-card-section-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--guaxi-text-secondary);
  margin-bottom: 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.agent-tool-card-pre {
  margin: 0;
  font-size: 11px;
  max-height: 200px;
  overflow: auto;
  background: color-mix(in srgb, var(--guaxi-content-bg) 80%, transparent);
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 76%, transparent);
  border-radius: 8px;
  padding: 8px 9px;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Cascadia Code', Consolas, monospace;
}

.agent-tool-card-error { color: #ef4444; }

.agent-tool-spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid color-mix(in srgb, var(--guaxi-border) 60%, transparent);
  border-top-color: #3b82f6;
  border-radius: 50%;
  animation: ring-spin 0.8s linear infinite;
}
</style>
