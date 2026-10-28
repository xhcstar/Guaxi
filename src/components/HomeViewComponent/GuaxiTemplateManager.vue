<template>
  <div
    ref="pageRef"
    class="tm"
    :class="{
      'is-inspector-collapsed': inspectorCollapsed,
      'is-browser-collapsed': browserCollapsed,
      'is-dragging': dragActive,
    }"
    tabindex="-1"
    @keydown="onKeydown"
  >
    <!-- ───────────── 顶栏 ───────────── -->
    <header class="tm-header">
      <button
        type="button"
        class="tm-icon-btn is-pane-toggle"
        :title="browserCollapsed ? '显示模板列表（Ctrl+B）' : '收起模板列表（Ctrl+B）'"
        :aria-label="browserCollapsed ? '显示模板列表' : '收起模板列表'"
        :aria-expanded="!browserCollapsed"
        @click="toggleBrowser"
      >
        <Icon :icon="browserCollapsed ? panelLeftOpenIcon : panelLeftCloseIcon" width="16" height="16" />
      </button>

      <div class="tm-brand">
        <span class="tm-brand-mark" aria-hidden="true">
          <Icon :icon="libraryIcon" width="17" height="17" />
        </span>
        <h1 class="tm-title">模板</h1>
        <button
          type="button"
          class="tm-sync-pill"
          :class="{ 'is-busy': loading }"
          :title="repoTitle"
          @click="revealRepo"
        >
          <span class="tm-sync-dot" :data-state="loading ? 'running' : repoPath ? 'success' : 'idle'" />
          {{ repoPath ? '模板库已连接' : '仅内置模板' }}
        </button>
      </div>

      <div class="tm-search">
        <Icon :icon="searchIcon" width="15" height="15" class="tm-search-icon" />
        <input
          ref="searchRef"
          v-model="search"
          type="search"
          class="tm-search-input"
          placeholder="搜索模板名称、分类或正文"
          spellcheck="false"
          @keydown.esc="search = ''"
        />
        <kbd v-if="!search" class="tm-kbd">/</kbd>
        <button v-else type="button" class="tm-search-clear" title="清除搜索" @click="search = ''">
          <Icon :icon="xIcon" width="13" height="13" />
        </button>
      </div>

      <div class="tm-header-actions">
        <button type="button" class="tm-btn" title="新建空白模板" :disabled="busy" @click="openDraft">
          <Icon :icon="plusIcon" width="15" height="15" />
          <span>新建</span>
        </button>
        <button type="button" class="tm-btn" title="从 Markdown 文件导入模板" :disabled="busy" @click="importFromFile">
          <Icon :icon="fileUpIcon" width="15" height="15" />
          <span>导入</span>
        </button>
        <button
          type="button"
          class="tm-icon-btn"
          :class="{ 'is-busy': loading }"
          title="刷新模板列表（F5）"
          aria-label="刷新模板列表"
          @click="refresh({ reason: 'manual' })"
        >
          <Icon :icon="refreshIcon" width="16" height="16" />
        </button>
      </div>

      <button
        type="button"
        class="tm-icon-btn is-pane-toggle"
        :title="inspectorCollapsed ? '显示预览面板（Ctrl+Alt+P）' : '收起预览面板（Ctrl+Alt+P）'"
        :aria-label="inspectorCollapsed ? '显示预览面板' : '收起预览面板'"
        :aria-expanded="!inspectorCollapsed"
        @click="toggleInspector"
      >
        <Icon :icon="inspectorCollapsed ? panelOpenIcon : panelCloseIcon" width="16" height="16" />
      </button>
    </header>

    <!-- ───────────── 工具栏 ───────────── -->
    <div class="tm-toolbar">
      <div class="tm-filters" role="group" aria-label="按分类筛选">
        <button
          type="button"
          class="tm-chip"
          :class="{ 'is-active': familyFilter === 'all' }"
          @click="setFamilyFilter('all')"
        >
          全部
          <span class="tm-chip-count">{{ summary.total }}</span>
        </button>
        <button
          v-for="chip in familyChips"
          :key="chip.family"
          type="button"
          class="tm-chip"
          :class="{ 'is-active': familyFilter === chip.family }"
          :title="chip.family"
          @click="setFamilyFilter(chip.family)"
        >
          {{ chip.family }}
          <span class="tm-chip-count">{{ chip.count }}</span>
        </button>
      </div>

      <div v-if="hasDraft && !inspectorCollapsed" class="tm-inline-fields">
        <label class="tm-inline-field is-name">
          <span class="tm-inline-label">名称</span>
          <input
            v-model="draft.name"
            class="tm-inline-input"
            placeholder="模板名称"
            spellcheck="false"
            aria-label="模板名称"
            @input="markDirty"
          />
        </label>
        <label class="tm-inline-field is-type">
          <span class="tm-inline-label">分类</span>
          <input
            v-model="draft.type"
            class="tm-inline-input"
            placeholder="例如 工作 / 周报"
            spellcheck="false"
            aria-label="模板分类"
            list="tm-type-options"
            @input="markDirty"
          />
        </label>
        <span v-if="draft.builtin" class="tm-badge">内置</span>
        <span v-else-if="draft.isUser" class="tm-badge is-user">自定义</span>
      </div>

      <div class="tm-view-actions">
        <button
          v-if="searchActive"
          type="button"
          class="tm-scope-pill"
          :class="{ 'is-on': searchContent }"
          :title="searchContent ? '正文命中也会列出来' : '只在名称和分类里找'"
          @click="toggleSearchContent"
        >
          <Icon :icon="searchContent ? fileTextIcon : textIcon" width="13" height="13" />
          {{ searchContent ? '含正文' : '仅标题' }}
        </button>

        <div class="tm-segmented" role="group" aria-label="视图">
          <button
            v-for="mode in viewModes"
            :key="mode.key"
            type="button"
            class="tm-seg"
            :class="{ 'is-active': viewMode === mode.key }"
            :title="mode.title"
            :aria-pressed="viewMode === mode.key"
            @click="setViewMode(mode.key)"
          >
            <Icon :icon="mode.icon" width="15" height="15" />
          </button>
        </div>
      </div>
    </div>

    <!-- ───────────── 主体：列表 + 文档 ───────────── -->
    <main class="tm-main" :style="mainStyle">
      <section v-if="!browserCollapsed" class="tm-browser">
        <div v-if="loadError" class="tm-banner is-danger">
          <Icon :icon="alertIcon" width="15" height="15" />
          <span class="tm-banner-text">{{ loadError }}</span>
          <button type="button" class="tm-link-btn" @click="refresh({ reason: 'retry' })">重试</button>
        </div>
        <div v-if="truncated" class="tm-banner is-warning">
          <Icon :icon="alertIcon" width="15" height="15" />
          自定义模板超过 {{ formatCount(MAX_TEMPLATES) }} 个，只载入了前一部分。
        </div>

        <div v-if="loading && !templates.length" class="tm-state">
          <span class="tm-spinner" />
          <p>正在读取模板库…</p>
        </div>

        <div v-else-if="!groups.length" class="tm-state">
          <div class="tm-state-glyph">
            <Icon :icon="searchActive ? searchIcon : folderSearchIcon" width="32" height="32" />
          </div>
          <h2>{{ emptyTitle }}</h2>
          <p>{{ emptyHint }}</p>
          <button v-if="searchActive" type="button" class="tm-btn" @click="search = ''">清除搜索</button>
          <button v-else-if="canMutate" type="button" class="tm-btn" @click="openDraft">
            <Icon :icon="plusIcon" width="15" height="15" />
            新建第一个模板
          </button>
        </div>

        <div
          v-else
          ref="listRef"
          class="tm-list"
          :class="`is-${searchActive ? 'results' : viewMode}`"
          role="tree"
          aria-label="模板列表"
        >
          <template v-if="searchActive">
            <div
              v-for="result in searchResults"
              :key="result.item.name"
              class="tm-row is-result"
              :class="{
                'is-selected': result.item.name === selectedName,
                'is-primary': result.item.name === selectedName,
                'is-dirty': isDirty(result.item.name),
              }"
              :data-name="result.item.name"
              role="treeitem"
              :aria-selected="result.item.name === selectedName"
              @click="selectTemplate(result.item.name)"
              @dblclick="insertTemplate(result.item)"
              @contextmenu.prevent="openMenu(result.item, $event)"
            >
              <span class="tm-row-main">
                <span class="tm-type-icon" :data-kind="result.item.kind">
                  <Icon :icon="result.item.kind === 'python' ? codeIcon : fileTextIcon" width="15" height="15" />
                </span>
                <span class="tm-row-text">
                  <span class="tm-row-title">
                    <span class="tm-row-name">{{ result.item.name }}</span>
                    <span v-if="result.hit.field === 'content' && result.hit.snippet" class="tm-row-snippet">
                      <span class="tm-snippet-line">第 {{ result.hit.snippet.line }} 行</span>
                      <span class="tm-snippet-text">
                        {{ result.hit.snippet.prefix }}<mark>{{ result.hit.snippet.match }}</mark>{{ result.hit.snippet.suffix }}
                      </span>
                    </span>
                    <span v-else class="tm-row-sub">{{ result.item.type }}</span>
                  </span>
                </span>
                <span v-if="result.item.builtin" class="tm-badge">内置</span>
                <span v-else-if="result.item.overriddenBuiltin" class="tm-badge is-user">已改</span>
              </span>
              <button
                type="button"
                class="tm-row-insert"
                title="插入到编辑器（Enter）"
                aria-label="插入到编辑器"
                @click.stop="insertTemplate(result.item)"
              >
                <Icon :icon="sendIcon" width="14" height="14" />
              </button>
            </div>
          </template>

          <template v-else-if="viewMode === 'grid'">
            <button
              v-for="item in flatTemplates"
              :key="item.name"
              type="button"
              class="tm-tile"
              :class="{
                'is-primary': item.name === selectedName,
                'is-dirty': isDirty(item.name),
              }"
              :data-name="item.name"
              @click="selectTemplate(item.name)"
              @dblclick="insertTemplate(item)"
              @contextmenu.prevent="openMenu(item, $event)"
            >
              <span class="tm-tile-head">
                <span class="tm-type-icon" :data-kind="item.kind">
                  <Icon :icon="item.kind === 'python' ? codeIcon : fileTextIcon" width="14" height="14" />
                </span>
                <span class="tm-tile-family">{{ templateFamilyOf(item.type) }}</span>
                <span v-if="item.builtin" class="tm-badge is-quiet">内置</span>
              </span>
              <span class="tm-tile-name" :title="item.name">{{ item.name }}</span>
              <span class="tm-tile-excerpt">{{ excerptOf(item.content) || '（空模板）' }}</span>
              <span class="tm-tile-foot">
                <span class="tm-tile-variant">{{ templateVariantOf(item.type) || item.type }}</span>
                <span class="tm-tile-len">{{ formatChars(item.content.length) }}</span>
              </span>
            </button>
          </template>

          <template v-else>
            <section
              v-for="group in groups"
              :key="group.family"
              class="tm-group"
            >
              <header class="tm-group-head">
                <button type="button" class="tm-group-toggle" @click="toggleGroup(group.family)">
                  <Icon
                    :icon="chevronRightIcon"
                    width="12"
                    height="12"
                    class="tm-chevron"
                    :class="{ 'is-open': !collapsedGroups.has(group.family) }"
                  />
                  <span class="tm-group-name">{{ group.family }}</span>
                  <span class="tm-group-count">{{ group.items.length }}</span>
                </button>
                <span v-if="group.variantized" class="tm-group-hint">按子类排序</span>
              </header>

              <template v-if="!collapsedGroups.has(group.family)">
                <div
                  v-for="item in group.items"
                  :key="item.name"
                  class="tm-row"
                  :class="{
                    'is-selected': item.name === selectedName,
                    'is-primary': item.name === selectedName,
                    'is-dirty': isDirty(item.name),
                  }"
                  :data-name="item.name"
                  role="treeitem"
                  :aria-selected="item.name === selectedName"
                  @click="selectTemplate(item.name)"
                  @dblclick="insertTemplate(item)"
                  @contextmenu.prevent="openMenu(item, $event)"
                >
                  <span class="tm-row-main">
                    <span class="tm-type-icon" :data-kind="item.kind">
                      <Icon :icon="item.kind === 'python' ? codeIcon : fileTextIcon" width="15" height="15" />
                    </span>
                    <span class="tm-row-text">
                      <span class="tm-row-name">{{ item.name }}</span>
                      <span v-if="group.variantized" class="tm-row-variant">{{ templateVariantOf(item.type) }}</span>
                    </span>
                    <span v-if="isDirty(item.name)" class="tm-dirty-dot" title="有未保存的改动" />
                    <span v-if="item.builtin" class="tm-badge is-quiet">内置</span>
                    <span v-else-if="item.overriddenBuiltin" class="tm-badge is-user">已改</span>
                  </span>
                  <button
                    type="button"
                    class="tm-row-insert"
                    title="插入到编辑器（Enter）"
                    aria-label="插入到编辑器"
                    @click.stop="insertTemplate(item)"
                  >
                    <Icon :icon="sendIcon" width="14" height="14" />
                  </button>
                </div>
              </template>
            </section>
          </template>
        </div>

        <footer class="tm-status">
          <span>{{ formatCount(visibleCount) }} 个模板</span>
          <span class="tm-status-sep">·</span>
          <span>内置 {{ formatCount(summary.builtin) }}</span>
          <span class="tm-status-sep">·</span>
          <span>自定义 {{ formatCount(summary.user) }}</span>
          <span class="tm-status-spacer" />
          <span v-if="dirtyName" class="tm-status-dirty">
            <span class="tm-dirty-dot" />
            未保存
          </span>
          <span v-else-if="selectedName" class="tm-status-path" :title="selectedName">{{ selectedName }}</span>
        </footer>
      </section>

      <div
        v-if="!inspectorCollapsed && !browserCollapsed"
        class="tm-resizer"
        title="拖动调整文档宽度"
        @pointerdown="startResize"
      />

      <aside v-if="!inspectorCollapsed" class="tm-inspector">
        <template v-if="hasDraft">
          <div class="tm-doc-head">
            <div class="tm-doc-title">
              <span class="tm-type-icon" :data-kind="draftKind">
                <Icon :icon="draftKind === 'python' ? codeIcon : fileTextIcon" width="15" height="15" />
              </span>
              <span class="tm-doc-name" :title="draft.name">{{ draft.name || '未命名模板' }}</span>
            </div>
            <div class="tm-doc-head-actions">
              <button
                type="button"
                class="tm-btn"
                :disabled="busy || !draft.content.trim()"
                title="把正文插入到主编辑器光标处（Ctrl+Enter）"
                @click="insertDraft"
              >
                <Icon :icon="sendIcon" width="15" height="15" />
                <span>插入</span>
              </button>
              <button
                type="button"
                class="tm-btn is-primary"
                :disabled="busy || !dirty"
                :title="dirty ? '保存模板（Ctrl+S）' : '没有改动'"
                @click="saveDraft"
              >
                <Icon :icon="saveIcon" width="15" height="15" />
                <span>保存</span>
              </button>
              <button
                type="button"
                class="tm-icon-btn is-small"
                title="关闭文档"
                @click="closeDraft"
              >
                <Icon :icon="xIcon" width="14" height="14" />
              </button>
            </div>
          </div>

          <div class="tm-doc-preview-toggle">
            <button
              type="button"
              class="tm-tab"
              :class="{ 'is-active': docMode === 'edit' }"
              @click="setDocMode('edit')"
            >
              <Icon :icon="pencilIcon" width="13" height="13" />
              编辑
            </button>
            <button
              type="button"
              class="tm-tab"
              :class="{ 'is-active': docMode === 'preview' }"
              @click="setDocMode('preview')"
            >
              <Icon :icon="eyeIcon" width="13" height="13" />
              预览
            </button>
            <span class="tm-doc-stats">
              {{ stats.lines }} 行 · {{ formatChars(stats.chars) }}
            </span>
          </div>

          <div class="tm-doc-stage">
            <div
              ref="editorContainer"
              class="tm-monaco"
              :class="{ 'is-hidden': docMode !== 'edit' }"
            />
            <div
              v-if="docMode === 'edit'"
              class="tm-empty-overlay"
              :class="{ 'is-visible': !draft.content.trim() }"
              aria-hidden="true"
            >
              <p class="tm-empty-title">{{ emptyStageTitle }}</p>
              <p class="tm-empty-hint">{{ emptyStageHint }}</p>
            </div>
            <div v-else class="tm-preview">
              <div v-if="previewLoading" class="tm-preview-loading">
                <span class="tm-spinner is-small" />
                正在渲染预览…
              </div>
              <div v-else-if="previewError" class="tm-preview-fallback">
                <p class="tm-empty-title">预览暂时不可用</p>
                <p class="tm-empty-hint">{{ previewError }}</p>
                <button type="button" class="tm-btn is-quiet" @click="renderPreview">重试</button>
              </div>
              <div
                v-else
                ref="previewRef"
                class="tm-preview-body markdown-content"
                v-html="previewHtml"
              />
            </div>
          </div>

          <div class="tm-doc-foot">
            <GuaxiAuxEditorStatusBar :editor-instance="editorInstance" :language="draftKind" />
          </div>
        </template>

        <div v-else class="tm-inspector-empty">
          <div class="tm-state-glyph is-quiet">
            <Icon :icon="fileTextIcon" width="30" height="30" />
          </div>
          <h2>选一个模板，或新建一个</h2>
          <p>左侧选中模板后在这里编辑正文与分类；插入按钮会把内容送到主编辑器光标处。</p>
          <dl class="tm-facts">
            <div><dt>模板</dt><dd>{{ formatCount(summary.total) }}</dd></div>
            <div><dt>分类</dt><dd>{{ formatCount(summary.categories) }}</dd></div>
            <div><dt>内置</dt><dd>{{ formatCount(summary.builtin) }}</dd></div>
            <div><dt>自定义</dt><dd>{{ formatCount(summary.user) }}</dd></div>
          </dl>
          <button v-if="canMutate" type="button" class="tm-btn is-quiet" @click="openDraft">
            <Icon :icon="plusIcon" width="15" height="15" />
            新建模板
          </button>
        </div>
      </aside>
    </main>

    <!-- 分类的补全候选：文档字段与导入弹窗共用一份，放在根节点下即可被两处 list= 引用。 -->
    <datalist id="tm-type-options">
      <option v-for="type in typeSuggestions" :key="type" :value="type" />
    </datalist>

    <!-- ───────────── 拖放导入 ───────────── -->
    <div v-if="dragActive" class="tm-drop">
      <div class="tm-drop-card">
        <Icon :icon="fileUpIcon" width="26" height="26" />
        <div class="tm-drop-title">释放以导入 Markdown 模板</div>
        <div class="tm-drop-hint">每份文件会成为一份独立模板，同名时自动加序号。</div>
      </div>
    </div>

    <n-dropdown
      trigger="manual"
      placement="bottom-start"
      :show="menu.show"
      :x="menu.x"
      :y="menu.y"
      :options="menuOptions"
      @select="onMenuSelect"
      @clickoutside="menu.show = false"
    />

    <!-- ───────────── 导入确认 ───────────── -->
    <n-modal
      v-model:show="importModal.show"
      preset="card"
      class="tm-modal"
      title="导入模板"
      :bordered="false"
      @after-enter="focusImportInput"
    >
      <div class="tm-modal-body">
        <p class="tm-modal-hint">
          共 {{ importModal.items.length }} 份 Markdown，导入到
          <strong>{{ importModal.type || '未分类' }}</strong>
        </p>
        <ul class="tm-import-list">
          <li v-for="item in importModal.items.slice(0, 6)" :key="item.source">
            <Icon :icon="fileTextIcon" width="13" height="13" />
            <span class="tm-import-origin">{{ item.origin }}</span>
            <span class="tm-import-arrow">→</span>
            <span class="tm-import-name">{{ item.name }}</span>
          </li>
          <li v-if="importModal.items.length > 6" class="tm-import-more">
            还有 {{ importModal.items.length - 6 }} 份…
          </li>
        </ul>
        <label class="tm-field">
          <span class="tm-field-label">分类</span>
          <input
            ref="importTypeRef"
            v-model="importModal.type"
            class="tm-text-input"
            placeholder="例如 工作 / 会议"
            list="tm-type-options"
            @keydown.enter.prevent="confirmImport"
            @keydown.esc.prevent="importModal.show = false"
          />
        </label>
      </div>
      <template #footer>
        <div class="tm-modal-actions">
          <button type="button" class="tm-btn" @click="importModal.show = false">取消</button>
          <button type="button" class="tm-btn is-primary" :disabled="busy" @click="confirmImport">
            导入
          </button>
        </div>
      </template>
    </n-modal>

    <!-- ───────────── 删除确认 ───────────── -->
    <n-modal v-model:show="deleteModal.show" preset="card" class="tm-modal" title="删除模板" :bordered="false">
      <div class="tm-modal-body">
        <p class="tm-modal-danger">确定删除「{{ deleteModal.name }}」吗？</p>
        <p class="tm-modal-hint">
          {{ deleteModal.overridden
            ? '这是覆盖了内置模板的版本。删除后会恢复成内置模板的原样。'
            : '模板文件会从模板库目录里移除，应用内无法撤销。' }}
        </p>
      </div>
      <template #footer>
        <div class="tm-modal-actions">
          <button type="button" class="tm-btn" @click="deleteModal.show = false">取消</button>
          <button type="button" class="tm-btn is-danger" :disabled="busy" @click="confirmDelete">
            {{ deleteModal.overridden ? '移除自定义版本' : '删除' }}
          </button>
        </div>
      </template>
    </n-modal>

    <!-- ───────────── 离开确认 ───────────── -->
    <n-modal v-model:show="leaveModal.show" preset="card" class="tm-modal" title="有未保存的改动" :bordered="false">
      <div class="tm-modal-body">
        <p class="tm-modal-danger">「{{ leaveModal.name }}」有未保存的改动。</p>
        <p class="tm-modal-hint">保存后再继续，还是丢弃这些改动？</p>
      </div>
      <template #footer>
        <div class="tm-modal-actions">
          <button type="button" class="tm-btn" @click="resolveLeave('discard')">丢弃改动</button>
          <button type="button" class="tm-btn" @click="leaveModal.show = false">取消</button>
          <button type="button" class="tm-btn is-primary" :disabled="busy" @click="resolveLeave('save')">
            保存并继续
          </button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Icon } from '@iconify/vue'
