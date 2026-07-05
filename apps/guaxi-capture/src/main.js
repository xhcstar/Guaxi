import {
  createCaptureDraft,
  fileToAsset,
  loadEndpoint,
  loadQueue,
  saveEndpoint,
  saveQueue,
  submitCapture,
} from './services/captureClient.js'
import {
  createDraftFromIncomingPayload,
  readAndroidSharePayload,
  readClipboardPayload,
  readInitialSharePayload,
  readLastAndroidShareDomPayload,
  readStartupNativePayload,
  registerAndroidShareDomListener,
  registerNativeCaptureListener,
} from './services/shareIntake.js'
import './styles.css'

const app = document.querySelector('#app')
const initialDraft = createDraftFromIncomingPayload(readInitialSharePayload())

const state = {
  endpoint: loadEndpoint(),
  queue: loadQueue(),
  selectedId: '',
  tab: 'inbox',
  busy: false,
  status: '准备接收分享',
  settingsOpen: false,
  lastIncomingSignature: '',
  lastIncomingAt: 0,
  manual: createCaptureDraft({
    sourceApp: 'manual',
    tagsText: 'capture,manual',
  }),
}

if (initialDraft) {
  queueDraft(initialDraft, 'pending', '启动分享')
}

render()

registerNativeCaptureListener((payload) => {
  receiveIncomingPayload(payload, '系统分享')
})

registerAndroidShareDomListener((payload) => {
  receiveIncomingPayload(payload, 'Android Share')
})

readStartupNativePayload().then((payload) => {
  if (payload) receiveIncomingPayload(payload, '启动分享')
})

readAndroidSharePayload().then((payload) => {
  if (payload) receiveIncomingPayload(payload, 'Android Share')
})

const initialAndroidWindowPayload = readLastAndroidShareDomPayload()
if (initialAndroidWindowPayload) {
  receiveIncomingPayload(initialAndroidWindowPayload, 'Android Share')
}

setInterval(async () => {
  const windowPayload = readLastAndroidShareDomPayload()
  if (windowPayload) {
    receiveIncomingPayload(windowPayload, 'Android Share')
    return
  }

  const payload = await readAndroidSharePayload()
  if (payload) receiveIncomingPayload(payload, 'Android Share')
}, 1800)

function render() {
  const counts = getQueueCounts()
  const selected = selectedDraft()
  const items = filteredQueue()

  app.innerHTML = `
    <main class="app-shell">
      <header class="hero">
        <div class="hero-main">
          <div class="brand-mark">G</div>
          <div>
            <h1>Guaxi Capture</h1>
            <p>${escapeHtml(state.status)}</p>
          </div>
        </div>
        <button class="icon-button" data-action="toggle-settings" title="连接设置">设置</button>
      </header>

      <section class="connection ${endpointLooksLan() ? 'is-online' : 'is-local'}">
        <div>
          <span class="eyebrow">PC 连接</span>
          <strong>${escapeHtml(state.endpoint)}</strong>
        </div>
        <button data-action="sync-all" ${state.busy || !state.queue.length ? 'disabled' : ''}>同步</button>
      </section>

      ${state.settingsOpen ? renderSettings() : ''}

      <nav class="tabs" aria-label="队列筛选">
        ${renderTab('inbox', '收件箱', counts.total)}
        ${renderTab('pending', '待发送', counts.pending)}
        ${renderTab('failed', '失败', counts.failed)}
        ${renderTab('sent', '已发送', counts.sent)}
      </nav>

      <section class="workspace">
        <section class="list-panel">
          <div class="quick-actions">
            <button data-action="paste">剪贴板</button>
            <label class="upload-button">
              <input type="file" data-action="images" accept="image/*" multiple />
              图片 OCR
            </label>
            <button data-action="new-note">手记</button>
          </div>

          <div class="capture-list">
            ${items.map(renderCaptureItem).join('') || renderEmptyState()}
          </div>
        </section>

        <section class="detail-panel">
          ${selected ? renderDetail(selected) : renderGuide()}
        </section>
      </section>
    </main>
  `

  bindEvents()
}

