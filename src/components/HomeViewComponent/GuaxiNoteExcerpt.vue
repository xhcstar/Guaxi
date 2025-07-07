<template>
  <div class="excerpt-navigator">
    <n-empty v-if="excerptResults.length === 0" description="暂无摘录笔记搜索结果" style="margin-top: 20px">
      <template #extra>
        <n-text depth="3">尝试不同的搜索词</n-text>
      </template>
    </n-empty>
    <n-list v-else hoverable clickable>
      <n-list-item v-for="item in excerptResults" :key="item.file_path" @click="openExcerpt(item)">
        <div class="excerpt-result-item">
          <div class="file-name" :title="item.file_path">
            <IconifyIcon v-bind="getFileIconPropsWithLog(item.file_path)" />
            <span class="filename-text">{{ item.file_name }}</span>
          </div>
          <!-- <div class="file-path" :title="item.file_path">
            {{ item.file_path }}
          </div> -->
          <div class="excerpt-actions">
            <n-button size="tiny" type="primary" tertiary @click.stop="insertExcerpt(item)">
              <template #icon>
                <n-icon :component="TextGrammarArrowLeft20Regular" />
              </template>
              插入
            </n-button>
          </div>
        </div>
      </n-list-item>
    </n-list>
  </div>
</template>

<script setup>
import { h, defineComponent, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import {
  NEmpty,
  NText,
  NList,
  NListItem,
  NButton,
  NIcon
} from 'naive-ui';
import { TextGrammarArrowLeft20Regular } from '@vicons/fluent';
import { useFileTreeStore } from '../../stores/fileTreeStore';
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

const fileTreeStore = useFileTreeStore();
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

const openExcerpt = async (item) => {
  try {
    // 通过 fileTreeStore 统一打开摘录文件
    const content = await fileTreeStore.openFileByPath(item.file_path);

    editorStore.setFile({
      path: item.file_path,
      content: content || '',
    });

    window.$message?.success('已打开摘录文件');
  } catch (err) {
    console.error('打开摘录文件失败:', err);
    window.$message?.error('打开摘录文件失败');
  }
};

const insertExcerpt = (item) => {
  // 插入Markdown格式的链接
  const content = `[${item.file_name.replace('.json', '')}](${item.file_path})`;
  editorStore.insertContent(content);
  window.$message?.success('摘录已插入');
};

// 监听搜索结果变化，有结果时自动打开右侧边栏
watch(() => props.excerptResults, (newVal) => {
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
  max-width: 600px;
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
  gap: 10px;
  font-size: 14px;
  font-weight: 500;
  color: var(--text-color);
  width: 100%;
  white-space: nowrap;
}

.filename-text {
  display: inline;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-path {
  font-size: 12px;
  color: #666;
  width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-left: 28px;
  line-height: 1.5;
}

.excerpt-actions {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
}
</style>