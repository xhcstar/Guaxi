<template>
  <div
    ref="pageRef"
    class="rrm"
    :class="{ 'is-dropping': dropActive, 'is-inspector-collapsed': inspectorCollapsed }"
    tabindex="-1"
    @keydown="onKeydown"
  >
    <header class="rrm-header">
      <div class="rrm-brand">
        <span class="rrm-brand-mark" aria-hidden="true">
          <Icon :icon="folderTreeIcon" width="17" height="17" />
        </span>
        <h1 class="rrm-title">资源库</h1>
        <button
          type="button"
          class="rrm-sync-pill"
          :data-state="syncUi.state"
          :title="syncUi.title"
          @click="showSync = !showSync"
        >
          <span class="rrm-sync-dot" />
          {{ syncUi.tagText }}
        </button>
      </div>

      <div class="rrm-search">
        <Icon :icon="searchIcon" width="15" height="15" class="rrm-search-icon" />
        <input
          ref="searchRef"
          v-model="search"
          type="search"
          class="rrm-search-input"
          placeholder="搜索文件名或路径"
          spellcheck="false"
          @keydown.esc="search = ''"
        />
        <kbd v-if="!search" class="rrm-kbd">/</kbd>
        <button v-else type="button" class="rrm-search-clear" title="清除搜索" @click="search = ''">
          <Icon :icon="xIcon" width="13" height="13" />
        </button>
      </div>

      <div class="rrm-header-actions">
        <n-dropdown
          trigger="click"
          placement="bottom-start"
          :options="createOptions"
          :disabled="!canMutate"
          @select="openCreate"
        >
          <button
            type="button"
            class="rrm-btn"
            :title="canMutate ? '新建文件夹或文件' : '尚未配置资源库目录'"
            :disabled="!canMutate"
          >
            <Icon :icon="folderPlusIcon" width="15" height="15" />
            <span>新建</span>
            <Icon :icon="chevronDownIcon" width="13" height="13" class="rrm-btn-caret" />
          </button>
        </n-dropdown>
        <button
          type="button"
          class="rrm-btn"
          :title="canMutate ? '从本机导入文件到当前目录' : '尚未配置资源库目录'"
          :disabled="!canMutate"
          @click="importFiles"
        >
          <Icon :icon="uploadIcon" width="15" height="15" />
          <span>导入</span>
        </button>
        <button
          type="button"
          class="rrm-icon-btn"
          :class="{ 'is-busy': scanning }"
          title="重新扫描资源库（F5）"
          aria-label="重新扫描资源库"
          @click="rescanAll({ reason: 'manual' })"
        >
          <Icon :icon="refreshIcon" width="16" height="16" />
        </button>
        <button type="button" class="rrm-icon-btn" title="打开资源库目录" aria-label="打开资源库目录" @click="openRootInExplorer">
          <Icon :icon="folderOpenIcon" width="16" height="16" />
        </button>
        <button type="button" class="rrm-icon-btn" title="测试同步连接" aria-label="测试同步连接" :disabled="busy" @click="testConnection">
          <Icon :icon="plugIcon" width="16" height="16" />
        </button>
        <button type="button" class="rrm-btn is-primary" title="立即同步资源库" :disabled="busy" @click="syncNow">
          <Icon :icon="syncIcon" width="15" height="15" />
          <span>同步</span>
        </button>
        <button
          type="button"
          class="rrm-icon-btn"
          :class="{ 'is-on': !inspectorCollapsed }"
          :title="inspectorCollapsed ? '显示预览面板' : '收起预览面板'"
          :aria-label="inspectorCollapsed ? '显示预览面板' : '收起预览面板'"
          @click="toggleInspector"
        >
          <Icon :icon="inspectorCollapsed ? panelOpenIcon : panelCloseIcon" width="16" height="16" />
        </button>
      </div>
    </header>

    <div class="rrm-toolbar">
      <div class="rrm-crumbs">
        <button
          type="button"
          class="rrm-icon-btn is-small"
          :disabled="!activeDir"
          title="上一级（←）"
          @click="goUp"
        >
          <Icon :icon="arrowUpIcon" width="14" height="14" />
        </button>
        <button
          type="button"
          class="rrm-crumb is-root"
          :class="{ 'is-current': !activeDir }"
          title="资源库根目录"
          @click="navigateTo('')"
        >
          <Icon :icon="folderIcon" width="14" height="14" />
          资源库
        </button>
        <template v-for="crumb in crumbs" :key="crumb.path">
          <span class="rrm-crumb-sep">/</span>
          <button
            type="button"
            class="rrm-crumb"
            :class="{ 'is-current': crumb.path === activeDir }"
            @click="navigateTo(crumb.path)"
          >
            {{ crumb.name }}
          </button>
        </template>
        <span v-if="searchActive" class="rrm-search-scope">全库搜索</span>
      </div>

      <div class="rrm-filters">
        <button
          v-for="chip in typeChips"
          :key="chip.key"
          type="button"
          class="rrm-chip"
          :class="{ 'is-active': typeFilter === chip.key }"
          :data-type="chip.key"
          @click="setTypeFilter(chip.key)"
        >
          <span class="rrm-chip-dot" />
          {{ chip.label }}
          <span class="rrm-chip-count">{{ chip.count }}</span>
        </button>
      </div>

      <div class="rrm-view-actions">
        <button
          type="button"
          class="rrm-icon-btn is-small"
          :class="{ 'is-on': includeHidden }"
          :title="includeHidden ? '隐藏以 . 开头的目录（.guaxi-p2p 等）' : '显示以 . 开头的目录'"
          @click="toggleHidden"
        >
          <Icon :icon="includeHidden ? eyeIcon : eyeOffIcon" width="15" height="15" />
        </button>
        <button
          type="button"
          class="rrm-icon-btn is-small"
          :title="sortLabel"
          @click="cycleSort"
        >
          <Icon :icon="sortIcon" width="15" height="15" />
          <span class="rrm-sort-text">{{ sortLabel }}</span>
        </button>
        <div class="rrm-segmented" role="group" aria-label="视图">
          <button
            v-for="mode in viewModes"
            :key="mode.key"
            type="button"
            class="rrm-seg"
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

    <main class="rrm-main" :style="gridStyle">
      <section class="rrm-browser">
        <div v-if="scanError" class="rrm-banner is-danger">
          <Icon :icon="warningIcon" width="15" height="15" />
          <span class="rrm-banner-text">{{ scanError }}</span>
          <button type="button" class="rrm-link-btn" @click="rescanAll({ reason: 'retry' })">重试</button>
        </div>
        <div v-if="truncated" class="rrm-banner is-warning">
          <Icon :icon="alertIcon" width="15" height="15" />
          条目超过 {{ formatCount(scanMeta.limit) }} 个，只载入了前一部分。请在搜索框里缩小范围。
        </div>

        <div v-if="!root" class="rrm-state">
          <div class="rrm-state-glyph"><Icon :icon="folderSearchIcon" width="34" height="34" /></div>
          <h2>还没有配置资源库目录</h2>
          <p>到「设置 → 资源库」里指定一个目录。之后在编辑器插入的图片、图表数据都会落到这里。</p>
        </div>

        <div v-else-if="scanning && !rows.length" class="rrm-state">
          <span class="rrm-spinner" />
          <p>正在扫描资源库…</p>
        </div>

        <div v-else-if="!rows.length" class="rrm-state">
          <div class="rrm-state-glyph">
            <Icon :icon="searchActive ? searchIcon : folderOpenIcon" width="32" height="32" />
          </div>
          <h2>{{ emptyTitle }}</h2>
          <p>{{ emptyHint }}</p>
          <button v-if="!searchActive && canMutate" type="button" class="rrm-btn" @click="importFiles">
            <Icon :icon="uploadIcon" width="15" height="15" />
            选择文件导入
          </button>
        </div>

        <div
          v-else
          ref="listRef"
          class="rrm-list"
          :class="`is-${searchActive ? 'results' : viewMode}`"
          role="tree"
          aria-label="资源列表"
        >
          <div
            v-if="!searchActive && viewMode === 'list'"
            class="rrm-row is-header"
            role="presentation"
          >
            <span class="rrm-row-main">名称</span>
            <span class="rrm-row-size">{{ sortKey === 'size' ? '大小 ' + (sortDir === 'asc' ? '↑' : '↓') : '大小' }}</span>
            <span class="rrm-row-time">{{ sortKey === 'mtime' ? '修改时间 ' + (sortDir === 'asc' ? '↑' : '↓') : '修改时间' }}</span>
            <span class="rrm-row-kind">类型</span>
          </div>

          <button
            v-for="row in rows"
            :key="row.node.path"
            type="button"
            class="rrm-row"
            :class="{
              'is-selected': selection.has(row.node.path),
              'is-primary': primary === row.node.path,
              'is-dir': row.node.isDir,
            }"
            :data-path="row.node.path"
            :data-type="row.type"
            :style="{ '--rrm-depth': row.depth }"
            role="treeitem"
            :aria-selected="selection.has(row.node.path)"
            @click="onRowClick(row, $event)"
            @dblclick="onRowOpen(row)"
            @contextmenu.prevent="openMenu(row, $event)"
          >
            <span class="rrm-row-main">
              <span
                v-if="row.node.isDir && (viewMode === 'tree' || searchActive)"
                class="rrm-chevron"
                :class="{ 'is-open': expanded.has(row.node.path) }"
                role="button"
                tabindex="-1"
                :title="expanded.has(row.node.path) ? '收起文件夹' : '展开文件夹'"
                :aria-label="expanded.has(row.node.path) ? '收起' : '展开'"
                @click.stop="toggleExpand(row.node)"
              >
                <Icon :icon="chevronRightIcon" width="12" height="12" />
              </span>
              <span v-else class="rrm-chevron is-placeholder" aria-hidden="true" />
              <span v-if="viewMode === 'grid' && row.type === 'image'" class="rrm-tile-thumb">
                <ResourceThumb :node="row.node" :root="root" />
              </span>
              <span v-else class="rrm-type-icon" :data-type="row.type">
                <Icon :icon="iconFor(row)" width="15" height="15" />
              </span>
              <span class="rrm-row-name">{{ row.node.name }}</span>
              <span v-if="row.showParent" class="rrm-row-parent">{{ parentLabel(row.node) }}</span>
            </span>
            <span
              class="rrm-row-size"
              :title="row.node.isDir ? dirCountLabel(row.node) : ''"
            >{{ row.node.isDir ? dirCountLabel(row.node) : humanBytes(row.node.size) }}</span>
            <span class="rrm-row-time" :title="formatDateTime(row.node.mtime)">{{ humanAge(row.node.mtime) }}</span>
            <span class="rrm-row-kind">{{ typeLabel(row.type) }}</span>
          </button>
        </div>

        <footer class="rrm-status">
          <span>{{ formatCount(rows.length) }} 项</span>
          <span class="rrm-status-sep">·</span>
          <span>{{ scopeSizeLabel }}</span>
          <span v-if="scanMeta.elapsedMs" class="rrm-status-sep">·</span>
          <span v-if="scanMeta.elapsedMs">扫描 {{ scanMeta.elapsedMs }} ms</span>
          <span class="rrm-status-spacer" />
          <span v-if="selection.size > 1">已选 {{ selection.size }} 项</span>
          <span v-else-if="primary" class="rrm-status-path" :title="primary">{{ primary }}</span>
        </footer>
      </section>

      <div
        v-if="!inspectorCollapsed"
        class="rrm-resizer"
        title="拖动调整预览栏宽度"
        @pointerdown="startResize"
      />

      <aside v-if="!inspectorCollapsed" class="rrm-inspector">
        <template v-if="previewNode">
          <div class="rrm-preview-head">
            <div class="rrm-preview-title">
              <span class="rrm-type-icon" :data-type="previewType">
                <Icon :icon="iconForRow(previewNode, previewType)" width="16" height="16" />
              </span>
              <span class="rrm-preview-name" :title="previewNode.name">{{ previewNode.name }}</span>
            </div>
            <button type="button" class="rrm-icon-btn is-small" title="关闭预览" @click="clearSelection">
              <Icon :icon="xIcon" width="14" height="14" />
            </button>
          </div>

          <div class="rrm-preview-body">
            <ResourceRepoPreview
              :node="previewNode"
              :absolute-path="previewAbsolutePath"
              :root-path="root"
              :dir-summary="previewDirSummary"
            />
          </div>

          <div class="rrm-inspector-scroll">
            <section class="rrm-meta">
              <div class="rrm-meta-row">
                <span class="rrm-meta-key">位置</span>
                <button
                  type="button"
                  class="rrm-meta-path"
                  :title="`在文件夹中显示：${previewAbsolutePath}`"
                  @click="revealPath(previewAbsolutePath)"
                >
                  {{ previewParentLabel || '资源库根目录' }}
                </button>
              </div>
              <div class="rrm-meta-row">
                <span class="rrm-meta-key">大小</span>
                <span class="rrm-meta-value is-num">
                  {{ previewNode.isDir ? humanBytes(previewDirSummary.size) : humanBytes(previewNode.size) }}
                  <span v-if="previewNode.isDir" class="rrm-meta-weak">
                    （{{ previewDirSummary.files }} 文件 / {{ previewDirSummary.dirs }} 目录）
                  </span>
                </span>
              </div>
              <div class="rrm-meta-row">
                <span class="rrm-meta-key">修改</span>
                <span class="rrm-meta-value">{{ formatDateTime(previewNode.mtime) }}</span>
              </div>
              <div v-if="!previewNode.isDir" class="rrm-meta-row">
                <span class="rrm-meta-key">引用</span>
                <span class="rrm-meta-value">
                  <template v-if="previewUsage.length">
                    被 {{ previewUsage.length }} 篇文档使用
                  </template>
                  <span v-else class="rrm-meta-weak">没有文档引用它</span>
                </span>
              </div>
            </section>

            <section v-if="previewUsage.length" class="rrm-usage">
              <h3 class="rrm-section-title">被这些文档引用</h3>
              <button
                v-for="row in previewUsage.slice(0, 6)"
                :key="row.owner_file_path + row.relative_path"
                type="button"
                class="rrm-usage-item"
                :title="row.owner_file_path"
                @click="openOwner(row.owner_file_path)"
              >
                <Icon :icon="textIcon" width="14" height="14" />
                <span class="rrm-usage-name">{{ baseName(row.owner_file_path) }}</span>
                <span class="rrm-usage-type">{{ usageTypeLabel(row.resource_type) }}</span>
              </button>
              <p v-if="previewUsage.length > 6" class="rrm-more">
                还有 {{ previewUsage.length - 6 }} 篇…
              </p>
            </section>

            <section class="rrm-actions">
              <h3 class="rrm-section-title">操作</h3>
              <div class="rrm-action-grid">
                <button type="button" class="rrm-action" @click="openNode(previewNode)">
                  <Icon :icon="eyeIcon" width="15" height="15" />
                  打开
                </button>
                <button type="button" class="rrm-action" :disabled="previewNode.isDir" @click="openWithSystem(previewNode)">
                  <Icon :icon="externalIcon" width="15" height="15" />
                  系统程序
                </button>
                <button type="button" class="rrm-action" :disabled="previewNode.isDir" @click="copyRelative(previewNode)">
                  <Icon :icon="copyIcon" width="15" height="15" />
                  相对路径
                </button>
                <button type="button" class="rrm-action" @click="renameNode(previewNode)">
                  <Icon :icon="pencilIcon" width="15" height="15" />
                  重命名
                </button>
                <button
                  v-if="copyPresets.length"
                  type="button"
                  class="rrm-action is-wide"
                  :data-kind="copyKind"
                  @click="copyPrimaryPreset"
                >
                  <Icon :icon="linkIcon" width="15" height="15" />
                  {{ copyPresets[0].label }}
                </button>
                <button type="button" class="rrm-action is-danger is-wide" @click="requestDelete([previewNode])">
                  <Icon :icon="trashIcon" width="15" height="15" />
                  删除{{ previewNode.isDir ? '文件夹' : '文件' }}
                </button>
              </div>
            </section>

            <section v-if="showSync" class="rrm-sync-detail">
              <h3 class="rrm-section-title">
                同步
                <button type="button" class="rrm-link-btn" @click="showSync = false">收起</button>
              </h3>
              <div class="rrm-sync-title">{{ syncUi.title }}</div>
              <p class="rrm-sync-line">{{ syncState.detail || `最近操作：${syncState.action || '—'} · ${syncState.at || '—'}` }}</p>
              <div class="rrm-sync-meta">
                <span class="rrm-sync-provider">{{ syncProgress.provider === 'p2p' ? 'P2P' : '通用' }}</span>
                <span>{{ syncProgress.phase || 'idle' }}</span>
              </div>
              <div class="rrm-sync-bar">
                <span class="rrm-sync-bar-fill" :data-status="syncProgress.status" :style="{ width: `${syncProgressPercent}%` }" />
              </div>
              <dl class="rrm-sync-facts">
                <div><dt>消息</dt><dd>{{ syncProgress.message || '尚未收到同步进度事件' }}</dd></div>
                <div><dt>文件</dt><dd>{{ syncProgress.current || 0 }} / {{ syncProgress.total || 0 }}</dd></div>
                <div><dt>数据</dt><dd>{{ humanBytes(syncProgress.bytes_current || 0) }} / {{ humanBytes(syncProgress.bytes_total || 0) }}</dd></div>
                <div v-if="syncProgress.file_path"><dt>当前</dt><dd class="rrm-ellipsis" :title="syncProgress.file_path">{{ syncProgress.file_path }}</dd></div>
              </dl>
            </section>
          </div>
        </template>

        <div v-else class="rrm-inspector-empty">
          <div class="rrm-state-glyph is-quiet"><Icon :icon="eyeIcon" width="30" height="30" /></div>
          <h2>选中一项来查看</h2>
          <p>图片、PDF、CSV、Markdown 与文本都能直接在这里预览。删除或重命名不会打断你正在看的位置。</p>
          <dl class="rrm-repo-facts">
            <div><dt>条目</dt><dd>{{ formatCount(repoSummary.total) }}</dd></div>
            <div><dt>目录</dt><dd>{{ formatCount(repoSummary.dirs) }}</dd></div>
            <div><dt>占用</dt><dd>{{ humanBytes(repoSummary.size) }}</dd></div>
            <div><dt>已载入</dt><dd>{{ formatCount(loadedFileCount) }} 文件</dd></div>
          </dl>
          <button v-if="!showSync" type="button" class="rrm-btn is-quiet" @click="showSync = true">
            <Icon :icon="syncIcon" width="15" height="15" />
            查看同步状态
          </button>
        </div>
      </aside>
    </main>

    <div v-if="dropActive" class="rrm-drop">
      <div class="rrm-drop-card">
        <Icon :icon="uploadIcon" width="26" height="26" />
        <div class="rrm-drop-title">释放以导入到 {{ dropTargetLabel }}</div>
        <div class="rrm-drop-hint">文件与文件夹会复制进来，同名时自动改名。</div>
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

    <n-modal
      v-model:show="createModal.show"
      preset="card"
      class="rrm-modal"
      :title="createModal.title"
      :bordered="false"
      @after-enter="focusCreateInput"
    >
      <div class="rrm-modal-body">
        <p class="rrm-modal-hint">创建到 <strong>{{ targetDirLabel }}</strong></p>
        <input
          ref="createInputRef"
          v-model="createModal.name"
          class="rrm-text-input"
          :placeholder="createModal.placeholder"
          @keydown.enter.prevent="confirmCreate"
          @keydown.esc.prevent="createModal.show = false"
        />
      </div>
      <template #footer>
        <div class="rrm-modal-actions">
          <button type="button" class="rrm-btn" @click="createModal.show = false">取消</button>
          <button type="button" class="rrm-btn is-primary" :disabled="busy" @click="confirmCreate">创建</button>
        </div>
      </template>
    </n-modal>

    <n-modal
      v-model:show="renameModal.show"
      preset="card"
      class="rrm-modal"
      title="重命名"
      :bordered="false"
      @after-enter="focusRenameInput"
    >
      <div class="rrm-modal-body">
        <p class="rrm-modal-hint">当前：{{ renameModal.origin }}</p>
        <input
          ref="renameInputRef"
          v-model="renameModal.name"
          class="rrm-text-input"
          placeholder="新名称"
          @keydown.enter.prevent="confirmRename"
          @keydown.esc.prevent="renameModal.show = false"
        />
      </div>
      <template #footer>
        <div class="rrm-modal-actions">
          <button type="button" class="rrm-btn" @click="renameModal.show = false">取消</button>
          <button type="button" class="rrm-btn is-primary" :disabled="busy" @click="confirmRename">保存</button>
        </div>
      </template>
    </n-modal>

    <n-modal v-model:show="deleteModal.show" preset="card" class="rrm-modal" title="删除资源" :bordered="false">
      <div class="rrm-modal-body">
        <p class="rrm-modal-danger">{{ deleteModal.title }}</p>
        <p class="rrm-modal-hint">目录会连同内部资源一起移到系统回收站，可在文件管理器中还原。</p>
        <p v-if="deleteModal.usedBy" class="rrm-modal-warn">
          <Icon :icon="warningIcon" width="14" height="14" />
          其中有资源仍被文档引用，删除后对应位置会显示为缺失。
        </p>
      </div>
      <template #footer>
        <div class="rrm-modal-actions">
          <button type="button" class="rrm-btn" @click="deleteModal.show = false">取消</button>
          <button type="button" class="rrm-btn is-danger" :disabled="busy" @click="confirmDelete">移到回收站</button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import { Icon } from '@iconify/vue'