function renderSettings() {
  return `
    <section class="settings-panel">
      <label>
        <span>Guaxi PC 接口</span>
        <input data-field="endpoint" value="${escapeHtml(state.endpoint)}" />
      </label>
      <p>手机端只做采集和排队。网页转换、图片 OCR、附件入库由 PC 端 Capture Import 接口处理。</p>
    </section>
  `
}

function renderTab(key, label, count) {
  return `
    <button class="${state.tab === key ? 'active' : ''}" data-tab="${key}">
      <span>${label}</span>
      <strong>${count}</strong>
    </button>
  `
}

function renderCaptureItem(item) {
  const active = item.id === state.selectedId ? 'active' : ''
  return `
    <article class="capture-card ${active}" data-action="select" data-id="${escapeHtml(item.id)}">
      <div class="card-top">
        <strong>${escapeHtml(item.title || '未命名采集')}</strong>
        <span class="status ${escapeHtml(item.status)}">${statusLabel(item.status)}</span>
      </div>
      <p>${escapeHtml(item.sourceUrl || item.description || previewText(item))}</p>
      <div class="meta-row">
        <span>${sourceLabel(item.sourceApp)}</span>
        <span>${item.assets?.length || 0} 附件</span>
        <span>${escapeHtml(item.updatedAt || item.createdAt)}</span>
      </div>
    </article>
  `
}

function renderDetail(item) {
  return `
    <form class="detail-card" data-form="detail">
      <div class="detail-head">
        <div>
          <span class="eyebrow">采集预览</span>
          <h2>${escapeHtml(item.title || '未命名采集')}</h2>
        </div>
        <span class="status ${escapeHtml(item.status)}">${statusLabel(item.status)}</span>
      </div>

      <label>
        <span>标题</span>
        <input data-edit="title" value="${escapeHtml(item.title)}" />
      </label>

      <label>
        <span>来源链接</span>
        <input data-edit="sourceUrl" value="${escapeHtml(item.sourceUrl)}" placeholder="https://..." />
      </label>

      <div class="field-grid">
        <label>
          <span>文件库</span>
          <input data-edit="fileRepo" value="${escapeHtml(item.fileRepo)}" />
        </label>
        <label>
          <span>知识库</span>
          <input data-edit="knowledgeBase" value="${escapeHtml(item.knowledgeBase)}" />
        </label>
      </div>

      <label>
        <span>标签</span>
        <input data-edit="tagsText" value="${escapeHtml(item.tagsText)}" />
      </label>

      <label>
        <span>备注 / 已捕获文本</span>
        <textarea data-edit="markdown" placeholder="可选。为空时 PC 会根据来源 URL 或图片附件进行后续转换。">${escapeHtml(item.markdown)}</textarea>
      </label>

      <div class="asset-summary">
        ${(item.assets || []).map((asset) => `<span>${escapeHtml(asset.name || asset.mime || '附件')}</span>`).join('') || '<span>无附件</span>'}
      </div>

      ${item.lastError ? `<p class="error-text">${escapeHtml(item.lastError)}</p>` : ''}

      <div class="detail-actions">
        <button type="button" data-action="send-selected" ${state.busy ? 'disabled' : ''}>发送到 PC</button>
        <button type="button" data-action="duplicate-selected">复制草稿</button>
        <button type="button" data-action="delete-selected">删除</button>
      </div>
    </form>
  `
}

function renderGuide() {
  return `
    <section class="guide-card">
      <span class="eyebrow">工作流</span>
      <h2>从任何 App 分享到 Guaxi</h2>
      <p>文章链接、选中文本、截图和照片先进入手机收件箱；当 PC 在线时，一键同步到文件库。图片不在手机端 OCR，直接交给 PC 处理。</p>
      <div class="guide-steps">
        <div><strong>1</strong><span>分享或粘贴</span></div>
        <div><strong>2</strong><span>自动入队</span></div>
        <div><strong>3</strong><span>PC 转换入库</span></div>
      </div>
    </section>
  `
}

function renderEmptyState() {
  return `
    <section class="empty-state">
      <h2>收件箱为空</h2>
      <p>从知乎、公众号、小红书、B站或相册分享内容到 Guaxi Capture，这里会自动生成待同步项目。</p>
    </section>
  `
}

