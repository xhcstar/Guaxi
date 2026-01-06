<template>
  <!-- 主容器 - 直接作为页面内容 -->
  <div class="gitkraken-container">
    <!-- 左侧面板 - 类似VSCode的Git面板 -->
    <div class="git-sidebar" ref="sidebar" :style="{ width: isSidebarCollapsed ? '0px' : '350px' }">
      <!-- 侧边栏右侧拖动条 -->
      <div class="sidebar-resize-handle" @mousedown="startSidebarDrag"></div>

      <!-- 新增的折叠按钮（放在侧边栏内部但绝对定位到右侧） -->
      <div class="sidebar-collapse-button" :class="{ 'collapsed': isSidebarCollapsed }" @click="toggleSidebar">
        <n-icon>
          <ChevronLeft20Filled v-if="!isSidebarCollapsed" />
          <ChevronRight20Filled v-else />
        </n-icon>
      </div>

      <!-- 仓库管理和提交区域容器 -->
      <div class="sidebar-content" :style="{ display: isSidebarCollapsed ? 'none' : 'block' }">
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

        <!-- 变更文件列表容器 -->
        <div class="changes-container">
          <!-- 原有内容保持不变 -->
          <div class="panels-container" ref="panelsContainer">
            <!-- Changes区域 -->
            <div class="changes-section" ref="changesSection" :style="{ flex: `0 0 ${changesHeight}px` }">
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
            </div>

            <!-- Changes和Staged Changes之间的分隔线 -->
            <div class="section-divider" ref="changesDivider" @mousedown="startChangesDrag">
              <div class="divider-handle"></div>
            </div>

            <!-- Staged Changes区域 -->
            <div class="staged-section" ref="stagedSection" :style="{ flex: `0 0 ${stagedHeight}px` }">
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

            <!-- Staged Changes和提交历史之间的分隔线 -->
            <div class="commits-divider" ref="commitsDivider" @mousedown="startCommitsDrag">
              <div class="divider-handle"></div>
            </div>

            <!-- 提交历史区域 -->
            <div class="commits-section" ref="commitsSection">
              <n-collapse :default-expanded-names="['commits']">
                <n-collapse-item name="commits" title="提交历史">
                  <!-- 操作工具栏 -->
                  <div class="commits-toolbar">
                    <n-space justify="space-between" align="center">
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
                            <n-button text @click="showBranchPanel = true">
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
                      <n-tag type="info" size="small" @click="showBranchPanel = true" style="cursor: pointer">
                        <n-icon>
                          <GitBranchOutline />
                        </n-icon>
                        {{ currentBranch }}
                      </n-tag>
                    </n-space>
                  </div>

                  <!-- 提交历史列表 -->
                  <div class="commits-list">
                    <div v-for="commit in commits" :key="commit.id" class="commit-item" :class="{
                      'selected-commit': selectedCommit?.id === commit.id,
                      'expanded-commit': expandedCommit?.id === commit.id
                    }" @click="selectCommit(commit)">
                      <!-- 提交信息摘要 -->
                      <div class="commit-summary">
                        <n-text strong>{{ commit.summary }}</n-text>
                        <n-text depth="3" class="commit-hash">{{ commit.id.slice(0, 7) }}</n-text>
                      </div>

                      <!-- 提交元信息 -->
                      <div class="commit-meta">
                        <n-text depth="3">{{ commit.author }}</n-text>
                        <n-text depth="3">{{ formatDateRelative(commit.date) }}</n-text>
                      </div>

                      <!-- 展开的文件变更列表 -->
                      <div v-if="expandedCommit?.id === commit.id" class="commit-files">
                        <n-list>
                          <n-list-item v-for="file in commitFiles" :key="file.path"
                            @click.stop="handleCommitFileClick(file)"
                            :class="{ 'selected-file': selectedFile?.path === file.path }">
                            <n-space align="center">
                              <n-icon :component="getFileStatusIcon(file.status)" size="16" />
                              <n-text>{{ file.path }}</n-text>
                              <n-tag v-if="file.status !== ' '" size="small" :type="getStatusTagType(file.status)">
                                {{ file.status }}
                              </n-tag>
                            </n-space>
                          </n-list-item>
                        </n-list>
                      </div>
                    </div>
                  </div>
                </n-collapse-item>
              </n-collapse>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 右侧内容区域 -->
    <div class="git-content">
      <!-- 顶部导航栏 -->
      <div class="content-navbar">
        <n-space align="center" justify="space-between">
          <n-space align="center">
            <!-- 显示文件信息 -->
            <template v-if="selectedFile">
              <n-text strong>{{ selectedFile.path }}</n-text>
              <!-- 添加加载状态 -->
              <div v-if="diffLoading.file" class="loading-overlay">
                <n-spin size="large" />
              </div>
            </template>

            <!-- 显示提交信息（如果有） -->
            <template v-if="selectedCommit">
              <n-divider vertical />
              <n-space align="center">
                <n-text strong>提交: {{ selectedCommit.id.slice(0, 7) }}</n-text>
                <n-text>主题: {{ selectedCommit.message.split('\n')[0] }}</n-text>
                <n-text>作者: {{ selectedCommit.author }}</n-text>
                <n-text>日期: {{ formatDateFull(selectedCommit.date) }}</n-text>
              </n-space>
              <div v-if="diffLoading.commit" class="loading-overlay">
                <n-spin size="large" />
              </div>
            </template>

            <template v-if="!selectedFile && !selectedCommit">
              <n-text>未选择文件或提交</n-text>
            </template>
          </n-space>

          <!-- 导航按钮 -->
          <n-space>
            <n-button text @click="navigateDiff('prev')" :disabled="!selectedFile || diffLoading.file">
              <template #icon>
                <n-icon>
                  <ChevronUpOutline />
                </n-icon>
              </template>
            </n-button>
            <n-button text @click="navigateDiff('next')" :disabled="!selectedFile || diffLoading.file">
              <template #icon>
                <n-icon>
                  <ChevronDownOutline />
                </n-icon>
              </template>
            </n-button>
          </n-space>
        </n-space>
      </div>

      <!-- 文件差异或提交详情 -->
      <div class="content-viewer">
        <div v-if="selectedFile" class="diff-viewer">
          <!-- Monaco Editor 容器 -->
          <div ref="diffEditorContainer" class="diff-editor-container"></div>
        </div>

        <!-- 在 commit-details 部分添加加载状态 -->
        <div v-else-if="selectedCommit" class="commit-details">
          <!-- 提交差异 -->
          <div ref="diffEditorContainer" class="diff-editor-container"></div>
        </div>

        <div v-else="!selectedFile && !selectedCommit" class="empty-state">
          <n-empty description="选择文件或提交查看详情" />
        </div>
      </div>
    </div>
  </div>

  <!-- 分支选择面板 -->
  <n-modal v-model:show="showBranchPanel" preset="card" title="选择分支" :bordered="false" size="medium"
    style="width: 600px" :mask-closable="false">
    <div class="branch-panel-container">
      <!-- 分支列表 -->
      <n-scrollbar style="max-height: 400px">
        <n-list hoverable clickable>
          <n-list-item v-for="branch in branches" :key="branch.name" @click="handleBranchItemClick(branch)"
            :class="{ 'current-branch-item': branch.is_current }">
            <n-space align="center" justify="space-between">
              <n-space align="center">
                <n-icon v-if="branch.is_current" color="green">
                  <CheckmarkCircleOutline />
                </n-icon>
                <n-text :type="branch.is_current ? 'success' : undefined">
                  {{ branch.name }}
                </n-text>
              </n-space>
              <n-tag v-if="branch.is_current" size="small" type="success">
                当前分支
              </n-tag>
            </n-space>
          </n-list-item>
        </n-list>
      </n-scrollbar>

      <!-- 创建分支按钮 -->
      <div class="create-branch-footer">
        <n-button type="primary" @click="showCreateBranchModal = true" style="width: 100%; margin-top: 16px">
          <template #icon>
            <n-icon>
              <AddCircleOutline />
            </n-icon>
          </template>
          创建新分支
        </n-button>
      </div>
    </div>
  </n-modal>

  <n-modal v-model:show="showCreateBranchModal" preset="dialog" title="创建新分支">
    <n-form>
      <n-form-item label="分支名称" required>
        <n-input v-model:value="newBranchName" placeholder="请输入新分支名称" />
      </n-form-item>
    </n-form>
    <template #action>
      <n-space justify="end">
        <n-button @click="showCreateBranchModal = false">取消</n-button>
        <n-button type="primary" @click="handleCreateBranch" :disabled="!newBranchName.trim()">
          创建
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<script setup>
import { ref, h, onMounted, onUnmounted, nextTick, watch, markRaw } from 'vue'
import {
  NIcon, NTag, NButton, NCollapse, NCollapseItem, NDivider, NTooltip, NForm,
  NFormItem,
  NInput,
  createDiscreteApi,
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
  GitCommitOutline,
  CheckmarkCircleOutline,
  ChevronUpOutline,
  ChevronDownOutline
} from '@vicons/ionicons5'
import { ChevronLeft20Filled, ChevronRight20Filled } from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import * as monaco from 'monaco-editor'
import { useSettingsStore } from '../../stores/settingsStore'
import { registerDiffLanguages } from './utils/monaco-diff' // 自定义语言注册

