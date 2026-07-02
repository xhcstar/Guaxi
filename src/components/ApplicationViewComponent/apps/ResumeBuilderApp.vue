<template>
  <div class="resume-app" :style="appStyle">
    <div class="resume-toolbar">
      <div class="toolbar-title-block">
        <div class="toolbar-title">简历生成器</div>
        <div class="toolbar-meta">{{ toolbarMeta }}</div>
      </div>

      <div class="toolbar-actions">
        <div class="mode-switch">
          <n-button
            size="small"
            :type="activeMode === 'form' ? 'primary' : 'default'"
            :secondary="activeMode !== 'form'"
            @click="activeMode = 'form'"
          >
            <template #icon><Icon :icon="fileTextIcon" /></template>
            表单
          </n-button>
          <n-button
            size="small"
            :type="activeMode === 'source' ? 'primary' : 'default'"
            :secondary="activeMode !== 'source'"
            @click="activeMode = 'source'"
          >
            <template #icon><Icon :icon="code2Icon" /></template>
            源码
          </n-button>
        </div>
        <n-select
          v-if="activeMode === 'form'"
          v-model:value="selectedThemeKey"
          class="theme-select"
          size="small"
          :options="themeOptions"
        />
        <div v-if="activeMode === 'source'" class="header-source-actions">
          <n-button size="small" secondary @click="syncSourceFromForm">
            <template #icon><Icon :icon="refreshCwIcon" /></template>
            同步表单
          </n-button>
          <n-button size="small" secondary @click="importHtmlTemplate">
            <template #icon><Icon :icon="uploadIcon" /></template>
            导入
          </n-button>
          <n-button size="small" secondary @click="saveSourceAsTemplate">
            <template #icon><Icon :icon="saveIcon" /></template>
            模板库
          </n-button>
          <n-button size="small" secondary @click="saveSourceHistoryVersion">
            <template #icon><Icon :icon="clock3Icon" /></template>
            保存版本
          </n-button>
          <n-button size="small" type="primary" @click="saveSourceAsHtml">
            <template #icon><Icon :icon="fileTextIcon" /></template>
            保存
          </n-button>
          <n-select
            v-model:value="externalEditor"
            class="external-editor-select"
            size="small"
            :options="externalEditorOptions"
          />
          <n-button size="small" secondary @click="openCurrentSourceExternally">
            <template #icon><Icon :icon="externalLinkIcon" /></template>
            外部编辑
          </n-button>
        </div>
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" secondary @click="togglePreview">
              <template #icon>
                <Icon :icon="showPreview ? panelRightCloseIcon : panelRightOpenIcon" />
              </template>
              {{ showPreview ? '隐藏预览' : '显示预览' }}
            </n-button>
          </template>
          {{ showPreview ? '隐藏预览区，扩大编辑空间' : '恢复右侧预览区' }}
        </n-tooltip>
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" secondary @click="downloadHtml">
              <template #icon><Icon :icon="fileTextIcon" /></template>
              HTML
            </n-button>
          </template>
          下载当前简历 HTML
        </n-tooltip>
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-button size="small" type="primary" @click="exportPdf">
              <template #icon><Icon :icon="printerIcon" /></template>
              PDF
            </n-button>
          </template>
          使用系统打印导出 PDF
        </n-tooltip>
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
          <n-tabs v-model:value="activeTab" type="segment" size="small" animated>
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
                  <n-button secondary @click="pickAvatar">
                    <template #icon><Icon :icon="imageIcon" /></template>
                    头像
                  </n-button>
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
                <n-button size="small" secondary @click="addEducation">
                  <template #icon><Icon :icon="plusIcon" /></template>
                  新增
                </n-button>
              </div>
              <div v-for="(item, index) in education" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>教育 {{ index + 1 }}</span>
                  <n-button quaternary circle size="small" @click="removeEntry(education, index, 'education')">
                    <template #icon><Icon :icon="trash2Icon" /></template>
                  </n-button>
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
                <n-button size="small" secondary @click="addWork">
                  <template #icon><Icon :icon="plusIcon" /></template>
                  新增
                </n-button>
              </div>
              <div v-for="(item, index) in works" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>工作 {{ index + 1 }}</span>
                  <n-button quaternary circle size="small" @click="removeEntry(works, index, 'work')">
                    <template #icon><Icon :icon="trash2Icon" /></template>
                  </n-button>
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
                <n-button size="small" secondary @click="addProject">
                  <template #icon><Icon :icon="plusIcon" /></template>
                  新增
                </n-button>
              </div>
              <div v-for="(item, index) in projects" :key="item.id" class="entry-block">
                <div class="entry-head">
                  <span>项目 {{ index + 1 }}</span>
                  <n-button quaternary circle size="small" @click="removeEntry(projects, index, 'project')">
                    <template #icon><Icon :icon="trash2Icon" /></template>
                  </n-button>
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
            <n-button quaternary size="small" @click="resetDemo">
              <template #icon><Icon :icon="rotateCcwIcon" /></template>
              恢复示例
            </n-button>
            <span>{{ resumeStats }}</span>
          </div>
        </template>

        <template v-else>
          <div class="source-panel">
            <div class="source-topbar">
              <div>
                <div class="section-heading source-heading">HTML 源码编辑</div>
                <div class="source-subtitle">可直接编辑完整 HTML / CSS。使用变量占位时，修改样式不会写死用户资料。</div>
              </div>
            </div>

            <div class="template-row">
              <n-select
                v-model:value="selectedTemplateId"
                class="template-select"
                size="small"
                clearable
                placeholder="选择本地 HTML 模板"
                :options="customTemplateOptions"
                :disabled="!customTemplateOptions.length"
              />
              <n-button size="small" secondary :disabled="!selectedTemplateId" @click="applySelectedTemplate">
                应用模板
              </n-button>
              <n-button size="small" quaternary :disabled="!selectedTemplateId" @click="removeSelectedTemplate">
                删除
              </n-button>
            </div>

            <div class="template-row">
              <n-select
                v-model:value="selectedHistoryId"
                class="template-select"
                size="small"
                clearable
                placeholder="选择历史版本继续修改"
                :options="historyOptions"
                :disabled="!historyOptions.length"
              />
              <n-button size="small" secondary :disabled="!selectedHistoryId" @click="applySelectedHistory">
                加载历史
              </n-button>
              <n-button size="small" quaternary :disabled="!selectedHistoryId" @click="removeSelectedHistory">
                删除
              </n-button>
            </div>

            <div class="variable-guide">
              <span>常用变量</span>
              <code v-for="token in variableTokens" :key="token">{{ token }}</code>
            </div>

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
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { Icon } from '@iconify/vue'
import * as monaco from 'monaco-editor'
import clock3Icon from '@iconify-icons/lucide/clock-3'
import code2Icon from '@iconify-icons/lucide/code-2'
import externalLinkIcon from '@iconify-icons/lucide/external-link'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import imageIcon from '@iconify-icons/lucide/image'
import panelRightCloseIcon from '@iconify-icons/lucide/panel-right-close'
import panelRightOpenIcon from '@iconify-icons/lucide/panel-right-open'
import plusIcon from '@iconify-icons/lucide/plus'
import printerIcon from '@iconify-icons/lucide/printer'
import refreshCwIcon from '@iconify-icons/lucide/refresh-cw'
import rotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import saveIcon from '@iconify-icons/lucide/save'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import uploadIcon from '@iconify-icons/lucide/upload'
import { open, save } from '@tauri-apps/plugin-dialog'
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
const selectedTemplateId = ref(null)
const sourceHistories = ref(loadSourceHistories())
const selectedHistoryId = ref(null)
const externalEditor = ref('vscode')
const sourceEditor = shallowRef(null)
const sourceModel = shallowRef(null)
let sourceEditorContentDisposable = null
let sourceEditorResizeObserver = null
let monacoThemesReady = false
let resizePreviousCursor = ''
let resizePreviousUserSelect = ''

