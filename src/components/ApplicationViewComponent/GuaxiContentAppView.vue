<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider v-model:collapsed="isLeftSiderCollapsed" :width="leftSiderbarWidth" collapse-mode="width"
        :collapsed-width="0" bordered content-style="padding: 0px;">
        <GuaxiSidebarAppView
          :categories="categories"
          v-model:selectedCategory="selectedCategory"
        />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </n-layout-sider>
      <GuaxiCenterAppView
        :apps="apps"
        :selected-category="selectedCategory"
        @open-app="handleOpenApp"
      />
      <n-layout has-sider sider-placement="right">
        <n-layout-sider v-model:collapsed="isRightSiderCollapsed" :width="rightSiderbarWidth" collapse-mode="width"
          :collapsed-width="0" bordered content-style="padding: 0px;">
          <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar">
            <GuaxiRightContentAppView />
          </div>
        </n-layout-sider>
      </n-layout>
    </n-layout>
  </div>
</template>


<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onBeforeUnmount, onMounted } from "vue"
import { invoke } from '@tauri-apps/api/core'
import { useSiderStore } from '../../stores/useSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import GuaxiSidebarAppView from "./GuaxiSidebarAppView.vue"
import GuaxiCenterAppView from "./GuaxiCenterAppView.vue"
import GuaxiRightContentAppView from "./GuaxiRightContentAppView.vue"
import LifeAlbumApp from "./apps/LifeAlbumApp.vue"
import MusicPlayerApp from "./apps/MusicPlayerApp.vue"
import LanguageTranslatorApp from "./apps/LanguageTranslatorApp.vue"
import FileTransferAssistantApp from "./apps/FileTransferAssistantApp.vue"
// import PythonPlaygroundApp from "./apps/PythonPlaygroundApp.vue"
// import PlaceholderApp from "./apps/PlaceholderApp.vue"
import TTSBookApp from "./apps/TTSBookApp.vue"
import OcrApp from "./apps/OcrApp.vue"
import ResumeBuilderApp from "./apps/ResumeBuilderApp.vue"
import AIFilmStudioApp from "./apps/AIFilmStudioApp.vue"
import RemoteTerminalApp from "./apps/RemoteTerminalApp.vue"
import ResourceHunterApp from "./apps/ResourceHunterApp.vue"
import MailApp from "./apps/MailApp.vue"

const logger = createLogger('GuaxiContentAppView')
const settingsStore = useSettingsStore()

const categories = [
  { key: 'creation', label: '创作类' },
  { key: 'tools', label: '工具类' },
  { key: 'language', label: '语言类' },
  { key: 'music', label: '音乐类' },
  { key: 'work', label: '工作类' },
  { key: 'life', label: '生活类' }
]

const apps = [
  {
    id: 'tools-mail',
    name: '邮件',
    category: 'work',
    categoryLabel: '工作类',
    description: '多账户 IMAP/SMTP 客户端：收信规则、分类标签、延迟发送与撤回，全链路支持代理',
    icon: '✉',
    color: 'linear-gradient(135deg, #5b7cfa, #2b3a67)',
    component: MailApp
  },
  {
    id: 'tools-resource-hunter',
    name: '资源嗅探下载',
    category: 'tools',
    categoryLabel: '工具类',
    description: '全网搜索书籍/论文/视频/音频/图片，嗅探直链，多线程断点续传下载并统一管理',
    icon: '🧲',
    color: 'linear-gradient(135deg, #30cfd0, #330867)',
    component: ResourceHunterApp
  },
  {
    id: 'creation-ai-film-studio',
    name: 'AI 影像工坊',
    category: 'creation',
    categoryLabel: '创作类',
    description: '将 Markdown、史料、地理数据和小说编译为 Blender 分镜、预演与电影制作项目',
    icon: '🎬',
    color: 'linear-gradient(135deg, #667eea, #302b63)',
    component: AIFilmStudioApp
  },
  {
    id: 'life-album',
    name: '相册',
    category: 'life',
    categoryLabel: '生活类',
    description: '管理和浏览你的本地照片和截图',
    icon: '🖼️',
    color: 'linear-gradient(135deg, #fbc2eb, #a6c1ee)',
    component: LifeAlbumApp
  },
  {
    id: 'music-player',
    name: '音乐播放器',
    category: 'music',
    categoryLabel: '音乐类',
    description: '播放本地或在线音乐，支持播放列表和收藏',
    icon: '🎵',
    color: 'linear-gradient(135deg, #667eea, #764ba2)',
    component: MusicPlayerApp
  },
  {
    id: 'language-translator',
    name: '翻译助手',
    category: 'language',
    categoryLabel: '语言类',
    description: '多语言互译与单词收藏，支持常用语速查',
    icon: '🌐',
    color: 'linear-gradient(135deg, #42e695, #3bb2b8)',
    component: LanguageTranslatorApp
  },
  {
    id: 'tools-file-transfer',
    name: '文件传输助手',
    category: 'tools',
    categoryLabel: '工具类',
    description: '局域网/互联网互传文件，支持多任务与断点续传',
    icon: '📤',
    color: 'linear-gradient(135deg, #ff9a9e, #fecfef)',
    component: FileTransferAssistantApp
  },
  // {
  //   id: 'tools-python-playground',
  //   name: 'Python Playground',
  //   category: 'tools',
  //   categoryLabel: '工具类',
  //   description: '运行/调试 Python 脚本：自动化、数据分析与智能处理',
  //   icon: '🐍',
  //   color: 'linear-gradient(135deg, #a1c4fd, #c2e9fb)',
  //   component: PythonPlaygroundApp
  // },
  {
    id: 'tools-tts-book',
    name: '有声书 / 配音工坊',
    category: 'tools',
    categoryLabel: '工具类',
    description: '导入 Word/PDF/Markdown/TXT/EPUB，自动清洗并合成完整语音',
    icon: '🎙️',
    color: 'linear-gradient(135deg, #f6d365, #fda085)',
    component: TTSBookApp
  },
  {
    id: 'tools-ocr',
    name: '图片文字识别',
    category: 'tools',
    categoryLabel: '工具类',
    description: '离线 OCR：截图/图片一键提取文字，中文与多语言',
    icon: '🔤',
    color: 'linear-gradient(135deg, #84fab0, #8fd3f4)',
    component: OcrApp
  },
  {
    id: 'tools-remote-terminal',
    name: '终端调试工坊',
    category: 'tools',
    categoryLabel: '工具类',
    description: 'SSH/串口/Telnet 连接远端，捕获海量输出并做检索、聚类、时序图表与 GDB 调试分析',
    icon: '🖥️',
    color: 'linear-gradient(135deg, #30cfd0, #330867)',
    component: RemoteTerminalApp
  },
  // {
  //   id: 'work-kanban',
  //   name: '任务看板',
  //   category: 'work',
  //   categoryLabel: '工作类',
  //   description: '用看板视图管理任务和进度',
  //   icon: '📌',
  //   color: 'linear-gradient(135deg, #89f7fe, #66a6ff)',
  //   component: PlaceholderApp
  // },
  {
    id: 'work-resume',
    name: '简历工坊',
    category: 'work',
    categoryLabel: '工作类',
    description: '多份简历管理：版本历史、模板库与 HTML / PDF 导出',
    icon: '📝',
    color: 'linear-gradient(135deg, #fddb92, #d1fdff)',
    component: ResumeBuilderApp
  }
]

