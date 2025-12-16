<template>
  <div class="editor-container">
    <!-- 标签页栏 -->
    <div class="tab-bar" ref="tabBarRef" @wheel="handleWheelScroll">
      <div class="tab-region tab-region-left">
        <div
          v-for="tab in leftTabs"
          :key="tab.path"
          :class="['tab-item', { 'active': fileTreeStore.activeTab === tab.path }]"
          @click="switchTab(tab.path)"
          @mousedown.middle="closeTab(tab.path)"
        >
          <span>{{ tab.fileName }}</span>
          <n-icon size="14" @click.stop="closeTab(tab.path)">
            <CloseOutline />
          </n-icon>
        </div>
      </div>
      <div v-if="layoutMode !== 'single'" class="tab-region tab-region-right">
        <div
          v-for="tab in rightTabs"
          :key="tab.path"
          :class="['tab-item', { 'active': fileTreeStore.activeTab === tab.path }]"
          @click="switchTab(tab.path)"
          @mousedown.middle="closeTab(tab.path)"
        >
          <span>{{ tab.fileName }}</span>
          <n-icon size="14" @click.stop="closeTab(tab.path)">
            <CloseOutline />
          </n-icon>
        </div>
      </div>
    </div>
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-editing-actions">
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertHeading(1)">
              H1
            </n-button>
          </template>
          插入一级标题
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertHeading(2)">
              H2
            </n-button>
          </template>
          插入二级标题
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleBold">
              B
            </n-button>
          </template>
          加粗
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleItalic">
              I
            </n-button>
          </template>
          斜体
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertUnorderedList">
              <n-icon size="16" :component="ListOutline" />
            </n-button>
          </template>
          插入列表
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertTable">
              <n-icon size="16" :component="GridOutline" />
            </n-button>
          </template>
          插入表格
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertImage">
              <n-icon size="16" :component="ImagesOutline" />
            </n-button>
          </template>
          插入图片
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertCodeBlockFromToolbar">
              <n-icon size="16" :component="CodeSlashOutline" />
            </n-button>
          </template>
          插入代码块
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="setFontSize">
              <n-icon size="16" :component="TextOutline" />
            </n-button>
          </template>
          设置字号
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="setFontColor">
              <n-icon size="16" :component="ColorPaletteOutline" />
            </n-button>
          </template>
          设置字体颜色
        </n-tooltip>
      </div>
      <div class="toolbar-spacer"></div>
      <div class="toolbar-actions">
        <!-- 添加 Git 操作按钮 -->
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleGitPanel">
              <n-icon>
                <GitBranchOutline />
              </n-icon>
            </n-button>
          </template>
          Git 操作
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleVimMode">
              <n-icon :component="vimMode ? LogoVimeo : Edit16Filled" />
            </n-button>
          </template>
          {{ vimMode ? '禁用Vim模式' : '启用Vim模式' }}
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleMode">
              <n-icon :component="mode === 'edit' ? DocumentEdit16Regular : EyeOutline" />
            </n-button>
          </template>
          {{ mode === 'edit' ? '编辑模式' : '阅读模式' }}
        </n-tooltip>
      </div>
    </div>

    <!-- 编辑器 -->
    <div v-show="selectedFile && mode === 'edit'" class="editor-wrapper">
      <div class="editor-split-wrapper" :class="layoutMode">
        <div ref="editorContainer" class="editor-container">
          <!-- 编辑器内容将在这里渲染 -->
        </div>
        <div
          v-if="layoutMode !== 'single'"
          ref="secondaryEditorContainer"
          class="editor-container"
        >
        </div>
      </div>
      <div v-if="vimMode" class="vim-status-bar" ref="vimStatusBar"></div>
      <GuaxiEditorStatusBar class="editor-status-bar" />
    </div>

    <!-- 阅读模式 -->
    <div v-show="selectedFile && mode === 'preview'" v-html="renderedHtml" class="preview" ref="previewContainer">
    </div>
    <!-- 添加 GitPanel 组件 -->
    <!--
    <GitPanel ref="gitPanel" :workspace-path="fileTreeStore.activeRepoPath" />
    -->
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick, provide } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import GuaxiEditorStatusBar from './GuaxiEditorStatusBar.vue'
import { NTooltip, NIcon, useMessage } from 'naive-ui'
import { EyeOutline, LogoVimeo, CloseOutline, TextOutline, ListOutline, ImagesOutline, CodeSlashOutline, ColorPaletteOutline, GridOutline } from '@vicons/ionicons5'
import { DocumentEdit16Regular, Edit16Filled } from '@vicons/fluent'
import { dirname } from '@tauri-apps/api/path'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { debounce } from 'lodash-es'
import * as monaco from 'monaco-editor'
import { initVimMode } from 'monaco-vim';
import { initTextmate } from '../../monaco/textmateLoader'
import { initLspClient } from '../../monaco/lspClient'
// 更新所有语言导入路径
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'
import 'monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution'
import 'monaco-editor/esm/vs/basic-languages/rust/rust.contribution'
import 'monaco-editor/esm/vs/basic-languages/shell/shell.contribution'
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution'
import 'monaco-editor/esm/vs/basic-languages/java/java.contribution'
import 'monaco-editor/esm/vs/basic-languages/lua/lua.contribution'
import 'monaco-editor/esm/vs/basic-languages/ini/ini.contribution'

