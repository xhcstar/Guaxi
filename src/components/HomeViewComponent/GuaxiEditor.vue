<template>
  <div class="editor-container">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-spacer"></div>
      <div class="mode-toggle" @click="toggleMode">
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-icon :component="mode === 'edit' ? PencilOutline : EyeOutline" />
          </template>
          {{ mode === 'edit' ? '编辑模式' : '阅读模式' }}
        </n-tooltip>
      </div>
    </div>

    <!-- 编辑器 -->
    <div v-show="selectedFile && mode === 'edit'" ref="editorContainer" class="editor"></div>

    <!-- 阅读模式 -->
    <div v-show="selectedFile && mode === 'preview'" v-html="renderedHtml" class="preview" ref="previewContainer">
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { NTooltip, NIcon } from 'naive-ui'
import { EyeOutline, PencilOutline } from '@vicons/ionicons5'
import { dirname } from '@tauri-apps/api/path'
import * as monaco from 'monaco-editor'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { debounce } from 'lodash-es'

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const mode = ref<'edit' | 'preview'>('edit');
const renderedHtml = ref('');
const previewContainer = ref<HTMLElement | null>(null);
const editorContainer = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
const selectedFile = computed(() => fileTreeStore.selectedFile)
// 定义 rawContent 变量
const rawContent = ref(selectedFile.value?.content || '')

// 初始化编辑器
const initEditor = () => {
  if (!editorContainer.value) {
    console.error('编辑器容器未找到');
    return;
  }

  editor = monaco.editor.create(editorContainer.value, {
    value: rawContent.value,
    language: 'markdown',
    theme: 'vs',
    automaticLayout: true,
    minimap: { enabled: false },
    lineNumbers: 'on',
    scrollBeyondLastLine: false,
    fontSize: 18,
    fontFamily: "'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    fontLigatures: true,
  });


  editor.onDidChangeModelContent(() => {
    rawContent.value = editor.getValue()
    // 内容变化时触发预览更新
    if (mode.value === 'preview') {
      renderDebounced(rawContent.value)
    }
  })
};

// 更新编辑器内容
const updateEditorContent = (content: string) => {
  if (editor) {
    editor.setValue(content);
    rawContent.value = content;
  } else {
    initEditor();
  }
};

// 保存内容
const saveContent = () => {
  if (selectedFile.value && editor) {
    const content = editor.getValue();
    fileTreeStore.updateFileContent(selectedFile.value.path, content);
    console.log('内容已保存');
  }
};

// 快捷键处理
const handleKeyDown = (e: KeyboardEvent) => {
  if (e.ctrlKey && e.key === 's') {
    e.preventDefault();
    saveContent();
  }
};

// 切换模式
const toggleMode = debounce(async () => {
  mode.value = mode.value === 'edit' ? 'preview' : 'edit';

  if (mode.value === 'edit') {
    await nextTick();
    if (!editor) {
      initEditor();
    }
  } else {
    await renderDebounced(rawContent.value);
  }
}, 150);

// 防抖渲染
const renderDebounced = debounce(async (content) => {
  if (!selectedFile.value || !mode.value || !content) {
    renderedHtml.value = '';
    return;
  }

  try {
    const basePath = selectedFile.value.path ? await dirname(selectedFile.value.path) : '';
    renderedHtml.value = await renderMarkdown(content, {
      basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: 'default'
    });

    // 确保DOM更新后添加按钮
    nextTick(() => {
      if (previewContainer.value) {
        addCopyButtons(previewContainer.value);
      }
    });
  } catch (error) {
    console.error('渲染错误:', error);
    renderedHtml.value = `
      <div class="render-error">
        <p>渲染错误: ${error instanceof Error ? error.message : String(error)}</p>
        <button @click="retryRender">重试</button>
      </div>
    `;
  }
}, 300);

// 监听文件选择
watch(selectedFile, (newFile) => {
  if (newFile) {
    rawContent.value = newFile.content || '';
    if (editor) {
      updateEditorContent(newFile.content || '');
    } else {
      nextTick(() => {
        initEditor();
        updateEditorContent(newFile.content || '');
      });
    }

    if (!selectedFile.value || !mode.value) {
      renderedHtml.value = ''
      return
    }

    // 如果是预览模式立即渲染
    if (mode.value === 'preview') {
      renderDebounced(newFile.content)
    }
  }
}, { immediate: true });

// 监听内容变化
watch([rawContent, mode], () => {
  if (mode.value === 'preview') {
    renderDebounced(rawContent.value);
  }
});

// 监听内容变化
watch(renderedHtml, () => {
  nextTick(() => {
    if (previewContainer.value) {
      addCopyButtons(previewContainer.value);
    }
  });
});

// 生命周期
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  editor?.dispose();
  editor = null;
});

</script>


<style scoped>
.editor-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 0;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 4px 8px;
  /* 减小内边距 */
  height: 28px;
  /* 固定高度 */
  background-color: var(--toolbar-bg, #f8f8f8);
  border-bottom: 1px solid var(--divider-color, #e0e0e0);
}

.toolbar-spacer {
  flex-grow: 1;
}

.mode-toggle {
  cursor: pointer;
  padding: 2px 6px;
  /* 减小内边距 */
  border-radius: 4px;
  transition: background-color 0.2s;
}

.mode-toggle:hover {
  background-color: var(--hover-color, #f0f0f0);
}

.editor {
  flex: 1;
  width: 100%;
  height: calc(100% - 28px);
  /* 调整高度 */
}

.preview {
  flex: 1;
  height: calc(100% - 28px);
  /* 调整高度 */
  padding: 8px;
  font-size: 17px;
  font-family: 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.6;
  overflow-y: auto;
}

.render-error {
  color: var(--error-color, #d32f2f);
  background-color: var(--error-bg, #ffebee);
  padding: 16px;
  border-radius: 4px;
  margin: 16px;
}

.render-error button {
  margin-top: 8px;
  padding: 4px 8px;
  background-color: var(--primary-color, #1976d2);
  color: white;
  border: none;
  border-radius: 2px;
  cursor: pointer;
}

/* 暗色模式适配 */
.dark .toolbar {
  background-color: var(--dark-toolbar-bg, #1e1e1e);
  border-bottom-color: var(--dark-divider-color, #333);
}

.dark .mode-toggle:hover {
  background-color: var(--dark-hover-color, #333);
}
</style>

<style>
/* 数学公式样式 */
.math-inline {
  display: inline;
  vertical-align: middle;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
}

.dark .mermaid {
  background-color: #2d2d2d;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-family: 'JetBrainsMono Nerd Font Mono', monospace;
  background: #f6f8fa !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

/* 代码块复制按钮 */
.hljs::before {
  position: absolute;
  right: 10px;
  top: 10px;
  padding: 2px 8px;
  font-size: 12px;
  background: #e1e4e8;
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.2s;
}

.hljs:hover::before {
  opacity: 1;
}

/* 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
}

.dark .hljs::before {
  background: #444;
  color: #fff;
}


/* 复制按钮图标样式 */
.copy-button svg {
  width: 14px;
  height: 14px;
  vertical-align: middle;
}

/* 复制按钮悬停效果 */
.hljs:hover .copy-button {
  opacity: 1 !important;
}

/* 暗色模式适配 */
.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
  color: #eee !important;
}

.dark .copy-button svg {
  fill: #eee;
}
</style>