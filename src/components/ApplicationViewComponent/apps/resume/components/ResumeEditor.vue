<template>
  <div class="rs-editor">
    <div v-if="!store.doc" class="rs-empty">
      <div class="rs-empty-mark" />
      <h3>还没有打开的简历</h3>
      <p>从左侧新建一份，或导入之前导出的项目文件。所有内容都会自动保存在本机。</p>
      <button type="button" class="rs-btn rs-btn--primary" @click="store.createDocument({})">新建一份简历</button>
    </div>

    <div v-else class="rs-editor-inner">
      <div class="rs-editor-toolbar">
        <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" :disabled="!store.canUndo" title="撤销（Ctrl+Z）" aria-label="撤销" @click="store.undo()">
          <Icon :icon="undoIcon" width="14" height="14" />
        </button>
        <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" :disabled="!store.canRedo" title="重做（Ctrl+Shift+Z）" aria-label="重做" @click="store.redo()">
          <Icon :icon="redoIcon" width="14" height="14" />
        </button>

        <div class="rs-completion-wrap">
          <button type="button" class="rs-btn" :class="{ 'is-on': completionOpen }" @click="completionOpen = !completionOpen">
            <span class="rs-progress" :style="{ '--p': store.completion.percent }"><span /></span>
            完成度 {{ store.completion.percent }}%
          </button>
          <div v-if="completionOpen" class="rs-menu rs-completion-menu" @click.stop>
            <div class="rs-completion-title">还差这些内容</div>
            <div
              v-for="item in store.completion.items"
              :key="item.key"
              class="rs-completion-row"
              :class="{ done: item.done }"
              role="button"
              tabindex="0"
              @click="focusCompletion(item)"
              @keydown.enter="focusCompletion(item)"
            >
              <Icon :icon="item.done ? checkIcon : dotIcon" width="12" height="12" />
              <span>{{ item.label }}</span>
              <span class="rs-completion-flag">{{ item.done ? '已填' : '去补' }}</span>
            </div>
          </div>
        </div>

        <span class="rs-grow" />

        <button type="button" class="rs-btn" :class="{ 'is-on': mode === 'template' }" @click="toggleMode">
          <Icon :icon="codeIcon" width="14" height="14" />
          {{ mode === 'template' ? '返回内容编辑' : '模板源码' }}
        </button>
      </div>

      <template v-if="mode === 'form'">
        <section class="rs-card rs-profile">
          <div class="rs-profile-main">
            <div class="rs-name-row">
              <input v-model="store.doc.data.profile.name" data-field="name" class="rs-name-input" placeholder="你的姓名" />
            </div>
            <div class="rs-form-grid">
              <label class="rs-field">
                <span class="rs-label">职位标题</span>
                <input v-model="store.doc.data.profile.title" class="rs-input" placeholder="嵌入式软件工程师" />
              </label>
              <label class="rs-field">
                <span class="rs-label">求职意向</span>
                <input v-model="store.doc.data.profile.target" data-field="target" class="rs-input" placeholder="Linux C 开发 / 嵌入式开发" />
              </label>
              <label class="rs-field">
                <span class="rs-label">联系方式</span>
                <input v-model="store.doc.data.profile.phone" data-field="phone" class="rs-input" placeholder="138-0000-0000" />
              </label>
              <label class="rs-field">
                <span class="rs-label">电子邮箱</span>
                <input v-model="store.doc.data.profile.email" class="rs-input" placeholder="name@example.com" />
              </label>
              <label class="rs-field">
                <span class="rs-label">城市</span>
                <input v-model="store.doc.data.profile.city" class="rs-input" placeholder="所在城市" />
              </label>
              <label class="rs-field">
                <span class="rs-label">个人主页</span>
                <input v-model="store.doc.data.profile.website" class="rs-input" placeholder="github.com/you" />
              </label>
              <label class="rs-field rs-field--wide">
                <span class="rs-label">个人优势 / 职业摘要</span>
                <textarea v-model="store.doc.data.profile.summary" data-field="summary" class="rs-textarea" rows="3" placeholder="两到三句话说明你的经验与擅长的方向" />
              </label>
            </div>
          </div>

          <div class="rs-avatar-box">
            <button type="button" class="rs-avatar" title="上传头像" aria-label="上传头像" @click="pickAvatar">
              <img v-if="store.doc.data.profile.avatar" :src="store.doc.data.profile.avatar" alt="头像" />
              <Icon v-else :icon="imageIcon" width="20" height="20" />
            </button>
            <div class="rs-avatar-actions">
              <button v-if="store.doc.data.profile.avatar" type="button" class="rs-btn rs-btn--ghost rs-btn--sm" @click="store.doc.data.profile.avatar = ''">移除</button>
              <span v-else class="rs-eyebrow">头像</span>
            </div>
            <input ref="avatarInput" type="file" accept="image/*" hidden @change="onAvatarPicked" />
          </div>
        </section>

        <section
          v-for="(section, sIndex) in store.doc.data.sections"
          :key="section.id"
          class="rs-card rs-section"
          :class="{ 'is-hidden': !section.visible, 'is-drop': dragOverIndex === sIndex }"
          :data-section-type="section.type"
          @dragover.prevent="dragOverIndex = sIndex"
          @drop="onDropSection(sIndex)"
        >
          <header class="rs-section-head">
            <span class="rs-index">{{ String(sIndex + 1).padStart(2, '0') }}</span>
            <span v-if="!section.visible" class="rs-hidden-flag">已隐藏</span>
            <Icon
              :icon="gripIcon"
              width="13"
              height="13"
              class="rs-grip"
              draggable="true"
              title="拖动调整顺序"
              @dragstart.stop="onDragStart(sIndex, $event)"
              @dragend="dragIndex = -1; dragOverIndex = -1"
            />
            <input v-model="section.title" class="rs-section-title" :placeholder="sectionTypeLabel(section.type)" />
            <div class="rs-section-tools">
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" :title="section.visible ? '隐藏此模块' : '显示此模块'" @click="section.visible = !section.visible">
                <Icon :icon="section.visible ? eyeIcon : eyeOffIcon" width="13" height="13" />
              </button>
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="上移" :disabled="sIndex === 0" @click="store.moveSection(sIndex, -1)">
                <Icon :icon="chevronUpIcon" width="13" height="13" />
              </button>
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="下移" :disabled="sIndex === store.doc.data.sections.length - 1" @click="store.moveSection(sIndex, 1)">
                <Icon :icon="chevronDownIcon" width="13" height="13" />
              </button>
              <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--danger" title="删除模块" @click="removeSection(sIndex)">
                <Icon :icon="trashIcon" width="13" height="13" />
              </button>
            </div>
          </header>

          <div v-if="section.type === 'skills'" class="rs-items">
            <div class="rs-item">
              <div class="rs-field">
                <span class="rs-label">技能清单</span>
                <textarea
                  class="rs-textarea"
                  rows="5"
                  :value="section.items.map((item) => item.text).join('\n')"
                  placeholder="每行一条技能"
                  @input="setSkillLines(section, $event.target.value)"
                />
                <span class="rs-lines-hint">{{ section.items.length }} 条 · 每行一条</span>
              </div>
            </div>
          </div>

          <div v-else class="rs-items">
            <article v-for="(item, iIndex) in section.items" :key="item.id" class="rs-item">
              <div class="rs-item-head">
                <span class="rs-item-title">{{ itemTitle(section.type, item) }}</span>
                <div class="rs-item-tools">
                  <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="上移" :disabled="iIndex === 0" @click="store.moveItem(section, iIndex, -1)">
                    <Icon :icon="chevronUpIcon" width="12" height="12" />
                  </button>
                  <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="下移" :disabled="iIndex === section.items.length - 1" @click="store.moveItem(section, iIndex, 1)">
                    <Icon :icon="chevronDownIcon" width="12" height="12" />
                  </button>
                  <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon" title="复制这一条" @click="store.duplicateItem(section, iIndex)">
                    <Icon :icon="copyIcon" width="12" height="12" />
                  </button>
                  <button type="button" class="rs-btn rs-btn--ghost rs-btn--icon rs-btn--danger" title="删除这一条" @click="removeItem(section, iIndex)">
                    <Icon :icon="trashIcon" width="12" height="12" />
                  </button>
                </div>
              </div>

              <div class="rs-form-grid">
                <label
                  v-for="field in fieldsOf(section.type)"
                  :key="field.key"
                  class="rs-field"
                  :class="{ 'rs-field--wide': field.span === 2 }"
                >
                  <span class="rs-label">{{ field.label }}</span>
                  <textarea
                    v-if="field.type === 'lines'"
                    v-model="item[field.key]"
                    class="rs-textarea"
                    rows="3"
                    :placeholder="field.placeholder"
                  />
                  <input
                    v-else
                    v-model="item[field.key]"
                    class="rs-input"
                    :class="{ 'rs-input--mono': field.mono }"
                    :placeholder="field.placeholder"
                  />
                </label>
              </div>
            </article>

            <button type="button" class="rs-add-item" @click="addItem(section)">
              <Icon :icon="plusIcon" width="13" height="13" />
              添加一条{{ sectionTypeLabel(section.type) }}
            </button>
          </div>
        </section>

        <div class="rs-add-section">
          <span class="rs-eyebrow">添加模块</span>
          <button
            v-for="type in addableTypes"
            :key="type.type"
            type="button"
            class="rs-add-chip"
            :title="type.hint"
            @click="store.createSection(type.type)"
          >
            <Icon :icon="plusIcon" width="12" height="12" />
            {{ type.label }}
          </button>
        </div>
      </template>

      <template v-else>
        <section class="rs-card rs-template-mode">
          <header class="rs-template-mode-head">
            <div>
              <div class="rs-eyebrow">正在编辑这份简历的模板</div>
              <div class="rs-template-mode-name">{{ store.activeTemplate.name }}</div>
            </div>
            <span class="rs-grow" />
            <button type="button" class="rs-btn" @click="store.resetTemplateToBuiltin()">
              <Icon :icon="rotateIcon" width="14" height="14" />
              恢复默认模板
            </button>
          </header>
          <div class="rs-template-source">
            <TemplateSourcePanel
              v-if="store.doc"
              :model-value="store.activeTemplateHtml"
              :theme="theme"
              @update:model-value="onTemplateInput"
            />
          </div>
          <footer class="rs-template-mode-foot">
            <span class="rs-eyebrow">模板修改会自动保存，预览实时更新</span>
            <span class="rs-grow" />
            <span class="rs-eyebrow">{{ templateStatus }}</span>
          </footer>
        </section>
      </template>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import undoIcon from '@iconify-icons/lucide/undo-2'