import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import { GitBranchOutline } from '@vicons/ionicons5'
import GitPanel from './GitPanel.vue'
import GitService from './utils/GitService'
import { invoke } from '@tauri-apps/api/core'

const outlineStore = useOutlineStore()
// 添加 GitPanel 组件引用
const gitPanel = ref(null)


// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const mode = ref('edit');
const layoutMode = computed(() => fileTreeStore.layoutMode || 'single');
const renderedHtml = ref('');
const previewContainer = ref(null);
const editorContainer = ref(null);
const secondaryEditorContainer = ref(null);
const scrollRatio = ref(0);
const pendingPreviewScrollRestore = ref(false);
const pendingPreviewScrollLine = ref(null);
let editor = null;
let secondaryEditor = null;
const selectedFile = computed(() => fileTreeStore.selectedFile)
// 定义 rawContent 变量
const rawContent = ref(selectedFile.value?.content || '')
// 1. 获取当前激活tab对象和内容
const activeTab = computed(() => fileTreeStore.openTabs.find(tab => tab.path === fileTreeStore.activeTab));
const activeTabContent = computed(() => activeTab.value?.content || '');

const leftTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(0, fileTreeStore.splitIndex);
});

const rightTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return [];
  }
  if (fileTreeStore.openTabs.length === 1) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(fileTreeStore.splitIndex);
});


const message = useMessage()

const vimMode = ref(false);
const vimStatusBar = ref(null);
let vimModeInstance = null;

let markdownFeaturesRegistered = false;

