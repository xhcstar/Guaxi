<template>
  <div class="timeline">
    <div v-if="!renderedTurns.length" class="timeline-empty">
      <div class="empty-mark" aria-hidden="true">
        <span class="mark-ring"></span>
        <span class="mark-dot"></span>
      </div>
      <h2 class="empty-title">给这台机器一个任务</h2>
      <p class="empty-body">
        它会自己读文件、跑命令、查资料。每一步都摊开在这里，右侧能看见改动与环境。
        需要你决定的时候，会在输入框上方停下来问你。
      </p>
      <div class="empty-suggestions">
        <button
          v-for="s in suggestions"
          :key="s.title"
          type="button"
          class="suggestion"
          @click="$emit('suggest', s.prompt)"
        >
          <span class="suggestion-title">{{ s.title }}</span>
          <span class="suggestion-prompt">{{ s.prompt }}</span>
        </button>
      </div>
    </div>

    <section
      v-for="turn in renderedTurns"
      :key="turn.id"
      class="turn"
      :class="{ 'turn--failed': turn.status === 'failed' }"
    >
      <template v-for="block in turn.blocks" :key="block.key">
        <!-- 用户 -->
        <div v-if="block.kind === 'user'" class="user-row">
          <div class="user-bubble">
            <div v-for="(part, i) in block.parts" :key="i" class="user-part">
              <p v-if="part.type === 'text'" class="user-text">{{ part.text }}</p>
              <img
                v-else-if="part.type === 'localImage' || part.type === 'image'"
                class="user-image"
                :src="imageSrc(part)"
                :alt="part.name || '附件'"
                @click="$emit('open-image', imageSrc(part))"
              />
              <span v-else class="user-chip">
                {{ part.type === 'mention' ? '@' : part.type === 'skill' ? '技能 ' : '' }}{{ part.name || part.path || part.type }}
              </span>
            </div>
            <span v-if="block.item.steer" class="user-tag">转向</span>
          </div>
        </div>

        <!-- 助手正文 -->
        <article v-else-if="block.kind === 'agent'" class="agent-row">
          <AgentMarkdown
            :text="block.item.text"
            :streaming="Boolean(block.item.streaming && running)"
          />
          <div v-if="block.item.streaming && running" class="agent-caret" aria-hidden="true"></div>
          <div v-else class="agent-actions">
            <button type="button" class="ghost-btn" @click="copy(block.item.text)">
              <Icon :icon="copyIcon" width="13" height="13" />
              复制
            </button>
            <button
              v-if="block.item.id"
              type="button"
              class="ghost-btn"
              title="从这个回合分叉出新会话"
              @click="$emit('fork', turn.id)"
            >
              <Icon :icon="forkIcon" width="13" height="13" />
              分叉
            </button>
          </div>
        </article>

        <!-- 思考 -->
        <div v-else-if="block.kind === 'reasoning'" class="reasoning">
          <button type="button" class="reasoning-head" @click="toggle(block.key, defaultReasoningOpen(block))">
            <Icon :icon="brainIcon" width="14" height="14" class="reasoning-icon" />
            <span>{{ block.streaming && running ? '思考中…' : '思考过程' }}</span>
            <span v-if="!block.streaming && block.durationText" class="reasoning-time">{{ block.durationText }}</span>
            <span class="spacer"></span>
            <Icon :icon="chevronIcon" width="13" height="13" :class="{ rotated: isOpen(block.key, defaultReasoningOpen(block)) }" />
          </button>
          <div v-if="isOpen(block.key, defaultReasoningOpen(block))" class="reasoning-body">
            <AgentMarkdown
              v-for="(item, i) in block.items"
              :key="item.id || i"
              :text="item.summary.filter(Boolean).join('\n\n') || '（模型没有给出摘要）'"
              :streaming="Boolean(item.streaming && running)"
            />
            <details v-if="hasRawReasoning(block)" class="reasoning-raw">
              <summary>原始推理</summary>
              <p v-for="(item, i) in block.items" :key="i" class="raw-text">{{ item.content.filter(Boolean).join('\n') }}</p>
            </details>
          </div>
        </div>

        <!-- 命令组 -->
        <div v-else-if="block.kind === 'commands'" class="tool-group">
          <button
            type="button"
            class="group-head"
            @click="toggle(block.key, defaultCommandOpen(block))"
          >
            <Icon :icon="terminalIcon" width="14" height="14" class="group-icon" />
            <span>运行了 {{ block.items.length }} 条命令</span>
            <span v-if="block.failed" class="warn-text">{{ block.failed }} 条失败</span>
            <span v-if="block.running" class="running-dot"></span>
            <span class="spacer"></span>
            <Icon :icon="chevronIcon" width="13" height="13" :class="{ rotated: isOpen(block.key, defaultCommandOpen(block)) }" />
          </button>
          <div v-if="isOpen(block.key, defaultCommandOpen(block))" class="group-body">
            <div v-for="item in block.items" :key="item.id" class="cmd">
              <button type="button" class="cmd-head" @click="toggle(item.id, item.status === 'inProgress')">
                <span class="cmd-prompt">$</span>
                <code class="cmd-text">{{ item.command || '命令' }}</code>
                <span v-if="item.status === 'inProgress'" class="cmd-status run">运行中</span>
                <span v-else-if="item.status === 'failed'" class="cmd-status fail">失败 {{ item.exitCode ?? '' }}</span>
                <span v-else-if="item.durationMs" class="cmd-status">{{ formatDuration(item.durationMs) }}</span>
                <Icon :icon="chevronIcon" width="12" height="12" class="cmd-caret" :class="{ rotated: isOpen(item.id, item.status === 'inProgress') }" />
              </button>
              <div v-if="item.cwd" class="cmd-cwd">{{ item.cwd }}</div>
              <pre
                v-if="isOpen(item.id, item.status === 'inProgress')"
                class="cmd-output"
              >{{ item.output || (item.status === 'inProgress' ? '等待输出…' : '（无输出）') }}</pre>
            </div>
          </div>
        </div>

        <!-- 文件改动 -->
        <div v-else-if="block.kind === 'files'" class="tool-group">
          <button
            type="button"
            class="group-head"
            @click="toggle(block.key, true)"
          >
            <Icon :icon="fileDiffIcon" width="14" height="14" class="group-icon" />
            <span>修改了 {{ block.files.length }} 个文件</span>
            <span class="diff-add">+{{ block.added }}</span>
            <span class="diff-del">−{{ block.removed }}</span>
            <span class="spacer"></span>
            <Icon :icon="chevronIcon" width="13" height="13" :class="{ rotated: isOpen(block.key, true) }" />
          </button>
          <div v-if="isOpen(block.key, true)" class="group-body file-list">
            <button
              v-for="f in block.files"
              :key="f.path"
              type="button"
              class="file-row"
              @click="$emit('open-diff', f)"
            >
              <span class="file-kind">{{ kindLabel(f.kind) }}</span>
              <span class="file-path">{{ f.path }}</span>
              <span class="file-nums">
                <span v-if="f.added" class="diff-add">+{{ f.added }}</span>
                <span v-if="f.removed" class="diff-del">−{{ f.removed }}</span>
              </span>
            </button>
          </div>
        </div>

        <!-- MCP / 动态工具 -->
        <div v-else-if="block.kind === 'tools'" class="tool-group">
          <button
            type="button"
            class="group-head"
            @click="toggle(block.key, block.items.length <= 2)"
          >
            <Icon :icon="plugIcon" width="14" height="14" class="group-icon" />
            <span>调用了 {{ block.items.length }} 个工具</span>
            <span v-if="block.failed" class="warn-text">{{ block.failed }} 个失败</span>
            <span v-if="block.running" class="running-dot"></span>
            <span class="spacer"></span>
            <Icon :icon="chevronIcon" width="13" height="13" :class="{ rotated: isOpen(block.key, block.items.length <= 2) }" />
          </button>
          <div v-if="isOpen(block.key, block.items.length <= 2)" class="group-body">
            <div v-for="item in block.items" :key="item.id" class="tool-call">
              <button type="button" class="tool-head" @click="toggle(item.id, item.status === 'inProgress')">
                <span class="tool-name">{{ item.server ? `${item.server} · ` : '' }}{{ item.tool || '工具' }}</span>
                <span v-if="item.status === 'inProgress'" class="cmd-status run">运行中</span>
                <span v-else-if="item.status === 'failed'" class="cmd-status fail">失败</span>
                <span v-else-if="item.durationMs" class="cmd-status">{{ formatDuration(item.durationMs) }}</span>
                <Icon :icon="chevronIcon" width="12" height="12" class="cmd-caret" :class="{ rotated: isOpen(item.id, item.status === 'inProgress') }" />
              </button>
              <div v-if="isOpen(item.id, item.status === 'inProgress')" class="tool-detail">
                <pre class="tool-pre">{{ pretty(item.arguments) }}</pre>
                <pre v-if="item.progress" class="tool-pre tool-progress">{{ item.progress }}</pre>
                <div v-if="toolImages(item).length" class="tool-images">
                  <img
                    v-for="(src, i) in toolImages(item)"
                    :key="i"
                    :src="src"
                    alt="工具返回的图片"
                    @click="$emit('open-image', src)"
                  />
                </div>
                <pre v-if="toolResult(item)" class="tool-pre">{{ toolResult(item) }}</pre>
              </div>
            </div>
          </div>
        </div>

        <!-- 联网搜索 -->
        <div v-else-if="block.kind === 'search'" class="search-card">
          <div class="search-head">
            <Icon :icon="globeIcon" width="14" height="14" />
            <span>搜索</span>
            <span class="search-query">{{ block.item.query }}</span>
          </div>
          <div v-if="block.item.results.length" class="search-results">
            <a
              v-for="(r, i) in block.item.results"
              :key="i"
              class="search-result"
              :href="resultUrl(r)"
              target="_blank"
              rel="noreferrer"
            >
              <span class="result-title">{{ resultTitle(r) }}</span>
              <span v-if="resultSnippet(r)" class="result-snippet">{{ resultSnippet(r) }}</span>
              <span class="result-url">{{ resultUrl(r) }}</span>
            </a>
          </div>
          <p v-else class="search-empty">正在检索…</p>
        </div>

        <!-- 计划 -->
        <div v-else-if="block.kind === 'plan'" class="plan-card">
          <div class="plan-head">
            <Icon :icon="listIcon" width="14" height="14" />
            <span>计划</span>
          </div>
          <AgentMarkdown :text="block.item.text || ''" :streaming="Boolean(block.item.streaming && running)" />
        </div>

        <!-- 图片 -->
        <div v-else-if="block.kind === 'image'" class="image-view">
          <img :src="imageSrc(block.item)" :alt="block.item.path || '图片'" @click="$emit('open-image', imageSrc(block.item))" />
          <span class="image-caption">{{ block.item.path || '生成的图片' }}</span>
        </div>

        <!-- 上下文压缩 -->
        <div v-else-if="block.kind === 'compaction'" class="divider">
          <span>上下文已压缩</span>
        </div>

        <!-- 提示 / 系统消息 -->
        <div v-else-if="block.kind === 'notice'" class="notice">{{ block.item.text }}</div>

        <div v-else-if="block.kind === 'review'" class="divider">
          <span>{{ block.item.type === 'enteredReviewMode' ? '开始代码审查' : '代码审查结束' }}</span>
        </div>

        <div v-else class="raw-item">
          <span class="raw-type">{{ block.item.type }}</span>
          <pre>{{ pretty(block.item) }}</pre>
        </div>
      </template>

      <div v-if="turn.error?.message" class="turn-error">
        <Icon :icon="alertIcon" width="14" height="14" />
        <span>{{ turn.error.message }}</span>
      </div>
    </section>
  </div>
