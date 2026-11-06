<template>
  <div ref="host" class="rs-source" />
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as monaco from 'monaco-editor'

const props = defineProps({
  modelValue: { type: String, default: '' },
  theme: { type: String, default: 'light' },
})

const emit = defineEmits(['update:modelValue'])

const host = ref(null)
let editor = null
let model = null
let themeReady = false
let applying = false

const FONT_FAMILY = "'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei'"
const themeName = () => (props.theme === 'dark' ? 'guaxi-resume-src-dark' : 'guaxi-resume-src-light')

function ensureThemes() {
  if (themeReady) return
  monaco.editor.defineTheme('guaxi-resume-src-light', {
    base: 'vs',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#fbf9f4',
      'editor.foreground': '#2b271f',
      'editorGutter.background': '#fbf9f4',
      'editorLineNumber.foreground': '#b3aa9a',
      'editorLineNumber.activeForeground': '#6b6255',
      'editorCursor.foreground': '#b23a2b',
      'editor.selectionBackground': '#e6d9c8',
      'editor.lineHighlightBackground': '#f4efe5',
      'editorWidget.background': '#fbf9f4',
      'editorWidget.border': '#ddd4c4',
    },
  })
  monaco.editor.defineTheme('guaxi-resume-src-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#1c1a17',
      'editor.foreground': '#e6dfd1',
      'editorGutter.background': '#1c1a17',
      'editorLineNumber.foreground': '#5c5548',
      'editorLineNumber.activeForeground': '#c7bfae',
      'editorCursor.foreground': '#e0624c',
      'editor.selectionBackground': '#3c352b',
      'editor.lineHighlightBackground': '#232019',
      'editorWidget.background': '#1c1a17',
      'editorWidget.border': '#302c25',
    },
  })
  themeReady = true
}

onMounted(() => {
  ensureThemes()
  model = monaco.editor.createModel(props.modelValue || '', 'html')
  editor = monaco.editor.create(host.value, {
    model,
    theme: themeName(),
    automaticLayout: true,
    minimap: { enabled: false },
    wordWrap: 'on',
    wrappingIndent: 'same',
    scrollBeyondLastLine: false,
    fontSize: 12.5,
    lineHeight: 20,
    fontFamily: FONT_FAMILY,
    tabSize: 2,
    renderWhitespace: 'selection',
    smoothScrolling: true,
    padding: { top: 12, bottom: 16 },
    overviewRulerLanes: 0,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
  })
  editor.onDidChangeModelContent(() => {
    if (applying) return
    emit('update:modelValue', model.getValue())
  })
})

watch(() => props.modelValue, (value) => {
  if (!model) return
  if (model.getValue() === value) return
  applying = true
  model.setValue(value || '')
  applying = false
})

watch(() => props.theme, () => {
  if (editor) monaco.editor.setTheme(themeName())
})

onBeforeUnmount(() => {
  editor?.dispose()
  model?.dispose()
  editor = null
  model = null
})
</script>

<style scoped>
.rs-source {
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>
