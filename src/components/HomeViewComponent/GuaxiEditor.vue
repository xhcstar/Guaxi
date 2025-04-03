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
    <div v-show="selectedFile && mode === 'preview'" v-html="renderedHtml" class="preview"></div>

    <!-- 未选中文件时的提示 -->
    <!-- <n-alert v-else type="info" class="info-message">请从左侧选择一个文件进行编辑。</n-alert> -->
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { NTooltip, NIcon } from 'naive-ui'
import { EyeOutline, PencilOutline } from '@vicons/ionicons5'
import { convertFileSrc } from '@tauri-apps/api/core';
import { join, dirname } from '@tauri-apps/api/path'
import * as monaco from 'monaco-editor'
import { marked } from 'marked'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import Prism from 'prismjs'
import 'prismjs/themes/prism.css'
import mermaid from 'mermaid'
import { useFileTreeStore } from '../../stores/fileTreeStore'
// import { useAsyncState } from '@vueuse/core';

// 渲染数学公式
function renderMath(html) {
  const latexRegex = /\$\$(.+?)\$\$|\$(.+?)\$/g;
  return html.replace(latexRegex, (match, blockMath, inlineMath) => {
    let mathContent = blockMath || inlineMath;
    try {
      if (blockMath) {
        return `<div class="katex-display">${katex.renderToString(mathContent, { throwOnError: false })}</div>`;
      } else {
        return `<span class="katex-inline">${katex.renderToString(mathContent, { throwOnError: false })}</span>`;
      }
    } catch (e) {
      console.error("KaTeX 渲染错误：", e);
      return match; // 如果渲染失败，返回原始文本
    }
  });
}

