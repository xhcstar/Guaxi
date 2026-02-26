<template>
  <div class="search-navigator" ref="navigatorRef" tabindex="0" @keydown="handleNavigatorKeydown"
    @mousedown="focusNavigator" @click="focusNavigator">
    <n-tabs v-model:value="activeTab" type="line" animated>
      <!-- 文件搜索结果标签页 -->
      <n-tab-pane name="files" :tab="filesTab">
        <div class="tab-content">
          <n-empty v-if="fileResults.length === 0" description="暂无文件搜索结果" style="margin-top: 20px" />
          <n-list v-else hoverable clickable>
            <div class="scroller">
              <n-virtual-list ref="fileListRef" :items="keyedFileResults" :item-size="48" :key-field="'_uniqueKey'"
                class="virtual-list" @scroll="debouncedScroll">
                <template #default="{ item, index }">
                <n-list-item @click="handleResultClick(item, index)">
                  <div class="search-result-item" :class="{ 'is-active': item._uniqueKey === activeResultKey }">
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
                </template>
                <template #footer>
                  <div v-if="searchState.loading" class="loading-more">
                    <n-spin size="small" />
                  </div>
                </template>
              </n-virtual-list>
            </div>
          </n-list>
        </div>
      </n-tab-pane>

      <!-- 内容搜索结果标签页 -->
      <n-tab-pane name="notes" :tab="notesTab">
        <div class="tab-content">
          <div v-if="invalidNoteItems.length > 0" class="error-banner">
            警告：发现 {{ invalidNoteItems.length }} 个无效搜索结果项
            <n-button text @click="showInvalidItems = !showInvalidItems" size="tiny">
              {{ showInvalidItems ? '隐藏详情' : '显示详情' }}
            </n-button>
            <div v-if="showInvalidItems" class="invalid-items-detail">
              <div v-for="(item, index) in invalidNoteItems" :key="index" class="invalid-item">
                <n-text type="error">无效项 {{ index + 1 }}: {{ JSON.stringify(item) }}</n-text>
              </div>
            </div>
          </div>
          <n-empty v-if="noteResults.length === 0" description="暂无内容搜索结果" style="margin-top: 20px" />
          <n-list v-else hoverable clickable>
            <div class="scroller">
              <n-virtual-list ref="noteListRef" :items="keyedNoteResults" :item-size="64" :key-field="'_uniqueKey'"
                class="virtual-list" @scroll="debouncedScroll">
                <template #default="{ item, index }">
                <n-list-item @click="handleResultClick(item, index)">
                  <div class="search-result-item" :class="{ 'is-active': item._uniqueKey === activeResultKey }">
                    <div class="file-name" :title="getFullPath(item)">
                      <IconifyIcon v-bind="getFileIconPropsWithLog(item.path)" />
                      <span class="filename-text">{{ getFileName(item) }}</span>
                      <span class="line-badge">L{{ item.line_number }}</span>
                    </div>
                    <div class="preview-content" v-html="formatPreview(item)"></div>
                  </div>
                </n-list-item>
                </template>
                <template #footer>
                  <div v-if="searchState.loading" class="loading-more">
                    <n-spin size="small" />
                  </div>
                </template>
              </n-virtual-list>
            </div>
          </n-list>
        </div>
      </n-tab-pane>

      <!-- 摘录笔记标签页 -->
      <n-tab-pane name="excerpts" :tab="excerptsTab">
        <div class="tab-content-excerpts">
          <GuaxiNoteExcerpt ref="excerptRef" :excerpt-results="excerptResults" />
        </div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { h, defineComponent, reactive, ref, watch, onMounted, computed, onBeforeUnmount, nextTick } from 'vue';
import { Icon } from '@iconify/vue';
import { debounce } from 'lodash-es';
import {
  NIcon,
  NSpin,
  NTooltip,
  NButton,
  NText,
  NVirtualList
} from 'naive-ui';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { useEditorStore } from '../../stores/editorStore';
import { useRightSiderStore } from '../../stores/rightSiderStore';
import GuaxiNoteExcerpt from './GuaxiNoteExcerpt.vue';

