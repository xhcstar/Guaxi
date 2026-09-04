<template>
  <div class="search-navigator" ref="navigatorRef" tabindex="0" @keydown="handleNavigatorKeydown"
    @mousedown="focusNavigator" @click="focusNavigator">
    <div v-if="loading" class="search-panel-status search-panel-status--loading">
      <n-spin size="small" />
      <span>正在扫描并整理完整搜索结果…</span>
    </div>
    <div v-else-if="error" class="search-panel-status search-panel-status--error">
      {{ error }}
    </div>
    <n-tabs v-model:value="activeTab" type="line" animated>
      <!-- 文件搜索结果标签页 -->
      <n-tab-pane name="files" :tab="filesTab">
        <div class="tab-content">
          <div v-if="fileResults.length > 0" class="result-summary">
            <span>共找到 {{ fileResults.length }} 个文件</span>
          </div>
          <n-empty v-if="fileResults.length === 0" description="暂无文件搜索结果" style="margin-top: 20px" />
          <!--
            文件结果和内容结果的组头是同一种东西（一个文件一行），所以用同一套 .sr-row 类：
            以前这里是每条一张 48px 的卡片，带边框、圆角、悬停位移和外发光，
            和内容页的密排行、摘录页的卡片各是一套观感。行高统一后同屏能多看近一倍的结果。
          -->
          <div v-else class="scroller">
            <n-virtual-list ref="fileListRef" :items="keyedFileResults" :item-size="SR_ROW_HEIGHT"
              :key-field="'_uniqueKey'" class="virtual-list" @scroll="debouncedScroll">
              <template #default="{ item, index }">
                <div class="sr-row sr-row--group" :class="{ 'is-active': item._uniqueKey === activeResultKey }"
                  :title="getFullPath(item)" @click="handleResultClick(item, index)">
                  <IconifyIcon v-bind="getFileIconPropsWithLog(item.path)" :size="16" />
                  <span class="sr-name" v-html="formatFileName(item)"></span>
                  <!-- 查询可能是靠目录名中的，光给文件名会让人看不懂这条为什么会出现 -->
                  <span v-if="getRelDir(item)" class="sr-dir">{{ getRelDir(item) }}</span>
                </div>
              </template>
              <template #footer>
                <div v-if="loading" class="loading-more">
                  <n-spin size="small" />
                </div>
              </template>
            </n-virtual-list>
          </div>
        </div>
      </n-tab-pane>

      <!-- 内容搜索结果标签页 -->
      <n-tab-pane name="notes" :tab="notesTab">
        <div class="tab-content">
          <div v-if="noteRows.length > 0" class="result-summary">
            <span>共找到 {{ noteResults.length }} 处匹配，涉及 {{ uniqueNoteFileCount }} 个文件</span>
            <n-button text size="tiny" class="summary-action" @click="toggleAllNoteGroups">
              {{ allNoteGroupsCollapsed ? '全部展开' : '全部折叠' }}
            </n-button>
          </div>
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
          <n-empty v-if="noteRows.length === 0" description="暂无内容搜索结果" style="margin-top: 20px" />
          <!--
            内容结果按 VSCode 的路子排：一个文件一行组头，命中逐行排在它下面，一行一条、不换行。
            以前每条命中都是一张带边框的卡片，预览用 -webkit-line-clamp 掐 3 行，
            长行（代码、表格、压成一行的 JSON）里的高亮往往落在第 4 行开外，
            结果是"搜到了但看不见搜到了什么"。现在每条命中只占一行，并且预览窗口会跟着第一处高亮平移。
          -->
          <div v-else class="scroller">
            <n-virtual-list ref="noteListRef" :items="noteRows" :item-size="SR_ROW_HEIGHT"
              :key-field="'_uniqueKey'" class="virtual-list" @scroll="debouncedScroll">
              <template #default="{ item: row, index }">
                <div v-if="row.kind === 'file'" class="sr-row sr-row--group"
                  :class="{ 'is-active': row._uniqueKey === activeResultKey }" :title="getFullPath(row.item)"
                  @click="handleNoteRowClick(row, index)">
                  <Icon class="sr-caret" :class="{ 'is-collapsed': isNoteGroupCollapsed(row.groupKey) }"
                    :icon="lucideChevronDown" :width="14" :height="14" />
                  <IconifyIcon v-bind="getFileIconPropsWithLog(row.item.path)" :size="16" />
                  <span class="sr-name">{{ getFileName(row.item) }}</span>
                  <span v-if="getRelDir(row.item)" class="sr-dir">{{ getRelDir(row.item) }}</span>
                  <span class="sr-count">{{ row.count }}</span>
                </div>
                <div v-else class="sr-row sr-row--match"
                  :class="{ 'is-active': row._uniqueKey === activeResultKey }" :title="matchRowTitle(row.item)"
                  @click="handleNoteRowClick(row, index)">
                  <span class="sr-line-no">{{ row.item.line_number }}</span>
                  <span class="sr-line-text">
                    <span v-if="previewOf(row.item).leadCut" class="sr-cut">…</span><span
                      v-for="(seg, i) in previewOf(row.item).segments" :key="i"
                      :class="seg.hit ? 'search-highlight' : null">{{ seg.text }}</span><span
                      v-if="previewOf(row.item).tailCut" class="sr-cut">…</span>
                  </span>
                </div>
              </template>
              <template #footer>
                <div v-if="loading" class="loading-more">
                  <n-spin size="small" />
                </div>
              </template>
            </n-virtual-list>
          </div>
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
import { h, defineComponent, ref, watch, onMounted, computed, onBeforeUnmount, nextTick } from 'vue';
import { Icon } from '@iconify/vue';
// 标签图标直接 import 图标数据对象，不传 'lucide:xxx' 这种名字。
// 名字要靠 src/main.js 里的 addIcon 提前注册，一旦漏注册，<Icon> 会去
// api.iconify.design 现取 —— release 包在断网、内网、CSP 收紧的机器上就是三个空白。
// 传对象走的是 Icon 的 object 分支：图标数据被 Vite 打进 bundle，完全不碰网络，
// 名字写错也不用等运行时，构建期就报"模块找不到"。
import lucideFileSearch from '@iconify-icons/lucide/file-search';
import lucideBookOpenText from '@iconify-icons/lucide/book-open-text';
import lucideHighlighter from '@iconify-icons/lucide/highlighter';
import lucideChevronDown from '@iconify-icons/lucide/chevron-down';
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
import { buildPreviewSegments } from '../../utils/searchPreview';

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
let navigatorResizeObserver = null;

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
  },
  loading: {
    type: Boolean,
    default: false
  },
  error: {
    type: String,
    default: ''
  }
});

