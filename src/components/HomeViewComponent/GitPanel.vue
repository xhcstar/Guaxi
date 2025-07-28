<template>
  <!-- 主容器 - 直接作为页面内容 -->
  <div class="gitkraken-container">
    <!-- 左侧面板 - 类似VSCode的Git面板 -->
    <div class="git-sidebar" ref="sidebar">
      <!-- 侧边栏右侧拖动条 -->
      <div class="sidebar-resize-handle" @mousedown="startSidebarDrag"></div>

      <div class="source-control-header">
        <n-space align="center" justify="space-between">
          <n-text strong>仓库管理</n-text>
          <n-button text @click="refreshGitStatus">
            <template #icon>
              <n-icon>
                <RefreshOutline />
              </n-icon>
            </template>
          </n-button>
        </n-space>
      </div>

      <!-- 提交区域 -->
      <div class="commit-section">
        <n-input v-model:value="commitForm.message" type="textarea" placeholder="提交信息 (必填)"
          :autosize="{ minRows: 1, maxRows: 3 }" style="min-width: 250px; margin-bottom: 8px;" />
        <n-button type="primary" @click="handleCommit"
          :disabled="!commitForm.message.trim() || stagedFiles.length === 0" style="width: 100%;">
          提交
        </n-button>
      </div>

      <!-- 变更文件列表 -->
      <div class="changes-section" ref="changesSection">
        <n-collapse :default-expanded-names="['changes']">
          <n-collapse-item name="changes" title="Changes">
            <template #header-extra>
              <n-badge :value="unstagedFiles.length" :max="99" />
            </template>

            <n-list>
              <!-- 操作按钮行 -->
              <n-list-item>
                <n-space justify="space-between">
                  <n-space>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click="handleStageAll" :disabled="unstagedFiles.length === 0">
                          <template #icon>
                            <n-icon>
                              <AddCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      暂存全部
                    </n-tooltip>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click="handleDiscardAll" :disabled="unstagedFiles.length === 0">
                          <template #icon>
                            <n-icon>
                              <CloseCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      丢弃全部
                    </n-tooltip>
                  </n-space>

                  <n-tag type="info" size="small">
                    <n-icon>
                      <GitBranchOutline />
                    </n-icon>
                    {{ currentBranch }}
                  </n-tag>
                </n-space>
              </n-list-item>

              <!-- 文件列表项 -->
              <n-list-item v-for="file in unstagedFiles" :key="file.path">
                <n-space align="center" justify="space-between">
                  <n-space align="center" @click="handleFileClick(file)">
                    <n-icon :component="getFileStatusIcon(file.status)" size="16" />
                    <n-text>{{ file.path }}</n-text>
                  </n-space>
                  <n-space>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click.stop="handleStageFile(file)">
                          <template #icon>
                            <n-icon>
                              <AddCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      暂存
                    </n-tooltip>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click.stop="handleDiscardFile(file)">
                          <template #icon>
                            <n-icon>
                              <CloseCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      丢弃
                    </n-tooltip>
                  </n-space>
                </n-space>
              </n-list-item>
            </n-list>
          </n-collapse-item>
        </n-collapse>

        <n-collapse :default-expanded-names="['staged']">
          <n-collapse-item name="staged" title="Staged Changes">
            <template #header-extra>
              <n-badge :value="stagedFiles.length" :max="99" />
            </template>

            <n-list>
              <!-- 操作按钮行 -->
              <n-list-item>
                <n-space justify="space-between">
                  <n-space>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click="handleUnstageAll" :disabled="stagedFiles.length === 0">
                          <template #icon>
                            <n-icon>
                              <RemoveCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      取消暂存全部
                    </n-tooltip>
                  </n-space>
                </n-space>
              </n-list-item>

              <!-- 暂存文件列表项 -->
              <n-list-item v-for="file in stagedFiles" :key="file.path">
                <n-space align="center" justify="space-between">
                  <n-space align="center" @click="handleFileClick(file)">
                    <n-icon :component="getFileStatusIcon(file.status)" size="16" />
                    <n-text>{{ file.path }}</n-text>
                  </n-space>
                  <n-space>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click.stop="handleUnstageFile(file)">
                          <template #icon>
                            <n-icon>
                              <RemoveCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      取消暂存
                    </n-tooltip>
                  </n-space>
                </n-space>
              </n-list-item>
            </n-list>
          </n-collapse-item>
        </n-collapse>
      </div>

      <!-- 区域分隔线 - 添加更明显的拖动手柄 -->
      <div class="section-divider" ref="divider" @mousedown="startVerticalDrag">
        <div class="divider-handle"></div>
      </div>

      <!-- 提交历史 -->
      <div class="commits-section" ref="commitsSection">
        <n-collapse :default-expanded-names="['commits']">
          <n-collapse-item name="commits" title="提交历史">
            <n-list>
              <!-- 操作按钮行 -->
              <n-list-item>
                <n-space>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="handlePull" :loading="gitLoading.pull">
                        <template #icon>
                          <n-icon>
                            <CloudDownloadOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    拉取
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="handlePush" :loading="gitLoading.push">
                        <template #icon>
                          <n-icon>
                            <CloudUploadOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    推送
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="showCreateBranchModal = true">
                        <template #icon>
                          <n-icon>
                            <GitBranchOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    创建分支
                  </n-tooltip>
                </n-space>
              </n-list-item>

              <!-- 提交历史列表项 -->
              <n-list-item v-for="commit in commits" :key="commit.id" @click="selectCommit(commit)"
                :class="{ 'selected-commit': selectedCommit?.id === commit.id }">
                <n-space vertical>
                  <n-text strong>{{ commit.summary }}</n-text>
                  <n-text depth="3" style="font-size: 12px">
                    {{ commit.author }} - {{ formatDate(commit.date) }}
                  </n-text>
                </n-space>
              </n-list-item>
            </n-list>
          </n-collapse-item>
        </n-collapse>
      </div>
    </div>

    <!-- 右侧内容区域 -->
    <div class="git-content">
      <!-- 文件差异或提交详情 -->
      <div class="content-viewer">
        <div v-if="selectedFile" class="diff-viewer">
          <n-card size="small" :title="selectedFile.path" embedded>
            <n-scrollbar style="height: 100%">
              <pre class="diff-content">{{ selectedFile.diff || '无差异内容' }}</pre>
            </n-scrollbar>
          </n-card>
        </div>

        <div v-if="selectedCommit" class="commit-details">
          <n-card size="small" :title="`提交 ${selectedCommit.id.slice(0, 7)}`" embedded>
            <n-scrollbar style="height: 100%">
              <div class="commit-meta">
                <n-space vertical>
                  <n-text strong>作者: {{ selectedCommit.author }}</n-text>
                  <n-text>日期: {{ formatDateFull(selectedCommit.date) }}</n-text>
                </n-space>
              </div>
              <pre class="commit-message">{{ selectedCommit.message }}</pre>
              <div class="commit-diff">
                <n-divider title-placement="left">修改内容</n-divider>
                <!-- 使用 pre 标签保留格式 -->
                <pre class="diff-content">{{ selectedCommit.diff }}</pre>
              </div>
            </n-scrollbar>
          </n-card>
        </div>

        <div v-if="!selectedFile && !selectedCommit" class="empty-state">
          <n-empty description="选择文件或提交查看详情" />
        </div>
      </div>
    </div>
  </div>

  <!-- 新建分支模态框 -->
  <n-modal v-model:show="showCreateBranchModal" title="新建分支" preset="dialog" positive-text="创建"
    @positive-click="handleCreateBranch">
    <n-input v-model:value="newBranchName" placeholder="输入新分支名称" />
  </n-modal>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import {
  NIcon, NTag, NButton, NCollapse, NCollapseItem, NDivider, NTooltip
} from 'naive-ui'
import {
  GitBranchOutline,
  CloudDownloadOutline,
  CloudUploadOutline,
  RefreshOutline,
  AddCircleOutline,
  CreateOutline,
  RemoveCircleOutline,
  CloseCircleOutline,
  DocumentOutline,
  GitCommitOutline
} from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'

