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

    <div ref="messagesRef" class="assistant-messages">
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
        :class="message.role === 'user' ? 'assistant-message-user' : 'assistant-message-ai'"
      >
        <div class="assistant-message-avatar">
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
            v-if="message.role === 'assistant'"
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
            v-if="message.role === 'web'"
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

    <div class="assistant-footer">
      <div class="composer-shell" @click="focusComposerInput">
        <div v-if="pendingAttachments.length" class="attachment-list">
          <div v-for="file in pendingAttachments" :key="file.id" class="attachment-chip">
            <span class="attachment-name" :title="file.path">{{ file.name }}</span>
            <span class="attachment-size">{{ formatAttachmentSize(file.size) }}</span>
            <button class="attachment-remove" type="button" title="移除附件" @click="removeAttachment(file.id)">
              ×
            </button>
          </div>
        </div>

        <textarea
          ref="composerInputRef"
          class="composer-input"
          placeholder="输入问题，Enter 发送，Shift + Enter 换行"
          v-model="question"
          :disabled="pending"
          @compositionstart="handleCompositionStart"
          @compositionend="handleCompositionEnd"
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
              class="composer-model-select"
              size="small"
              :value="modelId"
              :options="modelOptions"
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
const messagesRef = ref(null)
const composerInputRef = ref(null)
const question = ref('')
const pendingAttachments = ref([])
const streamUnlisten = ref(null)
const webPushUnlisten = ref(null)
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

const messages = computed(() => rightSiderStore.assistant.messages)
const pending = computed(() => rightSiderStore.assistant.pending)
const modelId = computed(() => rightSiderStore.assistant.modelId)
const webSearchEnabled = computed(() => rightSiderStore.assistant.webSearchEnabled)
const canSend = computed(() => !pending.value && question.value.trim().length > 0)
const canClickSend = computed(() => pending.value || question.value.trim().length > 0)
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

const scrollToBottom = () => {
  nextTick(() => {
    const el = messagesRef.value
    if (!el) return
    el.scrollTop = el.scrollHeight
  })
}

watch(messages, () => {
  scrollToBottom()
  enhanceRenderedMessages()
}, { deep: true })

watch(pending, () => {
  scrollToBottom()
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
  if (streamRenderTimer.value) {
    clearTimeout(streamRenderTimer.value)
    streamRenderTimer.value = null
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
}

const isWebMessageCollapsed = (messageId) => collapsedWebMessageIds.value.has(String(messageId || ''))

const toggleWebMessageCollapse = (messageId) => {
  const id = String(messageId || '')
  if (!id) return
  const next = new Set(collapsedWebMessageIds.value)
  if (next.has(id)) {
    next.delete(id)
  } else {
    next.add(id)
  }
  collapsedWebMessageIds.value = next
}

const handleModelChange = (value) => {
  rightSiderStore.setAssistantModelId(value)
}

const handleWebSearchEnabledChange = (value) => {
  rightSiderStore.setAssistantWebSearchEnabled(value)
}

const handleInputKeydown = (event) => {
  if (isComposing.value || event.isComposing || event.keyCode === 229) return
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

const buildQuestionWithAttachments = (text) => {
  const base = (text ?? '').toString().trim()
  const files = Array.isArray(pendingAttachments.value) ? pendingAttachments.value : []
  if (!files.length) return base
  const list = files.map((f, idx) => `${idx + 1}. ${f.name} (${f.path})`).join('\n')
  return `${base}\n\n[附件]\n${list}\n[/附件]`
}

const buildWebContextPrompt = (rawQuestion, webItems) => {
  if (!Array.isArray(webItems) || webItems.length === 0) {
    return rawQuestion
  }

  const lines = webItems.map((item, idx) => {
    const title = String(item?.title || '').trim()
    const snippet = String(item?.snippet || '').trim()
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
        .filter((x) => x.length > 0)
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

const getHostFromUrl = (url) => {
  try {
    const host = new URL(String(url || '')).hostname || ''
    return host.replace(/^www\./, '')
  } catch {
    return ''
  }
}

const getFaviconUrl = (url) => {
  const host = getHostFromUrl(url)
  if (!host) return ''
  return `https://www.google.com/s2/favicons?domain=${encodeURIComponent(host)}&sz=64`
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
  const queries = plannedQueries
    .map((x) => String(x || '').trim())
    .filter((x) => x.length > 0)
    .slice(0, 4)
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
    return {
      items: mergeWebSearchItems(all),
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
  streamRenderTimer.value = setTimeout(async () => {
    streamRenderTimer.value = null
    const messageId = ensureStreamingAssistantMessage()
    if (!messageId) return
    const raw = String(currentStreamText.value || '')
    const html = await renderAssistantMarkdown(raw)
    rightSiderStore.updateAssistantMessage(messageId, {
      content: raw,
      html,
    })
    scrollToBottom()
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
    const html = await renderAssistantMarkdown(finalText)
    rightSiderStore.updateAssistantMessage(messageId, {
      content: finalText,
      html,
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
}

const stopQuestion = async () => {
  if (!pending.value) return
  try {
    await invoke('cancel_chat_stream')
  } catch (err) {
    logger.warn('[assistant] cancel stream invoke failed', err)
  }
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
  const rawQuestion = question.value.trim()
  if (!rawQuestion || pending.value) return

  const selectedModelId = Number(modelId.value)
  const isWebModel = WEB_MODEL_IDS.has(selectedModelId)
  const sessionId = rightSiderStore.assistant.sessionId || `${Date.now()}-${Math.random().toString(16).slice(2)}`
  const shouldUseWebSearch = webSearchEnabled.value && !disableWebSearchNextSend.value
  disableWebSearchNextSend.value = false

  currentRequestModelId.value = selectedModelId
  currentRequestSessionId.value = sessionId
  currentStreamText.value = ''
  currentStreamingMessageId.value = ''
  pushInputHistory(rawQuestion)

  question.value = ''
  rightSiderStore.setAssistantPending(true)
  rightSiderStore.addAssistantMessage({
    role: 'user',
    content: rawQuestion,
  })

  try {
    if (Number(modelId.value) === 6) {
      await invoke('ensure_local_deepseek_service')
    }

    const searchOutcome = await queryWebSearch(rawQuestion, selectedModelId, shouldUseWebSearch)
    const webItems = Array.isArray(searchOutcome?.items) ? searchOutcome.items : []
    const withAttachments = buildQuestionWithAttachments(rawQuestion)
    const mergedQuestion = buildWebContextPrompt(withAttachments, webItems)
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
      }
    })
    rightSiderStore.setAssistantSessionId(sessionId)
    pendingAttachments.value = []
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
  border-radius: 12px;
  padding: 10px 12px;
  line-height: 1.6;
  font-size: 13px;
  border: 1px solid var(--guaxi-border);
  overflow: hidden;
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
  border: 1px solid var(--guaxi-border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, #ffffff 10%);
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.06);
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  cursor: text;
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
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  padding: 2px 8px;
  max-width: 100%;
  background: color-mix(in srgb, var(--guaxi-header-bg) 80%, transparent);
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
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  padding: 0;
}

.attachment-remove:hover {
  color: var(--guaxi-text-primary);
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
}

.composer-model-select {
  width: 172px;
}

.composer-model-select :deep(.n-base-selection) {
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 86%, transparent);
}

.composer-icon-btn {
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
  flex: 1 1 auto;
}

.send-btn {
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
</style>
