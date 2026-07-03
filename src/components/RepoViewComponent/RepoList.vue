<template>
  <div class="repo-sidebar">
    <div class="repo-header">
      <n-text class="repo-title">知识库</n-text>
      <button
        v-if="store.currentDatabase"
        class="repo-header-del"
        title="删除当前文件库（高风险）"
        @click="openDeleteModal(store.currentDatabase)"
      >
        <n-icon :component="Delete20Regular" :size="18" />
      </button>
    </div>

    <!-- 知识库列表（自定义条目：点击切换、悬停/右键删除） -->
    <div class="repo-menu">
      <div
        v-for="repo in store.repos"
        :key="repo"
        class="repo-item"
        :class="{ active: repo === store.currentDatabase }"
        @click="selectRepo(repo)"
        @contextmenu.prevent="openCtx($event, repo)"
      >
        <n-icon class="repo-item-icon" :component="BookmarksOutline" />
        <span class="repo-item-label" :title="repo">{{ repo }}</span>
        <button class="repo-item-del" title="删除该文件库" @click.stop="openDeleteModal(repo)">
          <n-icon :component="Delete20Regular" :size="16" />
        </button>
      </div>
      <div v-if="!store.repos.length" class="repo-empty">暂无知识库</div>
    </div>

    <!-- 右键菜单 -->
    <template v-if="ctx.open">
      <div class="repo-ctx-backdrop" @click="closeCtx" @contextmenu.prevent="closeCtx" />
      <div class="repo-ctx" :style="{ left: ctx.x + 'px', top: ctx.y + 'px' }">
        <button class="repo-ctx-item danger" @click="ctxDelete">
          <svg viewBox="0 0 20 20" width="14" height="14" aria-hidden="true">
            <path fill="currentColor" d="M8.5 4h3a1.5 1.5 0 0 0-3 0zm-1 0a2.5 2.5 0 0 1 5 0h5a.5.5 0 0 1 0 1h-1v9.5a2.5 2.5 0 0 1-2.5 2.5h-8A2.5 2.5 0 0 1 3.5 14.5V5h-1a.5.5 0 0 1 0-1h5zm2 4.5a.5.5 0 0 0-1 0v5a.5.5 0 0 0 1 0v-5zm3 0a.5.5 0 0 0-1 0v5a.5.5 0 0 0 1 0v-5z" />
          </svg>
          删除文件库
        </button>
      </div>
    </template>

    <!-- 高风险删除确认弹窗（输入库名以确认） -->
    <Teleport to="body">
      <div v-if="del.open" class="del-mask" @mousedown.self="closeDeleteModal">
        <div class="del-dialog" role="alertdialog" aria-modal="true">
          <div class="del-band">
            <span class="del-band-icon">⚠</span>
            <span class="del-band-text">高风险 · 不可恢复</span>
          </div>
          <div class="del-body">
            <h3 class="del-title">删除文件库「{{ del.repo }}」？</h3>
            <p class="del-desc">
              该操作将<b>永久删除</b>此文件库中的全部书籍 / 文档及其对应笔记，
              并一并移除磁盘上的原始文件。删除后<b>无法撤销</b>。
            </p>
            <div class="del-stat">
              <span class="del-stat-num">{{ del.count == null ? '…' : del.count }}</span>
              <span class="del-stat-unit">份文档将被删除</span>
            </div>
            <label class="del-confirm-label">
              请输入文件库名称 <code>{{ del.repo }}</code> 以确认：
              <input
                v-model="del.confirmText"
                class="del-input"
                :placeholder="del.repo"
                spellcheck="false"
                autocomplete="off"
                @keyup.enter="confirmDelete"
              />
            </label>
          </div>
          <div class="del-actions">
            <button class="del-btn ghost" :disabled="del.busy" @click="closeDeleteModal">取消</button>
            <button class="del-btn danger" :disabled="!canConfirmDelete" @click="confirmDelete">
              {{ del.busy ? '删除中…' : '永久删除' }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup>
import { BookmarksOutline } from '@vicons/ionicons5'
import { Delete20Regular } from '@vicons/fluent'
import { NIcon, NText, useMessage } from 'naive-ui'
import { reactive, computed, onMounted, onBeforeUnmount } from 'vue'
import { repoStore } from '../../stores/repoStore'
import { invoke } from '@tauri-apps/api/core'
import { platform } from '@tauri-apps/plugin-os'
import { createLogger } from '../../utils/logger'

const logger = createLogger('RepoList')
const message = useMessage()
const store = repoStore()

// ── 切换当前文件库 ──
const selectRepo = (repo) => {
  if (repo === store.currentDatabase) return
  store.currentDatabase = repo
  store.fetchFiles(store.currentDatabase)
}

// ── 右键菜单 ──
const ctx = reactive({ open: false, x: 0, y: 0, repo: '' })
const openCtx = (e, repo) => {
  ctx.repo = repo
  // 防止菜单超出右/下边界
  ctx.x = Math.min(e.clientX, window.innerWidth - 168)
  ctx.y = Math.min(e.clientY, window.innerHeight - 60)
  ctx.open = true
}
const closeCtx = () => { ctx.open = false }
const ctxDelete = () => {
  const repo = ctx.repo
  closeCtx()
  openDeleteModal(repo)
}

// ── 高风险删除弹窗 ──
const del = reactive({ open: false, repo: '', count: null, confirmText: '', busy: false })
const canConfirmDelete = computed(
  () => !del.busy && del.confirmText.trim() === del.repo && !!del.repo
)

const openDeleteModal = async (repo) => {
  if (!repo) return
  del.repo = repo
  del.confirmText = ''
  del.count = null
  del.busy = false
  del.open = true
  try {
    const files = await invoke('get_imported_files', { knowledgeRepo: repo })
    del.count = Array.isArray(files) ? files.length : 0
  } catch (e) {
    logger.warn('统计文件库文档数失败', e)
    del.count = null
  }
}
const closeDeleteModal = () => {
  if (del.busy) return
  del.open = false
}

// 逐份删除该文件库下的文档（连带磁盘目录与笔记）；目录缺失时退回仅软删数据库记录。
const performDeleteLibrary = async (repo) => {
  const files = await invoke('get_imported_files', { knowledgeRepo: repo })
  const list = Array.isArray(files) ? files : []
  if (!store.librariesRoot) await store.get_librariesRoot()
  const os = await platform()
  const sep = os === 'windows' ? '\\' : '/'

  for (const f of list) {
    const name = f.import_file_name || ''
    const base = name.includes('.') ? name.slice(0, name.lastIndexOf('.')) : name
    const dir = [store.librariesRoot, 'Libraries', f.library, base].join(sep)
    try {
      await invoke('delete_imported_all_file', { id: f.id, path: dir })
    } catch (e) {
      logger.warn('整库删除：磁盘目录缺失，退回软删记录', { id: f.id, dir, err: String(e?.message || e) })
      try {
        await invoke('delete_imported_file', { id: f.id })
      } catch (e2) {
        logger.error('整库删除：软删记录仍失败', { id: f.id, err: String(e2?.message || e2) })
        throw e2
      }
    }
  }
}

const confirmDelete = async () => {
  if (!canConfirmDelete.value) return
  const repo = del.repo
  del.busy = true
  try {
    await performDeleteLibrary(repo)
    await store.fetchRepos()

    // 当前库被删或已不存在：切换到剩余的第一个库
    if (store.currentDatabase === repo || !store.repos.includes(store.currentDatabase)) {
      store.currentDatabase = store.repos[0] || ''
      if (store.currentDatabase) await store.fetchFiles(store.currentDatabase)
      else store.files = []
    }

    del.open = false
    message.success(`文件库「${repo}」已删除`)
  } catch (e) {
    logger.error('删除文件库失败', e)
    message.error('删除文件库失败：' + (e?.message || e))
  } finally {
    del.busy = false
  }
}

// Esc 关闭菜单/弹窗
const onKeydown = (e) => {
  if (e.key !== 'Escape') return
  if (del.open && !del.busy) del.open = false
  else if (ctx.open) ctx.open = false
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  await store.fetchRepos()
  if (store.repos.length > 0 && !store.currentDatabase) {
    store.currentDatabase = store.repos[0]
  }
  if (store.currentDatabase) await store.fetchFiles(store.currentDatabase)
})
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
.repo-sidebar {
  padding: 4px;
  background-color: var(--guaxi-panel-bg);
  height: 100%;
  min-height: 0;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.repo-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--guaxi-border);
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.repo-title {
  flex: 1;
  font-size: 18px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
}

