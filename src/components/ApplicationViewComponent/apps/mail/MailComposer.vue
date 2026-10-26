<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div
      class="mail-modal mail-composer"
      :class="{ 'is-maximized': maximized }"
      role="dialog"
      aria-label="写邮件"
    >
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">{{ title }}</h2>
        <select
          class="mail-select"
          :value="draft.accountId"
          title="发件账户"
          @change="$emit('update', { accountId: $event.target.value })"
        >
          <option v-for="account in accounts" :key="account.id" :value="account.id">
            {{ account.email }}
          </option>
        </select>
        <button
          class="mail-icon-btn"
          :title="maximized ? '还原窗口' : '最大化'"
          :aria-label="maximized ? '还原窗口' : '最大化'"
          @click="maximized = !maximized"
        >
          <n-icon :component="maximized ? ContractOutline : ExpandOutline" />
        </button>

        <!-- 发送类操作放顶部右侧：工具栏要留给格式按钮，否则两行都挤在一起。 -->
        <div class="mail-compose-actions">
          <button class="mail-btn" :disabled="sending" @click="$emit('save-draft')">保存草稿</button>
          <button
            class="mail-btn"
            :class="{ 'is-on': scheduleOpen }"
            :disabled="sending || !canSend"
            title="选择一个将来的时间发送"
            @click="toggleSchedule"
          >
            定时发送
          </button>
          <button
            v-if="undoSeconds > 0"
            class="mail-btn"
            :disabled="sending || !canSend"
            :title="`${undoSeconds} 秒内可以撤回`"
            @click="send(true)"
          >
            延迟 {{ undoSeconds }} 秒发送
          </button>
          <!-- 发送是主操作：只有它是实心主按钮；延迟发送退成次要按钮。 -->
          <button
            class="mail-btn is-primary mail-send-btn"
            :disabled="sending || !canSend"
            @click="send(false)"
          >
            {{ sending ? '发送中…' : '发送' }}
          </button>
        </div>

        <button class="mail-modal-close" title="关闭" @click="$emit('close')">×</button>
      </div>

      <div class="mail-compose-fields">
        <AddressInput
          :model-value="draft.to"
          label="收件人"
          placeholder="输入姓名或邮箱，回车确认"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { to: $event })"
        >
          <template #actions>
            <button class="mail-fmt-btn" title="抄送" @click="$emit('update', { showCc: !draft.showCc })">
              Cc
            </button>
            <button class="mail-fmt-btn" title="密送" @click="$emit('update', { showBcc: !draft.showBcc })">
              Bcc
            </button>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'to' }"
              @click="openPicker('to')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>

        <AddressInput
          v-if="draft.showCc"
          :model-value="draft.cc"
          label="抄送"
          placeholder="抄送地址"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { cc: $event })"
        >
          <template #actions>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择抄送人"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'cc' }"
              @click="openPicker('cc')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>
        <AddressInput
          v-if="draft.showBcc"
          :model-value="draft.bcc"
          label="密送"
          placeholder="密送地址"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { bcc: $event })"
        >
          <template #actions>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择密送人"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'bcc' }"
              @click="openPicker('bcc')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>
        <div class="mail-compose-line">
          <span class="key">主题</span>
          <input
            :value="draft.subject"
            placeholder="邮件主题"
            @input="$emit('update', { subject: $event.target.value })"
          />
        </div>
      </div>

      <AddressPicker
        v-if="pickerOpen"
        :exclude="[...draft.to, ...draft.cc, ...draft.bcc]"
        @pick="onPickAddress"
        @pick-group="onPickGroup"
        @close="pickerOpen = false"
      />

      <div class="mail-compose-toolbar">
        <template v-if="!sourceMode">
          <select
            class="mail-select mail-fmt-select mail-fmt-font"
            title="字体"
            @mousedown="rememberSelection"
            @change="onFontFamily($event)"
          >
            <option value="">字体</option>
            <option v-for="font in FONT_FAMILIES" :key="font.value" :value="font.value">
              {{ font.label }}
            </option>
          </select>
          <select
            class="mail-select mail-fmt-select"
            title="字号"
            @mousedown="rememberSelection"
            @change="onFontSize($event)"
          >
            <option value="">字号</option>
            <option v-for="size in FONT_SIZES" :key="size" :value="size">{{ size }} px</option>
          </select>
          <span class="mail-fmt-sep"></span>

          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.bold }"
            title="加粗 (Ctrl+B)"
            @mousedown.prevent="format('bold')"
          >
            <b>B</b>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.italic }"
            title="斜体 (Ctrl+I)"
            @mousedown.prevent="format('italic')"
          >
            <i>I</i>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.underline }"
            title="下划线 (Ctrl+U)"
            @mousedown.prevent="format('underline')"
          >
            <u>U</u>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.strike }"
            title="删除线"
            @mousedown.prevent="format('strikeThrough')"
          >
            <span class="mail-fmt-strike">S</span>
          </button>

          <label class="mail-fmt-btn mail-color-btn" title="文字颜色" @mousedown="rememberSelection">
            <span class="mail-color-letter">A</span>
            <span class="mail-color-bar" :style="{ background: lastTextColor }"></span>
            <input
              type="color"
              :value="lastTextColor"
              aria-label="文字颜色"
              @change="onTextColor($event.target.value)"
            />
          </label>
          <label class="mail-fmt-btn mail-color-btn" title="突出显示" @mousedown="rememberSelection">
            <span class="mail-hilite-letter" :style="{ background: lastHiliteColor }">A</span>
            <span class="mail-color-bar" :style="{ background: lastHiliteColor }"></span>
            <input
              type="color"
              :value="lastHiliteColor"
              aria-label="突出显示颜色"
              @change="onHiliteColor($event.target.value)"
            />
          </label>

          <span class="mail-fmt-sep"></span>
          <button
            v-for="align in ALIGNMENTS"
            :key="align.command"
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.align === align.key }"
            :title="align.title"
            @mousedown.prevent="format(align.command)"
          >
            {{ align.label }}
          </button>

          <span class="mail-fmt-sep"></span>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.unordered }"
            title="无序列表"
            @mousedown.prevent="format('insertUnorderedList')"
          >
            •≡
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.ordered }"
            title="有序列表"
            @mousedown.prevent="format('insertOrderedList')"
          >
            1≡
          </button>
          <button class="mail-fmt-btn" title="减少缩进" @mousedown.prevent="format('outdent')">
            ⇤
          </button>
          <button class="mail-fmt-btn" title="增加缩进" @mousedown.prevent="format('indent')">
            ⇥
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.quote }"
            title="引用"
            @mousedown.prevent="format('formatBlock', 'blockquote')"
          >
            ❝
          </button>
          <button class="mail-fmt-btn" title="分隔线" @mousedown.prevent="format('insertHorizontalRule')">
            —
          </button>

          <span class="mail-fmt-sep"></span>
          <button class="mail-fmt-btn" title="插入链接" @mousedown.prevent="insertLink">↗</button>
          <button class="mail-fmt-btn" title="插入代码块" @mousedown.prevent="insertCode">{ }</button>
          <button class="mail-fmt-btn" title="清除格式" @mousedown.prevent="format('removeFormat')">
            ⌫
          </button>
          <button class="mail-fmt-btn" title="撤销 (Ctrl+Z)" @mousedown.prevent="format('undo')">
            ↶
          </button>
          <button class="mail-fmt-btn" title="重做 (Ctrl+Y)" @mousedown.prevent="format('redo')">
            ↷
          </button>
          <span class="mail-fmt-sep"></span>
        </template>

        <button
          class="mail-fmt-btn"
          :class="{ 'is-on': sourceMode }"
          title="切换 HTML 源码"
          @click="toggleSource"
        >
          源码
        </button>
        <button class="mail-fmt-btn" title="添加附件" @click="pickAttachments">📎 附件</button>
        <button class="mail-fmt-btn" title="从笔记库选择文件作为附件" @click="pickFromNotes">
          📚 笔记
        </button>
        <button class="mail-fmt-btn" title="把图片插进正文（内联显示，不当作附件）" @click="pickInlineImages">
          🖼 内联图
        </button>

        <select
          v-if="templates.length"
          class="mail-select"
          style="max-width: 130px"
          title="套用模板"
          @change="onTemplateChange($event)"
        >
          <option value="">套用模板…</option>
          <option v-for="t in templates" :key="t.id" :value="t.id">{{ t.name || '（未命名）' }}</option>
        </select>

        <button
          v-if="needsPeerCert"
          class="mail-fmt-btn"
          :class="{ 'is-on': peerCertReady }"
          :title="peerCertReady ? '收件人证书已导入' : '加密发送需要收件人的 S/MIME 证书'"
          @click="importPeerCert"
        >
          {{ peerCertReady ? '✓ 收件人证书' : '⚠ 导入收件人证书' }}
        </button>
        <select
          class="mail-select"
          style="max-width: 150px"
          :value="draft.cryptoMode || ''"
          title="端到端加密 / 签名"
          @change="$emit('update', { cryptoMode: $event.target.value })"
        >
          <option value="">不加密</option>
          <option value="smime_sign">S/MIME 签名</option>
          <option value="smime_encrypt">S/MIME 加密</option>
          <option value="smime_sign_encrypt">S/MIME 签名并加密</option>
          <option value="pgp_sign">PGP 签名</option>
          <option value="pgp_encrypt">PGP 加密</option>
        </select>
      </div>

      <!-- 定时发送：预设几个常用时间，也可以选具体日期时间。 -->
      <div v-if="scheduleOpen" class="mail-schedule-panel">
        <div class="mail-schedule-presets">
          <span class="mail-hint">定时发送：</span>
          <button class="mail-chip" @click="scheduleAfter(1)">1 小时后</button>
          <button class="mail-chip" @click="scheduleAfter(3)">3 小时后</button>
          <button class="mail-chip" @click="scheduleTomorrowMorning()">明天 9:00</button>
        </div>
        <div class="mail-schedule-custom">
          <input
            v-model="scheduleInput"
            class="mail-input"
            type="datetime-local"
            :min="scheduleMin"
            aria-label="定时发送时间"
          />
          <button class="mail-btn is-primary" :disabled="!scheduleInput" @click="scheduleCustom">
            安排发送
          </button>
          <button class="mail-btn" @click="scheduleOpen = false">取消</button>
        </div>
      </div>

      <div class="mail-compose-body">
        <textarea
          v-if="sourceMode"
          v-show="!monacoAvailable"
          class="mail-compose-source"
          :value="htmlSource"
          spellcheck="false"
          @input="htmlSource = $event.target.value"
        ></textarea>
        <!-- 源码模式用 Monaco：语法高亮、括号匹配、查找替换，写复杂 HTML 比 textarea 稳。
             Monaco 是 MB 级依赖，切到源码模式才懒加载；加载失败时退回上面的 textarea。 -->
        <div
          v-if="sourceMode"
          v-show="monacoAvailable"
          ref="monacoSourceEl"
          class="mail-compose-source-editor"
        ></div>
        <div
          v-else
          ref="editorRef"
          class="mail-compose-editor"
          contenteditable="true"
          data-placeholder="写点什么…"
          @input="onEditorInput"
          @paste="onPaste"
          @keydown.ctrl.b.prevent="format('bold')"
          @keydown.ctrl.i.prevent="format('italic')"
          @keydown.ctrl.u.prevent="format('underline')"
          @keydown.ctrl.enter.prevent="send(false)"
        ></div>
      </div>

      <div v-if="draft.attachments.length || (draft.inlineImages || []).length" class="mail-attach-list">
        <button
          v-for="att in draft.attachments"
          :key="att.path"
          class="mail-attachment"
          :class="{ 'is-danger': isDangerous(att.name) }"
          :title="warningFor(att.name) || `移除 ${att.path}`"
          @click="removeAttachment(att.path)"
        >
          <span>{{ isDangerous(att.name) ? '⚠' : '📄' }}</span>
          <span class="mail-attachment-name">{{ att.name }}</span>
          <span v-if="sizeHint(att.path)" class="mail-attachment-size">{{ sizeHint(att.path) }}</span>
          <span class="mail-attachment-size">×</span>
        </button>
        <button
          v-for="img in draft.inlineImages || []"
          :key="img.contentId"
          class="mail-attachment is-inline"
          :title="`移除内联图片 ${img.name || img.path}`"
          @click="removeInlineImage(img.contentId)"
        >
          <span>🖼</span>
          <span class="mail-attachment-name">{{ img.name || '内联图片' }}</span>
          <span class="mail-attachment-size">×</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { NIcon } from 'naive-ui'
