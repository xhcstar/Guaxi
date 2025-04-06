<template>
  <div class="editor-container">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-spacer"></div>
      <div class="toolbar-actions">
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleVimMode">
              <n-icon :component="vimMode ? TerminalOutline : CodeSlashOutline" />
            </n-button>
          </template>
          {{ vimMode ? '禁用Vim模式' : '启用Vim模式' }}
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleMode">
              <n-icon :component="mode === 'edit' ? PencilOutline : EyeOutline" />
            </n-button>
          </template>
          {{ mode === 'edit' ? '编辑模式' : '阅读模式' }}
        </n-tooltip>
      </div>
    </div>

    <!-- 编辑器 -->
    <div v-show="selectedFile && mode === 'edit'" ref="editorContainer" class="editor-wrapper">
      <div ref="editorContainer" class="editor"></div>
      <div v-if="vimMode" class="vim-status-bar" ref="vimStatusBar"></div>
    </div>

    <!-- 阅读模式 -->
    <div v-show="selectedFile && mode === 'preview'" v-html="renderedHtml" class="preview" ref="previewContainer">
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { NTooltip, NIcon, useMessage } from 'naive-ui'
import { EyeOutline, PencilOutline, TerminalOutline, CodeSlashOutline } from '@vicons/ionicons5'
import { dirname } from '@tauri-apps/api/path'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { debounce } from 'lodash-es'
import * as monaco from 'monaco-editor'
import { initVimMode } from 'monaco-vim';
// 更新所有语言导入路径
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'
import 'monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution'
import 'monaco-editor/esm/vs/basic-languages/rust/rust.contribution'
import 'monaco-editor/esm/vs/basic-languages/shell/shell.contribution'
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution'


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

const message = useMessage() // 使用消息提示

const vimMode = ref(false);
const vimStatusBar = ref<HTMLElement | null>(null);
let vimModeInstance: any = null;

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
    automaticLayout: true,  // 自动布局
    minimap: {
      enabled: true,  // 启用迷你地图
      showSlider: 'always',
      size: 'fit'
    },
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      useShadows: true,
      verticalScrollbarSize: 10,
      horizontalScrollbarSize: 10,
      arrowSize: 10,
      alwaysConsumeMouseWheel: false,
      verticalHasArrows: false,
      horizontalHasArrows: false,
      handleMouseWheel: true,
      scrollByPage: false,
      theme: {
        scrollbarShadow: '#00000022',
        scrollbarSliderBackground: '#64646466',
        scrollbarSliderHoverBackground: '#64646499',
        scrollbarSliderActiveBackground: '#646464CC'
      }
    },
    vim: {
      enabled: vimMode.value,
      status: true,
      useSystemClipboard: true,
      handleKeys: {
        '<C-s>': false,
        '<C-c>': false,
        '<C-v>': false,
        '<C-a>': false
      }
    },
    contextmenu: {
      enabled: true,
      addDefaultActions: true,
      addNavigationActions: true
    },
    lineNumbers: 'on',
    lineNumbersMinChars: 3,  // 最小行号宽度
    scrollBeyondLastLine: false,
    fontSize: 18,  // 调整为更合适的字号
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    fontLigatures: true,
    wordWrap: 'on',  // 自动换行
    wrappingIndent: 'indent',  // 换行缩进
    renderWhitespace: 'selection',  // 选中时显示空白字符
    renderLineHighlight: 'all',  // 高亮当前行
    lineHeight: 24,  // 行高
    padding: {
      top: 8,  // 顶部内边距
      bottom: 8  // 底部内边距
    },
    suggest: {
      snippetsPreventQuickSuggestions: false  // 允许代码片段建议
    },
    quickSuggestions: {
      other: true,
      comments: false,
      strings: true
    },
    parameterHints: {
      enabled: true  // 启用参数提示
    },
    bracketPairColorization: {
      enabled: true  // 括号配对着色
    },
    guides: {
      bracketPairs: true,  // 显示括号配对引导线
      bracketPairsHorizontal: 'active'  // 水平引导线
    },
    mouseWheelZoom: true  // 启用鼠标滚轮缩放
  });

  if (vimMode.value) {
    nextTick(() => {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
    });
  }

  editor.onContextMenu((e) => {
    e.event.preventDefault();
    const menuItems = [
      // 基础操作
      { action: 'editor.action.clipboardCutAction', label: '剪切' },
      { action: 'editor.action.clipboardCopyAction', label: '复制' },
      { action: 'editor.action.clipboardPasteAction', label: '粘贴' },
      { type: 'separator' },

      // 格式相关
      { action: 'editor.action.formatDocument', label: '格式化文档' },
      { action: 'editor.action.commentLine', label: '注释/取消注释' },
      { type: 'separator' },

      // 自定义功能
      {
        label: '插入模板',
        submenu: [
          { action: 'insertTemplate.html', label: 'HTML模板' },
          { action: 'insertTemplate.js', label: 'JavaScript模板' },
          { action: 'insertTemplate.css', label: 'CSS模板' }
        ]
      },
      { type: 'separator' },

      // 其他功能
      { action: 'editor.action.quickCommand', label: '命令面板' }
    ];

    // 显示自定义菜单
    monaco.editor.showContextMenu({
      getActions: () => menuItems,
      getAnchor: () => ({ x: e.event.posx, y: e.event.posy }),
      onHide: () => { }
    });
  });

  // 根据文件扩展名自动设置语言
  const fileExt = selectedFile.value?.path?.split('.').pop() || 'markdown';
  const languageMap = {
    'js': 'javascript',
    'ts': 'typescript',
    'css': 'css',
    'html': 'html',
    'md': 'markdown',
    'py': 'python',
    'cpp': 'cpp',
    'c': 'cpp',
    'h': 'cpp',
    'hpp': 'cpp',
    'rs': 'rust',
    'sh': 'shell',
    'jsx': 'javascript',
    'tsx': 'javascript',
    'htm': 'html',
  };

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
    message.info('修改已保存');
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
    const fileExt = newFile.path?.split('.').pop() || 'markdown';
    const languageMap = {
      'py': 'python',
      'cpp': 'cpp',
      'c': 'cpp',
      'h': 'cpp',
      'hpp': 'cpp',
      'rs': 'rust',
      'sh': 'shell',
      'js': 'javascript',
      'jsx': 'javascript',
      'ts': 'javascript',
      'tsx': 'javascript',
      'html': 'html',
      'htm': 'html',
      'css': 'css',
      'md': 'markdown'
    };

    const language = languageMap[fileExt] || 'markdown';
    if (editor) {
      monaco.editor.setModelLanguage(editor.getModel(), language);
    }

    // 更新内容
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