.repo-title::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 16px;
  background-color: var(--guaxi-text-primary);
  margin-right: 8px;
  border-radius: 2px;
}

.repo-header-del {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  transition: all 0.18s ease;
}
.repo-header-del:hover {
  color: #e5484d;
  background: rgba(229, 72, 77, 0.12);
  border-color: rgba(229, 72, 77, 0.4);
}

.repo-menu {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  padding: 8px 0;
  /* 更细、更柔和的滚动条（Firefox） */
  scrollbar-width: thin;
  scrollbar-color: rgba(148, 163, 184, 0.35) transparent;
}
/* 更细、更柔和的滚动条（WebKit / Chromium） */
.repo-menu::-webkit-scrollbar {
  width: 8px;
}
.repo-menu::-webkit-scrollbar-track {
  background: transparent;
}
.repo-menu::-webkit-scrollbar-thumb {
  background-color: rgba(148, 163, 184, 0.28);
  border-radius: 8px;
  border: 2px solid transparent;
  background-clip: content-box;
  transition: background-color 0.2s ease;
}
.repo-menu:hover::-webkit-scrollbar-thumb {
  background-color: rgba(148, 163, 184, 0.5);
}
.repo-menu::-webkit-scrollbar-thumb:hover {
  background-color: var(--n-color-primary, #14b8a6);
}
.repo-menu::-webkit-scrollbar-thumb:active {
  background-color: var(--n-color-primary, #14b8a6);
}

/* 列表条目 */
.repo-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 4px 8px;
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  color: var(--guaxi-text-primary);
  transition: background-color 0.2s ease, color 0.2s ease;
}
.repo-item:hover { background-color: var(--guaxi-header-bg); }
.repo-item.active {
  background-color: rgba(45, 212, 191, 0.12);
  color: var(--n-color-primary, #14b8a6);
  font-weight: 500;
}
.repo-item-icon {
  flex: 0 0 auto;
  font-size: 18px;
  color: var(--guaxi-text-secondary);
}
.repo-item.active .repo-item-icon { color: var(--n-color-primary, #14b8a6); }
.repo-item-label {
  flex: 1;
  min-width: 0;
  font-size: 15px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.repo-item-del {
  flex: 0 0 auto;
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s ease, color 0.15s ease, background-color 0.15s ease;
}
.repo-item:hover .repo-item-del { opacity: 1; }
.repo-item-del:hover {
  color: #e5484d;
  background: rgba(229, 72, 77, 0.14);
}
.repo-empty {
  padding: 16px;
  text-align: center;
  font-size: 13px;
  color: var(--guaxi-text-secondary);
}

/* 右键菜单 */
.repo-ctx-backdrop {
  position: fixed;
  inset: 0;
  z-index: 4000;
}
.repo-ctx {
  position: fixed;
  z-index: 4001;
  min-width: 156px;
  padding: 5px;
  border-radius: 9px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.24);
}
.repo-ctx-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  color: var(--guaxi-text-primary);
}
.repo-ctx-item.danger { color: #e5484d; }
.repo-ctx-item.danger:hover { background: rgba(229, 72, 77, 0.12); }

/* ── 高风险删除弹窗 ── */
.del-mask {
  position: fixed;
  inset: 0;
  z-index: 5000;
  background: rgba(10, 8, 6, 0.55);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.del-dialog {
  width: min(460px, 96vw);
  border-radius: 16px;
  overflow: hidden;
  background: var(--guaxi-panel-bg);
  border: 1px solid rgba(229, 72, 77, 0.4);
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.4), 0 0 0 1px rgba(229, 72, 77, 0.15);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, monospace;
  animation: del-pop 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.1) both;
}
@keyframes del-pop {
  from { opacity: 0; transform: translateY(12px) scale(0.97); }
  to { opacity: 1; transform: none; }
}
.del-band {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 20px;
  color: #fff;
  background: repeating-linear-gradient(45deg, #c1272d 0 14px, #a81f24 14px 28px);
  letter-spacing: 1px;
}
.del-band-icon { font-size: 18px; }
.del-band-text { font-size: 13px; font-weight: 600; }

.del-body { padding: 20px 22px 6px; }
.del-title {
  margin: 0 0 10px;
  font-size: 19px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
  word-break: break-word;
}
.del-desc {
  margin: 0 0 14px;
  font-size: 13.5px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
}
.del-desc b { color: #e5484d; }
.del-stat {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 12px 16px;
  margin-bottom: 16px;
  border-radius: 10px;
  background: rgba(229, 72, 77, 0.08);
  border: 1px solid rgba(229, 72, 77, 0.22);
}
.del-stat-num {
  font-size: 26px;
  font-weight: 800;
  color: #e5484d;
  line-height: 1;
}
.del-stat-unit { font-size: 13px; color: var(--guaxi-text-secondary); }

.del-confirm-label {
  display: block;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  line-height: 1.9;
}
.del-confirm-label code {
  padding: 1px 7px;
  border-radius: 5px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  font-weight: 700;
}
.del-input {
  width: 100%;
  box-sizing: border-box;
  margin-top: 4px;
  padding: 10px 12px;
  font-size: 14px;
  font-family: inherit;
  color: var(--guaxi-text-primary);
  background: var(--guaxi-content-bg, var(--guaxi-header-bg));
  border: 1px solid var(--guaxi-border);
  border-radius: 9px;
  outline: none;
  transition: border-color 0.18s ease, box-shadow 0.18s ease;
}
.del-input:focus {
  border-color: #e5484d;
  box-shadow: 0 0 0 3px rgba(229, 72, 77, 0.18);
}

.del-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 22px 20px;
}
.del-btn {
  height: 38px;
  padding: 0 18px;
  border-radius: 10px;
  font-size: 14px;
  font-family: inherit;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s ease;
}
.del-btn.ghost {
  background: transparent;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
}
.del-btn.ghost:hover:not(:disabled) { background: var(--guaxi-header-bg); }
.del-btn.danger {
  background: linear-gradient(135deg, #e5484d, #c1272d);
  border: 1px solid #c1272d;
  color: #fff;
  box-shadow: 0 4px 14px rgba(193, 39, 45, 0.36);
}
.del-btn.danger:hover:not(:disabled) { filter: brightness(1.06); }
.del-btn:disabled { opacity: 0.45; cursor: not-allowed; box-shadow: none; }

.repo-menu::-webkit-scrollbar { width: 8px; }
.repo-menu::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 150, 0.35);
  border-radius: 8px;
  border: 2px solid transparent;
  background-clip: padding-box;
}
</style>