// 日志级别枚举
const LogLevel = {
  DEBUG: 0,
  INFO: 1,
  WARN: 2,
  ERROR: 3
}

// 当前日志级别 (可根据环境变量设置)
const currentLogLevel = import.meta.env.DEV ? LogLevel.DEBUG : LogLevel.INFO

// 自定义日志记录器
const logger = {
  debug: (...args) => {
    if (currentLogLevel <= LogLevel.DEBUG) {
      console.debug('[GitPanel][DEBUG]', ...args)
    }
  },
  info: (...args) => {
    if (currentLogLevel <= LogLevel.INFO) {
      console.log('[GitPanel][INFO]', ...args)
    }
  },
  warn: (...args) => {
    if (currentLogLevel <= LogLevel.WARN) {
      console.warn('[GitPanel][WARN]', ...args)
    }
  },
  error: (...args) => {
    if (currentLogLevel <= LogLevel.ERROR) {
      console.error('[GitPanel][ERROR]', ...args)
    }
  }
}

const props = defineProps({
  workspacePath: {
    type: String,
    required: true
  }
})

// 拖动相关状态
const sidebar = ref(null)
const changesSection = ref(null)
const commitsSection = ref(null)
const divider = ref(null)
const isDraggingSidebar = ref(false)
const isDraggingVertical = ref(false)
const startY = ref(0)
const startX = ref(0)
const initialHeightChanges = ref(0)
const initialHeightCommits = ref(0)
const initialWidthSidebar = ref(280)

