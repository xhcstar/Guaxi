<template>
  <div class="resume-app" :style="appStyle">
    <div class="resume-toolbar">
      <div class="toolbar-title-block">
        <div class="toolbar-title">简历生成器</div>
        <div v-if="toolbarMeta" class="toolbar-meta">{{ toolbarMeta }}</div>
      </div>

      <div class="toolbar-actions">
        <div class="mode-switch">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button
                size="small"
                :type="activeMode === 'form' ? 'primary' : 'default'"
                :secondary="activeMode !== 'form'"
                class="icon-tool-button"
                aria-label="表单模式"
                @click="setActiveMode('form')"
              >
                <template #icon><Icon :icon="fileTextIcon" /></template>
              </n-button>
            </template>
            表单模式
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button
                size="small"
                :type="activeMode === 'source' ? 'primary' : 'default'"
                :secondary="activeMode !== 'source'"
                class="icon-tool-button"
                aria-label="源码模式"
                @click="setActiveMode('source')"
              >
                <template #icon><Icon :icon="code2Icon" /></template>
              </n-button>
            </template>
            源码模式
          </n-tooltip>
        </div>
        <n-select
          v-if="activeMode === 'form'"
          v-model:value="selectedThemeKey"
          class="theme-select"
          size="small"
          :options="themeOptions"
        />
        <div v-if="activeMode === 'source'" class="header-source-actions">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary class="icon-tool-button" aria-label="同步表单" @click="syncSourceFromForm">
                <template #icon><Icon :icon="refreshCwIcon" /></template>
              </n-button>
            </template>
            从当前表单重新生成源码
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-select
                v-model:value="selectedTemplateId"
                class="template-select header-template-select"
                size="small"
                placeholder="选择 HTML 模板"
                :options="templateOptions"
                @update:value="applyTemplateById"
              />
            </template>
            默认模板和自定义模板统一在这里选择
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary class="icon-tool-button" aria-label="导入模板" @click="importHtmlTemplate">
                <template #icon><Icon :icon="uploadIcon" /></template>
              </n-button>
            </template>
            导入 HTML 作为自定义模板
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button
                size="small"
                quaternary
                class="icon-tool-button"
                aria-label="删除模板"
                :disabled="!canRemoveSelectedTemplate"
                @click="removeSelectedTemplate"
              >
                <template #icon><Icon :icon="trash2Icon" /></template>
              </n-button>
            </template>
            删除选中的自定义模板，默认模板不可删除
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" type="primary" class="icon-tool-button" aria-label="保存" @click="saveCurrentSourceDocument">
                <template #icon><Icon :icon="saveIcon" /></template>
              </n-button>
            </template>
            保存当前源码版本，Ctrl+S 也会触发
          </n-tooltip>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" secondary class="icon-tool-button" aria-label="历史版本" @click="openHistoryDrawer">
                <template #icon><Icon :icon="clock3Icon" /></template>
              </n-button>
            </template>
            查看、打开或删除已保存的简历源文件
          </n-tooltip>
          <n-dropdown
            trigger="click"
            :options="externalEditorMenuOptions"
            @select="openSourceWithEditor"
          >
            <n-button size="small" secondary class="icon-tool-button" aria-label="外部编辑" title="外部编辑">
              <template #icon><Icon :icon="externalLinkIcon" /></template>
            </n-button>
          </n-dropdown>
        </div>
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" secondary class="icon-tool-button" :aria-label="showPreview ? '隐藏预览' : '显示预览'" @click="togglePreview">
              <template #icon>
                <Icon :icon="showPreview ? panelRightCloseIcon : panelRightOpenIcon" />
              </template>
            </n-button>
          </template>
          {{ showPreview ? '隐藏预览区，扩大编辑空间' : '恢复右侧预览区' }}
        </n-tooltip>
        <n-dropdown
          trigger="click"
          :options="exportMenuOptions"
          @select="handleExportSelect"
        >
          <n-button size="small" type="primary" class="icon-tool-button" aria-label="导出简历" title="导出简历">
            <template #icon><Icon :icon="downloadIcon" /></template>
          </n-button>
        </n-dropdown>
      </div>
    </div>

    <div
      ref="workbenchRef"
      class="resume-workbench"
      :class="{ 'preview-hidden': !showPreview, 'is-resizing': isResizing }"
      :style="workbenchStyle"
    >
      <aside class="resume-editor">
        <template v-if="activeMode === 'form'">
          <div class="form-panel">
          <n-tabs v-model:value="activeTab" type="segment" size="small" :animated="false" class="form-tabs">
            <n-tab-pane name="basic" tab="基础">
              <div class="form-section">
                <div class="section-heading">个人信息</div>
                <div class="form-grid two">
                  <n-input v-model:value="profile.name" placeholder="姓名" />
                  <n-input v-model:value="profile.title" placeholder="职位标题" />
                  <n-input v-model:value="profile.phone" placeholder="联系方式" />
                  <n-input v-model:value="profile.email" placeholder="电子邮箱" />
                  <n-input v-model:value="profile.city" placeholder="联系地址" />
                  <n-input v-model:value="profile.website" placeholder="个人主页 / GitHub" />
                </div>
                <n-input v-model:value="profile.target" class="field-gap" placeholder="求职意向" />
                <n-input
                  v-model:value="profile.summary"
                  class="field-gap"
                  type="textarea"
                  :autosize="{ minRows: 3, maxRows: 6 }"
                  placeholder="个人优势 / 职业摘要"
                />
                <div class="avatar-tools">
                  <n-input v-model:value="profile.avatar" placeholder="头像 URL 或上传本地图片" />
                  <input ref="avatarInputRef" class="hidden-input" type="file" accept="image/*" @change="onAvatarPicked" />
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button secondary class="icon-tool-button" aria-label="上传头像" @click="pickAvatar">
                        <template #icon><Icon :icon="imageIcon" /></template>
                      </n-button>
                    </template>
                    上传头像
                  </n-tooltip>
                </div>
              </div>

              <div class="form-section">
                <div class="section-heading">专业技能</div>
                <n-input
                  v-model:value="skillsText"
                  type="textarea"
                  :autosize="{ minRows: 8, maxRows: 14 }"
                  placeholder="每行一条技能，例如：熟悉 C/C++、Python、Shell 和常见数据结构"
                />
              </div>
            </n-tab-pane>

            <n-tab-pane name="education" tab="教育">
              <div class="section-title-row">
                <div class="section-heading">教育经历</div>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" secondary class="icon-tool-button" aria-label="新增教育经历" @click="addEducation">
                      <template #icon><Icon :icon="plusIcon" /></template>
                    </n-button>
                  </template>
                  新增教育经历
                </n-tooltip>
              </div>
              <div v-for="(item, index) in education" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>教育 {{ index + 1 }}</span>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button
                        quaternary
                        circle
                        size="small"
                        aria-label="删除教育经历"
                        @click="removeEntry(education, index, 'education')"
                      >
                        <template #icon><Icon :icon="trash2Icon" /></template>
                      </n-button>
                    </template>
                    删除这段教育经历
                  </n-tooltip>
                </div>
                <div class="form-grid two">
                  <n-input v-model:value="item.period" placeholder="时间：2015.09-2019.07" />
                  <n-input v-model:value="item.school" placeholder="学校" />
                  <n-input v-model:value="item.college" placeholder="学院" />
                  <n-input v-model:value="item.major" placeholder="专业" />
                </div>
                <n-input v-model:value="item.degree" class="field-gap" placeholder="学历 / 学位" />
              </div>
            </n-tab-pane>

            <n-tab-pane name="work" tab="工作">
              <div class="section-title-row">
                <div class="section-heading">工作经历</div>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" secondary class="icon-tool-button" aria-label="新增工作经历" @click="addWork">
                      <template #icon><Icon :icon="plusIcon" /></template>
                    </n-button>
                  </template>
                  新增工作经历
                </n-tooltip>
              </div>
              <div v-for="(item, index) in works" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>工作 {{ index + 1 }}</span>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button
                        quaternary
                        circle
                        size="small"
                        aria-label="删除工作经历"
                        @click="removeEntry(works, index, 'work')"
                      >
                        <template #icon><Icon :icon="trash2Icon" /></template>
                      </n-button>
                    </template>
                    删除这段工作经历
                  </n-tooltip>
                </div>
                <div class="form-grid two">
                  <n-input v-model:value="item.period" placeholder="时间：2023.01-至今" />
                  <n-input v-model:value="item.company" placeholder="公司" />
                  <n-input v-model:value="item.department" placeholder="部门" />
                  <n-input v-model:value="item.role" placeholder="岗位" />
                </div>
                <n-input
                  v-model:value="item.description"
                  class="field-gap"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 10 }"
                  placeholder="主要职责 / 业绩，每行一条"
                />
              </div>
            </n-tab-pane>

            <n-tab-pane name="project" tab="项目">
              <div class="section-title-row">
                <div class="section-heading">项目经历</div>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" secondary class="icon-tool-button" aria-label="新增项目经历" @click="addProject">
                      <template #icon><Icon :icon="plusIcon" /></template>
                    </n-button>
                  </template>
                  新增项目经历
                </n-tooltip>
              </div>
              <div v-for="(item, index) in projects" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>项目 {{ index + 1 }}</span>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button
                        quaternary
                        circle
                        size="small"
                        aria-label="删除项目经历"
                        @click="removeEntry(projects, index, 'project')"
                      >
                        <template #icon><Icon :icon="trash2Icon" /></template>
                      </n-button>
                    </template>
                    删除这个项目经历
                  </n-tooltip>
                </div>
                <div class="form-grid two">
                  <n-input v-model:value="item.period" placeholder="时间：2020.02-2020.05" />
                  <n-input v-model:value="item.name" placeholder="项目名称" />
                </div>
                <n-input v-model:value="item.description" class="field-gap" placeholder="项目简介" />
                <n-input
                  v-model:value="item.details"
                  class="field-gap"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 12 }"
                  placeholder="职责 / 技术点，每行一条"
                />
              </div>
            </n-tab-pane>
          </n-tabs>

          <div class="editor-footer">
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button quaternary size="small" class="icon-tool-button" aria-label="恢复示例" @click="resetDemo">
                  <template #icon><Icon :icon="rotateCcwIcon" /></template>
                </n-button>
              </template>
              恢复示例内容
            </n-tooltip>
            <span>{{ resumeStats }}</span>
          </div>
          </div>
        </template>

        <template v-else>
          <div class="source-panel">
            <div
              ref="sourceEditorHostRef"
              class="source-editor"
            ></div>
          </div>
        </template>
      </aside>

      <div
        v-if="showPreview"
        class="resize-handle"
        :class="{ resizing: isResizing }"
        @mousedown="startResize"
      >
        <span></span>
      </div>

      <main v-if="showPreview" class="resume-preview-shell">
        <div v-if="activeMode === 'form'" class="preview-stage">
          <article
            ref="resumeRef"
            :class="['resume-paper', `resume-theme-${selectedThemeKey}`]"
          >
            <header class="resume-header">
              <div class="resume-profile">
                <h1>{{ profile.name || '姓名' }}</h1>
                <div v-if="profile.title" class="resume-role">{{ profile.title }}</div>
                <div class="contact-grid">
                  <div v-for="item in contactItems" :key="item.label" class="contact-item">
                    <strong>{{ item.label }}：</strong>
                    <span>{{ item.value }}</span>
                  </div>
                </div>
                <div v-if="profile.target" class="target-line">
                  <strong>求职意向：</strong>{{ profile.target }}
                </div>
              </div>
              <div v-if="profile.avatar" class="avatar-box">
                <img :src="profile.avatar" alt="头像" />
              </div>
            </header>

            <section v-if="profile.summary" class="resume-section">
              <h2>个人优势</h2>
              <p class="summary-text">{{ profile.summary }}</p>
            </section>

            <section v-if="education.length" class="resume-section">
              <h2>教育经历</h2>
              <div v-for="item in education" :key="item.id" class="timeline-row education-row">
                <div class="time-col">{{ item.period }}</div>
                <div class="content-col">
                  <div class="inline-meta">
                    <strong>{{ item.school }}</strong>
                    <span>{{ item.college }}</span>
                    <span>{{ item.major }}</span>
                    <span>{{ item.degree }}</span>
                  </div>
                </div>
              </div>
            </section>

            <section v-if="works.length" class="resume-section">
              <h2>工作经历</h2>
              <div v-for="item in works" :key="item.id" class="timeline-row">
                <div class="time-col">{{ item.period }}</div>
                <div class="content-col">
                  <div class="inline-meta">
                    <strong>{{ item.company }}</strong>
                    <span>{{ item.department }}</span>
                    <span>{{ item.role }}</span>
                  </div>
                  <ul v-if="toLines(item.description).length" class="resume-list">
                    <li v-for="line in toLines(item.description)" :key="line">{{ line }}</li>
                  </ul>
                </div>
              </div>
            </section>

            <section v-if="skillLines.length" class="resume-section">
              <h2>专业技能</h2>
              <ul class="resume-list skill-list">
                <li v-for="line in skillLines" :key="line">{{ line }}</li>
              </ul>
            </section>

            <section v-if="projects.length" class="resume-section">
              <h2>项目经历</h2>
              <div v-for="item in projects" :key="item.id" class="timeline-row project-row">
                <div class="time-col">{{ item.period }}</div>
                <div class="content-col">
                  <div class="project-title">{{ item.name }}</div>
                  <p v-if="item.description" class="project-desc">{{ item.description }}</p>
                  <ul v-if="toLines(item.details).length" class="resume-list">
                    <li v-for="line in toLines(item.details)" :key="line">{{ line }}</li>
                  </ul>
                </div>
              </div>
            </section>
          </article>
        </div>

        <iframe
          v-else
          ref="sourcePreviewRef"
          class="source-preview-frame"
          title="HTML 源码预览"
          sandbox="allow-same-origin"
          :srcdoc="renderedSourceHtml"
        ></iframe>
      </main>
    </div>

    <n-drawer v-model:show="historyDrawerVisible" :width="420" placement="right">
      <n-drawer-content title="简历源文件版本" closable>
        <div v-if="sourceHistories.length" class="history-list">
          <article v-for="history in sourceHistories" :key="history.id" class="history-card">
            <div class="history-card-main">
              <strong>{{ history.name }}</strong>
              <span>{{ formatHistoryDisplay(history.createdAt) }}</span>
            </div>
            <div class="history-card-actions">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button size="small" secondary class="icon-tool-button" aria-label="打开修改" @click="openHistoryItem(history)">
                    <template #icon><Icon :icon="fileTextIcon" /></template>
                  </n-button>
                </template>
                打开这个版本继续修改
              </n-tooltip>
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button size="small" quaternary class="icon-tool-button" aria-label="删除版本" @click="removeHistoryItem(history.id)">
                    <template #icon><Icon :icon="trash2Icon" /></template>
                  </n-button>
                </template>
                删除这个版本
              </n-tooltip>
            </div>
          </article>
        </div>
        <div v-else class="history-empty">
          暂无已保存的简历源文件
        </div>
      </n-drawer-content>
    </n-drawer>

    <div
      v-if="printMarkup"
      class="resume-print-host"
      aria-hidden="true"
      v-html="printMarkup"
    ></div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { Icon } from '@iconify/vue'
