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
import { ref, computed, onBeforeUnmount } from "vue"
import { invoke } from '@tauri-apps/api/core'
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebarAppView from "./GuaxiSidebarAppView.vue"
import GuaxiCenterAppView from "./GuaxiCenterAppView.vue"
import GuaxiRightContentAppView from "./GuaxiRightContentAppView.vue"
import LifeAlbumApp from "./apps/LifeAlbumApp.vue"
import MusicPlayerApp from "./apps/MusicPlayerApp.vue"
import LanguageTranslatorApp from "./apps/LanguageTranslatorApp.vue"
import PlaceholderApp from "./apps/PlaceholderApp.vue"


const categories = [
  { key: 'tools', label: '工具类' },
  { key: 'language', label: '语言类' },
  { key: 'music', label: '音乐类' },
  { key: 'work', label: '工作类' },
  { key: 'life', label: '生活类' }
]

const apps = [
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
    id: 'life-habits',
    name: '习惯打卡',
    category: 'life',
    categoryLabel: '生活类',
    description: '培养每日好习惯，记录打卡进度',
    icon: '📅',
    color: 'linear-gradient(135deg, #f6d365, #fda085)'
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
    id: 'tools-screenshot',
    name: '截图工具',
    category: 'tools',
    categoryLabel: '工具类',
    description: '快速截屏并保存到项目目录',
    icon: '📸',
    color: 'linear-gradient(135deg, #ff9a9e, #fecfef)',
    component: PlaceholderApp
  },
  {
    id: 'tools-table',
    name: '数据表格',
    category: 'tools',
    categoryLabel: '工具类',
    description: '查看和过滤结构化数据表格',
    icon: '📊',
    color: 'linear-gradient(135deg, #a18cd1, #fbc2eb)',
    component: PlaceholderApp
  },
  {
    id: 'work-kanban',
    name: '任务看板',
    category: 'work',
    categoryLabel: '工作类',
    description: '用看板视图管理任务和进度',
    icon: '📌',
    color: 'linear-gradient(135deg, #89f7fe, #66a6ff)',
    component: PlaceholderApp
  },
  {
    id: 'work-timer',
    name: '简历生成',
    category: 'work',
    categoryLabel: '工作类',
    description: '根据模板快速生成与导出简历',
    icon: '📝',
    color: 'linear-gradient(135deg, #fddb92, #d1fdff)',
    component: PlaceholderApp
  }
]

const selectedCategory = ref('life');

const handleOpenApp = async (app) => {
  try {
    await invoke('open_application_window', {
      appId: app.id,
      title: app.name,
    })
  } catch (err) {
    console.error('打开应用窗口失败:', err)
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
  console.log("onMouseDownRightSidebar");
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
  background: rgba(239, 243, 239, 0.3);
}

.n-layout-content {
  background: rgba(239, 243, 239, 0.3);
}

.guaxi-content {
  flex: 1 1 auto; /* 填充剩余空间 */
  display: flex;
  flex-direction: column;
  background-color: #ffffff; /* 示例背景色 */
  overflow-y: auto; /* 如果内容超出，允许垂直滚动 */
  margin: 0;
  padding: 0;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: #f8fdfa;
}

.sider-resize-handle-right {
  position: relative;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: #f8fdfa;
}
</style>