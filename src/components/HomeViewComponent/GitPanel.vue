<template>
    <n-modal v-model:show="visible" :mask-closable="false" preset="card" :style="modalStyle" class="gitkraken-modal"
        @after-leave="handleClose">
        <!-- 主容器 -->
        <div class="gitkraken-container">
            <!-- 左侧分支图 -->
            <div class="git-graph">
                <div class="graph-container" ref="graphContainer">
                    <div class="branch-list">
                        <n-scrollbar>
                            <n-list>
                                <n-list-item v-for="branch in branches" :key="branch.name"
                                    :class="{ 'current-branch': branch.is_current }">
                                    <n-space vertical>
                                        <n-space align="center" @click="toggleBranch(branch.name)">
                                            <n-icon v-if="branch.is_current" color="#4fc08d">
                                                <GitBranchOutline />
                                            </n-icon>
                                            <span>{{ branch.name }}</span>
                                            <n-icon>
                                                <ChevronDownOutline v-if="expandedBranches.has(branch.name)" />
                                                <ChevronForwardOutline v-else />
                                            </n-icon>
                                        </n-space>

                                        <n-collapse-transition v-if="expandedBranches.has(branch.name)">
                                            <n-list class="commit-list">
                                                <n-list-item v-for="commit in branchCommits[branch.name] || []"
                                                    :key="commit.id" @click="selectCommit(commit)"
                                                    :class="{ 'selected-commit': selectedCommit?.id === commit.id }">
                                                    <n-space vertical>
                                                        <n-text strong>{{ commit.summary }}</n-text>
                                                        <n-text depth="3" style="font-size: 12px">
                                                            {{ commit.author }} - {{ formatDate(commit.date) }}
                                                        </n-text>
                                                    </n-space>
                                                </n-list-item>
                                            </n-list>
                                        </n-collapse-transition>
                                    </n-space>
                                </n-list-item>
                            </n-list>
                        </n-scrollbar>
                    </div>
                </div>
            </div>

            <!-- 右侧操作区 -->
            <div class="git-operations">
                <!-- 顶部工具栏 -->
                <div class="git-toolbar">
                    <n-space>
                        <n-button type="primary" @click="handlePull" :loading="gitLoading.pull">
                            <template #icon>
                                <n-icon>
                                    <CloudDownloadOutline />
                                </n-icon>
                            </template>
                            拉取
                        </n-button>
                        <n-button type="primary" @click="handlePush" :loading="gitLoading.push">
                            <template #icon>
                                <n-icon>
                                    <CloudUploadOutline />
                                </n-icon>
                            </template>
                            推送
                        </n-button>
                        <n-button type="info" @click="refreshGitStatus">
                            <template #icon>
                                <n-icon>
                                    <RefreshOutline />
                                </n-icon>
                            </template>
                            刷新
                        </n-button>
                        <n-button type="success" @click="showCreateBranchModal = true">
                            <template #icon>
                                <n-icon>
                                    <GitBranchOutline />
                                </n-icon>
                            </template>
                            新建分支
                        </n-button>
                    </n-space>

                    <div class="branch-info">
                        <n-tag type="info" round>
                            <n-icon>
                                <GitBranchOutline />
                            </n-icon>
                            {{ currentBranch }}
                        </n-tag>
                    </div>
                </div>

                <!-- 变更文件列表 -->
                <div class="file-changes">
                    <n-tabs type="segment" size="small">
                        <n-tab-pane name="unstaged" tab="工作区">
                            <n-data-table :columns="fileColumns" :data="unstagedFiles" :pagination="pagination"
                                size="small" :row-class-name="getRowClass" @row-click="handleFileClick" />
                        </n-tab-pane>
                        <n-tab-pane name="staged" tab="暂存区">
                            <n-data-table :columns="fileColumns" :data="stagedFiles" :pagination="pagination"
                                size="small" :row-class-name="getRowClass" @row-click="handleFileClick" />
                        </n-tab-pane>
                    </n-tabs>

                    <!-- 文件差异预览 -->
                    <div class="diff-viewer" v-if="selectedFile">
                        <n-card size="small" :title="selectedFile.path">
                            <n-scrollbar style="max-height: 300px">
                                <pre class="diff-content">{{ selectedFile.diff || '无差异内容' }}</pre>
                            </n-scrollbar>
                        </n-card>
                    </div>

                    <!-- 提交详情预览 -->
                    <div class="commit-details" v-if="selectedCommit">
                        <n-card size="small" :title="`提交 ${selectedCommit.id.slice(0, 7)}`">
                            <n-scrollbar style="max-height: 300px">
                                <div class="commit-meta">
                                    <n-space vertical>
                                        <n-text strong>作者: {{ selectedCommit.author }}</n-text>
                                        <n-text>日期: {{ formatDateFull(selectedCommit.date) }}</n-text>
                                    </n-space>
                                </div>
                                <pre class="commit-message">{{ selectedCommit.message }}</pre>
                            </n-scrollbar>
                        </n-card>
                    </div>
                </div>

                <!-- 提交区域 -->
                <div class="commit-area">
                    <n-input v-model:value="commitForm.message" type="textarea" placeholder="提交信息 (必填)"
                        :autosize="{ minRows: 3 }" />
                    <div class="commit-actions">
                        <n-space>
                            <n-button type="primary" @click="handleCommit" :disabled="!commitForm.message.trim()">
                                提交
                            </n-button>
                            <n-button @click="handleStageAll">
                                暂存全部
                            </n-button>
                            <n-button @click="handleUnstageAll">
                                取消暂存
                            </n-button>
                        </n-space>
                        <div class="change-stats">
                            <n-space>
                                <n-tag type="success" size="small">
                                    <n-icon>
                                        <AddCircleOutline />
                                    </n-icon>
                                    {{ gitChanges.added }}
                                </n-tag>
                                <n-tag type="warning" size="small">
                                    <n-icon>
                                        <CreateOutline />
                                    </n-icon>
                                    {{ gitChanges.modified }}
                                </n-tag>
                                <n-tag type="error" size="small">
                                    <n-icon>
                                        <RemoveCircleOutline />
                                    </n-icon>
                                    {{ gitChanges.deleted }}
                                </n-tag>
                            </n-space>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- 新建分支模态框 -->
        <n-modal v-model:show="showCreateBranchModal" title="新建分支" preset="dialog" positive-text="创建"
            @positive-click="handleCreateBranch">
            <n-input v-model:value="newBranchName" placeholder="输入新分支名称" />
        </n-modal>
    </n-modal>