import { NModal, NDropdown, useMessage } from 'naive-ui'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import * as monaco from 'monaco-editor'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'

import alertIcon from '@iconify-icons/lucide/alert-triangle'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import codeIcon from '@iconify-icons/lucide/code-2'
import eyeIcon from '@iconify-icons/lucide/eye'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import fileUpIcon from '@iconify-icons/lucide/file-up'
import folderSearchIcon from '@iconify-icons/lucide/folder-search'
import gridIcon from '@iconify-icons/lucide/layout-grid'
import libraryIcon from '@iconify-icons/lucide/library'
import listTreeIcon from '@iconify-icons/lucide/list-tree'
import panelLeftCloseIcon from '@iconify-icons/lucide/panel-left-close'
import panelLeftOpenIcon from '@iconify-icons/lucide/panel-left-open'
import panelCloseIcon from '@iconify-icons/lucide/panel-right-close'
import panelOpenIcon from '@iconify-icons/lucide/panel-right-open'
import pencilIcon from '@iconify-icons/lucide/pencil'
import plusIcon from '@iconify-icons/lucide/plus'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import saveIcon from '@iconify-icons/lucide/save'
import searchIcon from '@iconify-icons/lucide/search'
import sendIcon from '@iconify-icons/lucide/send'
import textIcon from '@iconify-icons/lucide/text-cursor-input'
import xIcon from '@iconify-icons/lucide/x'