import * as monaco from 'monaco-editor'
import clock3Icon from '@iconify-icons/lucide/clock-3'
import code2Icon from '@iconify-icons/lucide/code-2'
import downloadIcon from '@iconify-icons/lucide/download'
import externalLinkIcon from '@iconify-icons/lucide/external-link'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import imageIcon from '@iconify-icons/lucide/image'
import panelRightCloseIcon from '@iconify-icons/lucide/panel-right-close'
import panelRightOpenIcon from '@iconify-icons/lucide/panel-right-open'
import plusIcon from '@iconify-icons/lucide/plus'
import refreshCwIcon from '@iconify-icons/lucide/refresh-cw'
import rotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import saveIcon from '@iconify-icons/lucide/save'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import uploadIcon from '@iconify-icons/lucide/upload'
import { open } from '@tauri-apps/plugin-dialog'
import { appDataDir, join } from '@tauri-apps/api/path'
import { openPath, openUrl } from '@tauri-apps/plugin-opener'
import { BaseDirectory, mkdir, readTextFile, writeTextFile } from '@tauri-apps/plugin-fs'
import { createDiscreteApi } from 'naive-ui'
import { useSettingsStore } from '../../../stores/settingsStore'
import { GUAXI_MONACO_SEMANTIC_TOKEN_COLORS, GUAXI_MONACO_THEME_RULES } from '../../../monaco/guaxiMonacoThemes'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('ResumeBuilderApp')
const { message } = createDiscreteApi(['message'])
const settingsStore = useSettingsStore()

const CUSTOM_TEMPLATE_STORAGE_KEY = 'guaxi.resumeBuilder.customTemplates.v1'
const HISTORY_STORAGE_KEY = 'guaxi.resumeBuilder.sourceHistory.v1'
const EXTERNAL_EDIT_DIR = 'resume-builder'
const BUILTIN_TEMPLATE_PREFIX = 'builtin:'
const SOURCE_EDITOR_FONT_FAMILY = "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'"
const activeMode = ref('form')
const activeTab = ref('basic')
const resumeRef = ref(null)
const sourcePreviewRef = ref(null)
const sourceEditorHostRef = ref(null)
const avatarInputRef = ref(null)
const workbenchRef = ref(null)
const selectedThemeKey = ref('blueprint')
const showPreview = ref(true)
const editorWidth = ref(720)
const isResizing = ref(false)
const resizeStartX = ref(0)
const resizeStartWidth = ref(0)
const sourceHtml = ref('')
const sourceHtmlPath = ref('')
const externalEditPath = ref('')
const customTemplates = ref(loadCustomTemplates())
const selectedTemplateId = ref(`${BUILTIN_TEMPLATE_PREFIX}blueprint`)
const sourceHistories = ref(loadSourceHistories())
const selectedHistoryId = ref(null)
const externalEditor = ref('vscode')
const historyDrawerVisible = ref(false)
const printMarkup = ref('')
const sourceEditor = shallowRef(null)
const sourceModel = shallowRef(null)
let sourceEditorContentDisposable = null
let sourceEditorResizeObserver = null
let monacoThemesReady = false
let resizePreviousCursor = ''
let resizePreviousUserSelect = ''
let activePrintStyleElement = null
let lastHistorySaveCreated = true

const externalEditorOptions = [
  { label: 'VS Code', value: 'vscode' },
  { label: 'Neovim', value: 'neovim' },
  { label: '系统默认', value: 'default' },
]

const exportMenuOptions = [
  { label: '导出 HTML', key: 'html' },
  { label: '打印 / PDF', key: 'pdf' },
]

const themes = [
  { label: '蓝白商务', value: 'blueprint', color: '#2f80c9' },
  { label: '墨灰极简', value: 'graphite', color: '#374151' },
  { label: '青绿色', value: 'emerald', color: '#0f8b8d' },
]

const themeOptions = themes.map((theme) => ({
  label: theme.label,
  value: theme.value,
}))

const externalEditorMenuOptions = externalEditorOptions.map((editor) => ({
  label: editor.label,
  key: editor.value,
}))

const selectedTheme = computed(() => themes.find((theme) => theme.value === selectedThemeKey.value) || themes[0])

const appFontSize = computed(() => {
  const configured = Number(settingsStore.appearanceFontSize)
  return `${Math.max(Number.isFinite(configured) ? configured : 14, 15)}px`
})

const appStyle = computed(() => ({
  fontSize: appFontSize.value,
  fontFamily: SOURCE_EDITOR_FONT_FAMILY,
  '--resume-builder-font-family': SOURCE_EDITOR_FONT_FAMILY,
}))

const sourceEditorFontSize = computed(() => {
  const configured = Number.parseInt(appFontSize.value, 10) || 15
  return Math.max(configured + 3, 17)
})

const monacoTheme = computed(() => (
  settingsStore.appearanceTheme === 'dark' ? 'guaxi-resume-dark' : 'guaxi-resume-light'
))

const toolbarMeta = computed(() => {
  if (activeMode.value === 'source') {
    return ''
  }
  return `${filledScore.value}% 完成 · ${selectedTheme.value.label}`
})

const workbenchStyle = computed(() => {
  if (!showPreview.value) {
    return { gridTemplateColumns: 'minmax(0, 1fr)' }
  }
  return { gridTemplateColumns: `${editorWidth.value}px 12px minmax(0, 1fr)` }
})

const templateOptions = computed(() => [
  ...themes.map((theme) => ({
    label: `默认 · ${theme.label}`,
    value: `${BUILTIN_TEMPLATE_PREFIX}${theme.value}`,
  })),
  ...customTemplates.value.map((template) => ({
    label: `自定义 · ${template.name}`,
    value: template.id,
  })),
])

const canRemoveSelectedTemplate = computed(() => (
  Boolean(selectedTemplateId.value)
  && !isBuiltinTemplateId(selectedTemplateId.value)
  && customTemplates.value.some((template) => template.id === selectedTemplateId.value)
))

// 预览专用：剥掉脚本后再交给 sandbox iframe，避免控制台刷 "Blocked script execution"。
const renderedSourceHtml = computed(() =>
  stripScriptsForPreview(normalizeStandaloneHtml(renderResumeTemplate(sourceHtml.value)))
)

const createId = () => `${Date.now()}-${Math.random().toString(16).slice(2)}`