import { ContractOutline, ExpandOutline } from '@vicons/ionicons5'
import { open } from '@tauri-apps/plugin-dialog'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { useContactsStore, expandGroupMembers, groupRecipientToken, parseGroupToken } from '../../../../stores/contactsStore'
import AddressInput from './AddressInput.vue'
import AddressPicker from './AddressPicker.vue'
import { createLogger } from '../../../../utils/logger'

const logger = createLogger('MailComposer')

const props = defineProps({
  draft: { type: Object, required: true },
  accounts: { type: Array, default: () => [] },
  settings: { type: Object, default: () => ({}) },
  themeMode: { type: String, default: 'dark' },
  undoSeconds: { type: Number, default: 0 },
  /** 模板清单（store.templates）。空数组时下拉不显示。 */
  templates: { type: Array, default: () => [] },
  /** 附件检查结果：[{ path, name, size, level, detail }]（store.checkAttachments）。 */
  attachmentChecks: { type: Array, default: () => [] },
})
const emit = defineEmits([
  'update',
  'send',
  'schedule',
  'save-draft',
  'close',
  'apply-template',
  'check-attachments',
])

const editorRef = ref(null)
const maximized = ref(false)
const pickerOpen = ref(false)
/** 通讯录选择器当前给哪个字段加人：收件人 / 抄送 / 密送。 */
const pickerTarget = ref('to')
const contacts = useContactsStore()
const sourceMode = ref(false)
const htmlSource = ref(props.draft.html || '')
const monacoSourceEl = ref(null)
const monacoAvailable = ref(false)
const sending = ref(false)
const scheduleOpen = ref(false)
const scheduleInput = ref('')
const scheduleMin = computed(() => toLocalInput(Date.now() + 60_000))
let lastHtml = props.draft.html || ''
let monacoNs = null
let sourceEditor = null