const logger = createLogger('GuaxiSearchNavigator')
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
const rightSiderStore = useRightSiderStore();
const activeTab = ref('files');
const showInvalidItems = ref(false);
const navigatorRef = ref(null);
const fileListRef = ref(null);
const noteListRef = ref(null);
const excerptRef = ref(null);
const activeResultIndex = ref(-1);
const activeResultKey = ref('');

const focusNavigator = (reason = 'focus') => {
  const el = navigatorRef.value
  if (!el) return

  nextTick(() => {
    el.focus?.()
    logger.debug('[GuaxiSearchNavigator] focus navigator', { reason, tab: activeTab.value })
  })
};

const props = defineProps({
  fileResults: {
    type: Array,
    required: true,
    default: () => []
  },
  noteResults: {
    type: Array,
    required: true,
    default: () => []
  },
  excerptResults: {
    type: Array,
    required: true,
    default: () => []
  },
  searchQuery: {
    type: String,
    required: true,
    default: ''
  },
  lastSearchType: {
    type: String,
    required: true,
    default: 'file'
  }
});

// 搜索状态管理
const searchState = reactive({
  loading: false,
  activeSearch: null,
  cancelToken: null,
  batchSize: 20,
  loadedResults: 0,
  hasMore: false
});

// 监听搜索类型变化，自动切换标签页
watch(() => props.lastSearchType, (newType) => {
  if (newType) {
    switch (newType) {
      case 'file':
        if (props.fileResults.length > 0) {
          activeTab.value = 'files';
        }
        break;
      case 'note':
        if (props.noteResults.length > 0) {
          activeTab.value = 'notes';
        }
        break;
      case 'excerpt':
        if (props.excerptResults.length > 0) {
          activeTab.value = 'excerpts';
        }
        break;
    }
  }
});

// 监听搜索结果变化，自动切换到有结果的标签页
watch([() => props.fileResults, () => props.noteResults, () => props.excerptResults], () => {
  // 如果当前标签页已经有结果，则不切换
  if ((activeTab.value === 'files' && props.fileResults.length > 0) ||
    (activeTab.value === 'notes' && props.noteResults.length > 0) ||
    (activeTab.value === 'excerpts' && props.excerptResults.length > 0)) {
    return;
  }

  // 根据搜索类型和结果自动切换
  if (props.lastSearchType === 'file' && props.fileResults.length > 0) {
    activeTab.value = 'files';
  } else if (props.lastSearchType === 'note' && props.noteResults.length > 0) {
    activeTab.value = 'notes';
  } else if (props.lastSearchType === 'excerpt' && props.excerptResults.length > 0) {
    activeTab.value = 'excerpts';
  } else {
    // 默认切换到第一个有结果的标签页
    if (props.fileResults.length > 0) {
      activeTab.value = 'files';
    } else if (props.noteResults.length > 0) {
      activeTab.value = 'notes';
    } else if (props.excerptResults.length > 0) {
      activeTab.value = 'excerpts';
    }
  }
}, { immediate: true });

// 计算属性：过滤无效的笔记结果
const validNoteResults = computed(() => {
  return props.noteResults.filter(item => {
    // 基本验证
    if (!item || typeof item !== 'object') return false;

    // 确保必要字段存在
    const hasPath = item.path && typeof item.path === 'string';
    const hasFileName = item.file_name && typeof item.file_name === 'string';
    const hasContent = (item.preview || item.text) && typeof (item.preview || item.text) === 'string';

    if (!hasPath && !hasFileName) {
      logger.warn('[InvalidItem] Missing path/file_name:', item);
      return false;
    }

    if (!hasContent) {
      logger.warn('[InvalidItem] Missing content:', item);
      return false;
    }

    return true;
  });
});

// 计算属性：获取无效的笔记项
const invalidNoteItems = computed(() => {
  return props.noteResults.filter(item => {
    try {
      const key = getKeyField(item);
      return !key;
    } catch {
      return true;
    }
  });
});

// 滚动事件处理
const handleScroll = (e) => {
  // 这里可以添加滚动时的逻辑
  logger.info('Scrolling:', e);
};

// 使用防抖
const debouncedScroll = debounce(handleScroll, 200);

// 文件图标映射
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
    logger.warn('[n-icon] icon prop missing!', path, props);
    // 返回一个默认的图标
    return {
      icon: FILE_ICON_MAP.default,
      size: 18,
      color: '#666'
    };
  }
  return props;
};