import { NModal, NDropdown, useMessage } from 'naive-ui'
import { open as openDialog } from '@tauri-apps/plugin-dialog'

import alertIcon from '@iconify-icons/lucide/alert-triangle'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import archiveIcon from '@iconify-icons/lucide/file-archive'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import copyIcon from '@iconify-icons/lucide/copy'
import externalIcon from '@iconify-icons/lucide/external-link'
import eyeIcon from '@iconify-icons/lucide/eye'
import eyeOffIcon from '@iconify-icons/lucide/eye-off'
import fileIcon from '@iconify-icons/lucide/file'
import filmIcon from '@iconify-icons/lucide/film'
import folderIcon from '@iconify-icons/lucide/folder'
import folderOpenIcon from '@iconify-icons/lucide/folder-open'
import folderPlusIcon from '@iconify-icons/lucide/folder-plus'
import folderSearchIcon from '@iconify-icons/lucide/folder-search'
import folderTreeIcon from '@iconify-icons/lucide/folder-tree'
import gridIcon from '@iconify-icons/lucide/layout-grid'
import imageIcon from '@iconify-icons/lucide/image'
import linkIcon from '@iconify-icons/lucide/link'
import listIcon from '@iconify-icons/lucide/list'
import listTreeIcon from '@iconify-icons/lucide/list-tree'
import musicIcon from '@iconify-icons/lucide/music'
import panelCloseIcon from '@iconify-icons/lucide/panel-right-close'
import panelOpenIcon from '@iconify-icons/lucide/panel-right-open'
import pencilIcon from '@iconify-icons/lucide/pencil'
import plugIcon from '@iconify-icons/lucide/plug-zap'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import searchIcon from '@iconify-icons/lucide/search'
import sortIcon from '@iconify-icons/lucide/arrow-down-up'
import syncIcon from '@iconify-icons/lucide/cloud'
import tableIcon from '@iconify-icons/lucide/table-2'
import textIcon from '@iconify-icons/lucide/file-text'
import trashIcon from '@iconify-icons/lucide/trash-2'
import uploadIcon from '@iconify-icons/lucide/upload'
import warningIcon from '@iconify-icons/lucide/alert-triangle'
import xIcon from '@iconify-icons/lucide/x'

import { getCurrentWindow } from '@tauri-apps/api/window'
import ResourceRepoPreview from './resourceRepo/ResourceRepoPreview.vue'
import ResourceThumb from './resourceRepo/ResourceThumb.vue'
import { dropThumbnails } from './resourceRepo/resourceThumbs'
import {
  RESOURCE_TYPE_META,
  baseName,
  bedUrlOf,
  breadcrumbs,
  buildResourceTree,
  chartRefOf,
  createComparator,
  extOf,
  formatDateTime,
  humanAge,
  humanBytes,
  joinPath,
  makeUniqueName,
  markdownImageOf,
  normalizeRepoPath,
  parentPath,
  resourceTypeOf,
  sortTree,
  summarizeDir,
  summarizeRepo,
  validateEntryName,
} from './resourceRepo/resourceRepoModel'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiResourceRepoManager')
const message = useMessage()

const RESOURCE_REPO_SYNC_STATUS_KEY = 'guaxi:resourceRepoSyncStatus:v1'
const PREFS_KEY = 'guaxi:resourceRepo:prefs:v1'
const EXPANDED_KEY = 'guaxi:resourceRepo:expanded:v1'
const MAX_ENTRIES = 60000

// ───────────────────────── 状态 ─────────────────────────

const pageRef = ref(null)
const listRef = ref(null)
const searchRef = ref(null)
const createInputRef = ref(null)
const renameInputRef = ref(null)

const scanning = ref(false)
const busy = ref(false)
const root = ref('')
// 整棵树用 shallowRef：几万个节点若被 Vue 递归代理，展开一个目录都要付代理开销。
// 结构变化由 touchTree() 显式通知，派生数据都读 treeRev 来建立依赖。
const tree = shallowRef(null)
const treeRev = ref(0)
const touchTree = () => {
  treeRev.value += 1
}
const scanMeta = reactive({ elapsedMs: 0, truncated: false, limit: MAX_ENTRIES })
const includeHidden = ref(false)

const activeDir = ref('')
const expanded = ref(new Set())
const selection = ref(new Set())
const primary = ref('')
const search = ref('')
const typeFilter = ref('all')

const viewMode = ref('tree')
const sortKey = ref('name')
const sortDir = ref('asc')
// 记住用户拖出来的宽度，另外算一个「当前窗口放得下多少」的显示宽度。
// 只存显示宽度的话，窗口一缩小面板就被永久压窄，再放大也回不来。
const inspectorWidth = ref(372)
const inspectorViewWidth = ref(0)
const inspectorCollapsed = ref(false)
const resizing = ref(false)
let resizeState = null

const usageMap = ref(new Map())
const showSync = ref(false)
const dropActive = ref(false)

const createModal = reactive({ show: false, mode: 'folder', name: '', title: '', placeholder: '' })
const renameModal = reactive({ show: false, target: null, origin: '', name: '' })
const deleteModal = reactive({ show: false, targets: [], title: '', usedBy: false })
const menu = reactive({ show: false, x: 0, y: 0, node: null })

const typeChipsOrder = ['image', 'data', 'text', 'document', 'media', 'other']

const createOptions = [
  { label: '新建文件夹', key: 'folder' },
  { label: '新建文件', key: 'file' },
]