const variableTokens = [
  '{{ profile.name }}',
  '{{ profile.target }}',
  '{{ contactItemsHtml }}',
  '{{ educationSection }}',
  '{{ worksSection }}',
  '{{ skillsSection }}',
  '{{ projectsSection }}',
]

const externalEditorOptions = [
  { label: 'VS Code', value: 'vscode' },
  { label: 'Neovim', value: 'neovim' },
  { label: '系统默认', value: 'default' },
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

const selectedTheme = computed(() => themes.find((theme) => theme.value === selectedThemeKey.value) || themes[0])

const appFontSize = computed(() => {
  const configured = Number(settingsStore.appearanceFontSize)
  return `${Math.max(Number.isFinite(configured) ? configured : 14, 15)}px`
})

const appStyle = computed(() => ({
  fontSize: appFontSize.value,
}))

const monacoTheme = computed(() => (
  settingsStore.appearanceTheme === 'dark' ? 'guaxi-resume-dark' : 'guaxi-resume-light'
))

const toolbarMeta = computed(() => {
  if (activeMode.value === 'source') {
    return `源码模式 · ${sourceHtml.value.length} 字符 · ${customTemplates.value.length} 个模板 · ${sourceHistories.value.length} 个历史版本`
  }
  return `${filledScore.value}% 完成 · ${selectedTheme.value.label}`
})

const workbenchStyle = computed(() => {
  if (!showPreview.value) {
    return { gridTemplateColumns: 'minmax(0, 1fr)' }
  }
  return { gridTemplateColumns: `${editorWidth.value}px 16px minmax(0, 1fr)` }
})

const customTemplateOptions = computed(() => customTemplates.value.map((template) => ({
  label: template.name,
  value: template.id,
})))

const historyOptions = computed(() => sourceHistories.value.map((history) => ({
  label: history.name,
  value: history.id,
})))

const renderedSourceHtml = computed(() => normalizeStandaloneHtml(renderResumeTemplate(sourceHtml.value)))

const createId = () => `${Date.now()}-${Math.random().toString(16).slice(2)}`

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

watch(activeMode, async (mode) => {
  if (mode === 'source' && !sourceHtml.value.trim()) {
    await nextTick()
    syncSourceFromForm()
  }
  if (mode === 'source') {
    await nextTick()
    ensureSourceMonacoEditor()
  }
  logger.info('[Resume] mode changed', { mode })
})

watch(sourceHtml, (value) => {
  const editor = sourceEditor.value
  const model = sourceModel.value
  if (!editor || !model) return
  if (model.getValue() !== value) {
    model.setValue(value)
  }
})

watch([appFontSize, monacoTheme], () => {
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

function getHostFontFamily() {
  try {
    return window.getComputedStyle(document.body).fontFamily || 'Microsoft YaHei, PingFang SC, Arial, sans-serif'
  } catch {
    return 'Microsoft YaHei, PingFang SC, Arial, sans-serif'
  }
}

function ensureSourceMonacoEditor() {
  if (sourceEditor.value || !sourceEditorHostRef.value) {
    layoutSourceEditor()
    return
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
    fontSize: Number.parseInt(appFontSize.value, 10) || 15,
    fontFamily: getHostFontFamily(),
    lineHeight: Math.max((Number.parseInt(appFontSize.value, 10) || 15) + 7, 24),
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
  logger.info('[Resume] Monaco source editor created', { fontSize: appFontSize.value })
}

function updateSourceEditorOptions() {
  const editor = sourceEditor.value
  if (!editor) return
  ensureResumeMonacoThemes()
  const fontSize = Number.parseInt(appFontSize.value, 10) || 15
  monaco.editor.setTheme(monacoTheme.value)
  editor.updateOptions({
    fontSize,
    fontFamily: getHostFontFamily(),
    lineHeight: Math.max(fontSize + 7, 24),
  })
  layoutSourceEditor()
}

function layoutSourceEditor() {
  try {
    sourceEditor.value?.layout()
  } catch (error) {
    logger.warn('[Resume] Monaco layout failed', error)
  }
}

function disposeSourceMonacoEditor() {
  sourceEditorContentDisposable?.dispose?.()
  sourceEditorContentDisposable = null
  sourceEditorResizeObserver?.disconnect?.()
  sourceEditorResizeObserver = null
  sourceEditor.value?.dispose?.()
  sourceEditor.value = null
  sourceModel.value?.dispose?.()
  sourceModel.value = null
  logger.info('[Resume] Monaco source editor disposed')
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
  sourceHtml.value = buildVariableTemplateHtml()
  sourceHtmlPath.value = ''
  logger.info('[Resume] source synced from form', { bytes: sourceHtml.value.length })
  message.success('已同步当前表单生成的 HTML')
}

function buildVariableTemplateHtml() {
  return [
    '<!doctype html>',
    '<html lang="zh-CN">',
    '<head>',
    '<meta charset="UTF-8" />',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    '<title>{{ profile.name }}-{{ profile.target }}</title>',
    `<style>${standaloneResumeCss}</style>`,
    '</head>',
    '<body>',
    `<article class="resume-paper resume-theme-${selectedThemeKey.value}">`,
    '  <header class="resume-header">',
    '    <div class="resume-profile">',
    '      <h1>{{ profile.name }}</h1>',
    '      {{ profileTitleBlock }}',
    '      <div class="contact-grid">{{ contactItemsHtml }}</div>',
    '      {{ targetBlock }}',
    '    </div>',
    '    {{ avatarBlock }}',
    '  </header>',
    '  {{ summarySection }}',
    '  {{ educationSection }}',
    '  {{ worksSection }}',
    '  {{ skillsSection }}',
    '  {{ projectsSection }}',
    '</article>',
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

async function saveSourceAsHtml() {
  try {
    if (!sourceHtml.value.trim()) syncSourceFromForm()
    const outputPath = await save({
      defaultPath: `${getFileBaseName()}-template.html`,
      filters: [{ name: 'HTML', extensions: ['html', 'htm'] }],
    })
    if (!outputPath) return
    const html = normalizeStandaloneHtml(sourceHtml.value)
    await writeTextFile(String(outputPath), html)
    sourceHtmlPath.value = String(outputPath)
    addSourceHistoryVersion(`保存：${getFileBaseName()}`, sourceHtml.value)
    logger.info('[Resume] source html saved', { path: String(outputPath), bytes: html.length })
    message.success('HTML 模板已保存')
  } catch (error) {
    logger.error('[Resume] save source html failed', error)
    message.error('保存 HTML 模板失败')
  }
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
  if (sourceHtmlPath.value) {
    return {
      writePath: sourceHtmlPath.value,
      openPath: sourceHtmlPath.value,
      baseDir: null,
    }
  }

  // 设计思路：未显式保存的源码先落到应用数据目录，既不弹“另存为”，也不扩大 Temp 写权限。
  const fileName = `${getFileBaseName()}-guaxi-current-template.html`
  const writePath = `${EXTERNAL_EDIT_DIR}/${fileName}`
  const openPath = await join(await appDataDir(), writePath)
  externalEditPath.value = openPath
  logger.info('[Resume] external edit app data path prepared', { path: openPath, writePath })
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

function saveSourceHistoryVersion() {
  if (!sourceHtml.value.trim()) syncSourceFromForm()
  const defaultName = `${getFileBaseName()} · ${formatHistoryTime(new Date())}`
  const name = window.prompt('请输入历史版本名称', defaultName)
  if (!name) return
  const item = addSourceHistoryVersion(name.trim() || defaultName, sourceHtml.value)
  selectedHistoryId.value = item.id
  message.success('已保存历史版本')
}

function addSourceHistoryVersion(name, html) {
  const item = {
    id: createId(),
    name,
    html: normalizeStandaloneHtml(html),
    createdAt: new Date().toISOString(),
  }
  sourceHistories.value = [item, ...sourceHistories.value].slice(0, 40)
  logger.info('[Resume] source history saved', { id: item.id, name: item.name, bytes: item.html.length })
  return item
}

function applySelectedHistory() {
  const history = sourceHistories.value.find((item) => item.id === selectedHistoryId.value)
  if (!history) return
  sourceHtml.value = history.html
  sourceHtmlPath.value = ''
  activeMode.value = 'source'
  logger.info('[Resume] source history applied', { id: history.id, name: history.name })
  message.success('已加载历史版本')
}

function removeSelectedHistory() {
  const id = selectedHistoryId.value
  if (!id) return
  const history = sourceHistories.value.find((item) => item.id === id)
  sourceHistories.value = sourceHistories.value.filter((item) => item.id !== id)
  selectedHistoryId.value = null
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
    sourceHtml.value = normalizeStandaloneHtml(html)
    sourceHtmlPath.value = String(path)
    activeMode.value = 'source'
    logger.info('[Resume] html template imported', { path: String(path), bytes: sourceHtml.value.length })
    message.success('已导入 HTML 模板')
  } catch (error) {
    logger.error('[Resume] import html template failed', error)
    message.error('导入 HTML 模板失败')
  }
}

function saveSourceAsTemplate() {
  if (!sourceHtml.value.trim()) syncSourceFromForm()
  const defaultName = `${selectedTheme.value.label}模板`
  const name = window.prompt('请输入模板名称', defaultName)
  if (!name) return
  const template = {
    id: createId(),
    name: name.trim() || defaultName,
    html: normalizeStandaloneHtml(sourceHtml.value),
    createdAt: new Date().toISOString(),
  }
  customTemplates.value.unshift(template)
  selectedTemplateId.value = template.id
  logger.info('[Resume] custom template saved', { id: template.id, name: template.name, bytes: template.html.length })
  message.success('已保存到本地模板库')
}

function applySelectedTemplate() {
  const template = customTemplates.value.find((item) => item.id === selectedTemplateId.value)
  if (!template) return
  sourceHtml.value = template.html
  sourceHtmlPath.value = ''
  activeMode.value = 'source'
  logger.info('[Resume] custom template applied', { id: template.id, name: template.name })
  message.success('已应用模板')
}

function removeSelectedTemplate() {
  const id = selectedTemplateId.value
  if (!id) return
  const template = customTemplates.value.find((item) => item.id === id)
  customTemplates.value = customTemplates.value.filter((item) => item.id !== id)
  selectedTemplateId.value = null
  logger.info('[Resume] custom template removed', { id, name: template?.name })
  message.success('模板已删除')
}

function exportPdf() {
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

  setTimeout(() => {
    if (activeMode.value === 'source') {
      const frameWindow = sourcePreviewRef.value?.contentWindow
      if (frameWindow) {
        frameWindow.focus()
        frameWindow.print()
      } else {
        window.print()
      }
    } else {
      window.print()
    }
    setTimeout(() => {
      document.title = oldTitle
    }, 300)
  }, 120)
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
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  restoreResizeInteraction()
  disposeSourceMonacoEditor()
})

const standaloneResumeCss = `
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
  break-inside: avoid;
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
@page { size: A4; margin: 0; }
@media print {
  body { background: #fff; }
  .resume-paper { margin: 0; box-shadow: none; }
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
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 14px;
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  background: var(--guaxi-panel-bg, #fff);
}

.toolbar-title-block {
  min-width: 180px;
}

.toolbar-title {
  font-size: 17px;
  font-weight: 700;
  line-height: 1.2;
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
  flex-wrap: wrap;
  justify-content: flex-end;
  min-width: 0;
}

.toolbar-actions :deep(.n-button__content),
.header-source-actions :deep(.n-button__content),
.template-row :deep(.n-button__content) {
  white-space: nowrap;
}

.mode-switch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 84%, #f8fafc);
}

.theme-select {
  width: 136px;
}

.header-source-actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
  padding: 4px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 86%, #f8fafc);
}

.external-editor-select {
  width: 116px;
}

.resume-workbench {
  flex: 1;
  min-height: 0;
  display: grid;
  gap: 0;
  padding-top: 14px;
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
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  background: var(--guaxi-panel-bg, #fff);
  overflow: hidden;
}

.resume-editor :deep(.n-tabs) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.resume-editor :deep(.n-tabs-nav) {
  padding: 12px 12px 0;
}

.resume-editor :deep(.n-tab-pane) {
  height: 100%;
  overflow: auto;
  padding: 12px;
}

.resume-editor :deep(.n-input),
.resume-editor :deep(.n-input-wrapper),
.resume-editor :deep(.n-input__input),
.resume-editor :deep(.n-input__textarea) {
  min-width: 0;
}

.form-section + .form-section {
  margin-top: 16px;
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
  gap: 8px;
}

.form-grid.two {
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
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
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  padding: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 88%, #f8fafc);
  overflow: hidden;
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
  padding: 10px 12px;
  border-top: 1px solid var(--guaxi-border, #e5e7eb);
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
}

.resize-handle {
  display: flex;
  align-items: stretch;
  justify-content: center;
  width: 16px;
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
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  background: #dfe3ea;
  overflow: auto;
}

.preview-stage {
  min-width: 900px;
  padding: 24px;
}

.source-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 12px;
  gap: 10px;
}

.source-topbar {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.source-heading {
  margin-bottom: 2px;
}

.source-subtitle {
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
}

.template-row {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
  flex-wrap: wrap;
}

.template-select {
  flex: 1 1 260px;
  min-width: 220px;
  max-width: 420px;
}

.variable-guide {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding: 8px 10px;
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
  color: var(--guaxi-text-secondary, #6b7280);
  font-size: 12px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 90%, #eef2ff);
}

.variable-guide code {
  padding: 2px 6px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  color: var(--guaxi-text-primary, #111827);
  background: var(--guaxi-app-bg, #f8fafc);
  font-family: "Cascadia Code", Consolas, "Liberation Mono", monospace;
  font-size: 11px;
}

.source-editor {
  flex: 1;
  min-height: 360px;
  border: 1px solid var(--guaxi-border, #d1d5db);
  border-radius: 8px;
  overflow: hidden;
  background: var(--guaxi-panel-bg, #fff);
}

.source-editor :deep(.monaco-editor),
.source-editor :deep(.monaco-editor .overflow-guard) {
  border-radius: 8px;
}

.source-editor :deep(.monaco-editor .view-overlays .current-line) {
  border-color: transparent !important;
}

.source-preview-frame {
  display: block;
  width: 100%;
  height: 100%;
  min-height: 0;
  border: 0;
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
  break-inside: avoid;
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

  .resume-paper,
  .resume-paper * {
    visibility: visible !important;
  }

  .resume-paper {
    position: absolute;
    inset: 0 auto auto 0;
    margin: 0 !important;
    box-shadow: none !important;
  }
}
</style>