const { notification } = createDiscreteApi(['notification'])

const settingsStore = useSettingsStore()
const getMonacoTheme = () => (settingsStore.appearanceTheme === 'dark' ? 'vs-dark' : 'vs')

watch(
  () => settingsStore.appearanceTheme,
  () => {
    try {
      monaco.editor.setTheme(getMonacoTheme())
    } catch (e) {
    }
  },
  { immediate: true }
)

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
const isDraggingSidebar = ref(false)
const startY = ref(0)
const startX = ref(0)
const initialHeightChanges = ref(0)
const initialHeightCommits = ref(0)
const initialWidthSidebar = ref(350)
const changesDivider = ref(null)
const stagedSection = ref(null)
const isDraggingChanges = ref(false)
const initialHeightStaged = ref(0)
const commitsDivider = ref(null)
const isDraggingCommits = ref(false)

// 状态数据
const currentBranch = ref('')
const branches = ref([])
const commits = ref([])
const unstagedFiles = ref([])
const stagedFiles = ref([])
const selectedFile = ref(null)
const gitLoading = ref({
  pull: false,
  push: false,
  commit: false,
  checkout: false  // 添加分支切换加载状态
});

const selectedCommit = ref(null)
const expandedCommit = ref(null)
const commitFiles = ref([])

// 表单数据
const commitForm = ref({
  message: ''
})

// 分支面板相关状态
const showBranchPanel = ref(false)
const showCreateBranchModal = ref(false)
const newBranchName = ref('')

// Monaco Editor 实例
const diffEditorContainer = ref(null)
const isEditorInitialized = ref(false)
let diffEditor = null
let resizeObserver = null

const diffLoading = ref({
  file: false,
  commit: false,
  navigation: false
});

// 添加侧边栏折叠状态
const isSidebarCollapsed = ref(false)

// 添加切换函数
const toggleSidebar = () => {
  isSidebarCollapsed.value = !isSidebarCollapsed.value
}

// 新增辅助函数 - 获取状态标签类型
const getStatusTagType = (status) => {
  switch (status) {
    case 'A': return 'success'
    case 'M': return 'warning'
    case 'D': return 'error'
    case 'R': return 'info'
    default: return 'default'
  }
}

// 修改 selectCommit 函数
const selectCommit = async (commit) => {
  logger.info('Selecting commit', { commitId: commit.id })
  try {
    // 如果点击的是已展开的提交，则收起
    if (expandedCommit.value?.id === commit.id) {
      logger.info('Collapsing already expanded commit')
      expandedCommit.value = null
      commitFiles.value = []
      selectedCommit.value = null
      return
    }

    selectedCommit.value = commit
    expandedCommit.value = commit
    selectedFile.value = null

    logger.info('Fetching commit files')
    const files = await invoke('get_commit_files', {
      path: props.workspacePath,
      commitId: commit.id
    })

    logger.info('Received commit files', { count: files.length })
    commitFiles.value = files

    // 默认显示第一个文件的差异
    if (files.length > 0) {
      logger.info('Selecting first file in commit')
      await handleCommitFileClick(files[0])
    } else {
      logger.info('No files in commit, clearing diff')
      if (diffEditor.value) {
        diffEditor.value.setModel(null)
      }
    }
  } catch (error) {
    logger.error('Failed to load commit diff', {
      commitId: commit.id,
      error: error.message
    })
    notification.error({
      title: '加载提交差异失败',
      content: `无法加载提交 ${commit.id.slice(0, 7)} 的差异内容: ${error}`,
      duration: 3000
    })
  }
}