// 内容搜索按“命中行”计数，同时额外统计去重文件数，让界面如实说明结果覆盖范围。
const uniqueNoteFileCount = computed(() => new Set(
  props.noteResults
    .map(item => String(item?.path || '').replace(/\\/g, '/').toLowerCase())
    .filter(Boolean)
).size)

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


/*
 * 标签页。
 *
 * 三个图标一律取自 lucide（同一套描边风格，粗细一致），并且各自说清"拿什么去比"：
 * 文件名是纸页配放大镜、正文是摊开的书、摘录是荧光笔。之前那组纸页 / 横线 / 引号
 * 表意含糊——光秃秃的几行横线和一个引号更像排版工具栏的按钮，跟"搜什么"没关系。
 * 三个字形的可见范围都以 y=12 为中心（1~23 / 2~22 / 3~21），所以并排时看着是一条线上的。
 * 图标数据是 import 进来的对象，不走名字注册、不碰 Iconify 在线接口（见文件头注释）。
 *
 * 只有当前标签带文字，另外两个收成图标 + 数字。右侧栏窄的时候三个完整标题
 * （"文件搜索结果（88）"）排不下，第三个会被面板边缘切掉——收起来才都装得下，
 * 完整标题和快捷键放进本来就有的 tooltip。
 */
/*
 * 三个图标的画框大小是逐个定的，不是统一 20px。
 * 用 getBBox 量过（含 2px 描边，24 单位画布）：file-search 的墨迹高 22 单位、
 * book-open-text 20、highlighter 18——统一画框的话高亮笔会明显比另外两个小一圈。
 * 但完全按墨迹拉平（20 / 22 / 24.4）又会让描边粗细差出 22%，高亮笔反而显得更黑。
 * 所以取一半补偿：20 / 21 / 22，墨迹和描边的差都压到 10% 以内，两边都看不出来。
 * 三个字形的垂直中心都在 y=12，所以不需要逐个上下微调。
 */
const TAB_ICON_SIZE = 20

