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
      <div class="sidebar-content" :style="{ display: isSidebarCollapsed ? 'none' : 'flex' }">
        <div class="source-control-header">
          <n-space vertical size="small">
            <n-space align="center" justify="space-between">
              <n-text strong>源代码管理</n-text>
              <n-button text @click="refreshGitStatus">
                <template #icon>
                  <n-icon size="20" class="panel-header-icon">
                    <RefreshOutline />
                  </n-icon>
                </template>
              </n-button>
            </n-space>
            <n-space align="center" justify="space-between" class="scm-header-sub">
              <n-space align="center">
                <n-icon size="18" class="panel-header-icon">
                  <GitBranchOutline />
                </n-icon>
                <n-text>{{ currentBranch || '未检出分支' }}</n-text>
              </n-space>
              <n-text v-if="workspacePathName" depth="3">{{ workspacePathName }}</n-text>
            </n-space>
          </n-space>
        </div>

        <!-- 提交区域 -->
        <div class="commit-section">
          <n-input
            v-model:value="commitForm.message"
            type="textarea"
            placeholder="提交信息（Ctrl+Enter 提交）"
            :autosize="{ minRows: 1, maxRows: 3 }"
            style="min-width: 250px; margin-bottom: 8px;"
            @keydown="handleCommitKeydown"
          />
          <n-button
            type="primary"
            @click="handleCommit"
            :disabled="!commitForm.message.trim() || stagedFiles.length === 0"
            style="width: 100%;"
          >
            提交
          </n-button>
        </div>

        <!-- 变更文件列表容器 -->
        <div class="changes-container">
          <!-- 原有内容保持不变 -->
          <div class="panels-container" ref="panelsContainer">
            <!-- Changes区域 -->
            <div class="changes-section" ref="changesSection" :style="{ flex: `0 0 ${changesHeight}px` }">
              <div class="section-header">
                <div class="section-header-left">
                  <span class="section-title">更改</span>
                  <span class="section-count">{{ unstagedFiles.length }}</span>
                </div>
                <div class="section-header-right">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        @click.stop="handleStageAll"
                        :disabled="unstagedFiles.length === 0"
                      >
                        <Codicon name="add" />
                      </button>
                    </template>
                    暂存全部
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        @click.stop="handleDiscardAll"
                        :disabled="unstagedFiles.length === 0"
                      >
                        <Codicon name="discard" />
                      </button>
                    </template>
                    丢弃全部
                  </n-tooltip>
                </div>
              </div>
              <div class="section-scroll">
                <n-list>
                  <n-list-item v-for="file in unstagedFiles" :key="file.path">
                    <div class="git-file-row">
                      <div class="git-file-left" @click="handleFileClick(file)">
                        <Codicon
                          :name="getFileStatusCodicon(file.status)"
                          class="git-status-icon"
                          :class="getStatusClass(file.status)"
                        />
                        <n-tooltip trigger="hover">
                          <template #trigger>
                            <n-text class="git-file-path">{{ file.path }}</n-text>
                          </template>
                          {{ file.path }}
                        </n-tooltip>
                      </div>
                      <div class="git-file-actions">
                        <n-space>
                          <n-tooltip trigger="hover">
                            <template #trigger>
                              <button
                                class="vscode-action-button"
                                type="button"
                                @click.stop="handleStageFile(file)"
                                :disabled="isGitFileBusy(file.path)"
                              >
                                <Codicon name="add" />
                              </button>
                            </template>
                            暂存
                          </n-tooltip>
                          <n-tooltip trigger="hover">
                            <template #trigger>
                              <button
                                class="vscode-action-button"
                                type="button"
                                @click.stop="handleDiscardFile(file)"
                                :disabled="isGitFileBusy(file.path)"
                              >
                                <Codicon name="discard" />
                              </button>
                            </template>
                            丢弃
                          </n-tooltip>
                        </n-space>
                      </div>
                    </div>
                  </n-list-item>
                </n-list>
              </div>
            </div>

            <!-- Changes和Staged Changes之间的分隔线 -->
            <div class="section-divider" ref="changesDivider" @mousedown="startChangesDrag">
              <div class="divider-handle"></div>
            </div>

            <!-- Staged Changes区域 -->
            <div class="staged-section" ref="stagedSection" :style="{ flex: `0 0 ${stagedHeight}px` }">
              <div class="section-header">
                <div class="section-header-left">
                  <span class="section-title">已暂存的更改</span>
                  <span class="section-count">{{ stagedFiles.length }}</span>
                </div>
                <div class="section-header-right">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        @click.stop="handleUnstageAll"
                        :disabled="stagedFiles.length === 0"
                      >
                          <Codicon name="remove" />
                      </button>
                    </template>
                    取消暂存全部
                  </n-tooltip>
                </div>
              </div>
              <div class="section-scroll">
                <n-list>
                  <n-list-item v-for="file in stagedFiles" :key="file.path">
                    <div class="git-file-row">
                      <div class="git-file-left" @click="handleFileClick(file)">
                        <Codicon
                          :name="getFileStatusCodicon(file.status)"
                          class="git-status-icon"
                          :class="getStatusClass(file.status)"
                        />
                        <n-tooltip trigger="hover">
                          <template #trigger>
                            <n-text class="git-file-path">{{ file.path }}</n-text>
                          </template>
                          {{ file.path }}
                        </n-tooltip>
                      </div>
                      <div class="git-file-actions">
                        <n-space>
                          <n-tooltip trigger="hover">
                            <template #trigger>
                              <button
                                class="vscode-action-button"
                                type="button"
                                @click.stop="handleUnstageFile(file)"
                                :disabled="isGitFileBusy(file.path)"
                              >
                                <Codicon name="remove" />
                              </button>
                            </template>
                            取消暂存
                          </n-tooltip>
                        </n-space>
                      </div>
                    </div>
                  </n-list-item>
                </n-list>
              </div>
            </div>

            <!-- Staged Changes和提交历史之间的分隔线 -->
            <div class="commits-divider" ref="commitsDivider" @mousedown="startCommitsDrag">
              <div class="divider-handle"></div>
            </div>

            <!-- 提交历史区域 -->
            <div class="commits-section" ref="commitsSection" :style="{ flex: `0 0 ${commitsHeight}px` }">
              <div class="section-header">
                <div class="section-header-left">
                  <span class="section-title">提交历史</span>
                  <span class="section-count">{{ commits.length }}</span>
                </div>
                <div class="section-header-right">
                  <n-space>
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-button text @click="handlePull" :loading="gitLoading.pull">
                          <template #icon>
                            <n-icon size="20" class="panel-header-icon">
                              <DownloadOutline />
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
                            <n-icon size="20" class="panel-header-icon">
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
                            <n-icon size="20" class="panel-header-icon">
                              <AddCircleOutline />
                            </n-icon>
                          </template>
                        </n-button>
                      </template>
                      创建分支
                    </n-tooltip>
                    <n-tag type="info" size="small" @click="showBranchPanel = true" style="cursor: pointer">
                      <n-icon size="16" class="panel-header-icon">
                        <GitBranchOutline />
                      </n-icon>
                      {{ currentBranch }}
                    </n-tag>
                  </n-space>
                </div>
              </div>

              <div class="commits-list">
                <n-spin
                  :show="commitHistoryLoading"
                  size="small"
                  description="正在加载提交历史..."
                  class="commit-history-spin"
                >
                  <n-tooltip
                    v-for="commit in visibleCommits"
                    :key="commit.id"
                    trigger="hover"
                    placement="right-start"
                    :delay="120"
                    :duration="80"
                    :raw="true"
                    :show-arrow="false"
                    :content-style="commitTooltipContentStyle"
                  >
                  <template #trigger>
                    <div
                      class="commit-item"
                      :class="{
                        'selected-commit': selectedCommit?.id === commit.id,
                        'expanded-commit': expandedCommit?.id === commit.id
                      }"
                      @click="selectCommit(commit)"
                    >
                      <div class="git-file-row commit-row">
                        <div class="git-file-left">
                          <n-icon :component="GitCommitOutline" size="16" class="commit-icon" />
                          <div class="git-file-path">
                            <n-text>{{ commit.summary }}</n-text>
                          </div>
                        </div>
                        <div class="commit-right">
                          <div class="commit-markers">
                        <n-tooltip v-if="headMarkers.local_head && commit.id === headMarkers.local_head" trigger="hover">
                          <template #trigger>
                            <span class="commit-marker-pill commit-marker-local">
                              <n-icon :component="GitBranchOutline" size="14" class="commit-marker-pill-icon" />
                              <span class="commit-marker-pill-text">{{ currentBranch || 'HEAD' }}</span>
                            </span>
                          </template>
                          {{ currentBranch || 'HEAD' }}
                        </n-tooltip>
                        <n-tooltip v-if="headMarkers.remote_head && commit.id === headMarkers.remote_head" trigger="hover">
                          <template #trigger>
                            <span class="commit-marker-pill commit-marker-remote">
                              <n-icon :component="CloudOutline" size="14" class="commit-marker-pill-icon" />
                              <span class="commit-marker-pill-text">{{ headMarkers.remote_ref || 'origin' }}</span>
                            </span>
                          </template>
                          {{ headMarkers.remote_ref || 'origin' }}
                        </n-tooltip>
                      </div>
                          <n-text depth="3" class="commit-time">{{ formatDateRelative(commit.date) }}</n-text>
                          <n-text depth="3" class="commit-hash">{{ commit.id.slice(0, 7) }}</n-text>
                        </div>
                      </div>
                      <div v-if="expandedCommit?.id === commit.id" class="commit-files">
                    <n-list>
                      <n-list-item
                        v-for="file in commitFiles"
                        :key="file.path"
                        @click.stop="handleCommitFileClick(file)"
                        :class="{ 'selected-file': selectedFile?.path === file.path }"
                      >
                        <div class="git-file-row">
                          <div class="git-file-left">
                            <Codicon
                              :name="getFileStatusCodicon(file.status)"
                              class="git-status-icon"
                              :class="getStatusClass(file.status)"
                            />
                            <n-tooltip trigger="hover">
                              <template #trigger>
                                <n-text class="git-file-path">{{ file.path }}</n-text>
                              </template>
                              {{ file.path }}
                            </n-tooltip>
                          </div>
                          <div class="git-file-actions">
                            <n-tag v-if="file.status !== ' '" size="small" :type="getStatusTagType(file.status)">
                              {{ file.status }}
                            </n-tag>
                          </div>
                        </div>
                      </n-list-item>
                    </n-list>
                      </div>
                    </div>
                  </template>

                  <div class="commit-tooltip vscode-commit-tooltip">
                    <div class="commit-tooltip-header">
                      {{ commit.author }} , {{ formatDateRelative(commit.date) }} ({{ formatDateFull(commit.date) }})
                    </div>
                    <div class="commit-tooltip-message">
                      <div class="commit-tooltip-subject">{{ commit.message.split('\n')[0] }}</div>
                      <pre class="commit-tooltip-body">{{ commit.message.split('\n').slice(1).join('\n') }}</pre>
                    </div>
                    <div class="commit-tooltip-stats">
                      已更改 {{ commit.files_changed }} 个文件, {{ commit.insertions }} 个插入(+), {{ commit.deletions }} 个删除(-)
                    </div>
                    <div class="commit-tooltip-hash">{{ commit.id.slice(0, 7) }}</div>
                  </div>
                  </n-tooltip>
                  <button
                    v-if="visibleCommits.length < commits.length"
                    class="commit-load-more"
                    type="button"
                    @click.stop="showMoreCommits"
                  >
                    显示更多提交 ({{ commits.length - visibleCommits.length }})
                  </button>
                </n-spin>
              </div>
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
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="navigateDiff('prev')" :disabled="!selectedFile || diffLoading.file">
                  <template #icon>
                    <n-icon>
                      <ChevronUpOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              上一个差异
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="navigateDiff('next')" :disabled="!selectedFile || diffLoading.file">
                  <template #icon>
                    <n-icon>
                      <ChevronDownOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              下一个差异
            </n-tooltip>
          </n-space>
        </n-space>
      </div>

      <!-- 文件差异或提交详情 -->
      <div class="content-viewer">
        <div v-if="selectedFile || selectedCommit" class="diff-viewer">
          <div ref="diffEditorContainer" class="diff-editor-container"></div>
          <div
            v-if="selectedFile && !selectedCommit && diffHunkActionVisible"
            class="diff-hunk-action"
            :style="{ top: `${diffHunkActionTop}px`, left: `${diffHunkActionLeft}px`, height: `${diffHunkActionHeight}px` }"
          >
            <n-tooltip trigger="hover">
              <template #trigger>
                <button
                  class="diff-hunk-action-button"
                  type="button"
                  @click="handleStageDiffHunk"
                  :disabled="!canStageCurrentHunk"
                >
                  <Codicon name="add" />
                </button>
              </template>
              暂存当前差异块
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <button
                  class="diff-hunk-action-button"
                  :class="{ danger: !selectedFile?.staged }"
                  type="button"
                  @click="handleDiscardDiffHunk"
                  :disabled="!canRevertCurrentHunk"
                >
                  <Codicon :name="selectedFile?.staged ? 'remove' : 'discard'" />
                </button>
              </template>
              {{ currentHunkRevertTooltip }}
            </n-tooltip>
          </div>
        </div>

        <div v-else class="empty-state">
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
import { ref, h, onMounted, onUnmounted, nextTick, watch, markRaw, computed } from 'vue'
import {
  NIcon, NTag, NButton, NDivider, NTooltip, NForm,
  NFormItem,
  NInput,
  NSpin,
  createDiscreteApi,
  darkTheme,
} from 'naive-ui'
import {
  GitBranchOutline,
  RefreshOutline,
  AddCircleOutline,
  AddOutline,
  CreateOutline,
  RemoveCircleOutline,
  RemoveOutline,
  CloseCircleOutline,
  TrashOutline,
  CheckmarkCircleOutline,
  DocumentOutline,
  GitCommitOutline,
  ChevronUpOutline,
  ChevronDownOutline,
  DownloadOutline,
  CloudUploadOutline,
  CloudOutline
} from '@vicons/ionicons5'
import { ChevronLeft20Filled, ChevronRight20Filled } from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import * as monaco from 'monaco-editor'
import { useSettingsStore } from '../../stores/settingsStore'
import { GUAXI_MONACO_SEMANTIC_TOKEN_COLORS, GUAXI_MONACO_THEME_RULES } from '../../monaco/guaxiMonacoThemes'
import { registerDiffLanguages } from './utils/monaco-diff' // 自定义语言注册
import { createLogger } from '../../utils/logger'