const viewModes = [
  { key: 'tree', title: '树形视图', icon: listTreeIcon },
  { key: 'list', title: '列表视图', icon: listIcon },
  { key: 'grid', title: '网格视图', icon: gridIcon },
]

// ───────────────────────── 派生数据 ─────────────────────────

const normalizedSearch = computed(() => search.value.trim().toLowerCase())
const searchActive = computed(() => normalizedSearch.value.length > 0)

const rootNode = computed(() => {
  void treeRev.value
  return tree.value
})

const crumbs = computed(() => breadcrumbs(activeDir.value))

const activeDirNode = computed(() => {
  void treeRev.value
  if (!tree.value) return null
  let node = tree.value
  for (const segment of normalizeRepoPath(activeDir.value).split('/').filter(Boolean)) {
    const next = node.childMap?.get(segment)
    if (!next) return tree.value
    node = next
  }
  return node
})

/**
 * 新建 / 导入落到哪个目录。
 *
 * 选中一个文件夹就是它；选中一个文件则是它所在目录；什么都没选才用当前浏览的目录。
 * 这样「右键某个文件夹 → 新建」和工具栏按钮走的是同一套判断，不会两处规则不一致。
 */
const targetDir = computed(() => {
  const node = previewNode.value
  if (!node) return activeDir.value
  if (node.isDir) return node.path
  const parent = parentPath(node.path)
  return findNodeIn(tree.value, parent) ? parent : activeDir.value
})

const targetDirLabel = computed(() => (targetDir.value ? normalizeRepoPath(targetDir.value) : '资源库根目录'))
const dropTargetLabel = computed(() => targetDirLabel.value)

const repoSummary = computed(() => {
  void treeRev.value
  return tree.value ? summarizeRepo(tree.value) : { files: 0, dirs: 0, size: 0, total: 0, byType: {} }
})
const loadedFileCount = computed(() => repoSummary.value.files)

const typeChips = computed(() => {
  const byType = repoSummary.value.byType || {}
  const chips = [{ key: 'all', label: '全部', count: repoSummary.value.total }]
  for (const key of typeChipsOrder) {
    const count = byType[key] || 0
    if (!count) continue
    chips.push({ key, label: RESOURCE_TYPE_META[key]?.label || key, count })
  }
  return chips
})

const truncated = computed(() => scanMeta.truncated)

const sortLabel = computed(() => {
  const name = sortKey.value === 'mtime' ? '修改时间' : sortKey.value === 'size' ? '大小' : '名称'
  return `${name}${sortDir.value === 'asc' ? '↑' : '↓'}`
})

const canMutate = computed(() => Boolean(root.value))

/**
 * 浏览区的行。
 *
 * 搜索时跨全库返回命中项（附带所在目录），否则：
 * - 树形：从根展开，深度决定缩进；
 * - 列表 / 网格：只显示当前目录的直接子项。
 *
 * 这里所有列表都由同一棵内存树派生，不做二次请求，所以删除、重命名之后
 * 只需替换受影响目录的子节点，行序与滚动位置就能原地保持。
 */
const rows = computed(() => {
  void treeRev.value
  const node = tree.value
  if (!node) return []
  const out = []
  const typeOk = (n) => typeFilter.value === 'all' || resourceTypeOf(n.name, n.isDir) === typeFilter.value
  const keyword = normalizedSearch.value

  if (keyword) {
    const walk = (current, depth) => {
      for (const child of current.children || []) {
        if (keyword && !`${child.name} ${child.path}`.toLowerCase().includes(keyword)) {
          if (child.isDir) walk(child, depth + 1)
          continue
        }
        if (typeOk(child)) {
          out.push({ node: child, depth: 0, type: resourceTypeOf(child.name, child.isDir), showParent: true })
        }
        if (child.isDir) walk(child, depth + 1)
      }
    }
    walk(node, 0)
    return out
  }

  if (viewMode.value === 'tree') {
    const walk = (current, depth) => {
      for (const child of current.children || []) {
        if (typeOk(child)) {
          out.push({ node: child, depth, type: resourceTypeOf(child.name, child.isDir) })
        }
        if (child.isDir && expanded.value.has(child.path)) walk(child, depth + 1)
      }
    }
    walk(node, 0)
    return out
  }

  const scope = activeDirNode.value || node
  for (const child of scope.children || []) {
    if (typeOk(child)) out.push({ node: child, depth: 0, type: resourceTypeOf(child.name, child.isDir) })
  }
  return out
})

const emptyTitle = computed(() => {
  if (searchActive.value) return '没有匹配的资源'
  if (viewMode.value === 'tree') return '资源库还是空的'
  return '这个文件夹是空的'
})

const emptyHint = computed(() => {
  if (searchActive.value) return '换个关键词，或把类型筛选切回「全部」。'
  return '把图片、数据文件拖进这里，或者在编辑器里插入图片 —— 资源会自动归到 images/ 与 charts/ 下。'
})

const scopeSizeLabel = computed(() => {
  void treeRev.value
  const scope = searchActive.value ? null : activeDirNode.value || rootNode.value
  if (!scope) return humanBytes(repoSummary.value.size)
  let size = 0
  const walk = (node) => {
    for (const child of node.children || []) {
      if (child.isDir) walk(child)
      else size += Number(child.size) || 0
    }
  }
  if (scope.isDir === false) size = Number(scope.size) || 0
  else walk(scope)
  return humanBytes(size)
})

const gridStyle = computed(() => ({
  '--rrm-inspector-width': `${inspectorViewWidth.value || inspectorWidth.value}px`,
}))

// ───────────────────────── 预览 ─────────────────────────

const previewNode = computed(() => {
  void treeRev.value
  if (!tree.value || !primary.value) return null
  let node = tree.value
  for (const segment of normalizeRepoPath(primary.value).split('/').filter(Boolean)) {
    const next = node.childMap?.get(segment)
    if (!next) return null
    node = next
  }
  return node.path ? node : null
})

const previewType = computed(() => (previewNode.value ? resourceTypeOf(previewNode.value.name, previewNode.value.isDir) : 'other'))
const previewAbsolutePath = computed(() => (previewNode.value ? joinPath(root.value, previewNode.value.path) : ''))
const previewParentLabel = computed(() => (previewNode.value ? parentPath(previewNode.value.path) : ''))
const previewDirSummary = computed(() => {
  void treeRev.value
  return previewNode.value?.isDir ? summarizeDir(previewNode.value) : { files: 0, dirs: 0, size: 0 }
})

const previewUsage = computed(() => {
  const node = previewNode.value
  if (!node || node.isDir) return []
  return usageMap.value.get(node.path) || []
})

const copyKind = computed(() => (previewNode.value ? resourceTypeOf(previewNode.value.name, false) : 'other'))

/** 复制预设随资源类型变化：图片给 Markdown / 图床地址，数据给 chart 引用。 */
const copyPresets = computed(() => {
  const node = previewNode.value
  if (!node || node.isDir) return []
  const type = resourceTypeOf(node.name, false)
  if (type === 'image') {
    return [
      { key: 'markdown', label: '复制 Markdown 图片', value: () => markdownImageOf(node) },
      { key: 'url', label: '复制图床地址', value: () => bedUrlOf(node.path) },
    ]
  }
  if (type === 'data' && ['csv', 'tsv', 'json'].includes(extOf(node.name))) {
    return [
      { key: 'chart-ref', label: '复制图表数据引用', value: () => chartRefOf(node) },
      { key: 'absolute', label: '复制绝对路径', value: () => joinPath(root.value, node.path) },
    ]
  }
  return [
    { key: 'absolute', label: '复制绝对路径', value: () => joinPath(root.value, node.path) },
    { key: 'relative', label: '复制相对路径', value: () => node.path },
  ]
})

// ───────────────────────── 同步状态（沿用既有 localStorage 契约）─────────────────────────

const readSyncState = () => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY)
    if (!raw) return { status: 'idle', action: '', at: '', detail: '' }
    const value = JSON.parse(raw)
    return {
      status: String(value?.status || 'idle'),
      action: String(value?.action || ''),
      at: String(value?.at || ''),
      detail: String(value?.detail || ''),
    }
  } catch (e) {
    logger.warn('[ResourceRepo] read sync state failed', e)
    return { status: 'idle', action: '', at: '', detail: '' }
  }
}

const nowLabel = () => {
  try {
    return new Date().toLocaleString()
  } catch {
    return ''
  }
}

const syncState = ref(readSyncState())
const syncStateRaw = ref('')
const syncProgress = reactive({
  run_id: '',
  provider: '',
  target: '',
  phase: '',
  status: 'idle',
  message: '',
  current: 0,
  total: 0,
  bytes_current: 0,
  bytes_total: 0,
  file_path: '',
  at: '',
})
let syncPollTimer = null
let syncProgressUnlisten = null

const syncUi = computed(() => {
  const status = String(syncState.value?.status || 'idle')
  const map = {
    running: { state: 'running', tagText: '同步中', title: '资源库同步进行中' },
    success: { state: 'success', tagText: '已同步', title: '资源库同步状态正常' },
    error: { state: 'error', tagText: '同步失败', title: '资源库同步失败' },
    skipped: { state: 'skipped', tagText: '已跳过', title: '资源库同步已跳过' },
  }
  return map[status] || { state: 'idle', tagText: '未同步', title: '资源库同步未运行' }
})

const syncProgressPercent = computed(() => {
  const total = Number(syncProgress.total || 0)
  const current = Number(syncProgress.current || 0)
  if (total > 0) return Math.max(0, Math.min(100, Math.round((current / total) * 100)))
  const bytesTotal = Number(syncProgress.bytes_total || 0)
  const bytesCurrent = Number(syncProgress.bytes_current || 0)
  if (bytesTotal > 0) return Math.max(0, Math.min(100, Math.round((bytesCurrent / bytesTotal) * 100)))
  return syncProgress.status === 'success' ? 100 : 0
})

const writeSyncState = (next) => {
  try {
    localStorage.setItem(RESOURCE_REPO_SYNC_STATUS_KEY, JSON.stringify(next))
  } catch (e) {
    logger.warn('[ResourceRepo] write sync state failed', e)
  }
}

const setSyncState = (patch) => {
  const next = { ...syncState.value, ...patch }
  syncState.value = next
  writeSyncState(next)
  try {
    syncStateRaw.value = JSON.stringify(next)
  } catch {}
  try {
    window.dispatchEvent(new CustomEvent('guaxi:resource-repo-sync-state', { detail: next }))
  } catch (e) {
    logger.warn('[ResourceRepo] dispatch sync state failed', e)
  }
  logger.info('[ResourceRepo] sync state', next)
}

const applySyncProgress = (payload, source) => {
  if (!payload) return
  Object.assign(syncProgress, {
    run_id: String(payload.run_id || ''),
    provider: String(payload.provider || ''),
    target: String(payload.target || ''),
    phase: String(payload.phase || ''),
    status: String(payload.status || 'idle'),
    message: String(payload.message || ''),
    current: Number(payload.current || 0),
    total: Number(payload.total || 0),
    bytes_current: Number(payload.bytes_current || 0),
    bytes_total: Number(payload.bytes_total || 0),
    file_path: String(payload.file_path || ''),
    at: String(payload.at || ''),
  })
  logger.info('[ResourceRepo] sync progress', { source, phase: syncProgress.phase })
}

const refreshSyncStateFromStorage = (source) => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY) || ''
    if (raw && raw === syncStateRaw.value) return
    syncStateRaw.value = raw
    if (!raw) {
      syncState.value = { status: 'idle', action: '', at: '', detail: '' }
      return
    }
    const value = JSON.parse(raw)
    syncState.value = {
      status: String(value?.status || 'idle'),
      action: String(value?.action || ''),
      at: String(value?.at || ''),
      detail: String(value?.detail || ''),
    }
    logger.info('[ResourceRepo] sync state from storage', { source })
  } catch (e) {
    logger.warn('[ResourceRepo] refresh sync state failed', e)
  }
}

const handleSyncStateEvent = (event) => {
  const detail = event?.detail
  if (!detail) return
  syncState.value = {
    status: String(detail.status || 'idle'),
    action: String(detail.action || ''),
    at: String(detail.at || ''),
    detail: String(detail.detail || ''),
  }
}

const handleStorageEvent = (event) => {
  if (event?.key !== RESOURCE_REPO_SYNC_STATUS_KEY) return
  refreshSyncStateFromStorage('storage-event')
}

const formatInvokeError = (error) => {
  const raw = error?.message ?? (typeof error?.toString === 'function' ? error.toString() : String(error))
  return String(raw || '').trim() || '操作失败'
}

// ───────────────────────── 偏好 ─────────────────────────

const loadPrefs = () => {
  try {
    const raw = localStorage.getItem(PREFS_KEY)
    if (!raw) return
    const value = JSON.parse(raw)
    if (['tree', 'list', 'grid'].includes(value?.viewMode)) viewMode.value = value.viewMode
    if (['name', 'mtime', 'size'].includes(value?.sortKey)) sortKey.value = value.sortKey
    if (['asc', 'desc'].includes(value?.sortDir)) sortDir.value = value.sortDir
    if (Number(value?.inspectorWidth) > 0) inspectorWidth.value = Math.round(Number(value.inspectorWidth)) || 372
    if (typeof value?.includeHidden === 'boolean') includeHidden.value = value.includeHidden
    if (typeof value?.inspectorCollapsed === 'boolean') inspectorCollapsed.value = value.inspectorCollapsed
    // 恢复上次看的位置：重开窗口时预览区不该是空的。
    if (typeof value?.lastPath === 'string' && value.lastPath) {
      primary.value = value.lastPath
      selection.value = new Set([value.lastPath])
    }
    if (typeof value?.lastDir === 'string') activeDir.value = value.lastDir
  } catch (e) {
    logger.warn('[ResourceRepo] read prefs failed', e)
  }
}

