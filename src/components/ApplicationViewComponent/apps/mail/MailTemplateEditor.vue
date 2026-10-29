<template>
  <div class="mail-template-editor">
    <div class="mail-template-toolbar">
      <button
        v-for="btn in TOOLBAR"
        :key="btn.cmd"
        class="mail-fmt-btn"
        :title="btn.title"
        :aria-label="btn.title"
        @mousedown.prevent="run(btn.cmd)"
      >
        <MailIcon v-if="btn.icon" :name="btn.icon" :size="15" />
        <span v-else class="mail-template-tool-label">{{ btn.label }}</span>
      </button>
      <span class="mail-fmt-sep"></span>
      <button
        class="mail-fmt-btn mail-template-mode"
        :class="{ 'is-on': sourceMode }"
        :title="sourceMode ? '回到富文本编辑' : '直接编辑 HTML 源码'"
        @mousedown.prevent="toggleSource"
      >
        {{ sourceMode ? '富文本' : '源码' }}
      </button>
    </div>

    <div
      v-if="!sourceMode"
      ref="editorRef"
      class="mail-template-body"
      contenteditable="true"
      :data-placeholder="placeholder"
      @input="onInput"
      @paste="onPaste"
    ></div>
    <textarea
      v-else
      ref="sourceRef"
      v-model="sourceText"
      class="mail-template-body mail-template-source"
      spellcheck="false"
      @input="onSourceInput"
    ></textarea>
  </div>
</template>

<script setup>
import { nextTick, onMounted, ref, watch } from 'vue'
import MailIcon from './MailIcon.vue'

/**
 * 模板正文编辑器。
 *
 * 只保留写信时最常用的一小组格式：加粗 / 斜体 / 下划线 / 删除线、列表、引用、清除格式，
 * 外加一个「源码」开关——模板里经常需要直接改 HTML（比如塞一段固定签名表格）。
 * 不做整套写信工具栏：那是给收件人、附件、加密用的，模板这里用不上。
 */
const props = defineProps({
  modelValue: { type: String, default: '' },
  placeholder: { type: String, default: '模板正文…' },
})
const emit = defineEmits(['update:modelValue'])

const TOOLBAR = [
  { cmd: 'bold', label: 'B', title: '加粗 (Ctrl+B)' },
  { cmd: 'italic', label: 'I', title: '斜体 (Ctrl+I)' },
  { cmd: 'underline', label: 'U', title: '下划线 (Ctrl+U)' },
  { cmd: 'strikeThrough', label: 'S', title: '删除线' },
  { cmd: 'insertUnorderedList', icon: 'listBullet', title: '无序列表' },
  { cmd: 'insertOrderedList', icon: 'listOrdered', title: '有序列表' },
  { cmd: 'formatBlock', label: '”', title: '引用', value: 'blockquote' },
  { cmd: 'removeFormat', icon: 'clearFormat', title: '清除格式' },
]

const editorRef = ref(null)
const sourceRef = ref(null)
const sourceMode = ref(false)
const sourceText = ref('')

function currentHtml() {
  return sourceMode.value ? sourceText.value : editorRef.value?.innerHTML || ''
}

function emitValue() {
  emit('update:modelValue', currentHtml())
}

function onInput() {
  lastEmitted = editorRef.value?.innerHTML || ''
  emitValue()
}

// 记下最近一次由自己发出的值，避免外部回写把光标顶回开头。
let lastEmitted = ''

function onSourceInput() {
  lastEmitted = sourceText.value
  emitValue()
}

function run(cmd, value = null) {
  const editor = editorRef.value
  if (!editor || sourceMode.value) return
  editor.focus()
  try {
    document.execCommand('styleWithCSS', false, true)
    document.execCommand(cmd, false, value)
  } catch {
    /* 旧命令不可用就算了，不要打断编辑 */
  }
  onInput()
}

async function toggleSource() {
  if (!sourceMode.value) {
    sourceText.value = editorRef.value?.innerHTML || ''
    sourceMode.value = true
    await nextTick()
    sourceRef.value?.focus()
    return
  }
  sourceMode.value = false
  await nextTick()
  if (editorRef.value) {
    editorRef.value.innerHTML = sourceText.value
    lastEmitted = editorRef.value.innerHTML
  }
}

/** 粘贴：只留安全的结构与格式，脚本 / 事件 / 样式表一律丢掉。 */
function onPaste(event) {
  const html = event.clipboardData?.getData('text/html')
  const text = event.clipboardData?.getData('text/plain')
  event.preventDefault()
  if (!html) {
    document.execCommand('insertText', false, text || '')
    onInput()
    return
  }
  document.execCommand('insertHTML', false, cleanPasted(html))
  onInput()
}

const ALLOWED = new Set([
  'A', 'B', 'STRONG', 'I', 'EM', 'U', 'S', 'STRIKE', 'DEL', 'BR', 'P', 'DIV', 'SPAN',
  'UL', 'OL', 'LI', 'BLOCKQUOTE', 'PRE', 'CODE', 'TABLE', 'THEAD', 'TBODY', 'TR', 'TD', 'TH',
  'H1', 'H2', 'H3', 'H4', 'HR', 'IMG',
])

function cleanPasted(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const walk = (node) => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType !== 1) continue
      if (!ALLOWED.has(child.tagName)) {
        // 不认识的容器：把子节点提上来，保留文字。
        while (child.firstChild) node.insertBefore(child.firstChild, child)
        node.removeChild(child)
        continue
      }
      for (const attr of [...child.attributes]) {
        const name = attr.name.toLowerCase()
        if (name.startsWith('on')) child.removeAttribute(attr.name)
        else if (name === 'href' && /^\s*javascript:/i.test(attr.value)) child.removeAttribute('href')
        else if (name === 'src' && /^\s*cid:/i.test(attr.value)) node.removeChild(child)
        else if (!['href', 'src', 'colspan', 'rowspan', 'style'].includes(name)) {
          child.removeAttribute(attr.name)
        }
      }
      walk(child)
    }
  }
  walk(doc.body)
  return doc.body.innerHTML
}

// 外部改了正文（切换模板 / 新建）时同步进来；自己发出的值不重复回写。
watch(
  () => props.modelValue,
  (value) => {
    const next = value || ''
    if (next === lastEmitted) return
    if (sourceMode.value) sourceText.value = next
    else if (editorRef.value && editorRef.value.innerHTML !== next) {
      editorRef.value.innerHTML = next
      lastEmitted = next
    }
  },
)

onMounted(() => {
  if (editorRef.value) {
    editorRef.value.innerHTML = props.modelValue || ''
    lastEmitted = editorRef.value.innerHTML
  }
})
</script>