import redoIcon from '@iconify-icons/lucide/redo-2'
import checkIcon from '@iconify-icons/lucide/check'
import dotIcon from '@iconify-icons/lucide/minus'
import codeIcon from '@iconify-icons/lucide/code-2'
import imageIcon from '@iconify-icons/lucide/image'
import plusIcon from '@iconify-icons/lucide/plus'
import gripIcon from '@iconify-icons/lucide/grip-vertical'
import eyeIcon from '@iconify-icons/lucide/eye'
import eyeOffIcon from '@iconify-icons/lucide/eye-off'
import chevronUpIcon from '@iconify-icons/lucide/chevron-up'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import trashIcon from '@iconify-icons/lucide/trash-2'
import copyIcon from '@iconify-icons/lucide/copy'
import rotateIcon from '@iconify-icons/lucide/rotate-ccw'
import { useResumeStore } from '../store/resumeStore'
import { SECTION_TYPES, SECTION_FIELDS, SECTION_META, createItem, itemTitle } from '../sectionSchema'
import { askConfirm } from '../ui'
import TemplateSourcePanel from './TemplateSourcePanel.vue'

const props = defineProps({
  theme: { type: String, default: 'light' },
})

const store = useResumeStore()
const theme = computed(() => props.theme)
const mode = ref('form')
const completionOpen = ref(false)
const avatarInput = ref(null)
const dragIndex = ref(-1)
const dragOverIndex = ref(-1)
const templateStatus = ref('模板已保存')