const renderCodeBlocks = (content) => {
  const codeBlocks = content.match(/```[\s\S]+?```/g) || [];

  codeBlocks.forEach((block) => {
    const language = block.match(/```(\w+)\n/);  // 获取语言类型
    if (language && language[1]) {
      const lang = language[1].toLowerCase();
      if (Prism.languages[lang]) {
        block = block.replace(/```(\w+)\n([\s\S]+?)```/g, (match, p1, p2) => {
          const highlightedCode = Prism.highlight(p2, Prism.languages[lang], lang);
          return `<pre class="code-block"><code class="language-${lang}">${highlightedCode}</code></pre>`;
        });
      }
    }
  });

  return content.replace(/```[\s\S]+?```/g, (block) => {
    return `<pre class="code-block">${block}</pre>`;
  });
};

const renderMermaid = (content) => {
  const mermaidRegex = /```mermaid([\s\S]+?)```/g
  return content.replace(mermaidRegex, (match, diagram) => {
    const mermaidId = `mermaid-${Math.random().toString(36).substring(2, 9)}`
    nextTick(() => {
      const target = document.getElementById(mermaidId)
      if (target) {
        target.style.width = '100%' // 确保容器宽度占满父容器
        target.style.height = 'auto' // 高度自适应
        mermaid.render(mermaidId, diagram, (svgCode) => {
          // 移除或调整 viewBox 和 max-width
          svgCode = svgCode.replace(/viewBox="[^"]+"/, 'viewBox="0 0 100 100"')
          svgCode = svgCode.replace(/style="[^"]+"/, 'style="width: 100%; height: auto;"')
          target.innerHTML = svgCode
        })
      }
    })
    return `<div id="${mermaidId}" class="mermaid">${diagram}</div>`
  })
}

const renderMarkdown = async (content, options = {}) => {
  console.log('[DEBUG] 原始Markdown内容:', content.slice(0, 100) + (content.length > 100 ? '...' : ''));
  if (!content) {
    console.warn('Markdown 内容为空或未定义')
    return ''
  }

  try {
    // 1. 配置自定义渲染器
    const renderer = new marked.Renderer();
    renderer.image = (href, title, text) => {
      const src = extractImagePath(href);
      return `<img src="${src}" alt="${text || ''}" class="markdown-image">`;
    };

    marked.use({
      renderer,
      async: true,
      breaks: true,
      gfm: true,
      highlight: (code, lang) => {
        if (Prism.languages[lang]) {
          return Prism.highlight(code, Prism.languages[lang], lang);
        }
        return code;
      }
    });

    // 2. 处理图片路径
    const processedContent = await processImages(content, options.basePath);

    // 3. 渲染 Mermaid 图表
    let html = renderMermaid(processedContent);

    // 4. 使用marked解析Markdown
    html = await marked.parse(html);

    // 5. 渲染数学公式
    html = renderMath(html);

    // 6. 渲染代码块
    html = renderCodeBlocks(html);

    // 添加表格样式
    html = html.replace(
      /<table>/g,
      '<table style="border-collapse: collapse; width: 100%;">'
    );
    html = html.replace(
      /<th>/g,
      '<th style="border: 1px solid black; padding: 8px; background: linear-gradient(to right, #7e807f, #7e806f); color: white; font-weight: bold; border-radius: 4px; box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2); transition: background-color 0.3s ease;">'
    );
    html = html.replace(
      /<td>/g,
      '<td style="border: 1px solid black; padding: 8px; background-color: #f9f9f9; color: #333;">'
    );

    nextTick(() => {
      Prism.highlightAll();
      mermaid.run();
    });

    console.log('[DEBUG] 渲染后的HTML内容:', html);
    return html;

  } catch (error) {
    console.error('Markdown渲染失败:', error);
    return `<div class="render-error">${error.message}</div>`;
  }
}

// 辅助函数：统一提取图片路径
function extractImagePath(href) {
  if (!href) return '';
  // 处理新版marked返回的对象路径
  if (typeof href === 'object' && href.href) return href.href;
  // 处理字符串路径
  if (typeof href === 'string') return href;
  console.warn('未知图片路径格式:', href);
  return '';
}

async function processImages(content, basePath = '') {
  const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
  const replacements = [];

  // 确保 basePath 是字符串而不是 Promise
  const resolvedBasePath = typeof basePath === 'string' ? basePath : await basePath;
  
  for (const match of content.matchAll(imgRegex)) {
    const [full, alt, src] = match;
    if (!src.match(/^(http|\/|data:image)/i)) {
      try {
        // 处理相对路径
        const normalizedSrc = src.replace(/^\.\//, ''); // 去除开头的./
        const fullPath = await join(resolvedBasePath, normalizedSrc);
        const safeSrc = await convertFileSrc(fullPath);
        replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
      } catch (error) {
        console.warn(`路径转换失败: ${src}`, error);
        // 如果路径转换失败，保留原始图片标记
        replacements.push({ from: full, to: `![${alt}](${src})` });
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const mode = ref('edit')
const toggleMode = () => {
  // console.log('切换模式，当前模式：', mode.value)
  mode.value = mode.value === 'edit' ? 'preview' : 'edit'
  if (mode.value === 'edit') {
    nextTick(() => {
      if (!editorContainer.value) {
        console.error('编辑器 DOM 元素不存在，无法更新编辑器内容')
        return
      }
      if (editor) {
        // console.log('切换到编辑模式，更新编辑器内容：', rawContent.value)
        editor.setValue(rawContent.value)
      } else {
        initEditor()
      }
    })
  }
}

const selectedFile = computed(() => fileTreeStore.selectedFile)
const editorContainer = ref(null)
let editor = null

// 定义 rawContent 变量
const rawContent = ref(selectedFile.value?.content || '')

const initEditor = () => {
  if (!editorContainer.value) {
    console.error('编辑器 DOM 元素不存在，无法初始化编辑器')
    return
  }
  if (!editor) {
    console.log('初始化编辑器，初始内容：', rawContent.value)
    editor = monaco.editor.create(editorContainer.value, {
      value: rawContent.value,
      language: 'markdown',
      theme: 'vs',
      automaticLayout: true,
      minimap: { enabled: false },
      lineNumbers: 'on',
      scrollBeyondLastLine: false,
      fontSize: 18, // 设置编辑模式的默认字体大小
      fontFamily: "'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'", // 字体配置
      fontLigatures: true, // 启用连字（仅对支持连字的字体有效）
    })

    // 监听编辑器内容变化，同步到 rawContent
    editor.onDidChangeModelContent(() => {
      // console.log('编辑器内容变化，更新 rawContent：', editor.getValue())
      rawContent.value = editor.getValue()
    })

    // console.log('编辑器实例已创建：', editor)
  }
}

// 更新编辑器内容
const updateEditorContent = (content) => {
  if (editor) {
    // console.log('更新编辑器内容：', content)
    editor.setValue(content || '')
    rawContent.value = content || '' // 同步更新 rawContent
  }
}

// 监听选中的文件变化
watch(selectedFile, (newFile) => {
  if (newFile) {
    // console.log('选中的文件发生变化，新文件内容：', newFile.content)
    rawContent.value = newFile.content || '' // 更新 rawContent
    if (editor) {
      // console.log('编辑器已存在，更新编辑器内容：', newFile.content)
      updateEditorContent(newFile.content)
    } else {
      nextTick(() => {
        // console.log('初始化编辑器并更新内容：', newFile.content)
        initEditor()
        updateEditorContent(newFile.content)
      })
    }
  }
})

// 保存文件内容
const saveContent = () => {
  if (selectedFile.value && editor) {
    const content = editor.getValue()
    fileTreeStore.updateFileContent(selectedFile.value.path, content)
    alert('文件保存成功')
  } else {
    alert('未选中任何文件，无法保存')
  }
}

// 监听快捷键 Ctrl + S
const handleKeyDown = (event) => {
  if (event.ctrlKey && event.key === 's') {
    event.preventDefault()
    saveContent()
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
  mermaid.initialize({
    startOnLoad: true,
    theme: 'default', // 使用默认主题
    flowchart: {
      useMaxWidth: false, // 禁用最大宽度限制
      htmlLabels: true, // 使用 HTML 标签
    },
  })
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  if (editor) {
    editor.dispose()
    editor = null
  }
})

// 定义并导出 renderedContent
const renderedHtml = ref('');

watch([selectedFile, mode, rawContent], async () => {
  if (!selectedFile.value || !mode.value) {
    renderedHtml.value = '';
    return;
  }
  
  try {
    const basePath = selectedFile.value.path ? await dirname(selectedFile.value.path) : '';
    renderedHtml.value = await renderMarkdown(rawContent.value, { basePath });
  } catch (error) {
    console.error('Markdown渲染错误:', error);
    renderedHtml.value = `<div class="render-error">渲染错误: ${error.message}</div>`;
  }
}, { immediate: true });

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
  padding: 0px;
  background-color: #f8f8f8;
}

.toolbar-spacer {
  flex-grow: 1;
}

.mode-toggle {
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  transition: background-color 0.3s ease;
}

.mode-toggle:hover {
  background-color: #e0e0e0;
}

.editor {
  flex: 1;
  width: 100%;
  height: calc(100% - 30px);
  /* 减去工具栏高度 */
}

.preview {
  flex: 1;
  /* width: 100%; */
  height: calc(100% - 30px);
  /* 减去工具栏高度 */
  padding: 8px;
  font-size: 17px;
  /* 设置阅读模式的默认字体大小 */
  font-family: 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace; /* 使用等宽字体 */
  line-height: 1.6;
  /* 可选：设置行高，提升阅读体验 */
  overflow-y: auto;
}

.info-message {
  margin-top: 16px;
}

.code-block {
  background-color: #282a36;
  /* 代码块背景色 */
  color: #f8f8f2;
  /* 代码文本颜色 */
  padding: 12px;
  /* 内边距 */
  border-radius: 6px;
  /* 圆角 */
  overflow-x: auto;
  /* 水平滚动 */
  max-width: fit-content;
  /* 宽度自适应代码行宽 */
  margin: 12px 0;
  /* 外边距 */
  font-family: monospace;
  /* 使用等宽字体 */
  font-size: 14px;
  /* 字体大小 */
}

.code-block code {
  background-color: transparent !important;
  /* 去除 code 标签的背景色 */
  color: inherit !important;
  /* 继承父元素颜色 */
}

/* 语法高亮主题 */
.token.keyword {
  color: #ff79c6;
  /* 关键字颜色 */
}

.token.string {
  color: #50fa7b;
  /* 字符串颜色 */
}

.token.comment {
  color: #6272a4;
  /* 注释颜色 */
}

.token.function {
  color: #8be9fd;
  /* 函数颜色 */
}

.token.number {
  color: #bd93f9;
  /* 数字颜色 */
}

.token.operator {
  color: #ff79c6;
  /* 操作符颜色 */
}

.token.punctuation {
  color: #f8f8f2;
  /* 标点符号颜色 */
}

.mermaid {
  width: 100%;
  /* 确保 Mermaid 图表占满父容器宽度 */
  height: auto;
  /* 高度自适应 */
  margin: 12px 0;
  /* 外边距 */
  overflow: visible !important;
  /* 确保内容不会被裁剪 */
}

.katex-display {
  display: block;
  margin: 10px 0;
}

.katex-inline {
  font-size: 1.2em;
  margin-left: 5px;
  margin-right: 5px;
}

th:hover {
  background-color: #7e807f !important;
}
</style>