// 控制显示状态
const showCreateBranchModal = ref(false)
const newBranchName = ref('')

// 状态数据
const currentBranch = ref('')
const branches = ref([])
const commits = ref([])
const gitChanges = ref({
  added: 0,
  modified: 0,
  deleted: 0
})
const unstagedFiles = ref([])
const stagedFiles = ref([])
const selectedFile = ref(null)
const gitLoading = ref({
  pull: false,
  push: false,
  commit: false
})
const selectedCommit = ref(null)

// 表单数据
const commitForm = ref({
  message: ''
})

// 拖动处理函数
const startSidebarDrag = (e) => {
  isDraggingSidebar.value = true
  startX.value = e.clientX
  initialWidthSidebar.value = parseInt(getComputedStyle(sidebar.value).width)
  document.body.style.cursor = 'col-resize'
  document.addEventListener('mousemove', handleSidebarDrag)
  document.addEventListener('mouseup', stopSidebarDrag)
  e.preventDefault()
}

const handleSidebarDrag = (e) => {
  if (isDraggingSidebar.value) {
    const dx = e.clientX - startX.value
    const newWidth = initialWidthSidebar.value + dx
    if (newWidth > 200 && newWidth < 600) {
      sidebar.value.style.width = `${newWidth}px`
    }
  }
}

const stopSidebarDrag = () => {
  isDraggingSidebar.value = false
  document.body.style.cursor = ''
  document.removeEventListener('mousemove', handleSidebarDrag)
  document.removeEventListener('mouseup', stopSidebarDrag)
}