const handleCommitFileClick = async (file) => {
  logger.info('Handling commit file click', {
    filePath: file.path,
    status: file.status
  })
  try {
    selectedFile.value = file  // 只更新selectedFile，保留selectedCommit
    diffLoading.value.file = true

    if (!isEditorInitialized.value) {
      await initMonacoEditor()
    }

    // 获取文件差异
    const diff = await invoke('get_commit_file_diff', {
      path: props.workspacePath,
      commitId: expandedCommit.value.id,
      filePath: file.path
    })

    // 确保diff是对象且包含original和modified
    if (!diff || typeof diff !== 'object') {
      throw new Error('Invalid diff format received from backend')
    }

    const language = getLanguageForFile(file.path)

    // 创建新模型前先处理空内容情况
    const originalContent = diff[0] || ''
    const modifiedContent = diff[1] || ''
    const originalModel = monaco.editor.createModel(originalContent, language)
    const modifiedModel = monaco.editor.createModel(modifiedContent, language)

    // 获取当前模型以便后续清理
    const oldModel = diffEditor.getModel();

    // 设置新模型
    diffEditor.setModel({
      original: originalModel,
      modified: modifiedModel
    });

    // 清理旧模型（如果有）
    if (oldModel) {
      setTimeout(() => {
        oldModel.original?.dispose();
        oldModel.modified?.dispose();
      }, 100);
    }

    // 配置差异编辑器
    diffEditor.updateOptions({
      renderSideBySide: true,
      renderIndicators: true,
      renderWhitespace: 'selection',
      readOnly: true,
      automaticLayout: true
    });

    // 确保布局正确
    await nextTick();
    if (diffEditor && diffEditorContainer.value) {
      const height = diffEditorContainer.value.clientHeight;
      const width = diffEditorContainer.value.clientWidth;
      diffEditor.layout({ width, height });

      // 导航到第一个差异
      setTimeout(async () => {
        const modifiedEditor = diffEditor.getModifiedEditor();
        const diffNavAction = modifiedEditor.getAction('editor.action.diffReview.next');
        if (diffNavAction) {
          await diffNavAction.run();
          modifiedEditor.revealPositionInCenter(modifiedEditor.getPosition());
        }
      }, 100);
    }
  } catch (error) {
    logger.error('Failed to load commit file diff', error)
    notification.error({
      title: '加载文件差异失败',
      content: `无法加载文件 ${file.path} 的差异内容: ${error}`,
      duration: 3000
    })
  } finally {
    diffLoading.value.file = false
  }
}

const navigateDiff = async (direction) => {
  if (!diffEditor || diffLoading.value.navigation) {
    logger.warn('Navigation skipped - editor not ready or navigation in progress');
    return;
  }

  diffLoading.value.navigation = true;

  try {
    const editor = diffEditor.getModifiedEditor();
    logger.info('Got modified editor instance');

    // 检查编辑器是否已准备好
    if (!editor.getModel()) {
      logger.warn('Editor model not ready for navigation');
      return;
    }

    // 尝试获取导航动作
    const actionName = direction === 'prev'
      ? 'editor.action.diffReview.prev'
      : 'editor.action.diffReview.next';

    const action = editor.getAction(actionName);
    if (!action) {
      // 如果标准动作不存在，尝试备用方案
      logger.warn(`Action ${actionName} not found, trying alternative approach`);

      // 备用方案：直接操作编辑器选择第一个差异
      const changes = diffEditor.getLineChanges();
      if (changes && changes.length > 0) {
        const change = direction === 'prev'
          ? changes[changes.length - 1]
          : changes[0];

        editor.setPosition({
          lineNumber: change.modifiedStartLineNumber,
          column: 1
        });
        editor.revealPositionInCenter({
          lineNumber: change.modifiedStartLineNumber,
          column: 1
        });
        editor.focus();
        logger.debug('Navigated using line changes API');
        return;
      }

      throw new Error(`No navigation action or changes available`);
    }

    logger.debug(`Executing ${actionName}`);
    await action.run();

    // 确保视图滚动到差异位置
    const position = editor.getPosition();
    if (position) {
      editor.revealPositionInCenter(position);
      editor.focus();
      logger.debug('Navigation completed successfully');
    }
  } catch (error) {
    logger.error(`Failed to navigate ${direction} diff`, error);
    notification.error({
      title: '导航失败',
      content: `无法跳转到差异位置: ${error.message}`,
      duration: 2000
    });
  } finally {
    diffLoading.value.navigation = false;
  }
}

const updateCommitDiff = async (commit) => {
  if (!isEditorInitialized.value || !diffEditorContainer.value) {
    await initMonacoEditor();
  } else {
    isEditorInitialized.value = false;
  }

  if (!commit || !diffEditor.value) return;

  diffLoading.value.commit = true;

  try {
    // 获取提交差异
    const diff = await invoke('get_commit_diff', {
      path: props.workspacePath,
      commitId: commit.id
    });

    // 解析差异内容
    const { original, modified } = parseUnifiedDiff(diff);

    // 创建 Monaco 模型
    const originalModel = monaco.editor.createModel(original, 'plaintext');
    const modifiedModel = monaco.editor.createModel(modified, 'plaintext');

    // 设置差异编辑器模型
    diffEditor.value.setModel({
      original: originalModel,
      modified: modifiedModel
    });

    // 导航到第一个差异
    setTimeout(() => {
      const modifiedEditor = diffEditor.value.getModifiedEditor();
      const diffNavAction = modifiedEditor.getAction('editor.action.diffReview.next');
      if (diffNavAction) {
        diffNavAction.run().then(() => {
          modifiedEditor.revealPositionInCenter(modifiedEditor.getPosition());
        });
      }
    }, 100);
  } catch (error) {
    logger.error('Failed to update commit diff', error);
  } finally {
    diffLoading.value.commit = false;
  }
}

// 根据文件扩展名获取语言
const getLanguageForFile = (path) => {
  const extension = path.split('.').pop().toLowerCase()
  switch (extension) {
    case 'js': return 'javascript'
    case 'ts': return 'typescript'
    case 'py': return 'python'
    case 'rb': return 'ruby'
    case 'go': return 'go'
    case 'rs': return 'rust'
    case 'java': return 'java'
    case 'c': return 'c'
    case 'cpp': return 'cpp'
    case 'h': return 'cpp'
    case 'hpp': return 'cpp'
    case 'cs': return 'csharp'
    case 'php': return 'php'
    case 'html': return 'html'
    case 'css': return 'css'
    case 'scss': return 'scss'
    case 'less': return 'less'
    case 'json': return 'json'
    case 'xml': return 'xml'
    case 'yml': return 'yaml'
    case 'yaml': return 'yaml'
    case 'md': return 'markdown'
    case 'sh': return 'shell'
    case 'vue': return 'vue'
    default: return 'plaintext'
  }
}

// 解析 unified diff 格式
const parseUnifiedDiff = (diff) => {
  const lines = diff.split('\n')
  let original = []
  let modified = []
  let lineNumber = 1

  for (const line of lines) {
    if (line.startsWith('@@')) {
      // 跳过差异头部
      continue
    }

    if (line.startsWith('-')) {
      original.push(line.substring(1))
    } else if (line.startsWith('+')) {
      modified.push(line.substring(1))
    } else {
      original.push(line.substring(1))
      modified.push(line.substring(1))
    }
  }

  return {
    original: original.join('\n'),
    modified: modified.join('\n')
  }
}

