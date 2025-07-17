<template>
    <n-modal v-model:show="visible" :mask-closable="false" preset="card" :style="modalStyle" class="gitkraken-modal"
        @after-leave="handleClose">
        <!-- 主容器 -->
        <div class="gitkraken-container">
            <!-- 左侧分支图 -->
            <div class="git-graph">
                <div class="graph-container" ref="graphContainer">
                    <svg class="branch-graph" :width="graphWidth" :height="graphHeight">
                        <g v-for="(commit, index) in commitHistory" :key="commit.id"
                            :transform="`translate(50, ${index * 80 + 40})`">
                            <circle cx="0" cy="0" r="15" :fill="getCommitColor(commit)" />
                            <text x="25" y="5" class="commit-message">{{ commit.message }}</text>
                            <text x="25" y="25" class="commit-meta">{{ commit.author }} - {{ formatDate(commit.date)
                                }}</text>
                            <text x="-20" y="5" class="commit-id">{{ commit.id.substring(0, 7) }}</text>
                        </g>
                    </svg>
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
                    </n-space>

                    <div class="branch-info">
                        <n-tag type="info" round>
                            <n-icon>
                                <GitBranchOutline />
                            </n-icon>
                            {{ gitBranch }}
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
    </n-modal>
</template>

<script setup>
import { ref, computed, onMounted, h } from 'vue'
import {
    GitBranchOutline,
    CloudDownloadOutline,
    CloudUploadOutline,
    RefreshOutline,
    AddCircleOutline,
    CreateOutline,
    RemoveCircleOutline
} from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { NButton, NIcon, NTag } from 'naive-ui'

const props = defineProps({
    workspacePath: {
        type: String,
        required: true
    }
})

const message = useMessage()

// 控制显示状态
const visible = ref(false)

// 状态数据
const gitBranch = ref('')
const gitChanges = ref({
    added: 0,
    modified: 0,
    deleted: 0
})
const unstagedFiles = ref([])
const stagedFiles = ref([])
const commitHistory = ref([])
const selectedFile = ref(null)
const gitLoading = ref({
    pull: false,
    push: false,
    commit: false
})

// 图形尺寸
const graphWidth = ref(400)
const graphHeight = computed(() => commitHistory.value.length * 80 + 100)

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
    visible.value = true
    refreshGitStatus()
}

const handleClose = () => {
    selectedFile.value = null
    commitForm.value.message = ''
}

const refreshGitStatus = async () => {
    try {
        // 获取当前分支
        gitBranch.value = await invoke('get_current_branch', { path: props.workspacePath })

        // 获取文件状态
        const statuses = await invoke('get_git_status', { path: props.workspacePath })

        // 计算变更数量
        gitChanges.value = {
            added: statuses.filter(s => s.status === 'A').length,
            modified: statuses.filter(s => s.status === 'M').length,
            deleted: statuses.filter(s => s.status === 'D').length
        }

        // 设置未暂存文件
        unstagedFiles.value = statuses.map(file => ({
            ...file,
            diff: '' // 初始为空，点击后加载
        }))

        // 获取暂存文件
        stagedFiles.value = await invoke('get_staged_files', { path: props.workspacePath })

        // 获取提交历史
        const commits = await invoke('get_commit_history', { path: props.workspacePath })
        commitHistory.value = commits.map(commit => ({
            id: commit.id,
            author: commit.author,
            date: commit.time * 1000,
            message: commit.message
        }))
    } catch (error) {
        message.error(`获取Git状态失败: ${error}`)
    }
}

const handleFileClick = async (row) => {
    try {
        selectedFile.value = row
        if (!row.diff) {
            const diff = await invoke('get_file_diff', {
                path: props.workspacePath,
                filePath: row.path
            })
            selectedFile.value.diff = diff
        }
    } catch (error) {
        message.error(`获取文件差异失败: ${error}`)
    }
}

const handleStageFile = async (file) => {
    try {
        await invoke('stage_file', {
            path: props.workspacePath,
            filePath: file.path
        })
        message.success(`已暂存文件: ${file.path}`)
        refreshGitStatus()
    } catch (error) {
        message.error(`暂存文件失败: ${error}`)
    }
}

const handleStageAll = async () => {
    try {
        await invoke('stage_all', { path: props.workspacePath })
        message.success('已暂存所有变更')
        refreshGitStatus()
    } catch (error) {
        message.error(`暂存全部失败: ${error}`)
    }
}