const registerMarkdownLanguageFeatures = () => {
  if (markdownFeaturesRegistered) return;
  markdownFeaturesRegistered = true;

  const createSnippet = (label, insertText, detail) => ({
    label,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  monaco.languages.registerCompletionItemProvider('markdown', {
    triggerCharacters: ['#', '-', '*', '`', '[', '!', '|'],
    provideCompletionItems(model, position) {
      const suggestions = [];
      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.slice(0, position.column - 1);
      const trimmed = beforeCursor.trim();

      if (!trimmed) {
        suggestions.push(
          createSnippet('H1 标题', '# ${1:标题}\n\n', '插入一级标题'),
          createSnippet('H2 标题', '## ${1:标题}\n\n', '插入二级标题'),
          createSnippet(
            '无序列表',
            '- ${1:列表项}\n- ${2:列表项}\n',
            '插入无序列表'
          ),
          createSnippet(
            '有序列表',
            '1. ${1:第一项}\n2. ${2:第二项}\n',
            '插入有序列表'
          ),
          createSnippet(
            '任务列表',
            '- [ ] ${1:任务}\n- [ ] ${2:任务}\n',
            '插入任务列表'
          ),
          createSnippet(
            '表格',
            '| 列1 | 列2 |\n| --- | --- |\n| ${1:值1} | ${2:值2} |\n',
            '插入表格'
          ),
          createSnippet(
            '代码块',
            '```${1:language}\n${2:代码}\n```\n',
            '插入代码块'
          )
        );
      }

      suggestions.push(
        createSnippet('加粗', '**${1:文本}**', '加粗文本'),
        createSnippet('斜体', '*${1:文本}*', '斜体文本'),
        createSnippet('内联代码', '`${1:code}`', '内联代码'),
        createSnippet(
          '链接',
          '[${1:描述}](${2:链接})',
          '插入链接'
        ),
        createSnippet(
          '图片',
          '![${1:描述}](${2:路径})',
          '插入图片'
        ),
        createSnippet('引用', '> ${1:引用内容}\n', '插入引用')
      );

      return { suggestions };
    }
  });
};

const tabBarRef = ref(null)
// 初始化编辑器
const initEditor = () => {
  if (!editorContainer.value) {
    console.error('编辑器容器未找到');
    return;
  }

  // 获取文件扩展名并确定语言
  const fileExt = selectedFile.value?.path?.split('.').pop() || 'markdown';
  const languageMap = {
    'js': 'javascript',
    'ts': 'typescript',
    'css': 'css',
    'html': 'html',
    'htm': 'html',
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
    'json': 'json',
    'yaml': 'yaml',
    'yml': 'yaml',
    'lua': 'lua',
    'java': 'java',
    'toml': 'ini',
  };
  const language = languageMap[fileExt] || 'markdown';

  monaco.editor.defineTheme('one-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '9e9e9e' },
      { token: 'keyword', foreground: '9c27b0' },
      { token: 'string', foreground: '2e7d32' },
      { token: 'number', foreground: 'ef6c00' },
      { token: 'regexp', foreground: '00897b' },
      { token: 'type', foreground: '6a1b9a' },
      { token: 'delimiter', foreground: '2f3337' },
      { token: 'variable', foreground: 'e53935' },
      { token: 'parameter', foreground: '3949ab' },
      { token: 'function', foreground: '1565c0' },
      { token: 'constant', foreground: '00838f' },
      { token: 'class', foreground: '283593' },
      { token: 'identifier', foreground: '2f3337' },
      { token: 'tag', foreground: 'd32f2f' },
      { token: 'attribute.name', foreground: '6d4c41' },
      { token: 'meta.embedded', foreground: '455a64' }
    ],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#2f3337',
      'editorLineNumber.foreground': '#b0b3ba',
      'editorLineNumber.activeForeground': '#2f3337',
      'editorCursor.foreground': '#1565c0',
      'editor.selectionBackground': '#cfe8ff',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f2f5fb',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editor.selectionHighlightBackground': '#ffe0b2',
      'editor.foldBackground': '#f7f9fc',
      'editorWhitespace.foreground': '#d0d0d5',
      'editorIndentGuide.background': '#e0e0e0',
      'editorIndentGuide.activeBackground': '#b0bec5',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#e0e0e0',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#e0e0e0',
      'editorSuggestWidget.foreground': '#2f3337',
      'editorSuggestWidget.selectedBackground': '#e3f2fd',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#e0e0e0',
      'scrollbar.shadow': '#00000033'
    }
  });

  editor = monaco.editor.create(editorContainer.value, {
    value: rawContent.value,
    language: language,
    theme: 'one-light',
    automaticLayout: true,  // 自动布局
    scrollBeyondLastColumn: 40, // 允许水平滚动超出最后一列
    fixedOverflowWidgets: true, // 确保浮动部件可以超出编辑器边界
    wordWrap: 'off',
    renderValidationDecorations: 'on',
    minimap: {
      enabled: false,  // 启用迷你地图
      showSlider: 'always',
      size: 'fit'
    },
    glyphMargin: true, // 启用行号左侧边栏
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
    // wrappingIndent: 'indent',  // 换行缩进
    renderWhitespace: 'selection',  // 选中时显示空白字符
    renderLineHighlight: 'all',  // 高亮当前行
    lineHeight: 24,  // 行高
    padding: {
      top: 8,  // 顶部内边距
      bottom: 8  // 底部内边距
    },
    suggest: {
      snippetsPreventQuickSuggestions: true  // 允许代码片段建议
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

  invoke('get_lsp_syntax_highlight_enabled')
    .then((result) => {
      const enableLspSyntaxHighlight = !!result
      if (enableLspSyntaxHighlight) {
        initTextmate(editor, language);
        initLspClient(editor, language, selectedFile.value?.path || '');
      }
    })
    .catch((error) => {
      console.error('加载 LSP 语法高亮配置失败:', error)
    });

  // 注册编辑器实例到store
  editorStore.setEditorInstance(editor);

  registerMarkdownLanguageFeatures();

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

  // 注册自定义命令
  const registerCommands = () => {
    // Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });
  };

  // 注册自定义命令和菜单项
  const registerContextMenu = () => {
    // 1. 注册Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      contextMenuGroupId: '2_custom',
      contextMenuOrder: 1,
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });

    // 2. 覆盖默认上下文菜单
    editor.onContextMenu((e) => {
      e.event.preventDefault();

      // 获取鼠标位置
      const { posx, posy } = e.event;

      // 3. 定义完整的菜单结构
      const menuItems = [
        // 基础编辑操作
        { id: 'editor.action.clipboardCutAction', label: '剪切' },
        { id: 'editor.action.clipboardCopyAction', label: '复制' },
        { id: 'editor.action.clipboardPasteAction', label: '粘贴' },
        { type: 'separator' },

        // 自定义功能
        { id: 'insert.md.template', label: '插入Markdown模板' },
        { type: 'separator' },

        // 其他功能
        { id: 'editor.action.quickCommand', label: '命令面板' }
      ];

      // 4. 触发菜单显示
      editor.focus();
      editor.trigger('', 'editor.action.showContextMenu', {
        anchor: { x: posx, y: posy },
        menuItems,
        domNode: editorContainer.value
      });
    });

    outlineStore.setJumpToLine(jumpToLine)
  };

  // 初始化上下文菜单
  registerContextMenu();
  setupEditorListeners();
};