const confirmBranchSwitch = () => {
  return new Promise((resolve) => {
    notification.warning({
      title: '未提交的更改',
      content: '当前分支有未提交的更改，切换分支可能会丢失这些更改。是否继续？',
      duration: 0,
      meta: () => h('div', {
        style: {
          display: 'flex',
          justifyContent: 'flex-end',
          marginTop: '12px'
        }
      }, [
        h(NButton, {
          size: 'small',
          onClick: () => resolve(false)
        }, '取消'),
        h(NButton, {
          size: 'small',
          type: 'primary',
          onClick: () => resolve(true),
          style: {
            marginLeft: '8px'
          }
        }, '继续切换')
      ])
    });
  });
};

// 获取分支列表
const fetchBranches = async () => {
  try {
    const result = await invoke('list_branches', {
      path: props.workspacePath
    });

    // 获取当前分支
    const current = await invoke('get_current_branch', {
      path: props.workspacePath
    });

    // 更新分支列表和当前分支
    branches.value = result.map(b => ({
      ...b,
      is_current: b.name === current
    }));

    currentBranch.value = current;
  } catch (error) {
    console.error('Failed to fetch branches:', error);
    notification.error({
      title: '获取分支失败',
      content: error.toString(),
      duration: 3000
    });
  }
};

// 切换分支
const handleBranchItemClick = async (branch) => {
  if (!branch.is_current) {
    // 检查是否有未提交的更改
    if (unstagedFiles.value.length > 0 || stagedFiles.value.length > 0) {
      const confirmed = await confirmBranchSwitch();
      if (!confirmed) return;
    }

    try {
      // 显示加载状态
      gitLoading.value.checkout = true;

      // 执行分支切换
      await invoke('checkout_branch', {
        path: props.workspacePath,
        branchName: branch.name
      });

      // 更新当前分支状态
      currentBranch.value = branch.name;

      // 关闭分支面板
      showBranchPanel.value = false;

      // 刷新所有相关状态
      await Promise.all([
        fetchBranches(),       // 刷新分支列表
        refreshGitStatus(),    // 刷新文件状态
        loadCommitHistory()    // 刷新提交历史
      ]);

      // 显示成功通知
      notification.success({
        title: '分支切换成功',
        content: `已切换到分支 ${branch.name}`,
        duration: 2000
      });
    } catch (error) {
      console.error('Failed to checkout branch:', error);
      notification.error({
        title: '分支切换失败',
        content: error.toString(),
        duration: 3000
      });
    } finally {
      gitLoading.value.checkout = false;
    }
  }
};

const handleCreateBranch = async () => {
  if (!newBranchName.value.trim()) {
    notification.warning({
      title: '分支名称不能为空',
      duration: 2000
    });
    return;
  }

  try {
    // 修改这里 - 确保参数名与后端一致
    await invoke('create_branch', {
      path: props.workspacePath,
      branchName: newBranchName.value  // 使用 branch_name 而不是 branchName
    });

    notification.success({
      title: '创建成功',
      content: `分支 ${newBranchName.value} 已创建`,
      duration: 2000
    });

    await fetchBranches();
    showCreateBranchModal.value = false;
    newBranchName.value = '';
  } catch (error) {
    notification.error({
      title: '创建分支失败',
      content: error.toString(),
      duration: 3000
    });
  }
}

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

// 在setup部分添加以下代码
const changesHeaderHeight = ref(40) // Changes区域标题高度
const stagedHeaderHeight = ref(40) // Staged区域标题高度
const commitsHeaderHeight = ref(40) // Commits区域标题高度
// 添加新的响应式变量
const changesHeight = ref(0)
const stagedHeight = ref(0)
const commitsHeight = ref(0)
const panelsContainer = ref(null)
const panelsHeight = ref(0)

// 修改 startChangesDrag 函数
const startChangesDrag = (e) => {
  if (e.target === changesDivider.value || e.target.closest('.divider-handle')) {
    isDraggingChanges.value = true
    startY.value = e.clientY
    initialHeightChanges.value = changesHeight.value
    initialHeightStaged.value = stagedHeight.value
    panelsHeight.value = panelsContainer.value.offsetHeight

    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'row-resize'
    document.addEventListener('mousemove', handleChangesDrag)
    document.addEventListener('mouseup', stopChangesDrag)
    e.preventDefault()
    e.stopPropagation()
  }
}

// 修改 handleChangesDrag 函数
const handleChangesDrag = (e) => {
  if (!isDraggingChanges.value) return;

  const dy = e.clientY - startY.value;
  const newChangesHeight = initialHeightChanges.value + dy;

  // 计算最小和最大高度限制
  const minChangesHeight = changesHeaderHeight.value;
  const maxChangesHeight = panelsHeight.value - stagedHeaderHeight.value - commitsHeaderHeight.value - 16;

  // 确保高度在限制范围内
  const clampedChangesHeight = Math.max(minChangesHeight, Math.min(maxChangesHeight, newChangesHeight));

  // 计算Staged区域的新高度（剩余空间减去分隔线高度）
  const newStagedHeight = panelsHeight.value - clampedChangesHeight - 8 - commitsHeight.value;

  requestAnimationFrame(() => {
    // 更新高度值
    changesHeight.value = clampedChangesHeight;
    stagedHeight.value = newStagedHeight;

    // 当区域高度接近最小值时，隐藏内容但保留标题
    toggleSectionVisibility(changesSection.value, changesHeaderHeight.value);
    toggleSectionVisibility(stagedSection.value, stagedHeaderHeight.value);

  });
};

const stopChangesDrag = () => {
  isDraggingChanges.value = false;
  logger.debug('停止拖拽Changes区域分隔线', {
    finalChangesHeight: changesSection.value.offsetHeight,
    finalStagedHeight: stagedSection.value.offsetHeight
  });
  document.body.style.userSelect = '';
  document.body.style.cursor = '';
  document.removeEventListener('mousemove', handleChangesDrag);
  document.removeEventListener('mouseup', stopChangesDrag);
}

// 修改 startCommitsDrag 函数
const startCommitsDrag = (e) => {
  if (e.target === commitsDivider.value || e.target.closest('.divider-handle')) {
    isDraggingCommits.value = true
    startY.value = e.clientY
    initialHeightCommits.value = commitsHeight.value
    initialHeightStaged.value = stagedHeight.value
    initialHeightChanges.value = changesHeight.value
    panelsHeight.value = panelsContainer.value.offsetHeight

    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'row-resize'
    document.addEventListener('mousemove', handleCommitsDrag)
    document.addEventListener('mouseup', stopCommitsDrag)
    e.preventDefault()
    e.stopPropagation()
  }
}

