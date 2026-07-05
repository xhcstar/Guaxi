import {
  createCaptureDraft,
  fileToAsset,
  hydrateDraftAssets,
  loadEndpoint,
  loadQueue,
  purgeDraftAssets,
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
import brandIconUrl from '../src-tauri/icons/128x128.png'
import './styles.css'

const app = document.querySelector('#app')
const initialDraft = createDraftFromIncomingPayload(readInitialSharePayload())

const state = {
  endpoint: loadEndpoint(),
  queue: loadQueue(),
  selectedId: '',
  view: 'inbox',
  tab: 'inbox',
  busy: false,
  status: '准备接收分享',
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
hydrateQueueAssetsFromStore()

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
  const selected = selectedDraft()

  app.innerHTML = `
    <main class="app-shell" data-active-view="${escapeHtml(state.view)}">
      ${renderTopBar()}
      <section class="app-content">
        ${state.view === 'inbox' ? renderInbox() : ''}
        ${state.view === 'edit' ? (selected ? renderDetail(selected) : renderNoSelection()) : ''}
        ${state.view === 'settings' ? renderSettings() : ''}
      </section>
      ${renderBottomNav()}
    </main>
  `

  bindEvents()
}

function renderTopBar() {
  const counts = getQueueCounts()
  const pendingLabel = counts.pending ? `${counts.pending} 项待同步` : '收件箱已整理'
  return `
    <header class="app-topbar">
      <div class="brand-main">
        <img class="brand-icon" src="${brandIconUrl}" alt="" />
        <div class="brand-copy">
          <h1>Guaxi</h1>
          <p>${escapeHtml(state.status || pendingLabel)}</p>
        </div>
      </div>
      <div class="topbar-actions">
        <button class="tool-button" data-action="paste" aria-label="读取剪贴板" title="读取剪贴板">
          ${icon('clipboard')}
        </button>
        <label class="tool-button file-tool" aria-label="导入图片" title="导入图片">
          ${icon('image')}
          <input type="file" data-action="images" accept="image/*" multiple />
        </label>
        <button class="tool-button primary" data-action="sync-all" aria-label="同步到 PC" title="同步到 PC" ${state.busy || !state.queue.length ? 'disabled' : ''}>
          ${icon('sync')}
        </button>
      </div>
    </header>
  `
}

function renderInbox() {
  const counts = getQueueCounts()
  const items = filteredQueue()
  return `
    <section class="connection-strip ${endpointLooksLan() ? 'is-online' : 'is-local'}">
      <div class="connection-copy">
        <span>PC</span>
        <strong>${escapeHtml(shortEndpoint(state.endpoint))}</strong>
      </div>
      <button class="pill-button" data-action="sync-all" ${state.busy || !state.queue.length ? 'disabled' : ''}>同步</button>
    </section>

    <nav class="filter-tabs" aria-label="队列筛选">
      ${renderTab('inbox', '收件箱', counts.total)}
      ${renderTab('pending', '待发送', counts.pending)}
      ${renderTab('failed', '失败', counts.failed)}
      ${renderTab('sent', '已发送', counts.sent)}
    </nav>

    <section class="capture-list">
      ${items.map(renderCaptureItem).join('') || renderEmptyState()}
    </section>
  `
}

function renderSettings() {
  return `
    <section class="settings-page">
      <div class="page-heading">
        <span>设置</span>
        <h2>连接与入库</h2>
      </div>

      <label class="form-field">
        <span>Guaxi PC 接口</span>
        <input data-field="endpoint" value="${escapeHtml(state.endpoint)}" />
      </label>

      <div class="settings-note">
        <strong>PC 负责重活</strong>
        <p>手机端只排队采集内容。网页正文抽取、Markdown 转换、图片 OCR 和附件入库都会交给 PC 端处理。</p>
      </div>
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
      <div class="card-title-row">
        <strong>${escapeHtml(item.title || '未命名采集')}</strong>
        <span class="status ${escapeHtml(item.status)}">${statusLabel(item.status)}</span>
      </div>
      <p class="card-preview">${escapeHtml(item.sourceUrl || item.description || previewText(item))}</p>
      <div class="meta-row">
        <span>${sourceLabel(item.sourceApp)}</span>
        <span>${item.assets?.length || 0} 附件</span>
        <span>${formatQueueTime(item.updatedAt || item.createdAt)}</span>
      </div>
    </article>
  `
}

function renderDetail(item) {
  return `
    <form class="detail-page" data-form="detail">
      <div class="detail-toolbar">
        <div class="page-heading">
          <span>编辑</span>
          <h2>${escapeHtml(item.title || '未命名采集')}</h2>
        </div>
        <span class="status ${escapeHtml(item.status)}">${statusLabel(item.status)}</span>
      </div>

      <label class="form-field">
        <span>标题</span>
        <input data-edit="title" value="${escapeHtml(item.title)}" />
      </label>

      <label class="form-field">
        <span>来源链接</span>
        <input data-edit="sourceUrl" value="${escapeHtml(item.sourceUrl)}" placeholder="https://..." />
      </label>

      <div class="field-grid">
        <label class="form-field">
          <span>文件库</span>
          <input data-edit="fileRepo" value="${escapeHtml(item.fileRepo)}" />
        </label>
        <label class="form-field">
          <span>知识库</span>
          <input data-edit="knowledgeBase" value="${escapeHtml(item.knowledgeBase)}" />
        </label>
      </div>

      <label class="form-field">
        <span>标签</span>
        <input data-edit="tagsText" value="${escapeHtml(item.tagsText)}" />
      </label>

      <label class="form-field">
        <span>备注 / 已捕获文本</span>
        <textarea data-edit="markdown" placeholder="可选。为空时 PC 会根据来源 URL 或图片附件进行后续转换。">${escapeHtml(item.markdown)}</textarea>
      </label>

      <div class="asset-summary">
        ${(item.assets || []).map((asset) => `<span>${escapeHtml(asset.name || asset.mime || '附件')}</span>`).join('') || '<span>无附件</span>'}
      </div>

      ${item.lastError ? `<p class="error-text">${escapeHtml(item.lastError)}</p>` : ''}

      <div class="detail-actions">
        <button class="action-button primary" type="button" data-action="send-selected" ${state.busy ? 'disabled' : ''}>发送到 PC</button>
        <button class="tool-button" type="button" data-action="duplicate-selected" aria-label="复制草稿" title="复制草稿">${icon('copy')}</button>
        <button class="tool-button danger" type="button" data-action="delete-selected" aria-label="删除" title="删除">${icon('trash')}</button>
      </div>
    </form>
  `
}

function renderNoSelection() {
  return `
    <section class="empty-state">
      <h2>还没有选中条目</h2>
      <p>从收件箱选择一条内容后再编辑。</p>
    </section>
  `
}

function renderEmptyState() {
  return `
    <section class="empty-state">
      <h2>收件箱为空</h2>
      <p>从知乎、公众号、小红书、B站或相册分享内容到 Guaxi，这里会自动生成待同步项目。</p>
    </section>
  `
}

function renderBottomNav() {
  const counts = getQueueCounts()
  return `
    <nav class="bottom-nav" aria-label="主导航">
      ${renderNavButton('inbox', '收件箱', icon('inbox'), counts.pending)}
      ${renderNavButton('edit', '编辑', icon('edit'), 0)}
      ${renderNavButton('settings', '设置', icon('settings'), 0)}
    </nav>
  `
}

function renderNavButton(view, label, svg, badge) {
  return `
    <button class="${state.view === view ? 'active' : ''}" data-view="${view}" aria-label="${label}" title="${label}">
      ${svg}
      <span>${label}</span>
      ${badge ? `<strong>${badge}</strong>` : ''}
    </button>
  `
}

function bindEvents() {
  app.querySelectorAll('[data-view]').forEach((button) => {
    button.addEventListener('click', () => {
      state.view = button.dataset.view
      render()
    })
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
      state.view = 'edit'
      render()
    })
  })

  bindCardSwipe()

  app.querySelectorAll('[data-action="sync-all"]').forEach((button) => {
    button.addEventListener('click', syncQueue)
  })
  app.querySelectorAll('[data-action="paste"]').forEach((button) => {
    button.addEventListener('click', importClipboard)
  })
  app.querySelector('[data-action="send-selected"]')?.addEventListener('click', sendSelected)
  app.querySelector('[data-action="duplicate-selected"]')?.addEventListener('click', duplicateSelected)
  app.querySelector('[data-action="delete-selected"]')?.addEventListener('click', deleteSelected)

  app.querySelectorAll('[data-action="images"]').forEach((input) => {
    input.addEventListener('change', async (event) => {
      const files = Array.from(event.currentTarget.files || [])
      if (files.length) await importImages(files)
    })
  })

  app.querySelectorAll('[data-edit]').forEach((field) => {
    field.addEventListener('input', (event) => {
      updateSelectedDraft(field.dataset.edit, event.currentTarget.value)
    })
  })
}

