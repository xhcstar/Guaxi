<template>
  <div class="excerpt-navigator">
    <n-empty v-if="excerptResults.length === 0" description="暂无摘录笔记搜索结果" style="margin-top: 20px">
      <template #extra>
        <n-text depth="3">尝试不同的搜索词</n-text>
      </template>
    </n-empty>
    <n-scrollbar v-else class="excerpt-scrollbar">
      <n-list hoverable clickable>
        <n-list-item v-for="item in excerptResults" :key="item.file_path" :data-key="item.file_path" @click="toggleFileExpand(item)">
        <div class="excerpt-result-item">
          <div class="file-name" :title="item.file_path">
            <n-icon :component="item.expanded ? ChevronDown : ChevronRight" size="12" @click.stop="toggleFileExpand(item)"
              class="expand-icon" />
            <IconifyIcon v-bind="getFileIconPropsWithLog(item.file_path)" />
            <span class="filename-text">{{ item.file_name }}</span>
            <n-button size="tiny" type="primary" tertiary @click.stop="insertExcerpt(item)" class="insert-btn">
              <template #icon>
                <n-icon :component="TextGrammarArrowLeft20Regular" />
              </template>
              插入
            </n-button>
          </div>
        </div>

        <div v-if="item.expanded" class="file-content" @click.stop>
          <n-spin :show="item.loading">
            <div v-if="item.content" class="card-container">
              <n-card v-for="card in item.content.cards" :key="card.id" class="excerpt-card">
                <template #header>
                  <div class="card-header">
                    <n-tag :type="getCardTypeColor(card.type)" size="small">
                      {{ formatCardType(card.type) }}
                    </n-tag>
                    <span class="card-title">{{ card.title }}</span>
                    <span class="card-timestamp">{{ formatTimestamp(card.timestamp) }}</span>
                  </div>
                </template>

                <div class="card-items">
                  <div v-for="(cardItem, index) in card.items" :key="index" class="card-item">
                    <div class="card-item-header">
                      <div class="card-item-type">{{ formatCardItemType(cardItem.type) }}</div>
                      <n-button size="tiny" type="primary" tertiary
                        @click.stop="insertCardItem(cardItem, item.file_path)" class="card-item-insert">
                        <template #icon>
                          <n-icon :component="TextGrammarArrowLeft20Regular" size="14" />
                        </template>
                        插入
                      </n-button>
                    </div>
                    <!-- 使用现有renderMarkdown接口渲染内容 -->
                    <div class="card-item-rendered" v-html="renderedContents[cardItem.id] || cardItem.content"></div>
                    <div class="card-item-timestamp">{{ formatTimestamp(cardItem.timestamp) }}</div>
                  </div>
                </div>
              </n-card>
            </div>
            <n-empty v-else-if="item.loading" description="无法加载文件内容" />
          </n-spin>
        </div>
        </n-list-item>
      </n-list>
    </n-scrollbar>
  </div>
</template>

<script setup>
import { h, defineComponent, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import { invoke } from '@tauri-apps/api/core';
import {
  NEmpty,
  NText,
  NList,
  NListItem,
  NButton,
  NIcon,
  NCard,
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
    console.warn('[n-icon] icon prop missing!', path, props);
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
        console.warn(`路径转换失败: ${src}`, error);
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

const getImagesDirectory = async (filePath) => {
  if (!filePath) {
    console.warn('未提供有效文件路径');
    return '';
  }

  try {
    const dirPath = await path.dirname(filePath);
    const parentDir = await path.dirname(dirPath);
    const imagesDir = await path.join(parentDir, 'images');
    return await path.normalize(imagesDir);
  } catch (error) {
    console.error('获取images目录失败:', error);
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
    console.error('渲染卡片内容失败:', error);
    return cardItem.content; // 失败时返回原始内容
  }
};

// 缓存渲染结果
const renderedContentCache = ref({});

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
              renderedContents.value[cardItem.id || `${card.id}_${cardItem.type}_${cardItem.timestamp}`] = rendered;
            }
          }
        }
      }
    } catch (err) {
      console.error('加载文件内容失败:', err);
      window.$message?.error('加载文件内容失败: ' + err);
    } finally {
      item.loading = false;
    }
  }
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
            console.log(">>>>>>>> processedImageContent:", processedImageContent);
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
    console.error('插入摘录失败:', error);
    window.$message?.error('插入摘录失败: ' + error.message);
  }
};

// 修改 insertCardItem 函数，添加 async
const insertCardItem = async (cardItem, parentFilePath) => {
  let contentToInsert = '';
  console.log('插入摘录:', cardItem);

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
        console.log('处理图片类型:', cardItem);
        // 优先使用卡片项自身的file_path，没有则使用父文件路径
        const fileBasePath = cardItem.file_path || parentFilePath || '';
        console.log('使用的基础路径:', fileBasePath);

        if (!fileBasePath) {
          console.warn('没有可用的文件路径，使用原始内容');
          contentToInsert = `\n${cardItem.content}`;
          break;
        }

        const imageDir = await getImagesDirectory(fileBasePath);
        console.log('图片目录:', imageDir);

        const processedImageContent = await processImages(cardItem.content, imageDir);
        console.log("处理后的图片内容:", processedImageContent);
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
    console.error('插入内容失败:', error);
    window.$message?.error('插入内容失败');
  }
};

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
  }
}, { immediate: true });
</script>

