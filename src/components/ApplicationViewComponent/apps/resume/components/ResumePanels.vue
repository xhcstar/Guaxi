<template>
  <!-- 模板库 -->
  <div v-if="templateOpen" class="rs-scrim" @click.self="$emit('close-templates')">
    <div class="rs-modal" role="dialog" aria-modal="true">
      <div class="rs-modal-head">
        <div class="rs-grow">
          <h2>模板库</h2>
          <p>选择模板只改变排版与配色，简历内容与版本记录不受影响。</p>
        </div>
        <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" aria-label="关闭" @click="$emit('close-templates')">
          <Icon :icon="xIcon" width="15" height="15" />
        </button>
      </div>

      <div class="rs-modal-body">
        <div class="rs-tabs">
          <button type="button" class="rs-tab" :class="{ 'is-active': tab === 'templates' }" @click="tab = 'templates'">模板</button>
          <button type="button" class="rs-tab" :class="{ 'is-active': tab === 'vars' }" @click="tab = 'vars'">模板变量</button>
          <span class="rs-grow" />
          <button type="button" class="rs-btn rs-btn--sm" :disabled="importing" @click="importHtmlTemplate">
            <Icon :icon="uploadIcon" width="12" height="12" />
            导入 HTML 模板
          </button>
        </div>

        <div v-if="tab === 'templates'" class="rs-template-grid">
          <div
            v-for="template in store.allTemplates"
            :key="template.id"
            class="rs-template-card"
            :class="{ 'is-active': template.id === store.doc?.templateId }"
            role="button"
            tabindex="0"
            @click="apply(template)"
            @keydown.enter="apply(template)"
          >
            <div class="rs-thumb">
              <iframe
                v-if="thumbnails[template.id]"
                :srcdoc="thumbnails[template.id]"
                sandbox="allow-same-origin"
                tabindex="-1"
                title="模板缩略图"
              />
            </div>
            <div class="rs-template-info">
              <div class="rs-template-name">
                <span
                  v-if="renamingId !== template.id"
                  @dblclick.stop="startRename(template)"
                >{{ template.name }}</span>
                <input
                  v-else
                  v-model="renameText"
                  class="rs-input"
                  style="height: 24px; padding: 2px 6px; font-size: 12px;"
                  @click.stop
                  @keydown.enter.stop="commitRename(template)"
                  @keydown.esc.stop="renamingId = ''"
                  @blur="commitRename(template)"
                />
                <span class="rs-template-tag">{{ template.custom ? '自定义' : '内置' }}</span>
              </div>
              <div class="rs-template-desc">{{ template.description || '未填写说明' }}</div>
            </div>
            <span v-if="template.id === store.doc?.templateId" class="rs-template-badge">使用中</span>
            <div v-if="template.custom" class="rs-template-card-actions" @click.stop>
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--sm" title="重命名" @click="startRename(template)">
                <Icon :icon="pencilIcon" width="11" height="11" />
              </button>
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--sm rs-btn--danger" title="删除模板" @click="removeTemplate(template)">
                <Icon :icon="trashIcon" width="11" height="11" />
              </button>
            </div>
          </div>
        </div>

        <div v-else class="rs-vars" style="margin-top: 0;">
          <div class="rs-vars-body">
            <div v-for="row in variableRows" :key="row.key" class="rs-var-row">
              <code>{{ row.key }}</code>
              <span>{{ row.desc }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- 版本历史 -->
  <template v-if="versionOpen">
    <div class="rs-scrim" style="z-index: 205; background: rgba(24,20,15,.28);" @click.self="$emit('close-versions')" />
    <aside class="rs-drawer" role="dialog" aria-modal="true">
      <div class="rs-drawer-head">
        <div class="rs-grow">
          <h2>版本历史</h2>
          <p>自动版本保留最近 20 个，手动保存与钉住的版本会一直保留。</p>
        </div>
        <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" aria-label="关闭" @click="$emit('close-versions')">
          <Icon :icon="xIcon" width="15" height="15" />
        </button>
      </div>
      <div class="rs-drawer-body">
        <div style="display: flex; gap: 8px; margin-bottom: 2px;">
          <button type="button" class="rs-btn rs-btn--primary" style="flex: 1; justify-content: center;" @click="saveVersion">
            <Icon :icon="saveIcon" width="13" height="13" />
            保存当前版本
          </button>
          <button type="button" class="rs-btn" :disabled="store.versionsLoading" @click="store.loadVersions()">
            <Icon :icon="refreshIcon" width="13" height="13" />
          </button>
        </div>

        <article
          v-for="(version, index) in store.versions"
          :key="version.id"
          class="rs-version"
          :class="{ 'is-pinned': version.pinned }"
          :style="{ animationDelay: `${Math.min(index, 12) * 22}ms` }"
        >
          <div class="rs-version-main">
            <span class="rs-version-label">{{ version.label }}</span>
            <span class="rs-version-meta">
              <span class="rs-badge" :class="{ 'rs-badge--pin': version.pinned }">{{ version.pinned ? '已钉住' : (version.kind === 'auto' ? '自动' : '手动') }}</span>
              <span>{{ formatTime(version.createdAt) }}</span>
            </span>
          </div>
          <div class="rs-version-actions">
            <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="与当前版本对比" @click="openCompare(version)">
              <Icon :icon="compareIcon" width="12" height="12" />
            </button>
            <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="恢复到这个版本" @click="restore(version)">
              <Icon :icon="rotateIcon" width="12" height="12" />
            </button>
            <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" :title="version.pinned ? '取消钉住' : '钉住保留'" @click="store.pinVersion(version, !version.pinned)">
              <Icon :icon="version.pinned ? pinOffIcon : pinIcon" width="12" height="12" />
            </button>
            <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--danger" :disabled="version.pinned" title="删除版本" @click="removeVersion(version)">
              <Icon :icon="trashIcon" width="12" height="12" />
            </button>
          </div>
        </article>

        <div v-if="!store.versions.length" class="rs-drawer-empty">
          {{ store.versionsLoading ? '正在读取版本…' : '还没有保存过版本。按 Ctrl+S 或点上面的按钮保存一个。' }}
        </div>
      </div>
    </aside>
  </template>

  <!-- 版本对比 -->
  <div v-if="compareTarget" class="rs-scrim" style="z-index: 220;" @click.self="compareTarget = null">
    <div class="rs-modal rs-compare" role="dialog" aria-modal="true">
      <div class="rs-modal-head">
        <div class="rs-grow">
          <h2>版本对比</h2>
          <p>{{ compareTarget.label }} · {{ formatTime(compareTarget.createdAt) }} ↔ 当前编辑</p>
        </div>
        <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" aria-label="关闭" @click="compareTarget = null">
          <Icon :icon="xIcon" width="15" height="15" />
        </button>
      </div>

      <div class="rs-modal-body">
        <div v-if="diffRows.length" class="rs-diff-list">
          <div v-for="(row, index) in diffRows" :key="index" class="rs-diff-row" :data-tone="row.tone">
            <span class="rs-diff-label">{{ row.label }}</span>
            <span class="rs-diff-detail">{{ row.detail }}</span>
          </div>
        </div>
        <div v-else class="rs-diff-empty">这个版本与当前编辑内容一致</div>

        <div class="rs-compare-grid">
          <figure>
            <figcaption>该版本</figcaption>
            <div class="rs-compare-frame">
              <iframe :srcdoc="compareBeforeHtml" sandbox="allow-same-origin" title="历史版本预览" />
            </div>
          </figure>
          <figure>
            <figcaption>当前编辑</figcaption>
            <div class="rs-compare-frame">
              <iframe :srcdoc="compareCurrentHtml" sandbox="allow-same-origin" title="当前版本预览" />
            </div>
          </figure>
        </div>
      </div>

      <div class="rs-modal-foot">
        <span class="rs-grow" />
        <button type="button" class="rs-btn" @click="compareTarget = null">关闭</button>
        <button type="button" class="rs-btn rs-btn--primary" @click="restoreFromCompare">恢复该版本</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { createDiscreteApi } from 'naive-ui'
import xIcon from '@iconify-icons/lucide/x'
import uploadIcon from '@iconify-icons/lucide/upload'
import pencilIcon from '@iconify-icons/lucide/pencil'
import trashIcon from '@iconify-icons/lucide/trash-2'
import saveIcon from '@iconify-icons/lucide/save'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import rotateIcon from '@iconify-icons/lucide/rotate-ccw'
import pinIcon from '@iconify-icons/lucide/pin'
import pinOffIcon from '@iconify-icons/lucide/pin-off'
import compareIcon from '@iconify-icons/lucide/git-compare'
import { useResumeStore, formatTime } from '../store/resumeStore'
import { renderResumeHtml, stripScriptsForPreview } from '../render/renderResumeHtml'
import { askConfirm } from '../ui'

const props = defineProps({
  templateOpen: { type: Boolean, default: false },
  versionOpen: { type: Boolean, default: false },
})

defineEmits(['close-templates', 'close-versions'])

const { message } = createDiscreteApi(['message'])
const store = useResumeStore()
const tab = ref('templates')
const thumbnails = ref({})
const importing = ref(false)
const renamingId = ref('')
const renameText = ref('')
const compareTarget = ref(null)

const compareBeforeHtml = computed(() => {
  if (!compareTarget.value || !store.doc) return ''
  try {
    const data = JSON.parse(compareTarget.value.dataJson || '{}')
    return stripScriptsForPreview(renderResumeHtml({ title: store.doc.title, data }, store.activeTemplateHtml))
  } catch {
    return ''
  }
})

const compareCurrentHtml = computed(() => (
  store.doc ? stripScriptsForPreview(renderResumeHtml(store.doc, store.activeTemplateHtml)) : ''
))

const diffRows = computed(() => {
  if (!compareTarget.value || !store.doc) return []
  try {
    const before = JSON.parse(compareTarget.value.dataJson || '{}')
    return buildDocumentDiff(before, store.doc.data)
  } catch {
    return []
  }
})

const PROFILE_LABELS = {
  name: '姓名',
  title: '职位标题',
  city: '城市',
  phone: '联系方式',
  email: '电子邮箱',
  website: '个人主页',
  target: '求职意向',
  summary: '个人优势',
  avatar: '头像',
}

function buildDocumentDiff(before = {}, after = {}) {
  const rows = []
  const beforeProfile = before.profile || {}
  const afterProfile = after.profile || {}
  const changedProfile = Object.keys(PROFILE_LABELS)
    .filter((key) => String(beforeProfile[key] || '') !== String(afterProfile[key] || ''))
  if (changedProfile.length) {
    rows.push({
      label: '基本信息',
      detail: changedProfile.map((key) => PROFILE_LABELS[key]).join('、'),
      tone: 'changed',
    })
  }

  const beforeSections = Array.isArray(before.sections) ? before.sections : []
  const afterSections = Array.isArray(after.sections) ? after.sections : []
  const beforeMap = new Map(beforeSections.map((section) => [section.id, section]))
  const afterMap = new Map(afterSections.map((section) => [section.id, section]))

  for (const section of afterSections) {
    const old = beforeMap.get(section.id)
    if (!old) {
      rows.push({ label: section.title || '模块', detail: '新增模块', tone: 'added' })
      continue
    }
    if (old.title !== section.title) {
      rows.push({ label: section.title || '模块', detail: '重命名模块', tone: 'changed' })
    }
    if (old.visible !== section.visible) {
      rows.push({ label: section.title || '模块', detail: section.visible ? '改为显示' : '改为隐藏', tone: 'changed' })
    }
    const oldItems = new Map((old.items || []).map((item) => [item.id, item]))
    const newItems = section.items || []
    let added = 0
    let changed = 0
    let removed = 0
    for (const item of newItems) {
      const previous = oldItems.get(item.id)
      if (!previous) added += 1
      else if (JSON.stringify(previous) !== JSON.stringify(item)) changed += 1
    }
    for (const item of old.items || []) {
      if (!newItems.some((entry) => entry.id === item.id)) removed += 1
    }
    if (added || changed || removed) {
      const parts = []
      if (added) parts.push(`新增 ${added} 条`)
      if (changed) parts.push(`修改 ${changed} 条`)
      if (removed) parts.push(`删除 ${removed} 条`)
      rows.push({ label: section.title || '模块', detail: parts.join(' · '), tone: 'changed' })
    }
  }

  for (const section of beforeSections) {
    if (!afterMap.has(section.id)) {
      rows.push({ label: section.title || '模块', detail: '删除模块', tone: 'removed' })
    }
  }
  return rows
}

function openCompare(version) {
  if (!store.doc) return
  compareTarget.value = version
}

async function restoreFromCompare() {
  const version = compareTarget.value
  if (!version) return
  compareTarget.value = null
  await restore(version)
}

const variableRows = [
  { key: '{{ profile.name }}', desc: '姓名' },
  { key: '{{ profile.title }}', desc: '职位标题' },
  { key: '{{ profile.target }}', desc: '求职意向' },
  { key: '{{ profile.summary }}', desc: '个人优势（纯文本）' },
  { key: '{{ profile.avatar }}', desc: '头像地址' },
  { key: '{{ contactItemsHtml }}', desc: '联系方式列表' },
  { key: '{{ profileTitleBlock }}', desc: '职位行（已排版）' },
  { key: '{{ targetBlock }}', desc: '求职意向行' },
  { key: '{{ avatarBlock }}', desc: '头像区域' },
  { key: '{{ summarySection }}', desc: '个人优势整段' },
  { key: '{{ sectionsHtml }}', desc: '全部模块（按大纲顺序）' },
  { key: '{{ educationSection }}', desc: '教育经历模块' },
  { key: '{{ worksSection }}', desc: '工作经历模块' },
  { key: '{{ projectsSection }}', desc: '项目经历模块' },
  { key: '{{ skillsSection }}', desc: '专业技能模块' },
  { key: '{{ customSections }}', desc: '自定义模块' },
]

function buildThumbnails() {
  if (!store.doc) return
  const next = {}
  for (const template of store.allTemplates) {
    next[template.id] = stripScriptsForPreview(renderResumeHtml(store.doc, template.html))
  }
  thumbnails.value = next
}

watch(() => props.templateOpen, (openNow) => {
  if (openNow) {
    tab.value = 'templates'
    renamingId.value = ''
    buildThumbnails()
  }
})

watch(() => store.activeTemplateHtml, () => {
  if (props.templateOpen) buildThumbnails()
})

watch(() => props.versionOpen, (openNow) => {
  if (openNow) store.loadVersions()
})

watch(() => store.activeDocId, () => {
  compareTarget.value = null
})

async function apply(template) {
  await store.applyTemplate(template.id)
  buildThumbnails()
}

function startRename(template) {
  if (!template.custom) return
  renamingId.value = template.id
  renameText.value = template.name
}

async function commitRename(template) {
  if (renamingId.value !== template.id) return
  const name = renameText.value.trim()
  renamingId.value = ''
  if (!name || name === template.name) return
  await store.saveTemplate({
    id: template.id,
    name,
    description: template.description,
    html: template.html,
  })
}

async function removeTemplate(template) {
  const ok = await askConfirm({
    title: '删除这个模板？',
    body: `「${template.name}」删除后，正在使用它的简历会回到默认模板。`,
    confirmText: '删除模板',
    danger: true,
  })
  if (!ok) return
  await store.deleteTemplate(template.id)
  buildThumbnails()
}

async function importHtmlTemplate() {
  importing.value = true
  try {
    const path = await open({
      title: '导入 HTML 模板',
      multiple: false,
      filters: [{ name: 'HTML 模板', extensions: ['html', 'htm'] }],
    })
    if (!path) return
    const raw = await invoke('read_file_content', { path: String(path) })
    const base = String(path).split(/[\\/]/).pop().replace(/\.html?$/i, '')
    await store.saveTemplate({
      name: base || '导入的模板',
      description: '从 HTML 文件导入',
      html: String(raw),
    })
    buildThumbnails()
    message.success('模板已导入')
  } catch (error) {
    message.error('导入模板失败')
  } finally {
    importing.value = false
  }
}

async function saveVersion() {
  await store.createVersion(`手动保存 · ${formatTime(new Date())}`, 'manual')
  message.success('已保存版本')
}

async function restore(version) {
  const ok = await askConfirm({
    title: '恢复到这个版本？',
    body: `当前内容会先自动备份为「恢复前备份」，再切换到「${version.label}」。`,
    confirmText: '恢复',
  })
  if (!ok) return
  await store.restoreVersion(version)
}

async function removeVersion(version) {
  const ok = await askConfirm({
    title: '删除这个版本？',
    body: `「${version.label}」删除后无法恢复。`,
    confirmText: '删除版本',
    danger: true,
  })
  if (!ok) return
  await store.deleteVersion(version)
}
</script>

<style scoped>
.rs-template-card { position: relative; }
.rs-template-card-actions {
  position: absolute;
  top: 6px;
  left: 6px;
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity .12s ease;
}
.rs-template-card:hover .rs-template-card-actions { opacity: 1; }
.rs-template-card-actions .rs-btn {
  background: color-mix(in srgb, var(--rs-surface) 88%, transparent);
  border: 1px solid var(--rs-line);
}
</style>