const savePrefs = () => {
  try {
    localStorage.setItem(
      PREFS_KEY,
      JSON.stringify({
        viewMode: viewMode.value,
        sortKey: sortKey.value,
        sortDir: sortDir.value,
        inspectorWidth: inspectorWidth.value,
        includeHidden: includeHidden.value,
        inspectorCollapsed: inspectorCollapsed.value,
        lastPath: primary.value || '',
        lastDir: activeDir.value || '',
      })
    )
  } catch {}
}

const loadExpanded = () => {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY)
    const value = raw ? JSON.parse(raw) : []
    expanded.value = new Set(Array.isArray(value) ? value.filter(Boolean) : [])
  } catch {
    expanded.value = new Set()
  }
}

let saveExpandedTimer = null
const scheduleSaveExpanded = () => {
  clearTimeout(saveExpandedTimer)
  saveExpandedTimer = setTimeout(() => {
    try {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify([...expanded.value].slice(-400)))
    } catch {}
  }, 400)
}

// ───────────────────────── 扫描与增量刷新 ─────────────────────────

const scanError = ref('')

let scanToken = 0
const rescanAll = async ({ reason = 'manual' } = {}) => {
  const token = ++scanToken
  scanning.value = true
  scanError.value = ''
  try {
    const result = await invoke('resource_repo_scan', {
      includeHidden: includeHidden.value,
      relativePath: '',
    })
    // 扫描期间用户又点了一次刷新：旧结果直接丢弃，否则会把新树覆盖回去。
    if (token !== scanToken) return
    root.value = normalizeRepoPath(result?.root || '')
    const nextTree = buildResourceTree(result?.entries || [])
    // 保留展开状态：只保留仍然存在的路径，避免删掉的目录一直留在集合里。
    const alive = new Set()
    const walk = (node) => {
      for (const child of node.children || []) {
        if (child.isDir) {
          if (expanded.value.has(child.path)) alive.add(child.path)
          walk(child)
        }
      }
    }
    walk(nextTree)
    expanded.value = alive
    tree.value = sortTree(nextTree, createComparator(sortKey.value, sortDir.value))
    touchTree()
    scanMeta.elapsedMs = Number(result?.elapsed_ms) || 0
    scanMeta.truncated = Boolean(result?.truncated)
    scanMeta.limit = MAX_ENTRIES

    // 选中项如果还在，就保持；否则退到它原来的目录。
    if (primary.value && !findNodeIn(nextTree, primary.value)) {
      const fallbackDir = parentPath(primary.value)
      selection.value = new Set(fallbackDir && findNodeIn(nextTree, fallbackDir) ? [fallbackDir] : [])
      primary.value = fallbackDir && findNodeIn(nextTree, fallbackDir) ? fallbackDir : ''
    }
    if (activeDir.value && !findNodeIn(nextTree, activeDir.value)) {
      activeDir.value = ''
    }
    expandAncestors(primary.value || activeDir.value)
    logger.info('[ResourceRepo] scan complete', {
      reason,
      entries: result?.entries?.length || 0,
      elapsed: scanMeta.elapsedMs,
      truncated: scanMeta.truncated,
    })
  } catch (error) {
    if (token !== scanToken) return
    const detail = formatInvokeError(error)
    const notConfigured = detail.includes('未配置') || detail.includes('不可用')
    root.value = ''
    tree.value = null
    expanded.value = new Set()
    selection.value = new Set()
    primary.value = ''
    scanError.value = notConfigured ? '' : detail
    if (notConfigured) {
      logger.warn('[ResourceRepo] resource root not configured', { detail })
    } else {
      logger.error('[ResourceRepo] scan failed', error)
      message.error(detail)
    }
  } finally {
    scanning.value = false
  }
}

const findNodeIn = (rootNodeValue, path) => {
  const target = normalizeRepoPath(path)
  if (!target) return rootNodeValue
  let node = rootNodeValue
  for (const segment of target.split('/')) {
    const next = node?.childMap?.get(segment)
    if (!next) return null
    node = next
  }
  return node
}

/**
 * 只刷新一个目录的子项，并原地替换。
 *
 * 这是「删一个文件不会回到顶层」的关键：不做全量重扫、不动 expanded / activeDir，
 * 已有子目录对象按路径复用，展开状态与滚动位置自然保留。
 */
const refreshDir = async (dirPath) => {
  if (!tree.value) return
  const target = normalizeRepoPath(dirPath)
  const node = findNodeIn(tree.value, target)
  if (!node || !node.isDir) {
    await rescanAll({ reason: 'refresh-fallback' })
    return
  }
  let entries = []
  try {
    const result = await invoke('resource_repo_scan', {
      includeHidden: includeHidden.value,
      relativePath: target,
      singleLevel: true,
    })
    entries = result?.entries || []
  } catch (error) {
    // 局部刷新失败不该让已经成功的写操作显示成失败：记录后保持现状，
    // 用户按 F5 还能整库重扫。
    logger.warn('[ResourceRepo] refreshDir failed', { dir: target, error: String(error?.message || error) })
    return
  }
  const removed = applyChildren(node, entries)
  if (removed.length) {
    for (const path of removed) {
      if (primary.value === path || primary.value.startsWith(`${path}/`)) {
        selection.value = new Set()
        primary.value = ''
      }
      if (activeDir.value === path || activeDir.value.startsWith(`${path}/`)) {
        activeDir.value = parentPath(path)
      }
    }
    dropThumbnails((thumbPath) => removed.some((path) => thumbPath === path || thumbPath.startsWith(`${path}/`)))
  }
  sortTree(node, createComparator(sortKey.value, sortDir.value))
  touchTree()
}

/** 用一批条目替换某个目录的子项，返回被移除的路径。 */
const applyChildren = (dirNode, entries) => {
  const dirPath = dirNode.path
  const previous = new Map()
  for (const child of dirNode.children || []) previous.set(child.path, child)

  const nextChildren = []
  const nextMap = new Map()
  for (const raw of entries) {
    const rel = normalizeRepoPath(raw?.relative_path ?? '')
    if (!rel || parentPath(rel) !== dirPath) continue
    const name = String(raw?.name ?? baseName(rel))
    const isDir = Boolean(raw?.is_dir)
    const old = previous.get(rel)
    const node = isDir && old?.isDir
      ? Object.assign(old, { name, size: 0, mtime: Number(raw?.mtime_ms) || 0, isDir: true })
      : {
          path: rel,
          name,
          isDir,
          size: isDir ? 0 : Number(raw?.size) || 0,
          mtime: Number(raw?.mtime_ms) || 0,
          children: isDir ? [] : undefined,
          childMap: isDir ? new Map() : undefined,
          parentPath: dirPath,
        }
    nextChildren.push(node)
    nextMap.set(name, node)
  }

  const nextPaths = new Set(nextChildren.map((child) => child.path))
  const removed = []
  for (const [path] of previous) {
    if (!nextPaths.has(path)) removed.push(path)
  }

  dirNode.children = nextChildren
  dirNode.childMap = nextMap
  return removed
}

/** 展开某个路径的所有上级目录，保证它可见。 */
const expandAncestors = (path) => {
  const normalized = normalizeRepoPath(path)
  if (!normalized) return
  const next = new Set(expanded.value)
  let acc = ''
  const segments = normalized.split('/')
  for (let i = 0; i < segments.length - 1; i += 1) {
    acc = acc ? `${acc}/${segments[i]}` : segments[i]
    next.add(acc)
  }
  expanded.value = next
  scheduleSaveExpanded()
}

// ───────────────────────── 选择与导航 ─────────────────────────

let savePrefsTimer = null
const scheduleSavePrefs = () => {
  clearTimeout(savePrefsTimer)
  savePrefsTimer = setTimeout(savePrefs, 500)
}

const selectOnly = (path) => {
  const normalized = normalizeRepoPath(path)
  selection.value = new Set(normalized ? [normalized] : [])
  primary.value = normalized
  scheduleSavePrefs()
}

const clearSelection = () => selectOnly('')

const selectRange = (row) => {
  const index = rows.value.findIndex((item) => item.node.path === row.node.path)
  const anchorIndex = rows.value.findIndex((item) => item.node.path === primary.value)
  if (index < 0) return
  const from = anchorIndex < 0 ? index : anchorIndex
  const [start, end] = from <= index ? [from, index] : [index, from]
  const next = new Set(selection.value)
  for (let i = start; i <= end; i += 1) next.add(rows.value[i].node.path)
  selection.value = next
}

const onRowClick = (row, event) => {
  const path = row.node.path
  const additive = event?.ctrlKey || event?.metaKey
  if (event?.shiftKey && primary.value) {
    selectRange(row)
  } else if (additive) {
    const next = new Set(selection.value)
    if (next.has(path)) next.delete(path)
    else next.add(path)
    selection.value = next
    primary.value = next.has(path) ? path : [...next][next.size - 1] || ''
  } else {
    selectOnly(path)
  }
  if (row.node.isDir) {
    activeDir.value = viewMode.value === 'tree' ? path : activeDir.value
  }
  logger.debug('[ResourceRepo] select', { path, count: selection.value.size })
}

const onRowOpen = (row) => {
  const node = row.node
  if (node.isDir) {
    if (viewMode.value === 'tree') toggleExpand(node)
    navigateTo(node.path)
    return
  }
  openNode(node)
}

const toggleExpand = (node) => {
  const next = new Set(expanded.value)
  if (next.has(node.path)) next.delete(node.path)
  else next.add(node.path)
  expanded.value = next
  scheduleSaveExpanded()
}

const navigateTo = (path) => {
  const normalized = normalizeRepoPath(path)
  const node = findNodeIn(tree.value, normalized)
  if (!node || !node.isDir) return
  activeDir.value = normalized
  expandAncestors(normalized)
  if (viewMode.value === 'tree') {
    const next = new Set(expanded.value)
    next.add(normalized)
    expanded.value = next
    scheduleSaveExpanded()
  }
  selectOnly(normalized)
  scrollRowIntoView(normalized)
}

const scrollRowIntoView = async (path) => {
  await nextTick()
  const container = listRef.value
  if (!container) return
  const el = container.querySelector(`[data-path="${cssEscape(path)}"]`)
  el?.scrollIntoView?.({ block: 'nearest' })
}

const cssEscape = (value) => String(value).replace(/["\\]/g, '\\$&')

const goUp = () => {
  if (!activeDir.value) return
  navigateTo(parentPath(activeDir.value))
}

const moveSelection = (delta, extend = false) => {
  const list = rows.value
  if (!list.length) return
  let index = list.findIndex((row) => row.node.path === primary.value)
  if (index < 0) index = delta > 0 ? -1 : list.length
  const nextIndex = Math.max(0, Math.min(list.length - 1, index + delta))
  const row = list[nextIndex]
  if (extend) selectRange(row)
  else selectOnly(row.node.path)
  scrollRowIntoView(row.node.path)
}

const toggleExpandPrimary = () => {
  const node = previewNode.value
  if (node?.isDir) toggleExpand(node)
}

// ───────────────────────── 打开 ─────────────────────────

const openNode = async (node) => {
  if (!node) return
  if (node.isDir) {
    navigateTo(node.path)
    return
  }
  const abs = joinPath(root.value, node.path)
  const type = resourceTypeOf(node.name, false)
  try {
    if (type === 'text' || type === 'data') {
      // 文本类交给主编辑器的既有打开链路：与在文件树里双击完全一致。
      await emit('open_file_in_home', { path: abs, lineNumber: 1 })
      logger.info('[ResourceRepo] open in editor', { path: abs })
      return
    }
    const { resolveOpenTarget } = await import('../../utils/openTargetKind')
    const target = await resolveOpenTarget(abs, 'auto')
    if (!target || target.reader === 'text') {
      await emit('open_file_in_home', { path: abs, lineNumber: 1 })
      return
    }
    const theme = document.documentElement.dataset.guaxiTheme === 'dark' ? 'dark' : 'light'
    await invoke('create_doc_reader_window', {
      path: abs,
      name: node.name,
      kind: String(target.kind || ''),
      reader: String(target.reader || ''),
      theme,
    })
    logger.info('[ResourceRepo] open in reader window', { path: abs, reader: target.reader })
  } catch (error) {
    logger.error('[ResourceRepo] open failed', { path: abs, error })
    message.error(formatInvokeError(error))
  }
}

const openWithSystem = async (node) => {
  if (!node || node.isDir) return
  await revealPath(joinPath(root.value, node.path))
}

const revealPath = async (absPath) => {
  if (!absPath) return
  try {
    await invoke('open_in_file_explorer', { path: absPath })
  } catch (error) {
    logger.error('[ResourceRepo] reveal failed', error)
    message.error(formatInvokeError(error))
  }
}

const openRootInExplorer = () => revealPath(root.value)

const openOwner = async (ownerPath) => {
  if (!ownerPath) return
  try {
    await emit('open_file_in_home', { path: ownerPath, lineNumber: 1 })
    logger.info('[ResourceRepo] open owner doc', { ownerPath })
  } catch (error) {
    logger.error('[ResourceRepo] open owner failed', error)
    message.error(formatInvokeError(error))
  }
}

// ───────────────────────── 剪贴板 ─────────────────────────

const writeClipboard = async (text) => {
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
    return true
  } catch (error) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch (e) {
      logger.warn('[ResourceRepo] clipboard write failed', { error: e, first: error })
      return false
    }
  }
}