// 收件箱卡片支持“向左滑动删除”。触点位移横向占优才判为滑动，避免与纵向滚动、点击选中冲突。
function bindCardSwipe() {
  const DELETE_THRESHOLD = 80 // 左滑超过该像素即删除
  const MAX_SHIFT = 120

  app.querySelectorAll('.capture-card[data-id]').forEach((card) => {
    const id = card.dataset.id
    let startX = 0
    let startY = 0
    let dx = 0
    let axisLocked = false
    let horizontal = false

    const reset = (animated) => {
      card.style.transition = animated ? 'transform 0.2s ease, opacity 0.2s ease' : 'none'
      card.style.transform = 'translateX(0)'
      card.style.opacity = '1'
      card.classList.remove('swiping')
    }

    card.addEventListener('touchstart', (event) => {
      const touch = event.touches[0]
      startX = touch.clientX
      startY = touch.clientY
      dx = 0
      axisLocked = false
      horizontal = false
      card.style.transition = 'none'
    }, { passive: true })

    card.addEventListener('touchmove', (event) => {
      const touch = event.touches[0]
      const mx = touch.clientX - startX
      const my = touch.clientY - startY

      if (!axisLocked && (Math.abs(mx) > 6 || Math.abs(my) > 6)) {
        axisLocked = true
        horizontal = Math.abs(mx) > Math.abs(my)
      }
      if (!horizontal) return

      dx = Math.max(-MAX_SHIFT, Math.min(0, mx)) // 只允许左滑
      card.style.transform = `translateX(${dx}px)`
      card.style.opacity = String(1 + dx / (MAX_SHIFT * 2))
      card.classList.toggle('swiping', dx < -DELETE_THRESHOLD)
      if (event.cancelable) event.preventDefault() // 横向滑动时阻止页面纵向滚动
    }, { passive: false })

    const finish = () => {
      if (dx < -DELETE_THRESHOLD) {
        card.style.transition = 'transform 0.18s ease, opacity 0.18s ease'
        card.style.transform = `translateX(-${MAX_SHIFT}px)`
        card.style.opacity = '0'
        removeQueueItem(id)
      } else if (horizontal) {
        reset(true)
      }
    }
    card.addEventListener('touchend', finish)
    card.addEventListener('touchcancel', () => reset(true))

    // 捕获阶段拦截：发生过左滑时，阻止随后的 click 误触发“选中并进入编辑”。
    card.addEventListener(
      'click',
      (event) => {
        if (dx < -6) {
          event.stopPropagation()
          event.preventDefault()
          dx = 0
        }
      },
      true,
    )
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
  state.view = 'inbox'
  state.status = `${label}已进入收件箱`
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
    // 附件 base64 可能只在 IndexedDB（重启后内存里仅有元数据），发送前先水合。
    const payloadDraft = await hydrateDraftAssets(item)
    await submitCapture(state.endpoint, payloadDraft)
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
  removeQueueItem(state.selectedId)
}

function removeQueueItem(id) {
  if (!id) return
  const removed = state.queue.find((item) => item.id === id)
  state.queue = state.queue.filter((item) => item.id !== id)

  // 删除的是当前正在编辑的条目时，退出编辑态回到收件箱，避免停留在“已删除条目”的空编辑页。
  if (state.selectedId === id) {
    state.selectedId = state.queue[0]?.id || ''
    if (state.view === 'edit') state.view = 'inbox'
  }

  state.status = '已删除'
  saveQueue(state.queue)
  if (removed) void purgeDraftAssets(removed)
  render()
}

// 启动时把队列里附件的 base64 从 IndexedDB 水合回内存，便于后续同步与预览。
async function hydrateQueueAssetsFromStore() {
  const needsHydration = state.queue.some((draft) =>
    (draft.assets || []).some((asset) => asset.id && !asset.dataBase64),
  )
  if (!needsHydration) return

  state.queue = await Promise.all(state.queue.map(hydrateDraftAssets))
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

function shortEndpoint(endpoint) {
  try {
    const url = new URL(endpoint)
    return `${url.hostname}:${url.port}${url.pathname}`
  } catch {
    return endpoint
  }
}

function formatQueueTime(value) {
  const text = String(value || '')
  return text.includes('T') ? text.slice(0, 16).replace('T', ' ') : text
}

function icon(name) {
  const icons = {
    clipboard: '<svg viewBox="0 0 24 24"><path d="M9 4h6l1 2h3v15H5V6h3l1-2Z"/><path d="M9 10h6M9 14h6M9 18h4"/></svg>',
    image: '<svg viewBox="0 0 24 24"><rect x="4" y="5" width="16" height="14" rx="2"/><path d="M8 15l3-3 3 3 2-2 3 3"/><circle cx="9" cy="9" r="1.5"/></svg>',
    sync: '<svg viewBox="0 0 24 24"><path d="M20 7h-5a6 6 0 0 0-10 3"/><path d="M17 4l3 3-3 3"/><path d="M4 17h5a6 6 0 0 0 10-3"/><path d="M7 20l-3-3 3-3"/></svg>',
    inbox: '<svg viewBox="0 0 24 24"><path d="M4 13l2-7h12l2 7v5H4v-5Z"/><path d="M4 13h5l1.5 2h3L15 13h5"/></svg>',
    edit: '<svg viewBox="0 0 24 24"><path d="M4 20h4l11-11-4-4L4 16v4Z"/><path d="M13 7l4 4"/></svg>',
    settings: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M4.8 5.8l2.1 2.1M17.1 16.1l2.1 2.1M3 12h3M18 12h3M4.8 18.2l2.1-2.1M17.1 7.9l2.1-2.1"/></svg>',
    copy: '<svg viewBox="0 0 24 24"><rect x="8" y="8" width="11" height="11" rx="2"/><path d="M5 15V5h10"/></svg>',
    trash: '<svg viewBox="0 0 24 24"><path d="M5 7h14M10 11v6M14 11v6M8 7l1-3h6l1 3M7 7l1 13h8l1-13"/></svg>',
  }
  return icons[name] || ''
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}