const addableTypes = computed(() => SECTION_TYPES)

watch(() => store.activeDocId, () => {
  flushTemplate()
  mode.value = 'form'
  completionOpen.value = false
})

const fieldsOf = (type) => SECTION_FIELDS[type] || SECTION_FIELDS.custom
const sectionTypeLabel = (type) => SECTION_META[type]?.label || '模块'

function addItem(section) {
  section.items.push(createItem(section.type))
}

async function removeItem(section, index) {
  const item = section.items[index]
  const label = itemTitle(section.type, item)
  const ok = await askConfirm({
    title: '删除这一条？',
    body: `「${label}」会从当前简历移除，可以用 Ctrl+Z 撤销。`,
    confirmText: '删除',
    danger: true,
  })
  if (!ok) return
  section.items.splice(index, 1)
}

async function removeSection(index) {
  const section = store.doc?.data.sections[index]
  if (!section) return
  const ok = await askConfirm({
    title: '删除整个模块？',
    body: `「${section.title}」里的 ${section.items.length} 条内容会一起移除，可以用 Ctrl+Z 撤销。`,
    confirmText: '删除模块',
    danger: true,
  })
  if (!ok) return
  store.removeSection(index)
}

function setSkillLines(section, value) {
  const lines = String(value).split(/\r?\n/)
  const next = lines.map((text, index) => ({
    id: section.items[index]?.id || `it_${Date.now()}_${index}`,
    text,
  }))
  // 保留末行空串，输入体验更自然；渲染时会过滤空行。
  section.items = next.length ? next : [createItem('skills')]
}