const startVerticalDrag = (e) => {
  logger.debug('Attempting to start vertical drag', {
    target: e.target,
    isDivider: e.target === divider.value,
    isHandle: e.target.closest('.divider-handle')
  })

  // 确保只在拖动句柄上触发
  if (e.target.closest('.divider-handle') || e.target === divider.value) {
    logger.debug('Vertical drag started', {
      startY: e.clientY,
      changesHeight: changesSection.value?.clientHeight,
      commitsHeight: commitsSection.value?.clientHeight
    })

    isDraggingVertical.value = true
    startY.value = e.clientY
    initialHeightChanges.value = changesSection.value?.clientHeight || 0
    initialHeightCommits.value = commitsSection.value?.clientHeight || 0

    // 防止文本选中
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'row-resize'
    document.addEventListener('mousemove', handleVerticalDrag)
    document.addEventListener('mouseup', stopVerticalDrag)
    e.preventDefault()
  } else {
    logger.debug('Vertical drag not started - incorrect target')
  }
}

const handleVerticalDrag = (e) => {
  if (!isDraggingVertical.value) return

  const dy = e.clientY - startY.value
  const newChangesHeight = Math.max(100, initialHeightChanges.value + dy)
  const newCommitsHeight = Math.max(100, initialHeightCommits.value - dy)

  // 使用requestAnimationFrame确保流畅更新
  requestAnimationFrame(() => {
    changesSection.value.style.height = `${newChangesHeight}px`
    commitsSection.value.style.height = `${newCommitsHeight}px`

    // 强制重新计算布局
    changesSection.value.style.display = 'block'
    commitsSection.value.style.display = 'block'
  })
}

const stopVerticalDrag = () => {
  logger.debug('Stopping vertical drag')
  isDraggingVertical.value = false
  document.body.style.userSelect = ''
  document.body.style.cursor = ''
  document.removeEventListener('mousemove', handleVerticalDrag)
  document.removeEventListener('mouseup', stopVerticalDrag)
}

onMounted(() => {
  logger.debug('Component mounted')
  refreshGitStatus()

  // 确保元素已挂载
  nextTick(() => {
    logger.debug('Checking elements after mount', {
      changesSection: !!changesSection.value,
      commitsSection: !!commitsSection.value,
      divider: !!divider.value
    })

    if (changesSection.value && commitsSection.value) {
      logger.debug('Setting initial heights')
      changesSection.value.style.height = '40%'
      commitsSection.value.style.height = '60%'
    }
  })
})

onUnmounted(() => {
  document.removeEventListener('mousemove', handleSidebarDrag)
  document.removeEventListener('mouseup', stopSidebarDrag)
  document.removeEventListener('mousemove', handleVerticalDrag)
  document.removeEventListener('mouseup', stopVerticalDrag)
})

// 其他原有方法保持不变...
const refreshGitStatus = async () => {
  logger.debug('Refreshing Git status')
  console.log('repo path:', props.workspacePath)
  try {
    const statuses = await invoke('get_git_status', { path: props.workspacePath })

    // 创建临时容器
    const newStagedFiles = []
    const newUnstagedFiles = []

    // 根据后端返回的 staged 字段正确分类文件
    statuses.forEach(file => {
      const fileData = {
        path: file.path,
        status: file.status,
        diff: file.diff || '',
        staged: file.staged // 使用后端返回的 staged 状态
      }

      if (fileData.staged) {
        newStagedFiles.push(fileData)
      } else {
        newUnstagedFiles.push(fileData)
      }
    })

    // 批量更新响应式数据
    stagedFiles.value = newStagedFiles
    unstagedFiles.value = newUnstagedFiles

    selectedFile.value = null

    // 添加获取提交历史的调用
    await loadCommitHistory()
  } catch (error) {
    logger.error('Failed to refresh Git status', error)
  }
}

const loadCommitHistory = async () => {
  try {
    const currentBranch = await invoke('get_current_branch', { path: props.workspacePath })
    commits.value = await invoke('get_git_log', {
      path: props.workspacePath,
      branchName: currentBranch
    })
    logger.debug('Commit history loaded', { count: commits.value.length })
  } catch (error) {
    logger.error('Failed to load commit history', error)
  }
}