function setActiveMode(mode) {
  if (!['form', 'source'].includes(mode)) return
  activeMode.value = mode
  logger.info('[Resume] mode switch requested', { mode })
}

function handleGlobalShortcut(event) {
  const key = String(event.key || '').toLowerCase()
  if (activeMode.value !== 'source' || key !== 's' || (!event.ctrlKey && !event.metaKey)) return
  event.preventDefault()
  logger.info('[Resume] source save shortcut captured')
  saveCurrentSourceDocument()
}

function isBuiltinTemplateId(id) {
  return String(id || '').startsWith(BUILTIN_TEMPLATE_PREFIX)
}

function getBuiltinTemplateThemeKey(id) {
  return String(id || '').slice(BUILTIN_TEMPLATE_PREFIX.length)
}

// 设计思路：表单模式负责稳定生成结构化简历，源码模式负责完全开放 HTML/CSS，让高级用户按模板系统继续扩展。
const createDemoState = () => ({
  profile: {
    name: '林泽远',
    title: '嵌入式软件工程师',
    city: '星海市',
    phone: '138-0000-0000',
    email: 'linzy@example.com',
    website: 'github.com/linzy-demo',
    target: 'Linux C 开发 / 嵌入式开发',
    summary: '具备 Linux 网络、嵌入式系统与 IoT 设备软件开发经验，熟悉从需求拆解、模块设计、联调定位到交付维护的完整流程。',
    avatar: '',
  },
  education: [
    {
      id: createId(),
      period: '2015.09-2019.07',
      school: '星河理工大学',
      college: '电子工程学院',
      major: '微电子科学与工程',
      degree: '本科',
    },
  ],
  works: [
    {
      id: createId(),
      period: '2019.07-2020.08',
      company: '蓝海智联科技有限公司',
      department: '物联网研究院',
      role: '嵌入式软件工程师',
      description: '主要负责物联网设备通信模块的软件开发工作',
    },
    {
      id: createId(),
      period: '2020.09-2022.04',
      company: '云岭开源软件有限公司',
      department: '操作系统事业部',
      role: '软件工程师',
      description: '主要负责 Euler / OpenEuler 操作系统网络相关软件包开发及维护工作',
    },
    {
      id: createId(),
      period: '2023.01-至今',
      company: '北辰安全科技股份有限公司',
      department: 'R&D Department',
      role: '研发工程师',
      description: '主要负责 APV 设备 SSL 相关功能的开发和维护工作',
    },
  ],
  projects: [
    {
      id: createId(),
      period: '2019.10-2020.01',
      name: '双云智能家居项目开发',
      description: '该项目实现智能电器与虚拟 IoT 平台对接，支持用户通过移动端 APP 控制智能电器。',
      details: '职责：负责通信模块的软件开发工作\n主要技术：C、FreeRTOS',
    },
    {
      id: createId(),
      period: '2020.02-2020.05',
      name: 'WiFi + BLE 智能家居网关设备',
      description: '负责网关通信链路、设备接入和异常恢复逻辑开发。',
      details: '设计设备配网流程和状态机\n定位并修复通信稳定性问题',
    },
  ],
  skillsText: [
    '熟悉 C/C++、Python、Shell 和基本的数据结构与算法',
    '熟悉 Linux 系统下网络 / 启动相关软件包的问题定位',
    '熟悉 Linux 下 C/C++ 程序设计、Shell 脚本编写以及嵌入式设备软件开发流程',
    '熟悉 TCP/IP、HTTP、TLS 等常见网络协议',
    '熟悉智能家居体系架构和 IoT 云平台下物联网设备的软件开发流程',
    '熟悉 FreeRTOS 等常见嵌入式操作系统',
  ].join('\n'),
})

const demoState = createDemoState()
const profile = ref({ ...demoState.profile })
const education = ref(demoState.education.map((item) => ({ ...item })))
const works = ref(demoState.works.map((item) => ({ ...item })))
const projects = ref(demoState.projects.map((item) => ({ ...item })))
const skillsText = ref(demoState.skillsText)

const contactItems = computed(() => [
  { label: '联系地址', value: profile.value.city },
  { label: '联系方式', value: profile.value.phone },
  { label: '电子邮箱', value: profile.value.email },
  { label: '个人主页', value: profile.value.website },
].filter((item) => item.value))

const skillLines = computed(() => toLines(skillsText.value))

const filledScore = computed(() => {
  const required = [
    profile.value.name,
    profile.value.phone,
    profile.value.email,
    profile.value.target,
    skillsText.value,
    education.value[0]?.school,
    works.value[0]?.company,
    projects.value[0]?.name,
  ]
  const filled = required.filter((item) => String(item || '').trim()).length
  return Math.round((filled / required.length) * 100)
})

const resumeStats = computed(() => {
  return `${education.value.length} 段教育 · ${works.value.length} 段工作 · ${projects.value.length} 个项目`
})

onMounted(() => {
  document.addEventListener('keydown', handleGlobalShortcut)
})

watch(activeMode, async (mode, previousMode) => {
  if (previousMode === 'source' && mode !== 'source') {
    disposeSourceMonacoEditor('mode-leave-source')
  }
  if (mode === 'source' && !sourceHtml.value.trim()) {
    await nextTick()
    syncSourceFromForm()
  }
  if (mode === 'source') {
    await nextTick()
    ensureSourceMonacoEditor()
    await nextTick()
    layoutSourceEditor()
  }
  logger.info('[Resume] mode changed', { mode, previousMode })
})

watch(sourceHtml, (value) => {
  const editor = sourceEditor.value
  const model = sourceModel.value
  if (!editor || !model) return
  if (model.getValue() !== value) {
    model.setValue(value)
  }
})

watch([sourceEditorFontSize, monacoTheme], () => {
  updateSourceEditorOptions()
})

watch([showPreview, editorWidth, activeMode], async () => {
  await nextTick()
  layoutSourceEditor()
})

watch(customTemplates, (templates) => {
  localStorage.setItem(CUSTOM_TEMPLATE_STORAGE_KEY, JSON.stringify(templates))
}, { deep: true })

watch(sourceHistories, (histories) => {
  localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(histories.slice(0, 40)))
}, { deep: true })

function toLines(text) {
  return String(text || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
}

function addEducation() {
  education.value.push({
    id: createId(),
    period: '',
    school: '',
    college: '',
    major: '',
    degree: '',
  })
  logger.info('[Resume] add education', { count: education.value.length })
}

function addWork() {
  works.value.push({
    id: createId(),
    period: '',
    company: '',
    department: '',
    role: '',
    description: '',
  })
  logger.info('[Resume] add work', { count: works.value.length })
}

function addProject() {
  projects.value.push({
    id: createId(),
    period: '',
    name: '',
    description: '',
    details: '',
  })
  logger.info('[Resume] add project', { count: projects.value.length })
}

function removeEntry(list, index, type) {
  list.splice(index, 1)
  logger.info('[Resume] remove entry', { type, index, count: list.length })
}

function resetDemo() {
  const next = createDemoState()
  profile.value = { ...next.profile }
  education.value = next.education.map((item) => ({ ...item }))
  works.value = next.works.map((item) => ({ ...item }))
  projects.value = next.projects.map((item) => ({ ...item }))
  skillsText.value = next.skillsText
  logger.info('[Resume] reset demo data')
  message.success('已恢复示例内容')
}

function pickAvatar() {
  avatarInputRef.value?.click()
}

function onAvatarPicked(event) {
  const file = event.target?.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = () => {
    profile.value.avatar = String(reader.result || '')
    logger.info('[Resume] avatar loaded', { name: file.name, size: file.size })
  }
  reader.onerror = () => {
    logger.warn('[Resume] avatar read failed', { name: file.name })
    message.error('头像读取失败')
  }
  reader.readAsDataURL(file)
  event.target.value = ''
}

function togglePreview() {
  showPreview.value = !showPreview.value
  logger.info('[Resume] preview visibility changed', { showPreview: showPreview.value })
}

function startResize(event) {
  event.preventDefault()
  isResizing.value = true
  resizeStartX.value = event.clientX
  resizeStartWidth.value = editorWidth.value
  resizePreviousCursor = document.body.style.cursor
  resizePreviousUserSelect = document.body.style.userSelect
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  document.addEventListener('mousemove', onResizeMove)
  document.addEventListener('mouseup', stopResize)
  logger.info('[Resume] editor resize started', { editorWidth: editorWidth.value })
}

function onResizeMove(event) {
  if (!isResizing.value) return
  event.preventDefault()
  const workbenchWidth = workbenchRef.value?.clientWidth || window.innerWidth
  const maxWidth = Math.max(560, workbenchWidth - 380)
  const nextWidth = resizeStartWidth.value + event.clientX - resizeStartX.value
  editorWidth.value = Math.min(Math.max(560, nextWidth), maxWidth)
}

function stopResize() {
  if (!isResizing.value) return
  restoreResizeInteraction()
  isResizing.value = false
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  logger.info('[Resume] editor resized', { editorWidth: editorWidth.value })
}

function restoreResizeInteraction() {
  if (!isResizing.value && !resizePreviousCursor && !resizePreviousUserSelect) return
  document.body.style.cursor = resizePreviousCursor
  document.body.style.userSelect = resizePreviousUserSelect
  resizePreviousCursor = ''
  resizePreviousUserSelect = ''
}

function ensureResumeMonacoThemes() {
  if (monacoThemesReady) return

  monaco.editor.defineTheme('guaxi-resume-light', {
    base: 'vs',
    inherit: true,
    rules: GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2937',
      'editorGutter.background': '#ffffff',
      'editorLineNumber.foreground': '#9ca3af',
      'editorLineNumber.activeForeground': '#374151',
      'editorCursor.foreground': '#2563eb',
      'editor.selectionBackground': '#bfdbfe',
      'editor.lineHighlightBackground': '#f8fafc',
      'editor.lineHighlightBorder': '#00000000',
      'editorWidget.background': '#ffffff',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#d1d5db',
    },
  })

  monaco.editor.defineTheme('guaxi-resume-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: GUAXI_MONACO_THEME_RULES.dark,
    semanticHighlighting: true,
    semanticTokenColors: GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark,
    colors: {
      'editor.background': '#1f242c',
      'editor.foreground': '#d7dde8',
      'editorGutter.background': '#1f242c',
      'editorLineNumber.foreground': '#697282',
      'editorLineNumber.activeForeground': '#d7dde8',
      'editorCursor.foreground': '#60a5fa',
      'editor.selectionBackground': '#334155',
      'editor.lineHighlightBackground': '#242a34',
      'editor.lineHighlightBorder': '#00000000',
      'editorWidget.background': '#1f242c',
      'editorSuggestWidget.background': '#1f242c',
      'editorSuggestWidget.border': '#374151',
    },
  })

  monacoThemesReady = true
  logger.info('[Resume] Monaco themes registered')
}