/** 工具栏字体 / 字号 / 对齐选项。字体只列系统里大概率存在的，避免选了却回退。 */
const FONT_FAMILIES = [
  { label: '系统默认', value: '' },
  { label: '微软雅黑', value: "'Microsoft YaHei', 'PingFang SC', sans-serif" },
  { label: '宋体', value: "SimSun, 'Songti SC', serif" },
  { label: '黑体', value: "SimHei, 'Heiti SC', sans-serif" },
  { label: '楷体', value: "KaiTi, 'Kaiti SC', serif" },
  { label: 'Arial', value: 'Arial, Helvetica, sans-serif' },
  { label: 'Calibri', value: 'Calibri, Candara, Segoe UI, sans-serif' },
  { label: 'Georgia', value: "Georgia, 'Times New Roman', serif" },
  { label: 'Times New Roman', value: "'Times New Roman', Times, serif" },
  { label: '等宽', value: "Consolas, 'Courier New', monospace" },
]
const FONT_SIZES = [12, 13, 14, 15, 16, 18, 20, 24, 28, 32]
const ALIGNMENTS = [
  { command: 'justifyLeft', key: 'left', label: '左', title: '左对齐' },
  { command: 'justifyCenter', key: 'center', label: '中', title: '居中' },
  { command: 'justifyRight', key: 'right', label: '右', title: '右对齐' },
  { command: 'justifyFull', key: 'full', label: '两端', title: '两端对齐' },
]
const formatState = reactive({
  bold: false,
  italic: false,
  underline: false,
  strike: false,
  unordered: false,
  ordered: false,
  quote: false,
  align: '',
})
const lastTextColor = ref('#1a5fd0')
const lastHiliteColor = ref('#fff3a3')

/** 工具栏控件抢焦点前的选区：下拉 / 取色器关闭后要能接着改刚才选中的文字。 */
let savedRange = null
function rememberSelection() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return
  if (!editorRef.value?.contains(selection.anchorNode)) return
  savedRange = selection.getRangeAt(0).cloneRange()
}

function restoreSelection() {
  const editor = editorRef.value
  if (!editor) return null
  editor.focus()
  const selection = window.getSelection()
  if (!selection) return null
  if (savedRange) {
    try {
      selection.removeAllRanges()
      selection.addRange(savedRange)
    } catch {
      // 之前的选区节点可能已经被删除（粘贴图片 / 清除格式）：退回光标当前的位置。
      savedRange = null
    }
  }
  return selection
}

