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

      <!-- 变更文件列表容器 -->
      <div class="changes-container">
        <!-- 新增 panels-container 作为共同父容器 -->
        <div class="panels-container" ref="panelsContainer">
          <!-- Changes区域 -->
          <div class="changes-section" ref="changesSection" :style="{ flex: `0 0 ${changesHeight}px` }">
            <n-collapse :default-expanded-names="['changes']">
              <n-collapse-item name="changes" title="Changes">
                <template #header-extra>
                  <n-badge :value="unstagedFiles.length" :max="99" />
                </template>
                <div class="scrollable-content">
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
                </div>
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
                <div class="scrollable-content">
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
                </div>
              </n-collapse-item>
            </n-collapse>
          </div>

          <!-- Staged Changes和提交历史之间的分隔线 -->
          <div class="commits-divider" ref="commitsDivider" @mousedown="startCommitsDrag">
            <div class="divider-handle"></div>
          </div>

          <!-- 提交历史区域 -->
          <div class="commits-section" ref="commitsSection" :style="{ flex: `0 0 ${commitsHeight}px` }">
            <n-collapse :default-expanded-names="['commits']">
              <n-collapse-item name="commits" title="提交历史">
                <div class="scrollable-content">
                  <n-list>
                    <!-- 操作按钮行 -->
                    <n-list-item>
                      <n-space justify="space-between">
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
                        <!-- 将分支标签移动到这里 -->
                        <n-tag type="info" size="small" @click="showBranchPanel = true" style="cursor: pointer">
                          <n-icon>
                            <GitBranchOutline />
                          </n-icon>
                          {{ currentBranch }}
                        </n-tag>
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
                </div>
              </n-collapse-item>
            </n-collapse>
          </div>
        </div>
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
import { ref, h, onMounted, onUnmounted, nextTick } from 'vue'
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
  CheckmarkCircleOutline
} from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'

const { notification } = createDiscreteApi(['notification'])

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
  commit: false,
  checkout: false  // 添加分支切换加载状态
});
const selectedCommit = ref(null)

// 表单数据
const commitForm = ref({
  message: ''
})

// 在现有代码的顶部添加
const MIN_HEIGHT_FOR_SCROLL = 200; // 触发滚动条的最小高度


// 分支面板相关状态
const showBranchPanel = ref(false)
const showCreateBranchModal = ref(false)
const newBranchName = ref('')

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

// 在现有函数后面添加新函数
const checkScrollableSections = () => {
  // Changes区域
  if (changesSection.value) {
    const content = changesSection.value.querySelector('.scrollable-content');
    if (content) {
      const shouldScroll = changesHeight.value < MIN_HEIGHT_FOR_SCROLL;
      content.style.overflowY = shouldScroll ? 'auto' : 'hidden';
      content.style.maxHeight = shouldScroll ? `${changesHeight.value - changesHeaderHeight.value}px` : '';
    }
  }

  // Staged区域
  if (stagedSection.value) {
    const content = stagedSection.value.querySelector('.scrollable-content');
    if (content) {
      const shouldScroll = stagedHeight.value < MIN_HEIGHT_FOR_SCROLL;
      content.style.overflowY = shouldScroll ? 'auto' : 'hidden';
      content.style.maxHeight = shouldScroll ? `${stagedHeight.value - stagedHeaderHeight.value}px` : '';
    }
  }

  // Commits区域
  if (commitsSection.value) {
    const content = commitsSection.value.querySelector('.scrollable-content');
    if (content) {
      const shouldScroll = commitsHeight.value < MIN_HEIGHT_FOR_SCROLL;
      content.style.overflowY = shouldScroll ? 'auto' : 'hidden';
      content.style.maxHeight = shouldScroll ? `${commitsHeight.value - commitsHeaderHeight.value}px` : '';
    }
  }
};

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

    // 检查是否需要显示滚动条
    checkScrollableSections();
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

    // 检查是否需要显示滚动条
    checkScrollableSections();
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

onMounted(() => {
  logger.debug('Component mounted')
  refreshGitStatus()
  fetchBranches() // 添加这行

  nextTick(() => {
    if (changesSection.value && stagedSection.value && commitsSection.value && panelsContainer.value) {
      panelsHeight.value = panelsContainer.value.offsetHeight

      // 设置初始高度比例
      changesHeight.value = panelsHeight.value * 0.3
      stagedHeight.value = panelsHeight.value * 0.3
      commitsHeight.value = panelsHeight.value * 0.4

      // 确保标题栏高度正确
      const changesHeader = changesSection.value.querySelector('.n-collapse-item__header')
      const stagedHeader = stagedSection.value.querySelector('.n-collapse-item__header')
      const commitsHeader = commitsSection.value.querySelector('.n-collapse-item__header')

      changesHeaderHeight.value = changesHeader?.offsetHeight || 40
      stagedHeaderHeight.value = stagedHeader?.offsetHeight || 40
      commitsHeaderHeight.value = commitsHeader?.offsetHeight || 40

      // 初始检查滚动条
      checkScrollableSections();
    }
  })
})

onUnmounted(() => {
  // 移除所有事件监听器
  document.removeEventListener('mousemove', handleSidebarDrag)
  document.removeEventListener('mouseup', stopSidebarDrag)
  document.removeEventListener('mousemove', handleChangesDrag)
  document.removeEventListener('mouseup', stopChangesDrag)
  document.removeEventListener('mousemove', handleCommitsDrag)
  document.removeEventListener('mouseup', stopCommitsDrag)
})

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
    // 刷新后检查滚动条
    nextTick(checkScrollableSections);
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

    // 操作完成后检查滚动条
    nextTick(checkScrollableSections);
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

    // 操作完成后检查滚动条
    nextTick(checkScrollableSections);
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
.gitkraken-container {
  display: flex;
  height: 100vh;
  width: 100vw;
  background-color: var(--bg-color);
  color: var(--text-color);
}

.git-sidebar {
  display: flex;
  flex-direction: column;
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

.source-control-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
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
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 40px;
  /* 标题栏高度 */
  transition: flex 0.1s ease-out;
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

.scrollable-content {
  flex: 1;
  overflow-y: hidden;
  /* 默认隐藏，根据高度动态显示 */
  min-height: 0;
  transition: all 0.2s ease;
  padding-right: 4px;
  /* 为滚动条留出空间 */
}

/* 自定义滚动条样式 */
.scrollable-content::-webkit-scrollbar {
  width: 6px;
}

.scrollable-content::-webkit-scrollbar-track {
  background: transparent;
}

.scrollable-content::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 3px;
  opacity: 0.3;
}

.scrollable-content::-webkit-scrollbar-thumb:hover {
  opacity: 0.6;
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

/* 自定义滚动条样式 */
.scrollable-content::-webkit-scrollbar {
  width: 6px;
}

.scrollable-content::-webkit-scrollbar-track {
  background: transparent;
}

.scrollable-content::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 3px;
  opacity: 0.3;
}

.scrollable-content::-webkit-scrollbar-thumb:hover {
  opacity: 0.6;
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
</style>