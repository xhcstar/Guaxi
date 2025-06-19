<template>
  <div class="search-navigator">
    <n-tabs type="line" animated>
      <n-tab-pane name="files" tab="文件搜索结果">
        <n-empty v-if="fileResults.length === 0" description="暂无文件搜索结果" style="margin-top: 20px" />
        <n-list v-else hoverable clickable>
          <n-list-item v-for="item in fileResults" :key="item.path" @click="openSearchResult(item)">
            <div class="search-result-item">
              <div class="file-name" :title="getFullPath(item)">
                <IconifyIcon v-bind="getFileIconPropsWithLog(item.path)" />
                <span class="filename-text">{{ getFileName(item) }}</span>
                <span class="line-badge" v-if="item.line_number">L{{ item.line_number }}</span>
              </div>
              <div v-if="item.preview" class="preview-content">
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
                <IconifyIcon v-bind="getFileIconPropsWithLog(item.path)" />
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
import { h, defineComponent } from 'vue';
import { Icon } from '@iconify/vue';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { useEditorStore } from '../../stores/editorStore';


const IconifyIcon = defineComponent({
  name: 'IconifyIcon',
  props: {
    icon: {
      type: String,
      required: true
    },
    size: {
      type: [Number, String],
      default: 18
    },
    color: {
      type: String,
      default: undefined
    }
  },
  setup(props) {
    return () =>
      h(Icon, {
        icon: props.icon,
        width: props.size,
        height: props.size,
        style: props.color ? { color: props.color } : undefined
      });
  }
});

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

/**
 * 文件类型到图标的映射表
 * 使用 VSCode 风格图标 + Material Design 图标作为补充
 */
const FILE_ICON_MAP = {
  // 文件夹
  folder: 'mdi:folder-outline',

  // 编程语言
  js: 'vscode-icons:file-type-js-official',
  ts: 'vscode-icons:file-type-typescript-official',
  py: 'vscode-icons:file-type-python',
  java: 'vscode-icons:file-type-java',
  cpp: 'vscode-icons:file-type-cpp',
  cc: 'vscode-icons:file-type-cpp',
  cxx: 'vscode-icons:file-type-cpp',
  hpp: 'vscode-icons:file-type-cpp-header',
  hxx: 'vscode-icons:file-type-cpp-header',
  c: 'vscode-icons:file-type-c',
  h: 'vscode-icons:file-type-c-header',
  cs: 'vscode-icons:file-type-csharp',
  go: 'vscode-icons:file-type-go',
  rs: 'vscode-icons:file-type-rust',
  php: 'vscode-icons:file-type-php',
  sh: 'mdi:bash',
  gitignore: 'mdi:git',
  rb: 'vscode-icons:file-type-ruby',
  md: 'vscode-icons:file-type-markdown',
  lua: 'vscode-icons:file-type-lua',
  vue: 'vscode-icons:file-type-vue',

  // 文档
  txt: 'mdi:file-document-outline',
  text: 'mdi:file-document-outline',
  log: 'mdi:file-document-outline',
  doc: 'vscode-icons:file-type-word',
  docx: 'vscode-icons:file-type-word',
  xls: 'vscode-icons:file-type-excel',
  xlsx: 'vscode-icons:file-type-excel',
  csv: 'mdi:file-delimited-outline',
  ppt: 'vscode-icons:file-type-powerpoint',
  pptx: 'vscode-icons:file-type-powerpoint',
  pdf: 'vscode-icons:file-type-pdf',

  // 代码文件
  json: 'vscode-icons:file-type-json',
  html: 'vscode-icons:file-type-html',
  css: 'vscode-icons:file-type-css',
  yaml: 'vscode-icons:file-type-yaml',
  toml: 'vscode-icons:file-type-toml',
  xml: 'vscode-icons:file-type-xml',

  // 压缩包
  zip: 'vscode-icons:file-type-zip',
  rar: 'vscode-icons:file-type-zip',
  '7z': 'vscode-icons:file-type-zip',
  tar: 'vscode-icons:file-type-zip',
  gz: 'vscode-icons:file-type-zip',

  // 媒体文件
  png: 'vscode-icons:file-type-image',
  jpg: 'vscode-icons:file-type-image',
  jpeg: 'vscode-icons:file-type-image',
  gif: 'vscode-icons:file-type-image',
  bmp: 'vscode-icons:file-type-image',
  svg: 'vscode-icons:file-type-svg',
  webp: 'vscode-icons:file-type-image',
  mp3: 'vscode-icons:file-type-audio',
  wav: 'vscode-icons:file-type-audio',
  ogg: 'vscode-icons:file-type-audio',
  flac: 'vscode-icons:file-type-audio',
  mp4: 'vscode-icons:file-type-video',
  avi: 'vscode-icons:file-type-video',
  mov: 'vscode-icons:file-type-video',
  mkv: 'vscode-icons:file-type-video',
  webm: 'vscode-icons:file-type-video',

  // 默认文件
  default: 'mdi:file-outline'
};

