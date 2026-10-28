<template>
  <!-- 主容器 - 直接作为页面内容 -->
  <div class="gitkraken-container">
    <!-- 左侧面板 - 类似VSCode的Git面板 -->
    <div class="git-sidebar" ref="sidebar" :style="{ width: isSidebarCollapsed ? '0px' : sidebarWidth + 'px' }">
      <!-- 侧边栏右侧拖动条 -->
      <div class="sidebar-resize-handle" @pointerdown="startSidebarDrag"></div>

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
          <div class="scm-header-row">
            <span class="scm-title">源代码管理</span>
            <div class="scm-header-actions">
              <button
                class="scm-icon-button"
                type="button"
                title="刷新状态"
                :disabled="isRefreshingStatus"
                @click="refreshAll"
              >
                <Codicon name="refresh" />
              </button>
              <n-dropdown
                trigger="click"
                placement="bottom-end"
                :options="moreMenuOptions"
                @select="handleMoreAction"
              >
                <button class="scm-icon-button" type="button" title="更多操作">
                  <Codicon name="ellipsis" />
                </button>
              </n-dropdown>
            </div>
          </div>

          <!-- 分支选择器：点击展开分支管理；后面跟 ahead/behind 同步按钮。 -->
          <div class="scm-header-sub">
            <n-popover
              v-model:show="branchPopoverVisible"
              trigger="click"
              placement="bottom-start"
              :width="330"
              :show-arrow="false"
              raw
            >
              <template #trigger>
                <button
                  class="scm-branch-button"
                  type="button"
                  :title="currentBranch || '未检出分支'"
                >
                  <Codicon name="git-branch" />
                  <span class="scm-branch-name">{{ currentBranch || '未检出分支' }}</span>
                  <Codicon name="chevron-down" class="scm-branch-caret" />
                </button>
              </template>

              <div class="branch-picker">
                <div class="branch-picker-search">
                  <n-input
                    v-model:value="branchFilter"
                    size="small"
                    placeholder="搜索分支"
                    clearable
                  >
                    <template #prefix>
                      <Codicon name="search" />
                    </template>
                  </n-input>
                </div>

                <div class="branch-picker-list">
                  <template v-for="group in branchGroups" :key="group.key">
                    <div class="branch-group-label">{{ group.label }}</div>
                    <div
                      v-for="branch in group.branches"
                      :key="`${group.key}-${branch.name}`"
                      class="branch-row"
                      :class="{ 'is-current': branch.is_current }"
                    >
                      <button
                        class="branch-row-main"
                        type="button"
                        :title="branch.upstream ? `${branch.name} → ${branch.upstream}` : branch.name"
                        @click="handleBranchItemClick(branch)"
                      >
                        <Codicon :name="branch.is_remote ? 'cloud' : 'git-branch'" class="branch-row-icon" />
                        <span class="branch-row-name">{{ branch.name }}</span>
                        <span v-if="branch.ahead" class="branch-row-count is-ahead">↑{{ branch.ahead }}</span>
                        <span v-if="branch.behind" class="branch-row-count is-behind">↓{{ branch.behind }}</span>
                        <Codicon v-if="branch.is_current" name="check" class="branch-row-check" />
                      </button>
                      <div v-if="!branch.is_current" class="branch-row-actions">
                        <button type="button" title="合并到当前分支" @click.stop="handleMergeBranch(branch)">
                          <Codicon name="git-merge" />
                        </button>
                        <button
                          v-if="!branch.is_remote"
                          type="button"
                          title="重命名分支"
                          @click.stop="promptRenameBranch(branch)"
                        >
                          <Codicon name="edit" />
                        </button>
                        <button
                          v-if="!branch.is_remote"
                          type="button"
                          title="删除分支"
                          @click.stop="handleDeleteBranch(branch)"
                        >
                          <Codicon name="trash" />
                        </button>
                      </div>
                    </div>
                  </template>
                  <div v-if="branchFilteredEmpty" class="branch-picker-empty">没有匹配的分支</div>
                </div>

                <div class="branch-picker-footer">
                  <button type="button" @click="openCreateBranchModal()">
                    <Codicon name="add" /> 创建分支…
                  </button>
                  <button type="button" title="刷新分支" @click="refreshBranches">
                    <Codicon name="refresh" />
                  </button>
                </div>
              </div>
            </n-popover>

            <span v-if="workspacePathName" class="scm-repo" :title="workspacePath">{{ workspacePathName }}</span>

            <button
              v-if="syncPending"
              class="scm-sync-button"
              type="button"
              :title="syncTooltip"
              :disabled="gitLoading.pull || gitLoading.push"
              @click="handleSync"
            >
              <Codicon name="sync" />
              <span v-if="syncState.behind" class="scm-sync-count">↓{{ syncState.behind }}</span>
              <span v-if="syncState.ahead" class="scm-sync-count">↑{{ syncState.ahead }}</span>
            </button>
          </div>

          <div
            v-if="syncState.merging || syncState.cherry_picking || syncState.rebasing"
            class="scm-merge-banner"
          >
            <Codicon name="git-merge" />
            <span>
              {{ syncState.merging ? '正在合并，解决冲突后提交即可完成'
                : (syncState.cherry_picking ? '正在拣选提交，解决冲突后提交即可完成' : '正在变基，解决冲突后继续') }}
            </span>
            <button type="button" title="中止当前操作" @click="handleAbortOperation">
              <Codicon name="close" />
            </button>
          </div>
        </div>

        <!-- 提交区域：输入框在上，按钮拆成「提交 + 下拉」两段，行为对齐 VS Code。 -->
        <div class="commit-section">
          <n-input
            v-model:value="commitForm.message"
            type="textarea"
            placeholder="提交信息（Ctrl+Enter 提交）"
            :autosize="{ minRows: 2, maxRows: 6 }"
            class="commit-message-input"
            @keydown="handleCommitKeydown"
          />
          <div class="commit-actions">
            <button
              class="commit-button"
              type="button"
              :disabled="!canCommit || gitLoading.commit"
              :title="conflictedFiles.length
                ? '先解决合并冲突'
                : (stagedFiles.length === 0 ? '先暂存要提交的文件' : '提交已暂存的更改（Ctrl+Enter）')"
              @click="handleCommit()"
            >
              <Codicon name="check" />
              <span>{{ gitLoading.commit ? '提交中…' : '提交' }}</span>
              <span v-if="stagedFiles.length" class="commit-button-count">{{ stagedFiles.length }}</span>
            </button>
            <n-dropdown
              trigger="click"
              placement="bottom-end"
              :options="commitMenuOptions"
              @select="handleCommitMenu"
            >
              <button class="commit-button-caret" type="button" title="更多提交操作">
                <Codicon name="chevron-down" />
              </button>
            </n-dropdown>
          </div>
          <div v-if="conflictedFiles.length" class="commit-conflict-hint">
            <Codicon name="warning" />
            <span>{{ conflictedFiles.length }} 个文件存在冲突，解决后才能提交</span>
          </div>
        </div>

        <!-- 合并冲突：单独一组，不参与「更改 / 已暂存」的高度分配。 -->
        <div v-if="conflictedFiles.length" class="conflicts-section">
          <div class="section-header">
            <div class="section-header-left">
              <span class="section-title">合并更改</span>
              <span class="section-count">{{ conflictedFiles.length }}</span>
            </div>
            <div class="section-header-right">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <button
                    class="section-icon-button"
                    type="button"
                    @click="openMergeEditor(conflictedFiles[0])"
                  >
                    <Codicon name="git-merge" />
                  </button>
                </template>
                打开合并编辑器
              </n-tooltip>
            </div>
          </div>
          <div class="section-scroll conflict-list">
            <div
              v-for="file in conflictedFiles"
              :key="`conflict-${file.path}`"
              class="git-file-row conflict-row"
              @click="openMergeEditor(file)"
            >
              <div class="git-file-left">
                <Icon :icon="getFileIcon(file)" class="git-file-icon" />
                <span class="git-file-label">
                  <span class="git-file-name">{{ getFileBaseName(file.path) }}</span>
                  <span v-if="getFileDirName(file.path)" class="git-file-dir">{{ getFileDirName(file.path) }}</span>
                </span>
              </div>
              <div class="git-file-actions">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <button class="vscode-action-button" type="button" title="打开合并编辑器" @click.stop="openMergeEditor(file)">
                      <Codicon name="git-merge" />
                    </button>
                  </template>
                  打开合并编辑器
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <button class="vscode-action-button" type="button" title="标记为已解决" @click.stop="handleMarkResolved(file)">
                      <Codicon name="check" />
                    </button>
                  </template>
                  标记为已解决
                </n-tooltip>
              </div>
              <span class="git-file-status git-status-conflict">U</span>
            </div>
          </div>
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
                        :title="fileViewMode === 'tree' ? '切换为列表视图' : '切换为树形视图'"
                        @click.stop="toggleFileViewMode"
                      >
                        <Codicon :name="fileViewMode === 'tree' ? 'list-flat' : 'list-tree'" />
                      </button>
                    </template>
                    {{ fileViewMode === 'tree' ? '切换为列表视图' : '切换为树形视图' }}
                  </n-tooltip>
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
                <template v-for="row in unstagedRows" :key="row.key">
                  <div
                    v-if="row.type === 'folder'"
                    class="git-folder-row"
                    :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
                    @click="toggleFolder(row.path)"
                  >
                    <Codicon
                      :name="isFolderCollapsed(row.path) ? 'chevron-right' : 'chevron-down'"
                      class="git-folder-chevron"
                    />
                    <Codicon
                      :name="isFolderCollapsed(row.path) ? 'folder' : 'folder-opened'"
                      class="git-folder-icon"
                    />
                    <span class="git-folder-name">{{ row.name }}</span>
                    <span class="git-folder-count">{{ row.count }}</span>
                  </div>
                  <div
                    v-else
                    class="git-file-row"
                    :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
                    @click="handleFileClick(row.file)"
                  >
                    <div class="git-file-left">
                      <Icon :icon="getFileIcon(row.file)" class="git-file-icon" />
                      <n-tooltip trigger="hover">
                        <template #trigger>
                          <span class="git-file-label">
                            <span class="git-file-name">{{ getFileBaseName(row.file.path) }}</span>
                            <span v-if="getRenameHint(row.file)" class="git-file-rename">{{ getRenameHint(row.file) }}</span>
                            <span
                              v-else-if="fileViewMode === 'list' && getFileDirName(row.file.path)"
                              class="git-file-dir"
                            >{{ getFileDirName(row.file.path) }}</span>
                          </span>
                        </template>
                        {{ getFileDisplayTitle(row.file) }}
                      </n-tooltip>
                    </div>
                    <div class="git-file-actions">
                      <n-tooltip trigger="hover">
                        <template #trigger>
                          <button
                            class="vscode-action-button"
                            type="button"
                            title="暂存"
                            @click.stop="handleStageFile(row.file)"
                            :disabled="isGitFileBusy(row.file.path)"
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
                            title="丢弃更改"
                            @click.stop="handleDiscardFile(row.file)"
                            :disabled="isGitFileBusy(row.file.path)"
                          >
                            <Codicon name="discard" />
                          </button>
                        </template>
                        丢弃
                      </n-tooltip>
                    </div>
                    <span class="git-file-status" :class="getStatusClass(row.file.status)">
                      {{ getStatusLetter(row.file.status) }}
                    </span>
                  </div>
                </template>
                <div v-if="!unstagedFiles.length" class="git-list-empty">没有更改</div>
              </div>
            </div>

            <!-- Changes和Staged Changes之间的分隔线 -->
            <div class="section-divider" ref="changesDivider" @pointerdown="startChangesDrag"></div>

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
                <template v-for="row in stagedRows" :key="row.key">
                  <div
                    v-if="row.type === 'folder'"
                    class="git-folder-row"
                    :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
                    @click="toggleFolder(row.path)"
                  >
                    <Codicon
                      :name="isFolderCollapsed(row.path) ? 'chevron-right' : 'chevron-down'"
                      class="git-folder-chevron"
                    />
                    <Codicon
                      :name="isFolderCollapsed(row.path) ? 'folder' : 'folder-opened'"
                      class="git-folder-icon"
                    />
                    <span class="git-folder-name">{{ row.name }}</span>
                    <span class="git-folder-count">{{ row.count }}</span>
                  </div>
                  <div
                    v-else
                    class="git-file-row"
                    :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
                    @click="handleFileClick(row.file)"
                  >
                    <div class="git-file-left">
                      <Icon :icon="getFileIcon(row.file)" class="git-file-icon" />
                      <n-tooltip trigger="hover">
                        <template #trigger>
                          <span class="git-file-label">
                            <span class="git-file-name">{{ getFileBaseName(row.file.path) }}</span>
                            <span v-if="getRenameHint(row.file)" class="git-file-rename">{{ getRenameHint(row.file) }}</span>
                            <span
                              v-else-if="fileViewMode === 'list' && getFileDirName(row.file.path)"
                              class="git-file-dir"
                            >{{ getFileDirName(row.file.path) }}</span>
                          </span>
                        </template>
                        {{ getFileDisplayTitle(row.file) }}
                      </n-tooltip>
                    </div>
                    <div class="git-file-actions">
                      <n-tooltip trigger="hover">
                        <template #trigger>
                          <button
                            class="vscode-action-button"
                            type="button"
                            title="取消暂存"
                            @click.stop="handleUnstageFile(row.file)"
                            :disabled="isGitFileBusy(row.file.path)"
                          >
                            <Codicon name="remove" />
                          </button>
                        </template>
                        取消暂存
                      </n-tooltip>
                    </div>
                    <span class="git-file-status" :class="getStatusClass(row.file.status)">
                      {{ getStatusLetter(row.file.status) }}
                    </span>
                  </div>
                </template>
                <div v-if="!stagedFiles.length" class="git-list-empty">没有已暂存的更改</div>
              </div>
            </div>

            <!-- Staged Changes和提交历史之间的分隔线 -->
            <div class="commits-divider" ref="commitsDivider" @pointerdown="startCommitsDrag"></div>

            <!-- 提交历史区域 -->
            <div class="commits-section" ref="commitsSection" :style="{ flex: `0 0 ${commitsHeight}px` }">
              <div class="section-header">
                <div class="section-header-left">
                  <span class="section-title">提交历史</span>
                  <span class="section-count">{{ commits.length }}</span>
                </div>
                <div class="section-header-right">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        :disabled="gitLoading.pull"
                        @click="handlePull"
                      >
                        <Codicon name="repo-pull" />
                      </button>
                    </template>
                    拉取
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        :disabled="gitLoading.push"
                        @click="handlePush"
                      >
                        <Codicon name="repo-push" />
                      </button>
                    </template>
                    推送
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <button
                        class="section-icon-button"
                        type="button"
                        @click="openCreateBranchModal()"
                      >
                        <Codicon name="add" />
                      </button>
                    </template>
                    创建分支
                  </n-tooltip>
                  <button
                    class="section-branch-chip"
                    type="button"
                    :title="currentBranch || '未检出分支'"
                    @click="branchPopoverVisible = true"
                  >
                    <Codicon name="git-branch" />
                    <span>{{ currentBranch || 'HEAD' }}</span>
                  </button>
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
                    v-for="(commit, commitIndex) in visibleCommits"
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
                      @contextmenu.prevent.stop="openCommitContextMenu($event, commit)"
                    >
                      <div class="git-file-row commit-row">
                        <!-- 泳道图：按行独立一块 SVG，宽度随 lane 数增长。
                             连线画在上下边界上，相邻行天然接得上，不会因为滚动错位。 -->
                        <svg
                          v-if="getRowNode(commitIndex)"
                          class="commit-graph"
                          :width="commitGraphWidth"
                          :height="COMMIT_ROW_HEIGHT"
                          :viewBox="`0 0 ${commitGraphWidth} ${COMMIT_ROW_HEIGHT}`"
                          aria-hidden="true"
                        >
                          <path
                            v-if="getRowPaths(getRowNode(commitIndex)).through"
                            class="commit-graph-line"
                            :d="getRowPaths(getRowNode(commitIndex)).through"
                            :stroke="laneColor(getRowNode(commitIndex).color)"
                          />
                          <!-- 进入本行的线用「通往本提交那条线」的颜色。
                               以前这里画成灰色半透明，于是每条竖线一屏里被切成半灰半彩、
                               看着像一串珠子——这也是泳道图显得廉价最主要的来源。 -->
                          <path
                            v-if="getRowPaths(getRowNode(commitIndex)).incoming"
                            class="commit-graph-line"
                            :d="getRowPaths(getRowNode(commitIndex)).incoming"
                            :stroke="laneColor(getRowNode(commitIndex).color)"
                          />
                          <path
                            v-if="getRowPaths(getRowNode(commitIndex)).outgoing"
                            class="commit-graph-line"
                            :d="getRowPaths(getRowNode(commitIndex)).outgoing"
                            :stroke="laneColor(getRowNode(commitIndex).color)"
                          />
                          <circle
                            class="commit-graph-dot"
                            :cx="laneCenterX(getRowNode(commitIndex).lane)"
                            :cy="COMMIT_ROW_HEIGHT / 2"
                            r="3.5"
                            :fill="laneColor(getRowNode(commitIndex).color)"
                          />
                        </svg>
                        <div class="git-file-left">
                          <n-icon :component="GitCommitOutline" size="16" class="commit-icon" />
                          <div class="git-file-path">
                            <n-text>{{ commit.summary }}</n-text>
                          </div>
                        </div>
                        <div class="commit-right">
                          <div class="commit-markers">
                        <n-tooltip
                          v-for="ref in getExtraRefs(commit)"
                          :key="ref"
                          trigger="hover"
                        >
                          <template #trigger>
                            <span class="commit-marker-pill commit-marker-other">
                              <n-icon :component="GitBranchOutline" size="14" class="commit-marker-pill-icon" />
                              <span class="commit-marker-pill-text">{{ ref }}</span>
                            </span>
                          </template>
                          {{ ref }}
                        </n-tooltip>
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
          <n-space align="center">
            <template v-if="selectedFile && !selectedCommit">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <button
                    class="vscode-action-button"
                    type="button"
                    title="暂存更改"
                    :disabled="selectedFile.staged || isGitFileBusy(selectedFile.path)"
                    @click="handleStageFile(selectedFile)"
                  >
                    <Codicon name="add" />
                  </button>
                </template>
                暂存更改
              </n-tooltip>
              <n-tooltip trigger="hover">
                <template #trigger>
                  <button
                    class="vscode-action-button"
                    type="button"
                    title="取消暂存"
                    :disabled="!selectedFile.staged || isGitFileBusy(selectedFile.path)"
                    @click="handleUnstageFile(selectedFile)"
                  >
                    <Codicon name="remove" />
                  </button>
                </template>
                取消暂存
              </n-tooltip>
              <n-tooltip trigger="hover">
                <template #trigger>
                  <button
                    v-if="!selectedFile.staged"
                    class="vscode-action-button"
                    type="button"
                    title="丢弃更改"
                    :disabled="isGitFileBusy(selectedFile.path)"
                    @click="handleDiscardFile(selectedFile)"
                  >
                    <Codicon name="discard" />
                  </button>
                </template>
                丢弃更改
              </n-tooltip>
              <n-divider vertical />
            </template>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="navigateDiff('prev')" :disabled="!canNavigateDiff">
                  <template #icon>
                    <n-icon>
                      <ChevronUpOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              上一个差异（Shift+F7 / Alt+↑）
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="navigateDiff('next')" :disabled="!canNavigateDiff">
                  <template #icon>
                    <n-icon>
                      <ChevronDownOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              下一个差异（F7 / Alt+↓）
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

  <!-- 提交历史右键菜单（手动定位） -->
  <n-dropdown
    :show="commitContextMenu.show"
    :x="commitContextMenu.x"
    :y="commitContextMenu.y"
    :options="commitContextOptions"
    placement="bottom-start"
    @select="handleCommitContextSelect"
    @clickoutside="closeCommitContextMenu"
  />

  <!-- 创建分支 -->
  <n-modal v-model:show="showCreateBranchModal" preset="dialog" title="创建分支">
    <n-form>
      <n-form-item label="分支名称" required>
        <n-input
          v-model:value="newBranchName"
          placeholder="请输入新分支名称"
          @keydown.enter="handleCreateBranch"
        />
      </n-form-item>
      <n-form-item v-if="newBranchStartPoint" label="起点">
        <n-input :value="newBranchStartPoint" readonly placeholder="当前 HEAD" />
      </n-form-item>
      <n-checkbox v-model:checked="newBranchCheckout">创建后立即检出</n-checkbox>
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

  <!-- 重命名分支 -->
  <n-modal v-model:show="showRenameBranchModal" preset="dialog" title="重命名分支">
    <n-form>
      <n-form-item label="分支名称" required>
        <n-input
          v-model:value="renameBranchForm.newName"
          placeholder="新的分支名称"
          @keydown.enter="handleRenameBranchSubmit"
        />
      </n-form-item>
    </n-form>
    <template #action>
      <n-space justify="end">
        <n-button @click="showRenameBranchModal = false">取消</n-button>
        <n-button
          type="primary"
          @click="handleRenameBranchSubmit"
          :disabled="!renameBranchForm.newName.trim() || renameBranchForm.newName.trim() === renameBranchForm.oldName"
        >
          重命名
        </n-button>
      </n-space>
    </template>
  </n-modal>

  <!-- 创建标签 -->
  <n-modal v-model:show="showTagModal" preset="dialog" title="创建标签">
    <n-form>
      <n-form-item label="标签名" required>
        <n-input v-model:value="tagForm.name" placeholder="例如 v1.0.0" @keydown.enter="handleCreateTag" />
      </n-form-item>
      <n-form-item label="说明（可选）">
        <n-input v-model:value="tagForm.message" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
      </n-form-item>
    </n-form>
    <template #action>
      <n-space justify="end">
        <n-button @click="showTagModal = false">取消</n-button>
        <n-button type="primary" @click="handleCreateTag" :disabled="!tagForm.name.trim()">创建</n-button>
      </n-space>
    </template>
  </n-modal>

  <!-- 贮藏管理 -->
  <n-modal v-model:show="showStashModal" preset="card" title="贮藏" style="width: 680px">
    <div class="stash-list">
      <div v-for="entry in stashes" :key="entry.selector" class="stash-row">
        <div class="stash-main">
          <span class="stash-selector">{{ entry.selector }}</span>
          <span class="stash-message" :title="entry.message">{{ entry.message }}</span>
        </div>
        <div class="stash-actions">
          <n-button text size="tiny" @click="handleStashApply(entry, false)">应用</n-button>
          <n-button text size="tiny" @click="handleStashApply(entry, true)">弹出</n-button>
          <n-button text size="tiny" @click="handleStashDrop(entry)">删除</n-button>
        </div>
      </div>
      <n-empty v-if="!stashes.length" description="没有贮藏" />
    </div>
  </n-modal>

  <GitMergeEditor
    v-model:show="mergeEditorVisible"
    :workspace-path="workspacePath"
    :file-path="mergeEditorFilePath"
    @saved="handleMergeSaved"
  />
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
  AddCircleOutline,
  CreateOutline,
  RemoveCircleOutline,
  DocumentOutline,
  GitCommitOutline,
  ChevronUpOutline,
  ChevronDownOutline,
  CloudOutline
} from '@vicons/ionicons5'
import { ChevronLeft20Filled, ChevronRight20Filled } from '@vicons/fluent'
import { Icon } from '@iconify/vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import * as monaco from 'monaco-editor'
import { useSettingsStore } from '../../stores/settingsStore'
import {
  defineGuaxiMonacoThemes,
  getGuaxiMonacoTheme,
  getLanguageForFile as getLanguageForFileShared,
} from '../../monaco/guaxiMonacoThemes'
import { registerDiffLanguages } from './utils/monaco-diff' // 自定义语言注册
import {
  computeCommitGraph,
  buildLinkPaths,
  laneColor,
  laneCenterX,
  LANE_WIDTH,
} from './utils/gitCommitGraph'
import { buildGitFileTreeRows } from './utils/gitFileTree'
import { getFileTreeIcon } from './fileTreeIcons'
import { ensureCodiconStyles } from '../../utils/codiconFont'
import GitMergeEditor from './GitMergeEditor.vue'
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

