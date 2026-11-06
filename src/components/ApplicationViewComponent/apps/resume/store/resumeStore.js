// 简历工坊的状态层。
//
// 组件只操作这里的文档副本；自动保存、撤销重做、版本快照、模板解析都在这一层
// 完成，避免各个面板各自 invoke、各自记保存状态。
import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createDiscreteApi } from 'naive-ui'
import { createLogger } from '../../../../../utils/logger'
import {
  BUILTIN_TEMPLATES,
  buildBuiltinTemplate,
  findBuiltinTemplate,
  isBuiltinTemplateId,
} from '../render/templates'
import {
  createEmptyDocument,
  createSampleDocument,
  createSection as buildSection,
  normalizeDocument,
  uid,
} from '../sectionSchema'
import { documentHasContent } from '../render/renderResumeHtml'

const logger = createLogger('ResumeStore')
const { message } = createDiscreteApi(['message'])

const LEGACY_TEMPLATE_KEY = 'guaxi.resumeBuilder.customTemplates.v1'
const LEGACY_HISTORY_KEY = 'guaxi.resumeBuilder.sourceHistory.v1'
const MIGRATION_KEY = 'guaxi.resume.migrated.v1'
const AUTOSAVE_DELAY = 700
const UNDO_BURST_MS = 450
const AUTO_VERSION_INTERVAL = 10 * 60 * 1000