</template>

<script setup>
import { ref, computed, onMounted, h } from 'vue'
import { NIcon, NTag, NButton, NCollapseTransition } from 'naive-ui'
import {
    GitBranchOutline,
    CloudDownloadOutline,
    CloudUploadOutline,
    RefreshOutline,
    AddCircleOutline,
    CreateOutline,
    RemoveCircleOutline,
    ChevronDownOutline,
    ChevronForwardOutline
} from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'

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

const message = useMessage()

// 控制显示状态
const visible = ref(false)
const showCreateBranchModal = ref(false)
const newBranchName = ref('')

// 状态数据
const currentBranch = ref('')
const branches = ref([])
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
const expandedBranches = ref(new Set())
const branchCommits = ref({})
const selectedCommit = ref(null)

// 表单数据
const commitForm = ref({
    message: ''
})

// 模态框样式
const modalStyle = computed(() => ({
    width: '90vw',
    height: '85vh',
    maxWidth: '1400px',
    maxHeight: '900px'
}))

// 表格列定义
const fileColumns = [
    {
        title: '文件',
        key: 'path',
        render: (row) => {
            const icon = row.status === 'A' ? AddCircleOutline :
                row.status === 'M' ? CreateOutline :
                    row.status === 'D' ? RemoveCircleOutline : null
            return h('div', { class: 'file-item' }, [
                icon ? h(NIcon, { component: icon, size: 16, class: 'file-icon' }) : null,
                h('span', { class: 'file-path' }, row.path)
            ])
        }
    },
    {
        title: '状态',
        key: 'status',
        render: (row) => {
            const statusMap = {
                'A': { type: 'success', text: '新增' },
                'M': { type: 'warning', text: '修改' },
                'D': { type: 'error', text: '删除' },
                '?': { type: 'default', text: '未跟踪' }
            }
            const status = statusMap[row.status] || { type: 'default', text: row.status }
            return h(NTag, { type: status.type, size: 'small', bordered: false }, { default: () => status.text })
        }
    },
    {
        title: '操作',
        key: 'actions',
        render: (row) => h('div', { class: 'file-actions' }, [
            h(NButton, {
                text: true,
                size: 'small',
                onClick: (e) => {
                    e.stopPropagation()
                    handleStageFile(row)
                }
            }, { default: () => '暂存' }),
            h(NButton, {
                text: true,
                size: 'small',
                onClick: (e) => {
                    e.stopPropagation()
                    handleDiscardFile(row)
                }
            }, { default: () => '丢弃' })
        ])
    }
]

