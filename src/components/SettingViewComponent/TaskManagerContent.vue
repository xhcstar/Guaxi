<template>
  <div class="preference-wrapper">
    <n-card title="任务管理">
      <n-space vertical size="large" style="width: 100%">
        <div>
          <div class="section-header">
            <h3>任务库根目录</h3>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handlePickRoot">
                  <n-icon :component="taskRoot ? DocumentEdit16Regular : FolderAdd20Regular" :size="22" />
                </n-button>
              </template>
              更改任务库根目录
            </n-tooltip>
          </div>
          <p class="description">
            用于存放任务数据（org 文件）。未配置时将使用程序同级目录下的
            <n-text code>task</n-text> 文件夹。根目录下自动创建
            <n-text code>work</n-text>、<n-text code>life</n-text>、<n-text code>study</n-text>
            三个子目录，分别对应「工作 / 生活 / 学习」。
          </p>

          <n-alert type="info" v-if="taskRoot" style="max-width: 95%;">
            当前任务库根目录：{{ taskRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置任务库根目录，将使用默认位置。
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="任务库根目录">
              <n-input v-model:value="taskRoot" :disabled="loading" placeholder="任务库根目录（留空使用默认 task 目录）" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" @click="handlePickRoot">浏览文件夹...</n-button>
                <n-button :disabled="loading" type="primary" @click="handleSaveRoot">保存</n-button>
                <n-button :disabled="loading" @click="handleResetDefault">恢复默认</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>任务分类目录</h3>
          <p class="description">
            目录使用英文路径，界面展示中文描述。点击下方「初始化目录」可立即在磁盘创建三类子目录。
          </p>

          <n-list hoverable style="max-width: 95%; margin-top: 8px;">
            <n-list-item v-for="cat in categoryRows" :key="cat.key">
              <template #prefix>
                <n-icon :component="cat.icon" :size="24" :color="cat.color" />
              </template>
              <n-space vertical :size="2" style="flex: 1; min-width: 0;">
                <n-space align="center" :size="6">
                  <n-text strong>{{ cat.zh }}</n-text>
                  <n-tag size="tiny" :bordered="false" type="info">{{ cat.key }}</n-tag>
                </n-space>
                <n-text depth="3" style="font-size: 12px; word-break: break-all;">
                  {{ cat.path || '保存后生成' }}
                </n-text>
              </n-space>
            </n-list-item>
          </n-list>

          <n-space style="margin-top: 10px;">
            <n-button :disabled="loading" @click="handleEnsureDirs">初始化目录</n-button>
            <n-button :disabled="loading" @click="handleScan">立即扫描索引</n-button>
          </n-space>
          <n-text v-if="scanResult !== null" depth="3" style="display:block;margin-top:8px;font-size:12px;">
            上次扫描：索引到 {{ scanResult }} 个任务节点。
          </n-text>
        </div>

        <n-divider />

        <div>
          <h3>任务库远程仓库</h3>
          <p class="description">
            配置后将应用到任务库根目录下的 Git 仓库。提交 / 推送 / 拉取 直接调用系统 Git，使用你的本机 Git 配置与凭据环境。
          </p>

          <n-alert type="info" v-if="gitRemoteUrl" style="max-width: 95%;">
            当前远程仓库：{{ gitRemoteUrl }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置远程仓库地址
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="远程仓库地址">
              <n-input v-model:value="gitRemoteUrl" :disabled="loading" placeholder="https://..." />
            </n-form-item>
            <n-form-item label="Git 用户名">
              <n-input v-model:value="gitUsername" :disabled="loading" placeholder="用于提交, 例如: Your Name" />
            </n-form-item>
            <n-form-item label="Git Email">
              <n-input v-model:value="gitEmail" :disabled="loading" placeholder="用于提交, 例如: you@example.com" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" type="primary" @click="handleSaveGitConfig">保存并应用</n-button>
                <n-button :disabled="loading" @click="handleGitCommitPush">提交并推送</n-button>
                <n-button :disabled="loading" @click="handleGitPull">拉取</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted } from 'vue';
import {
  NCard, NSpace, NText, NButton, NIcon, NInput, NAlert, NTooltip,
  NDivider, NForm, NFormItem, NList, NListItem, NTag, useMessage,
} from 'naive-ui';
import {
  FolderAdd20Regular, DocumentEdit16Regular,
  Briefcase20Regular, Home20Regular, Book20Regular,
} from '@vicons/fluent';
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('TaskManagerContent')
const message = useMessage()

const taskRoot = ref('')
const loading = ref(false)
const scanResult = ref(null)
const categoryPaths = ref({ work: '', life: '', study: '' })

// 任务库 git 远程仓库
const gitRemoteUrl = ref('')
const gitUsername = ref('')
const gitEmail = ref('')

const categoryRows = ref([
  { key: 'work', zh: '工作', icon: Briefcase20Regular, color: '#7c3aed', path: '' },
  { key: 'life', zh: '生活', icon: Home20Regular, color: '#059669', path: '' },
  { key: 'study', zh: '学习', icon: Book20Regular, color: '#2563eb', path: '' },
])

const syncCategoryRows = () => {
  for (const row of categoryRows.value) {
    row.path = categoryPaths.value[row.key] || ''
  }
}

const loadConfig = async () => {
  try {
    const cfg = await invoke('task_get_config')
    taskRoot.value = cfg?.root ? String(cfg.root) : ''
  } catch (e) {
    logger.error('加载任务配置失败:', e)
  }
  try {
    const dirs = await invoke('task_ensure_dirs')
    const cats = dirs?.categories || {}
    categoryPaths.value = {
      work: cats.work || '',
      life: cats.life || '',
      study: cats.study || '',
    }
    syncCategoryRows()
  } catch (e) {
    logger.error('加载任务目录失败:', e)
  }
  try {
    const git = await invoke('task_get_git_config')
    gitRemoteUrl.value = git?.remote_url ? String(git.remote_url) : ''
    gitUsername.value = git?.git_username ? String(git.git_username) : ''
    gitEmail.value = git?.git_email ? String(git.git_email) : ''
  } catch (e) {
    logger.error('加载任务库 git 配置失败:', e)
  }
}

const handleSaveGitConfig = async () => {
  try {
    loading.value = true
    await invoke('task_set_git_config', {
      remoteUrl: gitRemoteUrl.value || '',
      gitUsername: gitUsername.value || '',
      gitEmail: gitEmail.value || '',
    })
    message.success('任务库 Git 配置已保存并应用')
  } catch (e) {
    logger.error('保存任务库 git 配置失败:', e)
    message.error('保存失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

const handleGitCommitPush = async () => {
  const path = taskRoot.value
  if (!path) { message.warning('任务库根目录未就绪'); return }
  try {
    loading.value = true
    try {
      await invoke('git_commit', { path, message: `task: update ${new Date().toLocaleString()}` })
    } catch (e) {
      // 没有变更时 commit 会报错，忽略后继续推送
      logger.warn('提交无变更或失败:', e)
    }
    await invoke('git_push', { path })
    message.success('已提交并推送')
  } catch (e) {
    logger.error('推送失败:', e)
    message.error('推送失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

const handleGitPull = async () => {
  const path = taskRoot.value
  if (!path) { message.warning('任务库根目录未就绪'); return }
  try {
    loading.value = true
    await invoke('git_pull', { path })
    message.success('已拉取最新内容')
    await invoke('task_scan_and_index').catch(() => {})
  } catch (e) {
    logger.error('拉取失败:', e)
    message.error('拉取失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

const handlePickRoot = async () => {
  try {
    const selected = await open({ directory: true, multiple: false })
    if (!selected) return
    taskRoot.value = selected
    await handleSaveRoot()
  } catch (e) {
    logger.error('选择任务库根目录失败:', e)
  }
}

const handleSaveRoot = async () => {
  try {
    loading.value = true
    await invoke('task_set_config', { root: taskRoot.value || '' })
    await loadConfig()
    message.success('任务库配置已保存')
  } catch (e) {
    logger.error('保存任务库配置失败:', e)
    message.error('保存失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

const handleResetDefault = async () => {
  taskRoot.value = ''
  await handleSaveRoot()
}

const handleEnsureDirs = async () => {
  try {
    loading.value = true
    await invoke('task_ensure_dirs')
    await loadConfig()
    message.success('任务目录已就绪')
  } catch (e) {
    logger.error('初始化目录失败:', e)
    message.error('初始化失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

const handleScan = async () => {
  try {
    loading.value = true
    const count = await invoke('task_scan_and_index')
    scanResult.value = typeof count === 'number' ? count : 0
    message.success(`扫描完成，索引 ${scanResult.value} 个任务`)
  } catch (e) {
    logger.error('扫描索引失败:', e)
    message.error('扫描失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

onMounted(loadConfig)
</script>

<style scoped>
.preference-wrapper {
  width: 100%;
}
.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.section-header h3 {
  margin: 0;
}
.description {
  color: var(--guaxi-text-secondary);
  font-size: 13px;
  line-height: 1.6;
  margin: 6px 0 10px;
}
</style>