export function formatTime(value) {
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

export function relativeTime(value) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const diff = Date.now() - date.getTime()
  if (diff < 60_000) return '刚刚'
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} 天前`
  return formatTime(date).slice(0, 10)
}

function normalizeCustomTemplate(template) {
  return {
    id: template.id,
    name: template.name,
    description: template.description || '',
    html: template.html || '',
    accent: '#8a6d4b',
    custom: true,
    createdAt: template.createdAt,
    updatedAt: template.updatedAt,
  }
}

export const useResumeStore = defineStore('resume-workshop', () => {
  const booting = ref(true)
  const bootError = ref('')
  const docs = ref([])
  const deletedDocs = ref([])
  const templates = ref([])
  const doc = ref(null)
  const versions = ref([])
  const saveState = ref('idle')
  const lastSavedAt = ref('')
  const storagePath = ref('')
  const versionsLoading = ref(false)

  let autosaveTimer = null
  let savingPromise = null
  let queuedSave = false
  let queuedManual = false
  let suspended = false
  let committedSnapshot = ''
  let lastSnapshotAt = 0
  let lastAutoVersionAt = 0
  const undoStack = ref([])
  const redoStack = ref([])

  const customTemplates = computed(() => templates.value.map(normalizeCustomTemplate))

  const allTemplates = computed(() => [
    ...BUILTIN_TEMPLATES.map((template) => ({
      ...template,
      custom: false,
      themeKey: template.key,
    })),
    ...customTemplates.value,
  ])

  const activeTemplate = computed(() => {
    const id = doc.value?.templateId
    return allTemplates.value.find((template) => template.id === id)
      || BUILTIN_TEMPLATES[0]
  })

  const activeTemplateHtml = computed(() => activeTemplate.value?.html || buildBuiltinTemplate('blueprint'))

  const activeDocId = computed(() => doc.value?.id || '')

  const docCount = computed(() => docs.value.length)

  const completion = computed(() => {
    const data = doc.value?.data
    if (!data) return { percent: 0, items: [] }
    const profile = data.profile || {}
    const sections = data.sections || []
    const sectionFilled = (type) => {
      const section = sections.find((item) => item.type === type && item.visible !== false)
      if (!section) return false
      return documentHasContent({ data: { profile: {}, sections: [section] } })
    }
    const items = [
      { key: 'name', label: '姓名', done: Boolean(String(profile.name || '').trim()) },
      { key: 'contact', label: '联系方式', done: Boolean(String(profile.phone || '').trim() || String(profile.email || '').trim()) },
      { key: 'target', label: '求职意向', done: Boolean(String(profile.target || '').trim()) },
      { key: 'summary', label: '个人优势', done: Boolean(String(profile.summary || '').trim()) },
      { key: 'education', label: '教育经历', done: sectionFilled('education') },
      { key: 'work', label: '工作经历', done: sectionFilled('work') },
      { key: 'project', label: '项目经历', done: sectionFilled('project') },
      { key: 'skills', label: '专业技能', done: sectionFilled('skills') },
    ]
    const done = items.filter((item) => item.done).length
    return { percent: Math.round((done / items.length) * 100), items }
  })

  function captureSnapshot() {
    if (!doc.value) return ''
    return JSON.stringify({
      title: doc.value.title,
      templateId: doc.value.templateId,
      data: doc.value.data,
    })
  }

  function applySnapshot(raw) {
    if (!doc.value || !raw) return
    try {
      const parsed = JSON.parse(raw)
      suspended = true
      doc.value.title = parsed.title
      doc.value.templateId = parsed.templateId
      doc.value.data = parsed.data
      committedSnapshot = captureSnapshot()
      queueMicrotask(() => { suspended = false })
      scheduleAutosave()
    } catch (error) {
      logger.warn('[Resume] undo snapshot apply failed', error)
    }
  }

  function handleDocumentChange() {
    if (suspended || !doc.value) return
    const current = captureSnapshot()
    if (!current || current === committedSnapshot) return
    const now = Date.now()
    if (now - lastSnapshotAt > UNDO_BURST_MS) {
      undoStack.value.push(committedSnapshot)
      if (undoStack.value.length > 80) undoStack.value.shift()
      redoStack.value = []
    }
    lastSnapshotAt = now
    committedSnapshot = current
    scheduleAutosave()
  }

  watch(() => doc.value?.data, (value) => {
    if (!value) return
    handleDocumentChange()
  }, { deep: true })

  watch(() => [doc.value?.title, doc.value?.templateId], () => {
    handleDocumentChange()
  })

  function scheduleAutosave() {
    if (!doc.value) return
    saveState.value = 'dirty'
    if (autosaveTimer) clearTimeout(autosaveTimer)
    autosaveTimer = setTimeout(() => {
      autosaveTimer = null
      saveNow({ silent: true }).catch((error) => logger.warn('[Resume] autosave failed', error))
    }, AUTOSAVE_DELAY)
  }

  function summarizeFromDoc(record) {
    const data = record.data || {}
    const profile = data.profile || {}
    return {
      id: record.id,
      title: record.title,
      role: profile.target || profile.title || '',
      templateId: record.templateId,
      favorite: Boolean(record.favorite),
      sectionCount: (data.sections || []).length,
      createdAt: record.createdAt,
      updatedAt: record.updatedAt,
      deletedAt: record.deletedAt || '',
    }
  }

  function upsertSummary(record) {
    const summary = summarizeFromDoc(record)
    const index = docs.value.findIndex((item) => item.id === summary.id)
    if (index >= 0) docs.value.splice(index, 1, summary)
    else docs.value.unshift(summary)
    docs.value.sort((a, b) => {
      if (a.favorite !== b.favorite) return a.favorite ? -1 : 1
      return String(b.updatedAt).localeCompare(String(a.updatedAt))
    })
  }

  async function loadDocs() {
    const list = await invoke('resume_list_documents', { includeDeleted: false })
    docs.value = Array.isArray(list) ? list : []
  }

  async function loadDeletedDocs() {
    try {
      const list = await invoke('resume_list_documents', { includeDeleted: true })
      deletedDocs.value = (Array.isArray(list) ? list : []).filter((item) => item.deletedAt)
    } catch (error) {
      logger.warn('[Resume] deleted documents load failed', error)
      deletedDocs.value = []
    }
  }

  async function loadTemplates() {
    const list = await invoke('resume_list_templates')
    templates.value = Array.isArray(list) ? list : []
  }

  async function openDoc(id, { force = false } = {}) {
    if (!id) return
    if (!force && doc.value?.id === id) return
    if (doc.value && saveState.value === 'dirty') {
      try {
        await saveNow({ silent: true, reason: 'switch' })
      } catch {
        // 保存失败也允许切换，未保存的内容仍留在内存里，避免卡住整个界面。
      }
    }
    try {
      const record = await invoke('resume_get_document', { id })
      const parsed = JSON.parse(record.dataJson || '{}')
      suspended = true
      doc.value = {
        ...normalizeDocument({ title: record.title, templateId: record.templateId, data: parsed }),
        id: record.id,
        favorite: Boolean(record.favorite),
        createdAt: record.createdAt,
        updatedAt: record.updatedAt,
      }
      committedSnapshot = captureSnapshot()
      undoStack.value = []
      redoStack.value = []
      lastAutoVersionAt = Date.now()
      saveState.value = 'saved'
      lastSavedAt.value = record.updatedAt
      queueMicrotask(() => { suspended = false })
      await loadVersions()
      logger.info('[Resume] document opened', { id, title: record.title })
    } catch (error) {
      logger.error('[Resume] document open failed', error)
      message.error('打开简历失败')
    }
  }

  async function createDocument({ sample = false, templateId, open = true } = {}) {
    const preset = sample ? createSampleDocument(templateId) : createEmptyDocument(templateId)
    const record = await invoke('resume_save_document', {
      input: {
        id: null,
        title: preset.title,
        templateId: preset.templateId,
        dataJson: JSON.stringify(preset.data),
        favorite: false,
      },
    })
    await loadDocs()
    if (open) await openDoc(record.id, { force: true })
    logger.info('[Resume] document created', { id: record.id, sample })
    return record
  }

  async function saveNow({ reason = 'auto', silent = true } = {}) {
    if (!doc.value) return null
    if (savingPromise) {
      queuedSave = true
      if (reason === 'manual') queuedManual = true
      return savingPromise
    }
    if (autosaveTimer) {
      clearTimeout(autosaveTimer)
      autosaveTimer = null
    }
    saveState.value = 'saving'
    const sourceDoc = doc.value
    // 用户通常先填姓名/意向，再想起改标题：默认标题下自动跟随，省掉一次手动改名。
    const profile = sourceDoc.data?.profile || {}
    if ((!sourceDoc.title || sourceDoc.title === '未命名简历') && String(profile.name || '').trim()) {
      const autoTitle = [profile.name, profile.target].filter((item) => String(item || '').trim()).join(' · ')
      if (autoTitle) {
        suspended = true
        sourceDoc.title = autoTitle.slice(0, 60)
        committedSnapshot = captureSnapshot()
        queueMicrotask(() => { suspended = false })
      }
    }
    savingPromise = (async () => {
      try {
        const record = await invoke('resume_save_document', {
          input: {
            id: sourceDoc.id || null,
            title: sourceDoc.title,
            templateId: sourceDoc.templateId,
            dataJson: JSON.stringify(sourceDoc.data),
            favorite: Boolean(sourceDoc.favorite),
          },
        })
        upsertSummary({
          ...record,
          data: sourceDoc.data,
        })
        // 保存期间用户可能已经切到另一份简历，回写只作用于发起保存的那一份。
        if (doc.value !== sourceDoc) {
          logger.info('[Resume] save completed for background document', { id: record.id })
          return record
        }
        doc.value.id = record.id
        doc.value.updatedAt = record.updatedAt
        lastSavedAt.value = record.updatedAt
        saveState.value = 'saved'
        if (reason === 'manual') {
          await createVersion(`手动保存 · ${formatTime(new Date())}`, 'manual')
          if (!silent) message.success('已保存版本')
        } else if (documentHasContent(doc.value) && Date.now() - lastAutoVersionAt > AUTO_VERSION_INTERVAL) {
          lastAutoVersionAt = Date.now()
          await createVersion(`自动快照 · ${formatTime(new Date())}`, 'auto')
        }
        return record
      } catch (error) {
        saveState.value = 'error'
        logger.error('[Resume] save failed', error)
        if (!silent) message.error('保存简历失败')
        throw error
      } finally {
        savingPromise = null
        if (queuedSave) {
          queuedSave = false
          const manual = queuedManual
          queuedManual = false
          if (manual) saveNow({ reason: 'manual', silent }).catch(() => {})
          else scheduleAutosave()
        }
      }
    })()
    return savingPromise
  }

  function undo() {
    if (!undoStack.value.length) return
    redoStack.value.push(captureSnapshot())
    applySnapshot(undoStack.value.pop())
  }

  function redo() {
    if (!redoStack.value.length) return
    undoStack.value.push(captureSnapshot())
    applySnapshot(redoStack.value.pop())
  }

  async function loadVersions() {
    if (!doc.value?.id) {
      versions.value = []
      return
    }
    versionsLoading.value = true
    try {
      const list = await invoke('resume_list_versions', { resumeId: doc.value.id })
      versions.value = Array.isArray(list) ? list : []
    } catch (error) {
      logger.warn('[Resume] versions load failed', error)
      versions.value = []
    } finally {
      versionsLoading.value = false
    }
  }

  async function createVersion(label, kind = 'manual') {
    if (!doc.value?.id) return null
    const version = await invoke('resume_save_version', {
      input: {
        resumeId: doc.value.id,
        label,
        kind,
        dataJson: JSON.stringify(doc.value.data),
      },
    })
    await loadVersions()
    logger.info('[Resume] version created', { id: version?.id, kind, label })
    return version
  }

  async function restoreVersion(version) {
    if (!version || !doc.value) return
    try {
      const parsed = JSON.parse(version.dataJson || '{}')
      if (documentHasContent(doc.value)) {
        await createVersion(`恢复前备份 · ${formatTime(new Date())}`, 'manual')
      }
      suspended = true
      doc.value.data = normalizeDocument({ title: doc.value.title, templateId: doc.value.templateId, data: parsed }).data
      committedSnapshot = captureSnapshot()
      queueMicrotask(() => { suspended = false })
      await saveNow({ reason: 'restore' })
      message.success('已恢复该版本')
      logger.info('[Resume] version restored', { id: version.id, label: version.label })
    } catch (error) {
      logger.error('[Resume] version restore failed', error)
      message.error('恢复版本失败')
    }
  }

  async function deleteVersion(version) {
    if (!version) return
    await invoke('resume_delete_version', { id: version.id })
    await loadVersions()
  }

  async function pinVersion(version, pinned) {
    if (!version) return
    await invoke('resume_set_version_pinned', { id: version.id, pinned })
    await loadVersions()
  }

  async function duplicateDoc(id) {
    const record = await invoke('resume_duplicate_document', { id })
    await loadDocs()
    await openDoc(record.id, { force: true })
    message.success('已创建副本')
    return record
  }

  async function deleteDoc(id) {
    await invoke('resume_delete_document', { id })
    await loadDocs()
    await loadDeletedDocs()
    if (doc.value?.id === id) {
      doc.value = null
      versions.value = []
      if (docs.value.length) await openDoc(docs.value[0].id, { force: true })
    }
    message.success('已移到回收站')
  }

  async function restoreDoc(id) {
    await invoke('resume_restore_document', { id })
    await loadDocs()
    await loadDeletedDocs()
    message.success('已恢复到简历库')
  }

  async function purgeDoc(id) {
    await invoke('resume_purge_document', { id })
    await loadDeletedDocs()
    message.success('已彻底删除')
  }

  async function toggleFavorite(id, favorite) {
    await invoke('resume_set_favorite', { id, favorite })
    const summary = docs.value.find((item) => item.id === id)
    if (summary) summary.favorite = favorite
    if (doc.value?.id === id) doc.value.favorite = favorite
  }

  async function setTitle(title) {
    if (!doc.value) return
    doc.value.title = title
  }

  async function applyTemplate(templateId) {
    if (!doc.value) return
    doc.value.templateId = templateId
    await saveNow({ reason: 'template' })
  }

  async function saveTemplate({ id = null, name, description = '', html }) {
    const record = await invoke('resume_save_template', {
      input: { id, name, description, html },
    })
    await loadTemplates()
    logger.info('[Resume] template saved', { id: record.id, name: record.name })
    return record
  }

  async function deleteTemplate(id) {
    await invoke('resume_delete_template', { id })
    if (doc.value?.templateId === id) {
      doc.value.templateId = `${BUILTIN_TEMPLATES[0].id}`
      await saveNow({ reason: 'template-reset' })
    }
    await loadTemplates()
  }

  async function ensureEditableTemplate() {
    if (!doc.value) return null
    if (!isBuiltinTemplateId(doc.value.templateId)) {
      const existing = customTemplates.value.find((item) => item.id === doc.value.templateId)
      if (existing) return existing
    }
    const builtin = findBuiltinTemplate(doc.value.templateId) || BUILTIN_TEMPLATES[0]
    const record = await saveTemplate({
      name: `${doc.value.title || '未命名简历'} · 模板`,
      description: '从内置模板复制，仅用于这份简历',
      html: builtin.html,
    })
    await applyTemplate(record.id)
    return normalizeCustomTemplate(record)
  }

  async function updateTemplateHtml(templateId, html) {
    const template = customTemplates.value.find((item) => item.id === templateId)
    if (!template) return
    await saveTemplate({
      id: templateId,
      name: template.name,
      description: template.description,
      html,
    })
  }

  async function resetTemplateToBuiltin(templateId = doc.value?.templateId) {
    const builtin = findBuiltinTemplate(templateId)
    await applyTemplate(builtin ? builtin.id : BUILTIN_TEMPLATES[0].id)
    message.success('已恢复默认模板')
  }

  function serializeBundle({ includeVersions = true } = {}) {
    if (!doc.value) return null
    const template = customTemplates.value.find((item) => item.id === doc.value.templateId) || null
    return {
      kind: 'guaxi-resume',
      version: 1,
      exportedAt: new Date().toISOString(),
      document: {
        title: doc.value.title,
        templateId: doc.value.templateId,
        data: doc.value.data,
      },
      template: template ? { name: template.name, description: template.description, html: template.html } : null,
      versions: includeVersions
        ? versions.value.map((item) => ({
          kind: item.kind,
          label: item.label,
          pinned: item.pinned,
          dataJson: item.dataJson,
          createdAt: item.createdAt,
        }))
        : [],
    }
  }

  async function importBundle(bundle) {
    if (!bundle || bundle.kind !== 'guaxi-resume' || !bundle.document) {
      throw new Error('不是有效的简历文件')
    }
    let templateId = bundle.document.templateId
    if (bundle.template?.html) {
      const record = await saveTemplate({
        name: bundle.template.name || `${bundle.document.title} · 模板`,
        description: bundle.template.description || '从简历文件导入',
        html: bundle.template.html,
      })
      templateId = record.id
    }
    if (isBuiltinTemplateId(templateId) && !findBuiltinTemplate(templateId)) {
      templateId = BUILTIN_TEMPLATES[0].id
    }
    const data = normalizeDocument({ title: bundle.document.title, templateId, data: bundle.document.data })
    const record = await invoke('resume_save_document', {
      input: {
        id: null,
        title: data.title,
        templateId,
        dataJson: JSON.stringify(data.data),
        favorite: false,
      },
    })
    for (const version of Array.isArray(bundle.versions) ? bundle.versions : []) {
      if (!version?.dataJson) continue
      await invoke('resume_save_version', {
        input: {
          resumeId: record.id,
          label: version.label || '导入版本',
          kind: version.kind === 'auto' ? 'auto' : 'manual',
          dataJson: version.dataJson,
        },
      })
    }
    await loadDocs()
    await openDoc(record.id, { force: true })
    message.success('简历已导入')
    return record
  }

  async function migrateLegacy() {
    if (localStorage.getItem(MIGRATION_KEY)) return
    let importedTemplates = 0
    let importedDocs = 0
    try {
      const rawTemplates = localStorage.getItem(LEGACY_TEMPLATE_KEY)
      const legacyTemplates = rawTemplates ? JSON.parse(rawTemplates) : []
      for (const template of Array.isArray(legacyTemplates) ? legacyTemplates.slice(0, 20) : []) {
        if (!template?.html) continue
        await invoke('resume_save_template', {
          input: {
            id: null,
            name: template.name || '旧版模板',
            description: '从旧版简历生成器迁移',
            html: template.html,
          },
        })
        importedTemplates += 1
      }

      const rawHistory = localStorage.getItem(LEGACY_HISTORY_KEY)
      const legacyHistory = rawHistory ? JSON.parse(rawHistory) : []
      for (const history of Array.isArray(legacyHistory) ? legacyHistory.slice(0, 10) : []) {
        if (!history?.html) continue
        const template = await invoke('resume_save_template', {
          input: {
            id: null,
            name: `旧版源码 · ${history.name || '未命名'}`,
            description: '旧版保存的 HTML 源码，作为模板保留',
            html: history.html,
          },
        })
        const empty = createEmptyDocument(template.id)
        await invoke('resume_save_document', {
          input: {
            id: null,
            title: history.name || '旧版简历源码',
            templateId: template.id,
            dataJson: JSON.stringify(empty.data),
            favorite: false,
          },
        })
        importedDocs += 1
      }

      localStorage.setItem(MIGRATION_KEY, new Date().toISOString())
      if (importedTemplates || importedDocs) {
        await loadTemplates()
        await loadDocs()
        logger.info('[Resume] legacy data migrated', { importedTemplates, importedDocs })
      }
    } catch (error) {
      logger.warn('[Resume] legacy migration skipped', error)
    }
  }

  async function boot() {
    booting.value = true
    bootError.value = ''
    try {
      const info = await invoke('resume_init')
      storagePath.value = info?.dbPath || ''
      await loadTemplates()
      await loadDocs()
      await loadDeletedDocs()
      await migrateLegacy()
      if (docs.value.length) {
        await openDoc(docs.value[0].id, { force: true })
      } else {
        await createDocument({ sample: true, open: true })
      }
      logger.info('[Resume] workshop booted', {
        docs: docs.value.length,
        templates: templates.value.length,
        storagePath: storagePath.value,
      })
    } catch (error) {
      bootError.value = String(error?.message || error)
      logger.error('[Resume] boot failed', error)
    } finally {
      booting.value = false
    }
  }

  function createSection(type) {
    addSection(buildSection(type))
  }

  function addSection(section) {
    if (!doc.value) return
    doc.value.data.sections.push(section)
  }

  function moveSection(index, delta) {
    if (!doc.value) return
    const sections = doc.value.data.sections
    const target = index + delta
    if (target < 0 || target >= sections.length) return
    const [item] = sections.splice(index, 1)
    sections.splice(target, 0, item)
  }

  function removeSection(index) {
    if (!doc.value) return
    doc.value.data.sections.splice(index, 1)
  }

  function moveItem(section, index, delta) {
    const target = index + delta
    if (!section?.items || target < 0 || target >= section.items.length) return
    const [item] = section.items.splice(index, 1)
    section.items.splice(target, 0, item)
  }

  function duplicateItem(section, index) {
    if (!section?.items?.[index]) return
    const clone = JSON.parse(JSON.stringify(section.items[index]))
    clone.id = uid('it')
    section.items.splice(index + 1, 0, clone)
  }

  return {
    booting,
    bootError,
    docs,
    deletedDocs,
    templates,
    customTemplates,
    allTemplates,
    activeTemplate,
    activeTemplateHtml,
    activeDocId,
    doc,
    versions,
    versionsLoading,
    saveState,
    lastSavedAt,
    storagePath,
    completion,
    docCount,
    canUndo: computed(() => undoStack.value.length > 0),
    canRedo: computed(() => redoStack.value.length > 0),
    boot,
    openDoc,
    createDocument,
    saveNow,
    undo,
    redo,
    loadVersions,
    createVersion,
    restoreVersion,
    deleteVersion,
    pinVersion,
    duplicateDoc,
    deleteDoc,
    restoreDoc,
    purgeDoc,
    toggleFavorite,
    setTitle,
    applyTemplate,
    saveTemplate,
    deleteTemplate,
    ensureEditableTemplate,
    updateTemplateHtml,
    resetTemplateToBuiltin,
    serializeBundle,
    importBundle,
    createSection,
    addSection,
    moveSection,
    removeSection,
    moveItem,
    duplicateItem,
  }
})
