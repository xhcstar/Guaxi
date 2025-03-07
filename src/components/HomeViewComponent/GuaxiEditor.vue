<template>
  <div class="editor-container">
    <!-- 切换按钮 -->
    <div class="toolbar">
      <!-- 其他按钮可以放在这里 -->
      <div class="toolbar-spacer"></div> <!-- 占位符，用于将切换按钮推到最右边 -->
      <!-- 切换按钮 -->
      <div class="mode-toggle" @click="toggleMode">
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-icon :component="mode === 'edit' ? PencilOutline : EyeOutline" />
          </template>
          {{ mode === 'edit' ? '编辑模式' : '阅读模式' }}
        </n-tooltip>
      </div>
    </div>

    <!-- 编辑模式 -->
    <n-input v-if="selectedFile && mode === 'edit'" v-model:value="rawContent" type="textarea"
      :autosize="{ minRows: 10 }" placeholder="编辑文件内容" class="editor" />

    <!-- 阅读模式 -->
    <div v-else-if="selectedFile && mode === 'preview'" v-html="renderedContent" class="editor"></div>

    <!-- 未选中文件时的提示 -->
    <n-alert v-else type="info" class="info-message">请从左侧选择一个文件进行编辑。</n-alert>
  </div>
</template>

<script setup>
import { NInput, NAlert } from 'naive-ui'
import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { EyeOutline, PencilOutline } from '@vicons/ionicons5'

import { marked } from 'marked';
import katex from 'katex'; // 导入 KaTeX 库
import 'katex/dist/katex.min.css'; // 导入 KaTeX 样式
import Prism from 'prismjs';

import 'prismjs/components/prism-python.min.js';  // 引入 Python 支持
import 'prismjs/components/prism-c.min.js'
import 'prismjs/components/prism-cpp.min.js'
import 'prismjs/components/prism-markdown.min.js'
import 'prismjs/components/prism-rust.min.js'

import 'prismjs/components/prism-bash.min.js'
import 'prismjs/components/prism-lua.min.js'
import 'prismjs/components/prism-java.min.js'
import 'prismjs/components/prism-powershell.min.js'
import 'prismjs/components/prism-git.min.js'
import 'prismjs/components/prism-toml.min.js'
import 'prismjs/components/prism-yaml.min.js'
import 'prismjs/components/prism-sql.min.js'
import 'prismjs/components/prism-qml.min.js'

import 'prismjs/components/prism-perl.min.js'
import 'prismjs/components/prism-diff.min.js'
import 'prismjs/components/prism-css.min.js'
import 'prismjs/components/prism-csharp.min.js'
import 'prismjs/components/prism-go.min.js'
import 'prismjs/components/prism-log.min.js'
import 'prismjs/components/prism-latex.min.js'
import 'prismjs/components/prism-json.min.js'
import 'prismjs/components/prism-asciidoc.min.js'
import 'prismjs/components/prism-cmake.min.js'
import 'prismjs/themes/prism.css';  // 引入 Prism 默认样式

