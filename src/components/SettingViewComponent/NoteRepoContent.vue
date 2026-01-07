<template>
  <div class="preference-wrapper">
    <n-card title="笔记库与资源库">
      <n-space vertical size="large" style="width: 100%">

        <div>
          <div class="section-header">
            <h3>文件库根目录</h3>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handleOpenFileBrowserForLibraryRoot">
                  <n-icon :component="librariesRoot ? DocumentEdit16Regular : FolderAdd20Regular" :size="22" />
                </n-button>
              </template>
              {{ librariesRoot ? '更改文件库根目录' : '添加文件库根目录' }}
            </n-tooltip>
          </div>
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
            </n-list-item>
          </n-list>

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

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
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

          <n-alert v-if="resourceRoot" type="info" :show-icon="true" class="tip" style="max-width: 95%;">
            当前资源库根目录：{{ resourceRoot }}
          </n-alert>
          <n-alert v-else type="warning" :show-icon="true" class="tip" style="max-width: 95%;">
            未配置资源库根目录，将使用默认位置。
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="选择资源库根目录">
              <n-button :disabled="loading" @click="handleResourceRootPick">
                浏览文件夹...
              </n-button>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>模板库</h3>
          <p class="description">
            用于存放 Markdown 模板文件。未配置时启动会自动在程序同级目录创建 Template 文件夹并写入配置。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="模板库路径">
              <n-input v-model:value="templateRepoPath" :disabled="loading" placeholder="模板库路径" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" @click="handleTemplateRepoPick">浏览文件夹...</n-button>
                <n-button :disabled="loading" type="primary" @click="handleTemplateRepoSave">保存</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>知识库远程仓库</h3>
          <p class="description">
            配置后将应用到 KnowledgeBases 根目录下的 Git 仓库，可用于后续 git push / git pull。
          </p>

          <n-alert type="info" v-if="knowledgeBasesRemoteUrl" style="max-width: 95%;">
            当前远程仓库：{{ knowledgeBasesRemoteUrl }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置远程仓库地址
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="远程仓库地址">
              <n-input v-model:value="knowledgeBasesRemoteUrl" :disabled="loading" placeholder="https://..." />
            </n-form-item>
            <n-form-item label="远程仓库用户名">
              <n-input v-model:value="knowledgeBasesRemoteUsername" :disabled="loading" placeholder="可选" />
            </n-form-item>
            <n-form-item label="Git 用户名">
              <n-input v-model:value="knowledgeBasesGitUsername" :disabled="loading"
                placeholder="用于提交, 例如: Your Name" />
            </n-form-item>
            <n-form-item label="Git Email">
              <n-input v-model:value="knowledgeBasesGitEmail" :disabled="loading"
                placeholder="用于提交, 例如: you@example.com" />
            </n-form-item>
            <n-form-item label="访问秘钥">
              <n-input v-model:value="knowledgeBasesRemoteToken" :disabled="loading" type="password"
                :placeholder="knowledgeBasesRemoteHasToken ? '已配置（留空可保持不变）' : '请输入访问秘钥'" show-password-on="click" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" type="primary" @click="handleKnowledgeBasesRemoteSave">保存并应用</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>
      </n-space>
    </n-card>

  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
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
  NTooltip,
  NDivider,
  NForm,
  NFormItem,
} from 'naive-ui';
import {
  FolderAdd20Regular,
  Folder20Regular,
  DocumentEdit16Regular,
} from '@vicons/fluent';
import { repoStore } from '../../stores/repoStore'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const store = repoStore();

// 文件库根目录
const librariesRoot = ref('');

// 资源库根目录
const resourceRoot = ref('');

// 知识库根目录
const knowledgeBasesRoot = ref('');

// 模板库路径
const templateRepoPath = ref('');

// 知识库远程仓库配置
const knowledgeBasesRemoteUrl = ref('');
const knowledgeBasesRemoteUsername = ref('');
const knowledgeBasesRemoteToken = ref('');
const knowledgeBasesRemoteHasToken = ref(false);
const knowledgeBasesGitUsername = ref('');
const knowledgeBasesGitEmail = ref('');

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

// 加载配置
onMounted(async () => {
  librariesRoot.value = await invoke('get_libraries_root');

  try {
    knowledgeBasesRoot.value = await invoke('get_knowledge_bases_root');
  } catch (error) {
    console.error('加载知识库根目录失败:', error);
    knowledgeBasesRoot.value = '';
  }

  try {
    const remoteConfig = await invoke('get_knowledge_bases_remote_config');
    const remoteUrl = remoteConfig?.remoteUrl ?? remoteConfig?.remote_url;
    const hasToken = remoteConfig?.hasToken ?? remoteConfig?.has_token;
    const gitUsername = remoteConfig?.gitUsername ?? remoteConfig?.git_username;
    const gitEmail = remoteConfig?.gitEmail ?? remoteConfig?.git_email;

    knowledgeBasesRemoteUrl.value = remoteUrl ? String(remoteUrl) : '';
    knowledgeBasesRemoteUsername.value = remoteConfig?.username ? String(remoteConfig.username) : '';
    knowledgeBasesRemoteHasToken.value = !!hasToken;
    knowledgeBasesRemoteToken.value = '';
    knowledgeBasesGitUsername.value = gitUsername ? String(gitUsername) : '';
    knowledgeBasesGitEmail.value = gitEmail ? String(gitEmail) : '';
  } catch (error) {
    console.error('加载知识库远程仓库配置失败:', error);
    knowledgeBasesRemoteUrl.value = '';
    knowledgeBasesRemoteUsername.value = '';
    knowledgeBasesRemoteHasToken.value = false;
    knowledgeBasesRemoteToken.value = '';
    knowledgeBasesGitUsername.value = '';
    knowledgeBasesGitEmail.value = '';
  }

  try {
    const tplPath = await invoke('get_template_repo_path');
    templateRepoPath.value = typeof tplPath === 'string' ? tplPath : '';
  } catch (error) {
    console.error('加载模板库路径失败:', error);
    templateRepoPath.value = '';
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

const handleTemplateRepoPick = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (!selectedPath) return;

    loading.value = true;
    await invoke('set_template_repo_path', { path: selectedPath });
    templateRepoPath.value = selectedPath;
  } catch (error) {
    console.error('更新模板库路径失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleTemplateRepoSave = async () => {
  try {
    loading.value = true;
    await invoke('set_template_repo_path', { path: templateRepoPath.value });
  } catch (error) {
    console.error('更新模板库路径失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleKnowledgeBasesRemoteSave = async () => {
  try {
    loading.value = true;
    await invoke('set_knowledge_bases_remote_config', {
      remoteUrl: knowledgeBasesRemoteUrl.value,
      username: knowledgeBasesRemoteUsername.value,
      token: knowledgeBasesRemoteToken.value,
      gitUsername: knowledgeBasesGitUsername.value,
      gitEmail: knowledgeBasesGitEmail.value,
    });

    knowledgeBasesRemoteHasToken.value = knowledgeBasesRemoteHasToken.value || !!knowledgeBasesRemoteToken.value.trim();
    knowledgeBasesRemoteToken.value = '';
  } catch (error) {
    console.error('保存知识库远程仓库配置失败:', error);
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
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  max-width: 95%;
}

.section-header h3 {
  margin: 0;
}

.description {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
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