<style scoped>
.excerpt-navigator {
  height: 100%;
  width: 100%;
  min-height: 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.excerpt-scrollbar {
  flex: 1 1 0;
  min-height: 0;
  overflow: hidden;
}

:deep(.n-scrollbar-container) {
  min-height: 0;
}

:deep(.n-list) {
  min-height: 0;
  max-width: 475px;
  width: 100%;
  padding: 0;
  margin: 0;
  overflow: hidden;
}

:deep(.n-list__content) {
  min-height: 0;
}

:deep(.n-list-item) {
  width: 100%;
  max-width: 465px;
  margin: 2px !important;
  padding: 2px 0 !important;
  transition: all 0.2s ease;
}

:deep(.n-list-item:hover) {
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

:deep(.n-list::-webkit-scrollbar) {
  width: 6px;
}

:deep(.n-list::-webkit-scrollbar-track) {
  background: var(--guaxi-header-bg);
  border-radius: 3px;
}

:deep(.n-list::-webkit-scrollbar-thumb) {
  background: var(--guaxi-border);
  border-radius: 3px;
}

:deep(.n-list::-webkit-scrollbar-thumb:hover) {
  background: var(--guaxi-text-secondary);
}

.excerpt-result-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  top: 4px;
  gap: 4px;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  box-shadow: none;
  padding: 8px;
  margin-bottom: 2px;
  margin-left: 0;
  transition: all 0.2s ease;
  position: relative;
}

/* 增强的悬停效果 */
.n-list-item:hover .excerpt-result-item {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.22),
    0 2px 12px 0 rgba(0, 0, 0, 0.18);
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
  max-width: 420px;
  overflow: hidden;
  padding-right: 8px;
  box-sizing: border-box;
  transition: color 0.2s;
}

/* 文件名悬停效果 */
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

.insert-btn {
  margin-left: auto;
  flex-shrink: 0;
  white-space: nowrap;
  transition: all 0.2s;
}

/* 插入按钮悬停效果 */
.n-list-item:hover .insert-btn {
  background-color: rgba(24, 144, 255, 0.1);
  transform: translateX(-2px);
}

.expand-icon {
  flex-shrink: 0;
  cursor: pointer;
  margin-right: 4px;
  transition: transform 0.2s;
}

/* 展开图标悬停效果 */
.n-list-item:hover .expand-icon {
  color: #1890ff;
  transform: scale(1.2);
}

.file-content {
  width: 100%;
  background-color: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  margin-top: 8px;
  box-sizing: border-box;
  overflow: visible;
}

.card-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
  box-sizing: border-box;
  min-height: 0;
}

.excerpt-card {
  width: 100%;
  max-width: 100%;
  margin-bottom: 12px;
  box-sizing: border-box;
  transition: all 0.2s;
}

/* 卡片悬停效果 */
.excerpt-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  transform: translateY(-2px);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
}

.card-title {
  font-weight: 500;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-timestamp {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  flex-shrink: 0;
}

.card-items {
  margin-top: 12px;
  width: 100%;
  box-sizing: border-box;
}

.card-item {
  padding: 12px;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--guaxi-border);
  width: 100%;
  box-sizing: border-box;
  transition: all 0.2s;
}

/* 卡片项悬停效果 */
.card-item:hover {
  background-color: var(--guaxi-header-bg);
  border-radius: 4px;
}

.card-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
  width: 100%;
  min-width: 0;
}

.card-item-type {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  flex-shrink: 0;
}

.card-item-insert {
  margin-left: 8px;
  flex-shrink: 0;
  transition: all 0.2s;
}

/* 卡片项插入按钮悬停效果 */
.card-item:hover .card-item-insert {
  background-color: rgba(24, 144, 255, 0.1);
}

.card-item-timestamp {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  text-align: right;
}

.card-item-rendered {
  width: 100%;
  max-width: 100%;
  margin: 8px 0;
  padding: 8px;
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-radius: 4px;
  border: 1px solid var(--guaxi-border);
  box-sizing: border-box;
  overflow: auto;
  word-wrap: break-word;
  overflow-wrap: break-word;
  transition: all 0.2s;
}

/* 渲染内容悬停效果 */
.card-item:hover .card-item-rendered {
  background-color: var(--guaxi-content-bg);
  border-color: var(--guaxi-border);
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
  border: 1px solid var(--guaxi-border);
}

.card-item-rendered :deep(blockquote) {
  margin: 0;
  padding: 0 1em;
  color: var(--guaxi-text-secondary);
  border-left: 0.25em solid var(--guaxi-border);
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