function onDragStart(index, event) {
  dragIndex.value = index
  event.dataTransfer.effectAllowed = 'move'
  try {
    event.dataTransfer.setData('text/plain', String(index))
  } catch { /* 某些 WebView 不支持写入，不影响重排 */ }
}

function onDropSection(targetIndex) {
  const from = dragIndex.value
  dragIndex.value = -1
  dragOverIndex.value = -1
  if (from < 0 || from === targetIndex) return
  const sections = store.doc.data.sections
  const [section] = sections.splice(from, 1)
  sections.splice(targetIndex, 0, section)
}

function pickAvatar() {
  avatarInput.value?.click()
}

function onAvatarPicked(event) {
  const file = event.target?.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    if (store.doc) store.doc.data.profile.avatar = String(reader.result || '')
  }
  reader.readAsDataURL(file)
  event.target.value = ''
}

async function toggleMode() {
  if (mode.value === 'template') {
    await flushTemplate()
    mode.value = 'form'
    return
  }
  await store.ensureEditableTemplate()
  mode.value = 'template'
}

let templateTimer = null
let pendingTemplateHtml = ''
let pendingTemplateId = ''
function onTemplateInput(html) {
  if (!pendingTemplateHtml) pendingTemplateId = store.doc?.templateId || ''
  pendingTemplateHtml = html
  templateStatus.value = '正在保存…'
  if (templateTimer) clearTimeout(templateTimer)
  templateTimer = setTimeout(flushTemplate, 700)
}