import GuaxiAuxEditorStatusBar from './GuaxiAuxEditorStatusBar.vue'
import { createLogger } from '../../utils/logger'
import { attachSharedEditorFeatures, resolveSharedEditorDisplayOptions, startSharedLsp } from '../../monaco/editorFeatures'
import { GUAXI_MONACO_SEMANTIC_TOKEN_COLORS, GUAXI_MONACO_THEME_RULES } from '../../monaco/guaxiMonacoThemes'
import { DEFAULT_TEMPLATES, TEMPLATE_TYPE_ORDER } from './templateManager/builtinTemplates'
import {
  excerptOf,
  formatChars,
  formatCount,
  groupTemplatesByFamily,
  mergeTemplates,
  searchTemplates,
  summarizeFamilies,
  summarizeTemplates,
  templateFamilyOf,
  templateKindOf,
  templateVariantOf,
  textStatsOf,
  validateTemplateName,
  normalizeTemplateName,
  normalizeTemplateType,
} from './templateManager/templateModel'

const logger = createLogger('GuaxiTemplateManager')
const message = useMessage()

const PREFS_KEY = 'guaxi:templateManager:prefs:v1'
const COLLAPSED_KEY = 'guaxi:templateManager:collapsed:v1'
const INSPECTOR_MIN = 380
const INSPECTOR_MAX = 860
const MAX_TEMPLATES = 4000

const builtinIndex = new Map(DEFAULT_TEMPLATES.map((item) => [item.name, item]))

// ───────────────────────── 状态 ─────────────────────────

const pageRef = ref(null)
const listRef = ref(null)
const searchRef = ref(null)
const editorContainer = ref(null)
const previewRef = ref(null)
const importTypeRef = ref(null)

const loading = ref(false)
const busy = ref(false)
const loadError = ref('')
const truncated = ref(false)
const repoPath = ref('')

const templates = ref([])
// 已读过的自定义模板正文，按名称缓存：搜索正文与卡片摘要都要用，不需要先点开。
const contentCache = ref(new Map())
const contentLoaded = ref(new Set())

const selectedName = ref('')
const search = ref('')
const searchContent = ref(true)
const familyFilter = ref('all')
const collapsedGroups = ref(new Set())

const viewMode = ref('list')
const docMode = ref('edit')

const inspectorWidth = ref(560)
const inspectorViewWidth = ref(0)
const inspectorCollapsed = ref(false)
const browserCollapsed = ref(false)
const resizing = ref(false)
let resizeState = null

const dragActive = ref(false)

// 草稿就是文档面板的真相源：内置模板被选中时也是一份「未保存的副本」，
// 第一次保存才落盘成自定义模板（后端 save_template 只写文件）。
const draft = reactive({
  name: '',
  type: '未分类',
  content: '',
  builtin: false,
  isUser: false,
  overriddenBuiltin: false,
  originName: '',
})

// 打开时的快照。不能拿「库里同名那份」当基线：新草稿刚填了分类就改名成某个已存在的名字时，
// 会被误判成「没有改动」，保存按钮直接是灰的。
const baseline = reactive({ name: '', type: '未分类', content: '' })

const editorInstance = shallowRef(null)
const previewHtml = ref('')
const previewLoading = ref(false)
const previewError = ref('')

const menu = reactive({ show: false, x: 0, y: 0, item: null })
const importModal = reactive({ show: false, items: [], type: '' })
const deleteModal = reactive({ show: false, name: '', overridden: false })
const leaveModal = reactive({ show: false, name: '', resolve: null })

const viewModes = [
  { key: 'list', title: '分组列表', icon: listTreeIcon },
  { key: 'grid', title: '卡片', icon: gridIcon },
]

// ───────────────────────── 编辑器 ─────────────────────────

let editor = null
let model = null
let featuresDisposable = null
let themeObserver = null
let suppressChange = false
let currentLanguage = ''
let currentUri = ''

const defineEditorThemes = () => {
  // 模板窗口是独立 webview，不一定会先加载主编辑器；这里注册一份与主编辑器同色板的主题兜底。
  monaco.editor.defineTheme('guaxi-template-light', {
    base: 'vs',
    inherit: true,
    rules: GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2937',
      'editorLineNumber.foreground': '#8a94a6',
      'editorCursor.foreground': '#0f766e',
      'editor.selectionBackground': '#99f6e455',
      'editor.lineHighlightBackground': '#f6f8fa',
      'editor.lineHighlightBorder': '#00000000',
    },
  })
  monaco.editor.defineTheme('guaxi-template-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: GUAXI_MONACO_THEME_RULES.dark,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark,
    colors: {
      'editor.background': '#1f2329',
      'editor.foreground': '#d7dde8',
      'editorLineNumber.foreground': '#6b7280',
      'editorCursor.foreground': '#61afef',
      'editor.selectionBackground': '#264f78',
      'editor.lineHighlightBackground': '#2a2f38',
      'editor.lineHighlightBorder': '#00000000',
    },
  })
}

const themeName = () =>
  document.documentElement?.getAttribute('data-guaxi-theme') === 'dark'
    ? 'guaxi-template-dark'
    : 'guaxi-template-light'

// LSP 只认真实后缀：pyright 只分析 .py，markdown 服务只分析 .md。
const virtualUriOf = (language) => {
  const ext = language === 'python' ? 'py' : 'md'
  const slug = encodeURIComponent(selectedName.value || draft.name || 'draft')
  return monaco.Uri.parse(`file:///guaxi-template-manager/${slug}-${Date.now()}.${ext}`)
}

const createModel = (language, content) => {
  const uri = virtualUriOf(language)
  const next = monaco.editor.createModel(String(content || ''), language, uri)
  next.updateOptions({ tabSize: language === 'python' ? 4 : 2, insertSpaces: true })
  next.onDidChangeContent(() => {
    if (suppressChange) return
    draft.content = next.getValue()
  })
  currentUri = uri.toString()
  return next
}

const applyLanguage = (reason = 'manual') => {
  if (!editor || !model) return
  const next = templateKindOf({ name: draft.name, type: draft.type })
  if (next === currentLanguage) return
  const content = model.getValue()
  const previous = model
  model = createModel(next, content)
  editor.setModel(model)
  previous?.dispose?.()
  currentLanguage = next
  void startSharedLsp(editor, next, currentUri, {
    requestSource: 'template-manager',
    documentUri: currentUri,
  })
  logger.info('[TemplateManager] language switched', { to: next, reason })
}

const setEditorValue = (value) => {
  if (!model) return
  const next = String(value || '')
  if (model.getValue() === next) return
  suppressChange = true
  model.setValue(next)
  suppressChange = false
}