const ensureSecondaryEditor = () => {
  if (!secondaryEditorContainer.value || !editor || secondaryEditor) return;
  const model = editor.getModel();
  if (!model) return;
  const baseOptions = editor.getRawOptions ? editor.getRawOptions() : {};
  secondaryEditor = monaco.editor.create(secondaryEditorContainer.value, {
    ...baseOptions,
    model
  });
};

const disposeSecondaryEditor = () => {
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }
};

watch(
  () => fileTreeStore.layoutMode,
  (mode) => {
    if (mode === 'single') {
      disposeSecondaryEditor();
    } else {
      nextTick(() => {
        ensureSecondaryEditor();
      });
    }
  },
  { immediate: true }
);


// Git 服务
const gitService = ref(null)

// 初始化 Git 服务
const initGitService = async () => {
  if (selectedFile.value?.path) {
    const dir = await dirname(selectedFile.value.path)
    gitService.value = new GitService(dir)
    await updateGitStatus()
  }
}

// 更新 Git 状态
const updateGitStatus = async () => {
  if (!gitService.value || !selectedFile.value?.path) return

  try {
    const branch = await gitService.value.getCurrentBranch()
    editorStore.setGitBranch(branch)

    const status = await gitService.value.getFileStatus(selectedFile.value.path)
    editorStore.setGitChanges({
      added: status.filter(s => s.status === 'A').length,
      modified: status.filter(s => s.status === 'M').length,
      deleted: status.filter(s => s.status === 'D').length
    })

    await updateGitDecorations()
  } catch (error) {
    console.error('更新 Git 状态失败:', error)
  }
}

// 更新 Git 装饰器
const updateGitDecorations = async () => {
  if (!editor || !selectedFile.value?.path || !gitService.value) return

  try {
    const changes = await gitService.value.getLineChanges(selectedFile.value.path)
    editorStore.updateGitLineDecorations({
      added: changes.added,
      modified: changes.modified,
      deleted: changes.deleted
    })
  } catch (error) {
    console.error('更新 Git 装饰器失败:', error)
  }
}

// 切换 Git 面板
const toggleGitPanel = async () => {
  const windowLabel = `git-${Date.now()}` // 唯一窗口标识
  const windowTitle = `${fileTreeStore.activeRepoPath || '未命名文件'}` // 使用不带后缀的文件名

  console.log('打开 Git 窗口:', windowLabel, windowTitle)
  console.log('workspace_path:', fileTreeStore.activeRepoPath)
  try {
    await invoke('create_git_window', {
      label: windowLabel,
      title: windowTitle,
      workspacePath: fileTreeStore.activeRepoPath,
    })
  } catch (error) {
    console.error('创建窗口失败:', error)
    alert(`无法打开文件: ${error.message || '未知错误'}`)
  }
}

const handleWheelScroll = (e) => {
  if (!tabBarRef.value) return

  // 阻止默认的垂直滚动行为
  e.preventDefault()

  // 水平滚动（乘以一个系数可以调整滚动速度）
  tabBarRef.value.scrollLeft += e.deltaY * 0.5
}

const setupEditorListeners = () => {
  if (!editor) return;

  // 监听光标位置变化
  editor.onDidChangeCursorPosition(e => {
    editorStore.setCursorPosition(e.position);
  });

  // 监听内容变化（用于问题检测）
  const model = editor.getModel();
  if (model) {
    model.onDidChangeContent(debounce(() => {
      const content = model.getValue();
      const languageId = model.getLanguageId() || 'plaintext';
      const problems = analyzeContentForProblems(content, model, languageId);
      editorStore.setProblems(problems);

      // 更新原始内容
      rawContent.value = content;
    }, 300));
  }

  // 监听语言变化
  editor.onDidChangeModelLanguage(() => {
    const language = editor.getModel()?.getLanguageId() || 'plaintext';
    editorStore.setLanguage(language);
  });

  // 监听选择变化
  editor.onDidChangeCursorSelection(e => {
    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(e.selection);
    const status = selectedText
      ? `Selected ${countLines(selectedText)} lines, ${selectedText.length} chars`
      : null;
    editorStore.setSelectionStatus(status);
  });
};

// 内容问题分析
function analyzeContentForProblems(content, model, languageId) {
  if (!content) {
    if (model) {
      monaco.editor.setModelMarkers(model, 'markdown-lint', []);
    }
    return { errors: 0, warnings: 0 };
  }

  if (languageId === 'markdown') {
    return analyzeMarkdown(content, model);
  }

  let errors = 0;
  let warnings = 0;

  const lines = content.split('\n');
  lines.forEach((line) => {
    const openBraces = (line.match(/\(/g) || []).length;
    const closeBraces = (line.match(/\)/g) || []).length;
    if (openBraces !== closeBraces) errors++;

    if (line.length > 120) warnings++;
  });

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', []);
  }

  return { errors, warnings };
}