async function flushTemplate() {
  if (templateTimer) {
    clearTimeout(templateTimer)
    templateTimer = null
  }
  if (!pendingTemplateHtml) return
  const html = pendingTemplateHtml
  const templateId = pendingTemplateId
  pendingTemplateHtml = ''
  pendingTemplateId = ''
  try {
    await store.updateTemplateHtml(templateId, html)
    templateStatus.value = '模板已保存'
  } catch (error) {
    templateStatus.value = '模板保存失败'
  }
}

function closeCompletion(event) {
  if (!event.target.closest('.rs-completion-wrap')) completionOpen.value = false
}

const FIELD_KEYS = { name: 'name', contact: 'phone', target: 'target', summary: 'summary' }

async function focusCompletion(item) {
  completionOpen.value = false
  if (mode.value === 'template') mode.value = 'form'
  await nextTick()
  const root = document.querySelector('.rs-editor-inner')
  if (!root) return

  const field = FIELD_KEYS[item.key]
  if (field) {
    const target = root.querySelector(`[data-field="${field}"]`)
    target?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    target?.focus({ preventScroll: true })
    return
  }

  const section = root.querySelector(`[data-section-type="${item.key}"]`)
  if (!section) return
  section.scrollIntoView({ block: 'start', behavior: 'smooth' })
  section.classList.add('is-flash')
  setTimeout(() => section.classList.remove('is-flash'), 1100)
}

onMounted(() => document.addEventListener('click', closeCompletion))
onBeforeUnmount(() => {
  document.removeEventListener('click', closeCompletion)
  flushTemplate()
})
</script>

<style scoped>
.rs-grip { color: var(--rs-muted); cursor: grab; }
.rs-section-head:active .rs-grip { cursor: grabbing; }
.rs-section.is-drop { border-color: var(--rs-accent); box-shadow: 0 0 0 1px var(--rs-accent); }
.rs-hidden-flag {
  font-family: var(--rs-label);
  font-size: 9px;
  letter-spacing: .08em;
  text-transform: uppercase;
  color: var(--rs-accent);
  border: 1px solid var(--rs-accent);
  border-radius: 2px;
  padding: 0 4px;
  flex: none;
}

.rs-progress {
  --p: 0;
  position: relative;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: conic-gradient(var(--rs-accent) calc(var(--p) * 1%), var(--rs-line) 0);
  display: inline-grid;
  place-items: center;
}
.rs-progress span {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--rs-surface);
}
.rs-btn.is-on .rs-progress span { background: var(--rs-ink); }

.rs-completion-wrap { position: relative; }
.rs-completion-menu {
  left: 0;
  right: auto;
  width: 232px;
  padding: 8px;
  cursor: default;
}
.rs-completion-title {
  font-family: var(--rs-label);
  font-size: 9.5px;
  letter-spacing: .16em;
  text-transform: uppercase;
  color: var(--rs-muted);
  padding: 2px 6px 7px;
}
.rs-completion-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 4px 6px;
  border-radius: 2px;
  font-size: 12px;
  color: var(--rs-ink-soft);
  cursor: pointer;
}
.rs-completion-row:hover { background: var(--rs-surface-2); }
.rs-section.is-flash { border-color: var(--rs-accent); box-shadow: 0 0 0 2px var(--rs-accent-soft); }
.rs-completion-row.done { color: var(--rs-muted); }
.rs-completion-row.done svg { color: var(--rs-green); }
.rs-completion-flag { margin-left: auto; font-family: var(--rs-label); font-size: 9px; letter-spacing: .08em; text-transform: uppercase; }

.rs-template-mode { overflow: hidden; }
.rs-template-mode-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--rs-line);
  background: linear-gradient(180deg, var(--rs-surface), var(--rs-surface-2));
}
.rs-template-mode-name { margin-top: 3px; font-family: var(--rs-serif); font-size: 15px; color: var(--rs-ink); }
.rs-template-source { height: min(62vh, 620px); min-height: 320px; }
.rs-template-mode-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-top: 1px solid var(--rs-line);
  background: var(--rs-surface-2);
}
</style>