const copyText = async (text, label) => {
  const ok = await writeClipboard(text)
  if (ok) message.success(`已复制${label ? `：${label}` : ''}`)
  else message.error('复制失败')
}

const copyRelative = (node) => copyText(node.path, '相对路径')
const copyPrimaryPreset = () => {
  const preset = copyPresets.value[0]
  if (!preset) return
  copyText(preset.value(), preset.label.replace(/^复制/, ''))
}

// ───────────────────────── 新建 / 重命名 / 删除 ─────────────────────────

const openCreate = (mode) => {
  const dir = targetDir.value
  createModal.mode = mode === 'file' ? 'file' : 'folder'
  createModal.title = createModal.mode === 'folder' ? '新建文件夹' : '新建文件'
  createModal.placeholder = createModal.mode === 'folder' ? '文件夹名称' : '文件名称，例如 notes.md'
  createModal.name = createModal.mode === 'folder' ? '新建文件夹' : 'untitled.md'
  createModal.show = true
  logger.info('[ResourceRepo] open create modal', { mode: createModal.mode, dir })
}

/**
 * 弹层挂载完成后再聚焦：show 刚置 true 时输入框还没进 DOM，
 * nextTick 只能拿到 null，用户就得自己再点一次输入框。
 */
const focusCreateInput = () => {
  const el = createInputRef.value
  el?.focus?.()
  el?.select?.()
}

const confirmCreate = async () => {
  if (busy.value) return
  const checked = validateEntryName(createModal.name)
  if (!checked.ok) {
    message.warning(checked.reason)
    return
  }
  const dirNode = findNodeIn(tree.value, targetDir.value)
  const existing = (dirNode?.children || []).map((child) => child.name)
  const name = makeUniqueName(existing, checked.value)
  const rel = joinPath(targetDir.value, name)
  const abs = joinPath(root.value, rel)
  busy.value = true
  try {
    if (createModal.mode === 'folder') {
      await invoke('create_directory', { path: abs })
    } else {
      await invoke('create_file_with_content', { path: abs, content: '' })
    }
    createModal.show = false
    await refreshDir(targetDir.value)
    expandAncestors(rel)
    selectOnly(rel)
    if (createModal.mode === 'folder') {
      const next = new Set(expanded.value)
      next.add(rel)
      expanded.value = next
      scheduleSaveExpanded()
    }
    message.success(createModal.mode === 'folder' ? '文件夹已创建' : '文件已创建')
    logger.info('[ResourceRepo] create success', { rel, mode: createModal.mode })
  } catch (error) {
    logger.error('[ResourceRepo] create failed', error)
    message.error(formatInvokeError(error))
  } finally {
    busy.value = false
  }
}

const renameNode = (node) => {
  if (!node) return
  renameModal.target = node
  renameModal.origin = node.name
  renameModal.name = node.name
  renameModal.show = true
}

const focusRenameInput = () => {
  const el = renameInputRef.value
  const node = renameModal.target
  if (!el) return
  el.focus?.()
  // 只选中扩展名之前的部分：改文件名时不用先删掉 `.png`。
  const dot = node?.isDir ? -1 : String(node?.name || '').lastIndexOf('.')
  if (typeof el.setSelectionRange === 'function') {
    el.setSelectionRange(0, dot > 0 ? dot : String(renameModal.name || '').length)
  }
}

const confirmRename = async () => {
  const node = renameModal.target
  if (!node || busy.value) return
  const checked = validateEntryName(renameModal.name)
  if (!checked.ok) {
    message.warning(checked.reason)
    return
  }
  if (checked.value === node.name) {
    renameModal.show = false
    return
  }
  const oldPath = node.path
  const newPath = joinPath(parentPath(oldPath), checked.value)
  busy.value = true
  try {
    await invoke('move_path', {
      sourcePath: joinPath(root.value, oldPath),
      destinationPath: joinPath(root.value, newPath),
    })
    renameModal.show = false
    rebasePaths(oldPath, newPath)
    await refreshDir(parentPath(oldPath))
    if (parentPath(oldPath) !== parentPath(newPath)) await refreshDir(parentPath(newPath))
    selectOnly(newPath)
    expandAncestors(newPath)
    message.success('已重命名')
    logger.info('[ResourceRepo] rename success', { oldPath, newPath })
  } catch (error) {
    logger.error('[ResourceRepo] rename failed', error)
    message.error(formatInvokeError(error))
  } finally {
    busy.value = false
  }
}

/**
 * 重命名之后，把内存里受影响节点的路径整体换掉。
 *
 * 只改路径与父节点的 childMap 键，不重建子树 —— 子目录的展开状态、缩略图缓存键都
 * 跟着路径走，原地替换比重新扫描更稳，也不会把用户已经展开的层级收起来。
 */
const rebasePaths = (oldPath, newPath) => {
  if (!tree.value) return
  const node = findNodeIn(tree.value, oldPath)
  if (!node) return
  const parent = findNodeIn(tree.value, parentPath(oldPath))
  const oldName = node.name
  const newName = baseName(newPath)
  const prefix = `${oldPath}/`

  const rewrite = (target) => {
    target.path = target.path === oldPath ? newPath : `${newPath}/${target.path.slice(prefix.length)}`
    if (target.parentPath !== undefined) {
      target.parentPath = parentPath(target.path)
    }
    for (const child of target.children || []) {
      if (child.isDir) rewrite(child)
      else {
        child.path = child.path === oldPath ? newPath : `${newPath}/${child.path.slice(prefix.length)}`
        child.parentPath = parentPath(child.path)
      }
    }
  }
  rewrite(node)
  node.name = newName
  node.parentPath = parentPath(newPath)

  if (parent?.childMap) {
    parent.childMap.delete(oldName)
    parent.childMap.set(newName, node)
  }

  const rewritePath = (value) =>
    value === oldPath ? newPath : value.startsWith(prefix) ? `${newPath}/${value.slice(prefix.length)}` : value

  expanded.value = new Set([...expanded.value].map(rewritePath))
  selection.value = new Set([...selection.value].map(rewritePath))
  primary.value = rewritePath(primary.value)
  activeDir.value = rewritePath(activeDir.value)
  touchTree()
  scheduleSaveExpanded()
}

const requestDelete = (nodes) => {
  const list = (nodes || []).filter(Boolean)
  if (!list.length) return
  const usedBy = list.some((node) => {
    if (node.isDir) {
      return (usageMap.value.get(node.path)?.length || 0) > 0 ||
        [...usageMap.value.keys()].some((path) => path.startsWith(`${node.path}/`))
    }
    return (usageMap.value.get(node.path)?.length || 0) > 0
  })
  deleteModal.targets = list
  deleteModal.title = list.length === 1
    ? `确定删除「${list[0].name}」吗？`
    : `确定删除这 ${list.length} 项吗？`
  deleteModal.usedBy = usedBy
  deleteModal.show = true
  logger.info('[ResourceRepo] delete requested', { count: list.length })
}

const confirmDelete = async () => {
  const targets = deleteModal.targets || []
  if (!targets.length || busy.value) return
  busy.value = true
  const parents = new Set()
  const removedPaths = []
  try {
    for (const node of targets) {
      await invoke('delete_path', { path: joinPath(root.value, node.path) })
      parents.add(parentPath(node.path))
      removedPaths.push(node.path)
      logger.info('[ResourceRepo] deleted', { path: node.path })
    }
    deleteModal.show = false

    for (const parent of parents) await refreshDir(parent)
    dropThumbnails((path) => removedPaths.some((removed) => path === removed || path.startsWith(`${removed}/`)))

    // refreshDir 已经把失效的选中项清空；这里把焦点落到它原来的目录，
    // 而不是整棵树的顶层 —— 用户刚删完一个文件，视线不该被甩回根目录。
    if (!primary.value) {
      const fallback = [...parents][0] || ''
      if (fallback && findNodeIn(tree.value, fallback)) {
        selectOnly(fallback)
        if (!activeDir.value) activeDir.value = fallback
      }
    }
    message.success(targets.length === 1 ? '已删除' : `已删除 ${targets.length} 项`)
  } catch (error) {
    logger.error('[ResourceRepo] delete failed', error)
    message.error(formatInvokeError(error))
  } finally {
    busy.value = false
  }
}

const deleteSelection = () => {
  const selected = rows.value.filter((row) => selection.value.has(row.node.path)).map((row) => row.node)
  requestDelete(selected.length ? selected : previewNode.value ? [previewNode.value] : [])
}

// ───────────────────────── 导入 / 拖放 ─────────────────────────

const importFiles = async () => {
  if (!root.value) return
  try {
    const picked = await openDialog({ multiple: true, directory: false, title: '导入到资源库' })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    await importPaths(list)
  } catch (error) {
    logger.error('[ResourceRepo] import dialog failed', error)
    message.error(formatInvokeError(error))
  }
}

const importPaths = async (paths) => {
  const list = (paths || []).filter(Boolean)
  if (!list.length || !root.value) return
  if (!tree.value) await rescanAll({ reason: 'before-import' })
  const dir = targetDir.value
  const dirNode = findNodeIn(tree.value, dir)
  const existing = new Set((dirNode?.children || []).map((child) => child.name.toLowerCase()))
  busy.value = true
  let lastImported = ''
  let imported = 0
  try {
    for (const source of list) {
      const name = baseName(String(source).replace(/\\/g, '/'))
      if (!name) continue
      const finalName = makeUniqueName(existing, name)
      existing.add(finalName.toLowerCase())
      const destination = joinPath(root.value, dir, finalName)
      await invoke('copy_path', { sourcePath: String(source), destinationPath: destination })
      lastImported = joinPath(dir, finalName)
      imported += 1
      logger.info('[ResourceRepo] imported path', { source, destination })
    }
    if (activeDir.value !== dir) activeDir.value = dir
    await refreshDir(dir)
    if (lastImported) {
      expandAncestors(lastImported)
      selectOnly(lastImported)
      scrollRowIntoView(lastImported)
    }
    message.success(imported === 1 ? '已导入 1 项' : `已导入 ${imported} 项`)
  } catch (error) {
    logger.error('[ResourceRepo] import failed', error)
    message.error(formatInvokeError(error))
    await refreshDir(dir).catch(() => {})
  } finally {
    busy.value = false
  }
}

let unlistenDrop = null
const setupDragDrop = async () => {
  try {
    unlistenDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
      const payload = event?.payload || {}
      if (payload.type === 'enter' || payload.type === 'over') {
        dropActive.value = true
        return
      }
      if (payload.type === 'leave') {
        dropActive.value = false
        return
      }
      if (payload.type === 'drop') {
        dropActive.value = false
        await importPaths(payload.paths || [])
      }
    })
  } catch (error) {
    logger.warn('[ResourceRepo] drag drop listener failed', error)
  }
}

// ───────────────────────── 右键菜单 ─────────────────────────

const ensureMenuNode = (row) => {
  if (!selection.value.has(row.node.path)) selectOnly(row.node.path)
  return row
}

const openMenu = (row, event) => {
  const target = ensureMenuNode(row)
  menu.node = target
  menu.x = event?.clientX || 0
  menu.y = event?.clientY || 0
  menu.show = true
}

const menuOptions = computed(() => {
  const node = menu.node?.node
  if (!node) return []
  const items = node.isDir
    ? [
        { label: '打开文件夹', key: 'open' },
        { label: '在此新建文件夹', key: 'new-folder' },
        { label: '在此新建文件', key: 'new-file' },
        { type: 'divider', key: 'd1' },
        { label: '在文件管理器中显示', key: 'reveal' },
        { label: '重命名', key: 'rename' },
        { label: '删除文件夹', key: 'delete' },
      ]
    : [
        { label: '预览', key: 'preview' },
        { label: '打开', key: 'open' },
        { label: '用系统程序打开', key: 'system' },
        { type: 'divider', key: 'd1' },
        { label: '复制相对路径', key: 'copy-relative' },
        { label: '复制绝对路径', key: 'copy-absolute' },
        ...(resourceTypeOf(node.name, false) === 'image'
          ? [
              { label: '复制 Markdown 图片', key: 'copy-markdown' },
              { label: '复制图床地址', key: 'copy-url' },
            ]
          : []),
        ...(resourceTypeOf(node.name, false) === 'data' && ['csv', 'tsv', 'json'].includes(extOf(node.name))
          ? [{ label: '复制图表数据引用', key: 'copy-chart' }]
          : []),
        { type: 'divider', key: 'd2' },
        { label: '在文件管理器中显示', key: 'reveal' },
        { label: '重命名', key: 'rename' },
        { label: '删除', key: 'delete' },
      ]
  if (selection.value.size > 1) {
    items.push({ type: 'divider', key: 'd3' })
    items.push({ label: `删除所选 ${selection.value.size} 项`, key: 'delete-many' })
  }
  return items
})

/**
 * 手动触发的下拉不会自己收起，这里用窗口级 pointerdown 兜底。
 * 必须放过下拉自身的点击 —— 捕获阶段先于 click 触发，若在这里收起，
 * 选项还没收到 click 就被卸载，菜单看起来「点了没反应」。
 */
const closeMenu = (event) => {
  if (!menu.show) return
  const target = event?.target
  if (target instanceof Element && target.closest('.n-dropdown-menu, .n-dropdown')) return
  menu.show = false
}