const handleCommitsDrag = (e) => {
  if (!isDraggingCommits.value) return;

  // 边界检查
  if (panelsHeight.value <= 0) {
    logger.warn('无效的panelsHeight', panelsHeight.value);
    return;
  }

  const dy = e.clientY - startY.value;
  // logger.debug('Commits拖动开始', {
  //   dy,
  //   startY: startY.value,
  //   currentY: e.clientY,
  //   initialCommitsHeight: initialHeightCommits.value,
  //   initialChangesHeight: initialHeightChanges.value,
  //   initialStagedHeight: initialHeightStaged.value,
  //   panelsHeight: panelsHeight.value
  // });

  // 添加拖动视觉反馈
  commitsDivider.value?.classList.add('dragging');

  // 计算新高度（注意这里是减dy，确保方向正确）
  const newCommitsHeight = initialHeightCommits.value - dy;

  // 计算最小和最大高度限制
  const minCommitsHeight = Math.max(commitsHeaderHeight.value, 22); // 确保最小高度
  const maxCommitsHeight = Math.min(
    panelsHeight.value - changesHeaderHeight.value - stagedHeaderHeight.value - 16,
    panelsHeight.value * 0.8 // 不超过总高度的80%
  );

  // logger.debug('高度限制检查', {
  //   minCommitsHeight,
  //   maxCommitsHeight,
  //   newCommitsHeight,
  //   availableSpace: panelsHeight.value - changesHeaderHeight.value - stagedHeaderHeight.value
  // });

  // 应用边界限制
  const clampedCommitsHeight = Math.max(
    minCommitsHeight,
    Math.min(maxCommitsHeight, newCommitsHeight)
  );

  // 计算高度变化量（正值表示Commits区域减小）
  const heightChange = initialHeightCommits.value - clampedCommitsHeight;
  // logger.debug('高度变化计算', {
  //   heightChange,
  //   clampedCommitsHeight,
  //   initialCommitsHeight: initialHeightCommits.value
  // });

  // 使用requestAnimationFrame确保流畅更新
  requestAnimationFrame(() => {
    // 更新Commits区域高度
    commitsHeight.value = clampedCommitsHeight;

    // 计算上方区域应分配的高度变化
    const totalInitialHeight = initialHeightChanges.value + initialHeightStaged.value;
    const changesRatio = totalInitialHeight > 0
      ? initialHeightChanges.value / totalInitialHeight
      : 0.5;
    const stagedRatio = totalInitialHeight > 0
      ? initialHeightStaged.value / totalInitialHeight
      : 0.5;

    // logger.debug('区域比例计算', {
    //   totalInitialHeight,
    //   changesRatio,
    //   stagedRatio,
    //   changesHeaderHeight: changesHeaderHeight.value,
    //   stagedHeaderHeight: stagedHeaderHeight.value
    // });

    // 计算上方区域新高度（确保不低于最小高度）
    const newChangesHeight = Math.max(
      changesHeaderHeight.value,
      initialHeightChanges.value + (heightChange * changesRatio)
    );
    const newStagedHeight = Math.max(
      stagedHeaderHeight.value,
      initialHeightStaged.value + (heightChange * stagedRatio)
    );

    // 应用新高度
    changesHeight.value = newChangesHeight;
    stagedHeight.value = newStagedHeight;

    // logger.debug('最终高度设置', {
    //   changesHeight: changesHeight.value,
    //   stagedHeight: stagedHeight.value,
    //   commitsHeight: commitsHeight.value,
    //   remainingSpace: panelsHeight.value - changesHeight.value - stagedHeight.value - commitsHeight.value
    // });

    // 动态显示/隐藏内容区域
    toggleSectionVisibility(changesSection.value, changesHeaderHeight.value);
    toggleSectionVisibility(stagedSection.value, stagedHeaderHeight.value);

    // 强制重新布局（解决某些浏览器渲染问题）
    if (panelsContainer.value) {
      panelsContainer.value.style.overflow = 'hidden';
      panelsContainer.value.offsetHeight; // 触发重绘
      panelsContainer.value.style.overflow = '';
    }

  });
};

const stopCommitsDrag = () => {
  isDraggingCommits.value = false;
  commitsDivider.value?.classList.remove('dragging');
  document.body.style.userSelect = '';
  document.body.style.cursor = '';
  document.removeEventListener('mousemove', handleCommitsDrag);
  document.removeEventListener('mouseup', stopCommitsDrag);

  // 最终布局调整
  nextTick(() => {
    if (panelsContainer.value) {
      panelsContainer.value.style.display = 'none';
      panelsContainer.value.offsetHeight; // 触发重绘
      panelsContainer.value.style.display = 'flex';
    }
  });
};

// 新增辅助函数
const toggleSectionVisibility = (section, headerHeight) => {
  if (!section) return;

  const content = section.querySelector('.n-collapse-item__content');
  const header = section.querySelector('.n-collapse-item__header');
  const contentInner = section.querySelector('.n-collapse-item__content-inner');

  if (!content || !header || !contentInner) return;

  const sectionHeight = section.offsetHeight;
  const isCollapsed = sectionHeight <= headerHeight + 10;

  if (isCollapsed) {
    contentInner.classList.add('collapsed');
    header.style.cursor = 'row-resize';
  } else {
    contentInner.classList.remove('collapsed');
    header.style.cursor = '';
  }
};