const mountEditor = () => {
  if (editor || !editorContainer.value) return
  defineEditorThemes()
  const display = resolveSharedEditorDisplayOptions()
  currentLanguage = templateKindOf({ name: draft.name, type: draft.type })
  model = createModel(currentLanguage, draft.content)
  editor = monaco.editor.create(editorContainer.value, {
    model,
    automaticLayout: true,
    contextmenu: false,
    minimap: display.minimap,
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    lineNumbers: 'on',
    renderWhitespace: 'selection',
    fontFamily: display.fontFamily,
    fontSize: display.fontSize,
    lineHeight: display.lineHeight,
    padding: { top: 12, bottom: 24 },
    theme: themeName(),
  })
  featuresDisposable = attachSharedEditorFeatures({
    monaco,
    editor,
    language: currentLanguage,
    filePath: currentUri,
    enableLsp: true,
    lspDocumentUri: currentUri,
    lspRequestSource: 'template-manager',
    ensureThemes: defineEditorThemes,
    resolveThemeName: (dark) => (dark ? 'guaxi-template-dark' : 'guaxi-template-light'),
  })
  editorInstance.value = editor

  themeObserver = new MutationObserver(() => {
    try {
      monaco.editor.setTheme(themeName())
    } catch (error) {
      logger.debug('[TemplateManager] theme reapply failed', { error })
    }
    // 预览的代码块配色是渲染期注入的，主题切换后要重跑一次（有则重跑）。
    if (docMode.value === 'preview') void renderPreview()
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-guaxi-theme'] })
}

const unmountEditor = () => {
  featuresDisposable?.()
  featuresDisposable = null
  themeObserver?.disconnect?.()
  themeObserver = null
  editorInstance.value = null
  editor?.dispose?.()
  editor = null
  model?.dispose?.()
  model = null
  currentUri = ''
  currentLanguage = ''
}

// ───────────────────────── 派生数据 ─────────────────────────

const canMutate = computed(() => Boolean(repoPath.value))

const merged = computed(() => mergeTemplates(DEFAULT_TEMPLATES, templates.value, contentCache.value))

const summary = computed(() => summarizeTemplates(merged.value))

const familyChips = computed(() => summarizeFamilies(merged.value))

const searchActive = computed(() => search.value.trim().length > 0)

const visibleTemplates = computed(() => {
  let list = merged.value
  if (familyFilter.value !== 'all') {
    list = list.filter((item) => templateFamilyOf(item.type) === familyFilter.value)
  }
  return list
})

const searchResults = computed(() => {
  if (!searchActive.value) return []
  const query = search.value.trim()
  const list = visibleTemplates.value
  if (!searchContent.value) {
    return searchTemplates(list.map((item) => ({ ...item, content: '' })), query)
  }
  return searchTemplates(list, query)
})

const groups = computed(() => {
  if (searchActive.value) {
    // 搜索结果自带片段，不再需要分类分组；用一个虚拟组包住以便统一渲染。
    return searchResults.value.length ? [{ family: '搜索结果', items: [] }] : []
  }
  return groupTemplatesByFamily(visibleTemplates.value, TEMPLATE_TYPE_ORDER)
})

const flatTemplates = computed(() => visibleTemplates.value.slice())

const visibleCount = computed(() => (searchActive.value ? searchResults.value.length : visibleTemplates.value.length))

const emptyTitle = computed(() => {
  if (searchActive.value) return '没有匹配的模板'
  if (familyFilter.value !== 'all') return '这个分类下还没有模板'
  return '模板库还是空的'
})

const emptyHint = computed(() => {
  if (searchActive.value) return '换个关键词，或只在标题里找。正文命中需要打开「含正文」。'
  if (!canMutate.value) return '模板库目录不可用，暂时只能使用内置模板。'
  return '新建一份，或者把 Markdown 文件拖进来。'
})

const stats = computed(() => textStatsOf(draft.content))

const draftKind = computed(() => templateKindOf({ name: draft.name, type: draft.type }))

const draftItemName = computed(() => draft.originName || draft.name)

// draft 是 reactive 对象，恒为真；用它判断「有没有打开文档」会永远成立。
// 新建的空草稿也必须有名字，所以另用一个显式开关。
const hasDraft = ref(false)

const setBaseline = (item) => {
  baseline.name = item ? item.name : ''
  baseline.type = item ? normalizeTemplateType(item.type) : '未分类'
  baseline.content = item ? String(item.content || '') : ''
}

const dirty = computed(() => {
  // 空草稿（既没名字也没正文）不算改动，否则刚点「新建」就弹离开确认。
  if (!draft.name && !draft.content.trim()) return false
  return (
    String(draft.name || '') !== baseline.name ||
    normalizeTemplateType(draft.type) !== baseline.type ||
    String(draft.content || '') !== baseline.content
  )
})

const dirtyName = computed(() => (dirty.value ? draft.originName || draft.name : ''))

const typeSuggestions = computed(() => {
  const set = new Set(TEMPLATE_TYPE_ORDER)
  for (const item of merged.value) {
    const type = normalizeTemplateType(item.type)
    set.add(type)
    const family = templateFamilyOf(type)
    if (family) set.add(family)
  }
  return [...set]
})

const emptyStageTitle = computed(() => (draft.builtin ? '内置模板可以直接改造' : '从一段正文开始'))
const emptyStageHint = computed(() =>
  draft.builtin
    ? '改动后保存会生成一份同名的自定义模板，内置那份作为退路一直留着。'
    : '写操作步骤、检查清单或代码骨架，插入时会原样送到编辑器光标处。'
)

const repoTitle = computed(() =>
  repoPath.value ? `${repoPath.value}\n点击在文件管理器中打开` : '模板库目录不可用，点击查看设置提示'
)

const mainStyle = computed(() => ({
  '--tm-inspector-width': `${inspectorViewWidth.value || inspectorWidth.value}px`,
}))

const isDirty = (name) => {
  if (!name) return false
  return Boolean(dirty.value && (draft.originName || draft.name) === name)
}

const menuOptions = computed(() => {
  const item = menu.item
  if (!item) return []
  const options = [
    { label: '插入到编辑器', key: 'insert' },
    { label: '复制正文', key: 'copy-content' },
    { type: 'divider', key: 'd1' },
    { label: '编辑', key: 'edit' },
    { label: '复制为新模板', key: 'duplicate' },
  ]
  if (item.overriddenBuiltin) {
    options.push({ type: 'divider', key: 'd2' })
    options.push({ label: '恢复内置版本', key: 'restore' })
  }
  options.push({ type: 'divider', key: 'd3' })
  options.push({ label: item.builtin ? '删除（内置不可删）' : '删除', key: 'delete', disabled: item.builtin })
  return options
})

// ───────────────────────── 偏好 ─────────────────────────

const clampInspectorWidth = (width) => {
  const viewport = window.innerWidth || 0
  const available = viewport > 0 ? Math.max(INSPECTOR_MIN, viewport - 420) : INSPECTOR_MAX
  return Math.max(INSPECTOR_MIN, Math.min(INSPECTOR_MAX, available, Math.round(Number(width) || inspectorWidth.value)))
}

const loadPrefs = () => {
  try {
    const raw = localStorage.getItem(PREFS_KEY)
    if (raw) {
      const value = JSON.parse(raw)
      if (['list', 'grid'].includes(value?.viewMode)) viewMode.value = value.viewMode
      if (['edit', 'preview'].includes(value?.docMode)) docMode.value = value.docMode
      if (typeof value?.searchContent === 'boolean') searchContent.value = value.searchContent
      if (Number(value?.inspectorWidth) > 0) inspectorWidth.value = Math.round(Number(value.inspectorWidth))
      if (typeof value?.inspectorCollapsed === 'boolean') inspectorCollapsed.value = value.inspectorCollapsed
      if (typeof value?.browserCollapsed === 'boolean') browserCollapsed.value = value.browserCollapsed
      if (typeof value?.selectedName === 'string') selectedName.value = value.selectedName
      if (typeof value?.familyFilter === 'string') familyFilter.value = value.familyFilter
    }
  } catch (error) {
    logger.debug('[TemplateManager] read prefs failed', { error })
  }
  // 两侧都收起会只剩空白，恢复上次状态时至少留下文档区。
  if (browserCollapsed.value && inspectorCollapsed.value) inspectorCollapsed.value = false
  try {
    const raw = localStorage.getItem(COLLAPSED_KEY)
    const list = raw ? JSON.parse(raw) : []
    collapsedGroups.value = new Set(Array.isArray(list) ? list.filter(Boolean) : [])
  } catch {
    collapsedGroups.value = new Set()
  }
  inspectorViewWidth.value = clampInspectorWidth(inspectorWidth.value)
}

let savePrefsTimer = null
const savePrefs = () => {
  clearTimeout(savePrefsTimer)
  savePrefsTimer = setTimeout(() => {
    try {
      localStorage.setItem(
        PREFS_KEY,
        JSON.stringify({
          viewMode: viewMode.value,
          docMode: docMode.value,
          searchContent: searchContent.value,
          inspectorWidth: inspectorWidth.value,
          inspectorCollapsed: inspectorCollapsed.value,
          browserCollapsed: browserCollapsed.value,
          selectedName: selectedName.value || '',
          familyFilter: familyFilter.value,
        })
      )
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...collapsedGroups.value]))
    } catch {}
  }, 400)
}

// ───────────────────────── 加载 ─────────────────────────

const errorText = (error) => {
  const raw = error?.message ?? (typeof error?.toString === 'function' ? error.toString() : String(error))
  return String(raw || '').trim() || '操作失败'
}

/**
 * 只把界面需要的名称/分类拉回来；正文按需读。
 *
 * 搜索正文与卡片摘要都依赖正文，所以加载后会在空闲时把自定义模板正文补齐，
 * 但不阻塞首屏——先让用户看到列表。
 */
const refresh = async ({ reason = 'manual', preserveDraft = true } = {}) => {
  loading.value = true
  loadError.value = ''
  try {
    const [path, list] = await Promise.all([
      invoke('get_template_repo_path').catch((error) => {
        logger.warn('[TemplateManager] read repo path failed', { error })
        return ''
      }),
      invoke('list_templates'),
    ])
    repoPath.value = typeof path === 'string' ? path : ''
    const items = Array.isArray(list) ? list : []
    truncated.value = items.length > MAX_TEMPLATES
    templates.value = items.slice(0, MAX_TEMPLATES)

    // 缓存里的正文仍有效，只补新出现、还没读过的。
    await hydrateContents()

    if (preserveDraft && draft.name && dirty.value) {
      // 刷新不该踩掉用户正在写的东西；只是把它的「原始版本」指针尽量保持有效。
      logger.info('[TemplateManager] refresh kept dirty draft', { name: draft.name })
    } else if (preserveDraft && draft.originName) {
      const stillThere = merged.value.find((item) => item.name === draft.originName)
      if (stillThere) {
        loadIntoDraft(stillThere)
      }
    } else if (!draft.name && selectedName.value) {
      const target = merged.value.find((item) => item.name === selectedName.value)
      if (target) loadIntoDraft(target)
    }
    if (selectedName.value && !merged.value.some((item) => item.name === selectedName.value)) {
      selectedName.value = ''
    }
    logger.info('[TemplateManager] templates loaded', {
      reason,
      user: templates.value.length,
      builtin: DEFAULT_TEMPLATES.length,
    })
  } catch (error) {
    const detail = errorText(error)
    loadError.value = detail
    repoPath.value = ''
    logger.error('[TemplateManager] load templates failed', error)
  } finally {
    loading.value = false
  }
}

const hydrateContents = async () => {
  const nextCache = new Map(contentCache.value)
  const nextLoaded = new Set(contentLoaded.value)
  const pending = templates.value
    .map((item) => String(item?.name || '').trim())
    .filter((name) => name && !nextLoaded.has(name))

  // 并发读，但限个宽度：几十份模板一起 invoke 会把窗口的 IPC 队列堵住。
  const queue = pending.slice()
  const workerCount = Math.min(6, queue.length)
  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (queue.length) {
        const name = queue.shift()
        try {
          const data = await invoke('read_template', { name })
          nextCache.set(name, String(data?.content ?? ''))
          nextLoaded.add(name)
        } catch (error) {
          // 读失败就当作空内容，不阻断其它模板；名字仍在列表里，点开时会再试一次。
          logger.warn('[TemplateManager] read template content failed', { name, error: errorText(error) })
        }
      }
    })
  )
  // 已被删掉的模板不要一直占着缓存。
  const alive = new Set(templates.value.map((item) => String(item?.name || '').trim()))
  for (const name of [...nextCache.keys()]) {
    if (!alive.has(name)) nextCache.delete(name)
  }
  for (const name of [...nextLoaded]) {
    if (!alive.has(name)) nextLoaded.delete(name)
  }
  contentCache.value = nextCache
  contentLoaded.value = nextLoaded
}

// ───────────────────────── 选中与草稿 ─────────────────────────

const loadIntoDraft = (item) => {
  draft.originName = item.name
  draft.name = item.name
  draft.type = normalizeTemplateType(item.type)
  draft.builtin = Boolean(item.builtin)
  draft.isUser = !item.builtin
  draft.overriddenBuiltin = Boolean(item.overriddenBuiltin)
  draft.content = String(item.content || '')
  setBaseline(item)
  selectedName.value = item.name
  hasDraft.value = true
  setEditorValue(draft.content)
  applyLanguage('load')
  savePrefs()
}

/** 切走之前先处理未保存改动：返回 false 表示用户取消了这次切换。 */
const guardDirty = async () => {
  if (!dirty.value) return true
  return new Promise((resolve) => {
    leaveModal.name = draft.originName || draft.name || '当前模板'
    leaveModal.resolve = resolve
    leaveModal.show = true
  })
}

const resolveLeave = async (action) => {
  const resolve = leaveModal.resolve
  leaveModal.resolve = null
  leaveModal.show = false
  if (!resolve) return
  if (action === 'save') {
    const ok = await saveDraft()
    resolve(Boolean(ok))
    return
  }
  // 丢弃：把草稿恢复成打开时的快照（新草稿则是清空）。
  draft.name = baseline.name
  draft.type = baseline.type || '未分类'
  draft.content = baseline.content
  setEditorValue(draft.content)
  resolve(true)
}