const TAB_DEFS = [
  {
    name: 'files',
    icon: lucideFileSearch,
    size: TAB_ICON_SIZE,
    short: '文件',
    title: '文件名与路径匹配',
    hotkey: 'Alt+1',
    count: () => props.fileResults.length
  },
  {
    name: 'notes',
    icon: lucideBookOpenText,
    size: TAB_ICON_SIZE + 1,
    short: '内容',
    title: '文件内容匹配',
    hotkey: 'Alt+2',
    count: () => props.noteResults.length
  },
  {
    name: 'excerpts',
    icon: lucideHighlighter,
    size: TAB_ICON_SIZE + 2,
    short: '摘录',
    title: '摘录笔记匹配',
    hotkey: 'Alt+3',
    count: () => props.excerptResults.length
  }
]

const renderTab = (def) => () => h(NTooltip, { placement: 'bottom' }, {
  trigger: () => {
    const active = activeTab.value === def.name
    return h(
      'span',
      {
        class: ['tab-label', active ? 'tab-label--active' : null],
        // 收起文字后屏幕阅读器只剩一个图标和数字，标题得自己补上
        'aria-label': `${def.title}，${def.count()} 项`
      },
      [
        // 尺寸写在这里而不是只交给 CSS：renderTab 是 n-tab-pane 的 tab prop，
        // 元素在 naive-ui 的渲染上下文里造出来，不带本组件的 scope 属性
        //（样式那边靠 :deep() 兜住，见 .tab-label 注释）。
        h(Icon, { icon: def.icon, class: 'tab-icon', width: def.size, height: def.size }),
        active ? h('span', { class: 'tab-short' }, def.short) : null,
        h('span', { class: 'tab-count' }, String(def.count()))
      ]
    )
  },
  // tooltip 内容被 naive-ui teleport 到 body，scoped 样式和 :deep() 都够不着，只能写行内样式
  default: () => h('div', { class: 'tab-tip' }, [
    h('div', null, `${def.title}（${def.count()}）`),
    h('div', { style: 'margin-top:2px;font-size:11px;opacity:.72' }, `${def.hotkey} 直接切过来，← → 换标签`)
  ])
})

const filesTab = renderTab(TAB_DEFS[0])
const notesTab = renderTab(TAB_DEFS[1])
const excerptsTab = renderTab(TAB_DEFS[2])

// 工具函数
const getFileName = (item) => item.file_name || item.path?.split(/[\\/]/).pop() || '未知文件';
const getFullPath = (item) => item.path?.replace(/\\/g, '/') || '';

/**
 * 相对搜索根的目录部分。太深就掐掉中间几层——尾部离文件最近，信息量最大，
 * 而完整路径本来就在这一行的 title 里。
 * @param {Object} item
 * @returns {string}
 */
const getRelDir = (item) => {
  const rel = String(item?.rel_path || '').replace(/\\/g, '/')
  const cut = rel.lastIndexOf('/')
  if (cut <= 0) return ''
  const dir = rel.slice(0, cut)
  const segs = dir.split('/').filter(Boolean)
  return segs.length > 3 ? `…/${segs.slice(-2).join('/')}` : dir
}

/**
 * 后端给的是字符下标，JS 的字符串下标是 UTF-16 码元。汉字两者一致，
 * 表情之类的代理对会错位，所以只在真出现代理对时才建这张换算表。
 * @param {string} str
 * @returns {number[]|null} null 表示两种下标等价
 */
const charToUnitIndex = (str) => {
  const chars = Array.from(str)
  if (chars.length === str.length) return null
  const map = []
  let unit = 0
  for (const ch of chars) {
    map.push(unit)
    unit += ch.length
  }
  map.push(unit)
  return map
}

/**
 * 文件名高亮。用的是后端 name_matches（模糊匹配落在文件名上的字符区间），
 * 和 preview 那套 matches 不是同一个坐标系，不能混用。
 * @param {Object} item
 * @returns {string} 已转义的 HTML
 */
