<template>
  <div class="excerpt-navigator" ref="excerptNavigatorRef">
    <n-empty v-if="excerptResults.length === 0" description="暂无摘录笔记搜索结果" style="margin-top: 20px">
      <template #extra>
        <n-text depth="3">尝试不同的搜索词</n-text>
      </template>
    </n-empty>
    <!--
      结构说明（为什么不再用 n-list / n-card 套着）：
      这里以前是"结果项 → 展开区 → n-card → 卡片项 → 渲染内容"五层盒子，每层都带边框、圆角和内边距，
      窄侧栏里左右各被吃掉 60px 上下，正文只剩中间一条。现在只剩两层：
      文件行（和文件/内容两个标签页共用 .sr-row）+ 展开区，展开区里用缩进和一道引导线表达层级。
      交互一个没动：展开仍是懒加载 + 预渲染，两个"插入"按钮、键盘导航、data-index 定位都在原位。
    -->
    <template v-else>
      <div class="result-summary">
        <span>共找到 {{ excerptResults.length }} 个摘录文件</span>
      </div>
      <n-scrollbar ref="scrollbarRef" class="excerpt-scrollbar">
        <div v-for="(item, index) in excerptResults" :key="item.file_path" class="excerpt-group"
          :data-key="item.file_path" :data-index="index">
          <div class="sr-row sr-row--group" :class="{ 'is-active': activeExcerptIndex === index }"
            :title="item.file_path" @click="handleGroupClick(item, index)">
            <n-icon class="sr-caret" :component="item.expanded ? ChevronDown : ChevronRight" size="11" />
            <IconifyIcon v-bind="getFileIconPropsWithLog(item.file_path)" :size="16" />
            <span class="sr-name">{{ item.file_name }}</span>
            <n-button size="tiny" type="primary" tertiary @click.stop="insertExcerpt(item)" class="sr-action"
              title="插入摘录内容">
              <template #icon>
                <n-icon :component="TextGrammarArrowLeft20Regular" />
              </template>
              插入
            </n-button>
          </div>

          <div v-if="item.expanded" class="excerpt-body" @click.stop>
            <n-spin :show="item.loading">
              <div v-if="item.content" class="card-container">
                <div v-for="card in item.content.cards" :key="card.id" class="excerpt-card">
                  <div class="card-header">
                    <n-tag :type="getCardTypeColor(card.type)" size="small" :bordered="false">
                      {{ formatCardType(card.type) }}
                    </n-tag>
                    <span class="card-title">{{ card.title }}</span>
                    <span class="card-time">{{ formatTimestamp(card.timestamp) }}</span>
                  </div>

                  <div v-for="(cardItem, ci) in card.items" :key="ci" class="card-item">
                    <!-- 类型、时间、插入挤在同一条 meta 行上：以前它们各占一行，一条摘录光页眉就三行 -->
                    <div class="card-item-bar">
                      <span class="card-item-type">{{ formatCardItemType(cardItem.type) }}</span>
                      <span class="card-item-time">{{ formatTimestamp(cardItem.timestamp) }}</span>
                      <n-button size="tiny" type="primary" tertiary
                        @click.stop="insertCardItem(cardItem, item.file_path)" class="sr-action" title="插入这一条">
                        <template #icon>
                          <n-icon :component="TextGrammarArrowLeft20Regular" size="14" />
                        </template>
                        插入
                      </n-button>
                    </div>
                    <!-- 使用现有renderMarkdown接口渲染内容 -->
                    <div class="card-item-rendered" v-html="renderedOf(card, cardItem)"></div>
                  </div>
                </div>
              </div>
              <n-empty v-else-if="item.loading" description="无法加载文件内容" />
            </n-spin>
          </div>
        </div>
      </n-scrollbar>
    </template>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { h, defineComponent, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import { Icon } from '@iconify/vue';
import { invoke } from '@tauri-apps/api/core';
import {
  NEmpty,
  NText,
  NButton,
  NIcon,
  NTag,
  NSpin,
  NScrollbar
} from 'naive-ui';
import { TextGrammarArrowLeft20Regular } from '@vicons/fluent';
import { ChevronDown, ChevronRight } from '@vicons/fa';
import { useEditorStore } from '../../stores/editorStore';
import { useRightSiderStore } from '../../stores/rightSiderStore';
import { useSiderStore } from '../../stores/useSiderStore';
import { useSettingsStore } from '../../stores/settingsStore'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import { join } from '@tauri-apps/api/path'
import { path } from '@tauri-apps/api';
import { convertFileSrc } from '@tauri-apps/api/core';

const logger = createLogger('GuaxiNoteExcerpt')
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

const editorStore = useEditorStore();
const rightSiderStore = useRightSiderStore();
const siderStore = useSiderStore();
const settingsStore = useSettingsStore()
const renderedContents = ref({});
const scrollbarRef = ref(null);
const excerptNavigatorRef = ref(null);
const activeExcerptIndex = ref(-1);
let excerptResizeObserver = null;

const props = defineProps({
  excerptResults: {
    type: Array,
    default: () => [],
  }
});

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
  }
  return props;
};