const frontendLogger = createLogger('GitPanel')

const settingsStore = useSettingsStore()

const gitPanelNaiveConfigProviderProps = computed(() => {
  const isDark = settingsStore.appearanceTheme === 'dark'
  const palette = isDark
    ? {
        panel: '#21252b',
        border: '#3e4451',
        text1: '#abb2bf',
        text2: '#9da5b4',
      }
    : {
        panel: '#ffffff',
        border: '#d0d7de',
        text1: '#1f2328',
        text2: '#57606a',
      }

  // 设计说明：GitPanel 使用 createDiscreteApi 创建通知，它不继承 App 顶层的
  // n-config-provider；这里显式传入同一套亮/暗色基础变量，避免暗色模式弹出亮色通知。
  return {
    theme: isDark ? darkTheme : null,
    themeOverrides: {
      common: {
        cardColor: palette.panel,
        popoverColor: palette.panel,
        borderColor: palette.border,
        textColor1: palette.text1,
        textColor2: palette.text2,
      },
    },
  }
})

const { notification } = createDiscreteApi(['notification'], {
  configProviderProps: gitPanelNaiveConfigProviderProps,
})

const Codicon = (props) => h('span', {
  class: ['codicon', `codicon-${props.name}`, 'git-codicon'],
  'aria-hidden': 'true'
})

let guaxiMonacoThemesReady = false
const ensureGuaxiMonacoThemes = () => {
  if (guaxiMonacoThemesReady) return

  monaco.editor.defineTheme('guaxi-light', {
    base: 'vs',
    inherit: true,
    // 设计说明：语法与语义 token 色板统一从共享模块读取，避免 GitPanel 后注册同名主题时覆盖 Notebook/编辑器的 Python 高亮。
    rules: GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
    colors: {
      'editor.background': '#fafafa',
      'editor.foreground': '#383a42',
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#383a42',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#d7dae0',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editor.selectionHighlightBackground': '#ffe0b2',
      'editor.foldBackground': '#f7f9fc',
      'editorWhitespace.foreground': '#d0d0d0',
      'editorIndentGuide.background': '#d0d0d0',
      'editorIndentGuide.activeBackground': '#b0bec5',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#e0e0e0',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#e0e0e0',
      'editorSuggestWidget.foreground': '#383a42',
      'editorSuggestWidget.selectedBackground': '#e3f2fd',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#e0e0e0',
      'editorGutter.background': '#fafafa',
      'scrollbar.shadow': '#00000033'
    }
  })

  monaco.editor.defineTheme('guaxi-dark', {
    base: 'vs-dark',
    inherit: true,
    // 设计说明：暗色 Python 高亮的根因是多个入口重复注册 `guaxi-dark` 且 token 色值不同；这里改为共享色板。
    rules: GUAXI_MONACO_THEME_RULES.dark,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark,
    colors: {
      'editor.background': '#282c34',
      'editor.foreground': '#abb2bf',
      'editorLineNumber.foreground': '#5c6370',
      'editorLineNumber.activeForeground': '#abb2bf',
      'editorCursor.foreground': '#61afef',
      'editor.selectionBackground': '#3e4451',
      'editor.inactiveSelectionBackground': '#323842',
      'editor.lineHighlightBackground': '#2c313a',
      'editor.lineHighlightBorder': '#00000000',
      'editorBracketMatch.background': '#3b4048',
      'editorBracketMatch.border': '#528bff',
      'editor.selectionHighlightBackground': '#3e445155',
      'editor.findMatchBackground': '#3b5f8a66',
      'editor.findMatchHighlightBackground': '#3e4451aa',
      'editor.findRangeHighlightBackground': '#3e445155',
      'editor.foldBackground': '#2c313a',
      'editorWhitespace.foreground': '#3b4048',
      'editorIndentGuide.background': '#3b4048',
      'editorIndentGuide.activeBackground': '#4b5263',
      'editorHoverWidget.background': '#282c34',
      'editorHoverWidget.border': '#3b4048',
      'editorSuggestWidget.background': '#282c34',
      'editorSuggestWidget.border': '#3b4048',
      'editorSuggestWidget.foreground': '#abb2bf',
      'editorSuggestWidget.selectedBackground': '#2c313a',
      'editorWidget.background': '#282c34',
      'editorWidget.border': '#3b4048',
      'input.background': '#21252b',
      'input.foreground': '#abb2bf',
      'input.border': '#3b4048',
      'input.placeholderForeground': '#7f8898',
      'focusBorder': '#528bff80',
      'editorGutter.background': '#282c34',
      'scrollbar.shadow': '#00000066'
    }
  })

  guaxiMonacoThemesReady = true
  frontendLogger.info('[DiffTheme] Monaco themes registered with GitPanel dark overrides')
}

