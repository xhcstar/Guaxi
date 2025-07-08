<template>
  <div class="excerpt-navigator">
    <n-empty v-if="excerptResults.length === 0" description="暂无摘录笔记搜索结果" style="margin-top: 20px">
      <template #extra>
        <n-text depth="3">尝试不同的搜索词</n-text>
      </template>
    </n-empty>
    <n-list v-else hoverable clickable>
      <n-list-item v-for="item in excerptResults" :key="item.file_path">
        <div class="excerpt-result-item">
          <div class="file-name" :title="item.file_path">
            <!-- 将展开箭头移到最前面 -->
            <n-icon :component="item.expanded ? ChevronDown : ChevronRight" size="12" @click="toggleFileExpand(item)"
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

        <!-- 展开的文件内容区域 -->
        <div v-if="item.expanded" class="file-content">
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
                      <n-button size="tiny" type="primary" tertiary @click.stop="insertCardItem(cardItem)"
                        class="card-item-insert">
                        <template #icon>
                          <n-icon :component="TextGrammarArrowLeft20Regular" size="14" />
                        </template>
                        插入
                      </n-button>
                    </div>
                    <div class="card-item-content">
                      {{ cardItem.content }}
                    </div>
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
  NSpin
} from 'naive-ui';
import { TextGrammarArrowLeft20Regular } from '@vicons/fluent';
import { ChevronDown, ChevronRight } from '@vicons/fa';
import { useEditorStore } from '../../stores/editorStore';
import { useRightSiderStore } from '../../stores/rightSiderStore';
import { useSiderStore } from '../../stores/useSiderStore';

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

// 切换文件展开状态
const toggleFileExpand = async (item) => {
  item.expanded = !item.expanded;
  if (item.expanded && !item.content) {
    try {
      item.loading = true;
      // 使用 Tauri 接口读取摘录文件
      item.content = await invoke('read_excerpt_file', {
        path: item.file_path
      });
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

const insertExcerpt = (item) => {
  const content = `[${item.file_name.replace('.json', '')}](${item.file_path})`;
  editorStore.insertContent(content);
  window.$message?.success('摘录已插入');
};

// 插入卡片项内容到编辑器
const insertCardItem = (cardItem) => {
  let contentToInsert = '';

  // 根据不同类型格式化内容
  switch (cardItem.type) {
    case 'text':
      contentToInsert = cardItem.content;
      break;
    case 'math':
      contentToInsert = `${cardItem.content}`;
      break;
    case 'code':
      contentToInsert = `${cardItem.content}`;
      break;
    case 'image':
      contentToInsert = `${cardItem.content}`;
      break;
    case 'diagram':
      contentToInsert = `${cardItem.content}`;
      break;
    default:
      contentToInsert = cardItem.content;
  }

  // 插入到编辑器
  editorStore.insertContent(contentToInsert);
  window.$message?.success('内容已插入');
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

.n-list-item {
  margin: 0 !important;
  padding: 2px 0 !important;
}

.excerpt-result-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  max-width: 512px;
  width: 100%;
  box-sizing: border-box;
  background: var(--n-card-color, #fff);
  border-radius: 6px;
  box-shadow: none;
  padding: 8px 12px;
  margin-bottom: 2px;
  transition: box-shadow 0.2s;
  position: relative;
}

.n-list-item:hover .excerpt-result-item {
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.2), 0 2px 8px 0 rgba(7, 118, 161, 0.1);
  background: #f0f7ff;
  transition: box-shadow 0.2s, background 0.2s;
}

.file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 500;
  color: var(--text-color);
  width: 100%;
  white-space: nowrap;
  cursor: pointer;
}

.filename-text {
  display: inline;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-grow: 1;
}

.insert-btn {
  margin-left: 8px;
  flex-shrink: 0;
}

.expand-icon {
  flex-shrink: 0;
  cursor: pointer;
}

.file-content {
  padding: 2px;
  background-color: #f8fafc;
  border-radius: 6px;
  margin-top: 8px;
}

.card-container {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.excerpt-card {
  margin-bottom: 12px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-title {
  font-weight: 500;
  flex-grow: 1;
}

.card-timestamp {
  font-size: 12px;
  color: #666;
}

.card-items {
  margin-top: 12px;
}

.card-item {
  padding: 8px;
  margin-bottom: 8px;
  border-bottom: 1px solid #eee;
}

.card-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.card-item-type {
  font-size: 12px;
  color: #888;
}

.card-item-insert {
  margin-left: 8px;
}

.card-item-content {
  margin: 8px 0;
  white-space: pre-wrap;
}

.card-item-timestamp {
  font-size: 11px;
  color: #999;
  text-align: right;
}
</style>