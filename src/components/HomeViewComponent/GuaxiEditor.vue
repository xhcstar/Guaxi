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
              title="Python 控制台 (Ctrl+`)"
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

              <div class="python-runtime-panel__actions">
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="resetPythonKernelSession">
                  重启会话
                </button>
                <button type="button" class="python-runtime-chip-btn" :disabled="pythonRuntimeBusy" @click="installPythonRuntimeArchive">
                  导入运行时包
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
                        <button
                          type="button"
                          class="python-runtime-package-row__action"
                          :disabled="pythonRuntimeBusy"
                          @click="managePythonPackage(pkg, 'upgrade')"
                        >
                          升级
                        </button>
                        <button
                          type="button"
                          class="python-runtime-package-row__action"
                          :disabled="pythonRuntimeBusy"
                          @click="managePythonPackage(pkg, 'reinstall')"
                        >
                          重装
                        </button>
                        <button
                          type="button"
                          class="python-runtime-package-row__action is-danger"
                          :disabled="pythonRuntimeBusy"
                          @click="managePythonPackage(pkg, 'uninstall')"
                        >
                          卸载
                        </button>
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
      <div v-if="!selectedFile" class="editor-empty">请选择一个文件以开始编辑</div>
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
      <div v-show="previewMode !== 'realtime' && showEditor" class="editor-wrapper" ref="editorWrapperRef">
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

        <transition name="python-console-slide">
          <div
            v-if="pythonConsoleVisible"
            class="python-console-dock"
            :class="{ busy: pythonConsoleBusy, dragging: pythonConsoleResizing }"
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
              :context-path="getPythonSessionTargetPath(fileTreeStore.activeRegion || 'left')"
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

        <div v-if="vimMode" class="vim-status-bar" ref="vimStatusBar"></div>
        <GuaxiEditorStatusBar class="editor-status-bar" />
      </div>

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
import { buildPythonCellBlock, createPythonCellId, createPythonCellStableId, createPythonSessionId, dataframeToMarkdownTable, parsePythonCellBlocks, transformPythonCellBlocksForPreview } from './utils/guaxiPythonCellSyntax'
import { getPythonCellSnapshot, patchPythonCellSnapshot } from './utils/guaxiPythonCellState'
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
  resetLspDisabled
} from '../../monaco/lspClient'
import { LSP_SUPPORTED_LANGUAGES } from '../../monaco/lspCapabilities'
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
  if (exists) return
  monaco.languages.register({
    id: 'latex',
    extensions: ['.tex', '.ltx', '.latex', '.cls', '.sty'],
    aliases: ['LaTeX', 'TeX', 'latex', 'tex'],
    mimetypes: ['text/x-tex', 'text/x-latex']
  })
  logger.info('[Editor] language registered', { languageId: 'latex' })
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
let realtimeVditorLinkInterceptorCleanup = null;
const REALTIME_VDITOR_INIT_TIMEOUT_MS = 12000

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
            label: `${name}${suffix}`,
            kind: isDir
              ? monaco.languages.CompletionItemKind.Folder
              : monaco.languages.CompletionItemKind.File,
            insertText: `${name}${suffix}`,
            range: replaceRangeForPartial,
            detail: isDir ? '目录' : '文件',
            sortText: isDir ? `0_${name}` : `1_${name}`
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
    createLatexSnippet('notin', '\\notin', '∉')
  ];

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

      const mathLspSuggestions = await provideMarkdownMathLspSuggestions(model, position, context);
      if (mathLspSuggestions.length) {
        return { suggestions: mathLspSuggestions };
      }

      const fenceLspSuggestions = await provideMarkdownFenceLspSuggestions(model, position, context);
      if (fenceLspSuggestions.length) {
        return { suggestions: fenceLspSuggestions };
      }

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
};

const tabBarRef = ref(null)
const tabRegionLeftRef = ref(null)
const tabRegionRightRef = ref(null)

const languagePairingConfigured = new Set()
const lspEnabled = ref(false)
const markdownFenceCompletionModels = new Map()
const MARKDOWN_FENCE_TRIGGER_CHARACTERS = ['.', ':', '>', '/', '\\', '"', "'", '@', '#', '(', ',', '[', '{', '-']
const EMBEDDED_LSP_READY_TIMEOUT_MS = 4000

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
const EMBEDDED_SEMANTIC_MAX_LINES = 240;
const EMBEDDED_VISIBLE_BLOCK_LIMIT = 6;