const selectTemplate = async (name) => {
  const target = normalizeTemplateName(name)
  if (!target || target === draftItemName.value) {
    selectedName.value = target
    return
  }
  const ok = await guardDirty()
  if (!ok) return

  let item = merged.value.find((entry) => entry.name === target)
  if (!item) return

  // 还没读过正文的自定义/被覆盖模板：点开时补读一次。
  if (!item.builtin && !item.loaded) {
    try {
      const data = await invoke('read_template', { name: target })
      const nextCache = new Map(contentCache.value)
      nextCache.set(target, String(data?.content ?? ''))
      contentCache.value = nextCache
      const nextLoaded = new Set(contentLoaded.value)
      nextLoaded.add(target)
      contentLoaded.value = nextLoaded
      item = merged.value.find((entry) => entry.name === target) || item
    } catch (error) {
      message.error(`读取模板失败：${errorText(error)}`)
      return
    }
  }

  loadIntoDraft(item)
  nextTick(() => scrollRowIntoView(target))
  logger.info('[TemplateManager] template selected', { name: target, builtin: item.builtin })
}

const openDraft = async () => {
  const ok = await guardDirty()
  if (!ok) return
  draft.originName = ''
  draft.name = ''
  draft.type = familyFilter.value !== 'all' ? familyFilter.value : '未分类'
  draft.content = ''
  draft.builtin = false
  draft.isUser = true
  draft.overriddenBuiltin = false
  setBaseline(null)
  selectedName.value = ''
  hasDraft.value = true
  setEditorValue('')
  applyLanguage('new')
  docMode.value = 'edit'
  nextTick(() => {
    editor?.focus?.()
    editor?.setPosition?.({ lineNumber: 1, column: 1 })
  })
  logger.info('[TemplateManager] draft opened')
}

const closeDraft = async () => {
  const ok = await guardDirty()
  if (!ok) return
  draft.originName = ''
  draft.name = ''
  draft.type = '未分类'
  draft.content = ''
  draft.builtin = false
  draft.isUser = false
  draft.overriddenBuiltin = false
  setBaseline(null)
  selectedName.value = ''
  hasDraft.value = false
  setEditorValue('')
  savePrefs()
}

const markDirty = () => {
  applyLanguage('field-change')
  savePrefs()
}

// ───────────────────────── 保存 / 删除 ─────────────────────────

const saveDraft = async () => {
  if (busy.value) return false
  const checked = validateTemplateName(draft.name)
  if (!checked.ok) {
    message.warning(checked.reason)
    return false
  }
  const name = checked.value
  const type = normalizeTemplateType(draft.type)

  // 改成另一个已存在的名字会覆盖它，得先问一声。
  if (name !== draft.originName) {
    const clash = merged.value.find((item) => item.name === name)
    if (clash && !(clash.builtin && !clash.loaded)) {
      message.warning(`「${name}」已存在，换个名称或先删除它`)
      return false
    }
  }

  busy.value = true
  try {
    // 重命名：后端只搬文件与 meta，然后把正文写到新名字下。
    if (draft.originName && draft.originName !== name) {
      await invoke('rename_template', { oldName: draft.originName, newName: name })
      const nextCache = new Map(contentCache.value)
      nextCache.delete(draft.originName)
      contentCache.value = nextCache
    }
    await invoke('save_template', { name, templateType: type, content: draft.content || '' })
    const nextCache = new Map(contentCache.value)
    nextCache.set(name, String(draft.content || ''))
    contentCache.value = nextCache
    const nextLoaded = new Set(contentLoaded.value)
    nextLoaded.add(name)
    contentLoaded.value = nextLoaded

    draft.originName = name
    draft.name = name
    draft.type = type
    draft.builtin = false
    draft.isUser = true
    draft.overriddenBuiltin = builtinIndex.has(name)
    setBaseline({ name, type, content: draft.content || '' })
    selectedName.value = name
    await refresh({ reason: 'after-save' })
    applyLanguage('saved')
    message.success(builtinIndex.has(name) ? '已保存，内置版本仍保留' : '已保存')
    logger.info('[TemplateManager] template saved', { name, type, length: draft.content.length })
    return true
  } catch (error) {
    message.error(`保存失败：${errorText(error)}`)
    logger.error('[TemplateManager] save failed', error)
    return false
  } finally {
    busy.value = false
  }
}

const requestDelete = (item) => {
  if (!item || item.builtin) {
    if (item?.builtin) message.info('内置模板不可删除；改动会存成同名的自定义模板')
    return
  }
  deleteModal.name = item.name
  deleteModal.overridden = Boolean(item.overriddenBuiltin)
  deleteModal.show = true
}

const confirmDelete = async () => {
  const name = deleteModal.name
  if (!name || busy.value) return
  busy.value = true
  try {
    await invoke('delete_template', { name })
    deleteModal.show = false
    const nextCache = new Map(contentCache.value)
    nextCache.delete(name)
    contentCache.value = nextCache
    const nextLoaded = new Set(contentLoaded.value)
    nextLoaded.delete(name)
    contentLoaded.value = nextLoaded
    if (draft.originName === name) {
      // 删掉的就是当前打开的这份：退回内置原样（同名内置存在时），否则清空文档。
      const builtin = builtinIndex.get(name)
      draft.originName = ''
      draft.name = name
      draft.type = builtin ? normalizeTemplateType(builtin.templateType) : draft.type
      draft.builtin = Boolean(builtin)
      draft.isUser = false
      draft.overriddenBuiltin = false
      draft.content = builtin ? String(builtin.content || '') : ''
      setBaseline(builtin ? {
        name,
        type: normalizeTemplateType(builtin.templateType),
        content: String(builtin.content || ''),
      } : null)
      setEditorValue(draft.content)
    }
    if (selectedName.value === name) {
      selectedName.value = builtinIndex.has(name) ? name : ''
    }
    await refresh({ reason: 'after-delete' })
    message.success('已删除')
    logger.info('[TemplateManager] template deleted', { name })
  } catch (error) {
    message.error(`删除失败：${errorText(error)}`)
    logger.error('[TemplateManager] delete failed', error)
  } finally {
    busy.value = false
  }
}

// ───────────────────────── 插入 / 复制 ─────────────────────────

const insertTemplate = async (item) => {
  if (!item || busy.value) return
  let content = String(item.content || '')
  // 卡片/搜索结果里可能只有缓存正文；没有就现读。
  if (!content && !item.builtin) {
    try {
      const data = await invoke('read_template', { name: item.name })
      content = String(data?.content || '')
    } catch (error) {
      message.error(`读取模板失败：${errorText(error)}`)
      return
    }
  }
  if (!content.trim()) {
    message.warning('模板正文是空的，没有可插入的内容')
    return
  }
  try {
    await invoke('emit_template_insert', { content })
    message.success(`已插入「${item.name}」`)
    logger.info('[TemplateManager] template inserted', { name: item.name, length: content.length })
  } catch (error) {
    message.error(`插入失败：${errorText(error)}`)
    logger.error('[TemplateManager] insert failed', error)
  }
}

const insertDraft = () => {
  if (!draft.content.trim()) {
    message.warning('当前正文是空的')
    return
  }
  insertTemplate({ name: draft.name || '未命名模板', content: draft.content, builtin: draft.builtin })
}

const writeClipboard = async (text) => {
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
    return true
  } catch (error) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch (fallbackError) {
      logger.warn('[TemplateManager] clipboard write failed', { error: fallbackError, first: error })
      return false
    }
  }
}

const copyText = async (text, label) => {
  const ok = await writeClipboard(text)
  if (ok) message.success(`已复制${label ? `：${label}` : ''}`)
  else message.error('复制失败')
}

const duplicateTemplate = async (item) => {
  if (!item) return
  const base = item.name
  let candidate = `${base} 副本`
  let i = 2
  const names = new Set(merged.value.map((entry) => entry.name))
  while (names.has(candidate)) {
    candidate = `${base} 副本 ${i}`
    i += 1
  }
  const source = merged.value.find((entry) => entry.name === base) || item
  draft.originName = ''
  draft.name = candidate
  draft.type = normalizeTemplateType(source.type)
  draft.content = String(source.content || '')
  draft.builtin = false
  draft.isUser = false
  draft.overriddenBuiltin = false
  setBaseline(null)
  selectedName.value = candidate
  hasDraft.value = true
  setEditorValue(draft.content)
  applyLanguage('duplicate')
  docMode.value = 'edit'
  message.info('已复制到文档，保存后会生成新模板')
}

/** 把自定义模板退回内置原样：删掉文件，草稿换成内置正文。 */
const restoreBuiltin = async (item) => {
  if (!item?.overriddenBuiltin || busy.value) return
  busy.value = true
  try {
    await invoke('delete_template', { name: item.name })
    const nextCache = new Map(contentCache.value)
    nextCache.delete(item.name)
    contentCache.value = nextCache
    await refresh({ reason: 'after-restore' })
    const restored = merged.value.find((entry) => entry.name === item.name)
    if (restored) loadIntoDraft(restored)
    message.success('已恢复内置版本')
    logger.info('[TemplateManager] restored builtin', { name: item.name })
  } catch (error) {
    message.error(`恢复失败：${errorText(error)}`)
    logger.error('[TemplateManager] restore failed', error)
  } finally {
    busy.value = false
  }
}

// ───────────────────────── 导入 ─────────────────────────

const baseNameOf = (path) =>
  String(path || '')
    .replace(/\\/g, '/')
    .split('/')
    .pop()
    .replace(/\.(md|markdown)$/i, '')
    .trim()

const buildImportEntries = (paths) => {
  const existing = new Set(merged.value.map((item) => item.name))
  return paths.map((source) => {
    const origin = baseNameOf(source) || '未命名模板'
    let name = origin
    let i = 2
    while (existing.has(name)) {
      name = `${origin} (${i})`
      i += 1
    }
    existing.add(name)
    return { source, origin, name }
  })
}

const importFromFile = async () => {
  if (!canMutate.value) {
    message.warning('模板库目录不可用，无法导入')
    return
  }
  try {
    const picked = await openDialog({
      multiple: true,
      directory: false,
      title: '导入 Markdown 模板',
      filters: [{ name: 'Markdown', extensions: ['md', 'markdown'] }],
    })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    await stageImport(list)
  } catch (error) {
    message.error(`导入失败：${errorText(error)}`)
    logger.error('[TemplateManager] import dialog failed', error)
  }
}

const stageImport = async (paths) => {
  const items = buildImportEntries(paths.filter(Boolean))
  if (!items.length) return
  importModal.items = items
  importModal.type = familyFilter.value !== 'all' ? familyFilter.value : '未分类'
  importModal.show = true
  await nextTick()
  importTypeRef.value?.focus?.()
  logger.info('[TemplateManager] import staged', { count: items.length })
}

const focusImportInput = () => importTypeRef.value?.focus?.()

const confirmImport = async () => {
  if (busy.value || !importModal.items.length) return
  const type = normalizeTemplateType(importModal.type)
  busy.value = true
  let imported = 0
  try {
    for (const item of importModal.items) {
      await invoke('import_template_from_file', {
        sourcePath: item.source,
        name: item.name,
        templateType: type,
      })
      imported += 1
    }
    importModal.show = false
    await refresh({ reason: 'after-import' })
    const lastName = importModal.items[importModal.items.length - 1]?.name
    if (lastName) await selectTemplate(lastName)
    message.success(imported === 1 ? '已导入 1 份模板' : `已导入 ${imported} 份模板`)
    logger.info('[TemplateManager] import finished', { imported, type })
  } catch (error) {
    message.error(`导入失败：${errorText(error)}`)
    logger.error('[TemplateManager] import failed', error)
    await refresh({ reason: 'import-recover' }).catch(() => {})
  } finally {
    busy.value = false
  }
}