const selectCommit = async (commit) => {
  logger.debug('Selecting commit', { commit: commit.id })
  try {
    selectedCommit.value = commit
    selectedFile.value = null

    // 获取提交的差异内容
    const diff = await invoke('get_commit_diff', {
      path: props.workspacePath,
      commitId: commit.id
    })

    // 确保差异内容正确赋值
    selectedCommit.value = {
      ...commit,
      diff: diff || '无差异内容'
    }

    logger.debug('Commit diff loaded', { commit: commit.id })
  } catch (error) {
    logger.error('Failed to load commit diff', { commit: commit.id, error })
  }
}

const formatDate = (timestamp) => {
  return new Date(parseInt(timestamp) * 1000).toLocaleDateString()
}

const formatDateFull = (timestamp) => {
  return new Date(parseInt(timestamp) * 1000).toLocaleString()
}

const getFileStatusIcon = (status) => {
  switch (status) {
    case 'A': return AddCircleOutline // 已暂存的新文件
    case 'M': return CreateOutline    // 修改的文件
    case 'D': return RemoveCircleOutline // 删除的文件
    case '?': return DocumentOutline  // 未跟踪文件
    case 'R': return GitCommitOutline // 重命名文件
    case 'T': return GitCommitOutline // 类型变更
    default: return DocumentOutline   // 其他状态
  }
}

const handleFileClick = async (row) => {
  logger.debug('File clicked', { file: row.path })
  try {
    selectedFile.value = row
    selectedCommit.value = null
    if (!row.diff) {
      logger.debug('Loading file diff', { file: row.path })
      const content = await invoke('git_read_file', {
        path: props.workspacePath,
        filePath: row.path
      })
      selectedFile.value.diff = content
      logger.debug('File content loaded', { file: row.path, length: content.length })
    }
  } catch (error) {
    logger.error('Failed to load file content', { file: row.path, error })
  }
}

const handleStageFile = async (file) => {
  try {
    // 1. 立即从unstagedFiles中移除文件（视觉反馈）
    const fileIndex = unstagedFiles.value.findIndex(f => f.path === file.path)
    if (fileIndex !== -1) {
      const [stagedFile] = unstagedFiles.value.splice(fileIndex, 1)

      // 2. 添加到stagedFiles（带过渡状态）
      stagedFiles.value.push({
        ...stagedFile,
        status: stagedFile.status === '?' ? 'A' :
          stagedFile.status === 'M' ? 'M' : 'D',
        staged: true,
        __pending: true  // 标记为处理中状态
      })
    }

    // 3. 执行后端操作
    await invoke('stage_file', {
      path: props.workspacePath,
      filePath: file.path
    })

    // 4. 更新文件状态为已完成
    const stagedIndex = stagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (stagedIndex !== -1) {
      stagedFiles.value[stagedIndex].__pending = false
    }

    // 5. 执行完整状态刷新（但保留现有UI状态）
    const freshStatus = await invoke('get_git_status', { path: props.workspacePath })

    // 6. 智能合并状态（而不是完全替换）
    freshStatus.forEach(freshFile => {
      if (freshFile.path === file.path) {
        const existingIndex = stagedFiles.value.findIndex(f => f.path === freshFile.path)
        if (existingIndex !== -1) {
          // 保留用户可能已经展开查看的diff内容
          stagedFiles.value[existingIndex] = {
            ...freshFile,
            diff: stagedFiles.value[existingIndex].diff || freshFile.diff || ''
          }
        }
      }
    })
  } catch (error) {
    // 回滚UI状态
    const stagedIndex = stagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (stagedIndex !== -1) {
      const [revertedFile] = stagedFiles.value.splice(stagedIndex, 1)
      unstagedFiles.value.push({
        ...revertedFile,
        status: revertedFile.status === 'A' ? '?' :
          revertedFile.status === 'M' ? 'M' : 'D',
        staged: false,
        __pending: undefined
      })
    }
  }
}