const formatFileName = (item) => {
  const name = getFileName(item)
  const { escaped, indexMap } = escapeHtmlWithIndexMap(name)
  const ranges = (Array.isArray(item?.name_matches) ? item.name_matches : [])
    .filter(r => r && Number.isFinite(r.start) && Number.isFinite(r.end) && r.end > r.start)
    .sort((a, b) => a.start - b.start)
  if (!ranges.length) return escaped

  const charMap = charToUnitIndex(name)
  const unitAt = (i) => (charMap ? charMap[i] : i)
  const OPEN = '<span class="search-highlight">'
  const CLOSE = '</span>'
  let result = escaped
  let offset = 0
  let consumed = 0
  for (const r of ranges) {
    // 区间理应已经合并过，重叠的直接跳过，免得插出嵌套标签
    if (r.start < consumed) continue
    const start = indexMap[unitAt(r.start)]
    const end = indexMap[unitAt(r.end)]
    if (start === undefined || end === undefined) continue
    result =
      result.slice(0, start + offset) +
      OPEN +
      result.slice(start + offset, end + offset) +
      CLOSE +
      result.slice(end + offset)
    offset += OPEN.length + CLOSE.length
    consumed = r.end
  }
  return result
};

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
  if (activeTab.value === 'notes') return noteRows.value
  return []
})

/*
 * ===== 内容结果：按文件分组 + 一行一条命中 =====
 *
 * 后端返回的是"每个命中行一条记录"，而且刻意跨文件交错过（interleave_content_results），
 * 免得一个高频文件把前几十条全占了。交错的代价是同一个文件的命中散落在整个数组里，
 * 所以这里要重新聚成组：组的先后沿用文件在数组里第一次出现的位置（等于后端给的文件排名），
 * 组内按行号升序（读代码/笔记的人期望的是文档顺序，不是相关度顺序）。
 */

/**
 * 三个结果列表共用的行高。虚拟列表按固定行高算偏移，JS 里的数和 CSS 里的高度差一个像素，
 * 越往下滚越偏（上一版 :item-size="64" 配更高的卡片，看着就是"内容被截断了"）。
 * 所以不再手抄两遍：CSS 那边用 v-bind(SR_ROW_CSS_HEIGHT) 直接引这里的值。
 */
const SR_ROW_HEIGHT = 28
const SR_ROW_CSS_HEIGHT = `${SR_ROW_HEIGHT}px`

const collapsedNoteGroups = ref(new Set())

const normalizePathKey = (path) => String(path || '').replace(/\\/g, '/').toLowerCase()

const noteGroups = computed(() => {
  const groups = new Map()
  for (const item of keyedNoteResults.value) {
    const key = normalizePathKey(item.path)
    let group = groups.get(key)
    if (!group) {
      group = { key, item, matches: [] }
      groups.set(key, group)
    }
    group.matches.push(item)
  }
  for (const group of groups.values()) {
    group.matches.sort((a, b) => (a.line_number || 0) - (b.line_number || 0))
    group.item = group.matches[0]
  }
  return [...groups.values()]
})

/*
 * 拍平成一维行数组：两种行高度相同，虚拟列表继续用固定 itemSize，不必开 itemResizable。
 * 折叠的组只留组头。
 */
const noteRows = computed(() => {
  const rows = []
  for (const group of noteGroups.value) {
    rows.push({
      kind: 'file',
      groupKey: group.key,
      item: group.item,
      count: group.matches.length,
      _uniqueKey: `notegroup|${group.key}`
    })
    if (collapsedNoteGroups.value.has(group.key)) continue
    for (const match of group.matches) {
      rows.push({
        kind: 'match',
        groupKey: group.key,
        item: match,
        _uniqueKey: match._uniqueKey
      })
    }
  }
  return rows
})

const isNoteGroupCollapsed = (key) => collapsedNoteGroups.value.has(key)

const allNoteGroupsCollapsed = computed(() =>
  noteGroups.value.length > 0 && collapsedNoteGroups.value.size >= noteGroups.value.length)

/** 折叠状态变了行数就变了，选中项按 key 重新对回去，免得高亮跳到别的行上。 */
const resyncActiveRowByKey = () => {
  const key = activeResultKey.value
  if (!key) return
  const idx = noteRows.value.findIndex(row => row._uniqueKey === key)
  if (idx >= 0) activeResultIndex.value = idx
}