const handleUnstageAll = async () => {
    try {
        await invoke('unstage_all', { path: props.workspacePath })
        message.success('已取消所有暂存')
        refreshGitStatus()
    } catch (error) {
        message.error(`取消暂存失败: ${error}`)
    }
}

const handleDiscardFile = async (file) => {
    try {
        await invoke('discard_file', {
            path: props.workspacePath,
            filePath: file.path
        })
        message.success(`已丢弃文件变更: ${file.path}`)
        refreshGitStatus()
    } catch (error) {
        message.error(`丢弃文件失败: ${error}`)
    }
}

const handleCommit = async () => {
    if (!commitForm.value.message.trim()) {
        message.warning('请输入提交信息')
        return
    }

    gitLoading.value.commit = true
    try {
        await invoke('git_commit', {
            path: props.workspacePath,
            message: commitForm.value.message
        })
        message.success('提交成功')
        commitForm.value.message = ''
        refreshGitStatus()
    } catch (error) {
        message.error(`提交失败: ${error}`)
    } finally {
        gitLoading.value.commit = false
    }
}

const handlePull = async () => {
    gitLoading.value.pull = true
    try {
        await invoke('git_pull', { path: props.workspacePath })
        message.success('拉取成功')
        refreshGitStatus()
    } catch (error) {
        message.error(`拉取失败: ${error}`)
    } finally {
        gitLoading.value.pull = false
    }
}

const handlePush = async () => {
    gitLoading.value.push = true
    try {
        await invoke('git_push', { path: props.workspacePath })
        message.success('推送成功')
    } catch (error) {
        message.error(`推送失败: ${error}`)
    } finally {
        gitLoading.value.push = false
    }
}

const getCommitColor = (commit) => {
    // 简单示例：根据作者不同显示不同颜色
    const colors = ['#4fc08d', '#63e2b7', '#f2c97d', '#e88080', '#8fbcbb']
    const hash = commit.author.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0)
    return colors[hash % colors.length]
}

const formatDate = (timestamp) => {
    return new Date(timestamp).toLocaleString()
}

const getRowClass = (row) => {
    return `file-row file-status-${row.status.toLowerCase()}`
}

onMounted(() => {
    refreshGitStatus()
})

defineExpose({
    open
})
</script>

<style scoped>
.gitkraken-modal {
    border-radius: 8px;
    overflow: hidden;
}

.gitkraken-container {
    display: flex;
    height: 100%;
    background-color: var(--bg-color);
    color: var(--text-color);
}

.git-main {
    display: flex;
    height: 100%;
    overflow: hidden;
}

.git-graph {
    width: 400px;
    border-right: 1px solid var(--divider-color);
    overflow-y: auto;
    padding: 16px;
    background-color: var(--graph-bg);
}

.graph-container {
    width: 100%;
    height: 100%;
}

.branch-graph {
    width: 100%;
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
}

.file-changes {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.diff-viewer {
    border-top: 1px solid var(--divider-color);
    padding: 12px;
}

.diff-content {
    font-family: 'Maple Mono NF CN', monospace;
    white-space: pre-wrap;
    font-size: 14px;
}

.commit-area {
    padding: 12px;
    border-top: 1px solid var(--divider-color);
}

.commit-actions {
    display: flex;
    justify-content: space-between;
    margin-top: 12px;
}

.file-item {
    display: flex;
    align-items: center;
    gap: 8px;
}

.file-icon {
    flex-shrink: 0;
}

.file-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.file-actions {
    display: flex;
    gap: 4px;
}

.commit-message {
    font-size: 14px;
    font-weight: 500;
    fill: var(--text-color);
}

.commit-meta {
    font-size: 12px;
    fill: var(--text-color-secondary);
}

.commit-id {
    font-size: 12px;
    fill: var(--text-color-secondary);
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
</style>

<style>
/* 全局样式 */
:root {
    --bg-color: #1e1e1e;
    --text-color: #e0e0e0;
    --text-color-secondary: #a0a0a0;
    --divider-color: #333;
    --hover-color: rgba(255, 255, 255, 0.08);
    --graph-bg: #252526;
    --added-bg: rgba(46, 125, 50, 0.2);
    --modified-bg: rgba(255, 171, 0, 0.2);
    --deleted-bg: rgba(211, 47, 47, 0.2);
}

.n-card {
    background-color: var(--bg-color);
    border: 1px solid var(--divider-color);
}

.n-tabs .n-tabs-pane-wrapper {
    background-color: var(--bg-color);
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