const handleUnstageFile = async (file) => {
  try {
    // 1. 立即从stagedFiles中移除文件
    const fileIndex = stagedFiles.value.findIndex(f => f.path === file.path)
    if (fileIndex !== -1) {
      const [unstagedFile] = stagedFiles.value.splice(fileIndex, 1)

      // 2. 添加到unstagedFiles（带过渡状态）
      unstagedFiles.value.push({
        ...unstagedFile,
        status: unstagedFile.status === 'A' ? '?' :
          unstagedFile.status === 'M' ? 'M' : 'D',
        staged: false,
        __pending: true
      })
    }

    // 3. 执行后端操作
    await invoke('unstage_file', {
      path: props.workspacePath,
      filePath: file.path
    })

    // 4. 更新状态
    const unstagedIndex = unstagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (unstagedIndex !== -1) {
      unstagedFiles.value[unstagedIndex].__pending = false
    }

    // 5. 智能刷新状态
    const freshStatus = await invoke('get_git_status', { path: props.workspacePath })

    freshStatus.forEach(freshFile => {
      if (freshFile.path === file.path) {
        const existingIndex = unstagedFiles.value.findIndex(f => f.path === freshFile.path)
        if (existingIndex !== -1) {
          unstagedFiles.value[existingIndex] = {
            ...freshFile,
            diff: unstagedFiles.value[existingIndex].diff || freshFile.diff || ''
          }
        }
      }
    })
  } catch (error) {
    // 回滚UI状态
    const unstagedIndex = unstagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (unstagedIndex !== -1) {
      const [revertedFile] = unstagedFiles.value.splice(unstagedIndex, 1)
      stagedFiles.value.push({
        ...revertedFile,
        status: revertedFile.status === '?' ? 'A' :
          revertedFile.status === 'M' ? 'M' : 'D',
        staged: true,
        __pending: undefined
      })
    }
  }
}

const handleStageAll = async () => {
  logger.debug('Staging all files')
  try {
    await invoke('stage_all', {
      path: props.workspacePath
    })
    logger.info('All files staged', { count: unstagedFiles.value.length })
    await refreshGitStatus()
  } catch (error) {
    logger.error('Failed to stage all files', error)
  }
}

const handleUnstageAll = async () => {
  logger.debug('Unstaging all files')
  try {
    await invoke('unstage_all', {
      path: props.workspacePath
    })
    logger.info('All files unstaged', { count: stagedFiles.value.length })
    await refreshGitStatus()
  } catch (error) {
    logger.error('Failed to unstage all files', error)
  }
}

const handleDiscardFile = async (file) => {
  logger.debug('Discarding file changes', { file: file.path })
  try {
    await invoke('discard_file', {
      path: props.workspacePath,
      filePath: file.path
    })
    logger.info('File changes discarded', { file: file.path })
    await refreshGitStatus()
  } catch (error) {
    logger.error('Failed to discard file changes', { file: file.path, error })
  }
}

const handleDiscardAll = async () => {
  logger.debug('Discarding all changes')
  try {
    await invoke('discard_all', {
      path: props.workspacePath
    })
    logger.info('All changes discarded')
    await refreshGitStatus()
  } catch (error) {
    logger.error('Failed to discard all changes', error)
  }
}

const handleCommit = async () => {
  if (!commitForm.value.message.trim()) {
    return
  }

  try {
    await invoke('git_commit', {
      path: props.workspacePath,
      message: commitForm.value.message
    })
    commitForm.value.message = ''
    await refreshGitStatus()
  } catch (error) {
  }
}

const handlePull = async () => {
  logger.debug('Pulling changes')
  gitLoading.value.pull = true
  try {
    await invoke('git_pull', { path: props.workspacePath })
    logger.info('Pull successful')
    refreshGitStatus()
  } catch (error) {
    logger.error('Pull failed', { error })
  } finally {
    gitLoading.value.pull = false
  }
}