const refreshGitStatus = async () => {
  logger.debug('Refreshing Git status')
  console.log('repo path:', props.workspacePath)
  try {
    const statuses = await invoke('get_git_status', {
      path: props.workspacePath
    });

    // 重置选择状态
    selectedFile.value = null;
    selectedCommit.value = null;
    isEditorInitialized.value = false;

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

watch(() => settingsStore.appearanceTheme, () => {
  monaco.editor.setTheme(getMonacoTheme())
})

const initMonacoEditor = async () => {
  if (isEditorInitialized.value || !diffEditorContainer.value) return;

  try {
    // 清理容器
    while (diffEditorContainer.value.firstChild) {
      diffEditorContainer.value.removeChild(diffEditorContainer.value.firstChild);
    }

    // 创建新的容器元素
    const editorContainer = document.createElement('div');
    editorContainer.className = 'monaco-diff-editor-container';
    editorContainer.style.height = '100%';
    editorContainer.style.width = '100%';
    diffEditorContainer.value.appendChild(editorContainer);

    // 注册自定义语言
    await registerDiffLanguages();

    // 初始化差异编辑器
    diffEditor = markRaw(monaco.editor.createDiffEditor(editorContainer, {
      originalEditable: false,
      readOnly: true,
      automaticLayout: true,
      theme: getMonacoTheme(),
      fontSize: 16,
      fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
      fontLigatures: true,
      renderSideBySide: true,
      renderIndicators: true,
      scrollBeyondLastLine: false,
      diffAlgorithm: 'advanced',
      diffWordWrap: 'on',
      enableSplitViewResizing: true,
      lineHeight: 24,  // 行高
      padding: {
        top: 8,  // 顶部内边距
        bottom: 8  // 底部内边距
      },
      // 确保差异导航功能可用
      diffReview: {
        showMovedCodeBlocks: true,
        followCaret: true
      }
    }));

    isEditorInitialized.value = true;
    monaco.editor.setTheme(getMonacoTheme());

    // 初始空模型
    const emptyModel = monaco.editor.createModel('', 'plaintext');
    diffEditor.setModel({
      original: emptyModel,
      modified: emptyModel
    });

    // 添加resize observer
    resizeObserver = new ResizeObserver(() => {
      if (diffEditor && editorContainer) {
        const height = editorContainer.clientHeight;
        const width = editorContainer.clientWidth;
        diffEditor.layout({ width, height });
      }
    });
    resizeObserver.observe(editorContainer);

  } catch (error) {
    console.error('Failed to initialize Monaco diff editor:', error);
    notification.error({
      title: '编辑器初始化失败',
      content: error.message,
      duration: 3000
    });
  }
}

const updateFileDiff = async (file) => {
  console.log('updateFileDiff: isEditorInitialized.value ', isEditorInitialized.value, 'diffEditorContainer.value', diffEditorContainer.value);
  if (!isEditorInitialized.value || !diffEditorContainer.value) {
    await initMonacoEditor();
  } else {
    isEditorInitialized.value = false;
  }

  console.log('updateFileDiff: diffEditor ', diffEditor, 'diffEditorContainer.value', diffEditorContainer.value);
  if (!diffEditor || !diffEditorContainer.value) {
    console.error('Diff editor not available');
    return;
  }

  diffLoading.value.file = true;

  try {
    const filePath = file.path.replace(/\\/g, '/');
    console.log('[Diff] Loading diff for:', filePath);

    // 先获取新内容
    const [originalContent, modifiedContent] = await Promise.all([
      invoke('get_file_content', {
        path: props.workspacePath,
        filePath: filePath,
        revision: 'HEAD'
      }).catch(e => {
        console.warn('Failed to get original content:', e);
        return ''; // 对于新文件，原始内容为空
      }),
      invoke('get_file_content', {
        path: props.workspacePath,
        filePath: filePath
      }).catch(e => {
        console.error('Failed to get modified content:', e);
        throw e;
      })
    ]);

    const language = getLanguageForFile(file.path);

    // 创建新模型
    const originalModel = monaco.editor.createModel(originalContent, language);
    const modifiedModel = monaco.editor.createModel(modifiedContent, language);

    // 获取当前模型以便后续清理
    const oldModel = diffEditor.getModel();

    // 设置新模型
    diffEditor.setModel({
      original: originalModel,
      modified: modifiedModel
    });

    // 清理旧模型（如果有）
    if (oldModel) {
      setTimeout(() => {
        oldModel.original?.dispose();
        oldModel.modified?.dispose();
      }, 100);
    }

    // 配置差异编辑器
    diffEditor.updateOptions({
      renderSideBySide: true,
      renderIndicators: true,
      renderWhitespace: 'selection',
      readOnly: true,
      automaticLayout: true
    });

    // 确保布局正确
    await nextTick();
    if (diffEditor && diffEditorContainer.value) {
      const height = diffEditorContainer.value.clientHeight;
      const width = diffEditorContainer.value.clientWidth;
      diffEditor.layout({ width, height });

      // 导航到第一个差异
      setTimeout(async () => {
        const modifiedEditor = diffEditor.getModifiedEditor();
        const diffNavAction = modifiedEditor.getAction('editor.action.diffReview.next');
        if (diffNavAction) {
          await diffNavAction.run();
          modifiedEditor.revealPositionInCenter(modifiedEditor.getPosition());
        }
      }, 100);
    }
  } catch (error) {
    console.error('[Diff] Error in updateFileDiff:', error);
    const errorModel = monaco.editor.createModel(
      `Error loading file diff:\n${error.message}`,
      'text/plain'
    );
    diffEditor.setModel({
      original: errorModel,
      modified: errorModel
    });
  } finally {
    diffLoading.value.file = false;
  }
}

// 添加编辑器大小调整函数
const handleEditorResize = () => {
  if (diffEditor && diffEditorContainer.value) {
    const height = diffEditorContainer.value.clientHeight
    const width = diffEditorContainer.value.clientWidth
    diffEditor.layout({ width, height })
  }
}

// 修改文件点击处理
const handleFileClick = async (file) => {
  try {
    selectedFile.value = file
    selectedCommit.value = null

    if (!isEditorInitialized.value) {
      await initMonacoEditor()
    }

    await nextTick()
    await updateFileDiff(file)
  } catch (error) {
    console.error('Failed to load file content', error)
    notification.error({
      title: '加载文件失败',
      content: `无法加载文件 ${file.path} 的内容: ${error}`,
      duration: 3000
    })
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

// 格式化相对时间
const formatDateRelative = (timestamp) => {
  const now = new Date()
  const date = new Date(parseInt(timestamp) * 1000)
  const diffInSeconds = Math.floor((now - date) / 1000)

  if (diffInSeconds < 60) {
    return `${diffInSeconds}秒前`
  }

  const diffInMinutes = Math.floor(diffInSeconds / 60)
  if (diffInMinutes < 60) {
    return `${diffInMinutes}分钟前`
  }

  const diffInHours = Math.floor(diffInMinutes / 60)
  if (diffInHours < 24) {
    return `${diffInHours}小时前`
  }

  const diffInDays = Math.floor(diffInHours / 24)
  if (diffInDays < 30) {
    return `${diffInDays}天前`
  }

  const diffInMonths = Math.floor(diffInDays / 30)
  if (diffInMonths < 12) {
    return `${diffInMonths}个月前`
  }

  const diffInYears = Math.floor(diffInMonths / 12)
  return `${diffInYears}年前`
}

// 监听 selectedFile 变化
watch(selectedFile, (newVal) => {
  if (newVal) {
    nextTick(() => {
      updateFileDiff(newVal)
    })
  } else if (diffEditor) {
    diffEditor.setModel(null)
  }
})

// 监听 selectedCommit 变化
watch(selectedCommit, (newVal) => {
  if (newVal) {
    nextTick(() => {
      updateCommitDiff(newVal)
    })
  } else if (diffEditor.value) {
    diffEditor.value.setModel(null)
  }
})

onMounted(() => {
  logger.debug('Component mounted')

  // 初始化操作
  const initialize = async () => {
    try {
      await Promise.all([
        refreshGitStatus(),
        fetchBranches()
      ])

      // 初始化编辑器但不等待
      initMonacoEditor().catch(console.error)

      // 设置面板高度
      nextTick(() => {
        if (changesSection.value && stagedSection.value && commitsSection.value && panelsContainer.value) {
          panelsHeight.value = panelsContainer.value.offsetHeight
          changesHeight.value = panelsHeight.value * 0.3
          stagedHeight.value = panelsHeight.value * 0.3
          commitsHeight.value = panelsHeight.value * 0.4

          const changesHeader = changesSection.value.querySelector('.n-collapse-item__header')
          const stagedHeader = stagedSection.value.querySelector('.n-collapse-item__header')
          const commitsHeader = commitsSection.value.querySelector('.n-collapse-item__header')

          changesHeaderHeight.value = changesHeader?.offsetHeight || 40
          stagedHeaderHeight.value = stagedHeader?.offsetHeight || 40
          commitsHeaderHeight.value = commitsHeader?.offsetHeight || 40

        }
      })
    } catch (error) {
      console.error('Initialization error:', error)
    }
  }

  // 执行初始化但不等待
  initialize().catch(console.error)
})

onUnmounted(() => {
  // 移除所有事件监听器
  document.removeEventListener('mousemove', handleSidebarDrag)
  document.removeEventListener('mouseup', stopSidebarDrag)
  document.removeEventListener('mousemove', handleChangesDrag)
  document.removeEventListener('mouseup', stopChangesDrag)
  document.removeEventListener('mousemove', handleCommitsDrag)
  document.removeEventListener('mouseup', stopCommitsDrag)

  // 清理编辑器
  if (diffEditor) {
    const model = diffEditor.getModel()
    if (model) {
      model.original?.dispose()
      model.modified?.dispose()
    }
    diffEditor.dispose()
    diffEditor = null
  }

  // 清理容器
  if (diffEditorContainer.value) {
    while (diffEditorContainer.value.firstChild) {
      diffEditorContainer.value.removeChild(diffEditorContainer.value.firstChild)
    }
  }

  // 移除resize observer
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }

  // 移除事件监听
  window.removeEventListener('resize', handleEditorResize)
})
</script>

<style>
/* 全局样式 - 浅色主题 */
:root {
  --bg-color: var(--guaxi-content-bg);
  --text-color: var(--guaxi-text-primary);
  --text-color-secondary: var(--guaxi-text-secondary);
  --divider-color: var(--guaxi-border);
  --hover-color: rgba(0, 0, 0, 0.05);
  --graph-bg: var(--guaxi-header-bg);
  --added-bg: rgba(40, 167, 69, 0.12);
  --modified-bg: rgba(255, 193, 7, 0.12);
  --deleted-bg: rgba(220, 53, 69, 0.12);
  --current-branch-bg: rgba(40, 167, 69, 0.16);
  --card-bg: var(--guaxi-panel-bg);
  --input-bg: var(--guaxi-panel-bg);
  --sidebar-bg: var(--guaxi-header-bg);
  --sidebar-width: 280px;
}

:root[data-guaxi-theme='dark'] {
  --hover-color: rgba(255, 255, 255, 0.06);
  --added-bg: rgba(40, 167, 69, 0.18);
  --modified-bg: rgba(255, 193, 7, 0.16);
  --deleted-bg: rgba(220, 53, 69, 0.16);
  --current-branch-bg: rgba(40, 167, 69, 0.22);
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
/* 主容器布局调整 */
.gitkraken-container {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  position: relative;
  background-color: var(--bg-color);
  color: var(--text-color);
}

.git-sidebar {
  width: 350px;
  min-width: 0px;
  max-width: 600px;
  height: 100%;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--divider-color);
  background-color: var(--sidebar-bg);
  position: relative;
  transition: width 0.2s ease;
}

/* 修改右侧内容区域样式 */
.git-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--card-bg);
}

/* 顶部导航栏样式 */
.content-navbar {
  padding: 8px 16px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  z-index: 1;
  flex-shrink: 0;
}

.content-viewer {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  min-height: 0;
  /* 关键：允许内容区域收缩 */
}

.diff-viewer,
.commit-details {
  flex: 1;
  min-height: 0;
  /* 关键：允许内部内容收缩 */
  display: flex;
  flex-direction: column;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

/* Monaco Editor 自定义样式 */
.monaco-diff-editor-container {
  height: 100%;
}

/* 差异高亮样式 */
.monaco-editor .line-insert {
  background-color: rgba(155, 185, 85, 0.2);
}

.monaco-editor .line-delete {
  background-color: rgba(255, 0, 0, 0.2);
}

.monaco-editor .char-insert {
  background-color: rgba(155, 185, 85, 0.3);
}

.monaco-editor .char-delete {
  background-color: rgba(255, 0, 0, 0.3);
}

/* 分隔线样式 */
.monaco-diff-editor .split-view-view {
  border-right: 1px solid var(--divider-color);
}

/* 行号样式 */
.monaco-editor .line-numbers {
  color: var(--text-color-secondary) !important;
}

/* 提交详情样式 */
.commit-details {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.commit-message {
  padding: 12px 16px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--bg-color);
}

/* 空状态样式 */
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}



.source-control-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  /* flex-shrink: 0; */
}