function analyzeMarkdown(content, model) {
  let errors = 0;
  let warnings = 0;
  const markers = [];
  const lines = content.split('\n');

  const pushMarker = (lineNumber, message, severity) => {
    if (!model) return;
    const maxColumn = model.getLineMaxColumn(lineNumber) || 1;
    markers.push({
      severity:
        severity === 'error'
          ? monaco.MarkerSeverity.Error
          : monaco.MarkerSeverity.Warning,
      message,
      startLineNumber: lineNumber,
      startColumn: 1,
      endLineNumber: lineNumber,
      endColumn: maxColumn
    });
    if (severity === 'error') {
      errors++;
    } else {
      warnings++;
    }
  };

  let lastHeadingLevel = 0;
  const headingTexts = new Map();
  const tokens = md.parse(content, {});

  tokens.forEach((token, idx) => {
    if (token.type !== 'heading_open') return;

    const level = Number(token.tag.slice(1));
    const nextToken = tokens[idx + 1];
    const lineNumber = token.map ? token.map[0] + 1 : 1;

    if (!nextToken || !nextToken.content || !nextToken.content.trim()) {
      pushMarker(lineNumber, '标题内容为空', 'warning');
    }

    if (lastHeadingLevel && level > lastHeadingLevel + 1) {
      pushMarker(
        lineNumber,
        `标题级别从 H${lastHeadingLevel} 跳到 H${level}`,
        'warning'
      );
    }

    const text = nextToken?.content?.trim();
    if (text) {
      const existed = headingTexts.get(text);
      if (existed) {
        pushMarker(
          lineNumber,
          `标题与第 ${existed} 行重复`,
          'warning'
        );
      } else {
        headingTexts.set(text, lineNumber);
      }
    }

    lastHeadingLevel = level;
  });

  const fenceLines = [];
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('```')) {
      fenceLines.push(index + 1);
    }

    const openBracket = (line.match(/\[/g) || []).length;
    const linkStart = (line.match(/\]\(/g) || []).length;
    const closeParen = (line.match(/\)/g) || []).length;
    if (openBracket && (linkStart === 0 || closeParen < linkStart)) {
      pushMarker(index + 1, '可能存在不完整的链接语法', 'warning');
    }
  });

  if (fenceLines.length % 2 !== 0) {
    const lineNumber = fenceLines[fenceLines.length - 1];
    pushMarker(lineNumber, '代码块未正确闭合', 'error');
  }

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', markers);
  }

  return { errors, warnings };
}

// 计算行数
function countLines(text) {
  return text.split(/\r\n|\n|\r/).length;
}

const ensureMarkdownEditor = () => {
  if (!editor) {
    message.warning('没有可编辑的文档');
    return false;
  }
  const model = editor.getModel();
  if (!model) return false;
  const languageId = model.getLanguageId();
  if (languageId !== 'markdown') {
    message.warning('当前工具栏功能仅在 Markdown 文件中可用');
    return false;
  }
  return true;
};

const wrapSelection = (prefix, suffix, placeholder = '') => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  const selectedText = model.getValueInRange(selection);
  const finalText = prefix + (selectedText || placeholder) + suffix;

  editor.executeEdits('markdown-toolbar', [
    {
      range: selection,
      text: finalText,
      forceMoveMarkers: true
    }
  ]);

  if (!selectedText && placeholder) {
    const startLineNumber = selection.startLineNumber;
    const startColumn = selection.startColumn + prefix.length;
    const endColumn = startColumn + placeholder.length;
    editor.setSelection(
      new monaco.Selection(
        startLineNumber,
        startColumn,
        startLineNumber,
        endColumn
      )
    );
  }

  editor.focus();
};

const insertBlockSnippet = (snippet) => {
  if (!ensureMarkdownEditor()) return;
  const position = editor.getPosition();
  const range = new monaco.Range(
    position.lineNumber,
    1,
    position.lineNumber,
    1
  );

  editor.executeEdits('markdown-toolbar', [
    {
      range,
      text: snippet,
      forceMoveMarkers: true
    }
  ]);

  editor.focus();
};

const insertHeading = (level) => {
  const hashes = '#'.repeat(level);
  insertBlockSnippet(`${hashes} 标题\n`);
};

const toggleBold = () => {
  wrapSelection('**', '**', '粗体文本');
};

const toggleItalic = () => {
  wrapSelection('*', '*', '斜体文本');
};