function ensureSourceMonacoEditor() {
  if (!sourceEditorHostRef.value) {
    logger.warn('[Resume] Monaco source editor host missing')
    return
  }

  const existingEditorNode = sourceEditor.value?.getDomNode?.()
  if (sourceEditor.value && existingEditorNode && sourceEditorHostRef.value.contains(existingEditorNode)) {
    layoutSourceEditor()
    return
  }

  if (sourceEditor.value) {
    logger.warn('[Resume] Monaco source editor host changed, recreating editor')
    disposeSourceMonacoEditor('host-changed')
  }

  ensureResumeMonacoThemes()
  const model = monaco.editor.createModel(sourceHtml.value || '', 'html')
  const editor = monaco.editor.create(sourceEditorHostRef.value, {
    model,
    theme: monacoTheme.value,
    automaticLayout: true,
    minimap: { enabled: false },
    wordWrap: 'on',
    wrappingIndent: 'same',
    scrollBeyondLastLine: false,
    lineNumbersMinChars: 4,
    fontSize: sourceEditorFontSize.value,
    fontFamily: SOURCE_EDITOR_FONT_FAMILY,
    lineHeight: Math.max(sourceEditorFontSize.value + 8, 26),
    tabSize: 2,
    renderWhitespace: 'selection',
    bracketPairColorization: { enabled: true },
    formatOnPaste: true,
    formatOnType: true,
    overviewRulerBorder: false,
    padding: { top: 10, bottom: 10 },
  })

  sourceEditorContentDisposable = editor.onDidChangeModelContent(() => {
    const next = model.getValue()
    if (sourceHtml.value !== next) {
      sourceHtml.value = next
    }
  })

  sourceEditorResizeObserver = new ResizeObserver(() => layoutSourceEditor())
  sourceEditorResizeObserver.observe(sourceEditorHostRef.value)
  sourceModel.value = model
  sourceEditor.value = editor
  updateSourceEditorOptions()
  layoutSourceEditor()
  logger.info('[Resume] Monaco source editor created', {
    fontSize: sourceEditorFontSize.value,
    fontFamily: SOURCE_EDITOR_FONT_FAMILY,
    hostWidth: sourceEditorHostRef.value.clientWidth,
    hostHeight: sourceEditorHostRef.value.clientHeight,
  })
}

function updateSourceEditorOptions() {
  const editor = sourceEditor.value
  if (!editor) return
  ensureResumeMonacoThemes()
  const fontSize = sourceEditorFontSize.value
  monaco.editor.setTheme(monacoTheme.value)
  editor.updateOptions({
    fontSize,
    fontFamily: SOURCE_EDITOR_FONT_FAMILY,
    lineHeight: Math.max(fontSize + 8, 26),
  })
  layoutSourceEditor()
}

function layoutSourceEditor() {
  try {
    const host = sourceEditorHostRef.value
    if (host && sourceEditor.value) {
      sourceEditor.value.layout({
        width: Math.max(host.clientWidth, 1),
        height: Math.max(host.clientHeight, 1),
      })
      return
    }
    sourceEditor.value?.layout()
  } catch (error) {
    logger.warn('[Resume] Monaco layout failed', error)
  }
}

function disposeSourceMonacoEditor(reason = 'unknown') {
  const modelValue = sourceModel.value?.getValue?.()
  if (typeof modelValue === 'string' && sourceHtml.value !== modelValue) {
    sourceHtml.value = modelValue
  }
  sourceEditorContentDisposable?.dispose?.()
  sourceEditorContentDisposable = null
  sourceEditorResizeObserver?.disconnect?.()
  sourceEditorResizeObserver = null
  sourceEditor.value?.dispose?.()
  sourceEditor.value = null
  sourceModel.value?.dispose?.()
  sourceModel.value = null
  logger.info('[Resume] Monaco source editor disposed', { reason })
}