.commit-section {
  padding: 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

/* 新增 panels-container 样式 */
.panels-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  position: relative;
}

/* 修改 changes-container 样式 */
.changes-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  position: relative;
}

.changes-section,
.staged-section,
.commits-section {
  overflow: auto;
  display: flex;
  flex-direction: column;
  min-height: 40px;
  /* 标题栏高度 */
  transition: flex 0.1s ease-out;
  background-color: var(--card-bg);
  border-top: 1px solid var(--divider-color);
}

/* 自定义滚动条样式 */
.changes-section::-webkit-scrollbar,
.staged-section::-webkit-scrollbar,
.commits-section::-webkit-scrollbar {
  width: 3px;
}

.changes-section::-webkit-scrollbar-track,
.staged-section::-webkit-scrollbar-track,
.commits-section::-webkit-scrollbar-track {
  background: transparent;
}

.changes-section::-webkit-scrollbar-thumb,
.staged-section::-webkit-scrollbar-thumb,
.commits-section::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 3px;
  opacity: 0.3;
}

.changes-section::-webkit-scrollbar-thumb:hover,
.staged-section::-webkit-scrollbar-thumb:hover,
.commits-section::-webkit-scrollbar-thumb:hover {
  opacity: 0.6;
}

.commits-toolbar {
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
}