const onMenuSelect = (key) => {
  const node = menu.node?.node
  menu.show = false
  if (!key || !node) return
  const abs = joinPath(root.value, node.path)
  switch (key) {
    case 'open':
      openNode(node)
      break
    case 'preview':
      selectOnly(node.path)
      break
    case 'system':
      openWithSystem(node)
      break
    case 'reveal':
      revealPath(abs)
      break
    case 'rename':
      renameNode(node)
      break
    case 'delete':
      requestDelete([node])
      break
    case 'delete-many':
      deleteSelection()
      break
    case 'copy-relative':
      copyRelative(node)
      break
    case 'copy-absolute':
      copyText(abs, '绝对路径')
      break
    case 'copy-markdown':
      copyText(markdownImageOf(node), 'Markdown 图片')
      break
    case 'copy-url':
      copyText(bedUrlOf(node.path), '图床地址')
      break
    case 'copy-chart':
      copyText(chartRefOf(node), '图表数据引用')
      break
    case 'new-folder':
      activeDir.value = node.path
      expandAncestors(node.path)
      openCreate('folder')
      break
    case 'new-file':
      activeDir.value = node.path
      expandAncestors(node.path)
      openCreate('file')
      break
    default:
      break
  }
  logger.info('[ResourceRepo] menu action', { key, path: node.path })
}

// ───────────────────────── 同步动作 ─────────────────────────

const testConnection = async () => {
  if (busy.value) return
  const action = '测试连接'
  busy.value = true
  setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
  applySyncProgress({ provider: 'unknown', phase: 'connect', status: 'running', message: '正在测试同步连接', at: nowLabel() }, 'local-test')
  try {
    const res = await invoke('resource_repo_test_connection')
    const text = typeof res === 'string' ? res : '连接正常'
    applySyncProgress({
      provider: text.includes('P2P') ? 'p2p' : 'other',
      phase: 'connect',
      status: 'success',
      message: text,
      current: 1,
      total: 1,
      at: nowLabel(),
    }, 'local-test')
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
    message.success(text)
    logger.info('[ResourceRepo] test connection success', { text })
  } catch (error) {
    const detail = formatInvokeError(error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail })
    message.error(detail)
    logger.error('[ResourceRepo] test connection failed', error)
  } finally {
    busy.value = false
  }
}

const syncNow = async () => {
  if (busy.value) return
  const action = '立即同步'
  busy.value = true
  setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
  applySyncProgress({ provider: 'unknown', phase: 'start', status: 'running', message: '正在启动资源库同步', at: nowLabel() }, 'local-sync')
  try {
    const res = await invoke('resource_repo_sync_now')
    const text = typeof res === 'string' ? res : '同步完成'
    if (syncProgress.provider !== 'p2p') {
      applySyncProgress({
        provider: 'other',
        phase: 'complete',
        status: 'success',
        message: text,
        current: 1,
        total: 1,
        at: nowLabel(),
      }, 'local-sync')
    }
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
    message.success(text)
    logger.info('[ResourceRepo] sync success', { text })
    await rescanAll({ reason: 'after-sync' })
  } catch (error) {
    const detail = formatInvokeError(error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail })
    message.error(detail)
    logger.error('[ResourceRepo] sync failed', error)
  } finally {
    busy.value = false
  }
}

// ───────────────────────── 样式辅助 ─────────────────────────

const typeIcons = {
  dir: folderIcon,
  image: imageIcon,
  data: tableIcon,
  text: textIcon,
  document: fileIcon,
  media: filmIcon,
  archive: archiveIcon,
  font: textIcon,
  other: fileIcon,
}

const iconForRow = (node, type) => {
  if (node?.isDir) return expanded.value.has(node.path) ? folderOpenIcon : folderIcon
  const ext = extOf(node?.name || '')
  if (['mp3', 'wav', 'flac', 'ogg', 'm4a', 'aac'].includes(ext)) return musicIcon
  if (['mp4', 'mkv', 'webm', 'avi', 'mov'].includes(ext)) return filmIcon
  return typeIcons[type] || fileIcon
}

const iconFor = (row) => iconForRow(row.node, row.type)

const typeLabel = (type) => RESOURCE_TYPE_META[type]?.label || '文件'
const usageTypeLabel = (type) => (type === 'data' ? '图表数据' : type === 'image' ? '图片引用' : '引用')

const dirCountLabel = (node) => {
  const summary = summarizeDir(node)
  return `${summary.total} 项`
}

const parentLabel = (node) => parentPath(node.path) || '资源库'

const formatCount = (value) => {
  try {
    return Number(value || 0).toLocaleString('zh-CN')
  } catch {
    return String(value || 0)
  }
}

// ───────────────────────── 其它交互 ─────────────────────────

/**
 * 切换到列表 / 网格时必须落到「当前选中项所在的目录」。
 *
 * 树形视图可以同时显示多个层级，切走之后只剩一层：如果沿用 activeDir（可能是根目录），
 * 用户刚选中的文件会从列表里消失，预览却还指着它 —— 看起来像选中丢了。
 */
const setViewMode = (mode) => {
  if (!['tree', 'list', 'grid'].includes(mode)) return
  const node = previewNode.value
  const wasTree = viewMode.value === 'tree'
  viewMode.value = mode
  if (mode !== 'tree' && wasTree && node) {
    const dir = node.isDir ? node.path : parentPath(node.path)
    if (findNodeIn(tree.value, dir)) activeDir.value = dir
  }
  if (mode !== 'tree') expandAncestors(activeDir.value)
  savePrefs()
  logger.info('[ResourceRepo] view mode', { mode, activeDir: activeDir.value })
}

const setTypeFilter = (key) => {
  typeFilter.value = key
  logger.info('[ResourceRepo] type filter', { key })
}

/**
 * 排序按钮循环三种常用顺序。
 * 「最近修改」和「最大文件」几乎总是想看新的/大的在前，所以它们默认降序；
 * 名称则默认升序。再点一次同一项就反过来，不必再开第二个菜单。
 */
const cycleSort = () => {
  const plan = [
    { key: 'name', dir: 'asc' },
    { key: 'mtime', dir: 'desc' },
    { key: 'size', dir: 'desc' },
    { key: 'name', dir: 'desc' },
  ]
  const index = plan.findIndex((item) => item.key === sortKey.value && item.dir === sortDir.value)
  const next = plan[(index + 1) % plan.length]
  sortKey.value = next.key
  sortDir.value = next.dir
  if (tree.value) {
    sortTree(tree.value, createComparator(sortKey.value, sortDir.value))
    touchTree()
  }
  savePrefs()
  logger.info('[ResourceRepo] sort', { key: sortKey.value, dir: sortDir.value })
}

/**
 * 隐藏项（`.guaxi-p2p` 这类同步元数据目录）默认不显示：它们不是用户资源，
 * 混在图片日期目录里只会让人误删。打开后需要全量重扫；关掉时同样要重扫，
 * 因为内存里的树已经包含这些节点了。
 */
const toggleHidden = async () => {
  includeHidden.value = !includeHidden.value
  savePrefs()
  await rescanAll({ reason: includeHidden.value ? 'show-hidden' : 'hide-hidden' })
}

const toggleInspector = () => {
  inspectorCollapsed.value = !inspectorCollapsed.value
  savePrefs()
}

const clampInspectorWidth = (width) => {
  const min = 300
  const max = 640
  const viewport = window.innerWidth || 0
  const available = viewport > 0 ? Math.max(min, viewport - 420) : max
  return Math.max(min, Math.min(max, available, Math.round(Number(width) || inspectorWidth.value)))
}

/** 把偏好宽度落到当前窗口允许的宽度上。 */
const applyInspectorWidth = () => {
  inspectorViewWidth.value = clampInspectorWidth(inspectorWidth.value)
}

const startResize = (event) => {
  if (!event) return
  event.preventDefault?.()
  resizing.value = true
  resizeState = { startX: Number(event.clientX || 0), startWidth: inspectorWidth.value }
  try {
    document.body.classList.add('resource-inspector-resizing')
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
    document.body.classList.remove('resource-inspector-resizing')
  } catch {}
  savePrefs()
}

const onWindowResize = () => {
  if (resizing.value) return
  applyInspectorWidth()
}

const isTypingTarget = (target) => {
  const tag = String(target?.tagName || '').toLowerCase()
  return tag === 'input' || tag === 'textarea' || target?.isContentEditable
}

const onKeydown = (event) => {
  if (createModal.show || renameModal.show || deleteModal.show) return
  const typing = isTypingTarget(event.target)

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
    if (search.value) search.value = ''
    else if (event.target === searchRef.value) event.target.blur?.()
    else clearSelection()
    return
  }
  if (typing) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      moveSelection(event.key === 'ArrowDown' ? 1 : -1)
    }
    return
  }

  switch (event.key) {
    case 'F2':
      event.preventDefault()
      if (previewNode.value) renameNode(previewNode.value)
      break
    case 'F5':
      event.preventDefault()
      rescanAll({ reason: 'keyboard' })
      break
    case 'Delete':
      event.preventDefault()
      deleteSelection()
      break
    case 'Enter':
      if (previewNode.value) {
        event.preventDefault()
        openNode(previewNode.value)
      }
      break
    case 'ArrowDown':
      event.preventDefault()
      moveSelection(1, event.shiftKey)
      break
    case 'ArrowUp':
      event.preventDefault()
      moveSelection(-1, event.shiftKey)
      break
    case 'ArrowRight':
      if (event.shiftKey) break
      event.preventDefault()
      toggleExpandPrimary()
      break
    case 'ArrowLeft':
      if (event.shiftKey) break
      event.preventDefault()
      if (previewNode.value?.isDir && expanded.value.has(previewNode.value.path)) toggleExpand(previewNode.value)
      else goUp()
      break
    case 'Home':
      event.preventDefault()
      if (rows.value.length) selectOnly(rows.value[0].node.path)
      break
    case 'End':
      event.preventDefault()
      if (rows.value.length) selectOnly(rows.value[rows.value.length - 1].node.path)
      break
    case 'a':
    case 'A':
      if (event.ctrlKey || event.metaKey) {
        event.preventDefault()
        selection.value = new Set(rows.value.map((row) => row.node.path))
        primary.value = rows.value[0]?.node.path || ''
      }
      break
    default:
      break
  }
}

// ───────────────────────── 生命周期 ─────────────────────────

const loadUsageIndex = async () => {
  try {
    const rowsData = await invoke('resource_repo_usage_index')
    const map = new Map()
    for (const row of rowsData || []) {
      const rel = normalizeRepoPath(row.relative_path)
      if (!rel) continue
      if (!map.has(rel)) map.set(rel, [])
      map.get(rel).push(row)
    }
    usageMap.value = map
    logger.info('[ResourceRepo] usage index loaded', { resources: map.size })
  } catch (error) {
    logger.warn('[ResourceRepo] usage index failed', error)
    usageMap.value = new Map()
  }
}

watch([sortKey, sortDir], () => {
  if (tree.value) {
    sortTree(tree.value, createComparator(sortKey.value, sortDir.value))
    touchTree()
  }
  savePrefs()
})

watch(inspectorCollapsed, savePrefs)

// 树被原地修改后，主选中项可能已经消失（删除/重命名）。只在真的找不到时清掉，
// 不做「一改就重置」，否则刚点开预览就会被刷新动作打断。
watch(treeRev, () => {
  if (!primary.value || !tree.value) return
  if (!findNodeIn(tree.value, primary.value)) {
    const fallback = parentPath(primary.value)
    if (fallback && findNodeIn(tree.value, fallback)) selectOnly(fallback)
    else clearSelection()
  }
  if (activeDir.value && !findNodeIn(tree.value, activeDir.value)) activeDir.value = ''
})

onMounted(async () => {
  loadPrefs()
  applyInspectorWidth()
  loadExpanded()
  window.addEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.addEventListener('storage', handleStorageEvent)
  window.addEventListener('resize', onWindowResize)
  window.addEventListener('pointerdown', closeMenu, true)
  window.addEventListener('blur', closeMenu)
  refreshSyncStateFromStorage('mounted')

  try {
    syncProgressUnlisten = await listen('resource_repo_sync_progress', (event) => {
      applySyncProgress(event?.payload, 'tauri-event')
    })
  } catch (error) {
    logger.warn('[ResourceRepo] sync progress listener failed', error)
  }

  syncPollTimer = setInterval(() => refreshSyncStateFromStorage('poll'), 1200)

  await setupDragDrop()
  await Promise.all([rescanAll({ reason: 'mounted' }), loadUsageIndex()])
  // 恢复上次的选中位置：展开它的上级，让预览一打开就有内容可看。
  if (primary.value && findNodeIn(tree.value, primary.value)) {
    expandAncestors(primary.value)
    if (viewMode.value !== 'tree') activeDir.value = parentPath(primary.value)
  }
  nextTick(() => pageRef.value?.focus?.())
})

onBeforeUnmount(() => {
  stopResize()
  clearTimeout(saveExpandedTimer)
  clearTimeout(savePrefsTimer)
  savePrefs()
  window.removeEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.removeEventListener('storage', handleStorageEvent)
  window.removeEventListener('resize', onWindowResize)
  window.removeEventListener('pointerdown', closeMenu, true)
  window.removeEventListener('blur', closeMenu)
  if (syncProgressUnlisten) {
    try {
      syncProgressUnlisten()
    } catch {}
    syncProgressUnlisten = null
  }
  if (syncPollTimer) {
    clearInterval(syncPollTimer)
    syncPollTimer = null
  }
  if (unlistenDrop) {
    try {
      unlistenDrop()
    } catch {}
    unlistenDrop = null
  }
})
</script>