const toggleNoteGroup = (key) => {
  const next = new Set(collapsedNoteGroups.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  collapsedNoteGroups.value = next
  resyncActiveRowByKey()
}

const toggleAllNoteGroups = () => {
  collapsedNoteGroups.value = allNoteGroupsCollapsed.value
    ? new Set()
    : new Set(noteGroups.value.map(group => group.key))
  resyncActiveRowByKey()
}

// 换一次搜索就重新全展开：上一轮折了哪些文件对新结果没有意义
watch(() => rightSiderStore.searchResults.noteResults, () => {
  if (collapsedNoteGroups.value.size) collapsedNoteGroups.value = new Set()
})

/*
 * 结果项是 store 里的稳定对象，切片结果可以挂在 WeakMap 上：
 * 模板里一行要读三次（leadCut / segments / tailCut），滚动时又会反复渲染同一批行，
 * 每次都重算 Array.from 是白费。搜索结果一换，旧对象没人引用，缓存自己就散了。
 */
const previewCache = new WeakMap()

const previewOf = (item) => {
  if (!item) return { segments: [], leadCut: false, tailCut: false, full: '' }
  let cached = previewCache.get(item)
  if (!cached) {
    cached = buildPreviewSegments(item)
    previewCache.set(item, cached)
  }
  return cached
}

const matchRowTitle = (item) => `第 ${item?.line_number || 0} 行：${previewOf(item).full}`

/** 组头点了是折叠/展开（跟 VSCode 一致），命中行点了才跳过去。 */
const handleNoteRowClick = (row, index) => {
  setActiveResultIndex(index)
  if (row?.kind === 'file') toggleNoteGroup(row.groupKey)
  else if (row?.item) openSearchResult(row.item)
}

/**
 * 回车落在当前选中项上。文件页的列表项就是结果本身，内容页的是行对象（组头 / 命中行两种）。
 * @param {Object} entry activeResultList 里的一项
 */
const activateListEntry = (entry) => {
  if (!entry) return
  if (entry.kind === 'file') {
    toggleNoteGroup(entry.groupKey)
    return
  }
  openSearchResult(entry.kind === 'match' ? entry.item : entry)
}

const scrollActiveIntoView = () => {
  const idx = activeResultIndex.value
  if (idx < 0) return

  const listRef = activeTab.value === 'files' ? fileListRef.value : noteListRef.value
  if (!listRef || typeof listRef.scrollTo !== 'function') return
  listRef.scrollTo({ index: idx })
}

// 设计思路：
// 1. 搜索面板放在可拖动的右侧栏里，宽度变化时不能再依赖初始 490px 布局。
// 2. 这里监听根节点尺寸变化，一方面记录实际适配宽度，另一方面在下一帧轻触虚拟列表滚动位置，
//    让结果列表在侧栏拉宽后重新按新的容器宽度排布。
const handleNavigatorResize = (entries = []) => {
  const entry = entries[0]
  const width = Math.round(entry?.contentRect?.width || navigatorRef.value?.clientWidth || 0)
  const height = Math.round(entry?.contentRect?.height || navigatorRef.value?.clientHeight || 0)
  logger.debug('[GuaxiSearchNavigator] resize', {
    width,
    height,
    activeTab: activeTab.value,
  })

  window.requestAnimationFrame(() => {
    scrollActiveIntoView()
  })
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

/** Alt+1/2/3 → 标签名。Alt 而不是 Ctrl：Ctrl+1 在几个阅读器里已经有主了。 */
const TAB_HOTKEYS = TAB_DEFS.reduce((acc, def, i) => {
  acc[String(i + 1)] = def.name
  return acc
}, {})

const TAB_ORDER = TAB_DEFS.map((def) => def.name)

/**
 * 这一下按键是不是该由搜索面板接手。
 *
 * Alt 组合只认 Alt+1/2/3；Ctrl / Cmd 一概不碰，那是应用级快捷键的地盘。
 * @param {KeyboardEvent} e
 * @returns {boolean}
 */
const isNavigationEvent = (e) => {
  const key = e?.key
  if (e?.ctrlKey || e?.metaKey) return false
  if (e?.altKey) return Object.prototype.hasOwnProperty.call(TAB_HOTKEYS, key)
  return isNavigationKey(key) || key === 'ArrowLeft' || key === 'ArrowRight'
}

/**
 * 切换标签页。
 * @param {string} name
 * @param {string} reason 只用于日志
 */
const switchTab = (name, reason) => {
  if (!name || name === activeTab.value) return
  logger.debug('[GuaxiSearchNavigator] switch tab', {
    from: activeTab.value,
    to: name,
    reason
  })
  activeTab.value = name
}

/** 切到相邻标签，两头循环。 */
const moveActiveTab = (delta) => {
  const cur = Math.max(0, TAB_ORDER.indexOf(activeTab.value))
  switchTab(TAB_ORDER[(cur + delta + TAB_ORDER.length) % TAB_ORDER.length], 'arrow')
}

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

  // 标签切换必须排在最前面：摘录页会把按键整个转交给子组件，
  // 放到后面的话进了摘录页就再也用键盘切不出来了。
  if (e?.ctrlKey || e?.metaKey) {
    logKeydownSkip('modifier-reserved', e)
    return
  }

  if (e?.altKey) {
    const target = TAB_HOTKEYS[e.key]
    if (target) {
      e.preventDefault()
      switchTab(target, 'hotkey')
    }
    return
  }

  if (e?.key === 'ArrowLeft' || e?.key === 'ArrowRight') {
    e.preventDefault()
    moveActiveTab(e.key === 'ArrowRight' ? 1 : -1)
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
      activateListEntry(first)
      return
    }

    const item = list[activeResultIndex.value]
    logger.debug('[GuaxiSearchNavigator] enter open', {
      index: activeResultIndex.value,
      key: item?._uniqueKey
    })
    if (item) activateListEntry(item)
  }
}