function renderMath(html) {
  // 匹配 LaTeX 表达式，包括以下情况：
  // 1. $$...$$ 块级公式
  // 2. $...$ 内联公式
  const latexRegex = /\$\$(.+?)\$\$|\$(.+?)\$/g;

  html = html.replace(latexRegex, (match, blockMath, inlineMath, bracketMath, parenMath) => {
    let mathContent = blockMath || inlineMath || bracketMath || parenMath;
    try {
      // 根据类型选择块级或内联渲染
      if (blockMath) {
        return `<div class="katex-display">${katex.renderToString(mathContent)}</div>`;
      } else {
        return `<span class="katex-inline">${katex.renderToString(mathContent)}</span>`;
      }
    } catch (e) {
      console.error("KaTeX 渲染错误：", e);
      return match; // 如果渲染失败，返回原始文本
    }
  });

  return html;
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

function renderMarkdown(content) {
  if (!content) {
    console.warn('Markdown 内容为空或未定义')
    return ''
  }
  console.log('content:', content);
  // 先将 Markdown 转换为 HTML
  let html = marked(content);

  console.log('htmlmark:', html);
  // 渲染数学公式
  html = renderMath(html);
  console.log('htmlmath:', html);
  // 渲染代码块
  html = renderCodeBlocks(html);

  return html;
}

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
// console.log('GuaxiEditor 组件初始化完成')

// 模式状态：'edit' 或 'preview'
const mode = ref('edit')

// 切换模式
const toggleMode = () => {
  mode.value = mode.value === 'edit' ? 'preview' : 'edit'
}

// 当前选中的文件
const selectedFile = computed(() => {
  // console.log('获取当前选中的文件:', fileTreeStore.selectedFile)
  return fileTreeStore.selectedFile
})

// 原始文件内容
const rawContent = ref(selectedFile.value?.content || '')
// console.log('初始化文件内容:', rawContent.value)

// 渲染后的文件内容
const renderedContent = computed(() => {
  return renderMarkdown(rawContent.value)
})

// 监听选中的文件变化
watch(selectedFile, (newFile) => {
  // console.log('选中的文件发生变化，新文件:', newFile)
  rawContent.value = newFile?.content || ''
  // console.log('更新文件内容:', rawContent.value)
})

// 保存文件内容
const saveContent = () => {
  // console.log('开始保存文件内容')
  if (selectedFile.value) {
    // console.log('保存文件路径:', selectedFile.value.path)
    // console.log('保存文件内容:', rawContent.value)
    fileTreeStore.updateFileContent(selectedFile.value.path, rawContent.value)
    // console.log('保存文件内容成功')
  } else {
    console.warn('未选中任何文件，无法保存')
  }
}
// 监听快捷键 Ctrl + S
const handleKeyDown = (event) => {
  if (event.ctrlKey && event.key === 's') {
    event.preventDefault() // 阻止默认保存行为
    saveContent()
  }
}

// 组件挂载时添加事件监听
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
})

// 组件卸载时移除事件监听
onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
})
</script>

<style scoped>
.editor-container {
  display: flex;
  flex-direction: column;
  height: 99%;
  /* 占满父组件高度 */
  padding: 0px;
  /* 添加内边距 */
}

/* 工具栏样式 */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end; /* 将内容推到最右边 */
  padding: 0px;
  background-color: #f8f8f8;
  /* border-bottom: 0px solid #ddd; */
}

/* 工具栏占位符 */
.toolbar-spacer {
  flex-grow: 1; /* 占据剩余空间 */
}

/* 切换按钮样式 */
.mode-toggle {
  cursor: pointer;
  padding: 2px;
  margin-right: 8px;
  border-radius: 4px;
  transition: background-color 0.3s ease;
}

.mode-toggle:hover {
  background-color: #e0e0e0;
}

.editor {
  flex: 1;
  /* 占满剩余空间 */
  width: 100%;
  /* 占满父组件宽度 */
  font-size: 17px;
  /* 调大默认字体大小 */
  font-family: 'JetBrainsMono Nerd Font', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  /* 使用等宽字体 */
  line-height: 1.6;
  /* 设置行高 */
  padding: 8px; /*添加内边距*/
  overflow-y: auto;
  /* 超过高度时触发滚动 */
}

.info-message {
  margin-top: 8px;
  /* 添加外边距 */
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

/* 代码块滚动区域样式 */
.code-block {
  background-color: #2d2d2d;
  color: #f8f8f2;
  padding: 16px;
  border-radius: 4px;
  overflow-x: auto;
  max-height: 300px;
  white-space: pre-wrap;
  /* 保持代码格式 */
  word-wrap: break-word;
  font-family: 'Courier New', monospace;
  margin-top: 10px;
}
</style>