function getFileBaseName() {
  const raw = `${profile.value.name || 'resume'}-${profile.value.target || '简历'}`
  return raw.replace(/[\\/:*?"<>|]/g, '').replace(/\s+/g, '-').slice(0, 60) || 'resume'
}

// 关键实现：表单导出从真实预览 DOM 获取，源码模式则完全以用户编辑的 HTML 为准。
function getCurrentExportHtml() {
  if (activeMode.value === 'source') {
    return renderedSourceHtml.value
  }
  return buildStandaloneHtml()
}

function buildStandaloneHtml() {
  const paperHtml = resumeRef.value?.outerHTML || buildResumeArticleHtml()
  return [
    '<!doctype html>',
    '<html lang="zh-CN">',
    '<head>',
    '<meta charset="UTF-8" />',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    `<title>${escapeHtml(getFileBaseName())}</title>`,
    `<style>${standaloneResumeCss}</style>`,
    '</head>',
    '<body>',
    paperHtml,
    '</body>',
    '</html>',
  ].join('\n')
}

function buildResumeArticleHtml() {
  const contactHtml = contactItems.value.map((item) => (
    `<div class="contact-item"><strong>${escapeHtml(item.label)}：</strong><span>${escapeHtml(item.value)}</span></div>`
  )).join('')
  const educationHtml = education.value.map((item) => `
    <div class="timeline-row education-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="inline-meta">
          <strong>${escapeHtml(item.school)}</strong>
          <span>${escapeHtml(item.college)}</span>
          <span>${escapeHtml(item.major)}</span>
          <span>${escapeHtml(item.degree)}</span>
        </div>
      </div>
    </div>
  `).join('')
  const worksHtml = works.value.map((item) => `
    <div class="timeline-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="inline-meta">
          <strong>${escapeHtml(item.company)}</strong>
          <span>${escapeHtml(item.department)}</span>
          <span>${escapeHtml(item.role)}</span>
        </div>
        ${buildListHtml(toLines(item.description))}
      </div>
    </div>
  `).join('')
  const projectHtml = projects.value.map((item) => `
    <div class="timeline-row project-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="project-title">${escapeHtml(item.name)}</div>
        ${item.description ? `<p class="project-desc">${escapeHtml(item.description)}</p>` : ''}
        ${buildListHtml(toLines(item.details))}
      </div>
    </div>
  `).join('')

  return `
    <article class="resume-paper resume-theme-${selectedThemeKey.value}">
      <header class="resume-header">
        <div class="resume-profile">
          <h1>${escapeHtml(profile.value.name || '姓名')}</h1>
          ${profile.value.title ? `<div class="resume-role">${escapeHtml(profile.value.title)}</div>` : ''}
          <div class="contact-grid">${contactHtml}</div>
          ${profile.value.target ? `<div class="target-line"><strong>求职意向：</strong>${escapeHtml(profile.value.target)}</div>` : ''}
        </div>
        ${profile.value.avatar ? `<div class="avatar-box"><img src="${escapeAttr(profile.value.avatar)}" alt="头像" /></div>` : ''}
      </header>
      ${profile.value.summary ? `<section class="resume-section"><h2>个人优势</h2><p class="summary-text">${escapeHtml(profile.value.summary)}</p></section>` : ''}
      ${education.value.length ? `<section class="resume-section"><h2>教育经历</h2>${educationHtml}</section>` : ''}
      ${works.value.length ? `<section class="resume-section"><h2>工作经历</h2>${worksHtml}</section>` : ''}
      ${skillLines.value.length ? `<section class="resume-section"><h2>专业技能</h2>${buildListHtml(skillLines.value, 'resume-list skill-list')}</section>` : ''}
      ${projects.value.length ? `<section class="resume-section"><h2>项目经历</h2>${projectHtml}</section>` : ''}
    </article>
  `
}

function buildListHtml(lines, className = 'resume-list') {
  if (!lines.length) return ''
  return `<ul class="${className}">${lines.map((line) => `<li>${escapeHtml(line)}</li>`).join('')}</ul>`
}

/**
 * 预览用 HTML：剥掉 <script> 与内联事件。
 *
 * 预览 iframe 的 sandbox 只给 allow-same-origin、不给 allow-scripts，
 * 浏览器会为每段脚本打印 "Blocked script execution in 'about:srcdoc'"，
 * 简历模板里带点脚本就刷屏。预览本来也不需要执行脚本，直接剔除更干净。
 *
 * 注意：只影响**预览**；导出/保存仍走原始 sourceHtml，不会被这里改动。
 */
function stripScriptsForPreview(html) {
  return String(html || '')
    .replace(/<script[^>]*>[^]*?<\/script>/gi, '')
    .replace(/<script[^>]*\/>/gi, '')
    .replace(/\son[a-z]+\s*=\s*"[^"]*"/gi, '')
    .replace(/\son[a-z]+\s*=\s*'[^']*'/gi, '')
}

function normalizeStandaloneHtml(html) {
  const content = String(html || '').trim()
  if (!content) return buildStandaloneHtml()
  if (/<!doctype html/i.test(content) || /<html[\s>]/i.test(content)) return content
  return [
    '<!doctype html>',
    '<html lang="zh-CN">',
    '<head>',
    '<meta charset="UTF-8" />',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    `<title>${escapeHtml(getFileBaseName())}</title>`,
    '</head>',
    '<body>',
    content,
    '</body>',
    '</html>',
  ].join('\n')
}

function syncSourceFromForm() {
  const templateId = `${BUILTIN_TEMPLATE_PREFIX}${selectedThemeKey.value}`
  selectedTemplateId.value = templateId
  sourceHtml.value = buildVariableTemplateHtml(selectedThemeKey.value)
  sourceHtmlPath.value = ''
  logger.info('[Resume] source synced from form', { bytes: sourceHtml.value.length, templateId })
  message.success('已同步当前表单生成的 HTML')
}

function buildVariableTemplateHtml(themeKey = selectedThemeKey.value) {
  const resolvedThemeKey = themes.some((theme) => theme.value === themeKey) ? themeKey : themes[0].value
  const themeClass = `resume-theme-${resolvedThemeKey}`
  logger.debug('[Resume] build commented variable template html', {
    theme: resolvedThemeKey,
    themeClass,
  })
  return [
    '<!doctype html>',
    '<html lang="zh-CN">',
    '<head>',
    '  <meta charset="UTF-8" />',
    '  <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    '  <title>{{ profile.name }}-{{ profile.target }}</title>',
    '',
    '  <!--',
    '    Guaxi 简历模板编辑说明',
    '    1. 只想改颜色、字号、间距、头像大小：优先修改下方 <style> 中的 CSS 变量或对应模块样式。',
    '    2. 想调整简历模块顺序：移动 <body> 里的 {{ educationSection }}、{{ worksSection }} 等模块占位符。',
    '    3. 想隐藏某个模块：删除或注释对应模块占位符即可，表单数据不会被修改。',
    '    4. 变量由表单自动注入，建议只改结构和 CSS，避免把用户资料写死在模板里。',
    '    5. 想强制分页：在任意两个模块之间插入 <div class="page-break"></div>。',
    '    6. 想避免某段内容被分页拆开：给外层元素加 class="avoid-break"。',
    '',
    '    可用基础变量：',
    '    - {{ profile.name }}：姓名',
    '    - {{ profile.title }}：当前职位 / 身份',
    '    - {{ profile.target }}：求职意向',
    '    - {{ profile.summary }}：个人优势原始文本',
    '',
    '    可用 HTML 片段变量：',
    '    - {{ profileTitleBlock }}：已排版的职位行',
    '    - {{ contactItemsHtml }}：联系方式列表',
    '    - {{ targetBlock }}：求职意向行',
    '    - {{ avatarBlock }}：头像区域',
    '    - {{ summarySection }} / {{ educationSection }} / {{ worksSection }} / {{ skillsSection }} / {{ projectsSection }}：完整模块',
    '  -->',
    '',
    `<style>${standaloneResumeCss}</style>`,
    '</head>',
    '<body>',
    '  <!-- 页面主体：主题类控制整份简历的配色；可切换为 resume-theme-blueprint / resume-theme-graphite / resume-theme-emerald。 -->',
    `  <article class="resume-paper ${themeClass}">`,
    '',
    '    <!-- 01 头部信息：姓名、职位、联系方式、求职意向、头像。改双栏比例、头像尺寸请看 CSS 的“头部区域”。 -->',
    '    <header class="resume-header">',
    '      <div class="resume-profile">',
    '        <h1>{{ profile.name }}</h1>',
    '        {{ profileTitleBlock }}',
    '        <div class="contact-grid">{{ contactItemsHtml }}</div>',
    '        {{ targetBlock }}',
    '      </div>',
    '      {{ avatarBlock }}',
    '    </header>',
    '',
    '    <!-- 分页控制示例：需要从下一页开始时，取消下面这一行注释即可。 -->',
    '    <!-- <div class="page-break"></div> -->',
    '',
    '    <!-- 02 个人优势：修改段落宽度、行高、颜色请看 CSS 的“正文段落”。 -->',
    '    {{ summarySection }}',
    '',
    '    <!-- 03 教育经历：修改时间列宽和学校/学院/专业排布请看 CSS 的“时间轴与多列信息”。 -->',
    '    {{ educationSection }}',
    '',
    '    <!-- 04 工作经历：修改公司、部门、岗位与职责列表样式请看 CSS 的“时间轴与列表”。 -->',
    '    {{ worksSection }}',
    '',
    '    <!-- 05 专业技能：修改技能列数、项目符号、缩进请看 CSS 的“技能列表”。 -->',
    '    {{ skillsSection }}',
    '',
    '    <!-- 06 项目经历：修改项目标题、描述、职责/技术列表请看 CSS 的“项目经历”。 -->',
    '    {{ projectsSection }}',
    '  </article>',
    '</body>',
    '</html>',
  ].join('\n')
}

function renderResumeTemplate(template) {
  const source = String(template || '').trim()
  if (!source) return buildStandaloneHtml()
  const variables = buildTemplateVariables()
  return source.replace(/\{\{\s*([\w.[\]-]+)\s*\}\}/g, (_, key) => {
    if (Object.prototype.hasOwnProperty.call(variables, key)) {
      return variables[key]
    }
    const value = resolveTemplatePath(key)
    if (value == null) {
      logger.warn('[Resume] unknown template variable', { key })
      return ''
    }
    return escapeHtml(value)
  })
}

function buildTemplateVariables() {
  const profileTitleBlock = profile.value.title
    ? `<div class="resume-role">${escapeHtml(profile.value.title)}</div>`
    : ''
  const targetBlock = profile.value.target
    ? `<div class="target-line"><strong>求职意向：</strong>${escapeHtml(profile.value.target)}</div>`
    : ''
  const avatarBlock = profile.value.avatar
    ? `<div class="avatar-box"><img src="${escapeAttr(profile.value.avatar)}" alt="头像" /></div>`
    : ''
  const contactItemsHtml = contactItems.value.map((item) => (
    `<div class="contact-item"><strong>${escapeHtml(item.label)}：</strong><span>${escapeHtml(item.value)}</span></div>`
  )).join('')

  const educationHtml = education.value.map((item) => `
    <div class="timeline-row education-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="inline-meta">
          <strong>${escapeHtml(item.school)}</strong>
          <span>${escapeHtml(item.college)}</span>
          <span>${escapeHtml(item.major)}</span>
          <span>${escapeHtml(item.degree)}</span>
        </div>
      </div>
    </div>
  `).join('')

  const worksHtml = works.value.map((item) => `
    <div class="timeline-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="inline-meta">
          <strong>${escapeHtml(item.company)}</strong>
          <span>${escapeHtml(item.department)}</span>
          <span>${escapeHtml(item.role)}</span>
        </div>
        ${buildListHtml(toLines(item.description))}
      </div>
    </div>
  `).join('')

  const projectsHtml = projects.value.map((item) => `
    <div class="timeline-row project-row">
      <div class="time-col">${escapeHtml(item.period)}</div>
      <div class="content-col">
        <div class="project-title">${escapeHtml(item.name)}</div>
        ${item.description ? `<p class="project-desc">${escapeHtml(item.description)}</p>` : ''}
        ${buildListHtml(toLines(item.details))}
      </div>
    </div>
  `).join('')

  return {
    'profile.name': escapeHtml(profile.value.name || '姓名'),
    'profile.title': escapeHtml(profile.value.title),
    'profile.city': escapeHtml(profile.value.city),
    'profile.phone': escapeHtml(profile.value.phone),
    'profile.email': escapeHtml(profile.value.email),
    'profile.website': escapeHtml(profile.value.website),
    'profile.target': escapeHtml(profile.value.target),
    'profile.summary': escapeHtml(profile.value.summary),
    'profile.avatar': escapeAttr(profile.value.avatar),
    skillsText: escapeHtml(skillsText.value),
    profileTitleBlock,
    targetBlock,
    avatarBlock,
    contactItemsHtml,
    educationHtml,
    worksHtml,
    skillsHtml: buildListHtml(skillLines.value, 'resume-list skill-list'),
    projectsHtml,
    summarySection: profile.value.summary
      ? `<section class="resume-section"><h2>个人优势</h2><p class="summary-text">${escapeHtml(profile.value.summary)}</p></section>`
      : '',
    educationSection: education.value.length
      ? `<section class="resume-section"><h2>教育经历</h2>${educationHtml}</section>`
      : '',
    worksSection: works.value.length
      ? `<section class="resume-section"><h2>工作经历</h2>${worksHtml}</section>`
      : '',
    skillsSection: skillLines.value.length
      ? `<section class="resume-section"><h2>专业技能</h2>${buildListHtml(skillLines.value, 'resume-list skill-list')}</section>`
      : '',
    projectsSection: projects.value.length
      ? `<section class="resume-section"><h2>项目经历</h2>${projectsHtml}</section>`
      : '',
  }
}

function resolveTemplatePath(key) {
  const path = String(key || '').replace(/\[(\d+)\]/g, '.$1').split('.').filter(Boolean)
  let cursor = {
    profile: profile.value,
    education: education.value,
    works: works.value,
    projects: projects.value,
    skills: skillLines.value,
  }
  for (const part of path) {
    if (cursor == null) return null
    cursor = cursor[part]
  }
  return cursor
}

function downloadHtml() {
  try {
    const html = getCurrentExportHtml()
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${getFileBaseName()}.html`
    link.click()
    URL.revokeObjectURL(url)
    logger.info('[Resume] html exported', { bytes: html.length, mode: activeMode.value, theme: selectedThemeKey.value })
    message.success('HTML 简历已生成')
  } catch (error) {
    logger.error('[Resume] html export failed', error)
    message.error('HTML 导出失败')
  }
}

function handleExportSelect(key) {
  logger.info('[Resume] export menu selected', { key, mode: activeMode.value })
  if (key === 'html') {
    downloadHtml()
    return
  }
  if (key === 'pdf') {
    exportPdf()
  }
}

async function openSourceWithEditor(editorKey) {
  externalEditor.value = editorKey || 'default'
  logger.info('[Resume] external editor menu selected', { editor: externalEditor.value })
  await openCurrentSourceExternally()
}

async function openCurrentSourceExternally() {
  try {
    if (!sourceHtml.value.trim()) syncSourceFromForm()
    const html = normalizeStandaloneHtml(sourceHtml.value)
    const target = await getCurrentSourceEditTarget()
    if (target.baseDir) {
      await mkdir(EXTERNAL_EDIT_DIR, { baseDir: target.baseDir, recursive: true })
      await writeTextFile(target.writePath, html, { baseDir: target.baseDir })
    } else {
      await writeTextFile(target.writePath, html)
    }
    addSourceHistoryVersion(`外部编辑前：${getFileBaseName()}`, sourceHtml.value)
    logger.info('[Resume] source html prepared for external editor', {
      path: target.openPath,
      writePath: target.writePath,
      editor: externalEditor.value,
      bytes: html.length,
    })
    await openHtmlInExternalEditor(target.openPath)
    message.success('已打开外部编辑器')
  } catch (error) {
    logger.error('[Resume] open external editor failed', error)
    message.error('打开外部编辑器失败')
  }
}

async function getCurrentSourceEditTarget() {
  // 设计思路：
  // 1. 外部编辑统一写入 AppData/resume-builder，避免 Temp 权限和用户任意路径 scope 不一致。
  // 2. 即使源码来自导入文件，也先写当前编辑器内容到这个受控路径，再交给外部编辑器打开。
  const fileName = `${getFileBaseName()}-guaxi-current-template.html`
  const writePath = `${EXTERNAL_EDIT_DIR}/${fileName}`
  const openPath = await join(await appDataDir(), writePath)
  externalEditPath.value = openPath
  logger.info('[Resume] external edit app data path prepared', {
    path: openPath,
    writePath,
    sourceHtmlPath: sourceHtmlPath.value,
  })
  return {
    writePath,
    openPath,
    baseDir: BaseDirectory.AppData,
  }
}

async function openHtmlInExternalEditor(path) {
  const editor = externalEditor.value
  if (editor === 'vscode') {
    try {
      await openPath(path, 'code')
      logger.info('[Resume] source html opened in vscode', { path })
      return
    } catch (error) {
      logger.warn('[Resume] open vscode command failed, trying vscode url', { path, error })
    }

    try {
      await openUrl(toVscodeFileUrl(path))
      logger.info('[Resume] source html opened with vscode url', { path })
      return
    } catch (error) {
      logger.warn('[Resume] open vscode url failed, fallback to default app', { path, error })
    }
  }

  if (editor === 'neovim') {
    const candidates = ['neovide', 'nvim-qt', 'goneovim', 'nvim', 'neovim']
    for (const candidate of candidates) {
      try {
        await openPath(path, candidate)
        logger.info('[Resume] source html opened in neovim', { path, candidate })
        return
      } catch (error) {
        logger.warn('[Resume] open neovim candidate failed', { path, candidate, error })
      }
    }
  }

  await openPath(path)
  logger.info('[Resume] source html opened with default app', { path, preferred: editor })
}

function toVscodeFileUrl(path) {
  const normalizedPath = String(path).replace(/\\/g, '/')
  const prefixedPath = normalizedPath.startsWith('/') ? normalizedPath : `/${normalizedPath}`
  return `vscode://file${encodeURI(prefixedPath)}`
}

function saveCurrentSourceDocument() {
  if (!sourceHtml.value.trim()) syncSourceFromForm()
  const normalizedHtml = normalizeStandaloneHtml(sourceHtml.value)
  let templateSaveAction = 'none'
  const customTemplate = customTemplates.value.find((template) => template.id === selectedTemplateId.value)
  if (customTemplate) {
    if (normalizeHistoryHtml(customTemplate.html) !== normalizeHistoryHtml(normalizedHtml)) {
      customTemplates.value = customTemplates.value.map((template) => (
        template.id === customTemplate.id
          ? { ...template, html: normalizedHtml, updatedAt: new Date().toISOString() }
          : template
      ))
      logger.info('[Resume] custom template updated from current source', {
        id: customTemplate.id,
        name: customTemplate.name,
        bytes: normalizedHtml.length,
      })
      templateSaveAction = 'update'
    } else {
      logger.info('[Resume] custom template update skipped because content is unchanged', {
        id: customTemplate.id,
        name: customTemplate.name,
      })
      templateSaveAction = 'skip'
    }
  } else if (isBuiltinTemplateId(selectedTemplateId.value) && isCurrentSourceDifferentFromBuiltin(normalizedHtml)) {
    const previousTemplateId = selectedTemplateId.value
    const template = createCustomTemplateFromSource(normalizedHtml)
    customTemplates.value.unshift(template)
    selectedTemplateId.value = template.id
    templateSaveAction = 'fork'
    logger.info('[Resume] builtin template forked into custom template', {
      id: template.id,
      name: template.name,
      builtinTemplateId: previousTemplateId,
      bytes: normalizedHtml.length,
    })
  }

  const item = addSourceHistoryVersion(`${getFileBaseName()} · ${formatHistoryTime(new Date())}`, sourceHtml.value)
  selectedHistoryId.value = item.id
  if (lastHistorySaveCreated) historyDrawerVisible.value = true
  logger.info('[Resume] current source document saved', {
    historyId: item.id,
    templateId: selectedTemplateId.value,
    templateSaveAction,
    historyCreated: lastHistorySaveCreated,
  })
  if (!lastHistorySaveCreated) {
    message.info('内容没有变化，未新增历史版本')
    return
  }
  if (templateSaveAction === 'fork') {
    message.success('已保存源文件，并从默认模板创建可编辑模板')
    return
  }
  message.success(customTemplate ? '已保存源文件并更新自定义模板' : '已保存简历源文件')
}

function isCurrentSourceDifferentFromBuiltin(html) {
  if (!isBuiltinTemplateId(selectedTemplateId.value)) return false
  const themeKey = getBuiltinTemplateThemeKey(selectedTemplateId.value)
  const builtinHtml = normalizeStandaloneHtml(buildVariableTemplateHtml(themeKey))
  const changed = normalizeHistoryHtml(builtinHtml) !== normalizeHistoryHtml(html)
  logger.debug('[Resume] builtin template diff checked', {
    templateId: selectedTemplateId.value,
    changed,
    bytes: String(html || '').length,
  })
  return changed
}

function createCustomTemplateFromSource(html) {
  const now = new Date()
  return {
    id: createId(),
    name: `${getFileBaseName()} · ${formatHistoryTime(now)}`,
    html,
    createdAt: now.toISOString(),
    updatedAt: now.toISOString(),
  }
}

function addSourceHistoryVersion(name, html) {
  const normalizedHtml = normalizeStandaloneHtml(html)
  const latestHistory = sourceHistories.value[0]
  if (latestHistory && normalizeHistoryHtml(latestHistory.html) === normalizeHistoryHtml(normalizedHtml)) {
    lastHistorySaveCreated = false
    logger.info('[Resume] source history save skipped because latest version is unchanged', {
      latestId: latestHistory.id,
      latestName: latestHistory.name,
      bytes: normalizedHtml.length,
    })
    return latestHistory
  }

  const item = {
    id: createId(),
    name,
    html: normalizedHtml,
    createdAt: new Date().toISOString(),
  }
  lastHistorySaveCreated = true
  sourceHistories.value = [item, ...sourceHistories.value].slice(0, 40)
  logger.info('[Resume] source history saved', { id: item.id, name: item.name, bytes: item.html.length })
  return item
}

function normalizeHistoryHtml(html) {
  return normalizeStandaloneHtml(html)
    .replace(/\r\n/g, '\n')
    .trim()
}

function openHistoryDrawer() {
  historyDrawerVisible.value = true
  logger.info('[Resume] source history drawer opened', { count: sourceHistories.value.length })
}

function openHistoryItem(history) {
  if (!history) return
  sourceHtml.value = history.html
  sourceHtmlPath.value = ''
  selectedTemplateId.value = null
  selectedHistoryId.value = history.id
  setActiveMode('source')
  historyDrawerVisible.value = false
  logger.info('[Resume] source history opened', { id: history.id, name: history.name })
  message.success('已打开历史版本')
}

function removeHistoryItem(id) {
  if (!id) return
  const history = sourceHistories.value.find((item) => item.id === id)
  sourceHistories.value = sourceHistories.value.filter((item) => item.id !== id)
  if (selectedHistoryId.value === id) selectedHistoryId.value = null
  logger.info('[Resume] source history removed', { id, name: history?.name })
  message.success('历史版本已删除')
}

async function importHtmlTemplate() {
  try {
    const picked = await open({
      multiple: false,
      filters: [{ name: 'HTML', extensions: ['html', 'htm'] }],
    })
    if (!picked) return
    const path = Array.isArray(picked) ? picked[0] : picked
    const html = await readTextFile(String(path))
    const template = {
      id: createId(),
      name: getTemplateNameFromPath(String(path)),
      html: normalizeStandaloneHtml(html),
      createdAt: new Date().toISOString(),
    }
    customTemplates.value.unshift(template)
    selectedTemplateId.value = template.id
    sourceHtml.value = template.html
    sourceHtmlPath.value = String(path)
    setActiveMode('source')
    logger.info('[Resume] html template imported', { path: String(path), templateId: template.id, bytes: sourceHtml.value.length })
    message.success('已导入 HTML 模板')
  } catch (error) {
    logger.error('[Resume] import html template failed', error)
    message.error('导入 HTML 模板失败')
  }
}

function applyTemplateById(id) {
  if (!id) return
  selectedTemplateId.value = id
  if (isBuiltinTemplateId(id)) {
    const themeKey = getBuiltinTemplateThemeKey(id)
    const theme = themes.find((item) => item.value === themeKey) || themes[0]
    selectedThemeKey.value = theme.value
    sourceHtml.value = buildVariableTemplateHtml(theme.value)
    sourceHtmlPath.value = ''
    setActiveMode('source')
    logger.info('[Resume] builtin template applied', { id, theme: theme.value })
    message.success('已加载默认模板')
    return
  }

  const template = customTemplates.value.find((item) => item.id === id)
  if (!template) return
  sourceHtml.value = template.html
  sourceHtmlPath.value = ''
  setActiveMode('source')
  logger.info('[Resume] custom template applied', { id: template.id, name: template.name })
  message.success('已加载自定义模板')
}

function removeSelectedTemplate() {
  const id = selectedTemplateId.value
  if (!id) return
  if (isBuiltinTemplateId(id)) {
    logger.warn('[Resume] refuse removing builtin template', { id })
    message.warning('默认模板不能删除')
    return
  }
  const template = customTemplates.value.find((item) => item.id === id)
  customTemplates.value = customTemplates.value.filter((item) => item.id !== id)
  selectedTemplateId.value = `${BUILTIN_TEMPLATE_PREFIX}${selectedThemeKey.value}`
  logger.info('[Resume] custom template removed', { id, name: template?.name })
  message.success('模板已删除')
}

async function exportPdf() {
  logger.info('[Resume] print export requested', {
    mode: activeMode.value,
    theme: selectedThemeKey.value,
    sections: {
      education: education.value.length,
      work: works.value.length,
      project: projects.value.length,
      skills: skillLines.value.length,
    },
  })

  const oldTitle = document.title
  document.title = getFileBaseName()
  message.info('正在打开系统打印窗口')

  try {
    const printPayload = buildPrintPayload(getCurrentExportHtml())
    applyPrintStyle(printPayload.styleText)
    printMarkup.value = printPayload.bodyHtml
    await nextTick()

    let cleanupTimer = null
    const cleanupPrint = () => {
      if (cleanupTimer) {
        window.clearTimeout(cleanupTimer)
        cleanupTimer = null
      }
      window.removeEventListener('afterprint', cleanupPrint)
      printMarkup.value = ''
      removePrintStyle()
      document.title = oldTitle
      logger.debug('[Resume] print export cleanup completed')
    }
    window.addEventListener('afterprint', cleanupPrint, { once: true })
    cleanupTimer = window.setTimeout(cleanupPrint, 30000)

    window.setTimeout(() => {
      try {
        window.print()
        logger.info('[Resume] main window print invoked', {
          bodyBytes: printMarkup.value.length,
          styleBytes: printPayload.styleText.length,
        })
      } catch (error) {
        cleanupPrint()
        logger.error('[Resume] main window print failed', error)
        message.error('PDF 导出失败')
      }
    }, 80)
  } catch (error) {
    printMarkup.value = ''
    removePrintStyle()
    document.title = oldTitle
    logger.error('[Resume] print export failed', error)
    message.error('PDF 导出失败')
  }
}

function buildPrintPayload(html) {
  const source = normalizeStandaloneHtml(html)
  const parsed = parsePrintDocument(source)
  const bodyHtml = sanitizePrintBodyHtml(parsed.bodyHtml)
  const wrappedBodyHtml = [
    '<div class="resume-print-root">',
    bodyHtml,
    '</div>',
  ].join('\n')

  logger.debug('[Resume] print payload prepared', {
    bytes: source.length,
    styleBytes: parsed.styleText.length,
    bodyBytes: bodyHtml.length,
    parser: parsed.parser,
    root: parsed.root,
  })

  return {
    bodyHtml: wrappedBodyHtml,
    styleText: parsed.styleText,
  }
}

function parsePrintDocument(source) {
  // Use DOMParser instead of regex so template comments containing "<style>" or "<body>" are not treated as tags.
  if (typeof DOMParser !== 'undefined') {
    const doc = new DOMParser().parseFromString(source, 'text/html')
    const styleText = Array.from(doc.head.querySelectorAll('style'))
      .map((styleElement) => styleElement.textContent || '')
      .join('\n')
    const resumeRoot = doc.body.querySelector('.resume-paper')
    if (resumeRoot) {
      return {
        styleText,
        bodyHtml: resumeRoot.outerHTML,
        parser: 'domparser',
        root: '.resume-paper',
      }
    }

    logger.warn('[Resume] print root .resume-paper not found, fallback to parsed body', {
      bodyBytes: doc.body.innerHTML.length,
    })
    return {
      styleText,
      bodyHtml: doc.body.innerHTML,
      parser: 'domparser',
      root: 'body',
    }
  }

  const styleText = Array.from(source.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi))
    .map((match) => match[1] || '')
    .join('\n')
  const bodyMatch = source.match(/<body\b[^>]*>([\s\S]*?)<\/body>/i)
  logger.warn('[Resume] DOMParser unavailable, fallback to conservative regex print parse')
  return {
    styleText,
    bodyHtml: bodyMatch ? bodyMatch[1] : source,
    parser: 'regex-fallback',
    root: bodyMatch ? 'body' : 'document',
  }
}

function sanitizePrintBodyHtml(html) {
  // Print body keeps resume structure only; CSS is attached through textContent to avoid Trusted Types blocks.
  return String(html || '')
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, '')
    .replace(/<\/?(?:html|head|body|meta|title|link)\b[^>]*>/gi, '')
}

function applyPrintStyle(styleText) {
  removePrintStyle()
  const css = String(styleText || '').trim()
  if (!css) {
    logger.debug('[Resume] print style skipped because template has no style block')
    return
  }

  const styleElement = document.createElement('style')
  styleElement.setAttribute('data-guaxi-resume-print-style', 'true')
  styleElement.textContent = css
  document.head.appendChild(styleElement)
  activePrintStyleElement = styleElement
  logger.debug('[Resume] print style attached', { bytes: css.length })
}

function removePrintStyle() {
  if (!activePrintStyleElement) return
  activePrintStyleElement.remove()
  activePrintStyleElement = null
  logger.debug('[Resume] print style removed')
}

function loadCustomTemplates() {
  try {
    const raw = localStorage.getItem(CUSTOM_TEMPLATE_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed)
      ? parsed.filter((item) => item?.id && item?.name && item?.html)
      : []
  } catch (error) {
    logger.warn('[Resume] load custom templates failed', error)
    return []
  }
}

function loadSourceHistories() {
  try {
    const raw = localStorage.getItem(HISTORY_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed)
      ? parsed.filter((item) => item?.id && item?.name && item?.html).slice(0, 40)
      : []
  } catch (error) {
    logger.warn('[Resume] load source histories failed', error)
    return []
  }
}

function formatHistoryTime(date) {
  const pad = (value) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function formatHistoryDisplay(value) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return formatHistoryTime(date)
}

function getTemplateNameFromPath(path) {
  const fileName = String(path || '').split(/[\\/]/).filter(Boolean).pop() || '导入模板'
  return fileName.replace(/\.(html?|HTML?)$/, '') || '导入模板'
}

function escapeHtml(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

function escapeAttr(value) {
  return escapeHtml(value).replace(/`/g, '&#96;')
}

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleGlobalShortcut)
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  removePrintStyle()
  restoreResizeInteraction()
  disposeSourceMonacoEditor()
})

const standaloneResumeCss = `
/*
  ========== 使用入口 ==========
  - 想快速换主色：修改 .resume-paper 里的 --resume-accent 和 --resume-line。
  - 想调整正文阅读感：修改 --resume-text、--resume-muted，以及 .summary-text / .resume-list 的 font-size、line-height。
  - 想改变页面留白：修改 .resume-paper 的 padding。
  - 想调整模块顺序：不要改 CSS，移动 <body> 里的模块占位符即可。
  - 想强制分页：在模块之间插入 <div class="page-break"></div>。
  - 想避免拆分页：给模块外层加 class="avoid-break"。
*/

/* 01 全局与纸张：控制 A4 页面、外边距、主色变量。 */
* { box-sizing: border-box; }
body {
  margin: 0;
  background: #f3f4f6;
  color: #1f2937;
  font-family: "Microsoft YaHei", "PingFang SC", Arial, sans-serif;
}
.resume-paper {
  width: 210mm;
  min-height: 297mm;
  margin: 0 auto;
  padding: 24mm 22mm;
  background: #fff;
  --resume-accent: #2f80c9;
  --resume-text: #1f2937;
  --resume-muted: #6b7280;
  --resume-line: #9dccf5;
}

/* 01.1 打印分页标记：源码中插入 <div class="page-break"></div> 可强制从下一页开始。 */
.page-break {
  display: block;
  height: 0;
  margin: 0;
  padding: 0;
  break-before: page;
  page-break-before: always;
}

/* 01.2 防止拆分：给某个模块加 avoid-break，可尽量避免一段经历被切到两页。 */
.avoid-break,
.resume-section,
.timeline-row {
  break-inside: avoid;
  page-break-inside: avoid;
}

/* 02 主题变量：新增主题时复制一组 resume-theme-*，只改颜色变量即可。 */
.resume-theme-graphite {
  --resume-accent: #374151;
  --resume-text: #111827;
  --resume-muted: #6b7280;
  --resume-line: #c7ccd3;
}
.resume-theme-emerald {
  --resume-accent: #0f8b8d;
  --resume-text: #163233;
  --resume-muted: #5f6f70;
  --resume-line: #9bd7d2;
}

/* 03 头部区域：控制姓名、职位、联系方式、求职意向、头像布局。 */
.resume-header {
  display: grid;
  grid-template-columns: 1fr 34mm;
  gap: 18mm;
  align-items: start;
  border-bottom: 1.5px solid var(--resume-line);
  padding-bottom: 8mm;
}
.resume-profile h1 {
  margin: 0 0 4mm;
  color: var(--resume-accent);
  font-size: 30px;
  line-height: 1.1;
  letter-spacing: 0;
}
.resume-role {
  color: var(--resume-text);
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 5mm;
}
.contact-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 2mm 8mm;
  color: var(--resume-text);
  font-size: 12px;
}
.contact-item strong,
.target-line strong {
  color: #111827;
}
.target-line {
  margin-top: 3mm;
  font-size: 13px;
}
.avatar-box {
  width: 32mm;
  height: 40mm;
  border: 1px solid #d8dee8;
  overflow: hidden;
  background: #f8fafc;
}
.avatar-box img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* 04 模块标题：控制“教育经历 / 工作经历”等标题和横线。 */
.resume-section {
  margin-top: 8mm;
}
.resume-section h2 {
  display: flex;
  align-items: center;
  gap: 5mm;
  margin: 0 0 5mm;
  color: var(--resume-accent);
  font-size: 17px;
  line-height: 1.2;
  font-weight: 700;
  letter-spacing: 0;
}
.resume-section h2::after {
  content: "";
  flex: 1;
  border-top: 1.5px solid var(--resume-line);
}

/* 05 正文段落：控制个人优势、项目描述等长文本的字号和行高。 */
.summary-text,
.project-desc {
  margin: 0 0 2mm;
  color: var(--resume-text);
  font-size: 12.5px;
  line-height: 1.75;
}

/* 06 时间轴与多列信息：控制教育、工作、项目的时间列和内容列。 */
.timeline-row {
  display: grid;
  grid-template-columns: 38mm 1fr;
  gap: 7mm;
  margin-bottom: 4mm;
}
.time-col {
  color: var(--resume-muted);
  font-size: 12.5px;
  font-weight: 700;
  white-space: nowrap;
}
.inline-meta {
  display: grid;
  grid-template-columns: 1.2fr 1fr 1fr;
  gap: 4mm;
  color: var(--resume-muted);
  font-size: 12.5px;
  font-weight: 700;
}
.education-row .inline-meta {
  grid-template-columns: 1.1fr 1fr 1fr 0.5fr;
}
.inline-meta strong {
  color: var(--resume-text);
}

/* 07 列表与技能：控制职责、技能、项目要点的项目符号、缩进和列数。 */
.resume-list {
  margin: 2mm 0 0;
  padding-left: 5mm;
  color: var(--resume-text);
  font-size: 12.5px;
  line-height: 1.75;
}
.resume-list li::marker {
  color: var(--resume-accent);
}
.skill-list {
  columns: 1;
  margin-left: 38mm;
}

/* 08 项目经历：控制项目标题样式，可在这里强化重点项目。 */
.project-title {
  color: var(--resume-text);
  font-weight: 700;
  font-size: 13px;
  margin-bottom: 2mm;
}

/* 09 打印导出：导出 PDF 时保持 A4 尺寸和白色背景。 */
@page { size: A4; margin: 0; }
@media print {
  body { background: #fff; }
  .resume-paper { margin: 0; box-shadow: none; }
  .page-break { break-before: page; page-break-before: always; }
}
`
</script>

<style scoped>
.resume-app {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--guaxi-text-primary, #1f2937);
  background: var(--guaxi-app-bg, #f3f4f6);
  font-family: inherit;
  overflow: hidden;
}

.resume-app :deep(.n-button),
.resume-app :deep(.n-input),
.resume-app :deep(.n-input__input-el),
.resume-app :deep(.n-input__textarea-el),
.resume-app :deep(.n-base-selection-label),
.resume-app :deep(.n-base-selection-input),
.resume-app :deep(.n-tabs-tab) {
  font-size: inherit;
  font-family: inherit;
}

.resume-toolbar {
  display: grid;
  grid-template-columns: minmax(132px, auto) minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--guaxi-border, #e5e7eb);
  background: var(--guaxi-panel-bg, #fff);
}

.toolbar-title-block {
  min-width: 132px;
  min-height: 30px;
  display: grid;
  align-content: center;
}

.toolbar-title {
  font-size: 17px;
  font-weight: 700;
  line-height: 1.2;
  white-space: nowrap;
}

.toolbar-meta {
  margin-top: 2px;
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: nowrap;
  justify-content: flex-end;
  min-width: 0;
  width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 2px;
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, var(--guaxi-text-secondary, #6b7280) 45%, transparent) transparent;
}

.toolbar-actions::-webkit-scrollbar {
  height: 6px;
}

.toolbar-actions::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-text-secondary, #6b7280) 35%, transparent);
}

.toolbar-actions > * {
  flex: 0 0 auto;
}

.toolbar-actions :deep(.n-button__content),
.header-source-actions :deep(.n-button__content) {
  white-space: nowrap;
}

.icon-tool-button {
  width: 34px;
  min-width: 34px;
  padding: 0;
}

.icon-tool-button :deep(.n-button__icon) {
  margin: 0;
}

.mode-switch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
  padding: 3px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 84%, #f8fafc);
}

.theme-select {
  width: 136px;
  flex: 0 0 136px;
}

.header-source-actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-wrap: nowrap;
  flex: 0 0 auto;
  min-width: 0;
  padding: 4px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 86%, #f8fafc);
}

.header-template-select {
  width: 240px;
  flex: 0 0 240px;
}

.resume-workbench {
  flex: 1;
  min-height: 0;
  display: grid;
  gap: 0;
  overflow: hidden;
}

.resume-workbench.preview-hidden {
  gap: 0;
}

.resume-workbench.is-resizing .resume-preview-shell {
  pointer-events: none;
}

.resume-editor {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  border-right: 1px solid var(--guaxi-border, #e5e7eb);
  background: var(--guaxi-panel-bg, #fff);
  overflow: hidden;
  box-sizing: border-box;
}

.form-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
}

.form-tabs,
.form-panel :deep(.n-tabs) {
  flex: 1;
  width: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
}

.form-panel :deep(.n-tabs-nav) {
  flex: 0 0 auto;
  padding: 4px 6px 0;
  min-width: 0;
  box-sizing: border-box;
}

.form-panel :deep(.n-tabs-nav-scroll-wrapper),
.form-panel :deep(.n-tabs-nav-scroll-content),
.form-panel :deep(.n-tabs-nav-scroll-content > div) {
  min-width: 0;
}

.form-panel :deep(.n-tabs-pane-wrapper) {
  flex: 1;
  width: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  box-sizing: border-box;
}

.form-panel :deep(.n-tab-pane) {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 8px 12px 12px 6px;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
}

.resume-editor :deep(.n-input),
.resume-editor :deep(.n-input-wrapper),
.resume-editor :deep(.n-input__input),
.resume-editor :deep(.n-input__textarea),
.resume-editor :deep(.n-base-selection),
.resume-editor :deep(.n-button) {
  box-sizing: border-box;
  max-width: 100%;
  min-width: 0;
}

.resume-editor :deep(.n-input) {
  width: 100%;
}

.form-section + .form-section {
  margin-top: 12px;
}

.section-title-row,
.entry-head,
.editor-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.section-heading {
  color: var(--guaxi-text-primary, #111827);
  font-size: 13px;
  font-weight: 700;
  margin-bottom: 10px;
}

.form-grid {
  display: grid;
  width: 100%;
  min-width: 0;
  gap: 8px;
  box-sizing: border-box;
}

.form-grid.two {
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 210px), 1fr));
}

.field-gap {
  margin-top: 8px;
}

.avatar-tools {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  margin-top: 8px;
}

.hidden-input {
  display: none;
}

.entry-block {
  min-width: 0;
  border-top: 1px solid var(--guaxi-border, #e5e7eb);
  padding: 12px 0 0;
  background: transparent;
  overflow: visible;
  box-sizing: border-box;
}

.entry-block + .entry-block {
  margin-top: 10px;
}

.entry-head {
  margin-bottom: 8px;
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
  font-weight: 700;
}

.editor-footer {
  flex: 0 0 auto;
  padding: 6px 10px;
  border-top: 1px solid var(--guaxi-border, #e5e7eb);
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
  background: var(--guaxi-panel-bg, #fff);
}

.resize-handle {
  display: flex;
  align-items: stretch;
  justify-content: center;
  width: 12px;
  min-height: 0;
  cursor: col-resize;
  touch-action: none;
  user-select: none;
}

.resize-handle span {
  width: 3px;
  margin: 8px 0;
  border-radius: 999px;
  background: var(--guaxi-border, #d1d5db);
  transition: background 0.15s, width 0.15s, box-shadow 0.15s;
}

.resize-handle:hover span,
.resize-handle.resizing span {
  width: 6px;
  background: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.12);
}

.resume-preview-shell {
  min-width: 0;
  min-height: 0;
  border-left: 1px solid var(--guaxi-border, #e5e7eb);
  background: #dfe3ea;
  overflow: auto;
}

.preview-stage {
  min-width: 900px;
  padding: 0;
}

.source-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 6px;
  gap: 10px;
}

.template-select {
  flex: 1 1 260px;
  min-width: 220px;
  max-width: 420px;
}

.source-editor {
  flex: 1;
  min-height: 360px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 4px;
  overflow: hidden;
  background: var(--guaxi-panel-bg, #fff);
}

.source-editor :deep(.monaco-editor),
.source-editor :deep(.monaco-editor .overflow-guard) {
  border-radius: 4px;
}

.source-editor :deep(.monaco-editor .view-overlays .current-line) {
  border-color: transparent !important;
}

.source-editor :deep(.monaco-editor),
.source-editor :deep(.monaco-editor .monaco-mouse-cursor-text) {
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New') !important;
}

.source-preview-frame {
  display: block;
  width: 100%;
  height: 100%;
  min-height: 0;
  border: 0;
  background: #fff;
}

.history-list {
  display: grid;
  gap: 10px;
}

.history-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  background: var(--guaxi-panel-bg, #fff);
}

.history-card-main {
  min-width: 0;
  display: grid;
  gap: 3px;
}

.history-card-main strong {
  overflow: hidden;
  color: var(--guaxi-text-primary, #111827);
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-card-main span,
.history-empty {
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
}

.history-card-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.history-empty {
  padding: 18px 8px;
  text-align: center;
}

.resume-print-host {
  display: none;
}

.resume-print-root {
  background: #fff;
}

.resume-paper {
  width: 210mm;
  min-height: 297mm;
  margin: 0 auto;
  padding: 24mm 22mm;
  background: #fff;
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.18);
  --resume-accent: #2f80c9;
  --resume-text: #1f2937;
  --resume-muted: #6b7280;
  --resume-line: #9dccf5;
  color: var(--resume-text);
  font-family: "Microsoft YaHei", "PingFang SC", Arial, sans-serif;
}

.page-break {
  display: block;
  height: 0;
  margin: 0;
  padding: 0;
  break-before: page;
  page-break-before: always;
}

.avoid-break,
.resume-section,
.timeline-row {
  break-inside: avoid;
  page-break-inside: avoid;
}

.resume-theme-graphite {
  --resume-accent: #374151;
  --resume-text: #111827;
  --resume-muted: #6b7280;
  --resume-line: #c7ccd3;
}

.resume-theme-emerald {
  --resume-accent: #0f8b8d;
  --resume-text: #163233;
  --resume-muted: #5f6f70;
  --resume-line: #9bd7d2;
}

.resume-header {
  display: grid;
  grid-template-columns: 1fr 34mm;
  gap: 18mm;
  align-items: start;
  border-bottom: 1.5px solid var(--resume-line);
  padding-bottom: 8mm;
}

.resume-profile h1 {
  margin: 0 0 4mm;
  color: var(--resume-accent);
  font-size: 30px;
  line-height: 1.1;
  letter-spacing: 0;
}

.resume-role {
  color: var(--resume-text);
  font-size: 14px;
  font-weight: 700;
  margin-bottom: 5mm;
}

.contact-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 2mm 8mm;
  color: var(--resume-text);
  font-size: 12px;
}

.contact-item strong,
.target-line strong {
  color: #111827;
}

.target-line {
  margin-top: 3mm;
  font-size: 13px;
}

.avatar-box {
  width: 32mm;
  height: 40mm;
  border: 1px solid #d8dee8;
  overflow: hidden;
  background: #f8fafc;
}

.avatar-box img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.resume-section {
  margin-top: 8mm;
}

.resume-section h2 {
  display: flex;
  align-items: center;
  gap: 5mm;
  margin: 0 0 5mm;
  color: var(--resume-accent);
  font-size: 17px;
  line-height: 1.2;
  font-weight: 700;
  letter-spacing: 0;
}

.resume-section h2::after {
  content: "";
  flex: 1;
  border-top: 1.5px solid var(--resume-line);
}

.summary-text,
.project-desc {
  margin: 0 0 2mm;
  color: var(--resume-text);
  font-size: 12.5px;
  line-height: 1.75;
}

.timeline-row {
  display: grid;
  grid-template-columns: 38mm 1fr;
  gap: 7mm;
  margin-bottom: 4mm;
}

.time-col {
  color: var(--resume-muted);
  font-size: 12.5px;
  font-weight: 700;
  white-space: nowrap;
}

.inline-meta {
  display: grid;
  grid-template-columns: 1.2fr 1fr 1fr;
  gap: 4mm;
  color: var(--resume-muted);
  font-size: 12.5px;
  font-weight: 700;
}

.education-row .inline-meta {
  grid-template-columns: 1.1fr 1fr 1fr 0.5fr;
}

.inline-meta strong {
  color: var(--resume-text);
}

.resume-list {
  margin: 2mm 0 0;
  padding-left: 5mm;
  color: var(--resume-text);
  font-size: 12.5px;
  line-height: 1.75;
}

.resume-list li::marker {
  color: var(--resume-accent);
}

.skill-list {
  columns: 1;
  margin-left: 38mm;
}

.project-title {
  color: var(--resume-text);
  font-weight: 700;
  font-size: 13px;
  margin-bottom: 2mm;
}

@media (max-width: 980px) {
  .resume-toolbar {
    grid-template-columns: minmax(96px, auto) minmax(0, 1fr);
    gap: 8px;
    padding: 8px;
  }

  .toolbar-title-block {
    min-width: 96px;
  }

  .toolbar-title {
    font-size: 15px;
  }

  .resume-workbench {
    grid-template-columns: minmax(0, 1fr) !important;
  }

  .resize-handle,
  .resume-preview-shell {
    display: none;
  }

  .toolbar-actions {
    justify-content: flex-start;
  }

  .header-source-actions {
    gap: 6px;
    padding: 3px;
  }

  .icon-tool-button {
    width: 32px;
    min-width: 32px;
  }

  .header-template-select {
    width: 184px;
    flex-basis: 184px;
  }
}

@page {
  size: A4;
  margin: 0;
}

@media print {
  :global(body) {
    background: #fff !important;
  }

  :global(body *) {
    visibility: hidden !important;
  }

  .resume-print-host,
  .resume-print-host *,
  .resume-print-root,
  .resume-print-root * {
    visibility: visible !important;
  }

  .resume-print-host {
    display: block !important;
    position: absolute;
    inset: 0 auto auto 0;
    width: 210mm;
    min-height: 297mm;
    background: #fff !important;
  }

  .resume-print-host .resume-paper {
    position: static;
    margin: 0 !important;
    box-shadow: none !important;
  }

  .page-break {
    break-before: page;
    page-break-before: always;
  }
}
</style>