// ───────────────────────── 拖放 ─────────────────────────

let unlistenDrop = null
const setupDragDrop = async () => {
  try {
    unlistenDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
      const payload = event?.payload || {}
      if (payload.type === 'enter' || payload.type === 'over') {
        dragActive.value = true
        return
      }
      if (payload.type === 'leave') {
        dragActive.value = false
        return
      }
      if (payload.type === 'drop') {
        dragActive.value = false
        const md = (payload.paths || []).filter((path) => /\.(md|markdown)$/i.test(String(path)))
        if (!md.length) {
          message.warning('只支持 .md / .markdown 文件')
          return
        }
        await stageImport(md)
      }
    })
  } catch (error) {
    logger.warn('[TemplateManager] drag drop listener failed', { error })
  }
}

// ───────────────────────── 右键菜单 ─────────────────────────

const openMenu = (item, event) => {
  if (!item) return
  menu.item = item
  menu.x = event?.clientX || 0
  menu.y = event?.clientY || 0
  menu.show = true
}

const closeMenu = (event) => {
  if (!menu.show) return
  const target = event?.target
  if (target instanceof Element && target.closest('.n-dropdown-menu, .n-dropdown')) return
  menu.show = false
}

const onMenuSelect = async (key) => {
  const item = menu.item
  menu.show = false
  if (!key || !item) return
  switch (key) {
    case 'insert':
      await insertTemplate(item)
      break
    case 'copy-content':
      await copyText(String(item.content || ''), '模板正文')
      break
    case 'edit':
      await selectTemplate(item.name)
      break
    case 'duplicate':
      await duplicateTemplate(item)
      break
    case 'restore':
      await restoreBuiltin(item)
      break
    case 'delete':
      requestDelete(item)
      break
    default:
      break
  }
}

// ───────────────────────── 预览 ─────────────────────────

let previewToken = 0
const renderPreview = async () => {
  if (docMode.value !== 'preview') return
  const token = ++previewToken
  previewLoading.value = true
  previewError.value = ''
  try {
    const { renderMarkdown } = await import('../GuaxiMarkdownRender/GuaxiMarker')
    const html = await renderMarkdown(draft.content || '', {
      buttons: false,
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: document.documentElement?.getAttribute('data-guaxi-theme') === 'dark' ? 'dark' : 'light',
      basePath: '',
    })
    if (token !== previewToken) return
    previewHtml.value = typeof html === 'string' ? html : ''
  } catch (error) {
    if (token !== previewToken) return
    previewHtml.value = ''
    previewError.value = errorText(error)
    logger.warn('[TemplateManager] preview render failed', { error: errorText(error) })
  } finally {
    if (token === previewToken) previewLoading.value = false
  }
}

const setDocMode = (mode) => {
  docMode.value = mode === 'preview' ? 'preview' : 'edit'
  savePrefs()
  if (docMode.value === 'preview') void renderPreview()
  else nextTick(() => editor?.layout?.())
}

// ───────────────────────── 视图交互 ─────────────────────────

const setViewMode = (mode) => {
  if (!['list', 'grid'].includes(mode)) return
  viewMode.value = mode
  savePrefs()
}

const setFamilyFilter = (family) => {
  familyFilter.value = family
  savePrefs()
}

const toggleSearchContent = () => {
  searchContent.value = !searchContent.value
  savePrefs()
}

const toggleGroup = (family) => {
  const next = new Set(collapsedGroups.value)
  if (next.has(family)) next.delete(family)
  else next.add(family)
  collapsedGroups.value = next
  savePrefs()
}

const toggleInspector = () => {
  inspectorCollapsed.value = !inspectorCollapsed.value
  // 文档区收起后若列表也收起就只剩空白，把列表放回来。
  if (inspectorCollapsed.value) browserCollapsed.value = false
  savePrefs()
  if (!inspectorCollapsed.value) nextTick(() => editor?.layout?.())
}

const toggleBrowser = () => {
  browserCollapsed.value = !browserCollapsed.value
  // 反过来同理：列表收起时文档区必须是展开的，才能独占整屏。
  if (browserCollapsed.value) {
    inspectorCollapsed.value = false
    nextTick(() => {
      mountEditor()
      editor?.layout?.()
    })
  } else {
    nextTick(() => editor?.layout?.())
  }
  savePrefs()
}