const getFileIconProps = (path) => {
  const ext = path.split('.').pop()?.toLowerCase() || '';
  const isActive = editorStore.activeFile?.path === path;

  if (path.endsWith('/')) {
    const folderIcon = {
      icon: FILE_ICON_MAP.folder,
      color: isActive ? '#3366ff' : '#666'
    };
    logger.info('[IconProps][folder]', path, folderIcon);
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
  // logger.info('[IconProps][file]', path, { ext, finalExt, icon, props });
  return props;
};

/**
 * 格式化预览内容（添加高亮，支持转义后索引映射）
 * @param {Object} item 文件项
 * @returns {string} HTML格式的预览内容
 */
const formatPreview = (item) => {
  if (!item.preview) return '';

  const q = String(props.searchQuery || '').trim()
  const matches = item.matches || [];
  const { escaped, indexMap } = escapeHtmlWithIndexMap(item.preview);

  if (!q) return escaped

  // 1. 先在转义后的字符串插入高亮标签（用映射后的索引）
  let result = escaped;
  // 记录偏移量，插入标签后字符串变长
  let offset = 0;
  [...matches].forEach(match => {
    const start = indexMap[match.start];
    const end = indexMap[match.end];
    if (start === undefined || end === undefined) return;
    result =
      result.slice(0, start + offset) +
      '<span class="search-highlight">' +
      result.slice(start + offset, end + offset) +
      '</span>' +
      result.slice(end + offset);
    offset += '<span class="search-highlight">'.length + '</span>'.length;
  });

  return result;
};

/**
 * 对字符串做HTML转义，并返回原始索引到转义后索引的映射
 * @param {string} str
 * @returns {{escaped: string, indexMap: number[]}}
 */
function escapeHtmlWithIndexMap(str) {
  let escaped = '';
  const indexMap = [];
  for (let i = 0, j = 0; i < str.length; i++) {
    const ch = str[i];
    let esc = '';
    switch (ch) {
      case '&': esc = '&amp;'; break;
      case '<': esc = '&lt;'; break;
      case '>': esc = '&gt;'; break;
      case '"': esc = '&quot;'; break;
      case "'": esc = '&#39;'; break;
      case ' ': esc = '&nbsp;'; break;
      default: esc = ch;
    }
    // 记录原始索引i在转义后字符串中的起始位置j
    indexMap[i] = escaped.length;
    escaped += esc;
    j = escaped.length;
  }
  // 补一个终点
  indexMap[str.length] = escaped.length;
  return { escaped, indexMap };
}

/**
 * 打开搜索结果
 * @param {Object} item 文件项
 */
const openSearchResult = async (item) => {
  try {
    const itemType = item?._searchType || (item?.preview ? 'note' : 'file')
    const q = String(props.searchQuery || '').trim()
    const shouldHighlight = itemType === 'note' && !!q

    logger.debug('[openSearchResult] open', {
      itemType,
      hasQuery: !!q,
      path: item?.path,
      line_number: item?.line_number
    })

    const result = await fileTreeStore.fileTreeOpenSearchResult({
      ...item,
      searchText: shouldHighlight ? q : ''
    })

    if (shouldHighlight) {
      editorStore.highlightInfo = {
        path: result.path,
        lineNumber: result.lineNumber,
        matches: result.matches,
        text: result.text,
      }
    } else {
      editorStore.highlightInfo = null
    }

    editorStore.setFile({
      path: result.path,
      content: result.content || '',
    })
  } catch (err) {
    logger.error('打开搜索结果失败:', err)
  }
}

const handleResultClick = (item, index) => {
  setActiveResultIndex(index)
  openSearchResult(item)
}

// 更健壮的 keyField 生成函数
const getKeyField = (item) => {
  // 防御性编程：确保 item 存在
  if (!item) {
    const fallback = `null_${Math.random().toString(36).substring(2, 9)}`;
    logger.error('[KeyError] Item is null/undefined, using fallback:', fallback);
    return fallback;
  }

  // 调试信息
  // logger.info('[KeyDebug] Generating key for:', {
  //   path: item.path,
  //   file_name: item.file_name,
  //   line_number: item.line_number
  // });

  // 优先使用 path
  if (item.path && typeof item.path === 'string') {
    return item.path;
  }

  // 次选 file_name + line_number
  const fileName = item.file_name || '';
  const lineNumber = item.line_number !== undefined ? item.line_number : '0';
  if (fileName || lineNumber !== '0') {
    return `${fileName}_${lineNumber}`;
  }

  // 尝试其他可能字段
  if (item.id) return `id_${item.id}`;
  if (item.name) return `name_${item.name}`;

  // 最终 fallback
  const fallback = `fb_${Math.random().toString(36).substring(2, 9)}`;
  logger.warn('[KeyFallback] Using fallback key:', fallback, 'for item:', item);
  return fallback;
};

// 添加调试信息
onMounted(() => {
  logger.info('[SearchNavigator] Component mounted with initial props:', {
    fileResults: props.fileResults,
    noteResults: props.noteResults,
    excerptResults: props.excerptResults,
    searchQuery: props.searchQuery
  });

  logger.info('[KeyDebug] Sample key for first file result:',
    props.fileResults[0] ? getKeyField(props.fileResults[0]) : 'No file results');

  logger.info('[KeyDebug] Sample key for first note result:',
    props.noteResults[0] ? getKeyField(props.noteResults[0]) : 'No note results');
});


// 标签页自定义内容
const filesTab = () => h(NTooltip, { placement: 'bottom' }, {
  trigger: () => h('span', { class: 'tab-label' }, [
    h(Icon, { icon: 'mdi:file-search-outline', style: { fontSize: '18px', marginRight: '6px' } }),
    '文件搜索结果'
  ]),
  default: () => '显示文件路径匹配的搜索结果'
});

const notesTab = () => h(NTooltip, { placement: 'bottom' }, {
  trigger: () => h('span', { class: 'tab-label' }, [
    h(Icon, { icon: 'mdi:text-box-search-outline', style: { fontSize: '18px', marginRight: '6px' } }),
    '内容搜索结果'
  ]),
  default: () => '显示文件内容匹配的搜索结果'
});

const excerptsTab = () => h(NTooltip, { placement: 'bottom' }, {
  trigger: () => h('span', { class: 'tab-label' }, [
    h(Icon, { icon: 'mdi:note-search-outline', style: { fontSize: '18px', marginRight: '6px' } }),
    '摘录笔记'
  ]),
  default: () => '显示摘录笔记的搜索结果'
});

// 工具函数
const getFileName = (item) => item.file_name || item.path?.split(/[\\/]/).pop() || '未知文件';
const getFullPath = (item) => item.path?.replace(/\\/g, '/') || '';

// 增强的键生成函数
const generateItemKey = (item, fallbackIndex) => {
  try {
    // 防御性编程
    if (!item) {
      return `null_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    }

    const type = item._searchType || 'unknown'
    const path = String(item.path || '')
    const fileName = String(item.file_name || '')
    const line = Number(item.line_number || 0)
    const excerptId = String(item.excerpt_id || '')
    const idx = Number.isFinite(fallbackIndex) ? fallbackIndex : 0

    // 直接拼接，保证 index 参与唯一性
    const key = `${type}|${path}|${fileName}|${line}|${excerptId}|${idx}`

    return `search_${key}`
  } catch (err) {
    logger.error('[KeyError] Failed to generate key:', err, 'for item:', item);
    return `err_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }
};

// 计算属性增强
const keyedFileResults = computed(() => {
  return rightSiderStore.searchResults.fileResults
    .map((item, index) => ({
      ...item,
      _uniqueKey: generateItemKey(item, index)
    }));
});

const keyedNoteResults = computed(() => {
  return rightSiderStore.searchResults.noteResults
    .filter(item => {
      const isValid = item.path && item.preview;
      if (!isValid) {
        logger.warn('[InvalidItem] Filtered out note result:', item);
      }
      return isValid;
    })
    .map((item, index) => ({
      ...item,
      _uniqueKey: generateItemKey(item, index)
    }));
});

const activeResultList = computed(() => {
  if (activeTab.value === 'files') return keyedFileResults.value
  if (activeTab.value === 'notes') return keyedNoteResults.value
  return []
})

const scrollActiveIntoView = () => {
  const idx = activeResultIndex.value
  if (idx < 0) return

  const listRef = activeTab.value === 'files' ? fileListRef.value : noteListRef.value
  if (!listRef || typeof listRef.scrollTo !== 'function') return
  listRef.scrollTo({ index: idx })
}

const setActiveResultIndex = (index) => {
  const list = activeResultList.value
  if (!list.length) {
    activeResultIndex.value = -1
    activeResultKey.value = ''
    return
  }

  const idx = Number(index)
  if (!Number.isFinite(idx)) return

  const next = Math.max(0, Math.min(list.length - 1, idx))
  activeResultIndex.value = next
  activeResultKey.value = list[next]?._uniqueKey || ''
  logger.debug('[GuaxiSearchNavigator] set active', {
    index: next,
    key: activeResultKey.value
  })
  scrollActiveIntoView()
}

const moveActiveResult = (delta) => {
  const list = activeResultList.value
  if (!list.length) return

  const cur = activeResultIndex.value
  if (cur < 0) {
    const nextIndex = delta >= 0 ? 0 : list.length - 1
    activeResultIndex.value = nextIndex
    activeResultKey.value = list[nextIndex]?._uniqueKey || ''
    logger.debug('[GuaxiSearchNavigator] move active', {
      index: nextIndex,
      key: activeResultKey.value
    })
    scrollActiveIntoView()
    return
  }

  const next = (cur + delta + list.length) % list.length
  activeResultIndex.value = next
  activeResultKey.value = list[next]?._uniqueKey || ''
  logger.debug('[GuaxiSearchNavigator] move active', {
    index: next,
    key: activeResultKey.value
  })
  scrollActiveIntoView()
}

const isActiveResult = (index) => activeResultIndex.value === index

// 判断是否在输入框内，避免抢走输入操作
const isEditableTarget = (target) => {
  const el = target
  if (!el) return false

  const tag = (el.tagName || '').toLowerCase()
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return true
  if (el.isContentEditable) return true
  return false
}

// 仅处理方向键与回车
const isNavigationKey = (key) => key === 'ArrowDown' || key === 'ArrowUp' || key === 'Enter'

const logKeydownSkip = (reason, e) => {
  logger.debug('[GuaxiSearchNavigator] skip keydown', {
    reason,
    key: e?.key,
    tab: activeTab.value,
    isOpen: rightSiderStore.isOpen,
    panel: rightSiderStore.panel
  })
}

const handleNavigatorKeydown = (e) => {
  logger.debug('[GuaxiSearchNavigator] handle keydown', {
    key: e?.key,
    tab: activeTab.value,
    activeIndex: activeResultIndex.value,
    listSize: activeResultList.value?.length || 0
  })
  if (e?.defaultPrevented) {
    logKeydownSkip('defaultPrevented', e)
    return
  }

  if (activeTab.value === 'excerpts') {
    // 摘录结果使用子组件处理键盘事件
    excerptRef.value?.handleKeydown?.(e)
    return
  }

  if (activeTab.value !== 'files' && activeTab.value !== 'notes') {
    logKeydownSkip('tab-not-supported', e)
    return
  }

  const key = e?.key
  if (key === 'ArrowDown') {
    e.preventDefault()
    moveActiveResult(1)
    return
  }

  if (key === 'ArrowUp') {
    e.preventDefault()
    moveActiveResult(-1)
    return
  }

  if (key === 'Enter') {
    const list = activeResultList.value
    if (!list.length) return

    e.preventDefault()

    if (activeResultIndex.value < 0) {
      setActiveResultIndex(0)
      const first = list[0]
      logger.debug('[GuaxiSearchNavigator] enter open', {
        index: 0,
        key: first?._uniqueKey
      })
      openSearchResult(first)
      return
    }

    const item = list[activeResultIndex.value]
    logger.debug('[GuaxiSearchNavigator] enter open', {
      index: activeResultIndex.value,
      key: item?._uniqueKey
    })
    if (item) openSearchResult(item)
  }
}

// 全局键盘事件转发，保证面板未聚焦也可操作
const handleGlobalKeydown = (e) => {
  const key = e?.key
  if (!isNavigationKey(key)) return
  if (!rightSiderStore.isOpen) {
    logKeydownSkip('panel-closed', e)
    return
  }
  if (rightSiderStore.panel && rightSiderStore.panel !== 'search') {
    logKeydownSkip('panel-not-search', e)
    return
  }
  if (isEditableTarget(e?.target)) {
    logKeydownSkip('editable-target', e)
    return
  }

  logger.debug('[GuaxiSearchNavigator] global keydown', {
    key,
    tab: activeTab.value,
    isOpen: rightSiderStore.isOpen,
    panel: rightSiderStore.panel
  })

  handleNavigatorKeydown(e)
}

const shouldAutoFocus = computed(() => {
  return rightSiderStore.isOpen && rightSiderStore.panel === 'search'
})

watch([activeTab, activeResultList], () => {
  const list = activeResultList.value
  if (!list.length) {
    activeResultIndex.value = -1
    activeResultKey.value = ''
    return
  }

  if (activeResultIndex.value < 0 || activeResultIndex.value >= list.length) {
    activeResultIndex.value = 0
  }

  activeResultKey.value = list[activeResultIndex.value]?._uniqueKey || ''
  logger.debug('[GuaxiSearchNavigator] sync active', {
    index: activeResultIndex.value,
    key: activeResultKey.value
  })
  scrollActiveIntoView()

  if (shouldAutoFocus.value) {
    focusNavigator('active-tab')
  }
})

watch(shouldAutoFocus, (next) => {
  if (next) focusNavigator('panel-open')
})

// 确保搜索结果数据有效
watch(() => props.fileResults, (newVal) => {
  if (newVal.some(item => !item.path)) {
    logger.warn('发现无效的搜索结果项:', newVal.filter(item => !item.path));
  }
}, { immediate: true });

watch(() => props.noteResults, (newVal) => {
  if (newVal.some(item => !item.path)) {
    logger.warn('发现无效的笔记结果项:', newVal.filter(item => !item.path));
  }
}, { immediate: true });

onMounted(() => {
  if (validNoteResults.value.length > 0) {
    activeTab.value = 'notes';
  } else if (props.fileResults.length > 0) {
    activeTab.value = 'files';
  } else if (props.excerptResults.length > 0) {
    activeTab.value = 'excerpts';
  }

  // 注册全局键盘监听，保证方向键可用
  window.addEventListener('keydown', handleGlobalKeydown, true)
  logger.debug('[GuaxiSearchNavigator] global keydown listener registered', { capture: true })
});

// 确保组件卸载时取消防抖
onBeforeUnmount(() => {
  debouncedScroll.cancel();
  window.removeEventListener('keydown', handleGlobalKeydown, true)
  logger.debug('[GuaxiSearchNavigator] global keydown listener removed')
});
</script>

<style scoped>
.search-navigator {
  height: 100%;
  width: 100%;
  min-width: 0;
  max-width: 490px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0;
  box-sizing: border-box;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.n-tabs {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.n-tabs .n-tabs-tab {
  font-size: 15px;
  font-weight: 500;
  padding: 0 18px;
  transition: color 0.2s, background 0.2s;
  border-radius: 8px 8px 0 0;
  min-height: 40px;
  display: flex;
  align-items: center;
}

.n-tabs .n-tabs-tab--active {
  color: var(--n-color-primary, #1890ff);
  background: var(--n-color-primary-suppl, var(--guaxi-header-bg));
  box-shadow: 0 2px 8px 0 rgba(24, 144, 255, 0.08);
}

.n-tabs .n-tabs-tab:hover {
  background: var(--guaxi-header-bg);
  color: var(--n-color-primary, #1890ff);
}

.tab-label {
  display: flex;
  align-items: center;
  gap: 4px;
}

:deep(.n-tabs) {
  height: 100%;
  display: flex;
  flex-direction: column;
}

:deep(.n-tabs-content) {
  flex: 1 1 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: 0 !important;
  margin: 0 !important;
}

:deep(.n-tab-pane) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 !important;
  margin: 0 !important;
}

.tab-content {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  overflow-x: hidden;
  padding: 0 8px 5px 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}

.tab-content-excerpts {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  padding: 0 8px 5px 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}

.tab-content-excerpts :deep(.n-list),
.tab-content-excerpts :deep(.n-list-item),
.tab-content-excerpts :deep(.excerpt-result-item),
.tab-content-excerpts :deep(.file-content),
.tab-content-excerpts :deep(.card-container),
.tab-content-excerpts :deep(.excerpt-card),
.tab-content-excerpts :deep(.n-card) {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  box-sizing: border-box;
}

.tab-content-excerpts :deep(.n-list-item),
.tab-content-excerpts :deep(.excerpt-result-item),
.tab-content-excerpts :deep(.file-content) {
  overflow: hidden;
}

.tab-content-excerpts :deep(.card-item-rendered),
.tab-content-excerpts :deep(.card-item-rendered *),
.tab-content-excerpts :deep(.n-card__content),
.tab-content-excerpts :deep(.n-card__content *) {
  max-width: 100%;
  box-sizing: border-box;
}

.tab-content-excerpts :deep(.card-item-rendered pre) {
  max-width: 100%;
  overflow: auto;
}

.tab-content-excerpts :deep(.card-item-rendered img) {
  max-width: 100%;
  height: auto;
}
.scroller {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  overflow: hidden;
}

.virtual-list {
  width: 100%;
  height: 100%;
  overflow-x: hidden;
}

:deep(.n-virtual-list) {
  overflow-x: hidden !important;
  overscroll-behavior: contain;
}

:deep(.n-virtual-list__content) {
  width: 100% !important;
  overflow-x: hidden;
}

:deep(.n-virtual-list__content > *) {
  width: 100% !important;
}

:deep(.n-virtual-list-item) {
  box-sizing: border-box;
  min-width: 0;
  overflow: hidden;
}

:deep(.n-virtual-list) {
  width: 100%;
  height: 100%;
}

:deep(.n-virtual-list-item) {
  width: 100%;
}

/* 确保虚拟列表项正确显示 */
:deep(.n-virtual-list) {
  width: 100%;
}

:deep(.n-virtual-list-item) {
  width: 100%;
}

.loading-more {
  display: flex;
  justify-content: center;
  padding: 8px;
}

.tab-content :deep(.n-list) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0;
  margin: 0;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  overflow-x: hidden;
}

.tab-content :deep(.n-list__content) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tab-content :deep(.n-list-item) {
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  overflow: hidden;
  margin: 0 !important;
  padding: 2px 4px !important;
  transition: all 0.2s ease;
}

.n-list-item:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.search-result-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  border-radius: 6px;
  box-shadow: none;
  padding: 8px;
  margin-bottom: 2px;
  margin-top: 5px;
  transition: all 0.2s ease;
  position: relative;
}

.search-result-item.is-active {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.3),
    0 2px 12px 0 rgba(7, 118, 161, 0.15);
}

.n-list-item:hover .search-result-item {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.3),
    0 2px 12px 0 rgba(7, 118, 161, 0.15);
}

.file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 500;
  color: var(--guaxi-text-primary);
  width: 100%;
  cursor: pointer;
  min-width: 0;
  overflow: hidden;
  box-sizing: border-box;
  transition: color 0.2s;
}