const insertUnorderedList = () => {
  insertBlockSnippet('- 列表项1\n- 列表项2\n');
};

const insertTable = () => {
  insertBlockSnippet('| 列1 | 列2 |\n| --- | --- |\n| 内容1 | 内容2 |\n');
};

const insertImage = () => {
  insertBlockSnippet('![图片描述](路径或URL)\n');
};

const insertCodeBlockFromToolbar = () => {
  insertBlockSnippet('```language\n代码\n```\n');
};

const setFontSize = () => {
  wrapSelection(
    '<span style="font-size: 20px;">',
    '</span>',
    '大号文字'
  );
};

const setFontColor = () => {
  wrapSelection(
    '<span style="color: #ff0000;">',
    '</span>',
    '彩色文字'
  );
};

// 更新编辑器内容
const updateEditorContent = (content) => {
  if (!editor && content) {
    initEditor();
    return;
  }

  if (editor) {
    // 当内容为空时，确保完全重置编辑器
    if (!content) {
      editor.setValue('');
      const model = editor.getModel();
      if (model) {
        model.setValue('');
      }
    } else {
      editor.setValue(content);
    }
    rawContent.value = content;
  }
};

// 保存内容
const saveContent = () => {
  if (selectedFile.value && editor) {
    const content = editor.getValue();
    fileTreeStore.updateFileContent(selectedFile.value.path, content);
    message.info('修改已保存');
    // 如果是vim模式，也显示保存提示
    if (vimMode.value) {
      message.info('修改已保存');
    }
  }
};

// 快捷键处理
const handleKeyDown = (e) => {
  if (e.ctrlKey && e.key === 's') {
    e.preventDefault();
    saveContent();
  }
};

const captureScrollPosition = () => {
  if (mode.value === 'edit' && editor) {
    const top = editor.getScrollTop();
    const height = editor.getScrollHeight();
    scrollRatio.value = height ? top / height : 0;
  } else if (mode.value === 'preview' && previewContainer.value) {
    const el = previewContainer.value;
    const height = el.scrollHeight;
    scrollRatio.value = height ? el.scrollTop / height : 0;
  } else {
    scrollRatio.value = 0;
  }
};

const restoreScrollPosition = () => {
  if (mode.value === 'preview') {
    pendingPreviewScrollRestore.value = true;
  } else if (mode.value === 'edit' && editor) {
    nextTick(() => {
      const height = editor.getScrollHeight();
      const targetTop = height * scrollRatio.value;
      editor.setScrollTop(targetTop);
    });
  }
};

const scrollPreviewToLine = (lineNumber) => {
  if (!previewContainer.value || !lineNumber) return;

  const container = previewContainer.value;
  const elements = Array.from(
    container.querySelectorAll('[data-md-start-line]')
  );

  if (!elements.length) return;

  let target = elements[0];
  let minDelta = Infinity;

  elements.forEach((el) => {
    const start = Number(el.dataset.mdStartLine || 0);
    const end = Number(el.dataset.mdEndLine || start);
    if (!start && !end) return;

    let delta = 0;
    if (lineNumber < start) {
      delta = start - lineNumber;
    } else if (lineNumber > end) {
      delta = lineNumber - end;
    } else {
      delta = 0;
    }

    if (delta < minDelta) {
      minDelta = delta;
      target = el;
    }
  });

  if (!target) return;

  const containerRect = container.getBoundingClientRect();
  const targetRect = target.getBoundingClientRect();
  const offset = targetRect.top - containerRect.top;
  container.scrollTop += offset - container.clientHeight * 0.2;
};

// 切换模式
const toggleMode = debounce(async () => {
  const isEdit = mode.value === 'edit';

  if (isEdit && editor) {
    const pos = editor.getPosition();
    pendingPreviewScrollLine.value =
      (pos && pos.lineNumber) || editorStore.cursorPosition.lineNumber || 1;
  }

  captureScrollPosition();
  mode.value = isEdit ? 'preview' : 'edit';

  if (mode.value === 'edit') {
    await nextTick();
    if (!editor) {
      initEditor();
    }
    restoreScrollPosition();
  } else {
    renderDebounced(rawContent.value);
    restoreScrollPosition();
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
      basePath: basePath,
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
watch(selectedFile, async (newFile) => {
  if (newFile) {
    const fileExt = newFile.path?.split('.').pop() || 'markdown';
    const languageMap = {
      'js': 'javascript',
      'ts': 'typescript',
      'css': 'css',
      'html': 'html',
      'htm': 'html',
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
      'json': 'json',
      'yaml': 'yaml',
      'yml': 'yaml',
      'lua': 'lua',
      'java': 'java',
      'toml': 'ini',
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

    await updateGitDecorations()
  } else {
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
    }
    renderedHtml.value = '';
    return;
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
      if (pendingPreviewScrollRestore.value) {
        const el = previewContainer.value;
        const height = el.scrollHeight;
        el.scrollTop = height * scrollRatio.value;
        pendingPreviewScrollRestore.value = false;
      }
      if (pendingPreviewScrollLine.value) {
        scrollPreviewToLine(pendingPreviewScrollLine.value);
        pendingPreviewScrollLine.value = null;
      }
    }
  });
});

// 添加Vim模式切换函数
const toggleVimMode = () => {
  if (mode.value === 'preview') {
    message.warning('阅读模式下不能启用Vim编辑模式');
    return;
  }

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

const switchTab = (path) => {
  // 保存当前编辑内容
  if (editor && fileTreeStore.activeTab) {
    const currentContent = editor.getValue();
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === fileTreeStore.activeTab);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].content = currentContent;
    }
  }

  fileTreeStore.setActiveTab(path);
  const tab = fileTreeStore.openTabs.find(t => t.path === path);
  if (tab) {
    updateEditorContent(tab.content);
  }
};