const scrollRowIntoView = async (name) => {
  await nextTick()
  const container = listRef.value
  if (!container) return
  const escaped = String(name).replace(/["\\]/g, '\\$&')
  const el = container.querySelector(`[data-name="${escaped}"]`)
  el?.scrollIntoView?.({ block: 'nearest' })
}

const revealRepo = async () => {
  if (!repoPath.value) {
    message.info('模板库目录不可用，请到「设置 → 模板」里指定一个目录')
    return
  }
  try {
    await invoke('open_in_file_explorer', { path: repoPath.value })
  } catch (error) {
    message.error(errorText(error))
  }
}

const startResize = (event) => {
  if (!event) return
  event.preventDefault?.()
  resizing.value = true
  resizeState = { startX: Number(event.clientX || 0), startWidth: inspectorWidth.value }
  try {
    document.body.classList.add('template-inspector-resizing')
  } catch {}
  window.addEventListener('pointermove', onResizeMove)
  window.addEventListener('pointerup', stopResize)
  window.addEventListener('pointercancel', stopResize)
}

const onResizeMove = (event) => {
  if (!resizeState) return
  const delta = resizeState.startX - Number(event?.clientX || 0)
  inspectorWidth.value = clampInspectorWidth(resizeState.startWidth + delta)
  inspectorViewWidth.value = inspectorWidth.value
}

const stopResize = () => {
  if (!resizing.value) return
  resizing.value = false
  resizeState = null
  window.removeEventListener('pointermove', onResizeMove)
  window.removeEventListener('pointerup', stopResize)
  window.removeEventListener('pointercancel', stopResize)
  try {
    document.body.classList.remove('template-inspector-resizing')
  } catch {}
  editor?.layout?.()
  savePrefs()
}

const onWindowResize = () => {
  if (resizing.value) return
  inspectorViewWidth.value = clampInspectorWidth(inspectorWidth.value)
  editor?.layout?.()
}

const isTypingTarget = (target) => {
  const tag = String(target?.tagName || '').toLowerCase()
  return tag === 'input' || tag === 'textarea' || target?.isContentEditable
}

// Monaco 内部是 contenteditable，方向键与 Ctrl+S 得让它自己处理；
// 这里只认「焦点不在编辑器/输入框」时的快捷键。
const isEditorFocused = () => {
  const active = document.activeElement
  return Boolean(active && (active.closest?.('.tm-monaco') || active.classList?.contains('monaco-editor')))
}

const onKeydown = (event) => {
  if (importModal.show || deleteModal.show || leaveModal.show) return
  const mod = event.ctrlKey || event.metaKey
  const key = String(event.key).toLowerCase()
  if (mod && key === 's') {
    event.preventDefault()
    if (draft.name) void saveDraft()
    return
  }
  // 两侧面板的开关在编辑器里也要生效，所以放在 typing 判断之前。
  if (mod && event.altKey && key === 'p') {
    event.preventDefault()
    toggleInspector()
    return
  }
  if (mod && !event.altKey && !event.shiftKey && key === 'b') {
    event.preventDefault()
    toggleBrowser()
    return
  }

  const typing = isTypingTarget(event.target) || isEditorFocused()

  if (event.key === '/' && !typing) {
    event.preventDefault()
    searchRef.value?.focus?.()
    return
  }
  if (event.key === 'Escape') {
    if (menu.show) {
      menu.show = false
      return
    }
    if (search.value) {
      search.value = ''
      return
    }
    if (event.target === searchRef.value) event.target.blur?.()
    return
  }
  if (typing) return

  if (event.key === 'F5') {
    event.preventDefault()
    void refresh({ reason: 'keyboard' })
    return
  }
  if (event.key === 'Enter') {
    const item = merged.value.find((entry) => entry.name === selectedName.value)
    if (item) {
      event.preventDefault()
      void insertTemplate(item)
    }
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    void moveSelection(event.key === 'ArrowDown' ? 1 : -1)
    return
  }
  if (event.key === 'Delete' && selectedName.value) {
    const item = merged.value.find((entry) => entry.name === selectedName.value)
    if (item) {
      event.preventDefault()
      requestDelete(item)
    }
  }
}

const moveSelection = async (delta) => {
  const list = searchActive.value
    ? searchResults.value.map((result) => result.item)
    : visibleTemplates.value
  if (!list.length) return
  const index = list.findIndex((item) => item.name === selectedName.value)
  const nextIndex = index < 0 ? (delta > 0 ? 0 : list.length - 1) : Math.max(0, Math.min(list.length - 1, index + delta))
  const target = list[nextIndex]
  if (!target) return
  // 上下键就是「换一份看」，和单击一样走完整流程（含未保存改动确认）。
  await selectTemplate(target.name)
  scrollRowIntoView(target.name)
}

// ───────────────────────── 生命周期 ─────────────────────────

const onBeforeUnload = (event) => {
  if (!dirty.value) return
  event.preventDefault()
  event.returnValue = ''
}

watch(docMode, (mode) => {
  if (mode === 'preview') void renderPreview()
})

watch(
  () => [draft.name, draft.type],
  () => {
    if (!editor) return
    // 名称里出现 Python Cell 时切到 Python，LSP 才会接管。
    applyLanguage('identity-change')
  }
)

onMounted(async () => {
  loadPrefs()
  window.addEventListener('resize', onWindowResize)
  window.addEventListener('pointerdown', closeMenu, true)
  window.addEventListener('blur', closeMenu)
  window.addEventListener('beforeunload', onBeforeUnload)
  await refresh({ reason: 'mounted', preserveDraft: false })
  await setupDragDrop()
  // 恢复上次选中：直接打开那份模板，免得每次进来都是空面板。
  if (selectedName.value) {
    const target = merged.value.find((item) => item.name === selectedName.value)
    if (target) await selectTemplate(target.name)
  } else if (familyFilter.value !== 'all' && !familyChips.value.some((c) => c.family === familyFilter.value)) {
    familyFilter.value = 'all'
  }
  if (!inspectorCollapsed.value) {
    await nextTick()
    mountEditor()
  }
  nextTick(() => pageRef.value?.focus?.())
})

onBeforeUnmount(() => {
  stopResize()
  clearTimeout(savePrefsTimer)
  savePrefs()
  window.removeEventListener('resize', onWindowResize)
  window.removeEventListener('pointerdown', closeMenu, true)
  window.removeEventListener('blur', closeMenu)
  window.removeEventListener('beforeunload', onBeforeUnload)
  if (unlistenDrop) {
    try {
      unlistenDrop()
    } catch {}
    unlistenDrop = null
  }
  unmountEditor()
})

// 收起预览栏时也会卸载编辑器；重新展开要把它接回来。
watch(inspectorCollapsed, async (collapsed) => {
  if (collapsed) {
    unmountEditor()
    return
  }
  await nextTick()
  mountEditor()
})

// 草稿第一次出现时编辑器可能还没建好（比如收起状态挂载后直接选中模板）。
watch(hasDraft, async (open) => {
  if (!open || inspectorCollapsed.value) return
  await nextTick()
  if (!editor) mountEditor()
  if (editor) setEditorValue(draft.content)
})
</script>

<style scoped>
/* ─────────────────────────────────────────────────────────────
   模板管理器

   视觉取向延续资源库那张「灯箱」：右侧文档区是主角，左边列表安静、
   只有分组标题与极轻的类型色。区别在于模板不是文件而是「可复用的文字」，
   所以列表第二行显示的是分类或正文命中片段，而不是大小和修改时间。

   编辑器字号比主编辑器大一档（模板大多是短文本，读得多、改得少），
   但字体跟随设置，避免同一份文字在两个窗口里长得不一样。
   ───────────────────────────────────────────────────────────── */
.tm {
  --tm-fg: var(--guaxi-text-primary, #1f2328);
  --tm-fg-2: var(--guaxi-text-secondary, #57606a);
  --tm-fg-3: color-mix(in srgb, var(--guaxi-text-secondary, #57606a) 72%, transparent);
  --tm-bg: var(--guaxi-app-bg, #ffffff);
  --tm-panel: var(--guaxi-panel-bg, #ffffff);
  --tm-line: color-mix(in srgb, var(--guaxi-border, #d0d7de) 82%, transparent);
  --tm-line-soft: color-mix(in srgb, var(--guaxi-border, #d0d7de) 48%, transparent);
  --tm-hover: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 5%, transparent);
  --tm-sel: color-mix(in srgb, var(--n-primary-color, #4078f2) 13%, transparent);
  --tm-sel-strong: color-mix(in srgb, var(--n-primary-color, #4078f2) 20%, transparent);
  --tm-accent: var(--n-primary-color, #4078f2);
  --tm-danger: var(--n-error-color, #e45649);
  --tm-radius: 9px;
  --tm-tone-markdown: #4078f2;
  --tm-tone-python: #a4761c;

  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--tm-bg);
  color: var(--tm-fg);
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 13px;
  line-height: 1.45;
  outline: none;
}

:root[data-guaxi-theme='dark'] .tm {
  --tm-fg-3: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 66%, transparent);
  --tm-hover: color-mix(in srgb, #ffffff 6%, transparent);
  --tm-tone-markdown: #61afef;
  --tm-tone-python: #e5c07b;
}

.tm * {
  box-sizing: border-box;
}

.tm :focus-visible {
  outline: 2px solid var(--tm-accent);
  outline-offset: -1px;
  border-radius: 6px;
}

/* ───────────── 顶栏 ───────────── */

.tm-header {
  flex: 0 0 auto;
  height: 50px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 12px 0 14px;
  border-bottom: 1px solid var(--tm-line);
  background: var(--tm-panel);
}

.tm-brand {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 9px;
}

.tm-brand-mark {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: var(--tm-accent);
  background: var(--tm-sel);
}

.tm-title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.tm-sync-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding: 0 9px;
  border: 1px solid var(--tm-line);
  border-radius: 999px;
  background: transparent;
  color: var(--tm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.tm-sync-pill:hover {
  background: var(--tm-hover);
  color: var(--tm-fg);
}

.tm-sync-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--tm-fg-3);
}

.tm-sync-dot[data-state='success'] {
  background: var(--n-success-color, #50a14f);
}

.tm-sync-dot[data-state='running'] {
  background: var(--n-info-color, #56b6c2);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--n-info-color, #56b6c2) 22%, transparent);
}

.tm-search {
  position: relative;
  flex: 1 1 auto;
  min-width: 180px;
  max-width: 620px;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--tm-line);
  border-radius: var(--tm-radius);
  background: color-mix(in srgb, var(--tm-panel) 82%, var(--tm-bg));
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.tm-search:focus-within {
  border-color: var(--tm-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--tm-accent) 16%, transparent);
}

.tm-search-icon {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
}

.tm-search-input {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--tm-fg);
  font: inherit;
}

.tm-search-input::placeholder {
  color: var(--tm-fg-3);
}

.tm-search-input::-webkit-search-cancel-button {
  display: none;
}

.tm-kbd {
  flex: 0 0 auto;
  padding: 1px 5px;
  border: 1px solid var(--tm-line);
  border-radius: 5px;
  color: var(--tm-fg-3);
  font-size: 10px;
  line-height: 1.4;
}

.tm-search-clear {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--tm-fg-3);
  cursor: pointer;
}

.tm-search-clear:hover {
  background: var(--tm-hover);
  color: var(--tm-fg);
}

.tm-header-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

/* ───────────── 通用按钮 ───────────── */

.tm-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 11px;
  border: 1px solid var(--tm-line);
  border-radius: 8px;
  background: var(--tm-panel);
  color: var(--tm-fg);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}

.tm-btn:hover:not(:disabled) {
  background: var(--tm-hover);
}

.tm-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.tm-btn.is-primary {
  border-color: transparent;
  background: var(--tm-accent);
  color: #fff;
}

.tm-btn.is-primary:hover:not(:disabled) {
  filter: brightness(1.06);
}

.tm-btn.is-primary:disabled {
  background: color-mix(in srgb, var(--tm-accent) 34%, var(--tm-panel));
  color: color-mix(in srgb, #ffffff 78%, var(--tm-accent));
  opacity: 1;
}

.tm-btn.is-danger {
  border-color: transparent;
  background: var(--tm-danger);
  color: #fff;
}

.tm-btn.is-quiet {
  border-style: dashed;
  background: transparent;
  color: var(--tm-fg-2);
}

.tm-icon-btn {
  display: inline-grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--tm-fg-2);
  cursor: pointer;
}

.tm-icon-btn:hover:not(:disabled) {
  background: var(--tm-hover);
  color: var(--tm-fg);
}

.tm-icon-btn.is-small {
  width: 26px;
  height: 26px;
}

/* 左右面板开关固定贴在顶栏两端，不跟着右侧按钮组一起被挤走。 */
.tm-icon-btn.is-pane-toggle {
  flex: 0 0 auto;
}

.tm-icon-btn.is-busy {
  color: var(--tm-accent);
}

.tm-icon-btn.is-busy :deep(svg) {
  animation: tm-spin 0.9s linear infinite;
}

/* ───────────── 工具栏 ───────────── */

.tm-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 38px;
  padding: 5px 12px 5px 14px;
  border-bottom: 1px solid var(--tm-line-soft);
  background: var(--tm-panel);
}

.tm-filters {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  scrollbar-width: none;
}

.tm-filters::-webkit-scrollbar {
  display: none;
}

.tm-chip {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 220px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--tm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
  white-space: nowrap;
}

.tm-chip:hover {
  background: var(--tm-hover);
}

.tm-chip.is-active {
  border-color: var(--tm-line);
  background: var(--tm-sel);
  color: var(--tm-fg);
}

.tm-chip-count {
  color: var(--tm-fg-3);
  font-variant-numeric: tabular-nums;
}

/* 文档字段搬到工具栏：不占正文高度，收起列表时还能贴着文档用。 */
.tm-inline-fields {
  flex: 0 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 12px;
  border-left: 1px solid var(--tm-line-soft);
}

.tm-inline-field {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  min-width: 0;
  padding: 0 8px;
  border: 1px solid var(--tm-line);
  border-radius: 7px;
  background: var(--tm-bg);
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.tm-inline-field:focus-within {
  border-color: var(--tm-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--tm-accent) 16%, transparent);
}

.tm-inline-field.is-name {
  flex: 1 1 200px;
  max-width: 360px;
}

.tm-inline-field.is-type {
  flex: 0 1 180px;
  min-width: 112px;
}

.tm-inline-label {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
  font-size: 10.5px;
  white-space: nowrap;
}

.tm-inline-input {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--tm-fg);
  font: inherit;
  font-size: 12px;
}

.tm-inline-input::placeholder {
  color: var(--tm-fg-3);
}

.tm-view-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tm-scope-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 9px;
  border: 1px solid var(--tm-line);
  border-radius: 999px;
  background: transparent;
  color: var(--tm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.tm-scope-pill:hover {
  background: var(--tm-hover);
}

.tm-scope-pill.is-on {
  border-color: color-mix(in srgb, var(--tm-accent) 40%, var(--tm-line));
  background: var(--tm-sel);
  color: var(--tm-accent);
}

.tm-segmented {
  display: inline-flex;
  padding: 2px;
  border: 1px solid var(--tm-line);
  border-radius: 8px;
  background: var(--tm-bg);
}

.tm-seg {
  display: grid;
  place-items: center;
  width: 26px;
  height: 22px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--tm-fg-3);
  cursor: pointer;
}

.tm-seg:hover {
  color: var(--tm-fg);
}

.tm-seg.is-active {
  background: var(--tm-panel);
  color: var(--tm-fg);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
}

/* ───────────── 主体 ───────────── */

.tm-main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 7px var(--tm-inspector-width, 560px);
  overflow: hidden;
}

.tm.is-inspector-collapsed .tm-main {
  grid-template-columns: minmax(0, 1fr);
}

.tm.is-browser-collapsed .tm-main {
  grid-template-columns: minmax(0, 1fr);
}

.tm-browser {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tm-banner {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 14px;
  font-size: 11.5px;
  border-bottom: 1px solid var(--tm-line-soft);
}

.tm-banner.is-warning {
  color: var(--n-warning-color, #c18401);
  background: color-mix(in srgb, var(--n-warning-color, #c18401) 9%, transparent);
}

.tm-banner.is-danger {
  color: var(--tm-danger);
  background: color-mix(in srgb, var(--tm-danger) 9%, transparent);
}

.tm-banner-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-state {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 32px 24px;
  text-align: center;
}

.tm-state h2 {
  margin: 4px 0 0;
  font-size: 14px;
  font-weight: 700;
}

.tm-state p {
  margin: 0;
  max-width: 380px;
  color: var(--tm-fg-2);
  font-size: 12px;
  line-height: 1.6;
}

.tm-state-glyph {
  display: grid;
  place-items: center;
  width: 58px;
  height: 58px;
  border-radius: 18px;
  color: var(--tm-fg-3);
  background: var(--tm-hover);
}

.tm-state-glyph.is-quiet {
  opacity: 0.75;
}

.tm-spinner {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--tm-line);
  border-top-color: var(--tm-accent);
  animation: tm-spin 0.8s linear infinite;
}

.tm-spinner.is-small {
  width: 13px;
  height: 13px;
  border-width: 1.5px;
}

@keyframes tm-spin {
  to {
    transform: rotate(360deg);
  }
}

/* ───────────── 列表 ───────────── */

.tm-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px 6px 8px;
  scrollbar-width: thin;
}

.tm-group {
  padding-top: 4px;
}

.tm-group-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 26px;
  padding: 0 10px 0 6px;
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--tm-bg);
}

.tm-group-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 0;
  border: 0;
  background: none;
  color: var(--tm-fg-2);
  font: inherit;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.04em;
  cursor: pointer;
}

.tm-group-toggle:hover {
  color: var(--tm-fg);
}

.tm-chevron {
  flex: 0 0 auto;
  transition: transform 0.14s ease;
}

.tm-chevron.is-open {
  transform: rotate(90deg);
}

.tm-group-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-group-count {
  color: var(--tm-fg-3);
  font-variant-numeric: tabular-nums;
}

.tm-group-hint {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
  font-size: 10px;
}

.tm-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  min-height: 32px;
  padding: 4px 8px 4px 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--tm-fg);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: default;
  user-select: none;
  position: relative;
}

.tm-row:hover {
  background: var(--tm-hover);
}

.tm-row.is-selected {
  background: var(--tm-sel);
}

.tm-row.is-primary {
  background: var(--tm-sel-strong);
}

.tm-row.is-primary::before {
  content: '';
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 2px;
  border-radius: 2px;
  background: var(--tm-accent);
}

.tm-row-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.tm-row-text {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.tm-row-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-row-variant {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

.tm-row-sub {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--tm-fg-3);
  font-size: 11px;
}

.tm-row-snippet {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
  flex: 1;
  color: var(--tm-fg-2);
  font-size: 11px;
}

.tm-snippet-line {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
  font-variant-numeric: tabular-nums;
}

.tm-snippet-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-snippet-text mark {
  padding: 0 1px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--n-warning-color, #c18401) 30%, transparent);
  color: inherit;
}

.tm-row.is-result .tm-row-text {
  flex-direction: column;
  align-items: stretch;
  gap: 1px;
}

.tm-row.is-result {
  min-height: 40px;
  align-items: flex-start;
  padding-top: 5px;
  padding-bottom: 5px;
}

.tm-row.is-result .tm-row-main {
  align-items: flex-start;
}

.tm-row.is-result .tm-type-icon {
  margin-top: 1px;
}

.tm-row.is-result .tm-row-title,
.tm-row.is-result .tm-row-snippet {
  display: flex;
}

.tm-row-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.tm-type-icon {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 16px;
  height: 16px;
  color: var(--tm-fg-2);
}

.tm-type-icon[data-kind='markdown'] {
  color: var(--tm-tone-markdown);
}

.tm-type-icon[data-kind='python'] {
  color: var(--tm-tone-python);
}

.tm-badge {
  flex: 0 0 auto;
  padding: 0 6px;
  border: 1px solid color-mix(in srgb, var(--tm-accent) 36%, var(--tm-line));
  border-radius: 999px;
  color: var(--tm-accent);
  font-size: 10px;
  line-height: 16px;
}

.tm-badge.is-quiet {
  border-color: var(--tm-line);
  color: var(--tm-fg-3);
}

.tm-badge.is-user {
  border-color: color-mix(in srgb, var(--n-warning-color, #c18401) 40%, var(--tm-line));
  color: var(--n-warning-color, #c18401);
}

.tm-dirty-dot {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--n-warning-color, #c18401);
}

.tm-row-insert {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--tm-fg-3);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.12s ease;
}

.tm-row:hover .tm-row-insert,
.tm-row.is-primary .tm-row-insert,
.tm-row-insert:focus-visible {
  opacity: 1;
}

.tm-row-insert:hover {
  background: var(--tm-sel-strong);
  color: var(--tm-accent);
}

/* 卡片视图：与列表共用同一套排版层级，只是把每项铺成一张方块。 */
.tm-list.is-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
  gap: 10px;
  align-content: start;
  padding: 10px;
}

.tm-tile {
  display: flex;
  flex-direction: column;
  gap: 7px;
  min-height: 128px;
  padding: 11px 12px;
  border: 1px solid var(--tm-line-soft);
  border-radius: 10px;
  background: var(--tm-panel);
  color: var(--tm-fg);
  font: inherit;
  text-align: left;
  cursor: default;
}

.tm-tile:hover {
  border-color: var(--tm-line);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.09);
}

.tm-tile.is-primary {
  border-color: var(--tm-accent);
  box-shadow: 0 0 0 1px var(--tm-accent) inset;
}

.tm-tile-head {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.tm-tile-family {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

.tm-tile-name {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  word-break: break-all;
}

.tm-tile-excerpt {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  color: var(--tm-fg-2);
  font-size: 11px;
  line-height: 1.55;
}

.tm-tile-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

.tm-tile-variant {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-tile-len {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
}

/* ───────────── 状态栏 ───────────── */

.tm-status {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 14px;
  border-top: 1px solid var(--tm-line-soft);
  color: var(--tm-fg-3);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.tm-status-sep {
  opacity: 0.5;
}

.tm-status-spacer {
  flex: 1;
}

.tm-status-path {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 60%;
}

.tm-status-dirty {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--n-warning-color, #c18401);
}

.tm-link-btn {
  border: 0;
  background: none;
  padding: 0;
  color: var(--tm-accent);
  font: inherit;
  font-size: 10.5px;
  cursor: pointer;
}

/* ───────────── 右侧文档 ───────────── */

.tm-resizer {
  position: relative;
  width: 7px;
  cursor: col-resize;
  background: linear-gradient(
    to right,
    transparent 0,
    transparent 3px,
    var(--tm-line) 3px,
    var(--tm-line) 4px,
    transparent 4px
  );
}

.tm-resizer:hover {
  background: linear-gradient(
    to right,
    transparent 1px,
    var(--tm-accent) 1px,
    var(--tm-accent) 3px,
    transparent 3px
  );
}

.tm-inspector {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-left: 1px solid var(--tm-line-soft);
  background: color-mix(in srgb, var(--tm-panel) 62%, var(--tm-bg));
}

.tm.is-browser-collapsed .tm-inspector {
  border-left: 0;
}

.tm-doc-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 40px;
  padding: 0 10px 0 14px;
  border-bottom: 1px solid var(--tm-line-soft);
}

.tm-doc-title {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.tm-doc-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  font-weight: 600;
}

.tm-doc-head-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tm-field {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.tm-field-label {
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

.tm-text-input {
  width: 100%;
  height: 28px;
  padding: 0 9px;
  border: 1px solid var(--tm-line);
  border-radius: 7px;
  background: var(--tm-bg);
  color: var(--tm-fg);
  font: inherit;
  font-size: 12px;
  outline: none;
}

.tm-text-input:focus {
  border-color: var(--tm-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--tm-accent) 16%, transparent);
}

.tm-doc-preview-toggle {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 4px;
  height: 30px;
  padding: 0 12px;
  border-bottom: 1px solid var(--tm-line-soft);
}

.tm-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 9px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--tm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.tm-tab:hover {
  background: var(--tm-hover);
}

.tm-tab.is-active {
  border-color: var(--tm-line);
  background: var(--tm-panel);
  color: var(--tm-fg);
}

.tm-doc-stats {
  margin-left: auto;
  color: var(--tm-fg-3);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
}

.tm-doc-stage {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.tm-monaco {
  position: absolute;
  inset: 0;
  background: var(--tm-bg);
}

.tm-monaco.is-hidden {
  visibility: hidden;
}

.tm-monaco :deep(.monaco-editor),
.tm-monaco :deep(.monaco-editor-background),
.tm-monaco :deep(.monaco-editor .margin) {
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace) !important;
}

/* 空正文时盖一层提示，但不挡住编辑：它只是把「这里可以写什么」说清楚。 */
.tm-empty-overlay {
  position: absolute;
  left: 64px;
  right: 24px;
  top: 18px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.tm-empty-overlay.is-visible {
  opacity: 1;
}

.tm-empty-title {
  margin: 0 0 3px;
  color: var(--tm-fg-2);
  font-size: 12.5px;
}

.tm-empty-hint {
  margin: 0;
  max-width: 460px;
  color: var(--tm-fg-3);
  font-size: 11px;
  line-height: 1.6;
}

.tm-preview {
  position: absolute;
  inset: 0;
  overflow: auto;
  padding: 6px 18px 24px;
  scrollbar-width: thin;
}

.tm-preview-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 18px 4px;
  color: var(--tm-fg-3);
  font-size: 11.5px;
}

.tm-preview-fallback {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 18px 4px;
}

.tm-preview-body {
  max-width: 860px;
}

.tm-doc-foot {
  flex: 0 0 auto;
  border-top: 1px solid var(--tm-line-soft);
}

.tm-doc-foot :deep(.aux-status-bar) {
  border-top: 0;
  background: transparent;
}

.tm-inspector-empty {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 9px;
  padding: 24px 22px;
  text-align: center;
}

.tm-inspector-empty h2 {
  margin: 2px 0 0;
  font-size: 13px;
  font-weight: 700;
}

.tm-inspector-empty p {
  margin: 0;
  max-width: 320px;
  color: var(--tm-fg-2);
  font-size: 11.5px;
  line-height: 1.65;
}

.tm-facts {
  width: 100%;
  max-width: 320px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1px;
  margin: 8px 0 2px;
  border: 1px solid var(--tm-line-soft);
  border-radius: 9px;
  overflow: hidden;
}

.tm-facts > div {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 10px;
  background: var(--tm-panel);
}

.tm-facts dt {
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

.tm-facts dd {
  margin: 0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* ───────────── 拖放 / 弹窗 ───────────── */

.tm.is-dragging::after {
  content: '';
  position: fixed;
  inset: 0;
  z-index: 40;
  pointer-events: none;
  border: 2px dashed var(--tm-accent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--tm-accent) 7%, transparent);
}

.tm-drop {
  position: fixed;
  inset: 0;
  z-index: 41;
  display: grid;
  place-items: center;
  pointer-events: none;
}

.tm-drop-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 22px 30px;
  border-radius: 14px;
  color: var(--tm-fg);
  background: var(--tm-panel);
  border: 1px solid var(--tm-line);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.22);
}

.tm-drop-title {
  font-size: 13px;
  font-weight: 700;
}

.tm-drop-hint {
  color: var(--tm-fg-2);
  font-size: 11px;
}

.tm-modal {
  width: min(460px, calc(100vw - 40px));
}

.tm-modal-body {
  display: grid;
  gap: 10px;
}

.tm-modal-hint {
  margin: 0;
  color: var(--tm-fg-2);
  font-size: 12px;
  word-break: break-word;
}

.tm-modal-danger {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.tm-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.tm-import-list {
  margin: 0;
  padding: 7px 9px;
  max-height: 190px;
  overflow: auto;
  list-style: none;
  border: 1px solid var(--tm-line-soft);
  border-radius: 8px;
  background: color-mix(in srgb, var(--tm-panel) 70%, var(--tm-bg));
}

.tm-import-list li {
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 24px;
  color: var(--tm-fg-2);
  font-size: 11.5px;
}

.tm-import-origin {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-import-arrow {
  flex: 0 0 auto;
  color: var(--tm-fg-3);
}

.tm-import-name {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--tm-fg);
}

.tm-import-more {
  color: var(--tm-fg-3);
  font-size: 10.5px;
}

body.template-inspector-resizing {
  cursor: col-resize;
  user-select: none;
}

/* ───────────── 窄窗口 ───────────── */

@media (max-width: 1080px) {
  .tm-toolbar {
    flex-wrap: wrap;
    gap: 6px 12px;
    padding-bottom: 6px;
  }

  .tm-view-actions {
    margin-left: auto;
  }

  .tm-header-actions .tm-btn span {
    display: none;
  }

  .tm-header-actions .tm-btn {
    width: 28px;
    padding: 0;
    justify-content: center;
  }
}

@media (max-width: 820px) {
  .tm-main {
    grid-template-columns: minmax(0, 1fr);
  }

  .tm-resizer,
  .tm-inspector {
    display: none;
  }

  .tm-inline-fields {
    display: none;
  }

  .tm-search {
    min-width: 120px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .tm-chevron,
  .tm-search,
  .tm-row-insert,
  .tm-empty-overlay {
    transition: none;
  }

  .tm-spinner,
  .tm-icon-btn.is-busy :deep(svg) {
    animation-duration: 2.4s;
  }
}
</style>