</template>

<script setup>
/**
 * 中间栏的转录。
 *
 * Codex 的正文不是「一问一答」，而是「一次 turn 里一串 item」：
 * 思考摘要、命令、文件改动、MCP 工具、联网搜索、计划，全都有先后和状态。
 * 这里按类型把 item 规整成块，**相邻的同类工具合组**——
 * 否则一次跑二十条命令会把正文撑得没法看。
 *
 * 三种东西绝不折叠：
 * - 用户自己说的话；
 * - 助手的最终回答；
 * - 失败的调用（折叠错误等于掩盖问题）。
 */
import { computed, reactive } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import chevronIcon from '@iconify-icons/lucide/chevron-right'
import brainIcon from '@iconify-icons/lucide/brain'
import terminalIcon from '@iconify-icons/lucide/terminal-square'
import fileDiffIcon from '@iconify-icons/lucide/file-diff'
import plugIcon from '@iconify-icons/lucide/plug'
import globeIcon from '@iconify-icons/lucide/globe'
import listIcon from '@iconify-icons/lucide/list-checks'
import copyIcon from '@iconify-icons/lucide/copy'
import forkIcon from '@iconify-icons/lucide/git-fork'
import alertIcon from '@iconify-icons/lucide/alert-triangle'
import AgentMarkdown from './AgentMarkdown.vue'