const getFileIconPropsWithLog = (path) => {
  const props = getFileIconProps(path);
  if (!props.icon) {
    console.warn('[n-icon] icon prop missing!', path, props);
  }
  return props;
};
const getFileIconComponent = (path) => {
  return IconifyIcon;
};
const getFileIconProps = (path) => {
  const ext = path.split('.').pop()?.toLowerCase() || '';
  const isActive = editorStore.activeFile?.path === path;

  if (path.endsWith('/')) {
    const folderIcon = {
      icon: FILE_ICON_MAP.folder,
      color: isActive ? '#3366ff' : '#666'
    };
    console.log('[IconProps][folder]', path, folderIcon);
    return folderIcon;
  }
  // 特殊处理复合扩展名
  const specialExtMap = {
    'd.ts': 'ts',
    'spec.js': 'js',
    'test.js': 'js'
  };
  const specialExt = Object.keys(specialExtMap).find(e => path.endsWith(`.${e}`));
  const finalExt = specialExt ? specialExtMap[specialExt] : ext;

  const icon = FILE_ICON_MAP[finalExt] || FILE_ICON_MAP.default;
  const props = {
    icon,
    color: isActive ? '#3366ff' : '#666'
  };
  console.log('[IconProps][file]', path, { ext, finalExt, icon, props });
  return props;
};

/**
 * 获取显示文件名
 * @param {Object} item 文件项
 * @returns {string} 文件名
 */
const getFileName = (item) => {
  return item.file_name || item.path.split(/[\\/]/).pop();
};

/**
 * 获取完整路径提示
 * @param {Object} item 文件项
 * @returns {string} 完整路径
 */
const getFullPath = (item) => {
  return item.path.replace(/\\/g, '/');
};

/**
 * 格式化预览内容（添加高亮）
 * @param {Object} item 文件项
 * @returns {string} HTML格式的预览内容
 */
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

/**
 * 只转义非高亮标签内容
 * @param {string} html 包含高亮标签的HTML
 * @returns {string} 安全转义后的HTML
 */
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

/**
 * 打开搜索结果
 * @param {Object} item 文件项
 */
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
  height: 100%;
  max-width: 512px;
  overflow-y: auto;
  padding: 0 0px;
  box-sizing: border-box;
}

.n-list {
  padding: 0;
  margin: 0;
  max-height: 100%;
  overflow-y: auto;
}

/* 紧凑化 n-list-item */
.n-list-item {
  margin: 0 !important;
  padding: 2px 0 !important;
}

/* 紧凑化 search-result-item，设置最大宽度和横向滚动 */
.search-result-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  max-width: 600px;
  width: 100%;
  box-sizing: border-box;
  background: var(--n-card-color, #fff);
  border-radius: 6px;
  box-shadow: none;
  padding: 4px 6px;
  margin-bottom: 2px;
  transition: box-shadow 0.2s;
  overflow-x: auto;
}

.n-list-item:hover .search-result-item {
  box-shadow: 0 0 0 3pxrgb(33, 189, 236), 0 2px 8px 0 rgba(7, 118, 161, 0.1);
  background: #f0f7ff;
  transition: box-shadow 0.2s, background 0.2s;
}

.file-name {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  font-weight: 500;
  color: var(--text-color);
  width: 100%;
  white-space: nowrap;
  overflow-x: auto;
}

.filename-text {
  display: inline;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.line-badge {
  background-color: #e6f7ff;
  color: #1890ff;
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 11px;
  margin-left: 6px;
  flex-shrink: 0;
  display: inline-block;
}

.preview-content {
  min-width: 0;
  max-width: 100%;
  width: 100%;
  white-space: pre-wrap;
  word-break: break-all;
  overflow-wrap: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  box-sizing: border-box;
  margin-left: 0px;
  font-size: 13px;
  color: #444;
  line-height: 1.5;
  background: #f8fafc;
  border-radius: 4px;
  padding: 0px 5px;
  position: relative;
}

.search-highlight {
  background: #ffe58f;
  color: #d48806;
  border-radius: 3px;
  padding: 0 2px;
  font-weight: bold;
  box-shadow: 0 1px 0 #fffbe6;
  transition: background 0.2s;
}

/* 动画美化 */
.n-tabs-pane-wrapper {
  transition: opacity 0.3s;
}

.n-tabs-pane-wrapper-enter-active,
.n-tabs-pane-wrapper-leave-active {
  transition: opacity 0.3s;
}

.n-tabs-pane-wrapper-enter-from,
.n-tabs-pane-wrapper-leave-to {
  opacity: 0;
}
</style>