// 全局键盘事件转发，保证面板未聚焦也可操作
const handleGlobalKeydown = (e) => {
  const key = e?.key
  if (!isNavigationEvent(e)) return
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
  if (navigatorRef.value && typeof ResizeObserver !== 'undefined') {
    navigatorResizeObserver = new ResizeObserver(handleNavigatorResize)
    navigatorResizeObserver.observe(navigatorRef.value)
  }
  logger.debug('[GuaxiSearchNavigator] global keydown listener registered', { capture: true })
});

// 确保组件卸载时取消防抖
onBeforeUnmount(() => {
  debouncedScroll.cancel();
  window.removeEventListener('keydown', handleGlobalKeydown, true)
  navigatorResizeObserver?.disconnect?.()
  navigatorResizeObserver = null
  logger.debug('[GuaxiSearchNavigator] global keydown listener removed')
});
</script>

<style scoped>
.search-navigator {
  height: 100%;
  width: 100%;
  min-width: 0;
  max-width: none;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0;
  box-sizing: border-box;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

/*
 * 面板整体可聚焦（方向键要在这里收）。鼠标点进来不画框——那圈白边在暗色主题下很刺眼；
 * 只有键盘聚焦时给一道内描边，否则用户不知道此刻方向键听谁的。
 */
.search-navigator:focus {
  outline: none;
  box-shadow: none;
}

.search-navigator:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 1px rgba(24, 144, 255, 0.35);
}

/*
 * 搜索状态放在标签页上方，明确区分“仍在扫描”和“扫描完成但没有结果”。
 * 这能避免完整搜索期间短暂的空数组被误解为真正的零结果。
 */
.search-panel-status {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--guaxi-border);
  font-size: 12px;
}