const pagination = {
    pageSize: 10
}

// 方法
const open = () => {
    logger.info('Opening Git panel', { workspacePath: props.workspacePath })
    visible.value = true
    refreshGitStatus()
}

const handleClose = () => {
    logger.debug('Closing Git panel')
    selectedFile.value = null
    selectedCommit.value = null
    commitForm.value.message = ''
    expandedBranches.value.clear()
}

const refreshGitStatus = async () => {
    logger.debug('Refreshing Git status')
    try {
        // 获取当前分支
        currentBranch.value = await invoke('get_current_branch', { path: props.workspacePath })
        logger.debug('Current branch retrieved', { branch: currentBranch.value })

        // 获取分支列表
        branches.value = await invoke('list_branches', { path: props.workspacePath })
        logger.debug('Branches retrieved', { count: branches.value.length })

        // 获取文件状态
        const statuses = await invoke('get_git_status', { path: props.workspacePath })
        logger.debug('Git status retrieved', { count: statuses.length })

        // 计算变更数量
        gitChanges.value = {
            added: statuses.filter(s => s.status === 'A').length,
            modified: statuses.filter(s => s.status === 'M').length,
            deleted: statuses.filter(s => s.status === 'D').length
        }
        logger.debug('Git changes calculated', gitChanges.value)

        // 设置未暂存文件
        unstagedFiles.value = statuses.map(file => ({
            path: file.path,
            status: file.status,
            diff: '' // 初始为空，点击后加载
        }))

        // 暂存区文件从工作区中筛选已暂存的文件
        stagedFiles.value = unstagedFiles.value.filter(file => ['A', 'M', 'D'].includes(file.status))
        logger.debug('Files categorized', {
            unstaged: unstagedFiles.value.length,
            staged: stagedFiles.value.length
        })
    } catch (error) {
        logger.error('Failed to refresh Git status', error)
        message.error(`获取Git状态失败: ${error}`)
    }
}

const toggleBranch = async (branchName) => {
    if (expandedBranches.value.has(branchName)) {
        expandedBranches.value.delete(branchName);
    } else {
        expandedBranches.value.add(branchName);
        if (!branchCommits.value[branchName]) {
            try {
                // 确保参数名与 Rust 后端定义一致
                branchCommits.value[branchName] = await invoke('get_git_log', {
                    path: props.workspacePath,
                    branchName: branchName  // 使用 branch_name 而不是 branchName
                });
            } catch (error) {
                logger.error('Failed to get git log', { branch: branchName, error });
                message.error(`获取提交历史失败: ${error}`);
            }
        }
    }
};

const selectCommit = (commit) => {
    selectedCommit.value = commit;
};

const formatDate = (timestamp) => {
    return new Date(parseInt(timestamp) * 1000).toLocaleDateString();
};

const formatDateFull = (timestamp) => {
    return new Date(parseInt(timestamp) * 1000).toLocaleString();
};

const handleFileClick = async (row) => {
    logger.debug('File clicked', { file: row.path })
    try {
        selectedFile.value = row
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
        message.error(`获取文件内容失败: ${error}`)
    }
}

const handleStageFile = async (file) => {
    logger.debug('Staging file', { file: file.path })
    try {
        file.status = 'M'
        logger.info('File staged', { file: file.path })
        message.success(`已暂存文件: ${file.path}`)
        refreshGitStatus()
    } catch (error) {
        logger.error('Failed to stage file', { file: file.path, error })
        message.error(`暂存文件失败: ${error}`)
    }
}

const handleStageAll = async () => {
    logger.debug('Staging all files')
    try {
        unstagedFiles.value.forEach(file => {
            if (file.status === '?') file.status = 'A'
        })
        logger.info('All files staged', { count: unstagedFiles.value.length })
        message.success('已暂存所有变更')
        refreshGitStatus()
    } catch (error) {
        logger.error('Failed to stage all files', error)
        message.error(`暂存全部失败: ${error}`)
    }
}

const handleUnstageAll = async () => {
    logger.debug('Unstaging all files')
    try {
        stagedFiles.value.forEach(file => {
            if (['A', 'M', 'D'].includes(file.status)) file.status = '?'
        })
        logger.info('All files unstaged', { count: stagedFiles.value.length })
        message.success('已取消所有暂存')
        refreshGitStatus()
    } catch (error) {
        logger.error('Failed to unstage all files', error)
        message.error(`取消暂存失败: ${error}`)
    }
}