const getFileIconProps = (path) => {
  const ext = path.split('.').pop()?.toLowerCase() || '';
  const isActive = editorStore.activeFile?.path === path;

  if (path.endsWith('/')) {
    return {
      icon: FILE_ICON_MAP.folder,
      color: isActive ? '#3366ff' : '#666'
    };
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
  return {
    icon,
    color: isActive ? '#3366ff' : '#666'
  };
};

// 为每个文件添加展开状态和内容
const excerptResults = ref(props.excerptResults.map(item => ({
  ...item,
  expanded: false,
  loading: false,
  content: null
})));

async function processImages(content, basePath = '') {
  const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
  const replacements = [];

  for (const match of content.matchAll(imgRegex)) {
    const [full, alt, src] = match;
    if (!src.match(/^(http|\/|data:image)/i)) {
      try {
        const fullPath = await join(basePath, src);
        const safeSrc = await convertFileSrc(fullPath);
        replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
      } catch (error) {
        logger.warn(`路径转换失败: ${src}`, error);
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

const getImagesDirectory = async (filePath) => {
  if (!filePath) {
    logger.warn('未提供有效文件路径');
    return '';
  }

  try {
    const dirPath = await path.dirname(filePath);
    const parentDir = await path.dirname(dirPath);
    const imagesDir = await path.join(parentDir, 'images');
    return await path.normalize(imagesDir);
  } catch (error) {
    logger.error('获取images目录失败:', error);
    return ''; // 返回空字符串而不是抛出错误
  }
};

// 使用现有的renderMarkdown接口渲染卡片内容
const renderedCardContent = async (cardItem, excerptFilePath) => {
  try {
    // 获取文件所在目录作为basePath
    const basePath = excerptFilePath ? excerptFilePath : '';
    const imageDir = await getImagesDirectory(basePath)
    const processedContent = await processImages(cardItem.content, imageDir);

    // 使用您提供的renderMarkdown接口
    return await renderMarkdown(processedContent, {
      basePath: basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: settingsStore.appearanceTheme
    });
  } catch (error) {
    logger.error('渲染卡片内容失败:', error);
    return cardItem.content; // 失败时返回原始内容
  }
};

// 缓存渲染结果
const renderedContentCache = ref({});

/**
 * 渲染结果的缓存键。写入和读取必须用同一个：
 * 以前写进去用的是 `cardItem.id || card.id_type_timestamp` 的兜底键，模板却只读 `cardItem.id`，
 * 于是没有 id 的摘录项永远读不到渲染结果，界面上显示的是没渲染的原始 Markdown。
 */
const cardItemKey = (card, cardItem) =>
  cardItem?.id || `${card?.id}_${cardItem?.type}_${cardItem?.timestamp}`

const renderedOf = (card, cardItem) =>
  renderedContents.value[cardItemKey(card, cardItem)] || cardItem?.content || ''

// 修改后的toggleFileExpand方法，预渲染内容
const toggleFileExpand = async (item) => {
  item.expanded = !item.expanded;
  if (item.expanded && !item.content) {
    try {
      item.loading = true;
      item.content = await invoke('read_excerpt_file', {
        path: item.file_path
      });

      // 预渲染所有卡片内容
      if (item.content?.cards) {
        for (const card of item.content.cards) {
          if (card.items) {
            for (const cardItem of card.items) {
              const rendered = await renderedCardContent(cardItem, item.file_path);
              renderedContents.value[cardItemKey(card, cardItem)] = rendered;
            }
          }
        }
      }
    } catch (err) {
      logger.error('加载文件内容失败:', err);
      window.$message?.error('加载文件内容失败: ' + err);
    } finally {
      item.loading = false;
    }
  }
};

/**
 * 点文件行：先把选中挪过来，再展开/折叠。
 * 以前点击只展开、不改选中，于是鼠标点的是一条、键盘上下移动的却是另一条，
 * 和文件/内容两个标签页的手感也不一样。
 */
const handleGroupClick = (item, index) => {
  setActiveExcerptIndex(index)
  toggleFileExpand(item)
};

// 格式化卡片类型
const formatCardType = (type) => {
  const typeMap = {
    'composite': '复合笔记',
    'single': '单条笔记'
  };
  return typeMap[type] || type;
};

// 获取卡片类型颜色
const getCardTypeColor = (type) => {
  const colorMap = {
    'composite': 'primary',
    'single': 'info'
  };
  return colorMap[type] || 'default';
};

// 格式化卡片项类型
const formatCardItemType = (type) => {
  const typeMap = {
    'text': '文本',
    'math': '公式',
    'code': '代码',
    'image': '图片',
    'diagram': '图表'
  };
  return typeMap[type] || type;
};

// 格式化时间戳
const formatTimestamp = (timestamp) => {
  if (!timestamp) return '';
  return new Date(timestamp).toLocaleString();
};

const insertExcerpt = async (item) => {
  try {
    // 如果文件未展开，先加载内容
    if (!item.content && !item.expanded) {
      await toggleFileExpand(item);
    }

    // 等待内容加载完成
    if (!item.content) {
      window.$message?.warning('文件内容尚未加载完成，请稍后再试');
      return;
    }

    // 收集所有卡片项内容
    let contentToInsert = "";

    // 使用 for...of 替代 forEach，以便使用 await
    for (const card of item.content.cards || []) {
      for (const cardItem of card.items || []) {
        // 确保每个卡片项都有父文件路径引用
        if (!cardItem.file_path) {
          cardItem.file_path = item.file_path; // 直接赋值给file_path
        }
        // 根据不同类型格式化内容
        switch (cardItem.type) {
          case 'text':
            contentToInsert += `\n${cardItem.content}`;
            break;
          case 'math':
            contentToInsert += `\n${cardItem.content}`;
            break;
          case 'code':
            contentToInsert += `\n${cardItem.content}`;
            break;
          case 'image':
            const fileBasePath = cardItem.file_path || '';
            const imageDir = await getImagesDirectory(fileBasePath);
            const processedImageContent = await processImages(cardItem.content, imageDir);
            logger.info(">>>>>>>> processedImageContent:", processedImageContent);
            contentToInsert += `\n${processedImageContent}`;
            break;
          case 'diagram':
            contentToInsert += `\n${cardItem.content}`;
            break;
          default:
            contentToInsert += `\n${cardItem.content}`;
        }
      }
    }

    // 插入到编辑器
    await editorStore.insertContent(contentToInsert);
    window.$message?.success(`已插入 ${item.content.cards?.length || 0} 个卡片的内容`);

  } catch (error) {
    logger.error('插入摘录失败:', error);
    window.$message?.error('插入摘录失败: ' + error.message);
  }
};

// 修改 insertCardItem 函数，添加 async
const insertCardItem = async (cardItem, parentFilePath) => {
  let contentToInsert = '';
  logger.info('插入摘录:', cardItem);

  try {
    // 根据不同类型格式化内容
    switch (cardItem.type) {
      case 'text':
        contentToInsert = `\n${cardItem.content}`;
        break;
      case 'math':
        contentToInsert = `\n${cardItem.content}`;
        break;
      case 'code':
        contentToInsert = `\n${cardItem.content}`;
        break;
      case 'image':
        logger.info('处理图片类型:', cardItem);
        // 优先使用卡片项自身的file_path，没有则使用父文件路径
        const fileBasePath = cardItem.file_path || parentFilePath || '';
        logger.info('使用的基础路径:', fileBasePath);

        if (!fileBasePath) {
          logger.warn('没有可用的文件路径，使用原始内容');
          contentToInsert = `\n${cardItem.content}`;
          break;
        }

        const imageDir = await getImagesDirectory(fileBasePath);
        logger.info('图片目录:', imageDir);

        const processedImageContent = await processImages(cardItem.content, imageDir);
        logger.info("处理后的图片内容:", processedImageContent);
        contentToInsert = `\n${processedImageContent}`;
        break;
      case 'diagram':
        contentToInsert = `\n${cardItem.content}`;
        break;
      default:
        contentToInsert = cardItem.content;
    }

    // 插入到编辑器
    await editorStore.insertContent(contentToInsert);
    window.$message?.success('内容已插入');
  } catch (error) {
    logger.error('插入内容失败:', error);
    window.$message?.error('插入内容失败');
  }
};

// 滚动激活项到可视区域
const scrollActiveIntoView = async () => {
  const idx = activeExcerptIndex.value
  if (idx < 0) return

  await nextTick()

  const scrollbar = scrollbarRef.value
  const root = scrollbar?.containerRef?.value
    || scrollbar?.contentRef?.value
    || scrollbar?.$el

  if (!root || typeof root.querySelector !== 'function') return

  const target = root.querySelector(`[data-index="${idx}"]`)
  if (!target) return

  logger.debug('[GuaxiNoteExcerpt] scroll active excerpt', { index: idx })
  target.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

// 设置激活索引并滚动
const setActiveExcerptIndex = (index) => {
  const list = excerptResults.value
  if (!list.length) {
    activeExcerptIndex.value = -1
    return
  }

  const next = Math.max(0, Math.min(list.length - 1, Number(index)))
  activeExcerptIndex.value = Number.isFinite(next) ? next : 0
  scrollActiveIntoView()
}

// 方向键移动激活项
const moveActiveExcerpt = (delta) => {
  const list = excerptResults.value
  if (!list.length) return

  const cur = activeExcerptIndex.value
  if (cur < 0) {
    activeExcerptIndex.value = delta >= 0 ? 0 : list.length - 1
    scrollActiveIntoView()
    return
  }

  const next = (cur + delta + list.length) % list.length
  activeExcerptIndex.value = next
  scrollActiveIntoView()
}

// 回车触发当前激活项
const activateActiveExcerpt = async () => {
  const list = excerptResults.value
  if (!list.length) return

  const idx = activeExcerptIndex.value < 0 ? 0 : activeExcerptIndex.value
  const item = list[idx]
  if (!item) return

  logger.debug('[GuaxiNoteExcerpt] activate excerpt', { index: idx, path: item.file_path })
  await toggleFileExpand(item)
}

// 对外暴露键盘处理
const handleKeydown = (e) => {
  if (e?.defaultPrevented) return

  const key = e?.key
  if (key === 'ArrowDown') {
    e.preventDefault()
    moveActiveExcerpt(1)
    return
  }

  if (key === 'ArrowUp') {
    e.preventDefault()
    moveActiveExcerpt(-1)
    return
  }

  if (key === 'Enter') {
    e.preventDefault()
    activateActiveExcerpt()
  }
}

defineExpose({ handleKeydown })

// 设计思路：
// 1. 摘录笔记面板跟随右侧栏宽度变化，列表项和“插入”按钮都应该重新占满可用宽度。
// 2. 这里记录根节点实际尺寸，便于排查右侧栏拖拽后内部列表是否仍被旧宽度约束。
const handleExcerptNavigatorResize = (entries = []) => {
  const entry = entries[0]
  const width = Math.round(entry?.contentRect?.width || excerptNavigatorRef.value?.clientWidth || 0)
  const height = Math.round(entry?.contentRect?.height || excerptNavigatorRef.value?.clientHeight || 0)
  logger.debug('[GuaxiNoteExcerpt] resize', {
    width,
    height,
    resultCount: excerptResults.value.length,
  })
}

onMounted(() => {
  if (excerptNavigatorRef.value && typeof ResizeObserver !== 'undefined') {
    excerptResizeObserver = new ResizeObserver(handleExcerptNavigatorResize)
    excerptResizeObserver.observe(excerptNavigatorRef.value)
  }
})

onBeforeUnmount(() => {
  excerptResizeObserver?.disconnect?.()
  excerptResizeObserver = null
})

// 监听搜索结果变化
watch(() => props.excerptResults, (newVal) => {
  excerptResults.value = newVal.map(item => ({
    ...item,
    expanded: false,
    loading: false,
    content: null
  }));
  if (newVal && newVal.length > 0) {
    rightSiderStore.open();
    if (siderStore.isRightSiderCollapsed['home']) {
      siderStore.toggleRightSider('home');
    }
    // 初始化选中项
    activeExcerptIndex.value = 0
    scrollActiveIntoView()
  } else {
    activeExcerptIndex.value = -1
  }
}, { immediate: true });
</script>

<style scoped>
.excerpt-navigator {
  /* 设计思路：
   * 1. 这个面板以前是"list → 结果项 → 展开区 → n-card → 卡片项 → 渲染内容"六层同心盒子，
   *    每层都有边框、圆角和内边距，正文左右各被吃掉近 60px，窄侧栏里只剩中间一条能读。
   *    现在层级只靠缩进和一道引导线表达，边框留给真正需要"框"的东西（代码块、表格）。
   * 2. 悬停/选中的颜色一律取 App.vue 里的 --sr-* 变量，和文件、内容两个标签页共用一套；
   *    这里是另一个 SFC，scoped 样式进不来，所以类名刻意和那边同名同义（.sr-row / .sr-name…）。
   * 3. 只调整视觉层级，展开、插入、键盘导航、data-index 定位都没动。
   */
  --excerpt-hairline: var(--sr-hairline, rgba(148, 163, 184, 0.18));
  height: 100%;
  width: 100%;
  min-width: 0;
  min-height: 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  container-type: inline-size;
  font-size: var(--sr-fs-body, 14px);
}

/* 和文件、内容两个标签页的统计条对齐：同一位置、同一底色、同一字号、同一条下边界 */
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

.excerpt-scrollbar {
  flex: 1 1 0;
  width: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

:deep(.n-scrollbar-container) {
  min-height: 0;
}

.excerpt-group {
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  overflow: hidden;
}

/* ===== 和文件 / 内容两个标签页共用的一行 =====
   同名类在 GuaxiSearchNavigator.vue 里还有一份（scoped 样式跨不过组件边界），
   两边的颜色都从 App.vue 的 --sr-* 取，改一处三个标签页一起变。
   这里用 min-height 而不是定高：行里挂着一个 22px 的"插入"按钮，定高会把它压出边界；
   28px 是对齐那边的 SR_ROW_HEIGHT，这个列表没走虚拟滚动，差一两个像素也不会错位。 */
.sr-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 28px;
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 2px 8px;
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  transition: background-color 0.12s ease;
}

.sr-row:hover {
  background: var(--sr-row-hover);
}

.sr-row.is-active {
  background: var(--sr-row-active);
  box-shadow: inset 2px 0 0 var(--sr-row-accent, #1890ff);
}

.sr-row--group {
  font-size: var(--sr-fs-group, 15px);
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.sr-caret {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
}

.sr-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 操作按钮永远贴右：侧栏拖窄时该被压缩的是文件名，不是能点的东西 */
.sr-action {
  flex: 0 0 auto;
  max-width: 72px;
  min-width: 34px;
  overflow: hidden;
  white-space: nowrap;
}

/* 设计思路：
 * 1. 右侧栏拖窄时，文件名是最适合被压缩的内容，操作按钮必须保留在最右侧。
 * 2. 在极窄宽度下只保留按钮图标，仍通过 title 保留“插入摘录内容”的提示。
 */
@container (max-width: 420px) {
  .sr-row {
    gap: 5px;
  }

  .sr-action {
    max-width: 34px;
    padding-left: 7px !important;
    padding-right: 7px !important;
  }

  .sr-action :deep(.n-button__content) {
    width: 0;
    overflow: hidden;
  }
}

/* 展开区：只用缩进 + 一道引导线表达"这些是这个文件里的"，不再套盒子。
   左边 10px 的缩进对齐组头的文件名，读起来是一棵树而不是一堆卡片。 */
.excerpt-body {
  width: 100%;
  box-sizing: border-box;
  margin: 0;
  padding: 2px 8px 8px 10px;
  margin-left: 8px;
  border-left: 1px solid var(--sr-rail, rgba(148, 163, 184, 0.28));
  overflow: hidden;
}

.card-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
  box-sizing: border-box;
  min-height: 0;
}

/* 一张摘录卡片。分隔靠上边那条 hairline 和自己的标题行，不再靠边框 + 圆角 + 阴影围一圈。 */
.excerpt-card {
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
  background: transparent;
}

.excerpt-card + .excerpt-card {
  margin-top: 4px;
  padding-top: 8px;
  border-top: 1px solid var(--excerpt-hairline);
}

/* 卡片标题行：标签 + 标题 + 时间，一行 22px。
   以前它是 n-card 的 header，字号被 naive 定成 ~18px——比两级之上的文件名（14px）还大，
   于是"标题挤掉正文"。现在标题和正文同为正文级字号，靠字重区分，比它更小的只有时间。 */
.card-header {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
  min-height: 22px;
}

.card-title {
  flex: 1 1 auto;
  min-width: 0;
  font-size: var(--sr-fs-body, 14px);
  font-weight: 600;
  color: var(--guaxi-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-time {
  flex: 0 0 auto;
  font-size: var(--sr-fs-meta, 12px);
  color: var(--guaxi-text-secondary);
  font-variant-numeric: tabular-nums;
}

.card-item {
  width: 100%;
  box-sizing: border-box;
  padding: 4px 0 6px;
}

.card-item + .card-item {
  border-top: 1px dashed var(--excerpt-hairline);
}

/* 类型、时间、插入按钮合成一条 meta 行：以前类型一行、正文一行、时间又一行，
   一条几十个字的摘录光页眉页脚就占掉三行。 */
.card-item-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
  min-height: 20px;
}

.card-item-type {
  flex: 0 0 auto;
  font-size: var(--sr-fs-meta, 12px);
  color: var(--guaxi-text-secondary);
}

.card-item-time {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--sr-fs-meta, 12px);
  color: var(--guaxi-text-secondary);
  opacity: 0.75;
  font-variant-numeric: tabular-nums;
}

/* 正文是这个面板真正要读的东西：给足字号和行距，左右不再套框，直接排到面板边缘。 */
.card-item-rendered {
  width: 100%;
  max-width: 100%;
  margin: 2px 0 0;
  padding: 0;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-size: var(--sr-fs-body, 14px);
  line-height: 1.65;
  box-sizing: border-box;
  overflow: auto;
  word-wrap: break-word;
  overflow-wrap: break-word;
}

/* markdown 渲染出来的 <p> 自带 1em 上下外边距，密排列表里一条摘录能被撑高一倍。 */
.card-item-rendered :deep(p) {
  margin: 0 0 0.4em;
}

.card-item-rendered :deep(p:last-child) {
  margin-bottom: 0;
}

.card-item-rendered :deep(ul),
.card-item-rendered :deep(ol) {
  margin: 0 0 0.4em;
  padding-left: 1.4em;
}

/* 摘录正文里的 markdown 标题按浏览器默认能到 2em（28px），比两级之上的文件名（15px）还大，
   在窄侧栏里就是"标题挤掉正文"的另一个来源。这里统一压到正文字号，只留字重做区分。 */
.card-item-rendered :deep(h1),
.card-item-rendered :deep(h2),
.card-item-rendered :deep(h3),
.card-item-rendered :deep(h4),
.card-item-rendered :deep(h5),
.card-item-rendered :deep(h6) {
  margin: 0.2em 0 0.3em;
  font-size: var(--sr-fs-body, 14px);
  font-weight: 600;
  line-height: 1.5;
}

.card-item-rendered :deep(a),
.card-item-rendered :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .card-item-rendered :deep(a),
.dark-mode .card-item-rendered :deep(a:visited),
.dark .card-item-rendered :deep(a),
.dark .card-item-rendered :deep(a:visited),
:root[data-guaxi-theme='dark'] .card-item-rendered :deep(a),
:root[data-guaxi-theme='dark'] .card-item-rendered :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.card-item-rendered :deep(a:hover) {
  opacity: 0.92;
}

.card-item-rendered :deep(pre) {
  max-width: 100%;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  padding: 0.5em;
  background: var(--guaxi-header-bg);
  border-radius: 3px;
}

.card-item-rendered :deep(code) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', monospace;
  font-size: 0.9em;
  white-space: pre-wrap;
}

.card-item-rendered :deep(img) {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}

.card-item-rendered :deep(table) {
  width: 100%;
  max-width: 100%;
  overflow: auto;
  display: block;
  border-collapse: collapse;
}

.card-item-rendered :deep(table td),
.card-item-rendered :deep(table th) {
  padding: 6px 13px;
  border: 1px solid var(--excerpt-hairline);
}

.card-item-rendered :deep(blockquote) {
  margin: 0;
  padding: 0 1em;
  color: var(--guaxi-text-secondary);
  border-left: 0.25em solid var(--excerpt-hairline);
}

/* 加载状态动画增强 */
.n-spin-container {
  transition: all 0.3s ease;
}

.n-spin-container .n-spin-content {
  opacity: 0.8;
  transition: opacity 0.3s;
}

.n-spin-container .n-spin-content:hover {
  opacity: 1;
}
</style>