const handlePush = async () => {
  logger.debug('Pushing changes')
  gitLoading.value.push = true
  try {
    await invoke('git_push', { path: props.workspacePath })
    logger.info('Push successful')
  } catch (error) {
    logger.error('Push failed', { error })
  } finally {
    gitLoading.value.push = false
  }
}

const checkoutBranch = async (branchName) => {
  logger.debug('Checking out branch', { branch: branchName })
  try {
    await invoke('checkout_branch', {
      path: props.workspacePath,
      branch_name: branchName
    })
    logger.info('Branch checkout successful', { branch: branchName })
    refreshGitStatus()
  } catch (error) {
    logger.error('Branch checkout failed', { branch: branchName, error })
  }
}

const handleCreateBranch = async () => {
  logger.debug('Creating new branch', { name: newBranchName.value })
  if (!newBranchName.value.trim()) {
    logger.warn('New branch name is empty')
    return
  }

  try {
    await invoke('create_branch', {
      path: props.workspacePath,
      branch_name: newBranchName.value
    })
    logger.info('Branch created', { name: newBranchName.value })
    showCreateBranchModal.value = false
    newBranchName.value = ''
    refreshGitStatus()
  } catch (error) {
    logger.error('Failed to create branch', { name: newBranchName.value, error })
  }
}
</script>

<style>
/* 全局样式 - 浅色主题 */
:root {
  --bg-color: #f8f9fa;
  --text-color: #212529;
  --text-color-secondary: #6c757d;
  --divider-color: #dee2e6;
  --hover-color: rgba(0, 0, 0, 0.05);
  --graph-bg: #e9ecef;
  --added-bg: rgba(40, 167, 69, 0.1);
  --modified-bg: rgba(255, 193, 7, 0.1);
  --deleted-bg: rgba(220, 53, 69, 0.1);
  --current-branch-bg: rgba(40, 167, 69, 0.15);
  --card-bg: #ffffff;
  --input-bg: #ffffff;
  --sidebar-bg: #f3f3f3;
  --sidebar-width: 280px;
}