.commits-list {
  height: calc(100% - 40px);
  overflow: hidden;
  margin-bottom: 40px;
  padding-bottom: 120px;
}

.commit-item {
  padding: 8px 12px;
  cursor: pointer;
  transition: background-color 0.2s ease;
  border-left: 3px solid transparent;
}

.commit-item:hover {
  background-color: var(--hover-color);
}

.commit-summary {
  display: flex;
  align-items: center;
  margin-bottom: 4px;
}

.commit-hash {
  margin-left: 8px;
  font-family: monospace;
  font-size: 12px;
}

.commit-meta {
  display: flex;
  font-size: 12px;
}

.commit-meta>*:not(:last-child) {
  margin-right: 8px;
}

.expanded-commit {
  background-color: rgba(0, 123, 255, 0.05);
  border-left: 3px solid #007bff;
}

.selected-commit {
  background-color: rgba(0, 123, 255, 0.1);
}

.commit-files {
  margin-top: 8px;
  margin-left: 16px;
  border-left: 2px solid var(--divider-color);
  padding-left: 8px;
}

.selected-file {
  background-color: rgba(0, 123, 255, 0.05);
  border-left: 2px solid #007bff;
}



/* 确保分隔线可见 */
.section-divider,
/* 分隔条样式优化 */
.commits-divider {
  height: 2px;
  background-color: #69abd8;
  cursor: row-resize;
  position: relative;
  transition: background-color 0.2s ease;
  z-index: 10;
}

.commits-divider.dragging {
  background-color: #69abd8;
  height: 2px;
  z-index: 100;
}

.commits-divider .divider-handle {
  position: absolute;
  top: 2px;
  left: 50%;
  transform: translateX(-50%);
  width: 40px;
  height: 2px;
  background-color: var(--primary-color);
  border-radius: 2px;
  opacity: 0.3;
  transition: opacity 0.2s, width 0.2s;
}

.commits-divider:hover .divider-handle,
.commits-divider.dragging .divider-handle {
  opacity: 1;
  width: 60px;
}

/* 防止拖动时文本选中 */
.panels-container {
  user-select: none;
}

/* 动态内容区域显示控制 */
.n-collapse-item__content-inner {
  transition: opacity 0.15s ease;
}

.n-collapse-item__content-inner.collapsed {
  opacity: 0;
  height: 0;
  overflow: hidden;
}

/* 确保标题栏始终可见 */
.n-collapse-item__header {
  position: sticky;
  top: 0;
  background-color: var(--card-bg);
  z-index: 1;
  height: 40px;
  /* 固定标题栏高度 */
  display: flex;
  align-items: center;
  padding: 0 12px;
}

.n-list {
  flex: 1;
  min-height: 0;
  overflow: hidden;
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

/* 差异对比视图样式 */
.diff-container {
  display: flex;
  height: 100%;
  width: 100%;
}

.diff-original,
.diff-modified {
  flex: 1;
  overflow: auto;
  border-right: 1px solid var(--divider-color);
  position: relative;
}

.diff-modified {
  border-right: none;
}

.diff-header {
  position: sticky;
  top: 0;
  background-color: var(--card-bg);
  padding: 8px 12px;
  font-weight: bold;
  border-bottom: 1px solid var(--divider-color);
  z-index: 1;
}

.diff-line {
  padding: 0 12px;
  line-height: 1.5;
  font-family: 'Maple Mono NF CN', monospace;
  font-size: 14px;
  white-space: pre;
}

.added-line {
  background-color: rgba(46, 160, 67, 0.1);
}

.removed-line {
  background-color: rgba(203, 36, 49, 0.1);
  text-decoration: line-through;
}

.unchanged-line {
  color: var(--text-color);
}

.empty-line {
  background-color: rgba(0, 0, 0, 0.05);
  min-height: 1.5em;
}

/* 行号样式 */
.diff-line::before {
  content: attr(data-line);
  display: inline-block;
  width: 40px;
  padding-right: 12px;
  color: var(--text-color-secondary);
  text-align: right;
  user-select: none;
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

.current-branch-item {
  background-color: rgba(40, 167, 69, 0.1);
}

/* 分支面板容器 */
.branch-panel-container {
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* 创建分支按钮区域 */
.create-branch-footer {
  margin-top: auto;
  padding-top: 16px;
  border-top: 1px solid var(--divider-color);
}

/* 优化编辑器容器样式 */
.diff-editor-container {
  flex: 1;
  height: calc(100% - 41px);
  width: 100%;
  min-height: 200px;
  position: relative;
  overflow: hidden;
  padding: 0px;
}

/* 调整 Monaco Editor 主题 */
.monaco-editor .line-numbers {
  color: var(--text-color-secondary) !important;
}

.monaco-editor .margin {
  background-color: var(--sidebar-bg) !important;
}

.monaco-editor .monaco-editor-background {
  background-color: var(--card-bg) !important;
}

/* 差异高亮样式 */
.monaco-editor .char-delete {
  background-color: rgba(203, 36, 49, 0.1) !important;
}

.monaco-editor .char-insert {
  background-color: rgba(40, 167, 69, 0.1) !important;
}

.monaco-editor .line-delete {
  background-color: rgba(203, 36, 49, 0.05) !important;
}

.monaco-editor .line-insert {
  background-color: rgba(40, 167, 69, 0.05) !important;
}

/* 添加加载状态样式 */
.loading-overlay {
  position: absolute;
  top: 41px;
  /* 工具栏高度 */
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(255, 255, 255, 0.7);
  z-index: 10;
}

:root[data-guaxi-theme='dark'] .loading-overlay {
  background-color: rgba(0, 0, 0, 0.5);
}

/* 新增折叠按钮样式 */
.sidebar-collapse-button {
  position: absolute;
  right: -20px;
  top: 50%;
  transform: translateY(-50%);
  width: 20px;
  height: 40px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--divider-color);
  border-left: none;
  border-radius: 0 4px 4px 0;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  z-index: 100;
  transition: all 0.2s ease;
}

.sidebar-collapse-button.collapsed {
  position: fixed;
  left: 0;
  right: auto;
  border-left: 1px solid var(--divider-color);
  border-radius: 0 4px 4px 0;
}

.sidebar-collapse-button:hover {
  background-color: var(--hover-color);
}

.sidebar-collapse-button .n-icon {
  font-size: 16px;
  color: var(--text-color-secondary);
}

.sidebar-content {
  height: 100%;
  overflow: hidden;
}
</style>