function bindEvents() {
  app.querySelector('[data-action="toggle-settings"]')?.addEventListener('click', () => {
    state.settingsOpen = !state.settingsOpen
    render()
  })

  app.querySelector('[data-field="endpoint"]')?.addEventListener('input', (event) => {
    state.endpoint = event.currentTarget.value
    saveEndpoint(state.endpoint)
  })

  app.querySelectorAll('[data-tab]').forEach((button) => {
    button.addEventListener('click', () => {
      state.tab = button.dataset.tab
      render()
    })
  })

  app.querySelectorAll('[data-action="select"]').forEach((card) => {
    card.addEventListener('click', () => {
      state.selectedId = card.dataset.id
      render()
    })
  })

  app.querySelector('[data-action="sync-all"]')?.addEventListener('click', syncQueue)
  app.querySelector('[data-action="paste"]')?.addEventListener('click', importClipboard)
  app.querySelector('[data-action="new-note"]')?.addEventListener('click', createManualNote)
  app.querySelector('[data-action="send-selected"]')?.addEventListener('click', sendSelected)
  app.querySelector('[data-action="duplicate-selected"]')?.addEventListener('click', duplicateSelected)
  app.querySelector('[data-action="delete-selected"]')?.addEventListener('click', deleteSelected)

  app.querySelector('[data-action="images"]')?.addEventListener('change', async (event) => {
    const files = Array.from(event.currentTarget.files || [])
    if (files.length) await importImages(files)
  })

  app.querySelectorAll('[data-edit]').forEach((field) => {
    field.addEventListener('input', (event) => {
      updateSelectedDraft(field.dataset.edit, event.currentTarget.value)
    })
  })
}

function receiveIncomingPayload(payload, label) {
  const draft = createDraftFromIncomingPayload(payload)
  if (!draft) {
    state.status = `${label}没有可采集内容`
    console.info('[capture-inbox] incoming payload ignored', { label })
    render()
    return
  }

  const signature = buildIncomingSignature(draft)
  const now = Date.now()
  if (signature && signature === state.lastIncomingSignature && now - state.lastIncomingAt < 5000) {
    console.info('[capture-inbox] duplicate incoming payload ignored', { label, signature })
    return
  }
  state.lastIncomingSignature = signature
  state.lastIncomingAt = now

  queueDraft(draft, 'pending', label)
  state.status = `${label}已进入收件箱，连接 PC 后可同步`
  render()
}

async function importClipboard() {
  try {
    state.status = '正在读取剪贴板'
    render()
    receiveIncomingPayload(await readClipboardPayload(), '剪贴板')
  } catch (error) {
    state.status = `剪贴板读取失败：${error.message}`
    console.warn('[capture-inbox] clipboard failed', error)
    render()
  }
}

async function importImages(files) {
  state.busy = true
  state.status = '正在读取图片'
  render()

  try {
    const assets = []
    for (const file of files) {
      assets.push(await fileToAsset(file))
    }

    const draft = createCaptureDraft({
      title: buildImageTitle(files),
      sourceApp: 'camera',
      tagsText: 'capture,ocr,camera',
      markdown: '',
      assets,
    })
    queueDraft(draft, 'pending', '图片')
    state.status = `${assets.length} 张图片已进入 PC OCR 队列`
    console.info('[capture-inbox] image assets queued for pc ocr', {
      count: assets.length,
    })
  } catch (error) {
    state.status = `图片读取失败：${error.message}`
    console.warn('[capture-inbox] image import failed', error)
  } finally {
    state.busy = false
    render()
  }
}

function createManualNote() {
  const draft = createCaptureDraft({
    title: '手记',
    sourceApp: 'manual',
    tagsText: 'capture,manual',
    markdown: '',
  })
  queueDraft(draft, 'draft', '手记')
  state.status = '已创建手记草稿'
  render()
}

function queueDraft(draft, status, reason) {
  const queued = {
    ...draft,
    status,
    lastError: '',
    updatedAt: new Date().toISOString(),
  }
  state.queue = [queued, ...state.queue.filter((item) => item.id !== queued.id)]
  state.selectedId = queued.id
  saveQueue(state.queue)
  console.info('[capture-inbox] draft queued', {
    reason,
    title: queued.title,
    sourceApp: queued.sourceApp,
    assetCount: queued.assets?.length || 0,
  })
}