/** 光标/选区变化时刷新工具栏的按下状态。 */
function refreshFormatState() {
  const editor = editorRef.value
  const selection = window.getSelection()
  if (!editor || !selection?.rangeCount || !editor.contains(selection.anchorNode)) return
  // 选区一变就记下来：否则先加粗 A、再选中 B 点斜体时，会拿旧的 A 去应用格式。
  savedRange = selection.getRangeAt(0).cloneRange()
  try {
    formatState.bold = document.queryCommandState('bold')
    formatState.italic = document.queryCommandState('italic')
    formatState.underline = document.queryCommandState('underline')
    formatState.strike = document.queryCommandState('strikeThrough')
    formatState.unordered = document.queryCommandState('insertUnorderedList')
    formatState.ordered = document.queryCommandState('insertOrderedList')
    formatState.align =
      ['justifyCenter', 'justifyRight', 'justifyFull', 'justifyLeft'].find((command) =>
        document.queryCommandState(command),
      )?.replace('justify', '').toLowerCase() || ''
    formatState.quote = String(document.queryCommandValue('formatBlock') || '')
      .toLowerCase()
      .includes('blockquote')
  } catch {
    /* WebView 对部分命令的查询会抛错：保持上一次状态即可 */
  }
}

const title = computed(
  () =>
    ({
      new: '新邮件',
      reply: '回复',
      replyAll: '全部回复',
      forward: '转发',
    })[props.draft.mode] || '新邮件',
)

const canSend = computed(
  () => props.draft.to.length || props.draft.cc.length || props.draft.bcc.length,
)

onMounted(async () => {
  // 通讯组只在地址栏放芯片，成员人数与发送确认要读 contacts.groups；
  // 这里顺手初始化一次，没打开过通讯录页时也能算出真实人数。
  contacts.init().catch(() => {})
  document.addEventListener('selectionchange', refreshFormatState)
  if (!editorRef.value) return
  // 直接把已有 HTML 灌进 contenteditable：用 innerHTML 而不是 v-html，
  // 避免 Vue 与浏览器的编辑结果互相覆盖（contenteditable 里的 DOM 是浏览器在管）。
  editorRef.value.innerHTML = props.draft.html || ''
  lastHtml = props.draft.html || ''
  resolveCidImages()
  await nextTick()
  editorRef.value.focus()
})

// 从「源码模式」切回所见即所得时，把编辑区的 DOM 同步成最新的源码。
watch(sourceMode, async (on) => {
  if (!on) {
    disposeSourceEditor()
    monacoAvailable.value = false
    await nextTick()
    if (editorRef.value) {
      editorRef.value.innerHTML = htmlSource.value
      lastHtml = htmlSource.value
      resolveCidImages()
    }
  } else {
    htmlSource.value = serializeEditorHtml() || props.draft.html || ''
    await nextTick()
    await ensureSourceEditor()
  }
})

// 源码编辑器跟随应用明暗主题。
watch(
  () => props.themeMode,
  (mode) => {
    monacoNs?.editor.setTheme(mode === 'light' ? 'vs' : 'vs-dark')
    sourceEditor?.layout()
  },
)

/** 懒加载 Monaco：只有切到源码模式才加载（MB 级依赖）。 */
async function ensureSourceEditor() {
  if (!sourceMode.value) return
  if (sourceEditor) {
    sourceEditor.layout()
    return
  }
  if (!monacoNs) {
    try {
      monacoNs = await import('monaco-editor')
    } catch (error) {
      logger.warn('Monaco 加载失败，源码模式回退到 textarea', error)
      return
    }
  }
  if (!sourceMode.value) return
  monacoAvailable.value = true
  await nextTick()
  if (!monacoSourceEl.value || sourceEditor) return
  try {
    sourceEditor = monacoNs.editor.create(monacoSourceEl.value, {
      value: htmlSource.value,
      language: 'html',
      theme: props.themeMode === 'light' ? 'vs' : 'vs-dark',
      automaticLayout: true,
      minimap: { enabled: false },
      wordWrap: 'on',
      fontSize: 12.5,
      lineNumbers: 'on',
      scrollBeyondLastLine: false,
      padding: { top: 12, bottom: 12 },
      tabSize: 2,
    })
    sourceEditor.onDidChangeModelContent(() => {
      if (!sourceEditor) return
      const value = sourceEditor.getValue()
      if (value !== htmlSource.value) htmlSource.value = value
    })
  } catch (error) {
    logger.warn('Monaco 初始化失败，源码模式回退到 textarea', error)
    monacoAvailable.value = false
    sourceEditor = null
  }
}

function disposeSourceEditor() {
  sourceEditor?.dispose()
  sourceEditor = null
}

function onEditorInput() {
  const html = serializeEditorHtml()
  if (html === lastHtml) return
  lastHtml = html
  emit('update', { html, text: htmlToText(html) })
}

/**
 * 编辑器里的 `cid:` 图片没法直接显示：用 asset URL 映射到本地临时文件，
 * 并把原始 cid 记在 `data-cid` 上；序列化时再换回 `cid:`（发信/存草稿要用）。
 */
function resolveCidImages() {
  if (!editorRef.value) return
  const images = props.draft.inlineImages || []
  if (!images.length) return
  const map = new Map(
    images.map((img) => [String(img.contentId || '').replace(/^<|>$/g, ''), img.path]),
  )
  editorRef.value.querySelectorAll('img').forEach((img) => {
    const src = img.getAttribute('src') || ''
    if (!/^cid:/i.test(src)) return
    const cid = decodeURIComponent(src.slice(4)).replace(/^<|>$/g, '')
    const path = map.get(cid)
    if (!path) return
    img.dataset.cid = cid
    img.src = convertFileSrc(path)
  })
}

/** 生成用于草稿/发送的 HTML：把显示用的 asset URL 换回 `cid:`。 */
function serializeEditorHtml() {
  if (!editorRef.value) return ''
  const clone = editorRef.value.cloneNode(true)
  clone.querySelectorAll('img[data-cid]').forEach((img) => {
    img.setAttribute('src', `cid:${img.dataset.cid}`)
    img.removeAttribute('data-cid')
  })
  return clone.innerHTML
}