<style scoped>
/* ─────────────────────────────────────────────────────────────
   资源库管理器

   视觉取向：一张灯箱（light table），而不是又一个文件管理器。
   预览是主角：右栏常驻，选中即看；左侧密排、安静，类型色只做极轻的点缀，
   避免满屏彩色图标把缩略图淹掉。

   结构上先定一条规则：所有「操作后的刷新」都只替换目录子节点，不重扫整库。
   因此选中态避免大面积填充 —— 行不会整体重绘，填充色会让人误以为跳转了位置。
   ───────────────────────────────────────────────────────────── */
.rrm {
  --rrm-fg: var(--guaxi-text-primary, #1f2328);
  --rrm-fg-2: var(--guaxi-text-secondary, #57606a);
  --rrm-fg-3: color-mix(in srgb, var(--guaxi-text-secondary, #57606a) 72%, transparent);
  --rrm-bg: var(--guaxi-app-bg, #ffffff);
  --rrm-panel: var(--guaxi-panel-bg, #ffffff);
  --rrm-line: color-mix(in srgb, var(--guaxi-border, #d0d7de) 82%, transparent);
  --rrm-line-soft: color-mix(in srgb, var(--guaxi-border, #d0d7de) 48%, transparent);
  --rrm-hover: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 5%, transparent);
  --rrm-sel: color-mix(in srgb, var(--n-primary-color, #4078f2) 13%, transparent);
  --rrm-sel-strong: color-mix(in srgb, var(--n-primary-color, #4078f2) 20%, transparent);
  --rrm-accent: var(--n-primary-color, #4078f2);
  --rrm-danger: var(--n-error-color, #e45649);
  --rrm-radius: 9px;

  --rrm-tone-image: #2f9e9a;
  --rrm-tone-data: #a4761c;
  --rrm-tone-text: #4078f2;
  --rrm-tone-document: #7a5bd6;
  --rrm-tone-media: #c0558a;
  --rrm-tone-archive: #7d8794;

  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--rrm-bg);
  color: var(--rrm-fg);
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  font-size: 13px;
  line-height: 1.45;
  outline: none;
}

:root[data-guaxi-theme='dark'] .rrm {
  --rrm-fg-3: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 66%, transparent);
  --rrm-hover: color-mix(in srgb, #ffffff 6%, transparent);
  --rrm-tone-image: #56b6c2;
  --rrm-tone-data: #e5c07b;
  --rrm-tone-text: #61afef;
  --rrm-tone-document: #c678dd;
  --rrm-tone-media: #e06c9f;
  --rrm-tone-archive: #9da5b4;
}

.rrm * {
  box-sizing: border-box;
}

.rrm :focus-visible {
  outline: 2px solid var(--rrm-accent);
  outline-offset: -1px;
  border-radius: 6px;
}

/* ───────────── 顶栏 ───────────── */

.rrm-header {
  flex: 0 0 auto;
  height: 50px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 12px 0 14px;
  border-bottom: 1px solid var(--rrm-line);
  background: var(--rrm-panel);
}

.rrm-brand {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 9px;
}

.rrm-brand-mark {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: var(--rrm-accent);
  background: var(--rrm-sel);
}

.rrm-title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.rrm-sync-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding: 0 9px;
  border: 1px solid var(--rrm-line);
  border-radius: 999px;
  background: transparent;
  color: var(--rrm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.rrm-sync-pill:hover {
  background: var(--rrm-hover);
}

.rrm-sync-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--rrm-fg-3);
}

.rrm-sync-pill[data-state='running'] .rrm-sync-dot {
  background: var(--n-info-color, #56b6c2);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--n-info-color, #56b6c2) 22%, transparent);
}

.rrm-sync-pill[data-state='success'] .rrm-sync-dot {
  background: var(--n-success-color, #50a14f);
}

.rrm-sync-pill[data-state='error'] .rrm-sync-dot {
  background: var(--rrm-danger);
}

.rrm-sync-pill[data-state='skipped'] .rrm-sync-dot {
  background: var(--n-warning-color, #c18401);
}

.rrm-search {
  position: relative;
  flex: 1 1 auto;
  min-width: 180px;
  max-width: 620px;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--rrm-line);
  border-radius: var(--rrm-radius);
  background: color-mix(in srgb, var(--rrm-panel) 82%, var(--rrm-bg));
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.rrm-search:focus-within {
  border-color: var(--rrm-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--rrm-accent) 16%, transparent);
}

.rrm-search-icon {
  flex: 0 0 auto;
  color: var(--rrm-fg-3);
}

.rrm-search-input {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--rrm-fg);
  font: inherit;
}

.rrm-search-input::placeholder {
  color: var(--rrm-fg-3);
}

.rrm-search-input::-webkit-search-cancel-button {
  display: none;
}

.rrm-kbd {
  flex: 0 0 auto;
  padding: 1px 5px;
  border: 1px solid var(--rrm-line);
  border-radius: 5px;
  color: var(--rrm-fg-3);
  font-size: 10px;
  line-height: 1.4;
}

.rrm-search-clear {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--rrm-fg-3);
  cursor: pointer;
}

.rrm-search-clear:hover {
  background: var(--rrm-hover);
  color: var(--rrm-fg);
}

.rrm-header-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

/* ───────────── 通用按钮 ───────────── */

.rrm-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 11px;
  border: 1px solid var(--rrm-line);
  border-radius: 8px;
  background: var(--rrm-panel);
  color: var(--rrm-fg);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}

.rrm-btn:hover:not(:disabled) {
  background: var(--rrm-hover);
}

.rrm-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.rrm-btn.is-primary {
  border-color: transparent;
  background: var(--rrm-accent);
  color: #fff;
}

.rrm-btn.is-primary:hover:not(:disabled) {
  filter: brightness(1.06);
}

.rrm-btn.is-danger {
  border-color: transparent;
  background: var(--rrm-danger);
  color: #fff;
}

.rrm-btn.is-quiet {
  border-style: dashed;
  background: transparent;
  color: var(--rrm-fg-2);
}

.rrm-icon-btn {
  display: inline-grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--rrm-fg-2);
  cursor: pointer;
}

.rrm-icon-btn:hover:not(:disabled) {
  background: var(--rrm-hover);
  color: var(--rrm-fg);
}

.rrm-icon-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.rrm-icon-btn.is-small {
  width: 26px;
  height: 26px;
}

.rrm-icon-btn.is-on {
  color: var(--rrm-accent);
  background: var(--rrm-sel);
}

.rrm-icon-btn.is-busy {
  color: var(--rrm-accent);
}

.rrm-icon-btn.is-busy :deep(svg) {
  animation: rrm-spin 0.9s linear infinite;
}

/* ───────────── 工具栏 ───────────── */

.rrm-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 38px;
  padding: 5px 12px 5px 14px;
  border-bottom: 1px solid var(--rrm-line-soft);
  background: var(--rrm-panel);
}

.rrm-crumbs {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  overflow: hidden;
}

.rrm-crumb {
  flex: 0 0 auto;
  max-width: 180px;
  height: 22px;
  padding: 0 6px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--rrm-fg-2);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rrm-crumb:hover {
  background: var(--rrm-hover);
  color: var(--rrm-fg);
}

.rrm-crumb.is-current {
  color: var(--rrm-fg);
  font-weight: 600;
}

.rrm-crumb.is-root {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--rrm-fg);
}

.rrm-crumb-sep {
  flex: 0 0 auto;
  color: var(--rrm-fg-3);
  font-size: 11px;
}

.rrm-search-scope {
  flex: 0 0 auto;
  margin-left: 6px;
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--rrm-sel);
  color: var(--rrm-accent);
  font-size: 11px;
}

.rrm-filters {
  flex: 0 1 auto;
  display: flex;
  align-items: center;
  gap: 4px;
  overflow: hidden;
}

.rrm-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--rrm-fg-2);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
  white-space: nowrap;
}

.rrm-chip:hover {
  background: var(--rrm-hover);
}

.rrm-chip.is-active {
  border-color: var(--rrm-line);
  background: var(--rrm-sel);
  color: var(--rrm-fg);
}

.rrm-chip-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--rrm-fg-3);
}

.rrm-chip[data-type='image'] .rrm-chip-dot {
  background: var(--rrm-tone-image);
}

.rrm-chip[data-type='data'] .rrm-chip-dot {
  background: var(--rrm-tone-data);
}

.rrm-chip[data-type='text'] .rrm-chip-dot {
  background: var(--rrm-tone-text);
}

.rrm-chip[data-type='document'] .rrm-chip-dot {
  background: var(--rrm-tone-document);
}

.rrm-chip[data-type='media'] .rrm-chip-dot {
  background: var(--rrm-tone-media);
}

.rrm-chip[data-type='other'] .rrm-chip-dot {
  background: var(--rrm-tone-archive);
}

.rrm-chip-count {
  color: var(--rrm-fg-3);
  font-variant-numeric: tabular-nums;
}

.rrm-view-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.rrm-view-actions .rrm-icon-btn {
  width: auto;
  padding: 0 8px;
  gap: 6px;
  display: inline-flex;
}

.rrm-sort-text {
  font-size: 11px;
  color: var(--rrm-fg-2);
}

.rrm-btn-caret {
  margin-left: -2px;
  opacity: 0.6;
}

.rrm-segmented {
  display: inline-flex;
  padding: 2px;
  border: 1px solid var(--rrm-line);
  border-radius: 8px;
  background: var(--rrm-bg);
}

.rrm-seg {
  display: grid;
  place-items: center;
  width: 26px;
  height: 22px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--rrm-fg-3);
  cursor: pointer;
}

.rrm-seg:hover {
  color: var(--rrm-fg);
}

.rrm-seg.is-active {
  background: var(--rrm-panel);
  color: var(--rrm-fg);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
}

/* ───────────── 主体 ───────────── */

.rrm-main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 7px var(--rrm-inspector-width, 372px);
  overflow: hidden;
}

.rrm.is-inspector-collapsed .rrm-main {
  grid-template-columns: minmax(0, 1fr);
}

.rrm-browser {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.rrm-banner {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 14px;
  font-size: 11.5px;
  border-bottom: 1px solid var(--rrm-line-soft);
}

.rrm-banner.is-warning {
  color: var(--n-warning-color, #c18401);
  background: color-mix(in srgb, var(--n-warning-color, #c18401) 9%, transparent);
}

.rrm-banner.is-danger {
  color: var(--rrm-danger);
  background: color-mix(in srgb, var(--rrm-danger) 9%, transparent);
}

.rrm-banner-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rrm-state {
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

.rrm-state h2 {
  margin: 4px 0 0;
  font-size: 14px;
  font-weight: 700;
}

.rrm-state p {
  margin: 0;
  max-width: 380px;
  color: var(--rrm-fg-2);
  font-size: 12px;
  line-height: 1.6;
}

.rrm-state-glyph {
  display: grid;
  place-items: center;
  width: 58px;
  height: 58px;
  border-radius: 18px;
  color: var(--rrm-fg-3);
  background: var(--rrm-hover);
}

.rrm-state-glyph.is-quiet {
  opacity: 0.75;
}

.rrm-spinner {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--rrm-line);
  border-top-color: var(--rrm-accent);
  animation: rrm-spin 0.8s linear infinite;
}

@keyframes rrm-spin {
  to {
    transform: rotate(360deg);
  }
}

/* ───────────── 列表 ───────────── */

.rrm-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 6px 6px;
  scrollbar-width: thin;
}

.rrm-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 86px 92px 68px;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 30px;
  padding: 0 10px 0 calc(10px + var(--rrm-depth, 0) * 15px);
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--rrm-fg);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: default;
  user-select: none;
  position: relative;
}

.rrm-row:hover {
  background: var(--rrm-hover);
}

.rrm-row.is-selected {
  background: var(--rrm-sel);
}

.rrm-row.is-primary {
  background: var(--rrm-sel-strong);
}

/* 左侧竖线是「当前项」的唯一强提示：刷新时它跟着行走，不跳动。 */
.rrm-row.is-primary::before {
  content: '';
  position: absolute;
  left: 0;
  top: 5px;
  bottom: 5px;
  width: 2px;
  border-radius: 2px;
  background: var(--rrm-accent);
}

.rrm-row.is-header {
  position: sticky;
  top: 0;
  z-index: 2;
  height: 26px;
  color: var(--rrm-fg-3);
  font-size: 10.5px;
  letter-spacing: 0.04em;
  background: var(--rrm-bg);
  border-radius: 0;
  cursor: default;
}

.rrm-row.is-header:hover {
  background: var(--rrm-bg);
}

.rrm-row-main {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.rrm-chevron {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 14px;
  height: 14px;
  border-radius: 4px;
  color: var(--rrm-fg-3);
  cursor: pointer;
  transition: transform 0.14s ease;
}

.rrm-chevron:hover {
  background: var(--rrm-hover);
  color: var(--rrm-fg);
}

.rrm-chevron.is-open {
  transform: rotate(90deg);
}

.rrm-chevron.is-placeholder {
  cursor: default;
  pointer-events: none;
}

.rrm-type-icon {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 16px;
  height: 16px;
  color: var(--rrm-fg-2);
}

.rrm-type-icon[data-type='dir'] {
  color: var(--rrm-accent);
}

.rrm-type-icon[data-type='image'] {
  color: var(--rrm-tone-image);
}

.rrm-type-icon[data-type='data'] {
  color: var(--rrm-tone-data);
}

.rrm-type-icon[data-type='text'] {
  color: var(--rrm-tone-text);
}

.rrm-type-icon[data-type='document'] {
  color: var(--rrm-tone-document);
}

.rrm-type-icon[data-type='media'] {
  color: var(--rrm-tone-media);
}

.rrm-type-icon[data-type='archive'] {
  color: var(--rrm-tone-archive);
}

.rrm-row-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rrm-row.is-dir .rrm-row-name {
  font-weight: 600;
}

.rrm-row-parent {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rrm-fg-3);
  font-size: 11px;
}

.rrm-row-size,
.rrm-row-time,
.rrm-row-kind {
  color: var(--rrm-fg-2);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rrm-row-time,
.rrm-row-size {
  text-align: right;
}

.rrm-row-kind {
  color: var(--rrm-fg-3);
  font-size: 11px;
}

.rrm-tile-thumb {
  flex: 0 0 auto;
  width: 26px;
  height: 26px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid var(--rrm-line-soft);
}

/* 网格：瓦片而不是卡片。预览图是主体，信息压成一行，
   与列表共用同一套安静的排版层级。 */
.rrm-list.is-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(152px, 1fr));
  gap: 8px;
  align-content: start;
  padding: 10px;
}

.rrm-list.is-grid .rrm-row {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  height: auto;
  padding: 8px;
  border: 1px solid var(--rrm-line-soft);
  border-radius: 10px;
  background: var(--rrm-panel);
}

.rrm-list.is-grid .rrm-row:hover {
  border-color: var(--rrm-line);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.rrm-list.is-grid .rrm-row.is-primary {
  border-color: var(--rrm-accent);
  box-shadow: 0 0 0 1px var(--rrm-accent) inset;
}

.rrm-list.is-grid .rrm-row.is-primary::before {
  display: none;
}

.rrm-list.is-grid .rrm-row-main {
  flex: 0 0 auto;
  flex-wrap: nowrap;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
}

.rrm-list.is-grid .rrm-row-main > .rrm-chevron,
.rrm-list.is-grid .rrm-row-main > .rrm-row-parent {
  display: none;
}

.rrm-list.is-grid .rrm-row-main > .rrm-type-icon,
.rrm-list.is-grid .rrm-row-main > .rrm-tile-thumb {
  flex: 0 0 auto;
  order: -1;
  width: 100%;
  height: 86px;
  border-radius: 8px;
  background: var(--rrm-hover);
}

.rrm-list.is-grid .rrm-row-main > .rrm-type-icon {
  color: var(--rrm-fg-3);
}

.rrm-list.is-grid .rrm-row-main > .rrm-type-icon :deep(svg) {
  width: 30px;
  height: 30px;
}

.rrm-list.is-grid .rrm-row-name {
  font-size: 12px;
  white-space: normal;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-height: 1.35;
  word-break: break-all;
  max-height: 2.7em;
  overflow: hidden;
}

.rrm-list.is-grid .rrm-row-parent,
.rrm-list.is-grid .rrm-row-time,
.rrm-list.is-grid .rrm-row-kind {
  display: none;
}

.rrm-list.is-grid .rrm-row-size {
  flex: 0 0 auto;
  font-size: 10.5px;
  text-align: left;
  color: var(--rrm-fg-3);
}

/* ───────────── 状态栏 ───────────── */

.rrm-status {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 14px;
  border-top: 1px solid var(--rrm-line-soft);
  color: var(--rrm-fg-3);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.rrm-status-sep {
  opacity: 0.5;
}

.rrm-status-spacer {
  flex: 1;
}

.rrm-status-path {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 60%;
}

/* ───────────── 右侧预览 ───────────── */

.rrm-resizer {
  position: relative;
  width: 7px;
  cursor: col-resize;
  background: linear-gradient(
    to right,
    transparent 0,
    transparent 3px,
    var(--rrm-line) 3px,
    var(--rrm-line) 4px,
    transparent 4px
  );
}

.rrm-resizer:hover {
  background: linear-gradient(
    to right,
    transparent 1px,
    var(--rrm-accent) 1px,
    var(--rrm-accent) 3px,
    transparent 3px
  );
}

.rrm-inspector {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-left: 1px solid var(--rrm-line-soft);
  background: color-mix(in srgb, var(--rrm-panel) 62%, var(--rrm-bg));
}

.rrm-preview-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 38px;
  padding: 0 10px 0 14px;
  border-bottom: 1px solid var(--rrm-line-soft);
}

.rrm-preview-title {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.rrm-preview-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  font-weight: 600;
}

/* 预览区必须能被压扁：窗口很矮时（最小窗口 560 高减去标题栏与工具栏）
   若只给 min-height，舞台会把下面的元信息挤出去，图片直接盖在「位置/大小」上。 */
.rrm-preview-body {
  flex: 1 1 46%;
  min-height: 110px;
  padding: 10px 12px 4px;
  overflow: hidden;
}

.rrm-inspector-scroll {
  flex: 1 1 54%;
  flex-shrink: 1;
  min-height: 0;
  overflow: auto;
  padding: 6px 12px 14px;
  scrollbar-width: thin;
}

.rrm-meta {
  display: grid;
  gap: 1px;
  padding: 4px 0 10px;
  border-bottom: 1px solid var(--rrm-line-soft);
}

.rrm-meta-row {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  gap: 8px;
  padding: 4px 0;
  font-size: 11.5px;
}

.rrm-meta-key {
  color: var(--rrm-fg-3);
}

.rrm-meta-value {
  min-width: 0;
  color: var(--rrm-fg);
  word-break: break-word;
}

.rrm-meta-value.is-num {
  font-variant-numeric: tabular-nums;
}

.rrm-meta-weak {
  color: var(--rrm-fg-3);
}

.rrm-meta-path {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rrm-fg-2);
  font: inherit;
  text-align: left;
  text-decoration: underline dotted;
  cursor: pointer;
  overflow-wrap: anywhere;
}

.rrm-meta-path:hover {
  color: var(--rrm-accent);
}

.rrm-section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin: 14px 0 8px;
  color: var(--rrm-fg-3);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.05em;
}

.rrm-usage {
  padding-bottom: 8px;
  border-bottom: 1px solid var(--rrm-line-soft);
}

.rrm-usage-item {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  height: 26px;
  padding: 0 6px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--rrm-fg-2);
  font: inherit;
  font-size: 11.5px;
  text-align: left;
  cursor: pointer;
}