// 影像工坊是当前重点创作应用，首次进入 App 页面默认展示创作类。
const selectedCategory = ref('creation');

// 关键日志：发布包出现“新应用卡片不可见”时，可从前端日志确认实际加载的应用注册表，
// 快速区分旧 dist/旧进程与分类过滤逻辑错误。
onMounted(() => {
  logger.info('[ApplicationView] application registry mounted', {
    selectedCategory: selectedCategory.value,
    categories: categories.map(item => item.key),
    appIds: apps.map(item => item.id),
  })
})

const handleOpenApp = async (app) => {
  try {
    await invoke('open_application_window', {
      appId: app.id,
      title: app.name,
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    logger.error('打开应用窗口失败:', err)
  }
}

const leftSiderbarWidth = ref(350); // 默认宽度
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);
const rightSiderbarWidth = ref(650); // 默认宽度
const rightSidebarresizing = ref(false);
const rightSidebarStartX = ref(0);
const rightSidebarStartWidth = ref(0);
const viewKey = 'app'
const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false, // 默认展开
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true, // 默认隐藏
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
})

const onMouseDownLeftSidebar = (event) => {
  leftSidebarresizing.value = true;
  leftSidebarStartX.value = event.clientX;
  leftSidebarStartWidth.value = leftSiderbarWidth.value;
  // Attach mousemove and mouseup listeners
  document.addEventListener("mousemove", onMouseMoveLeftSidebar);
  document.addEventListener("mouseup", onMouseUpLeftSidebar);
};

const onMouseMoveLeftSidebar = (event) => {
  if (!leftSidebarresizing.value) return;
  const delta = event.clientX - leftSidebarStartX.value;
  leftSiderbarWidth.value = Math.max(250, leftSidebarStartWidth.value + delta); // 设置最小宽度为200px
};

const onMouseUpLeftSidebar = () => {
  leftSidebarresizing.value = false;
  // Remove mousemove and mouseup listeners
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
  document.removeEventListener("mouseup", onMouseUpLeftSidebar);
};

const onMouseDownRightSidebar = (event) => {
  logger.info("onMouseDownRightSidebar");
  rightSidebarresizing.value = true;
  rightSidebarStartX.value = event.clientX;
  rightSidebarStartWidth.value = rightSiderbarWidth.value;

  // Attach mousemove and mouseup listeners
  document.addEventListener("mousemove", onMouseMoveRightSidebar);
  document.addEventListener("mouseup", onMouseUpRightSidebar);
};

const onMouseMoveRightSidebar = (event) => {
  if (!rightSidebarresizing.value) return;
  const delta = rightSidebarStartX.value - event.clientX;
  rightSiderbarWidth.value = Math.max(250, rightSidebarStartWidth.value + delta); // 设置最小宽度为200px
};

const onMouseUpRightSidebar = () => {
  rightSidebarresizing.value = false;
  // Remove mousemove and mouseup listeners
  document.removeEventListener("mousemove", onMouseMoveRightSidebar);
  document.removeEventListener("mouseup", onMouseUpRightSidebar);
};

// Cleanup listeners when the component is unmounted
onBeforeUnmount(() => {
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
  document.removeEventListener("mouseup", onMouseUpLeftSidebar);
  document.removeEventListener("mousemove", onMouseMoveRightSidebar);
  document.removeEventListener("mouseup", onMouseUpRightSidebar);
});

</script>


<style scoped>
.n-layout-sider {
  background: var(--guaxi-panel-bg);
}

.n-layout-content {
  background: var(--guaxi-content-bg);
}

.guaxi-content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
  margin: 0;
  padding: 0;
}

.guaxi-content :deep(.n-layout) {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.guaxi-content :deep(.n-layout-content) {
  min-height: 0;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-border);
}

.sider-resize-handle-right {
  position: relative;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-border);
}
</style>