const { notification, dialog } = createDiscreteApi(['notification', 'dialog'], {
  configProviderProps: gitPanelNaiveConfigProviderProps,
})

ensureCodiconStyles()

const Codicon = (props) => h('span', {
  class: ['codicon', `codicon-${props.name}`, 'git-codicon'],
  'aria-hidden': 'true'
})

let guaxiMonacoThemesReady = false
// 主题定义本身在 monaco/guaxiMonacoThemes.js 里，和差异对比窗口共用一份，
// 免得两个入口各注册一次同名主题、色值还不一样。
const ensureGuaxiMonacoThemes = () => {
  if (guaxiMonacoThemesReady) return

  defineGuaxiMonacoThemes(monaco)
  guaxiMonacoThemesReady = true
  frontendLogger.info('[DiffTheme] Monaco themes registered with translucent vivid diff highlights')
}

const getMonacoTheme = () => {
  ensureGuaxiMonacoThemes()
  return getGuaxiMonacoTheme(settingsStore.appearanceTheme)
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
// 设计说明：侧边栏宽度改为响应式来源。此前模板绑定的是常量 350px，拖动只命令式地写 inline
// style，任何响应式重渲染（如每 5s 的 Git 状态自动刷新）都会把宽度复位回 350px，导致拖宽后
// 又自动缩回、最右侧暂存/丢弃图标被裁掉。用 ref 驱动后，拖动结果会被保留。
const sidebarWidth = ref(350)
// 侧栏拖动起点宽度：拖动过程中只按"起点 + 位移"计算，避免每帧累加误差。
const initialWidthSidebar = ref(350)
const changesDivider = ref(null)
const stagedSection = ref(null)
const isDraggingChanges = ref(false)
const commitsDivider = ref(null)
const isDraggingCommits = ref(false)
// 用户手动拖过分隔条之后，自动刷新与窗口尺寸变化都不再按比例重算高度。
let panelsUserResized = false

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
const conflictedFiles = ref([])
const selectedFile = ref(null)
const gitFileOperationKeys = ref(new Set())
const gitLoading = ref({
  pull: false,
  push: false,
  commit: false,
  checkout: false  // 添加分支切换加载状态
});

// 顶部分支行 / 同步按钮 / 贮藏入口共用的仓库状态。
const syncState = ref({
  branch: null,
  detached: false,
  upstream: null,
  ahead: 0,
  behind: 0,
  has_remote: false,
  merging: false,
  rebasing: false,
  cherry_picking: false,
  stash_count: 0,
  conflicted: 0
})

const gitAutoRefresh = ref({
  enabled: true,
  intervalMs: 5000
})

const isRefreshingStatus = ref(false)
const fileChangeUnlisten = ref(null)
let autoRefreshIntervalId = null
let refreshDebounceTimerId = null
let commitHistoryRefreshPromise = null
let autoRefreshFocusHandler = null
let autoRefreshVisibilityHandler = null

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
  refreshDebounceTimerId = setTimeout(async () => {
    refreshDebounceTimerId = null
    await refreshGitAfterOperation({
      reason: 'auto-refresh',
      resetSelection: false,
      includeCommits: false,
      refreshBranches: false
    })
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
const branchPopoverVisible = ref(false)
const branchFilter = ref('')
const showCreateBranchModal = ref(false)
const newBranchName = ref('')
const newBranchStartPoint = ref('')
const newBranchCheckout = ref(true)
const showRenameBranchModal = ref(false)
const renameBranchForm = ref({ oldName: '', newName: '' })

// 文件列表视图：VS Code 默认按目录分组，这里也默认树形，可切回扁平列表。
const fileViewMode = ref('tree')
const collapsedFolders = ref(new Set())

// 提交按钮右侧的下拉菜单 / 提交历史的右键菜单。
const commitContextMenu = ref({ show: false, x: 0, y: 0, commit: null })
const showTagModal = ref(false)
const tagForm = ref({ name: '', message: '', commitId: null })

// 贮藏
const showStashModal = ref(false)
const stashes = ref([])

// 冲突合并编辑器
const mergeEditorVisible = ref(false)
const mergeEditorFilePath = ref('')

// Monaco Editor 实例
const diffEditorContainer = ref(null)
const isEditorInitialized = ref(false)
let diffEditor = null
let resizeObserver = null
let panelsResizeObserver = null
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

// 侧栏宽度与三段高度都按仓库记住：Git 窗口经常开关，每次回到默认值会很烦。
const gitPanelLayoutStorageKey = computed(() => `guaxi:gitPanelLayout:v2:${props.workspacePath || ''}`)

const readStoredLayout = () => {
  try {
    const raw = localStorage.getItem(gitPanelLayoutStorageKey.value)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object') return null
    return parsed
  } catch {
    return null
  }
}

const writeStoredLayout = () => {
  try {
    localStorage.setItem(gitPanelLayoutStorageKey.value, JSON.stringify({
      sidebarWidth: sidebarWidth.value,
      changesHeight: changesHeight.value,
      stagedHeight: stagedHeight.value,
      commitsHeight: commitsHeight.value,
      userResized: panelsUserResized,
    }))
  } catch {}
}

const restoreLayoutFromStorage = () => {
  const saved = readStoredLayout()
  if (!saved) return false
  const width = Number(saved.sidebarWidth)
  if (Number.isFinite(width) && width > 0) {
    sidebarWidth.value = clamp(width, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
  }
  if (saved.userResized) {
    const ch = Number(saved.changesHeight)
    const st = Number(saved.stagedHeight)
    const co = Number(saved.commitsHeight)
    if ([ch, st, co].every((v) => Number.isFinite(v) && v > 0)) {
      changesHeight.value = ch
      stagedHeight.value = st
      commitsHeight.value = co
      panelsUserResized = true
    }
  }
  return true
}

const visibleCommits = computed(() => commits.value.slice(0, commitHistoryVisibleLimit.value))

/**
 * 提交泳道图。
 *
 * 必须拿**完整**的 commits 算，不能只算可见的那一段：lane 的分配依赖前面所有行，
 * 截断后重新计算会让同一条线在滚动到"显示更多"之后突然换列、换色。
 * 图例按行号索引，渲染时用 `commitGraphNodes[index]` 取。
 */
const COMMIT_ROW_HEIGHT = 22
const commitGraph = computed(() => computeCommitGraph(commits.value))
const commitGraphNodes = computed(() => commitGraph.value.nodes)
const commitGraphWidth = computed(() => Math.max(LANE_WIDTH, commitGraph.value.laneCount * LANE_WIDTH))

/**
 * 每一行的 SVG path 只算一次。
 *
 * 渲染时模板里会按行取 `row.outgoingPath` 等；如果直接在模板里调
 * buildLinkPaths，每次重渲染（拖动列表、悬停、提交弹窗）都会把所有行重新拼一遍字符串。
 * 用 Map 缓存，键是节点对象本身——commits 变了会生成新对象，缓存自然失效。
 */
const rowPathCache = new WeakMap()
const getRowPaths = (node) => {
  if (!node) return { incoming: '', outgoing: '', through: '' }
  const cached = rowPathCache.get(node)
  if (cached) return cached
  const paths = {
    incoming: buildLinkPaths(node.incoming, COMMIT_ROW_HEIGHT, 'in'),
    outgoing: buildLinkPaths(node.outgoing, COMMIT_ROW_HEIGHT, 'out'),
    through: buildLinkPaths(node.through, COMMIT_ROW_HEIGHT, 'through'),
  }
  rowPathCache.set(node, paths)
  return paths
}

/** 本行需要画的圆点位置；合并提交画成空心圈，和线性历史的实心点区分开。 */
const getRowNode = (index) => commitGraphNodes.value[index] || null

/**
 * 过滤掉已经被 headMarkers 单独渲染的引用，避免同一个分支在行尾出现两个胶囊。
 *
 * 后端返回的 refs 里本来就包含当前分支和远端跟踪分支，而下面那两个 is HEAD 的胶囊
 * 用的是 headMarkers，两者必须去重，否则当前分支会被画两遍。
 */
const getExtraRefs = (commit) => {
  const all = Array.isArray(commit?.refs) ? commit.refs : []
  const skip = new Set()
  const local = String(currentBranch.value || '')
  if (local) skip.add(local)
  const remote = String(headMarkers.value?.remote_ref || '')
  if (remote) skip.add(remote)
  return all.filter((ref) => ref && !skip.has(ref))
}

// ───── VS Code 风格的列表 / 菜单 ───────────────────────────────

const unstagedRows = computed(() => buildGitFileTreeRows(unstagedFiles.value, {
  collapsed: collapsedFolders.value,
  mode: fileViewMode.value,
  prefix: 'work',
}))

const stagedRows = computed(() => buildGitFileTreeRows(stagedFiles.value, {
  collapsed: collapsedFolders.value,
  mode: fileViewMode.value,
  prefix: 'staged',
}))

const getFileIcon = (file) => getFileTreeIcon(getFileBaseName(file?.path), false)

const getStatusLetter = (status) => {
  const normalized = normalizeGitStatus(status)
  if (normalized === '?') return 'U'
  return normalized || ''
}

const toggleFileViewMode = () => {
  fileViewMode.value = fileViewMode.value === 'tree' ? 'list' : 'tree'
}

const isFolderCollapsed = (path) => collapsedFolders.value.has(path)
const toggleFolder = (path) => {
  const next = new Set(collapsedFolders.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  collapsedFolders.value = next
}

const canCommit = computed(() => (
  !!commitForm.value.message.trim()
  && stagedFiles.value.length > 0
  && conflictedFiles.value.length === 0
))

const syncPending = computed(() => syncState.value.behind > 0 || syncState.value.ahead > 0)
const syncTooltip = computed(() => {
  const parts = []
  if (syncState.value.behind) parts.push(`拉取 ${syncState.value.behind} 个提交`)
  if (syncState.value.ahead) parts.push(`推送 ${syncState.value.ahead} 个提交`)
  return parts.length ? `同步更改（${parts.join('，')}）` : '同步更改'
})

const branchGroups = computed(() => {
  const keyword = branchFilter.value.trim().toLowerCase()
  const matches = (branch) => !keyword || String(branch.name || '').toLowerCase().includes(keyword)
  const locals = branches.value
    .filter((branch) => !branch.is_remote && matches(branch))
    .sort((a, b) => Number(b.is_current) - Number(a.is_current) || a.name.localeCompare(b.name))
  const remotes = branches.value
    .filter((branch) => branch.is_remote && matches(branch))
    .sort((a, b) => a.name.localeCompare(b.name))
  return [
    { key: 'local', label: '本地', branches: locals },
    { key: 'remote', label: '远程', branches: remotes },
  ].filter((group) => group.branches.length)
})

const branchFilteredEmpty = computed(() => branchGroups.value.length === 0)

const commitMenuOptions = computed(() => [
  {
    label: '提交已暂存的更改',
    key: 'commit',
    disabled: !canCommit.value,
    icon: () => h(Codicon, { name: 'check' }),
  },
  {
    label: '提交全部更改',
    key: 'commit-all',
    disabled: !commitForm.value.message.trim() || conflictedFiles.value.length > 0,
    icon: () => h(Codicon, { name: 'check-all' }),
  },
  {
    label: '修订上次提交',
    key: 'amend',
    disabled: !commitForm.value.message.trim()
      || conflictedFiles.value.length > 0
      || syncState.value.merging,
    icon: () => h(Codicon, { name: 'git-commit' }),
  },
  {
    label: '提交并推送',
    key: 'commit-push',
    disabled: !canCommit.value,
    icon: () => h(Codicon, { name: 'repo-push' }),
  },
  { type: 'divider', key: 'divider-1' },
  {
    label: '撤销上次提交',
    key: 'undo-commit',
    disabled: syncState.value.merging || syncState.value.cherry_picking || syncState.value.rebasing,
    icon: () => h(Codicon, { name: 'history' }),
  },
])

const moreMenuOptions = computed(() => {
  const options = [
    { label: '刷新', key: 'refresh', icon: () => h(Codicon, { name: 'refresh' }) },
    { label: '获取 (Fetch)', key: 'fetch', icon: () => h(Codicon, { name: 'repo-fetch' }) },
    { label: '拉取 (Pull)', key: 'pull', icon: () => h(Codicon, { name: 'repo-pull' }) },
    { label: '拉取（变基）', key: 'pull-rebase', icon: () => h(Codicon, { name: 'repo-pull' }) },
    { label: '推送 (Push)', key: 'push', icon: () => h(Codicon, { name: 'repo-push' }) },
    { label: '强制推送（--force-with-lease）', key: 'push-force', icon: () => h(Codicon, { name: 'repo-push' }) },
    { type: 'divider', key: 'divider-1' },
    { label: '贮藏更改…', key: 'stash-push', icon: () => h(Codicon, { name: 'archive' }) },
    { label: '管理贮藏…', key: 'stash-list', icon: () => h(Codicon, { name: 'archive' }) },
    { label: '创建分支…', key: 'create-branch', icon: () => h(Codicon, { name: 'git-branch' }) },
  ]
  if (syncState.value.merging || syncState.value.cherry_picking || syncState.value.rebasing) {
    options.push(
      { type: 'divider', key: 'divider-2' },
      { label: '中止当前操作', key: 'abort-operation', icon: () => h(Codicon, { name: 'close' }) },
    )
  }
  return options
})

const commitContextOptions = [
  { label: '复制提交哈希', key: 'copy-hash', icon: () => h(Codicon, { name: 'copy' }) },
  { label: '检出此提交（分离 HEAD）', key: 'checkout', icon: () => h(Codicon, { name: 'git-commit' }) },
  { label: '从此提交创建分支…', key: 'create-branch', icon: () => h(Codicon, { name: 'git-branch' }) },
  { type: 'divider', key: 'divider-1' },
  { label: '拣选此提交 (Cherry Pick)', key: 'cherry-pick', icon: () => h(Codicon, { name: 'git-merge' }) },
  { label: '还原此提交 (Revert)', key: 'revert', icon: () => h(Codicon, { name: 'discard' }) },
  { label: '软重置到此提交', key: 'reset-soft', icon: () => h(Codicon, { name: 'history' }) },
  { label: '在此提交打标签…', key: 'tag', icon: () => h(Codicon, { name: 'tag' }) },
]

const confirmDialog = (options) => new Promise((resolve) => {
  let settled = false
  const settle = (value) => {
    if (settled) return
    settled = true
    resolve(value)
  }
  dialog.warning({
    title: options.title || '确认操作',
    content: options.content || '',
    positiveText: options.positiveText || '确定',
    negativeText: options.negativeText || '取消',
    onPositiveClick: () => settle(true),
    onNegativeClick: () => settle(false),
    onClose: () => settle(false),
    onMaskClick: () => settle(false),
  })
})


const showMoreCommits = () => {
  commitHistoryVisibleLimit.value += 60
  frontendLogger.info('[GitPanel] Expanded visible commit history', {
    visible: Math.min(commitHistoryVisibleLimit.value, commits.value.length),
    total: commits.value.length
  })
}

// ───── 同步状态 / 远程 ─────────────────────────────────────────

const fetchGitSyncState = async () => {
  try {
    const state = await invoke('get_git_sync_state', { path: props.workspacePath })
    syncState.value = {
      branch: state?.branch || null,
      detached: !!state?.detached,
      upstream: state?.upstream || null,
      ahead: Number(state?.ahead) || 0,
      behind: Number(state?.behind) || 0,
      has_remote: !!state?.has_remote,
      merging: !!state?.merging,
      rebasing: !!state?.rebasing,
      cherry_picking: !!state?.cherry_picking,
      stash_count: Number(state?.stash_count) || 0,
      conflicted: Number(state?.conflicted) || 0,
    }
    if (state?.branch) {
      currentBranch.value = state.branch
    } else if (state?.detached) {
      currentBranch.value = 'HEAD (分离)'
    }
  } catch (error) {
    frontendLogger.warn('[GitPanel] Failed to load sync state', error)
  }
}

const handleFetch = async () => {
  try {
    await invoke('git_fetch', { path: props.workspacePath })
    notification.success({ title: '获取成功', content: '已从远程更新引用', duration: 2000 })
    await fetchGitSyncState()
    await fetchBranches()
    await loadCommitHistory({ force: true })
  } catch (error) {
    notification.error({
      title: '获取失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const handleSync = async () => {
  if (gitLoading.value.pull || gitLoading.value.push) return
  if (syncState.value.behind > 0) {
    await handlePull()
  }
  await fetchGitSyncState()
  if (syncState.value.ahead > 0) {
    await handlePush()
  }
  await fetchGitSyncState()
}

const handleMoreAction = (key) => {
  switch (key) {
    case 'refresh':
      refreshAll()
      break
    case 'fetch':
      handleFetch()
      break
    case 'pull':
      handlePull()
      break
    case 'pull-rebase':
      handlePull({ rebase: true })
      break
    case 'push':
      handlePush()
      break
    case 'push-force':
      handlePush({ force: true })
      break
    case 'stash-push':
      handleStashPush()
      break
    case 'stash-list':
      openStashModal()
      break
    case 'create-branch':
      openCreateBranchModal()
      break
    case 'abort-operation':
      handleAbortOperation()
      break
    default:
      break
  }
}

const handleAbortOperation = async () => {
  const confirmed = await confirmDialog({
    title: '中止当前操作',
    content: '将放弃这次合并 / 拣选 / 还原产生的冲突与部分结果，恢复到操作前的状态。',
    positiveText: '中止操作',
  })
  if (!confirmed) return
  try {
    await invoke('abort_git_operation', { path: props.workspacePath })
    notification.success({ title: '操作已中止', duration: 2000 })
    await refreshGitAfterOperation({
      reason: 'abort-operation',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true,
    })
  } catch (error) {
    notification.error({
      title: '中止操作失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

// ───── 分支管理 ───────────────────────────────────────────────

const openCreateBranchModal = (startPoint = '') => {
  newBranchStartPoint.value = startPoint || ''
  newBranchName.value = ''
  newBranchCheckout.value = true
  showCreateBranchModal.value = true
}

const refreshBranches = async () => {
  await fetchBranches()
  await fetchGitSyncState()
}

const handleMergeBranch = async (branch) => {
  if (!branch || branch.is_current) return
  const confirmed = await confirmDialog({
    title: '合并分支',
    content: `将 ${branch.name} 合并到当前分支 ${currentBranch.value || 'HEAD'}？`,
    positiveText: '合并',
  })
  if (!confirmed) return

  branchPopoverVisible.value = false
  try {
    const outcome = await invoke('merge_branch', {
      path: props.workspacePath,
      branchName: branch.name,
    })
    if (outcome?.status === 'conflicts') {
      notification.warning({
        title: '合并产生冲突',
        content: `${outcome.conflicts?.length || 0} 个文件需要在「合并更改」中解决`,
        duration: 5000,
      })
    } else {
      notification.success({
        title: '合并成功',
        content: `已将 ${branch.name} 合并到 ${currentBranch.value}`,
        duration: 2500,
      })
    }
    await refreshGitAfterOperation({
      reason: 'merge-branch',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true,
    })
  } catch (error) {
    notification.error({
      title: '合并失败',
      content: String(error?.message || error),
      duration: 5000,
    })
  }
}

const handleDeleteBranch = async (branch) => {
  if (!branch || branch.is_remote) return
  const confirmed = await confirmDialog({
    title: '删除分支',
    content: `确定删除分支 ${branch.name} 吗？`,
    positiveText: '删除',
  })
  if (!confirmed) return

  try {
    await invoke('delete_branch', {
      path: props.workspacePath,
      branchName: branch.name,
      force: false,
    })
  } catch (error) {
    const forced = await confirmDialog({
      title: '分支尚未合并',
      content: `${error?.message || error}\n仍然强制删除 ${branch.name}？`,
      positiveText: '强制删除',
    })
    if (!forced) return
    try {
      await invoke('delete_branch', {
        path: props.workspacePath,
        branchName: branch.name,
        force: true,
      })
    } catch (forceError) {
      notification.error({
        title: '删除分支失败',
        content: String(forceError?.message || forceError),
        duration: 4000,
      })
      return
    }
  }

  notification.success({ title: '分支已删除', content: branch.name, duration: 2000 })
  await refreshBranches()
  await loadCommitHistory({ force: true })
}

const promptRenameBranch = (branch) => {
  if (!branch || branch.is_remote) return
  branchPopoverVisible.value = false
  renameBranchForm.value = { oldName: branch.name, newName: branch.name }
  showRenameBranchModal.value = true
}

const handleRenameBranchSubmit = async () => {
  const { oldName, newName } = renameBranchForm.value
  if (!newName.trim() || newName.trim() === oldName) {
    showRenameBranchModal.value = false
    return
  }
  try {
    await invoke('rename_branch', {
      path: props.workspacePath,
      oldName,
      newName: newName.trim(),
    })
    showRenameBranchModal.value = false
    notification.success({ title: '分支已重命名', content: newName.trim(), duration: 2000 })
    await refreshBranches()
    await loadCommitHistory({ force: true })
  } catch (error) {
    notification.error({
      title: '重命名分支失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

// ───── 提交 / 历史操作 ────────────────────────────────────────

const handleCommitMenu = (key) => {
  if (key === 'commit') handleCommit()
  else if (key === 'commit-all') handleCommit({ all: true })
  else if (key === 'amend') handleCommit({ amend: true })
  else if (key === 'commit-push') handleCommit({ push: true })
  else if (key === 'undo-commit') handleUndoCommit()
}

const handleUndoCommit = async () => {
  const confirmed = await confirmDialog({
    title: '撤销上次提交',
    content: '提交会被撤销，改动回到暂存区；不会丢失文件内容。',
    positiveText: '撤销提交',
  })
  if (!confirmed) return
  try {
    await invoke('undo_last_commit', { path: props.workspacePath })
    notification.success({ title: '已撤销上次提交', duration: 2000 })
    await refreshGitAfterOperation({
      reason: 'undo-commit',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true,
    })
  } catch (error) {
    notification.error({
      title: '撤销提交失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const openCommitContextMenu = (event, commit) => {
  if (!commit) return
  commitContextMenu.value = {
    show: true,
    x: event.clientX,
    y: event.clientY,
    commit,
  }
}

const closeCommitContextMenu = () => {
  commitContextMenu.value = { ...commitContextMenu.value, show: false }
}

const copyCommitHash = async (commit) => {
  const writeClipboard = async (text) => {
    if (navigator?.clipboard?.writeText) {
      try {
        await navigator.clipboard.writeText(text)
        return
      } catch {}
    }
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(text)
  }
  try {
    await writeClipboard(commit.id)
    notification.success({ title: '已复制提交哈希', content: commit.id.slice(0, 7), duration: 1600 })
  } catch (error) {
    notification.warning({ title: '复制失败', content: String(error?.message || error), duration: 2500 })
  }
}

const handleCommitContextSelect = async (key) => {
  const commit = commitContextMenu.value.commit
  commitContextMenu.value = { ...commitContextMenu.value, show: false }
  if (!commit) return

  if (key === 'copy-hash') {
    await copyCommitHash(commit)
    return
  }
  if (key === 'create-branch') {
    openCreateBranchModal(commit.id)
    return
  }
  if (key === 'tag') {
    tagForm.value = { name: '', message: '', commitId: commit.id }
    showTagModal.value = true
    return
  }

  if (key === 'checkout') {
    const confirmed = await confirmDialog({
      title: '检出提交',
      content: `将以分离 HEAD 状态检出 ${commit.id.slice(0, 7)}，确定继续吗？`,
      positiveText: '检出',
    })
    if (!confirmed) return
    try {
      await invoke('git_checkout_commit', {
        path: props.workspacePath,
        commitId: commit.id,
      })
      currentBranch.value = 'HEAD (分离)'
      await refreshGitAfterOperation({
        reason: 'checkout-commit',
        resetSelection: true,
        includeCommits: true,
        refreshBranches: true,
      })
    } catch (error) {
      notification.error({
        title: '检出提交失败',
        content: String(error?.message || error),
        duration: 4000,
      })
    }
    return
  }

  if (key === 'reset-soft') {
    const confirmed = await confirmDialog({
      title: '软重置',
      content: `将把当前分支重置到 ${commit.id.slice(0, 7)}，之后的提交变成未暂存的改动；不会丢失文件内容。`,
      positiveText: '软重置',
    })
    if (!confirmed) return
    try {
      await invoke('git_reset_to_commit', {
        path: props.workspacePath,
        commitId: commit.id,
        mode: 'soft',
      })
      notification.success({ title: '已软重置到该提交', duration: 2000 })
      await refreshGitAfterOperation({
        reason: 'reset-soft',
        resetSelection: true,
        includeCommits: true,
        refreshBranches: true,
      })
    } catch (error) {
      notification.error({
        title: '重置失败',
        content: String(error?.message || error),
        duration: 4000,
      })
    }
    return
  }

  if (key === 'cherry-pick' || key === 'revert') {
    const isPick = key === 'cherry-pick'
    const confirmed = await confirmDialog({
      title: isPick ? '拣选提交' : '还原提交',
      content: isPick
        ? `将把 ${commit.id.slice(0, 7)} 应用到当前分支，确定继续吗？`
        : `将撤销 ${commit.id.slice(0, 7)} 的改动，确定继续吗？`,
      positiveText: isPick ? '拣选' : '还原',
    })
    if (!confirmed) return
    try {
      const outcome = await invoke(isPick ? 'git_cherry_pick' : 'git_revert_commit', {
        path: props.workspacePath,
        commitId: commit.id,
      })
      if (outcome?.status === 'conflicts') {
        notification.warning({
          title: isPick ? '拣选产生冲突' : '还原产生冲突',
          content: `${outcome.conflicts?.length || 0} 个文件需要解决`,
          duration: 5000,
        })
      } else {
        notification.success({ title: isPick ? '拣选完成' : '还原完成', duration: 2000 })
      }
      await refreshGitAfterOperation({
        reason: isPick ? 'cherry-pick' : 'revert',
        resetSelection: true,
        includeCommits: true,
        refreshBranches: true,
      })
    } catch (error) {
      notification.error({
        title: isPick ? '拣选失败' : '还原失败',
        content: String(error?.message || error),
        duration: 5000,
      })
    }
  }
}

const handleCreateTag = async () => {
  const name = tagForm.value.name.trim()
  if (!name) return
  try {
    await invoke('git_create_tag', {
      path: props.workspacePath,
      name,
      message: tagForm.value.message.trim() || null,
      commitId: tagForm.value.commitId || null,
    })
    showTagModal.value = false
    notification.success({ title: '标签已创建', content: name, duration: 2000 })
    await loadCommitHistory({ force: true })
  } catch (error) {
    notification.error({
      title: '创建标签失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

// ───── 贮藏 ───────────────────────────────────────────────────

const handleStashPush = async () => {
  try {
    await invoke('git_stash_push', {
      path: props.workspacePath,
      message: null,
      includeUntracked: true,
    })
    notification.success({ title: '已贮藏当前更改', duration: 2000 })
    await refreshGitAfterOperation({
      reason: 'stash-push',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: false,
    })
  } catch (error) {
    notification.error({
      title: '贮藏失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const openStashModal = async () => {
  showStashModal.value = true
  try {
    stashes.value = await invoke('git_stash_list', { path: props.workspacePath })
  } catch (error) {
    notification.error({
      title: '读取贮藏失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const handleStashApply = async (entry, pop = false) => {
  try {
    const outcome = await invoke('git_stash_apply', {
      path: props.workspacePath,
      index: entry.index,
      pop,
    })
    if (outcome?.status === 'conflicts') {
      notification.warning({
        title: '应用贮藏产生冲突',
        content: `${outcome.conflicts?.length || 0} 个文件需要解决`,
        duration: 5000,
      })
    } else {
      notification.success({ title: pop ? '已弹出贮藏' : '已应用贮藏', duration: 2000 })
    }
    await refreshGitAfterOperation({
      reason: 'stash-apply',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: false,
    })
    await openStashModal()
  } catch (error) {
    notification.error({
      title: '应用贮藏失败',
      content: String(error?.message || error),
      duration: 5000,
    })
  }
}

const handleStashDrop = async (entry) => {
  const confirmed = await confirmDialog({
    title: '删除贮藏',
    content: `删除 ${entry.selector}？此操作不能撤销。`,
    positiveText: '删除',
  })
  if (!confirmed) return
  try {
    await invoke('git_stash_drop', { path: props.workspacePath, index: entry.index })
    await openStashModal()
    await fetchGitSyncState()
  } catch (error) {
    notification.error({
      title: '删除贮藏失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

// ───── 合并编辑器 ─────────────────────────────────────────────

const openMergeEditor = (file) => {
  if (!file?.path) return
  mergeEditorFilePath.value = file.path
  mergeEditorVisible.value = true
}

const handleMergeSaved = async () => {
  notification.success({
    title: '冲突已解决',
    content: mergeEditorFilePath.value,
    duration: 2000,
  })
  await refreshGitAfterOperation({
    reason: 'resolve-conflict',
    resetSelection: true,
    includeCommits: true,
    refreshBranches: false,
  })
}

const handleMarkResolved = async (file) => {
  try {
    const content = await invoke('git_read_file', {
      path: props.workspacePath,
      filePath: file.path,
    })
    if (/^(<{7}|={7}|>{7})/m.test(content)) {
      const confirmed = await confirmDialog({
        title: '仍包含冲突标记',
        content: '文件中还能找到 <<<<<<< / ======= / >>>>>>> 标记，标记为已解决会把它们一起提交。仍要继续吗？',
        positiveText: '仍然标记',
      })
      if (!confirmed) return
    }
    await invoke('resolve_conflict', {
      path: props.workspacePath,
      filePath: file.path,
      content,
    })
    await refreshGitAfterOperation({
      reason: 'mark-resolved',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: false,
    })
  } catch (error) {
    notification.error({
      title: '标记失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const refreshAll = () => refreshGitAfterOperation({
  reason: 'manual-refresh',
  resetSelection: false,
  includeCommits: true,
  refreshBranches: true,
})

const isGitFileBusy = (filePath) => gitFileOperationKeys.value.has(String(filePath || ''))
const setGitFileBusy = (filePath, busy) => {
  const key = String(filePath || '')
  if (!key) return
  const next = new Set(gitFileOperationKeys.value)
  if (busy) next.add(key)
  else next.delete(key)
  gitFileOperationKeys.value = next
}

/**
 * 按文件规模决定 diff 的渲染参数。
 *
 * 三条开销最大的设置，都随行数涨得很凶：
 * - `diffAlgorithm: 'advanced'`：对每一对行做相似度打分，大文件上是 O(n²) 量级；
 *   换个词级别（`advanced`）能换来更准的配对，但几千行以上用户要的是「先出画面」。
 * - `diffWordWrap: 'on'`：每行都要算断行，配合行内 diff 会让首帧更慢。
 * - 行内字符级高亮：同样按行对计算。
 * 大文件统一退化成 `smartWidth` + 按字符级关闭，观感和 VS Code 在大文件上的表现一致。
 */
const getDiffPerformanceOptions = (originalContent = '', modifiedContent = '') => {
  const originalLines = String(originalContent || '').split(/\r\n|\r|\n/).length
  const modifiedLines = String(modifiedContent || '').split(/\r\n|\r|\n/).length
  const totalLines = originalLines + modifiedLines
  const totalBytes = String(originalContent).length + String(modifiedContent).length
  // 三档而不是两档：中等文件保留并排视图（可读性明显更好），只砍掉最贵的两三项。
  const level = totalLines > 8000 || totalBytes > 900_000
    ? 'huge'
    : (totalLines > 2000 || totalBytes > 250_000 ? 'large' : 'normal')
  const huge = level === 'huge'
  const large = level !== 'normal'
  return {
    level,
    largeDiff: large,
    options: {
      renderSideBySide: !huge,
      renderIndicators: !huge,
      renderWhitespace: 'none',
      renderOverviewRuler: !huge,
      occurrencesHighlight: large ? 'off' : 'singleFile',
      selectionHighlight: !huge,
      // 大文件关掉「行内字符级高亮」：这是 diff 首帧最贵的一步。
      renderLineHighlight: huge ? 'none' : 'all',
      diffWordWrap: huge ? 'off' : 'on',
      // 'simple' 在几千行以上比 'advanced' 快一个数量级，粗粒度差异仍然准确。
      diffAlgorithm: huge ? 'simple' : (large ? 'smartWidth' : 'advanced'),
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

const canNavigateDiff = computed(() => {
  if (!selectedFile.value && !selectedCommit.value) return false
  if (diffLoading.value.file || diffLoading.value.commit || diffLoading.value.navigation) return false
  return !!diffEditor
})

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

const isEditableKeyboardTarget = (target) => {
  if (!target || !(target instanceof HTMLElement)) return false
  const tagName = target.tagName?.toLowerCase()
  return target.isContentEditable
    || tagName === 'input'
    || tagName === 'textarea'
    || tagName === 'select'
}

const handleDiffNavigationShortcut = (event) => {
  if (!event || event.defaultPrevented) return
  if (!canNavigateDiff.value) return
  const fromDiffEditor = event.target instanceof Node
    && !!diffEditorContainer.value
    && diffEditorContainer.value.contains(event.target)
  if (!fromDiffEditor && isEditableKeyboardTarget(event.target)) return

  let direction = null
  if (event.key === 'F7' && !event.ctrlKey && !event.metaKey && !event.altKey) {
    direction = event.shiftKey ? 'prev' : 'next'
  } else if (event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
    if (event.key === 'ArrowUp') direction = 'prev'
    if (event.key === 'ArrowDown') direction = 'next'
  }

  if (!direction) return

  event.preventDefault()
  event.stopPropagation()
  frontendLogger.info('[Diff] Keyboard shortcut navigating diff', {
    direction,
    key: event.key,
    altKey: event.altKey,
    shiftKey: event.shiftKey
  })
  navigateDiff(direction)
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

/**
 * 主动拉一次 hunk 列表（不带文件内容）。
 *
 * 正常切换文件时 hunk 由 `get_git_file_diff` 一并返回，用不到这里；
 * 只有「暂存 / 丢弃某个差异块之后」需要在不重载整个 diff 的前提下刷新这一处缓存。
 */
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
    await refreshGitAfterOperation({
      reason: isStagedDiff ? 'unstage-hunk' : 'discard-hunk',
      resetSelection: false,
      includeCommits: false
    })
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
    await refreshGitAfterOperation({
      reason: 'stage-hunk',
      resetSelection: false,
      includeCommits: false
    })
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

    // 和文件 diff 同样的顺序要求：性能参数先应用，setModel 之后 Monaco 就会直接用这套算法。
    const diffPerf = getDiffPerformanceOptions(original, modified)
    diffRenderSideBySide.value = !!diffPerf.options.renderSideBySide
    diffEditor.updateOptions(diffPerf.options)

    const originalModel = monaco.editor.createModel(original, 'plaintext');
    const modifiedModel = monaco.editor.createModel(modified, 'plaintext');

    const oldModel = diffEditor.getModel();
    diffEditor.setModel({
      original: originalModel,
      modified: modifiedModel
    });
    refreshDiffLineChangesCache()

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
const getLanguageForFile = getLanguageForFileShared

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
      title: '仍是合并状态',
      content: '当前有未解决的合并冲突，切换分支前建议先解决冲突或中止合并。是否继续？',
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
      invoke('get_current_branch', { path: props.workspacePath }).catch(() => '')
    ])

    // 后端现在同时返回远端分支和 upstream/领先落后信息，直接透传。
    branches.value = (result || []).map((branch) => ({
      ...branch,
      is_remote: !!branch.is_remote,
      upstream: branch.upstream || null,
      ahead: Number(branch.ahead) || 0,
      behind: Number(branch.behind) || 0,
      is_current: !branch.is_remote && branch.name === current,
    }));

    if (current) currentBranch.value = current;
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
  if (!branch || branch.is_current) return

  // 没有冲突时不再拦截：git checkout 会安全保留本地改动，只有在会被覆盖时才失败。
  if (syncState.value.merging || conflictedFiles.value.length > 0) {
    const confirmed = await confirmBranchSwitch();
    if (!confirmed) return;
  }

  try {
    gitLoading.value.checkout = true;
    await invoke('checkout_branch', {
      path: props.workspacePath,
      branchName: branch.name
    });

    branchPopoverVisible.value = false;
    await refreshGitAfterOperation({
      reason: 'checkout-branch',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true
    });

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
      duration: 4000
    });
  } finally {
    gitLoading.value.checkout = false;
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
    await invoke('create_branch', {
      path: props.workspacePath,
      branchName: newBranchName.value.trim(),
      startPoint: newBranchStartPoint.value || null,
      checkout: newBranchCheckout.value,
    });

    notification.success({
      title: '创建成功',
      content: `分支 ${newBranchName.value.trim()} 已创建`,
      duration: 2000
    });

    showCreateBranchModal.value = false;
    newBranchName.value = '';
    newBranchStartPoint.value = '';
    await refreshGitAfterOperation({
      reason: 'create-branch',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true,
    });
  } catch (error) {
    notification.error({
      title: '创建分支失败',
      content: error.toString(),
      duration: 4000
    });
  }
}

// ───── 拖动 ─────
//
// 旧实现有三处会让拖动"时而生效时而失效"：
// 1. 分隔条只有 2px 高，起点还必须是 e.target 本身，鼠标稍快就脱靶；
// 2. 移动/松开事件挂在 document 上，指针拖出窗口后收不到 mouseup，状态就卡在"正在拖动"；
// 3. 每 5 秒的自动刷新会调用 normalizePanelHeightsToContainer，把用户刚拖的高度
//    按比例重算回去。
// 现在统一用 pointer capture：一旦按下，后续 pointermove / pointerup 一定回到同一个元素，
// 拖出窗口也不会丢；并记住"用户手动调过高度"，让自动刷新不再覆盖。

const SIDEBAR_MIN_WIDTH = 220
const SIDEBAR_MAX_WIDTH = 640

const clamp = (value, min, max) => Math.min(max, Math.max(min, value))

const beginPointerDrag = (event, { onMove, onEnd, cursor }) => {
  const startX = event.clientX
  const startY = event.clientY
  const target = event.currentTarget

  const handleMove = (e) => {
    // 同步更新，不经过 rAF 节流：rAF 在后台标签页 / 部分合成器场景会被挂起，
    // 一旦挂起，用户拖到一半画面就僵住；而这里每次只改两三个 flex 高度，
    // 浏览器本身会在帧边界合并布局，同步调用没有抖动，反而更跟手。
    onMove({ dx: e.clientX - startX, dy: e.clientY - startY })
  }

  const teardown = () => {
    target?.removeEventListener?.('pointermove', handleMove)
    target?.removeEventListener?.('pointerup', handleUp)
    target?.removeEventListener?.('pointercancel', handleUp)
    try {
      if (target?.hasPointerCapture?.(event.pointerId)) {
        target.releasePointerCapture(event.pointerId)
      }
    } catch {}
    document.body.classList.remove('git-panel-resizing')
    document.body.style.cursor = ''
  }

  function handleUp() {
    teardown()
    onEnd?.()
  }

  try {
    target?.setPointerCapture?.(event.pointerId)
  } catch {}
  target?.addEventListener?.('pointermove', handleMove)
  target?.addEventListener?.('pointerup', handleUp)
  target?.addEventListener?.('pointercancel', handleUp)
  document.body.classList.add('git-panel-resizing')
  document.body.style.cursor = cursor

  event.preventDefault()
}

// 侧栏宽度：拖过就记住，不要再被模板的初始值覆盖。
const startSidebarDrag = (event) => {
  isDraggingSidebar.value = true
  beginPointerDrag(event, {
    cursor: 'col-resize',
    onMove: ({ dx }) => {
      sidebarWidth.value = clamp(initialWidthSidebar.value + dx, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
    },
    onEnd: () => {
      isDraggingSidebar.value = false
      writeStoredLayout()
    }
  })
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

const updatePanelHeaderHeights = () => {
  const changesHeader = changesSection.value?.querySelector?.('.section-header')
  const stagedHeader = stagedSection.value?.querySelector?.('.section-header')
  const commitsHeader = commitsSection.value?.querySelector?.('.section-header')

  changesHeaderHeight.value = changesHeader?.offsetHeight || 40
  stagedHeaderHeight.value = stagedHeader?.offsetHeight || 40
  commitsHeaderHeight.value = commitsHeader?.offsetHeight || 40
}

const getPanelsAvailableHeight = () => {
  const containerHeight = panelsContainer.value?.offsetHeight || 0
  const dividerHeight = (changesDivider.value?.offsetHeight || 0) + (commitsDivider.value?.offsetHeight || 0)
  panelsHeight.value = containerHeight
  return Math.max(0, containerHeight - dividerHeight)
}

const shrinkPanelHeightsToFit = (heights, minimums, targetTotal) => {
  let total = heights.reduce((sum, value) => sum + value, 0)
  let overflow = total - targetTotal
  if (overflow <= 0) return heights

  // 设计说明：窗口最大化/还原时按比例重算高度，但每段仍要至少露出标题栏。
  // 若最小高度导致总和超出可用高度，就从仍有余量的区块里分摊收缩，避免底部留下未分配空白。
  for (let guard = 0; guard < 4 && overflow > 0.5; guard += 1) {
    const shrinkableIndexes = heights
      .map((height, index) => ({ index, room: Math.max(0, height - minimums[index]) }))
      .filter((item) => item.room > 0)
    const shrinkableTotal = shrinkableIndexes.reduce((sum, item) => sum + item.room, 0)
    if (!shrinkableTotal) break

    shrinkableIndexes.forEach(({ index, room }) => {
      const delta = Math.min(room, overflow * (room / shrinkableTotal))
      heights[index] -= delta
    })
    total = heights.reduce((sum, value) => sum + value, 0)
    overflow = total - targetTotal
  }

  return heights
}

const applyPanelHeightRatios = (ratios, reason = 'panel-layout') => {
  if (!panelsContainer.value) return
  updatePanelHeaderHeights()
  const availableHeight = getPanelsAvailableHeight()
  if (availableHeight <= 0) return

  const minimums = [
    Math.max(changesHeaderHeight.value, 40),
    Math.max(stagedHeaderHeight.value, 40),
    Math.max(commitsHeaderHeight.value, 140),
  ]
  const normalizedRatios = ratios && ratios.length === 3 ? ratios : [0.3, 0.3, 0.4]
  let heights = normalizedRatios.map((ratio, index) => Math.max(minimums[index], availableHeight * ratio))
  heights = shrinkPanelHeightsToFit(heights, minimums, availableHeight)

  const roundedChanges = Math.max(minimums[0], Math.floor(heights[0]))
  const roundedStaged = Math.max(minimums[1], Math.floor(heights[1]))
  const roundedCommits = Math.max(
    minimums[2],
    Math.round(availableHeight - roundedChanges - roundedStaged)
  )

  changesHeight.value = roundedChanges
  stagedHeight.value = roundedStaged
  commitsHeight.value = roundedCommits

  frontendLogger.info('[GitPanel] Panel heights normalized', {
    reason,
    containerHeight: panelsHeight.value,
    availableHeight,
    changesHeight: changesHeight.value,
    stagedHeight: stagedHeight.value,
    commitsHeight: commitsHeight.value,
  })
}

const normalizePanelHeightsToContainer = (reason = 'panel-resize') => {
  if (isDraggingChanges.value || isDraggingCommits.value) return
  // 用户手动拖过高度之后，自动刷新和窗口尺寸变化都不该再按比例重算——
  // 这正是"拖好的高度过几秒自己弹回去"的原因。只有容器尺寸真的变了才重新适配。
  if (panelsUserResized) {
    applyPanelHeightsAbsolute(reason)
    return
  }
  const currentTotal = changesHeight.value + stagedHeight.value + commitsHeight.value
  const ratios = currentTotal > 0
    ? [
        changesHeight.value / currentTotal,
        stagedHeight.value / currentTotal,
        commitsHeight.value / currentTotal,
      ]
    : [0.3, 0.3, 0.4]
  applyPanelHeightRatios(ratios, reason)
}

/**
 * 按"三段加起来的绝对高度 = 可用高度"来适配，保持用户设定的各段数值不变。
 *
 * 用户拖过分隔条之后，三段高度就是他的意图本身；此时容器尺寸变化只能做两件事：
 * 把多出来的高度补给最下面一段，或者按最小高度从下往上收。绝不能按比例缩放，
 * 否则窗口一动，用户刚调好的比例又漂了。
 */
const applyPanelHeightsAbsolute = (reason = 'panel-resize-absolute') => {
  if (!panelsContainer.value) return
  updatePanelHeaderHeights()
  const availableHeight = getPanelsAvailableHeight()
  if (availableHeight <= 0) return

  const minimums = [
    Math.max(changesHeaderHeight.value, 32),
    Math.max(stagedHeaderHeight.value, 32),
    Math.max(commitsHeaderHeight.value, 96),
  ]
  const heights = [changesHeight.value, stagedHeight.value, commitsHeight.value]
  const total = heights.reduce((sum, value) => sum + value, 0)

  if (total < availableHeight) {
    // 有富余：全部给最后一段（提交历史），符合"列表越长越该占满"的直觉。
    heights[2] += availableHeight - total
  } else if (total > availableHeight) {
    let overflow = total - availableHeight
    for (let i = 2; i >= 0 && overflow > 0; i -= 1) {
      const room = Math.max(0, heights[i] - minimums[i])
      const take = Math.min(room, overflow)
      heights[i] -= take
      overflow -= take
    }
  }

  changesHeight.value = Math.floor(Math.max(minimums[0], heights[0]))
  stagedHeight.value = Math.floor(Math.max(minimums[1], heights[1]))
  commitsHeight.value = Math.max(
    minimums[2],
    Math.round(availableHeight - changesHeight.value - stagedHeight.value)
  )

  frontendLogger.debug('[GitPanel] Panel heights fitted to container', {
    reason,
    availableHeight,
    changesHeight: changesHeight.value,
    stagedHeight: stagedHeight.value,
    commitsHeight: commitsHeight.value,
  })
}

const waitForGitStatusRefreshIdle = async (reason = 'git-refresh') => {
  // 设计说明：文件监听和用户点击可能同时触发刷新。若显式 Git 操作刚完成时
  // 直接碰到 isRefreshingStatus，就会被 refreshGitStatus 的并发保护短路，导致
  // 提交历史 head marker、push/pull 图标位置仍停留在旧 DOM 状态。
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!isRefreshingStatus.value) return
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  frontendLogger.warn('[GitPanel] Timed out waiting for previous Git refresh to finish', { reason })
}

const refreshGitAfterOperation = async ({
  reason = 'git-operation',
  resetSelection = false,
  includeCommits = true,
  refreshBranches = false
} = {}) => {
  if (refreshDebounceTimerId) {
    clearTimeout(refreshDebounceTimerId)
    refreshDebounceTimerId = null
  }

  frontendLogger.info('[GitPanel] Refreshing after Git operation', {
    reason,
    resetSelection,
    includeCommits,
    refreshBranches
  })

  await waitForGitStatusRefreshIdle(reason)
  await refreshGitStatus({
    resetSelection,
    includeCommits,
    forceCommits: includeCommits
  })

  // ahead/behind、stash 数、合并状态都由 sync state 提供，每次操作后一并刷新。
  await fetchGitSyncState()

  if (refreshBranches) {
    await fetchBranches()
  }
  if (showStashModal.value) {
    try {
      stashes.value = await invoke('git_stash_list', { path: props.workspacePath })
    } catch {}
  }

  await nextTick()
  normalizePanelHeightsToContainer(`git-operation-${reason}`)

  if (diffEditor) {
    diffEditor.layout()
  }

  frontendLogger.info('[GitPanel] Git operation refresh completed', {
    reason,
    commits: commits.value.length,
    staged: stagedFiles.value.length,
    unstaged: unstagedFiles.value.length,
    localHead: headMarkers.value.local_head,
    remoteHead: headMarkers.value.remote_head
  })
}

// ───── 三个分区之间的分隔条 ─────
//
// 语义：拖动「更改 / 已暂存」之间的分隔条只调整这两段；拖动「已暂存 / 提交历史」
// 之间的分隔条调整后两段。两段相加的高度保持不变，所以下方内容不会突然被顶出视口。
// 三层的最小高度都以「标题栏高度」为准——再小就只剩一条看不出来是谁的空白。

const MIN_COMMITS_HEIGHT = 96

/** 一次纵向拖动：在 [锚点上方, 锚点下方] 两段之间重新分配高度。 */
const startSectionResize = (event, { topKey, bottomKey, topMin, bottomMin, reason, onStart, onFinish }) => {
  const heights = {
    changes: changesHeight.value,
    staged: stagedHeight.value,
    commits: commitsHeight.value,
  }
  // 两段之和在整次拖动中恒定，这是「拖动不会把其他分区挤得跳来跳去」的关键。
  const pairTotal = heights[topKey] + heights[bottomKey]
  const startTop = heights[topKey]

  // 拖动期间要挡住 ResizeObserver / 自动刷新发起的高度重算，否则它们会和手指抢控制权。
  onStart?.()

  beginPointerDrag(event, {
    cursor: 'row-resize',
    onMove: ({ dy }) => {
      const nextTop = clamp(startTop + dy, topMin, Math.max(topMin, pairTotal - bottomMin))
      heights[topKey] = nextTop
      heights[bottomKey] = pairTotal - nextTop
      changesHeight.value = heights.changes
      stagedHeight.value = heights.staged
      commitsHeight.value = heights.commits
    },
    onEnd: () => {
      panelsUserResized = true
      // 收尾时对齐到可用高度，避免 1~2px 的累积误差在底部留一条缝。
      applyPanelHeightsAbsolute(`${reason}-drag-end`)
      toggleSectionVisibility(changesSection.value, changesHeaderHeight.value)
      toggleSectionVisibility(stagedSection.value, stagedHeaderHeight.value)
      writeStoredLayout()
      // 复位必须发生在"对齐到可用高度"之后，否则 ResizeObserver 会抢先按比例重算。
      onFinish?.()
    },
  })
}

const startChangesDrag = (event) => {
  panelsHeight.value = panelsContainer.value?.offsetHeight || 0
  updatePanelHeaderHeights()
  startSectionResize(event, {
    topKey: 'changes',
    bottomKey: 'staged',
    topMin: Math.max(changesHeaderHeight.value, 32),
    bottomMin: Math.max(stagedHeaderHeight.value, 32),
    reason: 'changes-divider',
    onStart: () => { isDraggingChanges.value = true },
    onFinish: () => { isDraggingChanges.value = false },
  })
}

const startCommitsDrag = (event) => {
  panelsHeight.value = panelsContainer.value?.offsetHeight || 0
  updatePanelHeaderHeights()
  // 分隔条上方是「已暂存」、下方是「提交历史」，所以往上拖必须让下方变大：
  // topKey 必须是 staged。写成 commits 会让偏移量符号反过来，拖上去反而往下走。
  startSectionResize(event, {
    topKey: 'staged',
    bottomKey: 'commits',
    topMin: Math.max(stagedHeaderHeight.value, 32),
    bottomMin: MIN_COMMITS_HEIGHT,
    reason: 'commits-divider',
    onStart: () => { isDraggingCommits.value = true },
    onFinish: () => { isDraggingCommits.value = false },
  })
}

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

const refreshGitStatus = async ({ resetSelection = true, includeCommits = true, forceCommits = false } = {}) => {
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
    const newConflictedFiles = []

    // 根据后端返回的 staged / conflicted 字段正确分类文件
    statuses.forEach(file => {
      if (isIgnoredGitPath(file?.path)) return

      const fileData = {
        path: file.path,
        status: file.status,
        diff: file.diff || '',
        staged: file.staged, // 使用后端返回的 staged 状态
        conflicted: !!file.conflicted,
        // 重命名/移动时后端会给出移动前的旧路径（orig_path）。保留下来，
        // 差异视图据此读取“旧路径的原始内容”，避免把移动当成整文件删除。
        origPath: file.orig_path || file.origPath || null
      }

      if (fileData.conflicted) {
        newConflictedFiles.push(fileData)
      } else if (fileData.staged) {
        newStagedFiles.push(fileData)
      } else {
        newUnstagedFiles.push(fileData)
      }
    })

    conflictedFiles.value = newConflictedFiles
    stagedFiles.value = newStagedFiles
    unstagedFiles.value = newUnstagedFiles

    if (includeCommits) {
      await loadCommitHistory({ force: forceCommits })
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
        const all = [...conflictedFiles.value, ...stagedFiles.value, ...unstagedFiles.value]
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

const loadCommitHistory = async ({ force = false } = {}) => {
  if (commitHistoryRefreshPromise) {
    frontendLogger.info('[GitPanel] Waiting for in-flight commit history refresh', { force })
    await commitHistoryRefreshPromise
    if (!force) return
    frontendLogger.info('[GitPanel] Reloading commit history after in-flight refresh for Git operation')
  }

  commitHistoryLoading.value = true

  commitHistoryRefreshPromise = (async () => {
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
      // 不请求逐提交的 diff 统计：那会在 80 个提交上跑 80 次全树比对，而它只用于悬停提示，
      // 需要时由长列表的 tooltip 自己补（见 commitStatsCache）。
      commits.value = await invoke('get_git_log', {
        path: props.workspacePath,
        branchName: branch || currentBranch.value,
        limit: 80,
        withStats: false,
        // 泳道图需要跨分支的拓扑：只走当前分支的话，合并进来的那条线永远是直的。
        allBranches: true
      })
      commitHistoryVisibleLimit.value = 60

      headMarkers.value = markers
      logger.debug('Commit history loaded', { count: commits.value.length, visible: commitHistoryVisibleLimit.value })
    } catch (error) {
      logger.error('Failed to load commit history', error)
    } finally {
      commitHistoryLoading.value = false
      commitHistoryRefreshPromise = null
    }
  })()

  return commitHistoryRefreshPromise
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
    case 'U': return 'warning'
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

// 只取文件名部分，供列表主标题显示（与 VSCode 一致：文件名醒目、目录淡显）。
const getFileBaseName = (filePath) => {
  const normalized = String(filePath || '').replace(/\\/g, '/')
  const segments = normalized.split('/')
  return segments[segments.length - 1] || normalized
}

// 目录部分（不含文件名）。VSCode 在文件名右侧用淡色显示所属目录。
const getFileDirName = (filePath) => {
  const normalized = String(filePath || '').replace(/\\/g, '/')
  const idx = normalized.lastIndexOf('/')
  return idx > 0 ? normalized.slice(0, idx) : ''
}

// 重命名/移动条目的副标题：显示“旧路径 → 新目录”，让移动去向一目了然。
const getRenameHint = (file) => {
  if (normalizeGitStatus(file?.status) !== 'R' || !file?.origPath) return ''
  const oldPath = String(file.origPath).replace(/\\/g, '/')
  const newDir = getFileDirName(file.path)
  return newDir ? `${oldPath} → ${newDir}` : `${oldPath} →`
}

// 列表项的完整 tooltip / 无障碍标题。
const getFileDisplayTitle = (file) => {
  if (normalizeGitStatus(file?.status) === 'R' && file?.origPath) {
    return `${String(file.origPath).replace(/\\/g, '/')} → ${String(file.path).replace(/\\/g, '/')}`
  }
  return String(file?.path || '').replace(/\\/g, '/')
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

    // 设计说明：重命名/移动时，原始（左）侧必须读“移动前旧路径”的内容，
    // 修改（右）侧读“移动后新路径”的内容。否则旧路径在新版本里已不存在，
    // 后端读到空字符串，Monaco 就把整文件渲染成“全部删除”。
    const status = normalizeGitStatus(file.status)
    const originalPath = (status === 'R' && file.origPath)
      ? normalizeDiffFilePath(file.origPath)
      : filePath

    const originalRevision = isStagedDiff ? 'HEAD' : 'INDEX'
    const modifiedRevision = isStagedDiff ? 'INDEX' : null
    frontendLogger.info('[Diff] Loading diff for:', {
      filePath,
      originalPath,
      staged: isStagedDiff,
      originalRevision,
      modifiedRevision: modifiedRevision || 'WORKTREE',
      context: context.key,
      requestId
    });

    // 设计说明：部分暂存后，同一文件会在“更改”和“已暂存”各出现一条。
    // 已暂存视图展示 HEAD -> INDEX；未暂存视图展示 INDEX -> 工作区，
    // 这样胶囊按钮拿到的 hunk 范围才和当前面板一致。
    //
    // 性能：以前这里并发两个 get_file_content，之后再去 get_file_diff_hunks 拉一次
    // （还会 spawn 一个 git 子进程）。三次调用各自 open 一次仓库、各自 write_tree 一遍索引，
    // 大仓库里点一个文件就要等一下。现在合成一次调用，后端只算一次索引树。
    const payload = await invoke('get_git_file_diff', {
      path: props.workspacePath,
      filePath,
      staged: isStagedDiff,
      // 重命名时左侧要读旧路径，否则旧路径在新树里不存在，会显示成整文件删除。
      origPath: originalPath !== filePath ? originalPath : null
    })

    const originalContent = String(payload?.original ?? '')
    const modifiedContent = String(payload?.modified ?? '')

    const countLines = (value) => String(value || '').split(/\r\n|\r|\n/).length
    frontendLogger.info('[Diff] Loaded file contents for Monaco diff', {
      filePath,
      originalPath,
      staged: isStagedDiff,
      originalRevision,
      modifiedRevision: modifiedRevision || 'WORKTREE',
      originalBytes: String(originalContent || '').length,
      modifiedBytes: String(modifiedContent || '').length,
      originalLines: countLines(originalContent),
      modifiedLines: countLines(modifiedContent),
      status,
      context: context.key,
      requestId
    })
    if (status === 'M' && !String(modifiedContent || '').length) {
      frontendLogger.warn('[Diff] Modified file content is empty for a modified entry; check backend workdir path resolution', {
        filePath,
        workspacePath: props.workspacePath,
        context: context.key,
        requestId
      })
    }

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

    // hunk 已随内容一起返回，直接落缓存；不再为此单独发一次请求。
    diffGitHunksCache = Array.isArray(payload?.hunks) ? payload.hunks.map(toMonacoLikeGitHunk) : []
    diffGitHunksCacheContext = context
    frontendLogger.info('[Diff] Git hunk cache filled from combined payload', {
      context: context.key,
      hunks: diffGitHunksCache.length
    })
    scheduleHunkActionPositionUpdateAfterLayout('git-hunk-cache-loaded')

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

    // 语言优先用后端给的建议值：它和文件扩展名一一对应，省掉一次本地映射的边界情况。
    const language = payload?.language || getLanguageForFile(file.path);

    // 性能参数必须在 setModel 之前应用：Monaco 一旦拿到新模型就会立刻算 diff，
    // 放到后面等于先用最贵的算法算一遍、再按新参数重算，大文件上正好慢一倍。
    const diffPerf = getDiffPerformanceOptions(originalContent, modifiedContent)
    diffRenderSideBySide.value = !!diffPerf.options.renderSideBySide
    diffEditor.updateOptions(diffPerf.options);

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

    if (diffPerf.largeDiff) {
      frontendLogger.info('[Diff] Diff rendered in performance mode', {
        filePath,
        level: diffPerf.level,
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
  if (file?.conflicted) {
    openMergeEditor(file)
    return
  }
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
      filePath: file.path,
      // 重命名/移动时把旧路径一并传给后端，暂存新路径的同时移除旧路径的幽灵删除记录。
      origPath: (normalizeGitStatus(file.status) === 'R' && file.origPath) ? file.origPath : null
    })

    // 4. 更新文件状态为已完成
    const stagedIndex = stagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (stagedIndex !== -1) {
      stagedFiles.value[stagedIndex].__pending = false
    }

    await refreshGitAfterOperation({
      reason: 'stage-file',
      resetSelection: false,
      includeCommits: true
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
      filePath: file.path,
      // 取消暂存重命名时，旧路径也要恢复回 HEAD，否则旧文件会凭空消失。
      origPath: (normalizeGitStatus(file.status) === 'R' && file.origPath) ? file.origPath : null
    })

    // 4. 更新状态
    const unstagedIndex = unstagedFiles.value.findIndex(f => f.path === file.path && f.__pending)
    if (unstagedIndex !== -1) {
      unstagedFiles.value[unstagedIndex].__pending = false
    }

    await refreshGitAfterOperation({
      reason: 'unstage-file',
      resetSelection: false,
      includeCommits: true
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
    await refreshGitAfterOperation({
      reason: 'stage-all',
      resetSelection: true,
      includeCommits: true
    })
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
    await refreshGitAfterOperation({
      reason: 'unstage-all',
      resetSelection: true,
      includeCommits: true
    })
  } catch (error) {
    logger.error('Failed to unstage all files', error)
  }
}

const handleDiscardFile = async (file) => {
  const status = normalizeGitStatus(file?.status)
  const confirmed = await confirmDialog({
    title: '丢弃更改',
    content: status === '?'
      ? `将删除未跟踪的文件 ${file.path}，此操作不能撤销。`
      : `将把 ${file.path} 恢复到 HEAD 的状态，未提交的修改会丢失。`,
    positiveText: '丢弃更改',
  })
  if (!confirmed) return

  logger.debug('Discarding file changes', { file: file.path })
  try {
    await invoke('discard_file', {
      path: props.workspacePath,
      filePath: file.path,
      // 撤销重命名时恢复旧路径并删除移动后的新文件。
      origPath: (normalizeGitStatus(file.status) === 'R' && file.origPath) ? file.origPath : null
    })
    logger.info('File changes discarded', { file: file.path })
    await refreshGitAfterOperation({
      reason: 'discard-file',
      resetSelection: true,
      includeCommits: true
    })
  } catch (error) {
    logger.error('Failed to discard file changes', { file: file.path, error })
    notification.error({
      title: '丢弃更改失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const handleDiscardAll = async () => {
  const confirmed = await confirmDialog({
    title: '丢弃全部更改',
    content: '所有未暂存的工作区改动都会被还原，此操作不能撤销。',
    positiveText: '全部丢弃',
  })
  if (!confirmed) return

  logger.debug('Discarding all changes')
  try {
    await invoke('discard_all', {
      path: props.workspacePath
    })
    logger.info('All changes discarded')
    await refreshGitAfterOperation({
      reason: 'discard-all',
      resetSelection: true,
      includeCommits: true
    })
  } catch (error) {
    logger.error('Failed to discard all changes', error)
    notification.error({
      title: '丢弃全部失败',
      content: String(error?.message || error),
      duration: 4000,
    })
  }
}

const handleCommit = async ({ all = false, amend = false, push = false } = {}) => {
  const message = commitForm.value.message.trim()
  if (!message) {
    notification.warning({ title: '请先填写提交信息', duration: 2000 })
    return
  }
  if (conflictedFiles.value.length > 0) {
    notification.warning({
      title: '存在未解决的冲突',
      content: '请先在「合并更改」中解决冲突再提交',
      duration: 3000,
    })
    return
  }

  gitLoading.value.commit = true
  try {
    await invoke('git_commit', {
      path: props.workspacePath,
      message,
      amend: amend || null,
      all: all || null,
    })
    commitForm.value.message = ''
    await refreshGitAfterOperation({
      reason: amend ? 'amend-commit' : 'commit',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true
    })
    notification.success({
      title: amend ? '已修订上次提交' : '提交成功',
      content: message.split('\n')[0],
      duration: 2000,
    })
    if (push) {
      await handlePush()
    }
  } catch (error) {
    frontendLogger.error('Commit failed', error)
    notification.error({
      title: '提交失败',
      content: String(error?.message || error),
      duration: 5000,
    })
  } finally {
    gitLoading.value.commit = false
  }
}

const handleCommitKeydown = (e) => {
  if (!e) return
  if (e.ctrlKey && e.key === 'Enter') {
    e.preventDefault()
    if (!canCommit.value) return
    handleCommit()
  }
}

const handlePull = async ({ rebase = false } = {}) => {
  logger.debug('Pulling changes', { path: props.workspacePath, rebase })
  gitLoading.value.pull = true
  try {
    await invoke('git_pull', { path: props.workspacePath, rebase })
    logger.info('Pull successful')
    notification.success({
      title: '拉取成功',
      content: currentBranch.value ? `已更新 ${currentBranch.value}` : '已更新本地分支',
      duration: 2000
    })
    await refreshGitAfterOperation({
      reason: 'pull',
      resetSelection: true,
      includeCommits: true,
      refreshBranches: true
    })
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

const handlePush = async ({ force = false } = {}) => {
  if (force) {
    const confirmed = await confirmDialog({
      title: '强制推送',
      content: '将使用 --force-with-lease 推送；如果远端有你尚未拉取的提交，推送会被拒绝。',
      positiveText: '强制推送',
    })
    if (!confirmed) return
  }
  logger.debug('Pushing changes', { path: props.workspacePath, force })
  gitLoading.value.push = true
  try {
    await invoke('git_push', { path: props.workspacePath, force })
    logger.info('Push successful')
    notification.success({
      title: '推送成功',
      content: currentBranch.value ? `已推送 ${currentBranch.value}` : '已推送到远端',
      duration: 2000
    })
    await refreshGitAfterOperation({
      reason: 'push',
      resetSelection: false,
      includeCommits: true,
      refreshBranches: true
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
  window.addEventListener('keydown', handleDiffNavigationShortcut, true)

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
      refreshGitAfterOperation({
        reason: 'interval-refresh',
        resetSelection: false,
        includeCommits: false,
        refreshBranches: false
      })
    }, gitAutoRefresh.value.intervalMs)

    if (autoRefreshFocusHandler) {
      window.removeEventListener('focus', autoRefreshFocusHandler)
    }
    autoRefreshFocusHandler = () => scheduleAutoRefresh()
    window.addEventListener('focus', autoRefreshFocusHandler)

    if (autoRefreshVisibilityHandler) {
      document.removeEventListener('visibilitychange', autoRefreshVisibilityHandler)
    }
    autoRefreshVisibilityHandler = () => {
      if (document.visibilityState === 'visible') scheduleAutoRefresh()
    }
    document.addEventListener('visibilitychange', autoRefreshVisibilityHandler)
  }

  // 初始化操作
  const initialize = async () => {
    try {
      await setupAutoRefresh()
      await Promise.all([
        refreshGitStatus(),
        fetchBranches(),
        fetchGitSyncState()
      ])

      // 初始化编辑器但不等待
      initMonacoEditor().catch(frontendLogger.error)

      // 设置面板高度
      nextTick(() => {
        if (changesSection.value && stagedSection.value && commitsSection.value && panelsContainer.value) {
          // 上次手动拖过就沿用存下来的数值，否则按默认比例摊开。
          const restored = restoreLayoutFromStorage()
          if (restored && panelsUserResized) {
            applyPanelHeightsAbsolute('initial-panel-layout-restored')
          } else {
            applyPanelHeightRatios([0.3, 0.3, 0.4], 'initial-panel-layout')
          }
          toggleSectionVisibility(changesSection.value, changesHeaderHeight.value)
          toggleSectionVisibility(stagedSection.value, stagedHeaderHeight.value)
          if (!panelsResizeObserver && typeof ResizeObserver !== 'undefined') {
            panelsResizeObserver = new ResizeObserver(() => {
              normalizePanelHeightsToContainer('panels-container-resize')
            })
            panelsResizeObserver.observe(panelsContainer.value)
          }
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
  if (autoRefreshFocusHandler) {
    window.removeEventListener('focus', autoRefreshFocusHandler)
    autoRefreshFocusHandler = null
  }
  if (autoRefreshVisibilityHandler) {
    document.removeEventListener('visibilitychange', autoRefreshVisibilityHandler)
    autoRefreshVisibilityHandler = null
  }
  window.removeEventListener('keydown', handleDiffNavigationShortcut, true)

  // 拖动用的监听器都挂在分隔条元素自己身上（pointer capture），组件卸载时元素一并销毁，
  // 这里只需要把可能残留的全局 class 清掉。
  document.body.classList.remove('git-panel-resizing')

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
  if (panelsResizeObserver) {
    panelsResizeObserver.disconnect()
    panelsResizeObserver = null
  }
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
  /* 行高 22px 与提交泳道图对齐；左右内边距让图标不贴边。 */
  min-height: 22px;
  padding: 0 12px;
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
  color: #1a7f37;
}

.git-status-o {
  color: var(--text-color-secondary);
}

:root[data-guaxi-theme='dark'] .git-status-a {
  color: #3fb950;
}

:root[data-guaxi-theme='dark'] .git-status-u {
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

/* 非当前分支的引用（其它本地分支 / 远端分支 / 标签）：保持低调，
   只比正文略深一点，避免一屏里出现七八个抢眼的胶囊。 */
.commit-marker-other {
  background-color: color-mix(in srgb, var(--text-color-secondary) 10%, transparent);
  border-color: color-mix(in srgb, var(--divider-color) 70%, transparent);
}

.commit-marker-other .commit-marker-pill-icon {
  color: var(--text-color-secondary);
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

/* VSCode 风格：文件名醒目，目录/重命名提示淡显在同一行右侧。 */
.git-file-label {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.git-file-name {
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
}

.git-file-dir,
.git-file-rename {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 0.82em;
  opacity: 0.6;
  color: var(--text-color-secondary);
}

.git-file-rename {
  font-style: italic;
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

/* 差异高亮样式
   设计说明：这里是 Monaco DOM 装饰的兜底颜色。整行背景必须足够透明，
   避免和注释、行号、选区叠加后降低可读性；字符级背景稍强用于精确识别变更。 */
.gitkraken-container .monaco-editor .line-insert {
  background-color: rgba(40, 167, 69, 0.06);
}

.gitkraken-container .monaco-editor .line-delete {
  background-color: rgba(203, 36, 49, 0.06);
}

.gitkraken-container .monaco-editor .char-insert {
  background-color: rgba(40, 167, 69, 0.16);
}

.gitkraken-container .monaco-editor .char-delete {
  background-color: rgba(203, 36, 49, 0.16);
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
  padding: 6px 12px 8px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

.scm-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 24px;
}

/* 标题用和 VS Code 侧栏一致的字重与字号：13px / 600，不用更粗的字重，
   否则整条侧栏会显得头重脚轻。 */
.scm-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.scm-header-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: 0 0 auto;
}

.scm-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-color-secondary);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.scm-icon-button:hover:not(:disabled) {
  background-color: var(--hover-color);
  color: var(--text-color);
}

.scm-icon-button:disabled {
  opacity: 0.5;
  cursor: default;
}

.scm-header-sub {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 2px;
  min-width: 0;
  font-size: 11.5px;
  color: var(--text-color-secondary);
}

.scm-branch {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.scm-branch-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 仓库名靠右，和分支拉开；只在空间够时才显示完整，否则同样省略。 */
.scm-repo {
  margin-left: auto;
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.8;
}

.commit-section {
  padding: 8px 12px 10px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

/* 提交信息输入框：去掉 Naive 默认的底部外边距，间距交给这里的 flex gap 管。 */
.commit-message-input {
  display: block;
  margin-bottom: 6px;
}

.commit-message-input :deep(.n-input__textarea-el) {
  font-size: 12.5px;
  line-height: 1.6;
}

/* 提交按钮：VS Code 的观感——整宽、实心主色、左侧带一个对勾图标。
   禁用时降到中性灰，而不是保持蓝色只降透明度（后者看起来像还能点）。 */
.commit-button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  height: 26px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: 4px;
  background-color: var(--primary-color, #0e639c);
  color: #ffffff;
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
  transition: background-color 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}

.commit-button:hover:not(:disabled) {
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 82%, #ffffff);
}

.commit-button:active:not(:disabled) {
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 88%, #000000);
}

.commit-button:disabled {
  background-color: color-mix(in srgb, var(--text-color-secondary) 14%, transparent);
  color: var(--text-color-secondary);
  border-color: var(--divider-color);
  cursor: default;
}

.commit-button-count {
  padding: 0 5px;
  border-radius: 8px;
  background-color: rgba(255, 255, 255, 0.18);
  font-size: 11px;
  line-height: 16px;
}

.commit-button:disabled .commit-button-count {
  background-color: color-mix(in srgb, var(--text-color-secondary) 16%, transparent);
}

/* 设计说明：此前 .commit-history-spin 与内部 .n-spin-container 都设了 min-height:100%。
   但 .commit-history-spin 自身高度是 auto（不定高），其子级的 100% 百分比无法解析，
   于是在内容不足/展开提交导致布局变化时会留下一段无法消除的“死区”空白。改为让内容随
   .n-spin-content 自然撑开、由外层 .commits-list（flex:1; overflow-y:auto）负责滚动，
   既能占满又不会出现底部空白。 */
.commit-history-spin {
  flex: 1 0 auto;
  min-height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
}

/* 设计说明：n-spin 的根 DOM 同时拥有 .commit-history-spin 与 .n-spin-container。
   必须直接命中这个根节点，不能只写后代选择器，否则展开提交文件列表时根节点高度仍按
   auto 回流，commits-list 的滚动高度会短暂失真并在底部留下空白。 */
.commit-history-spin.n-spin-container {
  min-height: 100%;
  display: flex;
  flex-direction: column;
}

.commit-history-spin :deep(.n-spin-content) {
  flex: 1 0 auto;
  min-height: 100%;
  display: flex;
  flex-direction: column;
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
  background-color: var(--card-bg);
  border-top: 1px solid var(--divider-color);
}

/* 与 JS 的 MIN_COMMITS_HEIGHT 保持一致：硬编码更大的值会让拖到下限后
   分隔条停住、但相邻分区还在继续变高，面板总高就溢出容器。 */
.commits-section {
  min-height: 96px;
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
  display: flex;
  flex-direction: column;
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
  /* 行高与泳道图的行高一致（COMMIT_ROW_HEIGHT = 22px）：
     泳道图的连线从行上边界画到下边界，行高一旦对不上，折线在相邻行之间就会断开。 */
  padding: 0 12px;
  min-height: 22px;
  cursor: pointer;
  transition: background-color 0.12s ease;
  border-left: 2px solid transparent;
}

.commit-item:hover {
  background-color: var(--hover-color);
}

/* 选中行用和 VS Code 一致的左侧色条 + 淡蓝底，比整块变色更容易在第一眼定位。 */
.commit-item.selected-commit {
  background-color: color-mix(in srgb, #4a9eff 14%, transparent);
  border-left-color: #4a9eff;
}

/* 泳道图本身。
   连线用 currentColor 之外的显式颜色；1.5px 描边在 100% 缩放下清晰，
   在高 DPI 上也不会糊成一片。shape-rendering 保持几何精度，避免细线抖动。 */
.commit-graph {
  flex: 0 0 auto;
  display: block;
  margin-right: 6px;
  overflow: visible;
  shape-rendering: geometricPrecision;
}

.commit-graph-line {
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.commit-graph-dot {
  stroke: var(--card-bg);
  stroke-width: 1.5;
}

.commit-row {
  align-items: center;
  gap: 6px;
  /* 行高由 .commit-item 的 min-height 决定，这里只负责垂直居中。
     左右内边距由 .commit-item 提供；.git-file-row 也有一份，必须在这里压回 0，
     否则提交行会比文件行多缩进 12px，和泳道图对不齐。 */
  min-height: 22px;
  padding-left: 0;
  padding-right: 0;
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



/* 分隔条。
 *
 * 旧版是 2px 高的实心线、事件还要求 e.target 正好命中它，鼠标稍快就脱靶，
 * 表现成“时而生效时而失效”。参照 VS Code 的做法：本体是一条视觉细线，
 * 但用一个绝对定位的透明伪元素把命中区撑到 9px（上下各外扩 4px），
 * 视觉不变、手感稳得多。 */
.section-divider,
.commits-divider {
  position: relative;
  flex: 0 0 auto;
  height: 1px;
  background-color: var(--divider-color);
  cursor: row-resize;
  z-index: 10;
  transition: background-color 0.12s ease;
}

.section-divider::before,
.commits-divider::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: -4px;
  bottom: -4px;
  /* 命中区是透明的，但必须能接收指针事件 */
  background: transparent;
}

.section-divider:hover,
.commits-divider:hover,
.section-divider.dragging,
.commits-divider.dragging {
  background-color: var(--primary-color, #4a9eff);
}

/* 拖动期间要压住 Monaco / 文本的选中，否则鼠标划过会把右侧内容整片选蓝。 */
body.git-panel-resizing,
body.git-panel-resizing * {
  cursor: row-resize !important;
  user-select: none !important;
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

/* 侧栏宽度拖动条。
   和纵向分隔条同样的思路：细线居中，命中区用伪元素外扩到 8px，
   否则 4px 的热区在快速拖动时很容易一滑就脱靶。 */
.sidebar-resize-handle {
  position: absolute;
  right: -2px;
  top: 0;
  bottom: 0;
  width: 4px;
  cursor: col-resize;
  z-index: 20;
  background: transparent;
  transition: background-color 0.12s ease;
}

.sidebar-resize-handle::before {
  content: '';
  position: absolute;
  left: -3px;
  right: -3px;
  top: 0;
  bottom: 0;
  background: transparent;
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
  padding: 0;
}

/* 文件行本身负责左右内边距与行高；n-list-item 只当容器用。
   以前这里写死 `padding: 4px 12px`，再叠上 .git-file-row 的间距，
   一屏里能塞下的文件数比 VS Code 少将近三分之一。 */
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

/* 差异高亮样式：仅作用于 Git 面板，避免覆盖 Notebook/主编辑器中的 Monaco 实例。 */
.gitkraken-container .monaco-editor .char-delete {
  background-color: rgba(203, 36, 49, 0.16) !important;
}

.gitkraken-container .monaco-editor .char-insert {
  background-color: rgba(40, 167, 69, 0.16) !important;
}

.gitkraken-container .monaco-editor .line-delete {
  background-color: rgba(203, 36, 49, 0.06) !important;
}

.gitkraken-container .monaco-editor .line-insert {
  background-color: rgba(40, 167, 69, 0.06) !important;
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

/* ───── VS Code 风格 SCM 头部 / 分支选择器 ───── */

.scm-branch-button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  max-width: 100%;
  padding: 1px 4px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-color);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
}

.scm-branch-button:hover {
  background-color: var(--hover-color);
}

.scm-branch-caret {
  font-size: 12px;
  opacity: 0.7;
}

.scm-sync-button {
  margin-left: 4px;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  flex: 0 0 auto;
  padding: 1px 5px;
  border: 1px solid var(--divider-color);
  border-radius: 10px;
  background: transparent;
  color: var(--text-color-secondary);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.scm-sync-button:hover:not(:disabled) {
  background-color: var(--hover-color);
  color: var(--text-color);
}

.scm-sync-button:disabled {
  opacity: 0.5;
  cursor: default;
}

.scm-sync-count {
  font-variant-numeric: tabular-nums;
}

.scm-merge-banner {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  padding: 4px 6px;
  border-radius: 4px;
  background-color: color-mix(in srgb, #d29922 12%, transparent);
  color: #d29922;
  font-size: 11.5px;
}

.scm-merge-banner button {
  margin-left: auto;
  width: 20px;
  height: 20px;
  border: 0;
  border-radius: 3px;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.scm-merge-banner button:hover {
  background-color: var(--hover-color);
}

.branch-picker {
  display: flex;
  flex-direction: column;
  max-height: 420px;
  padding: 8px;
  gap: 6px;
  background-color: var(--card-bg);
  color: var(--text-color);
}

.branch-picker-search :deep(.n-input) {
  font-size: 12px;
}

.branch-picker-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.branch-group-label {
  padding: 6px 6px 3px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-color-secondary);
}

.branch-row {
  display: flex;
  align-items: center;
  gap: 4px;
  border-radius: 4px;
  min-height: 24px;
}

.branch-row:hover {
  background-color: var(--hover-color);
}

.branch-row.is-current {
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 12%, transparent);
}

.branch-row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 6px;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}

.branch-row-icon {
  flex: 0 0 auto;
  color: var(--text-color-secondary);
}

.branch-row-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.branch-row-count {
  flex: 0 0 auto;
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  color: var(--text-color-secondary);
}

.branch-row-check {
  flex: 0 0 auto;
  color: #2da44e;
}

.branch-row-actions {
  display: none;
  align-items: center;
  flex: 0 0 auto;
  padding-right: 3px;
}

.branch-row:hover .branch-row-actions {
  display: inline-flex;
}

.branch-row-actions button {
  width: 21px;
  height: 21px;
  border: 0;
  border-radius: 3px;
  background: transparent;
  color: var(--text-color-secondary);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.branch-row-actions button:hover {
  background-color: color-mix(in srgb, var(--text-color-secondary) 18%, transparent);
  color: var(--text-color);
}

.branch-picker-empty {
  padding: 14px 8px;
  font-size: 12px;
  color: var(--text-color-secondary);
  text-align: center;
}

.branch-picker-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 6px;
  border-top: 1px solid var(--divider-color);
}

.branch-picker-footer button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 6px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-color);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.branch-picker-footer button:hover {
  background-color: var(--hover-color);
}

/* ───── 提交按钮拆分 / 冲突提示 ───── */

.commit-actions {
  display: flex;
  align-items: stretch;
  gap: 0;
}

.commit-actions .commit-button {
  width: auto;
  flex: 1 1 auto;
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}

.commit-button-caret {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 26px;
  border: 1px solid transparent;
  border-left: 1px solid rgba(255, 255, 255, 0.22);
  border-radius: 0 4px 4px 0;
  background-color: var(--primary-color, #0e639c);
  color: #ffffff;
  cursor: pointer;
}

.commit-button-caret:hover {
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 82%, #ffffff);
}

.commit-conflict-hint {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 6px;
  font-size: 11.5px;
  color: #d29922;
}

/* ───── 合并更改分组 ───── */

.conflicts-section {
  flex: 0 0 auto;
  max-height: 34%;
  min-height: 64px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-top: 1px solid var(--divider-color);
  background-color: var(--card-bg);
}

.conflict-list {
  max-height: 220px;
}

.conflict-row {
  cursor: pointer;
}

.conflict-row:hover {
  background-color: var(--hover-color);
}

.git-status-conflict {
  color: #d29922;
}

/* ───── 树形列表 / 状态字母 ───── */

.git-folder-row {
  display: flex;
  align-items: center;
  gap: 5px;
  min-height: 22px;
  padding-right: 8px;
  font-size: 12px;
  color: var(--text-color-secondary);
  cursor: pointer;
  user-select: none;
}

.git-folder-row:hover {
  background-color: var(--hover-color);
}

.git-folder-chevron {
  font-size: 12px;
  opacity: 0.85;
}

.git-folder-icon {
  font-size: 14px;
  color: color-mix(in srgb, #4a9eff 75%, var(--text-color-secondary));
}

.git-folder-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.git-folder-count {
  margin-left: auto;
  font-size: 10.5px;
  opacity: 0.65;
}

.git-file-icon {
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
}

.git-file-row {
  cursor: pointer;
  padding-right: 8px;
}

.git-file-row:hover {
  background-color: var(--hover-color);
}

.git-file-actions {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.1s ease;
}

.git-file-row:hover .git-file-actions,
.git-file-row:focus-within .git-file-actions,
.git-file-actions:has(button:disabled) {
  opacity: 1;
}

.git-file-status {
  flex: 0 0 14px;
  min-width: 14px;
  text-align: center;
  font-size: 11px;
  font-weight: 700;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

.git-list-empty {
  padding: 12px 10px;
  font-size: 12px;
  color: var(--text-color-secondary);
}

.section-branch-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 130px;
  padding: 2px 6px;
  border: 1px solid var(--divider-color);
  border-radius: 10px;
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 10%, transparent);
  color: var(--text-color);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.section-branch-chip span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.section-branch-chip:hover {
  background-color: color-mix(in srgb, var(--primary-color, #0e639c) 18%, transparent);
}

/* ───── 贮藏 ───── */

.stash-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 50vh;
  overflow: auto;
}

.stash-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 6px;
  border-radius: 4px;
}

.stash-row:hover {
  background-color: var(--hover-color);
}

.stash-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.stash-selector {
  flex: 0 0 auto;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11.5px;
  color: var(--text-color-secondary);
}

.stash-message {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}

.stash-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}
</style>
