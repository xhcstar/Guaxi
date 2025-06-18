<template>
  <div class="search-navigator">
    <n-tabs type="line" animated>
      <n-tab-pane name="files" tab="文件搜索结果">
        <n-empty v-if="fileResults.length === 0" description="暂无文件搜索结果" style="margin-top: 20px" />
        <n-list v-else hoverable clickable>
          <n-list-item v-for="item in fileResults" :key="item.path" @click="openSearchResult(item)">
            <div class="search-result-item">
              <div class="file-name" :title="getFullPath(item)">
                <n-icon size="18" :component="getFileIcon(item.path)" />
                <span class="filename-text">{{ getFileName(item) }}</span>
              </div>
              <div v-if="item.preview" class="preview-content">
                <span class="line-number">L{{ item.line_number }}:</span>
                <span v-html="formatPreview(item)"></span>
              </div>
            </div>
          </n-list-item>
        </n-list>
      </n-tab-pane>

      <n-tab-pane name="notes" tab="内容搜索结果">
        <n-empty v-if="noteResults.length === 0" description="暂无内容搜索结果" style="margin-top: 20px" />
        <n-list v-else hoverable clickable>
          <n-list-item v-for="item in noteResults" :key="`${item.path}-${item.line_number}`"
            @click="openSearchResult(item)">
            <div class="search-result-item">
              <div class="file-name" :title="getFullPath(item)">
                <n-icon size="18" :component="getFileIcon(item.path)" />
                <span class="filename-text">{{ getFileName(item) }}</span>
                <span class="line-badge">L{{ item.line_number }}</span>
              </div>
              <div class="preview-content" v-html="formatPreview(item)"></div>
            </div>
          </n-list-item>
        </n-list>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import {
  DocumentTextOutline,
  CodeSlashOutline,
  FileTrayFullOutline,
  FolderOutline,
} from '@vicons/ionicons5';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { useEditorStore } from '../../stores/editorStore';

const fileTreeStore = useFileTreeStore();
const editorStore = useEditorStore();

const props = defineProps({
  fileResults: {
    type: Array,
    default: () => [],
  },
  noteResults: {
    type: Array,
    default: () => [],
  },
  searchQuery: {
    type: String,
    default: '',
  },
});

// 获取文件图标
const getFileIcon = (path) => {
  const ext = path.split('.').pop()?.toLowerCase() || '';

  if (path.endsWith('/')) return FolderOutline;

  if (['md', 'txt', 'text', 'log'].includes(ext)) {
    return DocumentTextOutline;
  } else if (['rs', 'js', 'ts', 'py', 'java', 'c', 'cpp', 'h'].includes(ext)) {
    return CodeSlashOutline;
  } else if (['json', 'yaml', 'toml', 'xml', 'html', 'css'].includes(ext)) {
    return FileTrayFullOutline;
  }
  return DocumentTextOutline;
};

// 获取显示文件名
const getFileName = (item) => {
  return item.file_name || item.path.split(/[\\/]/).pop();
};

// 获取完整路径提示
const getFullPath = (item) => {
  return item.path.replace(/\\/g, '/');
};

// 格式化预览内容
const formatPreview = (item) => {
  if (!item.preview) return '';

  console.log('formatPreview', item);
  const matches = item.matches || [];
  let preview = item.preview;

  // 1. 先在原始字符串插入高亮标签（从后往前，避免索引错乱）
  [...matches].sort((a, b) => b.start - a.start).forEach(match => {
    preview = [
      preview.slice(0, match.start),
      '<span class="search-highlight">',
      preview.slice(match.start, match.end),
      '</span>',
      preview.slice(match.end),
    ].join('');
  });

  // 2. 只转义非标签内容
  return escapeHtmlExceptHighlight(preview);
};

// 只转义非高亮标签内容
function escapeHtmlExceptHighlight(html) {
  // 用正则分割出所有 <span ...>...</span>，其余部分转义
  return html.split(/(<span class="search-highlight">.*?<\/span>)/g)
    .map(part => {
      if (part.startsWith('<span')) return part;
      // 其余部分转义
      const div = document.createElement('div');
      div.textContent = part;
      return div.innerHTML;
    })
    .join('');
}

// 打开搜索结果
const openSearchResult = async (item) => {
  try {
    // 通过 fileTreeStore 统一打开并获取内容和高亮信息
    const result = await fileTreeStore.fileTreeOpenSearchResult({
      ...item,
      searchText: props.searchQuery,
    });

    // 设置高亮和当前文件
    editorStore.highlightInfo = {
      path: result.path,
      lineNumber: result.lineNumber,
      matches: result.matches,
      text: result.text, // 这里用 text 字段
    };
    editorStore.setFile({
      path: result.path,
      content: result.content || '',
    });
  } catch (err) {
    console.error('打开搜索结果失败:', err);
  }
};
</script>

<style scoped>
.search-navigator {
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
  padding: 12px;
  height: 100%;
  overflow-y: auto;
}

.search-result-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  max-width: 100%;
  width: 100%;
  box-sizing: border-box;
}

.file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 500;
  color: var(--text-color);
}

.filename-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.line-badge {
  background-color: var(--tag-color);
  color: var(--tag-text-color);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
}

.preview-content {
  flex: 1 1 0;
  min-width: 0;
  max-width: 100%;
  width: 100%;
  white-space: pre-wrap;
  word-break: break-all;
  overflow-wrap: break-word;
  /* 可选：多行省略号 */
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  box-sizing: border-box;
}

.line-number {
  flex: 0 0 auto;
  min-width: 40px;
  color: #888;
  font-size: 12px;
  text-align: right;
  margin-right: 4px;
  user-select: none;
}

</style>