const handleDiscardFile = async (file) => {
    logger.debug('Discarding file changes', { file: file.path })
    try {
        const index = unstagedFiles.value.findIndex(f => f.path === file.path)
        if (index !== -1) {
            unstagedFiles.value.splice(index, 1)
        }
        logger.info('File changes discarded', { file: file.path })
        message.success(`已丢弃文件变更: ${file.path}`)
        refreshGitStatus()
    } catch (error) {
        logger.error('Failed to discard file changes', { file: file.path, error })
        message.error(`丢弃文件失败: ${error}`)
    }
}

const handleCommit = async () => {
    logger.debug('Preparing to commit', { message: commitForm.value.message })
    if (!commitForm.value.message.trim()) {
        logger.warn('Commit message is empty')
        message.warning('请输入提交信息')
        return
    }

    gitLoading.value.commit = true
    try {
        logger.info('Committing changes', { message: commitForm.value.message })
        await invoke('git_commit', {
            path: props.workspacePath,
            message: commitForm.value.message
        })
        logger.info('Commit successful')
        message.success('提交成功')
        commitForm.value.message = ''
        refreshGitStatus()
    } catch (error) {
        logger.error('Commit failed', { error })
        message.error(`提交失败: ${error}`)
    } finally {
        gitLoading.value.commit = false
    }
}

const handlePull = async () => {
    logger.debug('Pulling changes')
    gitLoading.value.pull = true
    try {
        await invoke('git_pull', { path: props.workspacePath })
        logger.info('Pull successful')
        message.success('拉取成功')
        refreshGitStatus()
    } catch (error) {
        logger.error('Pull failed', { error })
        message.error(`拉取失败: ${error}`)
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
        message.success('推送成功')
    } catch (error) {
        logger.error('Push failed', { error })
        message.error(`推送失败: ${error}`)
    } finally {
        gitLoading.value.push = false
    }
}

const checkoutBranch = async (branchName) => {
    logger.debug('Checking out branch', { branch: branchName })
    expandedBranches.value.delete(branchName)
    try {
        await invoke('checkout_branch', {
            path: props.workspacePath,
            branch_name: branchName
        })
        logger.info('Branch checkout successful', { branch: branchName })
        message.success(`已切换到分支 ${branchName}`)
        refreshGitStatus()
    } catch (error) {
        logger.error('Branch checkout failed', { branch: branchName, error })
        message.error(`切换分支失败: ${error}`)
    }
}

const handleCreateBranch = async () => {
    logger.debug('Creating new branch', { name: newBranchName.value })
    if (!newBranchName.value.trim()) {
        logger.warn('New branch name is empty')
        message.warning('请输入分支名称')
        return
    }

    try {
        await invoke('create_branch', {
            path: props.workspacePath,
            branch_name: newBranchName.value
        })
        logger.info('Branch created', { name: newBranchName.value })
        message.success(`已创建分支 ${newBranchName.value}`)
        showCreateBranchModal.value = false
        newBranchName.value = ''
        refreshGitStatus()
    } catch (error) {
        logger.error('Failed to create branch', { name: newBranchName.value, error })
        message.error(`创建分支失败: ${error}`)
    }
}

const getRowClass = (row) => {
    return `file-row file-status-${row.status.toLowerCase()}`
}

onMounted(() => {
    logger.debug('Component mounted')
    refreshGitStatus()
})

defineExpose({
    open
})
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
    background-color: var(--card-bg);
    padding: 8px;
    border-radius: 4px;
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
    height: 100%;
    background-color: var(--bg-color);
    color: var(--text-color);
}

.git-graph {
    width: 300px;
    border-right: 1px solid var(--divider-color);
    overflow-y: auto;
    padding: 16px;
    background-color: var(--graph-bg);
}

.git-operations {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.git-toolbar {
    padding: 12px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--divider-color);
    background-color: var(--card-bg);
}

.file-changes {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--bg-color);
}

.diff-viewer,
.commit-details {
    background-color: var(--card-bg);
    margin: 12px;
    border-radius: 6px;
}

.commit-area {
    padding: 12px;
    border-top: 1px solid var(--divider-color);
    background-color: var(--card-bg);
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

.diff-content {
    font-family: 'Maple Mono NF CN', monospace;
    white-space: pre-wrap;
    font-size: 14px;
    color: var(--text-color);
}
</style>