function format(command, value = null) {
  if (sourceMode.value) return
  restoreSelection()
  try {
    // styleWithCSS 让 execCommand 输出 <span style>，而不是 <font> / <b>：
    // 邮件客户端对行内样式支持最好，也能和主题 / 粘贴清洗保持一致。
    document.execCommand('styleWithCSS', false, true)
    document.execCommand(command, false, value)
  } catch (error) {
    logger.warn('格式命令失败', command, error)
  }
  savedRange = currentRange()
  onEditorInput()
  refreshFormatState()
}

function onFontFamily(event) {
  const family = event.target.value
  event.target.value = ''
  if (family) format('fontName', family)
}

function onFontSize(event) {
  const size = Number(event.target.value)
  event.target.value = ''
  if (size) applyFontSize(size)
}

/** execCommand('fontSize') 只支持 1-7 档；这里用 span 包选区，能精确到 px。 */
function applyFontSize(size) {
  if (sourceMode.value) return
  const selection = restoreSelection()
  if (!selection || !selection.rangeCount) return
  const range = selection.getRangeAt(0)
  if (range.collapsed) return
  const span = document.createElement('span')
  span.style.fontSize = `${size}px`
  span.appendChild(range.extractContents())
  range.insertNode(span)
  const next = document.createRange()
  next.selectNodeContents(span)
  selection.removeAllRanges()
  selection.addRange(next)
  savedRange = next.cloneRange()
  onEditorInput()
  refreshFormatState()
}

function onTextColor(color) {
  lastTextColor.value = color
  format('foreColor', color)
}

function onHiliteColor(color) {
  lastHiliteColor.value = color
  restoreSelection()
  try {
    document.execCommand('styleWithCSS', false, true)
    // Chromium/WebView2 支持 hiliteColor；个别环境只认 backColor。
    if (!document.execCommand('hiliteColor', false, color)) {
      document.execCommand('backColor', false, color)
    }
  } catch (error) {
    logger.warn('设置高亮失败', error)
  }
  savedRange = currentRange()
  onEditorInput()
  refreshFormatState()
}

function insertLink() {
  rememberSelection()
  const url = window.prompt('链接地址', 'https://')
  if (!url) return
  format('createLink', url)
}

function insertCode() {
  rememberSelection()
  const code = window.prompt('代码内容')
  if (code == null) return
  const selection = restoreSelection()
  if (!selection || !selection.rangeCount) return
  const pre = document.createElement('pre')
  pre.textContent = code
  const range = selection.getRangeAt(0)
  range.deleteContents()
  range.insertNode(pre)
  const next = document.createRange()
  next.setStartAfter(pre)
  next.collapse(true)
  selection.removeAllRanges()
  selection.addRange(next)
  savedRange = next.cloneRange()
  onEditorInput()
  refreshFormatState()
}

function toggleSource() {
  sourceMode.value = !sourceMode.value
}

/** 粘贴时只保留纯文本与基本行内格式：外部网页的样式会毁掉整封邮件的排版。 */
function onPaste(event) {
  // 剪贴板里的图片：优先作为内联图（截图贴进正文是最常见的用法）。
  // 有些环境只填 clipboardData.files、有些只填 items，两种都取。
  const imageItems = Array.from(event.clipboardData?.items || []).filter(
    (item) => item.kind === 'file' && String(item.type || '').startsWith('image/'),
  )
  const files = imageItems.length
    ? imageItems.map((item) => item.getAsFile()).filter(Boolean)
    : Array.from(event.clipboardData?.files || []).filter((file) =>
        String(file.type || '').startsWith('image/'),
      )
  if (files.length) {
    event.preventDefault()
    // 异步读文件期间编辑器可能失焦：先记住光标位置，读完再插回去。
    const range = currentRange()
    Promise.all(files.map(readFileAsDataUrl)).then(async (urls) => {
      const added = []
      const parts = []
      for (let index = 0; index < urls.length; index += 1) {
        const url = urls[index]
        if (!url) continue
        const inline = await materializePastedImage(url, index).catch((error) => {
          logger.warn('粘贴图片落盘失败，保留 data URL', error)
          return null
        })
        if (inline) {
          added.push(inline)
          parts.push(
            `<img src="cid:${inline.contentId}" data-cid="${inline.contentId}" alt="粘贴的图片" />`,
          )
        } else {
          parts.push(`<img src="${url}" alt="粘贴的图片" />`)
        }
      }
      if (added.length) {
        emit('update', { inlineImages: [...(props.draft.inlineImages || []), ...added] })
      }
      insertHtmlAt(range, parts.join(''))
      onEditorInput()
      nextTick(resolveCidImages)
    })
    return
  }
  const html = event.clipboardData?.getData('text/html')
  if (html) {
    event.preventDefault()
    const cleaned = sanitizePastedHtml(html)
    document.execCommand('insertHTML', false, cleaned)
    // 有些浏览器会把刚插入的表格/图片整块选中，留下一条蓝色高亮。
    collapseSelectionToEnd()
    onEditorInput()
    return
  }
  const text = event.clipboardData?.getData('text/plain')
  if (text != null) {
    event.preventDefault()
    document.execCommand('insertText', false, text)
    collapseSelectionToEnd()
    onEditorInput()
  }
}

function currentRange() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return null
  return selection.getRangeAt(0).cloneRange()
}

function collapseSelectionToEnd() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return
  const range = selection.getRangeAt(0)
  range.collapse(false)
  selection.removeAllRanges()
  selection.addRange(range)
}