.n-list-item:hover .file-name {
  color: var(--n-color-primary, #1890ff);
}

.filename-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.line-badge {
  background-color: var(--n-color-primary-suppl, rgba(24, 144, 255, 0.12));
  color: var(--n-color-primary, #1890ff);
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 11px;
  margin-left: auto;
  flex-shrink: 0;
  display: inline-block;
}

.preview-content {
  min-width: 0;
  max-width: 100%;
  width: 100%;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  box-sizing: border-box;
  margin-left: 0px;
  font-size: 13px;
  color: var(--guaxi-text-secondary);
  line-height: 1.5;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 4px;
  padding: 8px;
  max-height: calc(3 * 1.5em + 16px);
  position: relative;
  text-overflow: ellipsis;
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

:deep(.n-tab-pane) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

:deep(.n-tabs-pane-wrapper) {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0 !important;
  margin: 0 !important;
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

.error-banner {
  background: rgba(245, 34, 45, 0.12);
  padding: 8px 12px;
  margin: 8px 0;
  color: #f5222d;
  border: 1px solid rgba(245, 34, 45, 0.35);
  border-radius: 4px;
  font-size: 13px;
}

.invalid-items-detail {
  margin-top: 8px;
  padding: 8px;
  background: rgba(245, 34, 45, 0.08);
  border: 1px dashed rgba(245, 34, 45, 0.35);
  border-radius: 4px;
}

.invalid-item {
  padding: 4px 0;
  font-size: 12px;
  word-break: break-all;
}
</style>