const EMBEDDED_WORD_SUGGEST_OPTIONS = {
  wordBasedSuggestions: 'off',
  quickSuggestions: {
    other: false,
    comments: false,
    strings: false
  },
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

const getMarkdownFenceHighlightState = (targetEditor) => {
  let state = markdownFenceHighlightStates.get(targetEditor);
  if (!state) {
    state = { decorations: [], timer: 0, lastKey: '', requestSeq: 0 };
    markdownFenceHighlightStates.set(targetEditor, state);
  }
  return state;
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
  if (type.includes('keyword') || type.includes('storage')) return 'md-fence-token-keyword';
  if (type.includes('number') || type.includes('constant.numeric')) return 'md-fence-token-number';
  if (type.includes('boolean')) return 'md-fence-token-boolean';
  if (type.includes('type') || type.includes('interface') || type.includes('enum') || type.includes('struct')) return 'md-fence-token-type';
  if (type.includes('class')) return 'md-fence-token-class';
  if (type.includes('method')) return 'md-fence-token-method';
  if (type.includes('function')) return 'md-fence-token-function';
  if (type.includes('parameter') || type.includes('argument')) return 'md-fence-token-parameter';
  if (type.includes('property') || type.includes('field')) return 'md-fence-token-property';
  if (type.includes('namespace') || type.includes('module') || type.includes('package')) return 'md-fence-token-namespace';
  if (type.includes('tag')) return 'md-fence-token-tag';
  if (type.includes('attribute')) return 'md-fence-token-attribute';
  if (type.includes('builtin') || type.includes('support')) return 'md-fence-token-builtin';
  if (type.includes('decorator') || type.includes('annotation')) return 'md-fence-token-decorator';
  if (type.includes('variable') || type.includes('identifier')) return 'md-fence-token-variable';
  if (type.includes('constant')) return 'md-fence-token-constant';
  if (type.includes('operator')) return 'md-fence-token-operator';
  if (type.includes('delimiter') || type.includes('punctuation') || type.includes('bracket')) return 'md-fence-token-delimiter';
  return '';
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

  virtualModel.setValue(embeddedCtx.blockText || '');

  const hostEditor = getEmbeddedHostEditor();
  if (!hostEditor) return null;

  initLspClient(hostEditor, embeddedCtx.languageId, virtualModel.uri.toString());
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

const isSuggestWidgetVisibleForEditor = (targetEditor) => {
  const dom = targetEditor?.getDomNode?.();
  if (!dom) return false;
  return !!dom.querySelector('.suggest-widget.visible');
};

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
  let blocks = [];
  if (ctx) {
    blocks = [ctx];
  } else {
    const allBlocks = getCachedMarkdownFenceBlocks(model);
    if (visibleRanges.length) {
      blocks = allBlocks.filter((item) => visibleRanges.some((range) => (
        item.blockEndLine >= range.startLineNumber && item.blockStartLine <= range.endLineNumber
      )));
    } else {
      blocks = allBlocks;
    }
  }
  if (!ctx && blocks.length > EMBEDDED_VISIBLE_BLOCK_LIMIT) {
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
  const key = ctx
    ? `${ctx.languageId}:${ctx.blockStartLine}:${ctx.blockEndLine}:${versionId}`
    : `${versionId}:${rangeKey}`;
  if (state.lastKey === key && reason === 'cursor') return;
  state.lastKey = key;

  const decorations = [];
  for (const block of blocks) {
    warmupEmbeddedLspForContext(block);
    const blockText = block.blockText ?? buildMarkdownFenceBlockText(model, block.blockStartLine, block.blockEndLine);
    let tokenLines = [];
    if (shouldSkipMarkdownFenceTokenize(block.languageId)) {
      logger.debug('[MarkdownFence] skip extra tokenize for compatibility', {
        languageId: block.languageId,
        reason,
      });
      continue;
    }
    try {
      tokenLines = monaco.editor.tokenize(blockText || '', block.languageId) || [];
    } catch (error) {
      logger.warn('[MarkdownFence] tokenize failed', { language: block.languageId }, error);
      continue;
    }

    for (let i = 0; i < tokenLines.length; i += 1) {
      const tokens = tokenLines[i] || [];
      if (!tokens.length) continue;
      const lineNumber = block.blockStartLine + i;
      const lineContent = model.getLineContent(lineNumber);
      const lineLen = lineContent.length;

      decorations.push({
        range: new monaco.Range(lineNumber, 1, lineNumber, Math.max(1, lineLen + 1)),
        options: { inlineClassName: 'md-fence-token-base' }
      });

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

    if (state.requestSeq !== requestSeq) return;

    const shouldApplySemantic = reason !== 'cursor' && reason !== 'scroll';
    const semanticBundle = shouldApplySemantic ? await fetchEmbeddedSemanticTokens(block) : null;
    if (state.requestSeq !== requestSeq) return;
    if (semanticBundle) {
      appendEmbeddedSemanticDecorations(
        decorations,
        block,
        model,
        semanticBundle.semanticTokens,
        semanticBundle.legend
      );
    }
  }

  if (state.requestSeq !== requestSeq) return;
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

const buildCompletionDedupKey = (item) => {
  const label = getCompletionItemLabelText(item?.label).trim().toLowerCase()
  const insertText = String(item?.insertText || item?.textEdit?.text || '').trim().toLowerCase()
  const kind = String(item?.kind || '').trim()
  const detail = String(item?.detail || '').trim().toLowerCase()
  const filterText = String(item?.filterText || '').trim().toLowerCase()
  return [label, insertText, kind, detail, filterText].join('::')
}

const dedupeCompletionSuggestions = (items) => {
  const source = Array.isArray(items) ? items : []
  if (!source.length) return []

  const seen = new Set()
  const suggestions = []
  for (const item of source) {
    const key = buildCompletionDedupKey(item)
    if (!key || seen.has(key)) continue
    seen.add(key)
    suggestions.push(item)
  }
  return suggestions
}

const convertVscodeCompletionItemToMonaco = (item, replaceRange, index) => {
  const label = normalizeCompletionLabel(item?.label)
  if (!label) return null

  const insertTextValue = item?.insertText
  const insertText = typeof insertTextValue === 'string'
    ? insertTextValue
    : String(insertTextValue?.value || label)
  const isSnippet = typeof insertTextValue === 'object' && insertTextValue !== null
  const documentation = normalizeCompletionDocumentation(item?.documentation)
  const kind = VSCODE_TO_MONACO_COMPLETION_KIND[item?.kind] || monaco.languages.CompletionItemKind.Text

  return {
    label,
    kind,
    detail: item?.detail,
    documentation,
    filterText: item?.filterText,
    sortText: buildCompletionSortKey(item, label, index),
    insertText,
    insertTextRules: isSnippet
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

const provideEmbeddedVscodeCompletionSuggestions = async (virtualModel, positionInBlock, replaceRange, context) => {
  try {
    const triggerCharacter = context?.triggerKind === monaco.languages.CompletionTriggerKind.TriggerCharacter
      ? String(context?.triggerCharacter || '')
      : undefined

    const result = await vscodeCommands.executeCommand(
      'vscode.executeCompletionItemProvider',
      VscodeUri.parse(virtualModel.uri.toString()),
      new VscodePosition(positionInBlock.lineNumber - 1, positionInBlock.column - 1),
      triggerCharacter || undefined,
      0
    )

    const items = Array.isArray(result?.items) ? result.items : []
    if (!items.length) return []

    const suggestions = []
    for (let i = 0; i < items.length; i += 1) {
      const mapped = convertVscodeCompletionItemToMonaco(items[i], replaceRange, i)
      if (!mapped) continue
      suggestions.push(mapped)
    }
    return dedupeCompletionSuggestions(suggestions)
  } catch (error) {
    logger.debug('[EmbeddedLsp] vscode completion command failed, fallback to Monaco providers', error)
    return []
  }
}

const buildEmbeddedReplaceRange = (model, position) => {
  const wordInfo = model.getWordUntilPosition(position);
  return new monaco.Range(
    position.lineNumber,
    wordInfo?.startColumn || position.column,
    position.lineNumber,
    wordInfo?.endColumn || position.column
  );
}

const provideEmbeddedLspSuggestions = async (embeddedCtx, model, position, context) => {
  if (!embeddedCtx) return [];
  const virtualModel = prepareEmbeddedVirtualModel(embeddedCtx);
  if (!virtualModel) return [];
  const ready = await waitForLspConnection(embeddedCtx.languageId, EMBEDDED_LSP_READY_TIMEOUT_MS);
  if (!ready) return [];

  const replaceRange = buildEmbeddedReplaceRange(model, position);

  const vscodeSuggestions = await provideEmbeddedVscodeCompletionSuggestions(
    virtualModel,
    embeddedCtx.positionInBlock,
    replaceRange,
    context
  )
  if (vscodeSuggestions.length) {
    return vscodeSuggestions
  }

  const providers = (monaco.languages).CompletionProviderRegistry?.all?.(virtualModel) || [];
  if (!providers.length) return [];

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
      for (const item of items) {
        const insertText = String(item?.insertText || item?.textEdit?.text || '');
        if (!getCompletionItemLabelText(item?.label)) continue;

        suggestions.push({
          ...item,
          insertText,
          range: replaceRange,
          textEdit: undefined,
          additionalTextEdits: undefined,
        });
      }
    } catch {
    }
  }

  return dedupeCompletionSuggestions(suggestions);
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
  const ext = String(path || '').split('.').pop()?.toLowerCase() || 'markdown'
  return EXT_TO_LANGUAGE[ext] || 'markdown'
}

const resolveFenceLanguageId = (infoRaw) => {
  const info = String(infoRaw || '').trim().toLowerCase()
  if (!info) return ''
  const langToken = info.split(/\s+/)[0] || ''
  const mapped = MARKDOWN_FENCE_LANG_MAP[langToken]
  if (mapped) return mapped
  const languages = monaco.languages?.getLanguages?.() || []
  const exists = languages.some((item) => item?.id === langToken)
  return exists ? langToken : ''
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

const enableLspForEditorLanguage = (targetEditor, languageId, filePath = '') => {
  if (!targetEditor) return
  const lang = String(languageId || '').trim().toLowerCase()
  if (!lang) return

  initTextmate(targetEditor, lang).catch((e) => {
    logger.warn('[Editor] initTextmate failed, ignored', { language: lang }, e)
  })

  if (!lspEnabled.value) return
  if (!LSP_SUPPORTED_LANGUAGES.has(lang)) return
  initLspClient(targetEditor, lang, filePath || '')
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
      operator: '#383a42',
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
      property: '#4078f2',
      parameter: '#986801',
      variable: '#e45649',
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
      { token: 'parameter', foreground: '61afef' },           // Parameter → Function color
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
      { token: 'parameter.declaration', foreground: '61afef' },
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
      operator: '#abb2bf',
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
      property: '#61afef',
      parameter: '#61afef',
      'function.declaration': '#61afef',
      'function.defaultLibrary': '#61afef',
      'parameter.declaration': '#61afef',
      variable: '#e06c75',
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
      'scrollbar.shadow': '#00000066'
    }
  });

  const themeName = isDark ? 'guaxi-dark' : 'guaxi-light';

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

  void refreshLspEnabledSetting().then(() => {
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '')
  })
  refreshMarkdownSuggestMode(editor)
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

  enableLspForEditorLanguage(secondaryEditor, language, rightPath || '');
  scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'init');
  scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'init');
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
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'cursor');
    scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'cursor');
  });

  secondaryEditor.onDidScrollChange(() => {
    editorEffectsRight?.updateCursorTarget();
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'scroll');
    scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'scroll');
  });

  secondaryEditor.onDidType((text) => {
    editorEffectsRight?.spawnExplosionAtCursor(text);
    editorEffectsRight?.updateCursorTarget();
    maybeTriggerEmbeddedSuggestOnType(secondaryEditor, text)
  });

  secondaryEditor.onDidChangeModelLanguage(() => {
    const languageId = secondaryEditor?.getModel?.()?.getLanguageId?.() || 'plaintext';
    enableLspForEditorLanguage(secondaryEditor, languageId, rightPath || '');
    refreshMarkdownSuggestMode(secondaryEditor)
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'model');
    scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'model');
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

    model.onDidChangeContent(
      debounce(() => {
        if (fileTreeStore.activeRegion !== 'right') return;
        const path = fileTreeStore.activeTab;
        if (!path) return;
        const content = model.getValue();
        rawContent.value = content;
        fileTreeStore.updateTabContent(path, content);
        updateGitDecorations();
        if (model.getLanguageId() === 'markdown') {
          refreshMarkdownSuggestMode(secondaryEditor)
          scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'debounced');
          scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'debounced');
        }
        scheduleRefreshPythonCellManagers()
      }, 120)
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

  scheduleMarkdownFenceSyntaxHighlight(editor, 'init');
  scheduleMarkdownMathSyntaxHighlight(editor, 'init');
  refreshMarkdownSuggestMode(editor)
  scheduleRefreshPythonCellManagers()

  editor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    updateTableToolbar();
    refreshMarkdownSuggestMode(editor)
    scheduleMarkdownFenceSyntaxHighlight(editor, 'cursor');
    scheduleMarkdownMathSyntaxHighlight(editor, 'cursor');
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

    model.onDidChangeContent(
      debounce(() => {
        const content = model.getValue();
        const languageId = model.getLanguageId() || 'plaintext';
        const problems = analyzeContentForProblems(content, model, languageId);
        editorStore.setProblems(problems);
        rawContent.value = content;
        const path = fileTreeStore.activeTab;
        if (path) fileTreeStore.updateTabContent(path, content);
        recordEditLocationFromEditor();
        updateTableToolbar();
        scheduleAutoFormatActiveTable();
        refreshMarkdownSuggestMode(editor)
        scheduleMarkdownFenceSyntaxHighlight(editor, 'debounced');
        scheduleMarkdownMathSyntaxHighlight(editor, 'debounced');
        updateGitDecorations();
        scheduleRefreshPythonCellManagers()
      }, 300)
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
    scheduleMarkdownFenceSyntaxHighlight(editor, 'scroll');
    scheduleMarkdownMathSyntaxHighlight(editor, 'scroll');
    syncRealtimePreviewScroll();
  });

  editor.onDidChangeModelLanguage(() => {
    const language = editor.getModel()?.getLanguageId() || 'plaintext';
    editorStore.setLanguage(language);
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '');
    updateTableToolbar();
    refreshMarkdownSuggestMode(editor)
    scheduleMarkdownFenceSyntaxHighlight(editor, 'model');
    scheduleMarkdownMathSyntaxHighlight(editor, 'model');
    scheduleRefreshPythonCellManagers()
  });

  const isSuggestWidgetVisible = () => {
    const dom = editor?.getDomNode?.();
    if (!dom) return false;
    return !!dom.querySelector('.suggest-widget.visible');
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
  const tokens = md.parse(content, {});

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

  return formatted;
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
  const pipeCount = (trimmed.match(/\|/g) || []).length;
  return pipeCount >= 2;
};

const isTableSeparatorLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  return /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(trimmed);
};

const normalizeTableRowCells = (line) => {
  const raw = (line || '').trim();
  let text = raw;
  if (!text.startsWith('|')) text = `| ${text}`;
  if (!text.endsWith('|')) text = `${text} |`;
  const parts = text.split('|');
  return parts.slice(1, -1).map((c) => c.trim());
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
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  )) return true;

  return false;
};

// 计算字符串的“可视宽度”（用于 Markdown 表格源码对齐）。
// 规则：控制字符忽略；组合字符按 0；全宽字符按 2；其它按 1。
// 说明：这是为了提升中英文混排时 | 的对齐观感，而不是严格的排版引擎实现。
const getDisplayWidth = (input) => {
  const s = String(input ?? '');
  let width = 0;

  for (const ch of s) {
    const cp = ch.codePointAt(0) || 0;

    if (cp === 0) continue;
    if (cp < 32 || cp === 0x7f) continue;
    if (isCombiningCodePoint(cp)) continue;

    width += isFullWidthCodePoint(cp) ? 2 : 1;
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

  const widths = Array.from({ length: colCount }, () => 3);
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
        const w = Math.max(3, widths[i]);
        const dashes = '-'.repeat(w);
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

const insertImage = () => {
  insertBlockSnippet('![图片描述](路径或URL)\n');
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
    model.setValue(nextText)
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
    if (model && model.getValue() !== nextText) {
      model.setValue(nextText)
    } else if (!model && typeof targetEditor.setValue === 'function') {
      targetEditor.setValue(nextText)
    }
    targetEditor.layout?.()
    logger.debug('[Editor] syncSourceEditorContent', {
      reason,
      length: nextText.length,
      hasModel: !!model,
      editorSurfaceMode: editorSurfaceMode.value,
    })
  } catch (error) {
    logger.warn('[Editor] syncSourceEditorContent failed', { reason, error })
  }
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

  const model = targetEditor.getModel();
  const currentValue = model ? model.getValue() : targetEditor.getValue();
  if (currentValue === nextContent) return;

  if (model) {
    model.setValue(nextContent);
  } else {
    targetEditor.setValue(nextContent);
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

const openPythonRuntimePackageActionLog = (title, res) => {
  pythonRuntimePackageActionLog.value = {
    visible: true,
    title: String(title || 'Python 包管理输出'),
    stdout: String(res?.stdout || ''),
    stderr: String(res?.stderr || ''),
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
    if (outputEl) {
      outputEl.textContent = String(snapshot.outputText || '')
    }
    if (outputPre) {
      outputPre.style.display = String(snapshot.outputText || '').trim() ? '' : 'none'
    }

    if (snapshot.result) {
      renderPyCellDisplay(cell, outputPre, snapshot.result, String(snapshot.outputText || ''))
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
          <div class="guaxi-pycell-session-title">Python 会话</div>
          <div class="guaxi-pycell-session-meta">${escapeHtml(sessionId ? `共享会话 · ${vars.length} 个变量` : `${vars.length} 个变量`)}</div>
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

    actions.appendChild(createButton('导入文件', onImport))
    actions.appendChild(createButton('导出 CSV', () => onExport?.('csv')))
    actions.appendChild(createButton('导出 JSON', () => onExport?.('json')))
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

  const renderPyCellDisplay = (cell, outputPre, res, outputText) => {
    const display = getGuaxiDisplayFromResult(res)
    const dataframe = getGuaxiDataFrameFromResult(res)
    const shouldHideText = String(outputText || '').trim().length === 0

    if (!display && !dataframe) {
      const media = cell?.querySelector?.('.guaxi-pycell-media')
      if (media) {
        media.innerHTML = ''
        media.style.display = 'none'
      }
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    const media = ensurePyCellMediaContainer(cell, outputPre)
    if (!media) return

    if (dataframe) {
      const renderDataFrame = (table) => {
        media.innerHTML = ''
        media.appendChild(createDataFramePreviewCard(table, {
          onImport: async () => {
            try {
              const selected = await open({
                multiple: false,
                filters: [{
                  name: 'Table',
                  extensions: ['csv', 'tsv', 'json', 'xlsx', 'xls', 'parquet'],
                }],
              })
              if (!selected || Array.isArray(selected)) return
              const imported = await invoke('python_runtime_import_table_file', { request: { path: String(selected) } })
              if (!imported?.ok || !imported?.table) {
                throw new Error(String(imported?.error || '导入失败'))
              }
              const nextTable = {
                columns: Array.isArray(imported.table.columns) ? imported.table.columns.map((item) => String(item ?? '')) : [],
                rows: Array.isArray(imported.table.rows) ? imported.table.rows : [],
                rowCount: Number(imported.table.row_count || 0),
                truncated: !!imported.table.truncated,
                dtypes: imported.table.dtypes || {},
                sourcePath: String(imported.table.source_path || '').trim(),
              }
              renderDataFrame(nextTable)
              message.success('表格已导入到预览区')
            } catch (error) {
              logger.error('[PyCell] dataframe import failed', error)
              message.error(`导入表格失败：${String(error?.message || error)}`)
            }
          },
          onExport: async (format) => {
            await saveDataFrameToFile(table, format)
          },
        }))
        media.style.display = 'flex'
      }

      renderDataFrame(dataframe)
      if (outputPre && shouldHideText) {
        outputPre.style.display = 'none'
      }
      return
    }

    media.innerHTML = ''

    if (display?.type === 'html' && display.html) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-html-display'
      box.innerHTML = display.html
      media.appendChild(box)
      media.style.display = 'flex'
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    if (display?.type === 'svg' && display.svg) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-svg-display'
      box.innerHTML = display.svg
      media.appendChild(box)
      media.style.display = 'flex'
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    if (display?.type === 'markdown' && display.markdown) {
      const box = document.createElement('pre')
      box.className = 'guaxi-pycell-rich-display guaxi-pycell-markdown-display'
      box.textContent = display.markdown
      media.appendChild(box)
      media.style.display = 'flex'
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    const img = document.createElement('img')
    img.style.maxWidth = '100%'
    img.style.height = 'auto'
    img.style.display = 'block'
    img.style.marginLeft = 'auto'
    img.style.marginRight = 'auto'

    let src = ''
    if (display.dataUrl && display.dataUrl.startsWith('data:')) {
      src = display.dataUrl
    } else if (display.dataBase64) {
      src = `data:${display.mime};base64,${display.dataBase64}`
    } else if (display.data != null) {
      src = `data:${display.mime},${encodeURIComponent(display.data)}`
    }

    if (!src) {
      media.innerHTML = ''
      media.style.display = 'none'
      return
    }

    img.src = src
    media.innerHTML = ''
    media.appendChild(img)
    media.style.display = 'flex'

    if (outputPre && shouldHideText) {
      outputPre.style.display = 'none'
    }
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

    const syncPyCellButtons = () => {
      const media = cell.querySelector?.('.guaxi-pycell-media')
      const panel = cell.querySelector?.('.guaxi-pycell-session')
      const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
      const mediaVisible = media ? String(media.style.display || '').trim() !== 'none' : false
      const panelVisible = panel ? String(panel.style.display || '').trim() !== 'none' : false
      if (toggleBtn) toggleBtn.textContent = textVisible || mediaVisible ? '隐藏结果' : '显示结果'
      if (sessionBtn) sessionBtn.textContent = panelVisible ? '隐藏变量' : '会话变量'
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
      try {
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
        const outputText = formatPythonExecOutput(res)
        if (outputEl) outputEl.textContent = outputText
        if (outputPre) outputPre.style.display = String(outputText || '').trim().length > 0 ? '' : 'none'
        renderPyCellDisplay(cell, outputPre, res, outputText)
        renderPyCellSessionState(cell, res, { forceVisible: true })
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText,
          display: getGuaxiDisplayFromResult(res),
          dataframe: getGuaxiDataFrameFromResult(res),
          result: res,
          sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
          meta: {
            ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
            outputCollapsed: String(outputText || '').trim().length === 0,
            sessionCollapsed: false,
          },
        })
        scheduleRefreshPythonCellManagers()
        if (res?.ok === false) throw new Error(String(res?.stderr || res?.stdout || '导入失败'))
        message.success(`表格已导入到会话变量 ${variableName}`)
      } catch (err) {
        logger.error('[PyCell] import table failed', err)
        message.error(`导入表格失败：${String(err?.message || err)}`)
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

      const media = cell.querySelector?.('.guaxi-pycell-media')
      if (media) {
        media.innerHTML = ''
        media.style.display = 'none'
      }

      if (outputPre) outputPre.style.display = ''
      if (outputEl) outputEl.textContent = '运行中...'
      syncPyCellButtons()

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-py-action]') || [])
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
        const hideText = String(outputText || '').trim().length === 0
        if (outputPre) outputPre.style.display = hideText ? 'none' : ''
        if (outputEl) outputEl.textContent = hideText ? '' : outputText

        renderPyCellDisplay(cell, outputPre, res, outputText)
        renderPyCellSessionState(cell, res)
        syncPyCellButtons()
        patchSharedPythonCellState(cellId, {
          outputText: hideText ? '' : outputText,
          display: getGuaxiDisplayFromResult(res),
          dataframe: getGuaxiDataFrameFromResult(res),
          result: res,
          sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
          meta: {
            ...(getPythonCellSnapshot(getActivePythonCellFilePath(), cellId)?.meta || {}),
            outputCollapsed: hideText,
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

  // 可执行 Python 代码块：预览点击“运行/运行并写回/显示结果”
  const pyBtn = base.closest?.('[data-guaxi-py-action]')
  if (pyBtn && previewContainer.value && previewContainer.value.contains(pyBtn)) {
    const action = String(pyBtn.getAttribute('data-guaxi-py-action') || '').trim()

    if (action === 'toggle-output') {
      const cell = pyBtn.closest?.('.guaxi-pycell')
      if (!cell) return

      const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const media = cell.querySelector?.('.guaxi-pycell-media')

      if (!outputPre && !media) {
        message.warning('未找到运行结果区域')
        e.stopPropagation()
        return
      }

      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const hasImage = !!media?.querySelector?.('img')

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
        media.style.display = nextShow && hasImage ? 'flex' : 'none'
      }

      const anyVisibleAfter = isVisible(outputPre) || isVisible(media)
      pyBtn.textContent = anyVisibleAfter ? '隐藏结果' : '显示结果'

      logger.info('[PyCell] toggle output', {
        nextShow,
        hasText,
        hasImage,
        textVisible: isVisible(outputPre),
        imageVisible: isVisible(media),
      })

      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'run' || action === 'run-save') {
      const cell = pyBtn.closest?.('.guaxi-pycell')
      if (!cell) return

      const code = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-code'))
      const cellId = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-cell'))
      const startLine = Number(cell.getAttribute('data-guaxi-md-start-line'))

      if (!code) {
        message.warning('Python 代码为空')
        e.stopPropagation()
        return
      }

      const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const toggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-output"]')
      const syncToggleText = () => {
        if (!toggleBtn) return
        const media = cell.querySelector?.('.guaxi-pycell-media')
        const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
        const imageVisible = media ? String(media.style.display || '').trim() !== 'none' : false
        toggleBtn.textContent = textVisible || imageVisible ? '隐藏结果' : '显示结果'
      }

      // 说明：输出容器默认隐藏（避免未运行时占位）；运行时再展示。
      // 运行前清理上次的展示（例如 __guaxi_display__ 生成的图片），避免旧内容闪现。
      const media = cell.querySelector?.('.guaxi-pycell-media')
      if (media) {
        media.innerHTML = ''
        media.style.display = 'none'
      }

      if (outputPre) {
        outputPre.style.display = ''
      }

      syncToggleText()
      if (outputEl) outputEl.textContent = '运行中...'

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-py-action]') || [])
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
        const sessionId = docPath ? String(docPath) : 'guaxi:pycell:global'

        logger.info('[PyCell] python_exec', {
          action,
          cellId,
          codeLen: code.length,
          docPath,
          sessionId,
        })

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

        logger.info('[PyCell] python_exec result', {
          ok: res?.ok,
          stdoutLen: String(res?.stdout || '').length,
          stderrLen: String(res?.stderr || '').length,
          hasResultRepr: !!res?.result_repr,
          hasResultJson: res?.result_json != null,
        })

        const outputText = formatPythonExecOutput(res)

        const hideText = String(outputText || '').trim().length === 0

        if (outputPre) {
          outputPre.style.display = hideText ? 'none' : ''
        }
        syncToggleText()

        if (outputEl) {
          outputEl.textContent = hideText ? '' : outputText
        }

        renderPyCellDisplay(cell, outputPre, res, outputText)

        if (res?.ok === false) {
          message.error('Python 执行失败')
        }

        if (action === 'run-save') {
          const hasTextOutput = String(outputText || '').trim().length > 0
          if (!hasTextOutput) {
            logger.info('[PyCell] run-save skipped: empty text output', { cellId })
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
const renderDebounced = debounce(async (content) => {
  if (!selectedFile.value || !mode.value || !content || !canPreview.value) {
    renderedHtml.value = '';
    lastPreviewRenderedSource.value = '';
    return;
  }

  try {
    const basePath = selectedFile.value.path ? await dirname(selectedFile.value.path) : '';
    const previewTheme = getMarkdownPreviewThemeName()
    logger.info('[Preview] render markdown', {
      previewTheme,
      appearanceTheme: settingsStore.appearanceTheme,
      path: selectedFile.value?.path || '',
    })
    const previewSource = transformPythonCellBlocksForPreview(content, {
      filePath: selectedFile.value?.path || '',
    })
    const processed = preprocessObsidianWikilinks(previewSource);
    renderedHtml.value = await renderMarkdown(processed, {
      basePath: basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      pythonCells: true,
      codeCells: true,
      theme: previewTheme
    });

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
}, 300);

const canPreview = computed(() => {
  const path = selectedFile.value?.path || '';
  const ext = path.split('.').pop()?.toLowerCase();
  if (ext === 'md' || ext === 'markdown') return true;
  return editorStore.language === 'markdown';
});

const lastSelectedFilePath = ref(null)

// 监听文件选择
watch(selectedFile, async (newFile) => {
  flushPythonCellDrafts()
  if (newFile) {
    const nextPath = newFile.path || '';
    const prevPath = lastSelectedFilePath.value || '';

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
      const model = targetEditor.getModel();
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
      renderDebounced(newFile.content)
    }

    await initGitService()
  } else {
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
  , { label: 'Explorer Mode (Ctrl+5)', key: 'explorer', icon: () => h(Icon, { icon: scanSearchIcon, width: 16, height: 16 }) }
];

const previewModeLabel = computed(() => {
  if (isExplorerSurfaceActive.value) {
    return 'Explorer Mode';
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

  logger.debug('[Editor] switchTab', {
    from: targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft,
    to: path,
    region: targetRegion,
    hasEditor: !!editor,
    hasSecondary: !!secondaryEditor,
  })

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

  fileTreeStore.setActiveTab(path, targetRegion);
  const tab = fileTreeStore.openTabs.find((t) => t.path === path);
  if (!tab) return;

  rawContent.value = tab.content;

  if (previewMode.value === 'realtime' && realtimeVditor) {
    initRealtimeVditor();
  }

  if (fileTreeStore.layoutMode !== 'single' && targetRegion === 'right') {
    ensureSecondaryEditor();
  }

  updateEditorContent(tab.content);
  nextTick(() => {
    syncSourceEditorContent('switch-tab')
  })
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
      let first = matches[0];
      let start = Math.max(0, Math.min(first.start, lineLength));
      editor.revealPositionInCenter({
        lineNumber,
        column: start + 1
      });
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
      editor.revealRangeInCenter(foundMatches[0].range);
    }
  }
};

// 监听编辑器store的变化
watch(() => editorStore.currentFile, (file) => {
  if (file && editor) {
    logger.info("WWW>>> editorStore.currentFile", file)
    editor.setValue(file.content);
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
function generateOutline(content) {
  const file = activeTab.value;
  const ext = file?.path?.split('.').pop()?.toLowerCase();
  if (ext !== 'md' && ext !== 'markdown') {
    outlineStore.setOutline([]);
    return;
  }
  const tokens = md.parse(content, {})
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
  outlineStore.setOutline(result)
}

const generateOutlineDebounced = debounce((content) => {
  generateOutline(content)
}, 200)

const syncNotebookOutlineState = async (reason = 'unknown') => {
  if (!(editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value)) {
    return false
  }

  await nextTick()
  const outline = notebookViewRef.value?.getOutline?.() || []
  outlineStore.setOutline(Array.isArray(outline) ? outline : [])
  outlineStore.setJumpToLine(jumpToLine)

  logger.info('[Outline] synced notebook outline state', {
    reason,
    count: Array.isArray(outline) ? outline.length : 0,
    hasNotebookView: !!notebookViewRef.value,
  })
  return true
}

const handleNotebookOutlineChange = (outline) => {
  if (!(editorSurfaceMode.value === 'notebook' && canUseNotebookSurface.value)) return
  const normalizedOutline = Array.isArray(outline) ? outline : []
  outlineStore.setOutline(normalizedOutline)
  outlineStore.setJumpToLine(jumpToLine)
  logger.info('[Outline] received notebook outline change event', {
    count: normalizedOutline.length,
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
  generateOutlineDebounced(content)
}, { immediate: true })

const handleLspSettingChanged = (evt) => {
  const enabled = !!evt?.detail?.enabled
  lspEnabled.value = enabled
  applyMonacoThemeByLsp(enabled)
  scheduleMarkdownFenceSyntaxHighlight(editor, 'setting')
  scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'setting')
  scheduleMarkdownMathSyntaxHighlight(editor, 'setting')
  scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'setting')

  if (!enabled) {
    disposeAllLspClients()
    return
  }

  resetLspDisabled()

  const mainModel = editor?.getModel?.()
  if (mainModel) {
    enableLspForEditorLanguage(editor, mainModel.getLanguageId?.() || 'plaintext', selectedFile.value?.path || '')
  }

  const rightPath = fileTreeStore.activeTabRight || ''
  const rightModel = secondaryEditor?.getModel?.()
  if (rightModel) {
    enableLspForEditorLanguage(secondaryEditor, rightModel.getLanguageId?.() || 'plaintext', rightPath)
  }
}

watch(lspEnabled, (enabled) => {
  applyMonacoThemeByLsp(enabled)
  scheduleMarkdownFenceSyntaxHighlight(editor, 'setting')
  scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'setting')
  scheduleMarkdownMathSyntaxHighlight(editor, 'setting')
  scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'setting')
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
    scheduleMarkdownFenceSyntaxHighlight(editor, 'lsp-state')
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'lsp-state')
    scheduleMarkdownMathSyntaxHighlight(editor, 'lsp-state')
    scheduleMarkdownMathSyntaxHighlight(secondaryEditor, 'lsp-state')
    if (state !== 'connected') return
    triggerEmbeddedSuggestIfActive(editor, normalizedLanguageId)
    triggerEmbeddedSuggestIfActive(secondaryEditor, normalizedLanguageId)
  })
});

onActivated(() => {
  restoreHomePreviewMode('activated')
})

onUnmounted(() => {
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
      m.dispose?.()
    } catch {
    }
  })
  markdownFenceCompletionModels.clear()
  disposeFeatureDisposables(markdownLanguageFeatureDisposables)
  disposeFeatureDisposables(filesystemPathCompletionDisposables)
  markdownFeaturesRegistered = false
  filesystemPathCompletionRegistered = false
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
}

:global(:root[data-guaxi-theme='dark']) .guaxi-editor {
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
  border-color: rgba(71, 85, 105, 0.72);
  background:
    radial-gradient(circle at top left, rgba(30, 41, 59, 0.98), rgba(15, 23, 42, 0.96)),
    linear-gradient(180deg, rgba(17, 24, 39, 0.96), rgba(15, 23, 42, 0.98));
  box-shadow:
    0 22px 48px rgba(2, 6, 23, 0.42),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.python-runtime-panel.is-dark .python-runtime-meta__item,
.python-runtime-panel.is-dark .python-runtime-package-pill,
.python-runtime-panel.is-dark .python-runtime-guide,
.python-runtime-panel.is-dark .python-runtime-archive-card,
.python-runtime-panel.is-dark .python-runtime-status-card,
.python-runtime-panel.is-dark .python-runtime-install-card,
.python-runtime-panel.is-dark .python-runtime-package-browser,
.python-runtime-panel.is-dark .python-runtime-package-row {
  border-color: rgba(71, 85, 105, 0.72);
  background: rgba(15, 23, 42, 0.68);
}

.python-runtime-panel.is-dark .python-runtime-ghost-btn,
.python-runtime-panel.is-dark .python-runtime-chip-btn,
.python-runtime-panel.is-dark .python-runtime-install__btn,
.python-runtime-panel.is-dark .python-runtime-install__input,
.python-runtime-panel.is-dark .python-runtime-package-browser__search {
  border-color: rgba(71, 85, 105, 0.72);
  background: rgba(15, 23, 42, 0.72);
  color: #e6edf3;
}

.python-runtime-panel.is-dark .python-runtime-panel__title,
.python-runtime-panel.is-dark .python-runtime-status-card__value,
.python-runtime-panel.is-dark .python-runtime-section__title,
.python-runtime-panel.is-dark .python-runtime-package-row {
  color: #e6edf3;
}

.python-runtime-panel.is-dark .python-runtime-panel__subtitle,
.python-runtime-panel.is-dark .python-runtime-status-card__label,
.python-runtime-panel.is-dark .python-runtime-package-browser__empty {
  color: #94a3b8;
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-success {
  border-color: rgba(34, 197, 94, 0.3);
  background: linear-gradient(180deg, rgba(20, 83, 45, 0.48), rgba(20, 83, 45, 0.28));
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-warning {
  border-color: rgba(245, 158, 11, 0.3);
  background: linear-gradient(180deg, rgba(120, 53, 15, 0.48), rgba(120, 53, 15, 0.28));
}

.python-runtime-panel.is-dark .python-runtime-status-card.is-danger {
  border-color: rgba(244, 63, 94, 0.3);
  background: linear-gradient(180deg, rgba(127, 29, 29, 0.5), rgba(127, 29, 29, 0.28));
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
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.45;
  word-break: break-word;
  flex-wrap: wrap;
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
  gap: 8px;
  flex-wrap: wrap;
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
  background: rgba(15, 23, 42, 0.92);
  color: #e2e8f0;
  font-size: 12px;
  line-height: 1.5;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace);
  white-space: pre-wrap;
  word-break: break-word;
}

.python-runtime-package-log__output.is-error {
  color: #fecaca;
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

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element) {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.28);
}

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(255, 255, 255, 0.45);
}

.guaxi-editor :deep(.md-fence-token-base) { color: #abb2bf; }
.guaxi-editor :deep(.md-fence-token-keyword) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-string) { color: #98c379; }
.guaxi-editor :deep(.md-fence-token-number) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-boolean) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
.guaxi-editor :deep(.md-fence-token-type) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
.guaxi-editor :deep(.md-fence-token-method) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-property) { color: #98c379; }
.guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-macro) { color: #61afef; }
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
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-boolean) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-type) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-method) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-property) { color: #98c379; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-macro) { color: #61afef; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-tag) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-attribute) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-builtin) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-decorator) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-operator) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-delimiter) { color: #abb2bf; }

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
  background-color: var(--toolbar-bg, #f8f8f8);
  border-top: 1px solid var(--divider-color, #e0e0e0);
  padding: 2px 8px;
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  z-index: 1;
}

.editor-status-bar {
  height: 22px;
  background-color: var(--statusBar-background);
  border-top: 1px solid var(--statusBar-border);
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

.preview :deep(a),
.preview :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .preview :deep(a),
.dark-mode .preview :deep(a:visited),
.dark .preview :deep(a),
.dark .preview :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.preview :deep(a:hover) {
  opacity: 0.92;
}

/* ==================== Markdown 任务列表（Task List）样式 ==================== */
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
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 10px;
  padding: 10px 12px;
  margin: 12px 0;
  background: rgba(175, 184, 193, 0.12);
}

.preview :deep(.md-toc .md-toc-list) {
  list-style: none;
  padding-left: 0;
  margin: 0;
}

.preview :deep(.md-toc .md-toc-list .md-toc-list) {
  padding-left: 16px;
  margin-top: 6px;
}

.preview :deep(.md-toc-item) {
  margin: 6px 0;
  line-height: 1.35;
}

.preview :deep(a.md-toc-link) {
  color: var(--guaxi-text-link, var(--primary-color, #4acadb));
  text-decoration: none;
  cursor: pointer;
}

.preview :deep(a.md-toc-link:hover) {
  text-decoration: underline;
}

.dark-mode .preview :deep(.md-toc),
.dark .preview :deep(.md-toc) {
  border-color: #30363d;
  background: rgba(110, 118, 129, 0.12);
}

.dark-mode .preview :deep(a.md-toc-link),
.dark .preview :deep(a.md-toc-link) {
  color: var(--guaxi-text-link-dark, #4acadb);
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
.dark-mode .python-runtime-package-row,
.dark .python-runtime-package-row,
:global(:root[data-guaxi-theme='dark']) .python-runtime-meta__item,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-pill,
:global(:root[data-guaxi-theme='dark']) .python-runtime-guide,
:global(:root[data-guaxi-theme='dark']) .python-runtime-archive-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-install-card,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-browser,
:global(:root[data-guaxi-theme='dark']) .python-runtime-package-row {
  border-color: rgba(71, 85, 105, 0.72);
  background: rgba(15, 23, 42, 0.68);
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
  border-color: rgba(71, 85, 105, 0.72);
  background: rgba(15, 23, 42, 0.72);
  color: #e6edf3;
}

.dark-mode .python-runtime-guide__path-value,
.dark .python-runtime-guide__path-value,
.dark-mode .python-runtime-archive-card__path,
.dark .python-runtime-archive-card__path,
:global(:root[data-guaxi-theme='dark']) .python-runtime-guide__path-value,
:global(:root[data-guaxi-theme='dark']) .python-runtime-archive-card__path {
  background: rgba(2, 6, 23, 0.42);
  color: #e6edf3;
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
  background: rgba(30, 41, 59, 0.96);
}

.dark-mode .python-runtime-panel,
.dark .python-runtime-panel,
:global(:root[data-guaxi-theme='dark']) .python-runtime-panel {
  background:
    radial-gradient(circle at top left, rgba(30, 41, 59, 0.98), rgba(15, 23, 42, 0.96)),
    linear-gradient(180deg, rgba(17, 24, 39, 0.96), rgba(15, 23, 42, 0.98));
  box-shadow:
    0 22px 48px rgba(2, 6, 23, 0.42),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.dark-mode .python-runtime-status-card.is-success,
.dark .python-runtime-status-card.is-success,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-success {
  border-color: rgba(34, 197, 94, 0.3);
  background: linear-gradient(180deg, rgba(20, 83, 45, 0.48), rgba(20, 83, 45, 0.28));
}

.dark-mode .python-runtime-status-card.is-warning,
.dark .python-runtime-status-card.is-warning,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-warning {
  border-color: rgba(245, 158, 11, 0.3);
  background: linear-gradient(180deg, rgba(120, 53, 15, 0.48), rgba(120, 53, 15, 0.28));
}

.dark-mode .python-runtime-status-card.is-danger,
.dark .python-runtime-status-card.is-danger,
:global(:root[data-guaxi-theme='dark']) .python-runtime-status-card.is-danger {
  border-color: rgba(244, 63, 94, 0.3);
  background: linear-gradient(180deg, rgba(127, 29, 29, 0.5), rgba(127, 29, 29, 0.28));
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
  color: #e6edf3;
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
  color: #94a3b8;
}

:global(:root[data-guaxi-theme='dark']) .guaxi-dataframe-btn:hover {
  background: rgba(30, 41, 59, 0.96);
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

.preview pre:not(.hljs)>code {
  display: block;
  border-radius: 6px;
  border: 1px solid #e1e4e8;
  background: #ffffff;
  color: #24292f;
  padding: 1.5em 1em;
  overflow-x: auto;
}

.dark-mode .preview pre:not(.hljs)>code,
:global(:root[data-guaxi-theme='dark']) .preview pre:not(.hljs)>code {
  border-color: #30363d;
  background: #1f242d;
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

.preview :deep(img),
img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
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
}

.python-console-dock.dragging {
  transition: none;
}

.python-console-dock__resize-handle {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 14px;
  cursor: ns-resize;
  user-select: none;
  background: var(--toolbar-bg, #252526);
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
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.08);
}

.python-console-dock :deep(.guaxi-terminal-panel) {
  width: 100%;
  height: 100%;
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

.guaxi-pycell-card__panel.is-collapsed {
  display: none;
}

.guaxi-pycell-card__editor {
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

.guaxi-pycell-card__editor:focus {
  background: var(--pycell-editor-focus-bg);
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