function readFileAsDataUrl(file) {
  return new Promise((resolve) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => resolve('')
    reader.readAsDataURL(file)
  })
}

/**
 * 粘贴的图片先落到系统临时目录，再以内联附件（cid）插入正文：
 * 直接发 data URL，多数邮箱（Gmail/Outlook）会把图片当不安全内容剥掉。
 */
async function materializePastedImage(dataUrl, index) {
  const match = dataUrl.match(/^data:image\/([a-zA-Z0-9.+-]+);base64,([\s\S]+)$/)
  if (!match) return null
  const extension =
    match[1].toLowerCase() === 'jpeg' ? 'jpg' : match[1].replace(/[^a-z0-9]/gi, '') || 'png'
  const [{ writeFile, mkdir }, { appDataDir, join }] = await Promise.all([
    import('@tauri-apps/plugin-fs'),
    import('@tauri-apps/api/path'),
  ])
  // 放应用数据目录而不是系统临时目录：草稿可能留几天，TEMP 被清理会让图片变破图。
  const dir = await join(await appDataDir(), 'mail-inline')
  await mkdir(dir, { recursive: true }).catch(() => {})
  const contentId = `guaxi-${Date.now().toString(36)}-${index}-${Math.random().toString(36).slice(2, 8)}@guaxi`
  const name = `粘贴图片-${Date.now()}-${index}.${extension}`
  const path = await join(dir, name)
  const binary = atob(match[2].replace(/\s/g, ''))
  const bytes = Uint8Array.from(binary, (ch) => ch.charCodeAt(0))
  await writeFile(path, bytes)
  return { contentId, path, name }
}

/** 把 HTML 插到记下的光标处，末尾补一个换行并把光标放过去。 */
function insertHtmlAt(range, html) {
  if (!html) return
  const selection = window.getSelection()
  if (range && selection) {
    selection.removeAllRanges()
    selection.addRange(range)
  }
  document.execCommand('insertHTML', false, `${html}<br>`)
  collapseSelectionToEnd()
  editorRef.value?.focus()
}

/** 只保留白色名单里的标签；class/id/style 全部丢弃。 */
function sanitizePastedHtml(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const allowed = new Set([
    'B', 'STRONG', 'I', 'EM', 'U', 'S', 'P', 'BR', 'DIV', 'SPAN', 'A', 'UL', 'OL', 'LI',
    'BLOCKQUOTE', 'PRE', 'CODE', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'TABLE', 'THEAD',
    'TBODY', 'TR', 'TD', 'TH', 'HR', 'IMG',
  ])
  const walk = (node) => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType === 1) {
        if (!allowed.has(child.tagName)) {
          // 不认识的容器：把子节点提上来，保留文字。
          while (child.firstChild) node.insertBefore(child.firstChild, child)
          node.removeChild(child)
          continue
        }
        for (const attr of [...child.attributes]) {
          const name = attr.name.toLowerCase()
          if (name === 'href' && /^(https?:|mailto:|cid:)/i.test(attr.value)) continue
          if (child.tagName === 'IMG' && name === 'src' && /^(https?:|data:image\/|cid:)/i.test(attr.value)) {
            continue
          }
          child.removeAttribute(attr.name)
        }
        walk(child)
      }
    }
  }
  walk(doc.body)
  return doc.body.innerHTML
}

/** 地址栏补全：联系人与历史往来地址统一走 contacts store。 */
async function suggestAddress(prefix) {
  return contacts.suggest(prefix)
}

/** 通讯录选择器：同一个面板可以给收件人、抄送或密送加人。 */
function openPicker(field) {
  if (pickerOpen.value && pickerTarget.value === field) {
    pickerOpen.value = false
    return
  }
  pickerTarget.value = field
  pickerOpen.value = true
}

function appendToTarget(addresses) {
  const key = pickerTarget.value
  const current = [...(props.draft[key] || [])]
  for (const address of addresses) {
    const value = String(address || '').trim()
    if (value && !current.some((item) => sameAddress(item, value))) current.push(value)
  }
  emit('update', { [key]: current })
}

function onPickAddress(address) {
  appendToTarget([address])
}

function onPickGroup(groupId) {
  const group = (contacts.groups || []).find((g) => g.id === groupId)
  appendToTarget([groupRecipientToken(group || { id: groupId, name: '通讯组' })])
  pickerOpen.value = false
}

function sameAddress(a, b) {
  const norm = (value) => {
    const match = String(value || '').match(/<([^>]+)>/)
    return (match ? match[1] : value).trim().toLowerCase()
  }
  return norm(a) === norm(b)
}

/**
 * 收件人合计：通讯组按展开后的唯一地址计数，跨收件人/抄送/密送去重。
 * 大组发送前用它给一次确认，避免用户不知道这一封会发给上百人。
 */
function recipientSummary() {
  const seen = new Set()
  let groups = 0
  for (const list of [props.draft.to, props.draft.cc, props.draft.bcc]) {
    for (const token of list || []) {
      const group = parseGroupToken(token)
      if (group) {
        groups += 1
        for (const member of expandGroupMembers(contacts.groups, group.id)) {
          seen.add(member.email.toLowerCase())
        }
        continue
      }
      const match = String(token || '').match(/<([^>]+)>/)
      const email = (match ? match[1] : token).trim().toLowerCase()
      if (email) seen.add(email)
    }
  }
  return { groups, total: seen.size }
}