const closeTab = (path) => {
  // 保存当前内容
  if (editor && fileTreeStore.activeTab === path) {
    const content = editor.getValue();
    fileTreeStore.updateTabContent(path, content);
  }

  fileTreeStore.closeTab(path);
  if (fileTreeStore.openTabs.length > 0) {
    const tab = fileTreeStore.openTabs.find(t => t.path === fileTreeStore.activeTab);
    if (tab) {
      updateEditorContent(tab.content);
    }
  } else {
    // 完全清空状态
    fileTreeStore.setActiveTab(null);
    // 强制清空编辑器
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue(''); // 确保模型也被清空
    }
    // 隐藏编辑器区域
    // mode.value = 'preview';
    renderedHtml.value = '';
  }
};

const applyHighlights = () => {
  if (!editor || !editorStore.highlightInfo) return;

  const model = editor.getModel(); // 只声明一次
  if (!model) return;

  const { lineNumber, matches = [], text } = editorStore.highlightInfo;

  console.log("WWW>>> applyHighlights", { lineNumber, matches, text });
  // 清除旧的高亮
  if (editorStore.decorations) {
    editor.deltaDecorations(editorStore.decorations, []);
    editorStore.decorations = [];
  }

  let newDecorations = [];

  // 检查行号合法性
  const totalLines = model.getLineCount();
  const validLineNumber = typeof lineNumber === 'number' && lineNumber >= 1 && lineNumber <= totalLines;

  if (matches && matches.length > 0 && validLineNumber) {
    const lineContent = model.getLineContent(lineNumber);
    const lineLength = lineContent.length;

    newDecorations = matches
      .map(match => {
        let start = Math.max(0, Math.min(match.start, lineLength));
        let end = Math.max(start, Math.min(match.end, lineLength));
        if (start >= end) return null;
        return {
          range: new monaco.Range(
            lineNumber,
            start + 1,
            lineNumber,
            end + 1
          ),
          options: {
            inlineClassName: 'search-highlight',
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        };
      })
      .filter(Boolean);

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (matches.length > 0) {
      let first = matches[0];
      let start = Math.max(0, Math.min(first.start, lineLength));
      editor.revealPositionInCenter({
        lineNumber,
        column: start + 1
      });
    } else {
      editor.revealLineInCenter(lineNumber);
    }
  } else if (text) {
    const foundMatches = model.findMatches(text, false, false, true, null, true);

    newDecorations = foundMatches.map(match => ({
      range: match.range,
      options: {
        inlineClassName: 'search-highlight',
        stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
      }
    }));

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (foundMatches.length > 0) {
      editor.revealRangeInCenter(foundMatches[0].range);
    }
  }
};

// 监听编辑器store的变化
watch(() => editorStore.currentFile, (file) => {
  if (file && editor) {
    console.log("WWW>>> editorStore.currentFile", file)
    editor.setValue(file.content);
    nextTick(applyHighlights);
  }
});

watch(() => editorStore.highlightInfo, () => {
  if (editor) {
    console.log("WWW>>> editorStore.highlightInfo", editorStore.highlightInfo)
    applyHighlights();
  }
}, { deep: true });


const md = new MarkdownIt()
function generateOutline(content) {
  const file = activeTab.value;
  const ext = file?.path?.split('.').pop()?.toLowerCase();
  if (ext !== 'md' && ext !== 'markdown') {
    outlineStore.setOutline([]);
    return;
  }
  const tokens = md.parse(content, {})
  const result = []
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1))
      const nextToken = tokens[idx + 1]
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: token.map ? token.map[0] + 1 : 1
        })
      }
    }
  })
  outlineStore.setOutline(result)
}