const getMonacoTheme = () => {
  ensureGuaxiMonacoThemes()
  return settingsStore.appearanceTheme === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const commitTooltipContentStyle = computed(() => {
  const isDark = settingsStore.appearanceTheme === 'dark'
  return isDark
    ? {
        backgroundColor: '#1e1e1e',
        color: '#d4d4d4',
        border: '1px solid rgba(255, 255, 255, 0.12)',
        boxShadow: '0 8px 24px rgba(0, 0, 0, 0.35)',
        padding: '10px 12px',
        borderRadius: '8px',
        '--git-tooltip-text': '#d4d4d4',
        '--git-tooltip-text-secondary': '#9da7b3'
      }
    : {
        backgroundColor: '#ffffff',
        color: '#1f2328',
        border: 'none',
        boxShadow: '0 8px 24px rgba(140, 149, 159, 0.25)',
        padding: '10px 12px',
        borderRadius: '8px',
        '--git-tooltip-text': '#1f2328',
        '--git-tooltip-text-secondary': '#57606a'
      }
})

watch(
  () => settingsStore.appearanceTheme,
  () => {
    try {
      ensureGuaxiMonacoThemes()
      monaco.editor.setTheme(getMonacoTheme())
    } catch (e) {
    }
  },
  { immediate: true }
)

const logger = createLogger('GitPanel')

const props = defineProps({
  workspacePath: {
    type: String,
    required: true
  }
})

const workspacePathName = computed(() => {
  const s = String(props.workspacePath || '')
  if (!s) return ''
  const parts = s.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || s
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
const commitHistoryLoading = ref(false)
const commitHistoryVisibleLimit = ref(60)
const headMarkers = ref({
  local_head: null,
  remote_head: null,
  remote_ref: null
})
const unstagedFiles = ref([])
const stagedFiles = ref([])
const selectedFile = ref(null)
const gitFileOperationKeys = ref(new Set())
const gitLoading = ref({
  pull: false,
  push: false,
  commit: false,
  checkout: false  // 添加分支切换加载状态
});

const gitAutoRefresh = ref({
  enabled: true,
  intervalMs: 5000
})

const isRefreshingStatus = ref(false)
const fileChangeUnlisten = ref(null)
let autoRefreshIntervalId = null
let refreshDebounceTimerId = null

const normalizeFsPath = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$/g, '')
const isPathUnderRepo = (p) => {
  const root = normalizeFsPath(props.workspacePath)
  const s = normalizeFsPath(p)
  if (!root || !s) return false
  if (s === root) return true
  return s.startsWith(root + '/')
}

const scheduleAutoRefresh = () => {
  if (!gitAutoRefresh.value.enabled) return
  if (refreshDebounceTimerId) clearTimeout(refreshDebounceTimerId)
  refreshDebounceTimerId = setTimeout(() => {
    refreshGitStatus({ resetSelection: false, includeCommits: false })
  }, 250)
}

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
let diffHunkAnimationFrameId = null
let diffLineChangesCache = []
let diffLineChangesCacheContext = null
let diffGitHunksCache = []
let diffGitHunksCacheContext = null
let diffActiveContext = null
let diffFileDiffRequestId = 0
let suppressSelectedFileDiffWatch = false
let diffEditorHostElement = null
let diffUpdateDisposable = null

const diffLoading = ref({
  file: false,
  commit: false,
  navigation: false,
  hunk: false
});

const diffHunkActionTop = ref(8)
const diffHunkActionLeft = ref(8)
const diffHunkActionHeight = ref(52)
const diffHunkActionVisible = ref(false)
let diffHunkScrollDisposable = null
let diffHunkOriginalScrollDisposable = null
let diffHunkCursorDisposable = null
const diffRenderSideBySide = ref(true)

const visibleCommits = computed(() => commits.value.slice(0, commitHistoryVisibleLimit.value))
const showMoreCommits = () => {
  commitHistoryVisibleLimit.value += 60
  frontendLogger.info('[GitPanel] Expanded visible commit history', {
    visible: Math.min(commitHistoryVisibleLimit.value, commits.value.length),
    total: commits.value.length
  })
}

const isGitFileBusy = (filePath) => gitFileOperationKeys.value.has(String(filePath || ''))
const setGitFileBusy = (filePath, busy) => {
  const key = String(filePath || '')
  if (!key) return
  const next = new Set(gitFileOperationKeys.value)
  if (busy) next.add(key)
  else next.delete(key)
  gitFileOperationKeys.value = next
}

const getDiffPerformanceOptions = (originalContent = '', modifiedContent = '') => {
  const originalLines = String(originalContent || '').split(/\r\n|\r|\n/).length
  const modifiedLines = String(modifiedContent || '').split(/\r\n|\r|\n/).length
  const totalLines = originalLines + modifiedLines
  const largeDiff = totalLines > 8000 || String(originalContent).length + String(modifiedContent).length > 900_000
  return {
    largeDiff,
    options: {
      renderSideBySide: !largeDiff,
      renderIndicators: !largeDiff,
      renderWhitespace: largeDiff ? 'none' : 'selection',
      renderOverviewRuler: !largeDiff,
      occurrencesHighlight: largeDiff ? 'off' : 'singleFile',
      selectionHighlight: !largeDiff,
      readOnly: true,
      automaticLayout: false
    }
  }
}

const normalizeDiffFilePath = (filePath) => String(filePath || '').replace(/\\/g, '/')

const createDiffContext = (filePath, staged) => {
  const normalizedPath = normalizeDiffFilePath(filePath)
  return {
    filePath: normalizedPath,
    staged: !!staged,
    key: `${normalizedPath}::${staged ? 'staged' : 'unstaged'}`
  }
}

const getSelectedDiffContext = () => {
  if (!selectedFile.value) return null
  return createDiffContext(selectedFile.value.path, selectedFile.value.staged)
}

const isSameDiffContext = (left, right) => !!left && !!right && left.key === right.key

const isCurrentDiffContext = (context, requestId = diffFileDiffRequestId) => {
  if (!isSameDiffContext(diffActiveContext, context)) return false
  if (!isSameDiffContext(getSelectedDiffContext(), context)) return false
  return requestId === diffFileDiffRequestId
}

const clearDiffHunkCaches = (reason, context = diffActiveContext) => {
  diffLineChangesCache = []
  diffLineChangesCacheContext = null
  diffGitHunksCache = []
  diffGitHunksCacheContext = null
  frontendLogger.info('[Diff] Cleared hunk caches', {
    reason,
    context: context?.key || null
  })
}

const isDiffEditorAttachedToCurrentContainer = () => {
  if (!diffEditor || !diffEditorContainer.value || !diffEditorHostElement) return false
  return diffEditorContainer.value.contains(diffEditorHostElement)
}

const disposeDiffEditorInstance = (reason = 'dispose-diff-editor') => {
  frontendLogger.info('[Diff] Disposing diff editor instance', {
    reason,
    hasEditor: !!diffEditor,
    hasContainer: !!diffEditorContainer.value,
    hostAttached: isDiffEditorAttachedToCurrentContainer()
  })

  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (diffHunkScrollDisposable) {
    diffHunkScrollDisposable.dispose()
    diffHunkScrollDisposable = null
  }
  if (diffHunkOriginalScrollDisposable) {
    diffHunkOriginalScrollDisposable.dispose()
    diffHunkOriginalScrollDisposable = null
  }
  if (diffHunkCursorDisposable) {
    diffHunkCursorDisposable.dispose()
    diffHunkCursorDisposable = null
  }
  if (diffUpdateDisposable) {
    diffUpdateDisposable.dispose()
    diffUpdateDisposable = null
  }

  if (diffEditor) {
    const model = diffEditor.getModel?.()
    diffEditor.setModel?.(null)
    diffEditor.dispose?.()
    model?.original?.dispose?.()
    model?.modified?.dispose?.()
  }

  diffEditor = null
  diffEditorHostElement = null
  isEditorInitialized.value = false
  diffHunkActionVisible.value = false
  diffActiveContext = null
  clearDiffHunkCaches(reason, null)
}

const selectFileAndUpdateDiff = async (file, reason = 'select-file') => {
  suppressSelectedFileDiffWatch = true
  selectedFile.value = file
  selectedCommit.value = null
  try {
    await nextTick()
  } finally {
    suppressSelectedFileDiffWatch = false
  }
  frontendLogger.info('[Diff] Selecting file for explicit diff refresh', {
    reason,
    context: createDiffContext(file?.path, file?.staged).key
  })
  await updateFileDiff(file)
}

const canRevertCurrentHunk = computed(() => {
  if (!selectedFile.value) return false
  if (selectedCommit.value) return false
  if (diffLoading.value.file || diffLoading.value.commit || diffLoading.value.hunk) return false
  if (!isSameDiffContext(diffActiveContext, getSelectedDiffContext())) return false
  const status = normalizeGitStatus(selectedFile.value.status)
  if (!selectedFile.value.staged && status === 'D') return false
  if (!diffHunkActionVisible.value) return false
  return !!diffEditor
})

const canStageCurrentHunk = computed(() => {
  if (!selectedFile.value) return false
  if (selectedCommit.value) return false
  if (diffLoading.value.file || diffLoading.value.commit || diffLoading.value.hunk) return false
  if (!isSameDiffContext(diffActiveContext, getSelectedDiffContext())) return false
  if (selectedFile.value.staged) return false
  if (!diffHunkActionVisible.value) return false
  return !!diffEditor
})

const currentHunkRevertTooltip = computed(() => (
  selectedFile.value?.staged ? '取消暂存当前差异块' : '撤销当前差异块'
))

// 添加侧边栏折叠状态
const isSidebarCollapsed = ref(false)

// 添加切换函数
const toggleSidebar = () => {
  isSidebarCollapsed.value = !isSidebarCollapsed.value
}

// 新增辅助函数 - 获取状态标签类型
const normalizeGitStatus = (status) => {
  const value = String(status || '').trim()
  if (!value) return ''
  if (value.includes('?')) return '?'
  if (value.includes('A')) return 'A'
  if (value.includes('M')) return 'M'
  if (value.includes('D')) return 'D'
  if (value.includes('R')) return 'R'
  if (value.includes('T')) return 'T'
  return value[0] || ''
}

const getStatusTagType = (status) => {
  switch (normalizeGitStatus(status)) {
    case 'A': return 'success'
    case 'M': return 'warning'
    case 'D': return 'error'
    case 'R': return 'info'
    case '?': return 'info'
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
    refreshDiffLineChangesCache()

    // 清理旧模型（如果有）
    if (oldModel) {
      setTimeout(() => {
        oldModel.original?.dispose();
        oldModel.modified?.dispose();
      }, 100);
    }

    // 大文件/大差异时切到性能模式，减少 Monaco diff editor 的横向分栏和装饰计算。
    const diffPerf = getDiffPerformanceOptions(originalContent, modifiedContent)
    diffRenderSideBySide.value = !!diffPerf.options.renderSideBySide
    diffEditor.updateOptions(diffPerf.options);
    if (diffPerf.largeDiff) {
      frontendLogger.info('[Diff] Large commit file diff rendered in performance mode', { filePath: file.path })
    }

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
        updateHunkActionPosition()
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

    const changes = diffEditor.getLineChanges?.() || []
    if (changes.length > 0) {
      const currentLine = editor.getPosition()?.lineNumber || 1
      const lineOf = (change) => Math.max(1, change.modifiedStartLineNumber || change.modifiedEndLineNumber || change.originalStartLineNumber || change.originalEndLineNumber || 1)
      const endLineOf = (change) => Math.max(lineOf(change), change.modifiedEndLineNumber || change.modifiedStartLineNumber || change.originalEndLineNumber || change.originalStartLineNumber || lineOf(change))
      const currentIndex = changes.findIndex((change) => currentLine >= lineOf(change) && currentLine <= endLineOf(change))
      let target = null

      if (currentIndex >= 0) {
        const targetIndex = direction === 'next'
          ? (currentIndex + 1) % changes.length
          : (currentIndex - 1 + changes.length) % changes.length
        target = changes[targetIndex]
      } else if (direction === 'next') {
        target = changes.find((change) => lineOf(change) > currentLine) || changes[0]
      } else {
        for (let i = changes.length - 1; i >= 0; i -= 1) {
          if (lineOf(changes[i]) < currentLine) {
            target = changes[i]
            break
          }
        }
        target = target || changes[changes.length - 1]
      }

      const targetPosition = {
        lineNumber: lineOf(target),
        column: 1
      }
      editor.setPosition(targetPosition)
      editor.revealPositionInCenter(targetPosition)
      editor.focus()
      refreshDiffLineChangesCache()
      updateHunkActionPosition()
      logger.debug('Navigated using line changes API', {
        direction,
        currentLine,
        targetLine: targetPosition.lineNumber,
        changes: changes.length
      })
      return
    }

    // Monaco 版本差异较大，有些构建没有 diffReview action；有 changes 时优先使用上面的稳定实现。
    const actionName = direction === 'prev'
      ? 'editor.action.diffReview.prev'
      : 'editor.action.diffReview.next'
    const action = editor.getAction(actionName)
    if (!action) throw new Error(`No navigation action or changes available`)

    logger.debug(`Executing ${actionName}`)
    await action.run()

    const position = editor.getPosition()
    if (position) {
      editor.revealPositionInCenter(position)
      editor.focus()
      updateHunkActionPosition()
      logger.debug('Navigation completed successfully')
    }

    updateHunkActionPosition()
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

const getCurrentDiffChange = () => {
  if (!diffEditor) return null
  const context = getSelectedDiffContext()
  if (!isSameDiffContext(diffActiveContext, context)) return null

  const gitHunks = isSameDiffContext(diffGitHunksCacheContext, context) ? diffGitHunksCache : []
  if (gitHunks.length) {
    const editor = diffEditor.getModifiedEditor()
    const position = editor?.getPosition()
    const line = position?.lineNumber || 1
    const exact = gitHunks.find((hunk) => {
      const start = Math.max(1, hunk.modifiedStartLineNumber || hunk.modifiedEndLineNumber || 1)
      const end = Math.max(start, hunk.modifiedEndLineNumber || hunk.modifiedStartLineNumber || start)
      return line >= start && line <= end
    })
    if (exact) return exact

    const visibleRange = editor?.getVisibleRanges?.()?.[0]
    if (visibleRange) {
      // 设计说明：默认打开 diff 时光标可能还停在第一行；这时只在光标没有命中
      // hunk 后，才用视口内第一个 hunk 兜底，避免滚动时胶囊跳到错误块。
      const visible = gitHunks.find((hunk) => {
        const start = Math.max(1, hunk.modifiedStartLineNumber || hunk.modifiedEndLineNumber || 1)
        const end = Math.max(start, hunk.modifiedEndLineNumber || hunk.modifiedStartLineNumber || start)
        return start <= visibleRange.endLineNumber && end >= visibleRange.startLineNumber
      })
      if (visible) return visible
    }

    let nearest = gitHunks[0]
    let best = Number.POSITIVE_INFINITY
    for (const hunk of gitHunks) {
      const start = Math.max(1, hunk.modifiedStartLineNumber || hunk.modifiedEndLineNumber || 1)
      const end = Math.max(start, hunk.modifiedEndLineNumber || hunk.modifiedStartLineNumber || start)
      const dist = line < start ? start - line : line > end ? line - end : 0
      if (dist < best) {
        best = dist
        nearest = hunk
      }
    }
    return nearest
  }

  if (context?.staged) {
    frontendLogger.warn('[Diff] Refusing staged hunk action without git hunk cache', {
      context: context?.key || null,
      cacheContext: diffGitHunksCacheContext?.key || null,
      active: diffActiveContext?.key || null
    })
    return null
  }

  let changes = isSameDiffContext(diffLineChangesCacheContext, context) && diffLineChangesCache.length
    ? diffLineChangesCache
    : []
  if (!changes.length) {
    try {
      changes = diffEditor.getLineChanges() || []
      diffLineChangesCache = changes
      diffLineChangesCacheContext = context
      if (changes.length) {
        frontendLogger.info('[Diff] Recovered line changes from Monaco on demand', {
          changes: changes.length,
          context: context?.key || null
        })
      }
    } catch (error) {
      frontendLogger.warn('[Diff] Failed to recover line changes on demand', error)
    }
  }
  if (!changes.length) return null
  const editor = diffEditor.getModifiedEditor()
  const position = editor?.getPosition()
  if (!position) return changes[0]

  const line = position.lineNumber || 1
  const exact = changes.find((c) => {
    const start = Math.max(1, c.modifiedStartLineNumber || 1)
    const end = Math.max(start, c.modifiedEndLineNumber || start)
    return line >= start && line <= end
  })
  if (exact) return exact

  let nearest = changes[0]
  let best = Number.POSITIVE_INFINITY
  for (const c of changes) {
    const start = Math.max(1, c.modifiedStartLineNumber || 1)
    const end = Math.max(start, c.modifiedEndLineNumber || start)
    const dist = line < start ? start - line : line > end ? line - end : 0
    if (dist < best) {
      best = dist
      nearest = c
    }
  }
  return nearest
}

const refreshDiffLineChangesCache = () => {
  if (!diffEditor) {
    diffLineChangesCache = []
    diffLineChangesCacheContext = null
    return
  }
  const context = getSelectedDiffContext()
  if (!isSameDiffContext(diffActiveContext, context)) {
    diffLineChangesCache = []
    diffLineChangesCacheContext = null
    frontendLogger.warn('[Diff] Skipped Monaco line change cache refresh for inactive context', {
      active: diffActiveContext?.key || null,
      selected: context?.key || null
    })
    return
  }
  try {
    diffLineChangesCache = diffEditor.getLineChanges() || []
    diffLineChangesCacheContext = context
    frontendLogger.info('[Diff] Cached line changes for hunk toolbar', {
      changes: diffLineChangesCache.length,
      context: context?.key || null
    })
    scheduleHunkActionPositionUpdate()
  } catch (error) {
    diffLineChangesCache = []
    diffLineChangesCacheContext = null
    frontendLogger.warn('[Diff] Unable to cache line changes', error)
  }
}

const toMonacoLikeGitHunk = (hunk) => {
  const oldStart = Math.max(1, Number(hunk?.old_start || hunk?.oldStart || 1))
  const oldCount = Math.max(0, Number(hunk?.old_count || hunk?.oldCount || 0))
  const newStart = Math.max(1, Number(hunk?.new_start || hunk?.newStart || 1))
  const newCount = Math.max(0, Number(hunk?.new_count || hunk?.newCount || 0))
  return {
    hunkIndex: Number(hunk?.index ?? 0),
    gitHeader: String(hunk?.header || ''),
    originalStartLineNumber: oldStart,
    originalEndLineNumber: oldCount > 0 ? oldStart + oldCount - 1 : oldStart,
    modifiedStartLineNumber: newStart,
    modifiedEndLineNumber: newCount > 0 ? newStart + newCount - 1 : newStart,
    originalLineCount: oldCount,
    modifiedLineCount: newCount
  }
}

const refreshGitHunksCache = async (context, requestId = diffFileDiffRequestId) => {
  const filePath = context?.filePath || ''
  const staged = !!context?.staged
  try {
    const hunks = await invoke('get_file_diff_hunks', {
      path: props.workspacePath,
      filePath,
      staged
    })
    if (!isCurrentDiffContext(context, requestId)) {
      frontendLogger.warn('[Diff] Ignored stale git hunk cache result', {
        context: context?.key || null,
        active: diffActiveContext?.key || null,
        selected: getSelectedDiffContext()?.key || null,
        requestId,
        currentRequestId: diffFileDiffRequestId
      })
      return false
    }
    diffGitHunksCache = Array.isArray(hunks) ? hunks.map(toMonacoLikeGitHunk) : []
    diffGitHunksCacheContext = context
    frontendLogger.info(`[Diff] Git hunk cache loaded ${JSON.stringify({
      filePath,
      staged,
      context: context?.key || null,
      hunks: diffGitHunksCache.map((h) => ({
        index: h.hunkIndex,
        header: h.gitHeader,
        original: [h.originalStartLineNumber, h.originalEndLineNumber, h.originalLineCount],
        modified: [h.modifiedStartLineNumber, h.modifiedEndLineNumber, h.modifiedLineCount]
      }))
    })}`)
    scheduleHunkActionPositionUpdateAfterLayout('git-hunk-cache-loaded')
    return true
  } catch (error) {
    if (isCurrentDiffContext(context, requestId)) {
      diffGitHunksCache = []
      diffGitHunksCacheContext = null
    }
    frontendLogger.warn('[Diff] Failed to load git hunk cache; falling back to Monaco changes', {
      context: context?.key || null,
      error
    })
    return false
  }
}

const scheduleHunkActionPositionUpdate = () => {
  if (diffHunkAnimationFrameId) return
  diffHunkAnimationFrameId = window.requestAnimationFrame(() => {
    diffHunkAnimationFrameId = null
    updateHunkActionPosition()
  })
}

const scheduleHunkActionPositionUpdateAfterLayout = (reason) => {
  scheduleHunkActionPositionUpdate()
  ;[60, 180].forEach((delay) => {
    window.setTimeout(() => {
      frontendLogger.info('[Diff] Retrying hunk action position after layout', { reason, delay })
      scheduleHunkActionPositionUpdate()
    }, delay)
  })
}

const getDiffEditorLineHeight = (editor) => {
  try {
    return editor?.getOption?.(monaco.editor.EditorOption.lineHeight) || 20
  } catch {
    return 20
  }
}

const getDiffRangeViewportRect = (editor, startLine, endLine) => {
  if (!editor) return null
  const model = editor.getModel?.()
  const lineCount = Math.max(1, model?.getLineCount?.() || 1)
  const rawStart = Number.isFinite(startLine) && startLine > 0 ? startLine : endLine
  const rawEnd = Number.isFinite(endLine) && endLine > 0 ? endLine : rawStart
  if (!Number.isFinite(rawStart) || rawStart <= 0) return null

  const start = Math.min(Math.max(1, rawStart), lineCount)
  const end = Math.min(Math.max(start, rawEnd || start), lineCount)
  const scrollTop = editor.getScrollTop()
  const lineHeight = getDiffEditorLineHeight(editor)
  return {
    top: editor.getTopForLineNumber(start) - scrollTop,
    bottom: editor.getTopForLineNumber(end) - scrollTop + lineHeight
  }
}

const getDiffChangeViewportRect = (change) => {
  const originalEditor = diffEditor?.getOriginalEditor?.()
  const modifiedEditor = diffEditor?.getModifiedEditor?.()
  const context = getSelectedDiffContext()
  const modifiedRect = getDiffRangeViewportRect(
    modifiedEditor,
    change?.modifiedStartLineNumber,
    change?.modifiedEndLineNumber
  )
  const originalRect = getDiffRangeViewportRect(
    originalEditor,
    change?.originalStartLineNumber,
    change?.originalEndLineNumber
  )

  // 设计说明：未暂存视图以工作区(右侧)为操作对象；已暂存视图是 HEAD -> INDEX，
  // 取消暂存仍然要跟随索引侧(右侧)的 git hunk。只有纯删除块右侧没有真实变更行时，
  // 才回退到左侧，避免胶囊贴到另一侧大块或对齐填充区。
  if (modifiedRect && (change?.modifiedLineCount > 0 || !originalRect)) return modifiedRect
  if (context?.staged && modifiedRect) return modifiedRect
  return originalRect || modifiedRect || null
}

const getDiffHunkActionLeft = (editor, layout) => {
  const fallbackLeft = diffRenderSideBySide.value
    ? Math.max(8, Math.floor((diffEditorContainer.value?.clientWidth || layout.width || 0) / 2) + 8)
    : 8
  const containerRect = diffEditorContainer.value?.getBoundingClientRect?.()
  const editorRect = editor?.getDomNode?.()?.getBoundingClientRect?.()
  if (!containerRect || !editorRect || !layout) return fallbackLeft

  const editorLeft = editorRect.left - containerRect.left
  const lineNumberLeft = editorLeft + (layout.lineNumbersLeft || 0)
  const glyphMarginLeft = editorLeft + (layout.glyphMarginLeft || 0)
  const actionWidth = 24

  if (diffRenderSideBySide.value) {
    // 设计说明：优先放在右侧编辑器行号左边的 gutter 空隙内，既不压右侧行号，
    // 也不占左侧 editor 的滚动条/overview ruler。空间不足时贴着右侧编辑器左边界。
    const gutterLeft = Math.min(glyphMarginLeft, lineNumberLeft)
    return Math.max(editorLeft + 4, gutterLeft - actionWidth - 2)
  }

  return Math.max(8, lineNumberLeft - actionWidth - 2)
}

const updateHunkActionPosition = () => {
  if (!diffEditor || !selectedFile.value || selectedCommit.value) {
    diffHunkActionVisible.value = false
    return
  }

  const change = getCurrentDiffChange()
  if (!change) {
    diffHunkActionVisible.value = false
    return
  }

  const editor = diffEditor.getModifiedEditor()
  const layout = editor.getLayoutInfo()
  const hunkRect = getDiffChangeViewportRect(change)
  if (!hunkRect) {
    diffHunkActionVisible.value = false
    return
  }

  const viewportPadding = 8
  const viewportTop = viewportPadding
  const viewportBottom = Math.max(viewportPadding, layout.height - viewportPadding)
  const hunkTop = Math.min(hunkRect.top, hunkRect.bottom)
  const hunkBottom = Math.max(hunkRect.top, hunkRect.bottom)
  const hunkHeight = Math.max(1, hunkBottom - hunkTop)
  const visibleTop = Math.max(hunkTop, viewportTop)
  const visibleBottom = Math.min(hunkBottom, viewportBottom)
  const hasVisibleOverlap = visibleBottom > visibleTop
  const normalActionHeight = 52

  // 设计说明：条形区域高度跟随当前 hunk 的真实可见高度；
  // 当 hunk 高度超过一屏时，只取当前屏幕内可见的那一段，按钮组保持在可见条形区中央。
  // 单行/短 hunk 低于胶囊正常高度时，允许胶囊略高于变更区，避免按钮被挤压。
  const visibleHeight = hasVisibleOverlap
    ? Math.max(1, visibleBottom - visibleTop)
    : Math.min(hunkHeight, Math.max(1, viewportBottom - Math.min(Math.max(hunkTop, viewportTop), viewportBottom)))
  const actionHeight = Math.min(
    Math.max(visibleHeight, normalActionHeight),
    Math.max(normalActionHeight, viewportBottom - viewportTop)
  )
  const anchorCenter = hasVisibleOverlap
    ? (visibleTop + visibleBottom) / 2
    : Math.min(Math.max((hunkTop + hunkBottom) / 2, viewportTop), viewportBottom)
  const actionTop = Math.min(
    Math.max(anchorCenter - actionHeight / 2, viewportTop),
    Math.max(viewportTop, viewportBottom - actionHeight)
  )

  diffHunkActionTop.value = actionTop
  diffHunkActionHeight.value = actionHeight
  diffHunkActionLeft.value = getDiffHunkActionLeft(editor, layout)
  diffHunkActionVisible.value = true
}

const getChangeLineForRevert = (change) => {
  const cursorLine = diffEditor?.getModifiedEditor?.()?.getPosition?.()?.lineNumber
  const modifiedStart = Math.max(1, change?.modifiedStartLineNumber || change?.modifiedEndLineNumber || 1)
  const modifiedEnd = Math.max(modifiedStart, change?.modifiedEndLineNumber || change?.modifiedStartLineNumber || modifiedStart)
  if (Number.isFinite(cursorLine) && cursorLine >= modifiedStart && cursorLine <= modifiedEnd) {
    return cursorLine
  }

  const candidates = [
    change?.modifiedStartLineNumber,
    change?.modifiedEndLineNumber,
    change?.originalStartLineNumber,
    change?.originalEndLineNumber
  ].filter((n) => Number.isFinite(n) && n > 0)
  return candidates.length ? candidates[0] : 1
}

const getDiffHunkInvokeArgs = (change) => {
  const context = getSelectedDiffContext()
  const gitHunks = isSameDiffContext(diffGitHunksCacheContext, context) ? diffGitHunksCache : []
  const lineChanges = isSameDiffContext(diffLineChangesCacheContext, context) ? diffLineChangesCache : []
  const changes = gitHunks.length
    ? gitHunks
    : (lineChanges.length ? lineChanges : (diffEditor?.getLineChanges?.() || []))
  const source = gitHunks.length ? 'git-hunks' : 'monaco-lineChanges'
  const hunkIndex = Number.isFinite(change?.hunkIndex)
    ? change.hunkIndex
    : changes.findIndex((item) => item === change)
  const args = {
    line: getChangeLineForRevert(change),
    hunkIndex: hunkIndex >= 0 ? hunkIndex : null,
    originalStartLine: Number.isFinite(change?.originalStartLineNumber) ? change.originalStartLineNumber : null,
    originalEndLine: Number.isFinite(change?.originalEndLineNumber) ? change.originalEndLineNumber : null,
    modifiedStartLine: Number.isFinite(change?.modifiedStartLineNumber) ? change.modifiedStartLineNumber : null,
    modifiedEndLine: Number.isFinite(change?.modifiedEndLineNumber) ? change.modifiedEndLineNumber : null,
  }
  const precise = source === 'git-hunks' && Number.isFinite(args.hunkIndex)
  frontendLogger.info(`[Diff] Prepared hunk invoke args ${JSON.stringify({
    hunkIndex: args.hunkIndex,
    line: args.line,
    source,
    precise,
    totalHunks: changes.length,
    context: context?.key || null,
    gitHeader: change?.gitHeader || null,
    originalRange: [args.originalStartLine, args.originalEndLine],
    modifiedRange: [args.modifiedStartLine, args.modifiedEndLine]
  })}`)
  return {
    ...args,
    source,
    precise
  }
}

const handleDiscardDiffHunk = async () => {
  if (!selectedFile.value || selectedCommit.value || diffLoading.value.hunk) return

  const status = normalizeGitStatus(selectedFile.value.status)
  const targetFilePath = selectedFile.value.path
  const isStagedDiff = !!selectedFile.value.staged
  const targetContext = createDiffContext(targetFilePath, isStagedDiff)
  if (!isSameDiffContext(diffActiveContext, targetContext)) {
    frontendLogger.warn('[Diff] Blocked hunk revert/unstage because diff context is stale', {
      target: targetContext.key,
      active: diffActiveContext?.key || null,
      selected: getSelectedDiffContext()?.key || null
    })
    notification.warning({
      title: isStagedDiff ? '无法取消暂存差异块' : '无法撤销差异块',
      content: '当前差异视图仍在刷新，请稍后重试',
      duration: 2000
    })
    return
  }
  if (!isStagedDiff && status === 'D') {
    notification.warning({
      title: '无法撤销差异块',
      content: '已删除文件不支持差异块撤销',
      duration: 2000
    })
    return
  }

  const change = getCurrentDiffChange()
  if (!change) {
    notification.warning({
      title: '无法撤销差异块',
      content: '未找到可撤销的差异块',
      duration: 2000
    })
    return
  }

  const hunkArgs = getDiffHunkInvokeArgs(change)
  if (isStagedDiff && !hunkArgs.precise) {
    frontendLogger.warn('[Diff] Blocked staged hunk unstage because precise git hunk args are missing', {
      filePath: targetFilePath,
      ...hunkArgs
    })
    notification.warning({
      title: '无法取消暂存差异块',
      content: '当前块范围还未精确加载，请重新点击该块后再试',
      duration: 2200
    })
    return
  }
  const invokeHunkArgs = { ...hunkArgs }
  delete invokeHunkArgs.source
  delete invokeHunkArgs.precise

  diffLoading.value.hunk = true
  frontendLogger.info(isStagedDiff ? '[Diff] Unstaging hunk' : '[Diff] Reverting hunk', {
    filePath: targetFilePath,
    staged: isStagedDiff,
    ...hunkArgs,
    originalStart: change.originalStartLineNumber,
    originalEnd: change.originalEndLineNumber,
    modifiedStart: change.modifiedStartLineNumber,
    modifiedEnd: change.modifiedEndLineNumber
  })

  try {
    const command = isStagedDiff ? 'unstage_hunk' : 'discard_hunk'
    const payload = {
      path: props.workspacePath,
      filePath: targetFilePath,
      ...invokeHunkArgs
    }
    if (!isStagedDiff) {
      payload.reverse = true
    }
    await invoke(command, payload)
    frontendLogger.info(isStagedDiff ? '[Diff] Hunk unstaged' : '[Diff] Hunk reverted', {
      filePath: targetFilePath,
      line: invokeHunkArgs.line,
      staged: isStagedDiff
    })
    await refreshGitStatus({ resetSelection: false, includeCommits: false })
    const samePanelFile = (isStagedDiff ? stagedFiles.value : unstagedFiles.value)
      .find((item) => item?.path === targetFilePath)
    const fallbackUnstagedFile = isStagedDiff
      ? unstagedFiles.value.find((item) => item?.path === targetFilePath)
      : null
    const fileForDiff = samePanelFile || fallbackUnstagedFile
    // 设计说明：块级操作后只保持当前面板上下文。比如在“更改”里暂存/撤销
    // 当前 hunk 后，它不应该跳去“已暂存”同名文件继续展示刚处理掉的块。
    // 但已暂存面板最后一个 hunk 被取消暂存后，同名文件只剩“更改”条目；
    // 这时直接切到未暂存 diff，避免 v-if 卸载容器后留下脱离 DOM 的 Monaco 实例。
    if (fileForDiff) {
      await selectFileAndUpdateDiff(
        fileForDiff,
        samePanelFile
          ? (isStagedDiff ? 'unstage-hunk-refresh' : 'discard-hunk-refresh')
          : 'unstage-last-hunk-to-unstaged-refresh'
      )
    } else {
      selectedFile.value = null
      diffHunkActionVisible.value = false
      disposeDiffEditorInstance('hunk-action-no-file-left')
    }
    notification.success({
      title: isStagedDiff ? '已取消暂存差异块' : '已撤销差异块',
      content: targetFilePath,
      duration: 2000
    })
  } catch (error) {
    frontendLogger.error(isStagedDiff ? '[Diff] Failed to unstage hunk' : '[Diff] Failed to revert hunk', error)
    notification.error({
      title: isStagedDiff ? '取消暂存差异块失败' : '撤销差异块失败',
      content: String(error?.message || error),
      duration: 3000
    })
  } finally {
    diffLoading.value.hunk = false
    scheduleHunkActionPositionUpdate()
  }
}

const handleStageDiffHunk = async () => {
  if (!selectedFile.value || selectedCommit.value || diffLoading.value.hunk) return
  if (selectedFile.value.staged) {
    frontendLogger.warn('[Diff] Blocked hunk stage from staged diff view', {
      filePath: selectedFile.value.path
    })
    return
  }

  const targetFilePath = selectedFile.value.path
  const targetContext = createDiffContext(targetFilePath, selectedFile.value.staged)
  if (!isSameDiffContext(diffActiveContext, targetContext)) {
    frontendLogger.warn('[Diff] Blocked hunk stage because diff context is stale', {
      target: targetContext.key,
      active: diffActiveContext?.key || null,
      selected: getSelectedDiffContext()?.key || null
    })
    notification.warning({
      title: '无法暂存差异块',
      content: '当前差异视图仍在刷新，请稍后重试',
      duration: 2000
    })
    return
  }
  const change = getCurrentDiffChange()
  if (!change) {
    notification.warning({
      title: '无法暂存差异块',
      content: '未找到可暂存的差异块',
      duration: 2000
    })
    return
  }

  const hunkArgs = getDiffHunkInvokeArgs(change)
  const invokeHunkArgs = { ...hunkArgs }
  delete invokeHunkArgs.source
  delete invokeHunkArgs.precise
  diffLoading.value.hunk = true
  frontendLogger.info('[Diff] Staging hunk', {
    filePath: targetFilePath,
    ...hunkArgs,
    originalStart: change.originalStartLineNumber,
    originalEnd: change.originalEndLineNumber,
    modifiedStart: change.modifiedStartLineNumber,
    modifiedEnd: change.modifiedEndLineNumber
  })

  try {
    await invoke('stage_hunk', {
      path: props.workspacePath,
      filePath: targetFilePath,
      ...invokeHunkArgs
    })
    await refreshGitStatus({ resetSelection: false, includeCommits: false })
    const nextFile = unstagedFiles.value.find((item) => item?.path === targetFilePath)
    if (nextFile) {
      await selectFileAndUpdateDiff(nextFile, 'stage-hunk-refresh')
    } else {
      selectedFile.value = null
      diffHunkActionVisible.value = false
      diffEditor?.setModel?.(null)
    }
    notification.success({
      title: '已暂存差异块',
      content: targetFilePath,
      duration: 1600
    })
  } catch (error) {
    frontendLogger.error('[Diff] Failed to stage hunk', error)
    notification.error({
      title: '暂存差异块失败',
      content: String(error?.message || error),
      duration: 3000
    })
  } finally {
    diffLoading.value.hunk = false
    scheduleHunkActionPositionUpdate()
  }
}

const updateCommitDiff = async (commit) => {
  if (!isEditorInitialized.value || !diffEditorContainer.value) {
    await initMonacoEditor();
  }

  if (!commit || !diffEditor) return;

  diffLoading.value.commit = true;

  try {
    const diff = await invoke('get_commit_diff', {
      path: props.workspacePath,
      commitId: commit.id
    });

    const { original, modified } = parseUnifiedDiff(diff);

    const originalModel = monaco.editor.createModel(original, 'plaintext');
    const modifiedModel = monaco.editor.createModel(modified, 'plaintext');

    const oldModel = diffEditor.getModel();
    diffEditor.setModel({
      original: originalModel,
      modified: modifiedModel
    });
    refreshDiffLineChangesCache()
    const diffPerf = getDiffPerformanceOptions(original, modified)
    diffRenderSideBySide.value = !!diffPerf.options.renderSideBySide
    diffEditor.updateOptions(diffPerf.options)

    if (oldModel) {
      setTimeout(() => {
        oldModel.original?.dispose();
        oldModel.modified?.dispose();
      }, 100);
    }

    diffHunkActionVisible.value = false

    setTimeout(() => {
      const modifiedEditor = diffEditor.getModifiedEditor();
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
    const [result, current] = await Promise.all([
      invoke('list_branches', { path: props.workspacePath }),
      invoke('get_current_branch', { path: props.workspacePath })
    ])

    // 更新分支列表和当前分支
    branches.value = result.map(b => ({
      ...b,
      is_current: b.name === current
    }));

    currentBranch.value = current;
  } catch (error) {
    frontendLogger.error('Failed to fetch branches:', error);
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
        await refreshGitStatus({ includeCommits: true }),    // 刷新文件状态
        loadCommitHistory()    // 刷新提交历史
      ]);

      // 显示成功通知
      notification.success({
        title: '分支切换成功',
        content: `已切换到分支 ${branch.name}`,
        duration: 2000
      });
    } catch (error) {
      frontendLogger.error('Failed to checkout branch:', error);
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

const isIgnoredGitPath = (p) => {
  const s = String(p || '').replace(/\\/g, '/')
  const lower = s.toLowerCase()
  // 设计说明：`.cellmeta` 是探索者模式的内部侧车文件，由后台自动提交，
  // Git 面板只展示用户需要手动处理的正文/资源文件，避免元数据项污染工作区视图。
  return s === '.meta' || s.startsWith('.meta/') || s.includes('/.meta/') || lower.endsWith('.cellmeta')
}

const refreshGitStatus = async ({ resetSelection = true, includeCommits = true } = {}) => {
  if (isRefreshingStatus.value) return
  isRefreshingStatus.value = true

  logger.debug('Refreshing Git status')
  frontendLogger.info('repo path:', props.workspacePath)

  const prevSelectedFilePath = selectedFile.value?.path
  const prevSelectedFileStaged = typeof selectedFile.value?.staged === 'boolean'
    ? selectedFile.value.staged
    : null
  const prevSelectedCommitId = selectedCommit.value?.id

  try {
    const statuses = await invoke('get_git_status', {
      path: props.workspacePath
    });

    if (resetSelection) {
      selectedFile.value = null;
      selectedCommit.value = null;
      isEditorInitialized.value = false;
    }

    const newStagedFiles = []
    const newUnstagedFiles = []

    // 根据后端返回的 staged 字段正确分类文件
    statuses.forEach(file => {
      if (isIgnoredGitPath(file?.path)) return

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

    stagedFiles.value = newStagedFiles
    unstagedFiles.value = newUnstagedFiles

    if (includeCommits) {
      await loadCommitHistory()
    }

    if (!resetSelection) {
      if (prevSelectedCommitId) {
        const matchedCommit = commits.value.find((c) => c?.id === prevSelectedCommitId)
        if (matchedCommit) {
          if (selectedCommit.value && selectedCommit.value.id === matchedCommit.id) {
            Object.assign(selectedCommit.value, matchedCommit)
          } else {
            selectedCommit.value = matchedCommit
          }
        }
      }

      if (prevSelectedFilePath && !prevSelectedCommitId) {
        const all = [...stagedFiles.value, ...unstagedFiles.value]
        // 设计说明：同一文件在部分暂存后会同时出现在“更改”和“已暂存”。
        // 刷新状态时必须优先恢复原来的 staged/unstaged 面板上下文，否则胶囊按钮
        // 会拿当前视图的缓存去操作另一侧的 hunk，或因上下文不一致被隐藏。
        const matched = all.find((f) => f?.path === prevSelectedFilePath && f?.staged === prevSelectedFileStaged)
          || all.find((f) => f?.path === prevSelectedFilePath)
        if (matched) {
          frontendLogger.info('[GitPanel] Restored selected file after status refresh', {
            filePath: matched.path,
            previousStaged: prevSelectedFileStaged,
            restoredStaged: matched.staged
          })
          if (
            selectedFile.value
            && selectedFile.value.path === matched.path
            && selectedFile.value.staged === matched.staged
          ) {
            Object.assign(selectedFile.value, matched)
          } else {
            selectedFile.value = matched
          }
        } else {
          selectedFile.value = {
            path: prevSelectedFilePath,
            status: ' ',
            diff: '',
            staged: false
          }
        }
      }
    }
  } catch (error) {
    logger.error('Failed to refresh Git status', error)
  } finally {
    isRefreshingStatus.value = false
  }
}

const loadCommitHistory = async () => {
  if (commitHistoryLoading.value) return
  commitHistoryLoading.value = true

  try {
    const [markers, branch] = await Promise.all([
      invoke('get_git_head_markers', { path: props.workspacePath }),
      currentBranch.value
        ? Promise.resolve(currentBranch.value)
        : invoke('get_current_branch', { path: props.workspacePath })
    ])

    if (!currentBranch.value && branch) {
      currentBranch.value = branch
    }

    // 提交历史区域优先保证面板响应速度：先加载较小窗口，列表内再逐步展开可见项。
    commits.value = await invoke('get_git_log', {
      path: props.workspacePath,
      branchName: branch || currentBranch.value,
      limit: 80
    })
    commitHistoryVisibleLimit.value = 60

    headMarkers.value = markers
    logger.debug('Commit history loaded', { count: commits.value.length, visible: commitHistoryVisibleLimit.value })
  } catch (error) {
    logger.error('Failed to load commit history', error)
  } finally {
    commitHistoryLoading.value = false
  }
}

const formatDateFull = (timestamp) => {
  return new Date(parseInt(timestamp) * 1000).toLocaleString()
}

const getFileStatusCodicon = (status) => {
  switch (normalizeGitStatus(status)) {
    case 'A': return 'diff-added'
    case 'M': return 'diff-modified'
    case 'D': return 'diff-removed'
    case '?': return 'new-file'
    case 'R': return 'diff-renamed'
    case 'T': return 'symbol-file'
    default: return 'file'
  }
}

const getFileStatusIcon = (status) => {
  switch (normalizeGitStatus(status)) {
    case 'A': return AddCircleOutline
    case 'M': return CreateOutline
    case 'D': return RemoveCircleOutline
    case '?': return DocumentOutline
    case 'R': return GitCommitOutline
    case 'T': return GitCommitOutline
    default: return DocumentOutline
  }
}

const getStatusClass = (status) => {
  switch (normalizeGitStatus(status)) {
    case 'A': return 'git-status-a'
    case 'M': return 'git-status-m'
    case 'D': return 'git-status-d'
    case 'R': return 'git-status-r'
    case 'T': return 'git-status-t'
    case '?': return 'git-status-u'
    default: return 'git-status-o'
  }
}

watch(() => settingsStore.appearanceTheme, () => {
  ensureGuaxiMonacoThemes()
  monaco.editor.setTheme(getMonacoTheme())
})

const initMonacoEditor = async () => {
  if (isEditorInitialized.value && isDiffEditorAttachedToCurrentContainer()) return;
  if (isEditorInitialized.value || diffEditor) {
    disposeDiffEditorInstance('init-detected-detached-or-stale-editor')
  }
  if (!diffEditorContainer.value) return;

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
    diffEditorHostElement = editorContainer

    // 注册自定义语言
    await registerDiffLanguages();

    ensureGuaxiMonacoThemes()
    monaco.editor.setTheme(getMonacoTheme())

    // 初始化差异编辑器
    diffEditor = markRaw(monaco.editor.createDiffEditor(editorContainer, {
      originalEditable: false,
      readOnly: true,
      automaticLayout: false,
      scrollbar: {
        verticalScrollbarSize: 10,
        horizontalScrollbarSize: 10,
        arrowSize: 0,
        useShadows: false
      },
      theme: getMonacoTheme(),
      fontSize: 16,
      fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
      fontLigatures: true,
      renderSideBySide: true,
      renderIndicators: true,
      scrollBeyondLastLine: false,
      // Git diff 面板不依赖 sticky scroll；关闭它可以避免 Monaco 在暗色模式下渲染
      // editor.contrib.stickyScrollWidget 时，把首行左侧 gutter 露成亮色块。
      stickyScroll: {
        enabled: false
      },
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
    frontendLogger.info('[DiffTheme] Sticky scroll disabled for GitPanel diff editor')

    isEditorInitialized.value = true;
    ensureGuaxiMonacoThemes()
    monaco.editor.setTheme(getMonacoTheme());

    // 初始空模型
    const emptyModel = monaco.editor.createModel('', 'plaintext');
    diffEditor.setModel({
      original: emptyModel,
      modified: emptyModel
    });
    clearDiffHunkCaches('diff-editor-initial-model', null)

    if (diffHunkScrollDisposable) {
      diffHunkScrollDisposable.dispose()
      diffHunkScrollDisposable = null
    }
    if (diffHunkOriginalScrollDisposable) {
      diffHunkOriginalScrollDisposable.dispose()
      diffHunkOriginalScrollDisposable = null
    }
    if (diffHunkCursorDisposable) {
      diffHunkCursorDisposable.dispose()
      diffHunkCursorDisposable = null
    }
    if (diffUpdateDisposable) {
      diffUpdateDisposable.dispose()
      diffUpdateDisposable = null
    }

    const modifiedEditor = diffEditor.getModifiedEditor()
    const originalEditor = diffEditor.getOriginalEditor()
    diffHunkScrollDisposable = modifiedEditor.onDidScrollChange(() => {
      scheduleHunkActionPositionUpdate()
    })
    diffHunkOriginalScrollDisposable = originalEditor.onDidScrollChange(() => {
      scheduleHunkActionPositionUpdate()
    })
    diffHunkCursorDisposable = modifiedEditor.onDidChangeCursorPosition(() => {
      scheduleHunkActionPositionUpdate()
    })
    if (typeof diffEditor.onDidUpdateDiff === 'function') {
      diffUpdateDisposable = diffEditor.onDidUpdateDiff(() => {
        refreshDiffLineChangesCache()
        scheduleHunkActionPositionUpdateAfterLayout('monaco-diff-updated')
        frontendLogger.info('[Diff] Monaco diff update completed; hunk actions refreshed')
      })
    } else {
      frontendLogger.warn('[Diff] Monaco diff editor does not expose onDidUpdateDiff; using demand cache fallback')
    }

    // 添加resize observer
    resizeObserver = new ResizeObserver(() => {
      if (diffEditor && editorContainer) {
        const height = editorContainer.clientHeight;
        const width = editorContainer.clientWidth;
        diffEditor.layout({ width, height });
        scheduleHunkActionPositionUpdate()
      }
    });
    resizeObserver.observe(editorContainer);

  } catch (error) {
    frontendLogger.error('Failed to initialize Monaco diff editor:', error);
    notification.error({
      title: '编辑器初始化失败',
      content: error.message,
      duration: 3000
    });
  }
}

const updateFileDiff = async (file) => {
  frontendLogger.info('updateFileDiff: isEditorInitialized.value ', isEditorInitialized.value, 'diffEditorContainer.value', diffEditorContainer.value);
  if (diffEditor && !isDiffEditorAttachedToCurrentContainer()) {
    disposeDiffEditorInstance('file-diff-detected-detached-editor')
  }
  if (!isEditorInitialized.value || !diffEditorContainer.value) {
    await initMonacoEditor();
  }

  frontendLogger.info('updateFileDiff: diffEditor ', diffEditor, 'diffEditorContainer.value', diffEditorContainer.value);
  if (!diffEditor || !diffEditorContainer.value) {
    frontendLogger.error('Diff editor not available');
    return;
  }

  const requestId = diffFileDiffRequestId + 1
  diffLoading.value.file = true;

  try {
    const filePath = normalizeDiffFilePath(file.path);
    const isStagedDiff = !!file.staged
    const context = createDiffContext(filePath, isStagedDiff)
    diffFileDiffRequestId = requestId
    diffActiveContext = context
    clearDiffHunkCaches('file-diff-request-start', context)
    diffHunkActionVisible.value = false

    const originalRevision = isStagedDiff ? 'HEAD' : 'INDEX'
    const modifiedRevision = isStagedDiff ? 'INDEX' : null
    frontendLogger.info('[Diff] Loading diff for:', {
      filePath,
      staged: isStagedDiff,
      originalRevision,
      modifiedRevision: modifiedRevision || 'WORKTREE',
      context: context.key,
      requestId
    });

    // 设计说明：部分暂存后，同一文件会在“更改”和“已暂存”各出现一条。
    // 已暂存视图展示 HEAD -> INDEX；未暂存视图展示 INDEX -> 工作区，
    // 这样胶囊按钮拿到的 hunk 范围才和当前面板一致。
    const modifiedContentRequest = {
      path: props.workspacePath,
      filePath: filePath
    }
    if (modifiedRevision) {
      modifiedContentRequest.revision = modifiedRevision
    }

    const [originalContent, modifiedContent] = await Promise.all([
      invoke('get_file_content', {
        path: props.workspacePath,
        filePath: filePath,
        revision: originalRevision
      }).catch(e => {
        frontendLogger.warn('Failed to get original content:', e);
        return ''; // 对于新文件，原始内容为空
      }),
      invoke('get_file_content', modifiedContentRequest).catch(e => {
        frontendLogger.error('Failed to get modified content:', e);
        throw e;
      })
    ]);
    if (!isCurrentDiffContext(context, requestId)) {
      frontendLogger.warn('[Diff] Ignored stale file diff content result', {
        context: context.key,
        active: diffActiveContext?.key || null,
        selected: getSelectedDiffContext()?.key || null,
        requestId,
        currentRequestId: diffFileDiffRequestId
      })
      return
    }

    await refreshGitHunksCache(context, requestId)
    if (!isCurrentDiffContext(context, requestId)) {
      frontendLogger.warn('[Diff] Ignored stale file diff after hunk cache refresh', {
        context: context.key,
        active: diffActiveContext?.key || null,
        selected: getSelectedDiffContext()?.key || null,
        requestId,
        currentRequestId: diffFileDiffRequestId
      })
      return
    }

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
    refreshDiffLineChangesCache()
    scheduleHunkActionPositionUpdateAfterLayout('file-diff-model-set')

    // 清理旧模型（如果有）
    if (oldModel) {
      setTimeout(() => {
        oldModel.original?.dispose();
        oldModel.modified?.dispose();
      }, 100);
    }

    // 大文件/大差异时切到性能模式，避免 diff 操作因装饰和分栏计算卡顿。
    const diffPerf = getDiffPerformanceOptions(originalContent, modifiedContent)
    diffRenderSideBySide.value = !!diffPerf.options.renderSideBySide
    diffEditor.updateOptions(diffPerf.options);
    if (diffPerf.largeDiff) {
      frontendLogger.info('[Diff] Large working tree diff rendered in performance mode', {
        filePath,
        context: context.key
      })
    }

    // 确保布局正确
    await nextTick();
    if (!isCurrentDiffContext(context, requestId)) {
      frontendLogger.warn('[Diff] Skipped stale file diff layout', {
        context: context.key,
        active: diffActiveContext?.key || null,
        selected: getSelectedDiffContext()?.key || null,
        requestId,
        currentRequestId: diffFileDiffRequestId
      })
      return
    }
    if (diffEditor && diffEditorContainer.value) {
      const height = diffEditorContainer.value.clientHeight;
      const width = diffEditorContainer.value.clientWidth;
      diffEditor.layout({ width, height });

      // 导航到第一个差异
      setTimeout(async () => {
        if (!isCurrentDiffContext(context, requestId)) {
          frontendLogger.warn('[Diff] Skipped stale initial diff navigation', {
            context: context.key,
            active: diffActiveContext?.key || null,
            selected: getSelectedDiffContext()?.key || null,
            requestId,
            currentRequestId: diffFileDiffRequestId
          })
          return
        }
        const modifiedEditor = diffEditor.getModifiedEditor();
        const diffNavAction = modifiedEditor.getAction('editor.action.diffReview.next');
        if (diffNavAction) {
          await diffNavAction.run();
          modifiedEditor.revealPositionInCenter(modifiedEditor.getPosition());
        }
      }, 100);
    }
  } catch (error) {
    if (requestId !== diffFileDiffRequestId) {
      frontendLogger.warn('[Diff] Ignored stale file diff error', {
        requestId,
        currentRequestId: diffFileDiffRequestId,
        error
      })
      return
    }
    frontendLogger.error('[Diff] Error in updateFileDiff:', error);
    const errorModel = monaco.editor.createModel(
      `Error loading file diff:\n${error.message}`,
      'text/plain'
    );
    diffEditor.setModel({
      original: errorModel,
      modified: errorModel
    });
    clearDiffHunkCaches('file-diff-error', diffActiveContext)
  } finally {
    if (requestId === diffFileDiffRequestId) {
      diffLoading.value.file = false;
    }
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
    if (!isEditorInitialized.value) {
      await initMonacoEditor()
    }

    await selectFileAndUpdateDiff(file, 'file-click')
  } catch (error) {
    frontendLogger.error('Failed to load file content', error)
    notification.error({
      title: '加载文件失败',
      content: `无法加载文件 ${file.path} 的内容: ${error}`,
      duration: 3000
    })
  }
}

const handleStageFile = async (file) => {
  if (isGitFileBusy(file.path)) return
  setGitFileBusy(file.path, true)
  frontendLogger.info('[GitPanel] Stage file requested', { filePath: file.path })
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

    scheduleAutoRefresh()

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
    frontendLogger.error('[GitPanel] Stage file failed', { filePath: file.path, error })
  } finally {
    setGitFileBusy(file.path, false)
  }
}

const handleUnstageFile = async (file) => {
  if (isGitFileBusy(file.path)) return
  setGitFileBusy(file.path, true)
  frontendLogger.info('[GitPanel] Unstage file requested', { filePath: file.path })
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

    scheduleAutoRefresh()

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
    frontendLogger.error('[GitPanel] Unstage file failed', { filePath: file.path, error })
  } finally {
    setGitFileBusy(file.path, false)
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
    notification.error({
      title: '暂存全部失败',
      content: String(error?.message || error),
      duration: 3000
    })
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

const handleCommitKeydown = (e) => {
  if (!e) return
  if (e.ctrlKey && e.key === 'Enter') {
    e.preventDefault()
    if (!commitForm.value.message.trim() || stagedFiles.value.length === 0) return
    handleCommit()
  }
}

const handlePull = async () => {
  logger.debug('Pulling changes', { path: props.workspacePath })
  gitLoading.value.pull = true
  try {
    await invoke('git_pull', { path: props.workspacePath })
    logger.info('Pull successful')
    notification.success({
      title: '拉取成功',
      content: currentBranch.value ? `已更新 ${currentBranch.value}` : '已更新本地分支',
      duration: 2000
    })
    refreshGitStatus()
  } catch (error) {
    const msg = String(error?.message || error)
    logger.error('Pull failed', {
      path: props.workspacePath,
      error: msg
    })
    notification.error({
      title: '拉取失败',
      content: msg,
      duration: 4000
    })
  } finally {
    gitLoading.value.pull = false
  }
}

const handlePush = async () => {
  logger.debug('Pushing changes', { path: props.workspacePath })
  gitLoading.value.push = true
  try {
    await invoke('git_push', { path: props.workspacePath })
    logger.info('Push successful')
    notification.success({
      title: '推送成功',
      content: currentBranch.value ? `已推送 ${currentBranch.value}` : '已推送到远端',
      duration: 2000
    })
  } catch (error) {
    const msg = String(error?.message || error)
    logger.error('Push failed', {
      path: props.workspacePath,
      error: msg
    })
    notification.error({
      title: '推送失败',
      content: msg,
      duration: 4000
    })
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
  if (suppressSelectedFileDiffWatch) {
    frontendLogger.info('[Diff] Skipped selectedFile watcher because explicit diff refresh is running', {
      context: newVal ? createDiffContext(newVal.path, newVal.staged).key : null
    })
    return
  }
  if (newVal && !selectedCommit.value) {
    nextTick(() => {
      updateFileDiff(newVal)
    })
  } else if (!newVal && diffEditor) {
    disposeDiffEditorInstance('selected-file-cleared')
  }
})

// 监听 selectedCommit 变化
watch(selectedCommit, (newVal) => {
  if (newVal) {
    nextTick(() => {
      updateCommitDiff(newVal)
    })
  } else if (!selectedFile.value && diffEditor) {
    disposeDiffEditorInstance('selected-commit-cleared')
  }
})

onMounted(() => {
  logger.debug('Component mounted')

  const setupAutoRefresh = async () => {
    try {
      if (fileChangeUnlisten.value) {
        await fileChangeUnlisten.value()
        fileChangeUnlisten.value = null
      }

      fileChangeUnlisten.value = await listen('file_change', (event) => {
        const payload = event?.payload || {}
        if (!isPathUnderRepo(payload.path)) return
        scheduleAutoRefresh()
      })
    } catch (e) {
      logger.warn('Failed to listen file_change', e)
    }

    if (autoRefreshIntervalId) clearInterval(autoRefreshIntervalId)
    autoRefreshIntervalId = setInterval(() => {
      if (!gitAutoRefresh.value.enabled) return
      if (document.visibilityState !== 'visible') return
      refreshGitStatus({ resetSelection: false, includeCommits: false })
    }, gitAutoRefresh.value.intervalMs)

    const onFocus = () => scheduleAutoRefresh()
    window.addEventListener('focus', onFocus)

    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') scheduleAutoRefresh()
    }
    document.addEventListener('visibilitychange', onVisibilityChange)

    onUnmounted(() => {
      window.removeEventListener('focus', onFocus)
      document.removeEventListener('visibilitychange', onVisibilityChange)
    })
  }

  // 初始化操作
  const initialize = async () => {
    try {
      await setupAutoRefresh()
      await Promise.all([
        refreshGitStatus(),
        fetchBranches()
      ])

      // 初始化编辑器但不等待
      initMonacoEditor().catch(frontendLogger.error)

      // 设置面板高度
      nextTick(() => {
        if (changesSection.value && stagedSection.value && commitsSection.value && panelsContainer.value) {
          panelsHeight.value = panelsContainer.value.offsetHeight
          changesHeight.value = panelsHeight.value * 0.3
          stagedHeight.value = panelsHeight.value * 0.3
          commitsHeight.value = panelsHeight.value * 0.4

          const changesHeader = changesSection.value.querySelector('.section-header')
          const stagedHeader = stagedSection.value.querySelector('.section-header')
          const commitsHeader = commitsSection.value.querySelector('.section-header')

          changesHeaderHeight.value = changesHeader?.offsetHeight || 40
          stagedHeaderHeight.value = stagedHeader?.offsetHeight || 40
          commitsHeaderHeight.value = commitsHeader?.offsetHeight || 40

        }
      })
    } catch (error) {
      frontendLogger.error('Initialization error:', error)
    }
  }

  // 执行初始化但不等待
  initialize().catch(frontendLogger.error)
})

onUnmounted(() => {
  if (refreshDebounceTimerId) {
    clearTimeout(refreshDebounceTimerId)
    refreshDebounceTimerId = null
  }
  if (autoRefreshIntervalId) {
    clearInterval(autoRefreshIntervalId)
    autoRefreshIntervalId = null
  }
  if (diffHunkAnimationFrameId) {
    window.cancelAnimationFrame(diffHunkAnimationFrameId)
    diffHunkAnimationFrameId = null
  }
  if (fileChangeUnlisten.value) {
    fileChangeUnlisten.value().catch(() => {})
    fileChangeUnlisten.value = null
  }

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

  if (diffHunkScrollDisposable) {
    diffHunkScrollDisposable.dispose()
    diffHunkScrollDisposable = null
  }
  if (diffHunkOriginalScrollDisposable) {
    diffHunkOriginalScrollDisposable.dispose()
    diffHunkOriginalScrollDisposable = null
  }
  if (diffHunkCursorDisposable) {
    diffHunkCursorDisposable.dispose()
    diffHunkCursorDisposable = null
  }
  if (diffUpdateDisposable) {
    diffUpdateDisposable.dispose()
    diffUpdateDisposable = null
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
  --git-monaco-bg: #fafafa;
}

:root[data-guaxi-theme='dark'] {
  --hover-color: rgba(255, 255, 255, 0.06);
  --added-bg: rgba(40, 167, 69, 0.18);
  --modified-bg: rgba(255, 193, 7, 0.16);
  --deleted-bg: rgba(220, 53, 69, 0.16);
  --current-branch-bg: rgba(40, 167, 69, 0.22);
  --git-monaco-bg: #282c34;
}

/* 设计说明：
   1. Git diff 视图使用 Monaco DiffEditor，内部的 find-widget、当前行 margin、active line number
      不完全受主题 token 控制，暗色模式下会露出默认亮色边框或背景。
   2. 这里把兜底样式限制在 .gitkraken-container 内，避免影响主编辑器、Slidev 编辑器等其他 Monaco 实例。
   3. 主题 defineTheme 负责语义颜色；这组 CSS 只修复 Monaco 内部 DOM 在暗色模式下的残留亮色块。 */
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .margin,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .glyph-margin,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .margin-view-overlays,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .monaco-editor-background {
  background-color: #282c34 !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor {
  --vscode-editorWidget-background: #282c34 !important;
  --vscode-editorWidget-foreground: #abb2bf !important;
  --vscode-widget-border: #3b4048 !important;
  --vscode-widget-shadow: rgba(0, 0, 0, 0.42) !important;
  --vscode-input-background: #21252b !important;
  --vscode-input-foreground: #abb2bf !important;
  --vscode-input-border: #3b4048 !important;
  --vscode-input-placeholderForeground: #7f8898 !important;
  --vscode-focusBorder: rgba(82, 139, 255, 0.5) !important;
  --vscode-editorStickyScroll-background: #282c34 !important;
  --vscode-editorStickyScrollGutter-background: #282c34 !important;
  --vscode-editorStickyScrollHover-background: #2c313a !important;
  --vscode-editorStickyScroll-border: transparent !important;
  --vscode-editorStickyScroll-shadow: rgba(0, 0, 0, 0.42) !important;
  --vscode-editorLineNumber-foreground: #5c6370 !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-widget,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-widget-line-numbers,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-widget-lines-scrollable,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-widget-lines,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-line-number,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-line-content {
  background-color: #282c34 !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-widget {
  border-bottom-color: transparent !important;
  box-shadow: rgba(0, 0, 0, 0.42) 0 4px 2px -2px !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-line-number-inner,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-line-number .codicon {
  color: #5c6370 !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .sticky-line-content:hover {
  background-color: #2c313a !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget {
  background-color: #282c34 !important;
  border-color: #3b4048 !important;
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.32) !important;
  color: #abb2bf !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget .monaco-inputbox,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget .monaco-inputbox .input,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget input,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget textarea {
  background-color: #21252b !important;
  border-color: #3b4048 !important;
  color: #abb2bf !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget .monaco-inputbox.synthetic-focus {
  border-color: rgba(82, 139, 255, 0.5) !important;
  outline-color: rgba(82, 139, 255, 0.5) !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .find-widget textarea::placeholder {
  color: #7f8898 !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .current-line,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .view-overlays .current-line,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .margin-view-overlays .current-line,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .margin-view-overlays .current-line-margin,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .margin-view-overlays .current-line-margin-both,
:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .line-numbers.active-line-number {
  background-color: #2c313a !important;
  border-color: transparent !important;
}

:root[data-guaxi-theme='dark'] .gitkraken-container .monaco-editor .line-numbers.active-line-number {
  color: #d7deea !important;
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

.git-file-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.git-status-icon {
  width: 16px;
  flex: 0 0 16px;
}

.git-codicon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  font-size: 16px;
  line-height: 1;
}

.git-status-a {
  color: #2da44e;
}

.git-status-m {
  color: #bf8700;
}

.git-status-d {
  color: #d1242f;
}

.git-status-r,
.git-status-t {
  color: #1f6feb;
}

.git-status-u {
  color: var(--text-color-secondary);
}

.git-status-o {
  color: var(--text-color-secondary);
}

:root[data-guaxi-theme='dark'] .git-status-a {
  color: #3fb950;
}

:root[data-guaxi-theme='dark'] .git-status-m {
  color: #d29922;
}

:root[data-guaxi-theme='dark'] .git-status-d {
  color: #f85149;
}

:root[data-guaxi-theme='dark'] .git-status-r,
:root[data-guaxi-theme='dark'] .git-status-t {
  color: #58a6ff;
}

.commit-row {
  align-items: center;
}

.commit-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.commit-markers {
  display: flex;
  align-items: center;
  gap: 6px;
}

.panel-header-icon {
  color: var(--text-color);
}

.commit-marker-icon {
  color: var(--text-color-secondary);
}

.commit-marker-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border-radius: 999px;
  border: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  line-height: 1;
  max-width: 160px;
}

.commit-marker-pill-icon {
  flex: 0 0 auto;
}

.commit-marker-pill-text {
  font-size: 11px;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.commit-marker-local {
  background-color: rgba(0, 123, 255, 0.12);
}

.commit-marker-local .commit-marker-pill-icon {
  color: var(--primary-color);
}

.commit-marker-remote .commit-marker-pill-icon {
  color: var(--text-color-secondary);
}

.commit-time {
  font-size: 12px;
}

.commit-hash {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 12px;
}

.commit-tooltip-meta {
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-color-secondary);
}

.vscode-commit-tooltip {
  max-width: 520px;
  white-space: normal;
}

.commit-tooltip-header {
  font-size: 12px;
  color: var(--git-tooltip-text-secondary, var(--text-color-secondary));
  margin-bottom: 8px;
}

.commit-tooltip-message {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.commit-tooltip-subject {
  font-size: 13px;
  color: var(--git-tooltip-text, var(--text-color));
}

.commit-tooltip-body {
  margin: 0;
  font-size: 12px;
  color: var(--git-tooltip-text, var(--text-color));
  white-space: pre-wrap;
  word-break: break-word;
}

.commit-tooltip-stats {
  margin-top: 10px;
  font-size: 12px;
  color: var(--git-tooltip-text-secondary, var(--text-color-secondary));
}

.commit-tooltip-hash {
  margin-top: 6px;
  font-size: 12px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  color: var(--git-tooltip-text-secondary, var(--text-color-secondary));
}

.git-file-left {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1 1 auto;
}

.git-file-path {
  display: block;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.git-file-actions {
  flex: 0 0 auto;
}

.vscode-action-button {
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 3px;
  padding: 0;
  background: transparent;
  color: var(--text-color-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.vscode-action-button:hover:not(:disabled) {
  background-color: var(--hover-color);
  color: var(--text-color);
}

.vscode-action-button:disabled {
  cursor: default;
  opacity: 0.45;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

.section-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.section-header-right {
  display: flex;
  align-items: center;
  gap: 4px;
}

.section-title {
  font-size: 12px;
}

.section-count {
  font-size: 12px;
  color: var(--text-color-secondary);
}

.section-icon-button {
  border: none;
  padding: 4px 6px;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-color);
}

.section-icon-button:hover:not(:disabled) {
  background-color: var(--hover-color);
  color: var(--text-color);
}

.section-icon-button:disabled {
  color: var(--text-color-secondary);
  cursor: default;
}

.section-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
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
  font-family: 'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
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
  position: relative;
}

.diff-hunk-action {
  position: absolute;
  z-index: 5;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  width: 24px;
  padding: 2px 1px;
  box-sizing: border-box;
  border: 1px solid color-mix(in srgb, var(--divider-color) 55%, transparent);
  border-radius: 5px;
  background-color: color-mix(in srgb, var(--git-monaco-bg) 88%, var(--card-bg));
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  pointer-events: auto;
  transform: none;
  overflow: visible;
  transition: background-color 0.14s ease, border-color 0.14s ease, box-shadow 0.14s ease;
}

.diff-hunk-action:hover {
  border-color: color-mix(in srgb, var(--text-color-secondary) 34%, var(--divider-color));
  background-color: color-mix(in srgb, var(--git-monaco-bg) 76%, var(--hover-color));
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.18);
}

.diff-hunk-action-button {
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  border-radius: 3px;
  border: 0;
  background-color: transparent;
  color: color-mix(in srgb, var(--text-color-secondary) 85%, var(--git-monaco-bg));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.diff-hunk-action-button:hover:not(:disabled) {
  background-color: color-mix(in srgb, var(--hover-color) 75%, transparent);
  color: var(--text-color);
}

.diff-hunk-action-button.danger:hover:not(:disabled) {
  color: #f85149;
}

.diff-hunk-action-button:disabled {
  color: var(--text-color-secondary);
  cursor: default;
  opacity: 0.5;
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
  flex-shrink: 0;
}

.commit-section {
  padding: 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

.commit-history-spin {
  min-height: 100%;
}

.commit-history-spin :deep(.n-spin-container) {
  min-height: 100%;
  height: auto;
  display: flex;
  flex-direction: column;
}

.commit-history-spin :deep(.n-spin-content) {
  flex: 1 0 auto;
  min-height: 0;
}

.commit-history-spin :deep(.n-spin-body) {
  flex: 1;
  min-height: 64px;
  box-sizing: border-box;
  padding: 12px 0 24px;
  overflow: visible;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.commit-history-spin :deep(.n-spin-description) {
  padding-top: 6px;
  line-height: 1.25;
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
  flex: 1;
  display: flex;
  flex-direction: column;
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
  transition: flex 0.1s ease-out;
  background-color: var(--card-bg);
  border-top: 1px solid var(--divider-color);
}

.commits-section {
  min-height: 140px;
}

.changes-section .n-collapse,
.staged-section .n-collapse,
.commits-section .n-collapse {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.changes-section .n-collapse-item,
.staged-section .n-collapse-item,
.commits-section .n-collapse-item {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.section-scroll {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
}

.section-scroll {
  scrollbar-width: thin;
  scrollbar-color: var(--text-color-secondary) transparent;
}

.section-scroll::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.section-scroll::-webkit-scrollbar-track {
  background: transparent;
}

.section-scroll::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 999px;
  border: 3px solid transparent;
  background-clip: content-box;
}

.section-scroll::-webkit-scrollbar-thumb:hover {
  background-color: var(--text-color);
}

.commits-toolbar {
  padding: 8px 12px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
}

.commits-list {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  margin-bottom: 0;
  padding-bottom: 0;
  scrollbar-gutter: stable;
}

.commits-list {
  scrollbar-width: thin;
  scrollbar-color: var(--text-color-secondary) transparent;
}

.commits-list::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.commits-list::-webkit-scrollbar-track {
  background: transparent;
}

.commits-list::-webkit-scrollbar-thumb {
  background-color: var(--text-color-secondary);
  border-radius: 999px;
  border: 3px solid transparent;
  background-clip: content-box;
}

.commits-list::-webkit-scrollbar-thumb:hover {
  background-color: var(--text-color);
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

.commit-load-more {
  width: calc(100% - 16px);
  margin: 6px 8px 10px;
  padding: 5px 8px;
  border: 0;
  border-radius: 3px;
  background: transparent;
  color: var(--text-color-secondary);
  text-align: left;
  cursor: pointer;
}

.commit-load-more:hover {
  background-color: var(--hover-color);
  color: var(--text-color);
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
.n-collapse-item__content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.n-collapse-item__content-inner {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
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
  overflow: visible;
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
  padding: 0 12px;
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
  background-color: var(--git-monaco-bg) !important;
}

.monaco-editor .monaco-editor-background {
  background-color: var(--git-monaco-bg) !important;
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
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>
