<template>
  <n-card>
    <!-- 当前激活的笔记库路径 -->
    <n-alert type="info" v-if="activeRepoPath" style="max-width: 95%;">
      当前激活的笔记库路径：{{ activeRepoPath }}
    </n-alert>
    <n-alert type="warning" v-else style="max-width: 95%;">
      未选择笔记库路径
    </n-alert>

    <!-- 笔记库路径列表 -->
    <n-list hoverable style="max-width: 95%;">
      <n-list-item v-for="path in repopathlist" :key="path">
        <template #prefix>
          <n-icon :component="Folder20Regular" :size="24" />
        </template>
        <n-text strong>{{ path }}</n-text>
        <template #suffix>
          <n-space :wrap="false">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button
                  text
                  @click="handleActivate(path)"
                  :disabled="path === activeRepoPath"
                >
                  <n-icon
                    :component="CheckmarkCircle20Regular"
                    :size="24"
                    v-if="path === activeRepoPath"
                    style="color: #52c41a;"
                  />
                  <n-icon
                    :component="CheckmarkCircle32Regular"
                    :size="24"
                    v-else
                    style="color: #52c41a;"
                  />
                </n-button>
              </template>
              {{ path === activeRepoPath ? '已激活' : '激活' }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handleRemove(path)">
                  <n-icon :component="Delete20Regular" :size="24" />
                </n-button>
              </template>
              移除
            </n-tooltip>
          </n-space>
        </template>
      </n-list-item>
    </n-list>

    <!-- 添加新的笔记库路径 -->
    <n-tooltip trigger="hover">
      <template #trigger>
        <n-button
          type="primary"
          text
          class="add-path-button"
          @click="handleOpenFileBrowser"
        >
          <n-icon :component="FolderAdd20Regular" :size="24" />
        </n-button>
      </template>
      添加新的笔记库路径
    </n-tooltip>

    <!-- 添加路径的模态框 -->
    <n-modal v-model:show="showAddPathModal">
      <n-card style="width: 400px;" title="添加笔记库路径">
        <n-input
          v-model:value="newRepoPath"
          placeholder="输入新的笔记库路径"
          clearable
        />
        <template #footer>
          <n-space justify="end">
            <n-button @click="showAddPathModal = false">取消</n-button>
            <n-button type="primary" @click="handleAdd">添加</n-button>
          </n-space>
        </template>
      </n-card>
    </n-modal>
  </n-card>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue';
import {
  NCard,
  NList,
  NListItem,
  NText,
  NButton,
  NIcon,
  NInput,
  NSpace,
  NAlert,
  NModal,
  NTooltip,
} from 'naive-ui';
import {
  FolderAdd20Regular,
  CheckmarkCircle20Regular,
  CheckmarkCircle32Regular,
  Delete20Regular,
  Folder20Regular,
} from '@vicons/fluent';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const fileTreeStore = useFileTreeStore();
const newRepoPath = ref('');
const showAddPathModal = ref(false);
const activeRepoPath = ref(null);

// 获取笔记库路径列表
const repopathlist = computed(() => fileTreeStore.repopathlist);

// 加载激活的笔记库路径
onMounted(async () => {
  activeRepoPath.value = await invoke('get_active_repo_path');
  const paths = await invoke('get_repopathlist');
  fileTreeStore.setRepopathlist(paths); // 更新 store 中的数据
  console.log('加载激活的笔记库路径：', fileTreeStore.repopathlist);
});

// 激活笔记库路径
const handleActivate = async (path) => {
  await invoke('set_active_repo_path', { path });
  activeRepoPath.value = path;
  fileTreeStore.setActiveRepoPath(path);
};

// 移除笔记库路径
const handleRemove = (path) => {
  fileTreeStore.removeRepoPath(path);
};

// 添加新的笔记库路径
const handleAdd = async () => {
  if (newRepoPath.value) {
    // 将路径统一转换为 Linux 系统的正斜杠模式
    const normalizedPath = newRepoPath.value.replace(/\\/g, '/');
    fileTreeStore.addRepoPath(normalizedPath);
    newRepoPath.value = '';
    showAddPathModal.value = false;
  }
};

// 打开文件浏览器选择路径
const handleOpenFileBrowser = async () => {
  try {
    // 打开文件浏览器选择文件夹
    const selectedPath = await open({
      directory: true, // 选择文件夹
      multiple: false, // 单选
    });

    if (selectedPath) {
      // 将路径统一转换为 Linux 系统的正斜杠模式
      const normalizedPath = selectedPath.replace(/\\/g, '/');
      newRepoPath.value = normalizedPath;
      showAddPathModal.value = true; // 显示模态框
    }
  } catch (error) {
    console.error('打开文件浏览器失败:', error);
  }
};
</script>

<style scoped>
.n-card {
  position: relative;
  padding: 20px;
}

.add-path-button {
  position: absolute;
  top: 20px;
  right: 20px;
}

.n-list-item {
  padding: 12px 16px;
}

.n-list-item__suffix {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>