const generateOutlineDebounced = debounce((content) => {
  generateOutline(content)
}, 200)

function jumpToLine(line) {
  if (mode.value === 'preview') {
    scrollPreviewToLine(line)
    return
  }
  if (editor) {
    editor.revealLineInCenter(line)
    editor.setPosition({ lineNumber: line, column: 1 })
    editor.focus()
  }
}

// 监听 activeTab 和内容变化，生成大纲
watch([() => fileTreeStore.activeTab, activeTabContent], ([tabPath, content]) => {
  generateOutlineDebounced(content)
}, { immediate: true })


// 生命周期
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  outlineStore.setJumpToLine(jumpToLine)
  initGitService()
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }
  editor?.dispose();
  editor = null;
  editorStore.setEditorInstance(null); // 清除store中的引用
  if (vimModeInstance) {
    vimModeInstance.dispose();
  }
});

</script>


<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  /* 改为固定右对齐 */
  padding: 4px 8px;
  height: 30px;
  background-color: var(--toolbar-bg, #f8f8f8);
  border-bottom: 1px solid var(--divider-color, #e0e0e0);
  position: sticky;
  /* 改为粘性定位 */
  top: 0;
  width: 100%;
  box-sizing: border-box;
  min-width: 0;
  /* 允许工具栏缩小 */
  z-index: 10;
  /* 确保在最上层 */
}

.toolbar-editing-actions {
  display: flex;
  gap: 4px;
  align-items: center;
  flex-wrap: wrap;
}

.toolbar-editing-actions :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toolbar-editing-actions :deep(.n-button:hover) {
  background-color: rgba(0, 0, 0, 0.04);
}

.toolbar-spacer {
  flex-grow: 1;
  min-width: 16px;
  /* 添加最小间距 */
  flex-shrink: 1;
  /* 关键修复4: 允许间距缩小 */
}

.toolbar-actions {
  display: flex;
  gap: 12px;
  /* 新增按钮间距 */
  align-items: center;
  min-width: 0;
  /* 关键修复5: 允许操作区域缩小 */
  flex-shrink: 0;
  /* 关键修复6: 防止按钮被压缩 */
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
  height: calc(100% - 60px);
  /* 减去顶部工具栏和标签栏高度 */
  width: 100%;
  display: flex;
  flex-direction: column;
}

.editor-split-wrapper {
  flex: 1;
  display: flex;
  width: 100%;
  height: 100%;
}

.editor-split-wrapper.single,
.editor-split-wrapper.vertical {
  flex-direction: row;
}

.editor-split-wrapper.horizontal {
  flex-direction: column;
}

.editor-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
  position: relative;
}

.vim-status-bar {
  position: absolute;
  bottom: 22px;
  left: 0;
  right: 0;
  height: 20px;
  background-color: var(--toolbar-bg, #f8f8f8);
  border-top: 1px solid var(--divider-color, #e0e0e0);
  padding: 2px 8px;
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  z-index: 1;
}

.editor-status-bar {
  height: 22px;
  background-color: var(--statusBar-background);
  border-top: 1px solid var(--statusBar-border);
  flex-shrink: 0;
  /* 防止状态栏被压缩 */
  z-index: 2;
  /* 确保在编辑器上方 */
}

.preview {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  top: 40px;
  height: calc(100% - 25px);
  padding: 12px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.5;
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

.tab-bar {
  display: flex;
  height: 30px;
  background: #f5f5f5;
  border-bottom: 1px solid #ddd;
  overflow-x: auto;
}

.tab-region {
  display: flex;
  flex: 1;
}

.tab-region-right {
  border-left: 1px solid #ddd;
}

.tab-bar::-webkit-scrollbar {
  width: 3px;
  height: 4px;
}

.tab-bar::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.tab-bar::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.4);
}

.tab-item {
  display: flex;
  align-items: center;
  padding: 0 15px;
  border-right: 1px solid #ddd;
  cursor: pointer;
  user-select: none;
}

.tab-item.active {
  background: #fff;
  border-bottom: 2px solid #b6bdbe;
}

.tab-item:hover {
  background: #eee;
}

/* Git 行变更装饰器样式 */
.git-line-added {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%2361af61"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

.git-line-modified {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%23e5c07b"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

.git-line-deleted {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%23e06c75"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

/* 确保装饰器区域可见 */
.monaco-editor .glyph-margin {
  background-color: var(--vscode-editorGutter-background);
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

.search-highlight {
  background-color: rgb(241, 168, 10) !important;
  color: #000 !important;
  border-radius: 2px;
  padding: 0 2px;
}

.dark .search-highlight {
  background-color: rgb(255, 208, 0) !important;
  color: #000 !important;
}
</style>