.search-panel-status--loading {
  color: var(--guaxi-text-secondary, #8c8c8c);
  background: var(--guaxi-header-bg);
}

.search-panel-status--error {
  color: #f5222d;
  background: rgba(245, 34, 45, 0.1);
}

/* 设计思路：
 * 1. 搜索面板打开后会自动聚焦根节点，浏览器默认 focus outline 在暗色主题下会显成一圈白边。
 * 2. Naive UI Tabs 的导航区也带有默认边界色，这里统一接入 Guaxi 的面板/边框变量，让首次打开和拖拽后都贴合主题。
 */
:deep(.n-tabs-nav) {
  background: var(--guaxi-panel-bg);
  border-color: var(--guaxi-border) !important;
}

:deep(.n-tabs-nav-scroll-wrapper) {
  border-color: var(--guaxi-border) !important;
}

:deep(.n-tabs-tab) {
  outline: none;
}

:deep(.n-tabs-bar) {
  background-color: var(--n-color-primary, #1890ff) !important;
}

.n-tabs {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  min-width: 0;
  overflow: hidden;
}

/* 标签栏：图标 + 数字排一行，40px 高。
   这几条以前写成 `.n-tabs .n-tabs-tab`，但 scoped 样式加不到子组件内部的元素上，
   等于一直没生效——所以统一走 :deep()。 */
:deep(.n-tabs-tab) {
  padding: 0 14px;
  min-height: 40px;
  font-size: 13px;
  font-weight: 500;
  color: var(--guaxi-text-secondary, #8c8c8c);
  transition: color 0.15s ease, background 0.15s ease;
}

:deep(.n-tabs-tab:hover) {
  color: var(--guaxi-text-primary);
  background: rgba(128, 128, 128, 0.08);
}

:deep(.n-tabs-tab--active) {
  color: var(--n-color-primary, #1890ff);
  /* 不做卡片式底色和阴影：底部那条 2px 的蓝线已经说清楚当前是哪个了 */
  background: transparent;
}

/* 标签能用键盘切了，焦点就必须看得见 */
:deep(.n-tabs-tab:focus-visible) {
  outline: 2px solid rgba(24, 144, 255, 0.55);
  outline-offset: -2px;
  border-radius: 4px;
}

/* 下面四条也必须是 :deep()。
   renderTab 是当成 n-tab-pane 的 `tab` **prop** 传下去的（:tab="filesTab"），不是本组件的插槽。
   Vue 的 currentScopeId 跟着"当前正在渲染的实例"走，只有 withCtx 包过的插槽才会切回属主；
   prop 里的裸函数是在 naive-ui 自己 render 的时候被调用的，于是这些元素上没有 data-v-xxx，
   普通 scoped 选择器一条都命不中——上一版把图标改到 18px、gap 提到 9px 看不出变化就是这个原因。
   :deep() 编译成 `[data-v-xxx] .tab-label`，scope 属性落在本组件根元素上，一定命中。 */
:deep(.tab-label) {
  display: flex;
  align-items: center;
  /* 图标和文字之间要真的分开：放大镜挂在 file-search 右下角，间距小于 10px 就贴上了 */
  gap: 10px;
  /* 行高压到 1：文字盒子不带多余行距，和 20px 图标居中对齐才是准的 */
  line-height: 1;
}

/* 垂直居中靠 .tab-label 的 align-items: center + line-height: 1，不靠 svg 自己的排版：
   svg 默认是 inline，一旦哪天 .tab-label 不再是 flex 容器就会按基线排，看着图标偏上一点。
   display: block 是那种情况下的保险。尺寸在 renderTab 里以 width/height 传（见 TAB_DEFS）。 */
:deep(.tab-icon) {
  flex: 0 0 auto;
  display: block;
}

:deep(.tab-short) {
  letter-spacing: 0.5px;
}

/* 数字用等宽数位：8 变成 88 时整行不跟着抖。
   底色要够深才看得出是个独立的计数徽标，否则"文件"和"90"连成一串"文件90"。 */
:deep(.tab-count) {
  min-width: 20px;
  padding: 2px 6px;
  border-radius: 999px;
  background: rgba(128, 128, 128, 0.28);
  text-align: center;
  font-size: 11px;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
}

:deep(.tab-label--active .tab-count) {
  background: rgba(24, 144, 255, 0.18);
}

@media (prefers-reduced-motion: reduce) {
  :deep(.n-tabs-tab) {
    transition: none;
  }
}

:deep(.n-tabs) {
  height: 100%;
  width: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

:deep(.n-tabs-content) {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: 0 !important;
  margin: 0 !important;
}

:deep(.n-tab-pane) {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  min-width: 0;
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
  /* 行是通栏的（左右各自带 8px 内边距），容器再留边就等于给每一行加了一圈白边，
     窄侧栏里那点宽度全花在留白上。右边不留：滚动条自己会占掉布局宽度。 */
  padding: 0 0 4px 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}

/* 完整结果统计固定在列表上方，列表本身继续使用虚拟滚动承载大量匹配行。 */
.result-summary {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  color: var(--guaxi-text-secondary, #8c8c8c);
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  font-size: var(--sr-fs-meta, 12px);
  line-height: 18px;
}

/* 上百个文件时"全部折叠"是刚需，但它不该抢统计文字的注意力 */
.summary-action {
  flex: 0 0 auto;
  font-size: var(--sr-fs-meta, 12px);
  color: var(--guaxi-text-secondary);
}

.summary-action:hover {
  color: var(--n-color-primary, #1890ff);
}

.tab-content-excerpts {
  flex: 1 1 0;
  min-height: 0;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  padding: 0 0 4px 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}

.tab-content-excerpts :deep(.excerpt-group),
.tab-content-excerpts :deep(.excerpt-body),
.tab-content-excerpts :deep(.card-container),
.tab-content-excerpts :deep(.excerpt-card),
.tab-content-excerpts :deep(.n-card) {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  box-sizing: border-box;
}

.tab-content-excerpts :deep(.excerpt-group),
.tab-content-excerpts :deep(.excerpt-body) {
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

/*
 * ===== 三个标签页共用的一行 =====
 *
 * 文件页一条结果、内容页的文件组头、内容页的每条命中，本质都是"一行"，所以只有一套类：
 * .sr-row 管几何和状态，.sr-row--group / --match 只管这一层多出来的那点差异。
 * 摘录页在 GuaxiNoteExcerpt.vue 里用同名的一套（那是另一个 SFC，scoped 样式进不去），
 * 颜色和字号都从 App.vue 的 --sr-* 变量取，改一处三处一起变。
 *
 * 行高必须和脚本里的 SR_ROW_HEIGHT 严格一致：虚拟列表按固定行高算偏移，差一个像素就越滚越偏。
 * 所以这里不再手抄数字，直接 v-bind 脚本里的常量；一律 border-box、不设 margin、不设边框。
 */
.sr-row {
  display: flex;
  align-items: center;
  height: v-bind(SR_ROW_CSS_HEIGHT);
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 0 8px;
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  line-height: v-bind(SR_ROW_CSS_HEIGHT);
  /* 位移和外发光在这种密排行上是灾难：一抖就串行。状态只走底色 + 左侧竖条。 */
  transition: background-color 0.12s ease;
}

.sr-row:hover {
  background: var(--sr-row-hover);
}

/* 选中改色相（主题蓝），悬停改明度（灰）：两种状态不会被看成同一件事。 */
.sr-row.is-active {
  background: var(--sr-row-active);
  box-shadow: inset 2px 0 0 var(--sr-row-accent, #1890ff);
}

/* 文件行：文件名是这一屏的主标题，字号取共用梯级的最大一档，和摘录页的文件名同级 */
.sr-row--group {
  gap: 6px;
  font-size: var(--sr-fs-group, 15px);
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.sr-caret {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  transition: transform 0.15s ease;
}

.sr-caret.is-collapsed {
  transform: rotate(-90deg);
}

.sr-name {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 目录跟在文件名右边、更小更淡：同名文件一多，光看文件名认不出是哪个 */
.sr-dir {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: var(--sr-fs-meta, 12px);
  font-weight: 400;
  color: var(--guaxi-text-secondary);
}

.sr-count {
  flex: 0 0 auto;
  margin-left: auto;
  padding: 0 6px;
  border-radius: 999px;
  background: rgba(128, 128, 128, 0.22);
  color: var(--guaxi-text-secondary);
  font-size: var(--sr-fs-meta, 12px);
  line-height: 18px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

/* 命中行缩进到组头文件名的位置，层级一眼看得出来 */
.sr-row--match {
  gap: 10px;
  padding-left: 14px;
}

/* 行号当成编辑器的行号沟：右对齐 + 等宽数位，正文的起始位置才是齐的 */
.sr-line-no {
  flex: 0 0 auto;
  min-width: 36px;
  text-align: right;
  font-size: var(--sr-fs-meta, 12px);
  font-variant-numeric: tabular-nums;
  color: var(--guaxi-text-secondary);
  opacity: 0.75;
}

/* 命中的正文是这个面板真正要读的东西，字号跟着正文级走，比行号、目录都大一档。
   white-space: pre 保留原文里的连续空格，一行装不下就交给 CSS 省略号。
   不能用 pre-wrap：换行会把行高撑破，虚拟列表立刻错位。 */
.sr-line-text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: pre;
  tab-size: 4;
  font-size: var(--sr-fs-body, 14px);
  color: var(--guaxi-text-primary);
}

/* 窗口平移或截断留下的省略记号，颜色压下去，免得被当成原文里的省略号 */
.sr-cut {
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

/* 密排行里的高亮不留内边距和阴影：一加就把后面的字往右推，
   同一个词在不同行的位置看着都不一样。底色够亮，已经足够显眼。 */
.sr-line-text .search-highlight {
  padding: 0;
  box-shadow: none;
  font-weight: 600;
}

@media (prefers-reduced-motion: reduce) {

  .sr-row,
  .sr-caret {
    transition: none;
  }
}

/* 注意：GuaxiEditor.vue 里有一条全局的 .search-highlight，背景和前景都带 !important
   （Monaco 的命中装饰要用），所以下面这两行颜色实际会被它盖掉，全应用的命中色是统一的。
   这里保留是当兜底：万一哪天那个组件没挂载，命中也不能变成没有底色的裸文字。 */
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
  width: 100%;
  min-width: 0;
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