.rrm-usage-item:hover {
  background: var(--rrm-hover);
  color: var(--rrm-fg);
}

.rrm-usage-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rrm-usage-type {
  flex: 0 0 auto;
  color: var(--rrm-fg-3);
  font-size: 10.5px;
}

.rrm-more {
  margin: 4px 6px 0;
  color: var(--rrm-fg-3);
  font-size: 10.5px;
}

.rrm-actions {
  padding-bottom: 6px;
}

.rrm-action-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.rrm-action {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 9px;
  border: 1px solid var(--rrm-line);
  border-radius: 7px;
  background: var(--rrm-panel);
  color: var(--rrm-fg);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
}

.rrm-action:hover:not(:disabled) {
  background: var(--rrm-hover);
}

.rrm-action:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.rrm-action.is-wide {
  grid-column: 1 / -1;
}

.rrm-action[data-kind='image'] {
  color: var(--rrm-tone-image);
  border-color: color-mix(in srgb, var(--rrm-tone-image) 34%, var(--rrm-line));
}

.rrm-action[data-kind='data'] {
  color: var(--rrm-tone-data);
  border-color: color-mix(in srgb, var(--rrm-tone-data) 34%, var(--rrm-line));
}

.rrm-action.is-danger {
  color: var(--rrm-danger);
  border-color: color-mix(in srgb, var(--rrm-danger) 30%, var(--rrm-line));
}

.rrm-action.is-danger:hover {
  background: color-mix(in srgb, var(--rrm-danger) 10%, transparent);
}

.rrm-inspector-empty {
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

.rrm-inspector-empty h2 {
  margin: 2px 0 0;
  font-size: 13px;
  font-weight: 700;
}

.rrm-inspector-empty p {
  margin: 0;
  max-width: 300px;
  color: var(--rrm-fg-2);
  font-size: 11.5px;
  line-height: 1.65;
}

.rrm-repo-facts {
  width: 100%;
  max-width: 300px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1px;
  margin: 8px 0 2px;
  border: 1px solid var(--rrm-line-soft);
  border-radius: 9px;
  overflow: hidden;
}

.rrm-repo-facts > div {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 10px;
  background: var(--rrm-panel);
}

.rrm-repo-facts dt {
  color: var(--rrm-fg-3);
  font-size: 10.5px;
}

.rrm-repo-facts dd {
  margin: 0;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.rrm-sync-detail {
  border-top: 1px solid var(--rrm-line-soft);
  margin-top: 8px;
}

.rrm-sync-title {
  font-size: 12px;
  font-weight: 600;
}

.rrm-sync-line {
  margin: 3px 0 8px;
  color: var(--rrm-fg-2);
  font-size: 11px;
  line-height: 1.55;
  word-break: break-word;
}

.rrm-sync-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--rrm-fg-3);
  font-size: 10.5px;
}

.rrm-sync-provider {
  padding: 0 6px;
  border: 1px solid var(--rrm-line);
  border-radius: 999px;
}

.rrm-sync-bar {
  height: 4px;
  margin: 6px 0 8px;
  border-radius: 999px;
  background: var(--rrm-hover);
  overflow: hidden;
}

.rrm-sync-bar-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--rrm-accent);
  transition: width 0.25s ease;
}

.rrm-sync-bar-fill[data-status='error'] {
  background: var(--rrm-danger);
}

.rrm-sync-bar-fill[data-status='success'] {
  background: var(--n-success-color, #50a14f);
}

.rrm-sync-facts {
  display: grid;
  gap: 3px;
  margin: 0;
  font-size: 10.5px;
}

.rrm-sync-facts > div {
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr);
  gap: 8px;
}

.rrm-sync-facts dt {
  color: var(--rrm-fg-3);
}

.rrm-sync-facts dd {
  margin: 0;
  color: var(--rrm-fg-2);
  word-break: break-word;
}

.rrm-ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rrm-link-btn {
  border: 0;
  background: none;
  padding: 0;
  color: var(--rrm-accent);
  font: inherit;
  font-size: 10.5px;
  cursor: pointer;
}

/* ───────────── 拖放 ───────────── */

.rrm.is-dropping::after {
  content: '';
  position: fixed;
  inset: 0;
  z-index: 40;
  pointer-events: none;
  border: 2px dashed var(--rrm-accent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--rrm-accent) 7%, transparent);
}

.rrm-drop {
  position: fixed;
  inset: 0;
  z-index: 41;
  display: grid;
  place-items: center;
  pointer-events: none;
}

.rrm-drop-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 22px 30px;
  border-radius: 14px;
  color: var(--rrm-fg);
  background: var(--rrm-panel);
  border: 1px solid var(--rrm-line);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.22);
}

.rrm-drop-title {
  font-size: 13px;
  font-weight: 700;
}

.rrm-drop-hint {
  color: var(--rrm-fg-2);
  font-size: 11px;
}

/* ───────────── 弹窗 ───────────── */

.rrm-modal {
  width: min(440px, calc(100vw - 40px));
}

.rrm-modal-body {
  display: grid;
  gap: 10px;
}

.rrm-modal-hint {
  margin: 0;
  color: var(--rrm-fg-2);
  font-size: 12px;
  word-break: break-all;
}

.rrm-modal-danger {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.rrm-modal-warn {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin: 0;
  padding: 7px 9px;
  border-radius: 7px;
  color: var(--n-warning-color, #c18401);
  background: color-mix(in srgb, var(--n-warning-color, #c18401) 10%, transparent);
  font-size: 11.5px;
  line-height: 1.5;
}

.rrm-text-input {
  width: 100%;
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--rrm-line);
  border-radius: 8px;
  background: var(--rrm-bg);
  color: var(--rrm-fg);
  font: inherit;
  font-size: 12.5px;
  outline: none;
}

.rrm-text-input:focus {
  border-color: var(--rrm-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--rrm-accent) 16%, transparent);
}

.rrm-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

body.resource-inspector-resizing {
  cursor: col-resize;
  user-select: none;
}

/* ───────────── 窄窗口 ───────────── */

/* 侧栏收窄时先让出「类型」「修改时间」两列：名字本来就是最需要读的一列。
   列数减少后 grid 会自动把剩余空间给名称列，不需要另写模板。 */
@media (max-width: 1180px) {
  .rrm-row,
  .rrm-row.is-header {
    grid-template-columns: minmax(0, 1fr) 82px 78px;
  }

  .rrm-row-kind {
    display: none;
  }
}

@media (max-width: 980px) {
  .rrm-row,
  .rrm-row.is-header {
    grid-template-columns: minmax(0, 1fr) 74px;
  }

  .rrm-row-time {
    display: none;
  }
}

@media (max-width: 1080px) {
  .rrm-toolbar {
    flex-wrap: wrap;
    gap: 6px 12px;
    padding-bottom: 6px;
  }

  .rrm-filters {
    order: 3;
    flex-basis: 100%;
    overflow-x: auto;
  }

  .rrm-header-actions .rrm-btn span {
    display: none;
  }

  .rrm-header-actions .rrm-btn {
    width: 28px;
    padding: 0;
    justify-content: center;
  }
}

@media (max-width: 780px) {
  .rrm-main {
    grid-template-columns: minmax(0, 1fr);
  }

  .rrm-resizer,
  .rrm-inspector {
    display: none;
  }

  .rrm-search {
    min-width: 120px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .rrm-chevron,
  .rrm-search,
  .rrm-sync-bar-fill {
    transition: none;
  }

  .rrm-spinner,
  .rrm-icon-btn.is-busy :deep(svg) {
    animation-duration: 2.4s;
  }
}
</style>