const props = defineProps({
  turns: { type: Array, default: () => [] },
  running: { type: Boolean, default: false },
})

defineEmits(['suggest', 'open-diff', 'open-image', 'fork'])

const suggestions = [
  {
    title: '读懂这个仓库',
    prompt: '梳理这个仓库的结构，指出最值得先读的 5 个文件，并说明它们为什么重要。',
  },
  {
    title: '修好失败的测试',
    prompt: '找出当前失败的测试，定位原因并修复；改完运行相关测试确认。',
  },
  {
    title: '查我自己的资料',
    prompt: '先调用 guaxi_recall 查我知识库里与这个项目相关的结论，再结合代码回答。',
  },
  {
    title: '整理这次改动',
    prompt: '检查当前未提交的改动，指出风险，给出一个清晰的提交信息；我确认后再提交。',
  },
]

const expanded = reactive({})

const toggle = (key, fallback = false) => {
  expanded[key] = !isOpen(key, fallback)
}

const isOpen = (key, fallback = false) => {
  const value = expanded[key]
  return typeof value === 'boolean' ? value : fallback
}

function countPatch(patch) {
  let added = 0
  let removed = 0
  for (const line of String(patch || '').split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) added += 1
    else if (line.startsWith('-') && !line.startsWith('---')) removed += 1
  }
  return { added, removed }
}