// 添加Vim模式切换函数
const toggleVimMode = () => {
  vimMode.value = !vimMode.value;

  nextTick(() => {
    if (vimMode.value) {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
    } else {
      if (vimModeInstance) {
        vimModeInstance.dispose();
        vimModeInstance = null;
      }
      if (editor) {
        editor.updateOptions({
          vim: {
            enabled: false
          }
        });
      }
    }
  });
};

// 生命周期
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  editor?.dispose();
  editor = null;
  if (vimModeInstance) {
    vimModeInstance.dispose();
  }
});

</script>


<style scoped>
.editor-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
  position: relative;
  min-width: 0; /* 允许容器缩小 */
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  /* 改为固定右对齐 */
  padding: 4px 8px;
  height: 30px;
  background-color: var(--toolbar-bg, #f8f8f8);
  border-bottom: 1px solid var(--divider-color, #e0e0e0);
  position: sticky; /* 改为粘性定位 */
  top: 0;
  width: 100%;
  box-sizing: border-box;
  min-width: 0; /* 允许工具栏缩小 */
  z-index: 10; /* 确保在最上层 */
}

.toolbar-spacer {
  flex-grow: 1;
  min-width: 16px;
  /* 添加最小间距 */
  flex-shrink: 1; /* 关键修复4: 允许间距缩小 */
}

.toolbar-actions {
  display: flex;
  gap: 12px;
  /* 新增按钮间距 */
  align-items: center;
  min-width: 0; /* 关键修复5: 允许操作区域缩小 */
  flex-shrink: 0; /* 关键修复6: 防止按钮被压缩 */
}

.mode-toggle {
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: background-color 0.2s;
  position: relative;
  z-index: 1;
  white-space: nowrap;
  flex-shrink: 0;
  /* 防止按钮缩小 */
  box-sizing: border-box;
  /* 包含padding和border */
}

/* 响应式处理 */
@media (max-width: 600px) {
  .toolbar {
    padding: 4px;
  }

  .mode-toggle {
    padding: 2px 4px;
  }
}

.mode-toggle:hover {
  background-color: var(--hover-color, #f0f0f0);
}

.editor-wrapper {
  position: relative;
  height: calc(100% - 30px);
  width: 100%;
  box-sizing: border-box;
  overflow: hidden;
}

.editor {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  overflow: hidden;
}

.vim-status-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 20px;
  background-color: var(--toolbar-bg, #f8f8f8);
  border-top: 1px solid var(--divider-color, #e0e0e0);
  padding: 2px 8px;
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  z-index: 5;
  display: flex;
  align-items: center;
  box-sizing: border-box;
}

.preview {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  top: 30px;
  height: calc(100% - 25px);
  padding: 12px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.6;
  overflow-y: auto;
  overflow-x: hidden;
  word-wrap: break-word;
  box-sizing: border-box;
}

.preview code {
  font-family: inherit !important;
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
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.dark .mermaid {
  background-color: #2d2d2d;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #f6f8fa !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

.hljs code {
  font-family: inherit !important;
}

code:not(.hljs code) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
}

/* 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

/* 修改为以下样式： */
.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

table {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid #ddd;
}

th,
td {
  border: 1px solid #ddd;
  padding: 8px;
  text-align: center;
}

th {
  background-color: #f4f4f4;
  font-weight: bold;
}

tr:nth-child(even) {
  background-color: #f9f9f9;
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}
</style>