async function syncQueue() {
  if (state.busy) return
  const candidates = state.queue.filter((item) => ['pending', 'failed'].includes(item.status))
  if (!candidates.length) {
    state.status = '没有需要同步的内容'
    render()
    return
  }

  state.busy = true
  state.status = `正在同步 ${candidates.length} 项`
  render()

  for (const item of candidates) {
    await sendDraft(item)
  }

  state.busy = false
  state.status = getQueueCounts().failed ? '部分内容同步失败' : '同步完成'
  saveQueue(state.queue)
  render()
}

async function sendSelected() {
  const item = selectedDraft()
  if (!item || state.busy) return
  state.busy = true
  state.status = '正在发送当前项目'
  render()
  await sendDraft(item)
  state.busy = false
  saveQueue(state.queue)
  render()
}

async function sendDraft(item) {
  try {
    await submitCapture(state.endpoint, item)
    updateDraft(item.id, {
      status: 'sent',
      lastError: '',
      updatedAt: new Date().toISOString(),
    })
    console.info('[capture-sync] sent', { title: item.title, id: item.id })
  } catch (error) {
    updateDraft(item.id, {
      status: 'failed',
      lastError: error.message,
      updatedAt: new Date().toISOString(),
    })
    console.warn('[capture-sync] failed', { title: item.title, error })
  }
}

function duplicateSelected() {
  const item = selectedDraft()
  if (!item) return
  queueDraft({
    ...item,
    id: crypto.randomUUID(),
    title: `${item.title || '采集'} 副本`,
    status: 'draft',
    createdAt: new Date().toISOString(),
  }, 'draft', '复制')
  render()
}

function deleteSelected() {
  if (!state.selectedId) return
  state.queue = state.queue.filter((item) => item.id !== state.selectedId)
  state.selectedId = state.queue[0]?.id || ''
  state.status = '已删除'
  saveQueue(state.queue)
  render()
}

function updateSelectedDraft(key, value) {
  if (!state.selectedId) return
  const currentStatus = selectedDraft()?.status
  updateDraft(state.selectedId, {
    [key]: value,
    status: ['sent', 'draft'].includes(currentStatus) ? 'pending' : currentStatus,
    updatedAt: new Date().toISOString(),
  })
  saveQueue(state.queue)
}

function updateDraft(id, patch) {
  state.queue = state.queue.map((item) => item.id === id ? { ...item, ...patch } : item)
}

function selectedDraft() {
  return state.queue.find((item) => item.id === state.selectedId) || state.queue[0] || null
}

function filteredQueue() {
  if (state.tab === 'inbox') return state.queue
  if (state.tab === 'pending') {
    return state.queue.filter((item) => item.status === 'pending' || item.status === 'draft')
  }
  return state.queue.filter((item) => item.status === state.tab)
}

function getQueueCounts() {
  return {
    total: state.queue.length,
    pending: state.queue.filter((item) => item.status === 'pending' || item.status === 'draft').length,
    failed: state.queue.filter((item) => item.status === 'failed').length,
    sent: state.queue.filter((item) => item.status === 'sent').length,
  }
}

function endpointLooksLan() {
  return /^http:\/\/(?!127\.0\.0\.1|localhost)/i.test(state.endpoint)
}

function statusLabel(status) {
  return {
    pending: '待发送',
    failed: '失败',
    sent: '已发送',
    draft: '草稿',
  }[status] || '待发送'
}

function sourceLabel(sourceApp) {
  return {
    manual: '手记',
    wechat: '公众号',
    zhihu: '知乎',
    xiaohongshu: '小红书',
    bilibili: 'B站',
    camera: '图片',
    browser: '浏览器',
    clipboard: '剪贴板',
  }[sourceApp] || sourceApp || '采集'
}

function previewText(item) {
  return String(item.markdown || item.sourceUrl || '等待 PC 端转换处理').slice(0, 120)
}

function buildImageTitle(files) {
  if (files.length === 1) return files[0].name.replace(/\.[^.]+$/, '') || '图片 OCR'
  return `图片 OCR ${new Date().toLocaleString()}`
}

function buildIncomingSignature(draft) {
  return [
    draft.sourceApp || '',
    draft.sourceUrl || '',
    draft.title || '',
    draft.markdown || '',
    (draft.assets || []).map((asset) => asset.name || asset.mime || '').join('|'),
  ].join('::')
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}