.n-card {
  background-color: var(--card-bg);
  border: 1px solid var(--divider-color);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.n-modal {
  background-color: var(--bg-color);
}

.n-list-item {
  background-color: transparent;
}

.n-tabs .n-tabs-pane-wrapper {
  background-color: var(--bg-color);
}

.n-input,
.n-textarea {
  background-color: var(--input-bg);
}

/* 表格样式 */
.n-data-table {
  background-color: var(--card-bg);
}

.n-data-table .n-data-table-tr:hover {
  background-color: var(--hover-color);
}

/* 分支列表样式 */
.branch-list {
  background-color: var(--graph-bg);
}

.current-branch {
  background-color: var(--current-branch-bg);
}

/* 提交列表样式 */
.commit-list .n-list-item:hover {
  background-color: var(--hover-color);
}

.selected-commit {
  background-color: var(--current-branch-bg);
}

/* 差异查看器样式 */
.diff-content {
  font-family: 'Maple Mono NF CN', monospace;
  font-size: 14px;
  white-space: pre-wrap;
  margin: 0;
  padding: 8px;
  background-color: var(--card-bg);
  border-radius: 4px;
  overflow-x: auto;
}

/* 差异高亮样式 */
.diff-added {
  color: #22863a;
  background-color: rgba(46, 160, 67, 0.1);
}

.diff-removed {
  color: #cb2431;
  background-color: rgba(203, 36, 49, 0.1);
}

.diff-header {
  color: #6a737d;
  font-weight: bold;
}

/* 按钮样式 */
.n-button {
  transition: all 0.2s ease;
}

.n-button:hover {
  transform: translateY(-1px);
}

/* 模态框动画 */
.gitkraken-modal-enter-active,
.gitkraken-modal-leave-active {
  transition: all 0.3s ease;
}

.gitkraken-modal-enter-from,
.gitkraken-modal-leave-to {
  opacity: 0;
  transform: translateY(20px);
}
</style>

<style scoped>
.gitkraken-modal {
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.gitkraken-container {
  display: flex;
  height: 100vh;
  width: 100vw;
  background-color: var(--bg-color);
  color: var(--text-color);
}

.git-sidebar {
  width: var(--sidebar-width);
  min-width: 200px;
  max-width: 600px;
  border-right: 1px solid var(--divider-color);
  display: flex;
  flex-direction: column;
  background-color: var(--sidebar-bg);
  height: 100%;
  overflow: hidden;
  position: relative;
}

.sidebar-resize-handle {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  cursor: col-resize;
  z-index: 10;
}

.sidebar-resize-handle:hover {
  background-color: var(--divider-color);
}

.source-control-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
}

.commit-section {
  padding: 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
}

.changes-section {
  flex: 0 0 auto;
  /* 不自动增长，由高度属性控制 */
  overflow-y: auto;
  position: relative;
  min-height: 100px;
  max-height: 80%;
}

/* 修改分隔线样式 */
.section-divider {
  flex: 0 0 8px;
  /* 固定高度 */
  height: 8px;
  background-color: var(--divider-color);
  cursor: row-resize;
  margin: 0;
  position: relative;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: center;
}

.divider-handle {
  width: 30px;
  height: 4px;
  background-color: var(--text-color-secondary);
  border-radius: 2px;
  opacity: 0.3;
  transition: opacity 0.2s;
}

.section-divider:hover .divider-handle {
  opacity: 1;
}

.commits-section {
  flex: 1;
  overflow-y: auto;
  min-height: 100px;
  max-height: 80%;
}

/* 强制更新高度计算 */
.changes-section,
.commits-section {
  will-change: height;
  /* 提示浏览器优化重绘 */
  contain: strict;
  /* 限制渲染范围提高性能 */
}

/* 自定义滚动条样式 */
.changes-section::-webkit-scrollbar,
.commits-section::-webkit-scrollbar {
  width: 6px;
}

.changes-section::-webkit-scrollbar-track,
.commits-section::-webkit-scrollbar-track {
  background: transparent;
}

.changes-section::-webkit-scrollbar-thumb,
.commits-section::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 3px;
  opacity: 0.3;
}

.changes-section::-webkit-scrollbar-thumb:hover,
.commits-section::-webkit-scrollbar-thumb:hover {
  opacity: 0.6;
}

.git-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: auto;
  height: 100%;
}

.content-viewer {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.diff-viewer,
.commit-details {
  flex: 1;
  overflow: hidden;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.file-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.file-icon {
  color: var(--text-color-secondary);
}

.file-path {
  color: var(--text-color);
}

.file-actions {
  display: flex;
  gap: 4px;
}

.file-row:hover {
  background-color: var(--hover-color);
}

.file-status-a {
  background-color: var(--added-bg);
}

.file-status-m {
  background-color: var(--modified-bg);
}

.file-status-d {
  background-color: var(--deleted-bg);
}

.commit-list {
  margin-left: 24px;
  border-left: 1px solid var(--divider-color);
  padding-left: 8px;
}

.commit-list .n-list-item {
  padding: 8px;
  margin: 4px 0;
  border-radius: 4px;
}

.commit-meta {
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--divider-color);
}

.commit-message {
  white-space: pre-wrap;
  font-family: 'Maple Mono NF CN', monospace;
  font-size: 14px;
  margin-top: 8px;
  color: var(--text-color);
}

.commit-diff {
  margin-top: 16px;
}

.n-collapse-item .n-collapse-item__header {
  padding: 4px 12px;
  position: relative;
}

.n-collapse-item .n-collapse-item__content-inner {
  padding: 4px 0;
}

.n-list-item {
  padding: 4px 12px;
}

.n-list-item:hover {
  background-color: var(--hover-color);
}
</style>