/** 把一条 turn 的 items 归并成渲染块。 */
function groupItems(items) {
  const blocks = []
  const push = (block) => blocks.push(block)
  let i = 0
  while (i < items.length) {
    const item = items[i]
    const type = item.type

    if (type === 'commandExecution') {
      const group = []
      while (i < items.length && items[i].type === 'commandExecution') group.push(items[i++])
      push({
        kind: 'commands',
        key: `cmd-${group[0].id}`,
        items: group,
        failed: group.filter((x) => x.status === 'failed').length,
        running: group.some((x) => x.status === 'inProgress'),
      })
      continue
    }

    if (type === 'fileChange') {
      const group = []
      while (i < items.length && items[i].type === 'fileChange') group.push(items[i++])
      const files = []
      for (const entry of group) {
        if (!Array.isArray(entry.changes)) continue
        for (const change of entry.changes) {
          if (!change?.path) continue
          const counts = countPatch(change.diff)
          files.push({
            path: String(change.path),
            kind: change.kind || 'update',
            added: counts.added,
            removed: counts.removed,
            patch: change.diff || '',
          })
        }
      }
      push({
        kind: 'files',
        key: `files-${group[0].id}`,
        items: group,
        files,
        added: files.reduce((n, f) => n + f.added, 0),
        removed: files.reduce((n, f) => n + f.removed, 0),
      })
      continue
    }

    if (type === 'mcpToolCall' || type === 'dynamicToolCall') {
      const group = []
      while (i < items.length && (items[i].type === 'mcpToolCall' || items[i].type === 'dynamicToolCall')) {
        group.push(items[i++])
      }
      push({
        kind: 'tools',
        key: `tools-${group[0].id}`,
        items: group,
        failed: group.filter((x) => x.status === 'failed').length,
        running: group.some((x) => x.status === 'inProgress'),
      })
      continue
    }

    if (type === 'reasoning') {
      const group = []
      while (i < items.length && items[i].type === 'reasoning') group.push(items[i++])
      const streaming = group.some((x) => x.streaming)
      const durationMs = group.reduce((n, x) => n + (x.completedAt && x.startedAt ? x.completedAt - x.startedAt : 0), 0)
      push({
        kind: 'reasoning',
        key: `reason-${group[0].id}`,
        items: group,
        streaming,
        durationText: durationMs > 1000 ? formatDuration(durationMs) : '',
      })
      continue
    }

    if (type === 'userMessage') {
      push({
        kind: 'user',
        key: item.id,
        item,
        parts: normalizeParts(item.content),
      })
      i += 1
      continue
    }

    if (type === 'agentMessage') {
      push({ kind: 'agent', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'webSearch') {
      push({ kind: 'search', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'plan') {
      push({ kind: 'plan', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'imageView' || type === 'imageGeneration') {
      push({ kind: 'image', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'contextCompaction') {
      push({ kind: 'compaction', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'notice') {
      push({ kind: 'notice', key: item.id, item })
      i += 1
      continue
    }

    if (type === 'enteredReviewMode' || type === 'exitedReviewMode') {
      push({ kind: 'review', key: item.id, item })
      i += 1
      continue
    }

    push({ kind: 'raw', key: item.id || `${type}-${i}`, item })
    i += 1
  }
  return blocks
}

const renderedTurns = computed(() =>
  (props.turns || []).map((turn) => ({
    ...turn,
    blocks: groupItems(turn.items || []),
  })),
)

function normalizeParts(content) {
  if (!Array.isArray(content)) return [{ type: 'text', text: '' }]
  return content.map((part) => ({ ...part }))
}

function hasRawReasoning(block) {
  return block.items.some((item) => Array.isArray(item.content) && item.content.some((x) => String(x).trim()))
}

function defaultReasoningOpen(block) {
  return Boolean(block.streaming) || block.items.some((item) => item.status === 'failed')
}

function defaultCommandOpen(block) {
  return block.running || block.failed > 0 || block.items.length <= 2
}

function formatDuration(ms) {
  const value = Number(ms || 0)
  if (!value) return ''
  if (value < 1000) return `${Math.round(value)}ms`
  if (value < 60_000) return `${(value / 1000).toFixed(1)}s`
  return `${Math.floor(value / 60_000)}m ${Math.round((value % 60_000) / 1000)}s`
}

function pretty(value) {
  if (value == null) return ''
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function toolResult(item) {
  if (!item) return ''
  if (item.error) {
    const message = item.error.message || item.error
    return typeof message === 'string' ? message : pretty(message)
  }
  if (item.contentItems?.length) {
    return item.contentItems
      // 图片项没有 text，`pretty()` 会把整段 base64 摊到卡片里。
      // 图片交给 `toolImages()` 单独渲染缩略图，这里只留一行说明。
      .map((c) => c?.text || c?.inputText || (c?.imageUrl ? '（截图见上方缩略图）' : pretty(c)))
      .join('\n')
  }
  if (item.result) {
    if (Array.isArray(item.result.content)) {
      return item.result.content
        .map((c) => c?.text || (c?.type === 'image' ? '（返回了一张图片）' : pretty(c)))
        .join('\n')
    }
    return pretty(item.result)
  }
  return ''
}

/**
 * 工具结果里可以渲染成缩略图的图片。
 *
 * 两个来源：app-server dynamicTool 的 `inputImage`（截屏走这条路），
 * 以及 MCP 回包里的 `{type:"image", data, mimeType}`。
 */
function toolImages(item) {
  if (!item) return []
  const out = []
  for (const c of item.contentItems || []) {
    const url = c?.imageUrl || c?.url
    if (url) out.push(url)
  }
  for (const c of item.result?.content || []) {
    if (c?.type === 'image' && c?.data) {
      out.push(`data:${c.mimeType || 'image/png'};base64,${c.data}`)
    }
  }
  return out
}

function resultTitle(result) {
  if (!result || typeof result !== 'object') return String(result || '结果')
  return result.title || result.name || result.url || result.link || '结果'
}

function resultUrl(result) {
  if (!result || typeof result !== 'object') return '#'
  return result.url || result.link || result.href || '#'
}

function resultSnippet(result) {
  if (!result || typeof result !== 'object') return ''
  return result.snippet || result.description || result.text || ''
}

function kindLabel(kind) {
  const key = String(kind || 'update')
  return { add: '新增', delete: '删除', update: '修改', move: '移动' }[key] || key
}

function imageSrc(item) {
  const path = item?.path || ''
  if (path) return /^https?:|^data:/.test(path) ? path : convertFileSrc(path)
  return item?.url || item?.imageUrl || ''
}

async function copy(text) {
  const value = String(text || '')
  if (!value) return
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(value)
  } catch {
    try {
      await navigator.clipboard.writeText(value)
    } catch {
      // 剪贴板不可用时保持安静：复制不是主流程。
    }
  }
}
</script>

<style scoped>
.timeline {
  display: flex;
  flex-direction: column;
  gap: 22px;
  padding-bottom: 12px;
}

/* ── 空态 ───────────────────────────── */

.timeline-empty {
  max-width: 620px;
  padding: 34px 4px 18px;
}

.empty-mark {
  position: relative;
  width: 40px;
  height: 40px;
  margin-bottom: 20px;
}

.mark-ring {
  position: absolute;
  inset: 4px;
  border: 1px solid var(--guaxi-border);
  border-radius: 50%;
}

.mark-dot {
  position: absolute;
  inset: 15px;
  border-radius: 50%;
  background: #b47d1a;
  box-shadow: 0 0 16px rgba(180, 125, 26, 0.55);
}

.empty-title {
  margin: 0 0 10px;
  font-size: 21px;
  font-weight: 500;
  letter-spacing: 0.01em;
  line-height: 1.4;
}

.empty-body {
  margin: 0 0 22px;
  max-width: 54ch;
  font-size: 13.5px;
  line-height: 1.8;
  color: var(--guaxi-text-secondary);
}

.empty-suggestions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.suggestion {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 11px 13px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: transparent;
  color: var(--guaxi-text-primary);
  text-align: left;
  cursor: pointer;
  transition: border-color 140ms ease, background 140ms ease, transform 140ms ease;
}

.suggestion:hover {
  border-color: rgba(180, 125, 26, 0.5);
  background: rgba(180, 125, 26, 0.05);
  transform: translateY(-1px);
}

.suggestion-title {
  font-size: 12.5px;
  font-weight: 500;
}

.suggestion-prompt {
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--guaxi-text-secondary);
}

/* ── turn ───────────────────────────── */

.turn {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  animation: turn-in 220ms ease both;
}

@keyframes turn-in {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: none; }
}

.spacer { flex: 1 1 auto; }

/* ── 用户消息 ───────────────────────── */

.user-row {
  display: flex;
  justify-content: flex-end;
}

.user-bubble {
  position: relative;
  max-width: min(84%, 620px);
  padding: 9px 13px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.user-part + .user-part {
  margin-top: 8px;
}

.user-text {
  margin: 0;
  font-size: 14px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
}

.user-chip {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  border-radius: 5px;
  background: var(--guaxi-hover-bg);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

.user-image {
  max-width: 240px;
  max-height: 180px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  cursor: zoom-in;
  display: block;
}

.user-tag {
  position: absolute;
  right: 10px;
  bottom: -17px;
  font-size: 10.5px;
  color: #b47d1a;
}

/* ── 助手正文 ───────────────────────── */

.agent-row {
  position: relative;
  min-width: 0;
  padding-left: 2px;
}

.agent-caret {
  width: 7px;
  height: 15px;
  margin-top: 2px;
  background: #b47d1a;
  animation: caret 1s steps(2, start) infinite;
}

@keyframes caret {
  0%, 50% { opacity: 1; }
  50.01%, 100% { opacity: 0; }
}

.agent-actions {
  display: flex;
  gap: 4px;
  margin-top: 6px;
  opacity: 0;
  transition: opacity 140ms ease;
}

.agent-row:hover .agent-actions {
  opacity: 1;
}

.ghost-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 23px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
}

.ghost-btn:hover {
  border-color: var(--guaxi-border);
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

/* ── 思考 ───────────────────────────── */

.reasoning {
  border-left: 1px solid var(--guaxi-border);
  padding-left: 12px;
}

.reasoning-head {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 2px 0;
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  width: 100%;
  text-align: left;
}

.reasoning-head:hover { color: var(--guaxi-text-primary); }

.reasoning-icon { color: #8a93a5; }

.reasoning-time {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  opacity: 0.7;
}

.reasoning-body {
  padding: 4px 0 2px;
  font-size: 12.5px;
  line-height: 1.75;
  color: var(--guaxi-text-secondary);
  animation: unfold 160ms ease both;
}

@keyframes unfold {
  from { opacity: 0; }
  to { opacity: 1; }
}

.reasoning-raw {
  margin-top: 8px;
  font-size: 11.5px;
}

.reasoning-raw summary {
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  opacity: 0.75;
}

.raw-text {
  margin: 6px 0 0;
  white-space: pre-wrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
  line-height: 1.6;
  opacity: 0.7;
}

.rotated { transform: rotate(90deg); }

/* ── 工具组 ─────────────────────────── */

.tool-group {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 72%, transparent);
  overflow: hidden;
}

.group-head {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 7px 11px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.group-head:hover {
  background: var(--guaxi-hover-bg);
}

.group-icon { color: var(--guaxi-text-secondary); }

.group-body {
  border-top: 1px solid var(--guaxi-border);
  animation: unfold 140ms ease both;
}

.warn-text {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: #b42318;
}

.running-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #b47d1a;
  animation: pulse 1.2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.35; transform: scale(0.8); }
}

.cmd + .cmd,
.tool-call + .tool-call {
  border-top: 1px solid var(--guaxi-border);
}

.cmd-head,
.tool-head {
  display: flex;
  align-items: baseline;
  gap: 7px;
  width: 100%;
  padding: 6px 11px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.cmd-head:hover,
.tool-head:hover {
  background: var(--guaxi-hover-bg);
}

.cmd-prompt {
  flex: 0 0 auto;
  color: #b47d1a;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.cmd-text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.cmd-status {
  flex: 0 0 auto;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.cmd-status.run { color: #b47d1a; }
.cmd-status.fail { color: #b42318; }

.cmd-caret {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  align-self: center;
}

.cmd-cwd {
  padding: 0 11px 4px 30px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cmd-output {
  margin: 0;
  padding: 8px 11px 10px 30px;
  max-height: 280px;
  overflow: auto;
  border-top: 1px dashed var(--guaxi-border);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

/* ── 文件改动 ───────────────────────── */

.file-list { padding: 3px 0; }

.file-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 5px 11px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

.file-row:hover { background: var(--guaxi-hover-bg); }

.file-kind {
  flex: 0 0 30px;
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.file-path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.file-nums {
  display: flex;
  gap: 6px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
}

.diff-add { color: #4f8a5b; }
.diff-del { color: #b42318; }

/* ── 工具调用 ───────────────────────── */

.tool-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.tool-detail {
  border-top: 1px dashed var(--guaxi-border);
}

.tool-pre {
  margin: 0;
  padding: 8px 11px;
  max-height: 240px;
  overflow: auto;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.tool-pre + .tool-pre {
  border-top: 1px dashed var(--guaxi-border);
}

.tool-progress {
  color: #b47d1a;
}

/* 工具返回的图片（截屏）：缩略图而不是原图，点开看全尺寸。
   限制高度是为了不让一张 4K 截图把整条转录挤到屏幕外。 */
.tool-images {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 11px;
  border-top: 1px dashed var(--guaxi-border);
}

.tool-images img {
  max-height: 160px;
  max-width: 100%;
  border-radius: var(--guaxi-radius-sm, 4px);
  border: 1px solid var(--guaxi-border);
  cursor: zoom-in;
  object-fit: contain;
}

/* ── 搜索 ───────────────────────────── */

.search-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 10px 12px;
}

.search-head {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.search-query {
  color: var(--guaxi-text-primary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.search-results {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 8px;
}

.search-result {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px 8px;
  border-radius: 5px;
  color: inherit;
  text-decoration: none;
}

.search-result:hover {
  background: var(--guaxi-hover-bg);
}

.result-title {
  font-size: 12.5px;
  color: var(--guaxi-syntax-function);
}

.result-snippet,
.result-url {
  font-size: 11px;
  line-height: 1.5;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-url {
  font-family: var(--guaxi-editor-font-family);
  opacity: 0.65;
}

.search-empty {
  margin: 6px 0 0;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

/* ── 计划 ───────────────────────────── */

.plan-card {
  border: 1px solid rgba(180, 125, 26, 0.35);
  border-radius: 8px;
  padding: 10px 12px;
  background: rgba(180, 125, 26, 0.04);
}

.plan-head {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 4px;
  font-size: 12px;
  color: #b47d1a;
}

/* ── 其他 ───────────────────────────── */

.image-view {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.image-view img {
  max-width: 100%;
  max-height: 360px;
  object-fit: contain;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  cursor: zoom-in;
}

.image-caption {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.divider {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.divider::before,
.divider::after {
  content: '';
  flex: 1 1 auto;
  height: 1px;
  background: var(--guaxi-border);
}

.notice {
  padding-left: 10px;
  border-left: 2px solid var(--guaxi-border);
  font-size: 12px;
  line-height: 1.65;
  color: var(--guaxi-text-secondary);
  white-space: pre-wrap;
}

.raw-item {
  border: 1px dashed var(--guaxi-border);
  border-radius: 6px;
  padding: 8px 10px;
}

.raw-type {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.raw-item pre {
  margin: 5px 0 0;
  max-height: 180px;
  overflow: auto;
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}

.turn-error {
  display: flex;
  align-items: flex-start;
  gap: 7px;
  padding: 8px 10px;
  border: 1px solid rgba(180, 35, 24, 0.35);
  border-radius: 6px;
  background: rgba(180, 35, 24, 0.06);
  color: #b42318;
  font-size: 12px;
  line-height: 1.6;
}

@media (max-width: 900px) {
  .empty-suggestions {
    grid-template-columns: 1fr;
  }
}
</style>
