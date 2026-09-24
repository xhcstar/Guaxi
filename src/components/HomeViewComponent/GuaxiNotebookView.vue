<template>
  <div
    ref="notebookRootRef"
    class="guaxi-notebook"
    :class="[
      themeClass,
      {
        'is-top-toolbar-visible': isNotebookTopToolbarVisible,
        'has-active-python-cell': !!activeNotebookPythonCell,
      },
    ]"
    @mousemove="handleNotebookRootMouseMove"
    @mouseleave="handleNotebookRootMouseLeave"
    @wheel.capture.passive="handleNotebookRootWheel"
    @click="handleNotebookRenderedClick"
    @mouseup="handleNotebookSelectionMouseUp"
  >
    <div
      class="guaxi-notebook__toolbar-shell"
      :class="{ 'is-visible': isNotebookTopToolbarVisible }"
      @mouseenter="revealNotebookTopToolbar('toolbar-shell-enter')"
      @mouseleave="scheduleHideNotebookTopToolbar('toolbar-shell-leave')"
      @focusin="revealNotebookTopToolbar('toolbar-focusin')"
      @focusout="handleNotebookToolbarFocusOut"
    >
      <div class="guaxi-notebook__toolbar">
        <div class="guaxi-notebook__toolbar-main">
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            :disabled="!canUndoNotebookHistory"
            data-tooltip="撤销"
            data-shortcut="Ctrl+Z"
            aria-label="撤销"
            @click="undoNotebookHistory"
          >
            <Icon :icon="undoIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            :disabled="!canRedoNotebookHistory"
            data-tooltip="重做"
            data-shortcut="Ctrl+Y"
            aria-label="重做"
            @click="redoNotebookHistory"
          >
            <Icon :icon="redoIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            data-tooltip="保存当前文档"
            data-shortcut="Ctrl+S"
            aria-label="保存当前文档"
            @click="requestNotebookSave"
          >
            <Icon :icon="saveIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            data-tooltip="搜索探索模式内容"
            data-shortcut="Ctrl+F"
            aria-label="搜索探索模式内容"
            @click="openNotebookSearch"
          >
            <Icon :icon="searchIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            data-tooltip="新增 Python Cell"
            data-shortcut="Alt+P"
            aria-label="新增 Python Cell"
            @click="appendCell('python')"
          >
            <Icon :icon="terminalSquareIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            data-tooltip="新增 Markdown Cell"
            data-shortcut="Alt+M"
            aria-label="新增 Markdown Cell"
            @click="appendCell('markdown')"
          >
            <Icon :icon="filePlusIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            data-tooltip="新增数学块"
            data-shortcut="Alt+K"
            aria-label="新增数学块"
            @click="appendCell('math')"
          >
            <Icon :icon="sigmaIcon" width="16" height="16" />
          </button>
          <template v-if="activeNotebookPythonCell">
            <span class="guaxi-notebook__toolbar-divider" aria-hidden="true"></span>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :disabled="isActiveNotebookPythonSessionBusy"
              :data-tooltip="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，请稍候再导入数据' : '从外部文件导入数据到当前选中的 Python Cell 实验室会话'"
              data-shortcut="Alt+I"
              :aria-label="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，请稍候再导入数据' : '从外部文件导入数据到当前选中的 Python Cell 实验室会话'"
              @click="handleActivePythonToolbarAction('import-table')"
            >
              <Icon :icon="uploadIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn primary"
              :class="{ 'is-busy': isActiveNotebookPythonRunActive }"
              :data-tooltip="isActiveNotebookPythonRunActive ? '中断当前探索者 Python Cell 运行，并清理本次后台会话' : (isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行当前选中的 Python Cell' : '运行当前选中的 Python Cell')"
              data-shortcut="Ctrl+Enter"
              :aria-label="isActiveNotebookPythonRunActive ? '中断当前探索者 Python Cell 运行，并清理本次后台会话' : (isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行当前选中的 Python Cell' : '运行当前选中的 Python Cell')"
              @click="handleActivePythonToolbarAction('run')"
            >
              <Icon :icon="isActiveNotebookPythonRunActive ? squareIcon : playIcon" width="16" height="16" :class="{ 'guaxi-notebook__busy-spin': isActiveNotebookPythonSessionBusy && !isActiveNotebookPythonRunActive }" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :data-tooltip="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行并写回当前选中的 Python Cell' : '运行并将当前选中的 Python Cell 输出写回文档'"
              data-shortcut="Ctrl+Shift+Enter"
              :aria-label="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行并写回当前选中的 Python Cell' : '运行并将当前选中的 Python Cell 输出写回文档'"
              @click="handleActivePythonToolbarAction('run-writeback')"
            >
              <Icon :icon="checkCircleIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :disabled="!hasVisibleOutput(activeNotebookPythonCell)"
              :data-tooltip="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? '显示当前选中 Python Cell 输出' : '隐藏当前选中 Python Cell 输出'"
              data-shortcut="Alt+O"
              :aria-label="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? '显示当前选中 Python Cell 输出' : '隐藏当前选中 Python Cell 输出'"
              @click="handleActivePythonToolbarAction('toggle-output')"
            >
              <Icon :icon="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? eyeIcon : eyeOffIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :data-tooltip="isNotebookPythonSessionPanelVisible ? '隐藏当前选中 Python Cell 数据实验室' : '显示当前选中 Python Cell 数据实验室'"
              data-shortcut="Alt+L"
              :aria-label="isNotebookPythonSessionPanelVisible ? '隐藏当前选中 Python Cell 数据实验室' : '显示当前选中 Python Cell 数据实验室'"
              @click="handleActivePythonToolbarAction('toggle-session')"
            >
              <Icon :icon="flaskConicalIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              data-tooltip="打开 Python 控制台"
              data-shortcut="Alt+T"
              aria-label="打开 Python 控制台"
              @click="handleActivePythonToolbarAction('open-console')"
            >
              <Icon :icon="terminalIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              data-tooltip="打开 Python 官方文档助手"
              aria-label="打开 Python 官方文档助手"
              @click="openNotebookUsageAssistant('python', activeNotebookPythonCell, $event)"
            >
              <Icon :icon="circleHelpIcon" width="16" height="16" />
            </button>
          </template>
        </div>
      </div>
    </div>

    <div v-show="notebookSearchVisible" class="guaxi-notebook__search-panel-shell">
      <GuaxiFindWidget
        ref="notebookFindWidgetRef"
        class="guaxi-notebook__find-widget"
        :visible="notebookSearchVisible"
        :keyword="notebookSearchKeyword"
        :match-index="notebookSearchActiveIndex"
        :match-count="notebookSearchTotalMatchCount"
        :case-sensitive="notebookSearchCaseSensitive"
        :whole-word="notebookSearchWholeWord"
        :use-regex="notebookSearchUseRegex"
        :regex-invalid="notebookSearchRegexInvalid"
        placeholder="在探索者中查找"
        @update:keyword="notebookSearchKeyword = $event"
        @update:case-sensitive="notebookSearchCaseSensitive = $event"
        @update:whole-word="notebookSearchWholeWord = $event"
        @update:use-regex="notebookSearchUseRegex = $event"
        @next="focusNextNotebookSearchMatch"
        @prev="focusPrevNotebookSearchMatch"
        @close="closeNotebookSearch"
      />
    </div>

    <div
      v-show="notebookSelectionToolbarVisible"
      class="guaxi-notebook__selection-toolbar"
      :style="notebookSelectionToolbarStyle"
      @mousedown.prevent
      @click.stop
    >
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        :disabled="selectionTranslateBusy"
        data-tooltip="翻译选中文本"
        aria-label="翻译选中文本"
        @click="openNotebookSelectionTranslatePanel"
      >
        <Icon :icon="languagesIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        data-tooltip="发送到助手"
        aria-label="发送到助手"
        @click="askNotebookSelectionWithAssistant"
      >
        <Icon :icon="messageCircleIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        data-tooltip="用 AI 翻译"
        aria-label="用 AI 翻译"
        @click="sendNotebookSelectionAsAiTranslate"
      >
        <Icon :icon="botIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        :disabled="selectionReadAloudBusy"
        :data-tooltip="selectionReadAloudBusy ? '正在朗读' : '朗读选中文本'"
        :aria-label="selectionReadAloudBusy ? '正在朗读' : '朗读选中文本'"
        @click="readNotebookSelectionAloud"
      >
        <Icon :icon="volume2Icon" width="16" height="16" />
      </button>
    </div>

    <template v-for="(cell, index) in notebookCells" :key="cell.key">
      <div
        class="guaxi-notebook__cell"
        :class="[
          `is-${cell.type}`,
          {
            'is-search-match': isNotebookSearchMatchedCell(cell),
            'is-search-active': isNotebookSearchActiveCell(cell),
            'is-active-python-cell': isActiveNotebookPythonCell(cell),
            'is-variable-focus-cell': isNotebookVariableFocusedCell(cell),
          },
        ]"
        :data-cell-key="cell.key"
        @mousedown="handleNotebookCellPointerDown(cell, $event)"
      >
        <button
          v-if="cell.type === 'markdown'"
          type="button"
          class="guaxi-notebook__cell-delete"
          title="删除当前 Cell"
          aria-label="删除当前 Cell"
          @click.stop="confirmRemoveCell(cell)"
        >
          <Icon :icon="trash2Icon" width="16" height="16" />
        </button>
      <template v-if="cell.type === 'markdown'">
        <div
          class="guaxi-notebook__markdown-surface"
          :class="{ 'is-editing': isMarkdownCellEditing(cell) }"
        >
          <div v-if="isMarkdownCellEditing(cell)" class="guaxi-notebook__markdown-edit-shell">
            <GuaxiNotebookMarkdownEditor
              :ref="(instance) => setMarkdownEditorRef(cell.key, instance)"
              :cell-key="cell.key"
              :document-path="filePath"
              :model-value="cell.content"
              :theme="theme"
              @update:model-value="updateMarkdownCell(cell.key, $event)"
              @focus="activateNotebookCell(cell, 'markdown-editor-focus')"
              @request-exit="exitMarkdownEditMode(cell.key, $event)"
            />
          </div>
          <div
            v-else
            class="guaxi-notebook__markdown-preview-shell is-clickable"
            role="button"
            tabindex="0"
            @dblclick="enterMarkdownEditMode(cell.key, 'preview-dblclick')"
            @keydown.enter.prevent="enterMarkdownEditMode(cell.key, 'preview-enter')"
            @keydown.space.prevent="enterMarkdownEditMode(cell.key, 'preview-space')"
          >
            <div
              v-if="String(cell.content || '').trim()"
              class="guaxi-notebook__markdown-preview markdown-body"
              v-html="getRenderedMarkdownCellHtml(cell)"
            ></div>
            <div v-else class="guaxi-notebook__empty"></div>
          </div>
        </div>
      </template>

      <template v-else-if="cell.type === 'math'">
        <div class="guaxi-notebook__math-layout">
          <div class="guaxi-notebook__math-toolbar">
            <span class="guaxi-notebook__math-badge" :class="{ 'is-busy': isCellBusy(cell) }">
              <!-- 设计说明：数学块与 Python Cell 都会触发后台执行；运行中复用同一旋转动画，让 Notebook 的执行状态反馈保持一致。 -->
              <Icon
                :icon="sigmaIcon"
                width="13"
                height="13"
                class="guaxi-notebook__math-badge-icon"
                :class="{ 'guaxi-notebook__busy-spin': isCellBusy(cell) }"
              />
              <span>{{ isCellBusy(cell) ? '运行中' : 'Math' }}</span>
            </span>
            <div class="guaxi-notebook__math-actions">
              <!-- 主操作：运行数学工作区（逐行计算 / 绘图）。 -->
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                data-tooltip="运行数学工作区（逐行计算 / 绘图）"
                aria-label="运行数学工作区"
                @click="runMathCell(cell, false)"
              >
                <Icon :icon="playIcon" width="16" height="16" />
              </button>
              <!-- 预览模式切换：公式渲染(默认) <-> 命令模式(按代码展示，不渲染 LaTeX)。 -->
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :class="{ 'is-active': getMathCellPreviewMode(cell) === 'command' }"
                :data-tooltip="getMathCellPreviewMode(cell) === 'command' ? '当前：命令模式预览（点击切回公式渲染）' : '当前：公式渲染（点击切到命令模式）'"
                :aria-label="getMathCellPreviewMode(cell) === 'command' ? '切换到公式预览' : '切换到命令模式预览'"
                @click="toggleMathCellPreviewMode(cell)"
              >
                <Icon :icon="getMathCellPreviewMode(cell) === 'command' ? sigmaIcon : terminalIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                data-tooltip="打开 LaTeX / 数学模块文档助手"
                aria-label="打开 LaTeX / 数学模块文档助手"
                @click="openNotebookUsageAssistant('math', cell, $event)"
              >
                <Icon :icon="circleHelpIcon" width="16" height="16" />
              </button>
              <!-- 数学工作区面板：查看工作区变量 / 数值·仿真工作台（独立承载层，不影响 Python Cell 面板）。 -->
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :class="{ 'is-active': isNotebookMathWorkspacePanelVisible && activeMathWorkspaceCellKey === cell.key }"
                data-tooltip="打开数学工作区面板（工作区变量 / 数值·仿真工作台）"
                aria-label="打开数学工作区面板"
                @click="toggleMathWorkspacePanel(cell)"
              >
                <Icon :icon="panelRightIcon" width="16" height="16" />
              </button>
              <!-- 数学工具下拉菜单：函数/方程、微积分、级数、统计分布、积分变换。 -->
              <div class="guaxi-notebook__math-menu-anchor">
                <button
                  type="button"
                  class="guaxi-notebook__icon-btn guaxi-notebook__math-menu-trigger"
                  :class="{ 'is-active': activeMathToolMenuKey === cell.key }"
                  data-tooltip="插入命令：微积分 / 线代 / 概率 / 绘图"
                  aria-label="打开数学工具菜单"
                  :aria-expanded="activeMathToolMenuKey === cell.key"
                  @click="toggleMathToolMenu(cell)"
                >
                  <Icon :icon="sigmaIcon" width="16" height="16" />
                  <Icon :icon="chevronDownIcon" width="12" height="12" />
                </button>
                <div
                  v-if="activeMathToolMenuKey === cell.key"
                  class="guaxi-notebook__math-menu"
                  role="menu"
                >
                  <div
                    v-for="group in NOTEBOOK_MATH_TOOL_GROUPS"
                    :key="group.label"
                    class="guaxi-notebook__math-menu-group"
                  >
                    <div class="guaxi-notebook__math-menu-group-title">{{ group.label }}</div>
                    <button
                      v-for="item in group.items"
                      :key="item.label"
                      type="button"
                      class="guaxi-notebook__math-menu-item"
                      role="menuitem"
                      @click="handleMathToolSelect(cell, item.snippet)"
                    >
                      <span class="guaxi-notebook__math-menu-item-label">{{ item.label }}</span>
                      <span class="guaxi-notebook__math-menu-item-hint">{{ item.hint }}</span>
                    </button>
                  </div>
                </div>
              </div>
              <!-- 设计说明：数学块与 Python Cell 对称，表达式与输出只存 cellmeta 侧数据，
                   绝不写入源 Markdown（避免被预览/导出当作 $$ 公式渲染），因此不再提供「运行并写回」。 -->
              <button
                v-if="hasVisibleOutput(cell)"
                type="button"
                class="guaxi-notebook__icon-btn"
                :data-tooltip="getCellSnapshot(cell).meta.outputCollapsed ? '显示数学输出' : '隐藏数学输出'"
                :aria-label="getCellSnapshot(cell).meta.outputCollapsed ? '显示数学输出' : '隐藏数学输出'"
                @click="toggleMathCellOutputCollapsed(cell)"
              >
                <Icon :icon="getCellSnapshot(cell).meta.outputCollapsed ? eyeIcon : eyeOffIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn danger"
                data-tooltip="删除当前数学块"
                aria-label="删除当前数学块"
                @click="confirmRemoveCell(cell)"
              >
                <Icon :icon="trash2Icon" width="16" height="16" />
              </button>
            </div>
          </div>
          <GuaxiNotebookMathEditor
            class="guaxi-notebook__math-input"
            :ref="(instance) => setMathEditorRef(cell.key, instance)"
            :cell-key="cell.key"
            :document-path="filePath"
            :model-value="cell.expression"
            :theme="theme"
            placeholder="数学工作区：每行写赋值 / 表达式 / 命令&#10;f = sin(x)/x&#10;limit(f, x, 0)&#10;plot(f, (x,-10,10))"
            @update:model-value="updateMathCell(cell.key, $event)"
            @focus="activateNotebookCell(cell, 'math-editor-focus')"
          />
          <div class="guaxi-notebook__math-preview markdown-body" v-html="getRenderedMathCellHtml(cell)"></div>

          <div v-if="(hasVisibleOutput(cell) || isCellBusy(cell)) && !getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel guaxi-notebook__panel--math">
            <div class="guaxi-notebook__panel-head">
              <div class="guaxi-notebook__panel-title">数学输出 · {{ getCellOutputModules(cell).length }}</div>
            </div>
            <div class="guaxi-notebook__panel-content">
              <div v-if="isCellBusy(cell)" class="guaxi-notebook__running-banner">数学引擎正在运行，请稍候...</div>
              <article
                v-for="outputModule in getCellOutputModules(cell)"
                :key="`${cell.key}-${outputModule.id}`"
                class="guaxi-notebook__output-module"
                :class="{ 'is-collapsed': outputModule.collapsed }"
              >
                <div class="guaxi-notebook__output-module-head">
                  <div class="guaxi-notebook__output-module-copy">
                    <span class="guaxi-notebook__output-module-title">{{ getNotebookOutputModuleTitle(outputModule) }}</span>
                    <span v-if="getNotebookOutputModuleSummary(outputModule)" class="guaxi-notebook__output-module-summary">
                      {{ getNotebookOutputModuleSummary(outputModule) }}
                    </span>
                  </div>
                  <div class="guaxi-notebook__output-module-actions">
                    <button
                      v-if="canCopyOutputModuleText(outputModule)"
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      title="复制当前数学输出的文字和 LaTeX 源码"
                      aria-label="复制当前数学输出的文字和 LaTeX 源码"
                      @click="copyCellOutputModuleText(cell, outputModule.id)"
                    >
                      <Icon :icon="copyIcon" width="14" height="14" />
                    </button>
                    <!-- 写回 Markdown：用户显式把数学输出作为新 Markdown Cell 插入到当前块之后。
                         （注意：进入探索者模式不会自动写回，这是手动操作。） -->
                    <button
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      title="写回到 Markdown 文档"
                      aria-label="写回到 Markdown 文档"
                      @click="insertOutputMarkdownAfterCell(cell, outputModule.id)"
                    >
                      <Icon :icon="fileInputIcon" width="14" height="14" />
                    </button>
                    <button
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      title="导出为文件"
                      aria-label="导出为文件"
                      @click="exportCellOutputToFile(cell, outputModule.id)"
                    >
                      <Icon :icon="fileOutputIcon" width="14" height="14" />
                    </button>
                    <button
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      :title="outputModule.collapsed ? '显示当前数学输出' : '隐藏当前数学输出'"
                      :aria-label="outputModule.collapsed ? '显示当前数学输出' : '隐藏当前数学输出'"
                      @click="toggleCellOutputModule(cell, outputModule.id)"
                    >
                      <Icon :icon="outputModule.collapsed ? eyeIcon : eyeOffIcon" width="14" height="14" />
                    </button>
                    <button
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn danger"
                      title="删除当前数学输出"
                      aria-label="删除当前数学输出"
                      @click="deleteCellOutputModule(cell, outputModule.id)"
                    >
                      <Icon :icon="trash2Icon" width="14" height="14" />
                    </button>
                  </div>
                </div>
                <div v-if="!outputModule.collapsed" class="guaxi-notebook__output-module-body">
                  <GuaxiNotebookHtmlOutput
                    v-if="outputModule.kind === 'html' || outputModule.kind === 'svg'"
                    :html="outputModule.kind === 'svg' ? outputModule.display?.svg : outputModule.display?.html"
                    :source-path="outputModule.kind === 'html' ? getCellDisplaySourcePath(outputModule) : ''"
                    :title="`Math Output ${cell.key || ''}`"
                  />
                  <div
                    v-else-if="outputModule.kind === 'markdown'"
                    class="guaxi-notebook__output-markdown markdown-body"
                    v-html="getRenderedOutputMarkdownHtml(cell, outputModule)"
                  ></div>
                  <img
                    v-else-if="outputModule.kind === 'image' && getCellDisplayUrl(cell, outputModule)"
                    class="guaxi-notebook__image"
                    :src="getCellDisplayUrl(cell, outputModule)"
                    alt="Math output"
                  />
                  <pre v-else class="guaxi-notebook__output-text">{{ buildOutputModuleMarkdownText(outputModule) }}</pre>
                </div>
              </article>
            </div>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="guaxi-notebook__python-layout">
          <div class="guaxi-notebook__python-toolbar">
            <div class="guaxi-notebook__python-toolbar-copy">
              <span class="guaxi-notebook__python-badge" :class="{ 'is-busy': isCellBusy(cell) }">
                <Icon
                  :icon="pythonIcon"
                  width="14"
                  height="14"
                  class="guaxi-notebook__python-badge-icon"
                  :class="{ 'guaxi-notebook__busy-spin': isCellBusy(cell) }"
                />
                <span>{{ isCellBusy(cell) ? '运行中' : 'Python' }}</span>
              </span>
              <span v-if="String(cell.meta?.title || '').trim()" class="guaxi-notebook__python-title">{{ cell.meta?.title }}</span>
            </div>
            <div class="guaxi-notebook__python-toolbar-actions">
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :title="getCellSnapshot(cell).meta.cellCollapsed ? '展开当前 Python Cell' : '折叠当前 Python Cell'"
                :aria-label="getCellSnapshot(cell).meta.cellCollapsed ? '展开当前 Python Cell' : '折叠当前 Python Cell'"
                @click="togglePythonCellCollapsed(cell)"
              >
                <Icon :icon="getCellSnapshot(cell).meta.cellCollapsed ? chevronRightIcon : chevronDownIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                title="打开 Python 官方文档助手"
                aria-label="打开 Python 官方文档助手"
                @click="openNotebookUsageAssistant('python', cell, $event)"
              >
                <Icon :icon="circleHelpIcon" width="16" height="16" />
              </button>
              <button
                v-if="pythonCodegenAssistEnabled"
                type="button"
                class="guaxi-notebook__icon-btn"
                title="AI 生成 Python 代码"
                aria-label="AI 生成 Python 代码"
                @click="openCodegenPanel(cell, $event)"
              >
                <Icon :icon="sparklesIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn danger"
                title="删除当前 Python Cell"
                aria-label="删除当前 Python Cell"
                @click="confirmRemoveCell(cell)"
              >
                <Icon :icon="trash2Icon" width="16" height="16" />
              </button>
            </div>
          </div>

          <div v-if="getCellSnapshot(cell).meta.staleVariables?.length" class="guaxi-notebook__stale-banner">
            上游变量已更新：{{ getCellSnapshot(cell).meta.staleVariables.join(', ') }}。建议重新运行当前 Cell。
          </div>

          <template v-if="!getCellSnapshot(cell).meta.cellCollapsed">
            <GuaxiNotebookPythonEditor
              :cell-key="cell.key"
              :document-path="filePath"
              :model-value="cell.code"
              :theme="theme"
              :ref="(instance) => setPythonEditorRef(cell.key, instance)"
              @update:model-value="updatePythonCell(cell.key, $event)"
              @focus="handleNotebookPythonEditorFocus(cell)"
            />

            <div v-if="(hasVisibleOutput(cell) || isCellBusy(cell)) && !getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel">
              <div class="guaxi-notebook__panel-head">
                <div class="guaxi-notebook__panel-title">输出 · {{ getCellOutputModules(cell).length }}</div>
                <div class="guaxi-notebook__panel-actions">
                  <button
                    v-if="getLatestTextOutputModule(cell)"
                    type="button"
                    class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                    title="复制文本输出"
                    aria-label="复制文本输出"
                    @click="copyPythonCellTextOutput(cell)"
                  >
                    <Icon :icon="copyIcon" width="14" height="14" />
                  </button>
                  <button
                    type="button"
                    class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                    title="写回到 Markdown 文档"
                    aria-label="写回到 Markdown 文档"
                    @click="writeCellOutputToMarkdown(cell)"
                  >
                    <Icon :icon="fileInputIcon" width="14" height="14" />
                  </button>
                  <button
                    type="button"
                    class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                    title="导出为文件"
                    aria-label="导出为文件"
                    @click="exportCellOutputToFile(cell)"
                  >
                    <Icon :icon="fileOutputIcon" width="14" height="14" />
                  </button>
                </div>
              </div>
              <div class="guaxi-notebook__panel-content">
                <div v-if="isCellBusy(cell)" class="guaxi-notebook__running-banner">Python 正在运行，请稍候...</div>
                <div
                  v-for="outputModule in getCellOutputModules(cell)"
                  :key="`${cell.key}-${outputModule.id}`"
                  class="guaxi-notebook__output-module"
                  :class="{ 'is-collapsed': outputModule.collapsed }"
                >
                  <div class="guaxi-notebook__output-module-head">
                    <div class="guaxi-notebook__output-module-copy">
                      <span class="guaxi-notebook__output-module-title">{{ getNotebookOutputModuleTitle(outputModule) }}</span>
                      <span v-if="getNotebookOutputModuleSummary(outputModule)" class="guaxi-notebook__output-module-summary">
                        {{ getNotebookOutputModuleSummary(outputModule) }}
                      </span>
                    </div>
                    <div class="guaxi-notebook__output-module-actions">
                      <button
                        v-if="outputModule.kind === 'text'"
                        type="button"
                        class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                        title="复制当前模块文本"
                        aria-label="复制当前模块文本"
                        @click="copyPythonCellTextOutput(cell, outputModule.id)"
                      >
                        <Icon :icon="copyIcon" width="14" height="14" />
                      </button>
                      <button
                        type="button"
                        class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                        title="写回到 Markdown 文档"
                        aria-label="写回到 Markdown 文档"
                        @click="writeCellOutputToMarkdown(cell, outputModule.id)"
                      >
                        <Icon :icon="fileInputIcon" width="14" height="14" />
                      </button>
                      <button
                        type="button"
                        class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                        title="导出为文件"
                        aria-label="导出为文件"
                        @click="exportCellOutputToFile(cell, outputModule.id)"
                      >
                        <Icon :icon="fileOutputIcon" width="14" height="14" />
                      </button>
                      <button
                        type="button"
                        class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                        :title="outputModule.collapsed ? '显示当前输出模块' : '隐藏当前输出模块'"
                        :aria-label="outputModule.collapsed ? '显示当前输出模块' : '隐藏当前输出模块'"
                        @click="toggleCellOutputModule(cell, outputModule.id)"
                      >
                        <Icon :icon="outputModule.collapsed ? eyeIcon : eyeOffIcon" width="14" height="14" />
                      </button>
                      <button
                        type="button"
                        class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn danger"
                        title="删除当前输出模块"
                        aria-label="删除当前输出模块"
                        @click="deleteCellOutputModule(cell, outputModule.id)"
                      >
                        <Icon :icon="trash2Icon" width="14" height="14" />
                      </button>
                    </div>
                  </div>
                  <div v-if="!outputModule.collapsed" class="guaxi-notebook__output-module-body">
                    <div v-if="outputModule.kind === 'html'" class="guaxi-notebook__rich guaxi-notebook__rich--html">
                      <GuaxiNotebookHtmlOutput
                        :html="String(outputModule.display?.html || '')"
                        :source-path="getCellDisplaySourcePath(outputModule)"
                        :theme="theme"
                        :title="`Python Cell HTML Output ${cell.cellId || cell.key || ''}`"
                      />
                    </div>
                    <div v-else-if="outputModule.kind === 'svg'" class="guaxi-notebook__rich" v-html="String(outputModule.display?.svg || '')"></div>
                    <div
                      v-else-if="outputModule.kind === 'markdown'"
                      class="guaxi-notebook__rich markdown-body"
                      v-html="getRenderedOutputMarkdownHtml(cell, outputModule)"
                    ></div>
                    <img
                      v-else-if="outputModule.kind === 'image' && getCellDisplayUrl(cell, outputModule)"
                      class="guaxi-notebook__image"
                      :src="getCellDisplayUrl(cell, outputModule)"
                      alt="Python Cell Output"
                    />
                    <audio
                      v-else-if="outputModule.kind === 'audio' && getCellDisplayUrl(cell, outputModule)"
                      class="guaxi-notebook__media"
                      :src="getCellDisplayUrl(cell, outputModule)"
                      controls
                      preload="metadata"
                    ></audio>
                    <video
                      v-else-if="outputModule.kind === 'video' && getCellDisplayUrl(cell, outputModule)"
                      class="guaxi-notebook__media guaxi-notebook__media--video"
                      :src="getCellDisplayUrl(cell, outputModule)"
                      controls
                      loop
                      autoplay
                      muted
                      playsinline
                      preload="metadata"
                    ></video>
                    <div v-else-if="outputModule.kind === 'table' && outputModule.table" class="guaxi-notebook__dataframe guaxi-notebook__dataframe--virtual-table">
                      <GuaxiNotebookTableOutput :table="outputModule.table" />
                    </div>
                    <pre v-else-if="outputModule.kind === 'text'" class="guaxi-notebook__output-text">{{ outputModule.text }}</pre>
                  </div>
                </div>
                <div v-if="!getCellOutputModules(cell).length && !isCellBusy(cell)" class="guaxi-notebook__empty guaxi-notebook__empty--dashed">
                  当前还没有输出结果
                </div>
              </div>
            </div>
          </template>
          <button
            v-else
            type="button"
            class="guaxi-notebook__python-collapsed-summary"
            @click="togglePythonCellCollapsed(cell)"
          >
            <span>{{ getPythonCellOutlineTitle(cell) }}</span>
            <code>{{ getPythonCellCodeSummary(cell) }}</code>
          </button>
        </div>
      </template>
      </div>

      <div v-if="index < notebookCells.length - 1" class="guaxi-notebook__insert-strip">
        <div class="guaxi-notebook__insert-line"></div>
        <div class="guaxi-notebook__insert-trigger" aria-hidden="true">
          <Icon :icon="plusIcon" width="12" height="12" />
        </div>
        <div class="guaxi-notebook__insert-actions">
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Markdown Cell"
            aria-label="在这里插入 Markdown Cell"
            @click="insertCell(index, 'below', 'markdown')"
          >
            <Icon :icon="filePlusIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Python Cell"
            aria-label="在这里插入 Python Cell"
            @click="insertCell(index, 'below', 'python')"
          >
            <Icon :icon="terminalSquareIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入数学块"
            aria-label="在这里插入数学块"
            @click="insertCell(index, 'below', 'math')"
          >
            <Icon :icon="sigmaIcon" width="16" height="16" />
          </button>
        </div>
      </div>
    </template>

    <GuaxiSelectionTranslatePanel
      v-if="selectionTranslatePanelVisible"
      :pos="selectionTranslatePanelPos"
      :raw="selectionTranslateRaw"
      :result="selectionTranslateResult"
      :error="selectionTranslateError"
      :busy="selectionTranslateBusy"
      :pinned="selectionTranslatePinned"
      :items="selectionTranslateItems"
      :word-meta="selectionTranslateWordMeta"
      :from-lang="selectionTranslateFrom"
      :to-lang="selectionTranslateTo"
      :lang-options="selectionTranslateLangOptions"
      @update:from-lang="(value) => (selectionTranslateFrom = String(value || 'auto'))"
      @update:to-lang="(value) => (selectionTranslateTo = String(value || 'zh'))"
      @swap-langs="swapNotebookSelectionTranslateLangs"
      @translate="translateNotebookSelectionText"
      @copy="copyNotebookSelectionTranslateResult"
      @toggle-pin="toggleNotebookSelectionTranslatePinned"
      @update:pos="handleNotebookSelectionTranslatePanelUpdatePos"
      @close="closeNotebookSelectionTranslatePanel"
    />
    <GuaxiNotebookUsageAssistant
      :visible="notebookUsageAssistantVisible"
      :channel="notebookUsageAssistantChannel"
      :context-content="notebookUsageAssistantContextContent"
      :anchor-position="notebookUsageAssistantAnchorPosition"
      :theme="theme"
      @close="closeNotebookUsageAssistant"
      @update:channel="(value) => (notebookUsageAssistantChannel = value)"
      @insert-example="handleNotebookUsageAssistantInsertExample"
      @ask-assistant="handleNotebookUsageAssistantAsk"
    />
    <GuaxiNotebookCodegenPanel
      v-model:visible="codegenPanelVisible"
      :anchor="codegenPanelAnchor"
      :context="codegenPanelContext"
      :session-id="codegenPanelSessionId"
      @insert="handleCodegenInsert"
    />
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { notebookReadingLocationBackStack, notebookReadingLocationForwardStack } from '../../stores/notebookReadingStore'
import { Icon } from '@iconify/vue'
import { emit as emitTauriEvent, listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useMessage } from 'naive-ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { confirm as confirmDialog, open, save } from '@tauri-apps/plugin-dialog'
import mermaid from 'mermaid'
import MarkdownIt from 'markdown-it'
import { debounce } from 'lodash-es'
import filePlusIcon from '@iconify-icons/lucide/file-plus'
import terminalSquareIcon from '@iconify-icons/lucide/terminal-square'
import sigmaIcon from '@iconify-icons/lucide/sigma'
import playIcon from '@iconify-icons/lucide/play'
import squareIcon from '@iconify-icons/lucide/square'
import checkCircleIcon from '@iconify-icons/lucide/check-circle-2'
import plusIcon from '@iconify-icons/lucide/plus'
import searchIcon from '@iconify-icons/lucide/search'
import saveIcon from '@iconify-icons/lucide/save'
import uploadIcon from '@iconify-icons/lucide/upload'
import eyeIcon from '@iconify-icons/lucide/eye'
import eyeOffIcon from '@iconify-icons/lucide/eye-off'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import flaskConicalIcon from '@iconify-icons/lucide/flask-conical'
import terminalIcon from '@iconify-icons/lucide/terminal'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import copyIcon from '@iconify-icons/lucide/copy'
import languagesIcon from '@iconify-icons/lucide/languages'
import messageCircleIcon from '@iconify-icons/lucide/message-circle'
import botIcon from '@iconify-icons/lucide/bot'
import volume2Icon from '@iconify-icons/lucide/volume-2'
import circleHelpIcon from '@iconify-icons/lucide/help-circle'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import panelRightIcon from '@iconify-icons/lucide/panel-right'
import fileInputIcon from '@iconify-icons/lucide/file-input'
import fileOutputIcon from '@iconify-icons/lucide/file-output'
import pythonIcon from '@iconify-icons/vscode-icons/file-type-python'
import { createLogger } from '../../utils/logger'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSiderStore } from '../../stores/useSiderStore'
import { useMathWorkspaceStore } from '../../stores/mathWorkspaceStore'
import GuaxiNotebookPythonEditor from './GuaxiNotebookPythonEditor.vue'
import GuaxiNotebookMathEditor from './GuaxiNotebookMathEditor.vue'
import GuaxiNotebookMarkdownEditor from './GuaxiNotebookMarkdownEditor.vue'
import GuaxiNotebookHtmlOutput from './GuaxiNotebookHtmlOutput.vue'
import GuaxiNotebookTableOutput from './GuaxiNotebookTableOutput.vue'
import GuaxiSelectionTranslatePanel from './GuaxiSelectionTranslatePanel.vue'
import GuaxiNotebookUsageAssistant from './GuaxiNotebookUsageAssistant.vue'
import GuaxiNotebookCodegenPanel from './GuaxiNotebookCodegenPanel.vue'
import GuaxiFindWidget from './GuaxiFindWidget.vue'
import {
  SEARCH_HIT_ACTIVE_CLASS,
  SEARCH_HIT_CLASS,
  applySearchHighlights,
  buildSearchRegex,
  clearSearchHighlights,
  setActiveSearchHit,
} from '../../utils/domTextSearch'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import {
  createPythonCellId,
  createPythonSessionId,
  dataframeToMarkdownTable,
  maskPythonCellBlocksForMarkdown,
  parsePythonCellBlocks,
  replacePythonCellBlock,
} from './utils/guaxiPythonCellSyntax'
import {
  maskMathCellBlocksForMarkdown,
} from './utils/guaxiMathCellSyntax'
import {
  appendPythonCellOutputModules,
  buildPythonCellOutputModules,
  createPythonCellOutputModule,
  getLatestPythonCellOutputModule,
  getLatestPythonCellOutputModuleByKind,
  getPythonCellOutputDisplayDataUrl,
  getPythonCellOutputDisplaySourcePath,
  isPythonCellDisplayOutputKind,
  normalizePythonCellOutputDisplayType,
  normalizePythonCellOutputModules,
  removePythonCellOutputModule,
  togglePythonCellOutputModuleCollapsed,
} from './utils/guaxiPythonCellOutputs'
import {
  createNotebookCellsFromMarkdown,
  createNotebookMarkdownCell,
  createNotebookMathCell,
  createNotebookPythonCell,
  normalizeNotebookCells,
  serializeNotebookCells,
} from './utils/guaxiNotebookCells'
import {
  getPythonCellSnapshot,
  patchPythonCellSnapshot,
  replacePythonCellSnapshots,
} from './utils/guaxiPythonCellState'
import {
  DATA_LAB_SESSION_VARS_UPDATED_EVENT,
  filterVisibleWorkbenchSessionVariables,
} from './utils/guaxiDataLabWorkbenchClient'
import {
  NOTEBOOK_DATALAB_EVENT,
  classifyNotebookLabVariable,
  buildAnalysisCode,
  buildNotebookLabDatasetDescriptors,
  buildNotebookLabImportPlan,
  buildNotebookLabImportPlanFromSpec,
  buildNotebookLabMaterializeCode,
  buildNotebookLabMathCode,
  buildNotebookLabPlotCode,
  buildNotebookLabReportCode,
  buildNotebookLabVariableOperationCode,
  decorateNotebookLabOutputModules,
  installNotebookDataLabBridge,
  upsertNotebookLabImportRecord,
} from './utils/guaxiNotebookDataLab'

const props = defineProps({
  markdown: {
    type: String,
    default: '',
  },
  filePath: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
  },
})

const emit = defineEmits(['update:markdown', 'state-change', 'open-console', 'request-save', 'outline-change', 'open-local-link'])

const message = useMessage()
const logger = createLogger('GuaxiNotebookView')
const notebookOutlineMarkdownParser = new MarkdownIt()
const fileTreeStore = useFileTreeStore()
const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore()
// 数学块承载层专用 store（与 Python Cell 的 rightSiderStore 解耦，仅借用右侧区域/新窗口）。
const mathWorkspaceStore = useMathWorkspaceStore()
const NOTEBOOK_RIGHT_SIDER_VIEW_KEY = 'home'
// 当前被「数学工作区」面板选中的数学块 key（自成一套，不与 activeNotebookPythonCellKey 混用）。
const activeMathWorkspaceCellKey = ref('')

const refreshTick = ref(0)
const pendingFocusCellKey = ref('')
const busyMap = ref({})
const notebookActivePythonRun = ref(null)
const notebookCancelledPythonRunIds = new Set()
const notebookPendingPythonImport = ref(null)
const notebookQueuedPythonAction = ref(null)
const notebookRootRef = ref(null)
const notebookFindWidgetRef = ref(null)
const editingMarkdownCellKey = ref('')
const renderedMarkdownHtmlMap = ref({})
const renderedOutputMarkdownHtmlMap = ref({})
const resolvedDisplayUrlMap = ref({})
const markdownRenderVersion = ref(0)
const notebookCells = ref([])
const prewarmedPythonSessionKey = ref('')
const pythonCodegenAssistEnabled = ref(false)
const codegenPanelVisible = ref(false)
const codegenPanelAnchor = ref({ x: 0, y: 0 })
const codegenPanelContext = ref('')
const codegenPanelSessionId = ref('')
const codegenTargetCellKey = ref('')
const notebookSearchVisible = ref(false)
const notebookSearchKeyword = ref('')
const notebookSearchCellMatches = ref([])
const notebookSearchDomMatches = ref([])
const notebookSearchActiveIndex = ref(-1)
const notebookSearchIndexReady = ref(false)
// 与 Monaco find widget 对齐的三个搜索选项，以及“正则写错了”的提示状态。
const notebookSearchCaseSensitive = ref(false)
const notebookSearchWholeWord = ref(false)
const notebookSearchUseRegex = ref(false)
const notebookSearchRegexInvalid = ref(false)
const notebookSelectionToolbarVisible = ref(false)
const notebookSelectionToolbarStyle = ref({ left: '0px', top: '0px' })
const notebookSelectionText = ref('')
const notebookTopToolbarRevealed = ref(false)
const activeNotebookCellKey = ref('')
const activeNotebookPythonCellKey = ref('')
const notebookFocusedVariable = ref({ cellKey: '', variableName: '' })
// 数学块工具下拉菜单：记录当前展开菜单所属的数学块 key（空串表示全部收起）。
const activeMathToolMenuKey = ref('')
// 每个数学块的预览模式：'latex'（默认，按公式渲染）/ 'command'（命令式工作区，按代码展示，不渲染 LaTeX）。
const notebookMathPreviewModeMap = ref({})
// 离线文档助手只挂在探索者 Notebook 表面中，记录触发来源 Cell 以便按上下文搜索完整 Python / 数学文档。
const notebookUsageAssistantVisible = ref(false)
const notebookUsageAssistantChannel = ref('python')
const notebookUsageAssistantCellKey = ref('')
const notebookUsageAssistantAnchorPosition = ref({ left: null, top: null, width: null, maxHeight: null })
const notebookMarkdownWorkerRef = ref(null)
const notebookMarkdownWorkerReady = ref(false)
const lastEmittedMarkdown = ref(String(props.markdown || '').replace(/\r\n?/g, '\n'))
const imageBedRootCache = ref('')
const notebookSidecarLoadVersion = ref(0)
const notebookHistoryEntries = ref([])
const notebookHistoryIndex = ref(-1)
const notebookHistoryApplying = ref(false)
// 阅读位置历史栈从模块级 store 引用，组件卸载/重挂载后历史不丢失
let lastRecordedNotebookReadingLocationKey = ''
let lastRecordedNotebookReadingLocationAt = 0
let suppressNotebookReadingLocationRecording = false
const maxNotebookReadingLocationHistory = 200
const markdownEditorRefMap = new Map()
const pythonEditorRefMap = new Map()
const mathEditorRefMap = new Map()
const notebookMarkdownRenderCache = new Map()
const notebookDisplayUrlCache = new Map()
const notebookMarkdownWorkerPendingTasks = new Map()
const notebookLinkCardMetaCache = new Map()
const selectionTranslatePanelVisible = ref(false)
const selectionTranslatePinned = ref(false)
const selectionTranslatePanelPos = ref({ left: 8, top: 8 })
const selectionTranslateFrom = ref('auto')
const selectionTranslateTo = ref('zh')
const selectionTranslateBusy = ref(false)
const selectionTranslateRaw = ref('')
const selectionTranslateResult = ref('')
const selectionTranslateError = ref('')
const selectionTranslateItems = ref([])
const selectionTranslateWordMeta = ref(null)
const selectionReadAloudBusy = ref(false)
let notebookSidecarPersistTimer = 0
let notebookMarkdownEmitTimer = 0
let notebookSearchIndexSyncTimer = 0
let notebookSearchQueryTimer = 0
let notebookMarkdownWorkerTaskId = 0
let notebookMermaidRenderCounter = 0
let notebookTopToolbarHideTimer = 0
let notebookFocusedVariableTimer = 0
let notebookPointerQuietUntil = 0
let notebookMathPreviewRenderSeq = 0
let selectionAudioElement = null
let selectionAudioObjectUrl = ''
let selectionTranslateMultiNotFoundWarned = false
let disposeNotebookDataLabBridge = null
let notebookDataLabRpcUnlisten = null

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))
const isNotebookTopToolbarVisible = computed(() => notebookTopToolbarRevealed.value || notebookSearchVisible.value)
const activeNotebookPythonCell = computed(() => notebookCells.value.find((cell) => (
  cell?.type === 'python' && String(cell?.key || '') === String(activeNotebookPythonCellKey.value || '')
)) || null)
const notebookUsageAssistantContextCell = computed(() => notebookCells.value.find((cell) => (
  String(cell?.key || '') === String(notebookUsageAssistantCellKey.value || '')
)) || null)
const notebookUsageAssistantContextContent = computed(() => {
  const cell = notebookUsageAssistantContextCell.value
  if (!cell) return ''
  if (cell.type === 'math') return String(cell.expression || '')
  if (cell.type === 'python') return String(cell.code || '')
  return String(cell.content || '')
})
const selectionTranslateMultiAvailable = ref(true)
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
  { label: '泰语', value: 'th' },
]
const getSelectionTranslateLangLabel = (value) => {
  const item = selectionTranslateLangOptions.find((entry) => entry.value === value)
  return item ? String(item.label || '') : '中文'
}

const activeNotebookPythonCellSnapshot = computed(() => {
  if (!activeNotebookPythonCell.value) {
    return {
      meta: {
        outputCollapsed: false,
        sessionCollapsed: true,
      },
    }
  }
  return getCellSnapshot(activeNotebookPythonCell.value)
})
const isNotebookPythonSessionPanelVisible = computed(() => (
  rightSiderStore.isOpen
  && rightSiderStore.panel === 'python-session'
  && siderStore.isRightSiderCollapsed?.[NOTEBOOK_RIGHT_SIDER_VIEW_KEY] === false
))
// 数学工作区面板可见性：与 Python 面板互斥（panel 不同），互不干扰。
const isNotebookMathWorkspacePanelVisible = computed(() => (
  rightSiderStore.isOpen
  && rightSiderStore.panel === 'math-workspace'
  && siderStore.isRightSiderCollapsed?.[NOTEBOOK_RIGHT_SIDER_VIEW_KEY] === false
))

const bumpRefresh = () => {
  refreshTick.value += 1
  emit('state-change')
}

// 设计说明：
// 1. 探索者模式顶部工具栏现在需要知道“当前选中的 Python Cell”是谁，才能把运行/导入/数据实验室面板等操作代理过去。
// 2. 这里把选中态收敛成一个单独的 key，而不是把整份 cell 对象长期缓存下来，避免 cell 重建后引用失效。
// 3. Markdown Cell 被点击时会清空这份选中态，保证顶部工具栏只在真正选中 Python Cell 时才影响它。
const setActiveNotebookPythonCellKey = (cellKey, reason = 'unknown') => {
  const normalizedKey = String(cellKey || '').trim()
  if (String(activeNotebookPythonCellKey.value || '') === normalizedKey) return
  activeNotebookPythonCellKey.value = normalizedKey
  logger.info('[NotebookToolbar] active python cell changed', {
    cellKey: normalizedKey,
    reason,
  })
}

const clearActiveNotebookPythonCell = (reason = 'unknown') => {
  if (!String(activeNotebookPythonCellKey.value || '').trim()) return
  setActiveNotebookPythonCellKey('', reason)
  rightSiderStore.clearPythonSessionInspector()
  logger.info('[NotebookToolbar] cleared python session inspector because active cell was cleared', {
    reason,
  })
}

const activateNotebookCell = (cell, reason = 'unknown') => {
  activeNotebookCellKey.value = String(cell?.key || '')
  if (cell?.type === 'python') {
    setActiveNotebookPythonCellKey(String(cell?.key || ''), reason)
    revealNotebookTopToolbar(`python-cell-selected:${reason}`)
    return
  }
  clearActiveNotebookPythonCell(`non-python-cell:${reason}`)
}

const isNotebookInteractiveTarget = (target) => (
  target instanceof HTMLElement
  && !!target.closest('button, input, textarea, select, option, a, [contenteditable="true"]')
)

const handleNotebookCellPointerDown = (cell, event) => {
  if (!isNotebookInteractiveTarget(event?.target)) {
    recordNotebookReadingLocation('cell-pointerdown')
  }
  if (cell?.type === 'python') {
    activateNotebookCell(cell, isNotebookInteractiveTarget(event?.target) ? 'python-interactive-pointerdown' : 'python-cell-pointerdown')
    return
  }
  activateNotebookCell(cell, 'cell-pointerdown')
}

// 设计说明：
// 1. Monaco 编辑器经常会在内部吞掉 pointer 事件，导致“已经在当前 Python Cell 中编辑/运行”，
//    但探索模式外层并不知道该 Cell 已经成为当前上下文。
// 2. 右侧“数据实验室”完全依赖 active cell 来同步会话变量，因此这里在编辑器 focus 时显式补一次激活。
// 3. 如果右侧实验室已经打开，则顺手同步最新 inspector，避免用户运行后还停留在空变量面板。
const handleNotebookPythonEditorFocus = (cell) => {
  if (cell?.type !== 'python') return
  activateNotebookCell(cell, 'python-editor-focus')
  if (isNotebookPythonSessionPanelVisible.value) {
    syncNotebookPythonSessionInspector(cell, 'python-editor-focus')
  }
  logger.debug('[Notebook] python editor focused and synced active cell context', {
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    sessionPanelVisible: isNotebookPythonSessionPanelVisible.value,
  })
}

const clearNotebookTopToolbarHideTimer = () => {
  if (!notebookTopToolbarHideTimer) return
  window.clearTimeout(notebookTopToolbarHideTimer)
  notebookTopToolbarHideTimer = 0
}

const NOTEBOOK_TOP_TOOLBAR_TRIGGER_DISTANCE = 56
const NOTEBOOK_TOP_TOOLBAR_HIDE_DISTANCE = 120
const NOTEBOOK_TOP_TOOLBAR_SCROLL_QUIET_MS = 360
const NOTEBOOK_TOP_TOOLBAR_POINTER_IGNORE_SELECTOR = [
  '.guaxi-notebook-table-output__viewport',
  '.guaxi-notebook__dataframe',
].join(', ')

// 设计说明：
// 1. 探索模式顶部工具条默认压缩成极薄的吸顶感应层，只在鼠标靠近顶部或键盘聚焦时展开。
// 2. 搜索面板打开时强制保持工具条可见，避免搜索条和工具条的 sticky 叠层在滚动时来回跳动。
const setNotebookTopToolbarRevealed = (visible, reason = 'unknown') => {
  if (notebookTopToolbarRevealed.value === visible) return
  notebookTopToolbarRevealed.value = visible
  logger.info('[NotebookToolbar] top toolbar visibility changed', { visible, reason })
}

const revealNotebookTopToolbar = (reason = 'unknown') => {
  clearNotebookTopToolbarHideTimer()
  setNotebookTopToolbarRevealed(true, reason)
}

const hideNotebookTopToolbar = (reason = 'unknown') => {
  if (notebookSearchVisible.value) {
    logger.debug('[NotebookToolbar] keep toolbar visible because search panel is active', { reason })
    return
  }
  setNotebookTopToolbarRevealed(false, reason)
}

const scheduleHideNotebookTopToolbar = (reason = 'unknown') => {
  clearNotebookTopToolbarHideTimer()
  notebookTopToolbarHideTimer = window.setTimeout(() => {
    hideNotebookTopToolbar(reason)
  }, 220)
}

const handleNotebookToolbarFocusOut = (event) => {
  const nextTarget = event?.relatedTarget
  if (nextTarget instanceof Node && notebookRootRef.value?.contains?.(nextTarget)) return
  scheduleHideNotebookTopToolbar('toolbar-focusout')
}

const resolveNotebookToolbarTriggerRect = () => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return null
  const scrollHost = root.parentElement
  if (scrollHost instanceof HTMLElement) {
    return scrollHost.getBoundingClientRect()
  }
  return root.getBoundingClientRect()
}

const shouldIgnoreNotebookToolbarPointerEvent = (event) => {
  if (Date.now() < notebookPointerQuietUntil) return true
  const target = event?.target
  const relatedTarget = event?.relatedTarget
  const ignoredTarget = target instanceof Element && !!target.closest(NOTEBOOK_TOP_TOOLBAR_POINTER_IGNORE_SELECTOR)
  const ignoredRelatedTarget = relatedTarget instanceof Element && !!relatedTarget.closest(NOTEBOOK_TOP_TOOLBAR_POINTER_IGNORE_SELECTOR)
  const ignored = ignoredTarget || ignoredRelatedTarget
  if (ignored) {
    logger.debug('[NotebookToolbar] ignore pointer event from table output surface', {
      targetClass: target instanceof Element ? String(target.className || '') : '',
      relatedTargetClass: relatedTarget instanceof Element ? String(relatedTarget.className || '') : '',
    })
  }
  return ignored
}

const handleNotebookRootWheel = (event) => {
  const target = event?.target
  const fromTableOutput = target instanceof Element && !!target.closest(NOTEBOOK_TOP_TOOLBAR_POINTER_IGNORE_SELECTOR)
  notebookPointerQuietUntil = Date.now() + NOTEBOOK_TOP_TOOLBAR_SCROLL_QUIET_MS
  if (fromTableOutput) {
    logger.debug('[NotebookToolbar] quiet toolbar pointer tracking during table wheel scroll', {
      quietMs: NOTEBOOK_TOP_TOOLBAR_SCROLL_QUIET_MS,
    })
  }
}

// 设计说明：
// 1. 触发区域改成“探索者滚动视口的顶部一段距离”，而不是隐藏后的 8px 细条，避免工具条实际上不可达。
// 2. 这里使用 notebook 的滚动宿主边界作为基准，这样文档滚动很深时，鼠标靠近当前可视区顶部依然能唤起工具条。
const handleNotebookRootMouseMove = (event) => {
  if (shouldIgnoreNotebookToolbarPointerEvent(event)) return
  const triggerRect = resolveNotebookToolbarTriggerRect()
  if (!triggerRect) return
  const offsetFromTop = Number(event?.clientY || 0) - triggerRect.top
  if (offsetFromTop <= NOTEBOOK_TOP_TOOLBAR_TRIGGER_DISTANCE) {
    revealNotebookTopToolbar('root-mouse-near-top')
    return
  }
  if (offsetFromTop >= NOTEBOOK_TOP_TOOLBAR_HIDE_DISTANCE) {
    scheduleHideNotebookTopToolbar('root-mouse-away-from-top')
  }
}

const handleNotebookRootMouseLeave = (event) => {
  if (shouldIgnoreNotebookToolbarPointerEvent(event)) return
  if (event?.relatedTarget instanceof Node && notebookRootRef.value?.contains?.(event.relatedTarget)) {
    logger.debug('[NotebookToolbar] ignore synthetic root mouseleave inside notebook', {
      reason: 'related-target-still-inside-root',
    })
    return
  }
  scheduleHideNotebookTopToolbar('root-mouseleave')
}

const normalizeNotebookMarkdownText = (markdown = '') => String(markdown || '').replace(/\r\n?/g, '\n')
const NOTEBOOK_SELECTION_SURFACE_SELECTOR = '.guaxi-notebook__markdown-preview, .guaxi-notebook__rich, .guaxi-notebook__rich--html'
const NOTEBOOK_SELECTION_IGNORE_SELECTOR = '.guaxi-notebook__toolbar, .guaxi-notebook__search-panel-shell, .guaxi-notebook__selection-toolbar, .pot-selection-translate-panel, textarea, input, select, button, [contenteditable="true"]'
const escapeHtmlAttribute = (raw) => String(raw || '')
  .replace(/&/g, '&amp;')
  .replace(/"/g, '&quot;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
const escapeHtmlText = (raw) => String(raw || '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes
}

const uint8ArrayToBase64 = (bytes) => {
  const buffer = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes || [])
  let binary = ''
  const chunkSize = 0x8000
  for (let offset = 0; offset < buffer.length; offset += chunkSize) {
    const chunk = buffer.subarray(offset, Math.min(buffer.length, offset + chunkSize))
    binary += String.fromCharCode(...chunk)
  }
  return btoa(binary)
}

// 设计说明：
// 1. Notebook 写回输出时需要把二进制资源交给后端统一落盘，避免前端 capability scope 限制。
// 2. 这里保留一个通用二进制写入助手，供“导出文件”和“资源库入库”两条链路复用。
const writeBytesToPath = async (path, bytes, reason = 'unknown') => {
  const normalizedPath = String(path || '').trim()
  if (!normalizedPath) {
    throw new Error('缺少目标文件路径')
  }
  const base64 = uint8ArrayToBase64(bytes)
  await invoke('write_file_bytes_base64', {
    path: normalizedPath,
    base64,
  })
  logger.info('[Notebook] wrote binary file via backend command', {
    path: normalizedPath,
    byteLength: Number(bytes?.byteLength || bytes?.length || 0),
    reason,
  })
}

const readBytesFromPath = async (path, reason = 'unknown') => {
  const normalizedPath = String(path || '').trim()
  if (!normalizedPath) {
    throw new Error('缺少源文件路径')
  }
  const base64 = await invoke('read_file_bytes_base64', { path: normalizedPath })
  const bytes = base64ToUint8Array(base64)
  logger.info('[Notebook] read binary file via backend command', {
    path: normalizedPath,
    byteLength: Number(bytes?.byteLength || bytes?.length || 0),
    reason,
  })
  return bytes
}
const sanitizeFileSegment = (raw, fallback = 'output') => {
  const normalized = String(raw || '')
    .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || fallback
}
const joinNotebookPath = (...parts) => parts
  .map((part, index) => {
    const normalized = String(part || '').replace(/\\/g, '/')
    return index === 0 ? normalized.replace(/\/+$/g, '') : normalized.replace(/^\/+|\/+$/g, '')
  })
  .filter(Boolean)
  .join('/')
const getNotebookDocDir = () => {
  const filePath = String(props.filePath || '').trim().replace(/\\/g, '/')
  if (!filePath) return ''
  const lastSlash = filePath.lastIndexOf('/')
  return lastSlash >= 0 ? filePath.slice(0, lastSlash) : ''
}
// 设计说明：
// 1. Explorer 模式里的 wikilink、卡片元信息和点击打开都会复用这个路径归一化函数。
// 2. 旧实现先把 `E:/foo/bar.md` 拆成 `['E:', 'foo', 'bar.md']`，再额外补一次 `E:/` 前缀，
//    最终生成 `E:/E:/foo/bar.md`，这正是你日志里看到的重复盘符问题。
// 3. 这里改成和主编辑器同一思路：先识别锚定前缀（盘符/UNC/根目录），再只归一化后续路径片段，
//    这样绝对路径、相对路径、`..` 回退都能稳定工作。
const normalizeNotebookFsPath = (rawPath = '') => {
  let normalized = String(rawPath || '').trim().replace(/\\/g, '/')
  if (!normalized) return ''

  if (/^\/[A-Za-z]:\//.test(normalized)) {
    normalized = normalized.slice(1)
  }

  let anchored = false
  let prefix = ''
  let parts = []

  if (/^[A-Za-z]:\//.test(normalized)) {
    anchored = true
    prefix = normalized.slice(0, 3)
    parts = normalized.slice(3).split('/')
  } else if (normalized.startsWith('//')) {
    const uncParts = normalized.slice(2).split('/').filter(Boolean)
    if (uncParts.length >= 2) {
      anchored = true
      prefix = `//${uncParts[0]}/${uncParts[1]}`
      parts = uncParts.slice(2)
    } else {
      return normalized
    }
  } else if (normalized.startsWith('/')) {
    anchored = true
    prefix = '/'
    parts = normalized.slice(1).split('/')
  } else {
    parts = normalized.split('/')
  }

  const output = []
  for (const part0 of parts) {
    const part = String(part0 || '').trim()
    if (!part || part === '.') continue
    if (part === '..') {
      if (output.length && output[output.length - 1] !== '..') {
        output.pop()
      } else if (!anchored) {
        output.push(part)
      }
      continue
    }
    output.push(part)
  }

  if (!output.length) return prefix
  if (!prefix) return output.join('/')
  return prefix.endsWith('/') ? `${prefix}${output.join('/')}` : `${prefix}/${output.join('/')}`
}

const writeClipboardText = async (text) => {
  const value = String(text || '')
  if (!value) return false

  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch (error) {
    logger.warn('[NotebookSelection] navigator clipboard write failed', {
      error: String(error?.message || error),
    })
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.error('[NotebookSelection] clipboard write failed', {
      error: String(error?.message || error),
    })
    return false
  }
}

const readClipboardText = async () => {
  if (navigator?.clipboard?.readText) {
    try {
      return String(await navigator.clipboard.readText())
    } catch (error) {
      logger.warn('[NotebookDataLab] navigator clipboard read failed', {
        error: String(error?.message || error),
      })
    }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    return String(await mod.readText())
  } catch (error) {
    logger.error('[NotebookDataLab] clipboard read failed', {
      error: String(error?.message || error),
    })
    throw new Error('读取剪贴板失败')
  }
}

const normalizeSelectionTextForLanguageTasks = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n，。！？?!;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

const isWordSelection = (raw) => {
  const text = String(raw || '').trim()
  if (!text || /\s/.test(text)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(text)
}

const hideNotebookSelectionToolbar = () => {
  notebookSelectionToolbarVisible.value = false
}

const clearNotebookSelectionState = (reason = 'unknown') => {
  notebookSelectionText.value = ''
  hideNotebookSelectionToolbar()
  logger.info('[NotebookSelection] cleared selection state', { reason })
}

const stopSelectionAudioPlayback = () => {
  if (selectionAudioElement) {
    try {
      selectionAudioElement.pause()
      selectionAudioElement.src = ''
    } catch {}
    selectionAudioElement = null
  }
  if (selectionAudioObjectUrl) {
    try {
      URL.revokeObjectURL(selectionAudioObjectUrl)
    } catch {}
    selectionAudioObjectUrl = ''
  }
}

// 设计说明：
// 1. Explorer 模式里存在多个渲染面：Markdown 预览、输出面板、HTML 输出容器。
// 2. 这里统一从 `window.getSelection()` 反查最近的合法渲染面，避免把工具条错误地挂到搜索框、
//    Cell 编辑器或工具按钮上。
const resolveNotebookSelectionSurface = (selection, fallbackTarget = null) => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement) || !selection) return null

  const candidates = [
    selection.anchorNode,
    selection.focusNode,
    selection.rangeCount > 0 ? selection.getRangeAt(0).commonAncestorContainer : null,
    fallbackTarget,
  ]

  for (const node of candidates) {
    const element = node instanceof Element ? node : node?.parentElement
    if (!(element instanceof Element)) continue
    const surface = element.closest(NOTEBOOK_SELECTION_SURFACE_SELECTOR)
    if (surface instanceof HTMLElement && root.contains(surface)) {
      return surface
    }
  }

  return null
}

const getNotebookSelectionTextFromWindow = (fallbackTarget = null) => {
  const selection = window.getSelection?.()
  const root = notebookRootRef.value
  if (!selection || !(root instanceof HTMLElement) || selection.rangeCount === 0) {
    return { text: '', surface: null }
  }

  const text = normalizeSelectionTextForLanguageTasks(selection.toString())
  if (!text) return { text: '', surface: null }

  const surface = resolveNotebookSelectionSurface(selection, fallbackTarget)
  if (!(surface instanceof HTMLElement)) {
    return { text: '', surface: null }
  }

  return { text, surface }
}

const getNotebookSelectionTextForLanguageTasks = () => {
  const text = normalizeSelectionTextForLanguageTasks(
    notebookSelectionText.value || getNotebookSelectionTextFromWindow().text,
  )
  return text
}

const scheduleSelectionTranslate = debounce(() => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  translateNotebookSelectionText(raw)
}, 250)
const getNotebookDocStem = () => {
  const filePath = String(props.filePath || '').trim().replace(/\\/g, '/')
  const baseName = filePath.split('/').pop() || 'notebook'
  return sanitizeFileSegment(baseName.replace(/\.[^.]+$/, ''), 'notebook')
}
const getRelativeNotebookPath = (targetPath) => {
  const docDir = getNotebookDocDir()
  const normalizedTarget = String(targetPath || '').replace(/\\/g, '/')
  if (docDir && normalizedTarget.startsWith(`${docDir}/`)) {
    return normalizedTarget.slice(docDir.length + 1)
  }
  return normalizedTarget
}
const getNotebookSidecarPath = () => {
  const filePath = String(props.filePath || '').trim()
  if (!filePath) return ''
  return `${filePath}.cellmeta`
}
const stripNotebookOutputBlocks = (markdown = '') => {
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const kept = []
  let insideOutput = false
  for (const line of lines) {
    const text = String(line || '').trim()
    if (/^<!--\s+guaxi:pyout:.*:start\s+-->$/i.test(text)) {
      insideOutput = true
      continue
    }
    if (insideOutput && /^<!--\s+guaxi:pyout:.*:end\s+-->$/i.test(text)) {
      insideOutput = false
      continue
    }
    if (!insideOutput) {
      kept.push(line)
    }
  }
  return kept.join('\n')
}
const computeNotebookDigest = (text = '') => {
  const normalized = String(text || '')
  let hash = 2166136261
  for (let index = 0; index < normalized.length; index += 1) {
    hash ^= normalized.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `nb-${(hash >>> 0).toString(16)}-${normalized.length}`
}
const normalizeNotebookSourceForSidecar = (markdown = '') => {
  return stripNotebookOutputBlocks(normalizeNotebookMarkdownText(markdown))
}
const getNotebookCellStableId = (cell) => String(cell?.meta?.id || '').trim()
const getNotebookCellContentDigest = (cell) => {
  if (cell?.type === 'python') {
    return computeNotebookDigest(String(cell?.code || ''))
  }
  if (cell?.type === 'math') {
    return computeNotebookDigest(String(cell?.expression || ''))
  }
  return computeNotebookDigest(String(cell?.content || ''))
}
const createNotebookMathCellId = (cell) => {
  const cellKey = String(cell?.key || '').trim()
  const fileKey = computeNotebookDigest(String(props.filePath || '__guaxi_notebook__'))
  return cellKey ? `math::${fileKey}::${cellKey}` : ''
}
const isPythonCellSnapshotFresh = (cell, snapshot) => {
  const currentCodeDigest = getNotebookCellContentDigest(cell)
  const snapshotCodeDigest = String(snapshot?.codeDigest || '').trim()
  return !!currentCodeDigest && !!snapshotCodeDigest && currentCodeDigest === snapshotCodeDigest
}
const buildStalePythonCellSnapshot = (cell, snapshot = {}, reason = 'unknown') => {
  const nextSnapshot = {
    codeDigest: getNotebookCellContentDigest(cell),
    outputText: '',
    display: null,
    dataframe: null,
    outputs: [],
    result: null,
    // 设计说明：
    // 1. Python Notebook 这里使用的是“文件级持久会话”，代码被编辑后，旧输出确实应该失效；
    // 2. 但会话里的变量并不会因为用户改了一行代码就自动从 Python 进程里消失，
    //    否则就会出现“cell 中还能 print 出来，右侧实验室却显示 0 个变量”的割裂体验；
    // 3. 因此 stale snapshot 只清空依赖当前代码版本的输出，不再顺手清空 sessionVars。
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    lab: snapshot?.lab && typeof snapshot.lab === 'object'
      ? {
        ...snapshot.lab,
        imports: Array.isArray(snapshot?.lab?.imports) ? [...snapshot.lab.imports] : [],
      }
      : {
        imports: [],
      },
    meta: snapshot?.meta && typeof snapshot.meta === 'object' ? { ...snapshot.meta } : {},
  }
  logger.info('[Notebook] invalidate stale python snapshot', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId: getNotebookCellStableId(cell),
    reason,
    previousCodeDigest: String(snapshot?.codeDigest || ''),
    nextCodeDigest: nextSnapshot.codeDigest,
    preservedSessionVarCount: nextSnapshot.sessionVars.length,
  })
  return nextSnapshot
}
const buildNotebookCellRelation = (cells = [], index = 0) => {
  const current = cells[index] || null
  const previous = index > 0 ? cells[index - 1] : null
  const next = index + 1 < cells.length ? cells[index + 1] : null
  return {
    index,
    currentDigest: current ? getNotebookCellContentDigest(current) : '',
    previousType: String(previous?.type || ''),
    previousStableId: previous?.type === 'python' ? getNotebookCellStableId(previous) : '',
    previousDigest: previous ? getNotebookCellContentDigest(previous) : '',
    nextType: String(next?.type || ''),
    nextStableId: next?.type === 'python' ? getNotebookCellStableId(next) : '',
    nextDigest: next ? getNotebookCellContentDigest(next) : '',
  }
}
const cloneNotebookCellsForHistory = (cells = []) => {
  return normalizeNotebookCells(cells).map((cell) => {
    if (cell?.type === 'python') {
      return createNotebookPythonCell({
        key: cell.key,
        code: String(cell.code || ''),
        meta: {
          ...(cell.meta || {}),
        },
      })
    }
    if (cell?.type === 'math') {
      return createNotebookMathCell(String(cell?.expression || ''), {
        key: cell?.key,
        sourceType: cell?.sourceType,
        fenceLanguage: cell?.fenceLanguage,
        meta: {
          ...(cell.meta || {}),
        },
      })
    }
    return createNotebookMarkdownCell(String(cell?.content || ''), { key: cell?.key })
  })
}
const canUndoNotebookHistory = computed(() => notebookHistoryIndex.value > 0)
const canRedoNotebookHistory = computed(
  () => notebookHistoryIndex.value >= 0 && notebookHistoryIndex.value < notebookHistoryEntries.value.length - 1,
)
const serializeNotebookCellsForSidecar = (cells = []) => {
  const normalizedCells = normalizeNotebookCells(cells)
  return normalizedCells.map((cell, index) => {
    const relation = buildNotebookCellRelation(normalizedCells, index)
    if (cell?.type === 'python') {
      return {
        type: 'python',
        key: String(cell.key || ''),
        code: String(cell.code || ''),
        meta: {
          ...(cell.meta || {}),
        },
        relation,
      }
    }
    if (cell?.type === 'math') {
      return {
        type: 'math',
        key: String(cell?.key || ''),
        expression: String(cell?.expression || ''),
        sourceType: String(cell?.sourceType || 'marker'),
        fenceLanguage: String(cell?.fenceLanguage || 'math'),
        meta: cell?.meta && typeof cell.meta === 'object' ? { ...cell.meta } : {},
        relation,
      }
    }
    return {
      type: 'markdown',
      key: String(cell?.key || ''),
      content: String(cell?.content || ''),
      relation,
    }
  })
}
const serializeNotebookSnapshotsForSidecar = (cells = []) => {
  const snapshots = {}
  decorateNotebookCells(cells).forEach((cell) => {
    if (cell?.type !== 'python' && cell?.type !== 'math') return
    const cellId = String(cell?.cellId || (cell?.type === 'math' ? createNotebookMathCellId(cell) : ''))
    const stableId = getNotebookCellStableId(cell)
    const snapshotKey = stableId || cellId
    if (!snapshotKey) return
    const snapshot = getCellSnapshot(cell)
    snapshots[snapshotKey] = {
      stableId,
      cellId,
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: String(snapshot?.outputText || ''),
      display: snapshot?.display || null,
      dataframe: snapshot?.dataframe || null,
      outputs: Array.isArray(snapshot?.outputs) ? snapshot.outputs : [],
      // 设计说明：
      // 1. 会话变量只代表当前 Python 进程里的内存对象，关闭文件/窗口后这些对象不应从 sidecar 复活。
      // 2. sidecar 只负责恢复可写回的输出模块和 UI 折叠状态，避免用户重新打开文件时看到已经不存在的变量。
      // 3. 运行、导入、工作台保存等真实 Python 会话动作仍会在当前内存里重新写入 sessionVars。
      sessionVars: [],
      lab: snapshot?.lab && typeof snapshot.lab === 'object'
        ? {
          ...snapshot.lab,
          imports: Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
        }
        : {
          imports: [],
        },
      meta: snapshot?.meta || {},
    }
  })
  return snapshots
}
const applyNotebookSidecarSnapshots = (cells = [], snapshots = {}) => {
  const nextSnapshots = {}
  let restoredCount = 0
  let skippedCount = 0
  const executableCells = decorateNotebookCells(cells).filter((cell) => cell?.type === 'python' || cell?.type === 'math')
  executableCells.forEach((cell) => {
    if (cell?.type !== 'python' && cell?.type !== 'math') return
    const cellId = String(cell?.cellId || '')
    const stableId = getNotebookCellStableId(cell)
    if (!cellId && !stableId) return
    const persisted = snapshots && typeof snapshots === 'object'
      ? snapshots[stableId] || snapshots[cellId] || null
      : null
    if (!persisted || typeof persisted !== 'object') return
    const snapshotCodeDigest = String(persisted.codeDigest || '').trim()
    const currentCodeDigest = getNotebookCellContentDigest(cell)
    if (!snapshotCodeDigest || snapshotCodeDigest !== currentCodeDigest) {
      skippedCount += 1
      logger.info('[Notebook] skip stale sidecar snapshot', {
        cellId,
        stableId,
        snapshotCodeDigest,
        currentCodeDigest,
      })
      return
    }
    nextSnapshots[cellId] = {
      codeDigest: snapshotCodeDigest,
      outputText: String(persisted.outputText || ''),
      display: persisted.display || null,
      dataframe: persisted.dataframe || null,
      outputs: Array.isArray(persisted.outputs) ? persisted.outputs : [],
      // 设计说明：
      // 1. Python Cell 输出可以从 sidecar 恢复，因为它已经是文档视图状态。
      // 2. 会话变量是 Python worker 内存状态，重开文件后 worker 中对象可能已经销毁。
      // 3. 因此恢复快照时显式清空 sessionVars，只允许真实运行/导入后重新显示。
      sessionVars: [],
      lab: persisted?.lab && typeof persisted.lab === 'object'
        ? {
          ...persisted.lab,
          imports: Array.isArray(persisted?.lab?.imports) ? persisted.lab.imports : [],
        }
        : {
          imports: [],
        },
      meta: persisted.meta && typeof persisted.meta === 'object' ? persisted.meta : {},
    }
    restoredCount += 1
  })
  replacePythonCellSnapshots(props.filePath, nextSnapshots)
  logger.info('[Notebook] applied sidecar executable snapshots', {
    cellCount: executableCells.length,
    restoredCount,
    skippedCount,
    snapshotCount: Object.keys(snapshots && typeof snapshots === 'object' ? snapshots : {}).length,
    restoredSessionVarCount: 0,
  })
}

// 设计说明：
// 1. sidecar 不再直接替代当前 Markdown 扫描结果，而是作为“探索模式状态层”叠加回最新扫描结果。
// 2. 这样即使用户在源 Markdown 中新增/删除/重排内容，我们也能先重建当前 cell 结构，再把已知状态映射回来。
// 3. Python Cell 优先通过稳定 meta.id 对齐；若外部编辑导致稳定 id 丢失，再退化到代码摘要匹配。
// 4. Markdown Cell 始终以源文件最新内容为准，避免旧 sidecar 覆盖正文变更。
const reconcileNotebookCellsWithSidecar = (markdownCells = [], sidecar = null) => {
  const normalizedMarkdownCells = normalizeNotebookCells(markdownCells)
  const sidecarCells = Array.isArray(sidecar?.cells) ? normalizeNotebookCells(sidecar.cells) : []
  if (!sidecarCells.length) return normalizedMarkdownCells

  const sidecarPythonByStableId = new Map()
  const sidecarPythonByCodeDigest = new Map()
  // 设计说明：数学块与 Python Cell 对称 —— 源 Markdown 只保留引用块，表达式存 sidecar。
  // 这里为数学块单独建索引（按稳定 id / 表达式 digest），用于加载时回填表达式。
  const sidecarMathByStableId = new Map()
  const sidecarMathByDigest = new Map()
  sidecarCells.forEach((cell) => {
    const stableId = getNotebookCellStableId(cell)
    const contentDigest = getNotebookCellContentDigest(cell)
    if (cell?.type === 'python') {
      if (stableId) {
        sidecarPythonByStableId.set(stableId, cell)
      }
      if (contentDigest) {
        const bucket = sidecarPythonByCodeDigest.get(contentDigest) || []
        bucket.push(cell)
        sidecarPythonByCodeDigest.set(contentDigest, bucket)
      }
      return
    }
    if (cell?.type === 'math') {
      if (stableId) {
        sidecarMathByStableId.set(stableId, cell)
      }
      if (contentDigest) {
        const bucket = sidecarMathByDigest.get(contentDigest) || []
        bucket.push(cell)
        sidecarMathByDigest.set(contentDigest, bucket)
      }
    }
  })

  let stableIdMatchedCount = 0
  let codeDigestMatchedCount = 0
  let mathRestoredCount = 0
  const reconciledCells = normalizedMarkdownCells.map((cell) => {
    if (cell?.type === 'math') {
      const stableId = getNotebookCellStableId(cell)
      const currentStorage = String(cell?.meta?.storage || '').trim()
      const currentExpression = String(cell?.expression || '')
      // 只有引用块（storage=cellmeta 且正文为空）才需要从 sidecar 回填，
      // 旧式内联块自带表达式，保持原样即可。
      if (currentStorage !== 'cellmeta' || currentExpression) {
        return cell
      }
      let persistedCell = stableId ? sidecarMathByStableId.get(stableId) : null
      if (!persistedCell) {
        const digestCandidates = sidecarMathByDigest.get(getNotebookCellContentDigest(cell)) || []
        if (digestCandidates.length === 1) {
          persistedCell = digestCandidates[0]
        }
      }
      const persistedExpression = String(persistedCell?.expression || '')
      if (!persistedExpression) {
        return cell
      }
      mathRestoredCount += 1
      logger.info('[Notebook] restored math cell expression from sidecar reference block', {
        stableId,
        expressionLength: persistedExpression.length,
      })
      return createNotebookMathCell(persistedExpression, {
        ...cell,
        expression: persistedExpression,
        key: cell.key,
        meta: {
          ...(persistedCell.meta || {}),
          ...(cell.meta || {}),
          id: getNotebookCellStableId(persistedCell) || stableId,
        },
      })
    }
    if (cell?.type !== 'python') {
      return cell
    }
    const stableId = getNotebookCellStableId(cell)
    let persistedCell = stableId ? sidecarPythonByStableId.get(stableId) : null
    let matchStrategy = persistedCell ? 'stable-id' : 'none'
    if (!persistedCell) {
      const codeDigest = getNotebookCellContentDigest(cell)
      const digestCandidates = codeDigest ? sidecarPythonByCodeDigest.get(codeDigest) || [] : []
      if (digestCandidates.length === 1) {
        persistedCell = digestCandidates[0]
        matchStrategy = 'code-digest'
      }
    }
    if (!persistedCell) {
      return cell
    }
    const currentStorage = String(cell?.meta?.storage || '').trim()
    const currentCode = String(cell?.code || '')
    const persistedCode = String(persistedCell?.code || '')
    const shouldRestoreSidecarCode =
      cell?.type === 'python' &&
      currentStorage === 'cellmeta' &&
      !currentCode &&
      !!persistedCode
    if (matchStrategy === 'stable-id') {
      stableIdMatchedCount += 1
    } else if (matchStrategy === 'code-digest') {
      codeDigestMatchedCount += 1
    }
    if (shouldRestoreSidecarCode) {
      logger.info('[Notebook] restored python cell code from sidecar reference block', {
        stableId,
        codeLength: persistedCode.length,
        matchStrategy,
      })
    }
    return createNotebookPythonCell({
      ...cell,
      code: shouldRestoreSidecarCode ? persistedCode : String(cell.code || ''),
      meta: {
        ...(persistedCell.meta || {}),
        ...(cell.meta || {}),
        id: getNotebookCellStableId(persistedCell) || stableId,
      },
    })
  })
  logger.info('[Notebook] reconciled notebook cells with sidecar', {
    markdownCellCount: normalizedMarkdownCells.filter((cell) => cell?.type === 'markdown').length,
    pythonCellCount: normalizedMarkdownCells.filter((cell) => cell?.type === 'python').length,
    mathCellCount: normalizedMarkdownCells.filter((cell) => cell?.type === 'math').length,
    stableIdMatchedCount,
    codeDigestMatchedCount,
    mathRestoredCount,
  })
  return reconciledCells
}
const guessExtensionFromMime = (mime, fallback = 'bin') => {
  const normalized = String(mime || '').trim().toLowerCase()
  if (normalized.includes('markdown')) return 'md'
  if (normalized.includes('svg')) return 'svg'
  if (normalized.includes('html')) return 'html'
  if (normalized.includes('json')) return 'json'
  if (normalized.includes('plain')) return 'txt'
  if (normalized.includes('png')) return 'png'
  if (normalized.includes('jpeg') || normalized.includes('jpg')) return 'jpg'
  if (normalized.includes('gif')) return 'gif'
  if (normalized.includes('webp')) return 'webp'
  if (normalized.includes('mp3') || normalized.includes('mpeg')) return 'mp3'
  if (normalized.includes('wav')) return 'wav'
  if (normalized.includes('ogg')) return 'ogg'
  if (normalized.includes('mp4')) return 'mp4'
  if (normalized.includes('webm')) return 'webm'
  return fallback
}

const getImageBedRoot = async () => {
  if (String(imageBedRootCache.value || '').trim()) {
    return String(imageBedRootCache.value || '').trim()
  }
  const root = String(await invoke('get_image_bed_root') || '').trim()
  imageBedRootCache.value = root
  return root
}

const buildResourceRepoTarget = (cellId, extension) => {
  const now = new Date()
  const year = String(now.getFullYear())
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  const stamp = [
    year,
    month,
    day,
    '_',
    String(now.getHours()).padStart(2, '0'),
    String(now.getMinutes()).padStart(2, '0'),
    String(now.getSeconds()).padStart(2, '0'),
    '_',
    String(now.getMilliseconds()).padStart(3, '0'),
  ].join('')
  const safeStem = getNotebookDocStem()
  const safeCellId = sanitizeFileSegment(cellId, 'cell')
  const safeExt = String(extension || 'bin').replace(/^\.+/, '') || 'bin'
  const fileName = `${safeStem}-${safeCellId}-${stamp}.${safeExt}`
  const relativePath = joinNotebookPath('images', year, month, day, fileName)
  return {
    fileName,
    relativePath,
  }
}

const registerResourceFile = async ({
  bytes,
  resourceType,
  extension,
  ownerFilePath,
  cellId,
  source,
}) => {
  const root = await getImageBedRoot()
  if (!root) {
    throw new Error('资源库根目录不可用')
  }
  const target = buildResourceRepoTarget(cellId, extension)
  const absolutePath = joinNotebookPath(root, target.relativePath)
  await writeBytesToPath(absolutePath, bytes, 'resource-repo-register')
  await invoke('add_resource', {
    resource: {
      resource_type: String(resourceType || 'file'),
      file_name: target.fileName,
      relative_path: target.relativePath,
      absolute_path: absolutePath,
      owner_file_path: String(ownerFilePath || ''),
      created_at: '',
      extra: JSON.stringify({
        source: source || 'python-cell-output',
        cell_id: String(cellId || ''),
      }),
    },
  })
  const url = `http://localhost:8888/markdown/api/bed/${encodeURIComponent(target.relativePath)}`
  logger.info('[Notebook] registered resource file for markdown writeback', {
    cellId: String(cellId || ''),
    resourceType: String(resourceType || 'file'),
    absolutePath,
    relativePath: target.relativePath,
    url,
  })
  return {
    url,
    relativePath: target.relativePath,
    absolutePath,
  }
}
const normalizeDisplayType = (display) => {
  return normalizePythonCellOutputDisplayType(display)
}
const getDisplaySourcePath = (display) => getPythonCellOutputDisplaySourcePath(display)
const getDisplayDataUrl = (display) => getPythonCellOutputDisplayDataUrl(display)

const decorateNotebookCells = (cells = []) => {
  return normalizeNotebookCells(cells).map((cell, index) => {
    if (cell.type === 'math') {
      return {
        ...cell,
        index,
        cellId: createNotebookMathCellId(cell),
      }
    }
    if (cell.type !== 'python') {
      return {
        ...cell,
        index,
      }
    }

    const block = parsePythonCellBlocks(serializeNotebookCells([cell]))[0] || null
    return {
      ...cell,
      index,
      block,
      cellId: createPythonCellId(props.filePath, {
        ...(block || {}),
        meta: cell.meta,
      }),
    }
  })
}

const buildNotebookSidecarPayload = (cells = [], markdown = '') => {
  const normalizedCells = decorateNotebookCells(cells)
  const sourceCore = normalizeNotebookSourceForSidecar(markdown)
  return {
    version: 4,
    filePath: String(props.filePath || ''),
    updatedAt: new Date().toISOString(),
    sourceDigest: computeNotebookDigest(sourceCore),
    cells: serializeNotebookCellsForSidecar(normalizedCells),
    pythonSnapshots: serializeNotebookSnapshotsForSidecar(normalizedCells),
  }
}

const persistNotebookSidecar = async (reason = 'unknown') => {
  const sidecarPath = getNotebookSidecarPath()
  if (!sidecarPath) return
  const payload = buildNotebookSidecarPayload(
    notebookCells.value,
    lastEmittedMarkdown.value || props.markdown,
  )
  await invoke('write_file_content', {
    path: sidecarPath,
    content: JSON.stringify(payload, null, 2),
  })
  logger.info('[Notebook] persisted sidecar file', {
    reason,
    sidecarPath,
    cellCount: Array.isArray(payload.cells) ? payload.cells.length : 0,
    snapshotCount: Object.keys(payload.pythonSnapshots || {}).length,
  })
}

const schedulePersistNotebookSidecar = (reason = 'unknown', delay = 220) => {
  if (notebookSidecarPersistTimer) {
    window.clearTimeout(notebookSidecarPersistTimer)
  }
  notebookSidecarPersistTimer = window.setTimeout(() => {
    notebookSidecarPersistTimer = 0
    persistNotebookSidecar(reason).catch((error) => {
      logger.warn('[Notebook] persist sidecar failed', {
        reason,
        sidecarPath: getNotebookSidecarPath(),
        error: String(error?.message || error),
      })
    })
  }, delay)
}

const loadNotebookSidecar = async (markdown, reason = 'unknown') => {
  const sidecarPath = getNotebookSidecarPath()
  if (!sidecarPath) return null
  try {
    const raw = await invoke('read_file_content', { path: sidecarPath })
    const parsed = JSON.parse(String(raw || '{}'))
    const cells = Array.isArray(parsed?.cells) ? parsed.cells : []
    const sourceDigest = String(parsed?.sourceDigest || '').trim()
    const currentDigest = computeNotebookDigest(normalizeNotebookSourceForSidecar(markdown))
    if (!cells.length) {
      logger.info('[Notebook] ignore empty sidecar payload', {
        reason,
        sidecarPath,
      })
      return null
    }
    const digestMatch = !!sourceDigest && sourceDigest === currentDigest
    logger.info('[Notebook] loaded sidecar file', {
      reason,
      sidecarPath,
      cellCount: cells.length,
      digestMatch,
      sourceDigest,
      currentDigest,
      snapshotCount: Object.keys(parsed?.pythonSnapshots || {}).length,
    })
    return {
      ...parsed,
      digestMatch,
      currentDigest,
    }
  } catch (error) {
    const messageText = String(error?.message || error || '')
    if (/not found|系统找不到指定的文件|cannot find the file/i.test(messageText)) {
      logger.debug('[Notebook] sidecar file does not exist yet', {
        reason,
        sidecarPath,
      })
      return null
    }
    logger.warn('[Notebook] failed to load sidecar file', {
      reason,
      sidecarPath,
      error: messageText,
    })
    return null
  }
}

// 设计说明：
// 1. 探索模式需要允许“空 cell”暂时存在，等待用户填写，因此不能在每次 props.markdown 更新时都重新解析覆盖本地 cell 状态。
// 2. 这里把 notebookCells 作为本地状态维护，只在外部文档真正发生变化时再重新同步，既保留空 cell，也能明显减少重复切分带来的卡顿。
const syncNotebookCellsFromMarkdown = async (markdown, reason = 'external-sync', { force = false } = {}) => {
  flushPendingNotebookMarkdown(`before-sync:${reason}`)
  const normalizedMarkdown = normalizeNotebookMarkdownText(markdown)
  if (!force && normalizedMarkdown === lastEmittedMarkdown.value) {
    logger.debug('[Notebook] skip notebook cells sync', {
      reason,
      markdownLength: normalizedMarkdown.length,
    })
    return
  }

  const loadVersion = notebookSidecarLoadVersion.value + 1
  notebookSidecarLoadVersion.value = loadVersion
  const sidecar = await loadNotebookSidecar(normalizedMarkdown, reason)
  if (loadVersion !== notebookSidecarLoadVersion.value) {
    logger.debug('[Notebook] skip stale sidecar hydration result', {
      reason,
      loadVersion,
      latestVersion: notebookSidecarLoadVersion.value,
    })
    return
  }
  let nextCells = []
  let sidecarDigestMatch = !!sidecar?.digestMatch
  let restoredFromSidecar = !!sidecar?.cells?.length
  try {
    const backendResult = await invoke('notebook_sync_from_markdown', {
      markdown: normalizedMarkdown,
      sidecarJson: sidecar ? JSON.stringify(sidecar) : null,
    })
    nextCells = decorateNotebookCells(Array.isArray(backendResult?.cells) ? backendResult.cells : [])
    sidecarDigestMatch = !!backendResult?.sidecar_digest_match
    restoredFromSidecar = !!backendResult?.restored_from_sidecar
    logger.info('[Notebook] synced notebook cells via backend command', {
      reason,
      cellCount: nextCells.length,
      sidecarDigestMatch,
      restoredFromSidecar,
    })
  } catch (error) {
    logger.warn('[Notebook] backend notebook sync failed, fallback to frontend parser', {
      reason,
      error: String(error?.message || error),
    })
    const scannedCells = createNotebookCellsFromMarkdown(normalizedMarkdown, {
      filePath: props.filePath,
    })
    nextCells = decorateNotebookCells(reconcileNotebookCellsWithSidecar(scannedCells, sidecar))
  }

  notebookCells.value = nextCells
  notebookHistoryEntries.value = [{
    signature: serializeNotebookCells(cloneNotebookCellsForHistory(nextCells)),
    cells: cloneNotebookCellsForHistory(nextCells),
  }]
  notebookHistoryIndex.value = 0
  notebookHistoryApplying.value = false
  applyNotebookSidecarSnapshots(nextCells, sidecar?.pythonSnapshots || {})
  lastEmittedMarkdown.value = normalizedMarkdown

  logger.info('[Notebook] sync notebook cells from markdown', {
    reason,
    markdownLength: normalizedMarkdown.length,
    cellCount: nextCells.length,
    restoredFromSidecar,
    sidecarDigestMatch,
  })
  if (sidecar && !sidecarDigestMatch) {
    schedulePersistNotebookSidecar('rescan-source-markdown', 40)
  }
}

const getNotebookBasePath = () => {
  const filePath = String(props.filePath || '').trim()
  if (!filePath) return ''
  const normalized = filePath.replace(/\\/g, '/')
  const lastSlashIndex = normalized.lastIndexOf('/')
  return lastSlashIndex >= 0 ? normalized.slice(0, lastSlashIndex) : ''
}

const buildMarkdownRenderOptions = () => ({
  preserveOriginal: false,
  mermaid: true,
  math: true,
  codeHighlight: true,
  buttons: false,
  pythonCells: false,
  codeCells: false,
  theme: props.theme,
  basePath: getNotebookBasePath(),
})

const getNotebookSerializedLineCount = (content) => String(content || '').replace(/\r\n?/g, '\n').split('\n').length

const buildNotebookCellStartLineMap = (cells = []) => {
  const normalizedCells = Array.isArray(cells) ? cells : []
  const lineMap = new Map()
  let currentLine = 1
  let previousChunk = ''

  normalizedCells.forEach((cell) => {
    const cellKey = String(cell?.key || '')
    if (!cellKey) return

    const cellContent = cell?.type === 'python'
      ? serializeNotebookCells([cell])
      : String(cell?.content || '')

    // 设计说明：
    // 1. 探索者模式的大纲跳转仍然要对应“序列化后的 markdown 行号”，这样右侧大纲、顶部面包屑与原始文档保持同一套定位语义。
    // 2. 这里复用 notebook 序列化时的拼接规则：当相邻两个 chunk 之间没有显式换行时，补一个空行分隔。
    const needsSeparator =
      previousChunk &&
      !previousChunk.endsWith('\n') &&
      !cellContent.startsWith('\n')

    if (needsSeparator) {
      currentLine += 2
    }

    lineMap.set(cellKey, currentLine)
    currentLine += getNotebookSerializedLineCount(cellContent) - 1
    previousChunk = cellContent
  })

  return lineMap
}

const notebookOutlineEntries = computed(() => {
  const result = []
  const cellStartLineMap = buildNotebookCellStartLineMap(notebookCells.value)

  for (const cell of Array.isArray(notebookCells.value) ? notebookCells.value : []) {
    const cellKey = String(cell.key || '')
    const baseLine = Number(cellStartLineMap.get(cellKey) || 1)
    if (cell?.type === 'python') {
      result.push({
        level: 2,
        text: getPythonCellOutlineTitle(cell),
        line: baseLine,
        cellKey,
        outlineId: `nb-outline-python-${cellKey}`,
        kind: 'python-cell',
      })
      continue
    }

    if (cell?.type === 'math') {
      result.push({
        level: 2,
        text: getMathCellOutlineTitle(cell),
        line: baseLine,
        cellKey,
        outlineId: `nb-outline-math-${cellKey}`,
        kind: 'math-cell',
      })
      continue
    }

    if (cell?.type !== 'markdown') continue

    const content = String(cell.content || '')
    const outlineSource = maskMathCellBlocksForMarkdown(maskPythonCellBlocksForMarkdown(content))
    const tokens = notebookOutlineMarkdownParser.parse(outlineSource, {})
    let cellHeadingIndex = 0

    tokens.forEach((token, tokenIndex) => {
      if (token.type !== 'heading_open') return

      const level = Number(String(token.tag || '').slice(1))
      const nextToken = tokens[tokenIndex + 1]
      const text = String(nextToken?.content || '').trim()
      if (!Number.isFinite(level) || level <= 0 || !text) return

      const line = token.map ? baseLine + Number(token.map[0] || 0) : baseLine
      result.push({
        level,
        text,
        line,
        cellKey,
        outlineId: `nb-outline-${cellKey}-${cellHeadingIndex}`,
        kind: 'heading',
      })
      cellHeadingIndex += 1
    })
  }

  return result
})

const notebookOutlineEntriesByCellKey = computed(() => {
  const bucket = new Map()
  for (const entry of notebookOutlineEntries.value) {
    const cellKey = String(entry?.cellKey || '')
    if (!cellKey) continue
    const current = bucket.get(cellKey) || []
    current.push(entry)
    bucket.set(cellKey, current)
  }
  return bucket
})

const createNotebookOutlinePayload = () => {
  return notebookOutlineEntries.value.map((item) => ({
    level: Number(item.level || 0),
    text: String(item.text || ''),
    line: Number(item.line || 0),
    kind: String(item.kind || 'heading'),
    cellKey: String(item.cellKey || ''),
  }))
}

const getNotebookScrollHost = () => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return null

  const candidates = [
    root.parentElement,
    root,
  ].filter((item) => item instanceof HTMLElement)

  return candidates.find((item) => item.scrollHeight > item.clientHeight + 1) || candidates[0] || null
}

const getNotebookCellStartLine = (cellKey) => {
  const normalizedKey = String(cellKey || '').trim()
  if (!normalizedKey) return 0
  const outlineEntry = notebookOutlineEntries.value.find((item) => String(item?.cellKey || '') === normalizedKey)
  if (outlineEntry && Number(outlineEntry.line || 0) > 0) return Number(outlineEntry.line || 0)
  return Number(buildNotebookCellStartLineMap(notebookCells.value).get(normalizedKey) || 0)
}

const getNotebookReadingLocationFromElement = (element, scrollHost = getNotebookScrollHost()) => {
  const target = element instanceof HTMLElement ? element : null
  const cell = target?.closest?.('.guaxi-notebook__cell')
  const cellKey = String(cell?.getAttribute?.('data-cell-key') || '')
  const outlineLine = Number(target?.getAttribute?.('data-guaxi-outline-line') || 0)
  const lineNumber = Number.isFinite(outlineLine) && outlineLine > 0
    ? outlineLine
    : getNotebookCellStartLine(cellKey)
  const scrollTop = Number(scrollHost?.scrollTop)
  const scrollHeight = Number(scrollHost?.scrollHeight)
  const scrollRatio = scrollHeight > 0 && Number.isFinite(scrollTop) ? scrollTop / scrollHeight : 0
  return {
    path: String(props.filePath || ''),
    scrollTop: Number.isFinite(scrollTop) ? scrollTop : 0,
    scrollRatio,
    lineNumber: Number.isFinite(lineNumber) && lineNumber > 0 ? lineNumber : 1,
    cellKey,
  }
}

const getCurrentNotebookReadingLocation = () => {
  const root = notebookRootRef.value
  const scrollHost = getNotebookScrollHost()
  if (!(root instanceof HTMLElement) || !(scrollHost instanceof HTMLElement)) return null
  const path = String(props.filePath || '')
  if (!path) return null

  const hostRect = scrollHost.getBoundingClientRect()
  const candidates = Array.from(root.querySelectorAll(
    'h1[data-guaxi-outline-line], h2[data-guaxi-outline-line], h3[data-guaxi-outline-line], h4[data-guaxi-outline-line], h5[data-guaxi-outline-line], h6[data-guaxi-outline-line], .guaxi-notebook__cell',
  ) || [])
  let best = null
  let bestScore = Number.POSITIVE_INFINITY

  for (const candidate of candidates) {
    if (!(candidate instanceof HTMLElement)) continue
    const rect = candidate.getBoundingClientRect()
    const visible = rect.bottom > hostRect.top + 8 && rect.top < hostRect.bottom - 8
    const score = Math.abs(rect.top - hostRect.top) + (visible ? 0 : 100000)
    if (score < bestScore) {
      bestScore = score
      best = candidate
    }
  }

  return getNotebookReadingLocationFromElement(best || root, scrollHost)
}

const isSameNotebookReadingLocation = (a, b) => {
  if (!a || !b) return false
  if (String(a.path || '') !== String(b.path || '')) return false
  const aTop = Number(a.scrollTop)
  const bTop = Number(b.scrollTop)
  if (Number.isFinite(aTop) && Number.isFinite(bTop)) {
    return Math.round(aTop / 8) === Math.round(bTop / 8)
  }
  const aLine = Number(a.lineNumber || 0)
  const bLine = Number(b.lineNumber || 0)
  return aLine > 0 && aLine === bLine && String(a.cellKey || '') === String(b.cellKey || '')
}

const recordNotebookReadingLocation = (reason = 'unknown', overrideLocation = null) => {
  if (suppressNotebookReadingLocationRecording) return false
  const location = overrideLocation || getCurrentNotebookReadingLocation()
  if (!location) return false

  const now = Date.now()
  const scrollTop = Number(location.scrollTop)
  const scrollTopKey = Number.isFinite(scrollTop) ? Math.round(scrollTop / 8) : 0
  const key = `${location.path}:${scrollTopKey}:${location.cellKey || ''}`
  if (key === lastRecordedNotebookReadingLocationKey && now - lastRecordedNotebookReadingLocationAt < 350) {
    return false
  }

  const back = notebookReadingLocationBackStack.value
  const last = back[back.length - 1]
  if (last && isSameNotebookReadingLocation(last, location)) {
    lastRecordedNotebookReadingLocationKey = key
    lastRecordedNotebookReadingLocationAt = now
    return false
  }

  back.push({
    ...location,
    scrollTop: Number.isFinite(scrollTop) ? scrollTop : 0,
    scrollTopKey,
    ts: now,
    reason: String(reason || ''),
  })
  if (back.length > maxNotebookReadingLocationHistory) back.shift()
  notebookReadingLocationForwardStack.value = []
  lastRecordedNotebookReadingLocationKey = key
  lastRecordedNotebookReadingLocationAt = now
  logger.info('[NotebookNav] recorded reading location', {
    reason,
    path: location.path,
    lineNumber: location.lineNumber,
    cellKey: location.cellKey,
    historyLength: back.length,
  })
  return true
}

const goToNotebookReadingLocation = async (location, { source = 'shortcut' } = {}) => {
  if (!location) return false
  const scrollHost = getNotebookScrollHost()
  const root = notebookRootRef.value
  if (!(scrollHost instanceof HTMLElement) || !(root instanceof HTMLElement)) return false
  if (String(location.path || '') && String(location.path || '') !== String(props.filePath || '')) return false

  suppressNotebookReadingLocationRecording = true
  try {
    await nextTick()
    const scrollTop = Number(location.scrollTop)
    if (Number.isFinite(scrollTop)) {
      const maxTop = Math.max(0, scrollHost.scrollHeight - scrollHost.clientHeight)
      scrollHost.scrollTop = Math.min(Math.max(0, scrollTop), maxTop)
    } else if (String(location.cellKey || '').trim()) {
      const target = root.querySelector(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(String(location.cellKey || ''))}"]`)
      if (target instanceof HTMLElement) {
        target.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
      }
    } else if (Number(location.lineNumber || 0) > 0) {
      jumpToNotebookOutlineLine(location.lineNumber, { behavior: 'smooth', recordHistory: false })
    }

    logger.info('[NotebookNav] applied reading location', {
      source,
      path: String(location.path || ''),
      lineNumber: Number(location.lineNumber || 0),
      cellKey: String(location.cellKey || ''),
      scrollTop: Number.isFinite(scrollTop) ? scrollTop : 'na',
    })
    return true
  } finally {
    window.setTimeout(() => {
      suppressNotebookReadingLocationRecording = false
    }, 0)
  }
}

// 设计说明（跨模式 / 跨文件保留探索者阅读位置）：
// 1. 探索者视图由父组件用 v-if 挂载，每次切换模式都会销毁重建，内部的前进/后退栈会一起清空，
//    所以“当前阅读位置”必须交给父组件按文件缓存，这里只负责读取与应用。
// 2. 重建后 cell 是渐进渲染的（KaTeX/图片/代码高亮都会继续增高文档），一次性写 scrollTop 会被
//    当时还偏小的 scrollHeight 夹断，表现为“只恢复了一点点”甚至回到开头，因此需要多帧重试。
// 3. 切换文件时 cell 还要先经过一次异步解析（sidecar + 后端 notebook_sync_from_markdown），
//    在此期间 DOM 上挂的仍是上一份文档，可能“恰好”也能滚到目标位置。所以只有
//    “已落位 + 文档高度连续多帧不再变化”才算真正成功，否则继续等下一帧重试。
const NOTEBOOK_READING_LOCATION_STABLE_FRAMES = 2
const applyNotebookReadingLocationWithRetry = async (location, reason = 'external-apply', attempts = 10) => {
  const targetTop = Number(location?.scrollTop)
  let lastScrollHeight = -1
  let stableCount = 0
  let everReached = false

  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const applied = await goToNotebookReadingLocation(location, { source: `${reason}#${attempt}` })
    if (!applied) return false
    if (!Number.isFinite(targetTop) || targetTop <= 0) return true

    const host = getNotebookScrollHost()
    if (!(host instanceof HTMLElement)) return false

    const reached = Math.abs(host.scrollTop - targetTop) <= 2
    everReached = everReached || reached
    stableCount = host.scrollHeight === lastScrollHeight ? stableCount + 1 : 0
    lastScrollHeight = host.scrollHeight
    if (reached && stableCount >= NOTEBOOK_READING_LOCATION_STABLE_FRAMES) return true

    await new Promise((resolve) => {
      window.requestAnimationFrame(() => window.setTimeout(resolve, 40))
    })
  }

  logger.warn('[NotebookNav] reading location not fully restored after retries', {
    reason,
    attempts,
    targetTop: Number.isFinite(targetTop) ? Math.round(targetTop) : 'na',
    actualTop: Math.round(Number(getNotebookScrollHost()?.scrollTop || 0)),
  })
  return everReached
}

const navigateToPreviousNotebookReadingLocation = async () => {
  const current = getCurrentNotebookReadingLocation()
  if (!current) return false

  const back = notebookReadingLocationBackStack.value
  const forward = notebookReadingLocationForwardStack.value
  const last = back[back.length - 1]
  if (!last || !isSameNotebookReadingLocation(last, current)) {
    back.push({ ...current, ts: Date.now(), reason: 'implicit-current' })
    if (back.length > maxNotebookReadingLocationHistory) back.shift()
  }

  if (back.length < 2) {
    message.info('没有上一个探索者阅读位置')
    return false
  }

  const from = back.pop()
  forward.push(from)
  const target = back[back.length - 1]
  return goToNotebookReadingLocation(target, { source: 'alt-left' })
}

const navigateToNextNotebookReadingLocation = async () => {
  const forward = notebookReadingLocationForwardStack.value
  if (!forward.length) {
    message.info('没有下一个探索者阅读位置')
    return false
  }

  const target = forward.pop()
  const back = notebookReadingLocationBackStack.value
  const last = back[back.length - 1]
  if (!last || !isSameNotebookReadingLocation(last, target)) {
    back.push(target)
    if (back.length > maxNotebookReadingLocationHistory) back.shift()
  }
  return goToNotebookReadingLocation(target, { source: 'alt-right' })
}

// 设计说明：
// 1. 探索者模式是按 Cell 分段渲染的，markdown-it 的 TOC 插件拿不到“整篇文档”的标题上下文，
//    因此不能直接复用单 Cell 的目录输出。
// 2. 这里显式识别 [toc] / [[toc]] / [目录] 这类占位标记，再用 notebookOutlineEntries 汇总出的整篇标题树生成目录 HTML。
// 3. 目录中的锚点直接指向我们自己写入 heading 的 id，保证探索者模式里的目录跳转和右侧大纲共用同一批标题节点。
const NOTEBOOK_TOC_MARKER_RE = /^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/im

const hasNotebookTocMarker = (markdown = '') => NOTEBOOK_TOC_MARKER_RE.test(String(markdown || ''))

const isNotebookTocMarkerLine = (line = '') => /^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/i.test(String(line || ''))

const splitNotebookMarkdownByTocMarkers = (markdown = '') => {
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const parts = []
  let chunkStart = 0
  let fenceState = { active: false, char: '', length: 0 }

  const pushMarkdownChunk = (startIndex, endExclusive) => {
    const chunk = lines.slice(startIndex, endExclusive).join('\n')
    if (chunk.trim()) {
      parts.push({
        type: 'markdown',
        content: chunk,
      })
    }
  }

  lines.forEach((line, index) => {
    const textLine = String(line || '')
    if (!fenceState.active && isNotebookTocMarkerLine(textLine)) {
      pushMarkdownChunk(chunkStart, index)
      parts.push({
        type: 'toc',
        content: textLine,
      })
      chunkStart = index + 1
      return
    }

    const fenceMatch = textLine.match(/^\s*(`{3,}|~{3,})/)
    if (!fenceMatch) return

    const fenceText = String(fenceMatch[1] || '')
    const fenceChar = fenceText[0] || ''
    const fenceLength = fenceText.length
    if (!fenceState.active) {
      fenceState = { active: true, char: fenceChar, length: fenceLength }
      return
    }
    if (fenceState.char === fenceChar && fenceLength >= fenceState.length) {
      fenceState = { active: false, char: '', length: 0 }
    }
  })

  pushMarkdownChunk(chunkStart, lines.length)
  return parts
}

// 设计说明：
// 1. `[TOC]` 是一个行级占位符，不应该吞掉同一个 Markdown Cell 中的其它正文。
// 2. 因此这里按行切分，只把真正的 TOC 标记行替换成探索者模式的整篇目录 HTML。
// 3. 切分时显式跳过 fenced code block，避免示例代码里的 `[TOC]` 被误当作目录占位。
const renderNotebookMarkdownHtmlWithSyntheticToc = async (markdownText, cellKey, reason = 'unknown') => {
  const parts = splitNotebookMarkdownByTocMarkers(markdownText)
  if (!parts.length) return ''

  const htmlParts = []
  let tocCount = 0
  let markdownChunkCount = 0
  for (const part of parts) {
    if (part.type === 'toc') {
      tocCount += 1
      htmlParts.push(buildNotebookTocHtml({ currentCellKey: cellKey }))
      continue
    }
    markdownChunkCount += 1
    htmlParts.push(await renderMarkdownHtml(part.content, `${reason}:toc-chunk:${markdownChunkCount}`))
  }

  logger.info('[Notebook] rendered markdown cell with inline synthetic toc', {
    reason,
    cellKey,
    tocCount,
    markdownChunkCount,
    filePath: props.filePath,
  })
  return htmlParts.join('\n')
}

// 设计说明：
// 1. 探索者模式此前直接把 markdown 原文送进 worker，`[[note]]` 这类 Obsidian wikilink 不会被 markdown-it 识别成 <a>。
// 2. 文件卡片提升依赖最终 HTML 中已经存在链接节点，因此这里先把 wikilink 统一预处理成内部链接协议。
// 3. 代码块内的内容必须保持原样，避免把示例代码中的 `[[...]]` 误转成真实链接。
const preprocessNotebookObsidianWikilinks = (content = '') => {
  const text = String(content || '')
  if (!text.includes('[[')) return text

  const lines = text.replace(/\r/g, '').split('\n')
  let inFence = false
  let fenceToken = ''

  const out = lines.map((line) => {
    const fence = line.match(/^\s*(```+|~~~+)/)
    if (fence) {
      const token = String(fence[1] || '')
      if (!inFence) {
        inFence = true
        fenceToken = token
      } else if (token.startsWith(fenceToken)) {
        inFence = false
        fenceToken = ''
      }
      return line
    }

    if (inFence || !line.includes('[[')) return line

    return line.replace(/\[\[([^\]]+?)\]\]/g, (matched, innerRaw) => {
      const inner = String(innerRaw || '').trim()
      if (!inner || /^(toc|目录)$/i.test(inner)) return matched

      const parts = inner.split('|')
      const target = String(parts[0] || '').trim()
      const alias = String(parts[1] || '').trim()

      let filePart = target
      let anchorPart = ''
      const hashIndex = target.indexOf('#')
      if (hashIndex >= 0) {
        filePart = target.slice(0, hashIndex).trim()
        anchorPart = target.slice(hashIndex + 1).trim()
      }

      const display = alias || anchorPart || filePart || target
      const safeText = display.replace(/([\[\]\\])/g, '\\$1')
      const href = `guaxi://wikilink?file=${encodeURIComponent(filePart)}&anchor=${encodeURIComponent(anchorPart)}`
      return `[${safeText}](${href})`
    })
  })

  return out.join('\n')
}

const buildNotebookMarkdownCacheKey = (content, options = {}) => {
  const markdownText = String(content || '')
  const theme = String(options.theme || props.theme || '')
  const filePath = normalizeNotebookFsPath(options.filePath || props.filePath || '')
  const basePath = normalizeNotebookFsPath(options.basePath || getNotebookBasePath())
  return `${theme}::${filePath}::${basePath}::${markdownText}`
}

const escapeNotebookHtml = (value) => String(value ?? '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const buildNotebookTocHtml = ({ currentCellKey = '' } = {}) => {
  const entries = Array.isArray(notebookOutlineEntries.value) ? notebookOutlineEntries.value : []
  const tocEntries = entries.filter((entry) => {
    const outlineId = String(entry?.outlineId || '').trim()
    if (!outlineId) return false
    // 设计说明：
    // 如果目录占位本身写在一个带标题的 markdown Cell 里，目录中不再回指当前占位 Cell 的标题，避免自引用。
    if (currentCellKey && String(entry?.cellKey || '') === String(currentCellKey || '')) return false
    return true
  })

  if (!tocEntries.length) {
    logger.info('[Notebook] build toc html skipped because no outline entries were found', {
      currentCellKey,
    })
    return `
      <nav class="guaxi-notebook__toc" data-guaxi-notebook-toc="true">
        <div class="guaxi-notebook__toc-title">目录</div>
        <div class="guaxi-notebook__toc-empty">当前文档还没有可用标题</div>
      </nav>
    `
  }

  const itemsHtml = tocEntries.map((entry) => {
    const level = Math.max(1, Number(entry?.level || 1))
    const outlineId = String(entry?.outlineId || '').trim()
    const text = escapeNotebookHtml(String(entry?.text || '').trim() || '未命名标题')
    const line = Math.max(1, Number(entry?.line || 1))
    return `
      <li class="guaxi-notebook__toc-item level-${level}">
        <a
          class="guaxi-notebook__toc-link"
          href="#${outlineId}"
          data-guaxi-outline-id="${outlineId}"
          data-guaxi-outline-line="${line}"
        >${text}</a>
      </li>
    `
  }).join('')

  logger.info('[Notebook] build toc html', {
    currentCellKey,
    headingCount: tocEntries.length,
  })

  return `
    <nav class="guaxi-notebook__toc" data-guaxi-notebook-toc="true">
      <div class="guaxi-notebook__toc-title">目录</div>
      <ol class="guaxi-notebook__toc-list">
        ${itemsHtml}
      </ol>
    </nav>
  `
}

const decorateNotebookHeadingAnchors = (html, cellKey) => {
  const rawHtml = String(html || '')
  if (!rawHtml.trim()) return ''

  const outlineEntries = notebookOutlineEntriesByCellKey.value.get(String(cellKey || '')) || []
  if (!outlineEntries.length) return rawHtml

  const container = document.createElement('div')
  container.innerHTML = rawHtml

  const headings = Array.from(container.querySelectorAll('h1, h2, h3, h4, h5, h6'))
  headings.forEach((headingNode, index) => {
    if (!(headingNode instanceof HTMLElement)) return
    const entry = outlineEntries[index]
    if (!entry) return
    const outlineId = String(entry.outlineId || '')
    headingNode.setAttribute('data-guaxi-outline-id', outlineId)
    headingNode.setAttribute('data-guaxi-outline-line', String(entry.line || ''))
    if (outlineId) {
      headingNode.id = outlineId
    }
  })

  return container.innerHTML
}

// 设计说明：
// 1. 目录 HTML 里的 `<a data-guaxi-outline-id="...">` 和真正的标题节点共享同一个 outlineId。
// 2. 如果直接用 `[data-guaxi-outline-id="..."]` 查询，命中的往往是目录链接本身，
//    结果滚动会落到目录区域，而不是对应 markdown Cell 内的标题。
// 3. 这里统一优先查找带同名 id / data 属性的 heading，必要时再回退到非目录容器中的任意元素。
const findNotebookHeadingTargetByOutlineId = (outlineId) => {
  const root = notebookRootRef.value
  const normalizedOutlineId = String(outlineId || '').trim()
  if (!(root instanceof HTMLElement) || !normalizedOutlineId) return null

  const escapedId = typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
    ? CSS.escape(normalizedOutlineId)
    : normalizedOutlineId.replace(/["\\]/g, '\\$&')
  const escapedAttrValue = normalizedOutlineId.replace(/"/g, '&quot;')

  const headingById = root.querySelector(`h1#${escapedId}, h2#${escapedId}, h3#${escapedId}, h4#${escapedId}, h5#${escapedId}, h6#${escapedId}`)
  if (headingById instanceof HTMLElement) return headingById

  const headingByData = root.querySelector(
    `h1[data-guaxi-outline-id="${escapedAttrValue}"], h2[data-guaxi-outline-id="${escapedAttrValue}"], h3[data-guaxi-outline-id="${escapedAttrValue}"], h4[data-guaxi-outline-id="${escapedAttrValue}"], h5[data-guaxi-outline-id="${escapedAttrValue}"], h6[data-guaxi-outline-id="${escapedAttrValue}"]`,
  )
  if (headingByData instanceof HTMLElement) return headingByData

  const nonTocTarget = Array.from(root.querySelectorAll(`[data-guaxi-outline-id="${escapedAttrValue}"]`)).find((node) => {
    return node instanceof HTMLElement && !node.closest('[data-guaxi-notebook-toc="true"]')
  })
  return nonTocTarget instanceof HTMLElement ? nonTocTarget : null
}

// 设计说明：
// 1. Worker 负责 markdown-it、代码高亮和目录/任务列表这类纯字符串计算，先把最耗时的一段移出 UI 线程。
// 2. Mermaid SVG 和数学公式节点整理还依赖 DOM，因此保留在主线程做轻量收尾。
// 3. 如果 Worker 初始化或运行失败，会自动回退到原先的主线程渲染，避免探索模式不可用。
const initNotebookMarkdownWorker = () => {
  if (notebookMarkdownWorkerRef.value) return notebookMarkdownWorkerRef.value
  try {
    const worker = new Worker(new URL('../../workers/notebookMarkdownRenderWorker.js', import.meta.url), {
      type: 'module',
    })
    worker.onmessage = (event) => {
      const payload = event?.data || {}
      const taskId = Number(payload?.id || 0)
      const task = notebookMarkdownWorkerPendingTasks.get(taskId)
      if (!task) return
      notebookMarkdownWorkerPendingTasks.delete(taskId)
      if (payload?.ok) {
        task.resolve(String(payload?.html || ''))
        return
      }
      task.reject(new Error(String(payload?.error || 'Notebook markdown worker failed')))
    }
    worker.onerror = (error) => {
      notebookMarkdownWorkerReady.value = false
      logger.error('[Notebook] markdown worker runtime error', {
        error: String(error?.message || error),
      })
    }
    notebookMarkdownWorkerRef.value = worker
    notebookMarkdownWorkerReady.value = true
    logger.info('[Notebook] markdown worker initialized')
    return worker
  } catch (error) {
    notebookMarkdownWorkerReady.value = false
    logger.warn('[Notebook] failed to initialize markdown worker, fallback to main thread', {
      error: String(error?.message || error),
    })
    return null
  }
}

const disposeNotebookMarkdownWorker = (reason = 'dispose') => {
  if (notebookMarkdownWorkerRef.value) {
    notebookMarkdownWorkerRef.value.terminate()
  }
  notebookMarkdownWorkerRef.value = null
  notebookMarkdownWorkerReady.value = false
  if (notebookMarkdownWorkerPendingTasks.size) {
    for (const [, task] of notebookMarkdownWorkerPendingTasks) {
      task.reject(new Error(`Notebook markdown worker disposed: ${reason}`))
    }
    notebookMarkdownWorkerPendingTasks.clear()
  }
  logger.info('[Notebook] markdown worker disposed', {
    reason,
  })
}

const renderMarkdownHtmlWithWorker = (content, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return Promise.resolve('')
  const worker = notebookMarkdownWorkerRef.value || initNotebookMarkdownWorker()
  if (!worker) {
    return Promise.reject(new Error('Notebook markdown worker unavailable'))
  }
  const taskId = ++notebookMarkdownWorkerTaskId
  return new Promise((resolve, reject) => {
    notebookMarkdownWorkerPendingTasks.set(taskId, { resolve, reject })
    worker.postMessage({
      id: taskId,
      content: markdownText,
      options: buildMarkdownRenderOptions(),
      reason,
    })
  })
}

const finalizeNotebookRenderedMath = (container) => {
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

const finalizeNotebookRenderedMermaid = async (container, options = {}, reason = 'unknown') => {
  if (!options?.mermaid) return
  const mermaidFontFamily = '"Trebuchet MS", "Noto Sans SC", "Microsoft YaHei", "PingFang SC", "Helvetica Neue", Arial, sans-serif'
  mermaid.initialize({
    startOnLoad: false,
    // 设计说明：
    // Mermaid 图表延续项目里既有的“始终亮色渲染”策略，
    // 避免在探索者模式里因为外层深浅主题切换而得到不同配色的图。
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

      const renderId = `guaxi-notebook-mermaid-${Date.now()}-${notebookMermaidRenderCounter++}`
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
          logger.debug('[Notebook] mermaid bindFunctions failed', {
            reason,
            error: String(bindError?.message || bindError),
          })
        }
      }
    } catch (error) {
      logger.error('[Notebook] finalize mermaid failed', {
        reason,
        error: String(error?.message || error),
      })
    }
  }
}

const finalizeNotebookRenderedHtml = async (html, reason = 'unknown') => {
  const rawHtml = String(html || '')
  if (!rawHtml.trim()) return ''
  const container = document.createElement('div')
  container.innerHTML = rawHtml
  const options = buildMarkdownRenderOptions()
  await finalizeNotebookRenderedMermaid(container, options, reason)
  if (options.math) {
    finalizeNotebookRenderedMath(container)
  }
  return container.innerHTML
}

// 设计说明：后端的 Cell 级索引是“不区分大小写的子串匹配”，理解不了 Aa / ab / .* 这三个选项。
// 一旦用户开了其中任意一个，就不能再用 Cell 级结果兜底——否则会出现
// “计数显示 1 of 1、但页面上一个高亮都没有”的自相矛盾。默认全关时行为与之前完全一致。
const notebookSearchCellFallbackUsable = computed(() => (
  !notebookSearchCaseSensitive.value
  && !notebookSearchWholeWord.value
  && !notebookSearchUseRegex.value
))

// 命中总数：DOM 命中优先（用户能直接看到的），没有 DOM 命中时回退到后端的 Cell 级命中，
// 这样 Python 输出、变量面板这类非 DOM 内容仍然是“搜得到”的。
const notebookSearchTotalMatchCount = computed(() => {
  const domCount = Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value.length : 0
  if (domCount) return domCount
  if (!notebookSearchCellFallbackUsable.value) return 0
  return Array.isArray(notebookSearchCellMatches.value) ? notebookSearchCellMatches.value.length : 0
})

const notebookSearchMatchKeySet = computed(() => {
  const keySet = new Set()
  // 开了 Aa / ab / .* 时不采信 Cell 级结果（后端不理解这些选项），否则会给不该高亮的 cell 描边。
  ;(notebookSearchCellFallbackUsable.value && Array.isArray(notebookSearchCellMatches.value)
    ? notebookSearchCellMatches.value
    : [])
    .map((item) => String(item?.cell_key || ''))
    .filter(Boolean)
    .forEach((key) => keySet.add(key))

  ;(Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : [])
    .map((node) => String(node?.closest?.('.guaxi-notebook__cell')?.getAttribute?.('data-cell-key') || ''))
    .filter(Boolean)
    .forEach((key) => keySet.add(key))

  return keySet
})

const activeNotebookSearchMatch = computed(() => {
  const domMatches = Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : []
  const domActive = domMatches[Number(notebookSearchActiveIndex.value || 0)] || null
  if (domActive instanceof HTMLElement) {
    return {
      cell_key: String(domActive.closest('.guaxi-notebook__cell')?.getAttribute('data-cell-key') || ''),
    }
  }

  return Array.isArray(notebookSearchCellMatches.value)
    ? notebookSearchCellMatches.value[Number(notebookSearchActiveIndex.value || 0)] || null
    : null
})

const isNotebookSearchMatchedCell = (cell) => {
  return notebookSearchMatchKeySet.value.has(String(cell?.key || ''))
}

const isNotebookSearchActiveCell = (cell) => {
  return String(activeNotebookSearchMatch.value?.cell_key || '') === String(cell?.key || '')
}


const resizeTextareaToContent = (target) => {
  if (!(target instanceof HTMLTextAreaElement)) return
  target.style.height = 'auto'
  target.style.overflowY = 'hidden'
  const nextHeight = Math.max(target.scrollHeight, 88)
  target.style.height = `${nextHeight}px`
}

const syncNotebookTextareaHeights = (reason = 'unknown') => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return
  root.querySelectorAll('textarea').forEach((node) => {
    resizeTextareaToContent(node)
  })
}

const clearNotebookSearchDomHighlights = ({ resetIndex = true } = {}) => {
  const root = notebookRootRef.value
  if (root instanceof HTMLElement) {
    clearSearchHighlights(root, SEARCH_HIT_CLASS)
  }

  notebookSearchDomMatches.value = []
  if (resetIndex) {
    notebookSearchActiveIndex.value = -1
  }
}

const clearNotebookSearchHighlights = () => {
  clearNotebookSearchDomHighlights()
  notebookSearchCellMatches.value = []
}

// 探索者里的“非内容”元素：cell 工具条、选区工具条、右上角操作按钮。
// 代码相关的 code / pre 一律参与搜索。
const NOTEBOOK_SEARCH_EXTRA_EXCLUDE = [
  '.guaxi-notebook__toolbar',
  '.guaxi-notebook__cell-toolbar',
  '.guaxi-notebook__selection-toolbar',
  '.guaxi-notebook__search-panel-shell',
].join(', ')

const notebookSearchOptions = () => ({
  caseSensitive: notebookSearchCaseSensitive.value,
  wholeWord: notebookSearchWholeWord.value,
  useRegex: notebookSearchUseRegex.value,
})

// 设计说明：
// 1. Explorer 模式的后端索引继续负责 Cell 级命中定位，避免 Python 输出、变量等非纯 DOM 内容失去可搜索性。
// 2. 前端额外补一层 DOM 文本高亮，让用户在可见内容里能直接看到关键词，而不是只看到 Cell 外轮廓。
// 3. 两层结果共用一套 active index，优先在 DOM 命中间跳转，没有 DOM 命中时再回退到 Cell 级跳转。
// 4. DOM 这层的遍历与切分交给共享的 domTextSearch，与预览/分屏模式保持完全一致的搜索能力。
const applyNotebookSearchDomHighlights = (reason = 'unknown') => {
  clearNotebookSearchDomHighlights({ resetIndex: false })

  const root = notebookRootRef.value
  const keyword = String(notebookSearchKeyword.value || '')
  if (!(root instanceof HTMLElement) || !keyword.trim()) {
    notebookSearchRegexInvalid.value = false
    if (!Array.isArray(notebookSearchCellMatches.value) || !notebookSearchCellMatches.value.length) {
      notebookSearchActiveIndex.value = -1
    }
    return
  }

  const options = notebookSearchOptions()
  notebookSearchRegexInvalid.value = !buildSearchRegex(keyword, options)

  const matches = applySearchHighlights(root, {
    keyword,
    options,
    hitClass: SEARCH_HIT_CLASS,
    extraExcludeSelector: NOTEBOOK_SEARCH_EXTRA_EXCLUDE,
  })

  notebookSearchDomMatches.value = matches
  logger.info('[Notebook] apply DOM search highlights', {
    reason,
    keywordLength: keyword.length,
    domMatchCount: matches.length,
    cellMatchCount: Array.isArray(notebookSearchCellMatches.value) ? notebookSearchCellMatches.value.length : 0,
  })

  if (matches.length) {
    const nextIndex = notebookSearchActiveIndex.value >= 0
      ? Math.min(notebookSearchActiveIndex.value, matches.length - 1)
      : 0
    focusNotebookSearchMatchAt(nextIndex, { scroll: false })
    return
  }

  const cellFallbackCount = notebookSearchCellFallbackUsable.value
    && Array.isArray(notebookSearchCellMatches.value)
    ? notebookSearchCellMatches.value.length
    : 0
  if (cellFallbackCount) {
    notebookSearchActiveIndex.value = Math.min(
      Math.max(0, Number(notebookSearchActiveIndex.value || 0)),
      cellFallbackCount - 1,
    )
    return
  }

  notebookSearchActiveIndex.value = -1
}

const getNotebookSearchIndexKey = () => {
  const filePath = String(props.filePath || '').trim()
  return filePath || '__guaxi_notebook_memory__'
}

const stripHtmlTags = (html = '') => {
  return String(html || '')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

const buildNotebookSearchDocuments = (cells = []) => {
  return (Array.isArray(cells) ? cells : []).map((cell) => {
    if (cell?.type === 'python') {
      const snapshot = getCellSnapshot(cell)
      const outputTexts = getCellOutputModules(cell)
        .map((outputModule) => buildOutputModuleSearchText(outputModule))
        .filter(Boolean)
      const sessionText = Array.isArray(snapshot?.sessionVars)
        ? snapshot.sessionVars
          .map((item) => `${item?.name || ''} ${item?.value_type || item?.type || ''} ${item?.preview || ''}`.trim())
          .join('\n')
        : ''
      const parts = [
        String(cell?.meta?.title || ''),
        String(cell?.code || ''),
        ...outputTexts,
        sessionText,
      ].filter(Boolean)
      const searchText = parts.join('\n\n')
      return {
        cell_key: String(cell?.key || ''),
        cell_type: 'python',
        digest: computeNotebookDigest(searchText),
        search_text: searchText,
        summary_text: [String(cell?.meta?.title || ''), ...outputTexts.slice(0, 2)]
          .filter(Boolean)
          .join(' ')
          .trim(),
      }
    }

    if (cell?.type === 'math') {
      const outputTexts = getCellOutputModules(cell)
        .map((outputModule) => buildOutputModuleSearchText(outputModule))
        .filter(Boolean)
      const content = [
        String(cell?.expression || ''),
        ...outputTexts,
      ].filter(Boolean).join('\n\n')
      return {
        cell_key: String(cell?.key || ''),
        cell_type: 'math',
        digest: computeNotebookDigest(content),
        search_text: content,
        summary_text: [String(cell?.expression || ''), ...outputTexts.slice(0, 2)].filter(Boolean).join(' ').trim(),
      }
    }

    const content = String(cell?.content || '')
    return {
      cell_key: String(cell?.key || ''),
      cell_type: 'markdown',
      digest: computeNotebookDigest(content),
      search_text: content,
      summary_text: content,
    }
  }).filter((item) => item.cell_key)
}

const updateNotebookSearchIndex = async (reason = 'unknown') => {
  const indexKey = getNotebookSearchIndexKey()
  const documents = buildNotebookSearchDocuments(notebookCells.value)
  try {
    const result = await invoke('notebook_update_search_index', {
      filePath: indexKey,
      documents,
    })
    notebookSearchIndexReady.value = true
    logger.info('[Notebook] updated backend search index', {
      reason,
      filePath: indexKey,
      documentCount: Number(result?.document_count || documents.length),
      changedCount: Number(result?.changed_count || 0),
      reusedCount: Number(result?.reused_count || 0),
    })
  } catch (error) {
    notebookSearchIndexReady.value = false
    logger.warn('[Notebook] update backend search index failed', {
      reason,
      error: String(error?.message || error),
    })
  }
}

const scheduleNotebookSearchIndexUpdate = (reason = 'unknown', delay = 120) => {
  if (notebookSearchIndexSyncTimer) {
    window.clearTimeout(notebookSearchIndexSyncTimer)
  }
  notebookSearchIndexSyncTimer = window.setTimeout(() => {
    notebookSearchIndexSyncTimer = 0
    updateNotebookSearchIndex(reason)
  }, delay)
}

const searchNotebookViaBackend = async (keyword, reason = 'unknown') => {
  const normalizedKeyword = String(keyword || '').trim()
  if (!normalizedKeyword) {
    clearNotebookSearchHighlights()
    return
  }

  if (!notebookSearchIndexReady.value) {
    await updateNotebookSearchIndex(`${reason}:warmup`)
  }

  const indexKey = getNotebookSearchIndexKey()
  try {
    const response = await invoke('notebook_search', {
      filePath: indexKey,
      query: normalizedKeyword,
      limit: 200,
    })
    notebookSearchCellMatches.value = Array.isArray(response?.results) ? response.results : []
    notebookSearchActiveIndex.value = notebookSearchCellMatches.value.length ? 0 : -1
    logger.info('[Notebook] backend search finished', {
      reason,
      filePath: indexKey,
      keywordLength: normalizedKeyword.length,
      matchCount: notebookSearchCellMatches.value.length,
      totalMatches: Number(response?.total_matches || 0),
    })
  } catch (error) {
    logger.warn('[Notebook] backend search failed', {
      reason,
      error: String(error?.message || error),
    })
    const lowerKeyword = normalizedKeyword.toLowerCase()
    const fallbackResults = buildNotebookSearchDocuments(notebookCells.value)
      .map((document) => {
        const lowerText = String(document.search_text || '').toLowerCase()
        const matchCount = lowerText ? lowerText.split(lowerKeyword).length - 1 : 0
        if (matchCount <= 0) return null
        return {
          cell_key: document.cell_key,
          cell_type: document.cell_type,
          match_count: matchCount,
          snippet: String(document.summary_text || document.search_text || '').slice(0, 120),
        }
      })
      .filter(Boolean)
    notebookSearchCellMatches.value = fallbackResults
    notebookSearchActiveIndex.value = notebookSearchCellMatches.value.length ? 0 : -1
  }

  await nextTick()
  applyNotebookSearchDomHighlights(`${reason}:dom-highlight`)
}

const getNotebookSearchNavigationTargets = () => {
  const domMatches = (Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : [])
    .filter((node) => node instanceof HTMLElement && node.isConnected)

  if (domMatches.length) {
    if (domMatches.length !== notebookSearchDomMatches.value.length) {
      notebookSearchDomMatches.value = domMatches
    }
    return {
      mode: 'dom',
      matches: domMatches,
    }
  }

  return {
    mode: 'cell',
    matches: notebookSearchCellFallbackUsable.value && Array.isArray(notebookSearchCellMatches.value)
      ? notebookSearchCellMatches.value
      : [],
  }
}

const scheduleNotebookSearchQuery = (reason = 'unknown', delay = 90) => {
  if (notebookSearchQueryTimer) {
    window.clearTimeout(notebookSearchQueryTimer)
  }
  notebookSearchQueryTimer = window.setTimeout(() => {
    notebookSearchQueryTimer = 0
    searchNotebookViaBackend(notebookSearchKeyword.value, reason)
  }, delay)
}

const focusNotebookSearchMatchAt = (index, { scroll = true } = {}) => {
  const navigation = getNotebookSearchNavigationTargets()
  const matches = navigation.matches
  const total = matches.length
  if (!total) {
    notebookSearchActiveIndex.value = -1
    return
  }

  let nextIndex = Number(index)
  if (!Number.isFinite(nextIndex)) nextIndex = 0
  if (nextIndex < 0) nextIndex = total - 1
  if (nextIndex >= total) nextIndex = 0

  notebookSearchActiveIndex.value = nextIndex
  if (navigation.mode === 'dom') {
    // 激活态的切换与滚动交给共享实现，保证与预览/分屏模式的行为一致。
    setActiveSearchHit(matches, nextIndex, {
      activeClass: SEARCH_HIT_ACTIVE_CLASS,
      scroll,
    })
    if (!scroll) return
    const activeNode = matches[nextIndex]
    logger.debug('[Notebook] focus DOM search match', {
      index: nextIndex,
      total,
      cellKey: String(activeNode?.closest?.('.guaxi-notebook__cell')?.getAttribute?.('data-cell-key') || ''),
    })
    return
  }

  if (!scroll) return

  nextTick(() => {
    const active = matches[nextIndex]
    const activeCellKey = String(active?.cell_key || '')
    if (!activeCellKey) return
    const target = notebookRootRef.value?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(activeCellKey)}"]`)
    if (target instanceof HTMLElement) {
      target.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
      logger.debug('[Notebook] focus cell search match', {
        index: nextIndex,
        total,
        cellKey: activeCellKey,
      })
    }
  })
}

// 设计说明（对应“选中文本后 Ctrl+F 直接带入搜索框”）：
// 与预览模式同一套规则：优先取实时 window 选区，退回鼠标松开时记下的选区文本；
// 跨行选区不带入，与 Monaco 保持一致。
const getNotebookSearchSeedText = () => {
  const live = String(getNotebookSelectionTextFromWindow()?.text || '')
  const raw = live || String(notebookSelectionText.value || '')
  const seed = raw.trim()
  if (!seed || /[\r\n]/.test(seed)) return ''
  return seed.length <= 200 ? seed : ''
}

const openNotebookSearch = async ({ seedFromSelection = true } = {}) => {
  const seed = seedFromSelection ? getNotebookSearchSeedText() : ''
  revealNotebookTopToolbar('open-search')
  notebookSearchVisible.value = true
  if (seed) notebookSearchKeyword.value = seed
  logger.info('[Notebook] open search panel', { seeded: !!seed })
  await nextTick()
  notebookFindWidgetRef.value?.focusInput?.()
  scheduleNotebookSearchQuery('open-search', 0)
  applyNotebookSearchDomHighlights('open-search')
}

const closeNotebookSearch = () => {
  notebookSearchVisible.value = false
  notebookSearchKeyword.value = ''
  notebookSearchRegexInvalid.value = false
  logger.info('[Notebook] close search panel')
  clearNotebookSearchHighlights()
  scheduleHideNotebookTopToolbar('close-search')
}

const focusNextNotebookSearchMatch = () => {
  focusNotebookSearchMatchAt(notebookSearchActiveIndex.value + 1)
}

const focusPrevNotebookSearchMatch = () => {
  focusNotebookSearchMatchAt(notebookSearchActiveIndex.value - 1)
}

const setMarkdownEditorRef = (key, instance) => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return
  if (instance) {
    markdownEditorRefMap.set(normalizedKey, instance)
    return
  }
  markdownEditorRefMap.delete(normalizedKey)
}

const setPythonEditorRef = (key, instance) => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return
  if (instance) {
    pythonEditorRefMap.set(normalizedKey, instance)
    return
  }
  pythonEditorRefMap.delete(normalizedKey)
}

const setMathEditorRef = (key, instance) => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return
  if (instance) {
    mathEditorRefMap.set(normalizedKey, instance)
    return
  }
  mathEditorRefMap.delete(normalizedKey)
}

const refreshNotebookPythonEditor = (cell, reason = 'unknown') => {
  const key = String(cell?.key || '')
  const editorRef = pythonEditorRefMap.get(key)
  if (!editorRef?.refreshEditorServices) return
  window.setTimeout(() => {
    editorRef.refreshEditorServices(reason)
  }, 120)
  logger.debug('[Notebook] scheduled python editor service refresh', {
    key,
    cellId: String(cell?.cellId || ''),
    reason,
  })
}

const isNotebookVariableFocusedCell = (cell) => (
  cell?.type === 'python'
  && String(cell?.key || '') === String(notebookFocusedVariable.value?.cellKey || '')
  && !!String(notebookFocusedVariable.value?.variableName || '').trim()
)

const clearNotebookFocusedVariable = () => {
  if (notebookFocusedVariableTimer) {
    window.clearTimeout(notebookFocusedVariableTimer)
    notebookFocusedVariableTimer = 0
  }
  notebookFocusedVariable.value = { cellKey: '', variableName: '' }
}

// 设计说明：
// 1. 右侧变量列表只知道当前 inspector 的 cell 身份，真正的滚动和 Monaco decoration 必须由 Notebook 主视图完成。
// 2. 外层 cell 先提供明显选中背景；如果 Monaco 已挂载，再调用编辑器暴露的变量高亮方法定位源码中的变量名。
// 3. 高亮短暂保留，避免用户后续编辑时误以为变量仍处于“被锁定”状态。
const focusNotebookVariableInCell = async (cell, variableName, reason = 'session-variable-focus') => {
  const normalizedName = String(variableName || '').trim()
  if (cell?.type !== 'python' || !normalizedName) return false
  activateNotebookCell(cell, `focus-variable:${reason}`)
  if (getCellSnapshot(cell)?.meta?.cellCollapsed) {
    patchCellMeta(cell, { cellCollapsed: false })
  }
  notebookFocusedVariable.value = {
    cellKey: String(cell?.key || ''),
    variableName: normalizedName,
  }
  if (notebookFocusedVariableTimer) {
    window.clearTimeout(notebookFocusedVariableTimer)
  }
  notebookFocusedVariableTimer = window.setTimeout(() => {
    clearNotebookFocusedVariable()
  }, 4200)

  await nextTick()
  const root = notebookRootRef.value
  const target = root?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(String(cell?.key || ''))}"]`)
  if (target instanceof HTMLElement) {
    target.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
  }
  const editorRef = pythonEditorRefMap.get(String(cell?.key || ''))
  const highlighted = !!editorRef?.highlightVariableOccurrences?.(normalizedName, reason)
  logger.info('[NotebookDataLab] focused session variable source cell', {
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    variableName: normalizedName,
    editorHighlighted: highlighted,
    reason,
  })
  return true
}

const syncMarkdownEditorValueToCell = (key, reason = 'sync-markdown-editor-value') => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return false

  const editorRef = markdownEditorRefMap.get(normalizedKey)
  const nextValue = String(editorRef?.getEditorValue?.() || '')
  const currentCell = notebookCells.value.find((cell) => cell?.type === 'markdown' && String(cell.key) === normalizedKey)
  const currentValue = String(currentCell?.content || '')

  if (nextValue === currentValue) {
    logger.debug('[Notebook] skip markdown editor value sync because content is unchanged', {
      key: normalizedKey,
      reason,
      length: nextValue.length,
    })
    return false
  }

  notebookCells.value = notebookCells.value.map((cell) => {
    if (cell?.type !== 'markdown' || String(cell.key) !== normalizedKey) return cell
    return createNotebookMarkdownCell(nextValue, { key: cell.key })
  })

  logger.info('[Notebook] synchronized live Markdown editor value into notebook cell', {
    key: normalizedKey,
    reason,
    previousLength: currentValue.length,
    nextLength: nextValue.length,
  })
  return true
}

const getPythonCellCodeSummary = (cell) => {
  const lines = String(cell?.code || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
  return lines[0] || '空 Python Cell'
}

const getPythonCellOutlineTitle = (cell) => {
  const title = String(cell?.meta?.title || '').trim()
  if (title) return title
  const summary = getPythonCellCodeSummary(cell)
  return summary === '空 Python Cell' ? 'Python Cell' : `Python · ${summary}`
}

const getMathCellExpressionSummary = (cell) => {
  const lines = String(cell?.expression || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
  return lines[0] || '空 Math Cell'
}

const getMathCellOutlineTitle = (cell) => {
  const title = String(cell?.meta?.title || '').trim()
  if (title) return title
  const summary = getMathCellExpressionSummary(cell)
  return summary === '空 Math Cell' ? 'Math Cell' : `Math · ${summary}`
}

const jumpToNotebookOutlineLine = (line, options = {}) => {
  const targetLine = Number(line)
  if (!Number.isFinite(targetLine) || targetLine <= 0) {
    logger.warn('[Notebook] outline jump ignored because line is invalid', {
      line,
    })
    return false
  }

  const outlineEntry = notebookOutlineEntries.value.find((item) => Number(item?.line || 0) === targetLine)
  if (!outlineEntry) {
    logger.warn('[Notebook] outline jump skipped because heading was not found', {
      line: targetLine,
    })
    return false
  }

  const behavior = options?.behavior || 'smooth'
  const outlineId = String(outlineEntry.outlineId || '')
  const cellKey = String(outlineEntry.cellKey || '')
  const isPythonCellTarget = String(outlineEntry.kind || '') === 'python-cell'
  if (options?.recordHistory !== false) {
    recordNotebookReadingLocation('outline-jump')
  }

  nextTick(() => {
    const headingTarget = outlineId
      ? findNotebookHeadingTargetByOutlineId(outlineId)
      : null
    const fallbackCell = cellKey
      ? notebookRootRef.value?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(cellKey)}"]`)
      : null
    const target = headingTarget instanceof HTMLElement
      ? headingTarget
      : fallbackCell instanceof HTMLElement
        ? fallbackCell
        : null

    if (!target) {
      logger.warn('[Notebook] outline jump target missing in DOM', {
        line: targetLine,
        outlineId,
        cellKey,
      })
      return
    }

    target.scrollIntoView({
      block: headingTarget instanceof HTMLElement ? 'start' : 'center',
      inline: 'nearest',
      behavior,
    })
    if (isPythonCellTarget && cellKey) {
      const cell = notebookCells.value.find((item) => String(item?.key || '') === cellKey)
      if (cell?.type === 'python') {
        setActiveNotebookPythonCellKey(cellKey, 'outline-python-cell')
        patchCellMeta(cell, { cellCollapsed: false })
      }
    }

    logger.info('[Notebook] outline jump applied', {
      line: targetLine,
      outlineId,
      cellKey,
      kind: String(outlineEntry.kind || 'heading'),
    })
  })

  return true
}

const focusMarkdownEditor = (key, reason = 'focus-editor') => {
  nextTick(() => {
    const editorRef = markdownEditorRefMap.get(String(key || ''))
    if (editorRef?.focusEditor) {
      void editorRef.focusEditor({ cursor: 'end' })
      logger.info('[Notebook] focus markdown editor', {
        key: String(key || ''),
        reason,
        mode: 'monaco',
      })
    } else {
      logger.warn('[Notebook] focus markdown editor skipped because editor ref was not found', {
        key: String(key || ''),
        reason,
      })
    }
  })
}

const isMarkdownCellEditing = (cell) => {
  return cell?.type === 'markdown' && String(editingMarkdownCellKey.value || '') === String(cell?.key || '')
}

const enterMarkdownEditMode = (key, reason = 'manual-enter') => {
  const nextKey = String(key || '')
  if (!nextKey) return
  const currentEditingKey = String(editingMarkdownCellKey.value || '')
  if (currentEditingKey && currentEditingKey !== nextKey) {
    syncMarkdownEditorValueToCell(currentEditingKey, 'before-switch-markdown-edit-target')
    flushPendingNotebookMarkdown('switch-markdown-edit-target')
  }
  editingMarkdownCellKey.value = nextKey
  logger.info('[Notebook] enter markdown edit mode', {
    key: nextKey,
    reason,
  })
  focusMarkdownEditor(nextKey, reason)
}

const exitMarkdownEditMode = (key, reason = 'manual-exit') => {
  const targetKey = String(key || '')
  if (String(editingMarkdownCellKey.value || '') !== targetKey) return
  const syncedFromEditor = syncMarkdownEditorValueToCell(targetKey, `before-exit:${reason}`)
  if (syncedFromEditor) {
    flushNotebookMarkdown(`exit-markdown-edit:${reason}:force-sync`)
  } else {
  flushPendingNotebookMarkdown(`exit-markdown-edit:${reason}`)
  }
  editingMarkdownCellKey.value = ''
  logger.info('[Notebook] exit markdown edit mode', {
    key: targetKey,
    reason,
    syncedFromEditor,
  })
}

// 设计说明：
// 1. 直接在每次按键时序列化整个 notebook 并回推父组件，会把探索模式拖得很卡。
// 2. 这里把“本地 cell 状态更新”和“整篇 Markdown 回写”拆开：输入时先只更新本地，再稍后批量 flush。
// 3. 插入、删除、运行写回这类结构性操作仍保持立即同步，避免用户感知到延迟。
const flushNotebookMarkdown = (reason = 'unknown') => {
  // 先把防抖中的 cell 内容编辑落地，避免序列化出旧内容
  if (pendingCellContentEdits.size) {
    applyPendingCellContentEdits()
  }
  const nextMarkdown = serializeNotebookCells(notebookCells.value)
  lastEmittedMarkdown.value = normalizeNotebookMarkdownText(nextMarkdown)
  emit('update:markdown', nextMarkdown)
  logger.info('[Notebook] flush notebook markdown', {
    reason,
    markdownLength: nextMarkdown.length,
    cellCount: notebookCells.value.length,
  })
}

const scheduleFlushNotebookMarkdown = (reason = 'unknown', delay = 140) => {
  if (notebookMarkdownEmitTimer) {
    window.clearTimeout(notebookMarkdownEmitTimer)
  }
  notebookMarkdownEmitTimer = window.setTimeout(() => {
    notebookMarkdownEmitTimer = 0
    flushNotebookMarkdown(reason)
  }, delay)
}

const flushPendingNotebookMarkdown = (reason = 'unknown') => {
  if (!notebookMarkdownEmitTimer) return
  window.clearTimeout(notebookMarkdownEmitTimer)
  notebookMarkdownEmitTimer = 0
  flushNotebookMarkdown(reason)
}

const recordNotebookHistory = (cells, reason = 'unknown') => {
  if (notebookHistoryApplying.value) return
  const snapshot = cloneNotebookCellsForHistory(cells)
  const signature = serializeNotebookCells(snapshot)
  const currentEntry = notebookHistoryEntries.value[notebookHistoryIndex.value] || null
  if (currentEntry?.signature === signature) {
    return
  }
  const nextEntries = notebookHistoryEntries.value.slice(0, notebookHistoryIndex.value + 1)
  nextEntries.push({
    signature,
    cells: snapshot,
  })
  notebookHistoryEntries.value = nextEntries.slice(-80)
  notebookHistoryIndex.value = notebookHistoryEntries.value.length - 1
  logger.info('[Notebook] recorded history snapshot', {
    reason,
    historyLength: notebookHistoryEntries.value.length,
    historyIndex: notebookHistoryIndex.value,
    cellCount: snapshot.length,
  })
}

const applyNotebookHistoryAt = (index, reason = 'unknown') => {
  const target = notebookHistoryEntries.value[index] || null
  if (!target) return
  notebookHistoryApplying.value = true
  try {
    emitCells(target.cells, '', {
      deferMarkdownSync: false,
      recordHistory: false,
    })
    notebookHistoryIndex.value = index
    editingMarkdownCellKey.value = ''
    logger.info('[Notebook] applied history snapshot', {
      reason,
      historyIndex: index,
      cellCount: Array.isArray(target.cells) ? target.cells.length : 0,
    })
  } finally {
    notebookHistoryApplying.value = false
  }
}

const undoNotebookHistory = () => {
  if (!canUndoNotebookHistory.value) return
  applyNotebookHistoryAt(notebookHistoryIndex.value - 1, 'undo')
}

const redoNotebookHistory = () => {
  if (!canRedoNotebookHistory.value) return
  applyNotebookHistoryAt(notebookHistoryIndex.value + 1, 'redo')
}

const requestNotebookSave = () => {
  flushPendingNotebookMarkdown('before-request-save')
  emit('request-save')
  logger.info('[Notebook] requested parent save')
}

const emitCells = (cells, focusKey = '', { deferMarkdownSync = false, recordHistory = true } = {}) => {
  pendingFocusCellKey.value = String(focusKey || '')
  const normalizedCells = decorateNotebookCells(cells)
  notebookCells.value = normalizedCells
  logger.info('[Notebook] emit cells', {
    focusKey: pendingFocusCellKey.value,
    cellCount: normalizedCells.length,
    markdownCellCount: normalizedCells.filter((cell) => cell.type === 'markdown').length,
    mathCellCount: normalizedCells.filter((cell) => cell.type === 'math').length,
    pythonCellCount: normalizedCells.filter((cell) => cell.type === 'python').length,
    deferMarkdownSync,
    recordHistory,
  })
  if (recordHistory) {
    recordNotebookHistory(normalizedCells, 'emit-cells')
  }
  if (deferMarkdownSync) {
    scheduleFlushNotebookMarkdown('deferred-cell-edit')
  } else {
    flushNotebookMarkdown('emit-cells')
  }
  schedulePersistNotebookSidecar('emit-cells')
  nextTick(() => {
    if (!pendingFocusCellKey.value) return
    const target = document.querySelector(`[data-cell-key="${CSS.escape(pendingFocusCellKey.value)}"]`)
    if (target instanceof HTMLElement) {
      target.focus()
    }
    pendingFocusCellKey.value = ''
  })
}

const updateMarkdownCell = (key, event) => {
  const value = typeof event === 'string'
    ? event
    : String(event?.target?.value || '')
  queueCellContentEdit(String(key), 'markdown', value)
}

// ───── Cell 内容编辑：防抖重建 ─────
// 设计说明：
// 1. 之前每敲一个字符都 emitCells()，会整份重排 94 个 cell、重渲染所有 markdown、跑 tree-sitter、记历史、写 sidecar，
//    在主线程同步执行 → 大文档卡顿；这一重渲染还会打断正在聚焦 cell 的 IME 组合 → 中文只能交替输入。
// 2. 编辑期间 cell 的 Monaco 编辑器本身持有最新内容（回声守卫已避免外部回写），因此把整份重建防抖到停顿后再做一次即可。
// 3. 运行 cell / 保存 / markdown 同步等任何读取 cell 内容的操作前，必须先 flushPendingCellContentEdits() 落地，避免读到旧内容。
const pendingCellContentEdits = new Map()

const applyPendingCellContentEdits = () => {
  if (!pendingCellContentEdits.size) return
  const edits = new Map(pendingCellContentEdits)
  pendingCellContentEdits.clear()

  edits.forEach((edit, key) => {
    if (edit.type !== 'python') return
    const targetCell = notebookCells.value.find(
      (cell) => cell?.type === 'python' && String(cell?.key || '') === String(key),
    )
    if (!targetCell) return
    const currentDigest = getNotebookCellContentDigest(targetCell)
    const nextDigest = computeNotebookDigest(edit.value)
    if (currentDigest !== nextDigest) {
      const currentSnapshot = getPythonCellSnapshot(props.filePath, targetCell.cellId)
      patchPythonCellSnapshot(
        props.filePath,
        targetCell.cellId,
        buildStalePythonCellSnapshot(targetCell, currentSnapshot, 'code-edited'),
      )
      bumpRefresh()
    }
  })

  emitCells(
    notebookCells.value.map((cell) => {
      const edit = edits.get(String(cell.key))
      if (!edit) return cell
      if (edit.type === 'python' && cell.type === 'python') {
        return createNotebookPythonCell({ ...cell, code: edit.value })
      }
      if (edit.type === 'markdown' && cell.type === 'markdown') {
        return createNotebookMarkdownCell(edit.value, { key: cell.key })
      }
      return cell
    }),
    '',
    { deferMarkdownSync: true },
  )
}

const debouncedApplyPendingCellContentEdits = debounce(applyPendingCellContentEdits, 250)

const flushPendingCellContentEdits = () => {
  debouncedApplyPendingCellContentEdits.cancel()
  applyPendingCellContentEdits()
}

const queueCellContentEdit = (key, type, value) => {
  pendingCellContentEdits.set(String(key), { type, value })
  debouncedApplyPendingCellContentEdits()
}

const updatePythonCell = (key, payload) => {
  if (payload?.target) {
    resizeTextareaToContent(payload.target)
  }
  const value = typeof payload === 'string'
    ? payload
    : String(payload?.target?.value || '')
  queueCellContentEdit(String(key), 'python', value)
}

const updateMathCell = (key, eventOrValue) => {
  const value = typeof eventOrValue === 'string' ? eventOrValue : String(eventOrValue?.target?.value || '')
  logger.debug('[Notebook] update math cell expression', {
    key: String(key || ''),
    expressionLength: value.length,
  })
  void renderMathCellPreviewImmediately(key, value)
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'math' || String(cell.key) !== String(key)) return cell
      return createNotebookMathCell(value, {
        ...cell,
        key: cell.key,
      })
    }),
    '',
    { deferMarkdownSync: true },
  )
}

const splitMathExpressionForPreview = (expression = '') => {
  const lines = String(expression || '').replace(/\r\n?/g, '\n').split('\n')
  const blocks = []
  let environmentBlock = []
  let environmentDepth = 0

  const countLatexEnvironmentDelta = (line = '') => {
    const beginCount = (String(line || '').match(/\\begin\s*\{/g) || []).length
    const endCount = (String(line || '').match(/\\end\s*\{/g) || []).length
    return beginCount - endCount
  }

  const flushEnvironmentBlock = () => {
    const text = environmentBlock.join('\n').trim()
    if (text) blocks.push(text)
    environmentBlock = []
    environmentDepth = 0
  }

  for (const rawLine of lines) {
    const line = String(rawLine || '').trim()
    if (!line) {
      if (environmentBlock.length) flushEnvironmentBlock()
      continue
    }

    // 设计说明：
    // 1. 普通多行输入按“每个非空行一个 display math”渲染，用户在编辑器里换行，预览里也能看到换行。
    // 2. 显式 LaTeX 环境（aligned/cases/matrix 等）内部本来就有自己的换行语义，必须保持为同一个公式块。
    // 3. 拆分只影响前端预览 Markdown，不改变数学块保存内容，也不影响后端 SymPy/绘图执行。
    const delta = countLatexEnvironmentDelta(line)
    if (environmentBlock.length || delta > 0) {
      environmentBlock.push(line)
      environmentDepth += delta
      if (environmentDepth <= 0) flushEnvironmentBlock()
      continue
    }

    blocks.push(line)
  }

  if (environmentBlock.length) flushEnvironmentBlock()
  return blocks
}

const buildMathPreviewMarkdown = (expression = '', mode = 'latex') => {
  // 数学块预览有两种模式（由工具栏图标切换，默认 latex）：
  //   - 'latex'  ：把内容按 LaTeX 公式渲染（每行/每个环境一个 $$...$$），适合写公式时实时看渲染效果。
  //   - 'command'：命令式工作区模式，内容是「赋值 / 表达式 / 命令」的程序，不按 LaTeX 渲染，
  //                而是以代码块展示源码；真正的结果在点「运行」后输出。
  const text = String(expression || '').trim()
  if (!text) return ''
  if (String(mode) === 'command') {
    return `\`\`\`python\n${text}\n\`\`\``
  }
  const blocks = splitMathExpressionForPreview(expression)
  if (!blocks.length) return ''
  return blocks.map((block) => `$$\n${block}\n$$`).join('\n\n')
}

const renderMathCellPreviewImmediately = async (key, expression) => {
  const normalizedKey = String(key || '')
  const renderSeq = ++notebookMathPreviewRenderSeq
  const previewMode = notebookMathPreviewModeMap.value[normalizedKey] === 'command' ? 'command' : 'latex'
  const mathMarkdown = buildMathPreviewMarkdown(expression, previewMode)
  const mathBlockCount = splitMathExpressionForPreview(expression).length
  try {
    const html = await renderMarkdownHtml(mathMarkdown, `math-cell-live-preview:${normalizedKey}`)
    if (renderSeq !== notebookMathPreviewRenderSeq) return
    renderedMarkdownHtmlMap.value = {
      ...renderedMarkdownHtmlMap.value,
      [normalizedKey]: html,
    }
    logger.debug('[Notebook] rendered live math cell preview', {
      key: normalizedKey,
      expressionLength: String(expression || '').length,
      mathBlockCount,
      renderSeq,
    })
  } catch (error) {
    logger.warn('[Notebook] live math preview render failed', {
      key: normalizedKey,
      expressionLength: String(expression || '').length,
      error: String(error?.message || error),
    })
  }
}

const insertCell = (index, position = 'below', type = 'markdown') => {
  const next = notebookCells.value.slice()
  const newCell = type === 'python'
    ? createNotebookPythonCell({
      code: '',
    })
    : type === 'math'
      ? createNotebookMathCell('')
      : createNotebookMarkdownCell('')
  const offset = position === 'below' ? 1 : 0
  logger.info('[Notebook] insert cell', {
    index,
    position,
    type,
  })
  if (type === 'markdown') {
    editingMarkdownCellKey.value = newCell.key
    clearActiveNotebookPythonCell('insert-markdown-cell')
  } else if (type === 'math') {
    clearActiveNotebookPythonCell('insert-math-cell')
  } else {
    setActiveNotebookPythonCellKey(String(newCell.key || ''), 'insert-python-cell')
    revealNotebookTopToolbar('insert-python-cell')
  }
  next.splice(Math.max(0, index + offset), 0, newCell)
  emitCells(next, newCell.key)
  if (type === 'markdown') {
    focusMarkdownEditor(newCell.key, 'insert-markdown-cell')
  }
}

const appendCell = (type = 'markdown') => {
  insertCell(notebookCells.value.length - 1, 'below', type)
}

const findNotebookCellByKey = (key) => {
  const normalizedKey = String(key || '').trim()
  if (!normalizedKey) return null
  return notebookCells.value.find((cell) => String(cell?.key || '') === normalizedKey) || null
}

const resolveTemplateInsertTargetCell = () => {
  const activeElement = document.activeElement
  const activeCellEl = activeElement instanceof Element
    ? activeElement.closest?.('.guaxi-notebook__cell[data-cell-key]')
    : null
  const activeDomKey = notebookRootRef.value instanceof HTMLElement
    && activeCellEl instanceof HTMLElement
    && notebookRootRef.value.contains(activeCellEl)
    ? activeCellEl.getAttribute('data-cell-key')
    : ''

  const candidateKeys = [
    activeDomKey,
    activeNotebookCellKey.value,
    editingMarkdownCellKey.value,
    activeNotebookPythonCellKey.value,
    activeMathWorkspaceCellKey.value,
  ]

  for (const key of candidateKeys) {
    const cell = findNotebookCellByKey(key)
    if (cell) return cell
  }
  return null
}

const appendTemplateContentToCellState = (cell, content) => {
  const text = String(content || '')
  const join = (current) => {
    const existing = String(current || '')
    if (!existing.trim()) return text
    return `${existing.replace(/\s*$/g, '')}\n\n${text}`
  }

  if (cell?.type === 'python') {
    emitCells(
      notebookCells.value.map((item) => (
        String(item?.key || '') === String(cell.key)
          ? createNotebookPythonCell({ ...item, code: join(item.code) })
          : item
      )),
      cell.key,
      { deferMarkdownSync: true },
    )
    setActiveNotebookPythonCellKey(String(cell.key || ''), 'template-insert-fallback')
    return true
  }

  if (cell?.type === 'markdown') {
    emitCells(
      notebookCells.value.map((item) => (
        String(item?.key || '') === String(cell.key)
          ? createNotebookMarkdownCell(join(item.content), { key: item.key })
          : item
      )),
      cell.key,
      { deferMarkdownSync: true },
    )
    editingMarkdownCellKey.value = String(cell.key || '')
    return true
  }

  if (cell?.type === 'math') {
    updateMathCell(cell.key, join(cell.expression))
    return true
  }

  return false
}

const insertTemplateContentIntoNotebook = async (content) => {
  const raw = String(content || '')
  if (!raw.trim()) return false
  const text = raw.endsWith('\n') ? raw : `${raw}\n`
  const targetCell = resolveTemplateInsertTargetCell()

  if (!targetCell) {
    logger.info('[Notebook] template insert skipped: no active target cell', {
      length: text.length,
    })
    return false
  }

  activeNotebookCellKey.value = String(targetCell.key || '')
  logger.info('[Notebook] template insert routed to cell', {
    cellKey: String(targetCell.key || ''),
    cellType: String(targetCell.type || ''),
    length: text.length,
  })

  if (targetCell.type === 'markdown') {
    const wasEditing = isMarkdownCellEditing(targetCell)
    if (!isMarkdownCellEditing(targetCell)) {
      enterMarkdownEditMode(targetCell.key, 'template-insert')
      await nextTick()
    }
    const editorRef = markdownEditorRefMap.get(String(targetCell.key || ''))
    if (editorRef?.insertContent && await editorRef.insertContent(text, { cursor: wasEditing ? 'keep' : 'end' })) {
      return true
    }
    return appendTemplateContentToCellState(targetCell, text)
  }

  if (targetCell.type === 'python') {
    activateNotebookCell(targetCell, 'template-insert')
    const editorRef = pythonEditorRefMap.get(String(targetCell.key || ''))
    if (editorRef?.insertContent && await editorRef.insertContent(text)) {
      return true
    }
    return appendTemplateContentToCellState(targetCell, text)
  }

  if (targetCell.type === 'math') {
    activateNotebookCell(targetCell, 'template-insert')
    const editorRef = mathEditorRefMap.get(String(targetCell.key || ''))
    if (editorRef?.insertContent && await editorRef.insertContent(text)) {
      return true
    }
    return appendTemplateContentToCellState(targetCell, text)
  }

  return false
}

const removeCell = (key) => {
  logger.info('[Notebook] remove cell', { key: String(key || '') })
  if (String(editingMarkdownCellKey.value || '') === String(key || '')) {
    editingMarkdownCellKey.value = ''
  }
  const filtered = notebookCells.value.filter((cell) => String(cell.key) !== String(key))
  emitCells(filtered)
}

// 笔记本含 Python/数学 cell 时,后台预热该会话的 worker 进程(仅 spawn,不跑用户代码),
// 把冷启动成本移到打开时;首次运行 cell 直接命中已就绪 worker。每个会话只预热一次,失败可重试。
const maybePrewarmPythonSession = (cells) => {
  const hasExecutable = Array.isArray(cells)
    && cells.some((cell) => cell?.type === 'python' || cell?.type === 'math')
  if (!hasExecutable) return
  const sessionId = String(createPythonSessionId(props.filePath) || '').trim()
  if (!sessionId || prewarmedPythonSessionKey.value === sessionId) return
  prewarmedPythonSessionKey.value = sessionId
  invoke('python_runtime_prewarm_session', { sessionId }).catch((error) => {
    // 预热失败不影响功能:首次运行 cell 仍会自行 spawn。仅重置标记以便后续重试。
    prewarmedPythonSessionKey.value = ''
    logger.info('[Notebook] python session prewarm skipped', {
      sessionId,
      error: String(error?.message || error),
    })
  })
}

// —— AI 代码生成助手(受设置页开关控制,默认关闭) ——
const refreshCodegenAssistEnabled = async () => {
  try {
    pythonCodegenAssistEnabled.value = !!(await invoke('get_python_codegen_assist_enabled'))
  } catch (error) {
    pythonCodegenAssistEnabled.value = false
    logger.info('[Notebook] read codegen-assist setting failed', { error: String(error?.message || error) })
  }
}

const handleCodegenSettingChanged = (event) => {
  pythonCodegenAssistEnabled.value = !!event?.detail?.enabled
}

const openCodegenPanel = (cell, event) => {
  if (!pythonCodegenAssistEnabled.value) return
  const key = String(cell?.key || '')
  if (!key) return
  flushPendingCellContentEdits()
  codegenTargetCellKey.value = key
  const latest = notebookCells.value.find((c) => String(c?.key || '') === key) || cell
  codegenPanelContext.value = String(latest?.code || '')
  codegenPanelSessionId.value = String(createPythonSessionId(props.filePath) || '')
  codegenPanelAnchor.value = {
    x: Number(event?.clientX ?? 200) + 8,
    y: Number(event?.clientY ?? 200) + 8,
  }
  codegenPanelVisible.value = true
}

const handleCodegenInsert = ({ code, mode } = {}) => {
  const text = String(code || '')
  const key = String(codegenTargetCellKey.value || '')
  if (!text || !key) return
  if (mode === 'replace') {
    updatePythonCell(key, text)
    return
  }
  // 光标处插入:优先用编辑器 ref 的 insertContent(保 undo/聚焦),失败降级为状态层追加。
  const cell = notebookCells.value.find((c) => String(c?.key || '') === key)
  const editorRef = pythonEditorRefMap.get(String(key))
  if (editorRef && typeof editorRef.insertContent === 'function') {
    try {
      if (cell) activateNotebookCell(cell, 'codegen-insert')
      editorRef.insertContent(text)
      return
    } catch (error) {
      logger.info('[Notebook] codegen insertContent failed, fallback to append', { error: String(error?.message || error) })
    }
  }
  flushPendingCellContentEdits()
  const current = String(cell?.code || '')
  updatePythonCell(key, current ? `${current}\n\n${text}` : text)
}


watch(notebookCells, (cells) => {
  const activeKey = String(activeNotebookPythonCellKey.value || '')
  if (activeKey) {
    const stillExists = cells.some((cell) => cell?.type === 'python' && String(cell?.key || '') === activeKey)
    if (!stillExists) {
      clearActiveNotebookPythonCell('active-cell-removed')
    }
  }
  maybePrewarmPythonSession(cells)
}, { deep: false })

const confirmRemoveCell = async (cell) => {
  const targetKey = String(cell?.key || '')
  if (!targetKey) return
  const cellTypeLabel = cell?.type === 'python' ? ' Python' : cell?.type === 'math' ? '数学块' : ' Markdown'
  const confirmed = await confirmDialog(`确定删除这个${cellTypeLabel} Cell 吗？`, {
    title: '确认删除',
    kind: 'warning',
  })
  logger.info('[Notebook] confirm remove cell result', {
    key: targetKey,
    cellType: String(cell?.type || ''),
    confirmed: !!confirmed,
  })
  if (!confirmed) return
  removeCell(targetKey)
}

const isGuaxiDisplayResultRepr = (value = '') => {
  const text = String(value || '').trim()
  return text.startsWith("{'__guaxi_display__'") || text.startsWith('{"__guaxi_display__"')
}

const parseGuaxiDisplayFromResultRepr = (value = '') => {
  const text = String(value || '').trim()
  if (!isGuaxiDisplayResultRepr(text)) return null

  // 设计说明：
  // 1. 正常路径应由 Python worker 的 `result_json.__guaxi_display__` 传回结构化 display；
  // 2. 但旧 worker 或某些边界执行会只给 `result_repr`，于是图片 payload 被当成普通文本渲染；
  // 3. 这里只兜底解析我们自己生成的 display 字典，不尝试通用解析 Python repr，避免误吞用户文本。
  const typeMatch = text.match(/['"]type['"]\s*:\s*['"]([^'"]+)['"]/)
  const mimeMatch = text.match(/['"]mime['"]\s*:\s*['"]([^'"]+)['"]/)
  const dataBase64Match = text.match(/['"]data_base64['"]\s*:\s*['"]([^'"]+)['"]/)
  const markdownMatch = text.match(/['"]markdown['"]\s*:\s*(['"])([\s\S]*)\1\s*\}\s*\}?$/)
  const type = String(typeMatch?.[1] || '').trim()
  if (!type) return null
  if (type === 'image' && dataBase64Match?.[1]) {
    return {
      type: 'image',
      mime: String(mimeMatch?.[1] || 'image/png').trim() || 'image/png',
      data_base64: String(dataBase64Match[1] || '').trim(),
    }
  }
  if (type === 'markdown' && markdownMatch?.[2]) {
    return {
      type: 'markdown',
      mime: String(mimeMatch?.[1] || 'text/markdown').trim() || 'text/markdown',
      markdown: String(markdownMatch[2] || '')
        .replace(/\\n/g, '\n')
        .replace(/\\'/g, "'")
        .replace(/\\"/g, '"'),
    }
  }
  return null
}

const formatPythonExecOutput = (res) => {
  const chunks = []
  const stdout = String(res?.stdout || '').trim()
  const stderr = filterBenignPythonDisplayWarnings(String(res?.stderr || ''), res).trim()
  const resultRepr = String(res?.result_repr || '').trim()
  if (stdout) chunks.push(stdout)
  if (stderr) chunks.push(stderr)
  // 设计说明：
  // Python 代码块“正常执行但没有显式输出”时，最后一个表达式可能只是 `None`。
  // 这种值对探索模式用户来说属于噪音，不应该伪装成一次可见输出。
  if (
    resultRepr
    && resultRepr !== 'None'
    && !stdout.includes(resultRepr)
    && !isGuaxiDisplayResultRepr(resultRepr)
  ) {
    chunks.push(resultRepr)
  }
  return chunks.join('\n\n').trim()
}

const isBenignPythonDisplayWarningLine = (line = '') => {
  return /UserWarning:\s*FigureCanvasAgg is non-interactive,\s*and thus cannot be shown/i.test(String(line || ''))
}

const filterBenignPythonDisplayWarnings = (text = '', res = null) => {
  const normalizedText = String(text || '')
  if (!normalizedText.trim()) return ''
  // 设计说明：
  // 1. `plt.show()` 在 Matplotlib Agg 后端会把 FigureCanvasAgg warning 写到 stderr；
  //    但 worker 已经成功捕获了 image/html display 时，这条 warning 只是在说明当前后端不可交互显示。
  // 2. 只在本次结果确实存在 display 时过滤，避免真正没有图像/动画输出时吞掉有用的错误信息。
  // 3. 保留日志计数，后续如果 Matplotlib 文案变化，可以从日志看出过滤是否仍生效。
  if (!getGuaxiDisplayFromResult(res)) return normalizedText
  const lines = normalizedText.replace(/\r\n?/g, '\n').split('\n')
  const keptLines = lines.filter((line) => !isBenignPythonDisplayWarningLine(line))
  const removedCount = lines.length - keptLines.length
  if (removedCount > 0) {
    logger.info('[Notebook] filtered benign matplotlib display warning', {
      removedCount,
      displayType: String(getGuaxiDisplayFromResult(res)?.type || getGuaxiDisplayFromResult(res)?.mime || ''),
    })
  }
  return keptLines.join('\n').trim()
}

const stripNotebookMathTraceOutput = (text = '') => {
  return String(text || '')
    .split(/\r\n?|\n/)
    .filter((line) => !String(line || '').trim().startsWith('[GuaxiNotebookMath]'))
    .join('\n')
    .trim()
}
const getNotebookRepoRoot = () => {
  const normalizedFilePath = normalizeNotebookFsPath(props.filePath)
  if (!normalizedFilePath) return ''
  const normalizedRepoCandidates = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  let best = ''
  for (const candidate of normalizedRepoCandidates) {
    const normalizedCandidate = normalizeNotebookFsPath(candidate)
    if (!normalizedCandidate) continue
    if (normalizedFilePath === normalizedCandidate || normalizedFilePath.startsWith(`${normalizedCandidate}/`)) {
      if (normalizedCandidate.length > best.length) best = normalizedCandidate
    }
  }
  return best
}
const splitNotebookFsLinkAndAnchor = (href = '') => {
  const raw = String(href || '').trim()
  if (!raw) return { link: '', anchor: '' }
  const hashIndex = raw.indexOf('#')
  if (hashIndex < 0) return { link: raw, anchor: '' }
  return {
    link: raw.slice(0, hashIndex),
    anchor: raw.slice(hashIndex + 1),
  }
}
const ensureNotebookMarkdownExt = (raw = '') => {
  const normalized = String(raw || '').trim()
  if (!normalized) return ''
  if (/\.(md|markdown)$/i.test(normalized)) return normalized
  if (/[.][^/\\]+$/.test(normalized)) return normalized
  return `${normalized}.md`
}
const isNotebookHttpUrl = (href = '') => /^https?:\/\//i.test(String(href || '').trim())
const isNotebookMarkdownFilePath = (filePath = '') => /\.(md|markdown)$/i.test(String(filePath || '').trim())
const isNotebookWikilinkHref = (href = '') => String(href || '').trim().toLowerCase().startsWith('guaxi://wikilink')
const isNotebookPathLikeToken = (href = '') => {
  const raw = String(href || '').trim()
  if (!raw) return false
  if (raw.startsWith('#') || isNotebookHttpUrl(raw) || /^mailto:/i.test(raw)) return false
  return isNotebookWikilinkHref(raw) || raw.startsWith('.') || raw.startsWith('/') || /^[A-Za-z]:[\\/]/.test(raw) || raw.includes('/') || raw.includes('\\')
}
// 与 GuaxiEditor 的链接解析保持一致：链接目标里的非 ASCII 字符可能被写成百分号编码
// （如 `%E5%90%AF%E5%8A%A8`），直接交给后端 read_file_content 会因磁盘上无此字符串而报「找不到文件」。
// 仅在确实存在百分号编码时解码一次，失败回退原值，避免破坏文件名里合法的 `%`。
const decodeNotebookLinkPath = (rawPath = '') => {
  const s = String(rawPath || '')
  if (!s) return ''
  if (!/%[0-9A-Fa-f]{2}/.test(s)) return s
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}
const resolveNotebookRelativePath = (target, baseDir) => {
  const normalizedTarget = decodeNotebookLinkPath(String(target || '').trim())
  if (!normalizedTarget) return ''
  if (/^[A-Za-z]:[\\/]/.test(normalizedTarget)) return normalizeNotebookFsPath(normalizedTarget)
  if (normalizedTarget.startsWith('/')) return normalizeNotebookFsPath(normalizedTarget)
  return normalizeNotebookFsPath(joinNotebookPath(baseDir || '', normalizedTarget))
}
const buildNotebookLinkCardCacheKey = ({ targetPath = '', repoRoot = '', mtimeMs = null }) => {
  const normalizedTargetPath = normalizeNotebookFsPath(targetPath)
  const normalizedRepoRoot = normalizeNotebookFsPath(repoRoot)
  const normalizedMtime = Number.isFinite(Number(mtimeMs)) ? String(Number(mtimeMs)) : 'unknown'
  return `${normalizedRepoRoot}::${normalizedTargetPath}::${normalizedMtime}`
}
const extractNotebookLinkCardExcerpt = (content = '') => {
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
    if (cleaned) return cleaned.slice(0, 96)
  }
  return '打开关联内容'
}
const buildNotebookLinkCardTitle = (content, absPath, fallbackText = '') => {
  const heading = maskMathCellBlocksForMarkdown(maskPythonCellBlocksForMarkdown(content)).match(/^\s{0,3}#{1,6}\s+(.+)$/m)
  if (heading?.[1]) {
    return String(heading[1] || '').replace(/\s+#+\s*$/, '').trim()
  }
  const normalizedPath = normalizeNotebookFsPath(absPath)
  const fileName = normalizedPath.split('/').pop() || String(fallbackText || '').trim() || normalizedPath
  return fileName.replace(/\.(md|markdown)$/i, '') || '未命名笔记'
}
const buildNotebookLinkCardPathLabel = (absPath, repoRoot = '') => {
  const normalizedPath = normalizeNotebookFsPath(absPath)
  const normalizedRepoRoot = normalizeNotebookFsPath(repoRoot)
  if (normalizedRepoRoot && normalizedPath.startsWith(`${normalizedRepoRoot}/`)) {
    return normalizedPath.slice(normalizedRepoRoot.length + 1)
  }
  return normalizedPath
}
const resolveNotebookLinkTargetPath = async (href, { baseDocPath } = {}) => {
  const rawHref = String(href || '').trim()
  if (!rawHref) return ''
  const normalizedBaseDocPath = normalizeNotebookFsPath(baseDocPath || '')
  const baseDir = normalizedBaseDocPath
    ? normalizedBaseDocPath.replace(/\/[^/]*$/, '')
    : getNotebookDocDir()
  if (rawHref.toLowerCase().startsWith('guaxi://wikilink')) {
    try {
      const url = new URL(rawHref)
      const file = String(url.searchParams.get('file') || '').trim()
      if (!file) return ''
      return resolveNotebookRelativePath(ensureNotebookMarkdownExt(file), baseDir)
    } catch (error) {
      logger.warn('[NotebookLinkCard] resolve wikilink target failed', {
        href: rawHref,
        baseDocPath,
        error: String(error?.message || error),
      })
      return ''
    }
  }
  if (!isNotebookPathLikeToken(rawHref)) return ''
  const { link } = splitNotebookFsLinkAndAnchor(rawHref)
  const resolvedPath = resolveNotebookRelativePath(link, baseDir)
  logger.info('[NotebookLinkCard] resolved notebook link target path', {
    href: rawHref,
    baseDocPath: normalizedBaseDocPath,
    baseDir,
    resolvedPath,
  })
  return resolvedPath
}
const canPromoteNotebookLinkToCard = (anchor) => {
  if (!(anchor instanceof HTMLAnchorElement)) return false
  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href || href.startsWith('#') || isNotebookHttpUrl(href)) return false
  const container = anchor.closest('p, li, div')
  if (!container) return false
  const allAnchors = container.querySelectorAll('a')
  if (allAnchors.length !== 1 || allAnchors[0] !== anchor) return false
  const normalizeStandaloneLinkText = (value = '') => String(value || '')
    .replace(/\[\[|\]\]|\[|\]|\(|\)|【|】/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  const blockText = normalizeStandaloneLinkText(container.textContent || '')
  const anchorText = normalizeStandaloneLinkText(anchor.textContent || '')
  if (!!anchorText && blockText === anchorText) return true

  const onlyHasAnchorAndWhitespace = Array.from(container.childNodes || []).every((node) => {
    if (node === anchor) return true
    if (node.nodeType === Node.TEXT_NODE) {
      return !String(node.textContent || '').trim()
    }
    if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node
      if (element instanceof HTMLBRElement) return true
      return element.contains(anchor) && normalizeStandaloneLinkText(element.textContent || '') === anchorText
    }
    return false
  })
  return !!anchorText && onlyHasAnchorAndWhitespace
}
const loadNotebookLinkCardMeta = async (targetPath, { repoRoot = '' } = {}) => {
  const normalizedTargetPath = normalizeNotebookFsPath(targetPath)
  if (!normalizedTargetPath) return null
  try {
    const fileMeta = await invoke('get_file_meta', { path: normalizedTargetPath })
    const cacheKey = buildNotebookLinkCardCacheKey({
      targetPath: normalizedTargetPath,
      repoRoot,
      mtimeMs: fileMeta?.mtime_ms,
    })
    const cached = notebookLinkCardMetaCache.get(cacheKey)
    if (cached) {
      logger.info('[NotebookLinkCard] meta cache hit', {
        path: normalizedTargetPath,
        repoRoot,
        mtimeMs: fileMeta?.mtime_ms ?? null,
      })
      return cached
    }
    const isDirectory = !!fileMeta?.is_directory
    let content = ''
    if (!isDirectory && isNotebookMarkdownFilePath(normalizedTargetPath)) {
      content = String(await invoke('read_file_content', { path: normalizedTargetPath }) || '')
    }
    const meta = {
      path: normalizedTargetPath,
      repoRoot: normalizeNotebookFsPath(repoRoot),
      mtimeMs: fileMeta?.mtime_ms ?? null,
      kind: isDirectory ? 'directory' : 'file',
      title: isDirectory
        ? (normalizedTargetPath.split('/').pop() || '目录')
        : buildNotebookLinkCardTitle(content, normalizedTargetPath),
      excerpt: isDirectory
        ? '打开目录并继续浏览其中的笔记与资源'
        : extractNotebookLinkCardExcerpt(content),
      pathLabel: buildNotebookLinkCardPathLabel(normalizedTargetPath, repoRoot),
    }
    notebookLinkCardMetaCache.set(cacheKey, meta)
    logger.info('[NotebookLinkCard] meta loaded', {
      path: normalizedTargetPath,
      kind: meta.kind,
      repoRoot,
      mtimeMs: meta.mtimeMs,
    })
    return meta
  } catch (error) {
    logger.warn('[NotebookLinkCard] load meta failed', {
      path: normalizedTargetPath,
      repoRoot,
      error: String(error?.message || error),
    })
    return null
  }
}
const enhanceNotebookHtmlWithLinkCards = async (html, { baseDocPath } = {}) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<a')) return rawHtml
  const root = document.createElement('div')
  root.innerHTML = rawHtml
  const anchors = Array.from(root.querySelectorAll('a[href]'))
  if (!anchors.length) return rawHtml
  const repoRoot = getNotebookRepoRoot()
  for (const anchor of anchors) {
    try {
      if (!canPromoteNotebookLinkToCard(anchor)) continue
      const href = String(anchor.getAttribute('href') || '').trim()
      const targetPath = await resolveNotebookLinkTargetPath(href, { baseDocPath })
      if (!targetPath) continue
      const meta = await loadNotebookLinkCardMeta(targetPath, { repoRoot })
      if (!meta) continue
      const sourceBlock = anchor.closest('p, li')
      if (!sourceBlock) continue
      const card = document.createElement('div')
      card.className = `guaxi-preview-file-card guaxi-notebook__link-card is-${meta.kind}`
      card.setAttribute('data-guaxi-preview-card', meta.kind === 'directory' ? 'directory-link' : 'file-link')
      card.setAttribute('data-guaxi-preview-card-path', meta.path)
      const cardLink = document.createElement('a')
      cardLink.className = 'guaxi-preview-file-card__link'
      cardLink.setAttribute('href', href)
      cardLink.setAttribute('title', meta.path)
      cardLink.innerHTML = `
        <span class="guaxi-preview-file-card__eyebrow">${meta.kind === 'directory' ? '关联目录' : '关联笔记'}</span>
        <span class="guaxi-preview-file-card__title">${escapeHtmlAttribute(meta.title)}</span>
        <span class="guaxi-preview-file-card__excerpt">${escapeHtmlAttribute(meta.excerpt)}</span>
        <span class="guaxi-preview-file-card__meta">${escapeHtmlAttribute(meta.pathLabel)}</span>
      `
      card.appendChild(cardLink)
      sourceBlock.replaceWith(card)
      logger.info('[NotebookLinkCard] promoted markdown link to card', {
        href,
        targetPath: meta.path,
        kind: meta.kind,
      })
    } catch (error) {
      logger.warn('[NotebookLinkCard] promote link to card failed', {
        error: String(error?.message || error),
      })
    }
  }
  return root.innerHTML
}

// 设计说明：
// 1. Explorer 模式里的 Markdown 图片和主预览一样，可能以“同一个段落里多张图片”的形式出现。
// 2. 旧样式只把单张图片单独居中，没有给图片组容器建立布局，
//    结果就是多图时整体不居中，或者一张一行导致排版松散。
// 3. 这里把“纯图片段落”统一标记为媒体组，再交给样式层做更稳定的居中与间距控制。
const enhanceNotebookHtmlWithMediaGroups = (html) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<img')) return rawHtml

  const root = document.createElement('div')
  root.innerHTML = rawHtml
  const blocks = Array.from(root.querySelectorAll('p'))
  let mediaGroupCount = 0
  let mediaItemCount = 0
  let singleMediaGroupCount = 0
  let galleryMediaGroupCount = 0
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
        host: node,
        image: childElements[0],
      }
    }
    return null
  }

  for (const block of blocks) {
    // 设计说明：
    // 1. Explorer 模式同样会遇到 Markdown 中嵌入的原生 HTML，最常见的布局声明也是 `align` / `style`。
    // 2. 上一版判定条件过宽，导致标准 Markdown 图片段落也被误判。
    // 3. 这里改成只跳过明确声明过布局的 HTML 段落，保证 Markdown 图片语法继续正常增强。
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
    if (mediaItems.length === 1) {
      singleMediaGroupCount += 1
    } else {
      galleryMediaGroupCount += 1
    }

    for (const item of mediaItems) {
      // 设计说明：
      // 1. 裸 `<img>` 本身不适合作为画廊单元，无法稳定承载留白、阴影和响应式宽度。
      // 2. 这里在渲染后补一个轻量容器，让 Explorer 模式与主预览都能复用同一套媒体项布局策略。
      if (item.host === item.image) {
        const wrapper = document.createElement('span')
        wrapper.setAttribute('data-guaxi-media-item', 'image')
        item.image.replaceWith(wrapper)
        wrapper.appendChild(item.image)
        item.host = wrapper
      } else {
        item.host.setAttribute('data-guaxi-media-item', 'linked-image')
      }
      item.host.classList.add('guaxi-media-group__item')
      item.image.classList.add('guaxi-media-group__image')
    }

    mediaGroupCount += 1
    mediaItemCount += mediaItems.length
  }

  logger.info('[Notebook] enhanced media groups', {
    filePath: props.filePath,
    mediaGroupCount,
    mediaItemCount,
    singleMediaGroupCount,
    galleryMediaGroupCount,
    preservedAuthoredHtmlBlockCount,
  })
  return root.innerHTML
}

const getGuaxiDisplayFromResult = (res) => {
  const value = res?.result_json?.__guaxi_display__
  if (value && typeof value === 'object') return value
  const fallback = parseGuaxiDisplayFromResultRepr(res?.result_repr || '')
  if (fallback) {
    logger.info('[NotebookDataLab] recovered guaxi display from result_repr fallback', {
      type: String(fallback.type || ''),
      mime: String(fallback.mime || ''),
      hasDataBase64: !!String(fallback.data_base64 || '').trim(),
    })
  }
  return fallback
}

const getGuaxiDataflowFromResult = (res) => {
  const value = res?.result_json?.__guaxi_dataflow__
  return value && typeof value === 'object' ? value : null
}

const normalizeGuaxiTablePayloadFromResult = (value) => {
  if (!value || typeof value !== 'object') return null
  return {
    columns: Array.isArray(value.columns) ? value.columns.map((item) => String(item ?? '')) : [],
    rows: Array.isArray(value.rows) ? value.rows : [],
    rowCount: Number(value.row_count ?? (Array.isArray(value.rows) ? value.rows.length : 0)),
    truncated: !!value.truncated,
    dtypes: value.dtypes && typeof value.dtypes === 'object' ? value.dtypes : {},
    sourcePath: String(value.source_path || '').trim(),
    resultHandle: String(value.result_handle || '').trim(),
    sessionId: String(value.session_id || '').trim(),
    manifestPath: String(value.manifest_path || '').trim(),
    artifactPath: String(value.artifact_path || '').trim(),
    tableKey: String(value.table_key || '').trim(),
    tableName: String(value.table_name || '').trim(),
    tableKind: String(value.table_kind || '').trim(),
    tableCount: Number(value.table_count || 1),
    remote: !!value.remote,
  }
}

const getGuaxiTablesFromResult = (res) => {
  const explicitTables = Array.isArray(res?.result_json?.__guaxi_tables__)
    ? res.result_json.__guaxi_tables__.map((item) => normalizeGuaxiTablePayloadFromResult(item)).filter(Boolean)
    : []
  if (explicitTables.length) return explicitTables

  const fallback = normalizeGuaxiTablePayloadFromResult(res?.result_json?.__guaxi_dataframe__)
  return fallback ? [fallback] : []
}

const getGuaxiDataFrameFromResult = (res) => getGuaxiTablesFromResult(res)[0] || null

const getCellOutputModules = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return Array.isArray(snapshot?.outputs) ? snapshot.outputs : []
}

const getCellOutputModule = (cell, moduleId = '') => {
  const modules = getCellOutputModules(cell)
  const normalizedId = String(moduleId || '').trim()
  if (normalizedId) {
    return modules.find((item) => String(item?.id || '') === normalizedId) || null
  }
  return getLatestPythonCellOutputModule(modules) || null
}

const getLatestTextOutputModule = (cell) => getLatestPythonCellOutputModuleByKind(getCellOutputModules(cell), 'text')

const getNotebookOutputModuleTitle = (outputModule) => String(outputModule?.title || '输出结果')

const getNotebookOutputModuleSummary = (outputModule) => String(outputModule?.summary || '')

const buildOutputModuleMarkdownText = (outputModule) => {
  if (!outputModule) return ''
  if (outputModule.kind === 'table' && outputModule.table) {
    return dataframeToMarkdownTable(outputModule.table)
  }
  if (outputModule.kind === 'markdown') {
    return String(outputModule.display?.markdown || '')
  }
  if (outputModule.kind === 'html') {
    return String(outputModule.display?.html || outputModule.display?.path || outputModule.display?.source_path || '')
  }
  if (outputModule.kind === 'svg') {
    return String(outputModule.display?.svg || '')
  }
  if (outputModule.kind === 'text') {
    return String(outputModule.text || '')
  }
  return ''
}

const canCopyOutputModuleText = (outputModule) => {
  return !!String(buildOutputModuleMarkdownText(outputModule) || '').trim()
}

const buildOutputModuleCopyText = (outputModule) => {
  // 设计说明：解方程结果由后端整理为 Markdown，其中既包含文字说明也包含 $$...$$ LaTeX。
  // 复制时保留原始 Markdown，比复制渲染后的 DOM 更适合继续写回文档或发给其他数学工具。
  return String(buildOutputModuleMarkdownText(outputModule) || '').trim()
}

const buildOutputModuleSearchText = (outputModule) => {
  if (!outputModule) return ''
  if (outputModule.kind === 'table') return buildOutputModuleMarkdownText(outputModule)
  if (outputModule.kind === 'markdown') return String(outputModule.display?.markdown || '')
  if (outputModule.kind === 'html') return stripHtmlTags(String(outputModule.display?.html || outputModule.display?.path || outputModule.display?.source_path || ''))
  if (outputModule.kind === 'svg') return stripHtmlTags(String(outputModule.display?.svg || ''))
  if (outputModule.kind === 'text') return String(outputModule.text || '')
  return String(outputModule.summary || '')
}

const getCellSnapshot = (cell) => {
  refreshTick.value
  const persistedSnapshot = getPythonCellSnapshot(props.filePath, cell.cellId)
  const snapshot = isPythonCellSnapshotFresh(cell, persistedSnapshot)
    ? persistedSnapshot
    : buildStalePythonCellSnapshot(cell, persistedSnapshot, 'snapshot-read')
  // 设计说明：
  // 1. 旧快照里可能还只有 `outputText/display/dataframe` 三槽结构，这里读取时即时迁移成 `outputs[]`。
  // 2. Notebook 的新渲染层始终围绕模块列表工作，这样导入表格、运行结果、错误输出才能同时保留。
  // 3. 同时继续回填一份“最新文本/最新表格/最新富输出”给旧逻辑复用，降低这次改造对其余功能的侵入面。
  const outputs = normalizePythonCellOutputModules(snapshot?.outputs, {
    outputText: String(snapshot?.outputText || ''),
    display: snapshot?.display || null,
    dataframe: snapshot?.dataframe || null,
    source: 'legacy-snapshot',
    idPrefix: `legacy-${String(cell?.cellId || cell?.key || 'cell')}`,
  })
  const latestTextOutput = getLatestPythonCellOutputModuleByKind(outputs, 'text')
  const latestTableOutput = getLatestPythonCellOutputModuleByKind(outputs, 'table')
  const latestDisplayOutput = outputs.find((item) => isPythonCellDisplayOutputKind(item?.kind))
  return {
    codeDigest: String(snapshot?.codeDigest || getNotebookCellContentDigest(cell)),
    outputText: latestTextOutput?.text || String(snapshot?.outputText || ''),
    display: latestDisplayOutput?.display || snapshot?.display || null,
    dataframe: latestTableOutput?.table || snapshot?.dataframe || null,
    outputs,
    result: snapshot?.result || null,
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    lab: snapshot?.lab && typeof snapshot.lab === 'object'
      ? {
        ...snapshot.lab,
        imports: Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
      }
      : {
        imports: [],
      },
    meta: {
      ...(cell.meta || {}),
      ...(snapshot?.meta || {}),
      outputCollapsed: snapshot?.meta?.outputCollapsed ?? cell.meta?.outputCollapsed ?? false,
      sessionCollapsed: snapshot?.meta?.sessionCollapsed ?? cell.meta?.sessionCollapsed ?? true,
    },
  }
}

const getCellLabImports = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : []
}

const patchCellSnapshotLab = (cell, patch = {}) => {
  const snapshot = getCellSnapshot(cell)
  const nextLab = {
    ...(snapshot?.lab && typeof snapshot.lab === 'object' ? snapshot.lab : {}),
    ...(patch && typeof patch === 'object' ? patch : {}),
  }
  if (!Array.isArray(nextLab.imports)) {
    nextLab.imports = []
  }
  return nextLab
}

const updateNotebookCellDataflowSnapshot = (cell, dataflow = null, reason = 'unknown') => {
  if (!cell?.cellId || !dataflow || typeof dataflow !== 'object') return null
  const snapshot = getCellSnapshot(cell)
  const nextLab = patchCellSnapshotLab(cell, {
    dataflow: {
      ...dataflow,
      updatedAt: new Date().toISOString(),
      reason,
    },
  })
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    lab: nextLab,
    meta: {
      ...(snapshot?.meta || {}),
      staleVariables: [],
      staleReason: '',
    },
  })
  logger.info('[NotebookDataflow] cell dataflow snapshot updated', {
    reason,
    cellId: String(cell?.cellId || ''),
    used: Array.isArray(dataflow.used) ? dataflow.used : [],
    touched: Array.isArray(dataflow.touched) ? dataflow.touched : [],
  })
  return nextLab
}

const markDependentNotebookCellsStale = (sourceCell, dataflow = null, reason = 'unknown') => {
  if (!sourceCell?.cellId || !dataflow || typeof dataflow !== 'object') return []
  const touched = new Set(Array.isArray(dataflow.touched) ? dataflow.touched.map((item) => String(item || '').trim()).filter(Boolean) : [])
  if (!touched.size) return []

  const sourceIndex = notebookCells.value.findIndex((item) => String(item?.cellId || '') === String(sourceCell.cellId || ''))
  const affected = []
  notebookCells.value.forEach((cell, index) => {
    if (cell?.type !== 'python') return
    if (sourceIndex >= 0 && index <= sourceIndex) return
    const snapshot = getCellSnapshot(cell)
    const used = Array.isArray(snapshot?.lab?.dataflow?.used) ? snapshot.lab.dataflow.used : []
    const staleVariables = used.map((item) => String(item || '').trim()).filter((item) => touched.has(item))
    if (!staleVariables.length) return
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      meta: {
        ...(snapshot?.meta || {}),
        staleVariables,
        staleReason: `上游 Cell 更新了 ${staleVariables.join(', ')}`,
      },
    })
    affected.push({
      cellId: String(cell.cellId || ''),
      cellKey: String(cell.key || ''),
      staleVariables,
    })
  })

  if (affected.length) {
    logger.info('[NotebookDataflow] downstream cells marked stale', {
      reason,
      sourceCellId: String(sourceCell?.cellId || ''),
      touchedVariables: Array.from(touched),
      affectedCellCount: affected.length,
      affectedCellIds: affected.map((item) => item.cellId),
    })
  }
  return affected
}

const mergeNotebookSessionVarSummaries = (currentItems = [], updatedItems = []) => {
  const map = new Map()
  const visibleCurrentItems = filterVisibleWorkbenchSessionVariables(currentItems)
  const visibleUpdatedItems = filterVisibleWorkbenchSessionVariables(updatedItems)

  // 设计说明：
  // 1. 数据实验室导入现在走“仅回传目标变量摘要”的快路径，不能再用返回值直接覆盖整份会话变量列表。
  // 2. 这里把已有快照与增量更新按变量名合并，既保留旧变量，又让新导入的数据集可以立即出现在右侧实验室面板里。
  // 3. 合并前过滤 `_guaxi_*` / `__guaxi_*` 运行时变量，保证用户只看到自己定义或导入的数据。
  // 4. 统一在前端按名称排序，保证侧栏顺序稳定，避免用户误以为变量丢失或重复。
  ;(Array.isArray(visibleCurrentItems) ? visibleCurrentItems : []).forEach((item) => {
    const name = String(item?.name || '').trim()
    if (!name) return
    map.set(name, {
      ...item,
      name,
    })
  })

  ;(Array.isArray(visibleUpdatedItems) ? visibleUpdatedItems : []).forEach((item) => {
    const name = String(item?.name || '').trim()
    if (!name) return
    map.set(name, {
      ...(map.get(name) || {}),
      ...item,
      name,
    })
  })

  return Array.from(map.values()).sort((left, right) => {
    const leftName = String(left?.name || '')
    const rightName = String(right?.name || '')
    return leftName.localeCompare(rightName, 'zh-Hans-CN')
  })
}

// 设计说明：
// 1. Python worker 的正常执行链路应该返回 `session_vars`，但历史兼容路径或异常恢复路径可能缺失这个字段。
// 2. 旧逻辑一旦遇到“字段缺失”就把前端快照清空，右侧实验室会立刻显示成 0 个变量，形成“变量明明还能 print 却面板为空”的错觉。
// 3. 这里改成“字段存在就覆盖，字段缺失就保留上一次快照”，只在后端明确返回数组时才认定为最新会话变量列表。
const resolveNotebookSessionVarsFromExecResult = (result, previousSessionVars = []) => {
  if (Array.isArray(result?.session_vars)) {
    const visibleSessionVars = filterVisibleWorkbenchSessionVariables(result.session_vars)
    if (visibleSessionVars.length !== result.session_vars.length) {
      logger.info('[Notebook] hidden internal python session variables from inspector', {
        rawSessionVarCount: result.session_vars.length,
        visibleSessionVarCount: visibleSessionVars.length,
      })
    }
    return visibleSessionVars
  }
  logger.warn('[Notebook] python execution response missing session_vars, keep previous snapshot variables', {
    hasResult: !!result,
    resultKeys: result && typeof result === 'object' ? Object.keys(result) : [],
    previousSessionVarCount: Array.isArray(previousSessionVars) ? previousSessionVars.length : 0,
  })
  return filterVisibleWorkbenchSessionVariables(previousSessionVars)
}

const buildNotebookPythonSessionInspectorPayload = (cell) => {
  if (cell?.type !== 'python') {
    return {
      filePath: String(props.filePath || ''),
      cellId: '',
      cellKey: '',
      stableId: '',
      cellTitle: '',
      sessionId: createPythonSessionId(props.filePath),
      hasActiveCell: false,
      datasets: [],
      variables: [],
    }
  }

  const snapshot = getCellSnapshot(cell)
  const visibleSessionVars = filterVisibleWorkbenchSessionVariables(snapshot?.sessionVars)
  return {
    filePath: String(props.filePath || ''),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    stableId: String(cell?.meta?.id || ''),
    cellTitle: String(cell?.meta?.title || ''),
    sessionId: createPythonSessionId(props.filePath),
    hasActiveCell: true,
    datasets: buildNotebookLabDatasetDescriptors(
      visibleSessionVars,
      Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
    ),
    variables: visibleSessionVars,
  }
}

const syncNotebookPythonSessionInspector = (cell, reason = 'unknown') => {
  const payload = buildNotebookPythonSessionInspectorPayload(cell)
  rightSiderStore.setPythonSessionInspector(payload)
  logger.debug('[Notebook] sync python session inspector payload', {
    reason,
    cellId: payload.cellId,
    hasActiveCell: payload.hasActiveCell,
    variableCount: payload.variables.length,
  })
  return payload
}

const emitNotebookSessionVarsUpdated = async (cell, variables = [], reason = 'unknown') => {
  const sessionId = createPythonSessionId(props.filePath)
  const names = Array.from(new Set(
    (Array.isArray(variables) ? variables : [])
      .map((item) => String(item?.name || item?.variableName || '').trim())
      .filter(Boolean),
  ))
  if (!names.length || !sessionId) return

  // 设计说明：
  // 1. 独立数据工作台和 Notebook 主视图运行在不同 WebView，不能共享响应式 store。
  // 2. 每次 Python session 中有变量摘要更新时，通过 Tauri 全局事件广播“哪个 session 的哪些变量变了”。
  // 3. 工作台收到自己关心的变量变更后再从 worker 拉取分页快照，避免把大 DataFrame 直接塞进事件负载。
  await Promise.all(names.map((variableName) => emitTauriEvent(DATA_LAB_SESSION_VARS_UPDATED_EVENT, {
    sessionId,
    variableName,
    filePath: String(props.filePath || ''),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    stableId: String(cell?.meta?.id || ''),
    source: String(reason || 'notebook'),
    updatedAt: new Date().toISOString(),
  }).catch((error) => {
    logger.warn('[NotebookDataLab] emit session var update failed', {
      reason,
      variableName,
      error: String(error?.message || error),
    })
  })))

  logger.info('[NotebookDataLab] session var update event emitted', {
    reason,
    cellId: String(cell?.cellId || ''),
    variableNames: names,
  })
}

watch(
  [
    () => activeNotebookPythonCellKey.value,
    () => refreshTick.value,
    () => props.filePath,
  ],
  () => {
    const targetCell = activeNotebookPythonCell.value
    if (targetCell?.type === 'python') {
      syncNotebookPythonSessionInspector(targetCell, 'active-cell-or-refresh-watch')
      return
    }
    rightSiderStore.clearPythonSessionInspector()
    logger.debug('[Notebook] cleared python session inspector from watch because no active python cell exists')
  },
  { immediate: true },
)

// 文件切换时，清理上一份文件的数学块会话状态，保证切回 / 重开时是新数据而非过时旧值。
// （clearMathSessionState 在 setup 后段定义；watch 回调延迟执行，引用安全。）
watch(() => props.filePath, (nextPath, prevPath) => {
  const prev = String(prevPath || '').trim()
  if (prev && prev !== String(nextPath || '').trim()) {
    void clearMathSessionState(prev, 'file-change')
  }
})

const isActiveNotebookPythonCell = (cell) => {
  if (cell?.type !== 'python') return false
  return String(cell?.key || '') === String(activeNotebookPythonCellKey.value || '')
}

const buildCellOutputModuleCacheKey = (cell, outputModule) => [
  String(cell?.key || ''),
  String(outputModule?.id || ''),
].join('::')

const getCellDisplayKind = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  return String(targetModule?.kind || '').trim()
}

const getCellDisplayUrl = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  if (!targetModule) return ''
  // 富媒体输出可能直接携带 base64。优先即时生成 data URL，避免数学单元在
  // resolvedDisplayUrlMap 异步同步完成前只显示 image/png 标题而没有实际图像。
  const directDataUrl = getPythonCellOutputDisplayDataUrl(targetModule?.display || null)
  if (directDataUrl) return directDataUrl
  return String(resolvedDisplayUrlMap.value[buildCellOutputModuleCacheKey(cell, targetModule)] || '')
}

const getCellDisplaySourcePath = (outputModule = null) => {
  return getPythonCellOutputDisplaySourcePath(outputModule?.display || null)
}

const buildDisplayCacheKey = (display) => {
  const kind = normalizePythonCellOutputDisplayType(display)
  const source = getPythonCellOutputDisplaySourcePath(display)
  const dataUrl = getPythonCellOutputDisplayDataUrl(display)
  const mime = String(display?.mime || '')
  return [
    kind,
    mime,
    source,
    dataUrl ? computeNotebookDigest(dataUrl) : '',
  ].join('|')
}

const resolveDisplayUrl = async (display) => {
  const dataUrl = getPythonCellOutputDisplayDataUrl(display)
  if (dataUrl) return dataUrl

  const source = getPythonCellOutputDisplaySourcePath(display)
  if (!source) return ''
  if (/^(https?:|data:|blob:)/i.test(source)) return source

  try {
    const converted = convertFileSrc(source)
    return typeof converted?.then === 'function' ? await converted : String(converted || '')
  } catch (error) {
    logger.warn('[Notebook] convert display source path failed', {
      source,
      error: String(error?.message || error),
    })
    return source
  }
}

const syncResolvedDisplayUrls = async (cells, reason = 'unknown') => {
  const previousMap = resolvedDisplayUrlMap.value || {}
  const nextMap = {}
  const tasks = []
  let cacheHitCount = 0
  let resolvedCount = 0

  for (const cell of Array.isArray(cells) ? cells : []) {
    // Python cell 与 math block 共用 outputModule 富媒体结构；数学绘图失败显示为空，
    // 正是因为这里曾只同步 python 单元，导致 math image 永远没有可渲染 URL。
    if (!['python', 'math'].includes(String(cell?.type || ''))) continue
    for (const outputModule of getCellOutputModules(cell)) {
      if (!['image', 'audio', 'video', 'html'].includes(String(outputModule?.kind || ''))) continue
      const display = outputModule?.display || null
      const mapKey = buildCellOutputModuleCacheKey(cell, outputModule)
      const cacheKey = buildDisplayCacheKey(display)
      const directDataUrl = getPythonCellOutputDisplayDataUrl(display)
      if (directDataUrl) {
        nextMap[mapKey] = directDataUrl
        if (cacheKey) {
          notebookDisplayUrlCache.set(cacheKey, directDataUrl)
        }
        cacheHitCount += 1
        continue
      }
      if (cacheKey && notebookDisplayUrlCache.has(cacheKey)) {
        nextMap[mapKey] = notebookDisplayUrlCache.get(cacheKey)
        cacheHitCount += 1
        continue
      }
      if (cacheKey && previousMap[mapKey]) {
        nextMap[mapKey] = previousMap[mapKey]
        notebookDisplayUrlCache.set(cacheKey, previousMap[mapKey])
        cacheHitCount += 1
        continue
      }
      tasks.push((async () => {
        const resolvedUrl = await resolveDisplayUrl(display)
        nextMap[mapKey] = resolvedUrl
        if (cacheKey) {
          notebookDisplayUrlCache.set(cacheKey, resolvedUrl)
        }
        resolvedCount += 1
      })())
    }
  }

  await Promise.all(tasks)
  resolvedDisplayUrlMap.value = nextMap
  logger.info('[Notebook] sync display urls', {
    reason,
    mediaCount: Object.keys(nextMap).length,
    cacheHitCount,
    resolvedCount,
    cellTypes: [...new Set((Array.isArray(cells) ? cells : []).map((cell) => String(cell?.type || '')).filter(Boolean))],
  })
}

const hasVisibleOutput = (cell) => {
  return getCellOutputModules(cell).length > 0
}

const renderMarkdownHtml = async (content, reason = 'unknown') => {
  const markdownText = preprocessNotebookObsidianWikilinks(String(content || ''))
  if (!markdownText.trim()) return ''
  const cacheKey = buildNotebookMarkdownCacheKey(markdownText, {
    theme: props.theme,
    filePath: props.filePath,
    basePath: getNotebookBasePath(),
  })
  if (notebookMarkdownRenderCache.has(cacheKey)) {
    logger.info('[Notebook] markdown render cache hit', {
      reason,
      cacheKey,
      filePath: props.filePath,
    })
    return notebookMarkdownRenderCache.get(cacheKey)
  }
  try {
    let html = ''
    if (notebookMarkdownWorkerReady.value || notebookMarkdownWorkerRef.value) {
      const workerHtml = await renderMarkdownHtmlWithWorker(markdownText, reason)
      html = await finalizeNotebookRenderedHtml(workerHtml, reason)
      logger.info('[Notebook] rendered markdown via worker', {
        reason,
        markdownLength: markdownText.length,
      })
    } else {
      html = await renderMarkdown(markdownText, buildMarkdownRenderOptions())
      logger.info('[Notebook] rendered markdown via main thread fallback', {
        reason,
        markdownLength: markdownText.length,
      })
    }
    html = await enhanceNotebookHtmlWithLinkCards(html, {
      baseDocPath: props.filePath,
    })
    html = enhanceNotebookHtmlWithMediaGroups(html)
    notebookMarkdownRenderCache.set(cacheKey, html)
    logger.info('[Notebook] markdown render cache store', {
      reason,
      cacheKey,
      filePath: props.filePath,
    })
    return html
  } catch (error) {
    logger.warn('[Notebook] worker markdown render failed, fallback to main thread renderer', {
      reason,
      error: String(error?.message || error),
    })
    try {
      let html = await renderMarkdown(markdownText, buildMarkdownRenderOptions())
      html = await enhanceNotebookHtmlWithLinkCards(html, {
        baseDocPath: props.filePath,
      })
      html = enhanceNotebookHtmlWithMediaGroups(html)
      notebookMarkdownRenderCache.set(cacheKey, html)
      logger.info('[Notebook] markdown render cache store after fallback', {
        reason,
        cacheKey,
        filePath: props.filePath,
      })
      return html
    } catch (fallbackError) {
      logger.error('[Notebook] render markdown html failed', {
        reason,
        error: String(fallbackError?.message || fallbackError),
      })
      return markdownText
    }
  }
}

// 设计说明：
// Explorer 模式中的 Markdown 预览需要复用主编辑器的渲染链，
// 这样代码高亮、数学公式与其他增强能力才能和主界面保持一致。
const syncRenderedMarkdownHtml = async (cells, reason = 'unknown') => {
  const renderVersion = markdownRenderVersion.value + 1
  markdownRenderVersion.value = renderVersion

  logger.info('[Notebook] sync rendered markdown start', {
    reason,
    renderVersion,
    cellCount: Array.isArray(cells) ? cells.length : 0,
  })

  const nextMarkdownMap = {}
  const nextOutputMap = {}
  const renderTasks = []
  let markdownCacheHitCount = 0
  let outputCacheHitCount = 0
  let markdownRenderCount = 0
  let outputRenderCount = 0

  for (const cell of Array.isArray(cells) ? cells : []) {
    if (cell?.type === 'markdown' || cell?.type === 'math') {
      const cellKey = String(cell.key || '')
      const markdownText = cell?.type === 'math'
        ? buildMathPreviewMarkdown(cell.expression || '', getMathCellPreviewMode(cell))
        : String(cell.content || '')
      const renderReasonPrefix = cell?.type === 'math' ? 'math-cell' : 'markdown-cell'
      const logCellKind = cell?.type === 'math' ? 'math' : 'markdown'
      if (cell?.type === 'math') {
        logger.debug('[Notebook] build math cell preview markdown', {
          reason,
          cellKey,
          expressionLength: String(cell.expression || '').length,
          mathBlockCount: splitMathExpressionForPreview(cell.expression || '').length,
        })
      }
      const cacheKey = buildNotebookMarkdownCacheKey(markdownText, {
        theme: props.theme,
        filePath: props.filePath,
        basePath: getNotebookBasePath(),
      })
      if (cell?.type === 'markdown' && hasNotebookTocMarker(markdownText)) {
        renderTasks.push((async () => {
          const renderedHtml = await renderNotebookMarkdownHtmlWithSyntheticToc(
            markdownText,
            cellKey,
            `markdown-cell:${cellKey}`,
          )
          nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(renderedHtml, cellKey)
          markdownRenderCount += 1
        })())
        continue
      }
      const cachedMarkdownHtml = notebookMarkdownRenderCache.get(cacheKey)
      if (typeof cachedMarkdownHtml === 'string' && cachedMarkdownHtml) {
        // 设计说明：
        // 1. Markdown Cell 和 Math Cell 共用主 Markdown 渲染链，这样 KaTeX/LaTeX、代码高亮和图片路径解析保持一致。
        // 2. 缓存命中时仍按 cacheKey 读取，不复用上一轮 map，避免切换文件后同名 cellKey 串内容。
        nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(cachedMarkdownHtml, cellKey)
        markdownCacheHitCount += 1
        logger.info('[Notebook] reuse cached markdown html for cell', {
          reason,
          cellKey,
          cellType: logCellKind,
          cacheKey,
          filePath: props.filePath,
        })
        continue
      }
      renderTasks.push((async () => {
        const renderedHtml = await renderMarkdownHtml(
          markdownText,
          `${renderReasonPrefix}:${cellKey}`,
        )
        nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(renderedHtml, cellKey)
        markdownRenderCount += 1
      })())
      continue
    }

    if (cell?.type === 'python' || cell?.type === 'math') {
      for (const outputModule of getCellOutputModules(cell)) {
        if (outputModule.kind !== 'markdown') continue
        const outputMarkdown = String(outputModule.display?.markdown || '')
        if (!outputMarkdown.trim()) continue
        const cellKey = String(cell.key || '')
        const outputMapKey = buildCellOutputModuleCacheKey(cell, outputModule)
        const cacheKey = buildNotebookMarkdownCacheKey(outputMarkdown, {
          theme: props.theme,
          filePath: props.filePath,
          basePath: getNotebookBasePath(),
        })
        const cachedOutputHtml = notebookMarkdownRenderCache.get(cacheKey)
        if (typeof cachedOutputHtml === 'string' && cachedOutputHtml) {
          nextOutputMap[outputMapKey] = cachedOutputHtml
          outputCacheHitCount += 1
          logger.info('[Notebook] reuse cached markdown output html for python cell module', {
            reason,
            cellKey,
            outputModuleId: String(outputModule.id || ''),
            cacheKey,
            filePath: props.filePath,
          })
          continue
        }

        renderTasks.push((async () => {
          nextOutputMap[outputMapKey] = await renderMarkdownHtml(
            outputMarkdown,
            `python-output:${cellKey}:${String(outputModule.id || '')}`,
          )
          outputRenderCount += 1
        })())
      }
    }
  }

  for (const cell of Array.isArray(cells) ? cells : []) {
    if (cell?.type !== 'math') continue
    for (const outputModule of getCellOutputModules(cell)) {
      if (outputModule.kind !== 'markdown') continue
      const outputMarkdown = String(outputModule.display?.markdown || '')
      if (!outputMarkdown.trim()) continue
      const cellKey = String(cell.key || '')
      const outputMapKey = buildCellOutputModuleCacheKey(cell, outputModule)
      const cacheKey = buildNotebookMarkdownCacheKey(outputMarkdown, {
        theme: props.theme,
        filePath: props.filePath,
        basePath: getNotebookBasePath(),
      })
      const cachedOutputHtml = notebookMarkdownRenderCache.get(cacheKey)
      if (typeof cachedOutputHtml === 'string' && cachedOutputHtml) {
        nextOutputMap[outputMapKey] = cachedOutputHtml
        outputCacheHitCount += 1
        logger.info('[Notebook] reuse cached markdown output html for math cell module', {
          reason,
          cellKey,
          outputModuleId: String(outputModule.id || ''),
          cacheKey,
          filePath: props.filePath,
        })
        continue
      }

      renderTasks.push((async () => {
        nextOutputMap[outputMapKey] = await renderMarkdownHtml(
          outputMarkdown,
          `math-output:${cellKey}:${String(outputModule.id || '')}`,
        )
        outputRenderCount += 1
      })())
    }
  }

  await Promise.all(renderTasks)

  if (renderVersion !== markdownRenderVersion.value) {
    logger.info('[Notebook] skip stale rendered markdown result', {
      reason,
      renderVersion,
      latestRenderVersion: markdownRenderVersion.value,
    })
    return
  }

  renderedMarkdownHtmlMap.value = nextMarkdownMap
  renderedOutputMarkdownHtmlMap.value = nextOutputMap

  logger.info('[Notebook] sync rendered markdown finished', {
    reason,
    renderVersion,
    markdownCellCount: Object.keys(nextMarkdownMap).length,
    outputMarkdownCount: Object.keys(nextOutputMap).length,
    markdownCacheHitCount,
    outputCacheHitCount,
    markdownRenderCount,
    outputRenderCount,
  })
}

const getRenderedMarkdownCellHtml = (cell) => String(renderedMarkdownHtmlMap.value[String(cell?.key || '')] || '')
const getRenderedMathCellHtml = (cell) => String(renderedMarkdownHtmlMap.value[String(cell?.key || '')] || '')

const getRenderedOutputMarkdownHtml = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  if (!targetModule) return ''
  return String(renderedOutputMarkdownHtmlMap.value[buildCellOutputModuleCacheKey(cell, targetModule)] || '')
}

const handleNotebookSelectionMouseUp = (event) => {
  const target = event?.target instanceof Element ? event.target : null
  if (target?.closest(NOTEBOOK_SELECTION_IGNORE_SELECTOR)) return

  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return

  const { text, surface } = getNotebookSelectionTextFromWindow(target)
  if (!text || !(surface instanceof HTMLElement)) {
    clearNotebookSelectionState('mouseup-empty-selection')
    return
  }

  const rootRect = root.getBoundingClientRect()
  const x = Number(event?.clientX || rootRect.left)
  const y = Number(event?.clientY || rootRect.top)
  const toolbarWidth = 196
  const toolbarHeight = 38
  const pad = 8
  const gap = 10

  let left = x - rootRect.left
  let top = y - rootRect.top - toolbarHeight - gap
  if (top < pad) {
    top = y - rootRect.top + gap
  }

  const maxLeft = Math.max(pad, rootRect.width - toolbarWidth - pad)
  const maxTop = Math.max(pad, rootRect.height - toolbarHeight - pad)

  notebookSelectionText.value = text
  notebookSelectionToolbarStyle.value = {
    left: `${Math.min(Math.max(pad, left), maxLeft)}px`,
    top: `${Math.min(Math.max(pad, top), maxTop)}px`,
  }
  notebookSelectionToolbarVisible.value = true

  logger.info('[NotebookSelection] show toolbar from rendered selection', {
    textLength: text.length,
    tagName: surface.tagName,
  })
}

const handleNotebookGlobalSelectionChange = () => {
  if (!notebookSelectionToolbarVisible.value) return

  const { text } = getNotebookSelectionTextFromWindow()
  if (!text) {
    clearNotebookSelectionState('global-selection-change-empty')
    return
  }

  notebookSelectionText.value = text
}

const handleNotebookGlobalMouseDown = (event) => {
  const target = event?.target instanceof Element ? event.target : null

  // 数学工具下拉菜单：点击菜单/触发按钮以外的任何位置都收起菜单。
  if (activeMathToolMenuKey.value && (!target || !target.closest('.guaxi-notebook__math-menu-anchor'))) {
    closeMathToolMenu()
  }

  if (!notebookSelectionToolbarVisible.value) return

  if (!target) return
  if (target.closest('.guaxi-notebook__selection-toolbar')) return
  if (target.closest('.pot-selection-translate-panel')) return
  if (target.closest(NOTEBOOK_SELECTION_SURFACE_SELECTOR)) return

  clearNotebookSelectionState('global-mousedown-outside')
}

const handleNotebookSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return

  const left = Number(pos.left)
  const top = Number(pos.top)
  if (!Number.isFinite(left) || !Number.isFinite(top)) return

  const current = selectionTranslatePanelPos.value
  if (Number(current.left) === left && Number(current.top) === top) return

  selectionTranslatePanelPos.value = { left, top }
  logger.info('[NotebookSelection] translate panel moved', { left, top })
}

const clampNotebookViewportValue = (value, min, max) => {
  if (!Number.isFinite(value)) return min
  if (!Number.isFinite(min) || !Number.isFinite(max) || max < min) return value
  return Math.min(Math.max(value, min), max)
}

const toggleNotebookSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value
  logger.info('[NotebookSelection] translate panel pin toggled', {
    pinned: selectionTranslatePinned.value,
  })
}

const swapNotebookSelectionTranslateLangs = () => {
  const from = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = from
  logger.info('[NotebookSelection] swap translate languages', {
    from: selectionTranslateFrom.value,
    to: selectionTranslateTo.value,
  })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateNotebookSelectionText(selectionTranslateRaw.value)
  }
}

const closeNotebookSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false
  selectionTranslatePinned.value = false
  selectionTranslateWordMeta.value = null
  logger.info('[NotebookSelection] close translate panel')
}

const openNotebookSelectionTranslatePanel = async () => {
  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }

  // 设计说明：
  // 1. 翻译面板继续定位在 Notebook 根容器内部，和 Explorer 的滚动容器保持同一坐标系。
  // 2. 先复用划词工具条的锚点，再做一次边界钳制，能避免长文档滚动后面板跑出可视区。
  const root = notebookRootRef.value
  const toolbarStyle = notebookSelectionToolbarStyle.value
  const left = Number.parseFloat(String(toolbarStyle.left || ''))
  const top = Number.parseFloat(String(toolbarStyle.top || ''))
  const width = root?.clientWidth ?? window.innerWidth
  const height = root?.clientHeight ?? window.innerHeight
  const panelWidth = 360
  const panelHeight = 260
  const pad = 8

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, Number.isFinite(left) ? left : pad), Math.max(pad, width - panelWidth - pad)),
    top: Math.min(Math.max(pad, (Number.isFinite(top) ? top : pad) + 44), Math.max(pad, height - panelHeight - pad)),
  }
  selectionTranslatePanelVisible.value = true
  hideNotebookSelectionToolbar()

  logger.info('[NotebookSelection] open translate panel', {
    textLength: text.length,
    left: selectionTranslatePanelPos.value.left,
    top: selectionTranslatePanelPos.value.top,
  })

  await translateNotebookSelectionText(text)
}

const sendNotebookSelectionToAssistant = (overrideText, options = {}) => {
  const text = String(overrideText ?? getNotebookSelectionTextForLanguageTasks()).trim()
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

  logger.info('[NotebookSelection] send selection to assistant', {
    textLength: text.length,
    autoSend,
    disableWebSearch,
  })
  return true
}

const resolveNotebookUsageAssistantAnchorPosition = (event = null) => {
  const viewportWidth = Number(window.innerWidth || 0)
  const viewportHeight = Number(window.innerHeight || 0)
  const pad = 12
  const gap = 8
  const preferredWidth = 520
  const minWidth = 320
  const minReadableHeight = 280
  const maxPanelHeight = 760
  const rootRect = notebookRootRef.value?.getBoundingClientRect?.()
  const target = event?.currentTarget instanceof HTMLElement
    ? event.currentTarget
    : (event?.target instanceof HTMLElement ? event.target.closest?.('.guaxi-notebook__icon-btn') : null)
  const targetRect = target?.getBoundingClientRect?.()
  const safeRootLeft = Number.isFinite(rootRect?.left) ? rootRect.left : 0
  const safeRootRight = Number.isFinite(rootRect?.right) ? rootRect.right : viewportWidth
  const safeRootWidth = Math.max(0, safeRootRight - safeRootLeft)
  const panelWidth = Math.min(preferredWidth, Math.max(minWidth, safeRootWidth - pad * 2), Math.max(minWidth, viewportWidth - pad * 2))

  if (!targetRect || !Number.isFinite(targetRect.left) || !viewportWidth || !viewportHeight) {
    return {
      left: Math.max(pad, Math.min(viewportWidth - panelWidth - pad, safeRootRight - panelWidth - pad)),
      top: 56,
      width: panelWidth,
      maxHeight: Math.min(maxPanelHeight, Math.max(240, viewportHeight - 80)),
    }
  }

  // 设计说明：
  // 1. 文档助手由 Cell 内部 icon 触发，面板应出现在触发点附近，而不是固定回 Notebook 顶端。
  // 2. 横向以触发按钮右边缘对齐并夹在中间阅读区内；纵向优先贴近按钮上沿，空间不足时向上避让。
  const minLeft = Math.max(pad, safeRootLeft + pad)
  const maxLeft = Math.min(viewportWidth - panelWidth - pad, safeRootRight - panelWidth - pad)
  const left = clampNotebookViewportValue(targetRect.right - panelWidth, minLeft, Math.max(minLeft, maxLeft))
  const desiredTop = targetRect.top - gap
  const belowRoom = viewportHeight - desiredTop - pad
  const topRoom = targetRect.bottom - pad
  const opensNearTargetBelow = belowRoom >= minReadableHeight || belowRoom >= topRoom
  const maxHeight = Math.min(maxPanelHeight, Math.max(220, opensNearTargetBelow ? belowRoom : topRoom))
  const top = opensNearTargetBelow
    ? clampNotebookViewportValue(desiredTop, pad, Math.max(pad, viewportHeight - 220 - pad))
    : clampNotebookViewportValue(targetRect.bottom + gap - maxHeight, pad, Math.max(pad, viewportHeight - maxHeight - pad))

  return {
    left,
    top,
    width: panelWidth,
    maxHeight,
  }
}

const openNotebookUsageAssistant = (channel = 'python', cell = null, event = null) => {
  const normalizedChannel = channel === 'math' ? 'math' : 'python'
  const targetCell = cell || (normalizedChannel === 'python' ? activeNotebookPythonCell.value : null)
  const anchorPosition = resolveNotebookUsageAssistantAnchorPosition(event)
  notebookUsageAssistantChannel.value = normalizedChannel
  notebookUsageAssistantCellKey.value = String(targetCell?.key || '')
  notebookUsageAssistantAnchorPosition.value = anchorPosition
  notebookUsageAssistantVisible.value = true
  closeMathToolMenu()
  logger.info('[NotebookUsageAssistant] open from notebook cell', {
    channel: normalizedChannel,
    cellKey: String(targetCell?.key || ''),
    cellType: String(targetCell?.type || ''),
    anchorPosition,
  })
}

const closeNotebookUsageAssistant = () => {
  notebookUsageAssistantVisible.value = false
  logger.info('[NotebookUsageAssistant] close panel', {
    channel: notebookUsageAssistantChannel.value,
    cellKey: notebookUsageAssistantCellKey.value,
  })
}

const handleNotebookUsageAssistantInsertExample = ({ channel, example }) => {
  const cell = notebookUsageAssistantContextCell.value
  const code = String(example?.code || '').trim()
  if (!cell || !code) {
    message.warning('当前没有可插入示例的 Cell')
    return
  }

  // 设计说明：
  // 1. Python 官方文档示例通常是一段可执行代码，追加到当前 Cell 可以保留用户已有实验。
  // 2. 数学块示例通常就是一个表达式；为空时直接填入，非空时追加新行，避免覆盖用户正在推导的公式。
  if (channel === 'math' && cell.type === 'math') {
    const current = String(cell.expression || '').trim()
    updateMathCell(cell.key, current ? `${current}\n${code}` : code)
  } else if (cell.type === 'python') {
    const current = String(cell.code || '').replace(/\s+$/g, '')
    updatePythonCell(cell.key, current ? `${current}\n\n${code}` : code)
  } else {
    message.warning('当前 Cell 类型不支持插入该示例')
    return
  }

  logger.info('[NotebookUsageAssistant] inserted example into cell', {
    channel: String(channel || ''),
    cellKey: String(cell.key || ''),
    cellType: String(cell.type || ''),
    codeLength: code.length,
  })
  message.success('示例已插入当前 Cell')
}

const handleNotebookUsageAssistantAsk = ({ prompt }) => {
  const text = String(prompt || '').trim()
  if (!text) {
    message.warning('没有可发送给助手的文档上下文')
    return
  }
  sendNotebookSelectionToAssistant(text, {
    autoSend: false,
    disableWebSearch: true,
    successMessage: '已将离线文档上下文放入智能助手输入框',
  })
  closeNotebookUsageAssistant()
  logger.info('[NotebookUsageAssistant] draft sent to assistant', {
    channel: notebookUsageAssistantChannel.value,
    cellKey: notebookUsageAssistantCellKey.value,
    promptLength: text.length,
  })
}

const askNotebookSelectionWithAssistant = () => {
  sendNotebookSelectionToAssistant(undefined, {
    autoSend: true,
    successMessage: '已发送到助手并开始提问',
  })
}

const sendNotebookSelectionAsAiTranslate = () => {
  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }

  const targetLang = String(selectionTranslateTo.value || 'zh')
  const normalizedTargetLang = targetLang === 'auto' ? 'zh' : targetLang
  const targetLangLabel = getSelectionTranslateLangLabel(normalizedTargetLang)
  const prompt = `请将以下内容翻译成${targetLangLabel}，保持原意，并保留术语、代码、链接与原有格式，只输出译文：\n\n${text}`

  sendNotebookSelectionToAssistant(prompt, {
    autoSend: true,
    disableWebSearch: true,
    successMessage: `已发送到助手并翻译为${targetLangLabel}`,
  })
}

const readNotebookSelectionAloud = async () => {
  if (selectionReadAloudBusy.value) return

  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要朗读的文本')
    return
  }

  selectionReadAloudBusy.value = true
  try {
    const tts = await invoke('get_tts_config')
    const provider = String(tts?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    const model = String(tts?.model || '').trim() || (provider === 'sherpa-onnx'
      ? 'utils/Models/tts/kokoro_multi_lang/model.onnx'
      : 'FunAudioLLM/CosyVoice2-0.5B')
    const tokens = String(tts?.tokens || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(tts?.exe || '').trim() : ''
    const voice = provider === 'sherpa-onnx' ? '0' : 'alex'
    const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'

    logger.info('[NotebookSelection] start text to speech', {
      provider,
      model,
      responseFormat,
      textLength: text.length,
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
    // 设计说明：release 版 WebView 会严格执行 CSP；这里记录 Blob 音频元信息，
    // 便于区分“后端合成异常”和“前端 media-src/解码层拒绝加载”。
    logger.info('[NotebookSelection] text to speech audio prepared', {
      contentType,
      audioBytes: bytes.byteLength,
      responseFormat,
      objectUrlProtocol: selectionAudioObjectUrl.split(':')[0],
    })

    const audio = new Audio(selectionAudioObjectUrl)
    selectionAudioElement = audio
    const waitForPlaybackEnd = new Promise((resolve, reject) => {
      audio.onended = () => {
        stopSelectionAudioPlayback()
        resolve()
      }
      audio.onerror = () => {
        const mediaError = audio.error
        logger.error('[NotebookSelection] audio element load failed', {
          contentType,
          audioBytes: bytes.byteLength,
          mediaErrorCode: mediaError?.code || 0,
          mediaErrorMessage: mediaError?.message || '',
          networkState: audio.networkState,
          readyState: audio.readyState,
          currentSrcProtocol: String(audio.currentSrc || '').split(':')[0],
        })
        stopSelectionAudioPlayback()
        reject(new Error('音频播放失败'))
      }
    })

    logger.info('[NotebookSelection] start audio playback', {
      contentType,
      audioBytes: bytes.byteLength,
    })
    await audio.play()
    await waitForPlaybackEnd
  } catch (error) {
    logger.error('[NotebookSelection] read aloud failed', { error })
    message.error(`朗读失败：${String(error?.message || error || '')}`)
  } finally {
    selectionReadAloudBusy.value = false
  }
}

const translateNotebookSelectionText = async (overrideText) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? getNotebookSelectionTextForLanguageTasks())
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  if (selectionTranslateBusy.value) return

  selectionTranslateRaw.value = text
  selectionTranslateResult.value = ''
  selectionTranslateError.value = ''
  selectionTranslateItems.value = []
  selectionTranslateWordMeta.value = null
  selectionTranslateBusy.value = true

  logger.info('[NotebookSelection] start translate', {
    textLength: text.length,
    from: selectionTranslateFrom.value,
    to: selectionTranslateTo.value,
  })

  try {
    // 词条模块（音标/短语/例句）和译文互不依赖，这里先并发发出去。
    // 原来是「译文 → 基础词条 → 详细词条」串行三跳，慢的时候要等七八秒。
    const needWordMeta = isWordSelection(text)
    const wordBasicPromise = needWordMeta
      ? invoke('language_learning_enrich_word_basic', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value,
      }).catch((error) => {
        logger.warn('[NotebookSelection] enrich basic failed', { error, word: text })
        return null
      })
      : Promise.resolve(null)
    const wordDetailsPromise = needWordMeta
      ? invoke('language_learning_enrich_word_details', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value,
      }).catch((error) => {
        logger.warn('[NotebookSelection] enrich details failed', { error, word: text })
        return null
      })
      : Promise.resolve(null)

    let hasUsableMultiResult = false
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value,
        })

        const items = Array.isArray(response?.items) ? response.items : []
        selectionTranslateItems.value = items.map((item) => ({
          engine: String(item?.engine || ''),
          success: Boolean(item?.success),
          text: String(item?.text || ''),
          error: item?.error ? String(item.error) : '',
        }))

        const primary = selectionTranslateItems.value.find((item) => item.success && String(item.text || '').trim())
        if (primary) {
          selectionTranslateResult.value = String(primary.text || '')
          hasUsableMultiResult = true
          logger.info('[NotebookSelection] multi translate success', {
            engines: selectionTranslateItems.value.length,
          })
        } else {
          throw new Error(String(response?.error || '翻译结果为空'))
        }
      } catch (error) {
        const messageText = String(error?.message || error || '')
        if (messageText.includes('selection_translate_free_multi') && messageText.toLowerCase().includes('not found')) {
          selectionTranslateMultiAvailable.value = false
          if (!selectionTranslateMultiNotFoundWarned) {
            selectionTranslateMultiNotFoundWarned = true
            logger.warn('[NotebookSelection] multi translate command not found, fallback to single path')
          }
        } else {
          logger.warn('[NotebookSelection] multi translate failed, fallback to single path', {
            error,
          })
        }
      }
    }

    if (!hasUsableMultiResult) {
      const response = await invoke('translate_text_free', {
        texts: [text],
        from: selectionTranslateFrom.value,
        to: selectionTranslateTo.value,
      })
      const translated = String(response?.translated_texts?.[0] || '')
      if (!translated.trim() || translated.includes('[Translation Failed]')) {
        throw new Error(String(response?.error || '翻译结果为空'))
      }

      selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
      selectionTranslateResult.value = translated
      logger.info('[NotebookSelection] single translate success', {
        translatedLength: translated.length,
      })
    }

    if (needWordMeta) {
      try {
        const [basic, details] = await Promise.all([wordBasicPromise, wordDetailsPromise])
        const base = {
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
          examples: Array.isArray(basic?.examples) ? basic.examples : [],
          example_translations: [],
        }
        selectionTranslateWordMeta.value = details
          ? {
            ...base,
            synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
            antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
            associates: Array.isArray(details?.associates) ? details.associates : [],
            synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
            antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
            associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
            phrases: Array.isArray(details?.phrases) ? details.phrases : [],
            phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
            examples: Array.isArray(details?.examples) && details.examples.length ? details.examples : base.examples,
            example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
          }
          : base
        logger.info('[NotebookSelection] enriched word translation metadata', {
          word: text,
        })
      } catch (error) {
        logger.warn('[NotebookSelection] enrich word failed', { error, word: text })
      }
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (finalTranslated) {
      try {
        const primaryItem = selectionTranslateItems.value.find((item) => item.success && String(item.text || '').trim())
        await invoke('language_learning_record_translation', {
          input: {
            source_text: text,
            translated_text: finalTranslated,
            source_lang: selectionTranslateFrom.value,
            target_lang: selectionTranslateTo.value,
            engine: String(primaryItem?.engine || 'Free'),
            record_type: isWordSelection(text) ? 'word' : undefined,
            word_meta: selectionTranslateWordMeta.value || undefined,
          },
        })
      } catch (error) {
        logger.warn('[NotebookSelection] record translation failed', { error })
      }
    }
  } catch (error) {
    const messageText = String(error?.message || error || '翻译失败')
    selectionTranslateError.value = messageText
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: messageText }]
    logger.error('[NotebookSelection] translate failed', { error })
    message.error(`翻译失败：${messageText}`)
  } finally {
    selectionTranslateBusy.value = false
  }
}

const copyNotebookSelectionTranslateResult = async (payload) => {
  const text = String((typeof payload === 'string' ? payload : payload?.text) || selectionTranslateResult.value || '')
  if (!text) return

  const ok = await writeClipboardText(text)
  if (ok) {
    message.success('已复制译文', { duration: 1200 })
    logger.info('[NotebookSelection] copied translated text', {
      textLength: text.length,
    })
    return
  }

  message.warning('复制失败：无法访问剪贴板')
}

const handleNotebookRenderedClick = async (event) => {
  const target = event?.target
  const anchor = target instanceof Element ? target.closest('a[href]') : null
  if (!(anchor instanceof HTMLAnchorElement)) return

  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href) return
  if (href.startsWith('#')) {
    event.preventDefault()
    event.stopPropagation()

    const outlineId = decodeURIComponent(href.slice(1))
    const root = notebookRootRef.value
    if (!(root instanceof HTMLElement) || !outlineId) return
    recordNotebookReadingLocation('toc-anchor')

    const escapedId = typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
      ? CSS.escape(outlineId)
      : outlineId.replace(/["\\]/g, '\\$&')
    const heading = findNotebookHeadingTargetByOutlineId(outlineId) || root.querySelector(`#${escapedId}`)
    if (heading instanceof HTMLElement) {
      heading.scrollIntoView({ behavior: 'smooth', block: 'center' })
      logger.info('[Notebook] toc anchor navigated', {
        outlineId,
        filePath: props.filePath,
      })
    }
    return
  }
  if (isNotebookHttpUrl(href)) {
    // 设计说明：http(s) 链接（如交互式图表的“查看交互版本”）必须用系统浏览器打开，
    // 否则会把当前 WebView 页面整个导航走（应用界面被替换）。这里拦截后交给 Tauri opener。
    event.preventDefault()
    event.stopPropagation()
    try {
      await openUrl(href)
    } catch (error) {
      logger.error('[NotebookLinkCard] open external url failed', {
        href,
        error: String(error?.message || error),
      })
      message.error('打开浏览器失败')
    }
    return
  }

  // 设计说明：
  // 1. `guaxi://wikilink` 是应用内协议，浏览器层并没有注册 handler。
  // 2. 如果等到异步解析路径之后再 preventDefault，浏览器会先尝试打开该 scheme 并报错。
  // 3. 因此这里对所有 notebook 内部链接都提前拦截，再交给本地文件跳转逻辑处理。
  event.preventDefault()
  event.stopPropagation()
  logger.info('[NotebookLinkCard] intercept rendered notebook link before browser navigation', {
    href,
    filePath: props.filePath,
  })

  // 设计说明（对应“四个模式对文件链接的处理要完全一致”）：
  // 本地文件链接的打开策略（目录 → 系统文件管理器 / Markdown → 新建 tab 并跳到锚点 /
  // 其它 → 系统文件管理器）统一由宿主 GuaxiEditor 的 openFsLinkSmart 负责。
  // 这里只把原始 href 和当前文档路径抛出去，不再自己解析和打开——否则探索者又会变成
  // 第四套实现（此前它就丢掉了链接里的锚点，打开后不会定位到具体标题）。
  emit('open-local-link', {
    href,
    baseDocPath: String(props.filePath || ''),
  })
}

const patchCellMeta = (cell, patch) => {
  const snapshot = getCellSnapshot(cell)
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    meta: {
      ...(snapshot.meta || {}),
      ...(patch || {}),
    },
  })
  bumpRefresh()
  schedulePersistNotebookSidecar('patch-cell-meta')
}

const toggleCellOutput = (cell) => {
  const snapshot = getCellSnapshot(cell)
  logger.info('[Notebook] toggle output panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextCollapsed: !snapshot.meta.outputCollapsed,
  })
  patchCellMeta(cell, {
    outputCollapsed: !snapshot.meta.outputCollapsed,
  })
}

const togglePythonCellCollapsed = (cell) => {
  const snapshot = getCellSnapshot(cell)
  const nextCollapsed = !snapshot.meta.cellCollapsed
  patchCellMeta(cell, {
    cellCollapsed: nextCollapsed,
  })
  logger.info('[Notebook] toggle python cell collapsed', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    collapsed: nextCollapsed,
  })
}

const toggleCellOutputModule = (cell, moduleId) => {
  const snapshot = getCellSnapshot(cell)
  const nextOutputs = togglePythonCellOutputModuleCollapsed(snapshot?.outputs || [], moduleId)
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    outputs: nextOutputs,
  })
  logger.info('[Notebook] toggle output module panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    outputModuleId: String(moduleId || ''),
    outputModuleCount: nextOutputs.length,
  })
  bumpRefresh()
  schedulePersistNotebookSidecar('toggle-output-module')
}

const deleteCellOutputModule = (cell, moduleId) => {
  const snapshot = getCellSnapshot(cell)
  const nextOutputs = removePythonCellOutputModule(snapshot?.outputs || [], moduleId)
  const latestOutput = getLatestPythonCellOutputModule(nextOutputs) || null
  const latestDisplay = nextOutputs.find((item) => isPythonCellDisplayOutputKind(item?.kind)) || null
  const latestTable = nextOutputs.find((item) => item?.kind === 'table') || null
  const latestText = nextOutputs.find((item) => item?.kind === 'text') || null

  // 设计说明：
  // 1. 删除单个输出模块只改变当前 cell 的输出快照，不触碰源码和会话变量；
  // 2. 同步刷新 legacy 的 outputText/display/dataframe 三个槽位，避免旧逻辑仍拿到已删除模块；
  // 3. 当最后一个输出被删掉时自动折叠输出面板，让过时结果不再留下空卡片。
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    outputText: String(latestText?.text || ''),
    display: latestDisplay?.display || null,
    dataframe: latestTable?.table || null,
    outputs: nextOutputs,
    meta: {
      ...(snapshot?.meta || {}),
      outputCollapsed: !nextOutputs.length,
    },
  })
  logger.info('[Notebook] deleted output module', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    cellType: String(cell?.type || ''),
    outputModuleId: String(moduleId || ''),
    nextLatestOutputModuleId: String(latestOutput?.id || ''),
    outputModuleCount: nextOutputs.length,
  })
  bumpRefresh()
  schedulePersistNotebookSidecar('delete-output-module')
}

// Design notes:
// 1. 数据实验室现在挂在共享右侧栏里，而不是 Python Cell 下方的内联变量卡片。
// 2. 这里仍然把开关状态镜像回 cell meta，保证 notebook sidecar 能稳定记录“该 cell 的实验室面板是否展开”。
const openNotebookPythonSessionPanel = (cell, reason = 'unknown') => {
  const payload = syncNotebookPythonSessionInspector(cell, `open-session-panel:${reason}`)
  rightSiderStore.setPanel('python-session')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, false)
  patchCellMeta(cell, {
    sessionCollapsed: false,
  })
  refreshNotebookPythonEditor(cell, `open-session-panel:${reason}`)
  logger.info('[Notebook] open data lab panel', {
    reason,
    cellId: payload.cellId,
    datasetCount: Array.isArray(payload.datasets) ? payload.datasets.length : 0,
    variableCount: payload.variables.length,
  })
}

const closeNotebookPythonSessionPanel = (cell, reason = 'unknown') => {
  if (rightSiderStore.panel === 'python-session') {
    rightSiderStore.close()
    siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, true)
  }
  if (cell?.type === 'python') {
    patchCellMeta(cell, {
      sessionCollapsed: true,
    })
    refreshNotebookPythonEditor(cell, `close-session-panel:${reason}`)
  }
  logger.info('[Notebook] close data lab panel', {
    reason,
    cellId: String(cell?.cellId || ''),
  })
}

const toggleCellSession = (cell) => {
  const nextVisible = !isNotebookPythonSessionPanelVisible.value
  logger.info('[Notebook] toggle data lab panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextVisible,
  })
  if (nextVisible) {
    openNotebookPythonSessionPanel(cell, 'toolbar-toggle')
    refreshNotebookPythonEditor(cell, 'data-lab-open')
    return
  }
  closeNotebookPythonSessionPanel(cell, 'toolbar-toggle')
  refreshNotebookPythonEditor(cell, 'data-lab-close')
}

// ===========================================================================
// 数学块承载层（右侧「数学工作区」面板）—— 与 Python Cell 的面板完全独立：
//   · 用独立 store mathWorkspaceStore，不读写 rightSiderStore.pythonSessionInspector；
//   · 仅借用通用的 setPanel/open/close 与右侧区域折叠开关；
//   · 通过一次无状态 python_exec 探测会话里的 __GUAXIMATH_WS__ / __GUAXIMATH_CARRIERS__。
// ===========================================================================
const buildMathWorkspaceInspectorCode = () => [
  'import json as __mws_json',
  '__mws_ws = globals().get("__GUAXIMATH_WS__", {}) or {}',
  '__mws_carriers = globals().get("__GUAXIMATH_CARRIERS__", {}) or {}',
  '__mws_syms = []',
  'for __mws_k, __mws_v in __mws_ws.items():',
  '    __mws_syms.append({"name": str(__mws_k), "type": type(__mws_v).__name__})',
  '__mws_out = {"symbols": __mws_syms, "carriers": [__mws_v for __mws_v in __mws_carriers.values() if isinstance(__mws_v, dict)]}',
  'print("__GUAXI_MWS__" + __mws_json.dumps(__mws_out, ensure_ascii=False, default=str))',
  'for __mws_key in ("__mws_json","__mws_ws","__mws_carriers","__mws_syms","__mws_out","__mws_k","__mws_v","__mws_key"):',
  '    globals().pop(__mws_key, None)',
].join('\n')

const syncMathWorkspaceInspector = async (cell, reason = 'unknown') => {
  const sessionId = String(createPythonSessionId(props.filePath) || '').trim()
  const payloadBase = {
    filePath: String(props.filePath || ''),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    stableId: getNotebookCellStableId(cell),
    cellTitle: '',
    sessionId,
    hasActiveCell: !!cell,
  }
  // 先用基础信息占位，保证面板立刻有上下文；再异步探测会话刷新符号 / carrier。
  mathWorkspaceStore.setInspector({
    ...payloadBase,
    workspaceSymbols: mathWorkspaceStore.inspector.workspaceSymbols,
    carriers: mathWorkspaceStore.inspector.carriers,
  })
  if (!sessionId || !cell) return
  try {
    const res = await invoke('python_exec', {
      request: {
        code: buildMathWorkspaceInspectorCode(),
        persist: true,
        expect_json: false,
        include_session_vars: false,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell?.cellId || ''),
        },
      },
    })
    const stdout = String(res?.stdout || '')
    const marker = stdout.indexOf('__GUAXI_MWS__')
    let parsed = { symbols: [], carriers: [] }
    if (marker >= 0) {
      const jsonLine = stdout.slice(marker + '__GUAXI_MWS__'.length).split('\n')[0]
      parsed = JSON.parse(jsonLine)
    }
    mathWorkspaceStore.setInspector({
      ...payloadBase,
      workspaceSymbols: Array.isArray(parsed.symbols) ? parsed.symbols : [],
      carriers: Array.isArray(parsed.carriers) ? parsed.carriers : [],
    })
    logger.info('[NotebookMath] synced math workspace inspector', {
      reason,
      cellKey: payloadBase.cellKey,
      symbolCount: Array.isArray(parsed.symbols) ? parsed.symbols.length : 0,
      carrierCount: Array.isArray(parsed.carriers) ? parsed.carriers.length : 0,
    })
  } catch (error) {
    logger.warn('[NotebookMath] sync math workspace inspector failed', {
      reason,
      error: String(error?.message || error),
    })
  }
}

const openMathWorkspacePanel = (cell, reason = 'unknown') => {
  activeMathWorkspaceCellKey.value = String(cell?.key || '')
  rightSiderStore.setPanel('math-workspace')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, false)
  void syncMathWorkspaceInspector(cell, `open:${reason}`)
  logger.info('[NotebookMath] open math workspace panel', {
    reason,
    cellKey: String(cell?.key || ''),
  })
}

const closeMathWorkspacePanel = (reason = 'unknown') => {
  if (rightSiderStore.panel === 'math-workspace') {
    rightSiderStore.close()
    siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, true)
  }
  logger.info('[NotebookMath] close math workspace panel', { reason })
}

const toggleMathWorkspacePanel = (cell) => {
  const sameCell = String(activeMathWorkspaceCellKey.value || '') === String(cell?.key || '')
  if (isNotebookMathWorkspacePanelVisible.value && sameCell) {
    closeMathWorkspacePanel('toolbar-toggle')
    return
  }
  openMathWorkspacePanel(cell, 'toolbar-toggle')
}

// 文件关闭 / 切换时清理数学块在会话里的状态，保证下次加载是全新数据而不是过时旧值。
// 设计说明：数学状态与 Python Cell 共用同一持久会话，但这里**只清理数学自己的键**
// （__GUAXIMATH_WS__ / __GUAXIMATH_CARRIERS__ / __GUAXIMATH_CARRIER_SEQ__ / 暂存的 __gx_ode_sol_*），
// 不动 Python Cell 的变量、也不丢弃整个会话——保持两套模块解耦。引擎模块与内联版本标记是代码不是数据，保留。
const buildMathSessionClearCode = () => [
  'for __mws_k in ("__GUAXIMATH_WS__", "__GUAXIMATH_CARRIERS__", "__GUAXIMATH_CARRIER_SEQ__"):',
  '    globals().pop(__mws_k, None)',
  'for __mws_n in [__mws_x for __mws_x in list(globals()) if __mws_x.startswith("__gx_ode_sol_")]:',
  '    globals().pop(__mws_n, None)',
  'globals().pop("__mws_k", None)',
  'globals().pop("__mws_n", None)',
].join('\n')

const clearMathSessionState = async (filePath, reason = 'unknown') => {
  const targetPath = String(filePath || '').trim()
  // 先同步清空前端面板状态（与会话无关），让面板立即回到空态。
  if (isNotebookMathWorkspacePanelVisible.value) closeMathWorkspacePanel(`clear:${reason}`)
  mathWorkspaceStore.clear()
  activeMathWorkspaceCellKey.value = ''
  const sessionId = String(createPythonSessionId(targetPath) || '').trim()
  if (!sessionId) return
  try {
    await invoke('python_exec', {
      request: {
        code: buildMathSessionClearCode(),
        persist: true,
        expect_json: false,
        include_session_vars: false,
        vars: { _guaxi_session: sessionId, _doc_path: targetPath },
      },
    })
    logger.info('[NotebookMath] cleared math session state', { reason, sessionId, targetPath })
  } catch (error) {
    logger.warn('[NotebookMath] clear math session state failed', {
      reason,
      targetPath,
      error: String(error?.message || error),
    })
  }
}

const NOTEBOOK_PYTHON_SESSION_BUSY_PREFIX = 'python-session::'
const createNotebookActionRequestId = (prefix = 'nb-task') => [
  prefix,
  Date.now().toString(36),
  Math.random().toString(36).slice(2, 10),
].join('-')
const buildNotebookPythonSessionBusyKey = (filePath = props.filePath) => {
  const sessionId = String(createPythonSessionId(filePath) || '').trim()
  if (!sessionId) return ''
  return `${NOTEBOOK_PYTHON_SESSION_BUSY_PREFIX}${sessionId}`
}
const getNotebookPythonCellIdentity = (cell) => ({
  cellId: String(cell?.cellId || '').trim(),
  cellKey: String(cell?.key || '').trim(),
  stableId: getNotebookCellStableId(cell),
})
const resolveNotebookPythonCellByIdentity = (identity = {}) => {
  const targetCellId = String(identity?.cellId || '').trim()
  const targetCellKey = String(identity?.cellKey || '').trim()
  const targetStableId = String(identity?.stableId || '').trim()
  return notebookCells.value.find((entry) => {
    if (entry?.type !== 'python') return false
    if (targetCellId && String(entry?.cellId || '').trim() === targetCellId) return true
    if (targetCellKey && String(entry?.key || '').trim() === targetCellKey) return true
    return !!targetStableId && getNotebookCellStableId(entry) === targetStableId
  }) || null
}
const getNotebookBusyKeys = (cell) => {
  const cellKey = String(cell?.key || '').trim()
  return cellKey ? [cellKey] : []
}
const getNotebookPythonSessionBusyKeys = (cell) => {
  if (cell?.type !== 'python' && cell?.type !== 'math') return []
  const sessionBusyKey = buildNotebookPythonSessionBusyKey(props.filePath)
  return sessionBusyKey ? [sessionBusyKey] : []
}
// 设计说明：
// 1. 同一份 Notebook 的 Python Cell 共享会话，所以“导入表格/运行代码”的后端占用不能只挂在单个 cell 上。
// 2. 这里同时写入 cell 级 busy 标记和 session 级 busy 标记，这样用户切换到别的 Python Cell 时，UI 仍然知道共享会话正在忙。
// 3. 这样既能避免误判空闲导致的串行请求叠加，也方便排查“切换选中 cell 后结果串写”的问题。
const setNotebookBusyState = (cell, nextBusy, reason = 'unknown') => {
  const busyKeys = [
    ...getNotebookBusyKeys(cell),
    ...getNotebookPythonSessionBusyKeys(cell),
  ]
  if (!busyKeys.length) return
  const nextMap = {
    ...busyMap.value,
  }
  busyKeys.forEach((key) => {
    nextMap[key] = !!nextBusy
  })
  busyMap.value = nextMap
  logger.debug('[Notebook] updated busy state', {
    reason,
    nextBusy: !!nextBusy,
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    busyKeys,
  })
}
const isCellBusy = (cell) => getNotebookBusyKeys(cell).some((key) => !!busyMap.value[key])
const isNotebookPythonSessionBusy = (cell) => getNotebookPythonSessionBusyKeys(cell).some((key) => !!busyMap.value[key])
const isActiveNotebookPythonSessionBusy = computed(() => isNotebookPythonSessionBusy(activeNotebookPythonCell.value))
const isNotebookPythonRunActive = (cell) => {
  const activeRun = notebookActivePythonRun.value
  if (!activeRun || cell?.type !== 'python') return false
  const cellId = String(cell?.cellId || '').trim()
  const cellKey = String(cell?.key || '').trim()
  return (!!cellId && cellId === String(activeRun.cellId || '')) || (!!cellKey && cellKey === String(activeRun.cellKey || ''))
}
const isActiveNotebookPythonRunActive = computed(() => isNotebookPythonRunActive(activeNotebookPythonCell.value))
const cancelNotebookPythonRun = async (cell, reason = 'manual-cancel') => {
  const activeRun = notebookActivePythonRun.value
  if (!activeRun?.requestId) {
    logger.warn('[Notebook] cancel python run ignored because no active run exists', {
      reason,
      cellId: String(cell?.cellId || ''),
      cellKey: String(cell?.key || ''),
    })
    return false
  }

  logger.info('[Notebook] cancelling python cell run', {
    reason,
    requestId: activeRun.requestId,
    sessionId: activeRun.sessionId,
    cellId: activeRun.cellId,
    cellKey: activeRun.cellKey,
  })

  try {
    notebookCancelledPythonRunIds.add(activeRun.requestId)
    await invoke('python_runtime_cancel_request', {
      request: {
        request_id: activeRun.requestId,
        session_id: activeRun.sessionId,
        clear_session: true,
      },
    })
    notebookQueuedPythonAction.value = null
    setNotebookBusyState(cell || activeNotebookPythonCell.value, false, `cancel-python-cell:${activeRun.requestId}`)
    notebookActivePythonRun.value = null
    message.info('已中断当前 Python Cell 运行，并清理后台会话')
    return true
  } catch (error) {
    notebookCancelledPythonRunIds.delete(activeRun.requestId)
    logger.error('[Notebook] cancel python cell run failed', {
      reason,
      requestId: activeRun.requestId,
      sessionId: activeRun.sessionId,
      error: String(error?.message || error),
    })
    message.error(`中断失败：${String(error?.message || error)}`)
    return false
  }
}
const queueNotebookPythonAction = (cell, action, reason = 'unknown') => {
  const targetIdentity = getNotebookPythonCellIdentity(cell)
  const queuedRequestId = createNotebookActionRequestId('nb-queued-python')
  notebookQueuedPythonAction.value = {
    requestId: queuedRequestId,
    action: String(action || ''),
    targetIdentity,
    queuedAt: Date.now(),
    reason: String(reason || 'unknown'),
  }
  logger.info('[Notebook] queued python action', {
    queuedRequestId,
    action: String(action || ''),
    reason: String(reason || 'unknown'),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    targetCellId: targetIdentity.cellId,
    targetCellKey: targetIdentity.cellKey,
    activeImportRequestId: String(notebookPendingPythonImport.value?.requestId || ''),
  })
}

const flushQueuedNotebookPythonActionIfIdle = async (reason = 'unknown') => {
  const queuedAction = notebookQueuedPythonAction.value
  if (!queuedAction?.action) return
  const targetCell = resolveNotebookPythonCellByIdentity(queuedAction.targetIdentity)
  if (!targetCell) {
    logger.warn('[Notebook] drop queued python action because target cell no longer exists', {
      queuedRequestId: String(queuedAction?.requestId || ''),
      action: String(queuedAction?.action || ''),
      reason: String(reason || 'unknown'),
      targetCellId: String(queuedAction?.targetIdentity?.cellId || ''),
      targetCellKey: String(queuedAction?.targetIdentity?.cellKey || ''),
    })
    notebookQueuedPythonAction.value = null
    return
  }
  if (isNotebookPythonSessionBusy(targetCell)) {
    logger.debug('[Notebook] queued python action still waiting for idle session', {
      queuedRequestId: String(queuedAction?.requestId || ''),
      action: String(queuedAction?.action || ''),
      reason: String(reason || 'unknown'),
      cellId: String(targetCell?.cellId || ''),
      cellKey: String(targetCell?.key || ''),
    })
    return
  }

  notebookQueuedPythonAction.value = null
  logger.info('[Notebook] flush queued python action', {
    queuedRequestId: String(queuedAction?.requestId || ''),
    action: String(queuedAction?.action || ''),
    reason: String(reason || 'unknown'),
    cellId: String(targetCell?.cellId || ''),
    cellKey: String(targetCell?.key || ''),
  })

  if (queuedAction.action === 'run') {
    await runPythonCell(targetCell, { writeBack: false })
    return
  }
  if (queuedAction.action === 'run-writeback') {
    await runPythonCell(targetCell, { writeBack: true })
  }
}

const handleActivePythonToolbarAction = async (action) => {
  const targetCell = activeNotebookPythonCell.value
  if (!targetCell) {
    message.warning('请先选中一个 Python Cell')
    return
  }

  activateNotebookCell(targetCell, `toolbar-action:${action}`)

  if (action === 'run' && isNotebookPythonRunActive(targetCell)) {
    await cancelNotebookPythonRun(targetCell, 'toolbar-run-button')
    return
  }

  if ((action === 'run' || action === 'run-writeback') && isNotebookPythonSessionBusy(targetCell)) {
    queueNotebookPythonAction(targetCell, action, `toolbar:${action}:session-busy`)
    message.info(action === 'run'
      ? '当前 Python 会话正在忙，已在空闲后自动运行当前 Cell'
      : '当前 Python 会话正在忙，已在空闲后自动运行并写回当前 Cell')
    return
  }

  if (action === 'import-table') {
    await importTableToPythonSession(targetCell)
    return
  }
  if (action === 'run') {
    await runPythonCell(targetCell, { writeBack: false })
    return
  }
  if (action === 'run-writeback') {
    await runPythonCell(targetCell, { writeBack: true })
    return
  }
  if (action === 'toggle-output') {
    toggleCellOutput(targetCell)
    return
  }
  if (action === 'toggle-session') {
    toggleCellSession(targetCell)
    return
  }
  if (action === 'open-console') {
    emit('open-console')
    return
  }

  logger.warn('[NotebookToolbar] unknown active python action', {
    action,
    cellKey: String(targetCell?.key || ''),
  })
}

const isNotebookShortcutEditableTarget = (target) => (
  target instanceof HTMLElement
  && !!target.closest('input, textarea, select, option, [contenteditable="true"], .monaco-editor textarea')
)

const runNotebookShortcutAction = async (action, event) => {
  logger.info('[NotebookShortcut] trigger notebook shortcut', {
    action,
    key: String(event?.key || ''),
    ctrlKey: !!event?.ctrlKey,
    metaKey: !!event?.metaKey,
    shiftKey: !!event?.shiftKey,
    altKey: !!event?.altKey,
    activeCellKey: String(activeNotebookPythonCellKey.value || ''),
  })
  if (action === 'save') {
    requestNotebookSave()
    return
  }
  if (action === 'search') {
    await openNotebookSearch()
    return
  }
  if (action === 'undo') {
    undoNotebookHistory()
    return
  }
  if (action === 'redo') {
    redoNotebookHistory()
    return
  }
  if (action === 'add-python') {
    appendCell('python')
    return
  }
  if (action === 'add-markdown') {
    appendCell('markdown')
    return
  }
  if (action === 'add-math') {
    appendCell('math')
    return
  }
  if (action === 'import-table') {
    await handleActivePythonToolbarAction('import-table')
    return
  }
  if (action === 'run') {
    await handleActivePythonToolbarAction('run')
    return
  }
  if (action === 'run-writeback') {
    await handleActivePythonToolbarAction('run-writeback')
    return
  }
  if (action === 'toggle-output') {
    await handleActivePythonToolbarAction('toggle-output')
    return
  }
  if (action === 'toggle-session') {
    await handleActivePythonToolbarAction('toggle-session')
    return
  }
  if (action === 'open-console') {
    await handleActivePythonToolbarAction('open-console')
  }
}

const handleNotebookShortcutKeydown = (event) => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return
  const target = event?.target
  if (target instanceof Node && !root.contains(target)) return

  const key = String(event?.key || '').toLowerCase()
  const primary = !!(event?.ctrlKey || event?.metaKey)
  const editableTarget = isNotebookShortcutEditableTarget(target)
  let action = ''

  if (primary && !event.altKey && key === 's') action = 'save'
  else if (primary && !event.altKey && !event.shiftKey && key === 'enter') action = 'run'
  else if (primary && !event.altKey && event.shiftKey && key === 'enter') action = 'run-writeback'
  else if (!editableTarget && primary && !event.altKey && !event.shiftKey && key === 'f') action = 'search'
  else if (!editableTarget && primary && !event.altKey && !event.shiftKey && key === 'z') action = 'undo'
  else if (!editableTarget && primary && !event.altKey && !event.shiftKey && key === 'y') action = 'redo'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'p') action = 'add-python'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'm') action = 'add-markdown'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'k') action = 'add-math'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'i') action = 'import-table'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'o') action = 'toggle-output'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 'l') action = 'toggle-session'
  else if (!editableTarget && event.altKey && !primary && !event.shiftKey && key === 't') action = 'open-console'

  if (!action) return
  event.preventDefault()
  event.stopPropagation()
  void runNotebookShortcutAction(action, event)
}

const getPythonCellOrderIndex = (cell) => {
  const targetKey = String(cell?.key || '')
  if (!targetKey) return -1
  return notebookCells.value
    .filter((item) => item?.type === 'python')
    .findIndex((item) => String(item?.key || '') === targetKey)
}

// 设计说明：
// 1. Explorer 模式会先在前端内存里生成稳定 meta.id，随后再把它写回隐藏 Python Cell 源码。
// 2. 老文档第一次进入探索模式时，源码块里可能还没有 meta.id，这时如果只按 cellId 回找很容易失败。
// 3. 因此这里优先用稳定 id，找不到时再兼容旧 cellId，最后按当前 Python Cell 的顺序兜底。
const resolveSourcePythonBlock = (cell, markdown) => {
  const blocks = parsePythonCellBlocks(markdown)
  const stableId = String(cell?.meta?.id || '').trim()
  if (stableId) {
    const stableBlock = blocks.find((block) => String(block?.meta?.id || '').trim() === stableId)
    if (stableBlock) {
      return {
        block: stableBlock,
        blocks,
        matchStrategy: 'meta-id',
      }
    }
  }

  const cellId = String(cell?.cellId || '').trim()
  if (cellId) {
    const directBlock = blocks.find((block) => createPythonCellId(props.filePath, block) === cellId)
    if (directBlock) {
      return {
        block: directBlock,
        blocks,
        matchStrategy: 'cell-id',
      }
    }
  }

  const orderIndex = getPythonCellOrderIndex(cell)
  if (orderIndex >= 0 && blocks[orderIndex]) {
    return {
      block: blocks[orderIndex],
      blocks,
      matchStrategy: 'python-order',
    }
  }

  return {
    block: null,
    blocks,
    matchStrategy: 'missing',
  }
}

const ensureSourcePythonBlockHasStableId = (markdown, cell, resolved) => {
  const stableId = String(cell?.meta?.id || '').trim()
  const targetBlock = resolved?.block || null
  if (!stableId || !targetBlock) {
    return {
      markdown,
      block: targetBlock,
      blocks: resolved?.blocks || [],
      persisted: false,
    }
  }

  if (String(targetBlock?.meta?.id || '').trim() === stableId) {
    return {
      markdown,
      block: targetBlock,
      blocks: resolved?.blocks || [],
      persisted: false,
    }
  }

  const nextMarkdown = replacePythonCellBlock(
    markdown,
    targetBlock,
    String(targetBlock.code || ''),
    {
      ...(targetBlock.meta || {}),
      id: stableId,
    },
  )
  const reparsed = resolveSourcePythonBlock(cell, nextMarkdown)
  logger.info('[Notebook] persist stable python cell id into markdown', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId,
    previousMatchStrategy: String(resolved?.matchStrategy || 'unknown'),
    nextMatchStrategy: String(reparsed?.matchStrategy || 'unknown'),
  })
  return {
    markdown: nextMarkdown,
    block: reparsed.block,
    blocks: reparsed.blocks,
    persisted: true,
  }
}

const writeMarkdownOutputBlock = (markdown, block, cellId, markdownOutput) => {
  const startMarker = `<!-- guaxi:pyout:${cellId}:start -->`
  const endMarker = `<!-- guaxi:pyout:${cellId}:end -->`
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const blockEndIndex = Math.max(0, Number(block?.endLine || 1) - 1)
  const existingStart = lines.findIndex((line) => String(line || '').trim() === startMarker)
  const nextChunk = ['', startMarker, ...String(markdownOutput || '').replace(/\r\n?/g, '\n').split('\n'), endMarker]

  if (existingStart >= 0) {
    const existingEnd = lines.findIndex((line, index) => index > existingStart && String(line || '').trim() === endMarker)
    if (existingEnd >= 0) {
      lines.splice(existingStart, existingEnd - existingStart + 1, ...nextChunk)
      return lines.join('\n')
    }
  }

  lines.splice(blockEndIndex + 1, 0, ...nextChunk)
  return lines.join('\n')
}

const buildNotebookAssetTarget = (cellId, extension) => {
  const safeStem = getNotebookDocStem()
  const safeCellId = sanitizeFileSegment(cellId, 'cell')
  const safeExt = String(extension || 'bin').replace(/^\.+/, '') || 'bin'
  const fileName = `${safeStem}-${safeCellId}-output.${safeExt}`
  const absolutePath = getNotebookDocDir()
    ? joinNotebookPath(getNotebookDocDir(), fileName)
    : fileName
  return {
    absolutePath,
    relativePath: encodeURI(getRelativeNotebookPath(absolutePath)),
    fileName,
  }
}

const serializeDisplayAssetForMarkdown = async (display, cellId) => {
  const kind = normalizeDisplayType(display)
  const sourcePath = getDisplaySourcePath(display)
  const mime = String(display?.mime || '').trim()
  const dataUrl = getDisplayDataUrl(display)
  let bytes = null
  let assetMime = mime || 'application/octet-stream'

  if (dataUrl) {
    const matched = dataUrl.match(/^data:([^;]+);base64,(.+)$/i)
    if (!matched) {
      return {
        kind,
        reference: dataUrl,
        assetPath: '',
      }
    }
    assetMime = matched[1] || assetMime
    bytes = base64ToUint8Array(matched[2] || '')
  } else if (sourcePath) {
    bytes = await readBytesFromPath(sourcePath, 'markdown-writeback-source-asset')
  } else {
    return null
  }

  const extension = guessExtensionFromMime(
    assetMime,
    kind === 'image' ? 'png' : kind === 'audio' ? 'mp3' : kind === 'video' ? 'mp4' : 'bin',
  )
  const resourceType = kind || 'file'
  const stored = await registerResourceFile({
    bytes,
    resourceType,
    extension,
    ownerFilePath: props.filePath,
    cellId,
    source: 'python-cell-output',
  })
  return {
    kind,
    reference: stored.url,
    assetPath: stored.absolutePath,
  }
}

// 设计说明（交互式图表静态化）：
// 1. plotly 等交互输出的 HTML 片段里已内联 plotly.js（include_plotlyjs=True），
//    因此可以挂进一个隐藏 iframe，等 Plotly 完成首帧绘制后用 Plotly.toImage 取静态 PNG。
// 2. 这样源 Markdown 只插入 ![](图片url)（+ 交互版链接），绝不内联大段 HTML/JS。
// 3. 与 GuaxiEditor 的 rasterizeMermaidSvgToPng 思路一致：浏览器栅格化 + 资源入库。
const rasterizePlotlyHtmlToPng = async (html, { timeoutMs = 8000 } = {}) => {
  const fragment = String(html || '')
  if (!fragment.trim()) return null
  const iframe = document.createElement('iframe')
  iframe.setAttribute('aria-hidden', 'true')
  iframe.style.position = 'fixed'
  iframe.style.left = '-10000px'
  iframe.style.top = '0'
  iframe.style.width = '1024px'
  iframe.style.height = '768px'
  iframe.style.border = '0'
  iframe.style.visibility = 'hidden'
  iframe.srcdoc = `<!DOCTYPE html><html><head><meta charset="utf-8"><style>html,body{margin:0;padding:0;background:#fff;}</style></head><body>${fragment}</body></html>`
  document.body.appendChild(iframe)
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('plotly iframe load timeout')), timeoutMs)
      iframe.onload = () => { clearTimeout(timer); resolve() }
      iframe.onerror = () => { clearTimeout(timer); reject(new Error('plotly iframe load failed')) }
    })
    const win = iframe.contentWindow
    const docRef = iframe.contentDocument || win?.document
    if (!win || !docRef) return null
    // 轮询等待 Plotly 加载并完成首帧绘制（gd._fullLayout 出现即认为已绘制）。
    const deadline = Date.now() + timeoutMs
    let gd = null
    while (Date.now() < deadline) {
      gd = docRef.querySelector('.plotly-graph-div')
      if (win.Plotly && gd && gd._fullLayout) break
      await new Promise((r) => setTimeout(r, 120))
    }
    if (!win.Plotly || !gd || !gd._fullLayout) return null
    const width = Math.max(1, Math.round(gd.clientWidth || gd._fullLayout?.width || 1024))
    const height = Math.max(1, Math.round(gd.clientHeight || gd._fullLayout?.height || 768))
    const dataUrl = await win.Plotly.toImage(gd, { format: 'png', width, height, scale: 2 })
    const matched = String(dataUrl || '').match(/^data:[^;]+;base64,(.+)$/i)
    if (!matched) return null
    return base64ToUint8Array(matched[1])
  } finally {
    iframe.remove()
  }
}

// 把交互式 HTML（字节）整体写入资源库，返回可访问 URL；用于「查看交互版本」链接。
const registerInteractiveHtmlResource = async (htmlOrBytes, cellId) => {
  const bytes = htmlOrBytes instanceof Uint8Array
    ? htmlOrBytes
    : new TextEncoder().encode(String(htmlOrBytes || ''))
  const stored = await registerResourceFile({
    bytes,
    resourceType: 'file',
    extension: 'html',
    ownerFilePath: props.filePath,
    cellId,
    source: 'python-cell-interactive',
  })
  return stored.url
}

const ensureNotebookTableRowsMaterialized = async (table = null) => {
  if (!table || typeof table !== 'object') return null
  const expectedRowCount = Math.max(0, Number(table.rowCount || 0))
  const existingRows = Array.isArray(table.rows) ? table.rows : []
  if (!String(table.resultHandle || '').trim() || existingRows.length >= expectedRowCount) {
    return table
  }

  logger.info('[Notebook] materialize full table rows for downstream action', {
    tableKey: String(table.tableKey || ''),
    tableName: String(table.tableName || ''),
    expectedRowCount,
  })

  const rows = []
  const pageSize = 1000
  for (let offset = 0; offset < expectedRowCount; offset += pageSize) {
    const response = await invoke('python_runtime_get_table_rows', {
      request: {
        handle: String(table.resultHandle || ''),
        sessionId: String(table.sessionId || ''),
        tableKey: String(table.tableKey || ''),
        manifestPath: String(table.manifestPath || ''),
        offset,
        limit: Math.min(pageSize, expectedRowCount - offset),
      },
    })
    if (!response?.ok) {
      throw new Error(String(response?.error || '读取完整表格数据失败'))
    }
    const nextRows = Array.isArray(response?.rows) ? response.rows : []
    rows.push(...nextRows)
  }

  return {
    ...table,
    rows,
  }
}

// 交互式图表写回标记：居中静态图 + 右上角小图标（点击用系统浏览器打开交互版）。
// 设计说明：
// 1. 用自包含内联样式的 HTML，保证预览 / 探索者 / 导出三处都一致渲染（不依赖外部 CSS class）。
// 2. 链接带 target="_blank"，且点击由 notebook/preview 的链接拦截器统一改走系统浏览器，
//    不会把当前 WebView 页面导航走。
// 3. 图标用内联 SVG（外部链接图标），不依赖图标库，导出到 Word/EPUB 时也能显示。
const buildInteractiveArtifactMarkup = (pngUrl, htmlUrl) => {
  const safePng = String(pngUrl || '').trim()
  const safeHtml = String(htmlUrl || '').trim()
  if (!safePng && !safeHtml) return '> 交互式图表无法写回（资源入库失败）'

  const externalIcon = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path><polyline points="15 3 21 3 21 9"></polyline><line x1="10" y1="14" x2="21" y2="3"></line></svg>'

  // 设计说明（icon 排布 · 对应问题 1）：
  // 1. 图片右上角被多个承载层的按钮占用：预览模式的媒体「查看/导出」按钮、
  //    探索者模式的 cell 删除按钮、代码块复制按钮，全部固定贴在右上角。
  // 2. 因此把「查看交互版本」入口放到图片右下角——那里没有任何按钮竞争，
  //    在预览模式和探索者模式下都不会与右上角按钮重叠。
  const iconLink = safeHtml
    ? `<a class="guaxi-figure-action--interactive" href="${escapeHtmlAttribute(safeHtml)}" target="_blank" rel="noopener noreferrer" title="在浏览器中查看交互版本" aria-label="在浏览器中查看交互版本" style="position:absolute;bottom:10px;right:10px;display:inline-flex;align-items:center;justify-content:center;width:30px;height:30px;border-radius:8px;background:rgba(15,23,42,0.62);color:#fff;text-decoration:none;box-shadow:0 4px 12px rgba(15,23,42,0.18);backdrop-filter:blur(2px);z-index:11;">${externalIcon}</a>`
    : ''

  if (!safePng) {
    // 没有静态图时，单独居中显示交互版入口（带文案，避免孤立图标无上下文）。
    return `<div style="text-align:center;margin:8px 0;"><a class="guaxi-figure-action--interactive" href="${escapeHtmlAttribute(safeHtml)}" target="_blank" rel="noopener noreferrer" title="在浏览器中查看交互版本" aria-label="在浏览器中查看交互版本" style="display:inline-flex;align-items:center;gap:6px;color:var(--guaxi-text-secondary,#667085);text-decoration:none;">${externalIcon}<span>查看交互版本</span></a></div>`
  }

  // 有静态图：静态图居中显示，交互版入口固定在静态图右下角。
  return `<div style="text-align:center;margin:8px 0;"><span class="guaxi-interactive-figure" style="position:relative;display:inline-block;max-width:100%;"><img src="${escapeHtmlAttribute(safePng)}" alt="交互图表静态预览" style="max-width:100%;height:auto;display:block;" />${iconLink}</span></div>`
}

const buildMarkdownOutputFromModule = async (cell, outputModule = null, snapshot = null) => {
  const resolvedSnapshot = snapshot || getCellSnapshot(cell)
  const targetModule = outputModule || getCellOutputModule(cell)

  if (targetModule?.kind === 'table' && targetModule.table) {
    const materializedTable = await ensureNotebookTableRowsMaterialized(targetModule.table)
    return dataframeToMarkdownTable(materializedTable)
  }

  const display = targetModule?.display || resolvedSnapshot?.display || null
  const displayType = targetModule?.kind || normalizeDisplayType(display)
  const cellId = String(cell?.cellId || cell?.key || '')
  if (displayType === 'markdown' && display?.markdown) return String(display.markdown)

  // 设计说明（统一产物写回 · 交互式 HTML）：
  // plotly 等交互输出绝不再把整段 HTML/JS 内联进源 Markdown。改为：
  //   1) 把交互图栅格化成静态 PNG → 资源库 → 居中显示的静态图；
  //   2) 把交互 HTML 整体入库 → 在静态图右上角放一个小图标按钮，点击用系统浏览器打开交互版。
  // 两者任一失败都降级，但永不内联原始 HTML。
  if (displayType === 'html' && display?.html) {
    let pngUrl = ''
    let htmlUrl = ''
    try {
      const png = await rasterizePlotlyHtmlToPng(String(display.html))
      if (png) {
        const stored = await registerResourceFile({
          bytes: png,
          resourceType: 'image',
          extension: 'png',
          ownerFilePath: props.filePath,
          cellId,
          source: 'python-cell-output',
        })
        pngUrl = stored.url
      }
    } catch (error) {
      logger.warn('[Notebook] rasterize interactive html to png failed, keep interactive link only', {
        cellId,
        error: String(error?.message || error),
      })
    }
    try {
      htmlUrl = await registerInteractiveHtmlResource(String(display.html), cellId)
    } catch (error) {
      logger.error('[Notebook] register interactive html resource failed', {
        cellId,
        error: String(error?.message || error),
      })
    }
    return buildInteractiveArtifactMarkup(pngUrl, htmlUrl)
  }
  if (displayType === 'html' && getDisplaySourcePath(display)) {
    // 基于文件的交互 HTML（如 matplotlib htmlwriter 动画）：整文件入库后插入链接，
    // 不再用相对 iframe 引用（文档移动即失效），也保持源 Markdown 干净。
    const sourcePath = getDisplaySourcePath(display)
    try {
      const bytes = await readBytesFromPath(sourcePath, 'markdown-writeback-html-source')
      const htmlUrl = await registerInteractiveHtmlResource(bytes, cellId)
      return `[查看交互版本](${htmlUrl})`
    } catch (error) {
      logger.error('[Notebook] register interactive html file failed', {
        cellId,
        error: String(error?.message || error),
      })
      return '> 交互式图表无法写回（资源入库失败）'
    }
  }
  if (displayType === 'svg' && display?.svg) return String(display.svg)

  if (['image', 'audio', 'video'].includes(displayType)) {
    const asset = await serializeDisplayAssetForMarkdown(display, cellId)
    if (asset?.reference) {
      if (displayType === 'image') {
        // 图片走 Markdown 图片语法；纯图片段落的居中由渲染器 CSS（:has(img)）统一处理。
        return `![Python Cell 输出](${asset.reference})`
      }
      // 音频 / 视频用居中容器包裹，与图片布局保持一致（CSS 的 :has(img) 命中不到这两类，
      // 因此这里显式用内联样式的 <div> 居中，预览 / 探索者 / 导出三处都生效）。
      if (displayType === 'audio') {
        return `<div style="text-align:center;margin:8px 0;"><audio controls src="${escapeHtmlAttribute(asset.reference)}"></audio></div>`
      }
      if (displayType === 'video') {
        return `<div style="text-align:center;margin:8px 0;"><video controls src="${escapeHtmlAttribute(asset.reference)}" style="max-width:100%;height:auto;"></video></div>`
      }
    }
  }

  // 其它产物（附件 / 未识别的二进制）：统一入资源库后以链接插入，绝不内联；同样居中显示。
  if (display && (getDisplaySourcePath(display) || getDisplayDataUrl(display))) {
    const asset = await serializeDisplayAssetForMarkdown(display, cellId)
    if (asset?.reference) {
      const name = String(getDisplaySourcePath(display) || '').split(/[\\/]/).pop() || '附件'
      return `<div style="text-align:center;margin:8px 0;"><a href="${escapeHtmlAttribute(asset.reference)}">${escapeHtmlText(name)}</a></div>`
    }
  }

  const text = String(targetModule?.text || resolvedSnapshot?.outputText || '').trim()
  if (!text) return ''
  return `\`\`\`text\n${text}\n\`\`\``
}

// 把某个输出模块的内容作为一个新的 Markdown Cell 插入到当前 Cell 之后。
// 设计说明：
// 1. 这是用户显式触发的“写回 Markdown”——与“进入探索者模式自动写 $$”是两码事，后者已禁止。
// 2. Math Cell 没有 Python Cell 的受管 <!--output--> 块机制（writeCellOutputToMarkdown 依赖
//    python 源块定位），因此数学输出写回采用“插入新 Markdown Cell”的方式。
// 3. 产物统一经 buildMarkdownOutputFromModule（绘图入资源库转图片、文本转代码块、LaTeX 转 $$ 等）。
const insertOutputMarkdownAfterCell = async (cell, moduleId = '') => {
  flushPendingNotebookMarkdown('before-insert-output-markdown-after-cell')
  const snapshot = getCellSnapshot(cell)
  const outputModule = getCellOutputModule(cell, moduleId)
  const markdownOutput = await buildMarkdownOutputFromModule(cell, outputModule, snapshot)
  if (!markdownOutput) {
    message.warning('当前输出暂时无法写回为 Markdown')
    return
  }

  const targetIndex = notebookCells.value.findIndex((entry) => String(entry?.key || '') === String(cell?.key || ''))
  if (targetIndex < 0) {
    logger.error('[Notebook] failed to insert output markdown because source cell is missing', {
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      moduleId: String(outputModule?.id || moduleId || ''),
    })
    message.error('没有找到要写回的源 Cell')
    return
  }

  const outputCell = createNotebookMarkdownCell(markdownOutput)
  const nextCells = notebookCells.value.slice()
  nextCells.splice(targetIndex + 1, 0, outputCell)
  logger.info('[Notebook] insert output markdown cell after executable cell', {
    sourceType: String(cell?.type || ''),
    sourceKey: String(cell?.key || ''),
    sourceCellId: String(cell?.cellId || ''),
    outputCellKey: String(outputCell?.key || ''),
    moduleId: String(outputModule?.id || moduleId || ''),
    markdownLength: markdownOutput.length,
  })
  emitCells(nextCells, outputCell.key)
  message.success('输出已写回为新的 Markdown Cell')
}

const getTableExportFilters = (table = {}, cell = null) => {
  const totalTables = Number(table?.tableCount || 1)
  const baseStem = buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'xlsx').fileName.replace(/\.[^.]+$/, '')
  const preferredSheetName = String(table?.tableName || table?.tableKey || 'table').trim().replace(/[^\w.-]+/g, '_')
  const defaultBase = preferredSheetName ? `${baseStem}-${preferredSheetName}` : baseStem
  const filters = []

  if (totalTables > 1) {
    filters.push({ name: 'Excel 工作簿', extensions: ['xlsx'] })
    filters.push({ name: 'JSON 工作簿', extensions: ['json'] })
  }
  filters.push({ name: 'CSV', extensions: ['csv'] })
  filters.push({ name: 'TSV', extensions: ['tsv'] })
  filters.push({ name: 'JSON', extensions: ['json'] })
  filters.push({ name: 'Excel', extensions: ['xlsx'] })
  filters.push({ name: 'Parquet', extensions: ['parquet'] })
  filters.push({ name: 'SQLite 数据库', extensions: ['sqlite', 'db'] })
  filters.push({ name: 'DuckDB 数据库', extensions: ['duckdb'] })

  return {
    defaultBase,
    filters,
  }
}

const buildExportPayloadFromModule = async (cell, outputModule = null, snapshot = null) => {
  const resolvedSnapshot = snapshot || getCellSnapshot(cell)
  const targetModule = outputModule || getCellOutputModule(cell)

  if (targetModule?.kind === 'table' && targetModule.table) {
    if (String(targetModule.table.resultHandle || '').trim()) {
      const { defaultBase, filters } = getTableExportFilters(targetModule.table, cell)
      return {
        kind: 'python-table-handle',
        defaultName: `${defaultBase}.xlsx`,
        filters,
        bytes: null,
        summary: 'python-table-handle',
        table: targetModule.table,
      }
    }

    const csvLines = [
      (targetModule.table.columns || []).join(','),
      ...(targetModule.table.rows || []).map((row) => (Array.isArray(row) ? row : []).map((value) => JSON.stringify(value ?? '')).join(',')),
    ].join('\n')
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'csv').fileName,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
      bytes: new TextEncoder().encode(csvLines),
      summary: 'dataframe',
    }
  }

  const display = targetModule?.display || resolvedSnapshot?.display || null
  const displayType = targetModule?.kind || normalizeDisplayType(display)
  if (displayType === 'markdown' && display?.markdown) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'md').fileName,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
      bytes: new TextEncoder().encode(String(display.markdown || '')),
      summary: 'markdown',
    }
  }
  if (displayType === 'html' && display?.html) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'html').fileName,
      filters: [{ name: 'HTML', extensions: ['html'] }],
      bytes: new TextEncoder().encode(String(display.html || '')),
      summary: 'html',
    }
  }
  if (displayType === 'html' && getDisplaySourcePath(display)) {
    const sourcePath = getDisplaySourcePath(display)
    return {
      defaultName: sourcePath.split(/[\\/]/).pop() || buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'html').fileName,
      filters: [{ name: 'HTML', extensions: ['html'] }],
      bytes: null,
      summary: sourcePath,
    }
  }
  if (displayType === 'svg' && display?.svg) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'svg').fileName,
      filters: [{ name: 'SVG', extensions: ['svg'] }],
      bytes: new TextEncoder().encode(String(display.svg || '')),
      summary: 'svg',
    }
  }
  if (['image', 'audio', 'video'].includes(displayType)) {
    const dataUrl = getDisplayDataUrl(display)
    if (dataUrl) {
      const matched = dataUrl.match(/^data:([^;]+);base64,(.+)$/i)
      if (matched) {
        const extension = guessExtensionFromMime(
          matched[1] || display?.mime,
          displayType === 'image' ? 'png' : displayType === 'audio' ? 'mp3' : 'mp4',
        )
        return {
          defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), extension).fileName,
          filters: [{ name: String(matched[1] || display?.mime || 'Binary'), extensions: [extension] }],
          bytes: base64ToUint8Array(matched[2] || ''),
          summary: displayType,
        }
      }
    }

    const sourcePath = getDisplaySourcePath(display)
    if (sourcePath) {
      return {
        defaultName: sourcePath.split(/[\\/]/).pop() || buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'bin').fileName,
        filters: undefined,
        bytes: null,
        summary: sourcePath,
      }
    }
  }

  const text = String(targetModule?.text || resolvedSnapshot?.outputText || '').trim()
  if (!text) return null
  return {
    defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'txt').fileName,
    filters: [{ name: 'Text', extensions: ['txt'] }],
    bytes: new TextEncoder().encode(text),
    summary: 'text',
  }
}

const copyPythonCellTextOutput = async (cell, moduleId = '') => {
  const outputModule = moduleId ? getCellOutputModule(cell, moduleId) : getLatestTextOutputModule(cell)
  const text = String(outputModule?.text || '')
  if (!text) {
    message.warning('当前没有可复制的文本输出')
    return
  }

  const copied = await writeClipboardText(text)
  if (!copied) {
    message.error('复制文本输出失败')
    return
  }

  logger.info('[Notebook] copied python cell text output', {
    cellId: String(cell?.cellId || ''),
    outputModuleId: String(outputModule?.id || ''),
    textLength: text.length,
  })
  message.success('文本输出已复制')
}

const copyCellOutputModuleText = async (cell, moduleId = '') => {
  const outputModule = getCellOutputModule(cell, moduleId)
  const text = buildOutputModuleCopyText(outputModule)
  if (!text) {
    message.warning('当前输出没有可复制的文字或 LaTeX 源码')
    return
  }

  const copied = await writeClipboardText(text)
  if (!copied) {
    message.error('复制输出失败')
    return
  }

  logger.info('[Notebook] copied output module markdown text', {
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    cellType: String(cell?.type || ''),
    outputModuleId: String(outputModule?.id || moduleId || ''),
    outputKind: String(outputModule?.kind || ''),
    textLength: text.length,
  })
  message.success('输出文字和 LaTeX 源码已复制')
}

const writeCellOutputToMarkdown = async (cell, moduleId = '') => {
  flushPendingNotebookMarkdown('before-writeback')
  const snapshot = getCellSnapshot(cell)
  const outputModule = getCellOutputModule(cell, moduleId)
  const markdownOutput = await buildMarkdownOutputFromModule(cell, outputModule, snapshot)
  if (!markdownOutput) {
    message.warning('当前输出暂时无法写回到 Markdown')
    return
  }

  const initialMarkdown = normalizeNotebookMarkdownText(lastEmittedMarkdown.value || props.markdown)
  const resolvedBlock = resolveSourcePythonBlock(cell, initialMarkdown)
  const normalized = ensureSourcePythonBlockHasStableId(initialMarkdown, cell, resolvedBlock)
  const currentMarkdown = normalized.markdown
  const targetBlock = normalized.block
  if (!targetBlock) {
    logger.error('[Notebook] failed to resolve source python block for markdown writeback', {
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      stableId: String(cell?.meta?.id || ''),
      matchStrategy: String(resolvedBlock?.matchStrategy || 'missing'),
    })
    message.error('没有找到对应的 Python Cell 源区块')
    return
  }

  const nextMarkdown = writeMarkdownOutputBlock(currentMarkdown, targetBlock, String(cell.cellId), markdownOutput)
  logger.info('[Notebook] write cell output back to markdown', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId: String(cell?.meta?.id || ''),
    outputModuleId: String(outputModule?.id || ''),
    matchStrategy: String(resolvedBlock?.matchStrategy || 'unknown'),
    persistedStableId: !!normalized.persisted,
    outputKind: String(outputModule?.kind || normalizeDisplayType(snapshot?.display) || (snapshot?.dataframe ? 'dataframe' : 'text')),
  })
  emit('update:markdown', nextMarkdown)
  lastEmittedMarkdown.value = normalizeNotebookMarkdownText(nextMarkdown)
  schedulePersistNotebookSidecar('write-cell-output-to-markdown')
  message.success('Python Cell 输出已写回到源 Markdown')
}

const exportCellOutputToFile = async (cell, moduleId = '') => {
  try {
    const snapshot = getCellSnapshot(cell)
    const outputModule = getCellOutputModule(cell, moduleId)
    const payload = await buildExportPayloadFromModule(cell, outputModule, snapshot)
    if (!payload) {
      message.warning('当前没有可导出的输出内容')
      return
    }

    if (payload.kind === 'python-table-handle' && payload.table) {
      const outputPath = await save({
        defaultPath: payload.defaultName,
        filters: payload.filters,
      })
      if (!outputPath) return

      const extension = String(outputPath).split('.').pop()?.trim().toLowerCase() || 'xlsx'
      const exportScope = Number(payload.table?.tableCount || 1) > 1 && ['xlsx', 'json'].includes(extension)
        ? 'workbook'
        : 'table'

      logger.info('[Notebook] start export handled python table result', {
        cellId: String(cell?.cellId || ''),
        outputModuleId: String(outputModule?.id || ''),
        tableKey: String(payload.table?.tableKey || ''),
        tableCount: Number(payload.table?.tableCount || 1),
        format: extension,
        exportScope,
        outputPath,
      })

      const response = await invoke('python_runtime_export_table_result', {
        request: {
          handle: String(payload.table?.resultHandle || ''),
          sessionId: String(payload.table?.sessionId || ''),
          tableKey: String(payload.table?.tableKey || ''),
          manifestPath: String(payload.table?.manifestPath || ''),
          outputPath: String(outputPath),
          format: extension,
          scope: exportScope,
        },
      })
      if (!response?.ok) {
        throw new Error(String(response?.error || '导出表格失败'))
      }

      logger.info('[Notebook] export handled python table result completed', {
        cellId: String(cell?.cellId || ''),
        outputModuleId: String(outputModule?.id || ''),
        tableKey: String(payload.table?.tableKey || ''),
        format: extension,
        exportScope,
        outputPath,
      })
      message.success('当前表格输出已导出到文件')
      return
    }

    if (!payload.bytes) {
      const outputPath = await save({
        defaultPath: payload.defaultName,
        filters: payload.filters,
      })
      if (!outputPath) return

      // 设计说明：动画/音视频/HTML 等输出的源文件位于会话沙箱 .guaxi-display-artifacts 目录，
      // 不在 plugin-fs 的作用域内。统一改用后端命令 readBytesFromPath(read_file_bytes_base64) 读取，
      // 与 GuaxiNotebookHtmlOutput / 写回逻辑一致，确保 debug/release 都能把 mp4/gif 像图片一样另存。
      const sourceBytes = await readBytesFromPath(String(payload.summary || ''), 'export-existing-file')
      await writeBytesToPath(outputPath, sourceBytes, 'export-existing-file')
      logger.info('[Notebook] export existing output file to new path', {
        cellId: String(cell?.cellId || ''),
        sourcePath: payload.summary,
        outputPath,
      })
      message.success('当前输出文件已另存为新文件')
      return
    }

    const outputPath = await save({
      defaultPath: payload.defaultName,
      filters: payload.filters,
    })
    if (!outputPath) return

    await writeBytesToPath(outputPath, payload.bytes, 'export-payload')
    logger.info('[Notebook] export cell output to file', {
      cellId: String(cell?.cellId || ''),
      outputPath,
      summary: payload.summary,
    })
    message.success('当前输出已导出到文件')
  } catch (error) {
    logger.error('[Notebook] export cell output to file failed', {
      cellId: String(cell?.cellId || ''),
      outputModuleId: String(moduleId || ''),
      error: String(error?.message || error),
    })
    message.error(`导出输出失败：${String(error?.message || error)}`)
  }
}

const importExternalDataToPythonSession = async (cell, {
  sourceType = 'file',
  inlineText = '',
  sourceLabel = '',
  variableName = '',
  sourceUrl = '',
  spec = null,
} = {}) => {
  if (!cell?.key) return
  const requestId = createNotebookActionRequestId('nb-import')
  // 设计说明：传入完整 Import Spec 时（导入向导/import-source 动作）直接据此构造执行计划，
  // 跳过旧的 sourceType 取数分支与文件对话框（路径/选项已由向导解析并确认）。
  const importSpec = spec && typeof spec === 'object' && Object.keys(spec).length ? spec : null
  const normalizedSourceType = importSpec
    ? String(importSpec.category || 'file').trim().toLowerCase()
    : String(sourceType || 'file').trim().toLowerCase()
  const targetIdentity = getNotebookPythonCellIdentity(cell)
  const activeCellKeyAtStart = String(activeNotebookPythonCellKey.value || '')
  notebookPendingPythonImport.value = {
    requestId,
    ...targetIdentity,
    filePath: String(props.filePath || ''),
  }
  setNotebookBusyState(cell, true, `import-data:start:${requestId}`)

  try {
    let importPlan
    if (importSpec) {
      importPlan = buildNotebookLabImportPlanFromSpec(importSpec)
    } else {
      let selectedPath = ''
      let effectiveText = String(inlineText || '')
      let effectiveLabel = String(sourceLabel || '')
      let effectiveUrl = String(sourceUrl || '').trim()

      if (normalizedSourceType === 'file') {
        const selected = await open({
          multiple: false,
          filters: [{
            name: 'External Data',
            extensions: ['csv', 'tsv', 'txt', 'xlsx', 'xls', 'json', 'jsonl', 'parquet', 'feather', 'db', 'sqlite', 'sqlite3', 'duckdb', 'md', 'markdown', 'yaml', 'yml', 'log', 'py', 'js', 'ts'],
          }],
        })
        if (!selected) return
        selectedPath = String(selected)
        effectiveLabel = effectiveLabel || (selectedPath.split(/[\\/]/).pop() || selectedPath)
      } else if (normalizedSourceType === 'url') {
        if (!effectiveUrl) {
          effectiveUrl = String(window.prompt?.('输入要导入的 CSV / JSON / Excel / 文本 URL', '') || '').trim()
        }
        if (!effectiveUrl) return
        effectiveLabel = effectiveLabel || effectiveUrl
      } else if (normalizedSourceType === 'clipboard') {
        effectiveText = String(await readClipboardText())
        if (!String(effectiveText || '').trim()) {
          message.warning('剪贴板里没有可导入的文本数据')
          return
        }
        effectiveLabel = effectiveLabel || 'clipboard'
      } else if (!String(effectiveText || '').trim()) {
        throw new Error('缺少可导入的数据内容')
      }

      importPlan = buildNotebookLabImportPlan({
        sourceType: normalizedSourceType,
        sourcePath: selectedPath,
        sourceLabel: effectiveLabel,
        variableName,
        text: effectiveText,
        sourceUrl: effectiveUrl,
      })
    }
    logger.info('[NotebookDataLab] prepared import plan', {
      requestId,
      sourceType: normalizedSourceType,
      sourceLabel: importPlan.sourceLabel,
      variableName: importPlan.variableName,
      cellId: String(cell?.cellId || ''),
      targetCellId: targetIdentity.cellId,
      targetStableId: targetIdentity.stableId,
    })

    const sessionId = createPythonSessionId(props.filePath)
    const result = await invoke('python_exec', {
      request: {
        code: importPlan.code,
        persist: true,
        expect_json: false,
        include_session_vars: true,
        session_var_names: [importPlan.variableName],
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    if (result?.ok === false) {
      throw new Error(String(result?.stderr || result?.stdout || '导入失败'))
    }

    const targetCell = resolveNotebookPythonCellByIdentity(targetIdentity)
    if (!targetCell) {
      throw new Error('导入目标 Python Cell 已不存在，请重新选择后再试')
    }

    const snapshot = getCellSnapshot(targetCell)
    const nextSessionVars = mergeNotebookSessionVarSummaries(
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      Array.isArray(result?.session_vars) ? result.session_vars : [],
    )
    const dataflow = getGuaxiDataflowFromResult(result)
    const previousOutputCount = Array.isArray(snapshot?.outputs) ? snapshot.outputs.length : 0
    const outputText = normalizedSourceType === 'clipboard'
      ? `已将剪贴板数据导入到会话变量 ${importPlan.variableName}，可以在实验室面板中继续生成输出、写回 Markdown 或导出。`
      : `已将数据源 ${importPlan.sourceLabel} 导入到会话变量 ${importPlan.variableName}，可以在实验室面板中继续生成输出、写回 Markdown 或导出。`
    const nextOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      buildPythonCellOutputModules({
        outputText,
        display: null,
        dataframe: null,
        tables: [],
        source: 'data-lab-import',
      }),
    )
    // 设计说明：用导入后回传的会话变量摘要补全 Dataset Descriptor（dataKind/schema），
    // 连同完整 Import Spec 一并记账，便于后续“重新导入”与结构分析。
    const importedVarSummary = (Array.isArray(result?.session_vars) ? result.session_vars : [])
      .find((item) => String(item?.name || '') === importPlan.variableName) || null
    const importedDataKind = importedVarSummary ? classifyNotebookLabVariable(importedVarSummary) : 'value'
    const importDescriptor = {
      variableName: importPlan.variableName,
      sourceType: importPlan.sourceType,
      sourceLabel: importPlan.sourceLabel,
      dataKind: importedDataKind,
      importedAt: new Date().toISOString(),
    }
    if (importPlan.spec) {
      importDescriptor.source = {
        category: importPlan.spec.category,
        format: importPlan.spec.format,
        label: importPlan.spec.sourceLabel,
      }
      importDescriptor.options = importPlan.spec.options || {}
      importDescriptor.spec = importPlan.spec
      if (importedVarSummary) {
        importDescriptor.schema = {
          columns: Array.isArray(importedVarSummary.column_names)
            ? importedVarSummary.column_names.map((column) => String(column ?? ''))
            : [],
          dtypes: importedVarSummary.dtypes && typeof importedVarSummary.dtypes === 'object' ? { ...importedVarSummary.dtypes } : {},
          rowCount: importedVarSummary.rows ?? importedVarSummary.length ?? null,
          shape: Array.isArray(importedVarSummary.shape) ? [...importedVarSummary.shape] : [],
        }
      }
    }
    const nextImports = upsertNotebookLabImportRecord(getCellLabImports(targetCell), importDescriptor)

    patchPythonCellSnapshot(props.filePath, targetCell.cellId, {
      codeDigest: getNotebookCellContentDigest(targetCell),
      outputText,
      display: snapshot?.display || null,
      dataframe: snapshot?.dataframe || null,
      outputs: nextOutputs,
      result: snapshot?.result || null,
      sessionVars: nextSessionVars,
      lab: patchCellSnapshotLab(targetCell, {
        imports: nextImports,
        lastImportSourceType: importPlan.sourceType,
        lastImportAt: new Date().toISOString(),
        ...(dataflow
          ? {
            dataflow: {
              ...dataflow,
              updatedAt: new Date().toISOString(),
              reason: 'import-data-to-python-session',
            },
          }
          : {}),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.info('[NotebookDataLab] imported external data into python session', {
      requestId,
      cellId: String(targetCell?.cellId || ''),
      cellKey: String(targetCell?.key || ''),
      stableId: String(targetCell?.meta?.id || ''),
      variableName: importPlan.variableName,
      sourceType: importPlan.sourceType,
      sourceLabel: importPlan.sourceLabel,
      previousOutputCount,
      updatedSessionVarCount: Array.isArray(result?.session_vars) ? result.session_vars.length : 0,
      sessionVarCount: nextSessionVars.length,
      outputModuleCount: nextOutputs.length,
      activeCellChangedDuringImport: activeCellKeyAtStart !== String(activeNotebookPythonCellKey.value || ''),
      activeCellKeyAtFinish: String(activeNotebookPythonCellKey.value || ''),
    })
    await emitNotebookSessionVarsUpdated(targetCell, result?.session_vars || [{ name: importPlan.variableName }], 'import-data-to-python-session')
    bumpRefresh()
    schedulePersistNotebookSidecar('import-data-to-python-session')
    message.success(`已导入到会话变量 ${importPlan.variableName}`)
  } catch (error) {
    logger.error('[NotebookDataLab] import external data to python session failed', {
      requestId,
      cellId: String(cell?.cellId || ''),
      cellKey: String(cell?.key || ''),
      targetCellId: targetIdentity.cellId,
      targetCellKey: targetIdentity.cellKey,
      sourceType: normalizedSourceType,
      error: String(error?.message || error),
    })
    message.error(`导入数据失败：${String(error?.message || error)}`)
  } finally {
    if (String(notebookPendingPythonImport.value?.requestId || '') === requestId) {
      notebookPendingPythonImport.value = null
    }
    const resolvedTargetCell = resolveNotebookPythonCellByIdentity(targetIdentity) || cell
    setNotebookBusyState(resolvedTargetCell, false, `import-data:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`import-data:finish:${requestId}`)
  }
}

const importTableToPythonSession = async (cell) => {
  await importExternalDataToPythonSession(cell, { sourceType: 'file' })
}

const materializeSessionVariableOutput = async (cell, dataset = {}, {
  writeBack = false,
  exportAfter = false,
} = {}) => {
  if (!cell?.key) return null
  const variableName = String(dataset?.variableName || '').trim()
  if (!variableName) {
    message.warning('缺少变量名，无法生成实验室输出')
    return null
  }

  const requestId = createNotebookActionRequestId('nb-lab')
  setNotebookBusyState(cell, true, `data-lab-materialize:start:${requestId}`)

  try {
    const code = buildNotebookLabMaterializeCode(dataset)
    if (!code) {
      throw new Error('缺少实验室输出脚本')
    }

    const res = await invoke('python_exec', {
      request: {
        code,
        persist: true,
        expect_json: true,
        include_session_vars: false,
        vars: {
          _guaxi_session: createPythonSessionId(props.filePath),
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const display = getGuaxiDisplayFromResult(res)
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const runOutputs = decorateNotebookLabOutputModules(
      buildPythonCellOutputModules({
        outputText,
        display,
        dataframe,
        tables,
        source: 'data-lab-materialize',
      }),
      dataset,
      'data-lab-materialize',
    )
    const snapshot = getCellSnapshot(cell)
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(snapshot?.outputs || [], runOutputs)
      : (snapshot?.outputs || [])
    const outputModuleId = String(runOutputs?.[0]?.id || '')
    const hideOutput = !nextOutputs.length

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
      outputs: nextOutputs,
      result: res,
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      lab: patchCellSnapshotLab(cell, {
        lastMaterializedVariable: variableName,
        lastMaterializedAt: new Date().toISOString(),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: hideOutput,
      },
    })
    logger.info('[NotebookDataLab] materialized session variable output', {
      requestId,
      cellId: String(cell?.cellId || ''),
      variableName,
      dataKind: String(dataset?.dataKind || ''),
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      tableCount: tables.length,
      outputModuleId,
      totalOutputModuleCount: nextOutputs.length,
      writeBack,
      exportAfter,
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('data-lab-materialize-success')

    if (writeBack && outputModuleId) {
      await writeCellOutputToMarkdown(cell, outputModuleId)
    }
    if (exportAfter && outputModuleId) {
      await exportCellOutputToFile(cell, outputModuleId)
    }

    if (res?.ok === false) {
      message.error(`变量 ${variableName} 生成输出失败`)
      return {
        outputModuleId,
        ok: false,
      }
    }

    if (!writeBack && !exportAfter) {
      message.success(`变量 ${variableName} 已生成输出`)
    }
    return {
      outputModuleId,
      ok: true,
    }
  } catch (error) {
    const snapshot = getCellSnapshot(cell)
    const errorOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      decorateNotebookLabOutputModules(
        buildPythonCellOutputModules({
          outputText: `变量 ${variableName} 生成失败：${String(error?.message || error)}`,
          display: null,
          dataframe: null,
          source: 'data-lab-error',
        }),
        dataset,
        'data-lab-error',
      ),
    )
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: `变量 ${variableName} 生成失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      outputs: errorOutputs,
      result: null,
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      lab: snapshot?.lab || { imports: [] },
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.error('[NotebookDataLab] materialize session variable failed', {
      requestId,
      cellId: String(cell?.cellId || ''),
      variableName,
      error: String(error?.message || error),
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('data-lab-materialize-failed')
    message.error(`生成变量输出失败：${String(error?.message || error)}`)
    return null
  } finally {
    setNotebookBusyState(cell, false, `data-lab-materialize:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`data-lab-materialize:finish:${requestId}`)
  }
}

const runNotebookLabGeneratedCode = async (cell, {
  code = '',
  source = 'data-lab-action',
  dataset = {},
  writeBack = false,
  exportAfter = false,
  successMessage = '',
  labPatch = {},
} = {}) => {
  if (!cell?.key || !String(code || '').trim()) return null
  const requestId = createNotebookActionRequestId('nb-lab-action')
  setNotebookBusyState(cell, true, `${source}:start:${requestId}`)

  try {
    const res = await invoke('python_exec', {
      request: {
        code: String(code || ''),
        persist: true,
        expect_json: true,
        include_session_vars: true,
        vars: {
          _guaxi_session: createPythonSessionId(props.filePath),
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    let outputText = formatPythonExecOutput(res)
    if (source === 'math-block' || source === 'data-lab-math') {
      outputText = stripNotebookMathTraceOutput(outputText)
    }
    const display = getGuaxiDisplayFromResult(res)
    // 数学工作区（math-block）的引擎可能一次返回「多个 display」（数组）：
    // 符号结果走 markdown（MathJax，跟随暗色主题），每张图单独走 html（交互 Plotly）。
    // 这里把数组拆成多个输出模块；其它来源仍按单 display 处理，避免影响数据实验室其余链路。
    const displayList = Array.isArray(display)
      ? display.filter((item) => item && typeof item === 'object')
      : []
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const dataflow = getGuaxiDataflowFromResult(res)
    const snapshot = getCellSnapshot(cell)
    const legacyDisplay = displayList.length ? null : display
    const builtRunOutputs = displayList.length
      ? [
        ...displayList
          .map((item) => createPythonCellOutputModule({
            kind: normalizePythonCellOutputDisplayType(item),
            source,
            display: item,
          }))
          .filter(Boolean),
        // 表格与文本仍走统一构造（display 置空，避免与上面的多 display 重复）。
        ...buildPythonCellOutputModules({ outputText, display: null, dataframe, tables, source }),
      ]
      : buildPythonCellOutputModules({ outputText, display, dataframe, tables, source })
    const runOutputs = decorateNotebookLabOutputModules(builtRunOutputs, dataset, source)
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(snapshot?.outputs || [], runOutputs)
      : (snapshot?.outputs || [])
    const outputModuleId = String(runOutputs?.[0]?.id || '')
    const nextSessionVars = resolveNotebookSessionVarsFromExecResult(
      res,
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    )

    // 设计说明：
    // 1. 数据实验室生成动作的结果应以“本次请求”为准，不能在没有新 display/dataframe 时沿用旧快照；
    // 2. 否则上一次动画或表格会被 legacy display/dataframe 槽位重新暴露，看起来像当前操作又输出了一遍旧内容；
    // 3. outputs[] 仍按模块历史保留，legacy 三槽只镜像本次执行实际产生的可见结果。
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText,
      display: legacyDisplay,
      dataframe,
      outputs: nextOutputs,
      result: res,
      sessionVars: nextSessionVars,
      lab: patchCellSnapshotLab(cell, {
        ...(labPatch && typeof labPatch === 'object' ? labPatch : {}),
        ...(dataflow
          ? {
            dataflow: {
              ...dataflow,
              updatedAt: new Date().toISOString(),
              reason: source,
            },
          }
          : {}),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: !nextOutputs.length,
        staleVariables: [],
        staleReason: '',
      },
    })

    logger.info('[NotebookDataLab] generated lab action completed', {
      requestId,
      source,
      cellId: String(cell?.cellId || ''),
      variableName: String(dataset?.variableName || ''),
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      tableCount: tables.length,
      sessionVarCount: nextSessionVars.length,
      outputModuleCount: nextOutputs.length,
      touchedVariables: Array.isArray(dataflow?.touched) ? dataflow.touched : [],
      writeBack,
      exportAfter,
    })
    await emitNotebookSessionVarsUpdated(cell, nextSessionVars, `${source}:success`)
    if (dataflow) {
      markDependentNotebookCellsStale(cell, dataflow, `${source}:success`)
    }
    if (cell?.type === 'python' && (isActiveNotebookPythonCell(cell) || isNotebookPythonSessionPanelVisible.value)) {
      syncNotebookPythonSessionInspector(cell, `${source}:success`)
    }

    bumpRefresh()
    schedulePersistNotebookSidecar(`${source}-success`)

    if (writeBack && outputModuleId) {
      // 设计说明：
      // 1. Python Cell 写回是把产物写入受管 <!--output--> 块（且经资源库统一处理）。
      // 2. 数学块与 Python Cell 对称：表达式与输出只存 cellmeta 侧数据，
      //    绝不把内容或 $$ 公式写进源 Markdown（否则会被预览/导出当作真正的数学公式渲染）。
      //    因此数学块即使带 writeBack 也不写回源文件，输出仅留在 outputModule + sidecar 快照。
      if (cell?.type === 'python') {
        await writeCellOutputToMarkdown(cell, outputModuleId)
      }
    }
    if (exportAfter && outputModuleId) {
      await exportCellOutputToFile(cell, outputModuleId)
    }

    if (res?.ok === false) {
      message.error('实验室动作执行失败')
      return { ok: false, outputModuleId }
    }
    if (successMessage) message.success(successMessage)
    return { ok: true, outputModuleId }
  } catch (error) {
    logger.error('[NotebookDataLab] generated lab action failed', {
      requestId,
      source,
      cellId: String(cell?.cellId || ''),
      variableName: String(dataset?.variableName || ''),
      error: String(error?.message || error),
    })
    message.error(`实验室动作失败：${String(error?.message || error)}`)
    return null
  } finally {
    setNotebookBusyState(cell, false, `${source}:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`${source}:finish:${requestId}`)
  }
}

const runMathCell = async (cell, writeBack = false, mode = '', options = {}) => {
  const source = String(cell?.expression || '').trim()
  if (!source) {
    message.warning('数学工作区为空')
    return
  }

  // 设计说明（命令式数学工作区 v2）：
  // 1. 数学块是一个高级数学 REPL：整段文本（多行命令/赋值/裸表达式）作为 source 交给
  //    真实的 Python 引擎模块 guaximath_engine.py 执行（见 utils/guaxiNotebookMathRuntime.js）。
  // 2. 它复用同一个 Python 持久会话，可读取前面 Python Cell 的变量；工作区里定义的符号
  //    （f = sin(x)/x 等）也会跨多次运行 / 多个数学块持久复用。
  // 3. mode 为可选的「工具栏快捷动作」：仅当内容是单条表达式时，引擎侧会据此包一层
  //    （如 diff(<expr>)）；多行 / 已是命令的工作区程序则原样逐行执行。
  const code = buildNotebookLabMathCode({
    source,
    mode: String(mode || ''),
    variable: String(options.variable || 'x'),
    rangeMin: Number.isFinite(Number(options.rangeMin)) ? Number(options.rangeMin) : -10,
    rangeMax: Number.isFinite(Number(options.rangeMax)) ? Number(options.rangeMax) : 10,
    seriesPoint: Number.isFinite(Number(options.seriesPoint)) ? Number(options.seriesPoint) : 0,
    seriesOrder: Number.isFinite(Number(options.seriesOrder)) ? Number(options.seriesOrder) : 6,
    theme: props.theme,
  })
  logger.info('[NotebookMath] run math workspace', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    sourceLength: source.length,
    mode: String(mode || ''),
    writeBack: !!writeBack,
  })
  await runNotebookLabGeneratedCode(cell, {
    code,
    source: 'math-block',
    dataset: {
      variableName: 'math-block',
      dataKind: 'math',
    },
    writeBack,
    successMessage: writeBack ? '数学结果已生成并写回' : '数学结果已生成',
    labPatch: {
      lastMathExpression: source,
      lastMathBlockAt: new Date().toISOString(),
    },
  })
  // 运行后若数学工作区面板可见，刷新该块的工作区变量 / carrier（独立链路，不触碰 Python 面板）。
  if (isNotebookMathWorkspacePanelVisible.value) {
    activeMathWorkspaceCellKey.value = String(cell?.key || '')
    void syncMathWorkspaceInspector(cell, 'after-run')
  }
}

// 数学命令速查 / 片段菜单（命令式工作区 v2）：
// 每个分组对应一类数学场景，点击 item 会把 snippet 模板插入数学块（便于按命令式工作区书写）。
// 真正的计算逻辑在 Python 引擎 guaximath_engine.py 里，新增学科 = 加命令 + 加一条 snippet。
const NOTEBOOK_MATH_TOOL_GROUPS = [
  {
    label: '微积分',
    items: [
      { snippet: 'limit(sin(x)/x, x, 0)', label: '求极限', hint: 'limit(表达式, 变量, 趋近点)' },
      { snippet: 'diff(sin(x)*exp(x), x)', label: '求导数', hint: 'diff(表达式, 变量[, 阶数])' },
      { snippet: 'integrate(x**2, x)', label: '不定积分', hint: 'integrate(表达式, 变量)' },
      { snippet: 'integrate(exp(-x**2), (x, -oo, oo))', label: '定积分', hint: 'integrate(f, (变量, 下限, 上限))' },
      { snippet: 'taylor(cos(x), x, 0, 8)', label: '泰勒展开', hint: 'taylor(f, 变量, 展开点, 阶数)' },
      { snippet: 'solve(x**2 - 5*x + 6)', label: '解方程', hint: 'solve(方程或表达式[, vars=变量])' },
      { snippet: 'dsolve(Eq(Derivative(y(x), x, x) + y(x), 0))', label: '解微分方程', hint: 'dsolve(Eq(...))' },
    ],
  },
  {
    label: '线性代数',
    items: [
      { snippet: 'A = matrix([[2, 1], [1, 2]])', label: '定义矩阵', hint: 'A = matrix([[...], [...]])' },
      { snippet: 'det(A)', label: '行列式', hint: 'det(A)' },
      { snippet: 'inv(A)', label: '逆矩阵', hint: 'inv(A)' },
      { snippet: 'eig(A)', label: '特征值/向量', hint: 'eig(A)' },
      { snippet: 'rref(A)', label: '行最简形 RREF', hint: 'rref(A) / rank(A)' },
      { snippet: 'linsolve(matrix([[1, 1], [1, -1]]), matrix([3, 1]))', label: '解线性方程组', hint: 'linsolve(A, b)' },
      { snippet: 'svd(A)', label: 'SVD 分解', hint: 'svd(A) / qr(A) / lu(A)' },
    ],
  },
  {
    label: '概率统计',
    items: [
      { snippet: 'Normal(0, 1)', label: '正态分布', hint: 'Normal(μ, σ) 画 PDF/CDF' },
      { snippet: 'Binomial(10, 0.4)', label: '二项分布', hint: 'Binomial(n, p)' },
      { snippet: 'Poisson(3)', label: '泊松分布', hint: 'Poisson(λ)' },
      { snippet: 'expectation(Normal(0, 1))', label: '期望/方差', hint: 'expectation(...) / variance(...)' },
    ],
  },
  {
    label: '绘图（交互）',
    items: [
      { snippet: 'plot(sin(x), (x, -10, 10))', label: '一维曲线', hint: 'plot(f, (x, a, b))' },
      { snippet: 'plot([sin(x), cos(x)], (x, -pi, pi))', label: '多条曲线', hint: 'plot([f, g], (x, a, b))' },
      { snippet: 'plot(x**2 + y**2, (x, -3, 3), (y, -3, 3))', label: '三维曲面', hint: 'plot(f, (x,..), (y,..))' },
    ],
  },
  {
    label: '复变函数',
    items: [
      { snippet: 'residue(1/(z**2 + 1), z, I)', label: '留数', hint: 'residue(f, z, z0)' },
      { snippet: 'laurent(1/(z*(z - 1)), z, 0, 6)', label: '洛朗展开', hint: 'laurent(f, z, z0, n)' },
      { snippet: 'contour_integral(1/(z**2 + 1), z, poles=[I])', label: '围道积分', hint: 'contour_integral(f, z, poles=[...])' },
      { snippet: 'conformal(z**2, z)', label: '共形映射', hint: "conformal(f, z) → f'(z)" },
      { snippet: 'cauchy_riemann(x**2 - y**2, 2*x*y, x, y)', label: '柯西-黎曼', hint: 'cauchy_riemann(u, v, x, y)' },
    ],
  },
  {
    label: '数学物理方法',
    items: [
      { snippet: 'laplace(exp(-2*t), t, s)', label: '拉普拉斯变换', hint: 'laplace(f, t, s) / inverse_laplace(F, s, t)' },
      { snippet: 'fourier(exp(-x**2), x, k)', label: '傅里叶变换', hint: 'fourier(f, x, k) / inverse_fourier(F, k, x)' },
      { snippet: 'besselj(0, x)', label: '特殊函数', hint: 'besselj / legendre / hermite / laguerre ...' },
      { snippet: 'separate_variables(x*y + x)', label: '分离变量', hint: 'separate_variables(expr)' },
      { snippet: 'greens_function(x, xi, 1)', label: '格林函数', hint: 'greens_function(x, xi, L)（1D Dirichlet）' },
    ],
  },
  {
    label: '张量 / 微分几何',
    items: [
      { snippet: 'g = metric([[1, 0], [0, r**2]], [r, theta])', label: '定义度规', hint: 'g = metric([[...]], [坐标])' },
      { snippet: 'christoffel(g, [r, theta])', label: 'Christoffel 符号', hint: 'christoffel(g, [坐标])' },
      { snippet: 'ricci(g, [r, theta])', label: 'Ricci / 标量曲率', hint: 'ricci(g, ..) / ricci_scalar(g, ..)' },
      { snippet: 'geodesic_equations(g, [r, theta])', label: '测地线方程', hint: 'geodesic_equations(g, [坐标])' },
      { snippet: 'gaussian_curvature([a*cos(u)*sin(v), a*sin(u)*sin(v), a*cos(v)], (u, v))', label: '曲面曲率', hint: 'gaussian_curvature / mean_curvature(r, (u,v))' },
    ],
  },
  {
    label: '矩阵论 / 泛函',
    items: [
      { snippet: 'jordan(matrix([[2, 1], [0, 2]]))', label: 'Jordan 标准形', hint: 'jordan(A)' },
      { snippet: 'matrix_exp(matrix([[0, 1], [0, 0]]))', label: '矩阵指数', hint: 'matrix_exp(A)' },
      { snippet: 'schur(matrix([[2, 1], [1, 2]]))', label: 'Schur 分解', hint: 'schur(A)（数值）' },
      { snippet: 'gram_schmidt([matrix([1, 1, 0]), matrix([1, 0, 1])])', label: 'Gram-Schmidt', hint: 'gram_schmidt([v1, v2, ...])' },
      { snippet: "l2_approx(exp(x), 'legendre', x, -1, 1, 3)", label: 'L² 最佳逼近', hint: "l2_approx(f, 'legendre'/'fourier', x, a, b, n)" },
    ],
  },
  {
    label: '数值计算',
    items: [
      { snippet: 'nintegrate(exp(-x**2), (x, -5, 5))', label: '数值积分', hint: 'nintegrate(f, (x, a, b))' },
      { snippet: 'nsolve(x**3 - x - 2, 1.5)', label: '数值求根', hint: 'nsolve(f, x0, tol=.., maxiter=..)' },
      { snippet: 'minimize((x - 3)**2 + 1, 0)', label: '数值优化', hint: 'minimize(f, x0, method=..)' },
      { snippet: 'lstsq(matrix([[1, 1], [1, 2], [1, 3]]), matrix([1, 2, 2]))', label: '最小二乘', hint: 'lstsq(A, b)' },
      { snippet: 'odeint(-y, 1, (t, 0, 5))', label: 'ODE 数值解', hint: 'odeint(rhs, y0, (t, a, b), n=..)' },
    ],
  },
  {
    label: '仿真（轻量）',
    items: [
      { snippet: 'odeint([y, -x], [1, 0], (t, 0, 6.283), vars=[x, y])', label: 'ODE 系统', hint: 'odeint([f, g], [x0, y0], (t, a, b), vars=[x, y])' },
      { snippet: 'phase_portrait([y, -x], (x, -3, 3), (y, -3, 3))', label: '相图 / 向量场', hint: 'phase_portrait([f, g], (x,..), (y,..))' },
    ],
  },
]

// 读取某个数学块的预览模式（默认 latex）。
const getMathCellPreviewMode = (cell) => (
  notebookMathPreviewModeMap.value[String(cell?.key || '')] === 'command' ? 'command' : 'latex'
)

// 切换数学块预览模式：latex（公式渲染）<-> command（命令模式，不渲染 LaTeX，按代码展示），并立即刷新预览。
const toggleMathCellPreviewMode = (cell) => {
  const key = String(cell?.key || '')
  if (!key) return
  const next = getMathCellPreviewMode(cell) === 'command' ? 'latex' : 'command'
  notebookMathPreviewModeMap.value = {
    ...notebookMathPreviewModeMap.value,
    [key]: next,
  }
  void renderMathCellPreviewImmediately(key, cell?.expression || '')
  logger.info('[NotebookMath] toggle math preview mode', { key, mode: next })
}

// 切换某个数学块的工具下拉菜单展开状态（点击同一个按钮再次点击则收起）。
const toggleMathToolMenu = (cell) => {
  const key = String(cell?.key || '')
  activeMathToolMenuKey.value = activeMathToolMenuKey.value === key ? '' : key
}

const closeMathToolMenu = () => {
  activeMathToolMenuKey.value = ''
}

// 选择菜单项：把命令片段插入数学块（追加到末尾新行），方便按命令式工作区书写后再运行。
const handleMathToolSelect = (cell, snippet) => {
  closeMathToolMenu()
  const text = String(snippet || '').trim()
  if (!text) return
  const current = String(cell?.expression || '')
  const next = current.trim() ? `${current.replace(/\s+$/, '')}\n${text}` : text
  updateMathCell(cell.key, next)
  logger.info('[NotebookMath] insert command snippet', {
    key: String(cell?.key || ''),
    snippet: text,
  })
}

// 折叠 / 展开数学块的输出面板，复用 Python Cell 的 outputCollapsed 元数据机制。
const toggleMathCellOutputCollapsed = (cell) => {
  const snapshot = getCellSnapshot(cell)
  const nextCollapsed = !snapshot.meta.outputCollapsed
  patchCellMeta(cell, { outputCollapsed: nextCollapsed })
  logger.info('[Notebook] toggle math cell output collapsed', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    collapsed: nextCollapsed,
  })
}

const handleNotebookDataLabActionEvent = async (event) => {
  const detail = event?.detail && typeof event.detail === 'object' ? event.detail : {}
  const targetFilePath = String(detail?.filePath || '').trim()
  if (targetFilePath && targetFilePath !== String(props.filePath || '').trim()) return

  const targetCell = resolveNotebookPythonCellByIdentity(detail) || activeNotebookPythonCell.value
  if (!targetCell) {
    logger.warn('[NotebookDataLab] ignored lab action because target cell is missing', {
      action: String(detail?.action || ''),
      cellId: String(detail?.cellId || ''),
      stableId: String(detail?.stableId || ''),
    })
    return
  }

  activateNotebookCell(targetCell, `data-lab:${String(detail?.action || 'unknown')}`)
  const dataset = {
    variableName: String(detail?.variableName || ''),
    dataKind: String(detail?.dataKind || ''),
    chartType: String(detail?.chartType || ''),
  }

  if (detail.action === 'focus-variable') {
    await focusNotebookVariableInCell(targetCell, String(detail?.variableName || ''), 'session-panel')
    return
  }
  if (detail.action === 'import-source') {
    await importExternalDataToPythonSession(targetCell, {
      spec: detail?.spec && typeof detail.spec === 'object' ? detail.spec : {},
    })
    return
  }
  if (detail.action === 'import-file') {
    await importExternalDataToPythonSession(targetCell, { sourceType: 'file' })
    return
  }
  if (detail.action === 'import-clipboard') {
    await importExternalDataToPythonSession(targetCell, { sourceType: 'clipboard' })
    return
  }
  if (detail.action === 'import-url') {
    await importExternalDataToPythonSession(targetCell, {
      sourceType: 'url',
      sourceUrl: String(detail?.sourceUrl || ''),
      sourceLabel: String(detail?.sourceLabel || ''),
      variableName: String(detail?.variableName || ''),
    })
    return
  }
  if (detail.action === 'import-inline-data') {
    await importExternalDataToPythonSession(targetCell, {
      sourceType: String(detail?.sourceType || 'inline'),
      inlineText: String(detail?.text || ''),
      sourceLabel: String(detail?.sourceLabel || ''),
      variableName: String(detail?.variableName || ''),
    })
    return
  }
  if (detail.action === 'refresh-session-inspector') {
    // 设计说明：
    // 1. 独立数据工作台和 Notebook 主视图共享同一个 Python session，但它们是两个 WebView。
    // 2. 工作台保存变量后不能直接改主视图的 Pinia 快照，所以通过统一事件请求主视图重新同步右侧变量面板。
    // 3. 这里通过一段无输出轻量代码从 worker 拉取最新变量摘要，不重新执行用户 cell，也不追加输出模块。
    try {
      const snapshot = getCellSnapshot(targetCell)
      const variableName = String(detail?.variableName || '').trim()
      const res = await invoke('python_exec', {
        request: {
          code: 'print("[GuaxiDataWorkbench] refreshed notebook inspector")',
          persist: true,
          expect_json: true,
          include_session_vars: true,
          session_var_names: variableName ? [variableName] : undefined,
          vars: {
            _guaxi_session: createPythonSessionId(props.filePath),
            _doc_path: String(props.filePath || ''),
            _py_cell_id: String(targetCell.cellId || ''),
          },
        },
      })
      const nextSessionVars = variableName
        ? mergeNotebookSessionVarSummaries(
          Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
          Array.isArray(res?.session_vars) ? res.session_vars : [],
        )
        : resolveNotebookSessionVarsFromExecResult(res, Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [])
      patchPythonCellSnapshot(props.filePath, targetCell.cellId, {
        sessionVars: nextSessionVars,
        lab: patchCellSnapshotLab(targetCell, {
          lastWorkbenchRefreshAt: new Date().toISOString(),
          lastWorkbenchRefreshVariable: variableName,
        }),
      })
      syncNotebookPythonSessionInspector(targetCell, 'data-workbench-refresh-request')
      bumpRefresh()
      schedulePersistNotebookSidecar('data-workbench-refresh')
      logger.info('[NotebookDataLab] refreshed inspector from data workbench request', {
        cellId: String(targetCell?.cellId || ''),
        variableName,
        updatedSessionVarCount: nextSessionVars.length,
      })
      await emitNotebookSessionVarsUpdated(targetCell, nextSessionVars, 'data-workbench-refresh-request')
    } catch (error) {
      logger.warn('[NotebookDataLab] refresh inspector from data workbench failed', {
        cellId: String(targetCell?.cellId || ''),
        variableName: String(detail?.variableName || ''),
        error: String(error?.message || error),
      })
      syncNotebookPythonSessionInspector(targetCell, 'data-workbench-refresh-fallback')
    }
    return
  }
  if (detail.action === 'plot-variable' || detail.action === 'plot-variable-writeback' || detail.action === 'plot-variable-export') {
    const code = buildNotebookLabPlotCode({
      variableName: String(detail?.variableName || ''),
      chartType: String(detail?.chartType || 'line'),
      x: String(detail?.x || ''),
      y: String(detail?.y || ''),
      ySeries: Array.isArray(detail?.ySeries) ? detail.ySeries : [],
      z: String(detail?.z || ''),
      color: String(detail?.color || ''),
      title: String(detail?.title || ''),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-plot',
      dataset,
      writeBack: detail.action === 'plot-variable-writeback',
      exportAfter: detail.action === 'plot-variable-export',
      successMessage: '图表已生成',
      labPatch: {
        lastPlotVariable: String(detail?.variableName || ''),
        lastPlotType: String(detail?.chartType || 'line'),
        lastPlotAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'analyze-variable' || detail.action === 'analyze-variable-writeback') {
    const code = buildAnalysisCode({
      method: String(detail?.method || 'corr_matrix'),
      features: Array.isArray(detail?.features) ? detail.features : [],
      target: String(detail?.target || ''),
      group: String(detail?.group || ''),
      color: String(detail?.color || ''),
      timeIndex: String(detail?.timeIndex || ''),
      primaryVariable: String(detail?.primaryVariable || detail?.variableName || ''),
      title: String(detail?.title || ''),
      options: detail?.options && typeof detail.options === 'object' ? detail.options : {},
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-analysis',
      dataset: {
        variableName: String(detail?.variableName || detail?.primaryVariable || 'analysis'),
        dataKind: 'analysis',
      },
      writeBack: detail.action === 'analyze-variable-writeback',
      successMessage: '分析结果已生成',
      labPatch: {
        lastAnalysisMethod: String(detail?.method || ''),
        lastAnalysisAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'math-evaluate' || detail.action === 'math-writeback') {
    const code = buildNotebookLabMathCode({
      expression: String(detail?.expression || ''),
      mode: String(detail?.mode || 'plot2d'),
      variable: String(detail?.variable || 'x'),
      rangeMin: Number(detail?.rangeMin ?? -10),
      rangeMax: Number(detail?.rangeMax ?? 10),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-math',
      dataset: {
        variableName: 'math',
        dataKind: 'math',
      },
      writeBack: detail.action === 'math-writeback',
      successMessage: '数学结果已生成',
      labPatch: {
        lastMathExpression: String(detail?.expression || ''),
        lastMathMode: String(detail?.mode || 'plot2d'),
        lastMathAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'generate-report' || detail.action === 'writeback-report') {
    const code = buildNotebookLabReportCode({
      title: String(detail?.title || '探索实验室报告'),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-report',
      dataset: {
        variableName: 'report',
        dataKind: 'markdown',
      },
      writeBack: detail.action === 'writeback-report',
      successMessage: '报告草稿已生成',
      labPatch: {
        lastReportAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'variable-delete' || detail.action === 'variable-rename' || detail.action === 'variable-copy' || detail.action === 'variable-derive') {
    const variableAction = String(detail.action || '').replace(/^variable-/, '')
    const code = buildNotebookLabVariableOperationCode({
      action: variableAction,
      variableName: String(detail?.variableName || ''),
      targetName: String(detail?.targetName || ''),
      expression: String(detail?.expression || ''),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-variable-op',
      dataset,
      successMessage: '变量操作已完成',
      labPatch: {
        lastVariableAction: variableAction,
        lastVariableActionAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'materialize-variable') {
    await materializeSessionVariableOutput(targetCell, dataset)
    return
  }
  if (detail.action === 'writeback-variable') {
    await materializeSessionVariableOutput(targetCell, dataset, { writeBack: true })
    return
  }
  if (detail.action === 'export-variable') {
    await materializeSessionVariableOutput(targetCell, dataset, { exportAfter: true })
    return
  }

  logger.warn('[NotebookDataLab] unknown lab action received', {
    action: String(detail?.action || ''),
    cellId: String(targetCell?.cellId || ''),
  })
}

const runPythonCell = async (cell, { writeBack = false } = {}) => {
  // 运行前先把防抖中的编辑落地，并重新取到该 cell 的最新内容（emitCells 会重建 cell 对象）
  if (pendingCellContentEdits.size) {
    applyPendingCellContentEdits()
    const refreshed = notebookCells.value.find(
      (item) => item?.type === 'python'
        && (String(item?.cellId || '') === String(cell?.cellId || '')
          || String(item?.key || '') === String(cell?.key || '')),
    )
    if (refreshed) cell = refreshed
  }
  flushPendingNotebookMarkdown('before-run-python-cell')
  if (!cell?.code) {
    message.warning('Python 代码为空')
    return
  }

  const requestId = createNotebookActionRequestId('nb-run')
  const sessionId = createPythonSessionId(props.filePath)
  setNotebookBusyState(cell, true, `run-python-cell:start:${requestId}`)
  if (!writeBack) {
    notebookActivePythonRun.value = {
      requestId,
      sessionId,
      cellId: String(cell.cellId || ''),
      cellKey: String(cell.key || ''),
      startedAt: Date.now(),
    }
    logger.info('[Notebook] registered cancellable python cell run', {
      requestId,
      sessionId,
      cellId: String(cell.cellId || ''),
      cellKey: String(cell.key || ''),
    })
  }

  try {
    const res = await invoke('python_exec', {
      request: {
        code: String(cell.code || ''),
        persist: true,
        expect_json: true,
        request_id: requestId,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const display = getGuaxiDisplayFromResult(res)
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const snapshot = getCellSnapshot(cell)
    const runOutputs = buildPythonCellOutputModules({
      outputText,
      display,
      dataframe,
      tables,
      source: 'execution',
    })
    const nextSessionVars = resolveNotebookSessionVarsFromExecResult(
      res,
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    )
    const dataflow = getGuaxiDataflowFromResult(res)
    const retainedNonExecutionOutputs = Array.isArray(snapshot?.outputs)
      ? snapshot.outputs.filter((item) => {
        const source = String(item?.source || '').trim()
        return source !== 'execution' && source !== 'execution-error'
      })
      : []
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(retainedNonExecutionOutputs, runOutputs)
      : retainedNonExecutionOutputs
    const hideOutput = !nextOutputs.length
    logger.info('[Notebook] python cell executed', {
      requestId,
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      writeBack,
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      hasDataframe: !!dataframe,
      tableCount: tables.length,
      sessionVarCount: nextSessionVars.length,
      touchedVariables: Array.isArray(dataflow?.touched) ? dataflow.touched : [],
      hideOutput,
      appendedOutputModuleCount: runOutputs.length,
      totalOutputModuleCount: nextOutputs.length,
      prunedExecutionOutputModuleCount: Math.max(0, (Array.isArray(snapshot?.outputs) ? snapshot.outputs.length : 0) - retainedNonExecutionOutputs.length),
    })

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
      outputs: nextOutputs,
      result: res,
      sessionVars: nextSessionVars,
      lab: dataflow
        ? patchCellSnapshotLab(cell, {
          dataflow: {
            ...dataflow,
            updatedAt: new Date().toISOString(),
            reason: 'run-python-cell',
          },
        })
        : snapshot?.lab || { imports: [] },
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: hideOutput,
        staleVariables: [],
        staleReason: '',
      },
    })
    if (dataflow) {
      markDependentNotebookCellsStale(cell, dataflow, 'run-python-cell-success')
    }
    await emitNotebookSessionVarsUpdated(cell, nextSessionVars, 'run-python-cell-success')
    if (isActiveNotebookPythonCell(cell) || isNotebookPythonSessionPanelVisible.value) {
      syncNotebookPythonSessionInspector(cell, 'run-python-cell-success')
    }
    bumpRefresh()
    schedulePersistNotebookSidecar('run-python-cell-success')

    if (writeBack && !hideOutput) {
      await writeCellOutputToMarkdown(cell)
    }

    if (res?.ok === false) {
      message.error('Python 执行失败')
      return
    }

    message.success(writeBack ? '运行完成，输出已写回文档' : '运行完成')
  } catch (error) {
    if (notebookCancelledPythonRunIds.has(requestId)) {
      logger.info('[Notebook] python cell run cancelled without writing error output', {
        requestId,
        key: String(cell?.key || ''),
        cellId: String(cell?.cellId || ''),
        error: String(error?.message || error),
      })
      message.info('Python Cell 运行已中断')
      return
    }

    const snapshot = getCellSnapshot(cell)
    const errorOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      buildPythonCellOutputModules({
        outputText: `运行失败：${String(error?.message || error)}`,
        display: null,
        dataframe: null,
        source: 'execution-error',
      }),
    )
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: `运行失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      outputs: errorOutputs,
      result: null,
      // 设计说明：
      // 1. 运行失败并不等于 Python 持久会话被重置，之前已有的变量往往仍然存在；
      // 2. 保留上一份变量快照，可以避免用户一次失败后右侧实验室瞬间“清空”，
      //    与同一会话里后续仍能访问旧变量的行为不一致；
      // 3. 真正需要清空变量时，应由 `%reset` / drop session / 后端明确返回空变量列表来决定。
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.error('[Notebook] python cell execution failed', {
      requestId,
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      outputModuleCount: errorOutputs.length,
      error: String(error?.message || error),
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('run-python-cell-failed')
    message.error(`运行失败：${String(error?.message || error)}`)
  } finally {
    const wasCancelled = notebookCancelledPythonRunIds.has(requestId)
    setNotebookBusyState(cell, false, `run-python-cell:finish:${requestId}`)
    if (String(notebookActivePythonRun.value?.requestId || '') === requestId) {
      notebookActivePythonRun.value = null
      logger.info('[Notebook] cleared cancellable python cell run', {
        requestId,
        cellId: String(cell?.cellId || ''),
        cellKey: String(cell?.key || ''),
      })
    }
    if (wasCancelled) {
      notebookCancelledPythonRunIds.delete(requestId)
      notebookQueuedPythonAction.value = null
      return
    }
    await flushQueuedNotebookPythonActionIfIdle(`run-python-cell:finish:${requestId}`)
  }
}

watch(
  [() => props.markdown, () => props.filePath],
  ([nextMarkdown], oldPair = []) => {
    const prevFilePath = Array.isArray(oldPair) ? oldPair[1] : ''
    const filePathChanged = String(prevFilePath || '') !== String(props.filePath || '')
    if (filePathChanged) {
      // 设计说明：
      // 1. Notebook 组件在 tab 间复用时，文件切换与新内容渲染之间存在一个异步窗口。
      // 2. 如果不先清空旧文件的渲染结果，模板会短暂继续显示上一个文件的 HTML，
      //    在 cellKey 重复时甚至可能被错误复用。
      // 3. 因此在 filePath 切换的第一时间先重置渲染 map，确保界面只等待当前文件的新结果。
      renderedMarkdownHtmlMap.value = {}
      renderedOutputMarkdownHtmlMap.value = {}
      resolvedDisplayUrlMap.value = {}
      notebookSearchIndexReady.value = false
      clearNotebookSearchHighlights()
      logger.info('[Notebook] cleared rendered html maps for file switch', {
        previousFilePath: String(prevFilePath || ''),
        nextFilePath: String(props.filePath || ''),
      })
    }
    syncNotebookCellsFromMarkdown(nextMarkdown, 'props-markdown-or-file-watch', {
      force: filePathChanged,
    })
  },
  { immediate: true },
)

watch(
  notebookCells,
  (cells) => {
    const keys = new Set(cells.map((cell) => String(cell.key || '')))
    if (editingMarkdownCellKey.value && !keys.has(String(editingMarkdownCellKey.value))) {
      editingMarkdownCellKey.value = ''
    }
    syncRenderedMarkdownHtml(cells, 'notebook-cells-watch')
    syncResolvedDisplayUrls(cells, 'notebook-cells-watch')
    scheduleNotebookSearchIndexUpdate('notebook-cells-watch')
    nextTick(() => {
      syncNotebookTextareaHeights('notebook-cells-changed')
      if (notebookSearchVisible.value) {
        scheduleNotebookSearchQuery('notebook-cells-watch-visible', 0)
      }
    })
  },
  { deep: true, immediate: true },
)

watch(
  [() => props.theme, () => props.filePath, refreshTick],
  () => {
    syncRenderedMarkdownHtml(notebookCells.value, 'theme-file-refresh-watch')
    syncResolvedDisplayUrls(notebookCells.value, 'theme-file-refresh-watch')
    scheduleNotebookSearchIndexUpdate('theme-file-refresh-watch')
    if (notebookSearchVisible.value) {
      scheduleNotebookSearchQuery('theme-file-refresh-watch', 0)
    }
  },
)

watch(editingMarkdownCellKey, (nextKey) => {
  if (!nextKey) return
  focusMarkdownEditor(nextKey, 'editing-markdown-cell-key')
})

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return

  logger.info('[NotebookSelection] translate language changed', { from, to })
  scheduleSelectionTranslate()
})

watch(
  notebookOutlineEntries,
  (entries) => {
    const payload = createNotebookOutlinePayload()
    emit('outline-change', payload)
    logger.info('[Notebook] emit outline change', {
      count: Array.isArray(entries) ? entries.length : 0,
    })
  },
  { immediate: true, deep: true },
)

watch(notebookSearchKeyword, () => {
  if (!String(notebookSearchKeyword.value || '').trim()) {
    clearNotebookSearchHighlights()
    return
  }
  scheduleNotebookSearchQuery('keyword-change')
})

// 三个搜索选项的切换不需要重新问后端索引，只要按新规则重刷 DOM 高亮即可。
watch([notebookSearchCaseSensitive, notebookSearchWholeWord, notebookSearchUseRegex], () => {
  if (!notebookSearchVisible.value) return
  nextTick(() => {
    applyNotebookSearchDomHighlights('search-option-change')
  })
})

// 设计说明（Q3）：探索者模式 cell 用 worker 渲染、buttons:false，所以渲染结果里没有复制按钮。
// 这里在渲染结果落 DOM 后，对每个 Markdown cell 预览区与输出富文本区补挂复制按钮（addCopyButtons 幂等）。
// 复制按钮的具体位置由作用域 CSS 控制（移到左上角，避开右上角的 cell 删除按钮）。
const injectNotebookCopyButtons = (reason = 'unknown') => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return
  const containers = root.querySelectorAll('.guaxi-notebook__markdown-preview, .guaxi-notebook__rich')
  if (!containers.length) return
  containers.forEach((container) => {
    try {
      addCopyButtons(container, { theme: props.theme })
    } catch (error) {
      logger.warn('[Notebook] inject copy buttons failed', { reason, error: String(error?.message || error) })
    }
  })
}

watch(
  [renderedMarkdownHtmlMap, renderedOutputMarkdownHtmlMap],
  () => {
    nextTick(() => {
      injectNotebookCopyButtons('rendered-html-watch')
      if (notebookSearchVisible.value && String(notebookSearchKeyword.value || '').trim()) {
        applyNotebookSearchDomHighlights('rendered-html-watch')
      }
    })
  },
  { deep: false },
)

onMounted(() => {
  initNotebookMarkdownWorker()
  document.addEventListener('selectionchange', handleNotebookGlobalSelectionChange, true)
  document.addEventListener('mousedown', handleNotebookGlobalMouseDown, true)
  document.addEventListener('keydown', handleNotebookShortcutKeydown, true)
  window.addEventListener(NOTEBOOK_DATALAB_EVENT, handleNotebookDataLabActionEvent)
  void listen('notebook-data-lab-rpc', (event) => {
    logger.info('[NotebookDataLab] received backend rpc event', {
      requestId: String(event?.payload?.requestId || ''),
      action: String(event?.payload?.action || ''),
      filePath: String(event?.payload?.filePath || ''),
    })
    return handleNotebookDataLabActionEvent({
      detail: event?.payload && typeof event.payload === 'object' ? event.payload : {},
    })
  }).then((unlisten) => {
    notebookDataLabRpcUnlisten = unlisten
  }).catch((error) => {
    logger.warn('[NotebookDataLab] failed to bind backend rpc listener', {
      error: String(error?.message || error),
    })
  })
  disposeNotebookDataLabBridge = installNotebookDataLabBridge({
    getContext: () => {
      const targetCell = activeNotebookPythonCell.value
      const snapshot = targetCell ? getCellSnapshot(targetCell) : null
      return {
        filePath: String(props.filePath || ''),
        sessionId: createPythonSessionId(props.filePath),
        activeCell: targetCell
          ? {
            cellId: String(targetCell?.cellId || ''),
            cellKey: String(targetCell?.key || ''),
            stableId: String(targetCell?.meta?.id || ''),
            title: String(targetCell?.meta?.title || ''),
          }
          : null,
        datasets: targetCell
          ? buildNotebookLabDatasetDescriptors(
            Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
            Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
          )
          : [],
      }
    },
  })
  logger.info('[NotebookDataLab] mounted event bridge', {
    filePath: String(props.filePath || ''),
  })
  logger.info('[NotebookSelection] mounted selection listeners')
  void refreshCodegenAssistEnabled()
  window.addEventListener('guaxi:python-codegen-setting-changed', handleCodegenSettingChanged)
})

onBeforeUnmount(() => {
  clearNotebookTopToolbarHideTimer()
  window.removeEventListener('guaxi:python-codegen-setting-changed', handleCodegenSettingChanged)
  // 落地防抖中的 cell 内容编辑，避免关闭/切换时丢失最后一段输入
  if (pendingCellContentEdits.size) {
    applyPendingCellContentEdits()
  }
  // 笔记本关闭时清理本文件的数学块会话状态（与 Python Cell 解耦，只清数学自己的键）。
  void clearMathSessionState(props.filePath, 'unmount')
  if (notebookSearchIndexSyncTimer) {
    window.clearTimeout(notebookSearchIndexSyncTimer)
    notebookSearchIndexSyncTimer = 0
  }
  if (notebookSearchQueryTimer) {
    window.clearTimeout(notebookSearchQueryTimer)
    notebookSearchQueryTimer = 0
  }
  stopSelectionAudioPlayback()
  closeNotebookSelectionTranslatePanel()
  clearNotebookSelectionState('component-unmount')
  document.removeEventListener('selectionchange', handleNotebookGlobalSelectionChange, true)
  document.removeEventListener('mousedown', handleNotebookGlobalMouseDown, true)
  document.removeEventListener('keydown', handleNotebookShortcutKeydown, true)
  window.removeEventListener(NOTEBOOK_DATALAB_EVENT, handleNotebookDataLabActionEvent)
  clearNotebookFocusedVariable()
  if (typeof notebookDataLabRpcUnlisten === 'function') {
    notebookDataLabRpcUnlisten()
    notebookDataLabRpcUnlisten = null
  }
  if (typeof disposeNotebookDataLabBridge === 'function') {
    disposeNotebookDataLabBridge()
    disposeNotebookDataLabBridge = null
  }
  disposeNotebookMarkdownWorker('component-unmount')
})

defineExpose({
  insertTemplateContent(content) {
    return insertTemplateContentIntoNotebook(content)
  },
  insertCell(type = 'python') {
    appendCell(type)
  },
  undo() {
    undoNotebookHistory()
  },
  redo() {
    redoNotebookHistory()
  },
  save() {
    requestNotebookSave()
  },
  navigateReadingBack() {
    return navigateToPreviousNotebookReadingLocation()
  },
  navigateReadingForward() {
    return navigateToNextNotebookReadingLocation()
  },
  canUndo() {
    return canUndoNotebookHistory.value
  },
  canRedo() {
    return canRedoNotebookHistory.value
  },
  openSearch(options = {}) {
    return openNotebookSearch(options)
  },
  closeSearch() {
    closeNotebookSearch()
  },
  isSearchOpen() {
    return notebookSearchVisible.value
  },
  getOutline() {
    const outline = createNotebookOutlinePayload()
    logger.info('[Notebook] expose outline snapshot', {
      count: outline.length,
    })
    return outline
  },
  jumpToOutlineLine(line) {
    return jumpToNotebookOutlineLine(line)
  },
  captureReadingPosition(reason = 'external-capture') {
    return recordNotebookReadingLocation(reason)
  },
  getReadingPosition() {
    return getCurrentNotebookReadingLocation()
  },
  applyReadingPosition(location, reason = 'external-apply', attempts = 10) {
    const normalizedAttempts = Math.max(1, Math.min(60, Number(attempts) || 10))
    return applyNotebookReadingLocationWithRetry(location, reason, normalizedAttempts)
  },
  restoreReadingPosition(reason = 'external-restore') {
    const back = notebookReadingLocationBackStack.value
    const location = back[back.length - 1]
    if (location) {
      return goToNotebookReadingLocation(location, { source: reason })
    }
    return false
  },
})
</script>

<style scoped>
.guaxi-notebook {
  --guaxi-notebook-bg: var(--guaxi-content-bg, #f7f9fc);
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 94%, var(--guaxi-content-bg, #f7f9fc) 6%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 86%, var(--guaxi-content-bg, #f7f9fc) 14%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 78%, var(--guaxi-content-bg, #f7f9fc) 22%);
  --guaxi-notebook-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 86%, transparent);
  --guaxi-notebook-strong-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 100%, rgba(15, 23, 42, 0.04));
  --guaxi-notebook-text: var(--guaxi-text-primary, #172033);
  --guaxi-notebook-muted: var(--guaxi-text-secondary, rgba(23, 32, 51, 0.72));
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 82%, var(--guaxi-content-bg, #f7f9fc) 18%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #f7f9fc) 30%);
  --guaxi-notebook-primary: #3b82f6;
  --guaxi-notebook-primary-hover: #2563eb;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(59, 130, 246, 0.2);
  --guaxi-notebook-shadow: 0 10px 28px rgba(15, 23, 42, 0.06);
  --guaxi-notebook-sticky-top: 6px;
  --guaxi-notebook-toolbar-shell-height: 8px;
  --guaxi-notebook-toolbar-max-width: min(100%, 520px);
  /* 设计说明：
   * 1. 探索模式整体字体统一追随主编辑器，避免 Markdown/Python/变量面板各自使用不同字族。
   * 2. `--editor-font-family` 定义在父级编辑器容器上，这里直接继承；若外层变量缺失，再回退到既有等宽字体。
   */
  --guaxi-notebook-font: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --guaxi-notebook-mono: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  min-height: 100%;
  /* 设计说明：
   * 1. 右侧紧邻页面滚动条，过大的右内边距会在滚动条旁形成一条明显空带。
   * 2. 左/上/下仍保持原有阅读留白，只单独收窄右侧，让内容边界更贴近滚动区但不贴边。
   */
  padding: 8px;
  background: var(--guaxi-notebook-bg, #f6f8fc);
  color: var(--guaxi-notebook-text, #172033);
  font-family: var(--guaxi-notebook-font);
  box-sizing: border-box;
  /* 设计说明：
   * 1. 探索模式外层只负责纵向阅读，不承担横向查看；代码、表格、公式等超宽内容由各自模块内部滚动。
   * 2. 根节点不能设置 overflow-x:hidden：浏览器会把它变成裁剪/滚动上下文，导致顶部浮动工具栏的隐藏态
   *    translate 区域被截断，鼠标移到顶部也无法重新命中。
   * 3. 横向溢出治理下沉到 cell、Monaco、MathJax 和输出模块内部，既保留工具栏浮现，也避免整页横向滚动。
   */
}

.guaxi-notebook.theme-dark {
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 97%, #0b1220 3%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 86%, #334155 14%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 74%, #334155 26%);
  --guaxi-notebook-border: rgba(148, 163, 184, 0.28);
  --guaxi-notebook-strong-border: rgba(148, 163, 184, 0.38);
  --guaxi-notebook-text: #d8e0eb;
  --guaxi-notebook-muted: rgba(221, 230, 241, 0.82);
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 68%, #475569 32%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 56%, #64748b 44%);
  --guaxi-notebook-primary: #7cc2ff;
  --guaxi-notebook-primary-hover: #4f9dff;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 28%, #334155 72%);
  --guaxi-notebook-focus-ring: rgba(96, 165, 250, 0.24);
  --guaxi-notebook-shadow: 0 14px 34px rgba(2, 6, 23, 0.24);
}

.guaxi-notebook.theme-light {
  --guaxi-notebook-primary: #2563eb;
  --guaxi-notebook-primary-hover: #1d4ed8;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(37, 99, 235, 0.18);
}

.guaxi-notebook__toolbar,
.guaxi-notebook__cell,
.guaxi-notebook__panel {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  background: var(--guaxi-notebook-panel);
  box-shadow: 0 4px 14px rgba(15, 23, 42, 0.04);
}

.guaxi-notebook.is-top-toolbar-visible {
  --guaxi-notebook-toolbar-shell-height: 50px;
}

.guaxi-notebook.has-active-python-cell {
  --guaxi-notebook-toolbar-max-width: min(100%, 760px);
}

.guaxi-notebook__toolbar-shell {
  position: sticky;
  top: var(--guaxi-notebook-sticky-top);
  z-index: 26;
  display: flex;
  justify-content: center;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  height: var(--guaxi-notebook-toolbar-shell-height);
  min-height: var(--guaxi-notebook-toolbar-shell-height);
  margin: 0 auto 2px;
  transition: height 0.18s ease, min-height 0.18s ease;
  /* 设计说明：
   * 1. 根节点不能用 overflow-x:hidden，否则顶部浮动工具栏的纵向浮现区域会被一起裁掉。
   * 2. 横向回归来自工具栏 max-content 宽度参与页面 scrollWidth；这里只在 shell 这一层裁掉横向溢出。
   * 3. overflow-x:clip 不产生新的横向滚动容器，同时保留 overflow-y:visible，鼠标移到顶部仍能命中浮动工具栏。
   */
  overflow-x: clip;
  overflow-y: visible;
}

.guaxi-notebook__toolbar {
  position: absolute;
  inset: 0 auto auto 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* 设计说明：
   * 1. 绝对定位元素搭配 `fit-content` 时，浏览器会走 shrink-to-fit 计算，视觉上容易出现底板宽度比按钮行更短。
   * 2. 这里改成按内容宽度强制撑开，再给一个可视区上限，保证顶部工具条既能完整包裹 icon，也不会在窄窗口冲出容器。
   */
  width: max-content;
  min-width: 0;
  max-width: min(var(--guaxi-notebook-toolbar-max-width), calc(100% - 24px));
  box-sizing: border-box;
  margin: 0;
  /* 设计说明：
   * 1. 顶部工具条需要明确留出上下呼吸感，否则 30px 的 icon 按钮会显得比背景条更高。
   * 2. 这里同步提高内边距和最小高度，让“底板”稳定包住整排入口，避免出现按钮悬在条外的观感。
   */
  min-height: 42px;
  padding: 6px 12px;
  box-sizing: border-box;
  backdrop-filter: blur(10px);
  box-shadow: 0 16px 36px rgba(15, 23, 42, 0.12);
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, calc(-100% + 8px));
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.guaxi-notebook__toolbar-shell.is-visible .guaxi-notebook__toolbar {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, 0);
}

.guaxi-notebook__search-panel-shell {
  position: sticky;
  top: calc(var(--guaxi-notebook-sticky-top) + var(--guaxi-notebook-toolbar-shell-height));
  z-index: 24;
  min-height: 36px;
  display: flex;
  align-items: flex-start;
  justify-content: flex-end;
  padding-right: 4px;
  pointer-events: none;
}

/* 查找控件本体的外观全部在 GuaxiFindWidget 内部，这里只负责定位与可点击。 */
.guaxi-notebook__find-widget {
  margin-top: 2px;
  pointer-events: auto;
}

.guaxi-notebook__panel-btn {
  height: 26px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
  color: var(--guaxi-notebook-text);
  cursor: pointer;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__panel-btn:hover {
  background: var(--guaxi-notebook-btn-hover);
}

.guaxi-notebook__panel-icon-btn {
  width: 26px;
  height: 26px;
  border-radius: 6px;
}

.guaxi-notebook__panel-icon-btn.danger {
  color: #dc2626;
  background: transparent;
  border-color: color-mix(in srgb, #dc2626 28%, var(--guaxi-notebook-strong-border));
}

.guaxi-notebook__panel-icon-btn.danger:hover {
  background: color-mix(in srgb, #dc2626 10%, var(--guaxi-notebook-panel) 90%);
}

.guaxi-notebook__selection-toolbar {
  position: absolute;
  z-index: 25;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 86%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 94%, transparent);
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.14);
  backdrop-filter: blur(10px);
}

.guaxi-notebook__selection-tool-btn {
  position: relative;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 9px;
  background: var(--guaxi-notebook-panel-soft);
  color: var(--guaxi-notebook-text);
  cursor: pointer;
  transition: transform 0.16s ease, background-color 0.16s ease, border-color 0.16s ease;
}

.guaxi-notebook__selection-tool-btn:hover:not(:disabled) {
  background: var(--guaxi-notebook-btn-hover);
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 36%, var(--guaxi-notebook-strong-border));
  transform: translateY(-1px);
}

.guaxi-notebook__selection-tool-btn:disabled {
  opacity: 0.56;
  cursor: not-allowed;
}

.guaxi-notebook__toolbar-main {
  display: flex;
  align-items: center;
  justify-content: center;
  width: max-content;
  min-width: 0;
  max-width: 100%;
  flex-wrap: nowrap;
  gap: 8px;
}

.guaxi-notebook__toolbar-divider {
  width: 1px;
  height: 18px;
  background: color-mix(in srgb, var(--guaxi-notebook-strong-border) 82%, transparent);
  opacity: 0.8;
}

.guaxi-notebook__toolbar-main--solo {
  width: auto;
  justify-content: center;
}

.guaxi-notebook__cell {
  position: relative;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  /* 设计说明：
   * 1. 主编辑器 Monaco 仍保留自己的 `horizontal:auto`，长 token 由编辑器内部滚动查看。
   * 2. Notebook cell 这一层只阻断子组件的 scrollWidth 继续传到探索者中间界面，避免页面级水平滚动条回归。
   * 3. delete 按钮、搜索 outline 等交互都在 cell 盒子内完成，所以裁掉横向溢出不会影响顶部浮动工具栏。
   */
  overflow-x: clip;
  overflow-y: visible;
  box-sizing: border-box;
  transition: box-shadow 0.18s ease, outline-color 0.18s ease, background-color 0.18s ease;
}

.guaxi-notebook__cell.is-search-match {
  outline: 1px solid color-mix(in srgb, var(--guaxi-notebook-primary) 24%, transparent);
  outline-offset: 2px;
  border-radius: 10px;
}

.guaxi-notebook__cell.is-search-active {
  outline: 2px solid color-mix(in srgb, var(--guaxi-notebook-primary) 62%, transparent);
  outline-offset: 2px;
  border-radius: 10px;
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--guaxi-notebook-primary) 16%, transparent);
}

.guaxi-notebook__cell.is-active-python-cell {
  outline: 2px solid color-mix(in srgb, var(--guaxi-notebook-primary) 72%, transparent);
  outline-offset: 0;
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-notebook-primary) 5%, var(--guaxi-notebook-panel) 95%);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--guaxi-notebook-primary) 10%, transparent);
}

.guaxi-notebook__cell.is-variable-focus-cell {
  outline-color: color-mix(in srgb, #f59e0b 72%, transparent);
  box-shadow: 0 0 0 4px color-mix(in srgb, #f59e0b 18%, transparent), var(--guaxi-notebook-shadow);
}

.guaxi-notebook__cell.is-markdown {
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  overflow-x: clip;
  overflow-y: visible;
}

.guaxi-notebook__insert-strip {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 18px;
  margin: 0;
  z-index: 3;
}

.guaxi-notebook__insert-line {
  width: 100%;
  height: 1px;
  background: color-mix(in srgb, var(--guaxi-notebook-border) 82%, transparent);
  opacity: 0.34;
  transition: opacity 0.2s ease;
}

.guaxi-notebook__insert-trigger {
  position: absolute;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 72%, transparent);
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 94%, var(--guaxi-notebook-bg) 6%);
  color: var(--guaxi-notebook-muted);
  opacity: 0.48;
  transform: scale(0.96);
  box-shadow: 0 2px 8px rgba(15, 23, 42, 0.05);
  transition: opacity 0.2s ease, transform 0.2s ease, border-color 0.2s ease;
  pointer-events: none;
  z-index: 1;
}

.guaxi-notebook__insert-actions {
  position: absolute;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 74%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 96%, var(--guaxi-notebook-bg) 4%);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.08);
  opacity: 0;
  transform: translateY(2px) scale(0.96);
  transition: opacity 0.2s ease, transform 0.2s ease;
  pointer-events: none;
  z-index: 2;
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-line,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-line {
  opacity: 0.7;
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-trigger,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-trigger {
  opacity: 0;
  transform: scale(0.82);
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-actions,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-actions {
  opacity: 1;
  transform: translateY(0) scale(1);
  pointer-events: auto;
}

.guaxi-notebook__cell-meta,
.guaxi-notebook__empty,
.guaxi-notebook__var-preview {
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__python-layout {
  display: grid;
  gap: 4px;
  padding: 0 2px 1px;
  min-width: 0;
  width: 100%;
  max-width: 100%;
  overflow-x: clip;
  overflow-y: visible;
  box-sizing: border-box;
}

.guaxi-notebook__math-layout {
  display: grid;
  gap: 6px;
  padding: 4px 2px 3px;
  min-width: 0;
  width: 100%;
  max-width: 100%;
  overflow-x: clip;
  overflow-y: visible;
  box-sizing: border-box;
}

.guaxi-notebook__math-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 30px;
  padding: 0 2px;
}

.guaxi-notebook__math-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  min-width: 52px;
  height: 24px;
  padding: 0 10px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 82%, var(--guaxi-notebook-panel) 18%);
  color: var(--guaxi-notebook-muted);
  font-size: 11px;
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-notebook__math-badge.is-busy {
  color: #1d4ed8;
  border-color: color-mix(in srgb, #60a5fa 48%, var(--guaxi-notebook-strong-border));
  box-shadow: 0 10px 24px rgba(37, 99, 235, 0.14);
}

.guaxi-notebook__math-badge-icon {
  flex: 0 0 auto;
}

.guaxi-notebook__math-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

/* 数学工具下拉菜单：定位锚点 + 分组浮层。 */
.guaxi-notebook__math-menu-anchor {
  position: relative;
  display: inline-flex;
}

/* 触发按钮内含 sigma + chevron 两个图标，固定的 30px 方形按钮塞不下会错位，
   因此这里放宽宽度并让两个图标紧凑排布。 */
.guaxi-notebook__math-menu-trigger {
  width: auto;
  min-width: 42px;
  gap: 1px;
  padding: 0 6px;
}

.guaxi-notebook__math-menu-anchor .guaxi-notebook__icon-btn.is-active {
  background: color-mix(in srgb, #0891b2 16%, transparent);
  color: color-mix(in srgb, #0e7490 86%, var(--guaxi-notebook-text));
}

.guaxi-notebook__math-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 40;
  min-width: 268px;
  max-width: 340px;
  max-height: 60vh;
  overflow-y: auto;
  padding: 6px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 10px;
  background: var(--guaxi-notebook-panel);
  box-shadow: 0 12px 32px rgba(15, 23, 42, 0.22);
}

.guaxi-notebook__math-menu-group + .guaxi-notebook__math-menu-group {
  margin-top: 4px;
  padding-top: 4px;
  border-top: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 60%, transparent);
}

.guaxi-notebook__math-menu-group-title {
  padding: 4px 8px 2px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: color-mix(in srgb, var(--guaxi-notebook-text) 52%, transparent);
}

.guaxi-notebook__math-menu-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: 7px 8px;
  border: none;
  border-radius: 7px;
  background: transparent;
  text-align: left;
  cursor: pointer;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__math-menu-item:hover {
  background: color-mix(in srgb, #0891b2 12%, transparent);
}

.guaxi-notebook__math-menu-item-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-notebook-text);
}

.guaxi-notebook__math-menu-item-hint {
  font-size: 11px;
  line-height: 1.35;
  color: color-mix(in srgb, var(--guaxi-notebook-text) 56%, transparent);
}

.guaxi-notebook__math-input {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  box-sizing: border-box;
}

.guaxi-notebook__math-preview {
  min-width: 0;
  max-width: 100%;
  min-height: 42px;
  padding: 8px 10px;
  border-left: 3px solid color-mix(in srgb, #0891b2 46%, var(--guaxi-notebook-strong-border));
  background: color-mix(in srgb, #0891b2 5%, transparent);
  font-size: 18px;
  font-family: var(--guaxi-notebook-font);
  overflow-x: auto;
  overflow-y: hidden;
  box-sizing: border-box;
}

/* 设计说明：
 * 1. MathJax 的 SVG 会把重复多行公式合成为非常宽的节点，必须把滚动边界锁在数学预览内部。
 * 2. display math 仍保持居中；只有公式本身超过可用宽度时，用户在预览区横向滚动查看。
 * 3. 这里不缩放公式，避免偏微分、矩阵、分式这类数学内容因为强行压缩而不可读。
 */
.guaxi-notebook__math-preview :deep(mjx-container) {
  display: block;
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  box-sizing: border-box;
}

.guaxi-notebook__math-preview :deep(mjx-container > svg) {
  max-width: none;
}

.guaxi-notebook__markdown-surface {
  position: relative;
  display: grid;
  gap: 6px;
  padding: 0;
  min-width: 0;
  max-width: 100%;
  overflow-x: clip;
  overflow-y: visible;
  box-sizing: border-box;
}

.guaxi-notebook__cell-delete {
  position: absolute;
  top: 10px;
  right: 10px;
  /* 高于代码块复制按钮(z-index:10)，保证删除按钮在代码块上也能点到（问题 3）。 */
  z-index: 12;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 78%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 95%, transparent);
  color: #dc2626;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.08);
  opacity: 0;
  transform: translateY(-4px);
  pointer-events: none;
  transition: opacity 0.18s ease, transform 0.18s ease, background-color 0.18s ease, border-color 0.18s ease;
}

.guaxi-notebook__cell:hover .guaxi-notebook__cell-delete,
.guaxi-notebook__cell:focus-within .guaxi-notebook__cell-delete {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.guaxi-notebook__cell-delete:hover {
  background: color-mix(in srgb, #ef4444 12%, var(--guaxi-notebook-panel) 88%);
  border-color: color-mix(in srgb, #ef4444 28%, var(--guaxi-notebook-strong-border));
}

/* 设计说明（探索者模式代码块复制按钮与删除按钮避让 · 对应问题 3）：
 * 1. 复制按钮（addCopyButtons 注入）保持在代码块「右上角」（其默认 right:12/top:12），符合用户预期。
 * 2. Markdown cell 的删除按钮也在右上角（cell top:10/right:10）。两者只有在「代码块位于 cell 顶部」时才会同处一角。
 * 3. 因此对「以代码块开头」的 cell 在顶部预留一段空白，把代码块整体下移，
 *    让删除按钮浮在代码块上方的留白处、复制按钮落在代码块内的右上角，二者上下错开、互不遮挡，
 *    且不改动代码块本身的宽度（满足“不缩短代码块宽度”的约束）。
 * 4. addCopyButtons 会把 <pre> 包进 div.guaxi-code-block-anchor，故首元素既可能是 <pre> 也可能是该包裹层，两种都要匹配。 */
.guaxi-notebook__markdown-surface:has(.guaxi-notebook__markdown-preview > pre:first-child),
.guaxi-notebook__markdown-surface:has(.guaxi-notebook__markdown-preview > .guaxi-code-block-anchor:first-child) {
  padding-top: 34px;
}

.guaxi-notebook__markdown-edit-shell {
  display: grid;
  gap: 8px;
  min-width: 0;
  max-width: 100%;
}

.guaxi-notebook__python-editor {
  width: 100%;
  min-height: 88px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  outline: none;
  resize: none;
  padding: 16px 18px;
  color: var(--guaxi-notebook-text);
  background: var(--guaxi-notebook-panel-soft);
  font-size: 14px;
  line-height: 1.7;
  box-sizing: border-box;
  overflow-y: hidden;
}

.guaxi-notebook__python-editor,
.guaxi-notebook__output-text {
  font-family: var(--guaxi-notebook-mono);
}

.guaxi-notebook__python-editor:focus,
.guaxi-notebook__markdown-preview-shell.is-clickable:focus-visible {
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 62%, var(--guaxi-notebook-border) 38%);
  box-shadow: 0 0 0 3px var(--guaxi-notebook-focus-ring);
}

.guaxi-notebook__markdown-preview-shell {
  border: none;
  border-radius: 0;
  background: transparent;
  padding: 6px;
  min-width: 0;
  max-width: 100%;
  overflow-x: clip;
  overflow-y: visible;
  box-sizing: border-box;
  font-family: var(--guaxi-notebook-font);
  font-size: 17px;
  line-height: 1.5;
}

.guaxi-notebook__markdown-preview-shell.is-clickable {
  cursor: text;
}

.guaxi-notebook__markdown-preview {
  min-height: 0;
  padding: 0;
  overflow: visible;
  font-family: var(--guaxi-notebook-font);
  font-size: 17px;
  line-height: 1.5;
}

.guaxi-notebook__cell-btn,
.guaxi-notebook__toolbar-btn,
.guaxi-notebook__icon-btn {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 10px;
  background: var(--guaxi-notebook-btn);
  color: var(--guaxi-notebook-text);
  padding: 7px 11px;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__cell-btn:hover,
.guaxi-notebook__toolbar-btn:hover,
.guaxi-notebook__icon-btn:hover {
  background: var(--guaxi-notebook-btn-hover);
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 32%, var(--guaxi-notebook-strong-border));
  box-shadow: 0 8px 18px rgba(15, 23, 42, 0.09);
  transform: translateY(-1px);
}

.guaxi-notebook__cell-btn.primary,
.guaxi-notebook__toolbar-btn.primary,
.guaxi-notebook__icon-btn.primary {
  color: #ffffff;
  border-color: transparent;
  background: linear-gradient(135deg, var(--guaxi-notebook-primary), var(--guaxi-notebook-primary-hover));
}

.guaxi-notebook__icon-btn.primary.is-busy {
  color: #ffffff;
  background: linear-gradient(135deg, #dc2626, #b91c1c);
  box-shadow: 0 10px 22px rgba(220, 38, 38, 0.22);
}

.guaxi-notebook__cell-btn.ghost,
.guaxi-notebook__toolbar-btn.ghost,
.guaxi-notebook__icon-btn.ghost {
  background: transparent;
}

.guaxi-notebook__cell-btn.danger,
.guaxi-notebook__icon-btn.danger {
  background: var(--guaxi-notebook-danger);
}

.guaxi-notebook__icon-btn {
  position: relative;
  width: 30px;
  height: 30px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 9px;
}

.guaxi-notebook__icon-btn::after,
.guaxi-notebook__selection-tool-btn::after {
  content: attr(data-tooltip) "  " attr(data-shortcut);
  position: absolute;
  left: 50%;
  bottom: calc(100% + 9px);
  z-index: 120;
  width: max-content;
  max-width: min(320px, calc(100vw - 48px));
  padding: 6px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, #111827 92%, transparent);
  color: #ffffff;
  font-size: 12px;
  line-height: 1.35;
  text-align: center;
  white-space: normal;
  overflow-wrap: anywhere;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, 5px);
  transition: opacity 0.14s ease, transform 0.14s ease;
  box-shadow: 0 10px 26px rgba(15, 23, 42, 0.22);
}

/* 设计说明：
 * 1. 顶部悬浮工具条本身贴近 Notebook 顶边，tooltip 向上弹出时容易被父级滚动容器裁掉。
 * 2. 这里仅让顶部工具条的 icon 提示向下展开；普通 cell 内部 icon 仍沿用向上提示，避免遮住当前输出区域。
 */
.guaxi-notebook__toolbar .guaxi-notebook__icon-btn::after {
  top: calc(100% + 9px);
  bottom: auto;
  transform: translate(-50%, -5px);
}

/* 设计说明：
 * 1. 顶部工具栏在激活 Python Cell 后会接近中间阅读区宽度，首尾按钮的居中 tooltip 容易碰到 shell 的横向裁剪边界。
 * 2. 首尾提示分别向内展开，既保留 shell 对横向回归的约束，也避免“打开 Python 官方文档助手”等长提示被右边截断。
 */
.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:first-child::after {
  left: 0;
  transform: translate(0, -5px);
}

.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:last-child::after {
  left: auto;
  right: 0;
  text-align: left;
  transform: translate(0, -5px);
}

/* 设计说明：
 * 1. Cell 右侧操作区贴近探索者中间区域边界，默认居中的 tooltip 会向右溢出并被相邻布局裁切。
 * 2. 这些按钮组统一改为右边缘对齐，让提示文字向左展开，避免“删除当前 Python Cell”等长文本被截断。
 */
.guaxi-notebook__python-toolbar-actions .guaxi-notebook__icon-btn::after,
.guaxi-notebook__math-actions .guaxi-notebook__icon-btn::after,
.guaxi-notebook__panel-actions .guaxi-notebook__icon-btn::after,
.guaxi-notebook__output-module-actions .guaxi-notebook__icon-btn::after {
  left: auto;
  right: 0;
  text-align: left;
  transform: translate(0, 5px);
}

.guaxi-notebook__icon-btn:not([data-tooltip])::after,
.guaxi-notebook__selection-tool-btn:not([data-tooltip])::after {
  display: none;
}

.guaxi-notebook__icon-btn[title]:not([data-tooltip])::after,
.guaxi-notebook__selection-tool-btn[title]:not([data-tooltip])::after {
  content: attr(title);
  display: block;
}

.guaxi-notebook__icon-btn:hover::after,
.guaxi-notebook__icon-btn:focus-visible::after,
.guaxi-notebook__selection-tool-btn:hover::after,
.guaxi-notebook__selection-tool-btn:focus-visible::after {
  opacity: 1;
  transform: translate(-50%, 0);
}

.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:first-child:hover::after,
.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:first-child:focus-visible::after,
.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:last-child:hover::after,
.guaxi-notebook__toolbar-main > .guaxi-notebook__icon-btn:last-child:focus-visible::after,
.guaxi-notebook__python-toolbar-actions .guaxi-notebook__icon-btn:hover::after,
.guaxi-notebook__python-toolbar-actions .guaxi-notebook__icon-btn:focus-visible::after,
.guaxi-notebook__math-actions .guaxi-notebook__icon-btn:hover::after,
.guaxi-notebook__math-actions .guaxi-notebook__icon-btn:focus-visible::after,
.guaxi-notebook__panel-actions .guaxi-notebook__icon-btn:hover::after,
.guaxi-notebook__panel-actions .guaxi-notebook__icon-btn:focus-visible::after,
.guaxi-notebook__output-module-actions .guaxi-notebook__icon-btn:hover::after,
.guaxi-notebook__output-module-actions .guaxi-notebook__icon-btn:focus-visible::after {
  transform: translate(0, 0);
}

.guaxi-notebook__icon-btn:disabled {
  opacity: 0.48;
  cursor: not-allowed;
  transform: none;
}

.guaxi-notebook__python-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
  min-height: 34px;
  padding: 4px 2px 4px;
}

.guaxi-notebook__python-toolbar-copy {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.guaxi-notebook__python-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  min-width: 52px;
  height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 82%, var(--guaxi-notebook-panel) 18%);
  border: 1px solid var(--guaxi-notebook-strong-border);
  color: var(--guaxi-notebook-muted);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-notebook__python-badge.is-busy {
  color: #1d4ed8;
  border-color: color-mix(in srgb, #60a5fa 48%, var(--guaxi-notebook-strong-border));
  box-shadow: 0 10px 24px rgba(37, 99, 235, 0.14);
}

.guaxi-notebook__python-badge-icon {
  flex: 0 0 auto;
}

.guaxi-notebook__busy-spin {
  animation: guaxi-notebook-busy-spin 0.9s linear infinite;
}

.guaxi-notebook__python-title {
  min-width: 0;
  color: var(--guaxi-notebook-muted);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.guaxi-notebook__python-toolbar-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 8px;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn:not(.primary) {
  background: color-mix(in srgb, var(--guaxi-notebook-btn) 82%, transparent);
}

.guaxi-notebook__python-collapsed-summary {
  min-width: 0;
  display: grid;
  gap: 4px;
  border: 1px dashed var(--guaxi-notebook-border);
  border-radius: 8px;
  padding: 10px 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 78%, transparent);
  color: var(--guaxi-notebook-text);
  text-align: left;
  cursor: pointer;
}

.guaxi-notebook__python-collapsed-summary:hover {
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 36%, var(--guaxi-notebook-border));
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 92%, var(--guaxi-notebook-panel) 8%);
}

.guaxi-notebook__python-collapsed-summary span {
  font-weight: 700;
}

.guaxi-notebook__python-collapsed-summary code {
  min-width: 0;
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.guaxi-notebook__stale-banner {
  margin: 4px 0 8px;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, #d97706 34%, var(--guaxi-notebook-border));
  border-radius: 8px;
  background: color-mix(in srgb, #f59e0b 12%, var(--guaxi-notebook-panel) 88%);
  color: color-mix(in srgb, #92400e 82%, var(--guaxi-notebook-text) 18%);
  font-size: 12px;
  line-height: 1.45;
}

.guaxi-notebook__panel {
  margin: 0;
  padding: 6px 0 0;
  border-radius: 0;
}

.guaxi-notebook__panel--math {
  padding-bottom: 0;
}

.guaxi-notebook__insert-actions .guaxi-notebook__icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 999px;
}

.guaxi-notebook__panel-content {
  display: grid;
  min-width: 0;
  gap: 6px;
  padding-bottom: 0;
}

.guaxi-notebook__output-module {
  min-width: 0;
  border-radius: 8px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 72%, var(--guaxi-notebook-panel) 28%);
  /* 设计说明：
   * 1. 输出模块头部的复制/折叠/删除按钮也使用同一套 ::after tooltip，浮层需要越过模块边框显示。
   * 2. 模块正文的超宽内容仍由 .guaxi-notebook__output-module-body 和具体渲染器内部滚动处理，这里只放开浮层裁剪。
   */
  overflow: visible;
}

.guaxi-notebook__output-module.is-collapsed {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 74%, var(--guaxi-notebook-panel) 26%);
}

.guaxi-notebook__output-module-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
  padding: 7px 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 74%, transparent);
}

.guaxi-notebook__output-module.is-collapsed .guaxi-notebook__output-module-head {
  border-bottom: none;
}

.guaxi-notebook__output-module-copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.guaxi-notebook__output-module-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--guaxi-notebook-text);
}

.guaxi-notebook__output-module-summary {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  word-break: break-word;
}

.guaxi-notebook__output-module-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.guaxi-notebook__output-module-body {
  display: grid;
  min-width: 0;
  max-width: 100%;
  gap: 8px;
  padding: 10px 12px 12px;
  overflow-x: hidden;
  box-sizing: border-box;
}

.guaxi-notebook__output-markdown {
  min-width: 0;
  max-width: 100%;
  color: var(--guaxi-notebook-text);
  font-size: 15px;
  line-height: 1.72;
  overflow-x: auto;
  overflow-y: hidden;
  box-sizing: border-box;
}

.guaxi-notebook__output-markdown :deep(h2),
.guaxi-notebook__output-markdown :deep(h3) {
  margin: 10px 0 7px;
  padding: 0;
  border-bottom: 0;
  font-size: 16px;
  line-height: 1.35;
  letter-spacing: 0;
}

.guaxi-notebook__output-markdown :deep(h2) {
  font-weight: 800;
}

.guaxi-notebook__output-markdown :deep(h3) {
  font-weight: 700;
  color: color-mix(in srgb, var(--guaxi-notebook-text) 82%, var(--guaxi-notebook-primary));
}

.guaxi-notebook__output-markdown :deep(p),
.guaxi-notebook__output-markdown :deep(ul) {
  margin: 8px 0;
}

.guaxi-notebook__output-markdown :deep(mjx-container) {
  max-width: 100%;
  margin: 10px 0 6px;
  font-size: 112%;
  overflow-x: auto;
  overflow-y: hidden;
  box-sizing: border-box;
}

/* 设计说明：
 * 1. 数学输出是运行结果，优先保证可读性；正文和标题适度放大，公式再额外放大一点。
 * 2. 超宽 MathJax SVG 留给输出模块内部滚动，避免把 Notebook 右侧面板整体顶出去。
 */
.guaxi-notebook__output-markdown :deep(mjx-container > svg) {
  max-width: none;
}

.guaxi-notebook__panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
  padding: 0 2px;
  flex-wrap: wrap;
}

.guaxi-notebook__panel-title {
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-notebook__panel-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.guaxi-notebook__output-text,
.guaxi-notebook__rich,
.guaxi-notebook__dataframe {
  margin: 0;
  border: none;
  border-radius: 0;
  background: transparent;
  padding: 0;
  max-height: min(62vh, 720px);
  overflow-x: auto;
  overflow-y: auto;
}

.guaxi-notebook__dataframe--virtual-table {
  /* 设计说明：
   * 1. GuaxiNotebookTableOutput 自带纵向/横向滚动视口，外层 dataframe 再滚动会形成嵌套滚动容器。
   * 2. 嵌套滚动在 Tauri 的 Chromium 里会让内外层交替合成与重绘，表现为滚一会儿后整个中间区域闪烁。
   * 3. 表格输出模块因此把滚动职责固定在子组件内部，普通 HTML 表格输出仍沿用上面的 auto 规则。
   */
  max-height: none;
  overflow: hidden;
  contain: layout paint;
}

.guaxi-notebook__rich {
  font-size: 17px;
  line-height: 1.5;
  padding-top: 2px;
}

.guaxi-notebook__output-text {
  white-space: pre-wrap;
  word-break: break-word;
  padding-top: 2px;
}

.guaxi-notebook__dataframe table {
  width: max-content;
  min-width: 100%;
  border-collapse: collapse;
}

.guaxi-notebook__dataframe th,
.guaxi-notebook__dataframe td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--guaxi-notebook-border);
  text-align: left;
}

.guaxi-notebook__dataframe th {
  color: var(--guaxi-notebook-text);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 72%, var(--guaxi-notebook-panel) 28%);
}

.guaxi-notebook__image {
  display: block;
  max-width: 100%;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
}

.guaxi-notebook__media {
  display: block;
  width: 100%;
  max-width: 100%;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
}

.guaxi-notebook__media--video {
  max-height: 520px;
}

.guaxi-notebook__empty--dashed {
  min-height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--guaxi-notebook-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 90%, var(--guaxi-notebook-panel-soft) 10%);
}

.guaxi-notebook__vars {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__var {
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--guaxi-notebook-panel-soft);
  border: 1px solid var(--guaxi-notebook-strong-border);
}

.guaxi-notebook__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.guaxi-notebook__var-name {
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__var-type {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook.theme-dark .guaxi-notebook__cell-btn,
.guaxi-notebook.theme-dark .guaxi-notebook__toolbar-btn,
.guaxi-notebook.theme-dark .guaxi-notebook__icon-btn {
  color: #d8e0eb;
}

.guaxi-notebook.theme-dark .guaxi-notebook__cell-btn.primary,
.guaxi-notebook.theme-dark .guaxi-notebook__toolbar-btn.primary,
.guaxi-notebook.theme-dark .guaxi-notebook__icon-btn.primary {
  color: #08111f;
  font-weight: 700;
}

.guaxi-notebook.theme-dark .guaxi-notebook__rich,
.guaxi-notebook.theme-dark .guaxi-notebook__output-text,
.guaxi-notebook.theme-dark .guaxi-notebook__dataframe,
.guaxi-notebook.theme-dark .guaxi-notebook__output-module,
.guaxi-notebook.theme-dark .guaxi-notebook__var,
.guaxi-notebook.theme-dark .guaxi-notebook__python-editor {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 88%, #0f172a 12%);
}

@keyframes guaxi-notebook-busy-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.guaxi-notebook__markdown-preview :deep(.md-paragraph),
.guaxi-notebook__rich :deep(.md-paragraph) {
  color: var(--guaxi-notebook-text);
}

/* 设计说明：
 * 1. Explorer 模式下也需要区分“Markdown 渲染器生成的普通段落”和“作者手写的 HTML 段落”。
 * 2. 对于显式带 `align` 的 HTML 段落，这里恢复其原始对齐语义，避免被后续基础样式吞掉。
 */
.guaxi-notebook__markdown-preview :deep(p[align='center']),
.guaxi-notebook__rich.markdown-body :deep(p[align='center']) {
  text-align: center !important;
}

.guaxi-notebook__markdown-preview :deep(p[align='right']),
.guaxi-notebook__rich.markdown-body :deep(p[align='right']) {
  text-align: right !important;
}

.guaxi-notebook__markdown-preview :deep(p[align='left']),
.guaxi-notebook__rich.markdown-body :deep(p[align='left']) {
  text-align: left !important;
}

.guaxi-notebook__markdown-preview :deep(*),
.guaxi-notebook__rich.markdown-body :deep(*) {
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__markdown-preview :deep(a),
.guaxi-notebook__markdown-preview :deep(a:visited),
.guaxi-notebook__rich.markdown-body :deep(a),
.guaxi-notebook__rich.markdown-body :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

/* 设计说明：
 * 1. 这里的自动居中只服务于 Markdown 语法生成的图片与媒体组。
 * 2. 手写 HTML 中的图片布局需要交给作者自己的段落 / 行内结构控制，不能再被统一 `margin:auto`
 *    覆盖，否则徽章组、Logo 组会被错误地堆叠并居中。
 */
.guaxi-notebook__markdown-preview :deep(.md-paragraph img),
.guaxi-notebook__rich.markdown-body :deep(.md-paragraph img),
.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__image) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 0 auto;
}

.guaxi-notebook__markdown-preview :deep(p > img:only-child),
.guaxi-notebook__markdown-preview :deep(p > a:only-child > img:only-child),
.guaxi-notebook__rich.markdown-body :deep(p > img:only-child),
.guaxi-notebook__rich.markdown-body :deep(p > a:only-child > img:only-child) {
  display: block;
  margin-left: auto;
  margin-right: auto;
}

/* 设计说明：
 * 1. 对于作者直接写进 Markdown 的 HTML 图片块，需要保留其 inline / badge 排版能力。
 * 2. 因此带 `align` / `style="text-align"` 的段落里，把图片恢复为 inline-block，
 *    由段落自身的对齐属性来控制最终布局。
 */
.guaxi-notebook__markdown-preview :deep(p[align] > img),
.guaxi-notebook__markdown-preview :deep(p[align] > a > img),
.guaxi-notebook__markdown-preview :deep(p[style*='text-align'] > img),
.guaxi-notebook__markdown-preview :deep(p[style*='text-align'] > a > img),
.guaxi-notebook__rich.markdown-body :deep(p[align] > img),
.guaxi-notebook__rich.markdown-body :deep(p[align] > a > img),
.guaxi-notebook__rich.markdown-body :deep(p[style*='text-align'] > img),
.guaxi-notebook__rich.markdown-body :deep(p[style*='text-align'] > a > img) {
  display: inline-block;
  max-width: 100%;
  height: auto;
  margin: 0;
  vertical-align: middle;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group) {
  --guaxi-media-gap: clamp(14px, 2vw, 24px);
  display: flex !important;
  flex-wrap: wrap;
  justify-content: center !important;
  align-items: stretch;
  align-content: flex-start;
  gap: var(--guaxi-media-gap);
  width: 100% !important;
  max-width: 1000px;
  margin: 22px auto !important;
  padding: 6px 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single) {
  display: block !important;
  width: 100% !important;
  max-width: 100%;
  gap: 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-gallery),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-gallery) {
  display: grid !important;
  /* 设计说明：
   * 1. 探索者模式中的 Markdown Cell 是分段渲染的，相邻图片常在同一个媒体组中出现。
   * 2. 图集使用等宽 1fr 列，避免每张图按原始宽度各自收缩造成左右边缘不齐。
   * 3. 单图不走这里的网格规则，继续按原始尺寸优先、只在超过阅读宽度时收缩。
   */
  grid-template-columns: repeat(auto-fit, minmax(min(260px, 100%), 1fr));
  justify-content: center !important;
  justify-items: stretch;
  align-items: start;
  padding-left: 6px;
  padding-right: 6px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
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

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item:hover) {
  opacity: 0.98;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single > .guaxi-media-group__item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single > .guaxi-media-group__item) {
  display: flex;
  flex: 0 1 auto;
  min-width: 0;
  width: 100%;
  max-width: 100%;
  margin-left: auto;
  margin-right: auto;
  padding: 0;
  border: none;
  background: transparent;
  box-shadow: none;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-gallery > .guaxi-media-group__item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-gallery > .guaxi-media-group__item) {
  width: 100%;
  max-width: 100%;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__image) {
  display: block;
  width: 100%;
  max-width: 100%;
  max-height: min(56vh, 420px);
  height: auto;
  margin: 0;
  border-radius: 14px;
  object-fit: contain;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single .guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single .guaxi-media-group__image) {
  /* 设计说明：
   * 1. 探索者模式的单张 Markdown 图片应和普通预览一致：优先保持原始宽高，不再被 760px/视口高度限制提前压缩。
   * 2. `width: auto` 避免短图被放大；`max-width: 100%` 让大图尽量占满当前 Markdown Cell 的阅读宽度。
   */
  width: auto;
  max-width: 100%;
  max-height: none;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
  background: transparent;
  box-shadow: none;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item:hover) {
  opacity: 0.98;
}

@media (max-width: 760px) {
  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group),
  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-gallery),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-gallery) {
    width: 100% !important;
  }

  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
    max-width: 100%;
    width: 100%;
  }
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a:visited),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card) {
  display: inline-flex;
  max-width: min(360px, 100%);
  vertical-align: top;
  margin: 6px 0;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 75%, transparent 25%);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 86%, var(--guaxi-notebook-panel) 14%);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.08);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link) {
  display: inline-flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px 12px;
  min-width: 0;
  text-decoration: none;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 92%, var(--guaxi-notebook-panel) 8%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__eyebrow),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__eyebrow) {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__title),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__title) {
  font-size: 13px;
  font-weight: 700;
  color: var(--guaxi-notebook-text);
  line-height: 1.35;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__excerpt),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__excerpt) {
  font-size: 12px;
  line-height: 1.45;
  color: var(--guaxi-notebook-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__meta),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__meta) {
  font-size: 11px;
  color: var(--guaxi-notebook-muted);
  opacity: 0.9;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 90%, var(--guaxi-notebook-panel) 10%);
  border-color: rgba(148, 163, 184, 0.22);
  box-shadow: 0 12px 28px rgba(2, 6, 23, 0.35);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 96%, var(--guaxi-notebook-panel) 4%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc) {
  display: grid;
  gap: 10px;
  margin: 6px 0;
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 78%, transparent 22%);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 88%, var(--guaxi-notebook-panel) 12%) !important;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-title),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-title) {
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-list),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-list) {
  margin: 0;
  padding-left: 0;
  list-style: none;
  display: grid;
  gap: 6px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item) {
  margin: 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-2),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-2) {
  padding-left: 18px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-3),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-3) {
  padding-left: 36px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-4),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-4) {
  padding-left: 54px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-5),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-5) {
  padding-left: 72px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-6),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-6) {
  padding-left: 90px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link) {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 9px;
  color: var(--guaxi-notebook-text);
  text-decoration: none;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 82%, var(--guaxi-notebook-panel-soft) 18%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 92%, var(--guaxi-notebook-panel) 8%);
  border-color: color-mix(in srgb, var(--guaxi-notebook-accent, #38bdf8) 28%, transparent 72%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--guaxi-notebook-accent, #38bdf8) 16%, transparent 84%) inset;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-empty),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-empty) {
  font-size: 13px;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 90%, var(--guaxi-notebook-panel) 10%) !important;
  border-color: rgba(148, 163, 184, 0.24);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 82%, var(--guaxi-notebook-panel-soft) 18%);
  color: #e2e8f0;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 96%, var(--guaxi-notebook-panel) 4%);
  border-color: rgba(74, 202, 219, 0.28);
  box-shadow: 0 0 0 1px rgba(74, 202, 219, 0.16) inset;
}

/* 搜索命中配色与预览/分屏模式共用一套取值（见 GuaxiEditor 的同名规则）。 */
:deep(.guaxi-search-hit) {
  background: var(--guaxi-search-hit-bg, rgba(234, 92, 0, 0.33));
  border-radius: 2px;
  padding: 0 1px;
}

:deep(.guaxi-search-hit-active) {
  background: var(--guaxi-search-hit-active-bg, rgba(234, 92, 0, 0.62));
  outline: 1px solid var(--guaxi-search-hit-active-border, rgba(246, 154, 60, 0.9));
  outline-offset: -1px;
}

/* 设计说明（对应“选中没有精美的选中效果”）：
   与预览模式共用同一组取值，避免同一份文档在两个模式下选中色不一样。
   只改背景、保留前景色，代码块的语法着色不会被抹平。 */
.guaxi-notebook ::selection,
.guaxi-notebook :deep(::selection) {
  background: var(--guaxi-selection-bg, color-mix(in srgb, var(--guaxi-notebook-primary, #3b82f6) 34%, transparent));
  color: inherit;
}

.guaxi-notebook ::-moz-selection,
.guaxi-notebook :deep(::-moz-selection) {
  background: var(--guaxi-selection-bg, color-mix(in srgb, var(--guaxi-notebook-primary, #3b82f6) 34%, transparent));
  color: inherit;
}

.guaxi-notebook :deep(pre)::selection,
.guaxi-notebook :deep(pre ::selection),
.guaxi-notebook :deep(code)::selection,
.guaxi-notebook :deep(code ::selection) {
  background: var(--guaxi-selection-strong-bg, color-mix(in srgb, var(--guaxi-notebook-primary, #3b82f6) 48%, transparent));
}

.guaxi-notebook__markdown-preview :deep(code),
.guaxi-notebook__markdown-preview :deep(pre),
.guaxi-notebook__rich :deep(code),
.guaxi-notebook__rich :deep(pre) {
  font-family: var(--guaxi-notebook-mono);
}

.guaxi-notebook__markdown-preview :deep(pre),
.guaxi-notebook__rich :deep(pre) {
  max-width: 100%;
}

.guaxi-notebook__markdown-preview :deep(.hljs),
.guaxi-notebook__rich :deep(.hljs) {
  border-radius: 10px;
  width: 100%;
  box-sizing: border-box;
}

.guaxi-notebook__markdown-preview :deep(mjx-container),
.guaxi-notebook__rich :deep(mjx-container) {
  overflow-x: auto;
  overflow-y: hidden;
}

.guaxi-notebook__markdown-preview :deep(.copy-button),
.guaxi-notebook__rich :deep(.copy-button) {
  opacity: 0.92;
}

@media (max-width: 1100px) {
  .guaxi-notebook__toolbar-shell,
  .guaxi-notebook.is-top-toolbar-visible {
    --guaxi-notebook-toolbar-shell-height: 50px;
  }

  /* 设计说明：
   * 1. 顶部中间工具条承担运行、保存等高频操作，窄视口下仍需要保持横向一排，避免入口被挤成纵向难以扫视。
   * 2. Python Cell 工具条仍保留纵向折叠能力，因为它内容更长，窄屏时改成列布局更容易容纳。
   */
  .guaxi-notebook__toolbar {
    position: relative;
    inset: auto;
    opacity: 1;
    pointer-events: auto;
    transform: none;
    align-items: center;
    flex-direction: row;
  }

  .guaxi-notebook__toolbar-main {
    flex-wrap: nowrap;
    justify-content: center;
    gap: 8px;
  }

  .guaxi-notebook__python-toolbar {
    position: relative;
    inset: auto;
    opacity: 1;
    pointer-events: auto;
    transform: none;
    align-items: flex-start;
    flex-direction: column;
  }

  .guaxi-notebook__search-panel-shell {
    position: static;
    top: auto;
    height: auto;
    display: flex;
    justify-content: flex-start;
    pointer-events: auto;
  }

  .guaxi-notebook__find-widget {
    margin-top: -2px;
  }

  .guaxi-notebook__insert-actions {
    padding: 0 6px;
  }
}
</style>
