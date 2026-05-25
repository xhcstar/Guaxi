<template>
  <div class="editor-container guaxi-editor">
    <div class="editor-header" ref="editorHeaderRef">
      <div class="tab-bar" ref="tabBarRef" @wheel="handleWheelScroll">
        <div class="tab-region tab-region-left" ref="tabRegionLeftRef">
          <div v-for="tab in leftTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTabLeft === tab.path, 'dirty': !!tab.dirty }]"
            @click="switchTab(tab.path, 'left')" @mousedown.middle="closeTab(tab.path, 'left')">
            <span class="tab-title">{{ tab.fileName }}</span>
            <span class="tab-affordance">
              <span class="tab-dirty-dot" v-show="tab.dirty" />
              <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path, 'left')">
                <CloseOutline />
              </n-icon>
            </span>
          </div>
        </div>
        <div v-if="layoutMode !== 'single'" class="tab-region tab-region-right" ref="tabRegionRightRef">
          <div v-for="tab in rightTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTabRight === tab.path, 'dirty': !!tab.dirty }]"
            @click="switchTab(tab.path, 'right')" @mousedown.middle="closeTab(tab.path, 'right')">
            <span class="tab-title">{{ tab.fileName }}</span>
            <span class="tab-affordance">
              <span class="tab-dirty-dot" v-show="tab.dirty" />
              <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path, 'right')">
                <CloseOutline />
              </n-icon>
            </span>
          </div>
        </div>
      </div>


      <div class="toolbar">
        <div v-if="previewMode !== 'realtime' && editorSurfaceMode !== 'notebook'" class="toolbar-editing-actions">
          <n-dropdown trigger="click" :options="headingOptions" @select="handleHeadingSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text size="tiny">
                  <Icon :icon="headingIcon" width="16" height="16" />
                </n-button>
              </template>
              标题
            </n-tooltip>
          </n-dropdown>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBold">
                <Icon :icon="boldIcon" width="16" height="16" />
              </n-button>
            </template>
            加粗 (Ctrl+B)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleItalic">
                <Icon :icon="italicIcon" width="16" height="16" />
              </n-button>
            </template>
            斜体 (Ctrl+I)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleStrikethrough">
                <Icon :icon="strikethroughIcon" width="16" height="16" />
              </n-button>
            </template>
            删除线 (Ctrl+Shift+X)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertLink">
                <Icon :icon="linkIcon" width="16" height="16" />
              </n-button>
            </template>
            链接 (Ctrl+K)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBlockquote">
                <Icon :icon="quoteIcon" width="16" height="16" />
              </n-button>
            </template>
            引用
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleInlineCode">
                <Icon :icon="codeIcon" width="16" height="16" />
              </n-button>
            </template>
            行内代码
          </n-tooltip>
          <n-dropdown trigger="click" :options="codeBlockOptions" @select="handleCodeBlockSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text size="tiny">
                  <Icon :icon="squareTerminalIcon" width="16" height="16" />
                </n-button>
              </template>
              代码块
            </n-tooltip>
          </n-dropdown>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertPythonCellBlock">
                <Icon :icon="fileCode2Icon" width="16" height="16" />
              </n-button>
            </template>
            插入 Python Cell
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleUnorderedList">
                <Icon :icon="listIcon" width="16" height="16" />
              </n-button>
            </template>
            无序列表
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleOrderedList">
                <Icon :icon="listOrderedIcon" width="16" height="16" />
              </n-button>
            </template>
            有序列表
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleTaskList">
                <Icon :icon="listChecksIcon" width="16" height="16" />
              </n-button>
            </template>
            任务列表
          </n-tooltip>

          <n-popover trigger="click" placement="bottom-start" :show="tableInsertPopoverVisible"
            @update:show="handleTableInsertPopoverUpdateShow">
            <template #trigger>
              <n-tooltip placement="top" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny">
                    <Icon :icon="tableIcon" width="16" height="16" />
                  </n-button>
                </template>
                表格 (Ctrl+Alt+T)
              </n-tooltip>
            </template>

            <div class="table-insert-popover" @mousedown.stop>
              <div class="table-insert-hint">{{ tableInsertHint }}</div>
              <div class="table-insert-grid" @mouseup="handleTableGridMouseUp" @mouseleave="handleTableGridMouseLeave">
                <template v-for="r in tableGridMaxRows" :key="`r-${r}`">
                  <template v-for="c in tableGridMaxCols" :key="`c-${r}-${c}`">
                    <div class="table-insert-cell" :class="{ active: r <= tableGridRows && c <= tableGridCols }"
                      @mousedown.prevent="handleTableGridMouseDown(r, c)" @mouseover="handleTableGridMouseOver(r, c)" />
                  </template>
                </template>
              </div>

              <div class="table-insert-controls">
                <div class="table-insert-inputs">
                  <n-input-number v-model:value="tableInputRows" size="tiny" :min="2" :max="tableInsertMaxRows"
                    :show-button="false" placeholder="行" />
                  <span class="table-insert-mul">×</span>
                  <n-input-number v-model:value="tableInputCols" size="tiny" :min="1" :max="tableInsertMaxCols"
                    :show-button="false" placeholder="列" />
                  <n-button size="tiny" type="primary" @click="insertTableFromInputs">插入</n-button>
                </div>
              </div>
            </div>
          </n-popover>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertImage">
                <Icon :icon="imageIcon" width="16" height="16" />
              </n-button>
            </template>
            图片
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertHorizontalRule">
                <Icon :icon="minusIcon" width="16" height="16" />
              </n-button>
            </template>
            分隔线
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="undo">
                <Icon :icon="undoIcon" width="16" height="16" />
              </n-button>
            </template>
            撤销
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="redo">
                <Icon :icon="redoIcon" width="16" height="16" />
              </n-button>
            </template>
            重做
          </n-tooltip>
        </div>
        <div class="toolbar-spacer"></div>
        <div class="toolbar-actions">
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="toggleVimMode">
                <n-icon :component="vimMode ? LogoVimeo : Edit16Filled" width="16" height="16" />
              </n-button>
            </template>
            {{ vimMode ? '禁用Vim模式' : '启用Vim模式' }}
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="saveContent">
                <n-icon :component="SaveOutline" size="18" />
              </n-button>
            </template>
            Save (Ctrl+S)
          </n-tooltip>

          <n-popover v-if="false" class="toolbar-action-python" trigger="click" placement="bottom" :show="pythonRuntimePopoverVisible"
            @update:show="handlePythonRuntimePopoverUpdateShow">
            <template #trigger>
              <button type="button" class="python-runtime-entry" :class="{ busy: pythonRuntimeBusy }">
                <Icon :icon="pythonIcon" width="16" height="16" />
                <span class="python-runtime-entry__dot" :class="pythonRuntimeStatusClass"></span>
              </button>
            </template>

            <div class="python-runtime-popover" @mousedown.stop>
              <div class="python-runtime-popover__header">
                <div>
                  <div class="python-runtime-popover__title">Python 工作台</div>
                  <div class="python-runtime-popover__subtitle">{{ pythonRuntimeSummary }}</div>
                </div>
                <button type="button" class="python-runtime-ghost-btn" @click="refreshPythonRuntimeStatus('popover-refresh')">
                  刷新
                </button>
              </div>

              <div class="python-runtime-meta">
                <div class="python-runtime-meta__item">
                  <span class="python-runtime-meta__label">Guaxi Python</span>
                  <span class="python-runtime-meta__value">{{ pythonRuntimeStatus?.version || '未就绪' }}</span>
                </div>
                <div class="python-runtime-meta__item">
                  <span class="python-runtime-meta__label">运行时状态</span>
                  <span class="python-runtime-meta__value">{{ pythonRuntimeRuntimeLabel }}</span>
                </div>
                <div class="python-runtime-meta__item">
                  <span class="python-runtime-meta__label">包管理</span>
                  <span class="python-runtime-meta__value">{{ pythonRuntimeStatus?.pip_version || '未检测' }}</span>
                </div>
                <div class="python-runtime-meta__item">
                  <span class="python-runtime-meta__label">Worker</span>
                  <span class="python-runtime-meta__value">{{ pythonRuntimeStatus?.worker_alive ? '在线' : '待启动' }}</span>
                </div>
              </div>

              <div class="python-runtime-guide" :class="{ 'needs-setup': pythonRuntimeNeedsSetup }">
                <div class="python-runtime-guide__title">
                  {{ pythonRuntimeNeedsSetup ? '首次使用引导' : '运行时状态' }}
                </div>
                <div class="python-runtime-guide__text">
                  <template v-if="pythonRuntimeNeedsSetup">
                    推荐先导入 Guaxi 专属 Python 运行时压缩包，再执行初始化。初始化完成后，这套环境会作为软件内统一 Python 基础设施供各模块复用。
                  </template>
                  <template v-else>
                    当前显示的是 Guaxi 自己管理的 Python 配置，可直接用于 Python Cell、数据表格导入导出与后续模块复用。
                  </template>
                </div>
                <div class="python-runtime-guide__paths">
                  <div class="python-runtime-guide__path">
                    <span class="python-runtime-guide__path-label">运行时目录</span>
                    <code class="python-runtime-guide__path-value">{{ pythonRuntimeStatus?.runtime_dir || 'utils/python_runtime' }}</code>
                  </div>
                  <div class="python-runtime-guide__path">
                    <span class="python-runtime-guide__path-label">初始化标记</span>
                    <code class="python-runtime-guide__path-value">{{ pythonRuntimeStatus?.runtime_marker_path || 'runtime.json' }}</code>
                  </div>
                </div>
                <div class="python-runtime-guide__steps">
                  <span class="python-runtime-guide__step" :class="{ done: pythonRuntimeStatus?.bundled_runtime_present }">1. 导入压缩包</span>
                  <span class="python-runtime-guide__step" :class="{ done: pythonRuntimeStatus?.initialized }">2. 初始化环境</span>
                  <span class="python-runtime-guide__step" :class="{ done: pythonRuntimeStatus?.pip_version }">3. Pip 就绪</span>
                  <span class="python-runtime-guide__step" :class="{ done: pythonRuntimeStatus?.worker_alive }">4. Worker 在线</span>
                </div>
              </div>

              <div v-if="pythonRuntimeArchiveInspection" class="python-runtime-archive-card">
                <div class="python-runtime-archive-card__title">待导入压缩包</div>
                <div class="python-runtime-archive-card__path">{{ pythonRuntimeArchivePath }}</div>
                <div class="python-runtime-archive-card__meta">
                  <span>入口：{{ pythonRuntimeArchiveInspection?.python_entry_found ? '已检测到' : '未检测到' }}</span>
                  <span>候选数：{{ pythonRuntimeArchiveInspection?.candidate_count || 0 }}</span>
                </div>
                <div v-if="pythonRuntimeArchiveInspection?.warnings?.length" class="python-runtime-archive-card__warnings">
                  <span v-for="warning in pythonRuntimeArchiveInspection.warnings" :key="warning">{{ warning }}</span>
                </div>
              </div>

              <div class="python-runtime-actions">
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="initializePythonRuntime">
                  初始化环境
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="installPythonRuntimeArchive">
                  导入运行时压缩包
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="insertPythonTableImportSnippet">
                  导入表格代码
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="resetPythonKernelSession">
                  重启会话
                </button>
              </div>

              <div class="python-runtime-install">
                <input v-model="pythonRuntimePackageInput" class="python-runtime-install__input"
                  placeholder="输入包名，例如 pandas matplotlib openpyxl" />
                <button type="button" class="python-runtime-install__btn"
                  :disabled="pythonRuntimeBusy || !pythonRuntimePackageInput.trim()" @click="installPythonRuntimePackages">
                  安装
                </button>
              </div>

              <div class="python-runtime-packages">
                <div class="python-runtime-packages__title">推荐扩展包</div>
                <div class="python-runtime-packages__list">
                  <button
                    v-for="pkg in PYTHON_RUNTIME_RECOMMENDED_PACKAGES"
                    :key="pkg.value"
                    type="button"
                    class="python-runtime-package-pill python-runtime-package-pill--button"
                    :disabled="pythonRuntimeBusy"
                    :title="pkg.description"
                    @click="installRecommendedPythonPackage(pkg.value)"
                  >
                    {{ pkg.label }}
                  </button>
                </div>
              </div>

              <div v-if="pythonRuntimeStatus?.packages?.length" class="python-runtime-packages">
                <div class="python-runtime-packages__title">已检测到的包</div>
                <div class="python-runtime-packages__list">
                  <span v-for="pkg in pythonRuntimeStatus.packages" :key="pkg" class="python-runtime-package-pill">{{ pkg }}</span>
                </div>
              </div>
            </div>
          </n-popover>

          <n-dropdown trigger="click" :options="previewModeOptions" @select="handlePreviewModeSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text>
                  <Icon :icon="previewModeIcon" width="16" height="16" />
                </n-button>
              </template>
              {{ previewModeLabel }} (Ctrl+1/2/3/4/5)
            </n-tooltip>
          </n-dropdown>

          <n-dropdown trigger="click" :options="exportOptions" @select="handleExportSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text>
                  <n-icon :component="ExitOutline" size="18" />
                </n-button>
              </template>
              导出
            </n-tooltip>
          </n-dropdown>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="openMarkdownManual">
                <Icon :icon="bookOpenIcon" width="16" height="16" />
              </n-button>
            </template>
            Markdown语法查询手册
          </n-tooltip>
          <div ref="pythonRuntimeAnchorRef" class="python-runtime-anchor">
            <button
              type="button"
              class="python-runtime-entry"
              :class="{ busy: pythonRuntimeBusy, active: pythonRuntimePopoverVisible }"
              @click="togglePythonRuntimeWorkbench"
              title="Python 工作台"
            >
              <Icon :icon="pythonIcon" width="16" height="16" />
                <span class="python-runtime-entry__dot" :class="pythonRuntimeStatusClass"></span>
              </button>

            <button
              type="button"
              class="python-runtime-entry python-runtime-entry--console"
              :class="{ active: pythonConsoleVisible }"
              @click="togglePythonConsolePanel('toolbar-toggle-console')"
              title="终端 (Ctrl+`)"
            >
              <Icon :icon="squareTerminalIcon" width="14" height="14" />
              <span class="python-runtime-entry__label">Term</span>
            </button>

            <transition name="python-runtime-panel-fade">
              <div
                v-if="pythonRuntimePopoverVisible"
                ref="pythonRuntimePanelRef"
                class="python-runtime-workbench python-runtime-popover python-runtime-panel"
                :class="{ 'is-dark': settingsStore.appearanceTheme === 'dark' }"
                @mousedown.stop
              >
              <div class="python-runtime-panel__header">
                <div>
                  <div class="python-runtime-panel__title">Python 工作台</div>
                  <div class="python-runtime-panel__subtitle">{{ pythonRuntimeDisplaySummary }}</div>
                </div>
                <button type="button" class="python-runtime-ghost-btn" @click="refreshPythonRuntimeStatus('popover-refresh')">
                  刷新
                </button>
              </div>

              <div class="python-runtime-status-card" :class="pythonRuntimeStatusTone">
                <div class="python-runtime-status-card__row">
                  <span class="python-runtime-status-card__label">状态</span>
                  <span class="python-runtime-status-card__value">{{ pythonRuntimeDisplayStatus }}</span>
                </div>
                <div class="python-runtime-status-card__row">
                  <span class="python-runtime-status-card__label">版本</span>
                  <span class="python-runtime-status-card__value">{{ pythonRuntimeStatus?.version || '未就绪' }}</span>
                </div>
                <div class="python-runtime-status-card__row">
                  <span class="python-runtime-status-card__label">会话</span>
                  <span class="python-runtime-status-card__value">{{ pythonRuntimeStatus?.worker_alive ? '已连接' : '未启动' }}</span>
                </div>
              </div>

              <div v-if="pythonRuntimeOperation.active" class="python-runtime-operation-card">
                <div class="python-runtime-operation-card__title">包管理任务进行中</div>
                <div class="python-runtime-operation-card__subtitle">
                  {{ pythonRuntimeOperation.stage || '正在执行后台任务' }}
                </div>
                <div class="python-runtime-operation-card__packages">
                  {{ pythonRuntimeOperationSummary }}
                </div>
                <div class="python-runtime-operation-card__progress" aria-hidden="true">
                  <span class="python-runtime-operation-card__progress-bar"></span>
                </div>
              </div>

              <div class="python-runtime-panel__actions">
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="resetPythonKernelSession">
                  重启会话
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="installPythonRuntimeArchive">
                  导入运行时包
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="upgradeManagedPip">
                  升级 Pip
                </button>
              </div>

              <div v-if="pythonRuntimeArchiveInspection" class="python-runtime-archive-card">
                <div class="python-runtime-archive-card__title">运行时包检查结果</div>
                <div class="python-runtime-archive-card__meta">
                  <span>解释器：{{ pythonRuntimeArchiveInspection?.python_entry_found ? '已检测到' : '未检测到' }}</span>
                  <span>候选数：{{ pythonRuntimeArchiveInspection?.candidate_count || 0 }}</span>
                </div>
                <div v-if="pythonRuntimeArchiveInspection?.warnings?.length" class="python-runtime-archive-card__warnings">
                  <span v-for="warning in pythonRuntimeArchiveInspection.warnings" :key="warning">{{ warning }}</span>
                </div>
              </div>

              <div class="python-runtime-install-card">
                <div class="python-runtime-section__title">安装 Python 包</div>
                <div class="python-runtime-install">
                  <input v-model="pythonRuntimePackageInput" class="python-runtime-install__input"
                    placeholder="输入包名，例如 pandas matplotlib openpyxl" />
                  <button type="button" class="python-runtime-install__btn"
                    :disabled="pythonRuntimeBusy || !pythonRuntimePackageInput.trim()" @click="installPythonRuntimePackages">
                    安装
                  </button>
                </div>
                <div class="python-runtime-packages__list python-runtime-packages__list--compact">
                  <button
                    v-for="pkg in PYTHON_RUNTIME_RECOMMENDED_PACKAGES"
                    :key="pkg.value"
                    type="button"
                    class="python-runtime-package-pill python-runtime-package-pill--button"
                    :disabled="pythonRuntimeBusy"
                    :title="pkg.description"
                    @click="installRecommendedPythonPackage(pkg.value)"
                  >
                    {{ pkg.label }}
                  </button>
                </div>
              </div>

              <div class="python-runtime-package-browser">
                <div class="python-runtime-package-browser__header">
                  <div class="python-runtime-section__title">已安装的包</div>
                  <input v-model="pythonRuntimePackageSearch" class="python-runtime-package-browser__search"
                    placeholder="搜索已安装包" />
                </div>
                <div v-if="pythonRuntimeSelectedPackageCount" class="python-runtime-package-browser__bulkbar">
                  <span class="python-runtime-package-browser__bulktext">
                    已选择 {{ pythonRuntimeSelectedPackageCount }} 个包
                  </span>
                  <div class="python-runtime-package-browser__bulkactions">
                    <button
                      type="button"
                      class="python-runtime-package-row__action"
                      :disabled="pythonRuntimeBusy"
                      @click="manageSelectedPythonPackages('upgrade')"
                    >
                      批量升级
                    </button>
                    <button
                      type="button"
                      class="python-runtime-package-row__action is-danger"
                      :disabled="pythonRuntimeBusy"
                      @click="manageSelectedPythonPackages('uninstall')"
                    >
                      批量卸载
                    </button>
                    <button
                      type="button"
                      class="python-runtime-package-row__action"
                      :disabled="pythonRuntimeBusy"
                      @click="clearPythonRuntimePackageSelection"
                    >
                      取消选择
                    </button>
                  </div>
                </div>
                <div class="python-runtime-package-browser__list">
                  <div v-if="pythonRuntimePackagesFiltered.length" class="python-runtime-package-browser__items">
                    <div v-for="pkg in pythonRuntimePackagesFiltered" :key="pkg" class="python-runtime-package-row">
                      <label class="python-runtime-package-row__selector">
                        <input
                          type="checkbox"
                          :checked="pythonRuntimeSelectedPackages.includes(pkg)"
                          :disabled="pythonRuntimeBusy"
                          @change="togglePythonRuntimePackageSelection(pkg)"
                        />
                      </label>
                      <span class="python-runtime-package-row__name">{{ pkg }}</span>
                      <div class="python-runtime-package-row__actions">
                        <n-tooltip placement="top" trigger="hover">
                          <template #trigger>
                            <button
                              type="button"
                              class="python-runtime-package-row__action python-runtime-package-row__action--icon"
                              :disabled="pythonRuntimeBusy"
                              @click="managePythonPackage(pkg, 'upgrade')"
                            >
                              <Icon :icon="arrowUpCircleIcon" width="14" height="14" />
                            </button>
                          </template>
                          升级包
                        </n-tooltip>
                        <n-tooltip placement="top" trigger="hover">
                          <template #trigger>
                            <button
                              type="button"
                              class="python-runtime-package-row__action python-runtime-package-row__action--icon"
                              :disabled="pythonRuntimeBusy"
                              @click="managePythonPackage(pkg, 'reinstall')"
                            >
                              <Icon :icon="refreshCwIcon" width="14" height="14" />
                            </button>
                          </template>
                          重装包
                        </n-tooltip>
                        <n-tooltip placement="top" trigger="hover">
                          <template #trigger>
                            <button
                              type="button"
                              class="python-runtime-package-row__action python-runtime-package-row__action--icon is-danger"
                              :disabled="pythonRuntimeBusy"
                              @click="managePythonPackage(pkg, 'uninstall')"
                            >
                              <Icon :icon="trash2Icon" width="14" height="14" />
                            </button>
                          </template>
                          卸载包
                        </n-tooltip>
                      </div>
                    </div>
                  </div>
                  <div v-else class="python-runtime-package-browser__empty">
                    {{ pythonRuntimeStatus?.packages?.length ? '没有匹配的包' : '暂无已安装包信息' }}
                  </div>
                </div>
                <details
                  v-if="pythonRuntimePackageActionLog.visible"
                  class="python-runtime-package-log"
                  open
                >
                  <summary class="python-runtime-package-log__summary">
                    <span>{{ pythonRuntimePackageActionLog.title }}</span>
                    <button
                      type="button"
                      class="python-runtime-package-row__action"
                      :disabled="pythonRuntimeBusy"
                      @click.prevent="closePythonRuntimePackageActionLog"
                    >
                      关闭日志
                    </button>
                  </summary>
                  <div class="python-runtime-package-log__body">
                    <div class="python-runtime-package-log__section">
                      <div class="python-runtime-package-log__label">stdout</div>
                      <pre class="python-runtime-package-log__output">{{ pythonRuntimePackageActionLog.stdout || '(无输出)' }}</pre>
                    </div>
                    <div class="python-runtime-package-log__section">
                      <div class="python-runtime-package-log__label">stderr</div>
                      <pre class="python-runtime-package-log__output is-error">{{ pythonRuntimePackageActionLog.stderr || '(无输出)' }}</pre>
                    </div>
                  </div>
                </details>
              </div>
              </div>
            </transition>
          </div>
        </div>
      </div>
    </div>

    <div class="editor-content" :class="previewMode">
      <div v-if="showEditorEmptyState" class="editor-empty">请选择一个文件以开始编辑</div>
      <div v-show="selectedFile && previewMode === 'realtime' && canPreview" id="realtime-vditor"
        class="realtime-vditor">
      </div>
      <div v-show="previewMode !== 'realtime' && showPreview" class="preview-shell" ref="previewShellRef">
        <div v-show="previewSearchVisible" class="preview-search-panel" @mousedown.stop>
          <input
            ref="previewSearchInputRef"
            v-model="previewSearchKeyword"
            class="preview-search-input"
            type="text"
            placeholder="搜索预览内容..."
            @keydown.enter.prevent="focusNextPreviewSearchMatch"
            @keydown.shift.enter.prevent="focusPrevPreviewSearchMatch"
          />
          <span class="preview-search-count">{{ previewSearchMatchCountText }}</span>
          <button class="preview-search-btn" type="button" @click="focusPrevPreviewSearchMatch">上一个</button>
          <button class="preview-search-btn" type="button" @click="focusNextPreviewSearchMatch">下一个</button>
          <button class="preview-search-btn preview-search-close" type="button" @click="closePreviewSearch">关闭</button>
        </div>
        <div v-html="renderedHtml" class="preview" ref="previewContainer" @click="handlePreviewClick"
          @mouseup="handlePreviewSelectionMouseUp"></div>
        <div v-show="previewFloatingToolbarVisible" class="floating-md-toolbar" :style="previewFloatingToolbarStyle"
          @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionTranslateBusy" @click="openSelectionTranslatePanel">
                <Icon :icon="languagesIcon" width="16" height="16" />
              </n-button>
            </template>
            翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="askSelectionWithAssistant">
                <Icon :icon="messageCircleIcon" width="16" height="16" />
              </n-button>
            </template>
            问AI
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="openAssistantTranslateDialog">
                <Icon :icon="botIcon" width="16" height="16" />
              </n-button>
            </template>
            用AI翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button
                text
                size="tiny"
                class="floating-toolbar-icon-button"
                :loading="selectionReadAloudBusy"
                :disabled="selectionReadAloudBusy"
                @click="readSelectionAloud"
              >
                <Icon v-if="!selectionReadAloudBusy" :icon="volume2Icon" width="16" height="16" />
              </n-button>
            </template>
            {{ selectionReadAloudBusy ? '朗读中...' : '朗读' }}
          </n-tooltip>
        </div>
      </div>
      <div
        v-show="showEditorShell"
        class="editor-wrapper"
        ref="editorWrapperRef"
      >
        <div v-if="editorSurfaceMode === 'notebook' && canUseNotebookSurface" class="notebook-surface">
          <GuaxiNotebookView
            ref="notebookViewRef"
            :markdown="rawContent"
            :file-path="selectedFile?.path || ''"
            :theme="settingsStore.appearanceTheme"
            @update:markdown="applyNotebookMarkdown"
            @outline-change="handleNotebookOutlineChange"
            @state-change="syncAllPreviewPythonCells"
            @open-console="togglePythonConsolePanel('notebook-open-console')"
            @request-save="saveContent"
          />
        </div>
        <template v-if="editorSurfaceMode !== 'notebook' || !canUseNotebookSurface">
        <div v-show="tableToolbarVisible" class="floating-table-toolbar" :style="tableToolbarStyle" @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="addTableRowBelow">
                <Icon :icon="tableRowPlusAfter" width="16" height="16" />
              </n-button>
            </template>
            新增行 (Ctrl+Alt+↓)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableRow">
                <Icon :icon="tableRowRemove" width="16" height="16" />
              </n-button>
            </template>
            删除行 (Ctrl+Alt+Shift+↓)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="addTableColumnRight">
                <Icon :icon="tableColumnPlusAfter" width="16" height="16" />
              </n-button>
            </template>
            新增列 (Ctrl+Alt+→)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableColumn">
                <Icon :icon="tableColumnRemove" width="16" height="16" />
              </n-button>
            </template>
            删除列 (Ctrl+Alt+Shift+→)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('left')">
                <Icon :icon="alignLeftIcon" width="16" height="16" />
              </n-button>
            </template>
            左对齐
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('center')">
                <Icon :icon="alignCenterIcon" width="16" height="16" />
              </n-button>
            </template>
            居中
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('right')">
                <Icon :icon="alignRightIcon" width="16" height="16" />
              </n-button>
            </template>
            右对齐
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="formatTable">
                <Icon :icon="sparklesIcon" width="16" height="16" />
              </n-button>
            </template>
            格式化
          </n-tooltip>
        </div>
        <div v-show="floatingToolbarVisible" class="floating-md-toolbar" :style="floatingToolbarStyle"
          @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBold">
                <Icon :icon="boldIcon" width="16" height="16" />
              </n-button>
            </template>
            加粗 (Ctrl+B)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleItalic">
                <Icon :icon="italicIcon" width="16" height="16" />
              </n-button>
            </template>
            斜体 (Ctrl+I)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleStrikethrough">
                <Icon :icon="strikethroughIcon" width="16" height="16" />
              </n-button>
            </template>
            删除线 (Ctrl+Shift+X)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertLink">
                <Icon :icon="linkIcon" width="16" height="16" />
              </n-button>
            </template>
            链接 (Ctrl+K)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleInlineCode">
                <Icon :icon="codeIcon" width="16" height="16" />
              </n-button>
            </template>
            行内代码
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBlockquote">
                <Icon :icon="quoteIcon" width="16" height="16" />
              </n-button>
            </template>
            引用
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionTranslateBusy" @click="openSelectionTranslatePanel">
                <Icon :icon="languagesIcon" width="16" height="16" />
              </n-button>
            </template>
            翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="askSelectionWithAssistant">
                <Icon :icon="messageCircleIcon" width="16" height="16" />
              </n-button>
            </template>
            问AI
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="openAssistantTranslateDialog">
                <Icon :icon="botIcon" width="16" height="16" />
              </n-button>
            </template>
            用AI翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button
                text
                size="tiny"
                class="floating-toolbar-icon-button"
                :loading="selectionReadAloudBusy"
                :disabled="selectionReadAloudBusy"
                @click="readSelectionAloud"
              >
                <Icon v-if="!selectionReadAloudBusy" :icon="volume2Icon" width="16" height="16" />
              </n-button>
            </template>
            {{ selectionReadAloudBusy ? '朗读中...' : '朗读' }}
          </n-tooltip>
        </div>

        <div class="breadcrumb-bar">
          <div ref="breadcrumbScrollRef" class="breadcrumb-scroll" @wheel.prevent="handleBreadcrumbWheel">
            <n-breadcrumb separator="›">
              <n-breadcrumb-item v-for="it in breadcrumbItems" :key="it.key">
                <span class="breadcrumb-item" :class="{ disabled: !!it.disabled }" :title="it.title"
                  @click.stop="openBreadcrumbPicker(it, $event)">
                  <Icon v-if="it.icon" class="breadcrumb-icon" :icon="it.icon" width="14" height="14" />
                  <span class="breadcrumb-label">{{ it.label }}</span>
                </span>
              </n-breadcrumb-item>
            </n-breadcrumb>
          </div>

          <n-dropdown v-model:show="breadcrumbPickerVisible" trigger="manual" placement="bottom-start"
            :x="breadcrumbPickerPosition.x" :y="breadcrumbPickerPosition.y" :options="breadcrumbPickerOptions"
            @select="handleBreadcrumbPickerSelect" @clickoutside="closeBreadcrumbPicker" />
        </div>

        <n-modal v-model:show="assistantTranslateDialogVisible" preset="card" title="用AI翻译" style="width: 420px">
          <div class="assistant-translate-dialog-row">
            <Icon :icon="languagesIcon" width="16" height="16" />
            <span>选择目标语言</span>
          </div>
          <n-select
            v-model:value="assistantTranslateTargetLang"
            size="small"
            :options="assistantTranslateLangOptions"
          />
          <div class="assistant-translate-dialog-actions">
            <n-button size="small" @click="assistantTranslateDialogVisible = false">取消</n-button>
            <n-button size="small" type="primary" @click="sendSelectionAsAiTranslate">发送并翻译</n-button>
          </div>
        </n-modal>

        <div
          class="editor-split-wrapper"
          :class="[layoutMode, { 'vim-enabled': vimMode, 'with-python-console': pythonConsoleVisible }]"
          :style="pythonConsoleVisible ? { paddingBottom: `${pythonConsoleHeight + pythonConsoleBottomOffset}px` } : undefined"
        >
          <div ref="editorContainer" class="editor-container" @mousedown="activateRegion('left')">
            <!-- 编辑器内容将在这里渲染 -->
          </div>
          <div v-if="layoutMode !== 'single'" ref="secondaryEditorContainer" class="editor-container"
            @mousedown="activateRegion('right')">
          </div>
        </div>

        <n-dropdown v-model:show="editorContextMenuVisible" :options="editorContextMenuOptions" placement="bottom-start"
          :x="editorContextMenuPosition.x" :y="editorContextMenuPosition.y" @select="handleEditorContextMenuSelect"
          @clickoutside="handleEditorContextMenuClickoutside" />

        </template>

        <div v-if="vimMode" class="vim-status-bar" ref="vimStatusBar"></div>
        <GuaxiEditorStatusBar class="editor-status-bar" />
      </div>

      <transition name="python-console-slide">
        <div
          v-if="pythonConsoleVisible"
          class="python-console-dock"
          :class="{ busy: pythonConsoleBusy, dragging: pythonConsoleResizing, 'terminal-only': !selectedFile }"
          :style="{ height: `${pythonConsoleHeight}px`, bottom: `${pythonConsoleBottomOffset}px` }"
        >
          <div
            class="python-console-dock__resize-handle"
            title="拖拽调整控制台高度"
            @mousedown.prevent="startResizePythonConsole"
          >
            <span class="python-console-dock__resize-grip"></span>
          </div>
          <GuaxiTerminalPanel
            ref="terminalPanelRef"
            :visible="pythonConsoleVisible"
            :context-path="getTerminalContextPath(fileTreeStore.activeRegion || 'left')"
            :defer-resize-sync="pythonConsoleResizing"
            @close="togglePythonConsolePanel('panel-collapse')"
          />
          <div v-if="false" class="python-console-panel" :class="{ busy: pythonConsoleBusy }">
            <div class="python-console-panel__header">
              <div>
                <div class="python-console-panel__title">Python 控制台</div>
                <div class="python-console-panel__meta">
                  {{ pythonSessionInfo?.sandbox_dir ? `沙箱目录：${pythonSessionInfo.sandbox_dir}` : '当前文件共享一个独立 Python 会话与沙箱目录' }}
                </div>
              </div>
              <div class="python-console-panel__actions">
                <button type="button" class="python-console-panel__btn" @click="runPythonConsoleInput">执行</button>
                <button type="button" class="python-console-panel__btn" @click="resetCurrentPythonSession">重置会话</button>
                <button type="button" class="python-console-panel__btn" @click="toggleMaximizePythonConsole">
                  {{ pythonConsoleMaximized ? '还原' : '最大化' }}
                </button>
                <button type="button" class="python-console-panel__btn" @click="togglePythonConsolePanel('panel-collapse')">收起</button>
              </div>
            </div>

            <div ref="pythonConsoleOutputRef" class="python-console-panel__output">
              <div v-if="!pythonConsoleHistory.length && !pythonConsolePinnedOutput" class="python-console-panel__empty">
                使用 `Ctrl+\`` 打开或关闭控制台，支持逐条执行 Python / IPython 风格命令。
              </div>
              <div v-for="item in pythonConsoleHistory" :key="item.id" class="python-console-panel__entry">
                <div class="python-console-panel__prompt">In [ ]:</div>
                <pre class="python-console-panel__code">{{ item.input }}</pre>
                <div class="python-console-panel__prompt python-console-panel__prompt--out">Out:</div>
                <pre class="python-console-panel__result">{{ item.output }}</pre>
              </div>
              <pre v-if="pythonConsolePinnedOutput" class="python-console-panel__result python-console-panel__result--pinned">{{ pythonConsolePinnedOutput }}</pre>
            </div>

            <textarea
              v-model="pythonConsoleInput"
              class="python-console-panel__input"
              spellcheck="false"
              placeholder="输入 Python / IPython 风格命令，按 Ctrl+Enter 执行"
              @keydown.ctrl.enter.prevent="runPythonConsoleInput"
            />
          </div>
        </div>
      </transition>

      <GuaxiSelectionTranslatePanel v-if="selectionTranslatePanelVisible" :pos="selectionTranslatePanelPos"
        :raw="selectionTranslateRaw" :result="selectionTranslateResult" :error="selectionTranslateError"
        :busy="selectionTranslateBusy" :pinned="selectionTranslatePinned" :items="selectionTranslateItems"
        :word-meta="selectionTranslateWordMeta" :from-lang="selectionTranslateFrom" :to-lang="selectionTranslateTo"
        :lang-options="selectionTranslateLangOptions"
        @update:from-lang="(v) => (selectionTranslateFrom = String(v || 'auto'))"
        @update:to-lang="(v) => (selectionTranslateTo = String(v || 'zh'))" @swap-langs="swapSelectionTranslateLangs"
        @translate="translateSelectionText" @copy="copySelectionTranslateResult"
        @toggle-pin="toggleSelectionTranslatePinned" @update:pos="handleSelectionTranslatePanelUpdatePos"
        @close="closeSelectionTranslatePanel" />

    </div>
    <!-- 添加 GitPanel 组件 -->
    <!--
    <GitPanel ref="gitPanel" :workspace-path="fileTreeStore.activeRepoPath" />
    -->
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, nextTick, provide, h } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiEditorStatusBar from './GuaxiEditorStatusBar.vue'
import GuaxiSelectionTranslatePanel from './GuaxiSelectionTranslatePanel.vue'
import GuaxiNotebookView from './GuaxiNotebookView.vue'
import GuaxiTerminalPanel from './GuaxiTerminalPanel.vue'
import { NTooltip, NIcon, useMessage, NDropdown, NPopover, NInputNumber, NBreadcrumb, NBreadcrumbItem } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { EyeOutline, LogoVimeo, CloseOutline, ExitOutline, SaveOutline, ArrowBackOutline, ArrowForwardOutline } from '@vicons/ionicons5'
import { DocumentEdit16Regular, Edit16Filled, Image28Regular } from '@vicons/fluent'
// Iconify local icon imports (lucide + mdi) to preserve original visuals without runtime network fetch
import headingIcon from '@iconify-icons/lucide/heading'
import boldIcon from '@iconify-icons/lucide/bold'
import italicIcon from '@iconify-icons/lucide/italic'
import strikethroughIcon from '@iconify-icons/lucide/strikethrough'
import linkIcon from '@iconify-icons/lucide/link'
import quoteIcon from '@iconify-icons/lucide/quote'
import codeIcon from '@iconify-icons/lucide/code'
import fileCode2Icon from '@iconify-icons/lucide/file-code-2'
import squareTerminalIcon from '@iconify-icons/lucide/terminal-square'
import pythonIcon from '@iconify-icons/vscode-icons/file-type-python'
import listIcon from '@iconify-icons/lucide/list'
import listOrderedIcon from '@iconify-icons/lucide/list-ordered'
import listChecksIcon from '@iconify-icons/lucide/list-checks'
import tableIcon from '@iconify-icons/lucide/table'
import imageIcon from '@iconify-icons/lucide/image'
import minusIcon from '@iconify-icons/lucide/minus'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import alignLeftIcon from '@iconify-icons/lucide/align-left'
import alignCenterIcon from '@iconify-icons/lucide/align-center'
import alignRightIcon from '@iconify-icons/lucide/align-right'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import filePenLineIcon from '@iconify-icons/lucide/file-edit'
import eyeIcon from '@iconify-icons/lucide/eye'
import columns2Icon from '@iconify-icons/lucide/columns'
import bookOpenIcon from '@iconify-icons/lucide/book-open'
import scanSearchIcon from '@iconify-icons/lucide/scan-search'
import languagesIcon from '@iconify-icons/lucide/languages'
import messageCircleIcon from '@iconify-icons/lucide/message-circle'
import botIcon from '@iconify-icons/lucide/bot'
import volume2Icon from '@iconify-icons/lucide/volume-2'
import arrowUpCircleIcon from '@iconify-icons/lucide/arrow-up-circle'
import refreshCwIcon from '@iconify-icons/lucide/refresh-cw'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import mermaid from 'mermaid'
import folderIcon from '@iconify-icons/lucide/folder'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import hashIcon from '@iconify-icons/lucide/hash'

import tableRowPlusAfter from '@iconify-icons/mdi/table-row-plus-after'
import tableRowRemove from '@iconify-icons/mdi/table-row-remove'
import tableColumnPlusAfter from '@iconify-icons/mdi/table-column-plus-after'
import tableColumnRemove from '@iconify-icons/mdi/table-column-remove'
import { dirname, homeDir } from '@tauri-apps/api/path'
import { openUrl } from '@tauri-apps/plugin-opener'
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { createGuaxiPythonCellEditorManager } from './utils/guaxiPythonCellEditor'
import { buildPythonCellBlock, createPythonCellId, createPythonCellStableId, createPythonSessionId, dataframeToMarkdownTable, maskPythonCellBlocksForMarkdown, parsePythonCellBlocks, transformPythonCellBlocksForPreview } from './utils/guaxiPythonCellSyntax'
import { getPythonCellSnapshot, patchPythonCellSnapshot } from './utils/guaxiPythonCellState'
import { createMarkdownPreviewSegments } from './utils/guaxiNotebookCells'
import Vditor from 'vditor'
import 'vditor/dist/index.css'
import 'vditor/dist/method'
import 'vditor/dist/js/i18n/zh_CN.js'
import * as pdfjsLib from 'pdfjs-dist/build/pdf.mjs'
import { debounce } from 'lodash-es'
import * as monaco from 'monaco-editor'
import { initVimMode } from 'monaco-vim';
import { initTextmate } from '../../monaco/textmateLoader'
import {
  initLspClient,
  disposeAllLspClients,
  getLspConnectionState,
  onLspConnectionStateChange,
  waitForLspConnection,
  resetLspDisabled,
  provideDirectLspCompletionSuggestions
} from '../../monaco/lspClient'
import {
  LSP_SUPPORTED_LANGUAGES,
  refreshLspRuntimeExtensionSnapshot,
  isLspKnownLanguage,
} from '../../monaco/lspCapabilities'
import {
  retainVsCodeTextModel,
  releaseVsCodeTextModel,
  disposeAllVsCodeTextModelsForOwner,
  replaceModelContentPreserveUndo,
} from '../../monaco/vscodeModelBridge'
// 更新所有语言导入路径
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'
import 'monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution'
import 'monaco-editor/esm/vs/basic-languages/rust/rust.contribution'
import 'monaco-editor/esm/vs/basic-languages/shell/shell.contribution'
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution'
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution'
import 'monaco-editor/esm/vs/basic-languages/java/java.contribution'
import 'monaco-editor/esm/vs/basic-languages/lua/lua.contribution'
import 'monaco-editor/esm/vs/basic-languages/ini/ini.contribution'

import 'monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js'
import 'monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js'

import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import { GitBranchOutline } from '@vicons/ionicons5'
import GitPanel from './GitPanel.vue'
import GitService from './utils/GitService'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import { save, open, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { commands as vscodeCommands, Position as VscodePosition, Uri as VscodeUri } from 'vscode'

const logger = createLogger('GuaxiEditor')
pdfjsLib.GlobalWorkerOptions.workerSrc = new URL('pdfjs-dist/build/pdf.worker.min.mjs', import.meta.url).toString()
const outlineStore = useOutlineStore()

const ensureLatexLanguage = () => {
  const exists = monaco.languages.getLanguages().some((lang) => lang.id === 'latex')
  if (!exists) {
    monaco.languages.register({
      id: 'latex',
      extensions: ['.tex', '.ltx', '.latex', '.cls', '.sty'],
      aliases: ['LaTeX', 'TeX', 'latex', 'tex'],
      mimetypes: ['text/x-tex', 'text/x-latex']
    })
  }

  // 设计说明：
  // 1. Monaco 默认没有内置 latex contribution，仅 register 语言 ID 只能让状态栏显示 LaTeX，编辑区仍近似纯文本。
  // 2. 后端 tree-sitter 语义高亮是异步增强，打开单文件 .tex 时需要先有一层稳定的前端词法高亮兜底。
  // 3. 这里覆盖常见命令、环境、注释、可选参数和数字；更细粒度语义仍交给 syntax_highlight_tokens / texlab。
  monaco.languages.setMonarchTokensProvider('latex', {
    tokenizer: {
      root: [
        [/%.*$/, 'comment'],
        [/\\(?:begin|end)(\s*)(\{)([A-Za-z*_-][\w*:-]*)(\})/, ['keyword', 'white', 'delimiter.bracket', 'type', 'delimiter.bracket']],
        [/\\(?:documentclass|usepackage|usetikzlibrary|input|include|bibliography|bibliographystyle)(?=\s*(?:\[|\{))/, 'keyword'],
        [/\\(?:section|subsection|subsubsection|paragraph|title|author|date|maketitle|tableofcontents|newpage|item|label|ref|eqref|cite|emph|textbf|textit)\b/, 'function'],
        [/\\[A-Za-z@]+\*?/, 'keyword'],
        [/\\./, 'operator'],
        [/\[[^\]]*\]/, 'attribute'],
        [/\{|\}/, 'delimiter.bracket'],
        [/\$+\s*|\s*\$+/, 'string.delimiter'],
        [/\b\d+(?:\.\d+)?(?:pt|em|ex|cm|mm|in)?\b/, 'number'],
      ]
    }
  })
  logger.info('[Editor] LaTeX language registered/configured', { languageId: 'latex', existed: exists })
}

const ensureMermaidLanguage = () => {
  const exists = monaco.languages.getLanguages().some((lang) => lang.id === 'mermaid')
  if (!exists) {
    monaco.languages.register({
      id: 'mermaid',
      extensions: ['.mmd', '.mermaid'],
      aliases: ['Mermaid', 'mermaid'],
      mimetypes: ['text/x-mermaid']
    })
  }
  monaco.languages.setMonarchTokensProvider('mermaid', {
    tokenizer: {
      root: [
        [/%%.*$/, 'comment'],
        [/\b(?:graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|stateDiagram-v2|erDiagram|journey|gantt|pie|mindmap|timeline|gitGraph|quadrantChart|requirementDiagram|zenuml|block-beta|packet-beta|xychart-beta)\b/, 'keyword'],
        [/\b(?:subgraph|end|direction|title|section|participant|actor|class|style|linkStyle|click|accTitle|accDescr|accDescrMultiline)\b/, 'keyword'],
        [/\b(?:LR|RL|TB|BT|TD)\b/, 'type'],
        [/\b(?:true|false)\b/, 'constant'],
        [/"[^"]*"/, 'string'],
        [/'[^']*'/, 'string'],
        [/\[\[.*?\]\]|\[\(.*?\)\]|\[\[|\]\]|\[\(|\)\]|\[.*?\]|\(.*?\)|\{.*?\}|\(\(.*?\)\)/, 'string'],
        [/\b\d+(?:\.\d+)?\b/, 'number'],
        [/\-\->|\-\.\->|==>|==|---|--|-.->|-.|-x|x-|o-|-\)|\|.*?\|/, 'operator'],
        [/[{}[\]()]/, 'delimiter'],
        [/[,:;]/, 'delimiter'],
        [/[A-Za-z_][\w-]*/, 'identifier']
      ]
    }
  })
  logger.info('[Editor] language registered', { languageId: 'mermaid' })
}

ensureLatexLanguage()
ensureMermaidLanguage()
// 添加 GitPanel 组件引用
const gitPanel = ref(null)


// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const settingsStore = useSettingsStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore()
const mode = ref('edit');
const previewMode = ref('toggle');
const editorSurfaceMode = ref('source')
const layoutMode = computed(() => fileTreeStore.layoutMode || 'single');
const renderedHtml = ref('');
const lastPreviewRenderedSource = ref('');
const previewSegmentRenderCache = new Map()
const previewFullRenderCache = new Map()
const previewFileCardMetaCache = new Map()
const PREVIEW_SEGMENT_CACHE_LIMIT = 160
const PREVIEW_FULL_RENDER_CACHE_LIMIT = 48
const PREVIEW_FILE_CARD_CACHE_LIMIT = 80
const PREVIEW_RENDER_DELAY_DEFAULT_MS = 300
const PREVIEW_RENDER_DELAY_PREVIEW_ONLY_MS = 160
const PREVIEW_RENDER_DELAY_SPLIT_MS = 80
let previewMarkdownWorker = null
let previewMarkdownWorkerReady = false
let previewMarkdownWorkerTaskId = 0
const previewMarkdownWorkerPendingTasks = new Map()
let previewRenderTimer = null
let previewRenderRequestId = 0
const EDITOR_CONTENT_SYNC_DELAY_MS = 0
const EDITOR_POSTPROCESS_DELAY_MS = 220
const previewContainer = ref(null);
const pythonRuntimeAnchorRef = ref(null)
const pythonRuntimePanelRef = ref(null)
const pythonRuntimePopoverVisible = ref(false)
const pythonRuntimeBusy = ref(false)
const pythonRuntimeStatus = ref(null)
const pythonRuntimePackageInput = ref('')
const pythonRuntimePackageSearch = ref('')
const pythonRuntimeSelectedPackages = ref([])
const pythonRuntimePackageActionLog = ref({
  visible: false,
  title: '',
  stdout: '',
  stderr: '',
})
const pythonRuntimeOperation = ref({
  active: false,
  action: '',
  packages: [],
  startedAt: 0,
  stage: '',
})
const pythonRuntimeArchivePath = ref('')
const pythonRuntimeArchiveInspection = ref(null)
const pythonSessionInfo = ref(null)
const pythonConsoleVisible = ref(false)
const pythonConsoleBusy = ref(false)
const pythonConsoleInput = ref('')
const pythonConsoleHistory = ref([])
const pythonConsolePinnedOutput = ref('')
const pythonConsoleOutputRef = ref(null)
const notebookViewRef = ref(null)
const terminalPanelRef = ref(null)
let notebookDiskPersistTimer = null
let notebookDiskPersistInFlight = false
const pythonConsoleHeight = ref(240)
const EDITOR_STATUS_BAR_HEIGHT = 22
const VIM_STATUS_BAR_HEIGHT = 20
const pythonConsoleBottomOffset = computed(
  () => EDITOR_STATUS_BAR_HEIGHT + (vimMode.value ? VIM_STATUS_BAR_HEIGHT : 0)
)
const pythonConsoleMinHeight = 140
const pythonConsoleMaxHeight = 560
const pythonConsoleResizing = ref(false)
const pythonConsoleMaximized = ref(false)
let pythonConsoleResizeStartY = 0
let pythonConsoleResizeStartHeight = 240
let pythonConsoleRestoreHeight = 240
let leftPythonCellManager = null
let rightPythonCellManager = null
let handlePythonCellSharedStateChange = () => {}
let syncAllPreviewPythonCells = () => {}
const EMBEDDED_PYTHON_CELL_EDITOR_ENABLED = false
const PYTHON_CONSOLE_HEIGHT_STORAGE_KEY = 'guaxi_python_console_height'
const PYTHON_CONSOLE_MAXIMIZED_STORAGE_KEY = 'guaxi_python_console_maximized'
const PYTHON_RUNTIME_RECOMMENDED_PACKAGES = [
  { label: 'pandas', value: 'pandas', description: '数据表格与文件导入导出' },
  { label: 'matplotlib', value: 'matplotlib', description: '图像与绘图输出' },
  { label: 'openpyxl', value: 'openpyxl', description: 'Excel 读写支持' },
  { label: 'ipython', value: 'ipython', description: '增强交互体验与调试' },
]
const getMonacoThemeName = () => (settingsStore.appearanceTheme === 'dark' ? 'guaxi-dark' : 'guaxi-light')
const getMarkdownPreviewThemeName = () => (settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default')
let previewMermaidRenderSeq = 0
const renderPreviewMermaidDiagrams = async (container) => {
  const renderSeq = ++previewMermaidRenderSeq
  const mermaidTheme = 'default'
  logger.info('[Preview] mermaid render start', {
    renderSeq,
    mermaidTheme,
    appearanceTheme: settingsStore.appearanceTheme,
  })

  try {
    mermaid.initialize({
      startOnLoad: false,
      theme: mermaidTheme,
      securityLevel: 'loose',
    })
  } catch (error) {
    logger.warn('[Preview] mermaid initialize failed, continue with previous config', {
      renderSeq,
      mermaidTheme,
      error,
    })
  }

  if (!container?.querySelectorAll) return

  const nodes = Array.from(container.querySelectorAll('.mermaid'))
    .filter((node) => {
      const cachedSource = String(node.getAttribute?.('data-guaxi-mermaid-source') || '')
      const renderedSource = String(node.getAttribute?.('data-mermaid-source') || '')
      const alreadyProcessed =
        node.getAttribute?.('data-processed') === 'true' ||
        !!renderedSource ||
        !!node.querySelector?.('svg')
      if (alreadyProcessed) {
        logger.debug('[Preview] mermaid node already processed by markdown renderer, skip rerender', {
          renderSeq,
          hasRenderedSource: !!renderedSource,
          hasSvg: !!node.querySelector?.('svg'),
        })
        return false
      }

      const extractedSource = String(node.textContent || '')
        .replace(/&gt;/g, '>')
        .replace(/&lt;/g, '<')
        .replace(/&amp;/g, '&')
        .trim()
      const source = cachedSource || extractedSource
      if (!source) {
        logger.warn('[Preview] mermaid node skipped because source is empty', {
          renderSeq,
          className: node.className || '',
        })
        return false
      }
      // 这里只兜底处理尚未经过 GuaxiMarker 预渲染的原始 Mermaid 节点。
      // 对已经生成过 SVG 的节点，预览区会直接跳过，避免把 SVG/CSS 文本误当成 Mermaid 源码。
      node.setAttribute?.('data-guaxi-mermaid-source', source)
      // 这里只兜底处理尚未经过 GuaxiMarker 预渲染的原始 Mermaid 节点。
      // 已经带有 data-mermaid-source / svg 的节点说明主渲染器已经产出了 SVG，
      // 再次读取 textContent 会把 SVG/CSS 文本误当成 Mermaid 源码。
      node.setAttribute?.('data-guaxi-mermaid-source', source)
      node.innerHTML = source
      node.innerHTML = source
      return true
    })
  if (!nodes.length) return

  if (false) nodes.forEach((node) => {
    try {
      const content = String(node.textContent || '')
        .replace(/&gt;/g, '>')
        .replace(/&lt;/g, '<')
        .replace(/&amp;/g, '&')
      node.innerHTML = content
    } catch (error) {
      logger.error('预览区 Mermaid 内容预处理失败', error)
    }
  })

  await new Promise((resolve) => {
    requestAnimationFrame(() => resolve())
  })

  if (renderSeq !== previewMermaidRenderSeq) {
    logger.info('[Preview] mermaid render skipped because a newer render exists', {
      renderSeq,
      latestRenderSeq: previewMermaidRenderSeq,
    })
    return
  }

  let successCount = 0
  for (const node of nodes) {
    if (renderSeq !== previewMermaidRenderSeq) {
      logger.info('[Preview] mermaid render aborted by newer task', {
        renderSeq,
        latestRenderSeq: previewMermaidRenderSeq,
      })
      return
    }

    try {
      await mermaid.run({ nodes: [node] })
      successCount += 1
    } catch (error) {
      logger.warn('[Preview] mermaid single node render failed', {
        renderSeq,
        source: String(node.getAttribute?.('data-guaxi-mermaid-source') || '').slice(0, 160),
        error,
      })
    }
  }

  logger.info('[Preview] mermaid render complete', {
    renderSeq,
    nodeCount: nodes.length,
    successCount,
    mermaidTheme,
  })
}
const previewLineAnchorCache = {
  container: null,
  revision: 0,
  elements: []
}
let previewLineAnchorRevision = 0

const openMarkdownManual = async () => {
  const windowLabel = 'markdown-manual'
  const windowTitle = 'Markdown语法查询手册'

  try {
    await invoke('create_markdown_manual_window', {
      label: windowLabel,
      title: windowTitle,
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('Markdown 语法手册窗口已打开')
      return
    }
    logger.error('[MarkdownManual] open window failed:', err)
    message.error('打开 Markdown 语法手册失败')
  }
}

const editorContainer = ref(null);
const secondaryEditorContainer = ref(null);
const editorWrapperRef = ref(null);
const editorHeaderRef = ref(null);
const previewShellRef = ref(null);
const previewSearchInputRef = ref(null)
const floatingToolbarVisible = ref(false);
const floatingToolbarStyle = ref({ left: '0px', top: '0px' });
const previewFloatingToolbarVisible = ref(false)
const previewFloatingToolbarStyle = ref({ left: '0px', top: '0px' })
const previewSelectionText = ref('')
const selectionSurface = ref('editor')
const previewSearchVisible = ref(false)
const previewSearchKeyword = ref('')
const previewSearchMatches = ref([])
const previewSearchActiveIndex = ref(-1)

const selectionTranslatePanelVisible = ref(false);
const selectionTranslatePinned = ref(false);
const selectionTranslatePanelPos = ref({ left: 8, top: 8 });

const selectionTranslateFrom = ref('auto')
const selectionTranslateTo = ref('zh')

const selectionTranslateLangOptions = [
  { label: '自动检测', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' }
]
const assistantTranslateDialogVisible = ref(false)
const assistantTranslateTargetLang = ref('en')
const assistantTranslateSourceText = ref('')
const assistantTranslateLangOptions = selectionTranslateLangOptions.filter((x) => x.value !== 'auto')

const selectionTranslateBusy = ref(false);
const selectionTranslateRaw = ref('');
const selectionTranslateResult = ref('');
const selectionTranslateError = ref('');
const selectionTranslateItems = ref([]);
const selectionTranslateWordMeta = ref(null)
const selectionReadAloudBusy = ref(false)

let selectionAudioElement = null
let selectionAudioObjectUrl = ''

const selectionTranslateMultiAvailable = ref(true)
let selectionTranslateMultiNotFoundWarned = false

const scheduleSelectionTranslate = debounce(() => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  translateSelectionText(raw)
}, 250)

const normalizeSelectionTextForLanguageTasks = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n。！？.!?;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

const stopSelectionAudioPlayback = () => {
  if (selectionAudioElement) {
    try {
      selectionAudioElement.pause()
      selectionAudioElement.src = ''
    } catch { }
    selectionAudioElement = null
  }
  if (selectionAudioObjectUrl) {
    try {
      URL.revokeObjectURL(selectionAudioObjectUrl)
    } catch { }
    selectionAudioObjectUrl = ''
  }
}

const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

const readSelectionAloud = async () => {
  if (selectionReadAloudBusy.value) return
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate())
  if (!text) {
    message.warning('请先框选要朗读的文本')
    return
  }

  selectionReadAloudBusy.value = true
  try {
    const tts = await invoke('get_tts_config')
    const provider = String(tts?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    const model = String(tts?.model || '').trim() || (provider === 'sherpa-onnx' ? 'utils/Models/tts/kokoro_multi_lang/model.onnx' : 'FunAudioLLM/CosyVoice2-0.5B')
    const tokens = String(tts?.tokens || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(tts?.exe || '').trim() : ''
    const voice = provider === 'sherpa-onnx' ? '0' : 'alex'
    const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'
    logger.info('[GuaxiEditor] 朗读配置', {
      provider,
      model,
      endpoint: provider === 'sherpa-onnx' ? endpoint : '',
      responseFormat,
      isSherpaOnnx: provider === 'sherpa-onnx',
    })
    const response = await invoke('audio_text_to_speech', {
      request: {
        common: {
          provider,
          model,
          endpoint,
          tokens,
        },
        input: text,
        voice,
        response_format: responseFormat,
        stream: false,
      },
    })
    const audioBase64 = String(response?.audio_base64 || '')
    if (!audioBase64) {
      throw new Error('语音数据为空')
    }
    const contentType = String(response?.content_type || 'audio/mpeg')
    const bytes = base64ToUint8Array(audioBase64)
    const blob = new Blob([bytes], { type: contentType })
    stopSelectionAudioPlayback()
    selectionAudioObjectUrl = URL.createObjectURL(blob)
    const audio = new Audio(selectionAudioObjectUrl)
    selectionAudioElement = audio
    const waitForPlaybackEnd = new Promise((resolve, reject) => {
      audio.onended = () => {
        stopSelectionAudioPlayback()
        resolve()
      }
      audio.onerror = () => {
        stopSelectionAudioPlayback()
        reject(new Error('音频播放失败'))
      }
    })
    await audio.play()
    await waitForPlaybackEnd
  } catch (err) {
    logger.error('朗读失败:', err)
    message.error(`朗读失败: ${String(err?.message || err || '')}`)
  } finally {
    selectionReadAloudBusy.value = false
  }
}

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return
  scheduleSelectionTranslate()
})

const editorContextMenuVisible = ref(false);
const editorContextMenuPosition = ref({ x: 0, y: 0 });
const editorContextMenuOptions = ref([]);

const handleEditorContextMenuClickoutside = () => {
  editorContextMenuVisible.value = false;
};

const writeClipboardText = async (text) => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

const readClipboardText = async () => {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
};

const clipboardCopy = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection || selection.isEmpty()) return;
  const text = model.getValueInRange(selection);
  if (!text) return;
  const ok = await writeClipboardText(text);
  if (!ok) message.warning('复制失败：无剪贴板权限');
};

const clipboardCut = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection || selection.isEmpty()) return;

  const text = model.getValueInRange(selection);
  if (!text) return;

  const ok = await writeClipboardText(text);
  if (!ok) {
    message.warning('剪切失败：无剪贴板权限');
    return;
  }

  editor.executeEdits('clipboard-cut', [
    {
      range: selection,
      text: '',
      forceMoveMarkers: true
    }
  ]);
};

const clipboardPaste = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const text = await readClipboardText();
  if (text == null) {
    message.warning('粘贴失败：无剪贴板权限');
    return;
  }

  editor.executeEdits('clipboard-paste', [
    {
      range: selection,
      text,
      forceMoveMarkers: true
    }
  ]);
};

const runEditorAction = async (actionId) => {
  if (!editor) return;
  editor.focus();

  if (actionId === 'editor.action.clipboardCopyAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardCopy();
    return;
  }

  if (actionId === 'editor.action.clipboardCutAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardCut();
    return;
  }

  if (actionId === 'editor.action.clipboardPasteAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardPaste();
    return;
  }

  const action = editor.getAction?.(actionId);
  if (action) {
    await action.run();
    return;
  }

  try {
    editor.trigger('contextmenu', actionId, null);
  } catch {
    message.warning('该命令当前不可用');
  }
};

const buildEditorContextMenuOptions = () => {
  const model = editor?.getModel?.();
  const isMarkdown = (model?.getLanguageId?.() || '') === 'markdown';
  const selection = editor?.getSelection?.();
  const hasSelection = !!selection && !selection.isEmpty();

  return [
    { label: '剪切', key: 'editor.action.clipboardCutAction' },
    { label: '复制', key: 'editor.action.clipboardCopyAction' },
    { label: '粘贴', key: 'editor.action.clipboardPasteAction' },
    { type: 'divider' },

    { label: '文件内搜索 (Ctrl+F)', key: 'actions.find' },
    { type: 'divider' },

    { label: 'Markdown 格式化（全文）', key: 'guaxi.markdown.formatDocument', disabled: !isMarkdown },
    { label: 'Markdown 格式化（选中）', key: 'guaxi.markdown.formatSelection', disabled: !isMarkdown || !hasSelection },
    { type: 'divider' },

    { label: '跳转到上次编辑位置 (Alt+←)', key: 'guaxi.edit.navigateBack' },
    { label: '跳转到下一编辑位置 (Alt+→)', key: 'guaxi.edit.navigateForward' },
    { type: 'divider' },

    { label: '插入Markdown模板', key: 'insert.md.template' },
    { type: 'divider' },

    { label: '命令面板', key: 'editor.action.quickCommand' }
  ];
};

const handleEditorContextMenuSelect = async (key) => {
  editorContextMenuVisible.value = false;

  if (key === 'guaxi.markdown.formatDocument') {
    await formatMarkdownDocument();
    return;
  }

  if (key === 'guaxi.markdown.formatSelection') {
    await formatMarkdownSelection();
    return;
  }

  if (key === 'guaxi.edit.navigateBack') {
    await navigateToPreviousEditLocation();
    return;
  }

  if (key === 'guaxi.edit.navigateForward') {
    await navigateToNextEditLocation();
    return;
  }

  await runEditorAction(key);
};
const tableToolbarVisible = ref(false);
const tableToolbarStyle = ref({ left: '0px', top: '0px' });
const activeTableContext = ref(null);
let tableInsertAutoFormatBusy = false;
let lastTableToolbarInsertAt = 0;
const scrollRatio = ref(0);
const pendingPreviewScrollRestore = ref(false);
const pendingPreviewScrollLine = ref(null);
const pendingRealtimeScrollRestore = ref(false);
const realtimeVditorId = 'realtime-vditor';
const realtimeVditorReady = ref(false);
const pendingRealtimeVditorValue = ref(null);
let realtimeVditor = null;
let settingRealtimeVditorValue = false;
let sourceEditorProgrammaticSyncDepth = 0;
let realtimeVditorLinkInterceptorCleanup = null;
const REALTIME_VDITOR_INIT_TIMEOUT_MS = 12000
const PRIMARY_EDITOR_VSCODE_MODEL_OWNER = 'guaxi-editor-primary'
const SECONDARY_EDITOR_VSCODE_MODEL_OWNER = 'guaxi-editor-secondary'

const getMarkdownFenceVscodeModelOwner = (languageId) => {
  const key = String(languageId || '').trim().toLowerCase() || 'unknown'
  return `guaxi-editor-markdown-fence:${key}`
}

const getLocalVditorCdn = () => {
  // Vditor appends `/dist/...` internally, so the CDN value here must point to
  // the parent folder that contains the `dist` directory copied into `public`.
  return '/vditor'
}

const getBundledVditorI18n = () => {
  if (typeof window === 'undefined' || !window.VditorI18n) return undefined
  return { ...window.VditorI18n }
}

const createRealtimeVditorInitDiagnostics = () => {
  const resourceFailures = []
  const promiseFailures = []

  const recordResourceFailure = (target) => {
    if (!(target instanceof Element)) return
    const tag = String(target.tagName || '').toLowerCase()
    if (!['script', 'link', 'img'].includes(tag)) return
    const url =
      target.getAttribute?.('src') ||
      target.getAttribute?.('href') ||
      target.getAttribute?.('currentSrc') ||
      ''
    if (!url) return
    resourceFailures.push({
      tag,
      url,
      id: target.getAttribute?.('id') || '',
      rel: target.getAttribute?.('rel') || '',
    })
  }

  const onWindowError = (event) => {
    if (event?.target && event.target !== window) {
      recordResourceFailure(event.target)
      return
    }
    if (event?.message) {
      promiseFailures.push({
        type: 'window-error',
        message: String(event.message),
        filename: String(event.filename || ''),
      })
    }
  }

  const onUnhandledRejection = (event) => {
    const reason = event?.reason
    promiseFailures.push({
      type: 'unhandledrejection',
      message:
        reason instanceof Error
          ? `${reason.name}: ${reason.message}`
          : String(reason ?? 'unknown'),
    })
  }

  window.addEventListener('error', onWindowError, true)
  window.addEventListener('unhandledrejection', onUnhandledRejection)

  return {
    snapshot() {
      return {
        resourceFailures: resourceFailures.slice(-10),
        promiseFailures: promiseFailures.slice(-10),
      }
    },
    dispose() {
      window.removeEventListener('error', onWindowError, true)
      window.removeEventListener('unhandledrejection', onUnhandledRejection)
    }
  }
}

const syncRealtimeValueToRawContent = debounce((value) => {
  if (settingRealtimeVditorValue) return;
  rawContent.value = value;
  const path = fileTreeStore.activeTab;
  if (path) fileTreeStore.updateTabContent(path, value);
}, 120);

let editor = null;
let secondaryEditor = null;
let monacoCtrlClickDisposable = null;
let secondaryMonacoCtrlClickDisposable = null;
let monacoLinkLikeDecorDisposable = null;
let secondaryMonacoLinkLikeDecorDisposable = null;
let editorEffectsLeft = null;
let editorEffectsRight = null;

const createEditorEffects = (targetEditor) => {
  const host = targetEditor?.getDomNode?.();
  if (!host) return null;

  const overlay = document.createElement('div');
  overlay.className = 'guaxi-editor-effects';

  const canvas = document.createElement('canvas');
  canvas.className = 'guaxi-editor-effects-canvas';

  const cursor = document.createElement('div');
  cursor.className = 'guaxi-editor-smooth-cursor';

  overlay.appendChild(canvas);
  overlay.appendChild(cursor);
  host.appendChild(overlay);

  const ctx = canvas.getContext('2d');
  const palette = [
    'rgba(255, 208, 0, 0.95)',
    'rgba(255, 105, 180, 0.95)',
    'rgba(0, 220, 255, 0.95)',
    'rgba(140, 255, 120, 0.95)',
    'rgba(255, 90, 90, 0.95)',
    'rgba(180, 120, 255, 0.95)'
  ];

  const state = {
    host,
    overlay,
    canvas,
    ctx,
    dpr: 1,
    w: 0,
    h: 0,
    particles: [],
    raf: 0,
    lastTs: 0,
    cx: 0,
    cy: 0,
    tx: 0,
    ty: 0,
    ch: 18,
    tch: 18,
    cursorVisible: false,
    ro: null,
    resizeHandler: null
  };

  const resize = () => {
    const rect = host.getBoundingClientRect();
    const w = Math.max(1, Math.floor(rect.width));
    const h = Math.max(1, Math.floor(rect.height));
    const dpr = window.devicePixelRatio || 1;

    if (w === state.w && h === state.h && dpr === state.dpr) return;

    state.w = w;
    state.h = h;
    state.dpr = dpr;

    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    canvas.width = Math.max(1, Math.floor(w * dpr));
    canvas.height = Math.max(1, Math.floor(h * dpr));

    if (ctx) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
  };

  const step = (ts) => {
    const dt = Math.min(0.034, Math.max(0, (ts - (state.lastTs || ts)) / 1000));
    state.lastTs = ts;

    resize();

    const dx = state.tx - state.cx;
    const dy = state.ty - state.cy;
    const dh = state.tch - state.ch;

    const follow = 1 - Math.pow(1 - 0.35, dt * 60);
    state.cx += dx * follow;
    state.cy += dy * follow;
    state.ch += dh * follow;

    if (state.cursorVisible) {
      cursor.style.opacity = '1';
      cursor.style.height = `${Math.max(8, state.ch)}px`;
      cursor.style.transform = `translate3d(${state.cx}px, ${state.cy}px, 0)`;
    } else {
      cursor.style.opacity = '0';
    }

    const g = 900;
    const drag = Math.pow(0.04, dt);

    if (state.particles.length) {
      for (let i = state.particles.length - 1; i >= 0; i -= 1) {
        const p = state.particles[i];
        p.life -= dt;
        if (p.life <= 0) {
          state.particles.splice(i, 1);
          continue;
        }
        p.vy += g * dt;
        p.vx *= drag;
        p.vy *= drag;
        p.x += p.vx * dt;
        p.y += p.vy * dt;
      }
    }

    if (ctx) {
      ctx.clearRect(0, 0, state.w, state.h);
      if (state.particles.length) {
        ctx.globalCompositeOperation = 'lighter';
        for (let i = 0; i < state.particles.length; i += 1) {
          const p = state.particles[i];
          const a = Math.max(0, Math.min(1, p.life / p.ttl));
          ctx.globalAlpha = a;
          ctx.fillStyle = p.color;
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = 'source-over';
      }
    }

    // When there's nothing to animate, stop the RAF loop to reduce idle overhead.
    // 1) no particles and cursor not visible -> stop
    // 2) no particles and cursor already very close to target -> stop
    if (!state.particles.length) {
      if (!state.cursorVisible) {
        state.raf = 0;
        return;
      }

      const rdx = state.tx - state.cx;
      const rdy = state.ty - state.cy;
      const rdh = state.tch - state.ch;

      // Pixel-level settle threshold (CSS pixels)
      const settlePosEps = 0.25;
      const settleHeightEps = 0.25;
      const settled =
        Math.abs(rdx) < settlePosEps &&
        Math.abs(rdy) < settlePosEps &&
        Math.abs(rdh) < settleHeightEps;

      if (settled) {
        // Snap to target so the last rendered frame is exact.
        state.cx = state.tx;
        state.cy = state.ty;
        state.ch = state.tch;

        cursor.style.opacity = '1';
        cursor.style.height = `${Math.max(8, state.ch)}px`;
        cursor.style.transform = `translate3d(${state.cx}px, ${state.cy}px, 0)`;

        state.raf = 0;
        return;
      }
    }

    state.raf = requestAnimationFrame(step);
  };

  const ensureLoop = () => {
    if (state.raf) return;
    state.lastTs = performance.now();
    state.raf = requestAnimationFrame(step);
  };

  const updateCursorTarget = () => {
    const model = targetEditor.getModel();
    const pos = targetEditor.getPosition();
    if (!model || !pos) {
      state.cursorVisible = false;
      ensureLoop();
      return;
    }

    const visible = targetEditor.getScrolledVisiblePosition(pos);
    if (!visible) {
      state.cursorVisible = false;
      ensureLoop();
      return;
    }

    state.cursorVisible = true;
    state.tx = visible.left;
    state.ty = visible.top;
    state.tch = visible.height || 18;
    ensureLoop();
  };

  const spawnExplosionAtCursor = (text) => {
    const ch = text ? text[text.length - 1] : '';
    if (!ch || ch === '\n' || ch === '\r' || ch === '\t') return;

    const model = targetEditor.getModel();
    const pos = targetEditor.getPosition();
    if (!model || !pos) return;

    const visible = targetEditor.getScrolledVisiblePosition(pos);
    if (!visible) return;

    const x = visible.left;
    const y = visible.top + (visible.height || 18) * 0.7;

    const n = Math.min(48, 18 + Math.floor(Math.random() * 10));
    for (let i = 0; i < n; i += 1) {
      const a = Math.random() * Math.PI * 2;
      const s = 80 + Math.random() * 340;
      state.particles.push({
        x,
        y,
        vx: Math.cos(a) * s,
        vy: Math.sin(a) * s - (120 + Math.random() * 160),
        size: 0.8 + Math.random() * 2.2,
        ttl: 0.55 + Math.random() * 0.35,
        life: 0.55 + Math.random() * 0.35,
        color: palette[(Math.random() * palette.length) | 0]
      });
    }

    ensureLoop();
  };

  resize();

  if (typeof ResizeObserver !== 'undefined') {
    state.ro = new ResizeObserver(() => resize());
    state.ro.observe(host);
  } else {
    state.resizeHandler = () => resize();
    window.addEventListener('resize', state.resizeHandler);
  }

  // Don't start RAF loop until there's something to animate (cursor move / particles).
  return {
    updateCursorTarget,
    spawnExplosionAtCursor,
    destroy() {
      if (state.raf) cancelAnimationFrame(state.raf);
      state.raf = 0;
      if (state.ro) state.ro.disconnect();
      state.ro = null;
      if (state.resizeHandler) {
        window.removeEventListener('resize', state.resizeHandler);
        state.resizeHandler = null;
      }
      if (state.overlay?.parentNode) state.overlay.parentNode.removeChild(state.overlay);
      state.particles = [];
      state.cursorVisible = false;
    }
  };
};

let initEditorPending = false;

const editLocationBackStack = ref([]);
const editLocationForwardStack = ref([]);
let lastRecordedEditLocationKey = null;
let lastRecordedEditLocationAt = 0;
let suppressEditLocationRecording = false;
const maxEditLocationHistory = 200;

const previewLocationBackStack = ref([]);
const previewLocationForwardStack = ref([]);
let lastRecordedPreviewLocationKey = null;
let lastRecordedPreviewLocationAt = 0;
let suppressPreviewLocationRecording = false;
const maxPreviewLocationHistory = 200;

const getCurrentPreviewLocation = () => {
  const path = fileTreeStore.activeTab || selectedFile.value?.path;
  const container = previewContainer.value;
  if (!path || !(container instanceof HTMLElement)) return null;

  const scrollTop = container.scrollTop;
  const height = container.scrollHeight;
  const scrollRatio = height ? scrollTop / height : 0;

  const containerRect = container.getBoundingClientRect();
  const els = Array.from(container.querySelectorAll('[data-md-start-line]') || []);
  if (!els.length) return { path, lineNumber: 1, scrollTop, scrollRatio };

  let bestLine = 1;
  let bestScore = Number.POSITIVE_INFINITY;

  for (const el of els) {
    const start = Number(el?.dataset?.mdStartLine || 0);
    if (!Number.isFinite(start) || start <= 0) continue;

    const r = el.getBoundingClientRect();
    const visible = r.bottom > containerRect.top + 8 && r.top < containerRect.bottom - 8;
    const score = visible ? Math.abs(r.top - containerRect.top) : (Math.abs(r.top - containerRect.top) + 100000);
    if (score < bestScore) {
      bestScore = score;
      bestLine = start;
    }
  }

  return { path, lineNumber: bestLine || 1, scrollTop, scrollRatio };
};

const isSamePreviewLocation = (a, b) => {
  if (!a || !b) return false;
  if (a.path !== b.path) return false;

  const at = Number(a.scrollTop);
  const bt = Number(b.scrollTop);
  if (Number.isFinite(at) && Number.isFinite(bt)) {
    return Math.round(at / 8) === Math.round(bt / 8);
  }

  if (
    Number(a.lineNumber) > 0 &&
    Number(b.lineNumber) > 0 &&
    Number(a.lineNumber) === Number(b.lineNumber)
  ) {
    const ai = a.hitIndex;
    const bi = b.hitIndex;
    if (ai == null || bi == null) return true;
    return Number(ai) === Number(bi);
  }

  return false;
};

const recordPreviewLocation = (reason, overrideLoc = null) => {
  if (suppressPreviewLocationRecording) return;

  const loc = overrideLoc || getCurrentPreviewLocation();
  if (!loc) return;

  const now = Date.now();
  const st = Number(loc.scrollTop);
  const stKey = Number.isFinite(st) ? Math.round(st / 8) : 0;
  const key = `${loc.path}:${stKey}`;
  if (key === lastRecordedPreviewLocationKey && now - lastRecordedPreviewLocationAt < 350) return;

  const back = previewLocationBackStack.value;
  const last = back[back.length - 1];
  if (last && isSamePreviewLocation(last, loc)) {
    lastRecordedPreviewLocationKey = key;
    lastRecordedPreviewLocationAt = now;
    return;
  }

  back.push({
    ...loc,
    scrollTop: Number.isFinite(st) ? st : 0,
    scrollTopKey: stKey,
    ts: now,
    reason: String(reason || '')
  });
  if (back.length > maxPreviewLocationHistory) back.shift();
  previewLocationForwardStack.value = [];

  lastRecordedPreviewLocationKey = key;
  lastRecordedPreviewLocationAt = now;

  logger.debug('[PreviewNav] record', { reason, path: loc.path, line: loc.lineNumber, back: back.length });
};

const goToPreviewLocation = async (loc, { source = 'shortcut' } = {}) => {
  if (!loc) return false;
  const path = String(loc.path || '');
  const st = Number(loc.scrollTop);
  const ratio = Number(loc.scrollRatio);
  const line = Number(loc.lineNumber || 0);
  if (!path) return false;

  suppressPreviewLocationRecording = true;
  try {
    if (fileTreeStore.activeTab !== path) {
      await fileTreeStore.openFileInTab(path);
      await nextTick();
    }

    await nextTick();

    const container = previewContainer.value;
    if (container && container instanceof HTMLElement) {
      const maxTop = Math.max(0, container.scrollHeight - container.clientHeight);
      let nextTop = null;

      if (Number.isFinite(st)) {
        nextTop = Math.min(Math.max(0, st), maxTop);
      } else if (Number.isFinite(ratio) && ratio > 0) {
        nextTop = Math.min(Math.max(0, ratio * container.scrollHeight), maxTop);
      }

      if (nextTop != null) {
        container.scrollTop = nextTop;
      } else if (Number.isFinite(line) && line > 0) {
        scrollPreviewToLine(line);
      }
    } else if (Number.isFinite(line) && line > 0) {
      scrollPreviewToLine(line);
    }

    logger.info('[PreviewNav] go', {
      source,
      path,
      scrollTop: Number.isFinite(st) ? st : 'na',
      line: Number.isFinite(line) ? line : 'na'
    });
    return true;
  } finally {
    window.setTimeout(() => {
      suppressPreviewLocationRecording = false;
    }, 0);
  }
};

const navigateToPreviousPreviewLocation = async () => {
  const current = getCurrentPreviewLocation();
  if (!current) return;

  const back = previewLocationBackStack.value;
  const forward = previewLocationForwardStack.value;

  const last = back[back.length - 1];
  if (!last || !isSamePreviewLocation(last, current)) {
    back.push({ ...current, ts: Date.now(), reason: 'implicit-current' });
    if (back.length > maxPreviewLocationHistory) back.shift();
  }

  if (back.length < 2) {
    message.info('没有上一个预览位置');
    return;
  }

  const from = back.pop();
  forward.push(from);
  const target = back[back.length - 1];
  await goToPreviewLocation(target, { source: 'alt-left' });
};

const navigateToNextPreviewLocation = async () => {
  const forward = previewLocationForwardStack.value;
  if (forward.length === 0) {
    message.info('没有下一个预览位置');
    return;
  }

  const target = forward.pop();
  const back = previewLocationBackStack.value;
  const last = back[back.length - 1];
  if (!last || !isSamePreviewLocation(last, target)) {
    back.push(target);
    if (back.length > maxPreviewLocationHistory) back.shift();
  }

  await goToPreviewLocation(target, { source: 'alt-right' });
};

const getCurrentEditLocation = () => {
  if (!editor) return null;
  const path = fileTreeStore.activeTab || selectedFile.value?.path;
  const position = editor.getPosition();
  if (!path || !position) return null;
  return {
    path,
    lineNumber: position.lineNumber,
    column: position.column
  };
};

const isSameEditLocation = (a, b) => {
  if (!a || !b) return false;
  return a.path === b.path && a.lineNumber === b.lineNumber && a.column === b.column;
};

const recordEditLocationFromEditor = () => {
  if (!editor) return;
  if (suppressEditLocationRecording) return;

  const loc = getCurrentEditLocation();
  if (!loc) return;

  const now = Date.now();
  const key = `${loc.path}:${loc.lineNumber}:${loc.column}`;
  if (key === lastRecordedEditLocationKey && now - lastRecordedEditLocationAt < 500) return;

  const back = editLocationBackStack.value;
  const last = back[back.length - 1];
  if (last && isSameEditLocation(last, loc)) {
    lastRecordedEditLocationKey = key;
    lastRecordedEditLocationAt = now;
    return;
  }

  back.push({ ...loc, ts: now });
  if (back.length > maxEditLocationHistory) back.shift();
  editLocationForwardStack.value = [];

  lastRecordedEditLocationKey = key;
  lastRecordedEditLocationAt = now;
};

const goToEditLocation = async (loc) => {
  if (!loc) return false;

  suppressEditLocationRecording = true;
  try {
    if (fileTreeStore.activeTab !== loc.path) {
      await fileTreeStore.openFileInTab(loc.path);
    }

    await nextTick();

    if (!editor) return false;
    const pos = new monaco.Position(loc.lineNumber, loc.column);
    editor.focus();
    editor.setPosition(pos);
    editor.revealPositionInCenterIfOutsideViewport(pos);
    return true;
  } finally {
    suppressEditLocationRecording = false;
  }
};

const navigateToPreviousEditLocation = async () => {
  if (!editor) return;

  const current = getCurrentEditLocation();
  if (!current) return;

  const back = editLocationBackStack.value;
  const forward = editLocationForwardStack.value;

  if (back.length === 0) {
    back.push({ ...current, ts: Date.now() });
    return;
  }

  const last = back[back.length - 1];
  if (!isSameEditLocation(last, current)) {
    forward.push({ ...current, ts: Date.now() });
    await goToEditLocation(last);
    return;
  }

  if (back.length < 2) {
    message.info('没有上一个编辑位置');
    return;
  }

  const popped = back.pop();
  forward.push(popped);
  const target = back[back.length - 1];
  await goToEditLocation(target);
};

const navigateToNextEditLocation = async () => {
  if (!editor) return;

  const forward = editLocationForwardStack.value;
  if (forward.length === 0) {
    return;
  }

  const target = forward.pop();
  const back = editLocationBackStack.value;
  const last = back[back.length - 1];
  if (!last || !isSameEditLocation(last, target)) {
    back.push(target);
    if (back.length > maxEditLocationHistory) back.shift();
  }

  await goToEditLocation(target);
};

const selectedFile = computed(() => fileTreeStore.selectedFile)
// 定义 rawContent 变量
const rawContent = ref(selectedFile.value?.content || '')
// 1. 获取当前激活tab对象和内容
const activeTab = computed(() => fileTreeStore.openTabs.find(tab => tab.path === fileTreeStore.activeTab));
const activeTabContent = computed(() => activeTab.value?.content || '');
const canUseNotebookSurface = computed(() => {
  const path = String(selectedFile.value?.path || '').trim().toLowerCase()
  if (!path.endsWith('.md') && !path.endsWith('.markdown')) return false
  if (layoutMode.value !== 'single') return false
  if (activeTab.value?.unsupported) return false
  return true
})

const isExplorerSurfaceActive = computed(() => {
  return editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value
})

watch(canUseNotebookSurface, (enabled) => {
  if (!enabled && editorSurfaceMode.value !== 'source') {
    logger.info('[Editor] notebook surface disabled, fallback to source editor', {
      filePath: String(selectedFile.value?.path || ''),
      storedMode: String(editorStore.homePreviewModeKey || ''),
    })
    editorSurfaceMode.value = 'source'
    if (String(editorStore.homePreviewModeKey || '') === 'explorer') {
      setHomePreviewModeKeySafe('edit')
    }
  }
  if (enabled && editorSurfaceMode.value === 'source') {
    nextTick(() => {
      syncSourceEditorContent('can-use-notebook-changed')
    })
  }
})

// 面包屑导航：点击后联动文件树定位/展开
const EVENT_REVEAL_IN_FILE_TREE = 'guaxi_editor_reveal_in_file_tree'

// 统一路径分隔符（用于面包屑展示与前缀判断）
const normalizeSlashes = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$|\s+$/g, '').trim()

const cursorLineNumber = computed(() => {
  const n = Number(editorStore.cursorPosition?.lineNumber || 1)
  return Number.isFinite(n) && n > 0 ? n : 1
})

const safeOutlineFlat = computed(() => {
  const v = outlineStore?.outline
  if (Array.isArray(v)) return v
  if (Array.isArray(v?.value)) return v.value
  return []
})

const buildHeadingBreadcrumbItems = (outlineFlat, lineNumber) => {
  const arr = Array.isArray(outlineFlat) ? outlineFlat : []
  if (!arr.length) return []

  const ln = Number(lineNumber || 1)
  const stack = []

  for (const h of arr) {
    const hLine = Number(h?.line || 0)
    const level = Number(h?.level || 0)
    const text = String(h?.text || '').trim()
    if (!text || !Number.isFinite(hLine) || hLine <= 0 || !Number.isFinite(level) || level <= 0) continue
    if (hLine > ln) break

    while (stack.length && stack[stack.length - 1].level >= level) stack.pop()
    stack.push({ level, text, line: hLine })
  }

  return stack.map((h) => ({
    key: `heading:${h.line}:${h.level}:${h.text}`,
    label: h.text,
    title: `Ln ${h.line}: ${h.text}`,
    icon: hashIcon,
    kind: 'heading',
    line: h.line
  }))
}

// 从 repopathlist 中找出“最匹配”的知识库根路径
const findRepoRootForPath = (absPath) => {
  const target = normalizeSlashes(absPath)
  if (!target) return ''

  const repos = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  let best = ''
  for (const raw of repos) {
    const repo = normalizeSlashes(raw)
    if (!repo) continue
    if (target === repo || target.startsWith(`${repo}/`)) {
      if (repo.length > best.length) best = repo
    }
  }

  // 兜底：如果当前 activeRepoPath 能匹配，也返回
  const active = normalizeSlashes(fileTreeStore.activeRepoPath)
  if (!best && active && (target === active || target.startsWith(`${active}/`))) {
    best = active
  }

  return best
}

// 折叠过长的路径面包屑，避免把“标题/函数”等大纲面包屑挤出可视区（行为参考 VSCode：保留头部与尾部，中间用“…”折叠）
const collapsePathBreadcrumbItems = (items, maxVisible = 4) => {
  const arr = Array.isArray(items) ? items : []
  if (arr.length <= maxVisible) return arr

  const head = arr[0]
  const tailCount = Math.max(1, maxVisible - 2)
  const tail = arr.slice(-tailCount)

  const full = arr.map((it) => String(it?.title || it?.label || '')).filter(Boolean).join(' › ')

  return [
    head,
    {
      key: `ellipsis:${head?.key || 'path'}`,
      label: '…',
      title: full || '路径已折叠',
      kind: 'ellipsis',
      disabled: true
    },
    ...tail
  ]
}

const buildBreadcrumbItems = (absPath) => {
  const target = normalizeSlashes(absPath)
  if (!target) {
    return [{ key: 'placeholder', label: '未打开文件', title: '未打开文件', icon: fileTextIcon, kind: 'placeholder', disabled: true }]
  }

  const repoRoot = findRepoRootForPath(target)
  const items = []

  if (repoRoot) {
    const repoName = repoRoot.split('/').pop() || repoRoot
    items.push({ key: `repo:${repoRoot}`, label: repoName, title: repoRoot, icon: bookOpenIcon, kind: 'repo', path: repoRoot })

    const rel = target === repoRoot ? '' : target.slice(repoRoot.length + 1)
    const parts = rel ? rel.split('/').filter(Boolean) : []

    let cursor = repoRoot
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      cursor = `${cursor}/${part}`
      const isLast = i === parts.length - 1
      items.push({
        key: `path:${cursor}`,
        label: part,
        title: cursor,
        icon: isLast ? fileTextIcon : folderIcon,
        kind: isLast ? 'file' : 'dir',
        path: cursor
      })
    }

    return collapsePathBreadcrumbItems(items)
  }

  const leaf = target.split('/').pop() || target
  return [{ key: `leaf:${target}`, label: leaf, title: target, icon: fileTextIcon, kind: 'leaf', path: target }]
}

const breadcrumbItems = computed(() => {
  const absPath = activeTab.value?.path || selectedFile.value?.path || ''
  const pathItems = buildBreadcrumbItems(absPath)
  const headingItems = buildHeadingBreadcrumbItems(safeOutlineFlat.value, cursorLineNumber.value)
  return [...pathItems, ...headingItems]
})

const handleBreadcrumbClick = async (it) => {
  if (!it || it.disabled) return

  if (it.kind === 'heading' && Number.isFinite(Number(it.line))) {
    const ln = Number(it.line)
    logger.info('[Breadcrumb] jump to heading', { line: ln, label: it.label })
    try {
      jumpToLine(ln)
    } catch (e) {
      logger.warn('[Breadcrumb] jump failed:', e)
    }
    return
  }

  const path = it?.path ? String(it.path) : ''
  if (!path) return

  logger.info('[Breadcrumb] reveal in file tree', { label: it?.label, path })
  try {
    await emit(EVENT_REVEAL_IN_FILE_TREE, { path })
  } catch (e) {
    logger.warn('[Breadcrumb] emit reveal event failed:', e)
  }
}

const breadcrumbScrollRef = ref(null)

const handleBreadcrumbWheel = (e) => {
  const el = breadcrumbScrollRef.value
  if (!el) return

  const dx = Number(e?.deltaX || 0)
  const dy = Number(e?.deltaY || 0)
  const delta = Math.abs(dy) >= Math.abs(dx) ? dy : dx

  if (!Number.isFinite(delta) || delta === 0) return

  el.scrollLeft += delta
}

const breadcrumbPickerVisible = ref(false)
const breadcrumbPickerOptions = ref([])
const breadcrumbPickerPosition = ref({ x: 0, y: 0 })
const breadcrumbPickerPayloadByKey = ref(new Map())

const closeBreadcrumbPicker = () => {
  breadcrumbPickerVisible.value = false
  breadcrumbPickerOptions.value = []
  breadcrumbPickerPayloadByKey.value = new Map()
}

const normalizeBreadcrumbPath = (path) => {
  const raw = normalizeSlashes(path)
  if (!raw) return ''
  if (raw.match(/^[a-zA-Z]:\//)) return raw[0].toUpperCase() + raw.slice(1)
  return raw
}

const isHiddenPathForBreadcrumb = (path) => {
  const p = normalizeBreadcrumbPath(path)
  if (!p) return false
  return p.split('/').some((part) => String(part || '').startsWith('.'))
}

const getFileTreeIndex = (() => {
  let cachedTreeRef = null
  let cachedRootId = ''
  let cached = null

  const walk = (nodes, parentId, parentById, childrenByParent, nodeById) => {
    if (!Array.isArray(nodes)) return
    for (const node of nodes) {
      const id = normalizeBreadcrumbPath(node?.id)
      if (!id) continue

      nodeById.set(id, node)
      if (parentId) parentById.set(id, parentId)

      const list = childrenByParent.get(parentId) || []
      list.push(node)
      childrenByParent.set(parentId, list)

      if (Array.isArray(node?.children) && node.children.length) {
        walk(node.children, id, parentById, childrenByParent, nodeById)
      }
    }
  }

  return (rootId) => {
    const treeRef = fileTreeStore.fileTree
    const rid = normalizeBreadcrumbPath(rootId)
    if (treeRef && treeRef === cachedTreeRef && rid && rid === cachedRootId && cached) return cached

    const parentById = new Map()
    const childrenByParent = new Map()
    const nodeById = new Map()

    walk(treeRef, rid, parentById, childrenByParent, nodeById)

    cachedTreeRef = treeRef
    cachedRootId = rid
    cached = { parentById, childrenByParent, nodeById, rootId: rid }
    return cached
  }
})()

const buildOutlineTreeWithParents = (flatOutline) => {
  const stack = []
  const root = []
  const byKey = new Map()

  const arr = Array.isArray(flatOutline) ? flatOutline : []
  arr.forEach((item, idx) => {
    const level = Number(item?.level || 0)
    const text = String(item?.text || '').trim()
    const line = Number(item?.line || 0)
    if (!text || !Number.isFinite(level) || level <= 0 || !Number.isFinite(line) || line <= 0) return

    const key = `heading:${line}:${level}:${text}`
    const node = { key, level, text, line, parent: null, children: [], _idx: idx }

    while (stack.length && stack[stack.length - 1].level >= node.level) stack.pop()

    if (stack.length === 0) {
      root.push(node)
    } else {
      node.parent = stack[stack.length - 1]
      node.parent.children.push(node)
    }

    stack.push(node)
    byKey.set(key, node)
  })

  return { root, byKey }
}

const makeBreadcrumbPickerOption = (payload, isActive) => {
  const key = payload.key
  return {
    key,
    label: () => h('div', { class: ['breadcrumb-picker-item', isActive ? 'active' : ''] }, [
      payload.icon ? h(Icon, { icon: payload.icon, width: 14, height: 14, class: 'breadcrumb-picker-icon' }) : null,
      h('span', { class: 'breadcrumb-picker-label', title: payload.title || payload.label }, payload.label),
    ])
  }
}

const openBreadcrumbPicker = (it, ev) => {
  if (!it || it.disabled) return

  const x = Number(ev?.clientX || 0)
  const y = Number(ev?.clientY || 0)
  breadcrumbPickerPosition.value = { x, y }

  const payloadByKey = new Map()
  let payloads = []

  if (it.kind === 'heading') {
    const tree = buildOutlineTreeWithParents(safeOutlineFlat.value)
    const node = tree.byKey.get(String(it.key || ''))
    const siblings = node?.parent ? node.parent.children : tree.root

    payloads = (siblings || []).map((n) => ({
      key: `H:${n.key}`,
      label: n.text,
      title: `Ln ${n.line}: ${n.text}`,
      icon: hashIcon,
      kind: 'heading',
      line: n.line,
    }))
  } else {
    if (it.kind === 'repo') {
      payloads = (Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []).map((p) => {
        const path = normalizeBreadcrumbPath(p)
        const name = path.split('/').pop() || path
        return { key: `P:${path}`, label: name, title: path, icon: bookOpenIcon, kind: 'repo', path, isFile: false }
      })
    } else {
      const target = normalizeBreadcrumbPath(it.path)
      const repoRoot = normalizeBreadcrumbPath(findRepoRootForPath(target) || fileTreeStore.activeRepoPath || '')
      const index = getFileTreeIndex(repoRoot)

      const parentId = index?.parentById?.get(target) || repoRoot
      const siblingsRaw = index?.childrenByParent?.get(parentId) || []

      payloads = siblingsRaw
        .map((n) => {
          const path = normalizeBreadcrumbPath(n?.id)
          if (!path) return null
          if (isHiddenPathForBreadcrumb(path)) return null

          const label = String(n?.label || path.split('/').pop() || path)
          const isDir = Array.isArray(n?.children) && n.children.length > 0
          return {
            key: `P:${path}`,
            label,
            title: path,
            icon: isDir ? folderIcon : fileTextIcon,
            kind: isDir ? 'dir' : 'file',
            path,
            isFile: !isDir
          }
        })
        .filter(Boolean)
        .sort((a, b) => String(a.label || '').localeCompare(String(b.label || ''), 'zh-CN'))
    }
  }

  if (!payloads.length) {
    logger.info('[BreadcrumbPicker] no siblings, fallback click', { key: it.key, kind: it.kind })
    handleBreadcrumbClick(it)
    return
  }

  for (const p of payloads) payloadByKey.set(p.key, p)

  breadcrumbPickerPayloadByKey.value = payloadByKey
  breadcrumbPickerOptions.value = payloads.map((p) => {
    const isActive = it.kind === 'heading'
      ? String(it.key || '') === String(p.key || '').replace(/^H:/, '')
      : normalizeBreadcrumbPath(it.path) === normalizeBreadcrumbPath(p.path)

    return makeBreadcrumbPickerOption(p, isActive)
  })

  breadcrumbPickerVisible.value = true
  logger.info('[BreadcrumbPicker] opened', { kind: it.kind, key: it.key, count: payloads.length })
}

const handleBreadcrumbPickerSelect = async (key) => {
  const payload = breadcrumbPickerPayloadByKey.value?.get?.(key)
  closeBreadcrumbPicker()
  if (!payload) return

  if (payload.kind === 'heading') {
    logger.info('[BreadcrumbPicker] select heading', { line: payload.line, label: payload.label })
    handleBreadcrumbClick({ kind: 'heading', line: payload.line, label: payload.label })
    return
  }

  const path = payload.path
  if (!path) return

  logger.info('[BreadcrumbPicker] select path', { path, kind: payload.kind, isFile: !!payload.isFile })

  try {
    if (payload.isFile) {
      await fileTreeStore.openFileInTab(path)
    }
    await emit(EVENT_REVEAL_IN_FILE_TREE, { path })
  } catch (e) {
    logger.warn('[BreadcrumbPicker] select failed:', e)
  }
}

const leftTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(0, fileTreeStore.splitIndex);
});

const rightTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return [];
  }
  if (fileTreeStore.openTabs.length === 1) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(fileTreeStore.splitIndex);
});


const message = useMessage()
// 统一的空状态对象，避免工作台在首次打开或安装失败后出现字段缺失。
const createEmptyPythonRuntimeStatus = () => ({
  ready: false,
  source: '',
  python_path: '',
  runtime_dir: '',
  version: '',
  pip_version: '',
  worker_alive: false,
  packages: [],
  bundled_runtime_present: false,
  initialized: false,
  worker_script_ready: false,
  runtime_marker_path: '',
})
const pythonRuntimeStatusClass = computed(() => {
  const source = String(pythonRuntimeStatus.value?.source || '').trim()
  if (!pythonRuntimeStatus.value?.ready) return 'is-missing'
  if (source === 'bundled') return 'is-bundled'
  return 'is-system'
})
const pythonRuntimeSourceLabel = computed(() => {
  const source = String(pythonRuntimeStatus.value?.source || '').trim()
  if (source === 'bundled') return 'Guaxi 专属运行时'
  if (pythonRuntimeStatus.value?.initialized) return 'Guaxi 已接管运行时'
  if (source) return '等待导入 Guaxi 运行时'
  return '未检测'
})
const pythonRuntimeRuntimeLabel = computed(() => {
  if (pythonRuntimeStatus.value?.bundled_runtime_present) {
    return pythonRuntimeStatus.value?.initialized ? 'Guaxi 专属环境已就绪' : 'Guaxi 专属环境待初始化'
  }
  if (pythonRuntimeStatus.value?.ready) {
    return pythonRuntimeStatus.value?.initialized ? 'Guaxi 托管环境已就绪' : 'Guaxi 托管环境待初始化'
  }
  return '尚未接入'
})
const pythonRuntimeNeedsSetup = computed(() => {
  const status = pythonRuntimeStatus.value
  if (!status?.ready) return true
  if (status.source === 'bundled') return !status.initialized || !status.worker_script_ready
  return !status.initialized
})
const pythonRuntimeSummary = computed(() => {
  if (pythonRuntimeBusy.value) return '正在同步 Python 环境状态'
  if (!pythonRuntimeStatus.value?.ready) return '当前尚未检测到独立 Python 环境，可导入压缩包或初始化系统解释器'
  const sourceLabel = pythonRuntimeSourceLabel.value
  const workerLabel = pythonRuntimeStatus.value?.worker_alive ? 'Worker 已连接' : 'Worker 待启动'
  return `${sourceLabel} · ${workerLabel}`
})

const pythonRuntimeDisplayStatus = computed(() => {
  if (pythonRuntimeBusy.value) return '准备中'
  if (!pythonRuntimeStatus.value?.ready) return '不可用'
  if (!pythonRuntimeStatus.value?.initialized) return '待初始化'
  if (pythonRuntimeStatus.value?.worker_alive) return '运行中'
  return '已就绪'
})
const pythonRuntimeDisplaySummary = computed(() => {
  if (pythonRuntimeBusy.value) return '正在同步 Python 环境'
  if (!pythonRuntimeStatus.value?.ready) return '未检测到可用 Python'
  return `Guaxi Python · ${pythonRuntimeDisplayStatus.value}`
})
const pythonRuntimeStatusTone = computed(() => {
  if (!pythonRuntimeStatus.value?.ready) return 'is-danger'
  if (pythonRuntimeBusy.value || pythonRuntimeNeedsSetup.value) return 'is-warning'
  if (pythonRuntimeStatus.value?.worker_alive) return 'is-success'
  return 'is-neutral'
})
const pythonRuntimePackagesFiltered = computed(() => {
  const keyword = String(pythonRuntimePackageSearch.value || '').trim().toLowerCase()
  const packages = Array.isArray(pythonRuntimeStatus.value?.packages) ? pythonRuntimeStatus.value.packages : []
  if (!keyword) return packages
  return packages.filter((pkg) => String(pkg || '').toLowerCase().includes(keyword))
})
const pythonRuntimeSelectedPackageCount = computed(() => pythonRuntimeSelectedPackages.value.length)
const pythonRuntimeOperationSummary = computed(() => {
  if (!pythonRuntimeOperation.value?.active) return ''
  const packages = Array.isArray(pythonRuntimeOperation.value?.packages) ? pythonRuntimeOperation.value.packages : []
  const preview = packages.slice(0, 3).join(', ')
  return packages.length > 3 ? `${preview} 等 ${packages.length} 个包` : preview
})

let unlistenTemplateInsert = null
let templateInsertListenerUnmounted = false
const PENDING_TEMPLATE_INSERT_KEY = 'guaxi.pending_template_insert'
const TEMPLATE_INSERT_DEDUP_MS = 1500
let lastTemplateInsertPayload = ''
let lastTemplateInsertAt = 0

const isDuplicateTemplateInsert = (raw) => {
  const text = String(raw || '')
  if (!text) return false
  const now = Date.now()
  return text === lastTemplateInsertPayload && (now - lastTemplateInsertAt) < TEMPLATE_INSERT_DEDUP_MS
}

const markTemplateInsertApplied = (raw) => {
  lastTemplateInsertPayload = String(raw || '')
  lastTemplateInsertAt = Date.now()
}

const applyTemplateInsert = async (content) => {
  const raw = typeof content === 'string' ? content : ''
  if (!raw) return false
  if (isDuplicateTemplateInsert(raw)) return true
  const text = raw.endsWith('\n') ? raw : `${raw}\n`
  const inserted = !!(await editorStore.insertContent(text))
  if (inserted) {
    markTemplateInsertApplied(raw)
  }
  return inserted
}

const consumePendingTemplateInsert = async () => {
  try {
    const pending = localStorage.getItem(PENDING_TEMPLATE_INSERT_KEY) || ''
    if (!pending) return
    const inserted = await applyTemplateInsert(pending)
    if (inserted) {
      localStorage.removeItem(PENDING_TEMPLATE_INSERT_KEY)
    }
  } catch (e) {
    logger.debug('处理待插入模板内容失败:', e)
  }
}
const safeUnlistenTemplateInsert = () => {
  if (typeof unlistenTemplateInsert !== 'function') return
  try {
    Promise.resolve(unlistenTemplateInsert()).catch((e) => {
      logger.debug('反注册模板插入事件失败:', e)
    })
  } catch (e) {
    logger.debug('反注册模板插入事件失败:', e)
  }
  unlistenTemplateInsert = null
}

onMounted(async () => {
  templateInsertListenerUnmounted = false
  try {
    const unlisten = await listen('template_insert_event', async (event) => {
      const content = typeof event?.payload?.content === 'string' ? event.payload.content : ''
      if (!content) return

      const inserted = await applyTemplateInsert(content)
      if (!inserted) {
        localStorage.setItem(PENDING_TEMPLATE_INSERT_KEY, content)
        message.warning('插入模板失败：请先打开一个文件')
      } else {
        localStorage.removeItem(PENDING_TEMPLATE_INSERT_KEY)
      }
    })

    if (templateInsertListenerUnmounted) {
      try {
        Promise.resolve(unlisten()).catch((e) => {
          logger.debug('反注册模板插入事件失败:', e)
        })
      } catch (e) {
        logger.debug('反注册模板插入事件失败:', e)
      }
      return
    }

    unlistenTemplateInsert = unlisten
    await consumePendingTemplateInsert()
  } catch (e) {
    logger.error('注册模板插入事件失败:', e)
  }
})

onUnmounted(() => {
  templateInsertListenerUnmounted = true
  safeUnlistenTemplateInsert()
})

const headingOptions = [
  { label: '正文', key: 0 },
  { label: 'H1', key: 1 },
  { label: 'H2', key: 2 },
  { label: 'H3', key: 3 },
  { label: 'H4', key: 4 },
  { label: 'H5', key: 5 },
  { label: 'H6', key: 6 }
]

const codeBlockOptions = [
  { label: '纯代码块', key: 'plain' },
  { label: 'Python Cell', key: 'pycell' },
  { label: 'C', key: 'c' },
  { label: 'C++', key: 'cpp' },
  { label: 'Python', key: 'python' },
  { label: 'Shell', key: 'bash' },
  { label: 'Mermaid', key: 'mermaid' },
  { label: 'Rust', key: 'rust' },
  { label: 'JavaScript', key: 'javascript' },
  { label: 'TypeScript', key: 'typescript' },
  { label: 'Lua', key: 'lua' },
  { label: 'HTML', key: 'html' },
  { label: 'Perl', key: 'perl' },
  { label: 'JSON', key: 'json' },
  { label: 'XML', key: 'xml' },
  { label: 'TeX', key: 'tex' },
  { label: 'C#', key: 'csharp' },
  { label: 'Lisp', key: 'lisp' },
  { label: 'TOML', key: 'toml' },
  { label: 'PowerShell', key: 'powershell' }
]

const tableInsertPopoverVisible = ref(false)
const tableGridMaxRows = 10
const tableGridMaxCols = 10
const tableGridRows = ref(2)
const tableGridCols = ref(2)
const tableGridDragging = ref(false)

const tableInsertMaxRows = 120
const tableInsertMaxCols = 40
const tableInputRows = ref(2)
const tableInputCols = ref(2)

const tableInsertHint = computed(() => `${tableGridRows.value} 行 × ${tableGridCols.value} 列`)

const handleTableInsertPopoverUpdateShow = (val) => {
  tableInsertPopoverVisible.value = val
  if (val) {
    tableGridRows.value = 2
    tableGridCols.value = 2
    tableGridDragging.value = false
    tableInputRows.value = 2
    tableInputCols.value = 2
  } else {
    tableGridDragging.value = false
  }
}

const clampInt = (value, min, max) => {
  const n = Number(value)
  if (!Number.isFinite(n)) return min
  return Math.max(min, Math.min(max, Math.floor(n)))
}

const buildMarkdownTable = (rows, cols) => {
  const r = clampInt(rows, 2, tableInsertMaxRows)
  const c = clampInt(cols, 1, tableInsertMaxCols)

  const header = Array.from({ length: c }, (_, i) => `列${i + 1}`)
  const sep = Array.from({ length: c }, () => '---')

  const lines = []
  lines.push(`| ${header.join(' | ')} |`)
  lines.push(`| ${sep.join(' | ')} |`)

  const bodyRows = Math.max(1, r - 1)
  for (let i = 0; i < bodyRows; i += 1) {
    const cells = Array.from({ length: c }, (_, idx) => (i === 0 ? `内容${idx + 1}` : ''))
    lines.push(`| ${cells.join(' | ')} |`)
  }

  return `${lines.join('\n')}\n`
}

const insertTableWithSize = (rows, cols) => {
  if (!ensureMarkdownEditor()) return

  const model = editor.getModel()
  const pos = editor.getPosition()
  if (!model || !pos) return

  lastTableToolbarInsertAt = Date.now();

  const snippet = buildMarkdownTable(rows, cols)
  insertBlockSnippet(snippet)

  nextTick(() => {
    const line = pos.lineNumber + 2
    editor.setPosition(new monaco.Position(line, 3))
    editor.focus()
    updateTableToolbar()
    formatTable()
  })
}

const handleTableGridMouseDown = (r, c) => {
  tableGridDragging.value = true
  tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
  tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
}

const handleTableGridMouseOver = (r, c) => {
  if (!tableGridDragging.value) {
    tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
    tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
    return
  }
  tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
  tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
}

const handleTableGridMouseUp = () => {
  if (!tableInsertPopoverVisible.value) return
  if (!tableGridDragging.value) return
  tableGridDragging.value = false

  insertTableWithSize(tableGridRows.value, tableGridCols.value)
  tableInsertPopoverVisible.value = false
}

const handleTableGridMouseLeave = () => {
  if (!tableGridDragging.value) return
  tableGridDragging.value = false
}

const insertTableFromInputs = () => {
  const r = clampInt(tableInputRows.value, 2, tableInsertMaxRows)
  const c = clampInt(tableInputCols.value, 1, tableInsertMaxCols)
  insertTableWithSize(r, c)
  tableInsertPopoverVisible.value = false
}

const vimMode = ref(false);
const vimStatusBar = ref(null);
let vimModeInstance = null;

const applyVimOverlayInsets = () => {
  const baseTop = 8;
  const baseBottom = 8;
  const vimH = vimMode.value
    ? Math.ceil(vimStatusBar.value?.getBoundingClientRect?.()?.height || 0)
    : 0;
  const bottom = baseBottom + (vimH ? vimH + 6 : 0);

  const opt = {
    padding: {
      top: baseTop,
      bottom
    }
  };

  if (editor) {
    editor.updateOptions(opt);
    editor.layout();
  }
  if (secondaryEditor) {
    secondaryEditor.updateOptions(opt);
    secondaryEditor.layout();
  }
};

let markdownFeaturesRegistered = false;

let filesystemPathCompletionRegistered = false;
const markdownLanguageFeatureDisposables = []
const filesystemPathCompletionDisposables = []
const MARKDOWN_LANGUAGE_FEATURES_REGISTRY_KEY = '__guaxi_markdown_language_features_registry__'
const MARKDOWN_LANGUAGE_FEATURES_VERSION = '2026-05-22-embedded-completion-v8'
const disposeFeatureDisposables = (items) => {
  if (!Array.isArray(items) || !items.length) return
  while (items.length) {
    const disposable = items.pop()
    try {
      disposable?.dispose?.()
    } catch {
    }
  }
}
const getGlobalLanguageFeatureRegistryHost = () => {
  try {
    return globalThis
  } catch {
    return null
  }
}
const prepareMarkdownLanguageFeatureRegistration = () => {
  const host = getGlobalLanguageFeatureRegistryHost()
  const registry = host?.[MARKDOWN_LANGUAGE_FEATURES_REGISTRY_KEY]

  if (
    registry?.version === MARKDOWN_LANGUAGE_FEATURES_VERSION
    && Array.isArray(registry.disposables)
    && registry.disposables.length
  ) {
    markdownFeaturesRegistered = true
    logger.debug('[MarkdownCompletion] reuse global language feature registration', {
      version: registry.version,
      disposableCount: registry.disposables.length,
    })
    return false
  }

  if (Array.isArray(registry?.disposables) && registry.disposables.length) {
    // 设计说明：
    // 1. Monaco 的 language provider 是进程级全局注册，不跟 Vue 组件或 Vite HMR 模块生命周期自动绑定。
    // 2. 热更新/重挂载后如果旧 provider 没有释放，Monaco 会把多套 provider 的候选合并，表现为 `include` 成对重复。
    // 3. 注册新版本前先清理全局旧版本，保证 Markdown fenced code completion 始终只有一套宿主映射 provider。
    logger.warn('[MarkdownCompletion] dispose stale global language feature registration', {
      staleVersion: registry.version || '',
      nextVersion: MARKDOWN_LANGUAGE_FEATURES_VERSION,
      staleDisposableCount: registry.disposables.length,
    })
    disposeFeatureDisposables(registry.disposables)
  }

  disposeFeatureDisposables(markdownLanguageFeatureDisposables)
  markdownFeaturesRegistered = false
  return true
}
const publishMarkdownLanguageFeatureRegistration = () => {
  const host = getGlobalLanguageFeatureRegistryHost()
  if (!host) return

  host[MARKDOWN_LANGUAGE_FEATURES_REGISTRY_KEY] = {
    version: MARKDOWN_LANGUAGE_FEATURES_VERSION,
    disposables: markdownLanguageFeatureDisposables,
  }
  logger.info('[MarkdownCompletion] published global language feature registration', {
    version: MARKDOWN_LANGUAGE_FEATURES_VERSION,
    disposableCount: markdownLanguageFeatureDisposables.length,
  })
}
const clearGlobalMarkdownLanguageFeatureRegistration = () => {
  const host = getGlobalLanguageFeatureRegistryHost()
  const registry = host?.[MARKDOWN_LANGUAGE_FEATURES_REGISTRY_KEY]
  if (registry?.disposables === markdownLanguageFeatureDisposables) {
    delete host[MARKDOWN_LANGUAGE_FEATURES_REGISTRY_KEY]
    logger.debug('[MarkdownCompletion] cleared global language feature registration', {
      version: registry.version || '',
    })
  }
}
const filesystemPathCompletionCache = new Map();
const filesystemPathCompletionInflight = new Map();
let filesystemPathCompletionDocPath = '';
let filesystemPathCompletionDocDir = '';
let filesystemPathCompletionHomeDir = '';

const normalizeFsPath = (p) => {
  let s = (p || '').replace(/\\/g, '/');
  // Fix Windows drive path like `/C:/...` which may break relative completion like `../`
  if (/^\/[a-zA-Z]:\//.test(s)) {
    s = s.slice(1);
  }
  return s;
};

const joinFsPath = (base, sub) => {
  const a = normalizeFsPath(base).replace(/\/+$/, '');
  const b = normalizeFsPath(sub).replace(/^\/+/, '');
  if (!a) return b;
  if (!b) return a;
  return `${a}/${b}`;
};

const stripLinkBoundaryPunct = (raw) => {
  let s = String(raw || '').trim();
  if (!s) return '';
  s = s.replace(/^(["'`<({\[])+/, '');
  s = s.replace(/(["'`>)}\].,;!?])+$/, '');
  return s;
};

const isHttpUrl = (raw) => {
  const s = String(raw || '').trim().toLowerCase();
  return s.startsWith('http://') || s.startsWith('https://');
};

// 从 `path.md#标题` / `path.md#^blockId` 中拆出 anchor（仅用于非 http 链接）
const splitFsLinkAndAnchor = (rawLink) => {
  const raw = stripLinkBoundaryPunct(rawLink);
  if (!raw) return { link: '', anchor: '' };
  if (isHttpUrl(raw)) return { link: raw, anchor: '' };

  const idx = raw.indexOf('#');
  if (idx <= 0) return { link: raw, anchor: '' };
  return { link: raw.slice(0, idx), anchor: raw.slice(idx + 1) };
};

// 说明：为避免误判普通文本，仅对“明显路径特征”放行；另外允许 `xxx.md` 这种相对文件名。
const isPathLikeToken = (raw) => {
  const s0 = String(raw || '').trim();
  if (!s0) return false;
  const s = stripLinkBoundaryPunct(s0);
  if (!s) return false;

  const lower = s.toLowerCase();
  if (lower.startsWith('file://')) return true;
  if (s.startsWith('~/') || s.startsWith('./') || s.startsWith('../')) return true;
  if (/^[a-zA-Z]:[\\/]/.test(s)) return true;
  if (s.includes('/') || s.includes('\\')) return true;
  return /\.(md|markdown)$/i.test(s);
};

// Markdown 链接目标提取：当光标位于 `](` 与 `)` 之间时，返回括号内真实目标
const extractMarkdownLinkTargetAtLineColumn = (line, idx0) => {
  const i = Math.max(0, Math.min(line.length, idx0));

  const left = line.lastIndexOf('](', i);
  if (left >= 0) {
    const open = left + 1; // '(' index
    const close = line.indexOf(')', open + 1);
    if (close > open + 1 && i >= open + 1 && i <= close - 1) {
      return line.slice(open + 1, close).trim();
    }
  }

  const lt = line.lastIndexOf('<', i);
  if (lt >= 0) {
    const gt = line.indexOf('>', lt + 1);
    if (gt > lt + 1 && i >= lt + 1 && i <= gt - 1) {
      return line.slice(lt + 1, gt).trim();
    }
  }

  return '';
};

// 统一生成内部 wikilink href（给 Monaco Ctrl+Click / Preview / Realtime 共用）
const buildGuaxiWikilinkHref = (filePart, anchorPart) => {
  const file = String(filePart || '').trim();
  const anchor = String(anchorPart || '').trim();
  return `guaxi://wikilink?file=${encodeURIComponent(file)}&anchor=${encodeURIComponent(anchor)}`;
};

// Obsidian 双链提取：当光标位于 `[[...]]` 内时，返回内部可点击 href
// 支持：[[file]] / [[file#heading]] / [[file#^blockId]] / [[#heading]] / [[file|alias]]
const extractObsidianWikilinkHrefAtLineColumn = (line, idx0) => {
  const i = Math.max(0, Math.min(line.length, idx0));
  const left = line.lastIndexOf('[[', i);
  if (left < 0) return '';
  const right = line.indexOf(']]', left + 2);
  if (right < 0) return '';
  if (i < left + 2 || i > right - 1) return '';

  const inner = String(line.slice(left + 2, right) || '').trim();
  if (!inner) return '';

  const parts = inner.split('|');
  const target = String(parts[0] || '').trim();
  if (!target) return '';

  let filePart = target;
  let anchorPart = '';
  const hashIdx = target.indexOf('#');
  if (hashIdx >= 0) {
    filePart = target.slice(0, hashIdx).trim();
    anchorPart = target.slice(hashIdx + 1).trim();
  }

  return buildGuaxiWikilinkHref(filePart, anchorPart);
};

// Monaco 行内 token 提取：优先走 Obsidian/Markdown 语法提取，避免把 `xxx](` 左侧吞进来
const extractClickableTokenAtLineColumn = (lineContent, column) => {
  const line = String(lineContent || '');
  const idx0 = Math.max(0, Math.min(line.length, Number(column || 1) - 1));
  if (!line) return '';

  const wikiHref = extractObsidianWikilinkHrefAtLineColumn(line, idx0);
  if (wikiHref) return wikiHref;

  const mdTarget = extractMarkdownLinkTargetAtLineColumn(line, idx0);
  if (mdTarget) return mdTarget;

  // 注意：这里刻意不把 `[` `]` 当作 token 字符，避免得到类似 `path](../a.md` 的 token
  const isLinkChar = (ch) => /[0-9A-Za-z\-._~:/?#@!$&'()*+,;=%\\]/.test(ch);

  let l = idx0;
  while (l > 0 && isLinkChar(line[l - 1])) l -= 1;

  let r = idx0;
  while (r < line.length && isLinkChar(line[r])) r += 1;

  if (r <= l) return '';
  return line.slice(l, r);
};

const isMarkdownFilePath = (p) => {
  const v = String(p || '').toLowerCase();
  return v.endsWith('.md') || v.endsWith('.markdown');
};

const ensureMarkdownExt = (p) => {
  const s = String(p || '').trim();
  if (!s) return '';
  if (isMarkdownFilePath(s)) return s;
  const lastSlash = Math.max(s.lastIndexOf('/'), s.lastIndexOf('\\'));
  const lastDot = s.lastIndexOf('.');
  const hasExt = lastDot > lastSlash;
  if (hasExt) return s;
  return `${s}.md`;
};

const ensureKnowledgeBasesLoaded = async () => {
  const list = knowledgeBaseStore?.knowledgeBases;
  if (Array.isArray(list) && list.length) return list;
  try {
    const next = await knowledgeBaseStore.fetchKnowledgeBases();
    logger.debug('[LinkOpen] knowledge bases loaded', { count: Array.isArray(next) ? next.length : 0 });
    return next;
  } catch (e) {
    logger.warn('[LinkOpen] fetchKnowledgeBases failed', e);
    return [];
  }
};

const isPathInKnowledgeBase = async (absPath) => {
  const p = normalizeFsPath(absPath);
  if (!p) return false;
  const list = await ensureKnowledgeBasesLoaded();
  for (const kb of Array.isArray(list) ? list : []) {
    const root = normalizeFsPath(kb?.location);
    if (root && (p === root || p.startsWith(`${root}/`))) return true;
  }
  return false;
};

const decodeFileUrlToPath = (href) => {
  const s = String(href || '').trim();
  if (!s.toLowerCase().startsWith('file://')) return '';
  try {
    let rest = s.replace(/^file:\/\//i, '');
    if (rest.startsWith('/')) rest = rest.slice(1);
    return decodeURIComponent(rest);
  } catch {
    return '';
  }
};

// 将 path 规范化（处理 `.`/`..`，并尽量保留 Windows 盘符/UNC 前缀）
const normalizeResolvedFsPath = (rawPath) => {
  let s = normalizeFsPath(rawPath);
  if (!s) return '';

  // 兼容 `/C:/...`
  if (/^\/[a-zA-Z]:\//.test(s)) s = s.slice(1);

  let anchored = false;
  let prefix = '';
  let parts = [];

  if (/^[a-zA-Z]:\//.test(s)) {
    anchored = true;
    prefix = s.slice(0, 3); // `C:/`
    parts = s.slice(3).split('/');
  } else if (s.startsWith('//')) {
    const p = s.slice(2).split('/').filter(Boolean);
    if (p.length >= 2) {
      anchored = true;
      prefix = `//${p[0]}/${p[1]}`;
      parts = p.slice(2);
    } else {
      return s;
    }
  } else if (s.startsWith('/')) {
    anchored = true;
    prefix = '/';
    parts = s.slice(1).split('/');
  } else {
    parts = s.split('/');
  }

  const out = [];
  for (const seg0 of parts) {
    const seg = String(seg0 || '').trim();
    if (!seg || seg === '.') continue;
    if (seg === '..') {
      if (out.length && out[out.length - 1] !== '..') {
        out.pop();
      } else if (!anchored) {
        out.push('..');
      }
      continue;
    }
    out.push(seg);
  }

  if (!out.length) return prefix;
  if (!prefix) return out.join('/');
  return prefix.endsWith('/') ? `${prefix}${out.join('/')}` : `${prefix}/${out.join('/')}`;
};

const resolveFsLinkToAbsolute = async (rawLink, { baseDocPath } = {}) => {
  const raw = stripLinkBoundaryPunct(rawLink);
  if (!raw) return '';

  const fileUrlDecoded = decodeFileUrlToPath(raw);
  const link = normalizeFsPath(fileUrlDecoded || raw);
  if (!link) return '';

  const isAbs = /^[a-zA-Z]:\//.test(link) || link.startsWith('/') || link.startsWith('\\\\');
  if (isAbs) return normalizeResolvedFsPath(link);

  if (link.startsWith('~/')) {
    try {
      const base = await getFilesystemCompletionHomeDir();
      return normalizeResolvedFsPath(joinFsPath(base, link.slice(2)));
    } catch {
      return '';
    }
  }

  let baseDir = '';
  if (baseDocPath) {
    try {
      baseDir = await dirname(baseDocPath);
    } catch (e) {
      logger.warn('[LinkOpen] dirname(baseDocPath) failed, fallback to getFilesystemCompletionDocDir', { baseDocPath }, e);
      baseDir = '';
    }
  }
  if (!baseDir) {
    baseDir = await getFilesystemCompletionDocDir();
  }

  return normalizeResolvedFsPath(joinFsPath(baseDir, link));
};

const findMarkdownAnchorLine = (content, anchorRaw) => {
  const anchor = String(anchorRaw || '').trim();
  if (!anchor) return 1;

  const lines = String(content || '').replace(/\r/g, '').split('\n');

  if (anchor.startsWith('^')) {
    const id = anchor.slice(1);
    if (!id) return 1;
    const re = new RegExp(`\\^${id.replace(/[.*+?^${}()|[\\]\\\\]/g, '\\$&')}\\b`);
    for (let i = 0; i < lines.length; i += 1) {
      if (re.test(lines[i])) return i + 1;
    }
    return 1;
  }

  const norm = (s) => String(s || '').trim().toLowerCase().replace(/\s+/g, ' ');
  const want = norm(anchor.replace(/_/g, ' '));

  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^\s{0,3}#{1,6}\s+(.*)$/);
    if (!m) continue;
    const got = norm(m[1].replace(/#+\s*$/, ''));
    if (got === want) return i + 1;
  }

  return 1;
};

const findFootnoteDefinitionLine = (content, numRaw) => {
  const num = String(numRaw || '').trim();
  if (!num) return 0;

  const lines = String(content || '').replace(/\r/g, '').split('\n');
  const esc = num.replace(/[.*+?^${}()|[\\]\\\\]/g, '\\$&');
  const re = new RegExp(`^\\s*\\[\\s*${esc}\\s*\\]\\s*\\(@ref\\)\\s*:`, 'i');

  for (let i = 0; i < lines.length; i += 1) {
    if (re.test(lines[i])) return i + 1;
  }
  return 0;
};

const jumpToFootnoteDefinition = (num, { source = 'preview' } = {}) => {
  const content = editor?.getModel?.()?.getValue?.() ?? rawContent.value ?? '';
  const line = findFootnoteDefinitionLine(content, num);
  if (!line) {
    logger.debug('[Footnote] definition not found', { num, source });
    return false;
  }

  recordPreviewLocation('footnote');
  scrollPreviewToLine(line);
  if (showEditor.value) {
    syncEditorToLine(line, { focus: false });
  }

  logger.info('[Footnote] jumped to definition', { num, line, source });
  return true;
};

// ==================== Markdown 任务列表（Task List）交互 ====================
// 目标：预览模式点击 checklist 时：
// 1) 回写源 Markdown：- [ ] <-> - [x]
// 2) 不触发整篇 Markdown 的重渲染（大文件会卡顿）；只更新被点击条目的 DOM
// 注意：
// - 回写优先使用 Monaco 的 executeEdits 做最小范围替换，保留 undo/redo 以及光标状态。
// - 为避免 watch(rawContent) 触发 renderDebounced() 重渲染，使用一次性“抑制自动渲染”标记。
let suppressPreviewAutoRenderState = { until: 0, reason: '' };

const suppressPreviewAutoRenderOnce = (reason, ttlMs = 900) => {
  const ttl = Math.max(120, Number(ttlMs || 0) || 0);
  suppressPreviewAutoRenderState = {
    until: Date.now() + ttl,
    reason: String(reason || 'unknown')
  };
  logger.debug('[TaskList] suppress preview auto render', { reason: suppressPreviewAutoRenderState.reason, ttlMs: ttl });
};

const consumeSuppressPreviewAutoRender = (from) => {
  const now = Date.now();
  if (!suppressPreviewAutoRenderState?.until) return false;
  if (now > suppressPreviewAutoRenderState.until) {
    suppressPreviewAutoRenderState = { until: 0, reason: '' };
    return false;
  }

  logger.debug('[TaskList] preview auto render suppressed', {
    from: String(from || ''),
    reason: suppressPreviewAutoRenderState.reason,
    remainMs: suppressPreviewAutoRenderState.until - now
  });
  suppressPreviewAutoRenderState = { until: 0, reason: '' };
  return true;
};

const toggleMarkdownTaskAtLine = (lineNumber, checked, { source = 'preview' } = {}) => {
  const ln = Number(lineNumber || 0);
  if (!ln || ln < 1) return false;

  const desired = checked ? 'x' : ' ';
  const taskRe = /^(\s*(?:[-+*]|\d+[.)])\s+)\[( |x|X)\](\s+)/;

  // 预览点击触发的“回写源 Markdown”属于局部更新：抑制一次自动渲染，避免整篇重渲染。
  // 备注：抑制逻辑由 watch(rawContent) 处消费。
  suppressPreviewAutoRenderOnce(`task-toggle:${source}`, 1200);

  const targetEditor = editor;
  const model = targetEditor?.getModel?.();
  const lineText = model ? model.getLineContent(ln) : (String(rawContent.value || '').replace(/\r/g, '').split('\n')[ln - 1] || '');

  const m = taskRe.exec(String(lineText || ''));
  if (!m) {
    logger.debug('[TaskList] no task marker on line', { ln, source, lineText: String(lineText || '').slice(0, 120) });
    return false;
  }

  const current = String(m[2] || ' ');
  const nextChar = desired;
  if (current.toLowerCase() === nextChar) {
    logger.debug('[TaskList] already desired', { ln, checked, source });
    return true;
  }

  // '[' 的 0-based index = m[1].length；括号内字符的位置 = index + 1
  const insideIdx0 = String(m[1] || '').length + 1;

  if (model && targetEditor) {
    try {
      targetEditor.executeEdits('md-task-toggle', [
        {
          range: new monaco.Range(ln, insideIdx0 + 1, ln, insideIdx0 + 2),
          text: nextChar,
          forceMoveMarkers: true
        }
      ]);
      logger.info('[TaskList] toggled', { ln, checked, source });
      return true;
    } catch (e) {
      logger.warn('[TaskList] executeEdits failed, fallback to full replace', { ln, checked, source }, e);
    }
  }

  try {
    const lines = String(rawContent.value || '').replace(/\r/g, '').split('\n');
    if (ln > lines.length) return false;
    const src = String(lines[ln - 1] || '');
    const m2 = taskRe.exec(src);
    if (!m2) return false;

    const head = src.slice(0, m2[1].length + 1);
    const tail = src.slice(m2[1].length + 2);
    lines[ln - 1] = `${head}${nextChar}${tail}`;
    updateEditorContent(lines.join('\n'));

    logger.info('[TaskList] toggled via fallback replace', { ln, checked, source });
    return true;
  } catch (e) {
    logger.error('[TaskList] fallback replace failed', { ln, checked, source }, e);
    return false;
  }
};

const openPathInExplorer = async (absPath) => {
  const p = normalizeFsPath(absPath);
  if (!p) return;
  try {
    await invoke('open_in_file_explorer', { path: p });
  } catch (e) {
    logger.error('[LinkOpen] open_in_file_explorer failed', { path: p }, e);
    message.error('打开文件资源管理器失败');
  }
};

const openMarkdownInEditor = async (absPath, anchor, { silent = false, focus = true } = {}) => {
  const p = normalizeFsPath(absPath);
  if (!p) return false;

  try {
    // 先读取内容确认文件可打开（并避免 openFileInTab 内部失败但不抛错导致无法做 fallback）。
    const res = await fileTreeStore.fetchFileContent(p);
    if (!res?.ok) {
      logger.warn('[LinkOpen] openMarkdownInEditor: fetchFileContent failed', { path: p, anchor, code: res?.code, error: res?.error });
      if (!silent) message.error('打开文件失败');
      return false;
    }

    // 用已读取的内容打开/激活标签页，避免重复读取。
    await fileTreeStore.openFileInTab(p, res.content, { unsupported: !!res.unsupported });
    await nextTick();

    const content = String(res.content || fileTreeStore.selectedFile?.content || '');
    const line = findMarkdownAnchorLine(content, anchor);
    syncEditorToLine(line, { focus });
    return true;
  } catch (e) {
    logger.error('[LinkOpen] openMarkdownInEditor failed', { path: p, anchor }, e);
    if (!silent) message.error('打开文件失败');
    return false;
  }
};

// Obsidian 风格：[[Note]] 这种不带路径的文件名，允许在知识库内按 basename 做一次兜底查找。
// 说明：前端没有轻量 exists API，这里通过 get_file_tree 做“限量 BFS”，只在相对解析失败时触发。
const isBareWikilinkFileName = (filePart) => {
  const s = String(filePart || '').trim();
  if (!s) return false;
  if (s.startsWith('./') || s.startsWith('../') || s.startsWith('~/')) return false;
  if (s.includes('/') || s.includes('\\')) return false;
  if (/^[a-zA-Z]:[\\/]/.test(s)) return false;
  if (s.toLowerCase().startsWith('file://')) return false;
  return true;
};

const findMarkdownByBasenameInRepo = async (filePart, { repoRoot, maxDirs = 300 } = {}) => {
  const root = normalizeFsPath(repoRoot);
  if (!root) return '';

  const wantRaw = String(filePart || '').trim();
  if (!wantRaw) return '';

  const wantWithExt = ensureMarkdownExt(wantRaw);
  const wantNames = new Set([String(wantRaw).toLowerCase(), String(wantWithExt).toLowerCase()]);

  const queue = [root];
  const visited = new Set();
  let walked = 0;

  while (queue.length) {
    const dir0 = queue.shift();
    const dir = normalizeFsPath(dir0);
    if (!dir || visited.has(dir)) continue;
    visited.add(dir);

    walked += 1;
    if (walked > maxDirs) {
      logger.warn('[LinkOpen] wikilink basename search aborted: too many dirs', { repoRoot: root, want: wantRaw, maxDirs });
      return '';
    }

    let children = [];
    try {
      children = await invoke('get_file_tree', { path: dir });
    } catch (e) {
      logger.warn('[LinkOpen] wikilink basename search: get_file_tree failed', { dir }, e);
      continue;
    }

    for (const node of Array.isArray(children) ? children : []) {
      const id = normalizeFsPath(node?.id);
      const name = String(node?.name || '').trim();

      if (node?.is_directory) {
        if (id) queue.push(id);
        continue;
      }

      if (!id || !name) continue;
      if (!/\.(md|markdown)$/i.test(name)) continue;

      if (wantNames.has(name.toLowerCase())) {
        logger.info('[LinkOpen] wikilink basename search hit', { want: wantRaw, path: id, walked });
        return id;
      }
    }
  }

  logger.info('[LinkOpen] wikilink basename search miss', { want: wantRaw, repoRoot: root, walked });
  return '';
};

const openWikilinkFromHref = async (href, { baseDocPath, source } = {}) => {
  const src = String(source || 'unknown');
  const baseDoc = normalizeFsPath(baseDocPath || '');

  try {
    const u = new URL(String(href || '').trim());
    const file = stripLinkBoundaryPunct(u.searchParams.get('file') || '');
    const anchor = stripLinkBoundaryPunct(u.searchParams.get('anchor') || '');

    logger.info('[LinkOpen] open wikilink', { source: src, file, anchor, baseDocPath: baseDoc });

    // [[#heading]]：不带 file 时视为当前文件。
    if (!file) {
      if (baseDoc) return await openMarkdownInEditor(baseDoc, anchor);
      return false;
    }

    // 1) 优先按“当前文档所在目录”解析（Obsidian 默认行为）。
    const primaryAbs = await resolveFsLinkToAbsolute(ensureMarkdownExt(file), { baseDocPath: baseDoc });
    if (await openMarkdownInEditor(primaryAbs, anchor, { silent: true })) return true;

    // 2) 若是纯文件名（不含路径），在知识库内做一次限量 basename 搜索兜底。
    if (isBareWikilinkFileName(file)) {
      let repoRoot = normalizeFsPath(fileTreeStore.activeRepoPath || '');
      if (!repoRoot && baseDoc) {
        const list = await ensureKnowledgeBasesLoaded();
        for (const kb of Array.isArray(list) ? list : []) {
          const root = normalizeFsPath(kb?.location);
          if (root && (baseDoc === root || baseDoc.startsWith(`${root}/`))) {
            repoRoot = root;
            break;
          }
        }
      }

      const hit = await findMarkdownByBasenameInRepo(file, { repoRoot });
      if (hit && (await openMarkdownInEditor(hit, anchor, { silent: true }))) return true;
    }

    // 3) 最终失败：给用户一次明确反馈。
    logger.warn('[LinkOpen] wikilink target not found', { source: src, file, anchor, baseDocPath: baseDoc, primaryAbs });
    message.error('未找到目标文件');
    return false;
  } catch (err) {
    logger.warn('[LinkOpen] parse/open wikilink failed', { href, source: src, baseDocPath: baseDoc }, err);
    return false;
  }
};

const openFsLinkSmart = async (rawLink, { anchor, viaCtrl = false, baseDocPath } = {}) => {
  const abs = await resolveFsLinkToAbsolute(rawLink, { baseDocPath });
  if (!abs) return;

  const isMd = isMarkdownFilePath(abs) || isMarkdownFilePath(ensureMarkdownExt(abs));
  const mdPath = isMd ? ensureMarkdownExt(abs) : abs;

  if (isMd && (await isPathInKnowledgeBase(mdPath))) {
    await openMarkdownInEditor(mdPath, anchor);
    return;
  }

  if (viaCtrl) {
    await openPathInExplorer(abs);
  }
};

// Monaco：编辑模式 Ctrl+Click 打开 URL/路径
const installMonacoCtrlClickOpen = (targetEditor, { tag = 'left' } = {}) => {
  if (!targetEditor) return null;

  const disposable = targetEditor.onMouseDown((ev) => {
    const ctrl = !!(ev?.event?.ctrlKey || ev?.event?.metaKey);
    if (!ctrl) return;

    const pos = ev?.target?.position;
    const model = targetEditor.getModel?.();
    if (!pos || !model) return;

    const lineContent = model.getLineContent(pos.lineNumber);
    const token0 = extractClickableTokenAtLineColumn(lineContent, pos.column);
    const token = stripLinkBoundaryPunct(token0);
    if (!token) return;

    const lower = String(token || '').trim().toLowerCase();
    const isWikilink = lower.startsWith('guaxi://wikilink');
    const isUrl = isHttpUrl(token);
    const isPath = !isUrl && !isWikilink && isPathLikeToken(token);
    if (!isUrl && !isPath && !isWikilink) return;

    try {
      ev.event?.preventDefault?.();
      ev.event?.stopPropagation?.();
      ev.event?.browserEvent?.preventDefault?.();
      ev.event?.browserEvent?.stopPropagation?.();
    } catch {
      // ignore
    }

    const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
    logger.info('[LinkOpen] monaco ctrl+click', {
      tag,
      token,
      baseDocPath,
      line: pos.lineNumber,
      column: pos.column
    });

    (async () => {
      if (isWikilink) {
        await openWikilinkFromHref(token, { baseDocPath, source: `monaco:${tag}` });
        return;
      }

      if (isUrl) {
        try {
          await openUrl(token);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href: token }, err);
          message.error('打开浏览器失败');
        }
        return;
      }

      const { link, anchor } = splitFsLinkAndAnchor(token);
      if (!link) return;
      await openFsLinkSmart(link, { anchor, viaCtrl: true, baseDocPath });
    })();
  });

  logger.debug('[LinkOpen] monaco ctrl+click handler installed', { tag });
  return disposable;
};
const installMonacoLinkLikeDecorations = (targetEditor, { tag = 'left' } = {}) => {
  if (!targetEditor) return null;

  const classLinkLike = 'guaxi-link-like';
  const classMdBracket = 'guaxi-mdlink-bracket';
  const classMdAngle = 'guaxi-mdlink-angle';
  const classMdText = 'guaxi-mdlink-text';
  const classMdParen = 'guaxi-mdlink-paren';
  const classMdTarget = 'guaxi-mdlink-target';
  const classMdRefTarget = 'guaxi-mdref-target';
  const classWikiBracket = 'guaxi-wikilink-bracket';
  const classWikiText = 'guaxi-wikilink-text';

  let decorationIds = [];
  let timerId = null;

  const wikiRe = /\[\[[^\]]+?\]\]/g;
  const mdInlineLinkRe = /\[([^\]\n]+?)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
  const mdRefLinkRe = /\[([^\]\n]+?)\]\s*\[([^\]\n]*?)\]/g;
  const mdAutoLinkUrlRe = /<((?:https?|ftp):\/\/[^>\s]+)>/g;
  const mdAutoLinkEmailRe = /<((?:mailto:)?[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*)>/g;
  const mdFootnoteRefRe = /\[(\d+)\]\s*\(@ref\)/g;
  const urlRe = /(?:https?:\/\/|file:\/\/)[0-9A-Za-z\-._~:\/?#@!$&'()*+,;=%\\]+/g;
  const pathRe = /(?:\.\.\/|\.\/|~\/|[a-zA-Z]:[\\/]|\\\\)[0-9A-Za-z\-._~:\/?#@!$&'()*+,;=%\\]+/g;

  const isFence = (text) => /^\s*(```+|~~~+)/.test(String(text || ''));

  const update = (reason) => {
    const model = targetEditor.getModel?.();
    if (!model || model.getLanguageId?.() !== 'markdown') {
      if (decorationIds.length) decorationIds = targetEditor.deltaDecorations(decorationIds, []);
      return;
    }

    const ranges = targetEditor.getVisibleRanges?.() || [];
    if (!ranges.length) return;

    let startLine = Math.max(1, Math.min(...ranges.map((r) => r.startLineNumber)) - 80);
    let endLine = Math.min(model.getLineCount(), Math.max(...ranges.map((r) => r.endLineNumber)) + 80);

    const scanBack = 500;
    const scanStart = Math.max(1, startLine - scanBack);

    let inFence = false;
    let fenceToken = '';

    for (let ln = scanStart; ln < startLine; ln += 1) {
      const t = model.getLineContent(ln);
      const m = /^\s*(```+|~~~+)/.exec(t);
      if (!m) continue;
      const token = m[1];
      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
        inFence = false;
        fenceToken = '';
      }
    }

    const decs = [];

    for (let ln = startLine; ln <= endLine; ln += 1) {
      const line = model.getLineContent(ln);
      const fenceHit = /^\s*(```+|~~~+)/.exec(line);
      if (fenceHit) {
        const token = fenceHit[1];
        if (!inFence) {
          inFence = true;
          fenceToken = token;
        } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
          inFence = false;
          fenceToken = '';
        }
        continue;
      }
      if (inFence) continue;

      wikiRe.lastIndex = 0;
      for (const m of line.matchAll(wikiRe)) {
        const raw = String(m[0] || '');
        if (!raw) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        if (raw.length < 4) continue;

        // Obsidian 双链：[[...]]
        // 目标：括号用次要色，内部文本用链接色，避免 "[[" 左侧第二个括号颜色割裂。
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, start + 2 + 1),
          options: {
            inlineClassName: classWikiBracket,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
        decs.push({
          range: new monaco.Range(ln, end - 2 + 1, ln, end + 1),
          options: {
            inlineClassName: classWikiBracket,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
        if (end - start > 4) {
          decs.push({
            range: new monaco.Range(ln, start + 2 + 1, ln, end - 2 + 1),
            options: {
              inlineClassName: classWikiText,
              stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
            }
          });
        }
      }

      // Markdown 内联链接：[text](target "title")
      // 目标：括号符号易区分（#f35f4c），链接文本用独立色，URL/路径目标用链接色且仅目标下划线。
      mdInlineLinkRe.lastIndex = 0;
      for (const m of line.matchAll(mdInlineLinkRe)) {
        const rawText = String(m[1] || '');
        const rawTarget = String(m[2] || '');
        if (!rawTarget) continue;

        const token = stripLinkBoundaryPunct(rawTarget);
        if (!token) continue;
        if (!isPathLikeToken(token) && !isHttpUrl(token)) continue;

        const whole = String(m[0] || '');
        const startWhole = Number(m.index || 0);

        const closeBracketRel = whole.indexOf(']');
        if (closeBracketRel < 1) continue;

        const openParenRel = whole.indexOf('(', closeBracketRel);
        const closeParenRel = whole.lastIndexOf(')');
        if (openParenRel < 0 || closeParenRel <= openParenRel) continue;

        // '[' text ']'
        decs.push({ range: new monaco.Range(ln, startWhole + 1, ln, startWhole + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawText) decs.push({ range: new monaco.Range(ln, startWhole + 2, ln, startWhole + closeBracketRel + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + closeBracketRel + 1, ln, startWhole + closeBracketRel + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '(' target ')'（括号不下划线）
        decs.push({ range: new monaco.Range(ln, startWhole + openParenRel + 1, ln, startWhole + openParenRel + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        const targetRel = whole.indexOf(rawTarget, openParenRel + 1);
        if (targetRel >= 0) decs.push({ range: new monaco.Range(ln, startWhole + targetRel + 1, ln, startWhole + targetRel + rawTarget.length + 1), options: { inlineClassName: classMdTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + closeParenRel + 1, ln, startWhole + closeParenRel + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // Markdown 引用链接：[text][id] / [text][]
      mdRefLinkRe.lastIndex = 0;
      for (const m of line.matchAll(mdRefLinkRe)) {
        const rawText = String(m[1] || '');
        const rawId = String(m[2] || '');
        const whole = String(m[0] || '');
        const startWhole = Number(m.index || 0);

        const firstClose = whole.indexOf(']');
        const secondOpen = whole.indexOf('[', firstClose + 1);
        const secondClose = whole.lastIndexOf(']');
        if (firstClose < 1 || secondOpen < 0 || secondClose <= secondOpen) continue;

        // '[' text ']'
        decs.push({ range: new monaco.Range(ln, startWhole + 1, ln, startWhole + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawText) decs.push({ range: new monaco.Range(ln, startWhole + 2, ln, startWhole + firstClose + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + firstClose + 1, ln, startWhole + firstClose + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '[' id ']'
        decs.push({ range: new monaco.Range(ln, startWhole + secondOpen + 1, ln, startWhole + secondOpen + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawId) decs.push({ range: new monaco.Range(ln, startWhole + secondOpen + 2, ln, startWhole + secondClose + 1), options: { inlineClassName: classMdRefTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + secondClose + 1, ln, startWhole + secondClose + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // Markdown 自动链接：<https://...> / <contact@example.com>
      const decorateAngle = (start, innerLen) => {
        decs.push({ range: new monaco.Range(ln, start + 1, ln, start + 2), options: { inlineClassName: classMdAngle, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, start + innerLen + 2, ln, start + innerLen + 3), options: { inlineClassName: classMdAngle, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      };

      mdAutoLinkUrlRe.lastIndex = 0;
      for (const m of line.matchAll(mdAutoLinkUrlRe)) {
        const inner = String(m[1] || '');
        if (!inner) continue;
        const start = Number(m.index || 0);
        decorateAngle(start, inner.length);
        decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + inner.length), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      mdAutoLinkEmailRe.lastIndex = 0;
      for (const m of line.matchAll(mdAutoLinkEmailRe)) {
        const inner = String(m[1] || '');
        if (!inner) continue;
        const start = Number(m.index || 0);
        decorateAngle(start, inner.length);
        decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + inner.length), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // 脚注引用（当前工程语法）：[1](@ref)
      mdFootnoteRefRe.lastIndex = 0;
      for (const m of line.matchAll(mdFootnoteRefRe)) {
        const num = String(m[1] || '');
        const start = Number(m.index || 0);
        // '[' num ']'
        decs.push({ range: new monaco.Range(ln, start + 1, ln, start + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (num) decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + num.length), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, start + 2 + num.length, ln, start + 3 + num.length), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '(@ref)'
        const open = line.indexOf('(', start);
        const close = line.indexOf(')', start);
        if (open >= 0) decs.push({ range: new monaco.Range(ln, open + 1, ln, open + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (close > open + 1) decs.push({ range: new monaco.Range(ln, open + 2, ln, close + 1), options: { inlineClassName: classMdRefTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (close >= 0) decs.push({ range: new monaco.Range(ln, close + 1, ln, close + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // 引用链接定义：[id]: url "title"（仅 URL/路径下划线）
      const def = /^\s*\[([^\]]+?)\]\s*:\s*(.*)$/.exec(line);
      if (def) {
        const id = String(def[1] || '');
        const after = String(def[2] || '');
        const openIdx = line.indexOf('[');
        const closeIdx = line.indexOf(']', openIdx + 1);
        const colonIdx = line.indexOf(':', closeIdx + 1);

        if (openIdx >= 0) decs.push({ range: new monaco.Range(ln, openIdx + 1, ln, openIdx + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (id && closeIdx > openIdx + 1) decs.push({ range: new monaco.Range(ln, openIdx + 2, ln, closeIdx + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (closeIdx >= 0) decs.push({ range: new monaco.Range(ln, closeIdx + 1, ln, closeIdx + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        const base = colonIdx >= 0 ? colonIdx + 1 : closeIdx + 1;
        const raw = after.trimStart();
        const pad = after.length - raw.length;
        const rawOffset = base + pad;

        let tok = '';
        let tokStart = 0;
        let tokEnd = 0;
        if (raw.startsWith('`')) {
          const endTick = raw.indexOf('`', 1);
          if (endTick > 1) {
            tok = raw.slice(1, endTick);
            tokStart = 1;
            tokEnd = endTick;
          }
        } else if (raw.startsWith('<')) {
          const gt = raw.indexOf('>', 1);
          if (gt > 1) {
            tok = raw.slice(1, gt);
            tokStart = 1;
            tokEnd = gt;
          }
        } else {
          const m0 = /^\S+/.exec(raw);
          if (m0) {
            tok = m0[0];
            tokStart = 0;
            tokEnd = tok.length;
          }
        }

        const cleaned = stripLinkBoundaryPunct(tok);
        if (cleaned && (isHttpUrl(cleaned) || isPathLikeToken(cleaned))) {
          decs.push({ range: new monaco.Range(ln, rawOffset + tokStart + 1, ln, rawOffset + tokEnd + 1), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        }
      }

      urlRe.lastIndex = 0;
      for (const m of line.matchAll(urlRe)) {
        const raw = String(m[0] || '');
        const token = stripLinkBoundaryPunct(raw);
        if (!token || !isHttpUrl(token)) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, end + 1),
          options: {
            inlineClassName: classLinkLike,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
      }

      pathRe.lastIndex = 0;
      for (const m of line.matchAll(pathRe)) {
        const raw = String(m[0] || '');
        const token = stripLinkBoundaryPunct(raw);
        if (!token || !isPathLikeToken(token)) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, end + 1),
          options: {
            inlineClassName: classLinkLike,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
      }
    }

    decorationIds = targetEditor.deltaDecorations(decorationIds, decs);
    logger.debug('[LinkDecor] updated', { tag, reason, startLine, endLine, count: decs.length });
  };

  const schedule = (reason) => {
    if (timerId) window.clearTimeout(timerId);
    timerId = window.setTimeout(() => update(reason), 120);
  };

  const d1 = targetEditor.onDidScrollChange(() => schedule('scroll'));
  const d2 = targetEditor.onDidChangeModel(() => schedule('model'));
  const d3 = targetEditor.onDidChangeModelContent(() => schedule('content'));

  schedule('init');

  return {
    dispose() {
      try {
        d1?.dispose?.();
        d2?.dispose?.();
        d3?.dispose?.();
      } catch {
        // ignore
      }
      if (timerId) {
        window.clearTimeout(timerId);
        timerId = null;
      }
      if (decorationIds.length) {
        decorationIds = targetEditor.deltaDecorations(decorationIds, []);
      }
    }
  };
};

// Vditor realtime：点击链接时拦截为“系统浏览器/资源管理器”，避免内部跳转
const installRealtimeVditorLinkInterceptor = () => {
  const root = document.getElementById(realtimeVditorId);
  if (!root) {
    logger.warn('[LinkOpen] realtime vditor root not found, skip interceptor');
    return;
  }

  if (typeof realtimeVditorLinkInterceptorCleanup === 'function') {
    realtimeVditorLinkInterceptorCleanup();
    realtimeVditorLinkInterceptorCleanup = null;
  }

  const handler = (e) => {
    const base = e?.target instanceof Element ? e.target : e?.target?.parentElement;
    if (!base) return;

    const a = base.closest?.('a');
    if (!a || !root.contains(a)) return;

    const href0 = a.getAttribute('href') || '';
    const href = String(href0 || '').trim();
    if (!href) return;

    if (href.toLowerCase().startsWith('guaxi://wikilink')) {
      e.preventDefault();
      e.stopPropagation();

      const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
      logger.info('[LinkOpen] realtime wikilink', { href, baseDocPath });

      void (async () => {
        await openWikilinkFromHref(href, { baseDocPath, source: 'realtime' });
      })();
      return;
    }

    if (isHttpUrl(href)) {
      e.preventDefault();
      e.stopPropagation();

      logger.info('[LinkOpen] realtime openUrl', { href });
      void (async () => {
        try {
          await openUrl(href);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href }, err);
          message.error('打开浏览器失败');
        }
      })();
      return;
    }

    if (!isPathLikeToken(href)) return;

    e.preventDefault();
    e.stopPropagation();

    const { link, anchor } = splitFsLinkAndAnchor(href);
    if (!link) return;

    const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
    logger.info('[LinkOpen] realtime openFsLink', { link, anchor, baseDocPath });
    void (async () => {
      await openFsLinkSmart(link, { anchor, viaCtrl: true, baseDocPath });
    })();
  };

  root.addEventListener('click', handler, true);
  realtimeVditorLinkInterceptorCleanup = () => {
    root.removeEventListener('click', handler, true);
  };

  logger.debug('[LinkOpen] realtime vditor link interceptor installed');
};

const preprocessObsidianWikilinks = (content) => {
  const text = String(content || '');
  if (!text.includes('[[')) return text;

  const lines = text.replace(/\r/g, '').split('\n');
  let inFence = false;
  let fenceToken = '';

  const out = lines.map((line) => {
    const fence = line.match(/^\s*(```+|~~~+)/);
    if (fence) {
      const token = fence[1];
      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token.startsWith(fenceToken)) {
        inFence = false;
        fenceToken = '';
      }
      return line;
    }

    if (inFence || !line.includes('[[')) return line;

    return line.replace(/\[\[([^\]]+?)\]\]/g, (_m, innerRaw) => {
      const inner = String(innerRaw || '').trim();
      if (!inner) return _m;

      const parts = inner.split('|');
      const target = String(parts[0] || '').trim();
      const alias = String(parts[1] || '').trim();

      let filePart = target;
      let anchorPart = '';
      const hashIdx = target.indexOf('#');
      if (hashIdx >= 0) {
        filePart = target.slice(0, hashIdx).trim();
        anchorPart = target.slice(hashIdx + 1).trim();
      }

      const display = alias || anchorPart || filePart || target;
      const safeText = display.replace(/([\[\]\\])/g, '\\$1');

      const href = `guaxi://wikilink?file=${encodeURIComponent(filePart)}&anchor=${encodeURIComponent(anchorPart)}`;
      return `[${safeText}](${href})`;
    });
  });

  return out.join('\n');
};

const getFilesystemCompletionDocDir = async () => {
  const p = selectedFile.value?.path || '';
  if (!p) return fileTreeStore.activeRepoPath || '';
  if (p === filesystemPathCompletionDocPath && filesystemPathCompletionDocDir) {
    return filesystemPathCompletionDocDir;
  }
  filesystemPathCompletionDocPath = p;
  try {
    filesystemPathCompletionDocDir = await dirname(p);
  } catch {
    filesystemPathCompletionDocDir = fileTreeStore.activeRepoPath || '';
  }
  return filesystemPathCompletionDocDir;
};

const getFilesystemCompletionHomeDir = async () => {
  if (filesystemPathCompletionHomeDir) return filesystemPathCompletionHomeDir;
  try {
    filesystemPathCompletionHomeDir = await homeDir();
  } catch {
    filesystemPathCompletionHomeDir = '';
  }
  return filesystemPathCompletionHomeDir;
};

const isFilesystemPathContext = (beforeCursor) => {
  const s = beforeCursor || '';
  if (/\]\(\s*$/.test(s)) return true;
  if (/!\[[^\]]*\]\(\s*$/.test(s)) return true;
  if (/(src|href)=['"]\s*$/.test(s)) return true;
  if (/\(\s*$/.test(s)) return true;
  return false;
};

const extractPathToken = (beforeCursor) => {
  if (!beforeCursor) return '';
  let i = beforeCursor.length - 1;
  while (i >= 0) {
    const ch = beforeCursor[i];
    if (
      /\s/.test(ch) ||
      ch === '"' ||
      ch === "'" ||
      ch === ')' ||
      ch === '(' ||
      ch === '[' ||
      ch === ']' ||
      ch === '<' ||
      ch === '>' ||
      ch === '`' ||
      ch === '{' ||
      ch === '}'
    ) {
      break;
    }
    i -= 1;
  }
  return beforeCursor.slice(i + 1);
};

const splitTokenDirAndPartial = (token) => {
  if (!token) return { dirPrefix: '', partial: '' };
  if (token === '~') return { dirPrefix: '~/', partial: '' };

  const lastSlash = Math.max(token.lastIndexOf('/'), token.lastIndexOf('\\'));
  if (lastSlash >= 0) {
    return {
      dirPrefix: token.slice(0, lastSlash + 1),
      partial: token.slice(lastSlash + 1)
    };
  }
  if (/^[a-zA-Z]:$/.test(token)) {
    return { dirPrefix: `${token}/`, partial: '' };
  }
  return { dirPrefix: '', partial: token };
};

const listDirCached = async (dirPath) => {
  const key = normalizeFsPath(dirPath);
  const now = Date.now();
  const cached = filesystemPathCompletionCache.get(key);
  if (cached && now - cached.ts < 2000) return cached.nodes;

  const inflight = filesystemPathCompletionInflight.get(key);
  if (inflight) return inflight;

  const p = invoke('get_file_tree', { path: key })
    .then((nodes) => {
      const safe = Array.isArray(nodes) ? nodes : [];
      filesystemPathCompletionCache.set(key, { ts: Date.now(), nodes: safe });
      return safe;
    })
    .catch(() => [])
    .finally(() => {
      filesystemPathCompletionInflight.delete(key);
    });

  filesystemPathCompletionInflight.set(key, p);
  return p;
};

const registerFilesystemPathCompletionFeatures = () => {
  if (filesystemPathCompletionRegistered) return;
  filesystemPathCompletionRegistered = true;

  const languages = [
    'markdown',
    'javascript',
    'typescript',
    'python',
    'cpp',
    'rust',
    'shell',
    'html',
    'css',
    'yaml',
    'java',
    'lua',
    'ini',
    'json'
  ];

  languages.forEach((language) => {
    const disposable = monaco.languages.registerCompletionItemProvider(language, {
      triggerCharacters: ['/', '\\', '.', ':'],
      async provideCompletionItems(model, position) {
        const lineContent = model.getLineContent(position.lineNumber);
        const beforeCursor = lineContent.slice(0, position.column - 1);
        const token = extractPathToken(beforeCursor);
        const pathContext = isFilesystemPathContext(beforeCursor);

        if (language === 'markdown' && (getMarkdownMathContext(model, position) || getMarkdownFenceContext(model, position))) {
          // 设计说明：
          // 1. LaTeX 命令和 Mermaid 语句都经常以 `\`、`.`、`:` 触发补全，和路径补全触发字符重叠。
          // 2. Markdown 内嵌语言区域应由对应语言 provider 主导；路径补全只在普通 Markdown 文本和链接上下文工作。
          // 3. 这里主动让出 embedded context，避免 `\cos` / `graph TD` 这类语法补全被文件列表候选稀释或覆盖。
          logger.debug('[PathCompletion] skipped inside markdown embedded context', {
            token,
            lineNumber: position.lineNumber,
            column: position.column,
          });
          return { suggestions: [] };
        }

        const looksLikePath =
          (!!token &&
            (/[\\/]/.test(token) ||
              token.startsWith('.') ||
              token.startsWith('~') ||
              /^[a-zA-Z]:/.test(token))) ||
          pathContext;
        if (!looksLikePath) return { suggestions: [] };

        const replaceRange = new monaco.Range(
          position.lineNumber,
          Math.max(1, position.column - (token ? token.length : 0)),
          position.lineNumber,
          position.column
        );

        // Align with nvim-cmp: typing '.' / '..' should suggest './' / '../'
        if (token === '.' || token === '..') {
          const suggestions = [];
          if (token === '.') {
            suggestions.push(
              {
                label: './',
                kind: monaco.languages.CompletionItemKind.Folder,
                insertText: './',
                range: replaceRange,
                detail: '当前目录'
              },
              {
                label: '../',
                kind: monaco.languages.CompletionItemKind.Folder,
                insertText: '../',
                range: replaceRange,
                detail: '父目录'
              }
            );
          } else {
            suggestions.push({
              label: '../',
              kind: monaco.languages.CompletionItemKind.Folder,
              insertText: '../',
              range: replaceRange,
              detail: '父目录'
            });
          }
          return { suggestions: dedupeCompletionSuggestions(suggestions) };
        }

        const { dirPrefix, partial } = splitTokenDirAndPartial(token);

        const tokenStartsAbsolute =
          /^[a-zA-Z]:[\\/]/.test(token) || token.startsWith('/');
        const prefixIsAbsolute =
          /^[a-zA-Z]:[\\/]/.test(dirPrefix) || dirPrefix.startsWith('/');
        const isHomePrefix = dirPrefix.startsWith('~/') || dirPrefix.startsWith('~\\');

        let dirPath = dirPrefix;
        if (!dirPath) {
          dirPath = tokenStartsAbsolute ? '' : await getFilesystemCompletionDocDir();
        } else if (isHomePrefix) {
          const base = await getFilesystemCompletionHomeDir();
          if (!base) return { suggestions: [] };
          dirPath = joinFsPath(base, dirPrefix.slice(2));
          if (!dirPrefix.endsWith('/') && !dirPrefix.endsWith('\\')) {
            dirPath = `${dirPath}/`;
          }
        } else if (!prefixIsAbsolute) {
          const base = await getFilesystemCompletionDocDir();
          dirPath = joinFsPath(base, dirPath);
        }

        dirPath = normalizeFsPath(dirPath);
        if (!dirPath) return { suggestions: [] };

        if (tokenStartsAbsolute && !dirPrefix && token === '/') {
          dirPath = '/';
        }

        const nodes = await listDirCached(dirPath);
        if (!nodes.length) return { suggestions: [] };

        const lowerPartial = (partial || '').toLowerCase();
        const matching = nodes
          .filter((n) => {
            const name = String(n?.name || '');
            if (!name) return false;
            if (!lowerPartial) return true;
            return name.toLowerCase().startsWith(lowerPartial);
          })
          .slice(0, 50);

        if (!matching.length) {
          logger.debug('[PathCompletion] no match', {
            token,
            dirPrefix,
            partial,
            dirPath,
            nodeCount: nodes.length
          });
          return { suggestions: [] };
        }

        const partialLen = (partial || '').length;
        const replaceRangeForPartial = new monaco.Range(
          position.lineNumber,
          Math.max(1, position.column - partialLen),
          position.lineNumber,
          position.column
        );

        logger.debug('[PathCompletion] suggest', {
          token,
          dirPrefix,
          partial,
          dirPath,
          nodeCount: nodes.length,
          matchCount: matching.length
        });

        const suggestions = matching.map((n) => {
          const name = String(n?.name || '');
          const isDir = !!n?.is_directory;
          const suffix = isDir ? '/' : '';

          return {
            // 设计说明：
            // 1. 目录补全的可见 label / filterText 只保留目录名，避免 `/` 参与 Monaco 的前缀过滤。
            // 2. insertText 仍然补入尾随 `/`，让路径语义进入下一层目录。
            // 3. 接受目录项后通过 command 立刻触发下一轮 suggest，实现类似 nvim-cmp 的连续路径补全体验。
            label: isDir ? { label: name, detail: suffix } : name,
            kind: isDir
              ? monaco.languages.CompletionItemKind.Folder
              : monaco.languages.CompletionItemKind.File,
            insertText: `${name}${suffix}`,
            filterText: name,
            range: replaceRangeForPartial,
            detail: isDir ? '目录' : '文件',
            sortText: isDir ? `0_${name}` : `1_${name}`,
            command: isDir
              ? {
                  id: 'editor.action.triggerSuggest',
                  title: '继续补全路径'
                }
              : undefined
          };
        });

        return { suggestions: dedupeCompletionSuggestions(suggestions) };
      }
    });
    filesystemPathCompletionDisposables.push(disposable)
  });
};

const registerMarkdownLanguageFeatures = () => {
  if (markdownFeaturesRegistered) return;
  if (!prepareMarkdownLanguageFeatureRegistration()) return;
  markdownFeaturesRegistered = true;

  const createSnippet = (label, insertText, detail) => ({
    label,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  const clampInt = (value, min, max) => {
    const num = Number.parseInt(String(value || ''), 10);
    if (!Number.isFinite(num)) return null;
    if (num < min) return min;
    if (num > max) return max;
    return num;
  };

  const buildMarkdownTableSnippet = (rows, cols) => {
    const safeRows = clampInt(rows, 1, 30);
    const safeCols = clampInt(cols, 1, 30);
    if (!safeRows || !safeCols) return '';

    const headerCells = Array.from(
      { length: safeCols },
      (_, i) => ` 列${i + 1} `
    ).join('|');
    const sepCells = Array.from({ length: safeCols }, () => ' --- ').join('|');

    let tabStop = 1;
    const bodyLines = [];
    for (let r = 0; r < safeRows; r += 1) {
      const rowCells = [];
      for (let c = 0; c < safeCols; c += 1) {
        const label = `值${r + 1}-${c + 1}`;
        rowCells.push(' ${' + String(tabStop) + ':' + label + '} ');
        tabStop += 1;
      }
      bodyLines.push(`|${rowCells.join('|')}|`);
    }

    return `|${headerCells}|\n|${sepCells}|\n${bodyLines.join('\n')}\n`;
  };

  const markdownSnippets = [
    createSnippet('h1', '# ${1:标题}\n\n', '插入一级标题'),
    createSnippet('h2', '## ${1:标题}\n\n', '插入二级标题'),
    createSnippet('h3', '### ${1:标题}\n\n', '插入三级标题'),
    createSnippet('h4', '#### ${1:标题}\n\n', '插入四级标题'),
    createSnippet('h5', '##### ${1:标题}\n\n', '插入五级标题'),
    createSnippet('h6', '###### ${1:标题}\n\n', '插入六级标题'),
    createSnippet('ul', '- ${1:列表项}\n- ${2:列表项}\n', '插入无序列表'),
    createSnippet('ol', '1. ${1:第一项}\n2. ${2:第二项}\n', '插入有序列表'),
    createSnippet('taskl', '- [ ] ${1:任务}\n- [ ] ${2:任务}\n', '插入任务列表'),
    createSnippet('table', '| ${1:表头1} | ${2:表头2} |\n| --- | --- |\n| ${3:内容1} | ${4:内容2} |\n', '插入表格'),
    createSnippet('code', '```${1:language}\n${2:代码}\n```\n', '插入代码块'),
    createSnippet('pycell', '<!--py-->\n${1:# Python Cell\\nprint("Hello from Guaxi Python Cell")}\n<!--/py-->\n', '插入隐藏 Python Cell'),
    createSnippet('toc', '${1:目录}\n\n${2:目录内容}\n', '插入目录'),
    createSnippet('bold', '**${1:文本}**', '加粗文本'),
    createSnippet('italic', '*${1:文本}*', '斜体文本'),
    createSnippet('strikeout', '~~${1:文本}~~', '删除线'),
    createSnippet('inlineCode', '`${1:code}`', '内联代码'),
    createSnippet('link', '[${1:描述}](${2:链接})', '插入链接'),
    createSnippet('image', '![${1:描述}](${2:路径})', '插入图片'),
    createSnippet('quote', '> ${1:引用内容}\n', '插入引用'),
    createSnippet('hr', '\n---\n', '插入分隔线'),
    createSnippet('red', '<font color="red">${1:红色文字}</font>', '设置红色文本'),
    createSnippet('blue', '<font color="blue">${1:蓝色文字}</font>', '设置蓝色文本'),
    createSnippet('green', '<font color="green">${1:绿色文字}</font>', '设置绿色文本'),
    createSnippet('yellow', '<font color="yellow">${1:黄色文字}</font>', '设置黄色文本'),
    createSnippet('orange', '<font color="orange">${1:橙色文字}</font>', '设置橙色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
    createSnippet('cyan', '<font color="cyan">${1:青色文字}</font>', '设置青色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
  ];

  const createLatexSnippet = (key, insertText, detail, label) => ({
    key: String(key || '').toLowerCase(),
    label: label || `\\${key}`,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  const latexSnippets = [
    createLatexSnippet('frac', '\\frac{${1:a}}{${2:b}}', '分式'),
    createLatexSnippet('sqrt', '\\sqrt{${1:x}}', '平方根'),
    createLatexSnippet('sqrtn', '\\sqrt[${1:n}]{${2:x}}', 'n 次方根', '\\sqrt[n]{x}'),
    createLatexSnippet('sum', '\\sum_{${1:i=1}}^{${2:n}} ${3:expr}', '求和'),
    createLatexSnippet('prod', '\\prod_{${1:i=1}}^{${2:n}} ${3:expr}', '连乘'),
    createLatexSnippet('int', '\\int_{${1:a}}^{${2:b}} ${3:f(x)}\\,d${4:x}', '积分'),
    createLatexSnippet('lim', '\\lim_{${1:n}\\to\\infty} ${2:expr}', '极限'),
    createLatexSnippet('cdot', '\\cdot', '点乘'),
    createLatexSnippet('times', '\\times', '乘号'),
    createLatexSnippet('pm', '\\pm', '正负号'),
    createLatexSnippet('to', '\\to', '趋于'),
    createLatexSnippet('infty', '\\infty', '无穷'),

    createLatexSnippet('left', '\\left(${1:expr}\\right)', '自动伸缩括号'),
    createLatexSnippet('text', '\\text{${1:text}}', '文本'),
    createLatexSnippet('mathbf', '\\mathbf{${1:x}}', '粗体'),
    createLatexSnippet('mathbb', '\\mathbb{${1:R}}', '黑板粗体'),
    createLatexSnippet('mathcal', '\\mathcal{${1:L}}', '花体'),

    createLatexSnippet('begin', '\\begin{${1:env}}\n  ${0}\n\\end{${1:env}}', '环境'),
    createLatexSnippet('aligned', '\\begin{aligned}\n  ${0}\n\\end{aligned}', 'aligned 环境', '\\begin{aligned}...\\end{aligned}'),
    createLatexSnippet('align', '\\begin{align}\n  ${0}\n\\end{align}', 'align 环境', '\\begin{align}...\\end{align}'),
    createLatexSnippet('cases', '\\begin{cases}\n  ${1:expr} & ${2:cond}\\\\\n  ${3:expr} & ${4:cond}\n\\end{cases}', 'cases 环境', '\\begin{cases}...\\end{cases}'),

    createLatexSnippet('alpha', '\\alpha', '希腊字母 α'),
    createLatexSnippet('beta', '\\beta', '希腊字母 β'),
    createLatexSnippet('gamma', '\\gamma', '希腊字母 γ'),
    createLatexSnippet('delta', '\\delta', '希腊字母 δ'),
    createLatexSnippet('epsilon', '\\epsilon', '希腊字母 ε'),
    createLatexSnippet('theta', '\\theta', '希腊字母 θ'),
    createLatexSnippet('lambda', '\\lambda', '希腊字母 λ'),
    createLatexSnippet('mu', '\\mu', '希腊字母 μ'),
    createLatexSnippet('pi', '\\pi', '希腊字母 π'),
    createLatexSnippet('sigma', '\\sigma', '希腊字母 σ'),
    createLatexSnippet('phi', '\\phi', '希腊字母 φ'),
    createLatexSnippet('omega', '\\omega', '希腊字母 ω'),

    createLatexSnippet('leq', '\\leq', '≤'),
    createLatexSnippet('geq', '\\geq', '≥'),
    createLatexSnippet('neq', '\\neq', '≠'),
    createLatexSnippet('approx', '\\approx', '≈'),
    createLatexSnippet('equiv', '\\equiv', '≡'),
    createLatexSnippet('subseteq', '\\subseteq', '⊆'),
    createLatexSnippet('supseteq', '\\supseteq', '⊇'),
    createLatexSnippet('in', '\\in', '∈'),
    createLatexSnippet('notin', '\\notin', '∉'),
    createLatexSnippet('sin', '\\sin', '正弦'),
    createLatexSnippet('cos', '\\cos', '余弦'),
    createLatexSnippet('tan', '\\tan', '正切'),
    createLatexSnippet('log', '\\log', '对数'),
    createLatexSnippet('ln', '\\ln', '自然对数'),
    createLatexSnippet('exp', '\\exp', '指数'),
    createLatexSnippet('partial', '\\partial', '偏导符号'),
    createLatexSnippet('nabla', '\\nabla', 'Nabla 符号'),
    createLatexSnippet('vec', '\\vec{${1:v}}', '向量'),
    createLatexSnippet('hat', '\\hat{${1:x}}', '帽子重音'),
    createLatexSnippet('bar', '\\bar{${1:x}}', '横线重音'),
    createLatexSnippet('overline', '\\overline{${1:x}}', '上划线'),
    createLatexSnippet('underline', '\\underline{${1:x}}', '下划线'),
    createLatexSnippet('label', '\\label{${1:key}}', '标签'),
    createLatexSnippet('ref', '\\ref{${1:key}}', '引用标签'),
    createLatexSnippet('eqref', '\\eqref{${1:key}}', '公式引用'),
    createLatexSnippet('cite', '\\cite{${1:key}}', '文献引用'),
    createLatexSnippet('item', '\\item ${1:text}', '列表项'),
    createLatexSnippet('emph', '\\emph{${1:text}}', '强调文本'),
    createLatexSnippet('section', '\\section{${1:title}}', '章节标题'),
    createLatexSnippet('subsection', '\\subsection{${1:title}}', '二级章节'),
    createLatexSnippet('subsubsection', '\\subsubsection{${1:title}}', '三级章节'),
    createLatexSnippet('document', '\\begin{document}\n${0}\n\\end{document}', 'document 环境', '\\begin{document}...\\end{document}'),
    createLatexSnippet('equation', '\\begin{equation}\n  ${0}\n\\end{equation}', 'equation 环境', '\\begin{equation}...\\end{equation}'),
    createLatexSnippet('matrix', '\\begin{matrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{matrix}', 'matrix 环境', '\\begin{matrix}...\\end{matrix}'),
    createLatexSnippet('pmatrix', '\\begin{pmatrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{pmatrix}', 'pmatrix 环境', '\\begin{pmatrix}...\\end{pmatrix}'),
    createLatexSnippet('bmatrix', '\\begin{bmatrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{bmatrix}', 'bmatrix 环境', '\\begin{bmatrix}...\\end{bmatrix}'),
    createLatexSnippet('enumerate', '\\begin{enumerate}\n  \\item ${1:item}\n\\end{enumerate}', 'enumerate 环境', '\\begin{enumerate}...\\end{enumerate}'),
    createLatexSnippet('itemize', '\\begin{itemize}\n  \\item ${1:item}\n\\end{itemize}', 'itemize 环境', '\\begin{itemize}...\\end{itemize}')
  ];

  const createMermaidSnippet = (key, insertText, detail, label) => ({
    key: String(key || '').toLowerCase(),
    label: label || key,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  const mermaidSnippets = [
    createMermaidSnippet('graph', 'graph ${1:TD}\n  ${2:A}[${3:Start}] --> ${4:B}[${5:End}]', 'Mermaid flowchart'),
    createMermaidSnippet('flowchart', 'flowchart ${1:TD}\n  ${2:A}[${3:Start}] --> ${4:B}[${5:End}]', 'Mermaid flowchart'),
    createMermaidSnippet('sequenceDiagram', 'sequenceDiagram\n  participant ${1:A}\n  participant ${2:B}\n  ${1:A}->>${2:B}: ${3:message}', 'Mermaid sequence diagram'),
    createMermaidSnippet('classDiagram', 'classDiagram\n  class ${1:Name} {\n    +${2:method}()\n  }', 'Mermaid class diagram'),
    createMermaidSnippet('stateDiagram-v2', 'stateDiagram-v2\n  [*] --> ${1:State}\n  ${1:State} --> [*]', 'Mermaid state diagram'),
    createMermaidSnippet('erDiagram', 'erDiagram\n  ${1:USER} ||--o{ ${2:ORDER} : ${3:places}', 'Mermaid ER diagram'),
    createMermaidSnippet('gantt', 'gantt\n  title ${1:Project}\n  dateFormat YYYY-MM-DD\n  section ${2:Phase}\n  ${3:Task} :${4:task_id}, ${5:2026-01-01}, ${6:7d}', 'Mermaid gantt'),
    createMermaidSnippet('pie', 'pie title ${1:Title}\n  "${2:A}" : ${3:40}\n  "${4:B}" : ${5:60}', 'Mermaid pie chart'),
    createMermaidSnippet('mindmap', 'mindmap\n  root((${1:Root}))\n    ${2:Topic}\n      ${3:Item}', 'Mermaid mindmap'),
    createMermaidSnippet('timeline', 'timeline\n  title ${1:Timeline}\n  ${2:2026} : ${3:Event}', 'Mermaid timeline'),
    createMermaidSnippet('gitGraph', 'gitGraph\n  commit\n  branch ${1:feature}\n  checkout ${1:feature}\n  commit\n  checkout main\n  merge ${1:feature}', 'Mermaid gitGraph'),
    createMermaidSnippet('direction', 'direction ${1:LR}', '设置子图方向'),
    createMermaidSnippet('subgraph', 'subgraph ${1:title}\n  ${2:A} --> ${3:B}\nend', 'subgraph 块'),
    createMermaidSnippet('style', 'style ${1:A} fill:${2:#f9f},stroke:${3:#333},stroke-width:${4:2px}', '节点样式'),
    createMermaidSnippet('linkStyle', 'linkStyle ${1:0} stroke:${2:#f66},stroke-width:${3:2px}', '连线样式'),
    createMermaidSnippet('click', 'click ${1:A} "${2:https://example.com}" "${3:tooltip}"', '节点点击链接'),
    createMermaidSnippet('participant', 'participant ${1:Alice}', 'sequence participant'),
    createMermaidSnippet('actor', 'actor ${1:User}', 'sequence actor'),
    createMermaidSnippet('note', 'Note ${1:right of} ${2:Alice}: ${3:text}', 'sequence note')
  ];

  const getEmbeddedSyntaxSnippetSuggestions = (languageId, model, position) => {
    const normalizedLanguageId = String(languageId || '').trim().toLowerCase();
    if (normalizedLanguageId !== 'latex' && normalizedLanguageId !== 'mermaid') return [];

    const lineContent = model.getLineContent(position.lineNumber);
    const beforeCursor = lineContent.slice(0, position.column - 1);
    const snippetSource = normalizedLanguageId === 'latex' ? latexSnippets : mermaidSnippets;
    const match = normalizedLanguageId === 'latex'
      ? /\\[A-Za-z]*$/.exec(beforeCursor)
      : /[A-Za-z][\w-]*$/.exec(beforeCursor);

    if (normalizedLanguageId === 'latex' && !match) return [];

    const typed = normalizedLanguageId === 'latex'
      ? String(match?.[0] || '').slice(1)
      : String(match?.[0] || '');
    const prefix = typed.toLowerCase();
    const startColumn = match
      ? Math.max(1, position.column - match[0].length)
      : position.column;
    const range = new monaco.Range(
      position.lineNumber,
      startColumn,
      position.lineNumber,
      position.column
    );

    const suggestions = snippetSource
      .filter((s) => !prefix || String(s.key || '').startsWith(prefix))
      .slice(0, normalizedLanguageId === 'latex' ? 120 : 80)
      .map((s, index) => ({
        ...s,
        range,
        sortText: `0_${normalizedLanguageId}_${String(index).padStart(3, '0')}_${String(s.key || '')}`,
      }));

    if (suggestions.length) {
      logger.debug('[MarkdownCompletion] embedded syntax snippets resolved', {
        languageId: normalizedLanguageId,
        typed,
        lineNumber: position.lineNumber,
        column: position.column,
        suggestionCount: suggestions.length,
      });
    }
    return suggestions;
  };

  const mergeEmbeddedSyntaxAndLspSuggestions = (languageId, localSuggestions, lspSuggestions) => {
    const local = Array.isArray(localSuggestions) ? localSuggestions : [];
    const lsp = Array.isArray(lspSuggestions) ? lspSuggestions : [];
    const merged = dedupeCompletionSuggestions([...local, ...lsp]);
    logger.debug('[MarkdownCompletion] merged embedded syntax and LSP suggestions', {
      languageId,
      localCount: local.length,
      lspCount: lsp.length,
      mergedCount: merged.length,
      firstLocal: getCompletionItemLabelText(local[0]?.label),
      firstLsp: getCompletionItemLabelText(lsp[0]?.label),
    });
    return merged;
  };

  const isInMarkdownMath = (model, position) => {
    const maxScanLines = 250;
    const startLine = Math.max(1, position.lineNumber - maxScanLines + 1);

    let inFence = false;
    let fenceChar = '';
    let fenceLen = 0;

    let inDollar = false;
    let inDisplay = false;
    let inParen = false;
    let inBracket = false;

    for (let lineNumber = startLine; lineNumber <= position.lineNumber; lineNumber += 1) {
      let text = model.getLineContent(lineNumber);
      if (lineNumber === position.lineNumber) {
        text = text.slice(0, position.column - 1);
      }

      const fence = /^\s*(`{3,}|~{3,})/.exec(text);
      if (fence) {
        const mark = fence[1];
        if (!inFence) {
          inFence = true;
          fenceChar = mark[0];
          fenceLen = mark.length;
        } else if (mark[0] === fenceChar && mark.length >= fenceLen) {
          inFence = false;
          fenceChar = '';
          fenceLen = 0;
        }
        continue;
      }
      if (inFence) continue;

      for (let i = 0; i < text.length;) {
        const ch = text[i];

        if (ch === '\\') {
          const next = text[i + 1];
          if (next === '(') {
            inParen = true;
            i += 2;
            continue;
          }
          if (next === ')') {
            inParen = false;
            i += 2;
            continue;
          }
          if (next === '[') {
            inBracket = true;
            i += 2;
            continue;
          }
          if (next === ']') {
            inBracket = false;
            i += 2;
            continue;
          }
          i += 1;
          continue;
        }

        if (ch === '$') {
          const prev = i > 0 ? text[i - 1] : '';
          if (prev === '\\') {
            i += 1;
            continue;
          }

          const next = text[i + 1];
          if (next === '$') {
            inDisplay = !inDisplay;
            i += 2;
            continue;
          }

          if (!inDisplay) {
            inDollar = !inDollar;
          }
          i += 1;
          continue;
        }

        i += 1;
      }
    }

    return inDisplay || inDollar || inParen || inBracket;
  };


  markdownLanguageFeatureDisposables.push(monaco.languages.registerCompletionItemProvider('markdown', {
    triggerCharacters: MARKDOWN_FENCE_TRIGGER_CHARACTERS,
    async provideCompletionItems(model, position, context) {
      const mathCtx = getMarkdownMathContext(model, position);
      if (mathCtx) {
        const localSyntaxSuggestions = getEmbeddedSyntaxSnippetSuggestions(mathCtx.languageId, model, position);
        const lspEnabled = LSP_SUPPORTED_LANGUAGES.has(String(mathCtx.languageId || '').trim().toLowerCase());
        if (!lspEnabled) {
          logger.debug('[MarkdownCompletion] embedded math LSP skipped, use local syntax snippets', {
            languageId: mathCtx.languageId,
            lineNumber: position.lineNumber,
            column: position.column,
            localSuggestionCount: localSyntaxSuggestions.length,
          });
          return { suggestions: dedupeCompletionSuggestions(localSyntaxSuggestions) };
        }

        // 设计思路：
        // 1. 进入 Markdown 内嵌数学/代码块后，优先把补全请求交给对应语言的 LSP。
        // 2. LaTeX 这类语言的 LSP 常返回工程符号，本地语法片段负责补齐 vimtex 风格的命令/环境候选。
        // 3. 之前这里先走 Markdown 路径上下文判断，像 Python 函数调用里的 `(` 会被误判成路径场景，
        //    导致补全直接短路成空数组，界面只显示 `no suggestion`。
        // 4. 这里改成“内嵌语言优先”，避免 Markdown 自己的补全逻辑拦截 fenced code block 的语法补全。
        const mathLspSuggestions = await provideEmbeddedLspSuggestions(mathCtx, model, position, context);
        const mergedSuggestions = mergeEmbeddedSyntaxAndLspSuggestions(
          mathCtx.languageId,
          localSyntaxSuggestions,
          mathLspSuggestions
        );
        logger.debug('[MarkdownCompletion] embedded math suggestions resolved', {
          languageId: mathCtx.languageId,
          lineNumber: position.lineNumber,
          column: position.column,
          lspSuggestionCount: mathLspSuggestions.length,
          localSuggestionCount: localSyntaxSuggestions.length,
          suggestionCount: mergedSuggestions.length,
        });
        return { suggestions: mergedSuggestions };
      }

      const fenceCtx = getMarkdownFenceContext(model, position);
      if (fenceCtx) {
        const localSyntaxSuggestions = getEmbeddedSyntaxSnippetSuggestions(fenceCtx.languageId, model, position);
        const lspEnabled = LSP_SUPPORTED_LANGUAGES.has(String(fenceCtx.languageId || '').trim().toLowerCase());
        if (!lspEnabled) {
          logger.debug('[MarkdownCompletion] embedded fence LSP skipped, use local syntax snippets', {
            languageId: fenceCtx.languageId,
            lineNumber: position.lineNumber,
            column: position.column,
            localSuggestionCount: localSyntaxSuggestions.length,
          });
          return { suggestions: dedupeCompletionSuggestions(localSyntaxSuggestions) };
        }

        // 设计思路：
        // 1. fenced code block 内的编辑体验应由目标语言主导，Markdown 只负责承载容器。
        // 2. LSP 候选和本地语法片段合并，保证 texlab/mermaid-lsp 不返回语法关键字时仍有可用补全。
        // 3. 这里即使合并后为空，也直接返回，避免继续掉进 Markdown 片段/路径补全分支。
        const fenceLspSuggestions = await provideEmbeddedLspSuggestions(fenceCtx, model, position, context);
        const mergedSuggestions = mergeEmbeddedSyntaxAndLspSuggestions(
          fenceCtx.languageId,
          localSyntaxSuggestions,
          fenceLspSuggestions
        );
        logger.debug('[MarkdownCompletion] embedded fence suggestions resolved', {
          languageId: fenceCtx.languageId,
          lineNumber: position.lineNumber,
          column: position.column,
          lspSuggestionCount: fenceLspSuggestions.length,
          localSuggestionCount: localSyntaxSuggestions.length,
          suggestionCount: mergedSuggestions.length,
        });
        return { suggestions: mergedSuggestions };
      }

      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.slice(0, position.column - 1);

      const token = extractPathToken(beforeCursor);
      const pathContext = isFilesystemPathContext(beforeCursor);
      const inPathLike =
        pathContext ||
        (!!token &&
          (token.includes('/') ||
            token.includes('\\') ||
            token.startsWith('.') ||
            token.startsWith('~') ||
            /^[a-zA-Z]:/.test(token)));
      if (inPathLike) return { suggestions: [] };

      const wordInfo = model.getWordUntilPosition(position);
      const word = wordInfo?.word || '';
      if (!word) return { suggestions: [] };

      const lower = word.toLowerCase();
      const tableMatch = /^table(\d{1,2})(?:x|_)(\d{1,2})$/.exec(lower);
      if (tableMatch && wordInfo?.startColumn && wordInfo?.endColumn) {
        const rows = clampInt(tableMatch[1], 1, 30);
        const cols = clampInt(tableMatch[2], 1, 30);
        const insertText = buildMarkdownTableSnippet(rows, cols);

        if (insertText) {
          logger.debug('[MarkdownSnippet] table param', { word, rows, cols });

          const range = new monaco.Range(
            position.lineNumber,
            wordInfo.startColumn,
            position.lineNumber,
            wordInfo.endColumn
          );

          return {
            suggestions: [
              {
                label: `table${rows}x${cols}`,
                kind: monaco.languages.CompletionItemKind.Snippet,
                insertText,
                insertTextRules:
                  monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                detail: `插入 ${rows}×${cols} 表格`,
                range,
                sortText: '0_table_param'
              }
            ]
          };
        }

        logger.warn('[MarkdownSnippet] invalid table param', { word, rows, cols });
        return { suggestions: [] };
      }

      const range = new monaco.Range(
        position.lineNumber,
        wordInfo.startColumn,
        position.lineNumber,
        wordInfo.endColumn
      );

      const suggestions = markdownSnippets
        .filter((s) => String(s.label || '').toLowerCase().startsWith(lower))
        .slice(0, 50)
        .map((s, index) => ({
          ...s,
          range,
          sortText: s.sortText || `1_${String(index).padStart(3, '0')}_${String(s.label || '')}`
        }));

      return { suggestions: dedupeCompletionSuggestions(suggestions) };
    }
  }));

  markdownLanguageFeatureDisposables.push(monaco.languages.registerCompletionItemProvider('markdown', {
    triggerCharacters: ['\\'],
    provideCompletionItems(model, position) {
      if (!isInMarkdownMath(model, position)) return { suggestions: [] };

      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.slice(0, position.column - 1);

      const match = /\\[A-Za-z]*$/.exec(beforeCursor);
      if (!match) return { suggestions: [] };

      const typed = match[0].slice(1);
      const prefix = typed.toLowerCase();

      const startColumn = Math.max(1, position.column - match[0].length);
      const range = new monaco.Range(
        position.lineNumber,
        startColumn,
        position.lineNumber,
        position.column
      );

      const suggestions = latexSnippets
        .filter((s) => !prefix || String(s.key || '').startsWith(prefix))
        .slice(0, 80)
        .map((s, index) => ({
          ...s,
          range,
          sortText: `0_${String(index).padStart(3, '0')}_${String(s.key || '')}`
        }));

      return { suggestions: dedupeCompletionSuggestions(suggestions) };
    }
  }));

  markdownLanguageFeatureDisposables.push(monaco.languages.registerCompletionItemProvider('latex', {
    triggerCharacters: ['\\'],
    provideCompletionItems(model, position) {
      // 设计说明：
      // 1. Markdown 内嵌数学块原本已经有 LaTeX 本地 snippets，但独立 .tex 文件只依赖 texlab 时，
      //    常见命令片段可能为空或延迟返回，用户会看到“没有补全”。
      // 2. 这里复用同一份 snippets，作为 standalone LaTeX 文件的本地语法补全；texlab direct provider 仍会并行提供项目符号/引用等语义候选。
      // 3. 只在反斜杠命令前缀下返回，避免污染普通中文/正文输入。
      const suggestions = getEmbeddedSyntaxSnippetSuggestions('latex', model, position);
      logger.debug('[LatexCompletion] standalone snippets resolved', {
        lineNumber: position.lineNumber,
        column: position.column,
        suggestionCount: suggestions.length,
      });
      return { suggestions: dedupeCompletionSuggestions(suggestions) };
    }
  }));

  markdownLanguageFeatureDisposables.push(monaco.languages.registerSignatureHelpProvider('markdown', {
    signatureHelpTriggerCharacters: ['(', ',', '{', '['],
    signatureHelpRetriggerCharacters: [','],
    async provideSignatureHelp(model, position, token, context) {
      const mathCtx = getMarkdownMathContext(model, position);
      if (mathCtx) {
        const signature = await provideEmbeddedSignatureHelp(mathCtx, context);
        if (signature) return signature;
      }

      const fenceCtx = getMarkdownFenceContext(model, position);
      if (fenceCtx) {
        const signature = await provideEmbeddedSignatureHelp(fenceCtx, context);
        if (signature) return signature;
      }

      return null;
    }
  }));

  publishMarkdownLanguageFeatureRegistration()
};

const tabBarRef = ref(null)
const tabRegionLeftRef = ref(null)
const tabRegionRightRef = ref(null)

const languagePairingConfigured = new Set()
const lspEnabled = ref(false)
let lspRuntimeSnapshotRefreshPromise = null
const markdownFenceCompletionModels = new Map()
const MARKDOWN_FENCE_TRIGGER_CHARACTERS = ['.', ':', '>', '/', '\\', '"', "'", '@', '#', '(', ',', '[', '{', '-']
const EMBEDDED_LSP_READY_TIMEOUT_MS = 4000
const EMBEDDED_COMPLETION_CACHE_TTL_MS = 2500
const EMBEDDED_COMPLETION_CACHE_VERSION = 'embedded-completion-word-range-v4'
const embeddedCompletionCacheStates = new WeakMap()
const embeddedCompletionRetriggerStates = new WeakMap()

const EMBEDDED_LATEX_COMMAND_FALLBACKS = [
  'alpha',
  'beta',
  'gamma',
  'delta',
  'epsilon',
  'theta',
  'lambda',
  'mu',
  'pi',
  'sigma',
  'phi',
  'omega',
  'frac',
  'sqrt',
  'sum',
  'prod',
  'int',
  'lim',
  'cdot',
  'times',
  'pm',
  'to',
  'infty',
  'text',
  'mathbf',
  'mathbb',
  'mathcal',
  'begin',
  'end',
  'label',
  'ref',
  'cite',
  'includegraphics',
  'tableofcontents',
]

const getMarkdownMathContext = (model, position) => {
  if (!model || !position) return null;
  if (model.getLanguageId?.() !== 'markdown') return null;

  const totalLines = Number(model.getLineCount?.() || 0);
  if (!Number.isFinite(totalLines) || totalLines < 1) return null;

  const safeLineNumber = Math.min(Math.max(1, Number(position.lineNumber) || 1), totalLines);
  const safeLineMaxColumn = Math.max(1, Number(model.getLineMaxColumn?.(safeLineNumber) || 1));
  const safeColumn = Math.min(Math.max(1, Number(position.column) || 1), safeLineMaxColumn);
  const safePosition = new monaco.Position(safeLineNumber, safeColumn);

  let inFence = false;
  let fenceChar = '';
  let fenceLen = 0;
  let openMath = null;

  const openRegion = (kind, lineNumber, column, tokenLength) => {
    openMath = { kind, lineNumber, column, tokenLength };
  };

  const closeRegion = () => {
    openMath = null;
  };

  for (let lineNumber = 1; lineNumber <= safePosition.lineNumber; lineNumber += 1) {
    let text = model.getLineContent(lineNumber);
    if (lineNumber === safePosition.lineNumber) {
      text = text.slice(0, safePosition.column - 1);
    }

    const fence = /^\s*(`{3,}|~{3,})/.exec(text);
    if (fence) {
      const mark = fence[1];
      if (!inFence) {
        inFence = true;
        fenceChar = mark[0];
        fenceLen = mark.length;
      } else if (mark[0] === fenceChar && mark.length >= fenceLen) {
        inFence = false;
        fenceChar = '';
        fenceLen = 0;
      }
      continue;
    }
    if (inFence) continue;

    for (let i = 0; i < text.length;) {
      const ch = text[i];
      const next = text[i + 1];
      const prev = i > 0 ? text[i - 1] : '';

      if (ch === '\\') {
        if (next === '(') {
          if (openMath?.kind === 'paren') {
            closeRegion();
          } else if (!openMath) {
            openRegion('paren', lineNumber, i + 1, 2);
          }
          i += 2;
          continue;
        }
        if (next === ')') {
          if (openMath?.kind === 'paren') closeRegion();
          i += 2;
          continue;
        }
        if (next === '[') {
          if (openMath?.kind === 'bracket') {
            closeRegion();
          } else if (!openMath) {
            openRegion('bracket', lineNumber, i + 1, 2);
          }
          i += 2;
          continue;
        }
        if (next === ']') {
          if (openMath?.kind === 'bracket') closeRegion();
          i += 2;
          continue;
        }
        i += 2;
        continue;
      }

      if (ch === '$' && prev !== '\\') {
        if (next === '$') {
          if (openMath?.kind === 'display') {
            closeRegion();
          } else if (!openMath) {
            openRegion('display', lineNumber, i + 1, 2);
          }
          i += 2;
          continue;
        }

        if (openMath?.kind === 'inline') {
          closeRegion();
        } else if (!openMath) {
          openRegion('inline', lineNumber, i + 1, 1);
        }
        i += 1;
        continue;
      }

      i += 1;
    }
  }

  if (!openMath) return null;

  const findClose = () => {
    for (let lineNumber = openMath.lineNumber; lineNumber <= model.getLineCount(); lineNumber += 1) {
      const line = model.getLineContent(lineNumber);
      let i = lineNumber === openMath.lineNumber
        ? Math.max(0, openMath.column - 1 + openMath.tokenLength)
        : 0;
      while (i < line.length) {
        const ch = line[i];
        const next = line[i + 1];
        const prev = i > 0 ? line[i - 1] : '';

        if (openMath.kind === 'paren' && ch === '\\' && next === ')') {
          return { lineNumber, column: i + 1, tokenLength: 2 };
        }
        if (openMath.kind === 'bracket' && ch === '\\' && next === ']') {
          return { lineNumber, column: i + 1, tokenLength: 2 };
        }
        if ((openMath.kind === 'inline' || openMath.kind === 'display') && ch === '$' && prev !== '\\') {
          if (openMath.kind === 'display') {
            if (next === '$') return { lineNumber, column: i + 1, tokenLength: 2 };
          } else if (next !== '$') {
            return { lineNumber, column: i + 1, tokenLength: 1 };
          }
        }
        i += 1;
      }
    }
    return null;
  };

  const close = findClose();
  const contentStartColumn = openMath.column + openMath.tokenLength;
  const contentEndColumn = close ? close.column : model.getLineMaxColumn(model.getLineCount());
  const blockEndLine = close ? close.lineNumber : model.getLineCount();

  const blockLines = [];
  const lineRanges = [];
  for (let lineNumber = openMath.lineNumber; lineNumber <= blockEndLine; lineNumber += 1) {
    const line = model.getLineContent(lineNumber);
    const startColumn = lineNumber === openMath.lineNumber ? contentStartColumn : 1;
    const endColumn = lineNumber === blockEndLine
      ? contentEndColumn
      : model.getLineMaxColumn(lineNumber);
    const safeStart = Math.max(1, Math.min(startColumn, line.length + 1));
    const safeEnd = Math.max(safeStart, Math.min(endColumn, line.length + 1));
    blockLines.push(line.slice(safeStart - 1, safeEnd - 1));
    lineRanges.push({ lineNumber, startColumn: safeStart, endColumn: safeEnd });
  }

  const relativeLine = Math.max(1, safePosition.lineNumber - openMath.lineNumber + 1);
  const relativeColumn = safePosition.lineNumber === openMath.lineNumber
    ? Math.max(1, safePosition.column - contentStartColumn + 1)
    : safePosition.column;

  return {
    languageId: 'latex',
    kind: openMath.kind,
    blockText: blockLines.join('\n'),
    blockStartLine: openMath.lineNumber,
    blockEndLine,
    lineRanges,
    positionInBlock: new monaco.Position(relativeLine, Math.max(1, relativeColumn)),
  };
};

// Markdown 围栏高亮与补全的共享逻辑
const getMarkdownFenceContext = (model, position) => {
  if (!model || !position) return null;
  if (model.getLanguageId?.() !== 'markdown') return null;

  const totalLines = Number(model.getLineCount?.() || 0);
  if (!Number.isFinite(totalLines) || totalLines < 1) return null;

  const safeLineNumber = Math.min(Math.max(1, Number(position.lineNumber) || 1), totalLines);
  const safeLineMaxColumn = Math.max(1, Number(model.getLineMaxColumn?.(safeLineNumber) || 1));
  const safeColumn = Math.min(Math.max(1, Number(position.column) || 1), safeLineMaxColumn);
  const safePosition = new monaco.Position(safeLineNumber, safeColumn);

  let inFence = false;
  let fenceToken = '';
  let fenceLang = '';
  let fenceStartLine = 0;

  for (let ln = 1; ln <= safePosition.lineNumber; ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (!hit) continue;

    const token = hit[1];
    if (!inFence) {
      inFence = true;
      fenceToken = token;
      fenceStartLine = ln;
      fenceLang = String(hit[2] || '').trim();
    } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
      inFence = false;
      fenceToken = '';
      fenceLang = '';
      fenceStartLine = 0;
    }
  }

  if (!inFence || safePosition.lineNumber <= fenceStartLine) return null;
  const languageId = resolveFenceLanguageId(fenceLang);
  if (!languageId) return null;

  let fenceEndLine = model.getLineCount() + 1;
  for (let ln = safePosition.lineNumber + 1; ln <= model.getLineCount(); ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})/.exec(line);
    if (hit && hit[1][0] === fenceToken[0] && hit[1].length >= fenceToken.length) {
      fenceEndLine = ln;
      break;
    }
  }

  const blockStartLine = fenceStartLine + 1;
  const blockEndLine = Math.max(blockStartLine, fenceEndLine - 1);
  const blockLines = [];
  for (let ln = blockStartLine; ln <= blockEndLine; ln += 1) {
    blockLines.push(model.getLineContent(ln));
  }

  return {
    languageId,
    blockText: blockLines.join('\n'),
    positionInBlock: new monaco.Position(
      Math.max(1, safePosition.lineNumber - blockStartLine + 1),
      Math.max(1, safePosition.column)
    ),
    blockStartLine,
    blockEndLine
  };
};

const getMarkdownFenceBlocks = (model) => {
  if (!model) return [];
  const blocks = [];
  let inFence = false;
  let fenceToken = '';
  let fenceLang = '';
  let fenceStartLine = 0;

  for (let ln = 1; ln <= model.getLineCount(); ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (!hit) continue;

    const token = hit[1];
    if (!inFence) {
      inFence = true;
      fenceToken = token;
      fenceStartLine = ln;
      fenceLang = String(hit[2] || '').trim();
      continue;
    }

    if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
      const languageId = resolveFenceLanguageId(fenceLang);
      if (languageId) {
        const blockStartLine = fenceStartLine + 1;
        const blockEndLine = Math.max(blockStartLine, ln - 1);
        blocks.push({ languageId, blockStartLine, blockEndLine });
      }
      inFence = false;
      fenceToken = '';
      fenceLang = '';
      fenceStartLine = 0;
    }
  }

  return blocks;
};

const buildMarkdownFenceBlockText = (model, startLine, endLine) => {
  const lines = [];
  const safeStart = Math.max(1, startLine || 1);
  const safeEnd = Math.max(safeStart, endLine || safeStart);
  for (let ln = safeStart; ln <= safeEnd; ln += 1) {
    lines.push(model.getLineContent(ln));
  }
  return lines.join('\n');
};

const markdownFenceHighlightStates = new WeakMap();
const markdownFenceTokenizerWarmups = new Map();
const embeddedSemanticLegendCache = new Map();
const markdownSuggestModeStates = new WeakMap();
const embeddedSuggestTriggerStates = new WeakMap();
const markdownFenceBlocksCache = new WeakMap();
const markdownFenceTokenizeSkippedLanguages = new Set(['python']);
const TREE_SITTER_FENCE_HIGHLIGHT_LANGUAGES = new Set([
  'markdown',
  'javascript',
  'typescript',
  'tsx',
  'python',
  'rust',
  'shell',
  'bash',
  'lua',
  'java',
  'html',
  'c',
  'cpp',
  'latex',
]);
const EMBEDDED_SEMANTIC_MAX_LINES = 240;
const EMBEDDED_VISIBLE_BLOCK_LIMIT = 24;

// 设计说明：
// 1. 源码编辑模式按 VS Code 习惯保留输入时自动建议，用户输入标识符时能及时看到 LSP 候选。
// 2. 鼠标点击、滚动、光标移动不直接调用 triggerSuggest，避免探索/阅读时被建议框打断。
// 3. word suggestions 继续关闭，防止当前文档里的普通词汇混入 LSP 候选。
const EMBEDDED_WORD_SUGGEST_OPTIONS = {
  wordBasedSuggestions: 'off',
  quickSuggestions: {
    // 设计说明：
    // 1. Markdown 围栏代码块仍然挂在 markdown 宿主 editor 上，真正的代码补全来自我们注册的 markdown completion provider。
    // 2. 这里恢复 VS Code 式输入联想：键入标识符时自动出现候选，成员访问符仍走 trigger character。
    // 3. 光标移动和 LSP 连接成功事件不再强制打开 suggest widget，因此不会因为鼠标点击单独弹窗。
    other: true,
    comments: false,
    strings: true
  },
  suggestOnTriggerCharacters: true,
  suggest: {
    showWords: false,
    snippetsPreventQuickSuggestions: false
  }
};

const DEFAULT_WORD_SUGGEST_OPTIONS = {
  wordBasedSuggestions: 'currentDocument',
  quickSuggestions: {
    other: true,
    comments: false,
    strings: true
  },
  suggest: {
    showWords: true,
    snippetsPreventQuickSuggestions: true
  }
};

// 直接编辑源码文件时贴近 VS Code：输入标识符会自动请求 LSP 候选，点击/光标移动不触发。
const DIRECT_LSP_SUGGEST_OPTIONS = {
  wordBasedSuggestions: 'off',
  quickSuggestions: {
    // 设计说明：
    // 1. 直接编辑 Python/Lua/LaTeX/HTML 等 LSP 语言时，建议列表应优先来自语言服务器。
    // 2. `other: true` 保留 VS Code 默认输入体验，键入普通标识符时也能及时看到 LSP 候选。
    // 3. 同时关闭 word suggestions，避免文档碎片候选盖住真实语义补全。
    other: true,
    comments: false,
    strings: true
  },
  suggestOnTriggerCharacters: true,
  suggest: {
    showWords: false,
    snippetsPreventQuickSuggestions: false
  }
};

const getMarkdownFenceHighlightState = (targetEditor) => {
  let state = markdownFenceHighlightStates.get(targetEditor);
  if (!state) {
    state = { decorations: [], timer: 0, lastKey: '', requestSeq: 0 };
    markdownFenceHighlightStates.set(targetEditor, state);
  }
  return state;
};

const directFileSyntaxHighlightStates = new WeakMap();
const DIRECT_FILE_DECORATION_LANGUAGES = new Set(['latex']);

const getDirectFileSyntaxHighlightState = (targetEditor) => {
  let state = directFileSyntaxHighlightStates.get(targetEditor);
  if (!state) {
    state = { decorations: [], timer: 0, lastKey: '', requestSeq: 0 };
    directFileSyntaxHighlightStates.set(targetEditor, state);
  }
  return state;
};

const clearDirectFileSyntaxHighlight = (targetEditor) => {
  if (!targetEditor) return;
  const state = getDirectFileSyntaxHighlightState(targetEditor);
  if (!state.decorations?.length) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations, []);
};

const clearMarkdownFenceSyntaxHighlight = (targetEditor) => {
  if (!targetEditor) return;
  const state = getMarkdownFenceHighlightState(targetEditor);
  if (!state.decorations?.length) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations, []);
};

const ensureMarkdownFenceTokenizerReady = async (targetEditor, languageId) => {
  const lang = String(languageId || '').trim().toLowerCase();
  if (!targetEditor || !lang) return;
  if (markdownFenceTokenizeSkippedLanguages.has(lang)) return;

  let warmup = markdownFenceTokenizerWarmups.get(lang);
  if (!warmup) {
    warmup = initTextmate(targetEditor, lang)
      .catch((error) => {
        logger.warn('[MarkdownFence] tokenizer warmup failed', { languageId: lang }, error);
      })
      .finally(() => {
        markdownFenceTokenizerWarmups.delete(lang);
      });
    markdownFenceTokenizerWarmups.set(lang, warmup);
  }

  await warmup;
};

const shouldSkipMarkdownFenceTokenize = (languageId) => {
  const lang = String(languageId || '').trim().toLowerCase();
  if (!lang) return true;
  if (!markdownFenceTokenizeSkippedLanguages.has(lang)) return false;
  // 兼容说明：
  // 当前 Python TextMate grammar 在 Markdown fence 二次 tokenization 时会触发 endless loop 报错，
  // 这里先跳过这类语言的附加高亮，保证编辑器主体与 Python Cell UI 稳定可用。
  return true;
};

const resolveFenceTokenClass = (tokenType) => {
  const type = String(tokenType || '').toLowerCase();
  if (!type) return '';
  if (type.includes('comment')) return 'md-fence-token-comment';
  if (type.includes('string')) return 'md-fence-token-string';
  if (type.includes('regexp')) return 'md-fence-token-regexp';
  if (type === 'function.macro' || type.includes('macro')) return 'md-fence-token-macro';
  if (type === 'keyword.operator') return 'md-fence-token-operator';
  if (type.includes('keyword') || type.includes('storage')) return 'md-fence-token-keyword';
  if (type.includes('number') || type.includes('constant.numeric')) return 'md-fence-token-number';
  if (type.includes('boolean')) return 'md-fence-token-boolean';
  if (type.includes('type') || type.includes('interface') || type.includes('enum') || type.includes('struct')) return 'md-fence-token-type';
  if (type.includes('class') || type.includes('constructor')) return 'md-fence-token-class';
  if (type.includes('method')) return 'md-fence-token-method';
  if (type.includes('function.call') || type.includes('function.builtin') || type.includes('function.special') || type.includes('function')) return 'md-fence-token-function';
  if (type.includes('parameter') || type.includes('argument')) return 'md-fence-token-parameter';
  if (type.includes('property') || type.includes('field')) return 'md-fence-token-property';
  if (type.includes('namespace') || type.includes('module') || type.includes('package')) return 'md-fence-token-namespace';
  if (type.includes('tag')) return 'md-fence-token-tag';
  if (type.includes('attribute')) return 'md-fence-token-attribute';
  if (type.includes('constant')) return 'md-fence-token-constant';
  if (type.includes('builtin') || type.includes('support')) return 'md-fence-token-builtin';
  if (type.includes('decorator') || type.includes('annotation')) return 'md-fence-token-decorator';
  if (type.includes('variable') || type.includes('identifier')) return 'md-fence-token-variable';
  if (type.includes('operator')) return 'md-fence-token-operator';
  if (type.includes('delimiter') || type.includes('punctuation') || type.includes('bracket')) return 'md-fence-token-delimiter';
  return '';
};

const normalizeTreeSitterFenceHighlightLanguage = (languageId) => {
  const lang = String(languageId || '').trim().toLowerCase();
  if (lang === 'shell' || lang === 'sh' || lang === 'zsh' || lang === 'powershell' || lang === 'ps1') return 'bash';
  if (lang === 'tex') return 'latex';
  return lang;
};

const shouldUseTreeSitterFenceHighlight = (languageId) => {
  const lang = normalizeTreeSitterFenceHighlightLanguage(languageId);
  return !!lang && TREE_SITTER_FENCE_HIGHLIGHT_LANGUAGES.has(lang);
};

const buildFenceUtf8LineIndex = (text) => {
  const encoder = new TextEncoder();
  const lineStartBytes = [0];
  const lineStartOffsets = [0];
  let byteOffset = 0;
  for (let offset = 0; offset < text.length;) {
    const codePoint = text.codePointAt(offset) || 0;
    const char = String.fromCodePoint(codePoint);
    byteOffset += encoder.encode(char).length;
    offset += char.length;
    if (char === '\n') {
      lineStartBytes.push(byteOffset);
      lineStartOffsets.push(offset);
    }
  }
  return { encoder, lineStartBytes, lineStartOffsets };
};

const findFenceLineForByte = (lineStartBytes, byteOffset) => {
  let low = 0;
  let high = lineStartBytes.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    if (lineStartBytes[mid] <= byteOffset) low = mid + 1;
    else high = mid - 1;
  }
  return Math.max(0, high);
};

const fencePositionFromByteOffset = (text, index, byteOffset) => {
  const lineIndex = findFenceLineForByte(index.lineStartBytes, byteOffset);
  const lineStartByte = index.lineStartBytes[lineIndex];
  const lineStartOffset = index.lineStartOffsets[lineIndex];
  const targetBytes = Math.max(0, byteOffset - lineStartByte);
  let consumedBytes = 0;
  let column = 0;

  for (let offset = lineStartOffset; offset < text.length && consumedBytes < targetBytes;) {
    const codePoint = text.codePointAt(offset) || 0;
    const char = String.fromCodePoint(codePoint);
    if (char === '\n') break;
    const charBytes = index.encoder.encode(char).length;
    if (consumedBytes + charBytes > targetBytes) break;
    consumedBytes += charBytes;
    column += char.length;
    offset += char.length;
  }

  return { lineIndex, column: column + 1 };
};

const appendFenceBaseLineDecorations = (decorations, block, model, blockText) => {
  if (!decorations || !block || !model) return;
  const lineCount = Math.max(1, String(blockText || '').split('\n').length);
  for (let i = 0; i < lineCount; i += 1) {
    const lineNumber = Number(block.blockStartLine || 1) + i;
    if (lineNumber < 1 || lineNumber > model.getLineCount()) continue;
    const lineContent = model.getLineContent(lineNumber);
    decorations.push({
      range: new monaco.Range(lineNumber, 1, lineNumber, Math.max(1, lineContent.length + 1)),
      options: { inlineClassName: 'md-fence-token-base' }
    });
  }
};

const appendTreeSitterFenceDecorations = (decorations, block, model, blockText, spans) => {
  if (!decorations || !block || !model || !Array.isArray(spans) || !spans.length) return;
  const lineRanges = getEmbeddedBlockLineRanges({ ...block, blockText }, model);
  if (!lineRanges.length) return;

  const index = buildFenceUtf8LineIndex(blockText);
  for (const span of spans) {
    const startByte = Number(span?.start_byte ?? span?.startByte ?? 0);
    const endByte = Number(span?.end_byte ?? span?.endByte ?? 0);
    if (!Number.isFinite(startByte) || !Number.isFinite(endByte) || endByte <= startByte) continue;

    const className = resolveFenceTokenClass(span?.token_type ?? span?.tokenType ?? '');
    if (!className) continue;

    const start = fencePositionFromByteOffset(blockText, index, startByte);
    const end = fencePositionFromByteOffset(blockText, index, endByte);
    for (let lineIndex = start.lineIndex; lineIndex <= end.lineIndex; lineIndex += 1) {
      const lineRange = lineRanges[lineIndex];
      if (!lineRange) continue;

      const lineWidth = Math.max(0, lineRange.endColumn - lineRange.startColumn);
      const relativeStartColumn = lineIndex === start.lineIndex ? start.column : 1;
      const relativeEndColumn = lineIndex === end.lineIndex ? end.column : lineWidth + 1;
      const startColumn = Math.min(
        Math.max(lineRange.startColumn, lineRange.startColumn + relativeStartColumn - 1),
        lineRange.endColumn
      );
      const endColumn = Math.min(
        Math.max(startColumn, lineRange.startColumn + relativeEndColumn - 1),
        lineRange.endColumn
      );
      if (startColumn === endColumn) continue;

      decorations.push({
        range: new monaco.Range(lineRange.lineNumber, startColumn, lineRange.lineNumber, endColumn),
        options: { inlineClassName: className }
      });
    }
  }
};

const C_LIKE_KEYWORD_RE = /\b(?:alignas|alignof|asm|auto|break|case|class|const|constexpr|continue|decltype|default|delete|do|else|enum|explicit|export|extern|for|friend|goto|if|inline|mutable|namespace|new|noexcept|operator|override|private|protected|public|register|return|sizeof|static|struct|switch|template|this|thread_local|throw|try|typedef|typename|union|using|virtual|volatile|while)\b/g;
const C_LIKE_TYPE_RE = /\b(?:bool|char|char8_t|char16_t|char32_t|complex|double|float|int|long|short|signed|size_t|string|unsigned|void|wchar_t)\b/g;
const C_LIKE_PREPROCESSOR_RE = /^\s*#?\s*(include|define|elif|else|endif|error|if|ifdef|ifndef|line|pragma|undef|warning)\b/g;
const C_LIKE_HEADER_RE = /^\s*#?\s*include\s*(<[^>\n]+>|"[^"\n]+")/g;
const RUST_KEYWORD_RE = /\b(?:as|async|await|break|const|continue|crate|dyn|else|enum|extern|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|Self|static|struct|super|trait|type|unsafe|use|where|while)\b/g;
const RUST_TYPE_RE = /\b(?:bool|char|f32|f64|i8|i16|i32|i64|i128|isize|str|String|u8|u16|u32|u64|u128|usize|Vec|Option|Result)\b/g;

const appendRegexFenceLineDecorations = (decorations, lineRange, lineText, regex, className, captureIndex = 0, reject = null) => {
  if (!decorations || !lineRange || !lineText || !regex || !className) return 0;
  regex.lastIndex = 0;
  let count = 0;
  let match = null;
  while ((match = regex.exec(lineText)) !== null) {
    const captureText = captureIndex > 0 ? match[captureIndex] : match[0];
    if (!captureText) {
      if (match[0] === '') regex.lastIndex += 1;
      continue;
    }

    const captureOffset = captureIndex > 0
      ? Math.max(0, match[0].indexOf(captureText))
      : 0;
    const startOffset = match.index + captureOffset;
    const endOffset = startOffset + captureText.length;
    if (reject?.(captureText, match, startOffset, endOffset)) continue;

    decorations.push({
      range: new monaco.Range(
        lineRange.lineNumber,
        lineRange.startColumn + startOffset,
        lineRange.lineNumber,
        lineRange.startColumn + endOffset
      ),
      options: { inlineClassName: className }
    });
    count += 1;

    if (match[0] === '') regex.lastIndex += 1;
  }
  return count;
};

const appendLanguageAwareFenceDecorations = (decorations, block, model, blockText, reason) => {
  const languageId = normalizeTreeSitterFenceHighlightLanguage(block?.languageId || '');
  if (!decorations || !block || !model || !blockText || !languageId) return 0;
  if (!['c', 'cpp', 'rust', 'latex'].includes(languageId)) return 0;

  const lineRanges = getEmbeddedBlockLineRanges({ ...block, blockText }, model);
  const blockLines = String(blockText || '').split('\n');
  let appended = 0;

  for (let i = 0; i < blockLines.length; i += 1) {
    const lineRange = lineRanges[i];
    const lineText = blockLines[i] || '';
    if (!lineRange) continue;

    if (languageId === 'c' || languageId === 'cpp') {
      // 设计思路：
      // 1. C/C++ 预处理指令由 tree-sitter query 捕获不稳定，尤其 Markdown 源码编辑器里
      //    `#include` 有时会被 Markdown 自身语法和围栏装饰叠加影响，表现为 include 仍是 base 色。
      // 2. 这里只在行首补预处理指令，避免把普通函数名/变量名 include 误染为关键字。
      // 3. 头文件路径单独染成 builtin，让 `#include <iostream>` 的指令和导入目标都能清楚区分。
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_PREPROCESSOR_RE, 'md-fence-token-keyword', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_HEADER_RE, 'md-fence-token-builtin', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_KEYWORD_RE, 'md-fence-token-keyword');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_TYPE_RE, 'md-fence-token-type');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)\s*(?=\()/g, 'md-fence-token-function', 1, (word) => (
        /^(if|for|while|switch|return|sizeof)$/.test(word)
      ));
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)(?=::)/g, 'md-fence-token-namespace', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /::\s*([A-Za-z_]\w*)/g, 'md-fence-token-property', 1);
      continue;
    }

    if (languageId === 'rust') {
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, RUST_KEYWORD_RE, 'md-fence-token-keyword');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, RUST_TYPE_RE, 'md-fence-token-type');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)!(?=\s*[\(\[])/g, 'md-fence-token-macro', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)\s*(?=\()/g, 'md-fence-token-function', 1, (word) => (
        /^(if|for|while|match|loop|return)$/.test(word)
      ));
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)(?=::)/g, 'md-fence-token-namespace', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /::\s*([A-Za-z_]\w*)/g, 'md-fence-token-property', 1);
      continue;
    }

    if (languageId === 'latex') {
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /(\\[A-Za-z@]+\*?|\\.)/g, 'md-fence-token-keyword', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\\(?:begin|end)\s*\{\s*([A-Za-z*_-][\w*:-]*)\s*\}/g, 'md-fence-token-type', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\\(?:documentclass|usepackage|usetikzlibrary|input|include)\s*(?:\[[^\]]*\])?\s*\{\s*([^}\s,]+)/g, 'md-fence-token-builtin', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\[[^\]]*\]/g, 'md-fence-token-attribute');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b\d+(?:\.\d+)?\b/g, 'md-fence-token-number');
    }
  }

  if (appended > 0) {
    logger.debug('[MarkdownFence] language-aware supplemental decorations appended', {
      languageId,
      appended,
      blockStartLine: block.blockStartLine,
      blockEndLine: block.blockEndLine,
      reason,
    });
  }
  return appended;
};

const fetchTreeSitterFenceTokenSpans = async (languageId, code, context = {}) => {
  const normalized = normalizeTreeSitterFenceHighlightLanguage(languageId);
  if (!normalized) return null;

  const startedAt = performance.now();
  try {
    const response = await invoke('syntax_highlight_tokens', {
      request: {
        code: String(code || ''),
        language: normalized,
      },
    });

    if (!response?.supported) {
      logger.debug('[MarkdownFence][TreeSitter] backend returned unsupported language', {
        languageId,
        normalized,
        reason: context.reason || '',
        blockStartLine: context.blockStartLine || 0,
        blockEndLine: context.blockEndLine || 0,
        frontendElapsedMs: Math.round(performance.now() - startedAt),
      });
      return null;
    }

    const spans = Array.isArray(response?.spans) ? response.spans : [];
    logger.debug('[MarkdownFence][TreeSitter] token spans received', {
      languageId,
      normalized,
      backendDebugId: response?.debug_id,
      spanCount: spans.length,
      backendElapsedMs: response?.elapsed_ms,
      frontendElapsedMs: Math.round(performance.now() - startedAt),
      reason: context.reason || '',
      blockStartLine: context.blockStartLine || 0,
      blockEndLine: context.blockEndLine || 0,
    });
    return spans;
  } catch (error) {
    logger.warn('[MarkdownFence][TreeSitter] token request failed, fallback to Monaco tokenizer', {
      languageId,
      normalized,
      reason: context.reason || '',
      blockStartLine: context.blockStartLine || 0,
      blockEndLine: context.blockEndLine || 0,
      frontendElapsedMs: Math.round(performance.now() - startedAt),
      error,
    });
    return null;
  }
};

const getEmbeddedBlockLineRanges = (embeddedCtx, model) => {
  if (Array.isArray(embeddedCtx?.lineRanges) && embeddedCtx.lineRanges.length) {
    return embeddedCtx.lineRanges;
  }

  if (!embeddedCtx || !model) return [];

  const blockText = String(embeddedCtx.blockText || '');
  const lineCount = blockText ? blockText.split('\n').length : 1;
  const ranges = [];
  for (let i = 0; i < lineCount; i += 1) {
    const lineNumber = Math.max(1, Number(embeddedCtx.blockStartLine || 1) + i);
    const lineContent = model.getLineContent(lineNumber);
    ranges.push({
      lineNumber,
      startColumn: 1,
      endColumn: Math.max(1, lineContent.length + 1)
    });
  }
  return ranges;
};

const normalizeSemanticTokenBytes = (value) => {
  if (!value) return null;
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);

  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }

  if (value?.buffer instanceof Uint8Array) return value.buffer;
  if (value?.buffer instanceof ArrayBuffer) {
    return new Uint8Array(
      value.buffer,
      Number(value.byteOffset || 0),
      Number(value.byteLength || value.buffer.byteLength || 0)
    );
  }

  return null;
};

const decodeSemanticTokensDto = (value) => {
  const bytes = normalizeSemanticTokenBytes(value);
  if (!bytes || !bytes.byteLength || bytes.byteLength % 4 !== 0) return null;

  const wordCount = Math.floor(bytes.byteLength / 4);
  if (wordCount < 3) return null;

  const words = new Uint32Array(wordCount);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let i = 0; i < wordCount; i += 1) {
    words[i] = view.getUint32(i * 4, true);
  }

  const type = words[1];
  if (type !== 1) return null;

  const dataLen = words[2];
  const start = 3;
  const end = start + dataLen;
  if (end > words.length) return null;

  return {
    id: words[0],
    type: 'full',
    data: words.slice(start, end)
  };
};

const listSemanticTokenModifiers = (legend, mask) => {
  const names = Array.isArray(legend?.tokenModifiers) ? legend.tokenModifiers : [];
  const result = [];
  for (let i = 0; i < names.length; i += 1) {
    if (mask & (1 << i)) result.push(String(names[i] || ''));
  }
  return result;
};

const resolveEmbeddedSemanticTokenClass = (tokenType, tokenModifiers = []) => {
  const type = String(tokenType || '').toLowerCase();
  const modifiers = Array.isArray(tokenModifiers)
    ? tokenModifiers.map((item) => String(item || '').toLowerCase())
    : [];

  if (!type) return '';
  if (modifiers.includes('defaultlibrary')) return 'md-fence-token-builtin';
  if (modifiers.includes('readonly') && (type === 'variable' || type === 'property')) {
    return 'md-fence-token-constant';
  }

  if (type === 'macro') return 'md-fence-token-macro';
  if (type === 'namespace' || type === 'module') return 'md-fence-token-namespace';
  if (type === 'type' || type === 'interface' || type === 'enum' || type === 'struct' || type === 'typeparameter') {
    return 'md-fence-token-type';
  }
  if (type === 'class') return 'md-fence-token-class';
  if (type === 'parameter') return 'md-fence-token-parameter';
  if (type === 'property' || type === 'field') return 'md-fence-token-property';
  if (type === 'variable') return 'md-fence-token-variable';
  if (type === 'enummember' || type === 'event') return 'md-fence-token-constant';
  if (type === 'function') return 'md-fence-token-function';
  if (type === 'method') return 'md-fence-token-method';
  if (type === 'decorator') return 'md-fence-token-decorator';
  if (type === 'keyword' || type === 'modifier') return 'md-fence-token-keyword';
  if (type === 'comment') return 'md-fence-token-comment';
  if (type === 'string') return 'md-fence-token-string';
  if (type === 'number') return 'md-fence-token-number';
  if (type === 'regexp') return 'md-fence-token-regexp';
  if (type === 'operator') return 'md-fence-token-operator';

  return '';
};

const appendEmbeddedSemanticDecorations = (decorations, embeddedCtx, model, semanticTokens, legend) => {
  if (!embeddedCtx || !model || !semanticTokens?.data?.length) return;

  const lineRanges = getEmbeddedBlockLineRanges(embeddedCtx, model);
  const tokenTypes = Array.isArray(legend?.tokenTypes) ? legend.tokenTypes : [];
  if (!lineRanges.length || !tokenTypes.length) return;

  let lineIndex = 0;
  let charIndex = 0;
  const data = semanticTokens.data;

  for (let i = 0; i + 4 < data.length; i += 5) {
    const deltaLine = data[i];
    const deltaStart = data[i + 1];
    const length = data[i + 2];
    const tokenTypeIndex = data[i + 3];
    const tokenModifierBits = data[i + 4];

    if (deltaLine === 0) {
      charIndex += deltaStart;
    } else {
      lineIndex += deltaLine;
      charIndex = deltaStart;
    }

    const lineRange = lineRanges[lineIndex];
    if (!lineRange || !length) continue;

    const tokenType = String(tokenTypes[tokenTypeIndex] || '');
    const tokenModifiers = listSemanticTokenModifiers(legend, tokenModifierBits);
    const className = resolveEmbeddedSemanticTokenClass(tokenType, tokenModifiers);
    if (!className) continue;

    const lineWidth = Math.max(0, lineRange.endColumn - lineRange.startColumn);
    const startColumn = Math.min(
      Math.max(lineRange.startColumn, lineRange.startColumn + charIndex),
      lineRange.endColumn
    );
    const endColumn = Math.min(
      Math.max(startColumn, lineRange.startColumn + Math.min(charIndex + length, lineWidth)),
      lineRange.endColumn
    );
    if (startColumn === endColumn) continue;

    decorations.push({
      range: new monaco.Range(lineRange.lineNumber, startColumn, lineRange.lineNumber, endColumn),
      options: { inlineClassName: className }
    });
  }
};

const getEmbeddedSemanticTokenLegend = async (virtualModel) => {
  if (!virtualModel) return null;

  const languageId = String(virtualModel.getLanguageId?.() || '').trim().toLowerCase();
  if (!languageId) return null;
  if (embeddedSemanticLegendCache.has(languageId)) {
    return embeddedSemanticLegendCache.get(languageId) || null;
  }

  try {
    const legend = await vscodeCommands.executeCommand(
      '_provideDocumentSemanticTokensLegend',
      VscodeUri.parse(virtualModel.uri.toString())
    );
    if (legend?.tokenTypes?.length) {
      embeddedSemanticLegendCache.set(languageId, legend);
      return legend;
    }
  } catch (error) {
    logger.debug('[EmbeddedSemantic] legend fetch failed', { languageId }, error);
  }

  return null;
};

const shouldFetchEmbeddedSemanticTokens = (embeddedCtx) => {
  if (!embeddedCtx || !lspEnabled.value) return false;

  const languageId = String(embeddedCtx.languageId || '').trim().toLowerCase();
  if (!languageId || !LSP_SUPPORTED_LANGUAGES.has(languageId)) return false;

  const lspState = getLspConnectionState(languageId);
  if (lspState?.state === 'disabled') return false;

  const blockText = String(embeddedCtx.blockText || '');
  const lineCount = blockText ? blockText.split('\n').length : 1;
  return lineCount <= EMBEDDED_SEMANTIC_MAX_LINES;
};

const prepareEmbeddedVirtualModel = (embeddedCtx) => {
  if (!embeddedCtx) return null;
  if (!LSP_SUPPORTED_LANGUAGES.has(embeddedCtx.languageId)) return null;
  if (!lspEnabled.value) return null;

  const lspState = getLspConnectionState(embeddedCtx.languageId);
  if (lspState?.state === 'disabled') return null;

  const sourceFilePath = String(selectedFile.value?.path || '').trim();
  const virtualModel = getOrCreateMarkdownFenceModelForSource(embeddedCtx.languageId, sourceFilePath);
  if (!virtualModel) return null;

  replaceModelContentPreserveUndo(virtualModel, embeddedCtx.blockText || '', {
    logPrefix: '[EmbeddedLsp] synchronized virtual model content',
    uri: String(virtualModel.uri?.toString?.() || ''),
    owner: getMarkdownFenceVscodeModelOwner(embeddedCtx.languageId),
    reason: 'embedded-virtual-model-prepare-local',
  });
  void retainVsCodeTextModel({
    resource: virtualModel.uri,
    content: virtualModel.getValue(),
    languageId: embeddedCtx.languageId,
    owner: getMarkdownFenceVscodeModelOwner(embeddedCtx.languageId),
    reason: 'embedded-virtual-model-prepare',
  })

  const hostEditor = getEmbeddedHostEditor();
  if (!hostEditor) return null;

  // 设计说明：
  // 1. Markdown fenced code block 里的补全必须使用 fence 自己的虚拟文档 URI，
  //    不能复用宿主 Markdown 文档 URI，否则 Python/C++ 等 LSP 会一直在错误文档上下文里初始化。
  // 2. workspace 仍然沿用原始 Markdown 文件所在工作区，这样相对导入、工程上下文和根目录推断还能保持正确。
  initLspClient(hostEditor, embeddedCtx.languageId, sourceFilePath || virtualModel.uri.toString(), {
    documentUri: virtualModel.uri.toString(),
    workspacePath: sourceFilePath || virtualModel.uri.toString(),
    requestSource: 'embedded-markdown-fence',
  });
  logger.info('[EmbeddedLsp] init requested for fenced code block', {
    languageId: embeddedCtx.languageId,
    sourceFilePath,
    documentUri: virtualModel.uri.toString(),
    workspacePath: sourceFilePath || virtualModel.uri.toString(),
  })
  return virtualModel;
};

const warmupEmbeddedLspForContext = (embeddedCtx) => {
  if (!embeddedCtx || !lspEnabled.value) return;
  const languageId = String(embeddedCtx.languageId || '').trim().toLowerCase();
  if (!languageId || !LSP_SUPPORTED_LANGUAGES.has(languageId)) return;
  prepareEmbeddedVirtualModel(embeddedCtx);
};

const fetchEmbeddedSemanticTokens = async (embeddedCtx) => {
  if (!shouldFetchEmbeddedSemanticTokens(embeddedCtx)) return null;
  const virtualModel = prepareEmbeddedVirtualModel(embeddedCtx);
  if (!virtualModel) return null;
  const ready = await waitForLspConnection(embeddedCtx.languageId, EMBEDDED_LSP_READY_TIMEOUT_MS);
  if (!ready) return null;

  try {
    await retainVsCodeTextModel({
      resource: virtualModel.uri,
      content: virtualModel.getValue(),
      languageId: embeddedCtx.languageId,
      owner: getMarkdownFenceVscodeModelOwner(embeddedCtx.languageId),
      reason: 'embedded-semantic-tokens',
    })

    const [legend, encoded] = await Promise.all([
      getEmbeddedSemanticTokenLegend(virtualModel),
      vscodeCommands.executeCommand(
        '_provideDocumentSemanticTokens',
        VscodeUri.parse(virtualModel.uri.toString())
      )
    ]);

    const semanticTokens = decodeSemanticTokensDto(encoded);
    if (!legend || !semanticTokens?.data?.length) return null;
    return { legend, semanticTokens };
  } catch (error) {
    logger.debug('[EmbeddedSemantic] semantic token fetch failed', {
      languageId: embeddedCtx?.languageId
    }, error);
    return null;
  }
};

const refreshMarkdownSuggestMode = (targetEditor) => {
  if (!targetEditor || typeof targetEditor.updateOptions !== 'function') return;

  const model = targetEditor.getModel?.();
  const position = targetEditor.getPosition?.();
  const inEmbeddedMarkdown =
    model?.getLanguageId?.() === 'markdown' &&
    position &&
    (!!getMarkdownMathContext(model, position) || !!getMarkdownFenceContext(model, position));

  let state = markdownSuggestModeStates.get(targetEditor);
  if (!state) {
    state = { embedded: null };
    markdownSuggestModeStates.set(targetEditor, state);
  }
  if (state.embedded === inEmbeddedMarkdown) return;

  state.embedded = inEmbeddedMarkdown;
  targetEditor.updateOptions(inEmbeddedMarkdown ? EMBEDDED_WORD_SUGGEST_OPTIONS : DEFAULT_WORD_SUGGEST_OPTIONS);
};

const refreshDirectLspSuggestMode = (targetEditor, languageId) => {
  if (!targetEditor || typeof targetEditor.updateOptions !== 'function') return;
  const lang = String(languageId || targetEditor.getModel?.()?.getLanguageId?.() || '').trim().toLowerCase();
  if (!lang || lang === 'markdown') return;
  const knownLspLanguage = isLspKnownLanguage(lang);
  const runtimeSupported = LSP_SUPPORTED_LANGUAGES.has(lang);
  // Design note: for languages managed by LSP, keep Monaco word suggestions
  // off while the switch is on. Otherwise a slow or not-yet-ready runtime looks
  // like "completion works", but only returns current-document words.
  const useDirectLspMode = !!lspEnabled.value && knownLspLanguage;
  targetEditor.updateOptions(useDirectLspMode
    ? DIRECT_LSP_SUGGEST_OPTIONS
    : DEFAULT_WORD_SUGGEST_OPTIONS);
  logger.debug('[DirectLsp] suggest mode refreshed', {
    languageId: lang,
    knownLspLanguage,
    runtimeSupported,
    useDirectLspMode,
  });
};

const isSuggestWidgetVisibleForEditor = (targetEditor) => {
  const dom = targetEditor?.getDomNode?.();
  const doc = dom?.ownerDocument || (typeof document !== 'undefined' ? document : null)
  if (!doc?.querySelector) return false;
  // 设计思路：
  // 1. 当前编辑器启用了 fixedOverflowWidgets，Monaco 的 suggest widget 可能被挂到编辑器 DOM 之外。
  // 2. 之前只在 editor DOM 内查询 `.suggest-widget.visible`，会误判“建议框不可见”，从而重复触发 suggest。
  // 3. 这里改成在所属 document 全局查询，避免把已经显示的 widget 误判成隐藏状态。
  return !!doc.querySelector('.suggest-widget.visible');
};

const getSuggestControllerDebugState = (targetEditor) => {
  try {
    if (!targetEditor?.getContribution) {
      return {
        state: -1,
        widgetVisible: false,
        filteredCount: 0,
        rawCount: 0,
        firstFiltered: null,
        firstRaw: null,
      }
    }

    const controller = targetEditor.getContribution('editor.contrib.suggestController')
    const suggestModel = controller?.model
    const completionModel = suggestModel?.completionModel || suggestModel?._completionModel
    const filteredItems = Array.isArray(completionModel?.items) ? completionModel.items : []
    const rawItems = Array.isArray(completionModel?._items)
      ? completionModel._items
      : filteredItems

    return {
      state: Number(suggestModel?.state ?? suggestModel?._state ?? -1),
      widgetVisible: isSuggestWidgetVisibleForEditor(targetEditor),
      filteredCount: filteredItems.length,
      rawCount: rawItems.length,
      firstFiltered: filteredItems[0] || null,
      firstRaw: rawItems[0] || null,
    }
  } catch {
    return {
      state: -1,
      widgetVisible: false,
      filteredCount: 0,
      rawCount: 0,
      firstFiltered: null,
      firstRaw: null,
    }
  }
}

const isSuggestWidgetStaleForEditor = (targetEditor) => {
  const snapshot = getSuggestControllerDebugState(targetEditor)
  return snapshot.widgetVisible && snapshot.filteredCount === 0 && snapshot.rawCount === 0
}

const logSuggestControllerSnapshot = (targetEditor, reason, extra = {}) => {
  try {
    const snapshot = getSuggestControllerDebugState(targetEditor)
    const firstFiltered = snapshot.firstFiltered
    const firstRaw = snapshot.firstRaw
    logger.info('[EmbeddedLsp] suggest controller snapshot', {
      reason,
      state: snapshot.state,
      widgetVisible: snapshot.widgetVisible,
      filteredCount: snapshot.filteredCount,
      rawCount: snapshot.rawCount,
      staleVisibleWidget: snapshot.widgetVisible && snapshot.filteredCount === 0 && snapshot.rawCount === 0,
      firstFilteredLabel: getCompletionItemLabelText(firstFiltered?.completion?.label || firstFiltered?.textLabel || ''),
      firstFilteredWord: String(firstFiltered?.word || ''),
      firstFilteredIsInvalid: !!firstFiltered?.isInvalid,
      firstFilteredEditStart: firstFiltered?.editStart
        ? `${firstFiltered.editStart.lineNumber}:${firstFiltered.editStart.column}`
        : '',
      firstRawLabel: getCompletionItemLabelText(firstRaw?.completion?.label || firstRaw?.textLabel || ''),
      firstRawWord: String(firstRaw?.word || ''),
      firstRawIsInvalid: !!firstRaw?.isInvalid,
      firstRawEditStart: firstRaw?.editStart
        ? `${firstRaw.editStart.lineNumber}:${firstRaw.editStart.column}`
        : '',
      ...extra,
    })
  } catch (error) {
    logger.debug('[EmbeddedLsp] suggest controller snapshot failed', { reason, error })
  }
}

const forceRetriggerEmbeddedCompletion = (targetEditor, source, reason, extra = {}) => {
  if (!targetEditor) return

  const beforeSnapshot = getSuggestControllerDebugState(targetEditor)
  logger.info('[EmbeddedLsp] force retrigger suggest', {
    reason,
    beforeState: beforeSnapshot.state,
    beforeWidgetVisible: beforeSnapshot.widgetVisible,
    beforeFilteredCount: beforeSnapshot.filteredCount,
    beforeRawCount: beforeSnapshot.rawCount,
    beforeStaleVisibleWidget: beforeSnapshot.widgetVisible && beforeSnapshot.filteredCount === 0 && beforeSnapshot.rawCount === 0,
    ...extra,
  })

  try {
    targetEditor.focus?.()
    if (beforeSnapshot.widgetVisible) {
      targetEditor.trigger(source, 'hideSuggestWidget', null)
      logger.debug('[EmbeddedLsp] hide suggest widget before force retrigger', {
        reason,
        ...extra,
      })
    }
  } catch (error) {
    logger.debug('[EmbeddedLsp] hide suggest widget before force retrigger failed', {
      reason,
      error,
      ...extra,
    })
  }

  markEmbeddedSuggestTriggered(targetEditor)
  targetEditor.focus?.()
  targetEditor.trigger(source, 'editor.action.triggerSuggest', {})
}

const attachSuggestControllerTrace = (targetEditor, reason, extra = {}) => {
  try {
    if (!targetEditor?.getContribution) return () => {}
    const controller = targetEditor.getContribution('editor.contrib.suggestController')
    const suggestModel = controller?.model
    if (!suggestModel?.onDidCancel && !suggestModel?.onDidSuggest && !suggestModel?.onDidTrigger) {
      return () => {}
    }

    const disposables = []
    const pushDisposable = (candidate) => {
      if (candidate?.dispose) disposables.push(candidate)
    }

    pushDisposable(suggestModel?.onDidTrigger?.((event) => {
      logger.info('[EmbeddedLsp] suggest model triggered', {
        reason,
        auto: !!event?.auto,
        shy: !!event?.shy,
        position: event?.position
          ? `${event.position.lineNumber}:${event.position.column}`
          : '',
        ...extra,
      })
    }))

    pushDisposable(suggestModel?.onDidSuggest?.((event) => {
      logger.info('[EmbeddedLsp] suggest model produced completion model', {
        reason,
        completionCount: Number(event?.completionModel?.items?.length || 0),
        isFrozen: !!event?.isFrozen,
        auto: !!event?.triggerOptions?.auto,
        triggerKind: Number(event?.triggerOptions?.triggerKind || 0),
        ...extra,
      })
    }))

    pushDisposable(suggestModel?.onDidCancel?.((event) => {
      logger.info('[EmbeddedLsp] suggest model cancelled', {
        reason,
        retrigger: !!event?.retrigger,
        stateAfterCancel: Number(suggestModel?.state ?? -1),
        ...extra,
      })
    }))

    const timeout = window.setTimeout(() => {
      for (const disposable of disposables) {
        try {
          disposable.dispose()
        } catch {
        }
      }
    }, 600)

    return () => {
      window.clearTimeout(timeout)
      for (const disposable of disposables) {
        try {
          disposable.dispose()
        } catch {
        }
      }
    }
  } catch (error) {
    logger.debug('[EmbeddedLsp] attach suggest controller trace failed', { reason, error })
    return () => {}
  }
}

const buildEmbeddedCompletionCacheKey = (model, position, embeddedCtx) => {
  if (!model || !position || !embeddedCtx) return ''
  const modelUri = String(model.uri?.toString?.() || '')
  const languageId = String(embeddedCtx.languageId || '').trim().toLowerCase()
  const blockLine = Number(embeddedCtx.positionInBlock?.lineNumber || 0)
  const blockColumn = Number(embeddedCtx.positionInBlock?.column || 0)
  const lineNumber = Number(position.lineNumber || 0)
  const column = Number(position.column || 0)
  const linePrefix = String(model.getLineContent?.(lineNumber) || '')
    .slice(0, Math.max(0, column - 1))
    .slice(-32)
  return [
    EMBEDDED_COMPLETION_CACHE_VERSION,
    modelUri,
    lineNumber,
    column,
    languageId,
    blockLine,
    blockColumn,
    linePrefix,
  ].join('::')
}

const hydrateEmbeddedCachedSuggestions = (suggestions, replaceRange) => {
  const source = Array.isArray(suggestions) ? suggestions : []
  if (!source.length) return []
  return source.map((item) => ({
    ...item,
    range: replaceRange || item?.range,
  }))
}

const readEmbeddedCompletionCache = (targetEditor, model, position, embeddedCtx, replaceRange = null) => {
  if (!targetEditor || !model || !position || !embeddedCtx) return []
  const state = embeddedCompletionCacheStates.get(targetEditor)
  if (!state) return []
  const cacheKey = buildEmbeddedCompletionCacheKey(model, position, embeddedCtx)
  if (!cacheKey || state.key !== cacheKey) return []
  if (Date.now() - Number(state.ts || 0) > EMBEDDED_COMPLETION_CACHE_TTL_MS) return []
  const suggestions = Array.isArray(state.suggestions) ? state.suggestions : []
  if (!suggestions.length) return []
  const hydratedSuggestions = hydrateEmbeddedCachedSuggestions(suggestions, replaceRange)
  logger.debug('[EmbeddedLsp] reuse cached completion suggestions', {
    languageId: embeddedCtx.languageId,
    cacheKey,
    suggestionCount: hydratedSuggestions.length,
    replaceStartColumn: replaceRange?.startColumn || 0,
    replaceEndColumn: replaceRange?.endColumn || 0,
  })
  return hydratedSuggestions
}

const writeEmbeddedCompletionCache = (targetEditor, model, position, embeddedCtx, suggestions) => {
  if (!targetEditor || !model || !position || !embeddedCtx) return
  const normalizedSuggestions = Array.isArray(suggestions) ? suggestions : []
  const cacheKey = buildEmbeddedCompletionCacheKey(model, position, embeddedCtx)
  if (!cacheKey || !normalizedSuggestions.length) return
  embeddedCompletionCacheStates.set(targetEditor, {
    key: cacheKey,
    ts: Date.now(),
    suggestions: normalizedSuggestions,
  })
  logger.debug('[EmbeddedLsp] cached completion suggestions', {
    languageId: embeddedCtx.languageId,
    cacheKey,
    suggestionCount: normalizedSuggestions.length,
  })
}

const scheduleEmbeddedCompletionRetrigger = (targetEditor, model, position, embeddedCtx, reason) => {
  if (!targetEditor || !model || !position || !embeddedCtx) return
  const cacheKey = buildEmbeddedCompletionCacheKey(model, position, embeddedCtx)
  if (!cacheKey) return

  const prev = embeddedCompletionRetriggerStates.get(targetEditor)
  const now = Date.now()
  if (prev?.key === cacheKey && now - Number(prev?.ts || 0) < 800) {
    return
  }
  embeddedCompletionRetriggerStates.set(targetEditor, { key: cacheKey, ts: now })

  window.setTimeout(() => {
    if (!targetEditor?.hasTextFocus?.()) return
    const latestPosition = targetEditor.getPosition?.()
    const latestModel = targetEditor.getModel?.()
    if (!latestPosition || latestModel !== model) return
    if (buildEmbeddedCompletionCacheKey(latestModel, latestPosition, embeddedCtx) !== cacheKey) return
    logger.info('[EmbeddedLsp] retrigger suggest from cached completion results', {
      languageId: embeddedCtx.languageId,
      reason,
      cacheKey,
      lineNumber: latestPosition.lineNumber,
      column: latestPosition.column,
    })
    const disposeTrace = attachSuggestControllerTrace(targetEditor, 'first-retrigger', {
      languageId: embeddedCtx.languageId,
      cacheKey,
      lineNumber: latestPosition.lineNumber,
      column: latestPosition.column,
    })
    forceRetriggerEmbeddedCompletion(targetEditor, 'guaxi-embedded-lsp-cache', 'first-retrigger', {
      languageId: embeddedCtx.languageId,
      cacheKey,
      lineNumber: latestPosition.lineNumber,
      column: latestPosition.column,
    })
    window.setTimeout(() => {
      logSuggestControllerSnapshot(targetEditor, 'after-first-retrigger', {
        languageId: embeddedCtx.languageId,
        cacheKey,
        lineNumber: latestPosition.lineNumber,
        column: latestPosition.column,
      })
      disposeTrace()
    }, 20)
    window.setTimeout(() => {
      if (!targetEditor?.hasTextFocus?.()) return
      const retryPosition = targetEditor.getPosition?.()
      const retryModel = targetEditor.getModel?.()
      if (!retryPosition || retryModel !== model) return
      if (buildEmbeddedCompletionCacheKey(retryModel, retryPosition, embeddedCtx) !== cacheKey) return
      const retrySnapshot = getSuggestControllerDebugState(targetEditor)
      const hasVisibleSuggestions = retrySnapshot.widgetVisible && (retrySnapshot.filteredCount > 0 || retrySnapshot.rawCount > 0)
      if (hasVisibleSuggestions) {
        logger.debug('[EmbeddedLsp] skip second retrigger because suggestions are already visible', {
          languageId: embeddedCtx.languageId,
          reason,
          cacheKey,
          lineNumber: retryPosition.lineNumber,
          column: retryPosition.column,
          widgetVisible: retrySnapshot.widgetVisible,
          filteredCount: retrySnapshot.filteredCount,
          rawCount: retrySnapshot.rawCount,
        })
        return
      }
      logger.info('[EmbeddedLsp] retrigger suggest second pass from cached completion results', {
        languageId: embeddedCtx.languageId,
        reason,
        cacheKey,
        lineNumber: retryPosition.lineNumber,
        column: retryPosition.column,
        widgetVisible: retrySnapshot.widgetVisible,
        filteredCount: retrySnapshot.filteredCount,
        rawCount: retrySnapshot.rawCount,
        staleVisibleWidget: retrySnapshot.widgetVisible && retrySnapshot.filteredCount === 0 && retrySnapshot.rawCount === 0,
      })
      const disposeSecondTrace = attachSuggestControllerTrace(targetEditor, 'second-retrigger', {
        languageId: embeddedCtx.languageId,
        cacheKey,
        lineNumber: retryPosition.lineNumber,
        column: retryPosition.column,
      })
      forceRetriggerEmbeddedCompletion(targetEditor, 'guaxi-embedded-lsp-cache-second', 'second-retrigger', {
        languageId: embeddedCtx.languageId,
        cacheKey,
        lineNumber: retryPosition.lineNumber,
        column: retryPosition.column,
      })
      window.setTimeout(() => {
        logSuggestControllerSnapshot(targetEditor, 'after-second-retrigger', {
          languageId: embeddedCtx.languageId,
          cacheKey,
          lineNumber: retryPosition.lineNumber,
          column: retryPosition.column,
        })
        disposeSecondTrace()
      }, 60)
    }, 120)
  }, 24)
}

const markEmbeddedSuggestTriggered = (targetEditor) => {
  if (!targetEditor) return;
  embeddedSuggestTriggerStates.set(targetEditor, Date.now());
};

const shouldThrottleEmbeddedSuggestTrigger = (targetEditor, minIntervalMs = 120) => {
  if (!targetEditor) return true;
  const lastAt = Number(embeddedSuggestTriggerStates.get(targetEditor) || 0);
  const now = Date.now();
  if (now - lastAt < minIntervalMs) {
    return true;
  }
  embeddedSuggestTriggerStates.set(targetEditor, now);
  return false;
};

const triggerEmbeddedSuggestIfActive = (targetEditor, languageId) => {
  if (!targetEditor || !targetEditor.hasTextFocus?.()) return;
  if (isSuggestWidgetVisibleForEditor(targetEditor)) return;
  if (shouldThrottleEmbeddedSuggestTrigger(targetEditor, 180)) return;

  const model = targetEditor.getModel?.();
  const position = targetEditor.getPosition?.();
  if (!model || model.getLanguageId?.() !== 'markdown' || !position) return;

  const mathCtx = getMarkdownMathContext(model, position);
  if (mathCtx && mathCtx.languageId === languageId) {
    window.setTimeout(() => {
      markEmbeddedSuggestTriggered(targetEditor);
      targetEditor.trigger('guaxi-embedded-lsp', 'editor.action.triggerSuggest', {});
    }, 40);
    return;
  }

  const fenceCtx = getMarkdownFenceContext(model, position);
  if (fenceCtx && fenceCtx.languageId === languageId) {
    window.setTimeout(() => {
      markEmbeddedSuggestTriggered(targetEditor);
      targetEditor.trigger('guaxi-embedded-lsp', 'editor.action.triggerSuggest', {});
    }, 40);
  }
};

const shouldTriggerEmbeddedSuggestOnType = (text) => {
  const value = String(text || '');
  if (!value || value.length !== 1) return false;
  if (/\s/.test(value)) return false;
  return /[A-Za-z0-9_.:>\/\\'"@#()[\]{},-]/.test(value);
};

const maybeTriggerEmbeddedSuggestOnType = (targetEditor, text) => {
  if (!targetEditor || !shouldTriggerEmbeddedSuggestOnType(text)) return;

  const model = targetEditor.getModel?.();
  const position = targetEditor.getPosition?.();
  if (!model || model.getLanguageId?.() !== 'markdown' || !position) return;

  const mathCtx = getMarkdownMathContext(model, position);
  const fenceCtx = getMarkdownFenceContext(model, position);
  const embeddedCtx = mathCtx || fenceCtx;
  if (!embeddedCtx) return;
  if (!LSP_SUPPORTED_LANGUAGES.has(String(embeddedCtx.languageId || '').trim().toLowerCase())) return;
  if (!lspEnabled.value) return;
  if (isSuggestWidgetVisibleForEditor(targetEditor)) return;
  if (shouldThrottleEmbeddedSuggestTrigger(targetEditor, 120)) return;

  window.setTimeout(() => {
    try {
      markEmbeddedSuggestTriggered(targetEditor);
      targetEditor.trigger('guaxi-embedded-lsp-type', 'editor.action.triggerSuggest', {});
    } catch (error) {
      logger.debug('[EmbeddedLsp] trigger suggest on type failed', error);
    }
  }, 0);
};

const maybeTriggerDirectLspSuggestOnType = (targetEditor, text) => {
  if (!targetEditor || !shouldTriggerEmbeddedSuggestOnType(text)) return;

  const model = targetEditor.getModel?.();
  const position = targetEditor.getPosition?.();
  const languageId = String(model?.getLanguageId?.() || '').trim().toLowerCase();
  if (!model || !position || !languageId || languageId === 'markdown') return;
  if (!lspEnabled.value) return;
  if (!isLspKnownLanguage(languageId)) return;
  if (isSuggestWidgetVisibleForEditor(targetEditor)) return;
  if (shouldThrottleEmbeddedSuggestTrigger(targetEditor, 120)) return;

  // 设计说明：
  // 1. Markdown 内嵌代码块有自己的虚拟文档触发逻辑，普通源码文件则应直接让 Monaco/LSP provider 响应。
  // 2. 主编辑器之前只主动触发 Markdown 内嵌 LSP，Python/Lua/HTML 等直接文件只能等待 Monaco 默认策略，
  //    这会表现为“编辑模式不触发 LSP 语法补全”。
  // 3. 这里只在受 LSP 管理的非 Markdown 语言中触发，并配合关闭 word suggestions，避免出现文档碎片候选。
  window.setTimeout(() => {
    try {
      targetEditor.trigger('guaxi-direct-lsp-type', 'editor.action.triggerSuggest', {});
      logger.debug('[DirectLsp] trigger suggest on type', {
        languageId,
        lineNumber: position.lineNumber,
        column: position.column,
      });
    } catch (error) {
      logger.debug('[DirectLsp] trigger suggest on type failed', { languageId }, error);
    }
  }, 0);
};

const getCachedMarkdownFenceBlocks = (model) => {
  if (!model) return [];

  const versionId = typeof model.getVersionId === 'function'
    ? model.getVersionId()
    : model.getAlternativeVersionId?.() || 0;
  const cached = markdownFenceBlocksCache.get(model);
  if (cached?.versionId === versionId) {
    return cached.blocks;
  }

  const blocks = getMarkdownFenceBlocks(model);
  markdownFenceBlocksCache.set(model, { versionId, blocks });
  return blocks;
};

const updateMarkdownFenceSyntaxHighlight = async (targetEditor, reason) => {
  if (!targetEditor) return;
  const model = targetEditor.getModel?.();
  if (!model || model.getLanguageId?.() !== 'markdown') {
    clearMarkdownFenceSyntaxHighlight(targetEditor);
    return;
  }

  const position = targetEditor.getPosition?.();
  const ctx = getMarkdownFenceContext(model, position);
  const visibleRanges = typeof targetEditor.getVisibleRanges === 'function'
    ? targetEditor.getVisibleRanges()
    : [];
  const allBlocks = getCachedMarkdownFenceBlocks(model);
  let blocks = visibleRanges.length
    ? allBlocks.filter((item) => visibleRanges.some((range) => (
      item.blockEndLine >= range.startLineNumber && item.blockStartLine <= range.endLineNumber
    )))
    : allBlocks;

  if (ctx) {
    const sameBlock = (item) => (
      item?.blockStartLine === ctx.blockStartLine &&
      item?.blockEndLine === ctx.blockEndLine &&
      item?.languageId === ctx.languageId
    );
    blocks = [ctx, ...blocks.filter((item) => !sameBlock(item))];
  }

  if (!visibleRanges.length && blocks.length > EMBEDDED_VISIBLE_BLOCK_LIMIT) {
    blocks = blocks.slice(0, EMBEDDED_VISIBLE_BLOCK_LIMIT);
  }
  if (!blocks.length) {
    clearMarkdownFenceSyntaxHighlight(targetEditor);
    return;
  }

  const languagesToWarm = [...new Set(
    blocks
      .map((item) => String(item?.languageId || '').trim().toLowerCase())
      .filter(Boolean)
  )];
  for (const languageId of languagesToWarm) {
    await ensureMarkdownFenceTokenizerReady(targetEditor, languageId);
  }

  const state = getMarkdownFenceHighlightState(targetEditor);
  const requestSeq = (state.requestSeq || 0) + 1;
  state.requestSeq = requestSeq;
  const versionId = typeof model.getVersionId === 'function'
    ? model.getVersionId()
    : model.getAlternativeVersionId?.() || 0;
  const rangeKey = visibleRanges
    .map((range) => `${range.startLineNumber}-${range.endLineNumber}`)
    .join('|');
  const blockKey = blocks
    .map((item) => `${item.languageId}:${item.blockStartLine}:${item.blockEndLine}`)
    .join('|');
  const key = `${versionId}:${rangeKey}:${blockKey}`;
  if (state.lastKey === key && reason === 'cursor') return;
  state.lastKey = key;

  logger.debug('[MarkdownFence] syntax highlight blocks resolved', {
    reason,
    blockCount: blocks.length,
    hasCursorBlock: !!ctx,
    visibleRangeCount: visibleRanges.length,
    versionId,
  });

  const decorations = [];
  for (const block of blocks) {
    const blockText = block.blockText ?? buildMarkdownFenceBlockText(model, block.blockStartLine, block.blockEndLine);
    let appliedTreeSitterTokens = false;

    if (shouldUseTreeSitterFenceHighlight(block.languageId)) {
      const spans = await fetchTreeSitterFenceTokenSpans(block.languageId, blockText, {
        reason,
        blockStartLine: block.blockStartLine,
        blockEndLine: block.blockEndLine,
      });
      if (state.requestSeq !== requestSeq) return;
      if (spans?.length) {
        appendFenceBaseLineDecorations(decorations, block, model, blockText);
        appendTreeSitterFenceDecorations(decorations, block, model, blockText, spans);
        appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
        appliedTreeSitterTokens = true;
        logger.debug('[MarkdownFence][TreeSitter] decorations appended', {
          languageId: block.languageId,
          spanCount: spans.length,
          blockStartLine: block.blockStartLine,
          blockEndLine: block.blockEndLine,
          reason,
        });
      } else if (spans) {
        logger.debug('[MarkdownFence][TreeSitter] empty token spans, fallback to Monaco tokenizer', {
          languageId: block.languageId,
          blockStartLine: block.blockStartLine,
          blockEndLine: block.blockEndLine,
          reason,
        });
      }
    }

    let tokenLines = [];
    if (!appliedTreeSitterTokens && shouldSkipMarkdownFenceTokenize(block.languageId)) {
      logger.debug('[MarkdownFence] skip extra tokenize for compatibility', {
        languageId: block.languageId,
        reason,
      });
      appendFenceBaseLineDecorations(decorations, block, model, blockText);
      appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
      continue;
    }

    if (!appliedTreeSitterTokens) {
      try {
        tokenLines = monaco.editor.tokenize(blockText || '', block.languageId) || [];
      } catch (error) {
        logger.warn('[MarkdownFence] tokenize failed', { language: block.languageId }, error);
        appendFenceBaseLineDecorations(decorations, block, model, blockText);
        continue;
      }

      // 设计思路：
      // 1. tree-sitter 覆盖 Python/LaTeX/主流代码块，Monaco tokenizer 继续作为 css/json/yaml 等语言兜底。
      // 2. 无论 token 细分是否可用，都先铺一层 base 装饰，保证围栏代码块在暗色/亮色主题下保持可读。
      // 3. 后端失败时这里会留下日志并回退到原生 tokenizer，不影响用户继续编辑。
      appendFenceBaseLineDecorations(decorations, block, model, blockText);

      for (let i = 0; i < tokenLines.length; i += 1) {
        const tokens = tokenLines[i] || [];
        if (!tokens.length) continue;
        const lineNumber = block.blockStartLine + i;
        const lineContent = model.getLineContent(lineNumber);
        const lineLen = lineContent.length;

        for (let j = 0; j < tokens.length; j += 1) {
          const token = tokens[j];
          const className = resolveFenceTokenClass(token?.type || '');
          if (!className) continue;

          const startOffset = token?.offset || 0;
          const nextOffset = tokens[j + 1] ? tokens[j + 1].offset : lineLen;
          const startColumn = Math.min(Math.max(1, startOffset + 1), lineLen + 1);
          const endColumn = Math.min(Math.max(startColumn, nextOffset + 1), lineLen + 1);
          if (startColumn === endColumn) continue;

          decorations.push({
            range: new monaco.Range(lineNumber, startColumn, lineNumber, endColumn),
            options: { inlineClassName: className }
          });
        }
      }
      appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
    }

    if (state.requestSeq !== requestSeq) return;
  }

  if (state.requestSeq !== requestSeq) return;
  const decorationClassCounts = decorations.reduce((acc, item) => {
    const className = String(item?.options?.inlineClassName || 'unknown');
    acc[className] = (acc[className] || 0) + 1;
    return acc;
  }, {});
  logger.debug('[MarkdownFence] applying syntax decorations', {
    reason,
    decorationCount: decorations.length,
    decorationClassCounts,
    versionId,
  });
  state.decorations = targetEditor.deltaDecorations(state.decorations || [], decorations);
};

const scheduleMarkdownFenceSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const state = getMarkdownFenceHighlightState(targetEditor);
  if (state.timer) {
    window.clearTimeout(state.timer);
  }
  const delay = reason === 'cursor'
    ? 90
    : reason === 'scroll'
      ? 140
      : reason === 'lsp-state'
        ? 120
        : 60;
  state.timer = window.setTimeout(() => {
    state.timer = 0;
    updateMarkdownFenceSyntaxHighlight(targetEditor, reason).catch((error) => {
      logger.warn('[MarkdownFence] syntax highlight update failed', { reason }, error);
    });
  }, delay);
};

const forceScheduleMarkdownFenceSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const state = getMarkdownFenceHighlightState(targetEditor);
  state.lastKey = '';
  scheduleMarkdownFenceSyntaxHighlight(targetEditor, reason);
};

const updateDirectFileSyntaxHighlight = async (targetEditor, reason) => {
  if (!targetEditor) return;
  const model = targetEditor.getModel?.();
  const languageId = normalizeTreeSitterFenceHighlightLanguage(model?.getLanguageId?.() || '');
  if (!model || !DIRECT_FILE_DECORATION_LANGUAGES.has(languageId)) {
    clearDirectFileSyntaxHighlight(targetEditor);
    return;
  }

  const visibleRanges = typeof targetEditor.getVisibleRanges === 'function'
    ? targetEditor.getVisibleRanges()
    : [];
  const ranges = visibleRanges.length
    ? visibleRanges
    : [{ startLineNumber: 1, endLineNumber: Math.min(model.getLineCount(), 220) }];

  const state = getDirectFileSyntaxHighlightState(targetEditor);
  const requestSeq = (state.requestSeq || 0) + 1;
  state.requestSeq = requestSeq;
  const versionId = typeof model.getVersionId === 'function'
    ? model.getVersionId()
    : model.getAlternativeVersionId?.() || 0;
  const rangeKey = ranges.map((range) => `${range.startLineNumber}-${range.endLineNumber}`).join('|');
  const key = `${languageId}:${versionId}:${rangeKey}`;
  if (state.lastKey === key && reason === 'cursor') return;
  state.lastKey = key;

  const decorations = [];
  for (const range of ranges) {
    const startLine = Math.max(1, Number(range.startLineNumber || 1));
    const endLine = Math.min(model.getLineCount(), Math.max(startLine, Number(range.endLineNumber || startLine)));
    const lines = [];
    for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
      lines.push(model.getLineContent(lineNumber));
    }
    const blockText = lines.join('\n');
    const block = {
      languageId,
      blockStartLine: startLine,
      blockEndLine: endLine,
      blockText,
    };

    let treeSitterApplied = false;
    const spans = await fetchTreeSitterFenceTokenSpans(languageId, blockText, {
      reason: `direct-file:${reason}`,
      blockStartLine: startLine,
      blockEndLine: endLine,
    });
    if (state.requestSeq !== requestSeq) return;
    if (spans?.length) {
      appendTreeSitterFenceDecorations(decorations, block, model, blockText, spans);
      treeSitterApplied = true;
    }

    // 设计说明：
    // 1. 独立 .tex 文件和 Markdown 代码块应该复用同一套 md-fence-token-* 色板，避免同一段 LaTeX 在两个编辑模式下颜色漂移。
    // 2. 后端 LaTeX fallback scanner 只负责通用 token，documentclass/usepackage/begin 环境名等语法糖由这里补齐。
    // 3. 只处理当前可视范围，滚动和内容变更再增量刷新，避免大 tex 文件每次编辑都全量装饰。
    appendLanguageAwareFenceDecorations(decorations, block, model, blockText, `direct-file:${reason}`);

    logger.debug('[DirectSyntaxHighlight] visible block decorated', {
      languageId,
      startLine,
      endLine,
      treeSitterApplied,
      decorationCount: decorations.length,
      reason,
    });
  }

  if (state.requestSeq !== requestSeq) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations || [], decorations);
  logger.debug('[DirectSyntaxHighlight] applied decorations', {
    languageId,
    reason,
    versionId,
    rangeCount: ranges.length,
    decorationCount: decorations.length,
  });
};

const scheduleDirectFileSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const state = getDirectFileSyntaxHighlightState(targetEditor);
  if (state.timer) {
    window.clearTimeout(state.timer);
  }
  const delay = reason === 'scroll' ? 120 : reason === 'cursor' ? 90 : 60;
  state.timer = window.setTimeout(() => {
    state.timer = 0;
    updateDirectFileSyntaxHighlight(targetEditor, reason).catch((error) => {
      logger.warn('[DirectSyntaxHighlight] update failed', { reason }, error);
    });
  }, delay);
};

const scheduleSourceSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const languageId = String(targetEditor.getModel?.()?.getLanguageId?.() || '').trim().toLowerCase();
  if (languageId === 'markdown') {
    clearDirectFileSyntaxHighlight(targetEditor);
    scheduleMarkdownFenceSyntaxHighlight(targetEditor, reason);
    scheduleMarkdownMathSyntaxHighlight(targetEditor, reason);
    return;
  }
  clearMarkdownFenceSyntaxHighlight(targetEditor);
  clearMarkdownMathSyntaxHighlight(targetEditor);
  scheduleDirectFileSyntaxHighlight(targetEditor, reason);
};

const scheduleMarkdownFenceSyntaxHighlightAfterContentSync = (targetEditor, reason) => {
  if (!targetEditor) return;
  // 设计思路：
  // 1. 打开文件时 Monaco model 内容和可视行范围不是同一帧稳定下来，单次 init 刷新容易在空内容/旧 range 上提前返回。
  // 2. 这里用两次 requestAnimationFrame 等布局与 visibleRanges 稳定后再强制刷新围栏高亮，避免用户必须滚动才触发。
  // 3. 再补一个短延迟兜底，应对 VSCode model bridge 后续 pushEditOperations 把内容再次同步进同一 model。
  const run = (suffix) => {
    forceScheduleMarkdownFenceSyntaxHighlight(targetEditor, `${reason}:${suffix}`);
    logger.debug('[MarkdownFence] scheduled after content sync', {
      reason,
      suffix,
      languageId: targetEditor.getModel?.()?.getLanguageId?.() || '',
      lineCount: targetEditor.getModel?.()?.getLineCount?.() || 0,
    });
  };

  requestAnimationFrame(() => {
    requestAnimationFrame(() => run('raf'));
  });
  window.setTimeout(() => run('settled'), 160);
};

const markdownMathHighlightStates = new WeakMap();

const getMarkdownMathHighlightState = (targetEditor) => {
  let state = markdownMathHighlightStates.get(targetEditor);
  if (!state) {
    state = { decorations: [], timer: 0, lastKey: '', requestSeq: 0 };
    markdownMathHighlightStates.set(targetEditor, state);
  }
  return state;
};

const clearMarkdownMathSyntaxHighlight = (targetEditor) => {
  if (!targetEditor) return;
  const state = getMarkdownMathHighlightState(targetEditor);
  if (!state.decorations?.length) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations, []);
};

const updateMarkdownMathSyntaxHighlight = async (targetEditor, reason) => {
  if (!targetEditor) return;
  const model = targetEditor.getModel?.();
  if (!model || model.getLanguageId?.() !== 'markdown') {
    clearMarkdownMathSyntaxHighlight(targetEditor);
    return;
  }

  const position = targetEditor.getPosition?.();
  const ctx = getMarkdownMathContext(model, position);
  if (!ctx) {
    clearMarkdownMathSyntaxHighlight(targetEditor);
    return;
  }

  await ensureMarkdownFenceTokenizerReady(targetEditor, ctx.languageId);
  warmupEmbeddedLspForContext(ctx);

  const state = getMarkdownMathHighlightState(targetEditor);
  const requestSeq = (state.requestSeq || 0) + 1;
  state.requestSeq = requestSeq;
  const versionId = typeof model.getVersionId === 'function'
    ? model.getVersionId()
    : model.getAlternativeVersionId?.() || 0;
  const key = `${ctx.kind}:${ctx.blockStartLine}:${ctx.blockEndLine}:${versionId}`;
  if (state.lastKey === key && reason === 'cursor') return;
  state.lastKey = key;

  let tokenLines = [];
  try {
    tokenLines = monaco.editor.tokenize(ctx.blockText || '', ctx.languageId) || [];
  } catch (error) {
    logger.warn('[MarkdownMath] tokenize failed', { language: ctx.languageId }, error);
    clearMarkdownMathSyntaxHighlight(targetEditor);
    return;
  }

  const decorations = [];
  for (let i = 0; i < tokenLines.length; i += 1) {
    const tokens = tokenLines[i] || [];
    const lineRange = ctx.lineRanges[i];
    if (!tokens.length || !lineRange) continue;

    const lineLen = Math.max(0, lineRange.endColumn - lineRange.startColumn);
    decorations.push({
      range: new monaco.Range(
        lineRange.lineNumber,
        lineRange.startColumn,
        lineRange.lineNumber,
        lineRange.endColumn
      ),
      options: { inlineClassName: 'md-fence-token-base' }
    });

    for (let j = 0; j < tokens.length; j += 1) {
      const token = tokens[j];
      const className = resolveFenceTokenClass(token?.type || '');
      if (!className) continue;

      const startOffset = token?.offset || 0;
      const nextOffset = tokens[j + 1] ? tokens[j + 1].offset : lineLen;
      const startColumn = Math.min(
        Math.max(lineRange.startColumn, lineRange.startColumn + startOffset),
        lineRange.endColumn
      );
      const endColumn = Math.min(
        Math.max(startColumn, lineRange.startColumn + nextOffset),
        lineRange.endColumn
      );
      if (startColumn === endColumn) continue;

      decorations.push({
        range: new monaco.Range(lineRange.lineNumber, startColumn, lineRange.lineNumber, endColumn),
        options: { inlineClassName: className }
      });
    }
  }

  if (state.requestSeq !== requestSeq) return;

  const shouldApplySemantic = reason !== 'cursor' && reason !== 'scroll';
  const semanticBundle = shouldApplySemantic ? await fetchEmbeddedSemanticTokens(ctx) : null;
  if (state.requestSeq !== requestSeq) return;
  if (semanticBundle) {
    appendEmbeddedSemanticDecorations(
      decorations,
      ctx,
      model,
      semanticBundle.semanticTokens,
      semanticBundle.legend
    );
  }

  if (state.requestSeq !== requestSeq) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations || [], decorations);
};

const scheduleMarkdownMathSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const state = getMarkdownMathHighlightState(targetEditor);
  if (state.timer) {
    window.clearTimeout(state.timer);
  }
  const delay = reason === 'cursor'
    ? 90
    : reason === 'scroll'
      ? 140
      : reason === 'lsp-state'
        ? 120
        : 60;
  state.timer = window.setTimeout(() => {
    state.timer = 0;
    updateMarkdownMathSyntaxHighlight(targetEditor, reason).catch((error) => {
      logger.warn('[MarkdownMath] syntax highlight update failed', { reason }, error);
    });
  }, delay);
};

const getOrCreateMarkdownFenceModel = (languageId) => {
  return getOrCreateMarkdownFenceModelForSource(languageId, '')
};

const FENCE_LANGUAGE_EXTENSIONS = {
  javascript: 'js',
  typescript: 'ts',
  python: 'py',
  c: 'c',
  cpp: 'cpp',
  rust: 'rs',
  shell: 'sh',
  html: 'html',
  css: 'css',
  json: 'json',
  yaml: 'yml',
  lua: 'lua',
  java: 'java',
  latex: 'tex',
  mermaid: 'mmd',
}

const buildMarkdownFenceVirtualUri = (languageId, sourceFilePath = '') => {
  const key = String(languageId || '').trim().toLowerCase();
  if (!key) return null

  const ext = FENCE_LANGUAGE_EXTENSIONS[key] || key || 'txt'
  const rawSourcePath = String(sourceFilePath || '').trim()

  try {
    if (rawSourcePath.startsWith('file://')) {
      const uri = monaco.Uri.parse(rawSourcePath)
      const nextPath = String(uri.path || '/guaxi-markdown-fence')
        .replace(/\.[^./]+$/, '')
        .concat(`.guaxi-fence.${ext}`)
      return uri.with({ path: nextPath })
    }

    if (/^[a-zA-Z]:[\\/]/.test(rawSourcePath) || rawSourcePath.startsWith('/') || rawSourcePath.startsWith('\\')) {
      const nextPath = rawSourcePath.replace(/\.[^./\\]+$/, '').concat(`.guaxi-fence.${ext}`)
      return monaco.Uri.file(nextPath)
    }
  } catch (error) {
    logger.warn('[MarkdownFence] build virtual uri failed, fallback to temp file uri', {
      languageId: key,
      sourceFilePath: rawSourcePath,
      error,
    })
  }

  return monaco.Uri.file(`__guaxi__/markdown-fence.${ext}`)
}

const getOrCreateMarkdownFenceModelForSource = (languageId, sourceFilePath = '') => {
  const key = String(languageId || '').trim().toLowerCase()
  if (!key) return null

  const targetUri = buildMarkdownFenceVirtualUri(key, sourceFilePath)
  if (!targetUri) return null

  let model = markdownFenceCompletionModels.get(key);
  if (model && !model.isDisposed?.()) {
    if (String(model.uri?.toString?.() || '') !== String(targetUri.toString())) {
      releaseVsCodeTextModel(model.uri, getMarkdownFenceVscodeModelOwner(key), 'markdown-fence-uri-changed')
      markdownFenceCompletionModels.delete(key)
      model.dispose()
      model = null
    }
  }

  if (model && !model.isDisposed?.()) {
    if (model.getLanguageId?.() !== key) {
      monaco.editor.setModelLanguage(model, key);
    }
    return model;
  }

  model = monaco.editor.getModel(targetUri) || monaco.editor.createModel('', key, targetUri);
  markdownFenceCompletionModels.set(key, model);
  return model;
};

const VSCODE_TO_MONACO_COMPLETION_KIND = {
  0: monaco.languages.CompletionItemKind.Text,
  1: monaco.languages.CompletionItemKind.Method,
  2: monaco.languages.CompletionItemKind.Function,
  3: monaco.languages.CompletionItemKind.Constructor,
  4: monaco.languages.CompletionItemKind.Field,
  5: monaco.languages.CompletionItemKind.Variable,
  6: monaco.languages.CompletionItemKind.Class,
  7: monaco.languages.CompletionItemKind.Interface,
  8: monaco.languages.CompletionItemKind.Module,
  9: monaco.languages.CompletionItemKind.Property,
  10: monaco.languages.CompletionItemKind.Unit,
  11: monaco.languages.CompletionItemKind.Value,
  12: monaco.languages.CompletionItemKind.Enum,
  13: monaco.languages.CompletionItemKind.Keyword,
  14: monaco.languages.CompletionItemKind.Snippet,
  15: monaco.languages.CompletionItemKind.Color,
  16: monaco.languages.CompletionItemKind.File,
  17: monaco.languages.CompletionItemKind.Reference,
  18: monaco.languages.CompletionItemKind.Folder,
  19: monaco.languages.CompletionItemKind.EnumMember,
  20: monaco.languages.CompletionItemKind.Constant,
  21: monaco.languages.CompletionItemKind.Struct,
  22: monaco.languages.CompletionItemKind.Event,
  23: monaco.languages.CompletionItemKind.Operator,
  24: monaco.languages.CompletionItemKind.TypeParameter,
  25: monaco.languages.CompletionItemKind.Issue,
  26: monaco.languages.CompletionItemKind.User,
}

const LSP_TO_MONACO_COMPLETION_KIND = {
  1: monaco.languages.CompletionItemKind.Text,
  2: monaco.languages.CompletionItemKind.Method,
  3: monaco.languages.CompletionItemKind.Function,
  4: monaco.languages.CompletionItemKind.Constructor,
  5: monaco.languages.CompletionItemKind.Field,
  6: monaco.languages.CompletionItemKind.Variable,
  7: monaco.languages.CompletionItemKind.Class,
  8: monaco.languages.CompletionItemKind.Interface,
  9: monaco.languages.CompletionItemKind.Module,
  10: monaco.languages.CompletionItemKind.Property,
  11: monaco.languages.CompletionItemKind.Unit,
  12: monaco.languages.CompletionItemKind.Value,
  13: monaco.languages.CompletionItemKind.Enum,
  14: monaco.languages.CompletionItemKind.Keyword,
  15: monaco.languages.CompletionItemKind.Snippet,
  16: monaco.languages.CompletionItemKind.Color,
  17: monaco.languages.CompletionItemKind.File,
  18: monaco.languages.CompletionItemKind.Reference,
  19: monaco.languages.CompletionItemKind.Folder,
  20: monaco.languages.CompletionItemKind.EnumMember,
  21: monaco.languages.CompletionItemKind.Constant,
  22: monaco.languages.CompletionItemKind.Struct,
  23: monaco.languages.CompletionItemKind.Event,
  24: monaco.languages.CompletionItemKind.Operator,
  25: monaco.languages.CompletionItemKind.TypeParameter,
}

const normalizeCompletionLabel = (label) => {
  if (typeof label === 'string') return label
  if (label && typeof label === 'object') {
    const text = String(label.label || '')
    return text
  }
  return ''
}

const normalizeCompletionDocumentation = (documentation) => {
  if (!documentation) return undefined
  if (typeof documentation === 'string') return documentation
  if (typeof documentation?.value === 'string') return documentation.value
  return String(documentation || '')
}

const buildCompletionSortKey = (item, fallbackLabel, index) => {
  const explicit = String(item?.sortText || '').trim()
  if (explicit) return explicit
  return `z_${String(index).padStart(4, '0')}_${fallbackLabel}`
}

const getCompletionItemLabelText = (label) => {
  if (typeof label === 'string') return label
  if (label && typeof label === 'object') {
    return String(label.label || '')
  }
  return ''
}

const getCompletionInsertTextKey = (item) => {
  if (typeof item?.insertText === 'string') return item.insertText
  if (typeof item?.insertText?.value === 'string') return item.insertText.value
  if (typeof item?.textEdit?.newText === 'string') return item.textEdit.newText
  if (typeof item?.textEdit?.text === 'string') return item.textEdit.text
  return getCompletionItemLabelText(item?.label)
}

const buildCompletionDedupKey = (item) => {
  const label = getCompletionItemLabelText(item?.label).trim().toLowerCase()
  const insertText = String(getCompletionInsertTextKey(item) || '').trim().toLowerCase()
  // 设计说明：
  // 1. Markdown fenced code block 的补全可能同时经过 VSCode command 和 Guaxi direct LSP provider。
  // 2. clangd 对 `#include` / `#define` 这类候选会返回相同 label/insertText，但不同 detail、kind 或来源元信息。
  // 3. `#` 触发时 filterText 还可能在 `include` / `#include` 之间变化；它只影响过滤，不应制造肉眼重复项。
  // 4. 菜单展示层应该按“用户看到并会插入什么”去重，因此 key 只保留 label、insertText。
  return [label, insertText].join('::')
}

const dedupeCompletionSuggestions = (items) => {
  const source = Array.isArray(items) ? items : []
  if (!source.length) return []

  const seen = new Set()
  const suggestions = []
  let duplicateCount = 0
  let firstDuplicateLabel = ''
  for (const item of source) {
    const key = buildCompletionDedupKey(item)
    if (!key || seen.has(key)) {
      duplicateCount += 1
      if (!firstDuplicateLabel) {
        firstDuplicateLabel = getCompletionItemLabelText(item?.label)
      }
      continue
    }
    seen.add(key)
    suggestions.push(item)
  }
  if (duplicateCount) {
    logger.debug('[Completion] duplicate suggestions removed', {
      sourceCount: source.length,
      suggestionCount: suggestions.length,
      duplicateCount,
      firstDuplicateLabel,
    })
  }
  return suggestions
}

const PYTHON_IDENTIFIER_COMPLETION_RE = /^[A-Za-z_]\w*$/
const EMBEDDED_COMPLETION_FILTER_CONTEXT_MAX_CHARS = 96

const isEmbeddedPythonMemberAccessContext = (languageId, filterContext) => (
  String(languageId || '').trim().toLowerCase() === 'python'
  && String(filterContext?.filterPrefix || '').endsWith('.')
)

const normalizePythonMemberCompletionCandidate = (value) => (
  String(value || '')
    .trim()
    .replace(/\s.*$/, '')
    .replace(/\(.*$/, '')
)

const isEncodedOrPathLikeCompletionText = (value) => {
  const text = String(value || '').trim()
  if (!text) return false
  return /%[0-9a-f]{2}/i.test(text)
    || /[\\/]/.test(text)
    || /:\/\/|^[.]?[0-9a-f]{2}$/i.test(text)
}

const shouldKeepEmbeddedCompletionCandidate = (languageId, rawLabel, rawInsertText, rawFilterText, filterContext) => {
  if (!isEmbeddedPythonMemberAccessContext(languageId, filterContext)) return true

  // Design note:
  // VSCode completion can include word/path candidates from the virtual document.
  // In Python member access (`sys.`) those candidates are invalid attributes; if
  // one is prefixed into `sys..2f`, Monaco's suggest filtering can collapse the
  // whole visible result set into "No suggestions" although LSP returned items.
  const labelCandidate = normalizePythonMemberCompletionCandidate(rawLabel)
  if (!PYTHON_IDENTIFIER_COMPLETION_RE.test(labelCandidate)) return false
  if (isEncodedOrPathLikeCompletionText(rawLabel)) return false
  if (isEncodedOrPathLikeCompletionText(rawInsertText)) return false
  if (isEncodedOrPathLikeCompletionText(rawFilterText)) return false
  return true
}

const shouldDropNoisyEmbeddedMemberCandidate = (languageId, item, completionSource, filterContext) => {
  if (!isEmbeddedPythonMemberAccessContext(languageId, filterContext)) return false
  const rawKind = Number(item?.kind)
  if (!Number.isFinite(rawKind)) return false
  const mappedKind = completionSource === 'direct-lsp'
    ? LSP_TO_MONACO_COMPLETION_KIND[rawKind]
    : VSCODE_TO_MONACO_COMPLETION_KIND[rawKind]

  // 设计说明：
  // 1. Python LSP 在 Markdown fence 虚拟文档里可能把真实成员候选标成 Text。
  // 2. 旧逻辑把 `sys.` 后的 Text 全部视为噪声，导致 rawItemCount 有值但 noisyKindFilteredCount 等于全部数量。
  // 3. Text 候选后续会兜底映射为 Property；这里只继续丢弃 Keyword/Snippet 这类明确不是对象成员的候选。
  return mappedKind === monaco.languages.CompletionItemKind.Keyword
    || mappedKind === monaco.languages.CompletionItemKind.Snippet
}

const resolveEmbeddedCompletionKind = (item, completionSource, filterContext, languageId = '') => {
  const rawKind = Number(item?.kind)
  const kindMap = completionSource === 'direct-lsp'
    ? LSP_TO_MONACO_COMPLETION_KIND
    : VSCODE_TO_MONACO_COMPLETION_KIND
  const mappedKind = Number.isFinite(rawKind) ? kindMap[rawKind] : undefined
  if (
    Number.isFinite(mappedKind)
    && !(isEmbeddedPythonMemberAccessContext(languageId, filterContext) && mappedKind === monaco.languages.CompletionItemKind.Text)
  ) {
    return mappedKind
  }

  // 设计说明：
  // 1. direct LSP 返回协议层 1 基准 kind，VSCode command 返回 API 层 0 基准 kind，
  //    两者如果混用映射表，会把语义候选错误落到 Text。
  // 2. 当前 fenced code/Notebook 都关闭了 `showWords`，Text 候选会被 Monaco 的 kind filter 丢弃。
  // 3. Python 成员访问缺 kind 或被标成 Text 时兜底为 Property，
  //    既保留 LSP 语义补全，又不会放开普通文档单词补全。
  return isEmbeddedPythonMemberAccessContext(languageId, filterContext)
    ? monaco.languages.CompletionItemKind.Property
    : monaco.languages.CompletionItemKind.Text
}

const resolveEmbeddedCompletionFilterContext = (model, position) => {
  if (!model || !position) {
    return {
      hostToken: '',
      typedWord: '',
      filterPrefix: '',
      isMemberAccessContext: false,
      isIncludePathContext: false,
      shouldReplaceFullMemberAccessToken: false,
    }
  }

  const lineContent = String(model.getLineContent?.(position.lineNumber) || '')
  const beforeCursor = lineContent.slice(0, Math.max(0, Number(position.column || 1) - 1))
  const includePathMatch = /^\s*#\s*include\s*([<"])([^>"]*)$/.exec(beforeCursor)
  const hostToken = String(beforeCursor.match(/[^\s]+$/)?.[0] || '').slice(-EMBEDDED_COMPLETION_FILTER_CONTEXT_MAX_CHARS)
  const typedWord = String(hostToken.match(/([A-Za-z0-9_$]*)$/)?.[1] || '')
  const filterPrefix = hostToken.slice(0, Math.max(0, hostToken.length - typedWord.length))
  const isIncludePathContext = !!includePathMatch
  const isMemberAccessContext = !isIncludePathContext && /(\.|::|->)$/.test(filterPrefix)
  const shouldReplaceFullMemberAccessToken = isMemberAccessContext
    && /^(?:[A-Za-z_$][\w$]*)(?:(?:\.|::|->)[A-Za-z_$][\w$]*)*(?:\.|::|->)?[A-Za-z0-9_$]*$/.test(hostToken)

  if (isIncludePathContext) {
    logger.debug('[EmbeddedLsp] resolved include-path completion filter context', {
      lineNumber: Number(position.lineNumber || 0),
      column: Number(position.column || 0),
      delimiter: includePathMatch?.[1] || '',
      typedPath: includePathMatch?.[2] || '',
      hostToken,
      filterPrefix,
    })
  }

  return {
    hostToken,
    typedWord,
    filterPrefix,
    isMemberAccessContext,
    isIncludePathContext,
    shouldReplaceFullMemberAccessToken,
  }
}

const buildEmbeddedCompletionFilterText = (rawFilterText, fallbackLabel, filterContext) => {
  const baseFilterText = String(rawFilterText || fallbackLabel || '').trim()
  if (!baseFilterText) return ''
  if (filterContext?.isIncludePathContext) {
    return baseFilterText
  }
  if (filterContext?.isMemberAccessContext) {
    return baseFilterText
  }

  const prefix = String(filterContext?.filterPrefix || '')
  if (!prefix) return baseFilterText
  if (baseFilterText.startsWith(prefix)) return baseFilterText
  return `${prefix}${baseFilterText}`
}

const buildEmbeddedCompletionInsertText = (rawInsertText, fallbackLabel, filterContext) => {
  const baseInsertText = String(rawInsertText ?? fallbackLabel ?? '')
  if (!baseInsertText) return ''
  if (filterContext?.isIncludePathContext) return baseInsertText
  if (filterContext?.isMemberAccessContext) return baseInsertText

  const prefix = String(filterContext?.filterPrefix || '')
  if (prefix === '#' && baseInsertText.startsWith('#')) {
    const normalized = baseInsertText.replace(/^#+/, '')
    logger.debug('[EmbeddedLsp] normalized preprocessor insert text', {
      rawInsertText: baseInsertText.slice(0, 80),
      normalizedInsertText: normalized.slice(0, 80),
      filterPrefix: prefix,
    })
    return normalized
  }
  if (!prefix || !filterContext?.shouldReplaceFullMemberAccessToken) return baseInsertText
  if (baseInsertText.startsWith(prefix)) return baseInsertText
  return `${prefix}${baseInsertText}`
}

const normalizeEmbeddedCompletionInsertText = (languageId, rawInsertText, fallbackLabel, filterContext) => {
  const baseInsertText = buildEmbeddedCompletionInsertText(rawInsertText, fallbackLabel, filterContext)
  if (!baseInsertText) return ''

  const filterPrefix = String(filterContext?.filterPrefix || '')
  if (!filterContext?.isMemberAccessContext || !filterPrefix) return baseInsertText

  const duplicatedOperator = ['::', '->', '.'].find((operator) => (
    filterPrefix.endsWith(operator)
    && baseInsertText.startsWith(operator)
  ))
  if (!duplicatedOperator) return baseInsertText

  // 设计思路：
  // 1. TypeScript LSP 在 `screen.` 后返回的 textEdit.newText 可能是 `.availHeight`，
  //    这是给虚拟文档 range 使用的完整替换文本。
  // 2. Markdown fence 映射回宿主时，成员访问 range 只覆盖点号后的成员名，点号已经真实存在，
  //    因此必须剥掉服务端 newText 前面的访问符，避免插入成 `screen..availHeight`。
  // 3. 只在成员访问上下文处理 `. / :: / ->`，不会影响 LaTeX 命令、路径补全或普通标识符补全。
  const normalized = baseInsertText.slice(duplicatedOperator.length)
  logger.debug('[EmbeddedLsp] normalized member-access insert text', {
    languageId: String(languageId || ''),
    operator: duplicatedOperator,
    rawInsertText: String(rawInsertText ?? fallbackLabel ?? '').slice(0, 80),
    normalizedInsertText: normalized.slice(0, 80),
    filterPrefix,
  })
  return normalized
}

const getVscodeOrLspCompletionInsertText = (item, fallbackLabel) => {
  if (typeof item?.insertText === 'string') return item.insertText
  if (typeof item?.insertText?.value === 'string') return item.insertText.value
  if (typeof item?.textEdit?.newText === 'string') return item.textEdit.newText
  return String(fallbackLabel || '')
}

const buildEmbeddedCompletionDisplayLabel = (label, filterContext) => {
  const baseLabel = String(label || '').trim()
  if (!baseLabel) return ''

  const prefix = String(filterContext?.filterPrefix || '')
  const typedWord = String(filterContext?.typedWord || '')
  if (!prefix) return baseLabel
  if (filterContext?.isIncludePathContext) {
    return baseLabel
  }
  if (filterContext?.isMemberAccessContext) {
    return baseLabel
  }

  // 设计说明：
  // 1. 非成员访问场景仍可能需要保留宿主前缀，让 Markdown 宿主里的普通补全可以按完整 token 匹配。
  // 2. 成员访问场景已经在上方提前返回原始成员名，避免 `sys.` 被拼进 label/filterText 后和空替换区间打架。
  // 3. 因此这里仅处理非成员访问的兜底前缀拼接。
  if (typedWord && !filterContext?.shouldReplaceFullMemberAccessToken) return baseLabel
  if (baseLabel.startsWith(prefix)) return baseLabel
  return `${prefix}${baseLabel}`
}

const convertVscodeCompletionItemToMonaco = (item, replaceRange, index, filterContext = null, languageId = '', completionSource = 'vscode-command') => {
  const rawLabel = normalizeCompletionLabel(item?.label)
  if (!rawLabel) return null

  const insertTextValue = item?.insertText
  const rawInsertText = getVscodeOrLspCompletionInsertText(item, rawLabel)
  if (!shouldKeepEmbeddedCompletionCandidate(languageId, rawLabel, rawInsertText, item?.filterText, filterContext)) {
    return null
  }
  const insertText = normalizeEmbeddedCompletionInsertText(languageId, rawInsertText, rawLabel, filterContext)
  const isSnippet = typeof insertTextValue === 'object' && insertTextValue !== null
  const documentation = normalizeCompletionDocumentation(item?.documentation)
  const kind = resolveEmbeddedCompletionKind(item, completionSource, filterContext, languageId)
  const label = buildEmbeddedCompletionDisplayLabel(rawLabel, filterContext)

  return {
    label,
    kind,
    detail: item?.detail,
    documentation,
    filterText: buildEmbeddedCompletionFilterText(item?.filterText, rawLabel, filterContext),
    sortText: buildCompletionSortKey(item, rawLabel, index),
    insertText,
    insertTextRules: isSnippet
      || Number(item?.insertTextFormat || 0) === 2
      ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
      : undefined,
    range: replaceRange,
  }
}

const getEmbeddedHostEditor = () => (
  (editor?.hasTextFocus?.() ? editor : null) ||
  (secondaryEditor?.hasTextFocus?.() ? secondaryEditor : null) ||
  editor ||
  secondaryEditor
)

const provideEmbeddedVscodeCompletionSuggestions = async (virtualModel, positionInBlock, replaceRange, context, filterContext = null) => {
  try {
    await retainVsCodeTextModel({
      resource: virtualModel.uri,
      content: virtualModel.getValue(),
      languageId: virtualModel.getLanguageId?.() || '',
      owner: getMarkdownFenceVscodeModelOwner(virtualModel.getLanguageId?.() || ''),
      reason: 'embedded-vscode-completion',
    })

    const triggerCharacter = context?.triggerKind === monaco.languages.CompletionTriggerKind.TriggerCharacter
      ? String(context?.triggerCharacter || '')
      : undefined
    const languageId = String(virtualModel.getLanguageId?.() || '')

    logger.info('[EmbeddedLsp] vscode completion command start', {
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      languageId,
      triggerKind: Number(context?.triggerKind || 0),
      triggerCharacter: triggerCharacter || '',
      lineNumber: positionInBlock?.lineNumber || 0,
      column: positionInBlock?.column || 0,
      linePrefix: String(virtualModel.getLineContent?.(Number(positionInBlock?.lineNumber || 1)) || '')
        .slice(0, Math.max(0, Number(positionInBlock?.column || 1) - 1))
        .slice(-80),
      contentLength: Number(virtualModel.getValue?.()?.length || 0),
    })

    const completionTimeout = new Promise((resolve) => {
      window.setTimeout(() => {
        resolve({ __guaxiCompletionTimeout: true })
      }, 1800)
    })

    // 设计说明：
    // 1. Markdown 内嵌代码块仍然需要一个宿主 provider，把 fenced block 的虚拟文档候选映射回 Markdown range。
    // 2. 但候选来源不再绕过 monaco-languageclient 手写 `textDocument/completion` 请求，
    //    而是统一调用 VSCode/Monaco 已注册的 completion provider，保持 completion/resolve/排序过滤都走成熟 LSP 栈。
    // 3. 日志保留 provider 命令入口，方便确认候选来自标准语言客户端而不是旧 bridge。
    const result = await Promise.race([
      vscodeCommands.executeCommand(
        'vscode.executeCompletionItemProvider',
        VscodeUri.parse(virtualModel.uri.toString()),
        new VscodePosition(positionInBlock.lineNumber - 1, positionInBlock.column - 1),
        triggerCharacter || undefined,
        0
      ),
      completionTimeout,
    ])

    if (result?.__guaxiCompletionTimeout) {
      logger.warn('[EmbeddedLsp] vscode completion command timed out', {
        documentUri: String(virtualModel.uri?.toString?.() || ''),
        languageId: String(virtualModel.getLanguageId?.() || ''),
        triggerKind: Number(context?.triggerKind || 0),
        triggerCharacter: triggerCharacter || '',
        lineNumber: positionInBlock?.lineNumber || 0,
        column: positionInBlock?.column || 0,
      })
      return []
    }

    const items = Array.isArray(result?.items) ? result.items : []
    logger.info('[EmbeddedLsp] vscode completion command resolved', {
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      languageId: String(virtualModel.getLanguageId?.() || ''),
      triggerKind: Number(context?.triggerKind || 0),
      triggerCharacter: triggerCharacter || '',
      lineNumber: positionInBlock?.lineNumber || 0,
      column: positionInBlock?.column || 0,
      itemCount: items.length,
      isIncomplete: !!result?.isIncomplete,
    })
    if (!items.length) return []

    const suggestions = []
    let filteredOutCount = 0
    let noisyKindFilteredCount = 0
    let firstFilteredOutLabel = ''
    let firstRawKind = ''
    let firstMappedKind = ''
    let textKindSuggestionCount = 0
    for (let i = 0; i < items.length; i += 1) {
      const rawItemLabel = normalizeCompletionLabel(items[i]?.label)
      if (shouldDropNoisyEmbeddedMemberCandidate(languageId, items[i], 'vscode-command', filterContext)) {
        noisyKindFilteredCount += 1
        if (!firstFilteredOutLabel && rawItemLabel) firstFilteredOutLabel = rawItemLabel
        continue
      }
      const mapped = convertVscodeCompletionItemToMonaco(items[i], replaceRange, i, filterContext, languageId, 'vscode-command')
      if (!mapped) {
        filteredOutCount += 1
        if (!firstFilteredOutLabel && rawItemLabel) firstFilteredOutLabel = rawItemLabel
        continue
      }
      if (!firstRawKind) firstRawKind = String(items[i]?.kind ?? '')
      if (!firstMappedKind) firstMappedKind = String(mapped.kind ?? '')
      if (mapped.kind === monaco.languages.CompletionItemKind.Text) textKindSuggestionCount += 1
      suggestions.push(mapped)
    }
    logger.debug('[EmbeddedLsp] vscode completion items mapped to Monaco suggestions', {
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      languageId: String(virtualModel.getLanguageId?.() || ''),
      rawItemCount: items.length,
      filteredOutCount,
      noisyKindFilteredCount,
      firstFilteredOutLabel,
      mappedCount: suggestions.length,
      firstMappedLabel: getCompletionItemLabelText(suggestions[0]?.label),
      firstMappedInsertText: String(suggestions[0]?.insertText || ''),
      firstMappedFilterText: String(suggestions[0]?.filterText || ''),
      firstRawKind,
      firstMappedKind,
      textKindSuggestionCount,
      firstMappedRange: suggestions[0]?.range
        ? `${suggestions[0].range.startLineNumber}:${suggestions[0].range.startColumn}-${suggestions[0].range.endLineNumber}:${suggestions[0].range.endColumn}`
        : '',
      shouldReplaceFullMemberAccessToken: !!filterContext?.shouldReplaceFullMemberAccessToken,
      filterPrefix: String(filterContext?.filterPrefix || ''),
    })
    return dedupeCompletionSuggestions(suggestions)
  } catch (error) {
    logger.debug('[EmbeddedLsp] vscode completion command failed, fallback to Monaco providers', error)
    return []
  }
}

const resolveEmbeddedMemberAccessReplaceRange = (model, position) => {
  const hostLineNumber = Number(position?.lineNumber || 1)
  const hostColumn = Number(position?.column || 1)
  const lineContent = String(model?.getLineContent?.(hostLineNumber) || '')
  const beforeCursor = lineContent.slice(0, Math.max(0, hostColumn - 1))
  const hostToken = String(beforeCursor.match(/[^\s]+$/)?.[0] || '')
  if (!hostToken) return null

  const typedWord = String(hostToken.match(/([A-Za-z0-9_$]*)$/)?.[1] || '')
  const filterPrefix = hostToken.slice(0, Math.max(0, hostToken.length - typedWord.length))
  if (!/(\.|::|->)$/.test(filterPrefix)) return null
  if (!/^(?:[A-Za-z_$][\w$]*)(?:(?:\.|::|->)[A-Za-z_$][\w$]*)*(?:\.|::|->)?[A-Za-z0-9_$]*$/.test(hostToken)) {
    return null
  }

  // 设计思路：
  // 1. 成员访问补全要遵守 Monaco 的原生约定：range 只覆盖点号后的当前成员名。
  // 2. 对于简单成员访问链（如 `sys.` / `pkg.mod.ex`），这里只替换点号后的当前成员名。
  // 3. 候选项保持为 `exit/path/argv` 这类成员名，让 Monaco 用 `pa` 匹配 `path`，
  //    避免 `sys.path` 这类带宿主前缀的 filterText 和空 range 组合后被 suggest model 全部过滤。
  const startColumn = Math.max(1, hostColumn - typedWord.length)
  const range = new monaco.Range(
    hostLineNumber,
    startColumn,
    hostLineNumber,
    hostColumn
  )

  logger.debug('[EmbeddedLsp] resolved member-access replace range', {
    hostLineNumber,
    hostColumn,
    hostToken,
    typedWord,
    filterPrefix,
    startColumn,
    endColumn: hostColumn,
    shouldReplaceWholeToken: false,
  })
  return range
}

const resolveEmbeddedIncludePathReplaceRange = (model, position) => {
  const hostLineNumber = Number(position?.lineNumber || 1)
  const hostColumn = Number(position?.column || 1)
  const lineContent = String(model?.getLineContent?.(hostLineNumber) || '')
  const beforeCursor = lineContent.slice(0, Math.max(0, hostColumn - 1))
  const includePathMatch = /^\s*#\s*include\s*([<"])([^>"]*)$/.exec(beforeCursor)
  if (!includePathMatch) return null

  const delimiter = includePathMatch[1]
  const typedPath = includePathMatch[2] || ''
  const closeDelimiter = delimiter === '<' ? '>' : '"'
  const nextChar = lineContent.charAt(Math.max(0, hostColumn - 1))
  const startColumn = Math.max(1, hostColumn - typedPath.length)
  const endColumn = nextChar === closeDelimiter ? hostColumn + 1 : hostColumn
  const range = new monaco.Range(
    hostLineNumber,
    startColumn,
    hostLineNumber,
    Math.max(startColumn, endColumn)
  )

  // 设计思路：
  // 1. `#include <stdio.` 里的点号是头文件路径的一部分，不能按成员访问补全处理。
  // 2. Monaco 输入 `<` 后会自动生成右侧 `>`，clangd 的 include 候选又通常包含结尾 `>`。
  // 3. 因此 range 需要覆盖 `<` 后的已输入路径，并在右侧存在自动闭合符时一并覆盖，避免插入成 `stdio.stdio.h>>`。
  logger.debug('[EmbeddedLsp] resolved include-path replace range', {
    hostLineNumber,
    hostColumn,
    delimiter,
    typedPath,
    nextChar,
    startColumn: range.startColumn,
    endColumn: range.endColumn,
  })
  return range
}

const buildEmbeddedReplaceRange = (model, position, embeddedCtx, virtualModel) => {
  // 设计思路：
  // 1. 这里不能直接照搬宿主 Markdown model 的 `getWordUntilPosition()`。
  // 2. `#include <...` 先按路径补全处理，避免头文件里的 `.` 被误判成成员访问。
  // 3. 对简单成员访问链（`obj.` / `pkg.mod.ex`）我们优先整段替换 host token，
  //    让 Monaco 的过滤文本、显示标签、插入文本保持一致，避免出现“LSP 返回了，但 suggest model 判空”的情况。
  // 4. 其他普通场景仍然基于 embedded 虚拟语言模型计算 replace range，再把列偏移映射回宿主 Markdown 行列。
  const includePathRange = resolveEmbeddedIncludePathReplaceRange(model, position)
  if (includePathRange) {
    return includePathRange
  }

  const memberAccessRange = resolveEmbeddedMemberAccessReplaceRange(model, position)
  if (memberAccessRange) {
    return memberAccessRange
  }

  const hostLineNumber = Number(position?.lineNumber || 1)
  const hostColumn = Number(position?.column || 1)
  const blockPosition = embeddedCtx?.positionInBlock
  const virtualWordInfo = virtualModel?.getWordUntilPosition?.(blockPosition)

  if (!virtualWordInfo || !blockPosition) {
    const hostWordInfo = model.getWordUntilPosition(position);
    const fallbackRange = new monaco.Range(
      hostLineNumber,
      hostWordInfo?.startColumn || hostColumn,
      hostLineNumber,
      hostWordInfo?.endColumn || hostColumn
    )
    logger.debug('[EmbeddedLsp] fallback to host replace range', {
      languageId: embeddedCtx?.languageId || '',
      hostLineNumber,
      hostColumn,
      startColumn: fallbackRange.startColumn,
      endColumn: fallbackRange.endColumn,
    })
    return fallbackRange
  }

  const leftDelta = Math.max(0, Number(blockPosition.column || 1) - Number(virtualWordInfo.startColumn || blockPosition.column || 1))
  const rightDelta = Math.max(0, Number(virtualWordInfo.endColumn || blockPosition.column || 1) - Number(blockPosition.column || 1))
  const startColumn = Math.max(1, hostColumn - leftDelta)
  const endColumn = Math.max(startColumn, hostColumn + rightDelta)
  const range = new monaco.Range(
    hostLineNumber,
    startColumn,
    hostLineNumber,
    endColumn
  )

  logger.debug('[EmbeddedLsp] computed embedded replace range', {
    languageId: embeddedCtx?.languageId || '',
    hostLineNumber,
    hostColumn,
    blockLineNumber: Number(blockPosition.lineNumber || 0),
    blockColumn: Number(blockPosition.column || 0),
    embeddedWord: String(virtualWordInfo.word || ''),
    startColumn,
    endColumn,
  })
  return range
}

const provideEmbeddedLspSuggestions = async (embeddedCtx, model, position, context) => {
  if (!embeddedCtx) return [];
  const hostEditor = getEmbeddedHostEditor()
  const virtualModel = prepareEmbeddedVirtualModel(embeddedCtx);
  if (!virtualModel) return [];
  const ready = await waitForLspConnection(embeddedCtx.languageId, EMBEDDED_LSP_READY_TIMEOUT_MS);
  if (!ready) {
    logger.info('[EmbeddedLsp] completion skipped because connection is not ready', {
      languageId: embeddedCtx.languageId,
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      lineNumber: position?.lineNumber || 0,
      column: position?.column || 0,
      blockLineNumber: embeddedCtx.positionInBlock?.lineNumber || 0,
      blockColumn: embeddedCtx.positionInBlock?.column || 0,
    });
    return [];
  }

  const replaceRange = buildEmbeddedReplaceRange(model, position, embeddedCtx, virtualModel);
  const filterContext = resolveEmbeddedCompletionFilterContext(model, position)
  const cachedSuggestions = readEmbeddedCompletionCache(hostEditor, model, position, embeddedCtx, replaceRange)
  if (cachedSuggestions.length) {
    logger.info('[EmbeddedLsp] completion resolved from hydrated cache', {
      languageId: embeddedCtx.languageId,
      lineNumber: position?.lineNumber || 0,
      column: position?.column || 0,
      suggestionCount: cachedSuggestions.length,
      replaceStartColumn: replaceRange?.startColumn || 0,
      replaceEndColumn: replaceRange?.endColumn || 0,
      firstSuggestion: getCompletionItemLabelText(cachedSuggestions[0]?.label),
      firstSuggestionInsertText: String(cachedSuggestions[0]?.insertText || ''),
    })
    return cachedSuggestions
  }
  logger.debug('[EmbeddedLsp] resolved completion filter context', {
    languageId: embeddedCtx.languageId,
    hostToken: filterContext.hostToken,
    typedWord: filterContext.typedWord,
    filterPrefix: filterContext.filterPrefix,
    isMemberAccessContext: !!filterContext.isMemberAccessContext,
    shouldReplaceFullMemberAccessToken: !!filterContext.shouldReplaceFullMemberAccessToken,
    lineNumber: position?.lineNumber || 0,
    column: position?.column || 0,
    replaceStartColumn: replaceRange?.startColumn || 0,
    replaceEndColumn: replaceRange?.endColumn || 0,
  })

  const vscodeSuggestions = await provideEmbeddedVscodeCompletionSuggestions(
    virtualModel,
    embeddedCtx.positionInBlock,
    replaceRange,
    context,
    filterContext
  )
  if (vscodeSuggestions.length) {
    writeEmbeddedCompletionCache(hostEditor, model, position, embeddedCtx, vscodeSuggestions)
    if (hostEditor) {
      const visibleBeforeRetrigger = isSuggestWidgetVisibleForEditor(hostEditor)
      // 设计说明：
      // 1. 这里的 suggest widget “可见”并不等价于“已经正确展示了 embedded LSP 结果”。
      // 2. 实际排查中最常见的是：Monaco 先显示了一个旧的 `no suggestions` widget，随后异步 LSP 才返回 1423 条结果；
      //    如果我们因为“widget 可见”而跳过 retrigger，用户看到的就一直是空建议。
      // 3. 因此只要拿到了真实 suggestions，就统一走一次基于缓存结果的 hide + retrigger，
      //    让当前会话切换到 embedded fence 的补全结果上。节流仍由 scheduleEmbeddedCompletionRetrigger 内部负责。
      logger.debug('[EmbeddedLsp] schedule retrigger after VSCode completion resolved', {
        languageId: embeddedCtx.languageId,
        suggestionCount: vscodeSuggestions.length,
        visibleBeforeRetrigger,
        lineNumber: position?.lineNumber || 0,
        column: position?.column || 0,
      })
      logSuggestControllerSnapshot(hostEditor, 'before-schedule-retrigger-vscode-completion', {
        languageId: embeddedCtx.languageId,
        suggestionCount: vscodeSuggestions.length,
        lineNumber: position?.lineNumber || 0,
        column: position?.column || 0,
      })
      scheduleEmbeddedCompletionRetrigger(hostEditor, model, position, embeddedCtx, 'vscode-completion')
    }
    logger.debug('[EmbeddedLsp] completion resolved from VSCode command', {
      languageId: embeddedCtx.languageId,
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      suggestionCount: vscodeSuggestions.length,
      lineNumber: embeddedCtx.positionInBlock?.lineNumber || 0,
      column: embeddedCtx.positionInBlock?.column || 0,
      firstSuggestion: getCompletionItemLabelText(vscodeSuggestions[0]?.label),
      firstSuggestionFilterText: String(vscodeSuggestions[0]?.filterText || ''),
    });
    return vscodeSuggestions
  }

  const directLspSuggestions = await provideDirectLspCompletionSuggestions(
    embeddedCtx.languageId,
    virtualModel,
    embeddedCtx.positionInBlock,
    context || { triggerKind: monaco.languages.CompletionTriggerKind.Invoke },
    undefined
  )
  if (directLspSuggestions.length) {
    const languageId = String(virtualModel.getLanguageId?.() || embeddedCtx.languageId || '')
    const mappedSuggestions = []
    let directFilteredOutCount = 0
    let firstDirectFilteredOutLabel = ''
    for (const item of directLspSuggestions) {
      const labelText = getCompletionItemLabelText(item?.label)
      const insertText = getVscodeOrLspCompletionInsertText(item, labelText)
      if (!labelText) continue
      if (!shouldKeepEmbeddedCompletionCandidate(languageId, labelText, insertText, item?.filterText, filterContext)) {
        directFilteredOutCount += 1
        if (!firstDirectFilteredOutLabel) firstDirectFilteredOutLabel = labelText
        continue
      }

      mappedSuggestions.push({
        ...item,
        insertText: normalizeEmbeddedCompletionInsertText(languageId, insertText, labelText, filterContext),
        filterText: buildEmbeddedCompletionFilterText(item?.filterText, labelText, filterContext),
        range: replaceRange,
        textEdit: undefined,
        additionalTextEdits: undefined,
      })
    }

    const dedupedDirectSuggestions = dedupeCompletionSuggestions(mappedSuggestions)
    writeEmbeddedCompletionCache(hostEditor, model, position, embeddedCtx, dedupedDirectSuggestions)
    if (dedupedDirectSuggestions.length && hostEditor) {
      scheduleEmbeddedCompletionRetrigger(hostEditor, model, position, embeddedCtx, 'direct-lsp-completion')
    }
    logger.info('[EmbeddedLsp] completion resolved from direct LSP request', {
      languageId,
      documentUri: String(virtualModel.uri?.toString?.() || ''),
      rawSuggestionCount: directLspSuggestions.length,
      suggestionCount: dedupedDirectSuggestions.length,
      directFilteredOutCount,
      firstDirectFilteredOutLabel,
      lineNumber: embeddedCtx.positionInBlock?.lineNumber || 0,
      column: embeddedCtx.positionInBlock?.column || 0,
      firstSuggestion: getCompletionItemLabelText(dedupedDirectSuggestions[0]?.label),
    })
    if (dedupedDirectSuggestions.length) return dedupedDirectSuggestions
  }

  const providers = (monaco.languages).CompletionProviderRegistry?.all?.(virtualModel) || [];
  if (!providers.length) {
    logger.info('[EmbeddedLsp] no Monaco completion providers available for embedded model', {
      languageId: embeddedCtx.languageId,
      documentUri: String(virtualModel.uri?.toString?.() || ''),
    });
    return [];
  }

  const fallbackToken = {
    isCancellationRequested: false,
    onCancellationRequested: () => ({ dispose() { } })
  };

  const suggestions = [];
  for (const provider of providers) {
    if (!provider?.provideCompletionItems) continue;
    try {
        const result = await provider.provideCompletionItems(
          virtualModel,
          embeddedCtx.positionInBlock,
          context || { triggerKind: monaco.languages.CompletionTriggerKind.Invoke },
          fallbackToken
        );
      const items = Array.isArray(result?.suggestions) ? result.suggestions : [];
      let fallbackFilteredOutCount = 0
      let firstFallbackFilteredOutLabel = ''
      const languageId = String(virtualModel.getLanguageId?.() || '')
      for (const item of items) {
        const labelText = getCompletionItemLabelText(item?.label)
        const insertText = getVscodeOrLspCompletionInsertText(item, labelText)
        if (!labelText) continue;
        if (!shouldKeepEmbeddedCompletionCandidate(languageId, labelText, insertText, item?.filterText, filterContext)) {
          fallbackFilteredOutCount += 1
          if (!firstFallbackFilteredOutLabel) firstFallbackFilteredOutLabel = labelText
          continue
        }

        suggestions.push({
          ...item,
          insertText: normalizeEmbeddedCompletionInsertText(languageId, insertText, labelText, filterContext),
          filterText: buildEmbeddedCompletionFilterText(item?.filterText, labelText, filterContext),
          range: replaceRange,
          textEdit: undefined,
          additionalTextEdits: undefined,
        });
      }
      if (fallbackFilteredOutCount) {
        logger.debug('[EmbeddedLsp] filtered invalid fallback completion candidates', {
          languageId,
          filteredOutCount: fallbackFilteredOutCount,
          firstFilteredOutLabel: firstFallbackFilteredOutLabel,
        })
      }
    } catch {
    }
  }

  const dedupedSuggestions = dedupeCompletionSuggestions(suggestions);
  writeEmbeddedCompletionCache(hostEditor, model, position, embeddedCtx, dedupedSuggestions)
  if (dedupedSuggestions.length && hostEditor) {
    const visibleBeforeRetrigger = isSuggestWidgetVisibleForEditor(hostEditor)
    logger.debug('[EmbeddedLsp] schedule retrigger after Monaco provider completion resolved', {
      languageId: embeddedCtx.languageId,
      suggestionCount: dedupedSuggestions.length,
      visibleBeforeRetrigger,
      lineNumber: position?.lineNumber || 0,
      column: position?.column || 0,
    })
    scheduleEmbeddedCompletionRetrigger(hostEditor, model, position, embeddedCtx, 'monaco-provider-completion')
  }
  logger.debug('[EmbeddedLsp] completion resolved from Monaco providers', {
    languageId: embeddedCtx.languageId,
    documentUri: String(virtualModel.uri?.toString?.() || ''),
    providerCount: providers.length,
    suggestionCount: dedupedSuggestions.length,
    lineNumber: embeddedCtx.positionInBlock?.lineNumber || 0,
    column: embeddedCtx.positionInBlock?.column || 0,
  });
  return dedupedSuggestions;
};

const provideMarkdownFenceLspSuggestions = async (model, position, context) => {
  const ctx = getMarkdownFenceContext(model, position);
  return provideEmbeddedLspSuggestions(ctx, model, position, context);
};

const provideMarkdownMathLspSuggestions = async (model, position, context) => {
  const ctx = getMarkdownMathContext(model, position);
  return provideEmbeddedLspSuggestions(ctx, model, position, context);
};

const convertVscodeSignatureHelpToMonaco = (value) => {
  if (!value || !Array.isArray(value.signatures) || !value.signatures.length) return null
  return {
    value: {
      activeSignature: Number.isFinite(value.activeSignature) ? value.activeSignature : 0,
      activeParameter: Number.isFinite(value.activeParameter) ? value.activeParameter : 0,
      signatures: value.signatures.map((signature) => ({
        label: String(signature?.label || ''),
        documentation: normalizeCompletionDocumentation(signature?.documentation),
        parameters: Array.isArray(signature?.parameters)
          ? signature.parameters.map((parameter) => ({
            label: parameter?.label ?? '',
            documentation: normalizeCompletionDocumentation(parameter?.documentation),
          }))
          : [],
      })),
    },
    dispose() { }
  }
}

const provideEmbeddedSignatureHelp = async (embeddedCtx, context) => {
  if (!embeddedCtx) return null
  const virtualModel = prepareEmbeddedVirtualModel(embeddedCtx)
  if (!virtualModel) return null
  const ready = await waitForLspConnection(embeddedCtx.languageId, EMBEDDED_LSP_READY_TIMEOUT_MS)
  if (!ready) return null

  try {
    await retainVsCodeTextModel({
      resource: virtualModel.uri,
      content: virtualModel.getValue(),
      languageId: embeddedCtx.languageId,
      owner: getMarkdownFenceVscodeModelOwner(embeddedCtx.languageId),
      reason: 'embedded-signature-help',
    })

    const triggerCharacter = String(context?.triggerCharacter || '')
    const result = await vscodeCommands.executeCommand(
      'vscode.executeSignatureHelpProvider',
      VscodeUri.parse(virtualModel.uri.toString()),
      new VscodePosition(embeddedCtx.positionInBlock.lineNumber - 1, embeddedCtx.positionInBlock.column - 1),
      triggerCharacter || undefined
    )
    return convertVscodeSignatureHelpToMonaco(result)
  } catch (error) {
    logger.debug('[EmbeddedLsp] vscode signature help command failed', error)
    return null
  }
}

const EXT_TO_LANGUAGE = {
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  ts: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  css: 'css',
  html: 'html',
  htm: 'html',
  md: 'markdown',
  markdown: 'markdown',
  py: 'python',
  cpp: 'cpp',
  c: 'c',
  h: 'cpp',
  hpp: 'cpp',
  rs: 'rust',
  sh: 'shell',
  jsx: 'javascript',
  tsx: 'typescript',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  lua: 'lua',
  java: 'java',
  tex: 'latex',
  ltx: 'latex',
  latex: 'latex',
  cls: 'latex',
  sty: 'latex',
  mmd: 'mermaid',
  mermaid: 'mermaid',
  toml: 'ini',
}

const MARKDOWN_FENCE_LANG_MAP = {
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  javascript: 'javascript',
  jsx: 'javascript',
  ts: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  tsx: 'typescript',
  py: 'python',
  python: 'python',
  c: 'c',
  cpp: 'cpp',
  cxx: 'cpp',
  cc: 'cpp',
  h: 'cpp',
  hpp: 'cpp',
  rs: 'rust',
  rust: 'rust',
  sh: 'shell',
  shell: 'shell',
  bash: 'shell',
  zsh: 'shell',
  html: 'html',
  css: 'css',
  json: 'json',
  jsonc: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  lua: 'lua',
  java: 'java',
  tex: 'latex',
  latex: 'latex',
  mmd: 'mermaid',
  mermaid: 'mermaid',
}

const resolveMonacoLanguageByPath = (path) => {
  const fileName = String(path || '').split(/[\\/]/).pop() || ''
  const dotIndex = fileName.lastIndexOf('.')
  const ext = dotIndex > 0 ? fileName.slice(dotIndex + 1).toLowerCase() : ''
  const language = EXT_TO_LANGUAGE[ext] || 'plaintext'
  if (!EXT_TO_LANGUAGE[ext]) {
    logger.debug('[Editor] file extension resolved to plaintext', {
      path: String(path || ''),
      ext,
      language,
    })
  }
  return language
}

const resolveFenceLanguageId = (infoRaw) => {
  const info = String(infoRaw || '').trim().toLowerCase()
  if (!info) return 'plaintext'
  const langToken = info.split(/\s+/)[0] || ''
  const mapped = MARKDOWN_FENCE_LANG_MAP[langToken]
  if (mapped) return mapped
  const languages = monaco.languages?.getLanguages?.() || []
  const exists = languages.some((item) => item?.id === langToken)
  if (exists) return langToken
  logger.debug('[MarkdownFence] unknown fence language, fallback to plaintext', { infoRaw })
  return 'plaintext'
}

const refreshLspEnabledSetting = async () => {
  try {
    const enabled = await invoke('get_lsp_syntax_highlight_enabled')
    lspEnabled.value = !!enabled
    applyMonacoThemeByLsp(lspEnabled.value)
  } catch (error) {
    logger.error('加载 LSP 配置失败:', error)
    lspEnabled.value = false
    applyMonacoThemeByLsp(false)
  }
}

// LSP 开关驱动 Monaco 主题，确保高亮使用 One Dark
const applyMonacoTheme = (source = 'manual') => {
  const themeName = getMonacoThemeName()
  try {
    monaco.editor.setTheme(themeName)
    editor?.layout?.()
    editor?.render?.()
    secondaryEditor?.layout?.()
    secondaryEditor?.render?.()
    logger.info('[theme] monaco theme applied', {
      source,
      themeName,
      hasEditor: !!editor,
      hasSecondaryEditor: !!secondaryEditor,
    })
  } catch (error) {
    logger.warn('[theme] monaco theme apply failed', {
      source,
      themeName,
      error,
    })
  }
}

const applyMonacoThemeByLsp = (enabled) => {
  const themeName = getMonacoThemeName()
  logger.info('[theme] apply by lsp start', { enabled, themeName })
  applyMonacoTheme('lsp')
}

const refreshLspRuntimeSnapshotOnce = async (reason = 'editor') => {
  // Design note: installing an extension updates the backend snapshot before
  // the already-open editor knows about it. Share one refresh promise so cursor
  // moves or split editors do not stampede the backend.
  if (!lspRuntimeSnapshotRefreshPromise) {
    logger.info('[DirectLsp] refresh runtime extension snapshot requested', { reason })
    lspRuntimeSnapshotRefreshPromise = refreshLspRuntimeExtensionSnapshot()
      .finally(() => {
        lspRuntimeSnapshotRefreshPromise = null
      })
  }
  return lspRuntimeSnapshotRefreshPromise
}

const startLspClientForEditorLanguage = (targetEditor, lang, filePath, requestSource) => {
  initLspClient(targetEditor, lang, filePath || '', {
    requestSource,
  })
  logger.info('[DirectLsp] client start requested for editor language', {
    languageId: lang,
    filePath: String(filePath || ''),
    requestSource,
  })
}

const shouldPublishEditorLanguageToStatusBar = (targetEditor) => {
  if (!targetEditor) return false
  if (targetEditor === getActiveEditor?.()) return true
  if (targetEditor === editor && fileTreeStore.activeRegion !== 'right') return true
  if (targetEditor === secondaryEditor && fileTreeStore.activeRegion === 'right') return true
  return false
}

const enableLspForEditorLanguage = async (targetEditor, languageId, filePath = '') => {
  if (!targetEditor) return
  const lang = String(languageId || '').trim().toLowerCase()
  if (!lang) return

  ensureLanguagePairingConfig(lang)
  refreshDirectLspSuggestMode(targetEditor, lang)

  initTextmate(targetEditor, lang).catch((e) => {
    logger.warn('[Editor] initTextmate failed, ignored', { language: lang }, e)
  })

  if (!lspEnabled.value) return
  if (!isLspKnownLanguage(lang)) {
    logger.debug('[DirectLsp] skip unknown LSP language', { languageId: lang })
    return
  }

  if (LSP_SUPPORTED_LANGUAGES.has(lang)) {
    startLspClientForEditorLanguage(targetEditor, lang, filePath, 'editor-language-enable')
    return
  }

  await refreshLspRuntimeSnapshotOnce('editor-language-enable-runtime-miss')
  refreshDirectLspSuggestMode(targetEditor, lang)
  if (LSP_SUPPORTED_LANGUAGES.has(lang)) {
    startLspClientForEditorLanguage(targetEditor, lang, filePath, 'editor-language-enable-after-runtime-refresh')
    return
  }

  logger.info('[DirectLsp] runtime not ready for known language after snapshot refresh', {
    languageId: lang,
    filePath: String(filePath || ''),
  })
}

const activateEditorLanguageFeatures = (targetEditor, reason = 'manual') => {
  if (!targetEditor) return
  const model = targetEditor.getModel?.()
  const languageId = String(model?.getLanguageId?.() || 'plaintext').trim().toLowerCase() || 'plaintext'
  const filePath = getEditorBoundTabPath(targetEditor)

  // 设计说明：
  // 1. 切换到 .tex/.py/.html 这类单文件时，Monaco 会直接换成一个已经带 languageId 的新 model。
  //    这种路径不会触发 onDidChangeModelLanguage，因此不能只靠“语言改变”事件同步状态栏和 LSP。
  // 2. 这里把 model 切换、文件切换、手动语言切换统一收口，确保状态栏、tree-sitter 语义高亮、
  //    direct LSP 补全和建议策略都基于当前真实 model language。
  // 3. 非 Markdown 文件不再套用 Markdown 围栏高亮；Markdown 仍走原有的围栏/数学块补全与高亮链路。
  if (shouldPublishEditorLanguageToStatusBar(targetEditor)) {
    editorStore.setLanguage(languageId)
  }

  void enableLspForEditorLanguage(targetEditor, languageId, filePath)
  if (languageId === 'markdown') {
    clearDirectFileSyntaxHighlight(targetEditor)
    scheduleMarkdownFenceSyntaxHighlightAfterContentSync(targetEditor, reason)
    scheduleMarkdownMathSyntaxHighlight(targetEditor, reason)
  } else {
    clearMarkdownFenceSyntaxHighlight(targetEditor)
    clearMarkdownMathSyntaxHighlight(targetEditor)
    scheduleDirectFileSyntaxHighlight(targetEditor, reason)
  }

  logger.info('[Editor] activated language features for bound model', {
    reason,
    languageId,
    filePath: String(filePath || ''),
    uri: String(model?.uri?.toString?.() || ''),
    publishStatusBar: shouldPublishEditorLanguageToStatusBar(targetEditor),
  })
}

const ensureLanguagePairingConfig = (languageId) => {
  const id = languageId || 'plaintext'
  if (languagePairingConfigured.has(id)) return
  if (id !== 'markdown' && id !== 'plaintext') return

  try {
    monaco.languages.setLanguageConfiguration(id, {
      brackets: [
        ['{', '}'],
        ['[', ']'],
        ['(', ')']
      ],
      autoClosingPairs: [
        { open: '{', close: '}' },
        { open: '[', close: ']' },
        { open: '(', close: ')' },
        { open: '"', close: '"' },
        { open: "'", close: "'" },
        { open: '`', close: '`' },
        { open: '“', close: '”' },
        { open: '‘', close: '’' }
      ],
      surroundingPairs: [
        { open: '{', close: '}' },
        { open: '[', close: ']' },
        { open: '(', close: ')' },
        { open: '"', close: '"' },
        { open: "'", close: "'" },
        { open: '`', close: '`' },
        { open: '“', close: '”' },
        { open: '‘', close: '’' }
      ]
    })

    languagePairingConfigured.add(id)
    logger.debug('[Editor] language pairing configured', { languageId: id })
  } catch (e) {
    logger.warn('[Editor] failed to configure language pairing', { languageId: id, error: e })
  }
}

// 初始化编辑器
const initEditor = () => {
  logger.debug('[Editor] initEditor called', {
    hasContainer: !!editorContainer.value,
    selectedPath: selectedFile.value?.path,
    activeTab: fileTreeStore.activeTab
  })

  if (!selectedFile.value) return

  if (!editorContainer.value) {
    if (!initEditorPending) {
      initEditorPending = true;
      nextTick(() => {
        initEditorPending = false;
        if (!editor && editorContainer.value) {
          initEditor();
        }
      });
    }
    logger.debug('[Editor] editor container not ready');
    return;
  }

  // 获取文件扩展名并确定语言
  const language = resolveMonacoLanguageByPath(selectedFile.value?.path || '');

  ensureLanguagePairingConfig(language)

  const isDark = document.documentElement.dataset.guaxiTheme === 'dark';

  monaco.editor.defineTheme('guaxi-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: 'a0a1a7' },           // Comment
      { token: 'keyword', foreground: 'a626a4' },           // Keyword
      { token: 'string', foreground: '50a14f' },           // String
      { token: 'number', foreground: '986801' },           // Number
      { token: 'regexp', foreground: '0184bc' },           // RegExp → 用 Operator 相近色
      { token: 'type', foreground: 'c18401' },           // Type
      { token: 'delimiter', foreground: '383a42' },           // Delimiter → Main text color
      { token: 'variable', foreground: 'e45649' },           // Variable
      { token: 'parameter', foreground: '4078f2' },           // Parameter → Function color
      { token: 'function', foreground: '4078f2' },           // Function
      { token: 'constant', foreground: '986801' },           // Constant → Number color
      { token: 'class', foreground: 'c18401' },           // Class → Type color
      { token: 'identifier', foreground: '383a42' },           // Identifier → Main text color
      { token: 'tag', foreground: 'e45649' },           // Tag → Variable color
      { token: 'attribute.name', foreground: 'c18401' },           // Attribute name → Type color
      { token: 'meta.embedded', foreground: 'a0a1a7' }            // Meta embedded → Comment color
    ],
    semanticHighlighting: true,
    semanticTokenColors: {
      comment: '#a0a1a7',
      string: '#50a14f',
      keyword: '#a626a4',
      number: '#986801',
      regexp: '#0184bc',
      operator: '#0184bc',
      punctuation: '#383a42',
      'punctuation.definition': '#383a42',
      'punctuation.bracket': '#383a42',
      class: '#c18401',
      enum: '#c18401',
      interface: '#c18401',
      type: '#c18401',
      typeParameter: '#c18401',
      namespace: '#c18401',
      struct: '#c18401',
      function: '#4078f2',
      method: '#4078f2',
      property: '#e45649',
      parameter: '#986801',
      variable: '#e45649',
      constant: '#986801',
      attribute: '#986801',
      builtin: '#0184bc',
      'variable.readonly': '#986801',
      'variable.defaultLibrary': '#e45649',
      enumMember: '#986801',
      boolean: '#986801',
      macro: '#a626a4',
      decorator: '#a626a4',
      label: '#a626a4',
      'string.escape': '#0184bc'
    },
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#383a42',
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#383a42',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#e5e5e6',
      'editor.inactiveSelectionBackground': '#e5e5e6',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editor.lineHighlightBorder': '#00000000',
      'editorBracketMatch.background': '#d7dae0',
      'editorBracketMatch.border': '#c5c8ce',
      'editorBracketHighlight.foreground1': '#d73a49',
      'editorBracketHighlight.foreground2': '#f66a0a',
      'editorBracketHighlight.foreground3': '#b08800',
      'editorBracketHighlight.foreground4': '#22863a',
      'editorBracketHighlight.foreground5': '#0366d6',
      'editorBracketHighlight.foreground6': '#6f42c1',
      'editorBracketHighlight.unexpectedBracket.foreground': '#d73a49',
      'editorBracketPairGuide.background1': '#d73a4933',
      'editorBracketPairGuide.background2': '#f66a0a33',
      'editorBracketPairGuide.background3': '#b0880033',
      'editorBracketPairGuide.background4': '#22863a33',
      'editorBracketPairGuide.background5': '#0366d633',
      'editorBracketPairGuide.background6': '#6f42c133',
      'editorBracketPairGuide.activeBackground1': '#d73a4966',
      'editorBracketPairGuide.activeBackground2': '#f66a0a66',
      'editorBracketPairGuide.activeBackground3': '#b0880066',
      'editorBracketPairGuide.activeBackground4': '#22863a66',
      'editorBracketPairGuide.activeBackground5': '#0366d666',
      'editorBracketPairGuide.activeBackground6': '#6f42c166',
      'editor.selectionHighlightBackground': '#d7dae0',
      'editor.foldBackground': '#f0f0f0',
      'editorWhitespace.foreground': '#d0d0d0',
      'editorIndentGuide.background': '#d0d0d0',
      'editorIndentGuide.activeBackground': '#c5c8ce',
      'editorHoverWidget.background': '#fafafa',
      'editorHoverWidget.border': '#dcdfe4',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#dcdfe4',
      'editorSuggestWidget.foreground': '#383a42',
      'editorSuggestWidget.selectedBackground': '#e5e5e6',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#dcdfe4',
      'input.background': '#ffffff',
      'input.foreground': '#383a42',
      'input.border': '#dcdfe4',
      'input.placeholderForeground': '#8c8d91',
      'widget.border': '#dcdfe4',
      'widget.shadow': '#00000022',
      'focusBorder': '#4078f280',
      'editorStickyScroll.background': '#ffffff',
      'editorStickyScrollGutter.background': '#ffffff',
      'editorStickyScrollHover.background': '#f0f0f0',
      'editorStickyScroll.border': '#dcdfe4',
      'editorStickyScroll.shadow': '#00000022',
      'scrollbar.shadow': '#00000033'
    }
  });

  monaco.editor.defineTheme('guaxi-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '5c6370' },           // Comment
      { token: 'keyword', foreground: 'c678dd' },           // Keyword
      { token: 'string', foreground: '98c379' },           // String
      { token: 'number', foreground: 'd19a66' },           // Number
      { token: 'regexp', foreground: '56b6c2' },           // RegExp → Operator color
      { token: 'type', foreground: 'e5c07b' },           // Type
      { token: 'delimiter', foreground: 'abb2bf' },           // Delimiter → Main text color
      { token: 'variable', foreground: 'e06c75' },           // Variable
      { token: 'parameter', foreground: 'd19a66' },           // Parameter
      { token: 'function', foreground: '61afef' },           // Function
      { token: 'constant', foreground: 'd19a66' },           // Constant → Number color
      { token: 'class', foreground: 'e5c07b' },           // Class → Type color
      { token: 'identifier', foreground: 'abb2bf' },           // Identifier → Main text color
      { token: 'tag', foreground: 'e06c75' },           // Tag → Variable color
      { token: 'attribute.name', foreground: 'e5c07b' },           // Attribute name → Type color
      { token: 'meta.embedded', foreground: '5c6370' },           // Meta embedded → Comment color
      { token: 'operator', foreground: '56b6c2' },
      { token: 'punctuation', foreground: 'abb2bf' },
      { token: 'punctuation.definition', foreground: 'abb2bf' },
      { token: 'punctuation.bracket', foreground: 'abb2bf' },
      { token: 'property', foreground: '61afef' },
      { token: 'method', foreground: '61afef' },
      { token: 'function.declaration', foreground: '61afef' },
      { token: 'function.defaultLibrary', foreground: '61afef' },
      { token: 'parameter.declaration', foreground: 'd19a66' },
      { token: 'variable.readonly', foreground: 'd19a66' },
      { token: 'variable.defaultLibrary', foreground: 'e06c75' },
      { token: 'type.defaultLibrary', foreground: 'e5c07b' },
      { token: 'class.defaultLibrary', foreground: 'e5c07b' },
      { token: 'interface', foreground: 'e5c07b' },
      { token: 'enum', foreground: 'e5c07b' },
      { token: 'enumMember', foreground: 'd19a66' },
      { token: 'namespace', foreground: 'e5c07b' },
      { token: 'typeParameter', foreground: 'e5c07b' },
      { token: 'struct', foreground: 'e5c07b' },
      { token: 'macro', foreground: 'c678dd' },
      { token: 'decorator', foreground: 'c678dd' },
      { token: 'label', foreground: 'c678dd' },
      { token: 'keyword.control', foreground: 'c678dd' },
      { token: 'keyword.defaultLibrary', foreground: 'c678dd' },
      { token: 'string.escape', foreground: '56b6c2' },
      { token: 'number.float', foreground: 'd19a66' },
      { token: 'boolean', foreground: 'd19a66' }
    ],
    semanticHighlighting: true,
    semanticTokenColors: {
      comment: '#5c6370',
      string: '#98c379',
      keyword: '#c678dd',
      number: '#d19a66',
      regexp: '#56b6c2',
      operator: '#56b6c2',
      punctuation: '#abb2bf',
      'punctuation.definition': '#abb2bf',
      'punctuation.bracket': '#abb2bf',
      class: '#e5c07b',
      enum: '#e5c07b',
      interface: '#e5c07b',
      type: '#e5c07b',
      typeParameter: '#e5c07b',
      namespace: '#e5c07b',
      struct: '#e5c07b',
      function: '#61afef',
      method: '#61afef',
      property: '#e06c75',
      parameter: '#d19a66',
      'function.declaration': '#61afef',
      'function.defaultLibrary': '#61afef',
      'parameter.declaration': '#d19a66',
      variable: '#e06c75',
      constant: '#d19a66',
      attribute: '#d19a66',
      builtin: '#56b6c2',
      'variable.readonly': '#d19a66',
      'variable.defaultLibrary': '#e06c75',
      enumMember: '#d19a66',
      boolean: '#d19a66',
      macro: '#c678dd',
      decorator: '#c678dd',
      label: '#c678dd',
      'string.escape': '#56b6c2'
    },
    colors: {
      'editor.background': '#282c34',
      'editor.foreground': '#abb2bf',
      'editorLineNumber.foreground': '#5c6370',
      'editorLineNumber.activeForeground': '#abb2bf',
      'editorCursor.foreground': '#528bff',
      'editor.selectionBackground': '#3e4451',
      'editor.inactiveSelectionBackground': '#3e4451',
      'editor.lineHighlightBackground': '#2c313a',
      'editor.lineHighlightBorder': '#00000000',
      'editorBracketMatch.background': '#3e4451',
      'editorBracketMatch.border': '#61afef',
      'editorBracketHighlight.foreground1': '#e06c75',
      'editorBracketHighlight.foreground2': '#d19a66',
      'editorBracketHighlight.foreground3': '#e5c07b',
      'editorBracketHighlight.foreground4': '#98c379',
      'editorBracketHighlight.foreground5': '#56b6c2',
      'editorBracketHighlight.foreground6': '#c678dd',
      'editorBracketHighlight.unexpectedBracket.foreground': '#e06c75',
      'editorBracketPairGuide.background1': '#e06c7540',
      'editorBracketPairGuide.background2': '#d19a6640',
      'editorBracketPairGuide.background3': '#e5c07b40',
      'editorBracketPairGuide.background4': '#98c37940',
      'editorBracketPairGuide.background5': '#56b6c240',
      'editorBracketPairGuide.background6': '#c678dd40',
      'editorBracketPairGuide.activeBackground1': '#e06c7580',
      'editorBracketPairGuide.activeBackground2': '#d19a6680',
      'editorBracketPairGuide.activeBackground3': '#e5c07b80',
      'editorBracketPairGuide.activeBackground4': '#98c37980',
      'editorBracketPairGuide.activeBackground5': '#56b6c280',
      'editorBracketPairGuide.activeBackground6': '#c678dd80',
      'editor.selectionHighlightBackground': '#3e4451',
      'editor.foldBackground': '#21252b',
      'editorWhitespace.foreground': '#4b5263',
      'editorIndentGuide.background': '#4b5263',
      'editorIndentGuide.activeBackground': '#5c6370',
      'editorHoverWidget.background': '#282c34',
      'editorHoverWidget.border': '#3b4048',
      'editorSuggestWidget.background': '#282c34',
      'editorSuggestWidget.border': '#3b4048',
      'editorSuggestWidget.foreground': '#abb2bf',
      'editorSuggestWidget.selectedBackground': '#2c313a',
      'editorWidget.background': '#282c34',
      'editorWidget.border': '#3b4048',
      'editorGutter.background': '#282c34',
      'input.background': '#21252b',
      'input.foreground': '#abb2bf',
      'input.border': '#3b4048',
      'input.placeholderForeground': '#7f8898',
      'inputOption.activeBorder': '#528bff80',
      'widget.border': '#3b4048',
      'widget.shadow': '#00000066',
      'focusBorder': '#528bff80',
      'editorStickyScroll.background': '#282c34',
      'editorStickyScrollGutter.background': '#282c34',
      'editorStickyScrollHover.background': '#2c313a',
      'editorStickyScroll.border': '#00000000',
      'editorStickyScroll.shadow': '#00000066',
      'scrollbar.shadow': '#00000066'
    }
  });

  const themeName = isDark ? 'guaxi-dark' : 'guaxi-light';
  logger.debug('[theme] monaco chrome palette prepared', {
    themeName,
    isDark,
    stickyScrollBackground: isDark ? '#282c34' : '#ffffff',
    inputBorder: isDark ? '#3b4048' : '#dcdfe4'
  });

  editor = monaco.editor.create(editorContainer.value, {
    value: rawContent.value,
    language: language,
    theme: themeName,
    readOnly: !!activeTab.value?.unsupported,
    autoClosingBrackets: 'always',
    autoClosingQuotes: 'always',
    autoClosingDelete: 'auto',
    autoClosingOvertype: 'auto',
    autoSurround: 'languageDefined',
    automaticLayout: true,  // 自动布局
    scrollBeyondLastColumn: 40, // 允许水平滚动超出最后一列
    fixedOverflowWidgets: true, // 确保浮动部件可以超出编辑器边界
    wordWrap: 'on',
    renderValidationDecorations: 'on',
    minimap: {
      enabled: false,  // 启用迷你地图
      showSlider: 'always',
      size: 'fit'
    },
    glyphMargin: true, // 启用行号左侧边栏
    lineDecorationsWidth: 6,
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      useShadows: true,
      verticalScrollbarSize: 10,
      horizontalScrollbarSize: 10,
      arrowSize: 10,
      alwaysConsumeMouseWheel: false,
      verticalHasArrows: false,
      horizontalHasArrows: false,
      handleMouseWheel: true,
      scrollByPage: false,
      theme: {
        scrollbarShadow: '#00000022',
        scrollbarSliderBackground: '#64646466',
        scrollbarSliderHoverBackground: '#64646499',
        scrollbarSliderActiveBackground: '#646464CC'
      }
    },
    vim: {
      enabled: vimMode.value,
      status: true,
      useSystemClipboard: true,
      handleKeys: {
        '<C-s>': false,
        '<C-c>': false,
        '<C-v>': false,
        '<C-a>': false
      }
    },
    contextmenu: {
      enabled: true,
      addDefaultActions: true,
      addNavigationActions: true
    },
    lineNumbers: 'on',
    lineNumbersMinChars: 3,  // 最小行号宽度
    scrollBeyondLastLine: false,
    fontSize: 18,  // 调整为更合适的字号
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    fontLigatures: true,
    // wrappingIndent: 'indent',  // 换行缩进
    renderWhitespace: 'selection',  // 选中时显示空白字符
    renderLineHighlight: 'line',  // 高亮当前行
    lineHeight: 24,  // 行高
    padding: {
      top: 8,  // 顶部内边距
      bottom: 8  // 底部内边距
    },
    suggest: {
      snippetsPreventQuickSuggestions: true  // 允许代码片段建议
    },
    quickSuggestions: {
      other: true,
      comments: false,
      strings: true
    },
    parameterHints: {
      enabled: true  // 启用参数提示
    },
    bracketPairColorization: {
      enabled: true  // 括号配对着色
    },
    guides: {
      bracketPairs: true,  // 显示括号配对引导线
      bracketPairsHorizontal: 'active'  // 水平引导线
    },
    find: {
      addExtraSpaceOnTop: false
    },
    mouseWheelZoom: true  // 启用鼠标滚轮缩放
  });

  ensureEditorModelBinding(editor, {
    filePath: String(selectedFile.value?.path || activeTab.value?.path || ''),
    languageId: language,
    content: rawContent.value,
    owner: PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
    reason: 'primary-editor-init',
  })

  void refreshLspEnabledSetting().then(() => {
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '')
  })
  refreshMarkdownSuggestMode(editor)

  // 注册编辑器实例到store
  editorStore.setEditorInstance(editor);
  editorStore.setLanguage(language);
  logger.debug('[Editor] Monaco editor instance registered')

  if (editorEffectsLeft) {
    editorEffectsLeft.destroy();
    editorEffectsLeft = null;
  }
  editorEffectsLeft = createEditorEffects(editor);
  editorEffectsLeft?.updateCursorTarget();

  registerMarkdownLanguageFeatures();
  registerFilesystemPathCompletionFeatures();

  if (vimMode.value) {
    nextTick(() => {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
      applyVimOverlayInsets();
    });
  }

  // 注册自定义命令
  const registerCommands = () => {
    // Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });
  };

  // 注册自定义命令和菜单项
  const registerContextMenu = () => {
    // 1. 注册Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      contextMenuGroupId: '2_custom',
      contextMenuOrder: 1,
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });

    editor.addAction({
      id: 'guaxi.markdown.formatDocument',
      label: 'Markdown 格式化（全文）',
      run: () => formatMarkdownDocument()
    });

    editor.addAction({
      id: 'guaxi.markdown.formatSelection',
      label: 'Markdown 格式化（选中）',
      run: () => formatMarkdownSelection()
    });

    editor.addAction({
      id: 'guaxi.edit.navigateBack',
      label: '跳转到上次编辑位置',
      keybindings: [monaco.KeyMod.Alt | monaco.KeyCode.LeftArrow],
      run: () => navigateToPreviousEditLocation()
    });

    editor.addAction({
      id: 'guaxi.edit.navigateForward',
      label: '跳转到下一编辑位置',
      keybindings: [monaco.KeyMod.Alt | monaco.KeyCode.RightArrow],
      run: () => navigateToNextEditLocation()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleBold',
      label: '加粗',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyB],
      run: () => toggleBold()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleItalic',
      label: '斜体',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyI],
      run: () => toggleItalic()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleStrikethrough',
      label: '删除线',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyX],
      run: () => toggleStrikethrough()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleInlineCode',
      label: '内联代码',
      run: () => toggleInlineCode()
    });

    editor.addAction({
      id: 'guaxi.markdown.insertLink',
      label: '插入链接',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyK],
      run: () => insertLink()
    });

    editor.addAction({
      id: 'guaxi.markdown.insertTable',
      label: '插入表格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyT],
      run: () => insertTable()
    });

    editor.addAction({
      id: 'guaxi.table.addRowBelow',
      label: '表格：下方插入行',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.DownArrow],
      run: () => addTableRowBelow()
    });

    editor.addAction({
      id: 'guaxi.table.deleteRow',
      label: '表格：删除当前行',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.DownArrow],
      run: () => deleteTableRow()
    });

    editor.addAction({
      id: 'guaxi.table.addColumnRight',
      label: '表格：右侧插入列',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.RightArrow],
      run: () => addTableColumnRight()
    });

    editor.addAction({
      id: 'guaxi.table.deleteColumn',
      label: '表格：删除当前列',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.RightArrow],
      run: () => deleteTableColumn()
    });

    editor.addAction({
      id: 'guaxi.table.moveCellLeft',
      label: '表格：移动到左侧单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyH],
      run: () => moveTableCell(-1, 0)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellDown',
      label: '表格：移动到下方单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyJ],
      run: () => moveTableCell(0, 1)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellUp',
      label: '表格：移动到上方单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyK],
      run: () => moveTableCell(0, -1)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellRight',
      label: '表格：移动到右侧单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyL],
      run: () => moveTableCell(1, 0)
    });

    editor.addAction({
      id: 'guaxi.file.find',
      label: '文件内搜索',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyF],
      run: () => runEditorAction('actions.find')
    });

    editor.onContextMenu((e) => {
      e.event.preventDefault();
      const { posx, posy } = e.event;

      editor.focus();
      editorContextMenuPosition.value = { x: posx, y: posy };
      editorContextMenuOptions.value = buildEditorContextMenuOptions();
      editorContextMenuVisible.value = true;
    });

    outlineStore.setJumpToLine(jumpToLine)
  };

  // 初始化上下文菜单
  registerContextMenu();
  setupEditorListeners();
};

const ensureSecondaryEditor = () => {
  if (!secondaryEditorContainer.value || secondaryEditor) return;

  const rightPath = fileTreeStore.activeTabRight;
  const rightTab = rightPath ? fileTreeStore.openTabs.find((t) => t.path === rightPath) : null;

  const language = resolveMonacoLanguageByPath(rightPath || '');

  const baseOptions = editor?.getRawOptions ? editor.getRawOptions() : {};

  secondaryEditor = monaco.editor.create(secondaryEditorContainer.value, {
    ...baseOptions,
    value: rightTab?.content || '',
    language,
    readOnly: !!rightTab?.unsupported,
    autoClosingBrackets: 'always',
    autoClosingQuotes: 'always',
    autoClosingDelete: 'auto',
    autoClosingOvertype: 'auto',
    autoSurround: 'languageDefined',
    bracketPairColorization: { enabled: true },
    guides: {
      bracketPairs: true,
      bracketPairsHorizontal: 'active'
    },
    automaticLayout: true,
  });

  ensureEditorModelBinding(secondaryEditor, {
    filePath: rightPath || '',
    languageId: language,
    content: rightTab?.content || '',
    owner: SECONDARY_EDITOR_VSCODE_MODEL_OWNER,
    reason: 'secondary-editor-init',
  })

  enableLspForEditorLanguage(secondaryEditor, language, rightPath || '');
  scheduleSourceSyntaxHighlight(secondaryEditor, 'init');
  refreshMarkdownSuggestMode(secondaryEditor)
  scheduleRefreshPythonCellManagers()

  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }
  editorEffectsRight = createEditorEffects(secondaryEditor);
  editorEffectsRight?.updateCursorTarget();

  secondaryEditor.onDidFocusEditorText(() => {
    if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeTabRight) {
      fileTreeStore.setActiveTab(fileTreeStore.activeTabRight, 'right');
    }
    const pos = secondaryEditor?.getPosition?.();
    if (pos) {
      editorStore.setCursorPosition(pos);
    }
  });

  secondaryEditor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    editorEffectsRight?.updateCursorTarget();
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleSourceSyntaxHighlight(secondaryEditor, 'cursor');
  });

  secondaryEditor.onDidScrollChange(() => {
    editorEffectsRight?.updateCursorTarget();
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleSourceSyntaxHighlight(secondaryEditor, 'scroll');
  });

  secondaryEditor.onDidType((text) => {
    editorEffectsRight?.spawnExplosionAtCursor(text);
    editorEffectsRight?.updateCursorTarget();
    maybeTriggerEmbeddedSuggestOnType(secondaryEditor, text)
    maybeTriggerDirectLspSuggestOnType(secondaryEditor, text)
  });

  secondaryEditor.onDidChangeModelLanguage(() => {
    const languageId = secondaryEditor?.getModel?.()?.getLanguageId?.() || 'plaintext';
    enableLspForEditorLanguage(secondaryEditor, languageId, rightPath || '');
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleSourceSyntaxHighlight(secondaryEditor, 'model');
    scheduleRefreshPythonCellManagers()
  });

  secondaryEditor.onDidChangeModel(() => {
    activateEditorLanguageFeatures(secondaryEditor, 'secondary-model-changed')
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleRefreshPythonCellManagers()
  });

  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }
  secondaryMonacoCtrlClickDisposable = installMonacoCtrlClickOpen(secondaryEditor, { tag: 'right' });

  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
  }
  secondaryMonacoLinkLikeDecorDisposable = installMonacoLinkLikeDecorations(secondaryEditor, { tag: 'right' });

  const model = secondaryEditor.getModel();
  if (model) {
    model.onDidChangeContent(() => {
      if (!secondaryEditor) return;
      if (model.getLanguageId() !== 'markdown') return;
      scheduleRefreshPythonCellManagers()
    });

    // 设计说明：
    // 1. 分栏模式下右侧源编辑器的内容需要尽快同步到 rawContent / tab store，预览区才能及时拿到最新文本。
    // 2. 这里把“轻量同步”与“重分析/高亮”拆开：前者即时执行，后者仍保留防抖，避免每个按键都做重活。
    model.onDidChangeContent(() => {
      if (sourceEditorProgrammaticSyncDepth > 0) {
        logger.debug('[Editor] skip right source immediate sync during programmatic update', {
          suppressedDepth: sourceEditorProgrammaticSyncDepth,
        })
        return
      }
      if (fileTreeStore.activeRegion !== 'right') return
      const path = getEditorBoundTabPath(secondaryEditor)
      if (!path) return
      const content = model.getValue()
      rawContent.value = content
      fileTreeStore.updateTabContent(path, content)
      void retainVsCodeTextModel({
        resource: model.uri,
        content,
        languageId: model.getLanguageId?.() || 'plaintext',
        owner: SECONDARY_EDITOR_VSCODE_MODEL_OWNER,
        reason: 'secondary-editor-immediate-sync',
      })
      logger.debug('[Editor] synced right source content immediately', {
        path,
        length: content.length,
      })
    })

    model.onDidChangeContent(
      debounce(() => {
        if (sourceEditorProgrammaticSyncDepth > 0) {
          logger.debug('[Editor] skip right source debounced sync during programmatic update', {
            suppressedDepth: sourceEditorProgrammaticSyncDepth,
          })
          return
        }
        if (fileTreeStore.activeRegion !== 'right') return;
        const path = getEditorBoundTabPath(secondaryEditor);
        if (!path) return;
        const content = model.getValue();
        rawContent.value = content;
        fileTreeStore.updateTabContent(path, content);
        updateGitDecorations();
        refreshMarkdownSuggestMode(secondaryEditor)
        scheduleSourceSyntaxHighlight(secondaryEditor, 'debounced');
        scheduleRefreshPythonCellManagers()
      }, EDITOR_POSTPROCESS_DELAY_MS)
    );
  }
};

const disposeSecondaryEditor = () => {
  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }

  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }

  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
  }

  if (secondaryEditor) {
    releaseVsCodeTextModel(secondaryEditor.getModel?.()?.uri, SECONDARY_EDITOR_VSCODE_MODEL_OWNER, 'secondary-editor-dispose')
    secondaryEditor.dispose();
    secondaryEditor = null;
  }
};

watch(
  () => fileTreeStore.layoutMode,
  (mode) => {
    if (mode === 'single') {
      disposeSecondaryEditor();
    } else {
      nextTick(() => {
        ensureSecondaryEditor();
      });
    }
  },
  { immediate: true }
);


// Git 服务
const gitService = ref(null)

let gitDiffEditor = null
let gitDiffContainerEl = null
let gitDiffOriginalModel = null
let gitDiffModifiedModel = null
let gitDecorationsSeq = 0
let disposeLspConnectionStateListener = null

const ensureGitDiffEditor = (languageId) => {
  const lang = languageId || 'plaintext'

  if (gitDiffEditor && gitDiffOriginalModel && gitDiffModifiedModel) {
    monaco.editor.setModelLanguage(gitDiffOriginalModel, lang)
    monaco.editor.setModelLanguage(gitDiffModifiedModel, lang)
    return
  }

  gitDiffContainerEl = document.createElement('div')
  gitDiffContainerEl.style.position = 'absolute'
  gitDiffContainerEl.style.left = '-99999px'
  gitDiffContainerEl.style.top = '-99999px'
  gitDiffContainerEl.style.width = '1px'
  gitDiffContainerEl.style.height = '1px'
  gitDiffContainerEl.style.overflow = 'hidden'
  document.body.appendChild(gitDiffContainerEl)

  gitDiffEditor = monaco.editor.createDiffEditor(gitDiffContainerEl, {
    renderSideBySide: false,
    readOnly: true,
    renderIndicators: false,
    automaticLayout: false,
    scrollBeyondLastLine: false,
    diffAlgorithm: 'advanced'
  })

  gitDiffOriginalModel = monaco.editor.createModel('', lang)
  gitDiffModifiedModel = monaco.editor.createModel('', lang)

  gitDiffEditor.setModel({
    original: gitDiffOriginalModel,
    modified: gitDiffModifiedModel
  })
}

const computeUnsavedLineChanges = () => {
  const filePath = selectedFile.value?.path
  if (!filePath) return null

  const tab = fileTreeStore.openTabs.find((t) => t?.path === filePath)
  if (!tab) return null

  const original = tab.savedContent ?? ''
  const modified = tab.content ?? ''
  if (original === modified) return { added: [], modified: [], deleted: [] }

  const languageId = editor?.getModel?.()?.getLanguageId?.() || 'plaintext'
  ensureGitDiffEditor(languageId)

  gitDiffOriginalModel.setValue(original)
  gitDiffModifiedModel.setValue(modified)

  const lineChanges = gitDiffEditor.getLineChanges() || []
  const modifiedLineCount = gitDiffModifiedModel.getLineCount()

  const added = new Set()
  const changed = new Set()
  const deleted = new Set()

  const toLen = (start, end) => (end === 0 ? 0 : Math.max(0, end - start + 1))
  const clampLine = (n) => {
    const nn = Number(n) || 1
    if (modifiedLineCount <= 0) return 1
    return Math.min(Math.max(1, nn), modifiedLineCount)
  }

  for (const ch of lineChanges) {
    const oLen = toLen(ch.originalStartLineNumber, ch.originalEndLineNumber)
    const mLen = toLen(ch.modifiedStartLineNumber, ch.modifiedEndLineNumber)

    if (oLen === 0 && mLen > 0) {
      for (let ln = ch.modifiedStartLineNumber; ln <= ch.modifiedEndLineNumber; ln += 1) {
        added.add(clampLine(ln))
      }
      continue
    }

    if (mLen === 0 && oLen > 0) {
      deleted.add(clampLine(ch.modifiedStartLineNumber))
      continue
    }

    if (mLen > 0) {
      for (let ln = ch.modifiedStartLineNumber; ln <= ch.modifiedEndLineNumber; ln += 1) {
        changed.add(clampLine(ln))
      }
    }
  }

  return {
    added: Array.from(added).sort((a, b) => a - b),
    modified: Array.from(changed).sort((a, b) => a - b),
    deleted: Array.from(deleted).sort((a, b) => a - b)
  }
}

// 初始化 Git 服务
const initGitService = async () => {
  const filePath = selectedFile.value?.path
  if (filePath) {
    const dir = await dirname(filePath)
    logger.debug('[Git] initGitService', { filePath, dir })
    gitService.value = new GitService(dir)
    await updateGitStatus()
    return
  }

  logger.debug('[Git] initGitService skipped (no selected file)')
}

// 更新 Git 状态
const updateGitStatus = async () => {
  const filePath = selectedFile.value?.path
  const workspacePath = gitService.value?.workspacePath

  if (!gitService.value || !filePath) {
    logger.debug('[Git] updateGitStatus skipped', {
      hasGitService: !!gitService.value,
      filePath
    })
    return
  }

  try {
    logger.debug('[Git] updateGitStatus start', { filePath, workspacePath })

    const branch = await gitService.value.getCurrentBranch()
    editorStore.setGitBranch(branch)

    const status = await gitService.value.getFileStatus(filePath)
    const counts = {
      added: status.filter(s => s.status === 'A').length,
      modified: status.filter(s => s.status === 'M').length,
      deleted: status.filter(s => s.status === 'D').length
    }

    logger.debug('[Git] fileStatus', {
      filePath,
      statusCount: Array.isArray(status) ? status.length : null,
      counts,
      sample: Array.isArray(status) ? status.slice(0, 3) : status
    })

    editorStore.setGitChanges(counts)

    await updateGitDecorations()
    logger.debug('[Git] updateGitStatus done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    logger.error('[Git] updateGitStatus failed:', error)
  }
}

// 更新 Git 装饰器
const updateGitDecorations = async () => {
  const filePath = selectedFile.value?.path
  const seq = ++gitDecorationsSeq

  const apply = (changes) => {
    if (seq !== gitDecorationsSeq) return

    if (!changes) {
      editorStore.clearGitDecorations()
      return
    }

    editorStore.updateGitLineDecorations({
      added: changes.added || [],
      modified: changes.modified || [],
      deleted: changes.deleted || []
    })
  }

  if (!editor || !filePath || !gitService.value) {
    logger.debug('[Git] updateGitDecorations skipped', {
      hasEditor: !!editor,
      filePath,
      hasGitService: !!gitService.value
    })
    apply(null)
    return
  }

  const tab = fileTreeStore.openTabs.find((t) => t?.path === filePath)
  const preferUnsaved = !!tab?.dirty

  try {
    logger.debug('[Git] updateGitDecorations start', {
      filePath,
      hasEditorStoreInstance: !!editorStore.editorInstance,
      preferUnsaved
    })

    if (preferUnsaved) {
      const changes = computeUnsavedLineChanges()
      apply(changes)
      logger.debug('[Git] updateGitDecorations done (unsaved)', {
        gitDecorationsCount: editorStore.gitDecorations?.length || 0,
        gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
      })
      return
    }

    const changes = await gitService.value.getLineChanges(filePath)
    apply(changes)

    logger.debug('[Git] updateGitDecorations done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    logger.error('[Git] updateGitDecorations failed:', error)
    apply(null)
  }
}

// 切换 Git 面板
const toggleGitPanel = async () => {
  const windowLabel = `git-${Date.now()}` // 唯一窗口标识
  const windowTitle = `${fileTreeStore.activeRepoPath || '未命名文件'}` // 使用不带后缀的文件名

  logger.info('打开 Git 窗口:', windowLabel, windowTitle)
  logger.info('workspace_path:', fileTreeStore.activeRepoPath)
  try {
    await invoke('create_git_window', {
      label: windowLabel,
      title: windowTitle,
      workspacePath: fileTreeStore.activeRepoPath,
      theme: settingsStore.appearanceTheme,
    })
  } catch (error) {
    logger.error('创建窗口失败:', error)
    alert(`无法打开文件: ${error.message || '未知错误'}`)
  }
}

const handleWheelScroll = (e) => {
  const target = e?.target;
  const inRight = !!(target && target.closest && target.closest('.tab-region-right'));
  const container =
    fileTreeStore.layoutMode !== 'single' && inRight
      ? tabRegionRightRef.value
      : tabRegionLeftRef.value;

  if (!container) return;

  // 阻止默认的垂直滚动行为
  e.preventDefault()

  // 水平滚动（乘以一个系数可以调整滚动速度）
  container.scrollLeft += e.deltaY * 0.5
}

const setupEditorListeners = () => {
  if (!editor) return;

  editor.onDidFocusEditorText(() => {
    activateRegion('left');
  });

  if (monacoCtrlClickDisposable) {
    monacoCtrlClickDisposable.dispose();
    monacoCtrlClickDisposable = null;
  }
  monacoCtrlClickDisposable = installMonacoCtrlClickOpen(editor, { tag: 'left' });

  if (monacoLinkLikeDecorDisposable) {
    monacoLinkLikeDecorDisposable.dispose();
    monacoLinkLikeDecorDisposable = null;
  }
  monacoLinkLikeDecorDisposable = installMonacoLinkLikeDecorations(editor, { tag: 'left' });

  scheduleSourceSyntaxHighlight(editor, 'init');
  refreshMarkdownSuggestMode(editor)
  scheduleRefreshPythonCellManagers()

  editor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    updateTableToolbar();
    refreshMarkdownSuggestMode(editor)
    scheduleSourceSyntaxHighlight(editor, 'cursor');
    editorEffectsLeft?.updateCursorTarget();
  });

  const model = editor.getModel();
  if (model) {
    model.onDidChangeContent(() => {
      if (!editor) return;
      if (model.getLanguageId() !== 'markdown') return;
      scheduleRefreshPythonCellManagers()
    });

    model.onDidChangeContent((ev) => {
      if (!editor) return;
      if (tableInsertAutoFormatBusy) return;
      if (tableAutoFormatBusy) return;
      if (Date.now() - lastTableToolbarInsertAt < 450) return;
      if (model.getLanguageId() !== 'markdown') return;
      const selection = editor.getSelection();
      if (!selection) return;
      if (!ev?.changes?.length) return;

      const tableChange = ev.changes.find((c) => {
        if (!c?.text) return false;
        if (c.range?.startColumn !== 1) return false;
        const t = String(c.text).replace(/\r/g, '');
        if (!t.includes('\n')) return false;
        if (!/^\s*\|/.test(t)) return false;
        return /\n\|\s*-{3,}/.test(t);
      });

      if (!tableChange) return;

      tableInsertAutoFormatBusy = true;

      window.setTimeout(() => {
        if (!editor) {
          tableInsertAutoFormatBusy = false;
          return;
        }

        const ctx = detectTableContext();
        if (!ctx) {
          tableInsertAutoFormatBusy = false;
          return;
        }

        const targetLine = Math.min(model.getLineCount(), ctx.startLine + 2);
        editor.setPosition(new monaco.Position(targetLine, 3));
        updateTableToolbar();
        formatTable();

        nextTick(() => {
          tableInsertAutoFormatBusy = false;
        });
      }, 0);
    });

    // 设计说明：
    // 1. 预览区依赖 rawContent 变化触发刷新，如果这里也等重分析 debounce 完成，分栏模式就会出现“输入后明显滞后”的问题。
    // 2. 因此源文本与 tab 脏数据先即时同步，问题分析、语法高亮和 Git 装饰继续防抖，兼顾响应速度与整体开销。
    model.onDidChangeContent(() => {
      if (sourceEditorProgrammaticSyncDepth > 0) {
        logger.debug('[Editor] skip source immediate sync during programmatic update', {
          suppressedDepth: sourceEditorProgrammaticSyncDepth,
        });
        return;
      }
      const content = model.getValue();
      const path = getEditorBoundTabPath(editor);
      rawContent.value = content;
      if (path) fileTreeStore.updateTabContent(path, content);
      void retainVsCodeTextModel({
        resource: model.uri,
        content,
        languageId: model.getLanguageId?.() || 'plaintext',
        owner: PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
        reason: 'primary-editor-immediate-sync',
      })
      logger.debug('[Editor] synced source content immediately', {
        path: String(path || ''),
        length: content.length,
      });
    });

    model.onDidChangeContent(
      debounce(() => {
        if (sourceEditorProgrammaticSyncDepth > 0) {
          logger.debug('[Editor] skip source debounced sync during programmatic update', {
            suppressedDepth: sourceEditorProgrammaticSyncDepth,
          });
          return;
        }
        const content = model.getValue();
        const languageId = model.getLanguageId() || 'plaintext';
        const problems = analyzeContentForProblems(content, model, languageId);
        editorStore.setProblems(problems);
        rawContent.value = content;
        const path = getEditorBoundTabPath(editor);
        if (path) fileTreeStore.updateTabContent(path, content);
        recordEditLocationFromEditor();
        updateTableToolbar();
        scheduleAutoFormatActiveTable();
        refreshMarkdownSuggestMode(editor)
        scheduleSourceSyntaxHighlight(editor, 'debounced');
        updateGitDecorations();
        scheduleRefreshPythonCellManagers()
      }, EDITOR_POSTPROCESS_DELAY_MS)
    );
  }

  const syncRealtimePreviewScroll = debounce(() => {
    if (!editor) return;
    if (!showPreview.value) return;
    if (previewMode.value !== 'realtime') return;

    const ranges = editor.getVisibleRanges();
    const topLine = ranges?.[0]?.startLineNumber || editor.getPosition()?.lineNumber || 1;
    scrollPreviewToLine(topLine);
  }, 50);

  editor.onDidScrollChange(() => {
    if (tableToolbarVisible.value) {
      updateTableToolbar();
    }
    editorEffectsLeft?.updateCursorTarget();
    refreshMarkdownSuggestMode(editor)
    scheduleSourceSyntaxHighlight(editor, 'scroll');
    syncRealtimePreviewScroll();
  });

  editor.onDidChangeModelLanguage(() => {
    const language = editor.getModel()?.getLanguageId() || 'plaintext';
    editorStore.setLanguage(language);
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '');
    updateTableToolbar();
    refreshMarkdownSuggestMode(editor)
    scheduleSourceSyntaxHighlight(editor, 'model');
    scheduleRefreshPythonCellManagers()
  });

  editor.onDidChangeModel(() => {
    activateEditorLanguageFeatures(editor, 'primary-model-changed')
    updateTableToolbar();
    refreshMarkdownSuggestMode(editor)
    scheduleRefreshPythonCellManagers()
  });

  const isSuggestWidgetVisible = () => {
    return isSuggestWidgetVisibleForEditor(editor);
  };

  let lastSnipasteSuggestAt = 0;

  const forceTriggerSuggest = (source, meta) => {
    if (!editor) return;

    const now = Date.now();
    if (now - lastSnipasteSuggestAt < 120) {
      logger.debug('[KeywordSuggest] triggerSuggest skipped (too frequent)', {
        ...meta,
        dt: now - lastSnipasteSuggestAt
      });
      return;
    }
    lastSnipasteSuggestAt = now;

    const visibleBefore = isSuggestWidgetVisible();

    try {
      editor.trigger(source, 'hideSuggestWidget', null);
    } catch {
    }

    logger.debug('[KeywordSuggest] triggerSuggest (force)', {
      ...meta,
      visibleBefore
    });

    editor.trigger(source, 'editor.action.triggerSuggest', {});

    window.setTimeout(() => {
      if (!editor) return;
      const visibleAfter = isSuggestWidgetVisible();
      logger.debug('[KeywordSuggest] triggerSuggest check', { ...meta, visibleAfter });
      if (visibleAfter) return;
      editor.trigger(source, 'editor.action.triggerSuggest', {});
    }, 80);
  };

  editor.onDidType((text) => {
    if (!editor) return;
    const ch = text ? text[text.length - 1] : '';
    if (!ch) return;

    editorEffectsLeft?.spawnExplosionAtCursor(text);
    editorEffectsLeft?.updateCursorTarget();
    maybeTriggerEmbeddedSuggestOnType(editor, text)
    maybeTriggerDirectLspSuggestOnType(editor, text)

    const model = editor.getModel();
    const pos = editor.getPosition();
    if (!model || !pos) return;

    const word = model.getWordUntilPosition(pos)?.word || '';
    const lowerWord = String(word || '').trim().toLowerCase();
    if (lowerWord === 'snipaste') {
      forceTriggerSuggest('guaxi-keyword', { word: lowerWord, line: pos.lineNumber, col: pos.column });
    }

    const lineContent = model.getLineContent(pos.lineNumber);
    const beforeCursor = lineContent.slice(0, pos.column - 1);
    const token = extractPathToken(beforeCursor);
    const pathContext = isFilesystemPathContext(beforeCursor);

    const isPathLike =
      pathContext ||
      (!!token &&
        (token.includes('/') ||
          token.includes('\\') ||
          token.startsWith('.') ||
          token.startsWith('~') ||
          /^[a-zA-Z]:/.test(token)));

    if (!isPathLike) return;

    if (!/[0-9a-zA-Z._\-~\/\\:]/.test(ch)) return;

    logger.debug('[PathCompletion] triggerSuggest', {
      ch,
      token,
      pathContext
    });
    editor.trigger('guaxi-path', 'editor.action.triggerSuggest', {});
  });

  editor.onKeyDown((e) => {
    if (e.keyCode === monaco.KeyCode.Enter) {
      const handled = handleMarkdownListEnter(e);
      if (handled) return;
    }

    if (e.keyCode !== monaco.KeyCode.Tab) return;
    if (!isSuggestWidgetVisible()) return;

    e.preventDefault();
    e.stopPropagation();

    editor.trigger(
      'guaxi-suggest',
      e.shiftKey ? 'selectPrevSuggestion' : 'selectNextSuggestion',
      null
    );
  });

  editor.onDidBlurEditorWidget(() => {
    floatingToolbarVisible.value = false;
    tableToolbarVisible.value = false;
  });

  editor.onDidChangeCursorSelection((e) => {
    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(e.selection);
    const status = selectedText
      ? `Selected ${countLines(selectedText)} lines, ${selectedText.length} chars`
      : null;
    editorStore.setSelectionStatus(status);

    if (model.getLanguageId() !== 'markdown') {
      floatingToolbarVisible.value = false;
      tableToolbarVisible.value = false;
      hidePreviewFloatingToolbar()
      return;
    }

    const isEmpty = e.selection.isEmpty();
    if (isEmpty || !selectedText.trim()) {
      floatingToolbarVisible.value = false;
      return;
    }

    if (e.source !== 'mouse') {
      floatingToolbarVisible.value = false;
      return;
    }

    const wrapperEl = editorWrapperRef.value;
    const editorEl = editorContainer.value;
    if (!wrapperEl || !editorEl) {
      floatingToolbarVisible.value = false;
      return;
    }

    const startPos = e.selection.getStartPosition();
    const endPos = e.selection.getEndPosition();
    const startVisible = editor.getScrolledVisiblePosition(startPos);
    const endVisible = editor.getScrolledVisiblePosition(endPos);
    if (!startVisible || !endVisible) {
      floatingToolbarVisible.value = false;
      return;
    }

    const wrapperRect = wrapperEl.getBoundingClientRect();
    const editorRect = editorEl.getBoundingClientRect();

    const leftBase = editorRect.left - wrapperRect.left;
    const topBase = editorRect.top - wrapperRect.top;

    const selTop = Math.min(startVisible.top, endVisible.top);
    const selBottom = Math.max(startVisible.top + startVisible.height, endVisible.top + endVisible.height);
    const selLeft = Math.min(startVisible.left, endVisible.left);

    const headerEl = editorHeaderRef.value;
    const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
    const minTop = headerHeight + 8;

    const toolbarH = 36;
    const gap = 8;

    const preferTop = topBase + selTop - toolbarH - gap;
    const fallbackTop = topBase + selBottom + gap;

    const top = preferTop >= minTop ? preferTop : fallbackTop;
    const clampedTop = Math.min(Math.max(minTop, top), Math.max(minTop, wrapperRect.height - toolbarH - 8));

    const left = leftBase + selLeft;
    const clampedLeft = Math.min(Math.max(8, left), Math.max(8, wrapperRect.width - 360));

    floatingToolbarStyle.value = {
      left: `${clampedLeft}px`,
      top: `${clampedTop}px`
    };
    selectionSurface.value = 'editor'
    hidePreviewFloatingToolbar()
    floatingToolbarVisible.value = true;
  });
};

// 内容问题分析
function analyzeContentForProblems(content, model, languageId) {
  if (!content) {
    if (model) {
      monaco.editor.setModelMarkers(model, 'markdown-lint', []);
    }
    return { errors: 0, warnings: 0 };
  }

  if (languageId === 'markdown') {
    return analyzeMarkdown(content, model);
  }

  let errors = 0;
  let warnings = 0;

  const lines = content.split('\n');
  lines.forEach((line) => {
    const openBraces = (line.match(/\(/g) || []).length;
    const closeBraces = (line.match(/\)/g) || []).length;
    if (openBraces !== closeBraces) errors++;

    if (line.length > 120) warnings++;
  });

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', []);
  }

  return { errors, warnings };
}

function analyzeMarkdown(content, model) {
  let errors = 0;
  let warnings = 0;
  const markers = [];
  const lines = content.split('\n');
  const pyCellStartRe = /^<!--\s*py(?:\s+\{.*\})?\s*-->$/;
  const pyCellEndRe = /^<!--\s*\/py\s*-->$/;

  const pushMarker = (lineNumber, message, severity) => {
    if (!model) return;
    const maxColumn = model.getLineMaxColumn(lineNumber) || 1;
    markers.push({
      severity:
        severity === 'error'
          ? monaco.MarkerSeverity.Error
          : monaco.MarkerSeverity.Warning,
      message,
      startLineNumber: lineNumber,
      startColumn: 1,
      endLineNumber: lineNumber,
      endColumn: maxColumn
    });
    if (severity === 'error') {
      errors++;
    } else {
      warnings++;
    }
  };

  let lastHeadingLevel = 0;
  const headingTexts = new Map();
  const lintParseContent = maskPythonCellBlocksForMarkdown(content);
  const tokens = md.parse(lintParseContent, {});

  tokens.forEach((token, idx) => {
    if (token.type !== 'heading_open') return;

    const level = Number(token.tag.slice(1));
    const nextToken = tokens[idx + 1];
    const lineNumber = token.map ? token.map[0] + 1 : 1;

    if (!nextToken || !nextToken.content || !nextToken.content.trim()) {
      pushMarker(lineNumber, '标题内容为空', 'warning');
    }

    if (lastHeadingLevel && level > lastHeadingLevel + 1) {
      pushMarker(
        lineNumber,
        `标题级别从 H${lastHeadingLevel} 跳到 H${level}`,
        'warning'
      );
    }

    const text = nextToken?.content?.trim();
    if (text) {
      const existed = headingTexts.get(text);
      if (existed) {
        pushMarker(
          lineNumber,
          `标题与第 ${existed} 行重复`,
          'warning'
        );
      } else {
        headingTexts.set(text, lineNumber);
      }
    }

    lastHeadingLevel = level;
  });

  const fenceLines = [];
  let inFence = false;
  let fenceToken = '';
  let inHiddenPythonCell = false;

  lines.forEach((line, index) => {
    const trimmed = line.trim();

    // 1) fenced code block（```/~~~）：记录行号用于“未闭合”错误，同时在代码块内部跳过链接语法检查。
    const fence = /^(```+|~~~+)/.exec(trimmed);
    if (fence) {
      const token = fence[1];
      fenceLines.push(index + 1);

      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
        inFence = false;
        fenceToken = '';
      }
      return;
    }

    // 1.5) 隐藏 Python Cell 是 Guaxi 的编辑态存储语法，内部是 Python 源码而不是 Markdown 正文。
    //      这里跳过 Cell 内容，避免 `colors=[...]`、DataFrame 索引等代码被误判为“不完整链接”。
    if (pyCellStartRe.test(trimmed)) {
      inHiddenPythonCell = true;
      return;
    }
    if (inHiddenPythonCell) {
      if (pyCellEndRe.test(trimmed)) {
        inHiddenPythonCell = false;
      }
      return;
    }

    if (inFence) return;

    // 2) 任务列表 `- [ ] xxx` / `- [x] xxx`：方括号是合法语法，不参与“不完整链接”检查。
    if (/^\s*(?:[-*+]|\d+\.)\s+\[(?: |x|X)\]\s+/.test(line)) {
      return;
    }

    // 2.5) 目录标记：[TOC] / [toc] / [目录] / [[toc]]
    if (/^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/i.test(trimmed)) {
      return;
    }

    // 3) 剔除 Obsidian 双链与行内代码，降低误报。
    const lintLine = line
      .replace(/\[\[[\s\S]*?\]\]/g, '')
      .replace(/`[^`]*`/g, '')
      .replace(/\\[\[\]\(\)]/g, '');

    const openBracket = (lintLine.match(/\[/g) || []).length;
    const linkStart = (lintLine.match(/\]\s*\(/g) || []).length;
    const closeParen = (lintLine.match(/\)/g) || []).length;
    if (openBracket && (linkStart === 0 || closeParen < linkStart)) {
      pushMarker(index + 1, '可能存在不完整的链接语法', 'warning');
    }
  });

  if (fenceLines.length % 2 !== 0) {
    const lineNumber = fenceLines[fenceLines.length - 1];
    pushMarker(lineNumber, '代码块未正确闭合', 'error');
  }

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', markers);
  }

  return { errors, warnings };
}

// 计算行数
function countLines(text) {
  return text.split(/\r\n|\n|\r/).length;
}

const ensureMarkdownEditor = (targetEditor = getActiveEditor()) => {
  if (!targetEditor) {
    message.warning('没有可编辑的文档');
    return false;
  }
  const model = targetEditor.getModel();
  if (!model) return false;
  const languageId = model.getLanguageId();
  if (languageId !== 'markdown') {
    message.warning('当前工具栏功能仅在 Markdown 文件中可用');
    return false;
  }
  return true;
};

const invokeMarkdownFormat = async (content) => {
  const response = await invoke('markdown_format', {
    request: {
      content,
      options: {
        max_line_width: 0,
        indent_width: 2,
        list_indent: 2,
        format_tables: true,
        format_lists: true,
        preserve_code_blocks: true,
        remove_extra_blank_lines: true,
        fix_extra_whitespace: false
      }
    }
  });

  const success = response?.success;
  const formatted = response?.formatted_content ?? response?.formattedContent;
  const error = response?.error;

  if (!success || typeof formatted !== 'string') {
    throw new Error(typeof error === 'string' ? error : 'Markdown 格式化失败');
  }

  const tableAligned = formatMarkdownTablesInText(formatted, 'markdown-format-command');
  if (tableAligned !== formatted) {
    logger.info('[MarkdownTable] aligned tables after markdown format', {
      originalLength: formatted.length,
      alignedLength: tableAligned.length
    });
  }

  return tableAligned;
};

const formatMarkdownDocument = async () => {
  if (!ensureMarkdownEditor()) return;

  const model = editor.getModel();
  if (!model) return;

  const original = model.getValue();

  try {
    suppressEditLocationRecording = true;
    const formatted = await invokeMarkdownFormat(original);

    if (formatted === original) {
      message.info('无需格式化');
      return;
    }

    editor.executeEdits('markdown-format', [
      {
        range: model.getFullModelRange(),
        text: formatted,
        forceMoveMarkers: true
      }
    ]);

    editor.focus();
  } catch (e) {
    logger.error('Markdown 格式化失败:', e);
    message.error(e instanceof Error ? e.message : 'Markdown 格式化失败');
  } finally {
    suppressEditLocationRecording = false;
  }
};

const formatMarkdownSelection = async () => {
  if (!ensureMarkdownEditor()) return;

  const model = editor.getModel();
  if (!model) return;

  const selection = editor.getSelection();
  if (!selection || selection.isEmpty()) {
    message.info('请先选择需要格式化的内容');
    return;
  }

  const selectedText = model.getValueInRange(selection);
  if (!selectedText.trim()) {
    message.info('请先选择需要格式化的内容');
    return;
  }

  try {
    suppressEditLocationRecording = true;

    let formatted = await invokeMarkdownFormat(selectedText);
    if (!selectedText.endsWith('\n')) {
      formatted = formatted.replace(/\n$/, '');
    }

    editor.executeEdits('markdown-format-selection', [
      {
        range: selection,
        text: formatted,
        forceMoveMarkers: true
      }
    ]);

    editor.focus();
  } catch (e) {
    logger.error('Markdown 格式化失败:', e);
    message.error(e instanceof Error ? e.message : 'Markdown 格式化失败');
  } finally {
    suppressEditLocationRecording = false;
  }
};

const wrapSelection = (prefix, suffix, placeholder = '') => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  const selectedText = model.getValueInRange(selection);
  const finalText = prefix + (selectedText || placeholder) + suffix;

  editor.executeEdits('markdown-toolbar', [
    {
      range: selection,
      text: finalText,
      forceMoveMarkers: true
    }
  ]);

  if (!selectedText && placeholder) {
    const startLineNumber = selection.startLineNumber;
    const startColumn = selection.startColumn + prefix.length;
    const endColumn = startColumn + placeholder.length;
    editor.setSelection(
      new monaco.Selection(
        startLineNumber,
        startColumn,
        startLineNumber,
        endColumn
      )
    );
  }

  editor.focus();
};

const insertBlockSnippet = (snippet) => {
  const targetEditor = getActiveEditor();
  if (!ensureMarkdownEditor(targetEditor)) return;
  const position = targetEditor?.getPosition?.() || new monaco.Position(1, 1);
  const range = new monaco.Range(
    position.lineNumber,
    1,
    position.lineNumber,
    1
  );

  targetEditor.executeEdits('markdown-toolbar', [
    {
      range,
      text: snippet,
      forceMoveMarkers: true
    }
  ]);

  targetEditor.focus();
};

const toggleHeading = (level) => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const startLine = selection.startLineNumber;
  const endLine = selection.endLineNumber;
  const edits = [];

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = model.getLineContent(lineNumber);
    const content = text.replace(/^#{1,6}\s+/, '');

    let next = content;
    if (level > 0) {
      const sameLevel = new RegExp(`^#{${level}}\\s+`).test(text);
      next = sameLevel ? content : `${'#'.repeat(level)} ${content || '标题'}`;
    }

    edits.push({
      range: new monaco.Range(lineNumber, 1, lineNumber, model.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    });
  }

  editor.executeEdits('markdown-toolbar', edits);
  editor.focus();
};

const handleHeadingSelect = (key) => {
  const level = Number(key);
  if (Number.isNaN(level)) return;
  toggleHeading(level);
};

const toggleBold = () => {
  wrapSelection('**', '**', '粗体文本');
};

const toggleItalic = () => {
  wrapSelection('*', '*', '斜体文本');
};

const toggleStrikethrough = () => {
  wrapSelection('~~', '~~', '删除线');
};

const toggleInlineCode = () => {
  wrapSelection('`', '`', 'code');
};

const insertLink = () => {
  wrapSelection('[', '](https://)', '链接文字');
};

const toggleLinePrefix = (prefix, pattern, buildLine) => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const startLine = selection.startLineNumber;
  const endLine = selection.endLineNumber;
  const lines = [];
  let allMatch = true;

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = model.getLineContent(lineNumber);
    lines.push(text);
    if (!pattern.test(text)) allMatch = false;
  }

  const shouldMoveCursor = selection.isEmpty() && startLine === endLine && (!lines[0].trim() || selection.startColumn === 1);

  const edits = lines.map((text, index) => {
    const lineNumber = startLine + index;
    const next = allMatch
      ? text.replace(pattern, '')
      : (buildLine ? buildLine(text, index) : `${prefix}${text}`);

    return {
      range: new monaco.Range(lineNumber, 1, lineNumber, model.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    };
  });

  editor.executeEdits('markdown-toolbar', edits);

  if (shouldMoveCursor && !allMatch) {
    const nextLine = model.getLineContent(startLine);
    const m = nextLine.match(pattern);
    const nextColumn = m ? Math.max(1, m[0].length + 1) : 1;
    editor.setPosition(new monaco.Position(startLine, nextColumn));
  }

  editor.focus();
};

const toggleBlockquote = () => {
  toggleLinePrefix('> ', /^>\s+/, null);
};

const isWordSelection = (raw) => {
  const text = String(raw || '').trim()
  if (!text || /\s/.test(text)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(text)
}

const hidePreviewFloatingToolbar = () => {
  previewFloatingToolbarVisible.value = false
}

const clearPreviewSelectionState = () => {
  previewSelectionText.value = ''
  hidePreviewFloatingToolbar()
}

const previewSearchMatchCountText = computed(() => {
  const total = Array.isArray(previewSearchMatches.value) ? previewSearchMatches.value.length : 0
  if (!total) return '0 / 0'
  const idx = Math.max(0, Number(previewSearchActiveIndex.value || 0))
  return `${Math.min(idx + 1, total)} / ${total}`
})

const escapeRegExp = (raw) => String(raw || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const clearPreviewSearchHighlights = () => {
  const container = previewContainer.value
  if (!container) {
    previewSearchMatches.value = []
    previewSearchActiveIndex.value = -1
    return
  }
  const marks = container.querySelectorAll('span.preview-search-hit')
  marks.forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize?.()
  })
  previewSearchMatches.value = []
  previewSearchActiveIndex.value = -1
}

const focusPreviewSearchMatchAt = (index, options = {}) => {
  const matches = Array.isArray(previewSearchMatches.value) ? previewSearchMatches.value : []
  const total = matches.length
  if (!total) {
    previewSearchActiveIndex.value = -1
    return
  }
  let idx = Number(index)
  if (!Number.isFinite(idx)) idx = 0
  if (idx < 0) idx = total - 1
  if (idx >= total) idx = 0

  matches.forEach((el) => el?.classList?.remove('preview-search-hit-active'))
  const active = matches[idx]
  if (active) {
    active.classList.add('preview-search-hit-active')
    if (options.scroll !== false) {
      active.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
    }
  }
  previewSearchActiveIndex.value = idx
}

const applyPreviewSearchHighlights = () => {
  clearPreviewSearchHighlights()

  const container = previewContainer.value
  const keyword = String(previewSearchKeyword.value || '').trim()
  if (!container || !keyword) return

  const regex = new RegExp(escapeRegExp(keyword), 'gi')
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const text = String(node?.nodeValue || '')
      if (!text) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (parent.closest('.preview-search-panel')) return NodeFilter.FILTER_REJECT
      if (parent.closest('.preview-search-hit')) return NodeFilter.FILTER_REJECT
      if (parent.closest('script, style, code')) return NodeFilter.FILTER_REJECT
      regex.lastIndex = 0
      return regex.test(text) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
    },
  })

  const textNodes = []
  let cur = walker.nextNode()
  while (cur) {
    textNodes.push(cur)
    cur = walker.nextNode()
  }

  const matches = []
  textNodes.forEach((node) => {
    const text = String(node.nodeValue || '')
    const parent = node.parentNode
    if (!parent) return

    regex.lastIndex = 0
    let lastIdx = 0
    let m = regex.exec(text)
    if (!m) return

    const frag = document.createDocumentFragment()
    while (m) {
      const i = Number(m.index || 0)
      const matchedText = String(m[0] || '')
      if (i > lastIdx) {
        frag.appendChild(document.createTextNode(text.slice(lastIdx, i)))
      }
      const span = document.createElement('span')
      span.className = 'preview-search-hit'
      span.textContent = matchedText
      frag.appendChild(span)
      matches.push(span)
      lastIdx = i + matchedText.length
      m = regex.exec(text)
    }
    if (lastIdx < text.length) {
      frag.appendChild(document.createTextNode(text.slice(lastIdx)))
    }
    parent.replaceChild(frag, node)
  })

  previewSearchMatches.value = matches
  if (matches.length) {
    focusPreviewSearchMatchAt(0, { scroll: false })
  }
}

const openPreviewSearch = async () => {
  previewSearchVisible.value = true
  await nextTick()
  const input = previewSearchInputRef.value
  if (input?.focus) input.focus()
  if (input?.select) input.select()
  applyPreviewSearchHighlights()
}

const closePreviewSearch = () => {
  previewSearchVisible.value = false
  previewSearchKeyword.value = ''
  clearPreviewSearchHighlights()
}

const focusNextPreviewSearchMatch = () => {
  focusPreviewSearchMatchAt(previewSearchActiveIndex.value + 1)
}

const focusPrevPreviewSearchMatch = () => {
  focusPreviewSearchMatchAt(previewSearchActiveIndex.value - 1)
}

const updatePreviewSelectionFromWindow = () => {
  const selection = window.getSelection?.()
  const container = previewContainer.value
  if (!selection || !container || selection.rangeCount === 0) return ''
  const text = String(selection.toString() || '')
  if (!text.trim()) return ''
  const range = selection.getRangeAt(0)
  const target = range.commonAncestorContainer
  const el = target instanceof Element ? target : target?.parentElement
  if (!el || !container.contains(el)) return ''
  return text
}

const getSelectedTextForTranslate = () => {
  if (selectionSurface.value === 'preview') {
    const text = normalizeSelectionTextForLanguageTasks(previewSelectionText.value || updatePreviewSelectionFromWindow())
    if (text) return text
  }

  if (editor) {
    const model = editor.getModel();
    const selection = editor.getSelection();
    if (model && selection && !selection.isEmpty()) {
      const text = normalizeSelectionTextForLanguageTasks(model.getValueInRange(selection) || '')
      if (text) {
        selectionSurface.value = 'editor'
        return text
      }
    }
  }

  const previewText = normalizeSelectionTextForLanguageTasks(previewSelectionText.value || updatePreviewSelectionFromWindow())
  if (previewText) {
    selectionSurface.value = 'preview'
    return previewText
  }

  return '';
};

const handlePreviewSelectionMouseUp = (e) => {
  if (!showPreview.value || previewMode.value === 'realtime') return
  const container = previewContainer.value
  const shell = previewShellRef.value
  if (!container || !shell) return

  const text = normalizeSelectionTextForLanguageTasks(updatePreviewSelectionFromWindow())
  if (!text) {
    clearPreviewSelectionState()
    return
  }

  previewSelectionText.value = text
  selectionSurface.value = 'preview'

  const shellRect = shell.getBoundingClientRect()
  const x = Number(e?.clientX || shellRect.left)
  const y = Number(e?.clientY || shellRect.top)
  const toolbarW = 232
  const toolbarH = 34
  const pad = 8
  const gap = 10

  let left = x - shellRect.left
  let top = y - shellRect.top - toolbarH - gap
  if (top < pad) {
    top = y - shellRect.top + gap
  }
  const maxLeft = Math.max(pad, shellRect.width - toolbarW - pad)
  const maxTop = Math.max(pad, shellRect.height - toolbarH - pad)

  previewFloatingToolbarStyle.value = {
    left: `${Math.min(Math.max(pad, left), maxLeft)}px`,
    top: `${Math.min(Math.max(pad, top), maxTop)}px`,
  }
  previewFloatingToolbarVisible.value = true
}

const handleGlobalSelectionChange = () => {
  if (selectionSurface.value !== 'preview') return
  const text = normalizeSelectionTextForLanguageTasks(updatePreviewSelectionFromWindow())
  if (!text) {
    clearPreviewSelectionState()
  } else {
    previewSelectionText.value = text
  }
}

const handleGlobalMouseDownForPreviewToolbar = (event) => {
  if (!previewFloatingToolbarVisible.value) return
  const target = event?.target instanceof Element ? event.target : null
  if (!target) return
  if (target.closest('.floating-md-toolbar')) return
  if (target.closest('.pot-selection-translate-panel')) return
  if (target.closest('.preview')) return
  clearPreviewSelectionState()
}

const handleSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return;
  const left = Number(pos.left);
  const top = Number(pos.top);
  if (!Number.isFinite(left) || !Number.isFinite(top)) return;

  const cur = selectionTranslatePanelPos.value
  if (cur && Number(cur.left) === left && Number(cur.top) === top) return

  selectionTranslatePanelPos.value = { left, top };
};

const toggleSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value;
  logger.info('[SelectionTranslate] pin:', { pinned: selectionTranslatePinned.value });
};

const swapSelectionTranslateLangs = () => {
  const tmp = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = tmp
  logger.info('[SelectionTranslate] swap langs', { from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateSelectionText(selectionTranslateRaw.value)
  }
}

const closeSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false;
  selectionTranslatePinned.value = false;
  selectionTranslateWordMeta.value = null
};

const openSelectionTranslatePanel = async () => {
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate());
  if (!text) {
    message.warning('请先框选要翻译的文本');
    return;
  }

  const usePreviewAnchor = selectionSurface.value === 'preview' && previewFloatingToolbarVisible.value
  const anchorStyle = usePreviewAnchor ? previewFloatingToolbarStyle.value : floatingToolbarStyle.value
  const left = Number.parseFloat(String(anchorStyle?.left || ''));
  const top = Number.parseFloat(String(anchorStyle?.top || ''));

  const wrapperEl = usePreviewAnchor ? previewShellRef.value : editorWrapperRef.value
  const pw = wrapperEl?.clientWidth ?? window.innerWidth
  const ph = wrapperEl?.clientHeight ?? window.innerHeight

  const pad = 8
  const panelW = 360
  const panelH = 260
  const maxLeft = Math.max(pad, pw - panelW - pad)
  const maxTop = Math.max(pad, ph - panelH - pad)

  const nextLeft = Number.isFinite(left) ? left : pad
  const nextTop = Number.isFinite(top) ? top + 44 : 52

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, nextLeft), maxLeft),
    top: Math.min(Math.max(pad, nextTop), maxTop)
  };
  selectionTranslatePanelVisible.value = true;
  if (usePreviewAnchor) {
    hidePreviewFloatingToolbar()
  }

  await translateSelectionText(text);
};

const sendSelectionToAssistant = (overrideText, options = {}) => {
  const text = String(overrideText ?? getSelectedTextForTranslate()).trim()
  if (!text) {
    message.warning('请先框选要发送到助手的文本')
    return false
  }
  const autoSend = options.autoSend !== false
  const disableWebSearch = !!options.disableWebSearch
  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed('home', false)
  rightSiderStore.appendAssistantDraft(text, { autoSend, disableWebSearch })
  message.success(options.successMessage || (autoSend ? '已发送到助手并开始提问' : '已插入到助手输入框'))
  return true
}

const askSelectionWithAssistant = () => {
  sendSelectionToAssistant(undefined, { autoSend: true, successMessage: '已发送到助手并开始提问' })
}

const getTranslateLangLabel = (value) => {
  const item = assistantTranslateLangOptions.find((x) => x.value === value)
  return item ? String(item.label || '') : '目标语言'
}

const openAssistantTranslateDialog = () => {
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate())
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  assistantTranslateSourceText.value = text
  assistantTranslateDialogVisible.value = true
}

const sendSelectionAsAiTranslate = () => {
  const text = normalizeSelectionTextForLanguageTasks(assistantTranslateSourceText.value)
  if (!text) {
    assistantTranslateDialogVisible.value = false
    message.warning('请先框选要翻译的文本')
    return
  }
  const lang = String(assistantTranslateTargetLang.value || 'en')
  const label = getTranslateLangLabel(lang)
  const prompt = `请将以下内容翻译成${label}，保持原意并保留术语、代码与格式，只输出译文：\n\n${text}`
  const ok = sendSelectionToAssistant(prompt, {
    autoSend: true,
    disableWebSearch: true,
    successMessage: `已发送到助手并翻译为${label}`,
  })
  if (ok) {
    assistantTranslateDialogVisible.value = false
  }
}

const translateSelectionText = async (overrideText) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? getSelectedTextForTranslate());
  if (!text) {
    message.warning('请先框选要翻译的文本');
    return;
  }

  if (selectionTranslateBusy.value) return;

  selectionTranslateRaw.value = text;
  selectionTranslateResult.value = '';
  selectionTranslateError.value = '';
  selectionTranslateItems.value = [];
  selectionTranslateWordMeta.value = null
  selectionTranslateBusy.value = true;

  logger.info('[SelectionTranslate] start', { len: text.length, from: selectionTranslateFrom.value, to: selectionTranslateTo.value });

  try {
    let hasUsableMultiResult = false
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value
        });

        const items = Array.isArray(response?.items) ? response.items : []
        selectionTranslateItems.value = items.map((it) => ({
          engine: String(it?.engine || ''),
          success: Boolean(it?.success),
          text: String(it?.text || ''),
          error: it?.error ? String(it.error) : ''
        }))

        const primary = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        if (primary) {
          selectionTranslateResult.value = String(primary.text || '')
          selectionTranslateError.value = ''
          hasUsableMultiResult = true
          logger.info('[SelectionTranslate] multi success', { engines: selectionTranslateItems.value.length })
        } else {
          const err = String(response?.error || '翻译结果为空')
          throw new Error(err)
        }
      } catch (err) {
        const msg = String(err?.message || err || '')
        if (msg.includes('selection_translate_free_multi') && msg.toLowerCase().includes('not found')) {
          selectionTranslateMultiAvailable.value = false
          if (!selectionTranslateMultiNotFoundWarned) {
            selectionTranslateMultiNotFoundWarned = true
            logger.warn('[SelectionTranslate] multi command not found, disabled multi path. Please restart tauri backend.')
          }
        } else {
          logger.warn('[SelectionTranslate] multi failed, fallback to single:', err)
        }
      }
    }

    if (!hasUsableMultiResult) {
      const response = await invoke('translate_text_free', {
        texts: [text],
        from: selectionTranslateFrom.value,
        to: selectionTranslateTo.value
      });
      const translated = (response?.translated_texts?.[0] || '').toString();

      if (!translated.trim() || translated.includes('[Translation Failed]')) {
        const err = String(response?.error || '翻译结果为空');
        throw new Error(err);
      }

      if (!selectionTranslateResult.value) {
        selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
        selectionTranslateResult.value = translated;
        logger.info('[SelectionTranslate] single success', { len: translated.length });
      }
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (isWordSelection(text)) {
      try {
        const basic = await invoke('language_learning_enrich_word_basic', {
          word: text,
          sourceLang: selectionTranslateFrom.value,
          targetLang: selectionTranslateTo.value
        })
        selectionTranslateWordMeta.value = {
          word: String(basic?.word || text).trim(),
          phonetic: String(basic?.phonetic || '').trim(),
          phonetic_uk: String(basic?.phonetic_uk || '').trim(),
          phonetic_us: String(basic?.phonetic_us || '').trim(),
          part_of_speech: String(basic?.part_of_speech || '').trim(),
          meanings: Array.isArray(basic?.meanings) ? basic.meanings : [],
          synonyms: [],
          antonyms: [],
          associates: [],
          synonyms_translations: [],
          antonyms_translations: [],
          associates_translations: [],
          phrases: [],
          phrase_translations: [],
          examples: [],
          example_translations: [],
        }
        const details = await invoke('language_learning_enrich_word_details', {
          word: text,
          sourceLang: selectionTranslateFrom.value,
          targetLang: selectionTranslateTo.value
        })
        selectionTranslateWordMeta.value = {
          ...selectionTranslateWordMeta.value,
          synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
          antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
          associates: Array.isArray(details?.associates) ? details.associates : [],
          synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
          antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
          associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
          phrases: Array.isArray(details?.phrases) ? details.phrases : [],
          phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
          examples: Array.isArray(details?.examples) ? details.examples : [],
          example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
        }
      } catch (err) {
        logger.warn('[SelectionTranslate] enrich word failed:', err)
      }
    }

    if (finalTranslated) {
      try {
        const primaryItem = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        await invoke('language_learning_record_translation', {
          input: {
            source_text: text,
            translated_text: finalTranslated,
            source_lang: selectionTranslateFrom.value,
            target_lang: selectionTranslateTo.value,
            engine: String(primaryItem?.engine || 'Free'),
            record_type: isWordSelection(text) ? 'word' : undefined,
            word_meta: selectionTranslateWordMeta.value || undefined,
          }
        })
      } catch (err) {
        logger.warn('[SelectionTranslate] record save failed:', err)
      }
    }
  } catch (err) {
    const msg = String(err?.message || err || '翻译失败');
    selectionTranslateError.value = msg;
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: msg }]
    logger.error('[SelectionTranslate] failed:', err);
    message.error(`翻译失败: ${msg}`);
  } finally {
    selectionTranslateBusy.value = false;
  }
};

const copySelectionTranslateResult = async (payload) => {
  const t = typeof payload === 'string' ? payload : payload?.text
  const text = String(t || selectionTranslateResult.value || '')
  if (!text) return;

  const ok = await writeClipboardText(text);
  if (ok) {
    message.success('已复制译文', { duration: 1200 });
  } else {
    message.warning('复制失败：无剪贴板权限');
  }
};

const toggleUnorderedList = () => {
  toggleLinePrefix('- ', /^-\s+/, null);
};

const toggleOrderedList = () => {
  toggleLinePrefix(
    '1. ',
    /^\d+\.\s+/,
    (text, index) => `${index + 1}. ${text}`
  );
};

const toggleTaskList = () => {
  toggleLinePrefix('- [ ] ', /^-\s\[( |x|X)\]\s+/, null);
};

const isMarkdownListDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_md_list') || '').trim().toLowerCase();
    return v === '1' || v === 'true' || v === 'yes' || v === 'on';
  } catch {
    return false;
  }
};

const parseMarkdownListPrefix = (line) => {
  const text = String(line || '');

  const task = /^([\t ]*)([-+*])\s+\[( |x|X)\]\s+/.exec(text);
  if (task) {
    return {
      kind: 'task',
      indent: task[1] || '',
      marker: task[2] || '-',
      checked: (task[3] || '').toLowerCase() === 'x',
      prefix: task[0]
    };
  }

  const ordered = /^([\t ]*)(\d+)\.\s+/.exec(text);
  if (ordered) {
    return {
      kind: 'ordered',
      indent: ordered[1] || '',
      number: Number(ordered[2] || 1) || 1,
      prefix: ordered[0]
    };
  }

  const unordered = /^([\t ]*)([-+*])\s+/.exec(text);
  if (unordered) {
    return {
      kind: 'unordered',
      indent: unordered[1] || '',
      marker: unordered[2] || '-',
      prefix: unordered[0]
    };
  }

  return null;
};

const handleMarkdownListEnter = (e) => {
  if (!editor) return false;

  const model = editor.getModel();
  const pos = editor.getPosition();
  const selection = editor.getSelection();
  if (!model || !pos || !selection || !selection.isEmpty()) return false;

  if (model.getLanguageId() !== 'markdown') return false;

  const lineText = model.getLineContent(pos.lineNumber);
  const parsed = parseMarkdownListPrefix(lineText);
  if (!parsed) return false;

  if (e.shiftKey || e.altKey || e.ctrlKey || e.metaKey) return false;

  const afterPrefix = lineText.slice(parsed.prefix.length);
  const isEmptyItem = afterPrefix.trim().length === 0 && pos.column >= parsed.prefix.length + 1;

  e.preventDefault();
  e.stopPropagation();

  if (isMarkdownListDebug()) {
    logger.debug('[MarkdownList] enter', {
      lineNumber: pos.lineNumber,
      kind: parsed.kind,
      isEmptyItem
    });
  }

  if (isEmptyItem) {
    const fullRange = new monaco.Range(pos.lineNumber, 1, pos.lineNumber, model.getLineMaxColumn(pos.lineNumber));
    editor.executeEdits('md-list-enter-exit', [
      {
        range: fullRange,
        text: `${parsed.indent}\n${parsed.indent}`,
        forceMoveMarkers: true
      }
    ]);
    editor.setPosition(new monaco.Position(pos.lineNumber + 1, parsed.indent.length + 1));
    editor.focus();
    return true;
  }

  let nextPrefix = '';
  if (parsed.kind === 'ordered') {
    nextPrefix = `${parsed.indent}${parsed.number + 1}. `;
  } else if (parsed.kind === 'task') {
    nextPrefix = `${parsed.indent}${parsed.marker} [ ] `;
  } else {
    nextPrefix = `${parsed.indent}${parsed.marker} `;
  }

  editor.executeEdits('md-list-enter-continue', [
    {
      range: new monaco.Range(pos.lineNumber, pos.column, pos.lineNumber, pos.column),
      text: `\n${nextPrefix}`,
      forceMoveMarkers: true
    }
  ]);

  editor.setPosition(new monaco.Position(pos.lineNumber + 1, nextPrefix.length + 1));
  editor.focus();
  return true;
};

const isPotentialTableLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  const pipeCount = getMarkdownTablePipeCount(trimmed);
  return pipeCount >= 2;
};

const getMarkdownTablePipeCount = (line) => {
  const text = String(line || '');
  let count = 0;
  let escaped = false;
  let codeTickLen = 0;

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      escaped = true;
      continue;
    }
    if (ch === '`') {
      let j = i;
      while (j < text.length && text[j] === '`') j += 1;
      const len = j - i;
      if (!codeTickLen) {
        codeTickLen = len;
      } else if (len >= codeTickLen) {
        codeTickLen = 0;
      }
      i = j - 1;
      continue;
    }
    if (ch === '|' && !codeTickLen) count += 1;
  }

  return count;
};

const splitMarkdownTableRowCells = (line) => {
  const raw = String(line || '').trim();
  let text = raw;
  if (!text.startsWith('|')) text = `| ${text}`;
  if (!text.endsWith('|')) text = `${text} |`;

  const parts = [];
  let current = '';
  let escaped = false;
  let codeTickLen = 0;

  // 设计说明：
  // 1. Markdown 表格允许用 `\|` 表示单元格内的竖线，也允许在代码 span 中出现 `|`。
  // 2. 旧版直接 split('|') 会把这些内容误拆成多列，导致“某行分隔符数量不一致”时越格式化越歪。
  // 3. 这里只把非转义、非代码 span 中的 `|` 当作列分隔符。
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (escaped) {
      current += ch;
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      current += ch;
      escaped = true;
      continue;
    }
    if (ch === '`') {
      let j = i;
      while (j < text.length && text[j] === '`') j += 1;
      const ticks = text.slice(i, j);
      current += ticks;
      const len = ticks.length;
      if (!codeTickLen) {
        codeTickLen = len;
      } else if (len >= codeTickLen) {
        codeTickLen = 0;
      }
      i = j - 1;
      continue;
    }
    if (ch === '|' && !codeTickLen) {
      parts.push(current);
      current = '';
      continue;
    }
    current += ch;
  }
  parts.push(current);

  if (parts.length && !parts[0].trim()) parts.shift();
  if (parts.length && !parts[parts.length - 1].trim()) parts.pop();
  return parts.map((c) => c.trim());
};

const isTableSeparatorLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  const cells = splitMarkdownTableRowCells(trimmed);
  return cells.length >= 2 && cells.every((cell) => /^\s*:?-{3,}:?\s*$/.test(cell));
};

const normalizeTableRowCells = (line) => {
  return splitMarkdownTableRowCells(line);
};

const buildTableRow = (cells) => {
  return `| ${cells.map((c) => (c ?? '').trim()).join(' | ')} |`;
};

const detectSeparatorAlign = (cell) => {
  const v = String(cell || '').trim();
  const left = v.startsWith(':');
  const right = v.endsWith(':');
  if (left && right) return 'center';
  if (right) return 'right';
  if (left) return 'left';
  return null;
};

const normalizeSeparatorCell = (cell, align) => {
  const base = (cell || '').trim();
  const hasMin = base.replace(/:/g, '').replace(/-/g, '').length === 0;
  const detected = align || detectSeparatorAlign(base);

  if (!hasMin) {
    if (detected === 'left') return ':---';
    if (detected === 'center') return ':---:';
    if (detected === 'right') return '---:';
    return '---';
  }

  if (detected === 'left') return ':---';
  if (detected === 'center') return ':---:';
  if (detected === 'right') return '---:';
  return '---';
};

// 判断 Unicode 组合附加符号（Combining Marks）。
// 这类字符通常叠加在前一个字符上显示，因此在“可视宽度”计算中按 0 宽度处理。
const isCombiningCodePoint = (cp) => {
  return (
    (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x1ab0 && cp <= 0x1aff) ||
    (cp >= 0x1dc0 && cp <= 0x1dff) ||
    (cp >= 0x20d0 && cp <= 0x20ff) ||
    (cp >= 0xfe20 && cp <= 0xfe2f)
  );
};

const isZeroWidthCodePoint = (cp) => {
  return (
    isCombiningCodePoint(cp) ||
    cp === 0x200d ||
    cp === 0xfe0e ||
    cp === 0xfe0f ||
    (cp >= 0x1f3fb && cp <= 0x1f3ff) ||
    (cp >= 0xe0100 && cp <= 0xe01ef)
  );
};

const isEmojiLikeCodePoint = (cp) => {
  return (
    (cp >= 0x2300 && cp <= 0x23ff) ||
    (cp >= 0x2600 && cp <= 0x27bf) ||
    (cp >= 0x1f000 && cp <= 0x1faff)
  );
};

// 判断 Unicode 码点是否为“全宽”字符。
// 用于近似估算等宽字体下的显示宽度：CJK/全角标点/emoji 等通常按 2 列宽处理。
const isFullWidthCodePoint = (cp) => {
  if (cp >= 0x1100 && (
    cp <= 0x115f ||
    cp === 0x2329 ||
    cp === 0x232a ||
    (cp >= 0x2e80 && cp <= 0xa4cf && cp !== 0x303f) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe10 && cp <= 0xfe19) ||
    (cp >= 0xfe30 && cp <= 0xfe6f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1f64f) ||
    (cp >= 0x1f680 && cp <= 0x1f6ff) ||
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x1fa70 && cp <= 0x1faff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  )) return true;

  return false;
};

const segmentDisplayGraphemes = (input) => {
  const s = String(input ?? '');
  try {
    if (typeof Intl !== 'undefined' && typeof Intl.Segmenter === 'function') {
      const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
      return Array.from(segmenter.segment(s), (item) => item.segment);
    }
  } catch {
  }
  return Array.from(s);
};

// 计算字符串的“可视宽度”（用于 Markdown 表格源码对齐）。
// 规则：控制字符忽略；组合字符按 0；全宽字符按 2；其它按 1。
// 说明：这是为了提升中英文混排时 | 的对齐观感，而不是严格的排版引擎实现。
const getDisplayWidth = (input) => {
  const s = String(input ?? '');
  let width = 0;

  // 设计说明：
  // 1. 表格源码对齐看的是 Monaco 等宽字体中的“显示列宽”，不是 JS string.length。
  // 2. ✅ / ⏳ / 🚫 等 emoji 由单个或多个 code point 组成，通常显示为 2 列。
  // 3. 优先按 grapheme cluster 计宽，避免组合音标、变体选择符、ZWJ emoji 序列导致列宽漂移。
  for (const cluster of segmentDisplayGraphemes(s)) {
    const cps = Array.from(cluster).map((ch) => ch.codePointAt(0) || 0).filter(Boolean);
    if (!cps.length) continue;
    if (cps.every((cp) => cp < 32 || cp === 0x7f || isZeroWidthCodePoint(cp))) continue;
    if (cps.some((cp) => isEmojiLikeCodePoint(cp) || isFullWidthCodePoint(cp))) {
      width += 2;
      continue;
    }
    for (const cp of cps) {
      if (cp < 32 || cp === 0x7f || isZeroWidthCodePoint(cp)) continue;
      width += isFullWidthCodePoint(cp) ? 2 : 1;
    }
  }

  return width;
};

// 按列宽对齐单元格：用“可视宽度”补齐空格，而不是用 string.length。
// 这样在 Monaco 编辑器的等宽字体下，中英文混排时各行的竖线更容易对齐。
const padTableCell = (text, width, align) => {
  const raw = String(text ?? '').trim();
  const current = getDisplayWidth(raw);
  const w = Math.max(0, Number(width) || 0);
  if (current >= w) return raw;

  const delta = w - current;
  if (align === 'right') return `${' '.repeat(delta)}${raw}`;
  if (align === 'center') {
    const left = Math.floor(delta / 2);
    const right = delta - left;
    return `${' '.repeat(left)}${raw}${' '.repeat(right)}`;
  }
  return `${raw}${' '.repeat(delta)}`;
};

// 将表格的二维 cells 渲染为 Markdown 行，并根据列宽补空格。
// 目标：在源码编辑态也能保持每列 | 对齐，提升可读性。
const buildAlignedTableLines = (rows) => {
  const safeRows = Array.isArray(rows) ? rows : [];
  if (!safeRows.length) return [];

  const colCount = safeRows.reduce((m, r) => Math.max(m, Array.isArray(r) ? r.length : 0), 0) || 1;
  const sepRow = safeRows[1] || [];
  const aligns = Array.from({ length: colCount }, (_, i) => detectSeparatorAlign(sepRow[i]));

  const widths = Array.from({ length: colCount }, (_, i) => {
    const align = aligns[i];
    // 设计说明：
    // 1. Markdown 分隔行的 `:` 本身也占据源码列宽，不能只把宽度理解成 `-` 的数量。
    // 2. 左/右对齐至少需要 `:---` / `---:` 4 列，居中至少需要 `:---:` 5 列。
    // 3. 先把列宽提升到“分隔单元格总宽度”，后续正文补空格与分隔行渲染才能让 `|` 完全对齐。
    if (align === 'center') return 5;
    if (align === 'left' || align === 'right') return 4;
    return 3;
  });
  safeRows.forEach((row, rowIndex) => {
    if (!Array.isArray(row) || rowIndex === 1) return;
    for (let i = 0; i < colCount; i += 1) {
      const v = String(row[i] ?? '').trim();
      widths[i] = Math.max(widths[i], getDisplayWidth(v));
    }
  });

  return safeRows.map((row, rowIndex) => {
    const cells = Array.from({ length: colCount }, (_, i) => (Array.isArray(row) ? row[i] : ''));

    if (rowIndex === 1) {
      const sepCells = cells.map((c, i) => {
        const a = aligns[i];
        const colonCount = a === 'center' ? 2 : (a === 'left' || a === 'right' ? 1 : 0);
        const w = Math.max(3 + colonCount, widths[i]);
        const dashes = '-'.repeat(Math.max(3, w - colonCount));
        if (a === 'left') return `:${dashes}`;
        if (a === 'center') return `:${dashes}:`;
        if (a === 'right') return `${dashes}:`;
        return dashes;
      });
      return `| ${sepCells.join(' | ')} |`;
    }

    const padded = cells.map((c, i) => padTableCell(c, widths[i], aligns[i]));
    return `| ${padded.join(' | ')} |`;
  });
};

const formatMarkdownTableBlockLines = (lines) => {
  const rawLines = Array.isArray(lines) ? lines : [];
  if (rawLines.length < 2) return rawLines;
  const rows = rawLines.map((line) => normalizeTableRowCells(line));
  const colCount = rows.reduce((m, r) => Math.max(m, Array.isArray(r) ? r.length : 0), 0) || 1;
  const paddedRows = rows.map((row, rowIndex) => {
    const next = [...row];
    while (next.length < colCount) next.push('');
    if (rowIndex === 1) {
      return next.map((cell) => normalizeSeparatorCell(cell, null));
    }
    return next.map((cell) => String(cell ?? '').trim());
  });
  return buildAlignedTableLines(paddedRows);
};

const formatMarkdownTablesInText = (content, source = 'manual') => {
  const text = String(content ?? '');
  if (!text.includes('|')) return text;

  const hasTrailingNewline = /\n$/.test(text);
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  if (hasTrailingNewline && lines.length && lines[lines.length - 1] === '') {
    lines.pop();
  }

  const result = [];
  let formattedCount = 0;
  let inFence = false;
  let fenceChar = '';
  let fenceLen = 0;

  // 设计说明：
  // 1. 后端 Markdown 格式化器目前只保留表格块，不负责源码列对齐。
  // 2. 这里在前端二次处理真实 Markdown 表格，跳过 fenced code block，避免误改代码中的 `|`。
  // 3. 分隔行可能不在当前扫描块第一行，取“分隔行上一行”作为表头，兼容表格前后紧贴其它含 `|` 的行。
  for (let i = 0; i < lines.length;) {
    const line = lines[i] || '';
    const fence = /^\s*(`{3,}|~{3,})/.exec(line);
    if (fence) {
      const mark = fence[1];
      if (!inFence) {
        inFence = true;
        fenceChar = mark[0];
        fenceLen = mark.length;
      } else if (mark[0] === fenceChar && mark.length >= fenceLen) {
        inFence = false;
        fenceChar = '';
        fenceLen = 0;
      }
      result.push(line);
      i += 1;
      continue;
    }

    if (inFence || !isPotentialTableLine(line)) {
      result.push(line);
      i += 1;
      continue;
    }

    let end = i;
    while (end < lines.length && isPotentialTableLine(lines[end] || '')) {
      end += 1;
    }

    let separatorIndex = -1;
    for (let idx = i; idx < end; idx += 1) {
      if (isTableSeparatorLine(lines[idx] || '')) {
        separatorIndex = idx;
        break;
      }
    }

    if (separatorIndex <= i) {
      result.push(line);
      i += 1;
      continue;
    }

    const tableStart = separatorIndex - 1;
    for (let idx = i; idx < tableStart; idx += 1) {
      result.push(lines[idx]);
    }

    const tableLines = lines.slice(tableStart, end);
    const nextTableLines = formatMarkdownTableBlockLines(tableLines);
    result.push(...nextTableLines);
    formattedCount += 1;
    i = end;
  }

  const nextText = `${result.join('\n')}${hasTrailingNewline ? '\n' : ''}`;
  if (formattedCount > 0 && nextText !== text) {
    logger.info('[MarkdownTable] formatted markdown table blocks', {
      source,
      tableCount: formattedCount
    });
  }
  return nextText;
};

const getColumnIndexFromCursor = (line, column, colCount) => {
  const text = line || '';
  const positions = [];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i);
  }
  if (positions.length < 2) return 0;

  const cursor = Math.max(0, Math.min(text.length, (column || 1) - 1));

  if (cursor <= positions[0]) return 0;
  if (cursor > positions[positions.length - 1]) return Math.max(0, (colCount || positions.length - 1) - 1);

  for (let i = 0; i < positions.length - 1; i += 1) {
    if (cursor > positions[i] && cursor <= positions[i + 1]) {
      return Math.max(0, Math.min((colCount || positions.length - 1) - 1, i));
    }
  }

  return 0;
};

const detectTableContext = () => {
  if (!editor) return null;
  const model = editor.getModel();
  if (!model) return null;
  if (model.getLanguageId() !== 'markdown') return null;

  const pos = editor.getPosition();
  if (!pos) return null;

  const lineNumber = pos.lineNumber;
  const lineText = model.getLineContent(lineNumber);
  if (!isPotentialTableLine(lineText)) return null;

  const total = model.getLineCount();
  let start = lineNumber;
  while (start > 1 && isPotentialTableLine(model.getLineContent(start - 1))) {
    start -= 1;
  }

  let end = lineNumber;
  while (end < total && isPotentialTableLine(model.getLineContent(end + 1))) {
    end += 1;
  }

  let separatorLine = -1;
  for (let i = start; i <= end; i += 1) {
    if (isTableSeparatorLine(model.getLineContent(i))) {
      separatorLine = i;
      break;
    }
  }

  if (separatorLine === -1 || separatorLine === start) return null;

  const tableStart = separatorLine - 1;

  const rawLines = [];
  for (let i = tableStart; i <= end; i += 1) {
    rawLines.push(model.getLineContent(i));
  }

  const rows = rawLines.map((l) => normalizeTableRowCells(l));
  const colCount = rows.reduce((m, r) => Math.max(m, r.length), 0) || 1;
  const padded = rows.map((r) => {
    const next = [...r];
    while (next.length < colCount) next.push('');
    return next;
  });

  const activeRowIndex = lineNumber - tableStart;
  const cursorColIndex = getColumnIndexFromCursor(rawLines[Math.max(0, Math.min(rawLines.length - 1, activeRowIndex))], pos.column, colCount);

  return {
    startLine: tableStart,
    endLine: end,
    separatorLine,
    colCount,
    rows: padded,
    activeRowIndex,
    activeColIndex: cursorColIndex
  };
};

let tableAutoFormatBusy = false;

const isTableAutoFormatDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_md_table') || '').trim().toLowerCase();
    return v === '1' || v === 'true' || v === 'yes' || v === 'on';
  } catch {
    return false;
  }
};

const scheduleAutoFormatActiveTable = debounce(() => {
  if (!editor) return;
  if (tableAutoFormatBusy) return;

  const model = editor.getModel();
  const selection = editor.getSelection();
  const pos = editor.getPosition();
  if (!model || !pos || !selection || !selection.isEmpty()) return;
  if (model.getLanguageId() !== 'markdown') return;

  const ctx = detectTableContext();
  if (!ctx) return;

  const rawLines = [];
  for (let i = ctx.startLine; i <= ctx.endLine; i += 1) {
    rawLines.push(model.getLineContent(i));
  }

  const nextRows = ctx.rows.map((row, idx) => {
    if (idx !== 1) return row;
    return row.map((c) => normalizeSeparatorCell(c, null));
  });

  const nextLines = buildAlignedTableLines(nextRows);
  const currentText = rawLines.join('\n');
  const nextText = nextLines.join('\n');
  if (currentText === nextText) return;

  const activeRowIndex = Math.max(0, Math.min(rawLines.length - 1, pos.lineNumber - ctx.startLine));
  const activeColIndex = getColumnIndexFromCursor(rawLines[activeRowIndex] || '', pos.column, ctx.colCount);

  const currentCellColumn = getCursorColumnForTableCell(rawLines[activeRowIndex] || '', activeColIndex, 0);
  const offset = Math.max(0, (pos.column || 1) - currentCellColumn);

  if (isTableAutoFormatDebug()) {
    logger.debug('[MarkdownTable] auto-format', {
      startLine: ctx.startLine,
      endLine: ctx.endLine,
      activeRowIndex,
      activeColIndex,
      offset
    });
  }

  tableAutoFormatBusy = true;
  try {
    const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine));
    editor.executeEdits('md-table-auto-format', [
      {
        range,
        text: nextText,
        forceMoveMarkers: true
      }
    ]);

    const targetText = nextLines[activeRowIndex] || '';
    const targetCol = getCursorColumnForTableCell(targetText, activeColIndex, offset);
    editor.setPosition(new monaco.Position(ctx.startLine + activeRowIndex, targetCol));
  } finally {
    nextTick(() => {
      tableAutoFormatBusy = false;
      updateTableToolbar();
    });
  }
}, 220);

const updateTableToolbar = () => {
  if (!editor) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const model = editor.getModel();
  if (!model || model.getLanguageId() !== 'markdown') {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const ctx = detectTableContext();
  if (!ctx) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const wrapperEl = editorWrapperRef.value;
  const editorEl = editorContainer.value;
  if (!wrapperEl || !editorEl) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const wrapperRect = wrapperEl.getBoundingClientRect();
  const editorRect = editorEl.getBoundingClientRect();

  const leftBase = editorRect.left - wrapperRect.left;
  const topBase = editorRect.top - wrapperRect.top;

  const cursorPos = editor.getPosition();
  const anchorPos = cursorPos
    ? new monaco.Position(cursorPos.lineNumber, cursorPos.column)
    : new monaco.Position(ctx.startLine, 1);

  const visiblePos = editor.getScrolledVisiblePosition(anchorPos);
  if (!visiblePos) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const headerEl = editorHeaderRef.value;
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
  const minTop = headerHeight + 8;

  const toolbarH = 36;
  const toolbarW = 320;
  const gap = 8;

  const preferTop = topBase + visiblePos.top - toolbarH - gap;
  const fallbackTop = topBase + visiblePos.top + visiblePos.height + gap;
  const top = preferTop >= minTop ? preferTop : fallbackTop;
  const clampedTop = Math.min(
    Math.max(minTop, top),
    Math.max(minTop, wrapperRect.height - toolbarH - 8)
  );

  const layout = editor.getLayoutInfo ? editor.getLayoutInfo() : null;
  const contentLeft = layout && Number.isFinite(layout.contentLeft) ? layout.contentLeft : 0;
  const minLeft = Math.max(8, leftBase + contentLeft);

  const left = leftBase + visiblePos.left;
  const clampedLeft = Math.min(
    Math.max(minLeft, left),
    Math.max(minLeft, wrapperRect.width - toolbarW - 8)
  );

  tableToolbarStyle.value = {
    left: `${clampedLeft}px`,
    top: `${clampedTop}px`
  };

  if (isTableAutoFormatDebug()) {
    logger.debug('[MarkdownTable] toolbar', {
      left: clampedLeft,
      top: clampedTop,
      contentLeft,
      anchor: { lineNumber: anchorPos.lineNumber, column: anchorPos.column },
    });
  }

  activeTableContext.value = ctx;
  tableToolbarVisible.value = true;
};

const getPipePositions = (line) => {
  const text = String(line || '');
  const positions = [];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i);
  }
  return positions;
};

const getCursorColumnForTableCell = (line, colIndex, offset = 0) => {
  const text = String(line || '');
  const pipes = getPipePositions(text);
  if (pipes.length < 2) return 1;

  const col = Math.max(0, Math.min(pipes.length - 2, Number(colIndex) || 0));
  const left = pipes[col];
  const right = pipes[col + 1];
  const cellStart = Math.min(text.length, left + 2);
  const cellEnd = Math.max(cellStart, right - 1);
  const off = Math.max(0, Math.min(cellEnd - cellStart, Number(offset) || 0));

  return Math.max(1, Math.min(text.length + 1, cellStart + off + 1));
};

const applyTableEdits = (nextRows, nextActiveRowIndex, nextActiveColIndex) => {
  if (!editor) return;
  const model = editor.getModel();
  const ctx = activeTableContext.value;
  if (!model || !ctx) return;

  const normalizedRows = nextRows.map((row, idx) => {
    if (idx !== 1) return row;
    return row.map((c) => normalizeSeparatorCell(c, null));
  });

  const textLines = buildAlignedTableLines(normalizedRows);

  const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine));
  editor.executeEdits('md-table-toolbar', [
    {
      range,
      text: textLines.join('\n'),
      forceMoveMarkers: true
    }
  ]);

  const row = Math.max(2, Math.min(textLines.length - 1, nextActiveRowIndex));
  const col = Math.max(0, Math.min((normalizedRows[0]?.length || 1) - 1, nextActiveColIndex));
  const targetLine = ctx.startLine + row;
  const targetText = textLines[row] || '';
  const targetCol = getCursorColumnForTableCell(targetText, col, 0);

  editor.setPosition(new monaco.Position(targetLine, targetCol));
  editor.focus();

  nextTick(() => {
    updateTableToolbar();
  });
};

const addTableRowBelow = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const insertAt = ctx.activeRowIndex <= 1 ? 2 : Math.min(ctx.rows.length, ctx.activeRowIndex + 1);
  const empty = Array.from({ length: ctx.colCount }, () => '');

  const nextRows = [...ctx.rows];
  nextRows.splice(insertAt, 0, empty);

  applyTableEdits(nextRows, insertAt, ctx.activeColIndex);
};

const deleteTableRow = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  if (ctx.activeRowIndex <= 1) {
    message.warning('无法删除表头或分隔行');
    return;
  }

  const nextRows = [...ctx.rows];
  if (nextRows.length <= 2) return;

  nextRows.splice(ctx.activeRowIndex, 1);
  const nextRow = Math.min(nextRows.length - 1, ctx.activeRowIndex);

  applyTableEdits(nextRows, nextRow, ctx.activeColIndex);
};

const addTableColumnRight = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const insertAt = Math.min(ctx.colCount, ctx.activeColIndex + 1);
  const nextRows = ctx.rows.map((row, rowIndex) => {
    const next = [...row];
    if (rowIndex === 1) {
      next.splice(insertAt, 0, '---');
    } else {
      next.splice(insertAt, 0, '');
    }
    return next;
  });

  const nextCol = insertAt;
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol);
};

const deleteTableColumn = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  if (ctx.colCount <= 1) {
    message.warning('至少保留 1 列');
    return;
  }

  const removeAt = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex));
  const nextRows = ctx.rows.map((row) => {
    const next = [...row];
    next.splice(removeAt, 1);
    return next;
  });

  const nextCol = Math.max(0, Math.min(ctx.colCount - 2, removeAt));
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol);
};

const setTableColumnAlign = (align) => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const col = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex));
  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex !== 1) return row;
    const next = [...row];
    next[col] = normalizeSeparatorCell(next[col], align);
    return next;
  });

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex);
};

const formatTable = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex === 1) {
      return row.map((c) => normalizeSeparatorCell(c, null));
    }
    return row.map((c) => (c ?? '').trim());
  });

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex);
};

const moveTableCell = (dx, dy) => {
  if (!editor) return;

  const model = editor.getModel();
  const pos = editor.getPosition();
  const selection = editor.getSelection();
  if (!model || !pos || !selection) return;
  if (model.getLanguageId() !== 'markdown') return;
  if (!selection.isEmpty()) return;

  const ctx = detectTableContext();
  if (!ctx) return;

  const maxRow = Math.max(0, ctx.rows.length - 1);
  const nextCol = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex + (Number(dx) || 0)));

  let nextRow = Math.max(0, Math.min(maxRow, ctx.activeRowIndex + (Number(dy) || 0)));
  if (nextRow === 1 && dy < 0) nextRow = 0;
  if (nextRow === 1 && dy > 0) nextRow = Math.min(maxRow, 2);

  const currentLine = model.getLineContent(ctx.startLine + ctx.activeRowIndex);
  const currentCellCol = getCursorColumnForTableCell(currentLine, ctx.activeColIndex, 0);
  const offset = Math.max(0, (pos.column || 1) - currentCellCol);

  const targetLineNumber = ctx.startLine + nextRow;
  const targetLine = model.getLineContent(targetLineNumber);
  const targetCol = getCursorColumnForTableCell(targetLine, nextCol, offset);

  editor.setPosition(new monaco.Position(targetLineNumber, targetCol));
  editor.focus();
  updateTableToolbar();
};

const insertTable = () => {
  insertTableWithSize(2, 2)
};

const MARKDOWN_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'];

const sanitizeResourceFileSegment = (raw, fallback = 'image') => {
  const normalized = String(raw || '')
    .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '');
  return normalized || fallback;
};

const joinResourceRepoPath = (...parts) => parts
  .map((part, index) => {
    const normalized = String(part || '').replace(/\\/g, '/');
    return index === 0 ? normalized.replace(/\/+$/g, '') : normalized.replace(/^\/+|\/+$/g, '');
  })
  .filter(Boolean)
  .join('/');

const normalizePickedLocalPath = (path) => {
  const normalized = String(path || '').trim().replace(/\\/g, '/');
  // 设计说明：Windows 下部分 WebView / file URL 场景会把 `C:/...` 传成 `/C:/...`。
  // 后端复制文件前先在前端规整一次，避免资源入库阶段误判源文件不存在。
  return normalized.replace(/^\/([a-zA-Z]:\/)/, '$1');
};

const getFileNameFromPath = (path) => {
  const normalized = String(path || '').replace(/\\/g, '/');
  return normalized.split('/').filter(Boolean).pop() || '';
};

const getMarkdownImageExtension = (path) => {
  const fileName = getFileNameFromPath(path);
  const ext = String(fileName.split('.').pop() || '').trim().toLowerCase();
  return MARKDOWN_IMAGE_EXTENSIONS.includes(ext) ? ext : 'png';
};

const buildMarkdownImageResourceTarget = (sourcePath) => {
  const now = new Date();
  const year = String(now.getFullYear());
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  const stamp = [
    year,
    month,
    day,
    '_',
    String(now.getHours()).padStart(2, '0'),
    String(now.getMinutes()).padStart(2, '0'),
    String(now.getSeconds()).padStart(2, '0'),
    '_',
    String(now.getMilliseconds()).padStart(3, '0')
  ].join('');
  const sourceName = getFileNameFromPath(sourcePath);
  const ext = getMarkdownImageExtension(sourceName);
  const stem = sanitizeResourceFileSegment(sourceName.replace(/\.[^.]+$/, ''), 'image');
  const fileName = `${stem}-${stamp}.${ext}`;
  const relativeDir = joinResourceRepoPath('images', year, month, day);
  const relativePath = joinResourceRepoPath(relativeDir, fileName);

  return {
    fileName,
    relativeDir,
    relativePath,
    alt: stem
  };
};

const importMarkdownImageToResourceRepo = async (sourcePath) => {
  const normalizedSourcePath = normalizePickedLocalPath(sourcePath);
  if (!normalizedSourcePath) throw new Error('未选择图片文件');

  const root = String(await invoke('get_image_bed_root') || '').trim();
  if (!root) throw new Error('资源库根目录不可用');

  const target = buildMarkdownImageResourceTarget(normalizedSourcePath);
  const destDir = joinResourceRepoPath(root, target.relativeDir);
  const absolutePath = joinResourceRepoPath(root, target.relativePath);
  const ownerFilePath = String(selectedFile.value?.path || fileTreeStore.activeTab || '').trim();

  logger.info('[MarkdownImage] import selected image to resource repo start', {
    sourcePath: normalizedSourcePath,
    destDir,
    fileName: target.fileName,
    ownerFilePath
  });

  await invoke('copy_image_file', {
    srcPath: normalizedSourcePath,
    destDir,
    fileName: target.fileName
  });

  await invoke('add_resource', {
    resource: {
      resource_type: 'image',
      file_name: target.fileName,
      relative_path: target.relativePath,
      absolute_path: absolutePath,
      owner_file_path: ownerFilePath,
      created_at: '',
      extra: JSON.stringify({
        source: 'markdown-editor-insert-image',
        original_path: normalizedSourcePath
      })
    }
  });

  const url = `http://localhost:8888/markdown/api/bed/${encodeURIComponent(target.relativePath)}`;
  logger.info('[MarkdownImage] image imported and resource registered', {
    relativePath: target.relativePath,
    absolutePath,
    url
  });

  return {
    url,
    alt: target.alt,
    relativePath: target.relativePath,
    absolutePath
  };
};

const insertMarkdownTextAtCursor = (text, source = 'markdown-toolbar') => {
  const targetEditor = getActiveEditor();
  if (!ensureMarkdownEditor(targetEditor)) return false;

  const selection = targetEditor.getSelection?.();
  const position = targetEditor.getPosition?.() || new monaco.Position(1, 1);
  const range = selection || new monaco.Range(
    position.lineNumber,
    position.column,
    position.lineNumber,
    position.column
  );

  targetEditor.executeEdits(source, [
    {
      range,
      text,
      forceMoveMarkers: true
    }
  ]);

  targetEditor.focus();
  return true;
};

const insertImage = async () => {
  try {
    const targetEditor = getActiveEditor();
    if (!ensureMarkdownEditor(targetEditor)) {
      logger.warn('[MarkdownImage] insert aborted because markdown editor is not ready');
      return;
    }

    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: 'Image',
          extensions: MARKDOWN_IMAGE_EXTENSIONS
        }
      ]
    });

    const sourcePath = Array.isArray(selected) ? selected[0] : selected;
    if (!sourcePath) {
      logger.debug('[MarkdownImage] insert cancelled by user');
      return;
    }

    const imported = await importMarkdownImageToResourceRepo(sourcePath);
    const markdown = `![${imported.alt}](${imported.url})\n`;
    const inserted = insertMarkdownTextAtCursor(markdown, 'markdown-image-insert');
    if (!inserted) throw new Error('插入失败：Markdown 编辑器未就绪');
    message.success('图片已导入资源库并插入');
    logger.info('[MarkdownImage] markdown inserted', {
      relativePath: imported.relativePath,
      markdownLength: markdown.length
    });
  } catch (error) {
    logger.error('[MarkdownImage] insert image failed', error);
    message.error(`插入图片失败：${String(error?.message || error || '')}`);
  }
};

const insertCodeBlock = (lang) => {
  const v = String(lang || '').trim();
  const prefix = v ? `\`\`\`${v}\n` : '```\n';
  wrapSelection(prefix, '\n```\n', '代码');
};

const insertPythonCellBlock = () => {
  if (editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value) {
    notebookViewRef.value?.insertCell?.('python')
    return
  }

  if (canUseNotebookSurface.value) {
    logger.info('[Editor] route python cell insertion to explorer sidecar storage', {
      filePath: String(selectedFile.value?.path || ''),
      previousSurfaceMode: editorSurfaceMode.value,
    })
    void handlePreviewModeSelect('explorer').then(() => {
      nextTick(() => {
        notebookViewRef.value?.insertCell?.('python')
      })
    })
    return
  }

  const snippet = `${buildPythonCellBlock({
    code: ['# Python Cell', 'print("Hello from Guaxi Python Cell")'].join('\n'),
    meta: {
      id: createPythonCellStableId(),
    },
  })}\n`
  const previewOnly = previewMode.value === 'toggle' && mode.value === 'preview'
  insertBlockSnippet(snippet)
  if (previewOnly) {
    void setMainMode('edit')
  }
}

const handleCodeBlockSelect = (key) => {
  const k = String(key || '').trim();
  if (k === 'pycell') {
    insertPythonCellBlock();
    return;
  }
  const lang = k === 'plain' ? '' : k;
  insertCodeBlock(lang);
};

const insertCodeBlockFromToolbar = () => {
  insertCodeBlock('');
};

const insertHorizontalRule = () => {
  insertBlockSnippet('---\n');
};

const undo = () => {
  if (!editor) return;
  editor.focus();
  editor.trigger('toolbar', 'undo', null);
};

const redo = () => {
  if (!editor) return;
  editor.focus();
  editor.trigger('toolbar', 'redo', null);
};

const getActiveEditor = () => {
  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeRegion === 'right') {
    return secondaryEditor;
  }
  return editor;
};

const activateRegion = (region) => {
  if (fileTreeStore.layoutMode === 'single') return;

  const desired = region === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  if (desired) {
    fileTreeStore.setActiveTab(desired, region);
    return;
  }

  fileTreeStore.setActiveRegion(region);
};

// 更新编辑器内容
const toggleEditorSurfaceMode = () => {
  if (!canUseNotebookSurface.value) {
    editorSurfaceMode.value = 'source'
    return
  }
  editorSurfaceMode.value = editorSurfaceMode.value === 'notebook' ? 'source' : 'notebook'
  if (editorSurfaceMode.value === 'notebook') {
    disposePythonCellManagers()
    if (previewMode.value === 'toggle' && mode.value === 'preview') {
      void setMainMode('edit')
    }
  } else {
    nextTick(() => {
      ensurePrimarySourceEditorReady('toggle-source-surface')
    })
  }
}

const openSourceEditorSurface = () => {
  editorSurfaceMode.value = 'source'
  nextTick(() => {
    ensurePrimarySourceEditorReady('open-source-surface')
  })
}

const applyNotebookMarkdown = (nextMarkdown) => {
  const nextText = String(nextMarkdown || '')
  rawContent.value = nextText

  const targetEditor = editor || secondaryEditor
  const model = targetEditor?.getModel?.()
  if (model && model.getValue() !== nextText) {
    replaceModelContentPreserveUndo(model, nextText, {
      logPrefix: '[NotebookAutosave] sync source editor from notebook surface',
      uri: String(model.uri?.toString?.() || ''),
      owner: targetEditor === secondaryEditor
        ? SECONDARY_EDITOR_VSCODE_MODEL_OWNER
        : PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
      reason: 'apply-notebook-markdown',
    })
  }

  const targetPath = String(selectedFile.value?.path || '').trim()
  if (targetPath) {
    fileTreeStore.updateTabContent(targetPath, nextText)
  }

  if (canPreview.value && previewMode.value !== 'realtime') {
    renderDebounced(nextText)
  }

  scheduleNotebookDiskPersist('apply-notebook-markdown', nextText)
}

const persistNotebookToDisk = async (reason = 'unknown', nextText = null) => {
  const targetPath = String(selectedFile.value?.path || '').trim()
  if (!targetPath) {
    logger.warn('[NotebookAutosave] skip persist because target path is empty', { reason })
    return false
  }
  if (activeTab.value?.unsupported) {
    logger.warn('[NotebookAutosave] skip persist because file is unsupported', {
      reason,
      targetPath,
    })
    return false
  }
  if (notebookDiskPersistInFlight) {
    logger.debug('[NotebookAutosave] skip overlapping persist request', {
      reason,
      targetPath,
    })
    return false
  }

  const content = String(nextText ?? rawContent.value ?? '')
  const tab = fileTreeStore.openTabs.find((item) => item?.path === targetPath)
  const savedContent = String(tab?.savedContent ?? '')
  if (savedContent === content) {
    logger.debug('[NotebookAutosave] skip persist because file content is already saved', {
      reason,
      targetPath,
      length: content.length,
    })
    return true
  }

  notebookDiskPersistInFlight = true
  try {
    await fileTreeStore.updateFileContent(targetPath, content)
    editorStore.setLastAutoSaveAt(Date.now())
    logger.info('[NotebookAutosave] persisted notebook content to file', {
      reason,
      targetPath,
      length: content.length,
    })
    return true
  } catch (error) {
    logger.error('[NotebookAutosave] failed to persist notebook content to file', {
      reason,
      targetPath,
      error,
    })
    return false
  } finally {
    notebookDiskPersistInFlight = false
  }
}

const scheduleNotebookDiskPersist = (reason = 'unknown', nextText = null, delay = 420) => {
  if (notebookDiskPersistTimer) {
    window.clearTimeout(notebookDiskPersistTimer)
    notebookDiskPersistTimer = null
  }
  const scheduledContent = String(nextText ?? rawContent.value ?? '')
  notebookDiskPersistTimer = window.setTimeout(() => {
    notebookDiskPersistTimer = null
    void persistNotebookToDisk(reason, scheduledContent)
  }, delay)
  logger.debug('[NotebookAutosave] scheduled notebook disk persist', {
    reason,
    delay,
    length: scheduledContent.length,
  })
}

const flushNotebookDiskPersist = (reason = 'unknown', nextText = null) => {
  if (notebookDiskPersistTimer) {
    window.clearTimeout(notebookDiskPersistTimer)
    notebookDiskPersistTimer = null
  }
  return persistNotebookToDisk(reason, nextText)
}

const shouldRecreatePrimaryEditor = () => {
  if (!editorContainer.value) return false
  if (!editor) return true

  try {
    const currentContainer = editor.getContainerDomNode?.()
    return !(currentContainer instanceof HTMLElement) || !currentContainer.isConnected || currentContainer !== editorContainer.value
  } catch (error) {
    logger.warn('[Editor] detect primary editor container failed', { error })
    return true
  }
}

// Explorer/source 之间切换时，源码 Monaco 容器会被卸载并重新挂载。
// 这里在切回 source surface 时主动校验容器，必要时重建编辑器，避免主编辑区域出现空白。
const ensurePrimarySourceEditorReady = (reason = 'manual-ensure-source') => {
  if (editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value) return
  if (!editorContainer.value) return

  if (!shouldRecreatePrimaryEditor()) {
    syncSourceEditorContent(reason)
    return
  }

  logger.info('[Editor] recreate primary source editor after surface switch', {
    reason,
    filePath: String(selectedFile.value?.path || ''),
    hadEditorInstance: !!editor,
  })

  try {
    if (editorEffectsLeft) {
      editorEffectsLeft.destroy()
      editorEffectsLeft = null
    }
    editor?.dispose?.()
    editor = null
    editorStore.setEditorInstance(null)
  } catch (error) {
    logger.warn('[Editor] dispose primary editor before recreate failed', { reason, error })
  }

  initEditor()
  syncSourceEditorContent(`${reason}:after-recreate`)
}

const syncSourceEditorContent = (reason = 'manual-sync') => {
  if (editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value) return

  const nextText = String(rawContent.value ?? selectedFile.value?.content ?? '')
  const targetEditor = getActiveEditor() || editor || secondaryEditor
  if (!targetEditor) return

  try {
    const model = targetEditor.getModel?.()
    sourceEditorProgrammaticSyncDepth += 1
    if (model && model.getValue() !== nextText) {
      replaceModelContentPreserveUndo(model, nextText, {
        logPrefix: '[Editor] sync source editor content',
        uri: String(model.uri?.toString?.() || ''),
        owner: targetEditor === secondaryEditor
          ? SECONDARY_EDITOR_VSCODE_MODEL_OWNER
          : PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
        reason,
      })
    } else if (!model && typeof targetEditor.setValue === 'function') {
      targetEditor.setValue(nextText)
    }
    targetEditor.layout?.()
    scheduleSourceSyntaxHighlight(targetEditor, reason)
    logger.debug('[Editor] syncSourceEditorContent', {
      reason,
      length: nextText.length,
      hasModel: !!model,
      editorSurfaceMode: editorSurfaceMode.value,
      suppressedDepth: sourceEditorProgrammaticSyncDepth,
    })
  } catch (error) {
    logger.warn('[Editor] syncSourceEditorContent failed', { reason, error })
  } finally {
    sourceEditorProgrammaticSyncDepth = Math.max(0, sourceEditorProgrammaticSyncDepth - 1)
  }
}

const getEditorBoundTabPath = (targetEditor) => {
  if (targetEditor === secondaryEditor) {
    return String(fileTreeStore.activeTabRight || '').trim()
  }
  if (fileTreeStore.layoutMode !== 'single') {
    return String(fileTreeStore.activeTabLeft || fileTreeStore.activeTab || '').trim()
  }
  return String(fileTreeStore.activeTab || fileTreeStore.activeTabLeft || '').trim()
}

const isBindableEditorFilePath = (filePath) => {
  const normalized = String(filePath || '').trim()
  if (!normalized) return false
  if (normalized.startsWith('file://')) return true
  if (/^[a-zA-Z]:[\\/]/.test(normalized)) return true
  if (normalized.startsWith('/')) return true
  return normalized.startsWith('\\')
}

const shouldDisposeTransientEditorModel = (targetModel) => {
  const scheme = String(targetModel?.uri?.scheme || '').trim().toLowerCase()
  return scheme === 'inmemory' || scheme === 'untitled'
}

const resolveEditorModelUri = (filePath) => {
  const normalized = String(filePath || '').trim()
  if (!isBindableEditorFilePath(normalized)) return null
  try {
    return normalized.startsWith('file://')
      ? monaco.Uri.parse(normalized)
      : monaco.Uri.file(normalized)
  } catch (error) {
    logger.warn('[Editor] resolveEditorModelUri failed', {
      filePath: normalized,
      error,
    })
    return null
  }
}

const ensureEditorModelBinding = (targetEditor, options = {}) => {
  if (!targetEditor) return null

  const filePath = String(options.filePath || '').trim()
  const modelOwner = String(options.owner || '').trim()
  const languageId = String(
    options.languageId
      || targetEditor.getModel?.()?.getLanguageId?.()
      || 'plaintext'
  ).trim() || 'plaintext'
  const desiredContent = String(options.content ?? '')
  const targetUri = resolveEditorModelUri(filePath)
  const currentModel = targetEditor.getModel?.() || null

  if (!targetUri) {
    if (modelOwner && currentModel?.uri) {
      releaseVsCodeTextModel(currentModel.uri, modelOwner, 'editor-model-unbound')
    }
    if (currentModel && currentModel.getLanguageId?.() !== languageId) {
      monaco.editor.setModelLanguage(currentModel, languageId)
    }
    return currentModel
  }

  let nextModel = monaco.editor.getModel(targetUri)
  if (!nextModel) {
    nextModel = monaco.editor.createModel(desiredContent, languageId, targetUri)
    logger.info('[Editor] created bound Monaco model', {
      reason: String(options.reason || 'manual'),
      filePath,
      languageId,
      uri: targetUri.toString(),
    })
  } else {
    if (nextModel.getLanguageId?.() !== languageId) {
      monaco.editor.setModelLanguage(nextModel, languageId)
    }
    if (typeof options.content !== 'undefined' && nextModel.getValue() !== desiredContent) {
      replaceModelContentPreserveUndo(nextModel, desiredContent, {
        logPrefix: '[Editor] sync bound Monaco model',
        uri: String(nextModel.uri?.toString?.() || ''),
        owner: modelOwner || 'editor-model-binding',
        reason: String(options.reason || 'manual'),
      })
    }
  }

  if (currentModel !== nextModel) {
    if (modelOwner && currentModel?.uri) {
      releaseVsCodeTextModel(currentModel.uri, modelOwner, 'editor-model-rebound')
    }
    targetEditor.setModel(nextModel)
    logger.info('[Editor] rebound editor model to file URI', {
      reason: String(options.reason || 'manual'),
      filePath,
      languageId,
      uri: nextModel.uri?.toString?.() || '',
    })
    if (currentModel && shouldDisposeTransientEditorModel(currentModel)) {
      currentModel.dispose()
    }
  }

  activateEditorLanguageFeatures(targetEditor, String(options.reason || 'editor-model-binding'))

  if (modelOwner) {
    void retainVsCodeTextModel({
      resource: nextModel.uri,
      content: desiredContent,
      languageId,
      owner: modelOwner,
      reason: String(options.reason || 'manual'),
    })
  }

  return nextModel
}

const updateEditorContent = (content) => {
  const nextContent = content ?? '';
  rawContent.value = nextContent;

  if (!editor) {
    initEditor();
  }

  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeRegion === 'right') {
    ensureSecondaryEditor();
  }

  const targetEditor = getActiveEditor();
  if (!targetEditor) return;

  targetEditor.updateOptions({
    readOnly: !!activeTab.value?.unsupported,
  });

  const boundPath = getEditorBoundTabPath(targetEditor)
  const boundLanguage = resolveMonacoLanguageByPath(boundPath || selectedFile.value?.path || '')
  const model = ensureEditorModelBinding(targetEditor, {
    filePath: boundPath,
    languageId: boundLanguage,
    content: nextContent,
    owner: targetEditor === secondaryEditor ? SECONDARY_EDITOR_VSCODE_MODEL_OWNER : PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
    reason: 'update-editor-content',
  }) || targetEditor.getModel();
  const currentValue = model ? model.getValue() : targetEditor.getValue();
  if (currentValue === nextContent) {
    scheduleSourceSyntaxHighlight(targetEditor, 'update-editor-content:unchanged')
    return;
  }

  sourceEditorProgrammaticSyncDepth += 1
  try {
    if (model) {
      replaceModelContentPreserveUndo(model, nextContent, {
        logPrefix: '[Editor] update editor content applied programmatic sync',
        uri: String(model.uri?.toString?.() || ''),
        owner: targetEditor === secondaryEditor
          ? SECONDARY_EDITOR_VSCODE_MODEL_OWNER
          : PRIMARY_EDITOR_VSCODE_MODEL_OWNER,
        reason: 'update-editor-content',
      });
    } else {
      targetEditor.setValue(nextContent);
    }
    logger.debug('[Editor] updateEditorContent applied programmatic sync', {
      path: getEditorBoundTabPath(targetEditor),
      length: String(nextContent).length,
      suppressedDepth: sourceEditorProgrammaticSyncDepth,
    })
    scheduleSourceSyntaxHighlight(targetEditor, 'update-editor-content')
  } finally {
    sourceEditorProgrammaticSyncDepth = Math.max(0, sourceEditorProgrammaticSyncDepth - 1)
  }
};

const AUTO_SAVE_FALLBACK_INTERVAL_MS = 5 * 60 * 1000
let autoSaveTimerId = null
const autoSaving = ref(false)

const autoSaveDirtyTabs = async () => {
  if (autoSaving.value) return
  if (editorStore.autoSaveEnabled === false) return
  if (!fileTreeStore.openTabs.length) return

  autoSaving.value = true
  try {
    flushPythonCellDrafts()
    const activePath = fileTreeStore.activeTab
    if (activePath) {
      const targetEditor = getActiveEditor()
      const activeContent =
        previewMode.value === 'realtime' && realtimeVditor
          ? realtimeVditor.getValue()
          : targetEditor
            ? targetEditor.getValue()
            : rawContent.value
      fileTreeStore.updateTabContent(activePath, activeContent)
    }

    const dirtyTabs = fileTreeStore.openTabs.filter((tab) => tab?.dirty)
    if (!dirtyTabs.length) return

    for (const tab of dirtyTabs) {
      if (!tab?.path) continue
      if (tab.unsupported) continue
      await fileTreeStore.updateFileContent(tab.path, tab.content)
    }

    await updateGitStatus();
    editorStore.setLastAutoSaveAt(Date.now())
  } catch (e) {
    logger.error('自动保存失败:', e)
  } finally {
    autoSaving.value = false
  }
}

// 保存内容
const saveContent = async () => {
  if (!selectedFile.value) return;

  if (activeTab.value?.unsupported) {
    message.warning('不支持编辑/保存该文件类型');
    return;
  }

  flushPythonCellDrafts()
  const targetEditor = getActiveEditor();
  const content =
    previewMode.value === 'realtime' && realtimeVditor
      ? realtimeVditor.getValue()
      : targetEditor
        ? targetEditor.getValue()
        : rawContent.value;

  await fileTreeStore.updateFileContent(selectedFile.value.path, content);
  await updateGitStatus();
  message.info('修改已保存');

  if (rawContent.value !== content) {
    rawContent.value = content;
  }

  if (previewMode.value === 'split' && canPreview.value) {
    if (lastPreviewRenderedSource.value !== content) {
      renderDebounced(content);
    }
  }
};

// 快捷键处理
const handleKeyDown = (e) => {
  const explorerNotebook = editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value
  const isConsoleShortcut =
    (e.ctrlKey || e.metaKey) &&
    !e.shiftKey &&
    String(e.code || '') === 'Backquote'

  if (isConsoleShortcut) {
    e.preventDefault()
    void togglePythonConsolePanel('keyboard-toggle-console')
    return
  }

  const isFindShortcut =
    !e.altKey &&
    !e.shiftKey &&
    (e.ctrlKey || e.metaKey) &&
    String(e.key || '').toLowerCase() === 'f'

  if (isFindShortcut) {
    const eventTarget = e.target
    const terminalSearchScope = pythonConsoleVisible.value
      && eventTarget?.closest?.('.guaxi-terminal-panel')
    if (terminalSearchScope) {
      e.preventDefault()
      e.stopPropagation()
      terminalPanelRef.value?.openSearch?.()
      return
    }
    const previewOnly = mode.value === 'preview' && showPreview.value && !showEditor.value && previewMode.value !== 'realtime'
    if (previewOnly) {
      e.preventDefault()
      e.stopPropagation()
      openPreviewSearch()
      return
    }
    if (explorerNotebook) {
      e.preventDefault()
      e.stopPropagation()
      notebookViewRef.value?.openSearch?.()
      return
    }
  }

  const lowerKey = String(e.key || '').toLowerCase()
  const isNotebookUndoShortcut =
    explorerNotebook &&
    !e.altKey &&
    (e.ctrlKey || e.metaKey) &&
    !e.shiftKey &&
    lowerKey === 'z'
  const isNotebookRedoShortcut =
    explorerNotebook &&
    !e.altKey &&
    (e.ctrlKey || e.metaKey) &&
    (
      lowerKey === 'y' ||
      (e.shiftKey && lowerKey === 'z')
    )

  if (isNotebookUndoShortcut) {
    e.preventDefault()
    e.stopPropagation()
    notebookViewRef.value?.undo?.()
    return
  }

  if (isNotebookRedoShortcut) {
    e.preventDefault()
    e.stopPropagation()
    notebookViewRef.value?.redo?.()
    return
  }

  if (e.key === 'Escape' && previewSearchVisible.value) {
    e.preventDefault()
    e.stopPropagation()
    closePreviewSearch()
    return
  }

  if (e.key === 'Escape' && notebookViewRef.value?.isSearchOpen?.()) {
    e.preventDefault()
    e.stopPropagation()
    notebookViewRef.value?.closeSearch?.()
    return
  }

  if (e.ctrlKey && e.key === 's') {
    e.preventDefault();
    saveContent();
    return;
  }

  if (e.ctrlKey && !e.altKey && !e.metaKey) {
    if (e.key === '1') {
      e.preventDefault();
      handlePreviewModeSelect('edit');
      return;
    }

    if (e.key === '2') {
      e.preventDefault();
      handlePreviewModeSelect('preview');
      return;
    }

    if (e.key === '3') {
      e.preventDefault();
      handlePreviewModeSelect('split');
      return;
    }

    if (e.key === '4') {
      e.preventDefault();
      handlePreviewModeSelect('realtime');
      return;
    }

    if (e.key === '5') {
      e.preventDefault();
      handlePreviewModeSelect('explorer');
      return;
    }
  }

  if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
    const target = e.target;
    const tagName = (target?.tagName || '').toLowerCase();
    const isEditable =
      tagName === 'input' ||
      tagName === 'textarea' ||
      target?.isContentEditable;

    if (isEditable) return;

    const k = String(e.key || '');
    const c = String(e.code || '');
    const isAltLeft = k === 'ArrowLeft' || k === 'Left' || k === 'BrowserBack' || c === 'ArrowLeft';
    const isAltRight = k === 'ArrowRight' || k === 'Right' || k === 'BrowserForward' || c === 'ArrowRight';

    const previewOnly = mode.value === 'preview' && showPreview.value && !showEditor.value;

    if (isAltLeft) {
      e.preventDefault();
      e.stopPropagation();
      if (previewOnly) {
        navigateToPreviousPreviewLocation();
      } else {
        navigateToPreviousEditLocation();
      }
      return;
    }

    if (isAltRight) {
      e.preventDefault();
      e.stopPropagation();
      if (previewOnly) {
        navigateToNextPreviewLocation();
      } else {
        navigateToNextEditLocation();
      }
      return;
    }
  }
};

const getRealtimeScrollElement = () => {
  const root = document.getElementById(realtimeVditorId);
  if (!root) return null;
  return root.querySelector('.vditor-content') || root;
};

const captureScrollPosition = () => {
  if (previewMode.value === 'realtime') {
    const el = getRealtimeScrollElement();
    if (el) {
      const height = el.scrollHeight;
      scrollRatio.value = height ? el.scrollTop / height : 0;
      return;
    }
  }

  if (mode.value === 'edit' && editor) {
    const top = editor.getScrollTop();
    const height = editor.getScrollHeight();
    scrollRatio.value = height ? top / height : 0;
  } else if (mode.value === 'preview' && previewContainer.value) {
    const el = previewContainer.value;
    const height = el.scrollHeight;
    scrollRatio.value = height ? el.scrollTop / height : 0;
  } else {
    scrollRatio.value = 0;
  }
};

const restoreRealtimeScroll = () => {
  const el = getRealtimeScrollElement();
  if (!el) {
    pendingRealtimeScrollRestore.value = true;
    return;
  }
  const height = el.scrollHeight;
  el.scrollTop = height * scrollRatio.value;
  pendingRealtimeScrollRestore.value = false;
};

const restoreScrollPosition = () => {
  if (previewMode.value === 'realtime') {
    restoreRealtimeScroll();
    return;
  }

  if (mode.value === 'preview') {
    pendingPreviewScrollRestore.value = true;
  } else if (mode.value === 'edit' && editor) {
    nextTick(() => {
      const height = editor.getScrollHeight();
      const targetTop = height * scrollRatio.value;
      editor.setScrollTop(targetTop);
    });
  }
};

const scrollPreviewElementIntoView = (container, target, { behavior = 'smooth' } = {}) => {
  if (!container || !target) return false;
  const containerRect = container.getBoundingClientRect();
  const targetRect = target.getBoundingClientRect();
  const offset = targetRect.top - containerRect.top;
  const nextTop = Math.max(0, container.scrollTop + offset - container.clientHeight * 0.2);
  container.scrollTo({ top: nextTop, behavior });
  return true;
};

const getPreviewLineAnchors = () => {
  const container = previewContainer.value;
  if (!container) return [];

  if (
    previewLineAnchorCache.container === container &&
    previewLineAnchorCache.revision === previewLineAnchorRevision
  ) {
    return previewLineAnchorCache.elements;
  }

  const elements = Array.from(
    container.querySelectorAll('[data-md-start-line]')
  ).filter((el) => !el.closest?.('.md-toc'));

  previewLineAnchorCache.container = container;
  previewLineAnchorCache.revision = previewLineAnchorRevision;
  previewLineAnchorCache.elements = elements;
  return elements;
};

const scrollPreviewToLine = (lineNumber, { behavior = 'smooth' } = {}) => {
  if (!previewContainer.value || !lineNumber) return false;

  const container = previewContainer.value;
  const elements = getPreviewLineAnchors();

  if (!elements.length) return false;

  const exact = elements.find((el) => Number(el?.dataset?.mdStartLine || 0) === Number(lineNumber));
  if (exact) {
    return scrollPreviewElementIntoView(container, exact, { behavior });
  }

  let target = elements[0];
  let minDelta = Infinity;

  elements.forEach((el) => {
    const start = Number(el.dataset.mdStartLine || 0);
    const end = Number(el.dataset.mdEndLine || start);
    if (!start && !end) return;

    let delta = 0;
    if (lineNumber < start) {
      delta = start - lineNumber;
    } else if (lineNumber > end) {
      delta = lineNumber - end;
    } else {
      delta = 0;
    }

    if (delta < minDelta) {
      minDelta = delta;
      target = el;
    }
  });

  if (!target) return false;
  return scrollPreviewElementIntoView(container, target, { behavior });
};

const syncEditorToLine = (lineNumber, { focus = true } = {}) => {
  if (!lineNumber) return;
  if (!editor) {
    initEditor();
  }
  const targetEditor = getActiveEditor();
  if (!targetEditor) return;

  targetEditor.revealLineInCenter(lineNumber);
  targetEditor.setPosition({ lineNumber, column: 1 });
  if (focus) {
    targetEditor.focus();
  }
};

const splitPackageNames = (value) => Array.from(new Set(
  String(value || '')
    .split(/[\s,，;；]+/)
    .map((item) => item.trim())
    .filter(Boolean)
))

const insertSnippetAtCursor = (snippet) => {
  const targetEditor = mode.value === 'preview' ? secondaryEditor || editor : editor || secondaryEditor
  const model = targetEditor?.getModel?.()
  if (!targetEditor || !model) {
    updateEditorContent(`${String(rawContent.value || '')}\n${snippet}`)
    return
  }

  const selection = targetEditor.getSelection()
  const range = selection || new monaco.Range(
    model.getLineCount(),
    model.getLineMaxColumn(model.getLineCount()),
    model.getLineCount(),
    model.getLineMaxColumn(model.getLineCount())
  )

  targetEditor.executeEdits('python-runtime-snippet', [{ range, text: snippet, forceMoveMarkers: true }])
  targetEditor.focus?.()
}

const shouldAutoPreparePythonRuntime = (status) => !!status?.ready && !status?.initialized

const refreshPythonRuntimeStatus = async (reason = 'manual') => {
  pythonRuntimeBusy.value = true
  try {
    logger.info('[PythonRuntime] refresh status', { reason })
    const status = await invoke('python_runtime_get_status')
    pythonRuntimeStatus.value = status || createEmptyPythonRuntimeStatus()
    const installedPackages = Array.isArray(pythonRuntimeStatus.value?.packages)
      ? pythonRuntimeStatus.value.packages
      : []
    pythonRuntimeSelectedPackages.value = pythonRuntimeSelectedPackages.value.filter((pkg) => installedPackages.includes(pkg))
    logger.info('[PythonRuntime] status loaded', {
      reason,
      source: status?.source || '',
      workerAlive: !!status?.worker_alive,
      packageCount: Array.isArray(status?.packages) ? status.packages.length : 0,
      initialized: !!status?.initialized,
      bundledRuntimePresent: !!status?.bundled_runtime_present,
    })
  } catch (error) {
    pythonRuntimeStatus.value = createEmptyPythonRuntimeStatus()
    logger.warn('[PythonRuntime] load status failed', { reason, error })
  } finally {
    pythonRuntimeBusy.value = false
  }
}

const ensurePythonRuntimeInitialized = async (reason = 'auto', options = {}) => {
  const { silent = false, notify = !silent } = options
  if (!shouldAutoPreparePythonRuntime(pythonRuntimeStatus.value)) return false

  pythonRuntimeBusy.value = true
  try {
    logger.info('[PythonRuntime] auto initialize requested', { reason })
    const res = await invoke('python_runtime_initialize')
    logger.info('[PythonRuntime] auto initialize completed', {
      reason,
      runtimeDir: res?.runtime_dir || '',
      pythonPath: res?.python_path || '',
    })
    if (notify) {
      message.success('Python 运行时已初始化')
    }
    await refreshPythonRuntimeStatus(`initialized:${reason}`)
    return true
  } catch (error) {
    logger.error('[PythonRuntime] auto initialize failed', { reason, error })
    if (!silent) {
      message.error(`初始化失败：${String(error?.message || error)}`)
    }
    return false
  } finally {
    pythonRuntimeBusy.value = false
  }
}

const openPythonRuntimeWorkbench = async () => {
  pythonRuntimePopoverVisible.value = true
  await refreshPythonRuntimeStatus('popover-open')
  await ensurePythonRuntimeInitialized('popover-open', { silent: true, notify: false })
}

const closePythonRuntimeWorkbench = () => {
  pythonRuntimePopoverVisible.value = false
}

const togglePythonRuntimeWorkbench = () => {
  if (pythonRuntimePopoverVisible.value) {
    closePythonRuntimeWorkbench()
    return
  }
  void openPythonRuntimeWorkbench()
}

const getPythonSessionTargetPath = (region = 'left') => {
  if (region === 'right') {
    return String(fileTreeStore.activeTabRight || '').trim()
  }
  return String(fileTreeStore.activeTab || selectedFile.value?.path || '').trim()
}

const getTerminalContextPath = (region = 'left') => {
  const regionPath = getPythonSessionTargetPath(region)
  if (regionPath) {
    return regionPath
  }

  const fallbackPath = String(
    fileTreeStore.activeRepoPath
    || knowledgeBaseStore.selectedKnowledgeBase?.location
    || settingsStore.knowledgeBasesRoot
    || settingsStore.librariesRoot
    || ''
  ).trim()

  // 设计说明：
  // 1. terminal 现在允许在“没有打开文件”的情况下独立显示，所以不能再把 cwd 绑定死在当前文档上。
  // 2. 这里按“当前分栏文件 -> 当前 repo -> 当前知识库根 -> 全局根目录”的优先级回退，
  //    保证任何模式下都能拿到一个稳定可用的工作目录。
  logger.debug('[TerminalPanel] resolved context path', {
    region,
    regionPath,
    fallbackPath,
  })

  return fallbackPath
}

const getPythonSessionIdForPath = (filePath) => createPythonSessionId(String(filePath || '').trim())

const ensurePythonSessionInfo = async (filePath, reason = 'manual') => {
  const targetPath = String(filePath || '').trim()
  if (!targetPath) {
    pythonSessionInfo.value = null
    return null
  }

  try {
    const sessionId = getPythonSessionIdForPath(targetPath)
    const info = await invoke('python_runtime_get_session_info', {
      request: {
        session_id: sessionId,
      },
    })
    pythonSessionInfo.value = info || null
    logger.info('[PythonRuntime] session info loaded', { reason, sessionId, sandboxDir: info?.sandbox_dir || '' })
    return info
  } catch (error) {
    logger.warn('[PythonRuntime] session info load failed', { reason, targetPath, error })
    return null
  }
}

const dropPythonSessionForPath = async (filePath, { silent = false, reason = 'close-tab' } = {}) => {
  const targetPath = String(filePath || '').trim()
  if (!targetPath) return false

  try {
    const sessionId = getPythonSessionIdForPath(targetPath)
    await invoke('python_runtime_drop_session', {
      request: {
        session_id: sessionId,
      },
    })
    logger.info('[PythonRuntime] session dropped', { reason, sessionId, targetPath })
    if (pythonSessionInfo.value?.session_id === sessionId) {
      pythonSessionInfo.value = null
    }
    return true
  } catch (error) {
    logger.warn('[PythonRuntime] session drop failed', { reason, targetPath, error })
    if (!silent) {
      message.error(`销毁 Python 会话失败：${String(error?.message || error)}`)
    }
    return false
  }
}

const disposePythonCellManagers = () => {
  leftPythonCellManager?.dispose?.()
  rightPythonCellManager?.dispose?.()
  leftPythonCellManager = null
  rightPythonCellManager = null
}

const getPythonManagerApplyMarkdown = (targetEditor) => (nextMarkdown, { action = 'pycell-apply' } = {}) => {
  const model = targetEditor?.getModel?.()
  if (!model) return
  const current = String(model.getValue() || '')
  if (current === String(nextMarkdown || '')) return
  model.setValue(String(nextMarkdown || ''))
  rawContent.value = String(nextMarkdown || '')
  const targetPath = targetEditor === secondaryEditor
    ? String(fileTreeStore.activeTabRight || '').trim()
    : String(fileTreeStore.activeTab || '').trim()
  if (targetPath) {
    fileTreeStore.updateTabContent(targetPath, String(nextMarkdown || ''))
  }
  logger.info('[PyCellEditor] markdown applied', { action, targetPath, length: String(nextMarkdown || '').length })
}

const createPythonCellManagerForEditor = (targetEditor) => {
  if (!targetEditor) return null
  return createGuaxiPythonCellEditorManager({
    editor: targetEditor,
    monaco,
    logger,
    message,
    invoke,
    getMarkdown: () => String(targetEditor?.getModel?.()?.getValue?.() || ''),
    applyMarkdown: getPythonManagerApplyMarkdown(targetEditor),
    getFilePath: () => {
      if (targetEditor === secondaryEditor) return String(fileTreeStore.activeTabRight || '').trim()
      return String(fileTreeStore.activeTab || selectedFile.value?.path || '').trim()
    },
    onStateChange: handlePythonCellSharedStateChange,
    onRequestConsole: () => {
      pythonConsoleVisible.value = true
      void ensurePythonSessionInfo(getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left'), 'pycell-open-console')
    },
  })
}

const refreshPythonCellManagers = () => {
  if (!EMBEDDED_PYTHON_CELL_EDITOR_ENABLED || editorSurfaceMode.value === 'notebook') {
    disposePythonCellManagers()
    return
  }
  if (editor?.getModel?.()?.getLanguageId?.() === 'markdown') {
    if (!leftPythonCellManager) {
      leftPythonCellManager = createPythonCellManagerForEditor(editor)
    }
    leftPythonCellManager?.refresh?.()
  } else {
    leftPythonCellManager?.dispose?.()
    leftPythonCellManager = null
  }

  if (secondaryEditor?.getModel?.()?.getLanguageId?.() === 'markdown') {
    if (!rightPythonCellManager) {
      rightPythonCellManager = createPythonCellManagerForEditor(secondaryEditor)
    }
    rightPythonCellManager?.refresh?.()
  } else {
    rightPythonCellManager?.dispose?.()
    rightPythonCellManager = null
  }
}

const scheduleRefreshPythonCellManagers = debounce(() => {
  refreshPythonCellManagers()
}, 120)

const flushPythonCellDrafts = () => {
  if (!EMBEDDED_PYTHON_CELL_EDITOR_ENABLED || editorSurfaceMode.value === 'notebook') return
  const nextLeft = leftPythonCellManager?.flushDrafts?.()
  if (typeof nextLeft === 'string' && editor?.getModel?.() && nextLeft !== editor.getModel().getValue()) {
    getPythonManagerApplyMarkdown(editor)(nextLeft, { action: 'pycell-flush-left' })
  }

  const nextRight = rightPythonCellManager?.flushDrafts?.()
  if (typeof nextRight === 'string' && secondaryEditor?.getModel?.() && nextRight !== secondaryEditor.getModel().getValue()) {
    getPythonManagerApplyMarkdown(secondaryEditor)(nextRight, { action: 'pycell-flush-right' })
  }
}

const normalizeConsoleOutput = (res) => {
  const display = res?.result_json?.__guaxi_display__
  const dataframe = res?.result_json?.__guaxi_dataframe__
  const textParts = []
  if (res?.stdout) textParts.push(String(res.stdout).trim())
  if (res?.stderr) textParts.push(String(res.stderr).trim())
  if (res?.result_repr) textParts.push(String(res.result_repr).trim())
  let nextText = textParts.filter(Boolean).join('\n\n').trim()
  if (!nextText && dataframe) {
    nextText = dataframeToMarkdownTable({
      columns: Array.isArray(dataframe.columns) ? dataframe.columns : [],
      rows: Array.isArray(dataframe.rows) ? dataframe.rows : [],
    })
  }
  if (!nextText && display?.type === 'markdown') {
    nextText = String(display.markdown || '')
  }
  if (!nextText && display?.type === 'html') {
    nextText = String(display.html || '')
  }
  return nextText || '(无输出)'
}

const scrollPythonConsoleToBottom = () => {
  nextTick(() => {
    const el = pythonConsoleOutputRef.value
    if (!el) return
    el.scrollTop = el.scrollHeight
  })
}

const readPythonConsoleStoredNumber = (key, fallback) => {
  try {
    const raw = String(window?.localStorage?.getItem(key) || '').trim()
    const parsed = Number(raw)
    return Number.isFinite(parsed) ? parsed : fallback
  } catch {
    return fallback
  }
}

const writePythonConsoleStoredValue = (key, value) => {
  try {
    window?.localStorage?.setItem(key, String(value))
  } catch {
  }
}

const getPythonConsoleAvailableHeight = () => {
  const wrapperHeight = Number(editorWrapperRef.value?.clientHeight || 0)
  if (!wrapperHeight) return pythonConsoleMaxHeight
  return Math.max(pythonConsoleMinHeight, Math.min(pythonConsoleMaxHeight, wrapperHeight - 96))
}

const clampPythonConsoleHeight = (height) => {
  const maxHeight = getPythonConsoleAvailableHeight()
  return Math.max(
    pythonConsoleMinHeight,
    Math.min(maxHeight, Number(height || pythonConsoleMinHeight)),
  )
}

const persistPythonConsoleState = () => {
  writePythonConsoleStoredValue(PYTHON_CONSOLE_HEIGHT_STORAGE_KEY, pythonConsoleHeight.value)
  writePythonConsoleStoredValue(PYTHON_CONSOLE_MAXIMIZED_STORAGE_KEY, pythonConsoleMaximized.value ? '1' : '0')
}

const restorePythonConsoleState = () => {
  pythonConsoleRestoreHeight = clampPythonConsoleHeight(
    readPythonConsoleStoredNumber(PYTHON_CONSOLE_HEIGHT_STORAGE_KEY, pythonConsoleHeight.value),
  )
  pythonConsoleHeight.value = pythonConsoleRestoreHeight
  pythonConsoleMaximized.value = String(window?.localStorage?.getItem(PYTHON_CONSOLE_MAXIMIZED_STORAGE_KEY) || '') === '1'
  if (pythonConsoleMaximized.value) {
    pythonConsoleHeight.value = getPythonConsoleAvailableHeight()
  }
}

const layoutVisibleEditors = () => {
  nextTick(() => {
    if (pythonConsoleVisible.value && pythonConsoleMaximized.value) {
      pythonConsoleHeight.value = getPythonConsoleAvailableHeight()
    }
    try {
      editor?.layout?.()
    } catch {
    }
    try {
      secondaryEditor?.layout?.()
    } catch {
    }
    if (pythonConsoleVisible.value) {
      // 终端高度变化后同步触发一次 fit，避免 xterm 继续沿用旧行数，底部出现空白区域。
      requestAnimationFrame(() => {
        void terminalPanelRef.value?.fitTerminal?.('python-console-layout')
      })
    }
    logger.debug('[PythonConsole] layout refreshed', {
      visible: pythonConsoleVisible.value,
      height: pythonConsoleHeight.value,
      maximized: pythonConsoleMaximized.value,
    })
  })
}

const handleResizePythonConsoleMove = (event) => {
  if (!pythonConsoleResizing.value) return
  const delta = pythonConsoleResizeStartY - Number(event?.clientY || 0)
  const nextHeight = clampPythonConsoleHeight(pythonConsoleResizeStartHeight + delta)
  if (nextHeight === pythonConsoleHeight.value) return
  pythonConsoleMaximized.value = false
  pythonConsoleHeight.value = nextHeight
  pythonConsoleRestoreHeight = nextHeight
  logger.debug('[PythonConsole] resizing', {
    nextHeight,
    startHeight: pythonConsoleResizeStartHeight,
    clientY: Number(event?.clientY || 0),
  })
  persistPythonConsoleState()
  layoutVisibleEditors()
}

const stopResizePythonConsole = () => {
  if (!pythonConsoleResizing.value) return
  pythonConsoleResizing.value = false
  document.body.style.userSelect = ''
  document.body.style.cursor = ''
  window.removeEventListener('mousemove', handleResizePythonConsoleMove, true)
  window.removeEventListener('mouseup', stopResizePythonConsole, true)
  persistPythonConsoleState()
  layoutVisibleEditors()
}

const startResizePythonConsole = (event) => {
  pythonConsoleResizing.value = true
  pythonConsoleResizeStartY = Number(event?.clientY || 0)
  pythonConsoleResizeStartHeight = Number(pythonConsoleHeight.value || 240)
  logger.info('[PythonConsole] resize start', {
    startY: pythonConsoleResizeStartY,
    startHeight: pythonConsoleResizeStartHeight,
  })
  document.body.style.userSelect = 'none'
  document.body.style.cursor = 'ns-resize'
  window.addEventListener('mousemove', handleResizePythonConsoleMove, true)
  window.addEventListener('mouseup', stopResizePythonConsole, true)
}

const toggleMaximizePythonConsole = () => {
  if (pythonConsoleMaximized.value) {
    pythonConsoleMaximized.value = false
    pythonConsoleHeight.value = clampPythonConsoleHeight(pythonConsoleRestoreHeight || 240)
  } else {
    pythonConsoleRestoreHeight = clampPythonConsoleHeight(pythonConsoleHeight.value || 240)
    pythonConsoleMaximized.value = true
    pythonConsoleHeight.value = getPythonConsoleAvailableHeight()
  }
  persistPythonConsoleState()
  layoutVisibleEditors()
}

const runPythonConsoleInput = async () => {
  const code = String(pythonConsoleInput.value || '').trim()
  if (!code || pythonConsoleBusy.value) return

  const targetPath = getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left')
  await ensurePythonSessionInfo(targetPath, 'console-run')

  pythonConsoleBusy.value = true
  try {
    const sessionId = getPythonSessionIdForPath(targetPath)
    const res = await invoke('python_exec', {
      request: {
        code,
        persist: true,
        expect_json: true,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: targetPath,
          _py_cell_id: '__python_console__',
        },
      },
    })

    pythonConsoleHistory.value.push({
      id: `${Date.now()}-${pythonConsoleHistory.value.length}`,
      input: code,
      output: normalizeConsoleOutput(res),
    })
    pythonConsolePinnedOutput.value = ''
    pythonConsoleInput.value = ''
    scrollPythonConsoleToBottom()
    scheduleRefreshPythonCellManagers()
  } catch (error) {
    pythonConsolePinnedOutput.value = `执行失败：${String(error?.message || error)}`
    scrollPythonConsoleToBottom()
    message.error(`Python 控制台执行失败：${String(error?.message || error)}`)
  } finally {
    pythonConsoleBusy.value = false
  }
}

const resetCurrentPythonSession = async () => {
  const targetPath = getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left')
  const ok = await dropPythonSessionForPath(targetPath, { silent: false, reason: 'console-reset' })
  if (!ok) return
  pythonConsoleHistory.value = []
  pythonConsolePinnedOutput.value = '当前文件的 Python 会话与沙箱已重置'
  await ensurePythonSessionInfo(targetPath, 'console-reset')
  scheduleRefreshPythonCellManagers()
}

const togglePythonConsolePanel = async (reason = 'manual-toggle', options = {}) => {
  pythonConsoleVisible.value = !pythonConsoleVisible.value
  if (pythonConsoleVisible.value) {
    pythonConsoleHeight.value = pythonConsoleMaximized.value
      ? getPythonConsoleAvailableHeight()
      : clampPythonConsoleHeight(pythonConsoleRestoreHeight || pythonConsoleHeight.value || 240)
  }
  logger.info('[PythonConsole] visibility changed', {
    reason,
    visible: pythonConsoleVisible.value,
    height: pythonConsoleHeight.value,
    maximized: pythonConsoleMaximized.value,
  })
  persistPythonConsoleState()
  layoutVisibleEditors()
  if (!pythonConsoleVisible.value) return
  void ensurePythonSessionInfo(getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left'), reason)
  const requestedProfileId = String(
    options?.profileId
    || (String(reason || '').includes('notebook-open-console') ? 'ipython' : '')
  ).trim()
  await nextTick()
  // 这里故意不 await 终端创建，避免点击工具栏后把主界面卡在后端 profile/会话初始化上。
  void terminalPanelRef.value?.ensureSession?.({
    profileId: requestedProfileId || undefined,
    cwd: getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left'),
  })
  terminalPanelRef.value?.focusTerminal?.()
}

const handlePythonRuntimeWorkbenchPointerDown = (event) => {
  if (!pythonRuntimePopoverVisible.value) return
  const target = event?.target
  if (!(target instanceof Node)) return
  if (pythonRuntimeAnchorRef.value?.contains?.(target)) return
  if (pythonRuntimePanelRef.value?.contains?.(target)) return
  closePythonRuntimeWorkbench()
}

const handlePythonRuntimeWorkbenchKeydown = (event) => {
  if (!pythonRuntimePopoverVisible.value) return
  if (event?.key === 'Escape') {
    closePythonRuntimeWorkbench()
  }
}

const initializePythonRuntime = async () => {
  await ensurePythonRuntimeInitialized('manual', { silent: false, notify: true })
}

const installPythonPackages = async (packages, reason = 'manual') => {
  const normalizedPackages = splitPackageNames(Array.isArray(packages) ? packages.join(' ') : packages)
  if (!normalizedPackages.length) {
    message.warning('请输入要安装的 Python 包')
    return
  }

  pythonRuntimeBusy.value = true
  try {
    beginPythonRuntimeOperation('install', normalizedPackages, '正在解析依赖并安装包')
    logger.info('[PythonRuntime] install packages start', { packages: normalizedPackages, reason })
    const res = await invoke('python_runtime_install_packages', { request: { packages: normalizedPackages } })
    openPythonRuntimePackageActionLog(`安装输出 · ${normalizedPackages.join(', ')}`, res)
    updatePythonRuntimeOperationStage('正在刷新已安装包列表')
    if (res?.ok) {
      message.success(`已安装 ${normalizedPackages.join(', ')}`)
      pythonRuntimePackageInput.value = ''
      await refreshPythonRuntimeStatus(`packages-installed:${reason}`)
    } else {
      const errorText = String(res?.stderr || res?.stdout || '安装失败')
      message.error(errorText)
    }
  } catch (error) {
    logger.error('[PythonRuntime] install packages failed', { reason, error })
    message.error(`安装失败：${String(error?.message || error)}`)
  } finally {
    endPythonRuntimeOperation('安装流程已结束')
    pythonRuntimeBusy.value = false
  }
}

const installRecommendedPythonPackage = async (pkg) => {
  await installPythonPackages([pkg], `recommended:${pkg}`)
}

const installPythonRuntimeArchive = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!selected || Array.isArray(selected)) return

    pythonRuntimeBusy.value = true
    pythonRuntimeArchivePath.value = String(selected)
    // 先检查压缩包内容，再决定是否覆盖现有运行时，尽量把误操作风险挡在前面。
    logger.info('[PythonRuntime] inspect archive start', { archivePath: String(selected) })
    const inspection = await invoke('python_runtime_inspect_archive', { archivePath: String(selected) })
    pythonRuntimeArchiveInspection.value = inspection || null
    logger.info('[PythonRuntime] inspect archive completed', {
      archivePath: String(selected),
      pythonEntryFound: !!inspection?.python_entry_found,
      candidateCount: Number(inspection?.candidate_count || 0),
      warningCount: Array.isArray(inspection?.warnings) ? inspection.warnings.length : 0,
    })

    if (!inspection?.python_entry_found) {
      message.error('所选压缩包中没有检测到可用的 Python 解释器，请重新选择。')
      return
    }

    if (pythonRuntimeStatus.value?.bundled_runtime_present) {
      const shouldReplace = await confirmDialog(
        '当前已存在独立 Python 运行时，继续导入会覆盖 utils/python_runtime 中的现有内容，是否继续？',
        {
          title: '覆盖 Python 运行时',
          kind: 'warning',
          okLabel: '继续覆盖',
          cancelLabel: '取消',
        }
      )
      if (!shouldReplace) {
        logger.info('[PythonRuntime] archive install cancelled by user', { archivePath: String(selected) })
        return
      }
    }

    logger.info('[PythonRuntime] install archive start', { archivePath: String(selected) })
    await invoke('python_runtime_install_archive', { archivePath: String(selected) })
    message.success('Python 运行时压缩包已导入')
    await initializePythonRuntime()
  } catch (error) {
    logger.error('[PythonRuntime] install archive failed', error)
    message.error(`导入运行时失败：${String(error?.message || error)}`)
  } finally {
    pythonRuntimeBusy.value = false
  }
}

const installPythonRuntimePackages = async () => {
  await installPythonPackages(pythonRuntimePackageInput.value, 'manual-input')
}

const openPythonRuntimePackageActionLog = (title, res) => {
  pythonRuntimePackageActionLog.value = {
    visible: true,
    title: String(title || 'Python 包管理输出'),
    stdout: String(res?.stdout || ''),
    stderr: String(res?.stderr || ''),
  }
}

const beginPythonRuntimeOperation = (action, packages, stage) => {
  pythonRuntimeOperation.value = {
    active: true,
    action: String(action || '').trim(),
    packages: Array.isArray(packages) ? packages.slice() : [],
    startedAt: Date.now(),
    stage: String(stage || '').trim(),
  }
  logger.info('[PythonRuntime] operation start', {
    action: pythonRuntimeOperation.value.action,
    packages: pythonRuntimeOperation.value.packages,
    stage: pythonRuntimeOperation.value.stage,
  })
}

const updatePythonRuntimeOperationStage = (stage) => {
  if (!pythonRuntimeOperation.value?.active) return
  pythonRuntimeOperation.value = {
    ...pythonRuntimeOperation.value,
    stage: String(stage || '').trim(),
  }
}

const endPythonRuntimeOperation = (stage = '') => {
  if (!pythonRuntimeOperation.value?.active) return
  logger.info('[PythonRuntime] operation finish', {
    action: pythonRuntimeOperation.value.action,
    packages: pythonRuntimeOperation.value.packages,
    stage: String(stage || pythonRuntimeOperation.value.stage || '').trim(),
    elapsedMs: Date.now() - Number(pythonRuntimeOperation.value.startedAt || 0),
  })
  pythonRuntimeOperation.value = {
    active: false,
    action: '',
    packages: [],
    startedAt: 0,
    stage: '',
  }
}

const closePythonRuntimePackageActionLog = () => {
  pythonRuntimePackageActionLog.value = {
    visible: false,
    title: '',
    stdout: '',
    stderr: '',
  }
}

const togglePythonRuntimePackageSelection = (pkg) => {
  const normalizedPackage = String(pkg || '').trim()
  if (!normalizedPackage) return
  const next = new Set(pythonRuntimeSelectedPackages.value)
  if (next.has(normalizedPackage)) {
    next.delete(normalizedPackage)
  } else {
    next.add(normalizedPackage)
  }
  pythonRuntimeSelectedPackages.value = Array.from(next)
}

const clearPythonRuntimePackageSelection = () => {
  pythonRuntimeSelectedPackages.value = []
}

const executePythonPackageAction = async (packages, action = 'upgrade') => {
  const normalizedPackages = Array.from(new Set(
    (Array.isArray(packages) ? packages : [packages])
      .map((pkg) => String(pkg || '').trim())
      .filter(Boolean)
  ))
  const normalizedAction = String(action || '').trim().toLowerCase()
  if (!normalizedPackages.length) {
    message.warning('请选择要处理的 Python 包')
    return
  }

  const actionLabelMap = {
    upgrade: '升级',
    uninstall: '卸载',
    reinstall: '重装',
  }
  const actionLabel = actionLabelMap[normalizedAction] || normalizedAction

  if (normalizedAction === 'uninstall') {
    const shouldContinue = await confirmDialog(
      `确定要卸载 ${normalizedPackages.length} 个 Python 包吗？`,
      {
        title: '卸载 Python 包',
        kind: 'warning',
        okLabel: '确认卸载',
        cancelLabel: '取消',
      }
    )
    if (!shouldContinue) return
  }

  pythonRuntimeBusy.value = true
  try {
    beginPythonRuntimeOperation(normalizedAction, normalizedPackages, `${actionLabel}任务已提交`)
    logger.info('[PythonRuntime] manage package start', {
      action: normalizedAction,
      packages: normalizedPackages,
    })

    const res = await invoke('python_runtime_manage_packages', {
      request: {
        action: normalizedAction,
        packages: normalizedPackages,
      },
    })

    openPythonRuntimePackageActionLog(
      `${actionLabel}输出 · ${normalizedPackages.join(', ')}`,
      res
    )
    updatePythonRuntimeOperationStage('正在刷新已安装包列表')

    if (res?.ok) {
      message.success(`${actionLabel}成功：${normalizedPackages.join(', ')}`)
      await refreshPythonRuntimeStatus(`package-${normalizedAction}:${normalizedPackages.join(',')}`)
      if (normalizedAction === 'uninstall') {
        pythonRuntimeSelectedPackages.value = pythonRuntimeSelectedPackages.value.filter(
          (pkg) => !normalizedPackages.includes(pkg)
        )
      }
      return
    }

    const errorText = String(res?.stderr || res?.stdout || `${actionLabel}失败`)
    message.error(errorText)
  } catch (error) {
    logger.error('[PythonRuntime] manage package failed', {
      action: normalizedAction,
      packages: normalizedPackages,
      error,
    })
    message.error(`${actionLabel}失败：${String(error?.message || error)}`)
  } finally {
    endPythonRuntimeOperation(`${actionLabel}流程已结束`)
    pythonRuntimeBusy.value = false
  }
}

const managePythonPackage = async (pkg, action = 'upgrade') => {
  const normalizedPackage = String(pkg || '').trim()
  const normalizedAction = String(action || '').trim().toLowerCase()
  if (!normalizedPackage) {
    message.warning('请选择要处理的 Python 包')
    return
  }
  await executePythonPackageAction([normalizedPackage], normalizedAction)
}

const manageSelectedPythonPackages = async (action = 'upgrade') => {
  await executePythonPackageAction(pythonRuntimeSelectedPackages.value, action)
}

const upgradeManagedPip = async () => {
  // 这里单独暴露一个 pip 升级入口，避免用户必须手输命令，也让输出继续复用现有日志面板。
  logger.info('[PythonRuntime] upgrade pip requested from workbench')
  await executePythonPackageAction(['pip'], 'upgrade')
}

const resetPythonKernelSession = async () => {
  pythonRuntimeBusy.value = true
  try {
    await invoke('python_reset_repl')
    message.success('Python 会话已重启')
    await refreshPythonRuntimeStatus('reset-session')
  } catch (error) {
    logger.error('[PythonRuntime] reset session failed', error)
    message.error(`重启会话失败：${String(error?.message || error)}`)
  } finally {
    pythonRuntimeBusy.value = false
  }
}

const insertPythonTableImportSnippet = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Table',
        extensions: ['csv', 'tsv', 'json', 'xlsx', 'xls', 'parquet'],
      }],
    })
    if (!selected || Array.isArray(selected)) return

    const normalized = String(selected).replace(/\\/g, '\\\\')
    const lower = normalized.toLowerCase()
    let reader = 'pd.read_csv'
    let readerArgs = ''
    if (lower.endsWith('.tsv')) {
      readerArgs = ", sep='\\t'"
    } else if (lower.endsWith('.xlsx') || lower.endsWith('.xls')) {
      reader = 'pd.read_excel'
    } else if (lower.endsWith('.json')) {
      reader = 'pd.read_json'
    } else if (lower.endsWith('.parquet')) {
      reader = 'pd.read_parquet'
    }

    const snippet = `import pandas as pd\n\ndf = ${reader}(r"${normalized}"${readerArgs})\ndf\n`
    insertSnippetAtCursor(snippet)
    message.success('已插入表格导入代码')
  } catch (error) {
    logger.warn('[PythonRuntime] insert table import snippet failed', error)
    message.error(`插入导入代码失败：${String(error?.message || error)}`)
  }
}

const handlePreviewClick = async (e) => {
  const base = e?.target instanceof Element ? e.target : e?.target?.parentElement;
  if (!base) return;

  // 纯预览模式：任意点击都记录一次当前位置（优先使用被点击块的 data-md-start-line）。
  try {
    const previewOnly = mode.value === 'preview' && showPreview.value && !showEditor.value
    const container = previewContainer.value
    if (previewOnly && container && container.contains(base)) {
      const hit = base.closest?.('[data-md-start-line]')
      const ln = Number(hit?.dataset?.mdStartLine || 0)
      const path = fileTreeStore.activeTab || selectedFile.value?.path
      const st = container.scrollTop
      const h = container.scrollHeight
      const ratio = h ? st / h : 0

      if (path) {
        let hitIndex = null
        try {
          if (hit && Number.isFinite(ln) && ln > 0) {
            const sameLine = Array.from(container.querySelectorAll(`[data-md-start-line="${ln}"]`) || [])
            const idx = sameLine.indexOf(hit)
            hitIndex = idx >= 0 ? idx : null
          }
        } catch {
          hitIndex = null
        }

        recordPreviewLocation('click', {
          path,
          scrollTop: st,
          scrollRatio: ratio,
          lineNumber: Number.isFinite(ln) && ln > 0 ? ln : undefined,
          hitIndex
        })
      } else {
        recordPreviewLocation('click')
      }
    }
  } catch (err) {
    logger.warn('[PreviewNav] record click failed', err)
  }

  const guaxiSafeDecode = (v) => {
    try {
      return decodeURIComponent(String(v || ''))
    } catch {
      return String(v || '')
    }
  }

  const escapeHtml = (value) => String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')

  const splitPackageNames = (value) => Array.from(new Set(
    String(value || '')
      .split(/[\s,，;；]+/)
      .map((item) => item.trim())
      .filter(Boolean)
  ))

  const insertSnippetAtCursor = (snippet) => {
    const targetEditor = mode.value === 'preview' ? secondaryEditor || editor : editor || secondaryEditor
    const model = targetEditor?.getModel?.()
    if (!targetEditor || !model) {
      updateEditorContent(`${String(rawContent.value || '')}\n${snippet}`)
      return
    }

    const selection = targetEditor.getSelection()
    const range = selection || new monaco.Range(
      model.getLineCount(),
      model.getLineMaxColumn(model.getLineCount()),
      model.getLineCount(),
      model.getLineMaxColumn(model.getLineCount())
    )

    targetEditor.executeEdits('python-runtime-snippet', [{ range, text: snippet, forceMoveMarkers: true }])
    targetEditor.focus?.()
  }

  const refreshPythonRuntimeStatus = async (reason = 'manual') => {
    pythonRuntimeBusy.value = true
    try {
      logger.info('[PythonRuntime] refresh status', { reason })
      const status = await invoke('python_runtime_get_status')
      pythonRuntimeStatus.value = status || createEmptyPythonRuntimeStatus()
      logger.info('[PythonRuntime] status loaded', {
        reason,
        source: status?.source || '',
        workerAlive: !!status?.worker_alive,
        packageCount: Array.isArray(status?.packages) ? status.packages.length : 0,
        initialized: !!status?.initialized,
        bundledRuntimePresent: !!status?.bundled_runtime_present,
      })
    } catch (error) {
      pythonRuntimeStatus.value = createEmptyPythonRuntimeStatus()
      logger.warn('[PythonRuntime] load status failed', { reason, error })
    } finally {
      pythonRuntimeBusy.value = false
    }
  }

  const handlePythonRuntimePopoverUpdateShow = (show) => {
    pythonRuntimePopoverVisible.value = !!show
    if (show) {
      void refreshPythonRuntimeStatus('popover-open')
    }
  }

  const initializePythonRuntime = async () => {
    pythonRuntimeBusy.value = true
    try {
      logger.info('[PythonRuntime] initialize requested')
      const res = await invoke('python_runtime_initialize')
      logger.info('[PythonRuntime] initialize completed', {
        runtimeDir: res?.runtime_dir || '',
        pythonPath: res?.python_path || '',
      })
      message.success('Python 运行时已初始化')
      await refreshPythonRuntimeStatus('initialized')
    } catch (error) {
      logger.error('[PythonRuntime] initialize failed', error)
      message.error(`初始化失败：${String(error?.message || error)}`)
    } finally {
      pythonRuntimeBusy.value = false
    }
  }

  const installPythonPackages = async (packages, reason = 'manual') => {
    const normalizedPackages = splitPackageNames(Array.isArray(packages) ? packages.join(' ') : packages)
    if (!normalizedPackages.length) {
      message.warning('请输入要安装的 Python 包')
      return
    }

    pythonRuntimeBusy.value = true
    try {
      logger.info('[PythonRuntime] install packages start', { packages: normalizedPackages, reason })
      const res = await invoke('python_runtime_install_packages', { request: { packages: normalizedPackages } })
      if (res?.ok) {
        message.success(`已安装 ${normalizedPackages.join(', ')}`)
        pythonRuntimePackageInput.value = ''
        await refreshPythonRuntimeStatus(`packages-installed:${reason}`)
      } else {
        const errorText = String(res?.stderr || res?.stdout || '安装失败')
        message.error(errorText)
      }
    } catch (error) {
      logger.error('[PythonRuntime] install packages failed', { reason, error })
      message.error(`安装失败：${String(error?.message || error)}`)
    } finally {
      pythonRuntimeBusy.value = false
    }
  }

  const installRecommendedPythonPackage = async (pkg) => {
    await installPythonPackages([pkg], `recommended:${pkg}`)
  }

  const installPythonRuntimeArchive = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'ZIP', extensions: ['zip'] }],
      })
      if (!selected || Array.isArray(selected)) return

      pythonRuntimeBusy.value = true
      pythonRuntimeArchivePath.value = String(selected)
      // 先检查压缩包内容，再决定是否覆盖现有运行时，尽量把误操作风险挡在前面。
      logger.info('[PythonRuntime] inspect archive start', { archivePath: String(selected) })
      const inspection = await invoke('python_runtime_inspect_archive', { archivePath: String(selected) })
      pythonRuntimeArchiveInspection.value = inspection || null
      logger.info('[PythonRuntime] inspect archive completed', {
        archivePath: String(selected),
        pythonEntryFound: !!inspection?.python_entry_found,
        candidateCount: Number(inspection?.candidate_count || 0),
        warningCount: Array.isArray(inspection?.warnings) ? inspection.warnings.length : 0,
      })

      if (!inspection?.python_entry_found) {
        message.error('所选压缩包中没有检测到可用的 Python 解释器，请重新选择。')
        return
      }

      if (pythonRuntimeStatus.value?.bundled_runtime_present) {
        const shouldReplace = await confirmDialog(
          '当前已存在独立 Python 运行时，继续导入会覆盖 utils/python_runtime 中的现有内容，是否继续？',
          {
            title: '覆盖 Python 运行时',
            kind: 'warning',
            okLabel: '继续覆盖',
            cancelLabel: '取消',
          }
        )
        if (!shouldReplace) {
          logger.info('[PythonRuntime] archive install cancelled by user', { archivePath: String(selected) })
          return
        }
      }

      logger.info('[PythonRuntime] install archive start', { archivePath: String(selected) })
      await invoke('python_runtime_install_archive', { archivePath: String(selected) })
      message.success('Python 运行时压缩包已导入')
      await initializePythonRuntime()
    } catch (error) {
      logger.error('[PythonRuntime] install archive failed', error)
      message.error(`导入运行时失败：${String(error?.message || error)}`)
    } finally {
      pythonRuntimeBusy.value = false
    }
  }

  const installPythonRuntimePackages = async () => {
    await installPythonPackages(pythonRuntimePackageInput.value, 'manual-input')
  }

  const resetPythonKernelSession = async () => {
    pythonRuntimeBusy.value = true
    try {
      await invoke('python_reset_repl')
      message.success('Python 会话已重启')
      await refreshPythonRuntimeStatus('reset-session')
    } catch (error) {
      logger.error('[PythonRuntime] reset session failed', error)
      message.error(`重启会话失败：${String(error?.message || error)}`)
    } finally {
      pythonRuntimeBusy.value = false
    }
  }

  const insertPythonTableImportSnippet = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Table',
          extensions: ['csv', 'tsv', 'json', 'xlsx', 'xls', 'parquet'],
        }],
      })
      if (!selected || Array.isArray(selected)) return

      const normalized = String(selected).replace(/\\/g, '\\\\')
      const lower = normalized.toLowerCase()
      let reader = "pd.read_csv"
      let readerArgs = ""
      if (lower.endsWith('.tsv')) {
        readerArgs = ", sep='\\t'"
      } else if (lower.endsWith('.xlsx') || lower.endsWith('.xls')) {
        reader = 'pd.read_excel'
      } else if (lower.endsWith('.json')) {
        reader = 'pd.read_json'
      } else if (lower.endsWith('.parquet')) {
        reader = 'pd.read_parquet'
      }

      const snippet = `import pandas as pd\n\ndf = ${reader}(r"${normalized}"${readerArgs})\ndf\n`
      insertSnippetAtCursor(snippet)
      message.success('已插入表格导入代码')
    } catch (error) {
      logger.warn('[PythonRuntime] insert table import snippet failed', error)
      message.error(`插入导入代码失败：${String(error?.message || error)}`)
    }
  }

  const formatPythonExecOutput = (res) => {
    const ok = res?.ok
    const stdout = String(res?.stdout || '')
    const stderr = String(res?.stderr || '')
    const resultRepr = String(res?.result_repr || '')

    const hasStdout = stdout.trim().length > 0
    const hasStderr = stderr.trim().length > 0
    const hasResultRepr = resultRepr.trim().length > 0

    if (ok === false) {
      if (hasStderr) return stderr.trimEnd()
      if (hasStdout) return stdout.trimEnd()
      return '(执行失败：无错误输出)'
    }

    const parts = []
    if (hasStdout) parts.push(stdout.trimEnd())
    if (hasStderr) parts.push(stderr.trimEnd())
    if (!hasStdout && !hasStderr && hasResultRepr) parts.push(resultRepr.trimEnd())

    return parts.join('\n\n')
  }

  const buildPyCellSessionId = (docPath) => {
    const normalized = String(docPath || '').trim()
    return normalized || 'guaxi:pycell:global'
  }

  const getPyCellSessionLabel = (sessionId) => {
    const normalized = String(sessionId || '').trim()
    if (!normalized) return 'global'
    const parts = normalized.split(/[\\/]/).filter(Boolean)
    return parts.length ? parts[parts.length - 1] : normalized
  }

  const createPyCellOutputModuleId = (prefix = 'module') => {
    return `pycell-${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  }

  const setPyCellBusyState = (cell, busy) => {
    if (!(cell instanceof HTMLElement)) return
    cell.classList.toggle('is-running', !!busy)
    cell.setAttribute('data-pycell-busy', busy ? 'true' : 'false')
    const hint = cell.querySelector?.('.guaxi-pycell-hint')
    if (hint) {
      hint.textContent = busy
        ? 'Python 正在运行，请稍候...'
        : '共享当前笔记的 Python 会话，支持导入表格、注入笔记和查看变量'
    }
  }

  const normalizePyCellOutputModule = (module) => {
    if (!module || typeof module !== 'object') return null
    const type = String(module.type || '').trim()
    if (!type) return null
    return {
      id: String(module.id || createPyCellOutputModuleId(type)).trim(),
      type,
      title: String(module.title || '输出模块').trim() || '输出模块',
      meta: String(module.meta || '').trim(),
      table: module.table && typeof module.table === 'object' ? module.table : null,
      display: module.display && typeof module.display === 'object' ? module.display : null,
    }
  }

  const getPyCellOutputModulesFromSnapshot = (cellId) => {
    const snapshot = getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)
    const modules = Array.isArray(snapshot?.outputModules) ? snapshot.outputModules : []
    return modules
      .map((module) => normalizePyCellOutputModule(module))
      .filter(Boolean)
  }

  const buildPyCellOutputModules = (res) => {
    const modules = []
    const dataframe = getGuaxiDataFrameFromResult(res)
    const display = getGuaxiDisplayFromResult(res)

    if (dataframe) {
      const sourceName = String(dataframe.sourcePath || '').trim().split(/[\\/]/).pop() || ''
      modules.push(normalizePyCellOutputModule({
        id: createPyCellOutputModuleId('table'),
        type: 'dataframe',
        title: '表格输出',
        meta: sourceName || `${dataframe.rowCount} 行 × ${(dataframe.columns || []).length} 列`,
        table: dataframe,
      }))
    }

    if (display) {
      const typeLabelMap = {
        image: '图像输出',
        html: 'HTML 输出',
        svg: 'SVG 输出',
        markdown: 'Markdown 输出',
      }
      modules.push(normalizePyCellOutputModule({
        id: createPyCellOutputModuleId(display.type || 'display'),
        type: 'display',
        title: typeLabelMap[String(display.type || '').trim().toLowerCase()] || '富输出',
        meta: String(display.mime || '').trim(),
        display,
      }))
    }

    return modules.filter(Boolean)
  }

  const mergePyCellOutputModules = (cellId, incomingModules = [], { replace = false } = {}) => {
    const normalizedIncoming = (Array.isArray(incomingModules) ? incomingModules : [])
      .map((module) => normalizePyCellOutputModule(module))
      .filter(Boolean)
    if (replace) return normalizedIncoming

    const prevModules = getPyCellOutputModulesFromSnapshot(cellId)
    if (!normalizedIncoming.length) return prevModules
    return [...prevModules, ...normalizedIncoming]
  }

  const sanitizePythonIdentifier = (value, fallback = 'df') => {
    const normalized = String(value || '')
      .replace(/\.[^.]+$/, '')
      .replace(/[^A-Za-z0-9_]+/g, '_')
      .replace(/^_+/, '')

    const safe = normalized.length > 0 ? normalized : fallback
    return /^[A-Za-z_]/.test(safe) ? safe : `${fallback}_${safe}`
  }

  const ensurePyCellToolbarEnhancements = (cell) => {
    if (!(cell instanceof Element)) return
    const toolbar = cell.querySelector?.('.guaxi-pycell-toolbar')
    if (!toolbar) return

    const ensureButton = (action, label, className, beforeAction = 'toggle-output') => {
      let btn = toolbar.querySelector(`[data-guaxi-py-action="${action}"]`)
      if (btn) {
        btn.textContent = label
        return btn
      }
      btn = document.createElement('button')
      btn.type = 'button'
      btn.className = `guaxi-pycell-btn ${className}`.trim()
      btn.setAttribute('data-guaxi-py-action', action)
      btn.textContent = label

      const anchor = beforeAction ? toolbar.querySelector(`[data-guaxi-py-action="${beforeAction}"]`) : null
      if (anchor?.parentNode === toolbar) toolbar.insertBefore(btn, anchor)
      else toolbar.appendChild(btn)
      return btn
    }

    ensureButton('run', '运行', 'guaxi-pycell-run')
    ensureButton('run-save', '运行并写回', 'guaxi-pycell-run-save')
    ensureButton('import-table', '导入表格', 'guaxi-pycell-import')
    ensureButton('inject-note', '注入笔记', 'guaxi-pycell-note')
    ensureButton('toggle-session', '会话变量', 'guaxi-pycell-session-btn')
    ensureButton('restart-session', '重启会话', 'guaxi-pycell-reset')
    ensureButton('toggle-output', '显示结果', 'guaxi-pycell-output-toggle', null)

    let hint = toolbar.querySelector('.guaxi-pycell-hint')
    if (!hint) {
      hint = document.createElement('span')
      hint.className = 'guaxi-pycell-hint'
      toolbar.appendChild(hint)
    }
    hint.textContent = '共享当前笔记的 Python 会话，支持导入表格、注入笔记和查看变量'
  }

  function getActivePythonCellFilePath() {
    return String(selectedFile.value?.path || '').trim()
  }

  const ensurePyCellTextOutputToggle = (cell, outputPre, cellId = '') => {
    if (!(cell instanceof Element) || !(outputPre instanceof HTMLElement)) return null

    let shell = outputPre.parentElement
    if (!shell || !shell.classList.contains('guaxi-pycell-text-output-shell')) {
      shell = document.createElement('div')
      shell.className = 'guaxi-pycell-text-output-shell'
      outputPre.parentNode?.insertBefore(shell, outputPre)
      shell.appendChild(outputPre)
    }

    let toolbar = shell.querySelector('.guaxi-pycell-output-module-toolbar')
    if (!toolbar) {
      toolbar = document.createElement('div')
      toolbar.className = 'guaxi-pycell-output-module-toolbar'
      shell.insertBefore(toolbar, shell.firstChild)
    }

    let meta = toolbar.querySelector('.guaxi-pycell-output-module-meta')
    if (!meta) {
      meta = document.createElement('div')
      meta.className = 'guaxi-pycell-output-module-meta'
      meta.textContent = '文本输出'
      toolbar.appendChild(meta)
    }

    let btn = toolbar.querySelector('[data-guaxi-py-action="toggle-text-output"]')
    if (!btn) {
      btn = document.createElement('button')
      btn.type = 'button'
      btn.className = 'guaxi-pycell-output-module-btn'
      btn.setAttribute('data-guaxi-py-action', 'toggle-text-output')
      toolbar.appendChild(btn)
    }

    if (cellId) btn.setAttribute('data-guaxi-py-cell-id', cellId)
    const visible = String(outputPre.style.display || '').trim() !== 'none'
    btn.setAttribute('title', visible ? '隐藏文本输出' : '显示文本输出')
    btn.textContent = visible ? '▾ 文本' : '▸ 文本'
    return btn
  }

  function patchSharedPythonCellState(cellId, patch = {}) {
    if (!cellId) return null
    return patchPythonCellSnapshot(getActivePythonCellFilePath(), cellId, patch)
  }

  function applySharedPythonCellStateToPreviewCell(cell) {
    const cellId = guaxiSafeDecode(cell?.getAttribute?.('data-guaxi-py-cell'))
    if (!cellId) return
    const snapshot = getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)
    if (!snapshot) return

    const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
    const outputPre = outputEl ? outputEl.closest?.('pre') : null
    ensurePyCellTextOutputToggle(cell, outputPre, cellId)
    if (outputEl) {
      outputEl.textContent = String(snapshot.outputText || '')
    }
    if (outputPre) {
      outputPre.style.display = String(snapshot.outputText || '').trim() ? '' : 'none'
    }

    if (Array.isArray(snapshot.outputModules) && snapshot.outputModules.length > 0) {
      renderPyCellDisplay(cell, outputPre, null, String(snapshot.outputText || ''), {
        cellId,
        modules: snapshot.outputModules,
      })
      if (snapshot.result) {
        renderPyCellSessionState(cell, snapshot.result, {
          forceVisible: snapshot.meta?.sessionCollapsed === false,
        })
      }
    } else if (snapshot.result) {
      renderPyCellDisplay(cell, outputPre, snapshot.result, String(snapshot.outputText || ''), {
        cellId,
      })
      renderPyCellSessionState(cell, snapshot.result, {
        forceVisible: snapshot.meta?.sessionCollapsed === false,
      })
    }

    const media = cell.querySelector?.('.guaxi-pycell-media')
    if (snapshot.meta?.outputCollapsed) {
      if (outputPre) outputPre.style.display = 'none'
      if (media) media.style.display = 'none'
    }

    const outputToggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-output"]')
    if (outputToggleBtn) {
      const visible = (outputPre && String(outputPre.style.display || '').trim() !== 'none')
        || (media && String(media.style.display || '').trim() !== 'none')
      outputToggleBtn.textContent = visible ? '隐藏结果' : '显示结果'
    }

    const sessionToggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-session"]')
    if (sessionToggleBtn) {
      const panel = cell.querySelector?.('.guaxi-pycell-session')
      const textToggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-text-output"]')
      const visible = panel && String(panel.style.display || '').trim() !== 'none'
      sessionToggleBtn.textContent = visible ? '隐藏变量' : '会话变量'
    }
  }

  function syncAllPreviewPythonCells() {
    previewContainer.value?.querySelectorAll?.('.guaxi-pycell')?.forEach?.((cell) => {
      ensurePyCellToolbarEnhancements(cell)
      applySharedPythonCellStateToPreviewCell(cell)
    })
  }

  function handlePythonCellSharedStateChange() {
    syncAllPreviewPythonCells()
  }

  const ensurePyCellSessionContainer = (cell, afterEl) => {
    if (!(cell instanceof Element)) return null

    let panel = cell.querySelector('.guaxi-pycell-session')
    if (panel) return panel

    panel = document.createElement('div')
    panel.className = 'guaxi-pycell-session'
    panel.style.display = 'none'

    if (afterEl && afterEl.parentNode) afterEl.parentNode.insertBefore(panel, afterEl.nextSibling)
    else cell.appendChild(panel)

    return panel
  }

  const clearPyCellSessionPanel = (cell) => {
    const panel = cell?.querySelector?.('.guaxi-pycell-session')
    if (!panel) return
    panel.innerHTML = ''
    panel.style.display = 'none'
    panel.removeAttribute('data-force-visible')
  }

  const renderPyCellSessionState = (cell, res, { forceVisible = false } = {}) => {
    const anchor = cell?.querySelector?.('.guaxi-pycell-media') || cell?.querySelector?.('.guaxi-pycell-output')
    const panel = ensurePyCellSessionContainer(cell, anchor)
    if (!panel) return

    const vars = Array.isArray(res?.session_vars) ? res.session_vars : []
    const sessionId = String(res?.session_id || '').trim()
    const sessionLabel = getPyCellSessionLabel(sessionId)
    const visible = forceVisible || panel.getAttribute('data-force-visible') === 'true'
    if (visible) panel.setAttribute('data-force-visible', 'true')

    const rowsHtml = vars.length > 0
      ? vars.map((item) => {
        const meta = [
          item?.value_type ? String(item.value_type) : '',
          Number.isFinite(Number(item?.rows)) && Number.isFinite(Number(item?.columns))
            ? `${Number(item.rows)}x${Number(item.columns)}`
            : '',
          Number.isFinite(Number(item?.length)) ? `len=${Number(item.length)}` : '',
        ].filter(Boolean).join(' · ')

        return `
          <div class="guaxi-pycell-var-row">
            <div class="guaxi-pycell-var-main">
              <span class="guaxi-pycell-var-name">${escapeHtml(item?.name || '')}</span>
              <span class="guaxi-pycell-var-type">${escapeHtml(meta || String(item?.value_type || 'value'))}</span>
            </div>
            <div class="guaxi-pycell-var-preview">${escapeHtml(item?.preview || '')}</div>
          </div>
        `
      }).join('')
      : '<div class="guaxi-pycell-session-empty">当前会话里还没有可见变量</div>'

    panel.innerHTML = `
      <div class="guaxi-pycell-session-header">
        <div>
          <div class="guaxi-pycell-session-title">Python 会话 · ${escapeHtml(sessionLabel)}</div>
          <div class="guaxi-pycell-session-meta">${escapeHtml(`${vars.length} 个变量`)}</div>
        </div>
      </div>
      <div class="guaxi-pycell-session-list">${rowsHtml}</div>
    `
    panel.style.display = visible ? 'block' : 'none'
  }

  const getPdfArtifactFromCodeCell = (res) => {
    const artifacts = Array.isArray(res?.artifacts) ? res.artifacts : []
    if (!artifacts.length) return null

    const hit = artifacts.find((a) => {
      const kind = String(a?.kind || '').trim().toLowerCase()
      const mime = String(a?.mime || '').trim().toLowerCase()
      return kind === 'pdf' || mime === 'application/pdf'
    })

    if (!hit) return null

    const dataBase64 = String(hit?.data_base64 || hit?.dataBase64 || '').trim()
    const name = String(hit?.name || hit?.filename || hit?.fileName || '').trim() || 'output.pdf'
    if (!dataBase64) return null

    return { name, mime: 'application/pdf', dataBase64 }
  }

  const isLatexKind = (kind) => {
    const k = String(kind || '').trim().toLowerCase()
    return k === 'tex' || k === 'latex'
  }

  const formatCodeCellExecOutput = (res, kind) => {
    const ok = res?.ok
    const stdout = String(res?.stdout || '')
    const stderr = String(res?.stderr || '')

    const hasStdout = stdout.trim().length > 0
    const hasStderr = stderr.trim().length > 0

    const pdf = getPdfArtifactFromCodeCell(res)

    if (ok === false) {
      if (hasStderr) return stderr.trimEnd()
      if (hasStdout) return stdout.trimEnd()
      return '(执行失败：无错误输出)'
    }

    if (isLatexKind(kind) && pdf) {
      return ''
    }

    const parts = []
    if (hasStdout) parts.push(stdout.trimEnd())
    if (hasStderr) parts.push(stderr.trimEnd())

    return parts.join('\n\n')
  }

  const base64ToUint8Array = (b64) => {
    const bin = atob(String(b64 || ''))
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i)
    return bytes
  }

  const ensureCodeCellMediaContainer = (cell, afterEl) => {
    if (!(cell instanceof Element)) return null

    let media = cell.querySelector('.guaxi-codecell-media')
    if (media) return media

    media = document.createElement('div')
    media.className = 'guaxi-codecell-media'
    media.style.marginTop = '8px'
    media.style.display = 'none'
    media.style.flexDirection = 'column'
    media.style.alignItems = 'center'
    media.style.justifyContent = 'center'
    media.style.position = 'relative'

    if (afterEl && afterEl.parentNode) {
      afterEl.parentNode.insertBefore(media, afterEl.nextSibling)
    } else {
      cell.appendChild(media)
    }

    return media
  }

  const createPdfToolbarIcon = (pathD, { viewBox = '0 0 24 24' } = {}) => {
    return `
      <svg viewBox="${viewBox}" aria-hidden="true" focusable="false">
        <path d="${pathD}" fill="currentColor"></path>
      </svg>
    `
  }

  const PDF_TOOLBAR_ICONS = {
    prev: createPdfToolbarIcon('M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z'),
    next: createPdfToolbarIcon('m8.59 16.59 1.41 1.41 6-6-6-6-1.41 1.41L13.17 12z'),
    zoomOut: createPdfToolbarIcon('M19 13H5v-2h14z'),
    zoomIn: createPdfToolbarIcon('M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6z'),
    fitWidth: createPdfToolbarIcon('M3 5h6v2H5v4H3zm16 0h2v6h-2V7h-4V5zm0 14h-6v-2h4v-4h2zm-16 0v-6h2v4h4v2z'),
    download: createPdfToolbarIcon('M5 20h14v-2H5zm7-18v10.17l3.59-3.58L17 10l-5 5-5-5 1.41-1.41L11 12.17V2z'),
  }

  const savePdfArtifactToFile = async (pdf, bytes) => {
    const defaultName = String(pdf?.name || '').trim() || 'output.pdf'
    try {
      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      })
      if (!outputPath) {
        logger.info('[CodeCell] pdf save cancelled', { defaultName })
        return false
      }

      const { writeFile } = await import('@tauri-apps/plugin-fs')
      await writeFile(outputPath, bytes)
      logger.info('[CodeCell] pdf saved', { outputPath, bytes: bytes.length })
      message.success('PDF 已保存')
      return true
    } catch (error) {
      logger.error('[CodeCell] pdf save failed', error)
      message.error(`PDF 保存失败：${String(error?.message || error)}`)
      return false
    }
  }

  const savePdfArtifactToFileVerified = async (pdf, bytes) => {
    const defaultName = String(pdf?.name || '').trim() || 'output.pdf'
    try {
      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      })
      if (!outputPath) {
        logger.info('[CodeCell] pdf save cancelled (verified)', { defaultName })
        return false
      }

      // Use a detached Uint8Array and verify persisted bytes after writing.
      const fileBytes = bytes instanceof Uint8Array
        ? new Uint8Array(bytes)
        : new Uint8Array(Array.from(bytes || []))
      const { writeFile, readFile } = await import('@tauri-apps/plugin-fs')

      logger.info('[CodeCell] pdf save start (verified)', {
        outputPath,
        defaultName,
        bytes: fileBytes.byteLength,
        header: Array.from(fileBytes.slice(0, 8)),
      })

      await writeFile(outputPath, fileBytes)

      const savedBytes = await readFile(outputPath)
      const savedSize = Number(savedBytes?.byteLength || savedBytes?.length || 0)
      const savedHeader = Array.from((savedBytes || []).slice(0, 8))

      logger.info('[CodeCell] pdf save verify', {
        outputPath,
        savedSize,
        savedHeader,
      })

      if (savedSize <= 0) {
        throw new Error('保存后的 PDF 文件大小为 0')
      }

      logger.info('[CodeCell] pdf saved (verified)', {
        outputPath,
        bytes: fileBytes.byteLength,
        verifiedBytes: savedSize,
      })
      message.success('PDF 已保存')
      return true
    } catch (error) {
      logger.error('[CodeCell] pdf save failed (verified)', error)
      message.error(`PDF 保存失败：${String(error?.message || error)}`)
      return false
    }
  }

  const renderPdfArtifactWithPdfJs = async (media, pdf, bytes, blobUrl) => {
    const renderToken = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    media.setAttribute('data-render-token', renderToken)
    const downloadBytes = bytes instanceof Uint8Array
      ? bytes.slice(0)
      : new Uint8Array(Array.from(bytes || []))

    const calcPdfHeightPx = () => {
      const vh = Number(window?.innerHeight || 0)
      const ideal = Math.floor(vh > 0 ? vh * 0.78 : 820)
      return Math.max(780, Math.min(1200, ideal || 820))
    }

    const wrap = document.createElement('div')
    wrap.className = 'guaxi-pdf-preview-shell'
    wrap.style.minHeight = `${calcPdfHeightPx()}px`

    const toolbar = document.createElement('div')
    toolbar.className = 'guaxi-pdf-preview-toolbar'

    const toolbarLeft = document.createElement('div')
    toolbarLeft.className = 'guaxi-pdf-toolbar-group'

    const fileBadge = document.createElement('div')
    fileBadge.className = 'guaxi-pdf-file-badge'
    fileBadge.textContent = pdf.name || 'output.pdf'
    toolbarLeft.appendChild(fileBadge)

    const toolbarCenter = document.createElement('div')
    toolbarCenter.className = 'guaxi-pdf-toolbar-group'

    const toolbarRight = document.createElement('div')
    toolbarRight.className = 'guaxi-pdf-toolbar-group'

    const viewportHost = document.createElement('div')
    viewportHost.className = 'guaxi-pdf-preview-viewport'

    const pagesHost = document.createElement('div')
    pagesHost.className = 'guaxi-pdf-preview-pages'

    const createToolbarButton = (iconKey, title, onClick, { text = '', accent = false } = {}) => {
      const btn = document.createElement('button')
      btn.type = 'button'
      btn.className = `guaxi-pdf-toolbar-btn${accent ? ' accent' : ''}`
      btn.title = title
      btn.setAttribute('aria-label', title)
      btn.innerHTML = `${PDF_TOOLBAR_ICONS[iconKey] || ''}${text ? `<span>${text}</span>` : ''}`
      btn.addEventListener('click', (event) => {
        event.preventDefault()
        event.stopPropagation()
        void onClick?.(event)
      })
      return btn
    }

    const applyPdfScale = (nextScale) => {
      const clampedScale = Math.max(0.65, Math.min(1.8, Number(nextScale || 1)))
      viewportHost.setAttribute('data-scale', String(clampedScale))
      zoomIndicator.textContent = `${Math.round(clampedScale * 100)}%`

      Array.from(pagesHost.querySelectorAll('.guaxi-pdf-page-card')).forEach((pageEl) => {
        const baseWidth = Number(pageEl.getAttribute('data-base-width') || '0')
        if (baseWidth > 0) {
          pageEl.style.width = `${Math.ceil(baseWidth * clampedScale)}px`
        }
      })

      Array.from(pagesHost.querySelectorAll('.guaxi-pdf-page-canvas')).forEach((canvas) => {
        const baseWidth = Number(canvas.getAttribute('data-base-width') || '0')
        const baseHeight = Number(canvas.getAttribute('data-base-height') || '0')
        if (baseWidth > 0) canvas.style.width = `${Math.ceil(baseWidth * clampedScale)}px`
        if (baseHeight > 0) canvas.style.height = `${Math.ceil(baseHeight * clampedScale)}px`
      })
    }

    const pageIndicator = document.createElement('div')
    pageIndicator.className = 'guaxi-pdf-toolbar-indicator'
    pageIndicator.textContent = '页 0 / 0'

    const zoomIndicator = document.createElement('div')
    zoomIndicator.className = 'guaxi-pdf-toolbar-indicator'
    zoomIndicator.textContent = '100%'

    const prevPageBtn = createToolbarButton('prev', '上一页', () => {
      const current = Number(viewportHost.getAttribute('data-current-page') || '1')
      const target = pagesHost.querySelector(`[data-page-number="${Math.max(1, current - 1)}"]`)
      target?.scrollIntoView?.({ behavior: 'smooth', block: 'start' })
    })
    const nextPageBtn = createToolbarButton('next', '下一页', () => {
      const current = Number(viewportHost.getAttribute('data-current-page') || '1')
      const total = Number(viewportHost.getAttribute('data-total-pages') || '1')
      const target = pagesHost.querySelector(`[data-page-number="${Math.min(total, current + 1)}"]`)
      target?.scrollIntoView?.({ behavior: 'smooth', block: 'start' })
    })
    const zoomOutBtn = createToolbarButton('zoomOut', '缩小', () => {
      const currentScale = Number(viewportHost.getAttribute('data-scale') || '1')
      const nextScale = Math.max(0.65, Number((currentScale - 0.1).toFixed(2)))
      applyPdfScale(nextScale)
    })
    const zoomInBtn = createToolbarButton('zoomIn', '放大', () => {
      const currentScale = Number(viewportHost.getAttribute('data-scale') || '1')
      const nextScale = Math.min(1.8, Number((currentScale + 0.1).toFixed(2)))
      applyPdfScale(nextScale)
    })
    const fitWidthBtn = createToolbarButton('fitWidth', '适配宽度', () => {
      applyPdfScale(1)
      viewportHost.scrollTo?.({ top: 0, behavior: 'smooth' })
    })
    const downloadBtn = createToolbarButton('download', '保存 PDF', async () => {
      await savePdfArtifactToFileVerified(pdf, downloadBytes)
    }, { accent: true })

    // Re-apply safe labels here so UI text is stable even if terminal output displays mojibake.
    pageIndicator.textContent = '页 0 / 0'
    prevPageBtn.title = '上一页'
    prevPageBtn.setAttribute('aria-label', '上一页')
    nextPageBtn.title = '下一页'
    nextPageBtn.setAttribute('aria-label', '下一页')
    zoomOutBtn.title = '缩小'
    zoomOutBtn.setAttribute('aria-label', '缩小')
    zoomInBtn.title = '放大'
    zoomInBtn.setAttribute('aria-label', '放大')
    fitWidthBtn.title = '适配宽度'
    fitWidthBtn.setAttribute('aria-label', '适配宽度')
    downloadBtn.title = '保存 PDF'
    downloadBtn.setAttribute('aria-label', '保存 PDF')

    toolbarCenter.appendChild(prevPageBtn)
    toolbarCenter.appendChild(pageIndicator)
    toolbarCenter.appendChild(nextPageBtn)
    toolbarRight.appendChild(zoomOutBtn)
    toolbarRight.appendChild(zoomIndicator)
    toolbarRight.appendChild(zoomInBtn)
    toolbarRight.appendChild(fitWidthBtn)
    toolbarRight.appendChild(downloadBtn)

    wrap.appendChild(toolbar)
    toolbar.appendChild(toolbarLeft)
    toolbar.appendChild(toolbarCenter)
    toolbar.appendChild(toolbarRight)
    viewportHost.appendChild(pagesHost)
    wrap.appendChild(viewportHost)

    media.innerHTML = ''
    media.appendChild(wrap)
    media.style.display = 'flex'

    try {
      const loadingTask = pdfjsLib.getDocument({ data: bytes })
      const pdfDoc = await loadingTask.promise
      const availableWidth = Math.max(640, Math.floor((media.clientWidth || media.getBoundingClientRect().width || 900) - 40))
      viewportHost.setAttribute('data-total-pages', String(pdfDoc.numPages))
      viewportHost.setAttribute('data-current-page', '1')
      viewportHost.setAttribute('data-scale', '1')
      queueMicrotask(() => {
        pageIndicator.textContent = `页 1 / ${pdfDoc.numPages}`
      })
      pageIndicator.textContent = `页 1 / ${pdfDoc.numPages}`
      pageIndicator.textContent = `页 1 / ${pdfDoc.numPages}`

      for (let pageNumber = 1; pageNumber <= pdfDoc.numPages; pageNumber += 1) {
        if (media.getAttribute('data-render-token') !== renderToken) {
          try { await loadingTask.destroy() } catch { }
          return
        }

        const page = await pdfDoc.getPage(pageNumber)
        const baseViewport = page.getViewport({ scale: 1 })
        const scale = Math.max(0.5, availableWidth / Math.max(1, baseViewport.width))
        const viewport = page.getViewport({ scale })

        const pageWrap = document.createElement('div')
        pageWrap.className = 'guaxi-pdf-page-card'
        pageWrap.setAttribute('data-page-number', String(pageNumber))
        pageWrap.setAttribute('data-base-width', String(Math.ceil(viewport.width)))
        pageWrap.style.width = `${Math.ceil(viewport.width)}px`
        pageWrap.style.maxWidth = '100%'

        const pageLabel = document.createElement('div')
        pageLabel.className = 'guaxi-pdf-page-label'
        pageLabel.textContent = `第 ${pageNumber} / ${pdfDoc.numPages} 页`

        const canvas = document.createElement('canvas')
        canvas.className = 'guaxi-pdf-page-canvas'
        const context = canvas.getContext('2d', { alpha: false })
        if (!context) throw new Error('无法创建 PDF 渲染画布')
        canvas.width = Math.ceil(viewport.width)
        canvas.height = Math.ceil(viewport.height)
        canvas.setAttribute('data-base-width', String(Math.ceil(viewport.width)))
        canvas.setAttribute('data-base-height', String(Math.ceil(viewport.height)))
        canvas.style.width = `${Math.ceil(viewport.width)}px`
        canvas.style.height = `${Math.ceil(viewport.height)}px`

        pageWrap.appendChild(canvas)
        pagesHost.appendChild(pageWrap)

        await page.render({ canvasContext: context, viewport }).promise
      }

      pageIndicator.textContent = `页 1 / ${pdfDoc.numPages}`

      try {
        const observer = new IntersectionObserver((entries) => {
          let bestEntry = null
          for (const entry of entries) {
            if (!entry.isIntersecting) continue
            if (!bestEntry || entry.intersectionRatio > bestEntry.intersectionRatio) {
              bestEntry = entry
            }
          }
          if (!bestEntry) return

          const pageNumber = Number(bestEntry.target?.getAttribute?.('data-page-number') || '1')
          viewportHost.setAttribute('data-current-page', String(pageNumber))
          pageIndicator.textContent = `页 ${pageNumber} / ${pdfDoc.numPages}`
          queueMicrotask(() => {
            pageIndicator.textContent = `页 ${pageNumber} / ${pdfDoc.numPages}`
          })
          pageIndicator.textContent = `页 ${pageNumber} / ${pdfDoc.numPages}`
        }, {
          root: viewportHost,
          threshold: [0.35, 0.55, 0.8],
        })

        Array.from(pagesHost.querySelectorAll('[data-page-number]')).forEach((el) => observer.observe(el))
      } catch (observerError) {
        logger.warn('[CodeCell] pdf page observer failed', observerError)
      }
    } catch (error) {
      logger.error('[CodeCell] pdfjs render failed', error)
      pagesHost.innerHTML = ''
      const fallback = document.createElement('div')
      fallback.className = 'guaxi-pdf-preview-fallback'
      fallback.textContent = `PDF 预览渲染失败：${String(error?.message || error)}`
      pagesHost.appendChild(fallback)
    }
  }

  // 统一释放预览区里为 PDF 预览创建的 blob URL，避免组件卸载后继续占用内存。
  // 这里使用函数声明而不是 const，确保在 script setup 的 onUnmounted 提前注册时也能安全调用。
  function cleanupPreviewPdfBlobUrls() {
    const container = previewContainer.value
    if (!(container instanceof HTMLElement)) return

    const medias = Array.from(container.querySelectorAll('.guaxi-codecell-media[data-blob-url]'))
    medias.forEach((media) => {
      const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
      if (!oldUrl) return
      try {
        URL.revokeObjectURL(oldUrl)
      } catch (error) {
        logger.warn('[CodeCell] revoke preview pdf blob failed', error)
      }
      media.removeAttribute('data-blob-url')
    })

    logger.info('[CodeCell] preview pdf blob urls cleaned', { count: medias.length })
  }

  const renderCodeCellArtifacts = async (cell, outputPre, res, outputText, kind) => {
    const pdf = getPdfArtifactFromCodeCell(res)
    const shouldHideText = String(outputText || '').trim().length === 0

    if (!pdf) {
      const media = cell?.querySelector?.('.guaxi-codecell-media')
      if (media) {
        const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
        if (oldUrl) {
          try { URL.revokeObjectURL(oldUrl) } catch { }
          media.removeAttribute('data-blob-url')
        }

        media.removeAttribute('data-render-token')
        media.innerHTML = ''
        media.style.display = 'none'
      }
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    const media = ensureCodeCellMediaContainer(cell, outputPre)
    if (!media) return

    const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
    if (oldUrl) {
      try { URL.revokeObjectURL(oldUrl) } catch { }
      media.removeAttribute('data-blob-url')
    }

    const bytes = base64ToUint8Array(pdf.dataBase64)
    const blob = new Blob([bytes], { type: pdf.mime })
    const blobUrl = URL.createObjectURL(blob)
    media.setAttribute('data-blob-url', blobUrl)

    media.style.padding = '0'
    media.style.alignItems = 'stretch'
    media.style.justifyContent = 'stretch'
    media.style.overflow = 'hidden'

    await renderPdfArtifactWithPdfJs(media, pdf, bytes, blobUrl)

    try {
      message.info('PDF 已在下方渲染，可滚动预览或下载保存')
    } catch {
    }

    if (outputPre && shouldHideText) {
      outputPre.style.display = 'none'
    }

    logger.info('[CodeCell] pdf rendered in output area', { kind: String(kind || ''), name: pdf.name, bytes: bytes.length })
  }

  const getGuaxiDisplayFromResult = (res) => {
    const rj = res?.result_json
    if (!rj || typeof rj !== 'object') return null

    const display = rj?.__guaxi_display__ || rj?.guaxi_display || null
    if (!display || typeof display !== 'object') return null

    const t = String(display?.type || '').trim().toLowerCase()
    if (!['image', 'html', 'svg', 'markdown'].includes(t)) return null

    const mime = String(display?.mime || '').trim() || (t === 'svg' ? 'image/svg+xml' : t === 'html' ? 'text/html' : t === 'markdown' ? 'text/markdown' : 'image/png')
    const dataUrl = String(display?.data_url || '').trim()
    const dataBase64 = String(display?.data_base64 || '').trim()
    const data = display?.data

    return {
      type: t,
      mime,
      dataUrl,
      dataBase64,
      data: typeof data === 'string' ? data : null,
      html: typeof display?.html === 'string' ? display.html : '',
      svg: typeof display?.svg === 'string' ? display.svg : '',
      markdown: typeof display?.markdown === 'string' ? display.markdown : '',
    }
  }

  const getGuaxiDataFrameFromResult = (res) => {
    const payload = res?.result_json?.__guaxi_dataframe__
    if (!payload || typeof payload !== 'object') return null
    const columns = Array.isArray(payload.columns) ? payload.columns.map((item) => String(item ?? '')) : []
    const rows = Array.isArray(payload.rows) ? payload.rows.map((row) => Array.isArray(row) ? row : []) : []
    return {
      columns,
      rows,
      rowCount: Number(payload.row_count || rows.length || 0),
      truncated: !!payload.truncated,
      dtypes: payload.dtypes && typeof payload.dtypes === 'object' ? payload.dtypes : {},
      sourcePath: String(payload.source_path || '').trim(),
    }
  }

  const dataframeToDelimitedText = (table, delimiter = ',') => {
    const escapeField = (value) => {
      const text = value == null ? '' : String(value)
      if (text.includes('"') || text.includes('\n') || text.includes('\r') || text.includes(delimiter)) {
        return `"${text.replace(/"/g, '""')}"`
      }
      return text
    }
    const lines = []
    lines.push((table.columns || []).map(escapeField).join(delimiter))
    for (const row of table.rows || []) {
      lines.push((Array.isArray(row) ? row : []).map(escapeField).join(delimiter))
    }
    return lines.join('\n')
  }

  const saveDataFrameToFile = async (table, format = 'csv') => {
    const normalized = String(format || 'csv').trim().toLowerCase()
    const defaultName = `table-output.${normalized === 'tsv' ? 'tsv' : normalized === 'json' ? 'json' : 'csv'}`
    try {
      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{
          name: 'Table',
          extensions: [normalized === 'tsv' ? 'tsv' : normalized === 'json' ? 'json' : 'csv'],
        }],
      })
      if (!outputPath) return false

      let text = ''
      if (normalized === 'json') {
        text = JSON.stringify((table.rows || []).map((row) => {
          const item = {}
          ;(table.columns || []).forEach((column, index) => {
            item[column] = Array.isArray(row) ? row[index] ?? null : null
          })
          return item
        }), null, 2)
      } else {
        text = dataframeToDelimitedText(table, normalized === 'tsv' ? '\t' : ',')
      }

      const { writeFile } = await import('@tauri-apps/plugin-fs')
      await writeFile(outputPath, new TextEncoder().encode(text))
      logger.info('[PyCell] dataframe exported', { outputPath, format: normalized, rows: table.rows?.length || 0 })
      message.success('表格已导出')
      return true
    } catch (error) {
      logger.error('[PyCell] dataframe export failed', error)
      message.error(`表格导出失败：${String(error?.message || error)}`)
      return false
    }
  }

  const createDataFramePreviewCard = (table, { onImport, onExport } = {}) => {
    const wrap = document.createElement('div')
    wrap.className = 'guaxi-dataframe-card'

    const toolbar = document.createElement('div')
    toolbar.className = 'guaxi-dataframe-toolbar'

    const meta = document.createElement('div')
    meta.className = 'guaxi-dataframe-meta'
    const truncatedText = table.truncated ? ' · 已截断预览' : ''
    const sourceText = table.sourcePath ? ` · ${table.sourcePath.split(/[\\/]/).pop()}` : ''
    meta.textContent = `${table.rowCount} 行 × ${(table.columns || []).length} 列${truncatedText}${sourceText}`

    const actions = document.createElement('div')
    actions.className = 'guaxi-dataframe-actions'

    const createButton = (label, handler) => {
      const btn = document.createElement('button')
      btn.type = 'button'
      btn.className = 'guaxi-dataframe-btn'
      btn.textContent = label
      btn.addEventListener('click', (event) => {
        event.preventDefault()
        event.stopPropagation()
        void handler?.()
      })
      return btn
    }

    if (typeof onImport === 'function') actions.appendChild(createButton('导入文件', onImport))
    if (typeof onExport === 'function') actions.appendChild(createButton('导出 CSV', () => onExport?.('csv')))
    if (typeof onExport === 'function') actions.appendChild(createButton('导出 JSON', () => onExport?.('json')))
    toolbar.appendChild(meta)
    toolbar.appendChild(actions)

    const viewport = document.createElement('div')
    viewport.className = 'guaxi-dataframe-table-wrap'

    const tableEl = document.createElement('table')
    tableEl.className = 'guaxi-dataframe-table'

    const thead = document.createElement('thead')
    const headerRow = document.createElement('tr')
    ;(table.columns || []).forEach((column) => {
      const th = document.createElement('th')
      th.innerHTML = escapeHtml(column)
      headerRow.appendChild(th)
    })
    thead.appendChild(headerRow)

    const tbody = document.createElement('tbody')
    ;(table.rows || []).forEach((row) => {
      const tr = document.createElement('tr')
      ;(table.columns || []).forEach((_, index) => {
        const td = document.createElement('td')
        const value = Array.isArray(row) ? row[index] : ''
        td.innerHTML = escapeHtml(value == null ? '' : value)
        tr.appendChild(td)
      })
      tbody.appendChild(tr)
    })

    tableEl.appendChild(thead)
    tableEl.appendChild(tbody)
    viewport.appendChild(tableEl)
    wrap.appendChild(toolbar)
    wrap.appendChild(viewport)
    return wrap
  }

  const ensurePyCellMediaContainer = (cell, afterEl) => {
    if (!(cell instanceof Element)) return null

    let media = cell.querySelector('.guaxi-pycell-media')
    if (media) return media

    media = document.createElement('div')
    media.className = 'guaxi-pycell-media'
    media.style.marginTop = '8px'
    media.style.display = 'none'
    media.style.flexDirection = 'column'
    media.style.alignItems = 'center'
    media.style.justifyContent = 'center'

    const border = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-border') || '#e1e4e8'
    const panelBg = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-panel-bg') || '#fff'

    media.style.border = `1px solid ${String(border).trim() || '#e1e4e8'}`
    media.style.borderRadius = '8px'
    media.style.padding = '10px'
    media.style.background = String(panelBg).trim() || '#fff'

    if (afterEl && afterEl.parentNode) {
      afterEl.parentNode.insertBefore(media, afterEl.nextSibling)
    } else {
      cell.appendChild(media)
    }

    return media
  }

  const createPyCellModuleShell = (module) => {
    const shell = document.createElement('div')
    shell.className = 'guaxi-pycell-output-module'
    shell.setAttribute('data-pycell-module-id', String(module.id || ''))
    shell.setAttribute('data-pycell-module-type', String(module.type || 'display'))

    const toolbar = document.createElement('div')
    toolbar.className = 'guaxi-pycell-output-module-toolbar'

    const meta = document.createElement('div')
    meta.className = 'guaxi-pycell-output-module-meta'
    meta.textContent = module.meta ? `${module.title} · ${module.meta}` : module.title

    const btn = document.createElement('button')
    btn.type = 'button'
    btn.className = 'guaxi-pycell-output-module-btn'
    btn.setAttribute('data-guaxi-py-action', 'toggle-output-module')
    btn.setAttribute('data-guaxi-py-module-id', String(module.id || ''))
    btn.setAttribute('title', '隐藏该输出模块')
    btn.textContent = '▾ 模块'

    const body = document.createElement('div')
    body.className = 'guaxi-pycell-output-module-body'

    toolbar.appendChild(meta)
    toolbar.appendChild(btn)
    shell.appendChild(toolbar)
    shell.appendChild(body)
    return { shell, body, btn }
  }

  const renderPyCellModuleBody = (body, module) => {
    body.innerHTML = ''

    if (module.type === 'dataframe' && module.table) {
      body.appendChild(createDataFramePreviewCard(module.table, {
        onImport: async () => {},
        onExport: async (format) => {
          await saveDataFrameToFile(module.table, format)
        },
      }))
      return true
    }

    const display = module.display
    if (!display || typeof display !== 'object') return false

    if (display.type === 'html' && display.html) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-html-display'
      box.innerHTML = display.html
      body.appendChild(box)
      return true
    }

    if (display.type === 'svg' && display.svg) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-svg-display'
      box.innerHTML = display.svg
      body.appendChild(box)
      return true
    }

    if (display.type === 'markdown' && display.markdown) {
      const box = document.createElement('pre')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-markdown-display'
      box.textContent = display.markdown
      body.appendChild(box)
      return true
    }

    const img = document.createElement('img')
    img.style.maxWidth = '100%'
    img.style.height = 'auto'
    img.style.display = 'block'
    img.style.marginLeft = 'auto'
    img.style.marginRight = 'auto'

    let src = ''
    if (display.dataUrl && display.dataUrl.startsWith('data:')) src = display.dataUrl
    else if (display.dataBase64) src = `data:${display.mime};base64,${display.dataBase64}`
    else if (display.data != null) src = `data:${display.mime},${encodeURIComponent(display.data)}`
    if (!src) return false

    img.src = src
    body.appendChild(img)
    return true
  }

  const applyPyCellModuleVisibility = (cellId, media) => {
    if (!cellId || !(media instanceof HTMLElement)) return
    const snapshot = getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)
    const collapsedMap = snapshot?.meta?.moduleCollapsedMap && typeof snapshot.meta.moduleCollapsedMap === 'object'
      ? snapshot.meta.moduleCollapsedMap
      : {}

    media.querySelectorAll?.('.guaxi-pycell-output-module')?.forEach?.((moduleEl) => {
      const moduleId = String(moduleEl.getAttribute('data-pycell-module-id') || '').trim()
      const body = moduleEl.querySelector('.guaxi-pycell-output-module-body')
      const btn = moduleEl.querySelector('[data-guaxi-py-action="toggle-output-module"]')
      const collapsed = !!collapsedMap[moduleId]
      if (body) body.style.display = collapsed ? 'none' : ''
      if (btn) {
        btn.textContent = collapsed ? '▸ 模块' : '▾ 模块'
        btn.setAttribute('title', collapsed ? '显示该输出模块' : '隐藏该输出模块')
      }
    })
  }

  const renderPyCellDisplay = (cell, outputPre, res, outputText, { cellId = '', modules = null } = {}) => {
    const shouldHideText = String(outputText || '').trim().length === 0
    const resolvedModules = (Array.isArray(modules) ? modules : buildPyCellOutputModules(res))
      .map((module) => normalizePyCellOutputModule(module))
      .filter(Boolean)

    ensurePyCellTextOutputToggle(cell, outputPre, cellId)

    const media = ensurePyCellMediaContainer(cell, outputPre)
    if (!media) return

    if (!resolvedModules.length) {
      media.innerHTML = ''
      media.style.display = 'none'
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    media.innerHTML = ''
    resolvedModules.forEach((module) => {
      const { shell, body } = createPyCellModuleShell(module)
      const ok = renderPyCellModuleBody(body, module)
      if (ok) media.appendChild(shell)
    })
    media.style.display = media.childNodes.length > 0 ? 'flex' : 'none'
    applyPyCellModuleVisibility(cellId, media)

    logger.info('[PyCell] rendered output modules', {
      cellId,
      moduleCount: resolvedModules.length,
      hasTextOutput: !shouldHideText,
    })

    if (outputPre && shouldHideText) outputPre.style.display = 'none'
  }

  const upsertPythonOutputBlock = (markdown, startLine, cellId, outputText) => {
    const marker = `<!-- guaxi:pyout:${cellId} -->`
    const outputFenceStart = '```text'
    const outputFenceEnd = '```'

    const lines = String(markdown || '').split(/\r?\n|\r/)

    const findOpenFenceFrom = (idx) => {
      for (let i = Math.max(0, idx); i < lines.length; i += 1) {
        const t = String(lines[i] || '').trim()
        if (/^```\s*(python|py)(\s|$)/i.test(t)) return i
      }
      return -1
    }

    const openIdx = Number.isFinite(startLine)
      ? findOpenFenceFrom(Math.max(0, startLine - 1))
      : findOpenFenceFrom(0)

    if (openIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    let closeIdx = -1
    for (let i = openIdx + 1; i < lines.length; i += 1) {
      if (String(lines[i] || '').trim() === '```') {
        closeIdx = i
        break
      }
    }

    if (closeIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    const markerIdx = lines.findIndex((l) => String(l || '').trim() === marker)
    if (markerIdx >= 0) {
      let fenceStartIdx = -1
      for (let i = markerIdx + 1; i < lines.length; i += 1) {
        if (String(lines[i] || '').trim().startsWith('```')) {
          fenceStartIdx = i
          break
        }
      }
      if (fenceStartIdx >= 0) {
        let fenceEndIdx = -1
        for (let i = fenceStartIdx + 1; i < lines.length; i += 1) {
          if (String(lines[i] || '').trim() === '```') {
            fenceEndIdx = i
            break
          }
        }
        if (fenceEndIdx >= 0) {
          lines.splice(markerIdx, fenceEndIdx - markerIdx + 1, marker, outputFenceStart, outputText, outputFenceEnd)
          return `${lines.join('\n')}\n`
        }
      }
    }

    lines.splice(closeIdx + 1, 0, '', marker, outputFenceStart, outputText, outputFenceEnd)
    return `${lines.join('\n')}\n`
  }

  const upsertCodeCellOutputBlock = (markdown, startLine, cellId, kind, outputText) => {
    const marker = `<!-- guaxi:cellout:${cellId} -->`
    const outputFenceStart = '```text'
    const outputFenceEnd = '```'

    const lines = String(markdown || '').split(/\r?\n|\r/)

    const kindToFenceRe = (k) => {
      const t = String(k || '').trim().toLowerCase()
      if (t === 'javascript') return /^(javascript|js)(\s|$)/i
      if (t === 'latex') return /^(latex|tex)(\s|$)/i
      if (t === 'c') return /^c(\s|$)/i
      if (t === 'cpp') return /^(cpp|c\+\+)(\s|$)/i
      if (t === 'rust') return /^(rust|rs)(\s|$)/i
      return null
    }

    const fenceRe = kindToFenceRe(kind)

    const findOpenFenceFrom = (idx) => {
      for (let i = Math.max(0, idx); i < lines.length; i += 1) {
        const t = String(lines[i] || '').trim()
        if (!t.startsWith('```')) continue
        const info = t.replace(/^```\s*/, '')
        if (fenceRe ? fenceRe.test(info) : true) return i
      }
      return -1
    }

    const openIdx = Number.isFinite(startLine)
      ? findOpenFenceFrom(Math.max(0, startLine - 1))
      : findOpenFenceFrom(0)

    if (openIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    let closeIdx = -1
    for (let i = openIdx + 1; i < lines.length; i += 1) {
      if (String(lines[i] || '').trim() === '```') {
        closeIdx = i
        break
      }
    }

    if (closeIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    const markerIdx = lines.findIndex((l) => String(l || '').trim() === marker)
    if (markerIdx >= 0) {
      let fenceStartIdx = -1
      for (let i = markerIdx + 1; i < lines.length; i += 1) {
        if (String(lines[i] || '').trim().startsWith('```')) {
          fenceStartIdx = i
          break
        }
      }
      if (fenceStartIdx >= 0) {
        let fenceEndIdx = -1
        for (let i = fenceStartIdx + 1; i < lines.length; i += 1) {
          if (String(lines[i] || '').trim() === '```') {
            fenceEndIdx = i
            break
          }
        }
        if (fenceEndIdx >= 0) {
          lines.splice(markerIdx, fenceEndIdx - markerIdx + 1, marker, outputFenceStart, outputText, outputFenceEnd)
          return `${lines.join('\n')}\n`
        }
      }
    }

    lines.splice(closeIdx + 1, 0, '', marker, outputFenceStart, outputText, outputFenceEnd)
    return `${lines.join('\n')}\n`
  }

  const applyMarkdownReplaceFromPreview = (nextMarkdown, { action = 'pycell-run-save' } = {}) => {
    const targetEditor = editor
    const model = targetEditor?.getModel?.()

    if (targetEditor && model) {
      try {
        const selection = targetEditor.getSelection?.()
        targetEditor.executeEdits(action, [
          {
            range: model.getFullModelRange(),
            text: String(nextMarkdown || ''),
            forceMoveMarkers: true
          }
        ])
        if (selection) targetEditor.setSelection(selection)
        return true
      } catch (err) {
        logger.warn('[PyCell] executeEdits failed, fallback to updateEditorContent', err)
      }
    }

    updateEditorContent(String(nextMarkdown || ''))
    return true
  }

  const pyCellBtn = base.closest?.('[data-guaxi-py-action]')
  if (pyCellBtn && previewContainer.value && previewContainer.value.contains(pyCellBtn)) {
    const action = String(pyCellBtn.getAttribute('data-guaxi-py-action') || '').trim()
    const cell = pyCellBtn.closest?.('.guaxi-pycell')
    if (!cell) return

    ensurePyCellToolbarEnhancements(cell)

    const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
    const outputPre = outputEl ? outputEl.closest?.('pre') : null
    const toggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-output"]')
    const sessionBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-session"]')
    const docPath = selectedFile.value?.path || ''
    const sessionId = buildPyCellSessionId(docPath)
    const cellId = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-cell'))
    ensurePyCellTextOutputToggle(cell, outputPre, cellId)

    const syncPyCellButtons = () => {
      const media = cell.querySelector?.('.guaxi-pycell-media')
      const panel = cell.querySelector?.('.guaxi-pycell-session')
      const textToggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-text-output"]')
      const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
      const mediaVisible = media ? String(media.style.display || '').trim() !== 'none' : false
      const panelVisible = panel ? String(panel.style.display || '').trim() !== 'none' : false
      if (toggleBtn) toggleBtn.textContent = textVisible || mediaVisible ? '隐藏结果' : '显示结果'
      if (sessionBtn) sessionBtn.textContent = panelVisible ? '隐藏变量' : '会话变量'
      if (textToggleBtn) {
        textToggleBtn.textContent = textVisible ? '▾ 文本' : '▸ 文本'
        textToggleBtn.setAttribute('title', textVisible ? '隐藏文本输出' : '显示文本输出')
      }
    }

    if (action === 'toggle-text-output') {
      if (!outputPre) return
      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const nextVisible = String(outputPre.style.display || '').trim() === 'none'
      outputPre.style.display = nextVisible && hasText ? '' : 'none'
      pyCellBtn.textContent = nextVisible && hasText ? '▾' : '▸'
      pyCellBtn.setAttribute('title', nextVisible && hasText ? '隐藏文本输出' : '显示文本输出')
      logger.info('[PyCell] toggled text output visibility', { cellId, nextVisible, hasText })
      patchSharedPythonCellState(cellId, {
        meta: {
          ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
          textOutputCollapsed: !(nextVisible && hasText),
        },
      })
      syncPyCellButtons()
      scheduleRefreshPythonCellManagers()
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'toggle-output-module') {
      const moduleId = String(pyCellBtn.getAttribute('data-guaxi-py-module-id') || '').trim()
      const moduleEl = moduleId
        ? cell.querySelector?.(`.guaxi-pycell-output-module[data-pycell-module-id="${moduleId}"]`)
        : pyCellBtn.closest?.('.guaxi-pycell-output-module')
      const body = moduleEl?.querySelector?.('.guaxi-pycell-output-module-body')
      if (!moduleId || !body) return

      const nextVisible = String(body.style.display || '').trim() === 'none'
      body.style.display = nextVisible ? '' : 'none'
      pyCellBtn.textContent = nextVisible ? '▾' : '▸'
      pyCellBtn.setAttribute('title', nextVisible ? '隐藏该输出模块' : '显示该输出模块')

      const snapshot = getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)
      const moduleCollapsedMap = {
        ...((snapshot?.meta?.moduleCollapsedMap && typeof snapshot.meta.moduleCollapsedMap === 'object')
          ? snapshot.meta.moduleCollapsedMap
          : {}),
        [moduleId]: !nextVisible,
      }

      logger.info('[PyCell] toggled output module visibility', { cellId, moduleId, nextVisible })
      patchSharedPythonCellState(cellId, {
        meta: {
          ...(snapshot?.meta || {}),
          moduleCollapsedMap,
        },
      })
      syncPyCellButtons()
      scheduleRefreshPythonCellManagers()
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'toggle-output') {
      const media = cell.querySelector?.('.guaxi-pycell-media')
      if (!outputPre && !media) {
        message.warning('未找到运行结果区域')
        e.stopPropagation()
        return
      }

      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const hasMedia = !!media && media.childNodes && media.childNodes.length > 0
      const currentlyVisible = (outputPre && String(outputPre.style.display || '').trim() !== 'none')
        || (media && String(media.style.display || '').trim() !== 'none')
      const nextShow = !currentlyVisible
      if (outputPre) outputPre.style.display = nextShow && hasText ? '' : 'none'
      if (media) media.style.display = nextShow && hasMedia ? 'flex' : 'none'
      syncPyCellButtons()
      patchSharedPythonCellState(cellId, {
        meta: {
          ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
          outputCollapsed: !nextShow,
        },
      })
      scheduleRefreshPythonCellManagers()
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'toggle-session') {
      const anchor = cell.querySelector?.('.guaxi-pycell-media') || outputPre
      const panel = ensurePyCellSessionContainer(cell, anchor)
      if (!panel) return
      const nextVisible = String(panel.style.display || '').trim() === 'none'
      panel.style.display = nextVisible ? 'block' : 'none'
      if (nextVisible) panel.setAttribute('data-force-visible', 'true')
      else panel.removeAttribute('data-force-visible')
      if (!panel.innerHTML.trim()) {
        renderPyCellSessionState(cell, { session_id: sessionId, session_vars: [] }, { forceVisible: nextVisible })
      }
      syncPyCellButtons()
      patchSharedPythonCellState(cellId, {
        meta: {
          ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
          sessionCollapsed: !nextVisible,
        },
      })
      scheduleRefreshPythonCellManagers()
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'restart-session') {
      try {
        await invoke('python_reset_repl')
        if (outputEl) outputEl.textContent = ''
        if (outputPre) outputPre.style.display = 'none'
        const media = cell.querySelector?.('.guaxi-pycell-media')
        if (media) {
          media.innerHTML = ''
          media.style.display = 'none'
        }
        clearPyCellSessionPanel(cell)
        renderPyCellSessionState(cell, { session_id: sessionId, session_vars: [] }, { forceVisible: true })
        message.success('Python 会话已重启')
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText: '',
          display: null,
          dataframe: null,
          outputModules: [],
          result: { session_id: sessionId, session_vars: [] },
          sessionVars: [],
        })
        scheduleRefreshPythonCellManagers()
      } catch (err) {
        logger.error('[PyCell] reset failed', err)
        message.error(`重启 Python 会话失败：${String(err?.message || err)}`)
      }
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'inject-note') {
      const noteText = String(editor?.getModel?.()?.getValue?.() || rawContent.value || '')
      const code = [
        `_note_path = ${JSON.stringify(String(docPath))}`,
        `_note_text = ${JSON.stringify(noteText)}`,
        '_note_lines = _note_text.splitlines()',
        '_result = {"note_path": _note_path, "line_count": len(_note_lines), "char_count": len(_note_text)}',
      ].join('\n')

      try {
        const res = await invoke('python_exec', {
          request: {
            code,
            persist: true,
            expect_json: true,
            vars: {
              _guaxi_session: sessionId,
              _doc_path: String(docPath),
              _py_cell_id: String(cellId || ''),
            },
          },
        })
        renderPyCellSessionState(cell, res, { forceVisible: true })
        syncPyCellButtons()
        message.success('当前笔记已注入到 Python 会话')
      } catch (err) {
        logger.error('[PyCell] inject note failed', err)
        message.error(`注入笔记失败：${String(err?.message || err)}`)
      }
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'import-table') {
      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-py-action]') || [])
      try {
        setPyCellBusyState(cell, true)
        relatedBtns.forEach((b) => {
          try {
            b.setAttribute('disabled', 'true')
            b.style.opacity = '0.6'
            b.style.cursor = 'not-allowed'
          } catch {
          }
        })
        const selected = await open({
          multiple: false,
          filters: [{
            name: 'Table',
            extensions: ['csv', 'tsv', 'json', 'xlsx', 'xls', 'parquet'],
          }],
        })
        if (!selected || Array.isArray(selected)) {
          e.preventDefault()
          e.stopPropagation()
          return
        }

        const selectedPath = String(selected)
        const fileName = selectedPath.split(/[\\/]/).pop() || 'df'
        const variableName = sanitizePythonIdentifier(fileName, 'df')
        const code = [
          'from pathlib import Path',
          `__guaxi_table_path = ${JSON.stringify(selectedPath)}`,
          `__guaxi_var_name = ${JSON.stringify(variableName)}`,
          '__guaxi_ext = Path(__guaxi_table_path).suffix.lower()',
          'try:',
          '    import pandas as pd',
          'except Exception:',
          '    pd = None',
          'if pd is not None:',
          "    if __guaxi_ext in ('.csv', '.txt'):",
          '        __guaxi_value = pd.read_csv(__guaxi_table_path)',
          "    elif __guaxi_ext == '.tsv':",
          "        __guaxi_value = pd.read_csv(__guaxi_table_path, sep='\\t')",
          "    elif __guaxi_ext in ('.xlsx', '.xls'):",
          '        __guaxi_value = pd.read_excel(__guaxi_table_path)',
          "    elif __guaxi_ext == '.json':",
          '        __guaxi_value = pd.read_json(__guaxi_table_path)',
          "    elif __guaxi_ext == '.parquet':",
          '        __guaxi_value = pd.read_parquet(__guaxi_table_path)',
          '    else:',
          "        raise ValueError(f'Unsupported table format: {__guaxi_ext or __guaxi_table_path}')",
          'else:',
          "    if __guaxi_ext in ('.csv', '.txt', '.tsv'):",
          '        import csv',
          "        with open(__guaxi_table_path, 'r', encoding='utf-8-sig', newline='') as __guaxi_f:",
          "            __guaxi_value = list(csv.DictReader(__guaxi_f, delimiter='\\t' if __guaxi_ext == '.tsv' else ','))",
          "    elif __guaxi_ext == '.json':",
          '        import json',
          "        with open(__guaxi_table_path, 'r', encoding='utf-8') as __guaxi_f:",
          '            __guaxi_value = json.load(__guaxi_f)',
          '    else:',
          "        raise ValueError('当前运行环境需要 pandas 才能导入这种文件类型')",
          'globals()[__guaxi_var_name] = __guaxi_value',
          '_result = __guaxi_value',
        ].join('\n')

        const res = await invoke('python_exec', {
          request: {
            code,
            persist: true,
            expect_json: true,
            vars: {
              _guaxi_session: sessionId,
              _doc_path: String(docPath),
              _py_cell_id: String(cellId || ''),
            },
          },
        })
        // 设计说明：
        // 1. “导入表格”是一次数据装载操作，不属于用户显式希望看到的标准输出。
        // 2. 这里保留 dataframe/display 模块和 session 变量刷新，但不再额外制造文本输出，
        //    这样导入后的 cell 会稳定保留表格模块，而不会出现“已导入到变量 xxx”的冗余提示。
        const outputText = ''
        const nextModules = mergePyCellOutputModules(cellId, buildPyCellOutputModules(res))
        if (outputEl) outputEl.textContent = outputText
        if (outputPre) outputPre.style.display = String(outputText || '').trim().length > 0 ? '' : 'none'
        renderPyCellDisplay(cell, outputPre, res, outputText, { cellId, modules: nextModules })
        renderPyCellSessionState(cell, res, { forceVisible: true })
        logger.info('[PyCell] imported table modules updated', { cellId, moduleCount: nextModules.length })
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText,
          display: getGuaxiDisplayFromResult(res),
          dataframe: getGuaxiDataFrameFromResult(res),
          outputModules: nextModules,
          result: res,
          sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
          meta: {
            ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
            outputCollapsed: String(outputText || '').trim().length === 0,
            sessionCollapsed: false,
            textOutputCollapsed: String(outputText || '').trim().length === 0,
          },
        })
        scheduleRefreshPythonCellManagers()
        if (res?.ok === false) throw new Error(String(res?.stderr || res?.stdout || '导入失败'))
        logger.info('[PyCell] imported table into session', { cellId, variableName, selectedPath })
      } catch (err) {
        logger.error('[PyCell] import table failed', err)
        message.error(`导入表格失败：${String(err?.message || err)}`)
      } finally {
        setPyCellBusyState(cell, false)
        relatedBtns.forEach((b) => {
          try {
            b.removeAttribute('disabled')
            b.style.opacity = ''
            b.style.cursor = ''
          } catch {
          }
        })
      }
      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'run' || action === 'run-save') {
      const code = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-code'))
      const startLine = Number(cell.getAttribute('data-guaxi-md-start-line'))
      if (!code) {
        message.warning('代码为空')
        e.stopPropagation()
        return
      }

      if (outputPre) outputPre.style.display = ''
      if (outputEl) outputEl.textContent = '运行中...'
      syncPyCellButtons()

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-py-action]') || [])
      setPyCellBusyState(cell, true)
      relatedBtns.forEach((b) => {
        try {
          b.setAttribute('disabled', 'true')
          b.style.opacity = '0.6'
          b.style.cursor = 'not-allowed'
        } catch {
        }
      })

      try {
        const res = await invoke('python_exec', {
          request: {
            code,
            persist: true,
            expect_json: true,
            vars: {
              _guaxi_session: sessionId,
              _doc_path: String(docPath),
              _py_cell_id: String(cellId || ''),
            },
          },
        })

        const outputText = formatPythonExecOutput(res)
        const nextModules = mergePyCellOutputModules(cellId, buildPyCellOutputModules(res))
        const hideText = String(outputText || '').trim().length === 0
        if (outputPre) outputPre.style.display = hideText ? 'none' : ''
        if (outputEl) outputEl.textContent = hideText ? '' : outputText

        renderPyCellDisplay(cell, outputPre, res, outputText, { cellId, modules: nextModules })
        renderPyCellSessionState(cell, res)
        logger.info('[PyCell] run output modules updated', { cellId, moduleCount: nextModules.length, hideText })
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText: hideText ? '' : outputText,
          display: getGuaxiDisplayFromResult(res),
          dataframe: getGuaxiDataFrameFromResult(res),
          outputModules: nextModules,
          result: res,
          sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
          meta: {
            ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
            outputCollapsed: hideText,
            textOutputCollapsed: hideText,
          },
        })
        scheduleRefreshPythonCellManagers()

        if (res?.ok === false) {
          message.error('Python 执行失败')
        }

        if (action === 'run-save') {
          const hasTextOutput = String(outputText || '').trim().length > 0
          if (!hasTextOutput) {
            message.success('运行完成（无文字输出，未写回）')
          } else {
            suppressPreviewAutoRenderOnce(`pycell:${action}`, 1500)
            const current = String(editor?.getModel?.()?.getValue?.() || rawContent.value || '')
            const nextMarkdown = upsertPythonOutputBlock(
              current,
              Number.isFinite(startLine) ? startLine : null,
              cellId || 'pycell',
              outputText
            )
            applyMarkdownReplaceFromPreview(nextMarkdown, { action: 'pycell-run-save' })
            message.success('已写回到文档（未自动保存）')
          }
        } else {
          message.success('运行完成')
        }
      } catch (err) {
        logger.error('[PyCell] run failed', err)
        if (outputPre) outputPre.style.display = ''
        if (outputEl) outputEl.textContent = `运行失败：${String(err?.message || err)}`
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText: `运行失败：${String(err?.message || err)}`,
          display: null,
          dataframe: null,
          result: null,
          sessionVars: [],
        })
        scheduleRefreshPythonCellManagers()
        message.error('运行失败')
      } finally {
        setPyCellBusyState(cell, false)
        relatedBtns.forEach((b) => {
          try {
            b.removeAttribute('disabled')
            b.style.opacity = ''
            b.style.cursor = ''
          } catch {
          }
        })
      }

      e.preventDefault()
      e.stopPropagation()
      return
    }
  }

  // 可执行通用代码块：预览点击“运行/运行并写回/显示结果”
  const cellBtn = base.closest?.('[data-guaxi-cell-action]')
  if (cellBtn && previewContainer.value && previewContainer.value.contains(cellBtn)) {
    const action = String(cellBtn.getAttribute('data-guaxi-cell-action') || '').trim()

    if (action === 'toggle-output') {
      const cell = cellBtn.closest?.('.guaxi-codecell')
      if (!cell) return

      const outputEl = cell.querySelector?.('[data-guaxi-cell-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const media = cell.querySelector?.('.guaxi-codecell-media')

      if (!outputPre && !media) {
        message.warning('未找到运行结果区域')
        e.stopPropagation()
        return
      }

      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const hasMedia = !!media && media.childNodes && media.childNodes.length > 0

      const isVisible = (el) => {
        if (!el) return false
        return String(el.style.display || '').trim() !== 'none'
      }

      const currentlyVisible = isVisible(outputPre) || isVisible(media)
      const nextShow = !currentlyVisible

      if (outputPre) {
        outputPre.style.display = nextShow && hasText ? '' : 'none'
      }
      if (media) {
        media.style.display = nextShow && hasMedia ? 'flex' : 'none'
      }

      const anyVisibleAfter = isVisible(outputPre) || isVisible(media)
      cellBtn.textContent = anyVisibleAfter ? '隐藏结果' : '显示结果'

      logger.info('[CodeCell] toggle output', {
        nextShow,
        hasText,
        hasMedia,
        textVisible: isVisible(outputPre),
        mediaVisible: isVisible(media),
      })

      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'run' || action === 'run-save') {
      const cell = cellBtn.closest?.('.guaxi-codecell')
      if (!cell) return

      const kind = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-kind'))
      const code = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-code'))
      const cellId = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-id'))
      const startLine = Number(cell.getAttribute('data-guaxi-md-start-line'))

      if (!code) {
        message.warning('代码为空')
        e.stopPropagation()
        return
      }

      const outputEl = cell.querySelector?.('[data-guaxi-cell-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const toggleBtn = cell.querySelector?.('[data-guaxi-cell-action="toggle-output"]')
      const syncToggleText = () => {
        if (!toggleBtn) return
        const media = cell.querySelector?.('.guaxi-codecell-media')
        const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
        const mediaVisible = media ? String(media.style.display || '').trim() !== 'none' : false
        toggleBtn.textContent = textVisible || mediaVisible ? '隐藏结果' : '显示结果'
      }

      if (outputPre) {
        outputPre.style.display = ''
      }
      syncToggleText()
      if (outputEl) outputEl.textContent = '运行中...'

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-cell-action]') || [])
      relatedBtns.forEach((b) => {
        try {
          b.setAttribute('disabled', 'true')
          b.style.opacity = '0.6'
          b.style.cursor = 'not-allowed'
        } catch {
        }
      })

      try {
        const docPath = selectedFile.value?.path || ''

        logger.info('[CodeCell] code_cell_exec', {
          action,
          kind,
          cellId,
          codeLen: code.length,
          docPath,
        })

        const res = await invoke('code_cell_exec', {
          request: {
            kind: String(kind || '').trim(),
            code,
            vars: {
              _doc_path: String(docPath),
              _cell_id: String(cellId || ''),
            },
          }
        })

        logger.info('[CodeCell] code_cell_exec result', {
          ok: res?.ok,
          stdoutLen: String(res?.stdout || '').length,
          stderrLen: String(res?.stderr || '').length,
          hasArtifacts: Array.isArray(res?.artifacts) && res.artifacts.length > 0,
        })

        const outputText = formatCodeCellExecOutput(res, kind)
        const hideText = String(outputText || '').trim().length === 0

        if (outputPre) {
          outputPre.style.display = hideText ? 'none' : ''
        }
        syncToggleText()

        if (outputEl) {
          outputEl.textContent = hideText ? '' : outputText
        }

        await renderCodeCellArtifacts(cell, outputPre, res, outputText, kind)
        syncToggleText()

        if (res?.ok === false) {
          message.error('执行失败')
        }

        if (action === 'run-save') {
          suppressPreviewAutoRenderOnce(`codecell:${action}`, 1500)

          const current = String(editor?.getModel?.()?.getValue?.() || rawContent.value || '')
          const nextMarkdown = upsertCodeCellOutputBlock(
            current,
            Number.isFinite(startLine) ? startLine : null,
            cellId || `cell-${String(kind || 'code')}`,
            kind,
            outputText
          )

          applyMarkdownReplaceFromPreview(nextMarkdown, { action: 'codecell-run-save' })
          message.success('已写回到文档（未自动保存）')
        } else {
          message.success('运行完成')
        }
      } catch (err) {
        logger.error('[CodeCell] run failed', err)
        if (outputPre) {
          outputPre.style.display = ''
        }
        syncToggleText()
        if (outputEl) outputEl.textContent = `运行失败：${String(err?.message || err)}`
        message.error('运行失败')
      } finally {
        relatedBtns.forEach((b) => {
          try {
            b.removeAttribute('disabled')
            b.style.opacity = ''
            b.style.cursor = ''
          } catch {
          }
        })
      }

      e.preventDefault()
      e.stopPropagation()
      return
    }
  }

  // 任务列表：点击 checkbox 时，直接更新 Markdown 源内容（并仅更新当前条目 DOM）
  const checkbox = base.closest?.('input.task-list-item-checkbox');
  if (checkbox && previewContainer.value && previewContainer.value.contains(checkbox)) {
    const lineFromAttr = Number(checkbox.getAttribute('data-md-task-line') || 0);
    const lineFromParent = Number(
      checkbox.closest?.('[data-md-start-line]')?.getAttribute?.('data-md-start-line') || 0
    );
    const ln = lineFromAttr || lineFromParent || 0;
    const checked = !!checkbox.checked;

    if (!ln) {
      logger.warn('[TaskList] preview toggle ignored: missing line', { checked });
      return;
    }

    const ok = toggleMarkdownTaskAtLine(ln, checked, { source: 'preview' });

    // 仅更新被点击的条目：不依赖整篇 Markdown 重渲染
    const li = checkbox.closest?.('li.task-list-item');
    if (li) {
      li.classList.toggle('task-list-item-checked', checked);
      li.setAttribute('data-md-task', checked ? '1' : '0');
    }

    if (!ok) {
      logger.warn('[TaskList] preview toggle failed', { ln, checked });
    } else {
      logger.info('[TaskList] preview toggled (dom patched)', { ln, checked });
    }

    e.stopPropagation();
    return;
  }

  const a = base.closest('a');
  if (a && previewContainer.value && previewContainer.value.contains(a)) {
    const href0 = a.getAttribute('href') || '';
    const href = String(href0 || '').trim();
    const ctrl = !!(e?.ctrlKey || e?.metaKey);

    if (href) {
      if (href.toLowerCase() === '@ref') {
        e.preventDefault();
        e.stopPropagation();

        const text = String(a.textContent || '').trim();
        const m = text.match(/\d+/);
        const num = m ? m[0] : '';
        if (!num) {
          logger.debug('[Footnote] invalid ref click: missing number', { href, text });
          return;
        }

        jumpToFootnoteDefinition(num, { source: 'preview' });
        return;
      }

      if (href.toLowerCase().startsWith('guaxi://wikilink')) {
        e.preventDefault();
        e.stopPropagation();

        const baseDocPath = selectedFile.value?.path || fileTreeStore.activeTab || '';
        await openWikilinkFromHref(href, { baseDocPath, source: 'preview' });
        return;
      }

      if (href.startsWith('#')) {
        e.preventDefault();
        e.stopPropagation();

        recordPreviewLocation('anchor');

        const id = decodeURIComponent(href.slice(1));
        const targetLine = Number(a.getAttribute('data-md-start-line') || 0);
        const container = previewContainer.value;
        if (container) {
          const target = container.querySelector(`#${CSS.escape(id)}`);
          if (target) {
            scrollPreviewElementIntoView(container, target, { behavior: 'smooth' });
          } else if (targetLine > 0) {
            scrollPreviewToLine(targetLine, { behavior: 'smooth' });
          }
        }
        return;
      }

      if (isHttpUrl(href)) {
        e.preventDefault();
        e.stopPropagation();

        try {
          await openUrl(href);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href }, err);
          message.error('打开浏览器失败');
        }
        return;
      }

      e.preventDefault();
      e.stopPropagation();

      const { link, anchor } = splitFsLinkAndAnchor(href);
      if (!link) return;
      const baseDocPath = selectedFile.value?.path || fileTreeStore.activeTab || '';

      logger.debug('[LinkOpen] preview openFsLink', { link, anchor, ctrl, baseDocPath });
      await openFsLinkSmart(link, { anchor, viaCtrl: ctrl, baseDocPath });
      return;
    }
  }

  const hit = base.closest('[data-md-start-line]');
  if (!hit) return;

  const line = Number(hit.dataset.mdStartLine || 1);
  if (showEditor.value) {
    const shouldFocus = showEditor.value;
    syncEditorToLine(line, { focus: shouldFocus });
  }
};

const setMainMode = async (nextMode) => {
  if (nextMode !== 'edit' && nextMode !== 'preview') return;
  if (mode.value === nextMode) return;

  if (mode.value === 'edit' && nextMode === 'preview' && editor) {
    const pos = editor.getPosition();
    pendingPreviewScrollLine.value =
      (pos && pos.lineNumber) || editorStore.cursorPosition.lineNumber || 1;
  }

  captureScrollPosition();
  mode.value = nextMode;

  await nextTick();

  if (mode.value === 'edit') {
    if (!editor) {
      initEditor();
    }
    if (editor) {
      editor.layout();
    }
    restoreScrollPosition();
    return;
  }

  renderDebounced(rawContent.value);
  restoreScrollPosition();
};

// 防抖渲染
const rememberPreviewCacheEntry = (cache, key, value, limit = 100) => {
  if (!cache || !key) return
  if (cache.has(key)) cache.delete(key)
  cache.set(key, value)
  while (cache.size > Math.max(16, Number(limit || 0) || 0)) {
    const oldestKey = cache.keys().next().value
    if (!oldestKey) break
    cache.delete(oldestKey)
  }
}

const remapPreviewLineNumbers = (html, lineOffset = 0) => {
  const offset = Number(lineOffset || 0)
  if (!offset) return String(html || '')

  return String(html || '')
    .replace(/data-md-start-line="(\d+)"/g, (_m, line) => `data-md-start-line="${Math.max(1, Number(line || 0) + offset)}"`)
    .replace(/data-md-end-line="(\d+)"/g, (_m, line) => `data-md-end-line="${Math.max(1, Number(line || 0) + offset)}"`)
    .replace(/data-md-task-line="(\d+)"/g, (_m, line) => `data-md-task-line="${Math.max(1, Number(line || 0) + offset)}"`)
}

// 设计说明：
// 1. 文件卡片通过 innerHTML 注入标题、摘要和路径信息，因此这里必须做统一转义，避免预览增强引入 HTML 污染。
// 2. 单独定义在预览增强作用域，避免误引用 handlePreviewClick 内部的局部 escapeHtml。
const escapePreviewHtml = (value) => String(value ?? '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const buildPreviewSegmentCacheKey = (segment, options = {}) => {
  const theme = String(options.theme || '')
  const basePath = String(options.basePath || '')
  const filePath = String(options.filePath || '')
  const content = String(segment?.content || '')
  return `${theme}::${basePath}::${filePath}::${content}`
}

const buildPreviewDocumentCacheKey = (content, options = {}) => {
  const theme = String(options.theme || '')
  const basePath = String(options.basePath || '')
  const filePath = String(options.filePath || '')
  return `${theme}::${basePath}::${filePath}::${String(content || '')}`
}

// 设计说明：
// 1. [toc] / [[toc]] 这类目录标记依赖“整篇文档”的标题信息生成目录。
// 2. 一旦把文档切片后分别 renderMarkdown，目录插件就只能看到局部标题，最终会导致 TOC 缺失或不完整。
// 3. 因此预览阶段只要检测到目录标记，就回退到整篇渲染，优先保证正确性。
const hasPreviewTocMarker = (markdown = '') => /^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/im.test(String(markdown || ''))

// 设计说明：
// 1. 编辑器预览与探索者模式一样，Markdown 主体渲染是最耗时的一段，适合搬到 Worker 里做。
// 2. 主线程只保留 Mermaid 与数学节点的轻量收尾，这样输入时卡顿会明显下降。
// 3. Worker 不可用时自动回退到主线程，保证功能不退化。
const buildPreviewMarkdownRenderOptions = (options = {}) => ({
  basePath: options.basePath || '',
  mermaid: true,
  math: true,
  codeHighlight: true,
  pythonCells: true,
  codeCells: true,
  theme: options.theme || getMarkdownPreviewThemeName(),
})

const initPreviewMarkdownWorker = () => {
  if (previewMarkdownWorker) return previewMarkdownWorker
  try {
    const worker = new Worker(new URL('../../workers/notebookMarkdownRenderWorker.js', import.meta.url), {
      type: 'module',
    })
    worker.onmessage = (event) => {
      const payload = event?.data || {}
      const taskId = Number(payload?.id || 0)
      const task = previewMarkdownWorkerPendingTasks.get(taskId)
      if (!task) return
      previewMarkdownWorkerPendingTasks.delete(taskId)
      if (payload?.ok) {
        task.resolve(String(payload?.html || ''))
        return
      }
      task.reject(new Error(String(payload?.error || 'Preview markdown worker failed')))
    }
    worker.onerror = (error) => {
      previewMarkdownWorkerReady = false
      logger.error('[Preview] markdown worker runtime error', {
        error: String(error?.message || error),
      })
    }
    previewMarkdownWorker = worker
    previewMarkdownWorkerReady = true
    logger.info('[Preview] markdown worker initialized')
    return worker
  } catch (error) {
    previewMarkdownWorkerReady = false
    logger.warn('[Preview] failed to initialize markdown worker, fallback to main thread', {
      error: String(error?.message || error),
    })
    return null
  }
}

const disposePreviewMarkdownWorker = (reason = 'dispose') => {
  if (previewMarkdownWorker) {
    previewMarkdownWorker.terminate()
  }
  previewMarkdownWorker = null
  previewMarkdownWorkerReady = false
  if (previewMarkdownWorkerPendingTasks.size) {
    for (const [, task] of previewMarkdownWorkerPendingTasks) {
      task.reject(new Error(`Preview markdown worker disposed: ${reason}`))
    }
    previewMarkdownWorkerPendingTasks.clear()
  }
  logger.info('[Preview] markdown worker disposed', { reason })
}

const renderPreviewMarkdownHtmlWithWorker = (content, options = {}, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return Promise.resolve('')
  const worker = previewMarkdownWorker || initPreviewMarkdownWorker()
  if (!worker) {
    return Promise.reject(new Error('Preview markdown worker unavailable'))
  }
  const taskId = ++previewMarkdownWorkerTaskId
  return new Promise((resolve, reject) => {
    previewMarkdownWorkerPendingTasks.set(taskId, { resolve, reject })
    worker.postMessage({
      id: taskId,
      content: markdownText,
      options: buildPreviewMarkdownRenderOptions(options),
      reason,
    })
  })
}

const finalizePreviewRenderedMath = (container) => {
  container
    .querySelectorAll('.math-inline-container, .math-block-container')
    .forEach((element) => {
      const mathNode = element.querySelector('mjx-container')
      if (!(mathNode instanceof HTMLElement)) return
      mathNode.setAttribute('data-md-original', element.getAttribute('data-md-original') || '')
      mathNode.classList.add(
        element.classList.contains('math-inline-container')
          ? 'math-inline'
          : 'math-block',
      )
      element.replaceWith(mathNode)
    })
}

const finalizePreviewRenderedMermaid = async (container, options = {}, reason = 'unknown') => {
  if (!options?.mermaid) return
  const mermaidFontFamily = '"Trebuchet MS", "Noto Sans SC", "Microsoft YaHei", "PingFang SC", "Helvetica Neue", Arial, sans-serif'
  mermaid.initialize({
    startOnLoad: false,
    theme: 'default',
    securityLevel: 'loose',
    themeVariables: {
      background: '#ffffff',
      fontFamily: mermaidFontFamily,
    },
  })

  const mermaidElements = Array.from(container.querySelectorAll('.mermaid'))
  for (const element of mermaidElements) {
    try {
      const source = String(element.textContent || '')
        .replace(/&gt;/g, '>')
        .replace(/&lt;/g, '<')
        .replace(/&amp;/g, '&')
      if (!source.trim()) continue

      const renderId = `guaxi-preview-mermaid-${Date.now()}-${previewMarkdownWorkerTaskId}`
      const rendered = await Promise.resolve(mermaid.render(renderId, source))
      const svg = typeof rendered === 'string' ? rendered : String(rendered?.svg || '')
      if (!svg.trim()) continue

      const wrapper = document.createElement('div')
      wrapper.className = 'mermaid'
      wrapper.setAttribute('data-processed', 'true')
      wrapper.setAttribute('data-mermaid-source', source)
      const originalSource = element.getAttribute('data-md-original')
      if (originalSource) {
        wrapper.setAttribute('data-md-original', originalSource)
      }
      wrapper.innerHTML = svg
      const svgElement = wrapper.querySelector('svg')
      if (svgElement instanceof SVGElement) {
        svgElement.style.fontFamily = mermaidFontFamily
        svgElement.setAttribute('font-family', mermaidFontFamily)
        svgElement.querySelectorAll('text, tspan, foreignObject, foreignObject *').forEach((node) => {
          if (!(node instanceof Element)) return
          node.setAttribute('font-family', mermaidFontFamily)
          node.setAttribute('style', `${String(node.getAttribute('style') || '').trim()};font-family:${mermaidFontFamily};`)
        })
      }
      element.replaceWith(wrapper)

      if (typeof rendered?.bindFunctions === 'function') {
        try {
          rendered.bindFunctions(wrapper)
        } catch (bindError) {
          logger.debug('[Preview] mermaid bindFunctions failed', {
            reason,
            error: String(bindError?.message || bindError),
          })
        }
      }
    } catch (error) {
      logger.error('[Preview] finalize mermaid failed', {
        reason,
        error: String(error?.message || error),
      })
    }
  }
}

const finalizePreviewRenderedHtml = async (html, options = {}, reason = 'unknown') => {
  const rawHtml = String(html || '')
  if (!rawHtml.trim()) return ''
  const container = document.createElement('div')
  container.innerHTML = rawHtml
  const renderOptions = buildPreviewMarkdownRenderOptions(options)
  await finalizePreviewRenderedMermaid(container, renderOptions, reason)
  if (renderOptions.math) {
    finalizePreviewRenderedMath(container)
  }
  return container.innerHTML
}

const renderPreviewMarkdown = async (content, options = {}, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return ''
  try {
    if (previewMarkdownWorkerReady || previewMarkdownWorker) {
      const workerHtml = await renderPreviewMarkdownHtmlWithWorker(markdownText, options, reason)
      const html = await finalizePreviewRenderedHtml(workerHtml, options, reason)
      logger.debug('[Preview] rendered markdown via worker', {
        reason,
        markdownLength: markdownText.length,
      })
      return html
    }
  } catch (error) {
    logger.warn('[Preview] worker markdown render failed, fallback to main thread renderer', {
      reason,
      error: String(error?.message || error),
    })
  }

  const html = await renderMarkdown(markdownText, buildPreviewMarkdownRenderOptions(options))
  logger.debug('[Preview] rendered markdown via main thread fallback', {
    reason,
    markdownLength: markdownText.length,
  })
  return html
}

const renderPreviewSegmentsWithCache = async (markdown, options = {}) => {
  const text = String(markdown || '')
  const previewSegments = createMarkdownPreviewSegments(text)
  if (!previewSegments.length) return ''

  // 设计思路：
  // 1. 预览模式尽量复用探索者模式的“按标题/段落切分”思路，把整篇 Markdown 拆成多个稳定片段。
  // 2. 每个片段单独 renderMarkdown，并放入缓存，减少局部编辑时的重复全量渲染。
  // 3. 片段渲染后把局部行号回写成全局行号，保证预览点击联动、任务列表回写等功能不受影响。
  const htmlParts = []

  for (const segment of previewSegments) {
    const cacheKey = buildPreviewSegmentCacheKey(segment, options)
    let segmentHtml = previewSegmentRenderCache.get(cacheKey)

    if (!segmentHtml) {
      logger.info('[Preview] render segment cache miss', {
        startLine: Number(segment?.startLine || 1),
        endLine: Number(segment?.endLine || 1),
        length: String(segment?.content || '').length,
      })
      segmentHtml = await renderPreviewMarkdown(
        String(segment?.content || ''),
        {
          basePath: options.basePath,
          theme: options.theme,
        },
        `segment:${Number(segment?.startLine || 1)}-${Number(segment?.endLine || 1)}`,
      )
      rememberPreviewCacheEntry(previewSegmentRenderCache, cacheKey, segmentHtml, PREVIEW_SEGMENT_CACHE_LIMIT)
    } else {
      logger.debug('[Preview] render segment cache hit', {
        startLine: Number(segment?.startLine || 1),
        endLine: Number(segment?.endLine || 1),
      })
    }

    htmlParts.push(remapPreviewLineNumbers(segmentHtml, Math.max(0, Number(segment?.startLine || 1) - 1)))
  }

  return htmlParts.join('\n')
}

const extractPreviewFileCardExcerpt = (content) => {
  const lines = String(content || '')
    .replace(/\r/g, '')
    .split('\n')
    .map((line) => String(line || '').trim())
    .filter(Boolean)

  for (const line of lines) {
    if (/^\s{0,3}#{1,6}\s+/.test(line)) continue
    if (/^\s*>/.test(line)) continue
    if (/^\s*[-*+]\s+/.test(line)) continue
    if (/^\s*```/.test(line)) continue
    const cleaned = line
      .replace(/!\[[^\]]*\]\([^)]+\)/g, '')
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
      .replace(/\[\[([^\]]+)\]\]/g, '$1')
      .replace(/[`*_>#-]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()
    if (cleaned) return cleaned.slice(0, 140)
  }

  return '打开该笔记，查看其中的标题、正文与关联内容。'
}

const buildPreviewFileCardTitle = (content, absPath) => {
  const heading = String(content || '').match(/^\s{0,3}#{1,6}\s+(.+)$/m)
  if (heading?.[1]) {
    return String(heading[1] || '').replace(/\s+#+\s*$/, '').trim()
  }
  const normalizedPath = normalizeFsPath(absPath)
  const fileName = normalizedPath.split('/').pop() || normalizedPath
  return fileName.replace(/\.(md|markdown)$/i, '') || '未命名笔记'
}

const buildPreviewFileCardPathLabel = (absPath, { repoRoot } = {}) => {
  const normalizedPath = normalizeFsPath(absPath)
  const normalizedRepoRoot = normalizeFsPath(repoRoot || fileTreeStore.activeRepoPath || '')
  if (normalizedRepoRoot && normalizedPath.startsWith(`${normalizedRepoRoot}/`)) {
    return normalizedPath.slice(normalizedRepoRoot.length + 1)
  }
  return normalizedPath
}

const resolvePreviewFileCardRepoRoot = (absPath, { repoRoot } = {}) => {
  const explicitRepoRoot = normalizeFsPath(repoRoot || '')
  if (explicitRepoRoot) return explicitRepoRoot

  const byPath = normalizeFsPath(findRepoRootForPath(absPath) || '')
  if (byPath) return byPath

  return normalizeFsPath(fileTreeStore.activeRepoPath || '')
}

const buildPreviewFileCardMetaCacheKey = (absPath, { repoRoot = '', mtimeMs = null } = {}) => {
  const normalizedPath = normalizeFsPath(absPath)
  const normalizedRepoRoot = normalizeFsPath(repoRoot || '')
  const normalizedMtime = Number.isFinite(Number(mtimeMs)) ? String(Number(mtimeMs)) : 'unknown'
  return `${normalizedRepoRoot}::${normalizedPath}::${normalizedMtime}`
}

const removePreviewFileCardMetaCacheEntriesForPath = (absPath) => {
  const normalizedPath = normalizeFsPath(absPath)
  if (!normalizedPath || !previewFileCardMetaCache.size) return

  for (const key of Array.from(previewFileCardMetaCache.keys())) {
    if (String(key).includes(`::${normalizedPath}::`)) {
      previewFileCardMetaCache.delete(key)
    }
  }
}

const loadPreviewFileCardMeta = async (absPath, { repoRoot } = {}) => {
  const normalizedPath = normalizeFsPath(absPath)
  if (!normalizedPath) return null

  try {
    const resolvedRepoRoot = resolvePreviewFileCardRepoRoot(normalizedPath, { repoRoot })
    const metaInfo = await fileTreeStore.fetchFileMeta(normalizedPath)
    const cacheKey = buildPreviewFileCardMetaCacheKey(normalizedPath, {
      repoRoot: resolvedRepoRoot,
      mtimeMs: metaInfo?.mtimeMs,
    })
    const cached = previewFileCardMetaCache.get(cacheKey)
    if (cached) {
      logger.info('[PreviewCard] meta cache hit', {
        path: normalizedPath,
        repoRoot: resolvedRepoRoot,
        mtimeMs: metaInfo?.mtimeMs ?? null,
      })
      return cached
    }

    removePreviewFileCardMetaCacheEntriesForPath(normalizedPath)
    const content = String(await invoke('read_file_content', { path: normalizedPath }) || '')
    const meta = {
      path: normalizedPath,
      repoRoot: resolvedRepoRoot,
      mtimeMs: metaInfo?.mtimeMs ?? null,
      title: buildPreviewFileCardTitle(content, normalizedPath),
      excerpt: extractPreviewFileCardExcerpt(content),
      pathLabel: buildPreviewFileCardPathLabel(normalizedPath, { repoRoot: resolvedRepoRoot }),
    }
    rememberPreviewCacheEntry(previewFileCardMetaCache, cacheKey, meta, PREVIEW_FILE_CARD_CACHE_LIMIT)
    logger.info('[PreviewCard] meta loaded', {
      path: normalizedPath,
      title: meta.title,
      repoRoot: resolvedRepoRoot,
      mtimeMs: metaInfo?.mtimeMs ?? null,
    })
    return meta
  } catch (error) {
    logger.warn('[PreviewCard] load meta failed', { path: normalizedPath }, error)
    return null
  }
}

const resolvePreviewCardTargetPath = async (href, { baseDocPath } = {}) => {
  const rawHref = String(href || '').trim()
  if (!rawHref) return ''

  if (rawHref.toLowerCase().startsWith('guaxi://wikilink')) {
    try {
      const u = new URL(rawHref)
      const file = stripLinkBoundaryPunct(u.searchParams.get('file') || '')
      if (!file) return ''

      const primaryAbs = await resolveFsLinkToAbsolute(ensureMarkdownExt(file), { baseDocPath })
      if (isMarkdownFilePath(primaryAbs)) return primaryAbs

      if (isBareWikilinkFileName(file)) {
        let repoRoot = normalizeFsPath(fileTreeStore.activeRepoPath || '')
        if (!repoRoot && baseDocPath) {
          const list = await ensureKnowledgeBasesLoaded()
          for (const kb of Array.isArray(list) ? list : []) {
            const root = normalizeFsPath(kb?.location)
            if (root && (baseDocPath === root || String(baseDocPath).startsWith(`${root}/`))) {
              repoRoot = root
              break
            }
          }
        }
        const hit = await findMarkdownByBasenameInRepo(file, { repoRoot })
        if (isMarkdownFilePath(hit)) return hit
      }
    } catch (error) {
      logger.warn('[PreviewCard] resolve wikilink target failed', { href: rawHref, baseDocPath }, error)
    }
    return ''
  }

  if (!isPathLikeToken(rawHref)) return ''
  const { link } = splitFsLinkAndAnchor(rawHref)
  if (!link) return ''
  const abs = await resolveFsLinkToAbsolute(link, { baseDocPath })
  return isMarkdownFilePath(abs) ? abs : ''
}

const canPromotePreviewLinkToCard = (anchor) => {
  if (!(anchor instanceof HTMLAnchorElement)) return false
  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href || href.startsWith('#') || isHttpUrl(href)) return false

  const container = anchor.closest('p, li')
  if (!container) return false
  const allAnchors = container.querySelectorAll('a')
  if (allAnchors.length !== 1 || allAnchors[0] !== anchor) return false

  const blockText = String(container.textContent || '').replace(/\s+/g, ' ').trim()
  const anchorText = String(anchor.textContent || '').replace(/\s+/g, ' ').trim()
  return !!anchorText && blockText === anchorText
}

const enhancePreviewHtmlWithFileCards = async (html, { baseDocPath } = {}) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<a')) return rawHtml

  const root = document.createElement('div')
  root.innerHTML = rawHtml

  const anchors = Array.from(root.querySelectorAll('a[href]'))
  if (!anchors.length) return rawHtml

  for (const anchor of anchors) {
    try {
      if (!canPromotePreviewLinkToCard(anchor)) continue
      const href = String(anchor.getAttribute('href') || '').trim()
      const targetPath = await resolvePreviewCardTargetPath(href, { baseDocPath })
      if (!targetPath) continue

      const meta = await loadPreviewFileCardMeta(targetPath, {
        repoRoot: findRepoRootForPath(baseDocPath || targetPath) || fileTreeStore.activeRepoPath || '',
      })
      if (!meta) continue

      const sourceBlock = anchor.closest('p, li')
      if (!sourceBlock) continue

      const card = document.createElement('div')
      card.className = 'guaxi-preview-file-card'
      ;['data-md-start-line', 'data-md-end-line', 'data-md-original'].forEach((attr) => {
        const value = sourceBlock.getAttribute(attr)
        if (value) card.setAttribute(attr, value)
      })
      card.setAttribute('data-guaxi-preview-card', 'file-link')
      card.setAttribute('data-guaxi-preview-card-path', meta.path)

      const cardLink = document.createElement('a')
      cardLink.className = 'guaxi-preview-file-card__link'
      cardLink.setAttribute('href', href)
      cardLink.setAttribute('title', meta.path)
      const startLine = sourceBlock.getAttribute('data-md-start-line')
      if (startLine) cardLink.setAttribute('data-md-start-line', startLine)

      cardLink.innerHTML = `
        <span class="guaxi-preview-file-card__eyebrow">关联笔记</span>
        <span class="guaxi-preview-file-card__title">${escapePreviewHtml(meta.title)}</span>
        <span class="guaxi-preview-file-card__excerpt">${escapePreviewHtml(meta.excerpt)}</span>
        <span class="guaxi-preview-file-card__meta">${escapePreviewHtml(meta.pathLabel)}</span>
      `

      card.appendChild(cardLink)
      sourceBlock.replaceWith(card)

      logger.info('[PreviewCard] promoted markdown link to card', {
        href,
        targetPath: meta.path,
        title: meta.title,
      })
    } catch (error) {
      logger.warn('[PreviewCard] promote link to card failed', error)
    }
  }

  return root.innerHTML
}

// 设计说明：
// 1. Markdown 渲染器会把 `![a](...) ![b](...)` 这类写法输出为同一个 `<p>` 里的多个图片节点。
// 2. 旧样式只处理“单张图片居中”，没有处理“图片组容器”的布局，因此多图时不会整体居中；
//    在 Explorer 模式里又会因为图片被逐张当作块级元素处理，造成一张一行，视觉上很生硬。
// 3. 这里在渲染后识别“纯图片段落”，统一打上媒体组 class，让 preview / notebook 都能用同一套稳定布局。
const enhancePreviewHtmlWithMediaGroups = (html) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<img')) return rawHtml

  const root = document.createElement('div')
  root.innerHTML = rawHtml
  const blocks = Array.from(root.querySelectorAll('p'))
  let mediaGroupCount = 0
  let mediaItemCount = 0
  let preservedAuthoredHtmlBlockCount = 0

  const isWhitespaceNode = (node) => {
    if (!node) return true
    if (node.nodeType === Node.TEXT_NODE) return !String(node.textContent || '').trim()
    if (node.nodeType === Node.ELEMENT_NODE && node instanceof HTMLBRElement) return true
    return false
  }

  const shouldPreserveAuthoredHtmlLayout = (block) => {
    if (!(block instanceof HTMLParagraphElement)) return false
    return block.hasAttribute('align') || block.hasAttribute('style')
  }

  const resolveMediaItem = (node) => {
    if (!(node instanceof Element)) return null
    if (node instanceof HTMLImageElement) {
      return {
        kind: 'image',
        host: node,
        image: node,
      }
    }
    if (node instanceof HTMLAnchorElement) {
      const childElements = Array.from(node.children || [])
      if (childElements.length !== 1 || !(childElements[0] instanceof HTMLImageElement)) return null
      const hasNonMediaChild = Array.from(node.childNodes || []).some((child) => {
        if (child === childElements[0]) return false
        return !isWhitespaceNode(child)
      })
      if (hasNonMediaChild) return null
      return {
        kind: 'linked-image',
        host: node,
        image: childElements[0],
      }
    }
    return null
  }

  for (const block of blocks) {
    // 设计说明：
    // 1. Markdown 中可以直接混写 HTML，而这类块最常见的布局意图就是 `align` / `style`。
    // 2. 上一版把“手写 HTML”的识别做得过宽，误伤了正常 Markdown 图片段落。
    // 3. 这里改为只尊重最明确的作者布局声明，让标准 Markdown 图片仍然继续走媒体增强流程。
    if (shouldPreserveAuthoredHtmlLayout(block)) {
      preservedAuthoredHtmlBlockCount += 1
      continue
    }

    const meaningfulNodes = Array.from(block.childNodes || []).filter((node) => !isWhitespaceNode(node))
    if (!meaningfulNodes.length) continue

    const mediaItems = meaningfulNodes.map(resolveMediaItem)
    if (mediaItems.some((item) => !item)) continue

    block.classList.add('guaxi-media-group')
    block.classList.toggle('is-single', mediaItems.length === 1)
    block.classList.toggle('is-gallery', mediaItems.length > 1)
    block.setAttribute('data-guaxi-media-count', String(mediaItems.length))
    block.style.setProperty('--guaxi-media-count', String(mediaItems.length))

    for (const item of mediaItems) {
      // 设计说明：
      // 1. 纯图片节点原本没有外层包裹容器，导致样式层很难同时控制卡片留白、边框和响应式宽度。
      // 2. 这里在渲染后为“裸图片”补一个轻量 span 容器，多图时就能统一按媒体项布局，不影响原始 Markdown 语义。
      if (item.host === item.image) {
        const wrapper = document.createElement('span')
        wrapper.setAttribute('data-guaxi-media-item', item.kind)
        item.image.replaceWith(wrapper)
        wrapper.appendChild(item.image)
        item.host = wrapper
      } else {
        item.host.setAttribute('data-guaxi-media-item', item.kind)
      }
      item.host.classList.add('guaxi-media-group__item')
      item.image.classList.add('guaxi-media-group__image')
    }

    mediaGroupCount += 1
    mediaItemCount += mediaItems.length
  }

  logger.info('[Preview] enhanced media groups', {
    mediaGroupCount,
    mediaItemCount,
    preservedAuthoredHtmlBlockCount,
  })
  return root.innerHTML
}

const getPreviewRenderDelayMs = () => {
  if (previewMode.value === 'split') return PREVIEW_RENDER_DELAY_SPLIT_MS
  if (previewMode.value === 'toggle' && mode.value === 'preview') return PREVIEW_RENDER_DELAY_PREVIEW_ONLY_MS
  return PREVIEW_RENDER_DELAY_DEFAULT_MS
}

const performPreviewRender = async (content, requestId = 0, reason = 'scheduled', context = {}) => {
  const renderFilePath = String(context?.filePath || selectedFile.value?.path || '').trim()
  const currentSelectedPath = String(selectedFile.value?.path || '').trim()

  // 设计说明：
  // 1. 预览渲染是异步链路，文件切换时旧任务可能在新文件打开后才返回。
  // 2. 如果执行阶段直接读取 selectedFile，会把“旧内容”按“新文件路径”参与缓存与回填，最终出现串屏。
  // 3. 因此这里固定使用调度时捕获的文件快照，并在真正写回 HTML 前再次校验当前文件是否仍然一致。
  if (!selectedFile.value || !mode.value || !content || !canPreview.value) {
    renderedHtml.value = '';
    lastPreviewRenderedSource.value = '';
    return;
  }

  if (renderFilePath && currentSelectedPath && renderFilePath !== currentSelectedPath) {
    logger.info('[Preview] skip render because file changed before execution', {
      reason,
      requestId,
      renderFilePath,
      currentSelectedPath,
    })
    return
  }

  try {
    const basePath = renderFilePath ? await dirname(renderFilePath) : '';
    const previewTheme = String(context?.theme || getMarkdownPreviewThemeName())
    logger.info('[Preview] render markdown', {
      reason,
      requestId,
      previewTheme,
      appearanceTheme: settingsStore.appearanceTheme,
      path: renderFilePath,
    })
    const previewSource = transformPythonCellBlocksForPreview(content, {
      filePath: renderFilePath,
    })
    const processed = preprocessObsidianWikilinks(previewSource);
    const containsTocMarker = hasPreviewTocMarker(processed)
    let previewHtml = ''
    const previewSegments = createMarkdownPreviewSegments(processed)
    if (!containsTocMarker && previewSegments.length > 1) {
      logger.info('[Preview] use segmented cached renderer', {
        path: renderFilePath,
        segmentCount: previewSegments.length,
      })
      previewHtml = await renderPreviewSegmentsWithCache(processed, {
        basePath,
        filePath: renderFilePath,
        theme: previewTheme,
      })
    } else {
      const fullCacheKey = buildPreviewDocumentCacheKey(processed, {
        basePath,
        filePath: renderFilePath,
        theme: previewTheme,
      })
      const cachedHtml = previewFullRenderCache.get(fullCacheKey)
      if (cachedHtml) {
        logger.info('[Preview] use full render cache hit', {
          path: renderFilePath,
          reason: containsTocMarker ? 'toc-marker' : 'single-segment',
        })
        previewHtml = cachedHtml
      } else {
      if (containsTocMarker) {
        logger.info('[Preview] fallback to full render because toc marker was detected', {
          path: renderFilePath,
        })
      }
        previewHtml = await renderPreviewMarkdown(
          processed,
          {
            basePath,
            filePath: renderFilePath,
            theme: previewTheme,
          },
          containsTocMarker ? 'full-document-toc' : 'full-document',
        )
        rememberPreviewCacheEntry(previewFullRenderCache, fullCacheKey, previewHtml, PREVIEW_FULL_RENDER_CACHE_LIMIT)
      }
    }

    let enhancedHtml = await enhancePreviewHtmlWithFileCards(previewHtml, {
      baseDocPath: renderFilePath,
    })
    enhancedHtml = enhancePreviewHtmlWithMediaGroups(enhancedHtml)
    const latestSelectedPath = String(selectedFile.value?.path || '').trim()
    if ((requestId && requestId !== previewRenderRequestId) || (renderFilePath && latestSelectedPath && renderFilePath !== latestSelectedPath)) {
      logger.info('[Preview] drop stale render result', {
        requestId,
        latestRequestId: previewRenderRequestId,
        reason,
        renderFilePath,
        latestSelectedPath,
      })
      return
    }

    renderedHtml.value = enhancedHtml

    lastPreviewRenderedSource.value = content;

  } catch (error) {
    logger.error('渲染错误:', error);
    renderedHtml.value = `
      <div class="render-error">
        <p>渲染错误: ${error instanceof Error ? error.message : String(error)}</p>
        <button @click="retryRender">重试</button>
      </div>
    `;
  }
}

// 设计说明：
// 1. 分栏预览需要比纯预览更快的反馈，因此这里按模式动态缩短防抖时间。
// 2. 异步渲染可能晚于用户的下一次输入完成，用 requestId 丢弃过期结果，避免“旧结果压住新结果”。
const renderDebounced = (content, reason = 'scheduled', context = {}) => {
  if (previewRenderTimer) {
    window.clearTimeout(previewRenderTimer)
    previewRenderTimer = null
  }

  const requestId = ++previewRenderRequestId
  const delayMs = getPreviewRenderDelayMs()
  const filePathSnapshot = String(context?.filePath || selectedFile.value?.path || '').trim()
  const themeSnapshot = String(context?.theme || getMarkdownPreviewThemeName())
  logger.debug('[Preview] schedule render', {
    requestId,
    reason,
    delayMs,
    path: filePathSnapshot,
    previewMode: previewMode.value,
  })

  previewRenderTimer = window.setTimeout(() => {
    previewRenderTimer = null
    void performPreviewRender(String(content || ''), requestId, reason, {
      filePath: filePathSnapshot,
      theme: themeSnapshot,
    })
  }, delayMs)
}

const canPreview = computed(() => {
  const path = selectedFile.value?.path || '';
  const ext = path.split('.').pop()?.toLowerCase();
  if (ext === 'md' || ext === 'markdown') return true;
  return editorStore.language === 'markdown';
});

const lastSelectedFilePath = ref(null)
let skipPreviousTabSyncOnce = false

// 监听文件选择
watch(selectedFile, async (newFile) => {
  flushPythonCellDrafts()
  if (newFile) {
    const nextPath = newFile.path || '';
    const prevPath = lastSelectedFilePath.value || '';
    const switchedToDifferentFile = !!nextPath && !!prevPath && nextPath !== prevPath;
    const shouldSkipPreviousTabSync = skipPreviousTabSyncOnce
    skipPreviousTabSyncOnce = false

    if (switchedToDifferentFile) {
      // 设计说明：
      // 1. 从文件树直接打开另一个文件时，不会经过 switchTab，需要在这里补一次旧文件内容回写。
      // 2. 通过 tab 切换时 switchTab 已经做过回写；若这里再次读取编辑器内容，可能读到“新文件内容”，
      //    反而把旧 tab 覆盖掉，所以用一次性标记跳过重复同步。
      if (!shouldSkipPreviousTabSync && prevPath) {
        const targetEditor = getActiveEditor();
        const currentContent =
          previewMode.value === 'realtime' && realtimeVditor
            ? realtimeVditor.getValue()
            : targetEditor
              ? targetEditor.getValue()
              : rawContent.value;

        fileTreeStore.updateTabContent(prevPath, currentContent);
      }

      if (previewRenderTimer) {
        window.clearTimeout(previewRenderTimer)
        previewRenderTimer = null
      }
      previewRenderRequestId += 1
      renderedHtml.value = ''
      lastPreviewRenderedSource.value = ''
      logger.info('[Preview] cleared stale preview state after file switch', {
        from: prevPath,
        to: nextPath,
      })
    }

    const isSameFile = !!nextPath && !!prevPath && nextPath === prevPath;
    if (isSameFile) {
      const nextContent = newFile.content || '';
      const targetEditor = getActiveEditor();
      const currentContent =
        previewMode.value === 'realtime' && realtimeVditor
          ? realtimeVditor.getValue()
          : targetEditor
            ? targetEditor.getValue()
            : rawContent.value;

      if (currentContent === nextContent) {
        if (rawContent.value !== nextContent) {
          rawContent.value = nextContent;
        }
        return;
      }
    }

    lastSelectedFilePath.value = nextPath;

    const language = resolveMonacoLanguageByPath(newFile.path || '');
    void ensurePythonSessionInfo(newFile.path || '', 'selected-file-changed')

    // 更新内容
    rawContent.value = newFile.content || '';

    if (previewMode.value === 'realtime' && realtimeVditor) {
      const nextValue = rawContent.value || '';
      if (!realtimeVditorReady.value) {
        pendingRealtimeVditorValue.value = nextValue;
      } else {
        settingRealtimeVditorValue = true;
        try {
          realtimeVditor.setValue(nextValue);
        } finally {
          settingRealtimeVditorValue = false;
        }
      }
    }

    updateEditorContent(rawContent.value);
    nextTick(() => {
      syncSourceEditorContent('selected-file-changed')
    })

    const targetEditor = getActiveEditor();
    if (targetEditor) {
      const model = targetEditor.getModel?.() || null
      if (model) {
        monaco.editor.setModelLanguage(model, language);
      }
      enableLspForEditorLanguage(targetEditor, language, newFile.path || '');
    }

    if (!selectedFile.value || !mode.value) {
      renderedHtml.value = ''
      return
    }

    if (canPreview.value && (mode.value === 'preview' || previewMode.value !== 'toggle')) {
      renderDebounced(newFile.content, 'selected-file-changed', {
        filePath: nextPath,
      })
    }

    await initGitService()
  } else {
    disposeAllVsCodeTextModelsForOwner(PRIMARY_EDITOR_VSCODE_MODEL_OWNER, 'selected-file-cleared')
    disposeAllVsCodeTextModelsForOwner(SECONDARY_EDITOR_VSCODE_MODEL_OWNER, 'selected-file-cleared')
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
    }
    if (secondaryEditor) {
      secondaryEditor.setValue('');
      secondaryEditor.getModel().setValue('');
    }
    renderedHtml.value = '';
    return;
  }
}, { immediate: true });

const previewModeOptions = [
  { label: '编辑模式 (Ctrl+1)', key: 'edit', icon: () => h(Icon, { icon: filePenLineIcon, width: 16, height: 16 }) },
  { label: '预览模式 (Ctrl+2)', key: 'preview', icon: () => h(Icon, { icon: eyeIcon, width: 16, height: 16 }) },
  { label: '分屏预览 (Ctrl+3)', key: 'split', icon: () => h(Icon, { icon: columns2Icon, width: 16, height: 16 }) },
  { label: '实时预览 (Ctrl+4)', key: 'realtime', icon: () => h(Icon, { icon: sparklesIcon, width: 16, height: 16 }) }
  , { label: '探索模式 (Ctrl+5)', key: 'explorer', icon: () => h(Icon, { icon: scanSearchIcon, width: 16, height: 16 }) }
];

const previewModeLabel = computed(() => {
  if (isExplorerSurfaceActive.value) {
    return '探索模式';
  }
  if (previewMode.value === 'toggle') {
    return mode.value === 'edit' ? '编辑模式' : '预览模式';
  }
  return previewMode.value === 'split' ? '分屏预览' : '实时预览';
});

const previewModeIcon = computed(() => {
  if (isExplorerSurfaceActive.value) {
    return scanSearchIcon;
  }
  if (previewMode.value === 'toggle') {
    return mode.value === 'edit' ? filePenLineIcon : eyeIcon;
  }
  return previewMode.value === 'split' ? columns2Icon : sparklesIcon;
});

const showEditor = computed(() => {
  if (!selectedFile.value) return false;
  if (previewMode.value === 'realtime') return false;
  return previewMode.value === 'toggle' ? mode.value === 'edit' : true;
});

const showPreview = computed(() => {
  if (!selectedFile.value || !canPreview.value) return false;
  if (previewMode.value === 'realtime') return false;
  return previewMode.value === 'toggle' ? mode.value === 'preview' : true;
});

const showEditorShell = computed(() => {
  // 设计说明：
  // 1. 终端面板现在已经从 editor-wrapper 中拆出，避免“无文件 + 打开终端”时仍然渲染出
  //    面包屑栏和空编辑器容器，形成中间那块“未打开文件”的空区域。
  // 2. 这里保留 selectedFile 作为硬条件，让编辑区只在真正存在活动文件时出现。
  return !!selectedFile.value && ((previewMode.value !== 'realtime' && showEditor.value) || pythonConsoleVisible.value);
});

const showEditorEmptyState = computed(() => {
  // 当终端单独打开时，不再显示“请选择一个文件以开始编辑”的占位文案，
  // 这样界面会直接退化成纯 terminal 视图。
  return !selectedFile.value && !pythonConsoleVisible.value;
});

watch(renderedHtml, () => {
  previewLineAnchorRevision += 1
  previewLineAnchorCache.container = null
  previewLineAnchorCache.elements = []
  previewLineAnchorCache.revision = previewLineAnchorRevision
});

const syncPreviewToCursorLine = debounce((line) => {
  if (!line || !showPreview.value) return;
  scrollPreviewToLine(line, { behavior: 'auto' });
}, 40);

watch(
  () => editorStore.cursorPosition?.lineNumber,
  (line) => {
    if (!line) return;
    if (showPreview.value) {
      syncPreviewToCursorLine(line);
    }
  },
  { flush: 'post' }
);

watch([showEditor, editorSurfaceMode], ([editorVisible, surfaceMode]) => {
  if (!editorVisible) return
  if (surfaceMode === 'notebook' && canUseNotebookSurface.value) return
  nextTick(() => {
    ensurePrimarySourceEditorReady('editor-visible-or-surface-changed')
  })
}, { flush: 'post' })

watch([showEditorShell, pythonConsoleVisible, selectedFile], ([shellVisible, terminalVisible, file]) => {
  logger.debug('[EditorLayout] shell visibility updated', {
    shellVisible,
    terminalVisible,
    hasSelectedFile: !!file,
    filePath: String(file?.path || ''),
  })
}, { flush: 'post' })

const shouldAutoRenderPreview = computed(() => {
  if (!selectedFile.value || !canPreview.value) return false;
  return previewMode.value === 'toggle' && mode.value === 'preview';
});

const refreshPreview = () => {
  renderDebounced(rawContent.value);
};

const initRealtimeVditor = async () => {
  if (!selectedFile.value || !canPreview.value) return;
  if (previewMode.value !== 'realtime') return;

  await nextTick();

  const desiredValue = rawContent.value || '';
  const vditorCdn = getLocalVditorCdn();
  const vditorI18n = getBundledVditorI18n();

  if (!realtimeVditor) {
    realtimeVditorReady.value = false;
    pendingRealtimeVditorValue.value = null;
    const initDiagnostics = createRealtimeVditorInitDiagnostics()
    await new Promise((resolve, reject) => {
      const timeoutId = window.setTimeout(() => {
        reject(new Error(`Vditor init timeout after ${REALTIME_VDITOR_INIT_TIMEOUT_MS}ms`));
      }, REALTIME_VDITOR_INIT_TIMEOUT_MS);

      const finishResolve = () => {
        window.clearTimeout(timeoutId);
        resolve();
      };

      const finishReject = (error) => {
        window.clearTimeout(timeoutId);
        reject(error);
      };

      try {
        realtimeVditor = new Vditor(realtimeVditorId, {
          mode: 'ir',
          cache: { enable: false },
          cdn: vditorCdn,
          lang: 'zh_CN',
          i18n: vditorI18n,
          preview: {
            delay: 200,
            hljs: { enable: false }
          },
          toolbarConfig: { hide: false },
          toolbar: [
            'headings',
            'bold',
            'italic',
            'strike',
            'link',
            '|',
            'list',
            'ordered-list',
            'check',
            '|',
            'quote',
            'line',
            'code',
            'inline-code',
            '|',
            'table',
            '|',
            'undo',
            'redo'
          ],
          value: desiredValue,
          input: (value) => {
            if (settingRealtimeVditorValue) return;
            syncRealtimeValueToRawContent(value);
          },
          after: () => {
            realtimeVditorReady.value = true;

            installRealtimeVditorLinkInterceptor();

            if (pendingRealtimeVditorValue.value != null && realtimeVditor) {
              const v = pendingRealtimeVditorValue.value;
              pendingRealtimeVditorValue.value = null;
              settingRealtimeVditorValue = true;
              try {
                realtimeVditor.setValue(v);
              } finally {
                settingRealtimeVditorValue = false;
              }
            }

            if (pendingRealtimeScrollRestore.value) {
              restoreScrollPosition();
            }

            finishResolve();
          }
        });
      } catch (error) {
        finishReject(error);
      }
    }).catch((error) => {
      const diagnostics = initDiagnostics.snapshot()
      logger.error('[Editor] initRealtimeVditor failed', {
        error,
        cdn: vditorCdn,
        hasBundledI18n: Boolean(vditorI18n),
        resourceFailures: diagnostics.resourceFailures,
        promiseFailures: diagnostics.promiseFailures,
      });
      destroyRealtimeVditor();
      throw error;
    }).finally(() => {
      initDiagnostics.dispose()
    });

    return;
  }

  if (!realtimeVditorReady.value) {
    pendingRealtimeVditorValue.value = desiredValue;
    return;
  }

  settingRealtimeVditorValue = true;
  try {
    realtimeVditor.setValue(desiredValue);
  } finally {
    settingRealtimeVditorValue = false;
  }

  if (pendingRealtimeScrollRestore.value) {
    restoreScrollPosition();
  }
};

const destroyRealtimeVditor = () => {
  syncRealtimeValueToRawContent.cancel();

  if (typeof realtimeVditorLinkInterceptorCleanup === 'function') {
    realtimeVditorLinkInterceptorCleanup();
    realtimeVditorLinkInterceptorCleanup = null;
  }

  if (!realtimeVditor) return;
  try {
    realtimeVditor.destroy();
  } finally {
    realtimeVditor = null;
    realtimeVditorReady.value = false;
    pendingRealtimeVditorValue.value = null;
  }
};

const normalizeHomePreviewModeKey = (key) => {
  const k = String(key || '').trim().toLowerCase();
  return ['edit', 'preview', 'split', 'realtime', 'explorer'].includes(k) ? k : 'edit';
};

const getHomePreviewModeKeyFromState = () => {
  if (isExplorerSurfaceActive.value) {
    return 'explorer';
  }
  if (previewMode.value === 'toggle') {
    return mode.value === 'preview' ? 'preview' : 'edit';
  }
  return normalizeHomePreviewModeKey(previewMode.value);
};

const setHomePreviewModeKeySafe = (key) => {
  const next = normalizeHomePreviewModeKey(key);

  const setter = editorStore?.setHomePreviewModeKey;
  if (typeof setter === 'function') {
    try {
      setter.call(editorStore, next);
      return;
    } catch (e) {
      logger.warn('[Editor] setHomePreviewModeKey failed, fallback to direct write', e);
    }
  } else {
    logger.warn('[Editor] editorStore.setHomePreviewModeKey is not a function, fallback to direct write', {
      type: typeof setter,
      key: next,
    });
  }

  try {
    editorStore.homePreviewModeKey = next;
  } catch (e) {
    logger.error('[Editor] fallback write homePreviewModeKey failed', e);
  }
};

const handlePreviewModeSelect = async (key) => {
  const desiredKey = normalizeHomePreviewModeKey(key);
  logger.info('[Editor] handlePreviewModeSelect', {
    desiredKey,
    filePath: String(selectedFile.value?.path || ''),
    canPreview: canPreview.value,
    canUseNotebookSurface: canUseNotebookSurface.value,
    currentPreviewMode: previewMode.value,
    currentMainMode: mode.value,
    currentSurfaceMode: editorSurfaceMode.value,
  });

  if (previewMode.value === 'realtime') {
    captureScrollPosition();
    if (realtimeVditor) {
      const content = realtimeVditor.getValue();
      rawContent.value = content;
      if (editor) {
        updateEditorContent(content);
      }
    }
  }

  if (desiredKey === 'explorer') {
    if (!canUseNotebookSurface.value) {
      message.warning('Explorer mode currently requires a single Markdown editor surface.');
      setHomePreviewModeKeySafe('edit');
      editorSurfaceMode.value = 'source';
      previewMode.value = 'toggle';
      await setMainMode('edit');
      return;
    }

    logger.info('[Editor] switch to explorer surface', {
      filePath: String(selectedFile.value?.path || ''),
    });
    setHomePreviewModeKeySafe('explorer');
    previewMode.value = 'toggle';
    editorSurfaceMode.value = 'notebook';
    disposePythonCellManagers();
    await setMainMode('edit');
    return;
  }

  if (desiredKey === 'edit') {
    setHomePreviewModeKeySafe('edit');
    editorSurfaceMode.value = 'source';
    previewMode.value = 'toggle';
    await setMainMode('edit');
    return;
  }

  if (desiredKey === 'preview') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      return;
    }
    setHomePreviewModeKeySafe('preview');
    editorSurfaceMode.value = 'source';
    previewMode.value = 'toggle';
    await setMainMode('preview');
    return;
  }

  if (desiredKey === 'split') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    setHomePreviewModeKeySafe('split');
    editorSurfaceMode.value = 'source';
    previewMode.value = 'split';
    await setMainMode('edit');
    await nextTick();
    refreshPreview();
    return;
  }

  if (desiredKey === 'realtime') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    setHomePreviewModeKeySafe('realtime');
    editorSurfaceMode.value = 'source';
    captureScrollPosition();
    pendingRealtimeScrollRestore.value = true;
    previewMode.value = 'realtime';
    await setMainMode('edit');
  }
};

const restoreHomePreviewMode = async (from = 'mounted') => {
  const desiredKey = normalizeHomePreviewModeKey(editorStore.homePreviewModeKey);
  const currentKey = getHomePreviewModeKeyFromState();
  if (currentKey === desiredKey) return;

  logger.debug('[Editor] restoreHomePreviewMode', { from, desiredKey, currentKey });
  await handlePreviewModeSelect(desiredKey);
};

watch(previewMode, async (nextMode, prevMode) => {
  if (prevMode === 'realtime' && nextMode !== 'realtime') {
    destroyRealtimeVditor();
  }

  if (nextMode === 'toggle') return;

  if (!canPreview.value) {
    message.warning('仅 Markdown 文件支持预览');
    previewMode.value = 'toggle';
    await setMainMode('edit');
    renderedHtml.value = '';
    return;
  }

  if (nextMode === 'realtime') {
    pendingRealtimeScrollRestore.value = true;
    try {
      await initRealtimeVditor();
    } catch (error) {
      logger.error('[Editor] realtime preview init failed, fallback to split', error);
      pendingRealtimeScrollRestore.value = false;
      setHomePreviewModeKeySafe('split');
      previewMode.value = 'split';
      await setMainMode('edit');
      await nextTick();
      refreshPreview();
      message.error('实时预览初始化失败，已自动切换到分屏预览');
    }
    return;
  }

  await nextTick();
  if (!editor) {
    initEditor();
  }

  refreshPreview();
});

watch([rawContent, shouldAutoRenderPreview], () => {
  if (!shouldAutoRenderPreview.value) return;

  // 任务列表点击会回写 rawContent，但预览仅做局部 DOM 更新；此处抑制整篇重渲染。
  if (consumeSuppressPreviewAutoRender('watch:rawContent')) return;

  renderDebounced(rawContent.value);
});

function cleanupPreviewPdfBlobUrls() {
  const container = previewContainer.value
  if (!(container instanceof HTMLElement)) return

  const medias = Array.from(container.querySelectorAll('.guaxi-codecell-media[data-blob-url]'))
  medias.forEach((media) => {
    const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
    if (!oldUrl) return
    try {
      URL.revokeObjectURL(oldUrl)
    } catch (error) {
      logger.warn('[CodeCell] revoke preview pdf blob failed', error)
    }
    media.removeAttribute('data-blob-url')
  })

  logger.info('[CodeCell] preview pdf blob urls cleaned', { count: medias.length })
}

function ensurePyCellToolbarEnhancements(cell) {
  if (!(cell instanceof Element)) return
  const toolbar = cell.querySelector?.('.guaxi-pycell-toolbar')
  if (!toolbar) return

  const ensureButton = (action, label, className, beforeAction = 'toggle-output') => {
    let btn = toolbar.querySelector(`[data-guaxi-py-action="${action}"]`)
    if (btn) {
      btn.textContent = label
      return btn
    }
    btn = document.createElement('button')
    btn.type = 'button'
    btn.className = `guaxi-pycell-btn ${className}`.trim()
    btn.setAttribute('data-guaxi-py-action', action)
    btn.textContent = label

    const anchor = beforeAction ? toolbar.querySelector(`[data-guaxi-py-action="${beforeAction}"]`) : null
    if (anchor?.parentNode === toolbar) toolbar.insertBefore(btn, anchor)
    else toolbar.appendChild(btn)
    return btn
  }

  ensureButton('run', '运行', 'guaxi-pycell-run')
  ensureButton('run-save', '运行并写回', 'guaxi-pycell-run-save')
  ensureButton('import-table', '导入表格', 'guaxi-pycell-import')
  ensureButton('inject-note', '注入笔记', 'guaxi-pycell-note')
  ensureButton('toggle-session', '会话变量', 'guaxi-pycell-session-btn')
  ensureButton('restart-session', '重启会话', 'guaxi-pycell-reset')
  ensureButton('toggle-output', '显示结果', 'guaxi-pycell-output-toggle', null)

  let hint = toolbar.querySelector('.guaxi-pycell-hint')
  if (!hint) {
    hint = document.createElement('span')
    hint.className = 'guaxi-pycell-hint'
    toolbar.appendChild(hint)
  }
  hint.textContent = '共享当前笔记的 Python 会话，支持导入表格、注入笔记和查看变量'
}

// 监听内容变化
watch(renderedHtml, () => {
  nextTick(() => {
    if (previewContainer.value) {
      addCopyButtons(previewContainer.value, {
        theme: getMarkdownPreviewThemeName()
      });
      addMediaViewerButtons(previewContainer.value, {
        theme: getMarkdownPreviewThemeName()
      });
      syncAllPreviewPythonCells()
      void renderPreviewMermaidDiagrams(previewContainer.value).catch((error) => {
        logger.error('预览区 Mermaid 渲染失败', error);
      });
      if (pendingPreviewScrollRestore.value) {
        const el = previewContainer.value;
        const height = el.scrollHeight;
        el.scrollTop = height * scrollRatio.value;
        pendingPreviewScrollRestore.value = false;
      }
      if (pendingPreviewScrollLine.value) {
        scrollPreviewToLine(pendingPreviewScrollLine.value);
        pendingPreviewScrollLine.value = null;
      }
      applyPreviewSearchHighlights()
    }
  });
});

watch(previewSearchKeyword, () => {
  nextTick(() => {
    applyPreviewSearchHighlights()
  })
})

watch([showPreview, previewMode, mode], ([previewShown, currentPreviewMode, currentMode]) => {
  const previewOnly = previewShown && currentPreviewMode !== 'realtime' && currentMode === 'preview' && !showEditor.value
  if (!previewOnly && previewSearchVisible.value) {
    closePreviewSearch()
  }
})

// 添加Vim模式切换函数
const toggleVimMode = () => {
  if (mode.value === 'preview') {
    message.warning('阅读模式下不能启用Vim编辑模式');
    return;
  }

  vimMode.value = !vimMode.value;

  nextTick(() => {
    const currentTheme = document.documentElement?.getAttribute('data-guaxi-theme') || 'light';
    logger.info('[GuaxiEditor] Vim 模式切换', {
      enabled: vimMode.value,
      theme: currentTheme,
      hasStatusBar: Boolean(vimStatusBar.value)
    });

    if (vimMode.value) {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
    } else {
      if (vimModeInstance) {
        vimModeInstance.dispose();
        vimModeInstance = null;
      }
      if (editor) {
        editor.updateOptions({
          vim: {
            enabled: false
          }
        });
      }
    }

    applyVimOverlayInsets();
  });
};

const buildPrintHtml = (bodyHtml, title) => {
  const now = new Date();
  const dateStr = now.toLocaleString();

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<title>${title}</title>
<style>
  @page {
    size: A4;
    margin: 16mm;
  }

  html, body {
    margin: 0;
    padding: 0;
    background: #ffffff;
  }

  body {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
    color: #000000;
  }

  .export-root {
    box-sizing: border-box;
    padding: 12px 8px 24px 8px;
  }

  .export-header {
    margin-bottom: 24px;
    padding-bottom: 12px;
    border-bottom: 1px solid #e0e0e0;
  }

  .export-title {
    font-size: 22px;
    font-weight: 600;
    margin: 0 0 4px 0;
    word-break: break-all;
  }

  .export-meta {
    font-size: 12px;
    color: #666666;
  }

  .export-content {
    font-size: 16px;
    line-height: 1.6;
  }

  .export-content code,
  .export-content pre {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  }

  .export-content pre {
    background: #f5f5f5;
    padding: 10px 12px;
    border-radius: 4px;
    overflow-x: auto;
  }

  .export-content table {
    border-collapse: collapse;
    width: 100%;
  }

  .export-content table th,
  .export-content table td {
    border: 1px solid #ddd;
    padding: 6px 8px;
  }

  .export-content img {
    max-width: 100%;
    height: auto;
  }

  @media print {
    body {
      -webkit-print-color-adjust: exact;
      print-color-adjust: exact;
    }
  }
</style>
<style>
.guaxi-editor .guaxi-pycell-overlay-root {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 18;
}

.guaxi-editor .guaxi-pycell-overlay-card {
  position: absolute;
  pointer-events: none;
}

.guaxi-editor .guaxi-pycell-overlay-card > .guaxi-pycell-card {
  pointer-events: auto;
}

.guaxi-editor .guaxi-pycell-spacer {
  pointer-events: none;
}

.guaxi-editor .guaxi-pycell-card {
  margin: 4px 0 8px;
  padding: 0;
  border: 1px solid var(--pycell-card-border);
  border-radius: 16px;
  background: var(--pycell-card-bg);
  box-shadow: var(--pycell-card-shadow);
  overflow: hidden;
  pointer-events: auto;
}

.guaxi-editor .guaxi-pycell-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 14px 9px;
  border-bottom: 1px solid var(--pycell-header-border);
  background: var(--pycell-header-bg);
}

.guaxi-editor .guaxi-pycell-card__prompt {
  flex: 0 0 auto;
  min-width: 56px;
  font-size: 12px;
  color: var(--pycell-prompt);
  font-weight: 700;
  letter-spacing: 0.02em;
}

.guaxi-editor .guaxi-pycell-card.is-running .guaxi-pycell-card__prompt {
  color: #2563eb;
  animation: guaxi-pycell-prompt-pulse 1.1s ease-in-out infinite;
}

.guaxi-editor .guaxi-pycell-card__title {
  font-size: 14px;
  line-height: 1.3;
  font-weight: 700;
  color: var(--pycell-title);
}

.guaxi-editor .guaxi-pycell-card__meta {
  margin-top: 4px;
  font-size: 11px;
  color: var(--pycell-meta);
}

.guaxi-editor .guaxi-pycell-card__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.guaxi-editor .guaxi-pycell-card__btn,
.guaxi-editor .guaxi-inline-dataframe__btn {
  border: 1px solid var(--pycell-btn-border);
  border-radius: 999px;
  background: var(--pycell-btn-bg);
  color: var(--pycell-btn-text);
  font-size: 11px;
  font-weight: 600;
  padding: 5px 10px;
  cursor: pointer;
  pointer-events: auto;
  transition: background 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}

.guaxi-editor .guaxi-inline-dataframe__btn {
  font-size: 12px;
}

.guaxi-editor .guaxi-pycell-card__btn:hover,
.guaxi-editor .guaxi-inline-dataframe__btn:hover {
  border-color: var(--pycell-btn-hover-border);
  background: var(--pycell-btn-hover-bg);
  box-shadow: var(--pycell-btn-hover-shadow);
  transform: translateY(-1px);
}

.guaxi-editor .guaxi-pycell-card__btn:focus-visible,
.guaxi-editor .guaxi-inline-dataframe__btn:focus-visible {
  outline: none;
  border-color: var(--pycell-btn-hover-border);
  box-shadow: var(--pycell-btn-focus-ring);
}

.guaxi-editor .guaxi-pycell-card__body {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.guaxi-editor .guaxi-pycell-card__code-shell,
.guaxi-editor .guaxi-pycell-card__panel,
.guaxi-editor .guaxi-pycell-card__rich,
.guaxi-editor .guaxi-inline-dataframe {
  border: none;
  border-radius: 0;
  background: transparent;
}

.guaxi-editor .guaxi-pycell-card__panel.is-collapsed {
  display: none;
}

.guaxi-editor .guaxi-pycell-card__editor {
  width: 100%;
  min-height: 96px;
  resize: none;
  border: none;
  outline: none;
  padding: 12px 14px 14px;
  background: var(--pycell-editor-bg);
  color: var(--pycell-editor-text);
  font-size: 13px;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  box-sizing: border-box;
  pointer-events: auto;
  caret-color: #2563eb;
}

.guaxi-editor .guaxi-pycell-card__editor:focus {
  background: var(--pycell-editor-focus-bg);
}

.guaxi-editor .guaxi-pycell-card__panel {
  padding: 10px 14px 14px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
  color: var(--pycell-panel-text);
  background: var(--pycell-panel-bg);
}

.guaxi-editor .guaxi-pycell-card__panel--output::before,
.guaxi-editor .guaxi-pycell-card__panel--session::before {
  display: block;
  margin-bottom: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-editor .guaxi-pycell-card__panel--output::before {
  content: 'Out [ ]';
  color: var(--pycell-output-label);
}

.guaxi-editor .guaxi-pycell-card__panel--session::before {
  content: 'Session';
  color: var(--pycell-session-label);
}

.guaxi-editor .guaxi-pycell-card__stdout,
.guaxi-editor .guaxi-pycell-card__rich--markdown {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  color: var(--pycell-output-text);
  background: var(--pycell-output-bg);
  border: 1px solid var(--pycell-output-border);
  border-radius: 12px;
  padding: 12px 13px;
}

.guaxi-editor .guaxi-pycell-card__image {
  display: block;
  max-width: 100%;
  height: auto;
  border-radius: 12px;
}

.guaxi-editor .guaxi-pycell-card__rich--svg svg {
  max-width: 100%;
  height: auto;
}

.guaxi-editor .guaxi-pycell-card__empty {
  font-size: 12px;
  color: var(--pycell-empty);
}

.guaxi-editor .guaxi-pycell-card__var + .guaxi-pycell-card__var {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-editor .guaxi-pycell-card__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.guaxi-editor .guaxi-pycell-card__var-name {
  font-weight: 700;
  color: var(--pycell-var-name);
}

.guaxi-editor .guaxi-pycell-card__var-type {
  color: var(--pycell-var-type);
  font-size: 12px;
}

.guaxi-editor .guaxi-pycell-card__var-preview,
.guaxi-editor .guaxi-inline-dataframe__meta {
  margin-top: 4px;
  color: var(--pycell-var-preview);
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.guaxi-editor .guaxi-inline-dataframe {
  overflow: hidden;
}

.guaxi-editor .guaxi-inline-dataframe__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-editor .guaxi-inline-dataframe__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.guaxi-editor .guaxi-inline-dataframe__viewport {
  overflow: auto;
}

.guaxi-editor .guaxi-inline-dataframe__table {
  width: 100%;
  border-collapse: collapse;
}

.guaxi-editor .guaxi-inline-dataframe__table th,
.guaxi-editor .guaxi-inline-dataframe__table td {
  padding: 8px 10px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
  text-align: left;
  white-space: nowrap;
}

.guaxi-editor .guaxi-inline-dataframe__table th {
  background: rgba(226, 232, 240, 0.48);
  font-weight: 700;
}

:root[data-guaxi-theme='dark'] .guaxi-editor .guaxi-inline-dataframe__table th {
  background: rgba(51, 65, 85, 0.72);
}
</style>
</head>
<body>
  <div class="export-root">
    <div class="export-header">
      <div class="export-title">${title}</div>
      <div class="export-meta">导出时间：${dateStr}</div>
    </div>
    <div class="export-content">${bodyHtml}</div>
  </div>
</body>
</html>`;
};

const createPrintIframeAndPrint = (html) => {
  const iframe = document.createElement('iframe');
  iframe.style.position = 'fixed';
  iframe.style.right = '0';
  iframe.style.bottom = '0';
  iframe.style.width = '0';
  iframe.style.height = '0';
  iframe.style.border = '0';
  iframe.style.visibility = 'hidden';
  iframe.srcdoc = html;
  document.body.appendChild(iframe);

  const cleanup = () => {
    if (iframe.parentNode) {
      iframe.parentNode.removeChild(iframe);
    }
  };

  iframe.onload = () => {
    try {
      if (iframe.contentWindow) {
        iframe.contentWindow.focus();
        iframe.contentWindow.print();
      }
    } finally {
      setTimeout(cleanup, 1000);
    }
  };
};

const exportOptions = [
  { label: '导出为 PDF', key: 'pdf' },
  { label: '导出为 Word (.docx)', key: 'docx' },
  { label: '导出为 HTML (.html)', key: 'html' },
  { label: '导出为 LaTeX (.tex)', key: 'tex' },
  { label: '导出为 EPUB (.epub)', key: 'epub' },
  { label: '导出为 ODT (.odt)', key: 'odt' },
  { label: '导出为 纯文本 (.txt)', key: 'txt' }
];

const handleExportSelect = async (format) => {
  if (!selectedFile.value || !selectedFile.value.path) {
    message.warning('当前没有选中的文件');
    return;
  }

  const path = selectedFile.value.path || '';
  const ext = path.split('.').pop()?.toLowerCase() || '';

  if (mode.value === 'edit' && editor) {
    saveContent();
  }

  // 说明：.tex 文件导出 PDF 走 tectonic（不走 pandoc）
  const isTexFile = ext === 'tex'
  const isMarkdownFile = ext === 'md'

  if (!isMarkdownFile && !(isTexFile && format === 'pdf')) {
    if (isTexFile) {
      message.warning('当前为 .tex 文件，仅支持导出为 PDF')
    } else {
      message.warning('仅支持对 Markdown(.md) 文件进行导出')
    }
    return
  }

  const formatExtMap = {
    pdf: 'pdf',
    docx: 'docx',
    html: 'html',
    tex: 'tex',
    epub: 'epub',
    odt: 'odt',
    txt: 'txt'
  };

  const extName = formatExtMap[format] || format;

  let filterName = '';
  switch (format) {
    case 'pdf':
      filterName = 'PDF 文件';
      break;
    case 'docx':
      filterName = 'Word 文件';
      break;
    case 'html':
      filterName = 'HTML 文件';
      break;
    case 'tex':
      filterName = 'LaTeX 文件';
      break;
    case 'epub':
      filterName = 'EPUB 文件';
      break;
    case 'odt':
      filterName = 'ODT 文件';
      break;
    case 'txt':
      filterName = '文本文件';
      break;
    default:
      filterName = '导出文件';
  }

  try {
    let defaultPath = path;
    if (path.toLowerCase().endsWith('.md')) {
      defaultPath = path.slice(0, -3) + '.' + extName;
    } else {
      defaultPath = path + '.' + extName;
    }

    const outputPath = await save({
      defaultPath,
      filters: [
        {
          name: filterName,
          extensions: [extName]
        }
      ]
    });

    if (!outputPath) {
      message.info('已取消导出');
      return;
    }

    if (isTexFile && format === 'pdf') {
      await invoke('export_tex_pdf_tectonic', {
        inputPath: path,
        outputPath
      })
    } else if (format === 'pdf') {
      await invoke('export_markdown_pdf_pandoc', {
        inputPath: path,
        outputPath
      });
    } else {
      await invoke('export_markdown_pandoc', {
        inputPath: path,
        outputPath,
        format
      });
    }

    message.success(`已导出为 ${filterName}`);
  } catch (error) {
    logger.error('导出 Markdown 失败:', error);
    message.error('导出 Markdown 失败');
  }
};

const switchTab = (path, region) => {
  flushPythonCellDrafts()
  const targetRegion = region || 'left';

  const currentPath = targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  if (currentPath) {
    const currentEditor = targetRegion === 'right' ? secondaryEditor : editor;
    const currentContent =
      targetRegion === fileTreeStore.activeRegion && previewMode.value === 'realtime' && realtimeVditor
        ? realtimeVditor.getValue()
        : currentEditor
          ? currentEditor.getValue()
          : rawContent.value;
    fileTreeStore.updateTabContent(currentPath, currentContent);
  }

  // 标记本次切换已经完成旧内容回写，selectedFile watcher 不再重复读取编辑器内容。
  skipPreviousTabSyncOnce = true
  fileTreeStore.setActiveTab(path, targetRegion);
};

const closeTab = async (path, region) => {
  flushPythonCellDrafts()
  const targetRegion = region || 'left';
  const regionActivePath = targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  const regionEditor = targetRegion === 'right' ? secondaryEditor : editor;

  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.openTabs.length === 1) {
    if (regionEditor && regionActivePath === path) {
      const content = regionEditor.getValue();
      fileTreeStore.updateTabContent(path, content);
    }

    fileTreeStore.setLayoutMode('single');
    if (fileTreeStore.activeTab !== path) {
      fileTreeStore.setActiveTab(path, 'left');
    }
    return;
  }

  if (regionEditor && regionActivePath === path) {
    const content = regionEditor.getValue();
    fileTreeStore.updateTabContent(path, content);
  }

  const closingTab = fileTreeStore.openTabs.find((t) => t.path === path)
  if (closingTab?.dirty) {
    const ok = await confirmDialog(`文件「${closingTab.fileName || path}」尚未保存，确定要关闭并丢弃更改吗？`, {
      title: '未保存的更改',
      kind: 'warning'
    })
    if (!ok) return
  }

  fileTreeStore.closeTab(path, targetRegion);
  void dropPythonSessionForPath(path, { silent: true, reason: 'close-tab' })
  if (fileTreeStore.openTabs.length > 0) {
    const tab = fileTreeStore.openTabs.find(t => t.path === fileTreeStore.activeTab);
    if (tab) {
      updateEditorContent(tab.content);
    }
  } else {
    fileTreeStore.setActiveTab(null);
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
    }
    if (secondaryEditor) {
      secondaryEditor.setValue('');
      secondaryEditor.getModel().setValue('');
    }
    renderedHtml.value = '';
  }
};

const applyHighlights = () => {
  if (!editor) return;

  if (editorStore.decorations?.length) {
    editor.deltaDecorations(editorStore.decorations, []);
    editorStore.decorations = [];
  }

  if (!editorStore.highlightInfo) return;

  const model = editor.getModel();
  if (!model) return;

  const { lineNumber, matches = [], text } = editorStore.highlightInfo;

  logger.info("WWW>>> applyHighlights", { lineNumber, matches, text });

  let newDecorations = [];

  // 检查行号合法性
  const totalLines = model.getLineCount();
  const validLineNumber = typeof lineNumber === 'number' && lineNumber >= 1 && lineNumber <= totalLines;

  if (matches && matches.length > 0 && validLineNumber) {
    const lineContent = model.getLineContent(lineNumber);
    const lineLength = lineContent.length;

    newDecorations = matches
      .map(match => {
        let start = Math.max(0, Math.min(match.start, lineLength));
        let end = Math.max(start, Math.min(match.end, lineLength));
        if (start >= end) return null;
        return {
          range: new monaco.Range(
            lineNumber,
            start + 1,
            lineNumber,
            end + 1
          ),
          options: {
            inlineClassName: 'search-highlight',
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        };
      })
      .filter(Boolean);

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (matches.length > 0) {
      // 设计说明：
      // 1. 搜索结果面板点击后，用户期待的是“直接落到命中的关键字上”，而不是只滚到命中行附近。
      // 2. 因此这里除了 reveal，还同步设置 selection，让 Monaco 把首个命中片段真正聚焦出来。
      // 3. 这样即便同一行里存在多个词，界面也会稳定落到后端返回的首个匹配区间。
      const first = matches[0];
      const start = Math.max(0, Math.min(first.start, lineLength));
      const end = Math.max(start, Math.min(first.end, lineLength));
      const focusRange = new monaco.Range(
        lineNumber,
        start + 1,
        lineNumber,
        Math.max(start + 1, end + 1)
      );
      logger.info('[Search] focus exact keyword range from structured matches', {
        lineNumber,
        start,
        end,
        text,
      });
      editor.setSelection(focusRange);
      editor.revealRangeInCenter(focusRange);
    } else {
      editor.revealLineInCenter(lineNumber);
    }
  } else if (typeof text === 'string' && text.trim()) {
    const query = text.trim();
    const foundMatches = model.findMatches(query, false, false, true, null, true);

    newDecorations = foundMatches.map(match => ({
      range: match.range,
      options: {
        inlineClassName: 'search-highlight',
        stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
      }
    }));

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (foundMatches.length > 0) {
      const focusRange = foundMatches[0].range;
      logger.info('[Search] focus exact keyword range from text fallback', {
        lineNumber,
        text: query,
        startLineNumber: focusRange.startLineNumber,
        startColumn: focusRange.startColumn,
        endLineNumber: focusRange.endLineNumber,
        endColumn: focusRange.endColumn,
      });
      editor.setSelection(focusRange);
      editor.revealRangeInCenter(focusRange);
    }
  }
};

// 监听编辑器store的变化
watch(() => editorStore.currentFile, (file) => {
  if (file && editor) {
    logger.info("WWW>>> editorStore.currentFile", file)
    sourceEditorProgrammaticSyncDepth += 1
    try {
      editor.setValue(file.content);
    } finally {
      sourceEditorProgrammaticSyncDepth = Math.max(0, sourceEditorProgrammaticSyncDepth - 1)
    }
    nextTick(applyHighlights);
  }
});

watch(() => editorStore.highlightInfo, () => {
  if (editor) {
    logger.info("WWW>>> editorStore.highlightInfo", editorStore.highlightInfo)
    applyHighlights();
  }
}, { deep: true });


const md = new MarkdownIt()
// 设计说明：
// 1. 右侧大纲的真实数据源应该始终是 Markdown 标题本身，而不是 [TOC] 目录占位或某个渲染模式的临时结果。
// 2. 这里把“标题提取”抽成纯函数，并显式传入 filePath，避免 debounce 执行时读取到瞬时失效的 activeTab，误把大纲清空。
// 3. 当探索者模式暂时回传空大纲时，也能复用同一套解析逻辑做兜底，保证编辑 / 预览 / 探索者三种模式表现一致。
const extractMarkdownOutlineEntries = (content, filePath, reason = 'unknown') => {
  const normalizedPath = String(filePath || selectedFile.value?.path || activeTab.value?.path || '').trim()
  const lowerPath = normalizedPath.toLowerCase()
  if (!lowerPath.endsWith('.md') && !lowerPath.endsWith('.markdown')) {
    logger.info('[Outline] skip extraction because file is not markdown', {
      reason,
      filePath: normalizedPath,
    })
    return []
  }

  const outlineSource = maskPythonCellBlocksForMarkdown(content)
  const tokens = md.parse(outlineSource, {})
  const result = []
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1))
      const nextToken = tokens[idx + 1]
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: token.map ? token.map[0] + 1 : 1
        })
      }
    }
  })

  logger.info('[Outline] extracted markdown headings', {
    reason,
    filePath: normalizedPath,
    count: result.length,
    contentLength: String(content || '').length,
  })
  return result
}

function generateOutline(content, filePath, reason = 'unknown') {
  const result = extractMarkdownOutlineEntries(content, filePath, reason)
  outlineStore.setOutline(result)
}

const generateOutlineDebounced = debounce(({ content, filePath, reason }) => {
  generateOutline(content, filePath, reason || 'debounced')
}, 200)

const syncNotebookOutlineState = async (reason = 'unknown') => {
  if (!(editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value)) {
    return false
  }

  await nextTick()
  const outline = notebookViewRef.value?.getOutline?.() || []
  const normalizedOutline = Array.isArray(outline) ? outline : []
  const fallbackOutline = normalizedOutline.length
    ? normalizedOutline
    : extractMarkdownOutlineEntries(rawContent.value, selectedFile.value?.path || activeTab.value?.path || '', `${reason}-fallback-source`)
  outlineStore.setOutline(fallbackOutline)
  outlineStore.setJumpToLine(jumpToLine)

  logger.info('[Outline] synced notebook outline state', {
    reason,
    count: fallbackOutline.length,
    notebookCount: normalizedOutline.length,
    usedSourceFallback: !normalizedOutline.length && fallbackOutline.length > 0,
    hasNotebookView: !!notebookViewRef.value,
  })
  return true
}

const handleNotebookOutlineChange = (outline) => {
  if (!(editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value)) return
  const normalizedOutline = Array.isArray(outline) ? outline : []
  const finalOutline = normalizedOutline.length
    ? normalizedOutline
    : extractMarkdownOutlineEntries(rawContent.value, selectedFile.value?.path || activeTab.value?.path || '', 'notebook-outline-event-fallback')
  outlineStore.setOutline(finalOutline)
  outlineStore.setJumpToLine(jumpToLine)
  logger.info('[Outline] received notebook outline change event', {
    count: finalOutline.length,
    notebookCount: normalizedOutline.length,
    usedSourceFallback: !normalizedOutline.length && finalOutline.length > 0,
    surfaceMode: editorSurfaceMode.value,
  })
}

function jumpToLine(line) {
  if (editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value) {
    const handledByNotebook = !!notebookViewRef.value?.jumpToOutlineLine?.(line)
    logger.info('[Outline] jump routed to notebook surface', {
      line: Number(line || 0),
      handledByNotebook,
    })
    if (handledByNotebook) {
      return
    }
  }

  if (mode.value === 'preview') {
    recordPreviewLocation('outline');
    if (scrollPreviewToLine(line, { behavior: 'smooth' })) {
      return
    }
    nextTick(() => {
      if (scrollPreviewToLine(line, { behavior: 'smooth' })) return
      window.requestAnimationFrame(() => {
        scrollPreviewToLine(line, { behavior: 'smooth' })
      })
    })
    return
  }
  syncEditorToLine(line, { focus: true })
}

// 设计说明：
// 1. 源码编辑模式沿用现有 markdown-it 解析，保证大纲结果和普通编辑/预览模式一致。
// 2. 探索者模式改为优先从 notebook 视图读取大纲与跳转目标，避免点击大纲后把用户带回隐藏的源码编辑器。
// 3. 这里保留日志，后续如果再出现“大纲为空/跳转无效”，可以直接从模式分支上定位问题。
watch([() => fileTreeStore.activeTab, rawContent, editorSurfaceMode, canUseNotebookSurface, notebookViewRef], async ([tabPath, content, surfaceMode, notebookEnabled]) => {
  if (surfaceMode === 'notebook' && notebookEnabled) {
    generateOutlineDebounced.cancel?.()
    await syncNotebookOutlineState('watch-outline-state')
    return
  }

  outlineStore.setJumpToLine(jumpToLine)
  logger.info('[Outline] synced source outline state', {
    tabPath: String(tabPath || ''),
    contentLength: String(content || '').length,
    surfaceMode: String(surfaceMode || ''),
  })
  generateOutlineDebounced({
    content,
    filePath: String(tabPath || selectedFile.value?.path || ''),
    reason: 'watch-source-outline-state',
  })
}, { immediate: true })

const handleLspSettingChanged = (evt) => {
  const hasExplicitEnabled = typeof evt?.detail?.enabled === 'boolean'
  const enabled = hasExplicitEnabled ? !!evt.detail.enabled : !!lspEnabled.value
  lspEnabled.value = enabled
  logger.info('[DirectLsp] editor LSP setting changed', {
    enabled,
    hasExplicitEnabled,
  })
  applyMonacoThemeByLsp(enabled)
  scheduleSourceSyntaxHighlight(editor, 'setting')
  scheduleSourceSyntaxHighlight(secondaryEditor, 'setting')

  const mainModel = editor?.getModel?.()
  const rightPath = fileTreeStore.activeTabRight || ''
  const rightModel = secondaryEditor?.getModel?.()
  if (mainModel) {
    refreshDirectLspSuggestMode(editor, mainModel.getLanguageId?.() || 'plaintext')
  }
  if (rightModel) {
    refreshDirectLspSuggestMode(secondaryEditor, rightModel.getLanguageId?.() || 'plaintext')
  }

  if (!enabled) {
    disposeAllLspClients()
    return
  }

  resetLspDisabled()

  if (mainModel) {
    enableLspForEditorLanguage(editor, mainModel.getLanguageId?.() || 'plaintext', selectedFile.value?.path || '')
  }

  if (rightModel) {
    enableLspForEditorLanguage(secondaryEditor, rightModel.getLanguageId?.() || 'plaintext', rightPath)
  }
}

watch(lspEnabled, (enabled) => {
  refreshDirectLspSuggestMode(editor, editor?.getModel?.()?.getLanguageId?.() || '')
  refreshDirectLspSuggestMode(secondaryEditor, secondaryEditor?.getModel?.()?.getLanguageId?.() || '')
  applyMonacoThemeByLsp(enabled)
  scheduleSourceSyntaxHighlight(editor, 'setting')
  scheduleSourceSyntaxHighlight(secondaryEditor, 'setting')
})

// 生命周期
watch(
  () => settingsStore.appearanceTheme,
  async (theme, prevTheme) => {
    if (!theme || theme === prevTheme) return

    const previewVisible = !!(showPreview.value && previewContainer.value)
    logger.info('[theme] appearance theme changed', {
      previousTheme: prevTheme || '',
      theme,
      previewMode: previewMode.value,
      canPreview: canPreview.value,
      previewVisible,
    })

    applyMonacoTheme('appearanceTheme')

    if (previewVisible && previewMode.value !== 'realtime' && canPreview.value && rawContent.value) {
      logger.info('[theme] refresh markdown preview for appearance theme change', {
        theme,
        previewMode: previewMode.value,
        previewVisible,
      })
      renderDebounced(rawContent.value)
    }

    await nextTick()
    editor?.layout?.()
    secondaryEditor?.layout?.()
  },
  { flush: 'post' }
)

onMounted(() => {
  restorePythonConsoleState()
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('guaxi:lsp-setting-changed', handleLspSettingChanged);
  document.addEventListener('selectionchange', handleGlobalSelectionChange, true)
  document.addEventListener('mousedown', handleGlobalMouseDownForPreviewToolbar, true)
  document.addEventListener('mousedown', handlePythonRuntimeWorkbenchPointerDown, true)
  window.addEventListener('keydown', handlePythonRuntimeWorkbenchKeydown, true)
  outlineStore.setJumpToLine(jumpToLine)
  initGitService()
  void refreshLspEnabledSetting()

  restoreHomePreviewMode('mounted')
  void (async () => {
    await refreshPythonRuntimeStatus('mounted')
    await ensurePythonRuntimeInitialized('mounted', { silent: true, notify: false })
  })()

  const intervalMs = Number(editorStore.autoSaveIntervalMs) || AUTO_SAVE_FALLBACK_INTERVAL_MS
  if (editorStore.autoSaveEnabled !== false && intervalMs > 0) {
    autoSaveTimerId = window.setInterval(autoSaveDirtyTabs, intervalMs)
  }

  disposeLspConnectionStateListener = onLspConnectionStateChange(({ languageId, state }) => {
    const normalizedLanguageId = String(languageId || '').trim().toLowerCase()
    if (!LSP_SUPPORTED_LANGUAGES.has(normalizedLanguageId)) return
    scheduleSourceSyntaxHighlight(editor, 'lsp-state')
    scheduleSourceSyntaxHighlight(secondaryEditor, 'lsp-state')
    if (state !== 'connected') return
    logger.debug('[DirectLsp] connected without auto-opening suggest widget', {
      languageId: normalizedLanguageId,
    })
  })
});

onActivated(() => {
  restoreHomePreviewMode('activated')
})

onUnmounted(() => {
  if (previewRenderTimer) {
    window.clearTimeout(previewRenderTimer)
    previewRenderTimer = null
  }
  disposePreviewMarkdownWorker('editor-unmount')
  if (notebookDiskPersistTimer) {
    const pendingNotebookContent = String(rawContent.value ?? '')
    window.clearTimeout(notebookDiskPersistTimer)
    notebookDiskPersistTimer = null
    void persistNotebookToDisk('editor-unmount', pendingNotebookContent)
  }
  stopResizePythonConsole()
  persistPythonConsoleState()
  flushPythonCellDrafts()
  disposePythonCellManagers()
  stopSelectionAudioPlayback()
  closePreviewSearch()
  cleanupPreviewPdfBlobUrls()
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('guaxi:lsp-setting-changed', handleLspSettingChanged);
  document.removeEventListener('selectionchange', handleGlobalSelectionChange, true)
  document.removeEventListener('mousedown', handleGlobalMouseDownForPreviewToolbar, true)
  document.removeEventListener('mousedown', handlePythonRuntimeWorkbenchPointerDown, true)
  window.removeEventListener('keydown', handlePythonRuntimeWorkbenchKeydown, true)
  disposeLspConnectionStateListener?.()
  disposeLspConnectionStateListener = null
  clearPreviewSelectionState()
  destroyRealtimeVditor();

  if (monacoCtrlClickDisposable) {
    monacoCtrlClickDisposable.dispose();
    monacoCtrlClickDisposable = null;
  }
  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }
  if (monacoLinkLikeDecorDisposable) {
    monacoLinkLikeDecorDisposable.dispose();
    monacoLinkLikeDecorDisposable = null;
  }
  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
  }

  if (autoSaveTimerId) {
    window.clearInterval(autoSaveTimerId)
    autoSaveTimerId = null
  }

  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }

  if (editorEffectsLeft) {
    editorEffectsLeft.destroy();
    editorEffectsLeft = null;
  }
  editor?.dispose();
  editor = null;
  editorStore.setEditorInstance(null); // 清除store中的引用

  if (gitDiffEditor) {
    gitDiffEditor.dispose()
    gitDiffEditor = null
  }
  if (gitDiffOriginalModel) {
    gitDiffOriginalModel.dispose()
    gitDiffOriginalModel = null
  }
  if (gitDiffModifiedModel) {
    gitDiffModifiedModel.dispose()
    gitDiffModifiedModel = null
  }
  if (gitDiffContainerEl) {
    gitDiffContainerEl.remove()
    gitDiffContainerEl = null
  }

  if (vimModeInstance) {
    vimModeInstance.dispose();
  }

  markdownFenceCompletionModels.forEach((m) => {
    try {
      releaseVsCodeTextModel(
        m?.uri,
        getMarkdownFenceVscodeModelOwner(m?.getLanguageId?.() || ''),
        'guaxi-editor-unmount'
      )
      m.dispose?.()
    } catch {
    }
  })
  markdownFenceCompletionModels.clear()
  clearGlobalMarkdownLanguageFeatureRegistration()
  disposeFeatureDisposables(markdownLanguageFeatureDisposables)
  disposeFeatureDisposables(filesystemPathCompletionDisposables)
  markdownFeaturesRegistered = false
  filesystemPathCompletionRegistered = false
  disposeAllVsCodeTextModelsForOwner(PRIMARY_EDITOR_VSCODE_MODEL_OWNER, 'guaxi-editor-unmount')
  disposeAllVsCodeTextModelsForOwner(SECONDARY_EDITOR_VSCODE_MODEL_OWNER, 'guaxi-editor-unmount')
  disposeAllLspClients()
});

</script>


<style scoped>
.guaxi-editor {
  --editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  --pycell-card-border: rgba(15, 23, 42, 0.1);
  --pycell-card-bg:
    linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(248, 250, 252, 0.98));
  --pycell-card-shadow:
    0 10px 28px rgba(15, 23, 42, 0.07),
    0 1px 0 rgba(255, 255, 255, 0.82) inset;
  --pycell-header-bg:
    linear-gradient(180deg, rgba(244, 248, 255, 0.96), rgba(240, 246, 252, 0.86));
  --pycell-header-border: rgba(15, 23, 42, 0.08);
  --pycell-title: #10233f;
  --pycell-meta: #4f6077;
  --pycell-prompt: #0f766e;
  --pycell-btn-border: rgba(15, 23, 42, 0.12);
  --pycell-btn-bg: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(241, 245, 249, 0.96));
  --pycell-btn-text: #17355f;
  --pycell-btn-hover-border: rgba(59, 130, 246, 0.35);
  --pycell-btn-hover-bg: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(235, 244, 255, 0.98));
  --pycell-btn-hover-shadow: 0 6px 16px rgba(59, 130, 246, 0.16);
  --pycell-btn-focus-ring: 0 0 0 3px rgba(59, 130, 246, 0.18);
  --pycell-editor-bg:
    linear-gradient(180deg, rgba(250, 252, 255, 0.98), rgba(246, 249, 252, 0.98));
  --pycell-editor-focus-bg:
    linear-gradient(180deg, rgba(255, 255, 255, 0.99), rgba(247, 250, 255, 0.99));
  --pycell-editor-text: #0f172a;
  --pycell-panel-bg: rgba(248, 250, 252, 0.72);
  --pycell-panel-text: #1f2937;
  --pycell-output-label: #2563eb;
  --pycell-session-label: #0f766e;
  --pycell-output-bg: rgba(255, 255, 255, 0.52);
  --pycell-output-border: rgba(15, 23, 42, 0.08);
  --pycell-output-text: #1f2937;
  --pycell-empty: #5f6f86;
  --pycell-var-name: #10233f;
  --pycell-var-type: #0f766e;
  --pycell-var-preview: #506177;
  /* Vim 状态栏跟随 Guaxi 编辑器主题变量，避免 monaco-vim 默认浅色样式在主题切换后残留。 */
  --vim-status-bg: var(--guaxi-status-bar-background, var(--guaxi-panel-bg, #f8f8f8));
  --vim-status-border: var(--guaxi-status-bar-border, var(--guaxi-border, #e0e0e0));
  --vim-status-fg: var(--guaxi-status-bar-foreground, var(--guaxi-text-primary, #1f2328));
  --vim-status-input-bg: var(--guaxi-content-bg, #ffffff);
  --vim-status-input-border: var(--guaxi-border, #d0d7de);
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor) {
  /* 设计说明：
     1. monaco-vscode-api 会注入 VSCode 兼容 CSS 变量，查找框和 sticky scroll 会优先读取这些变量。
     2. Guaxi 编辑器只需要组件内部跟随自有暗色 UI，因此在 .guaxi-editor 作用域内重置 widget/input/sticky scroll 变量。
     3. 这里和 defineTheme 的颜色保持一致，避免滚动吸顶行号、查找框边框在暗色模式下短暂回落到亮色默认值。 */
  --vscode-editorWidget-background: #282c34;
  --vscode-editorWidget-foreground: #abb2bf;
  --vscode-widget-border: #3b4048;
  --vscode-widget-shadow: rgba(0, 0, 0, 0.42);
  --vscode-input-background: #21252b;
  --vscode-input-foreground: #abb2bf;
  --vscode-input-border: #3b4048;
  --vscode-input-placeholderForeground: #7f8898;
  --vscode-focusBorder: rgba(82, 139, 255, 0.5);
  --vscode-editorStickyScroll-background: #282c34;
  --vscode-editorStickyScrollGutter-background: #282c34;
  --vscode-editorStickyScrollHover-background: #2c313a;
  --vscode-editorStickyScroll-border: transparent;
  --vscode-editorStickyScroll-shadow: rgba(0, 0, 0, 0.42);
  --vscode-editorLineNumber-foreground: #5c6370;
  --pycell-card-border: rgba(148, 163, 184, 0.22);
  --pycell-card-bg:
    radial-gradient(circle at top right, rgba(56, 189, 248, 0.16), transparent 28%),
    linear-gradient(180deg, rgba(12, 18, 32, 0.99), rgba(15, 23, 42, 0.97));
  --pycell-card-shadow:
    0 20px 38px rgba(2, 6, 23, 0.42),
    0 1px 0 rgba(148, 163, 184, 0.08) inset;
  --pycell-header-bg:
    linear-gradient(180deg, rgba(18, 27, 46, 0.95), rgba(15, 23, 42, 0.78));
  --pycell-header-border: rgba(148, 163, 184, 0.18);
  --pycell-title: #e5eefc;
  --pycell-meta: #c7d4e5;
  --pycell-prompt: #5eead4;
  --pycell-btn-border: rgba(148, 163, 184, 0.34);
  --pycell-btn-bg: linear-gradient(180deg, rgba(30, 41, 59, 0.98), rgba(22, 30, 47, 0.98));
  --pycell-btn-text: #eff6ff;
  --pycell-btn-hover-border: rgba(125, 211, 252, 0.58);
  --pycell-btn-hover-bg: linear-gradient(180deg, rgba(37, 50, 72, 0.99), rgba(29, 41, 61, 0.99));
  --pycell-btn-hover-shadow: 0 8px 20px rgba(14, 165, 233, 0.16);
  --pycell-btn-focus-ring: 0 0 0 3px rgba(56, 189, 248, 0.22);
  --pycell-editor-bg:
    linear-gradient(180deg, rgba(10, 16, 28, 0.99), rgba(14, 22, 37, 0.99));
  --pycell-editor-focus-bg:
    linear-gradient(180deg, rgba(14, 22, 37, 1), rgba(18, 28, 46, 1));
  --pycell-editor-text: #f8fbff;
  --pycell-panel-bg: rgba(13, 20, 34, 0.9);
  --pycell-panel-text: #f6f9ff;
  --pycell-output-label: #60a5fa;
  --pycell-session-label: #5eead4;
  --pycell-output-bg: rgba(15, 23, 42, 0.82);
  --pycell-output-border: rgba(148, 163, 184, 0.16);
  --pycell-output-text: #f8fbff;
  --pycell-empty: #c7d4e5;
  --pycell-var-name: #e5eefc;
  --pycell-var-type: #5eead4;
  --pycell-var-preview: #c7d4e5;
  --vim-status-bg: var(--guaxi-status-bar-background, var(--guaxi-panel-bg, #21252b));
  --vim-status-border: var(--guaxi-status-bar-border, var(--guaxi-border, #3e4451));
  --vim-status-fg: var(--guaxi-status-bar-foreground, var(--guaxi-text-primary, #abb2bf));
  --vim-status-input-bg: var(--guaxi-content-bg, #282c34);
  --vim-status-input-border: var(--guaxi-border, #3e4451);
}

.editor-header {
  position: sticky;
  top: 0;
  z-index: 50;
}

.breadcrumb-bar {
  display: flex;
  align-items: center;
  height: 30px;
  min-height: 30px;
  padding: 0 8px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  box-sizing: border-box;
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.breadcrumb-scroll {
  flex: 1 1 auto;
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.22) transparent;
}

.breadcrumb-scroll::-webkit-scrollbar {
  height: 6px;
}

.breadcrumb-scroll::-webkit-scrollbar-track {
  background: transparent;
}

.breadcrumb-scroll::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.18);
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.breadcrumb-scroll::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.28);
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll {
  scrollbar-color: rgba(255, 255, 255, 0.22) transparent;
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.18);
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll::-webkit-scrollbar-thumb:hover {
  background-color: rgba(255, 255, 255, 0.26);
}

.breadcrumb-bar :deep(.n-breadcrumb) {
  white-space: nowrap;
}

.breadcrumb-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 320px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: bottom;
  cursor: pointer;
  user-select: none;
  padding: 0 4px;
  border-radius: 4px;
}

.breadcrumb-item.disabled {
  cursor: default;
  opacity: 0.6;
}

.breadcrumb-icon {
  flex: 0 0 auto;
  opacity: 0.85;
}

.breadcrumb-label {
  display: inline-block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.breadcrumb-item:hover {
  color: var(--n-color-primary, #18a058);
  background: rgba(127, 127, 127, 0.12);
}

.breadcrumb-item.disabled:hover {
  color: inherit;
  background: transparent;
}

.breadcrumb-picker-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 220px;
  max-width: 420px;
}

.breadcrumb-picker-icon {
  flex: 0 0 auto;
  opacity: 0.9;
}

.breadcrumb-picker-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.breadcrumb-picker-item.active .breadcrumb-picker-label {
  font-weight: 650;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 4px 8px;
  height: 30px;
  min-height: 30px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  position: relative;
  width: 100%;
  box-sizing: border-box;
  min-width: 0;
  flex-wrap: nowrap;
  overflow: visible;
}

.toolbar-editing-actions {
  display: flex;
  gap: 4px;
  align-items: center;
  flex: 1 1 auto;
  min-width: 0;
  flex-wrap: nowrap;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}

.toolbar-editing-actions::-webkit-scrollbar {
  height: 0;
}

.floating-md-toolbar {
  position: absolute;
  z-index: 9;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: 8px;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
}

.floating-md-toolbar :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 6px;
}

.floating-md-toolbar :deep(.floating-toolbar-icon-button) {
  width: 24px;
  min-width: 24px;
  justify-content: center;
}

.floating-md-toolbar :deep(.floating-toolbar-icon-button .n-base-loading) {
  margin: 0;
}

.assistant-translate-dialog-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  color: var(--guaxi-text-secondary);
}

.assistant-translate-dialog-actions {
  margin-top: 12px;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}


.floating-table-toolbar {
  position: absolute;
  z-index: 9;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: 8px;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
}

.floating-table-toolbar :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 6px;
}

.floating-table-toolbar-divider {
  width: 1px;
  height: 16px;
  background-color: var(--guaxi-border);
  margin: 0 2px;
}

.table-insert-popover {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  width: fit-content;
  max-width: min(240px, calc(100vw - 32px));
  padding: 6px;
}

.table-insert-hint {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.9;
}

.table-insert-grid {
  display: inline-grid;
  width: fit-content;
  grid-template-columns: repeat(10, 16px);
  gap: 4px;
  padding: 6px;
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  user-select: none;
}

.table-insert-cell {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 75%, transparent);
}

.table-insert-cell.active {
  background: rgba(74, 202, 219, 0.65);
  border-color: rgba(74, 202, 219, 0.9);
}

.table-insert-controls {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.table-insert-inputs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.table-insert-inputs :deep(.n-input-number) {
  width: 74px;
}

.table-insert-mul {
  opacity: 0.7;
}

.toolbar-editing-actions :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.toolbar-editing-actions :deep(.n-button:hover) {
  background-color: var(--guaxi-panel-bg);
}

.toolbar-spacer {
  flex-grow: 1;
  min-width: 16px;
  /* 添加最小间距 */
  flex-shrink: 1;
  /* 关键修复4: 允许间距缩小 */
}

.toolbar-actions {
  display: flex;
  gap: 14px;
  /* 右侧工具按钮整体稍微拉开，避免新增入口挤在一起 */
  align-items: center;
  min-width: 0;
  /* 关键修复5: 允许操作区域缩小 */
  flex-shrink: 0;
  /* 关键修复6: 防止按钮被压缩 */
}

.toolbar-actions :deep(.n-button) {
  flex-shrink: 0;
}

.toolbar-action-python {
  order: 999;
}

.python-runtime-entry {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 82%, transparent);
  background: linear-gradient(135deg,
      color-mix(in srgb, var(--guaxi-panel-bg) 92%, #82d8ff 8%),
      color-mix(in srgb, var(--guaxi-panel-bg) 94%, #9af0ce 6%));
  color: var(--guaxi-text-primary);
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease, border-color 0.15s ease;
  flex-shrink: 0;
}

.python-runtime-entry:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 40%, var(--guaxi-border));
}

.python-runtime-entry.busy {
  opacity: 0.72;
}

.python-runtime-entry.active {
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 46%, var(--guaxi-border));
  box-shadow: 0 12px 26px rgba(74, 202, 219, 0.16);
}

.python-runtime-anchor {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: 6px;
  padding-left: 12px;
  /* 把 Python 相关入口单独分组，视觉上更接近 VS Code 的工具栏节奏 */
  border-left: 1px solid color-mix(in srgb, var(--guaxi-border) 78%, transparent);
}

.python-runtime-workbench {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  z-index: 80;
}

.python-runtime-panel-fade-enter-active,
.python-runtime-panel-fade-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.python-runtime-panel-fade-enter-from,
.python-runtime-panel-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.98);
}

.python-runtime-entry__label {
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.python-runtime-entry__dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #f59f00;
  box-shadow: 0 0 0 3px rgba(245, 159, 0, 0.16);
}

.python-runtime-entry__dot.is-bundled {
  background: #18a058;
  box-shadow: 0 0 0 3px rgba(24, 160, 88, 0.16);
}

.python-runtime-entry__dot.is-system {
  background: #2080f0;
  box-shadow: 0 0 0 3px rgba(32, 128, 240, 0.16);
}

.python-runtime-entry__dot.is-missing {
  background: #d03050;
  box-shadow: 0 0 0 3px rgba(208, 48, 80, 0.16);
}

.python-runtime-popover {
  width: min(420px, calc(100vw - 32px));
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: min(50vh, calc(100dvh - 140px));
  overflow-y: auto;
  padding-right: 4px;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: rgba(120, 120, 120, 0.38) transparent;
}

.python-runtime-popover::-webkit-scrollbar {
  width: 7px;
}

.python-runtime-popover::-webkit-scrollbar-track {
  background: transparent;
}

.python-runtime-popover::-webkit-scrollbar-thumb {
  background: rgba(120, 120, 120, 0.35);
  border-radius: 999px;
}

.python-runtime-popover::-webkit-scrollbar-thumb:hover {
  background: rgba(120, 120, 120, 0.52);
}

.python-runtime-popover__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.python-runtime-popover__title {
  font-size: 14px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.python-runtime-popover__subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-ghost-btn,
.python-runtime-chip-btn,
.python-runtime-install__btn {
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
}

.python-runtime-ghost-btn:hover,
.python-runtime-chip-btn:hover,
.python-runtime-install__btn:hover {
  background: color-mix(in srgb, var(--guaxi-panel-bg) 84%, var(--primary-color, #4acadb));
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 45%, var(--guaxi-border));
}

.python-runtime-ghost-btn {
  min-width: 56px;
  height: 28px;
  padding: 0 10px;
}

.python-runtime-meta {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.python-runtime-meta__item {
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 88%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.python-runtime-meta__label {
  display: block;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-meta__value {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-primary);
  word-break: break-word;
}

.python-runtime-guide,
.python-runtime-archive-card {
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 88%, transparent);
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--guaxi-panel-bg) 96%, #8fd8ff 4%), var(--guaxi-panel-bg));
}

.python-runtime-guide.needs-setup {
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 45%, var(--guaxi-border));
  box-shadow: 0 10px 24px rgba(74, 202, 219, 0.08);
}

.python-runtime-guide__title,
.python-runtime-archive-card__title {
  font-size: 13px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.python-runtime-guide__text,
.python-runtime-archive-card__meta,
.python-runtime-archive-card__warnings {
  margin-top: 6px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-guide__paths {
  margin-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.python-runtime-guide__path {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.python-runtime-guide__path-label {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-guide__path-value,
.python-runtime-archive-card__path {
  display: block;
  padding: 8px 10px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-content-bg) 88%, white 12%);
  color: var(--guaxi-text-primary);
  font-size: 11px;
  line-height: 1.4;
  word-break: break-all;
}

.python-runtime-guide__steps {
  margin-top: 10px;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.python-runtime-guide__step {
  display: inline-flex;
  align-items: center;
  min-height: 26px;
  padding: 0 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 88%, transparent);
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 88%, transparent);
}

.python-runtime-guide__step.done {
  color: #12743b;
  border-color: rgba(24, 160, 88, 0.32);
  background: rgba(24, 160, 88, 0.1);
}

.python-runtime-archive-card__meta,
.python-runtime-archive-card__warnings {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.python-runtime-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.python-runtime-chip-btn {
  min-height: 32px;
  padding: 0 12px;
  white-space: nowrap;
}

.python-runtime-chip-btn[disabled],
.python-runtime-ghost-btn[disabled],
.python-runtime-package-pill--button[disabled] {
  opacity: 0.48;
  cursor: not-allowed;
  transform: none;
}

.python-runtime-install {
  display: flex;
  align-items: center;
  gap: 8px;
}

.python-runtime-install__input {
  flex: 1 1 auto;
  min-width: 0;
  height: 34px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.python-runtime-install__btn {
  height: 34px;
  padding: 0 14px;
  white-space: nowrap;
}

.python-runtime-install__btn[disabled] {
  opacity: 0.48;
  cursor: not-allowed;
}

.python-runtime-packages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.python-runtime-packages__title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-packages__list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.python-runtime-package-pill {
  display: inline-flex;
  align-items: center;
  min-height: 26px;
  padding: 0 10px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 88%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: var(--guaxi-text-primary);
  font-size: 12px;
}

.python-runtime-package-pill--button {
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
}

.python-runtime-package-pill--button:hover {
  transform: translateY(-1px);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 80%, var(--primary-color, #4acadb));
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 45%, var(--guaxi-border));
}

.python-runtime-panel {
  width: min(440px, calc(100vw - 28px));
  gap: 14px;
  padding: 14px;
  border-radius: 20px;
  background:
    radial-gradient(circle at top left, rgba(255, 255, 255, 0.98), rgba(247, 250, 253, 0.96)),
    linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(242, 247, 251, 0.98));
  box-shadow:
    0 22px 48px rgba(15, 23, 42, 0.14),
    inset 0 1px 0 rgba(255, 255, 255, 0.7);
}

.python-runtime-panel.is-dark {
  /* 设计说明：Python 工作台是编辑器工具面板，暗色模式应跟随 Guaxi/One Dark 的平面层级，而不是使用独立深蓝渐变。 */
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-panel-bg, #21252b);
  box-shadow:
    0 18px 42px rgba(0, 0, 0, 0.36),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
}

.python-runtime-panel.is-dark .python-runtime-meta__item,
.python-runtime-panel.is-dark .python-runtime-package-pill,
.python-runtime-panel.is-dark .python-runtime-guide,
.python-runtime-panel.is-dark .python-runtime-archive-card,
.python-runtime-panel.is-dark .python-runtime-status-card,
.python-runtime-panel.is-dark .python-runtime-operation-card,
.python-runtime-panel.is-dark .python-runtime-install-card,
.python-runtime-panel.is-dark .python-runtime-package-browser,
.python-runtime-panel.is-dark .python-runtime-package-log,
.python-runtime-panel.is-dark .python-runtime-package-row {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-content-bg, #282c34);
}

.python-runtime-panel.is-dark .python-runtime-package-log__output {
  background: var(--guaxi-content-bg, #282c34);
  border-color: var(--guaxi-border, #3e4451);
}

.python-runtime-panel.is-dark .python-runtime-package-log__output.is-error {
  background: var(--guaxi-content-bg, #282c34);
  border-color: color-mix(in srgb, #e06c75 38%, var(--guaxi-border, #3e4451));
}

.python-runtime-panel.is-dark .python-runtime-ghost-btn,
.python-runtime-panel.is-dark .python-runtime-chip-btn,
.python-runtime-panel.is-dark .python-runtime-install__btn,
.python-runtime-panel.is-dark .python-runtime-install__input,
.python-runtime-panel.is-dark .python-runtime-package-browser__search {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-panel-bg, #21252b);
  color: var(--guaxi-text-primary, #abb2bf);
}

.python-runtime-panel.is-dark .python-runtime-package-row__action {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-panel-bg, #21252b);
  color: var(--guaxi-text-primary, #abb2bf);
}

.python-runtime-panel.is-dark .python-runtime-package-row__action:hover {
  border-color: color-mix(in srgb, #61afef 42%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 84%, #abb2bf 16%);
}

.python-runtime-panel.is-dark .python-runtime-package-row__action.is-danger {
  color: #e06c75;
  border-color: color-mix(in srgb, #e06c75 36%, var(--guaxi-border, #3e4451));
}

.python-runtime-panel.is-dark .python-runtime-panel__title,
.python-runtime-panel.is-dark .python-runtime-status-card__value,
.python-runtime-panel.is-dark .python-runtime-section__title,
.python-runtime-panel.is-dark .python-runtime-package-row {
  color: var(--guaxi-text-primary, #abb2bf);
}

.python-runtime-panel.is-dark .python-runtime-panel__subtitle,
.python-runtime-panel.is-dark .python-runtime-status-card__label,
.python-runtime-panel.is-dark .python-runtime-package-browser__empty {
  color: var(--guaxi-text-secondary, #9da5b4);
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-success {
  border-color: color-mix(in srgb, #98c379 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #98c379 12%);
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-warning {
  border-color: color-mix(in srgb, #e5c07b 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #e5c07b 12%);
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-danger {
  border-color: color-mix(in srgb, #e06c75 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #e06c75 12%);
}

.python-runtime-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.python-runtime-panel__title {
  font-size: 15px;
  font-weight: 800;
  color: var(--guaxi-text-primary);
}

.python-runtime-panel__subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-panel__actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.python-runtime-status-card,
.python-runtime-install-card,
.python-runtime-package-browser,
.python-runtime-archive-card {
  border-radius: 16px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 82%, transparent);
  background: rgba(255, 255, 255, 0.82);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
}

.python-runtime-status-card {
  padding: 14px;
  display: grid;
  gap: 10px;
}

.python-runtime-status-card.is-success {
  border-color: rgba(24, 160, 88, 0.28);
  background: linear-gradient(180deg, rgba(240, 253, 244, 0.98), rgba(236, 252, 242, 0.88));
}

.python-runtime-status-card.is-warning {
  border-color: rgba(245, 159, 0, 0.28);
  background: linear-gradient(180deg, rgba(255, 249, 219, 0.98), rgba(255, 244, 199, 0.88));
}

.python-runtime-status-card.is-danger {
  border-color: rgba(208, 48, 80, 0.22);
  background: linear-gradient(180deg, rgba(255, 241, 242, 0.98), rgba(255, 228, 230, 0.88));
}

.python-runtime-status-card__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.python-runtime-status-card__label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-status-card__value {
  font-size: 13px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.python-runtime-operation-card {
  padding: 14px;
  border-radius: 16px;
  border: 1px solid color-mix(in srgb, var(--primary-color, #4acadb) 28%, var(--guaxi-border));
  background: color-mix(in srgb, var(--guaxi-panel-bg) 88%, rgba(74, 202, 219, 0.14));
}

.python-runtime-operation-card__title {
  font-size: 12px;
  font-weight: 800;
  color: var(--guaxi-text-primary);
}

.python-runtime-operation-card__subtitle,
.python-runtime-operation-card__packages {
  margin-top: 6px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.python-runtime-operation-card__progress {
  margin-top: 10px;
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-border) 72%, transparent);
}

.python-runtime-operation-card__progress-bar {
  display: block;
  width: 38%;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--primary-color, #4acadb), color-mix(in srgb, var(--primary-color, #4acadb) 68%, #ffffff));
  animation: python-runtime-progress-slide 1.2s ease-in-out infinite;
}

@keyframes python-runtime-progress-slide {
  0% {
    transform: translateX(-110%);
  }

  100% {
    transform: translateX(320%);
  }
}

.python-runtime-install-card,
.python-runtime-package-browser {
  padding: 14px;
}

.python-runtime-section__title {
  margin-bottom: 10px;
  font-size: 12px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.python-runtime-packages__list--compact {
  margin-top: 10px;
}

.python-runtime-package-browser__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.python-runtime-package-browser__bulkbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
  padding: 10px 12px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 94%, transparent);
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 84%, transparent);
}

.python-runtime-package-browser__bulktext {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.python-runtime-package-browser__bulkactions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.python-runtime-package-browser__header .python-runtime-section__title {
  margin-bottom: 0;
}

.python-runtime-package-browser__search {
  width: min(180px, 48%);
  height: 32px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.python-runtime-package-browser__list {
  max-height: 220px;
  overflow: auto;
  padding-right: 4px;
}

.python-runtime-package-browser__items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.python-runtime-package-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 7px 10px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.3;
  word-break: break-word;
  flex-wrap: nowrap;
}

.python-runtime-package-row__selector {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.python-runtime-package-row__selector input {
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.python-runtime-package-row__name {
  min-width: 0;
  flex: 1 1 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
}

.python-runtime-package-row__actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}

.python-runtime-package-row__action {
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.88);
  color: #17355f;
  font-size: 11px;
  font-weight: 600;
  padding: 4px 10px;
  cursor: pointer;
  transition: background 0.16s ease, border-color 0.16s ease, color 0.16s ease;
}

.python-runtime-package-row__action--icon {
  width: 26px;
  min-width: 26px;
  height: 26px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.python-runtime-package-row__action--icon svg {
  display: block;
}

.python-runtime-package-row__action:hover {
  border-color: rgba(59, 130, 246, 0.35);
  background: rgba(239, 246, 255, 0.95);
}

.python-runtime-package-row__action[disabled] {
  cursor: not-allowed;
  opacity: 0.58;
}

.python-runtime-package-row__action.is-danger {
  color: #b42318;
  border-color: rgba(180, 35, 24, 0.22);
}

.python-runtime-package-log {
  margin-top: 12px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  overflow: hidden;
}

.python-runtime-package-log__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  cursor: pointer;
  list-style: none;
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.python-runtime-package-log__summary::-webkit-details-marker {
  display: none;
}

.python-runtime-package-log__body {
  display: grid;
  gap: 10px;
  padding: 0 12px 12px;
}

.python-runtime-package-log__section {
  display: grid;
  gap: 6px;
}

.python-runtime-package-log__label {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
}

.python-runtime-package-log__output {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.5;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  white-space: pre-wrap;
  word-break: break-word;
}

.python-runtime-package-log__output.is-error {
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: color-mix(in srgb, #b91c1c 78%, var(--guaxi-text-primary));
  border-color: color-mix(in srgb, #b91c1c 24%, var(--guaxi-border));
}

.python-runtime-package-browser__empty {
  padding: 18px 10px;
  text-align: center;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

:global(.python-runtime-popover-overlay) {
  background: transparent !important;
}

:global(.python-runtime-popover-overlay .n-popover__content) {
  padding: 0 !important;
  background: transparent !important;
  box-shadow: none !important;
  border: none !important;
}

:global(.python-runtime-popover-overlay .n-popover-shared) {
  background: transparent !important;
  box-shadow: none !important;
  border: none !important;
}

:global(.python-runtime-popover-overlay .n-popover-arrow-wrapper) {
  display: none !important;
}

.mode-toggle {
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: background-color 0.2s;
  position: relative;
  z-index: 1;
  white-space: nowrap;
  flex-shrink: 0;
  /* 防止按钮缩小 */
  box-sizing: border-box;
  /* 包含padding和border */
}

/* 响应式处理 */
@media (max-width: 600px) {
  .toolbar {
    padding: 4px;
    height: 30px;
    flex-wrap: nowrap;
    justify-content: space-between;
    overflow: visible;
  }

  .toolbar-spacer {
    display: none;
  }

  .toolbar-editing-actions {
    flex: 1 1 auto;
    min-width: 0;
    flex-wrap: nowrap;
    overflow-x: auto;
    overflow-y: hidden;
    contain: paint;
    -webkit-mask-image: linear-gradient(to right, #000 0%, #000 85%, transparent 100%);
    mask-image: linear-gradient(to right, #000 0%, #000 85%, transparent 100%);
  }

  .toolbar-actions {
    flex: 0 0 auto;
    margin-left: 8px;
  }

  .mode-toggle {
    padding: 2px 4px;
  }
}

.mode-toggle:hover {
  background-color: var(--guaxi-panel-bg);
}

.editor-content {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.editor-empty {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #8a8f98);
  font-size: 14px;
  user-select: none;
}

.editor-content.toggle {
  flex-direction: column;
}

.editor-content.split {
  flex-direction: row;
}

.editor-content.split > .preview-shell,
.editor-content.split > .editor-wrapper {
  flex: 1 1 0;
  min-width: 0;
  width: 0;
}

.editor-content.split > .editor-wrapper {
  border-left: 1px solid var(--guaxi-border, #e0e0e0);
}

.editor-content.realtime {
  flex-direction: column;
}

.realtime-vditor {
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  overflow: hidden;
}

.realtime-vditor :deep(.vditor) {
  height: 100%;
  border: none;
}

.realtime-vditor :deep(.vditor-content) {
  height: 100%;
}

.realtime-vditor :deep(.vditor-toolbar) {
  display: flex;
  flex-direction: row !important;
  flex-wrap: nowrap;
  overflow-x: auto;
  overflow-y: hidden;
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-primary) !important;
  border-bottom: 1px solid var(--guaxi-border) !important;
}

.realtime-vditor :deep(.vditor-toolbar svg),
.realtime-vditor :deep(.vditor-toolbar svg *) {
  fill: currentColor !important;
  stroke: currentColor !important;
}

.realtime-vditor :deep(.vditor-reset) {
  background-color: var(--guaxi-content-bg) !important;
  color: var(--guaxi-text-primary) !important;
}

.realtime-vditor :deep(.vditor-reset table) {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid var(--guaxi-border, #d0d7de);
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.realtime-vditor :deep(.vditor-reset th),
.realtime-vditor :deep(.vditor-reset td) {
  border: 1px solid var(--guaxi-border, #d0d7de);
  padding: 8px 10px;
  text-align: left;
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.realtime-vditor :deep(.vditor-reset th) {
  background: rgba(175, 184, 193, 0.18);
  font-weight: 700;
}

.realtime-vditor :deep(.vditor-reset tr:nth-child(even) td) {
  background: rgba(175, 184, 193, 0.08);
}

.realtime-vditor :deep(.vditor-ir pre),
.realtime-vditor :deep(.vditor-wysiwyg pre),
.realtime-vditor :deep(.vditor-reset pre) {
  background: var(--guaxi-code-bg, #f6f8fa) !important;
  border: 1px solid var(--guaxi-border, #d0d7de) !important;
  color: var(--guaxi-text-primary, #1f2328) !important;
}

.realtime-vditor :deep(.vditor-ir code),
.realtime-vditor :deep(.vditor-wysiwyg code),
.realtime-vditor :deep(.vditor-reset code) {
  background: rgba(175, 184, 193, 0.2) !important;
  color: var(--guaxi-text-primary, #1f2328) !important;
}

.realtime-vditor :deep(.vditor-ir__marker),
.realtime-vditor :deep(.vditor-ir__node),
.realtime-vditor :deep(.vditor-wysiwyg__block),
.realtime-vditor :deep(.vditor-reset [contenteditable='true']) {
  color: var(--guaxi-text-primary, #1f2328) !important;
}

.realtime-vditor :deep(.vditor-reset .mermaid),
.realtime-vditor :deep(.vditor-reset .language-mermaid) {
  background: #ffffff !important;
  color: #1f2328 !important;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset table) {
  border-color: #30363d;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset th),
:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset td) {
  border-color: #30363d;
  background: #0f1720;
  color: #e6edf3;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset th) {
  background: #18222d;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset tr:nth-child(even) td) {
  background: #131d27;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-ir pre),
:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-wysiwyg pre),
:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset pre) {
  background: #1f242d !important;
  border-color: #30363d !important;
  color: #e6edf3 !important;
}

:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-ir code),
:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-wysiwyg code),
:global(:root[data-guaxi-theme='dark']) .realtime-vditor :deep(.vditor-reset code) {
  background: rgba(110, 118, 129, 0.24) !important;
  color: #e6edf3 !important;
}

.realtime-vditor :deep(.vditor-toolbar > *) {
  flex: 0 0 auto;
}

.realtime-vditor :deep(.vditor-toolbar::-webkit-scrollbar) {
  height: 4px;
}

.realtime-vditor :deep(.vditor-toolbar::-webkit-scrollbar-thumb) {
  background-color: rgba(0, 0, 0, 0.25);
  border-radius: 999px;
}

.editor-wrapper {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* 直接使用主题变量，避免首次启动暗色模式时先渲染出白色编辑器底。 */
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.notebook-surface {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
}

.editor-container {
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.editor-wrapper :deep(.monaco-scrollable-element) {
  scrollbar-width: thin;
  scrollbar-color: rgba(120, 120, 120, 0.45) transparent;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar) {
  width: 8px;
  height: 8px;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-track) {
  background: transparent;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(120, 120, 120, 0.45);
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(120, 120, 120, 0.65);
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-scrollable-element) {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.28);
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(255, 255, 255, 0.45);
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .margin),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .glyph-margin),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .margin-view-overlays),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .monaco-editor-background) {
  background-color: #282c34 !important;
}

/* 设计说明：
   1. sticky scroll 的行号区域由 monaco-vscode-api 使用 .sticky-widget-line-numbers 单独渲染，不会命中普通 .line-numbers 覆盖。
   2. 查找框的边框来自 --vscode-widget-border / --vscode-input-border；这里加 DOM 兜底，防止依赖加载顺序让亮色变量短暂获胜。
   3. 整条选择器放进 :global(...)，避免 Vue scoped CSS 把 :global(:root) 和 :deep(...) 编译成只命中根节点的规则。 */
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor) {
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

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-widget),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-widget-line-numbers),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-widget-lines-scrollable),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-widget-lines),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-line-number),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-line-content) {
  background-color: #282c34 !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-widget) {
  border-bottom-color: transparent !important;
  box-shadow: rgba(0, 0, 0, 0.42) 0 4px 2px -2px !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-line-number-inner),
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-line-number .codicon) {
  color: #5c6370 !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .sticky-line-content:hover) {
  background-color: #2c313a !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .find-widget) {
  background-color: #282c34 !important;
  border-color: #3b4048 !important;
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.32) !important;
  color: #abb2bf !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .find-widget .monaco-inputbox) {
  background-color: #21252b !important;
  border-color: #3b4048 !important;
  color: #abb2bf !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .find-widget .monaco-inputbox.synthetic-focus) {
  outline-color: rgba(82, 139, 255, 0.5) !important;
}

:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .find-widget textarea::placeholder) {
  color: #7f8898 !important;
}

/* 设计说明：
 * 1. 当前行高亮只允许出现在 view-overlays 内容区，避免背景条越过 Monaco 的行号/gutter 区域。
 * 2. 行号列保留文字高亮，不再铺背景色，这样视觉起点会与正文列对齐。
 */
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .view-overlays .current-line) {
  background-color: #2c313a !important;
  border-color: transparent !important;
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .margin-view-overlays .current-line),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .margin-view-overlays .current-line-margin),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .margin-view-overlays .line-numbers),
:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .line-numbers.active-line-number) {
  background-color: transparent !important;
  border-color: transparent !important;
}

:global(:root[data-guaxi-theme='dark'] .editor-wrapper .monaco-editor .line-numbers.active-line-number) {
  color: #d7deea !important;
}

.guaxi-editor :deep(.md-fence-token-base) { color: #383a42; }
.guaxi-editor :deep(.md-fence-token-keyword) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-string) { color: #98c379; }
.guaxi-editor :deep(.md-fence-token-number) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-boolean) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
.guaxi-editor :deep(.md-fence-token-type) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
.guaxi-editor :deep(.md-fence-token-method) { color: #61afef; }
.guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-property) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-macro) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-tag) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-attribute) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-builtin) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-decorator) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-operator) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-delimiter) { color: #abb2bf; }

:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-base) { color: #abb2bf; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-keyword) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-string) { color: #98c379; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-number) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-boolean) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-type) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-method) { color: #61afef; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-property) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-macro) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-tag) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-attribute) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-builtin) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-decorator) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-operator) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-delimiter) { color: #abb2bf; }

/* 设计说明：
   1. 未标注语言的 Markdown fenced code block 会按 plaintext 走整行 base 装饰。
   2. Monaco 装饰类位于编辑器内部 DOM，Vue scoped CSS 的 :deep + :global 组合在部分加载顺序下会被亮色 base 覆盖。
   3. 这里使用完整全局选择器和 !important 兜底，确保暗色模式下纯文本代码块始终保持可读亮色。 */
:global(:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-base) {
  color: #abb2bf !important;
}

.editor-split-wrapper {
  flex: 1;
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
  box-sizing: border-box;
}

.editor-split-wrapper.vim-enabled {
  box-sizing: border-box;
  padding-bottom: 0;
}

.editor-split-wrapper.single,
.editor-split-wrapper.vertical {
  flex-direction: row;
}

.editor-split-wrapper.horizontal {
  flex-direction: column;
}

.editor-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
  position: relative;
  isolation: isolate;
}

:deep(.guaxi-editor-effects) {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 20;
  overflow: hidden;
}

:deep(.guaxi-editor-effects-canvas) {
  width: 100%;
  height: 100%;
  display: block;
}

:deep(.guaxi-editor-smooth-cursor) {
  position: absolute;
  top: 0;
  left: 0;
  width: 2px;
  border-radius: 2px;
  background: rgba(0, 173, 255, 0.85);
  box-shadow:
    0 0 10px rgba(0, 173, 255, 0.55),
    0 0 20px rgba(255, 105, 180, 0.25);
  opacity: 0;
  will-change: transform, height, opacity;
}

.vim-status-bar {
  position: absolute;
  bottom: 22px;
  left: 0;
  right: 0;
  height: 20px;
  background-color: var(--vim-status-bg, var(--toolbar-bg, #f8f8f8));
  border-top: 1px solid var(--vim-status-border, var(--divider-color, #e0e0e0));
  color: var(--vim-status-fg, var(--guaxi-text-primary, #1f2328));
  padding: 2px 8px;
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  z-index: 1;
}

.vim-status-bar :deep(input) {
  /* monaco-vim 在运行时把命令输入框插入到状态栏内，必须用 :deep 命中这个没有 scoped 属性的节点。 */
  box-sizing: border-box;
  height: 16px;
  margin: 0 2px;
  background-color: var(--vim-status-input-bg, var(--vim-status-bg, #f8f8f8));
  border: 1px solid var(--vim-status-input-border, var(--vim-status-border, #e0e0e0));
  color: var(--vim-status-fg, var(--guaxi-text-primary, #1f2328));
  caret-color: var(--vim-status-fg, var(--guaxi-text-primary, #1f2328));
  font: inherit;
  outline: none;
}

.vim-status-bar :deep(input:focus) {
  border-color: var(--vscode-focusBorder, rgba(82, 139, 255, 0.5));
}

.editor-status-bar {
  height: 22px;
  /* 设计说明：
     1. 子组件内部已经按 Guaxi 主题绘制状态栏。
     2. 这里不能继续使用 VSCode 兼容变量 --statusBar-background，否则 monaco-vscode-api 的默认蓝色会覆盖 Guaxi 主题。
     3. 外层只保留稳定尺寸和层级，背景/边框交给状态栏组件自己的主题变量。 */
  background-color: transparent;
  border-top: 0;
  flex-shrink: 0;
  /* 防止状态栏被压缩 */
  z-index: 2;
  /* 确保在编辑器上方 */
}

.preview-shell {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* 预览区需要直接绑定主题背景，否则从亮色切到暗色时容器可能残留浅色底。 */
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.preview-search-panel {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 12;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 94%, transparent);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
}

.preview-search-input {
  width: 220px;
  height: 26px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.preview-search-input:focus {
  border-color: var(--primary-color, #4acadb);
}

.preview-search-count {
  font-size: 12px;
  opacity: 0.8;
  min-width: 52px;
  text-align: center;
}

.preview-search-btn {
  height: 24px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  cursor: pointer;
}

.preview-search-btn:hover {
  background: var(--guaxi-header-bg);
}

.preview-search-close {
  color: var(--error-color, #ff6b6b);
}

.preview :deep(.preview-search-hit) {
  background: rgba(255, 208, 0, 0.38);
  border-radius: 2px;
  padding: 0 1px;
}

.preview :deep(.preview-search-hit-active) {
  background: rgba(255, 145, 0, 0.65);
}

.preview {
  flex: 1 1 auto;
  min-height: 0;
  padding: 12px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.5;
  overflow-y: auto;
  overflow-x: hidden;
  word-wrap: break-word;
  box-sizing: border-box;
  position: relative;
  /* 主预览内容区和分栏预览共用这一层，显式继承主题底色以保证切主题立即生效。 */
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
}

.preview a,
.preview a:visited,
.preview :deep(a),
.preview :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .preview a,
.dark-mode .preview a:visited,
.dark-mode .preview :deep(a),
.dark-mode .preview :deep(a:visited),
.dark .preview a,
.dark .preview a:visited,
.dark .preview :deep(a),
.dark .preview :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.preview a:hover,
.preview :deep(a:hover) {
  opacity: 0.92;
}

/* 设计说明：
 * 1. 预览 HTML 会混入 markdown-body 等外部样式，这些样式有机会把 p / li 设成 flex，
 *    从而把普通链接挤到行尾，表现成参考式链接、列表项排版错乱。
 * 2. 这里对预览容器里的基础文档流做一次轻量重置，只修正文档段落与列表的布局，
 *    不影响专门的文件卡片、目录卡片等自定义组件。
 */
.preview :deep(p),
.preview :deep(li > p),
.preview :deep(.md-paragraph) {
  display: block !important;
  width: auto !important;
  max-width: 100%;
  text-align: left;
  white-space: normal;
  justify-content: flex-start !important;
  align-items: normal !important;
}

/* 设计说明：
 * 1. 预览区默认会把段落文本重新拉回普通文档流，但作者手写的 HTML 里如果显式写了 `align`
 *    或 `style="text-align:..."`，就应当尊重其原始布局意图。
 * 2. 这里单独为带 `align` 的段落恢复对应的文本对齐，避免它被上面的基础重置覆盖。
 */
.preview :deep(p[align='center']) {
  text-align: center !important;
}

.preview :deep(p[align='right']) {
  text-align: right !important;
}

.preview :deep(p[align='left']) {
  text-align: left !important;
}

.preview :deep(.md-paragraph > a:not(:only-child)),
.preview :deep(p > a:not(:only-child)) {
  display: inline !important;
  width: auto !important;
  margin: 0 !important;
}

.preview :deep(li) {
  display: list-item !important;
  width: auto !important;
  max-width: 100%;
  text-align: left;
}

.preview :deep(ul),
.preview :deep(ol) {
  display: block;
  width: 100%;
  max-width: 100%;
}

.preview :deep(li) {
  list-style-position: outside;
}

/* ==================== Markdown 任务列表（Task List）样式 ==================== */
.preview :deep(.guaxi-preview-file-card) {
  display: inline-flex;
  vertical-align: top;
  max-width: min(420px, 100%);
  margin: 8px 0;
  border-radius: 14px;
  border: 1px solid var(--guaxi-border, rgba(14, 116, 144, 0.16));
  background: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 86%, var(--guaxi-content-bg, #ffffff) 14%);
  overflow: hidden;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.06);
}

.preview :deep(.guaxi-preview-file-card__link) {
  display: inline-grid;
  gap: 4px;
  min-width: 0;
  padding: 10px 12px;
  color: inherit;
  text-decoration: none;
}

.preview :deep(.guaxi-preview-file-card__link:hover) {
  opacity: 1;
  text-decoration: none;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 92%, var(--guaxi-content-bg, #ffffff) 8%);
}

.preview :deep(.guaxi-preview-file-card__eyebrow) {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  width: fit-content;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(8, 145, 178, 0.1);
  color: var(--guaxi-text-link, #0f766e);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
}

.preview :deep(.guaxi-preview-file-card__title) {
  color: var(--guaxi-text-primary, #0f172a);
  font-size: 14px;
  font-weight: 700;
  line-height: 1.4;
}

.preview :deep(.guaxi-preview-file-card__excerpt) {
  color: var(--guaxi-text-secondary, rgba(51, 65, 85, 0.9));
  font-size: 12px;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.preview :deep(.guaxi-preview-file-card__meta) {
  color: var(--guaxi-text-secondary, rgba(71, 85, 105, 0.88));
  font-size: 11px;
  letter-spacing: 0.01em;
}

.preview :deep(li.task-list-item) {
  list-style: none;
}

.preview :deep(input.task-list-item-checkbox) {
  width: 16px;
  height: 16px;
  margin-right: 8px;
  vertical-align: -2px;
  cursor: pointer;

  /* 圆圈样式：替代默认方形 checkbox */
  appearance: none;
  -webkit-appearance: none;
  outline: none;
  border-radius: 999px;
  border: 2px solid var(--primary-color, #4acadb);
  background: transparent;
  display: inline-grid;
  place-content: center;
  transition: background-color 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
}

.preview :deep(input.task-list-item-checkbox:hover) {
  transform: scale(1.05);
}

.preview :deep(input.task-list-item-checkbox::after) {
  content: '';
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #ffffff;
  transform: scale(0);
  transition: transform 0.12s ease;
}

.preview :deep(input.task-list-item-checkbox:checked) {
  background: var(--primary-color, #4acadb);
  border-color: var(--primary-color, #4acadb);
}

.preview :deep(input.task-list-item-checkbox:checked::after) {
  transform: scale(1);
}

.dark-mode .preview :deep(input.task-list-item-checkbox),
.dark .preview :deep(input.task-list-item-checkbox) {
  border-color: var(--guaxi-text-link-dark, #4acadb);
}

.dark-mode .preview :deep(input.task-list-item-checkbox:checked),
.dark .preview :deep(input.task-list-item-checkbox:checked) {
  background: var(--guaxi-text-link-dark, #4acadb);
  border-color: var(--guaxi-text-link-dark, #4acadb);
}

.preview :deep(li.task-list-item.task-list-item-checked .task-list-item-text) {
  text-decoration: line-through;
  opacity: 0.78;
}

.preview :deep(.md-toc) {
  display: grid;
  gap: 10px;
  margin: 6px 0;
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #d0d7de) 78%, transparent 22%);
  background: color-mix(in srgb, var(--guaxi-code-bg, #f6f8fa) 88%, var(--guaxi-content-bg, #ffffff) 12%);
}

.preview :deep(.md-toc::before) {
  content: '目录';
  display: block;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary, #6b7280);
}

.preview :deep(.md-toc .md-toc-list) {
  list-style: none;
  padding-left: 0;
  margin: 0;
  display: grid;
  gap: 6px;
}

.preview :deep(.md-toc-item) {
  margin: 0;
  line-height: 1.35;
  padding-left: 0;
}

.preview :deep(.md-toc-item.md-toc-level-2) {
  padding-left: 18px;
}

.preview :deep(.md-toc-item.md-toc-level-3) {
  padding-left: 36px;
}

.preview :deep(.md-toc-item.md-toc-level-4) {
  padding-left: 54px;
}

.preview :deep(.md-toc-item.md-toc-level-5) {
  padding-left: 72px;
}

.preview :deep(.md-toc-item.md-toc-level-6) {
  padding-left: 90px;
}

.preview :deep(a.md-toc-link) {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 9px;
  color: var(--guaxi-text-primary, #1f2328);
  text-decoration: none;
  cursor: pointer;
  background: color-mix(in srgb, var(--guaxi-code-bg, #f6f8fa) 66%, transparent 34%);
}

.preview :deep(a.md-toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-code-bg, #f6f8fa) 82%, #ffffff 18%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary-color, #4acadb) 16%, transparent 84%) inset;
}

.dark-mode .preview :deep(.md-toc),
.dark .preview :deep(.md-toc),
:root[data-guaxi-theme='dark'] .preview :deep(.md-toc) {
  border-color: rgba(148, 163, 184, 0.24);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #111827) 90%, var(--guaxi-content-bg, #0f172a) 10%);
}

.dark-mode .preview :deep(.md-toc::before),
.dark .preview :deep(.md-toc::before),
:root[data-guaxi-theme='dark'] .preview :deep(.md-toc::before) {
  color: rgba(148, 163, 184, 0.92);
}

.dark-mode .preview :deep(a.md-toc-link),
.dark .preview :deep(a.md-toc-link),
:root[data-guaxi-theme='dark'] .preview :deep(a.md-toc-link) {
  color: #e2e8f0;
  background: color-mix(in srgb, var(--guaxi-content-bg, #0f172a) 82%, var(--guaxi-panel-bg, #111827) 18%);
}

.dark-mode .preview :deep(a.md-toc-link:hover),
.dark .preview :deep(a.md-toc-link:hover),
:root[data-guaxi-theme='dark'] .preview :deep(a.md-toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-panel-bg, #111827) 94%, var(--guaxi-content-bg, #0f172a) 6%);
  box-shadow: 0 0 0 1px rgba(74, 202, 219, 0.16) inset;
}

:deep(.monaco-editor .guaxi-link-like),
:deep(.monaco-editor .guaxi-mdlink-target) {
  color: var(--guaxi-text-link, var(--primary-color, #4acadb)) !important;
  text-decoration: underline !important;
  text-underline-offset: 2px;
  cursor: pointer;
}

:deep(.monaco-editor .guaxi-mdlink-text),
:deep(.monaco-editor .guaxi-wikilink-text) {
  color: var(--guaxi-mdlink-text-color, #c99016) !important;
  text-decoration: none !important;
}

:deep(.monaco-editor .guaxi-mdlink-bracket),
:deep(.monaco-editor .guaxi-mdlink-paren),
:deep(.monaco-editor .guaxi-mdlink-angle),
:deep(.monaco-editor .guaxi-wikilink-bracket) {
  color: #f35f4c !important;
  text-decoration: none !important;
}

:deep(.monaco-editor .guaxi-mdref-target) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da)) !important;
  text-decoration: none !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdref-target) {
  color: var(--guaxi-text-link-dark, #4acadb) !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-link-like),
:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdlink-target) {
  color: var(--guaxi-text-link-dark, #4acadb) !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdlink-text),
:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-wikilink-text) {
  color: var(--guaxi-mdlink-text-color-dark, #c678dd) !important;
}

.preview code {
  font-family: inherit !important;
}

.render-error {
  color: var(--error-color, #d32f2f);
  background-color: var(--error-bg, #ffebee);
  padding: 16px;
  border-radius: 4px;
  margin: 16px;
}

.render-error button {
  margin-top: 8px;
  padding: 4px 8px;
  background-color: var(--primary-color, #1976d2);
  color: white;
  border: none;
  border-radius: 2px;
  cursor: pointer;
}

/* 暗色模式适配 */
.dark .toolbar {
  background-color: var(--dark-toolbar-bg, #1e1e1e);
  border-bottom-color: var(--dark-divider-color, #333);
}

.dark .mode-toggle:hover {
  background-color: var(--dark-hover-color, #333);
}

.dark .tab-bar {
  background: var(--dark-toolbar-bg, #1e1e1e);
  border-bottom-color: var(--dark-divider-color, #333);
}

.dark .tab-region-right {
  border-left-color: var(--dark-divider-color, #333);
}

.dark .tab-item {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.08);
}

.dark .tab-item:hover {
  background: rgba(74, 163, 255, 0.18);
  border-color: rgba(74, 163, 255, 0.3);
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.35);
  transform: translateY(-1px);
}

.dark .tab-item.active {
  background: rgba(24, 144, 255, 0.28);
  border-color: rgba(74, 163, 255, 0.45);
  box-shadow: 0 1px 8px rgba(0, 0, 0, 0.4);
}

.dark .tab-item.active:hover {
  background: rgba(24, 144, 255, 0.32);
  border-color: rgba(74, 163, 255, 0.55);
}

.tab-bar {
  display: flex;
  height: 38px;
  min-height: 38px;
  flex: 0 0 38px;
  flex-shrink: 0;
  align-items: center;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.tab-region {
  display: flex;
  flex: 1 1 0;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
  padding: 4px 8px;
  gap: 6px;
}

.tab-region-right {
  border-left: 1px solid var(--guaxi-border);
}

.tab-region::-webkit-scrollbar {
  width: 4px;
  height: 4px;
}

.tab-region::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.tab-region::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.4);
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 12px;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
  flex-shrink: 0;
  color: var(--guaxi-text-primary);
  background: rgba(128, 128, 128, 0.08);
  transition: background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease;
}

.tab-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  max-width: 240px;
}

.tab-affordance {
  position: relative;
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
}

.tab-close {
  position: absolute;
  inset: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
}

.tab-dirty-dot {
  position: absolute;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgba(74, 163, 255, 0.7);
}

.tab-item:hover .tab-close,
.tab-item.active:not(.dirty) .tab-close {
  opacity: 1;
}

.tab-item.dirty:hover .tab-dirty-dot {
  opacity: 0;
}

.tab-item:hover .tab-affordance {
  background: rgba(74, 163, 255, 0.12);
}

.tab-item:hover .tab-close {
  color: rgba(74, 163, 255, 0.95);
}

.tab-item.active {
  background: rgba(24, 144, 255, 0.18);
  border-color: rgba(24, 144, 255, 0.35);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  color: var(--guaxi-text-primary);
}

.tab-item:hover {
  background: rgba(127, 127, 127, 0.12);
  border-color: rgba(0, 0, 0, 0.14);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  transform: translateY(-1px);
}

.tab-item.active:hover {
  background: rgba(24, 144, 255, 0.22);
  border-color: rgba(24, 144, 255, 0.45);
}

/* Git 行变更装饰器样式 */
:deep(.monaco-editor .margin-view-overlays .git-line-added),
:deep(.monaco-editor .margin-view-overlays .git-line-modified),
:deep(.monaco-editor .margin-view-overlays .git-line-deleted) {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  opacity: 1 !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-added) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.95) !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-modified) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.75) !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-deleted) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.55) !important;
}

/* 确保装饰器区域可见 */
:deep(.monaco-editor .glyph-margin) {
  background-color: var(--guaxi-header-bg) !important;
}
</style>

<style>
/* 数学公式样式 */
.math-inline {
  display: inline;
  vertical-align: middle;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* 设计说明：
 * 1. Markdown 源码编辑器里的围栏代码块高亮由 Monaco decoration 动态添加 `md-fence-token-*` 类。
 * 2. 这些 token decoration 会和整行 `md-fence-token-base` 兜底 decoration 重叠；若 base 使用
 *    `!important` 而 token 颜色没有同级优先级，最终看到的就会是单色代码块。
 * 3. 这里放在普通 `<style>` 块中，用全局选择器和 `!important` 让 token 颜色稳定覆盖 base 兜底色。
 */
.guaxi-editor .monaco-editor .md-fence-token-base { color: #383a42 !important; }
.guaxi-editor .monaco-editor .md-fence-token-keyword { color: #a626a4 !important; }
.guaxi-editor .monaco-editor .md-fence-token-string { color: #50a14f !important; }
.guaxi-editor .monaco-editor .md-fence-token-number { color: #986801 !important; }
.guaxi-editor .monaco-editor .md-fence-token-boolean { color: #0184bc !important; }
.guaxi-editor .monaco-editor .md-fence-token-comment { color: #7f848e !important; }
.guaxi-editor .monaco-editor .md-fence-token-type { color: #c18401 !important; }
.guaxi-editor .monaco-editor .md-fence-token-class { color: #c18401 !important; }
.guaxi-editor .monaco-editor .md-fence-token-function { color: #4078f2 !important; }
.guaxi-editor .monaco-editor .md-fence-token-method { color: #4078f2 !important; }
.guaxi-editor .monaco-editor .md-fence-token-parameter { color: #986801 !important; }
.guaxi-editor .monaco-editor .md-fence-token-variable { color: #e45649 !important; }
.guaxi-editor .monaco-editor .md-fence-token-constant { color: #986801 !important; }
.guaxi-editor .monaco-editor .md-fence-token-regexp { color: #0184bc !important; }
.guaxi-editor .monaco-editor .md-fence-token-property { color: #e45649 !important; }
.guaxi-editor .monaco-editor .md-fence-token-namespace { color: #c18401 !important; }
.guaxi-editor .monaco-editor .md-fence-token-macro { color: #4078f2 !important; }
.guaxi-editor .monaco-editor .md-fence-token-tag { color: #e45649 !important; }
.guaxi-editor .monaco-editor .md-fence-token-attribute { color: #986801 !important; }
.guaxi-editor .monaco-editor .md-fence-token-builtin { color: #0184bc !important; }
.guaxi-editor .monaco-editor .md-fence-token-decorator { color: #a626a4 !important; }
.guaxi-editor .monaco-editor .md-fence-token-operator { color: #0184bc !important; }
.guaxi-editor .monaco-editor .md-fence-token-delimiter { color: #383a42 !important; }

:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-base { color: #abb2bf !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-keyword { color: #c678dd !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-string { color: #98c379 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-number { color: #d19a66 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-boolean { color: #d19a66 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-comment { color: #5c6370 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-type { color: #e5c07b !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-class { color: #e5c07b !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-function { color: #61afef !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-method { color: #61afef !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-parameter { color: #d19a66 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-variable { color: #e06c75 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-constant { color: #d19a66 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-regexp { color: #56b6c2 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-property { color: #e06c75 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-namespace { color: #e5c07b !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-macro { color: #c678dd !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-tag { color: #e06c75 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-attribute { color: #d19a66 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-builtin { color: #56b6c2 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-decorator { color: #c678dd !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-operator { color: #56b6c2 !important; }
:root[data-guaxi-theme='dark'] .guaxi-editor .monaco-editor .md-fence-token-delimiter { color: #abb2bf !important; }

.n-dropdown-menu {
  max-height: 320px;
  max-height: min(320px, calc(100vh - 140px));
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 140, 0.45) transparent;
}

.n-dropdown-menu::-webkit-scrollbar {
  width: 8px;
}

.n-dropdown-menu::-webkit-scrollbar-track {
  background: transparent;
}

.n-dropdown-menu::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 140, 0.35);
  border-radius: 8px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.n-dropdown-menu::-webkit-scrollbar-thumb:hover {
  background: rgba(140, 140, 140, 0.55);
  background-clip: content-box;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: #ffffff;
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  overflow: auto;
  color: #1f2328;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}


/* 代码块样式 */
.guaxi-codecell-media {
  border: 1px solid var(--guaxi-border, #d9dee7);
  border-radius: 18px;
  background:
    radial-gradient(circle at top left, rgba(255, 255, 255, 0.96), rgba(255, 255, 255, 0.84)),
    linear-gradient(180deg, rgba(244, 247, 252, 0.98), rgba(236, 241, 247, 0.96));
  box-shadow:
    0 16px 32px rgba(15, 23, 42, 0.08),
    inset 0 1px 0 rgba(255, 255, 255, 0.82);
}

.preview .guaxi-codecell-toolbar,
.preview .guaxi-pycell-toolbar {
  display: flex !important;
  align-items: center !important;
  gap: 8px !important;
  flex-wrap: wrap;
}

/* 设计说明：
 * 1. 预览态 Python cell 本身没有稳定的独立图标节点，因此这里在 toolbar 前面补一个轻量的 Py 徽标。
 * 2. 运行中只切换 cell 的 `is-running` class，就能驱动徽标旋转和高亮，避免额外插入/销毁 DOM。
 */
.preview .guaxi-pycell-toolbar::before {
  content: 'Py';
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 9px;
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.16), rgba(14, 165, 233, 0.24));
  border: 1px solid rgba(59, 130, 246, 0.26);
  color: #1d4ed8;
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.02em;
  box-shadow: 0 6px 16px rgba(37, 99, 235, 0.12);
  flex: 0 0 auto;
}

.preview .guaxi-pycell.is-running .guaxi-pycell-toolbar::before {
  animation: guaxi-pycell-badge-spin 1.2s linear infinite, guaxi-pycell-badge-pulse 1.4s ease-in-out infinite;
}

.guaxi-pycell-text-output-shell,
.guaxi-pycell-media {
  margin-top: 10px;
}

.guaxi-pycell-media {
  gap: 12px;
}

.guaxi-pycell-output-module {
  width: 100%;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 14px;
  background: rgba(248, 250, 252, 0.92);
  overflow: hidden;
}

.guaxi-pycell-output-module-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.16);
  background: rgba(255, 255, 255, 0.72);
}

.guaxi-pycell-output-module-meta {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-secondary, #475569);
}

.guaxi-pycell-output-module-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.92);
  color: var(--guaxi-text-primary, #0f172a);
  cursor: pointer;
}

.guaxi-pycell-output-module-body {
  padding: 12px;
}

.guaxi-pycell-text-output-shell pre {
  margin: 0;
}

.guaxi-pycell-session {
  margin-top: 10px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 14px;
  background: rgba(248, 250, 252, 0.92);
  backdrop-filter: blur(12px);
  overflow: hidden;
}

.guaxi-pycell-session-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px 8px;
}

.guaxi-pycell-session-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--guaxi-text-primary, #0f172a);
}

.guaxi-pycell-session-meta {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.guaxi-pycell-session-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 240px;
  overflow: auto;
  padding: 0 14px 14px;
}

.guaxi-pycell-var-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.82);
  border: 1px solid rgba(148, 163, 184, 0.16);
}

.guaxi-pycell-var-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.guaxi-pycell-var-name {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 12px;
  font-weight: 700;
  color: var(--guaxi-text-primary, #0f172a);
}

.guaxi-pycell-var-type {
  font-size: 11px;
  color: var(--guaxi-text-secondary, #64748b);
}

.guaxi-pycell-var-preview {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #475569);
  word-break: break-word;
}

.guaxi-pycell-session-empty {
  padding: 12px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.7);
  color: var(--guaxi-text-secondary, #64748b);
  font-size: 12px;
}

.guaxi-pycell-rich-display {
  width: 100%;
  overflow: auto;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.86);
  border: 1px solid rgba(148, 163, 184, 0.2);
  padding: 14px;
}

.guaxi-pycell-svg-display svg {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}

.guaxi-pycell-markdown-display {
  margin: 0;
  white-space: pre-wrap;
}

.preview .guaxi-codecell-btn,
.preview .guaxi-pycell-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 32px;
  padding: 6px 12px !important;
  border-radius: 10px !important;
  border: 1px solid rgba(148, 163, 184, 0.3) !important;
  background: rgba(255, 255, 255, 0.92) !important;
  color: var(--guaxi-text-primary, #1f2937) !important;
  cursor: pointer !important;
  white-space: nowrap;
  flex: 0 0 auto;
  line-height: 1;
  transition: transform 0.16s ease, box-shadow 0.16s ease, border-color 0.16s ease, background 0.16s ease;
  box-shadow: 0 4px 12px rgba(15, 23, 42, 0.06);
}

.preview .guaxi-codecell-btn:hover,
.preview .guaxi-pycell-btn:hover {
  transform: translateY(-1px);
  border-color: rgba(37, 99, 235, 0.35) !important;
  background: rgba(239, 246, 255, 0.98) !important;
  box-shadow: 0 10px 20px rgba(37, 99, 235, 0.12);
}

.preview .guaxi-codecell-btn:active,
.preview .guaxi-pycell-btn:active {
  transform: translateY(0);
}

.preview .guaxi-codecell-btn[disabled],
.preview .guaxi-pycell-btn[disabled] {
  cursor: not-allowed !important;
  opacity: 0.6;
  transform: none;
  box-shadow: none;
}

.preview .guaxi-codecell-hint,
.preview .guaxi-pycell-hint {
  white-space: nowrap;
  flex: 0 1 auto;
}

.preview .guaxi-pycell.is-running .guaxi-pycell-hint {
  color: #2563eb;
  font-weight: 600;
}

@keyframes guaxi-pycell-badge-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes guaxi-pycell-badge-pulse {
  0%,
  100% {
    box-shadow: 0 6px 16px rgba(37, 99, 235, 0.12);
  }
  50% {
    box-shadow: 0 10px 24px rgba(37, 99, 235, 0.28);
  }
}

@keyframes guaxi-pycell-prompt-pulse {
  0%,
  100% {
    opacity: 1;
    transform: translateY(0);
  }
  50% {
    opacity: 0.58;
    transform: translateY(-1px);
  }
}

.guaxi-dataframe-card {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px;
}

.guaxi-dataframe-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.guaxi-dataframe-meta {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.guaxi-dataframe-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.guaxi-dataframe-btn {
  min-height: 30px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid rgba(148, 163, 184, 0.28);
  background: rgba(255, 255, 255, 0.88);
  color: var(--guaxi-text-primary, #1f2937);
  cursor: pointer;
  transition: transform 0.15s ease, border-color 0.15s ease, background 0.15s ease;
}

.guaxi-dataframe-btn:hover {
  transform: translateY(-1px);
  border-color: rgba(37, 99, 235, 0.32);
  background: rgba(239, 246, 255, 0.96);
}

.guaxi-dataframe-table-wrap {
  width: 100%;
  overflow: auto;
  border-radius: 14px;
  border: 1px solid rgba(148, 163, 184, 0.2);
  background: rgba(255, 255, 255, 0.86);
}

.guaxi-dataframe-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.guaxi-dataframe-table th,
.guaxi-dataframe-table td {
  padding: 10px 12px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.16);
  text-align: left;
  vertical-align: top;
  white-space: nowrap;
}

.guaxi-dataframe-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: rgba(248, 250, 252, 0.98);
  color: var(--guaxi-text-primary, #1f2937);
  font-weight: 700;
}

.guaxi-dataframe-table td {
  color: var(--guaxi-text-primary, #334155);
}

.guaxi-pdf-preview-shell {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
  overflow: hidden;
}

.guaxi-pdf-preview-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.22);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(246, 248, 252, 0.94));
  backdrop-filter: blur(12px);
}

.guaxi-pdf-toolbar-group {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.guaxi-pdf-file-badge {
  max-width: 280px;
  padding: 8px 12px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.9);
  border: 1px solid rgba(148, 163, 184, 0.22);
  color: var(--guaxi-text-primary, #1f2937);
  font-size: 12px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.guaxi-pdf-toolbar-indicator {
  min-width: 78px;
  padding: 7px 10px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.84);
  border: 1px solid rgba(148, 163, 184, 0.2);
  color: var(--guaxi-text-primary, #1f2937);
  text-align: center;
  font-size: 12px;
  font-weight: 600;
}

.guaxi-pdf-toolbar-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.94);
  color: var(--guaxi-text-primary, #1f2937);
  cursor: pointer;
  transition: transform 0.16s ease, box-shadow 0.16s ease, border-color 0.16s ease, background 0.16s ease;
  box-shadow: 0 6px 16px rgba(15, 23, 42, 0.07);
}

.guaxi-pdf-toolbar-btn:hover {
  transform: translateY(-1px);
  border-color: rgba(59, 130, 246, 0.3);
  box-shadow: 0 10px 24px rgba(59, 130, 246, 0.12);
}

.guaxi-pdf-toolbar-btn:active {
  transform: translateY(0);
}

.guaxi-pdf-toolbar-btn.accent {
  background: linear-gradient(135deg, #1d4ed8, #2563eb);
  border-color: rgba(37, 99, 235, 0.45);
  color: #ffffff;
}

.guaxi-pdf-toolbar-btn svg {
  width: 18px;
  height: 18px;
}

.guaxi-pdf-preview-viewport {
  flex: 1 1 auto;
  min-height: 640px;
  overflow: auto;
  padding: 20px 16px 22px;
  background:
    linear-gradient(180deg, rgba(233, 238, 246, 0.92), rgba(225, 231, 241, 0.9)),
    repeating-linear-gradient(45deg, rgba(255, 255, 255, 0.18) 0 12px, rgba(255, 255, 255, 0.1) 12px 24px);
}

.guaxi-pdf-preview-pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 18px;
}

.guaxi-pdf-page-card {
  overflow: hidden;
  border-radius: 18px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(252, 252, 253, 0.98));
  border: 1px solid rgba(148, 163, 184, 0.22);
  box-shadow:
    0 18px 34px rgba(15, 23, 42, 0.12),
    0 4px 12px rgba(15, 23, 42, 0.05);
}

.guaxi-pdf-page-canvas {
  display: block;
  width: 100%;
  height: auto;
  background: #ffffff;
  transform-origin: top center;
  transition: transform 0.16s ease;
}

.guaxi-pdf-preview-fallback {
  width: 100%;
  box-sizing: border-box;
  padding: 18px 20px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.9);
  color: var(--guaxi-text-primary, #1f2937);
}

.dark-mode .guaxi-codecell-media,
:global(:root[data-guaxi-theme='dark']) .guaxi-codecell-media {
  border-color: rgba(71, 85, 105, 0.65);
  background:
    radial-gradient(circle at top left, rgba(31, 41, 55, 0.96), rgba(17, 24, 39, 0.94)),
    linear-gradient(180deg, rgba(18, 24, 34, 0.98), rgba(11, 18, 28, 0.98));
  box-shadow:
    0 18px 34px rgba(2, 6, 23, 0.42),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

:global(:root[data-guaxi-theme='dark']) .python-runtime-entry {
  background: linear-gradient(135deg,
      color-mix(in srgb, var(--guaxi-panel-bg) 88%, #274c77 12%),
      color-mix(in srgb, var(--guaxi-panel-bg) 90%, #1f6f5c 10%));
  border-color: rgba(71, 85, 105, 0.72);
}

.dark-mode .python-runtime-meta__item,
.dark .python-runtime-meta__item,
.dark-mode .python-runtime-package-pill,
.dark .python-runtime-package-pill,
.dark-mode .python-runtime-guide,
.dark .python-runtime-guide,
.dark-mode .python-runtime-archive-card,
.dark .python-runtime-archive-card,
.dark-mode .python-runtime-status-card,
.dark .python-runtime-status-card,
.dark-mode .python-runtime-install-card,
.dark .python-runtime-install-card,
.dark-mode .python-runtime-package-browser,
.dark .python-runtime-package-browser,
.dark-mode .python-runtime-package-log,
.dark .python-runtime-package-log,
.dark-mode .python-runtime-package-row,
.dark .python-runtime-package-row,
:global(:root[data-guaxi-theme='dark']) .python-runtime-meta__item,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-pill,
:global(:root[data-guaxi-theme='dark']) .python-runtime-guide,
:global(:root[data-guaxi-theme='dark']) .python-runtime-archive-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-install-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-browser,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-log,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-content-bg, #282c34);
}

.dark-mode .python-runtime-package-log__output,
.dark .python-runtime-package-log__output,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-log__output {
  /* 设计说明：stdout/stderr 跟随编辑器内容底色，避免 Python 工作台在暗色模式里出现独立深蓝面板层。 */
  background: var(--guaxi-content-bg, #282c34);
  border-color: var(--guaxi-border, #3e4451);
}

.dark-mode .python-runtime-package-log__output.is-error,
.dark .python-runtime-package-log__output.is-error,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-log__output.is-error {
  background: var(--guaxi-content-bg, #282c34);
  border-color: color-mix(in srgb, #e06c75 38%, var(--guaxi-border, #3e4451));
}

.dark-mode .python-runtime-ghost-btn,
.dark .python-runtime-ghost-btn,
.dark-mode .python-runtime-chip-btn,
.dark .python-runtime-chip-btn,
.dark-mode .python-runtime-install__btn,
.dark .python-runtime-install__btn,
.dark-mode .python-runtime-install__input,
.dark .python-runtime-install__input,
.dark-mode .python-runtime-package-browser__search,
.dark .python-runtime-package-browser__search,
.dark-mode .guaxi-dataframe-btn,
:global(:root[data-guaxi-theme='dark']) .python-runtime-ghost-btn,
:global(:root[data-guaxi-theme='dark']) .python-runtime-chip-btn,
:global(:root[data-guaxi-theme='dark']) .python-runtime-install__btn,
:global(:root[data-guaxi-theme='dark']) .python-runtime-install__input,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-browser__search,
:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-btn {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-panel-bg, #21252b);
  color: var(--guaxi-text-primary, #abb2bf);
}

.dark-mode .python-runtime-package-row__action,
.dark .python-runtime-package-row__action,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row__action {
  border-color: var(--guaxi-border, #3e4451);
  background: var(--guaxi-panel-bg, #21252b);
  color: var(--guaxi-text-primary, #abb2bf);
}

.dark-mode .python-runtime-package-row__action:hover,
.dark .python-runtime-package-row__action:hover,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row__action:hover {
  border-color: color-mix(in srgb, #61afef 42%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 84%, #abb2bf 16%);
}

.dark-mode .python-runtime-package-row__action.is-danger,
.dark .python-runtime-package-row__action.is-danger,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row__action.is-danger {
  color: #e06c75;
  border-color: color-mix(in srgb, #e06c75 36%, var(--guaxi-border, #3e4451));
}

.dark-mode .python-runtime-guide__path-value,
.dark .python-runtime-guide__path-value,
.dark-mode .python-runtime-archive-card__path,
.dark .python-runtime-archive-card__path,
:global(:root[data-guaxi-theme='dark']) .python-runtime-guide__path-value,
:global(:root[data-guaxi-theme='dark']) .python-runtime-archive-card__path {
  background: var(--guaxi-panel-bg, #21252b);
  color: var(--guaxi-text-primary, #abb2bf);
}

.dark-mode .python-runtime-guide__step.done,
.dark .python-runtime-guide__step.done,
:global(:root[data-guaxi-theme='dark']) .python-runtime-guide__step.done {
  color: #8af0b4;
  border-color: rgba(74, 222, 128, 0.26);
  background: rgba(22, 101, 52, 0.28);
}

.dark-mode .python-runtime-package-pill--button:hover,
.dark .python-runtime-package-pill--button:hover,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-pill--button:hover {
  background: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 86%, var(--guaxi-text-primary, #abb2bf) 14%);
}

.dark-mode .python-runtime-panel,
.dark .python-runtime-panel,
:global(:root[data-guaxi-theme='dark']) .python-runtime-panel {
  background: var(--guaxi-panel-bg, #21252b);
  border-color: var(--guaxi-border, #3e4451);
  box-shadow:
    0 18px 42px rgba(0, 0, 0, 0.36),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
}

.dark-mode .python-runtime-status-card.is-success,
.dark .python-runtime-status-card.is-success,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-success {
  border-color: color-mix(in srgb, #98c379 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #98c379 12%);
}

.dark-mode .python-runtime-status-card.is-warning,
.dark .python-runtime-status-card.is-warning,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-warning {
  border-color: color-mix(in srgb, #e5c07b 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #e5c07b 12%);
}

.dark-mode .python-runtime-status-card.is-danger,
.dark .python-runtime-status-card.is-danger,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-danger {
  border-color: color-mix(in srgb, #e06c75 34%, var(--guaxi-border, #3e4451));
  background: color-mix(in srgb, var(--guaxi-content-bg, #282c34) 88%, #e06c75 12%);
}

.dark-mode .python-runtime-panel__title,
.dark .python-runtime-panel__title,
.dark-mode .python-runtime-status-card__value,
.dark .python-runtime-status-card__value,
.dark-mode .python-runtime-section__title,
.dark .python-runtime-section__title,
.dark-mode .python-runtime-package-row,
.dark .python-runtime-package-row,
:global(:root[data-guaxi-theme='dark']) .python-runtime-panel__title,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card__value,
:global(:root[data-guaxi-theme='dark']) .python-runtime-section__title,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row {
  color: var(--guaxi-text-primary, #abb2bf);
}

.dark-mode .python-runtime-panel__subtitle,
.dark .python-runtime-panel__subtitle,
.dark-mode .python-runtime-status-card__label,
.dark .python-runtime-status-card__label,
.dark-mode .python-runtime-package-browser__empty,
.dark .python-runtime-package-browser__empty,
:global(:root[data-guaxi-theme='dark']) .python-runtime-panel__subtitle,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card__label,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-browser__empty {
  color: var(--guaxi-text-secondary, #9da5b4);
}

:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-btn:hover {
  background: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 86%, var(--guaxi-text-primary, #abb2bf) 14%);
}

:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-table-wrap {
  border-color: rgba(71, 85, 105, 0.72);
  background: rgba(15, 23, 42, 0.64);
}

:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-table th {
  background: rgba(30, 41, 59, 0.98);
  color: #f8fafc;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-table td {
  color: #e6edf3;
  border-bottom-color: rgba(71, 85, 105, 0.4);
}

.dark-mode .guaxi-pycell-session,
.dark-mode .guaxi-pycell-rich-display,
:root[data-guaxi-theme='dark'] .guaxi-pycell-session,
:root[data-guaxi-theme='dark'] .guaxi-pycell-rich-display {
  background: rgba(15, 23, 42, 0.84);
  border-color: rgba(71, 85, 105, 0.42);
}

.dark-mode .guaxi-pycell-var-row,
.dark-mode .guaxi-pycell-session-empty,
:root[data-guaxi-theme='dark'] .guaxi-pycell-var-row,
:root[data-guaxi-theme='dark'] .guaxi-pycell-session-empty {
  background: rgba(15, 23, 42, 0.68);
  border-color: rgba(71, 85, 105, 0.35);
}

.dark-mode .guaxi-pycell-session-title,
.dark-mode .guaxi-pycell-var-name,
:root[data-guaxi-theme='dark'] .guaxi-pycell-session-title,
:root[data-guaxi-theme='dark'] .guaxi-pycell-var-name {
  color: rgba(226, 232, 240, 0.96);
}

.dark-mode .guaxi-pycell-session-meta,
.dark-mode .guaxi-pycell-var-type,
.dark-mode .guaxi-pycell-var-preview,
.dark-mode .guaxi-pycell-session-empty,
:root[data-guaxi-theme='dark'] .guaxi-pycell-session-meta,
:root[data-guaxi-theme='dark'] .guaxi-pycell-var-type,
:root[data-guaxi-theme='dark'] .guaxi-pycell-var-preview,
:root[data-guaxi-theme='dark'] .guaxi-pycell-session-empty {
  color: rgba(148, 163, 184, 0.94);
}

.dark-mode .preview :deep(.guaxi-preview-file-card),
:root.dark .preview :deep(.guaxi-preview-file-card),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card) {
  border-color: rgba(71, 85, 105, 0.42);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #111827) 90%, var(--guaxi-content-bg, #0f172a) 10%);
  box-shadow: 0 10px 24px rgba(2, 6, 23, 0.28);
}

.dark-mode .preview :deep(.guaxi-preview-file-card__link:hover),
:root.dark .preview :deep(.guaxi-preview-file-card__link:hover),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card__link:hover) {
  background: color-mix(in srgb, var(--guaxi-panel-bg, #111827) 94%, var(--guaxi-content-bg, #0f172a) 6%);
}

.dark-mode .preview :deep(.guaxi-preview-file-card__eyebrow),
:root.dark .preview :deep(.guaxi-preview-file-card__eyebrow),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card__eyebrow) {
  background: rgba(34, 211, 238, 0.14);
  color: #67e8f9;
}

.dark-mode .preview :deep(.guaxi-preview-file-card__title),
:root.dark .preview :deep(.guaxi-preview-file-card__title),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card__title) {
  color: #e5edf7;
}

.dark-mode .preview :deep(.guaxi-preview-file-card__excerpt),
:root.dark .preview :deep(.guaxi-preview-file-card__excerpt),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card__excerpt) {
  color: rgba(203, 213, 225, 0.9);
}

.dark-mode .preview :deep(.guaxi-preview-file-card__meta),
:root.dark .preview :deep(.guaxi-preview-file-card__meta),
:root[data-guaxi-theme='dark'] .preview :deep(.guaxi-preview-file-card__meta) {
  color: rgba(148, 163, 184, 0.9);
}

.dark-mode .preview .guaxi-codecell-btn,
.dark-mode .preview .guaxi-pycell-btn,
:root[data-guaxi-theme='dark'] .preview .guaxi-codecell-btn,
:root[data-guaxi-theme='dark'] .preview .guaxi-pycell-btn {
  border-color: rgba(71, 85, 105, 0.55) !important;
  background: rgba(15, 23, 42, 0.9) !important;
  color: #e5edf7 !important;
  box-shadow: 0 8px 18px rgba(2, 6, 23, 0.28);
}

.dark-mode .preview .guaxi-codecell-btn:hover,
.dark-mode .preview .guaxi-pycell-btn:hover,
:root[data-guaxi-theme='dark'] .preview .guaxi-codecell-btn:hover,
:root[data-guaxi-theme='dark'] .preview .guaxi-pycell-btn:hover {
  border-color: rgba(96, 165, 250, 0.55) !important;
  background: rgba(30, 41, 59, 0.98) !important;
  box-shadow: 0 12px 24px rgba(2, 6, 23, 0.36);
}

.dark-mode .guaxi-pdf-preview-toolbar,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-preview-toolbar {
  border-bottom-color: rgba(71, 85, 105, 0.45);
  background: linear-gradient(180deg, rgba(17, 24, 39, 0.98), rgba(20, 28, 42, 0.96));
}

.dark-mode .guaxi-pdf-file-badge,
.dark-mode .guaxi-pdf-toolbar-indicator,
.dark-mode .guaxi-pdf-toolbar-btn,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-file-badge,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-toolbar-indicator,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-toolbar-btn {
  background: rgba(15, 23, 42, 0.88);
  border-color: rgba(71, 85, 105, 0.5);
  color: #e5edf7;
}

.dark-mode .guaxi-pdf-toolbar-btn.accent,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-toolbar-btn.accent {
  background: linear-gradient(135deg, #2563eb, #3b82f6);
  color: #ffffff;
}

.dark-mode .guaxi-pdf-preview-viewport,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-preview-viewport {
  background:
    linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(17, 24, 39, 0.98)),
    repeating-linear-gradient(45deg, rgba(255, 255, 255, 0.04) 0 12px, rgba(255, 255, 255, 0.02) 12px 24px);
}

.dark-mode .guaxi-pdf-page-card,
:global(:root[data-guaxi-theme='dark']) .guaxi-pdf-page-card {
  border-color: rgba(100, 116, 139, 0.34);
  box-shadow:
    0 18px 34px rgba(2, 6, 23, 0.4),
    0 4px 12px rgba(2, 6, 23, 0.22);
}

@media (max-width: 920px) {
  .guaxi-pdf-preview-toolbar {
    flex-wrap: wrap;
    justify-content: center;
  }

  .guaxi-pdf-file-badge {
    max-width: min(100%, 360px);
  }

  .guaxi-pdf-preview-viewport {
    padding-left: 10px;
    padding-right: 10px;
  }
}

.preview :deep(.hljs),
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #ffffff !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
  color: #24292f;
}

.preview :deep(.hljs code),
.hljs code {
  font-family: inherit !important;
  background: transparent !important;
  border: 0 !important;
  padding: 0 !important;
}

.preview :deep(pre:not(.hljs)) {
  border-radius: 6px;
  border: 1px solid #e1e4e8;
  background: #ffffff !important;
  margin: 1em 0;
  overflow-x: auto;
}

.preview :deep(pre:not(.hljs) > code) {
  display: block;
  border-radius: inherit;
  border: 0;
  background: transparent !important;
  color: #24292f;
  padding: 1.5em 1em;
}

.dark-mode .preview :deep(pre:not(.hljs)),
:global(:root[data-guaxi-theme='dark']) .preview :deep(pre:not(.hljs)) {
  /* 设计说明：
   * 1. 未声明语言的 fenced code block 通常不会拿到 `.hljs` 类名，因此不能只依赖 `.hljs` 的暗色样式。
   * 2. 这类代码块是通过 `v-html` 注入到预览区里的，`scoped` 下必须配合 `:deep(...)` 才能真正命中内部 `pre` 元素。
   * 3. `code` 子节点继续保持透明背景，这样不会与外层容器叠出一层亮色底板。
   */
  border-color: #30363d;
  background: #1f242d !important;
}

.dark-mode .preview :deep(pre:not(.hljs) > code),
:global(:root[data-guaxi-theme='dark']) .preview :deep(pre:not(.hljs) > code) {
  background: transparent !important;
  color: #c9d1d9;
}

.preview :not(pre)>code:not(.hljs) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
  padding: 0.1em 0.35em;
  border-radius: 4px;
  background: rgba(175, 184, 193, 0.2);
}

/* 暗色模式代码块 */
.dark-mode .hljs,
:global(:root[data-guaxi-theme='dark']) .hljs,
:global(:root[data-guaxi-theme='dark']) .preview :deep(.hljs) {
  background: #1f242d !important;
  border-color: #30363d;
  color: #c9d1d9;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark-mode .hljs code,
:global(:root[data-guaxi-theme='dark']) .hljs code,
:global(:root[data-guaxi-theme='dark']) .preview :deep(.hljs code) {
  font-family: inherit !important;
}

.dark-mode .preview :not(pre)>code:not(.hljs),
:global(:root[data-guaxi-theme='dark']) .preview :not(pre)>code:not(.hljs) {
  background: rgba(110, 118, 129, 0.25);
  color: #c9d1d9;
}

.dark-mode .hljs-comment,
.dark-mode .hljs-quote,
:global(:root[data-guaxi-theme='dark']) .hljs-comment,
:global(:root[data-guaxi-theme='dark']) .hljs-quote {
  color: #8b949e;
}

.dark-mode .hljs-keyword,
.dark-mode .hljs-selector-tag,
.dark-mode .hljs-literal,
.dark-mode .hljs-built_in,
.dark-mode .hljs-type,
:global(:root[data-guaxi-theme='dark']) .hljs-keyword,
:global(:root[data-guaxi-theme='dark']) .hljs-selector-tag,
:global(:root[data-guaxi-theme='dark']) .hljs-literal,
:global(:root[data-guaxi-theme='dark']) .hljs-built_in,
:global(:root[data-guaxi-theme='dark']) .hljs-type {
  color: #ff7b72;
}

.dark-mode .hljs-string,
.dark-mode .hljs-doctag,
.dark-mode .hljs-regexp,
:global(:root[data-guaxi-theme='dark']) .hljs-string,
:global(:root[data-guaxi-theme='dark']) .hljs-doctag,
:global(:root[data-guaxi-theme='dark']) .hljs-regexp {
  color: #a5d6ff;
}

.dark-mode .hljs-number,
.dark-mode .hljs-symbol,
.dark-mode .hljs-bullet,
:global(:root[data-guaxi-theme='dark']) .hljs-number,
:global(:root[data-guaxi-theme='dark']) .hljs-symbol,
:global(:root[data-guaxi-theme='dark']) .hljs-bullet {
  color: #79c0ff;
}

.dark-mode .hljs-title,
.dark-mode .hljs-section,
.dark-mode .hljs-function .hljs-title,
:global(:root[data-guaxi-theme='dark']) .hljs-title,
:global(:root[data-guaxi-theme='dark']) .hljs-section,
:global(:root[data-guaxi-theme='dark']) .hljs-function .hljs-title {
  color: #d2a8ff;
}

.dark-mode .hljs-attr,
.dark-mode .hljs-attribute,
.dark-mode .hljs-variable,
.dark-mode .hljs-template-variable,
.dark-mode .hljs-property,
.dark-mode .hljs-params,
:global(:root[data-guaxi-theme='dark']) .hljs-attr,
:global(:root[data-guaxi-theme='dark']) .hljs-attribute,
:global(:root[data-guaxi-theme='dark']) .hljs-variable,
:global(:root[data-guaxi-theme='dark']) .hljs-template-variable,
:global(:root[data-guaxi-theme='dark']) .hljs-property,
:global(:root[data-guaxi-theme='dark']) .hljs-params {
  color: #ffa657 !important;
}

.dark-mode .hljs-subst,
.dark-mode .hljs-operator,
.dark-mode .hljs-punctuation,
:global(:root[data-guaxi-theme='dark']) .hljs-subst,
:global(:root[data-guaxi-theme='dark']) .hljs-operator,
:global(:root[data-guaxi-theme='dark']) .hljs-punctuation {
  color: #c9d1d9 !important;
}

.dark-mode .hljs-meta,
.dark-mode .hljs-tag,
.dark-mode .hljs-name,
:global(:root[data-guaxi-theme='dark']) .hljs-meta,
:global(:root[data-guaxi-theme='dark']) .hljs-tag,
:global(:root[data-guaxi-theme='dark']) .hljs-name {
  color: #7ee787;
}

/* 修改为以下样式： */
.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

.preview :deep(table) {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid #ddd;
  background: #ffffff;
  color: #24292f;
}

.preview :deep(th),
.preview :deep(td) {
  border: 1px solid #ddd;
  padding: 8px;
  text-align: center;
  background: #ffffff;
  color: #24292f;
}

.preview :deep(th) {
  background-color: #f4f4f4;
  font-weight: bold;
}

.preview :deep(tr:nth-child(even) td) {
  background-color: #f9f9f9;
}

.preview :deep(input),
.preview :deep(textarea),
.preview :deep(select),
.preview :deep([contenteditable='true']) {
  background: var(--guaxi-content-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
  border-color: var(--guaxi-border, #d0d7de);
}

:global(:root[data-guaxi-theme='dark']) .preview :deep(table) {
  border-color: #30363d;
  background: #0f1720;
  color: #e6edf3;
}

:global(:root[data-guaxi-theme='dark']) .preview :deep(th),
:global(:root[data-guaxi-theme='dark']) .preview :deep(td) {
  border-color: #30363d;
  background: #0f1720;
  color: #e6edf3;
}

:global(:root[data-guaxi-theme='dark']) .preview :deep(th) {
  background: #18222d;
}

:global(:root[data-guaxi-theme='dark']) .preview :deep(tr:nth-child(even) td) {
  background: #131d27;
}

:global(:root[data-guaxi-theme='dark']) .preview :deep(input),
:global(:root[data-guaxi-theme='dark']) .preview :deep(textarea),
:global(:root[data-guaxi-theme='dark']) .preview :deep(select),
:global(:root[data-guaxi-theme='dark']) .preview :deep([contenteditable='true']) {
  background: #11161c;
  color: #e6edf3;
  border-color: #30363d;
}

/* 设计说明：
 * 1. 这里位于普通 `<style>` 块，不是 `<style scoped>`。
 * 2. 旧代码使用 `.preview :deep(...)`，Vue 不会在这个样式块里帮我们展开 `:deep()`，
 *    所以预览区图片规则实际上没有命中，表现出来就是图片始终靠左。
 * 3. 预览内容直接通过 `v-html` 注入到 `.preview` 容器下，使用普通后代选择器即可准确生效。
 */
/* 设计说明：
 * 1. 自动居中的图片规则只应作用于 Markdown 渲染器生成的普通段落与媒体组，
 *    不能全局覆盖作者手写 HTML 的图片布局，否则像徽章组这类 inline 图片会被错误拆成一列。
 * 2. 因此这里去掉裸 `img` 选择器，只保留 Markdown 段落和媒体组的图片约束。
 */
.preview .md-paragraph img,
.preview .guaxi-media-group__image,
.preview li > p img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}

.preview p > img:only-child,
.preview p > a:only-child > img:only-child {
  display: block;
  margin-left: auto;
  margin-right: auto;
}

/* 设计说明：
 * 1. 对于手写 HTML 段落（例如 `<p align="center"><a><img></a>...</p>`），
 *    图片常常需要保持 inline / inline-block，交给段落自身的对齐方式控制。
 * 2. 这里把带 `align` / `style` 的段落中的图片恢复为 inline-block，避免全局 block 图片规则
 *    把徽章、Logo 组错误地一张一行展开。
 */
.preview p[align] > img,
.preview p[align] > a > img,
.preview p[style*='text-align'] > img,
.preview p[style*='text-align'] > a > img {
  display: inline-block;
  max-width: 100%;
  height: auto;
  margin: 0;
  vertical-align: middle;
}

/* 设计说明：
 * 1. 这里不再对 `p > a:only-child` 做通用块级化处理，因为参考式链接、脚注链接等文本链接
 *    也可能恰好成为段落中的唯一子元素，之前的写法会把它们错误地“媒体化”，导致正文排版被居中。
 * 2. 纯图片链接现在统一由 `enhancePreviewHtmlWithMediaGroups` 转成 `.guaxi-media-group`，
 *    由媒体分组样式负责居中和响应式布局，普通文字链接则继续走正常文档流。
 */

.preview .guaxi-media-group {
  --guaxi-media-gap: clamp(14px, 2vw, 24px);
  display: flex !important;
  flex-wrap: wrap;
  justify-content: center !important;
  align-items: stretch;
  align-content: flex-start;
  gap: var(--guaxi-media-gap);
  width: 100%;
  max-width: 1000px;
  margin: 22px auto;
  padding: 6px 0;
}

.preview .guaxi-media-group.is-single {
  display: block !important;
  width: min(100%, 760px);
  gap: 0;
}

.preview .guaxi-media-group.is-gallery {
  display: grid !important;
  grid-template-columns: repeat(auto-fit, minmax(220px, 320px));
  justify-content: center !important;
  justify-items: center;
  align-items: start;
  padding-left: 6px;
  padding-right: 6px;
}

.preview .guaxi-media-group__item {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  width: fit-content;
  max-width: 100%;
  box-sizing: border-box;
  padding: 0;
  border: none;
  background: transparent;
  box-shadow: none;
  overflow: visible;
  text-decoration: none;
  transition: opacity 0.18s ease;
}

.preview .guaxi-media-group__item:hover {
  opacity: 0.98;
}

.preview .guaxi-media-group.is-single > .guaxi-media-group__item {
  display: flex;
  flex: 0 1 auto;
  min-width: 0;
  width: fit-content;
  max-width: min(100%, 760px);
  margin-left: auto;
  margin-right: auto;
  padding: 0;
  border: none;
  background: transparent;
  box-shadow: none;
}

.preview .guaxi-media-group__image {
  display: block;
  width: 100%;
  max-width: 100%;
  max-height: min(56vh, 420px);
  height: auto;
  margin: 0;
  border-radius: 16px;
  object-fit: contain;
}

.preview .guaxi-media-group.is-single .guaxi-media-group__image {
  width: auto;
  max-width: min(100%, 760px);
  max-height: min(72vh, 640px);
}

:global(:root[data-guaxi-theme='dark']) .preview .guaxi-media-group__item {
  background: transparent;
  box-shadow: none;
}

:global(:root[data-guaxi-theme='dark']) .preview .guaxi-media-group__item:hover {
  opacity: 0.98;
}

@media (max-width: 760px) {
  .preview .guaxi-media-group,
  .preview .guaxi-media-group.is-gallery {
    width: 100%;
    max-width: 100%;
  }

  .preview .guaxi-media-group__item {
    max-width: 100%;
    width: 100%;
  }
}

.search-highlight {
  background-color: rgb(241, 168, 10) !important;
  color: #000 !important;
  border-radius: 2px;
  padding: 0 2px;
}

.dark .search-highlight {
  background-color: rgb(255, 208, 0) !important;
  color: #000 !important;
}

.python-console-slide-enter-active,
.python-console-slide-leave-active {
  transition: all 0.2s ease;
}

.python-console-slide-enter-from,
.python-console-slide-leave-to {
  opacity: 0;
  transform: translateY(8px);
}

.python-console-dock {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 22px;
  z-index: 8;
  display: flex;
  flex-direction: column;
  padding: 0;
  box-sizing: border-box;
  overflow: hidden;
  border-top: none;
  background: var(--toolbar-bg, #252526);
  box-shadow: none;
}

.python-console-dock.terminal-only {
  /* 设计说明：
   * 1. 当没有活动文件时，状态栏与编辑区都不会渲染。
   * 2. 这时终端 dock 直接贴到底部，避免底部再留出一条空白保留带。
   */
  bottom: 0 !important;
}

.python-console-dock::before,
.python-console-dock::after {
  content: none !important;
}

.python-console-dock.dragging {
  transition: none;
}

.python-console-dock__resize-handle {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 16px;
  cursor: ns-resize;
  user-select: none;
  background: var(--toolbar-bg, var(--guaxi-panel-bg, #252526));
  box-shadow: none;
  border-top: none;
}

.python-console-dock__resize-handle::before,
.python-console-dock__resize-handle::after {
  content: none !important;
}

.python-console-dock__resize-grip {
  width: 68px;
  height: 4px;
  border-radius: 999px;
  background: rgba(100, 116, 139, 0.36);
}

.python-console-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1 1 auto;
  height: 100%;
  min-height: 0;
  margin-top: 0;
  padding: 14px 16px;
  box-sizing: border-box;
  overflow: hidden;
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 14px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(245, 247, 250, 0.98));
  box-shadow: none;
}

.python-console-dock :deep(.guaxi-terminal-panel) {
  width: 100%;
  height: 100%;
  border-top: none !important;
  box-shadow: none !important;
}

.python-console-dock :deep(.guaxi-terminal-panel__toolbar) {
  border-top: none !important;
  box-shadow: none !important;
}

.python-console-dock :deep(.guaxi-terminal-panel__toolbar::before),
.python-console-dock :deep(.guaxi-terminal-panel__toolbar::after),
.python-console-dock :deep(.guaxi-terminal-panel::before),
.python-console-dock :deep(.guaxi-terminal-panel::after) {
  content: none !important;
}

.python-console-panel__header {
  display: flex;
  flex: 0 0 auto;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.python-console-panel__title {
  font-size: 15px;
  font-weight: 700;
  color: #10233f;
}

.python-console-panel__meta {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: #5b6b81;
  word-break: break-all;
}

.python-console-panel__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.python-console-panel__btn {
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.88);
  color: #17355f;
  font-size: 12px;
  font-weight: 600;
  padding: 6px 12px;
  cursor: pointer;
}

.python-console-panel__output {
  flex: 1 1 auto;
  min-height: 72px;
  overflow: auto;
  padding: 12px;
  box-sizing: border-box;
  border-radius: 12px;
  background: #0f172a;
  color: #dbe6ff;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.python-console-panel__entry + .python-console-panel__entry {
  margin-top: 16px;
}

.python-console-panel__prompt {
  font-size: 12px;
  font-weight: 700;
  color: #93c5fd;
}

.python-console-panel__prompt--out {
  margin-top: 8px;
  color: #86efac;
}

.python-console-panel__code,
.python-console-panel__result {
  margin: 6px 0 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.6;
}

.python-console-panel__empty {
  color: rgba(219, 230, 255, 0.72);
  font-size: 12px;
}

.python-console-panel__input {
  flex: 0 0 auto;
  width: 100%;
  min-height: 84px;
  max-height: 180px;
  resize: vertical;
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 12px;
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.92);
  color: #10233f;
  font-size: 13px;
  line-height: 1.6;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.editor-split-wrapper.with-python-console {
  transition: padding-bottom 0.2s ease;
}

.guaxi-pycell-card {
  margin: 4px 0 8px;
  padding: 0;
  border: 1px solid var(--pycell-card-border);
  border-radius: 16px;
  background: var(--pycell-card-bg);
  box-shadow: var(--pycell-card-shadow);
  overflow: hidden;
  pointer-events: auto;
}

.guaxi-pycell-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 14px 9px;
  border-bottom: 1px solid var(--pycell-header-border);
  background: var(--pycell-header-bg);
}

.guaxi-pycell-card__prompt {
  flex: 0 0 auto;
  min-width: 56px;
  font-size: 12px;
  color: var(--pycell-prompt);
  font-weight: 700;
  letter-spacing: 0.02em;
}

.guaxi-pycell-card.is-running .guaxi-pycell-card__prompt {
  color: #2563eb;
  animation: guaxi-pycell-prompt-pulse 1.1s ease-in-out infinite;
}

.guaxi-pycell-card__title {
  font-size: 14px;
  line-height: 1.3;
  font-weight: 700;
  color: var(--pycell-title);
}

.guaxi-pycell-card__meta {
  margin-top: 4px;
  font-size: 11px;
  color: var(--pycell-meta);
}

.guaxi-pycell-card__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.guaxi-pycell-card__btn {
  border: 1px solid var(--pycell-btn-border);
  border-radius: 999px;
  background: var(--pycell-btn-bg);
  color: var(--pycell-btn-text);
  font-size: 11px;
  font-weight: 600;
  padding: 5px 10px;
  cursor: pointer;
  pointer-events: auto;
  transition: background 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}

.guaxi-pycell-card__btn:hover {
  border-color: var(--pycell-btn-hover-border);
  background: var(--pycell-btn-hover-bg);
  box-shadow: var(--pycell-btn-hover-shadow);
  transform: translateY(-1px);
}

.guaxi-pycell-card__btn:focus-visible {
  outline: none;
  border-color: var(--pycell-btn-hover-border);
  box-shadow: var(--pycell-btn-focus-ring);
}

.guaxi-pycell-card__body {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.guaxi-pycell-card__code-shell,
.guaxi-pycell-card__panel,
.guaxi-pycell-card__rich,
.guaxi-inline-dataframe {
  border: none;
  border-radius: 0;
  background: transparent;
}

.guaxi-pycell-card__code-shell {
  position: relative;
  overflow: hidden;
  background: var(--pycell-editor-bg);
}

.guaxi-pycell-card__panel.is-collapsed {
  display: none;
}

.guaxi-pycell-card__editor {
  position: relative;
  z-index: 2;
  width: 100%;
  min-height: 96px;
  resize: none;
  border: none;
  outline: none;
  padding: 12px 14px 14px;
  background: transparent;
  color: transparent;
  font-size: 13px;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  box-sizing: border-box;
  pointer-events: auto;
  caret-color: var(--pycell-editor-text);
}

.guaxi-pycell-card__editor:focus {
  background: transparent;
}

.guaxi-pycell-card__editor::selection {
  color: transparent;
  background: rgba(96, 165, 250, 0.28);
}

.guaxi-pycell-card__highlight {
  position: absolute;
  inset: 0;
  z-index: 1;
  margin: 0;
  min-height: 96px;
  overflow: hidden;
  padding: 12px 14px 14px;
  box-sizing: border-box;
  white-space: pre-wrap;
  word-break: break-word;
  pointer-events: none;
  color: var(--pycell-editor-text);
  background: var(--pycell-editor-bg);
  font-size: 13px;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.guaxi-pycell-card__code-shell:focus-within .guaxi-pycell-card__highlight {
  background: var(--pycell-editor-focus-bg);
}

.guaxi-pycell-card__highlight code {
  font: inherit;
}

.guaxi-pycell-card__highlight .hljs-comment {
  color: #6a737d;
}

.guaxi-pycell-card__highlight .hljs-keyword {
  color: #a626a4;
}

.guaxi-pycell-card__highlight .hljs-string {
  color: #50a14f;
}

.guaxi-pycell-card__highlight .hljs-number,
.guaxi-pycell-card__highlight .hljs-literal {
  color: #986801;
}

.guaxi-pycell-card__highlight .hljs-title,
.guaxi-pycell-card__highlight .hljs-title.function_ {
  color: #4078f2;
}

.guaxi-pycell-card__highlight .hljs-property,
.guaxi-pycell-card__highlight .hljs-variable {
  color: #e45649;
}

.guaxi-pycell-card__panel {
  padding: 10px 14px 14px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
  color: var(--pycell-panel-text);
  background: var(--pycell-panel-bg);
}

.guaxi-pycell-card__panel--output::before,
.guaxi-pycell-card__panel--session::before {
  display: block;
  margin-bottom: 8px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-pycell-card__panel--output::before {
  content: 'Out [ ]';
  color: var(--pycell-output-label);
}

.guaxi-pycell-card__panel--session::before {
  content: 'Session';
  color: var(--pycell-session-label);
}

.guaxi-pycell-card__stdout,
.guaxi-pycell-card__rich--markdown {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.65;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  color: var(--pycell-output-text);
  background: var(--pycell-output-bg);
  border: 1px solid var(--pycell-output-border);
  border-radius: 12px;
  padding: 12px 13px;
}

.guaxi-pycell-card__image {
  display: block;
  max-width: 100%;
  height: auto;
  border-radius: 12px;
}

.guaxi-pycell-card__rich--svg svg {
  max-width: 100%;
  height: auto;
}

.guaxi-pycell-card__empty {
  font-size: 12px;
  color: var(--pycell-empty);
}

.guaxi-pycell-card__var + .guaxi-pycell-card__var {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-pycell-card__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.guaxi-pycell-card__var-name {
  font-weight: 700;
  color: var(--pycell-var-name);
}

.guaxi-pycell-card__var-type {
  color: var(--pycell-var-type);
  font-size: 12px;
}

.guaxi-pycell-card__var-preview {
  margin-top: 4px;
  color: var(--pycell-var-preview);
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.guaxi-inline-dataframe {
  overflow: hidden;
}

.guaxi-inline-dataframe__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
}

.guaxi-inline-dataframe__meta {
  color: #41536b;
  font-size: 12px;
}

.guaxi-inline-dataframe__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.guaxi-inline-dataframe__btn {
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 999px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(241, 245, 249, 0.96));
  color: #17355f;
  font-size: 12px;
  padding: 5px 10px;
  cursor: pointer;
  transition: background 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}

.guaxi-inline-dataframe__btn:hover {
  border-color: rgba(59, 130, 246, 0.35);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(235, 244, 255, 0.98));
  box-shadow: 0 6px 16px rgba(59, 130, 246, 0.14);
  transform: translateY(-1px);
}

.guaxi-inline-dataframe__viewport {
  overflow: auto;
}

.guaxi-inline-dataframe__table {
  width: 100%;
  border-collapse: collapse;
}

.guaxi-inline-dataframe__table th,
.guaxi-inline-dataframe__table td {
  padding: 8px 10px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
  text-align: left;
  white-space: nowrap;
}

.guaxi-inline-dataframe__table th {
  background: rgba(226, 232, 240, 0.48);
  font-weight: 700;
}

:global(:root[data-guaxi-theme='dark']) .python-console-panel,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card {
  border-color: rgba(148, 163, 184, 0.22);
  box-shadow:
    0 20px 38px rgba(2, 6, 23, 0.42),
    0 1px 0 rgba(148, 163, 184, 0.08) inset;
}

:global(:root[data-guaxi-theme='dark']) .python-console-dock {
  border-top-color: rgba(148, 163, 184, 0.16);
}

:global(:root[data-guaxi-theme='dark']) .python-console-dock__resize-grip {
  background: rgba(148, 163, 184, 0.42);
}

:global(:root[data-guaxi-theme='dark']) .python-console-panel__title,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__title,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__var-name {
  color: #e5eefc;
}

:global(:root[data-guaxi-theme='dark']) .python-console-panel__meta,
:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe__meta {
  color: #c7d4e5;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-comment {
  color: #5c6370;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-keyword {
  color: #c678dd;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-string {
  color: #98c379;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-number,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-literal {
  color: #d19a66;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-title,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-title.function_ {
  color: #61afef;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-property,
:global(:root[data-guaxi-theme='dark']) .guaxi-pycell-card__highlight .hljs-variable {
  color: #e06c75;
}

:global(:root[data-guaxi-theme='dark']) .python-console-panel__btn,
:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe__btn {
  border-color: rgba(148, 163, 184, 0.34);
  background: linear-gradient(180deg, rgba(30, 41, 59, 0.98), rgba(22, 30, 47, 0.98));
  color: #eff6ff;
  box-shadow: 0 6px 16px rgba(2, 6, 23, 0.18);
}

:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe__btn:hover {
  border-color: rgba(125, 211, 252, 0.58);
  background: linear-gradient(180deg, rgba(37, 50, 72, 0.99), rgba(29, 41, 61, 0.99));
  color: #ffffff;
  box-shadow: 0 8px 20px rgba(14, 165, 233, 0.16);
}

:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe__btn:focus-visible {
  outline: none;
  border-color: rgba(125, 211, 252, 0.68);
  box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.22);
}

:global(:root[data-guaxi-theme='dark']) .python-console-panel__input,
:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe {
  border-color: rgba(148, 163, 184, 0.16);
  background: rgba(13, 20, 34, 0.9);
  color: #f6f9ff;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-inline-dataframe__table th {
  background: rgba(51, 65, 85, 0.72);
}
</style>