async function pickAttachments() {
  try {
    const files = await open({ multiple: true, title: '选择附件' })
    if (!files) return
    addPaths(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('选择附件失败', error)
  }
}

/** 从笔记库挑文件作为附件：直接把笔记库目录当起始位置，省得用户自己找。 */
async function pickFromNotes() {
  try {
    const root = await invoke('get_libraries_root').catch(() => '')
    const files = await open({
      multiple: true,
      title: '从笔记库选择文件',
      defaultPath: root || undefined,
    })
    if (!files) return
    addPaths(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('从笔记库选择附件失败', error)
  }
}

/** 把图片作为 `multipart/related` 的内联资源插进正文。 */
async function pickInlineImages() {
  try {
    const files = await open({
      multiple: true,
      title: '选择要插入正文的图片',
      filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'] }],
    })
    if (!files) return
    addInlineImages(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('选择内联图片失败', error)
  }
}

/**
 * 登记内联图片并把 `<img src="cid:...">` 插到光标处。
 * Content-ID 用随机串而不是文件名：同名图片在多封邮件里复用同一 cid
 * 会让收信端把缓存串起来（换了图却还显示旧图）。
 */
function addInlineImages(paths) {
  if (sourceMode.value) {
    window.alert('请先切回富文本模式再插入图片')
    return
  }
  const next = [...(props.draft.inlineImages || [])]
  for (const path of paths) {
    if (!path) continue
    const name = path.split(/[\\/]/).pop()
    const contentId = `guaxi-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}@guaxi`
    next.push({ contentId, path, name })
    document.execCommand(
      'insertHTML',
      false,
      `<img src="cid:${contentId}" alt="${escapeHtml(name)}" />`,
    )
  }
  emit('update', { inlineImages: next })
  nextTick(resolveCidImages)
  onEditorInput()
}

function removeInlineImage(contentId) {
  const next = (props.draft.inlineImages || []).filter((img) => img.contentId !== contentId)
  emit('update', { inlineImages: next })
  // 正文里对应的 img 一并删掉，否则收信端会看到一个破图。
  if (editorRef.value) {
    editorRef.value.querySelectorAll('img').forEach((img) => {
      if (img.dataset.cid === contentId || img.getAttribute('src') === `cid:${contentId}`) {
        img.remove()
      }
    })
    onEditorInput()
  }
}

/** 套用模板：只覆盖主题与正文，保留收件人。 */
function onTemplateChange(event) {
  const id = event.target.value
  event.target.value = ''
  if (!id) return
  const template = props.templates.find((t) => t.id === id)
  if (!template) return
  if (!window.confirm(`套用模板「${template.name}」会覆盖当前主题与正文，继续？`)) return
  emit('apply-template', template)
  if (editorRef.value && template.html) {
    editorRef.value.innerHTML = template.html
    lastHtml = template.html
  }
}

// ── S/MIME 收件人证书 ──
//
// 加密发送必须有收件人的证书；没有就在工具栏上给一个明显的入口，
// 而不是等到点「发送」才报「无法加密」。
const needsPeerCert = computed(() => String(props.draft.cryptoMode || '').startsWith('smime_encrypt') || String(props.draft.cryptoMode || '').startsWith('smime_sign_encrypt'))
const peerCertReady = ref(false)
const peerAddress = computed(() => {
  const first = props.draft.to?.[0] || ''
  const match = String(first).match(/<([^>]+)>/)
  return (match ? match[1] : first).trim().toLowerCase()
})

watch(
  () => [needsPeerCert.value, peerAddress.value],
  async ([need, address]) => {
    if (!need || !address) {
      peerCertReady.value = false
      return
    }
    peerCertReady.value = await invoke('mail_smime_peer_status', { address }).catch(() => false)
  },
)

async function importPeerCert() {
  const address = peerAddress.value
  if (!address) {
    window.alert('请先填写收件人')
    return
  }
  const { open } = await import('@tauri-apps/plugin-dialog')
  const files = await open({
    multiple: false,
    title: `选择 ${address} 的 S/MIME 证书`,
    filters: [{ name: '证书', extensions: ['pem', 'crt', 'cer'] }],
  })
  if (!files) return
  try {
    const { readTextFile } = await import('@tauri-apps/plugin-fs')
    const certPem = await readTextFile(files)
    await invoke('mail_smime_save_peer', { address, certPem })
    peerCertReady.value = true
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

/** 附件危险类型 / 超限提示。 */
function checkOf(path) {
  return props.attachmentChecks.find((c) => c.path === path) || null
}

function isDangerous(name) {
  const ext = String(name || '').split('.').pop().toLowerCase()
  return [
    'exe', 'scr', 'com', 'bat', 'cmd', 'pif', 'msi', 'vbs', 'vbe', 'js', 'jse',
    'wsf', 'wsh', 'ps1', 'psm1', 'hta', 'cpl', 'reg', 'lnk', 'jar',
  ].includes(ext)
}

function warningFor(path) {
  const check = checkOf(path)
  if (!check) return ''
  return check.level === 'ok' ? '' : check.detail
}

function sizeHint(path) {
  const check = checkOf(path)
  if (!check || !check.size) return ''
  const mb = check.size / 1048576
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(check.size / 1024))} KB`
}

function addPaths(list) {
  const next = [...props.draft.attachments]
  for (const path of list) {
    if (!path) continue
    if (next.some((a) => a.path === path)) continue
    const name = path.replace(/\\/g, '/').split('/').pop()
    next.push({ path, name })
  }
  emit('update', { attachments: next })
  emit('check-attachments', next.map((a) => a.path))
}

/** 把一段文本作为正文追加（用于「把邮件/笔记内容发出去」这类场景）。 */
function appendBodyText(text, asQuote = false) {
  const safe = String(text || '').trim()
  if (!safe) return
  const html = safe
    .split(/\n{2,}/)
    .map((p) => `<p>${escapeHtml(p).replace(/\n/g, '<br>')}</p>`)
    .join('')
  const wrapped = asQuote ? `<blockquote>${html}</blockquote>` : html
  const nextHtml = `${props.draft.html || ''}${wrapped}`
  emit('update', { html: nextHtml, text: `${props.draft.text || ''}\n\n${safe}` })
  if (!sourceMode.value && editorRef.value) {
    editorRef.value.innerHTML = nextHtml
  }
}

function escapeHtml(text) {
  return String(text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

// ── 自动暂存 ──
//
// 每隔一段时间把当前草稿写进本地（不发送、不碰服务器）。目的很具体：
// 写完一大段话，点关闭或切账户后内容不能凭空消失——邮件客户端里最容易让人恼火的事
// 就是"写了半小时的邮件没了"。
const draftId = ref(props.draft.id || `draft-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`)
let autosaveTimer = null
const lastSavedAt = ref(0)

async function stashNow() {
  const hasContent =
    props.draft.subject.trim() ||
    (props.draft.html || '').trim() ||
    (sourceMode.value && htmlSource.value.trim()) ||
    props.draft.to.length ||
    props.draft.text.trim()
  if (!hasContent) return null
  try {
    const payload = {
      ...props.draft,
      id: draftId.value,
      // 编辑区里的最新内容优先：用户可能还没触发 input 事件。
      html: editorRef.value && !sourceMode.value ? serializeEditorHtml() : htmlSource.value,
    }
    await invoke('mail_stash_draft', { draft: payload })
    lastSavedAt.value = Date.now()
    return draftId.value
  } catch (error) {
    logger.warn('自动暂存失败', error)
    return null
  }
}

function startAutosave() {
  stopAutosave()
  // 20 秒一次：太频繁会不断写盘，太稀疏又会在崩溃时丢太多内容。
  autosaveTimer = setInterval(() => {
    stashNow()
  }, 20_000)
}

function stopAutosave() {
  if (autosaveTimer) {
    clearInterval(autosaveTimer)
    autosaveTimer = null
  }
}

onMounted(startAutosave)

onBeforeUnmount(() => {
  document.removeEventListener('selectionchange', refreshFormatState)
  disposeSourceEditor()
  stopAutosave()
  // 关闭时立刻存一次：这一步才是"不会丢"的保证，定时器只是兜底。
  stashNow()
})

defineExpose({ appendBodyText, addPaths, stashNow })

function removeAttachment(path) {
  const next = props.draft.attachments.filter((a) => a.path !== path)
  emit('update', { attachments: next })
  emit('check-attachments', next.map((a) => a.path))
}

async function send(useOutbox) {
  if (sending.value) return
  if (!confirmBulkRecipients()) return
  syncEditorToDraft()
  sending.value = true
  try {
    await nextTick()
    emit('send', { useOutbox })
  } finally {
    sending.value = false
  }
}

/** 大通讯组：地址栏只有一个组名，用户看不出真实人数；发送前报一次总数。 */
function confirmBulkRecipients() {
  const summary = recipientSummary()
  if (summary.total <= 50) return true
  const groupHint = summary.groups ? `（其中 ${summary.groups} 个通讯组）` : ''
  return window.confirm(
    `这封邮件将发给 ${summary.total} 位收件人${groupHint}。\n` +
      '部分邮件服务器对单封收件人数有上限（如 Gmail 100 人），超出时整封会被拒收。仍要发送吗？',
  )
}

/** 把编辑区最新内容并入草稿（用户可能还没触发 input 事件）。 */
function syncEditorToDraft() {
  if (!sourceMode.value && editorRef.value) {
    const html = serializeEditorHtml()
    emit('update', { html, text: htmlToText(html) })
  } else {
    emit('update', { html: htmlSource.value, text: htmlToText(htmlSource.value) })
  }
}

function toLocalInput(ms) {
  const date = new Date(ms)
  const pad = (value) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

function toggleSchedule() {
  if (!canSend.value) return
  if (!scheduleOpen.value) {
    scheduleInput.value = toLocalInput(Date.now() + 60 * 60 * 1000)
  }
  scheduleOpen.value = !scheduleOpen.value
}

function scheduleAfter(hours) {
  scheduleAt(Date.now() + hours * 60 * 60 * 1000)
}

function scheduleTomorrowMorning() {
  const date = new Date()
  date.setDate(date.getDate() + 1)
  date.setHours(9, 0, 0, 0)
  scheduleAt(date.getTime())
}

function scheduleCustom() {
  const ms = new Date(scheduleInput.value).getTime()
  if (!Number.isFinite(ms)) {
    window.alert('请选择有效的发送时间')
    return
  }
  scheduleAt(ms)
}

/** 定时发送：内容校验、大组确认、编辑区同步都和立即发送一致，只是投递时间不同。 */
async function scheduleAt(ms) {
  if (sending.value) return
  if (!canSend.value) return
  if (ms <= Date.now() + 30 * 1000) {
    window.alert('定时发送时间需要晚于当前时间至少 1 分钟')
    return
  }
  if (!confirmBulkRecipients()) return
  syncEditorToDraft()
  sending.value = true
  try {
    await nextTick()
    emit('schedule', ms)
    scheduleOpen.value = false
  } finally {
    sending.value = false
  }
}

function setList(key, value) {
  emit('update', { [key]: splitAddresses(value) })
}

function joinList(list) {
  return (list || []).join(', ')
}

/** 按逗号/分号切分，保留 `Name <addr>` 形态。 */
function splitAddresses(value) {
  return String(value || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
}

/** 富文本转纯文本（发给不支持 HTML 的客户端 / 保存搜索索引用的那一份）。 */
function htmlToText(html) {
  const doc = new DOMParser().parseFromString(String(html || ''), 'text/html')
  doc.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
  doc.querySelectorAll('p,div,li,tr,h1,h2,h3,h4,h5,h6,blockquote,pre').forEach((el) => {
    el.append('\n')
  })
  return (doc.body.textContent || '').replace(/\n{3,}/g, '\n\n').trim()
}
</script>
