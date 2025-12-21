<template>
  <div class="preference-wrapper">
    <n-card title="笔记库与资源库">
      <n-space vertical size="large" style="width: 100%">
        <div>
          <h3>笔记库列表</h3>
          <p class="description">
            管理当前可用的笔记库路径，选择一个作为激活笔记库进行阅读和编辑。
          </p>

          <n-alert type="info" v-if="activeRepoPath" style="max-width: 95%;">
            当前激活的笔记库路径：{{ activeRepoPath }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未选择笔记库路径
          </n-alert>

          <n-list hoverable style="max-width: 95%; margin-top: 8px;">
            <n-list-item v-for="path in repopathlist" :key="path">
              <template #prefix>
                <n-icon :component="Folder20Regular" :size="24" />
              </template>
              <n-text strong>{{ path }}</n-text>
              <template #suffix>
                <n-space :wrap="false">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="handleActivate(path)" :disabled="path === activeRepoPath">
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

          <n-space style="margin-top: 8px">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handleOpenFileBrowser">
                  <n-icon :component="FolderAdd20Regular" :size="24" />
                </n-button>
              </template>
              添加新的笔记库路径
            </n-tooltip>
          </n-space>
        </div>

        <n-divider />

        <div>
          <h3>文件库根目录</h3>
          <p class="description">
            用于存放导入的文件库数据。建议选择一个稳定且有足够空间的磁盘目录。
          </p>

          <n-alert type="info" v-if="librariesRoot" style="max-width: 95%;">
            当前文件库根目录：{{ librariesRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置文件库根目录
          </n-alert>

          <n-list hoverable style="max-width: 95%; margin-top: 8px;">
            <n-list-item v-if="librariesRoot">
              <template #prefix>
                <n-icon :component="Folder20Regular" :size="24" />
              </template>
              <n-text strong>{{ librariesRoot }}</n-text>
              <template #suffix>
                <n-space :wrap="false">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="handleOpenFileBrowserForLibraryRoot">
                        <n-icon :component="FolderAdd20Regular" :size="24" />
                      </n-button>
                    </template>
                    更改路径
                  </n-tooltip>
                </n-space>
              </template>
            </n-list-item>
          </n-list>

          <n-space style="margin-top: 8px">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handleOpenFileBrowserForLibraryRoot">
                  <n-icon :component="FolderAdd20Regular" :size="24" />
                </n-button>
              </template>
              {{ librariesRoot ? '更改文件库根目录' : '添加文件库根目录' }}
            </n-tooltip>
          </n-space>
        </div>

        <n-divider />

        <div>
          <h3>知识库根目录</h3>
          <p class="description">
            用于存放知识库数据。未配置时将使用程序同级目录下的 KnowledgeBases 文件夹。
          </p>

          <n-alert type="info" v-if="knowledgeBasesRoot" style="max-width: 95%;">
            当前知识库根目录：{{ knowledgeBasesRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置知识库根目录，将使用默认位置。
          </n-alert>

          <n-form
            label-placement="left"
            label-width="auto"
            :show-require-mark="false"
            style="margin-top: 8px"
          >
            <n-form-item label="选择知识库根目录">
              <n-button :disabled="loading" @click="handleKnowledgeBasesRootPick">
                浏览文件夹...
              </n-button>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>资源库根目录</h3>
          <p class="description">
            用于存放截图、图片等资源的根目录。未配置时将使用程序所在目录下的 ResourceRepo 文件夹。
          </p>

          <n-alert
            v-if="resourceRoot"
            type="info"
            :show-icon="true"
            class="tip"
            style="max-width: 95%;"
          >
            当前资源库根目录：{{ resourceRoot }}
          </n-alert>
          <n-alert
            v-else
            type="warning"
            :show-icon="true"
            class="tip"
            style="max-width: 95%;"
          >
            未配置资源库根目录，将使用默认位置。
          </n-alert>

          <n-form
            label-placement="left"
            label-width="auto"
            :show-require-mark="false"
            style="margin-top: 8px"
          >
            <n-form-item label="选择资源库根目录">
              <n-button :disabled="loading" @click="handleResourceRootPick">
                浏览文件夹...
              </n-button>
            </n-form-item>
          </n-form>
        </div>
      </n-space>
    </n-card>

    <n-modal v-model:show="showAddPathModal">
      <n-card style="width: 400px;" title="添加笔记库路径">
        <n-input v-model:value="newRepoPath" placeholder="输入新的笔记库路径" clearable />
        <template #footer>
          <n-space justify="end">
            <n-button @click="showAddPathModal = false">取消</n-button>
            <n-button type="primary" @click="handleAdd">添加</n-button>
          </n-space>
        </template>
      </n-card>
    </n-modal>
  </div>
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
  NDivider,
  NForm,
  NFormItem,
} from 'naive-ui';
import {
  FolderAdd20Regular,
  CheckmarkCircle20Regular,
  CheckmarkCircle32Regular,
  Delete20Regular,
  Folder20Regular,
} from '@vicons/fluent';
import { useFileTreeStore } from '../../stores/fileTreeStore';
import { repoStore } from '../../stores/repoStore'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const fileTreeStore = useFileTreeStore();
const store = repoStore();
const newRepoPath = ref('');
const showAddPathModal = ref(false);
const activeRepoPath = ref(null);

// 文件库根目录
const librariesRoot = ref('');

// 资源库根目录
const resourceRoot = ref('');

// 知识库根目录
const knowledgeBasesRoot = ref('');

const loading = ref(false);

// 打开文件浏览器选择文件库根目录
const handleOpenFileBrowserForLibraryRoot = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (selectedPath) {
      await invoke('set_libraries_root', { path: selectedPath });
      librariesRoot.value = selectedPath;
      store.set_librariesRoot(selectedPath);
      console.log('文件库根目录已更新为:', store.librariesRoot);
    }
  } catch (error) {
    console.error('打开文件浏览器失败:', error);
  }
};

// 获取笔记库路径列表
const repopathlist = computed(() => fileTreeStore.repopathlist);

// 加载激活的笔记库路径及配置
onMounted(async () => {
  activeRepoPath.value = await invoke('get_active_repo_path');
  const paths = await invoke('get_repopathlist');
  fileTreeStore.setRepopathlist(paths);
  console.log('加载激活的笔记库路径：', fileTreeStore.repopathlist);
  librariesRoot.value = await invoke('get_libraries_root');

  try {
    knowledgeBasesRoot.value = await invoke('get_knowledge_bases_root');
  } catch (error) {
    console.error('加载知识库根目录失败:', error);
    knowledgeBasesRoot.value = '';
  }

  try {
    loading.value = true;
    const imageBedRoot = await invoke('get_image_bed_root');
    if (typeof imageBedRoot === 'string') {
      resourceRoot.value = imageBedRoot;
    } else {
      resourceRoot.value = '';
    }
  } catch (error) {
    console.error('加载资源库根目录失败:', error);
    resourceRoot.value = '';
  } finally {
    loading.value = false;
  }
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
    fileTreeStore.addRepoPath(newRepoPath.value);
    newRepoPath.value = '';
    showAddPathModal.value = false;
  }
};

// 打开文件浏览器选择笔记库路径
const handleOpenFileBrowser = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (selectedPath) {
      newRepoPath.value = selectedPath;
      showAddPathModal.value = true;
    }
  } catch (error) {
    console.error('打开文件浏览器失败:', error);
  }
};

const handleKnowledgeBasesRootPick = async () => {
  try {
    loading.value = true;
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });
    if (selectedPath) {
      await invoke('set_knowledge_bases_root', { path: selectedPath });
      knowledgeBasesRoot.value = selectedPath;
    }
  } catch (error) {
    console.error('更新知识库根目录失败:', error);
  } finally {
    loading.value = false;
  }
};

// 打开文件浏览器选择资源库根目录
const handleResourceRootPick = async () => {
  try {
    loading.value = true;
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });
    if (selectedPath) {
      await invoke('set_image_bed_root', { path: selectedPath });
      resourceRoot.value = selectedPath;
    }
  } catch (error) {
    console.error('更新资源库根目录失败:', error);
  } finally {
    loading.value = false;
  }
};
</script>

<style scoped>
.preference-wrapper {
  width: 100%;
}

.description {
  margin-top: 4px;
  color: #666;
  font-size: 13px;
}

.tip {
  margin-top: 8px;
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