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
  buildOcrMarkdown,
  isImageAsset,
  loadOcrEndpoint,
  recognizeImageAsset,
  saveOcrEndpoint,
} from './services/ocrClient.js'
import {
  createDraftFromIncomingPayload,
  readClipboardPayload,
  readInitialSharePayload,
  readStartupNativePayload,
  registerNativeCaptureListener,
} from './services/shareIntake.js'
import './styles.css'

const app = document.querySelector('#app')
const initialDraft = createDraftFromIncomingPayload(readInitialSharePayload())

const state = {
  endpoint: loadEndpoint(),
  ocrEndpoint: loadOcrEndpoint(),
  current: initialDraft || createCaptureDraft(),
  queue: loadQueue(),
  busy: false,
  status: initialDraft ? '已接收外部分享草稿' : '',
}

function render() {
  app.innerHTML = `
    <section class="shell">
      <header class="topbar">
        <div>
          <h1>Guaxi Capture</h1>
          <p>${state.status || 'Ready'}</p>
        </div>
        <button class="icon-button" data-action="retry" title="同步队列">
          ↻
        </button>
      </header>

      <section class="panel endpoint-panel">
        <div class="row two">
          <label>
            <span>Guaxi 接口</span>
            <input data-field="endpoint" value="${escapeHtml(state.endpoint)}" />
          </label>
          <label>
            <span>OCR 接口</span>
            <input data-field="ocrEndpoint" value="${escapeHtml(state.ocrEndpoint)}" />
          </label>
        </div>
      </section>

      <section class="capture-grid">
        <form class="panel capture-form" data-form="capture">
          <div class="row two">
            <label>
              <span>标题</span>
              <input data-field="title" value="${escapeHtml(state.current.title)}" placeholder="文章、截图或笔记标题" />
            </label>
            <label>
              <span>来源</span>
              <select data-field="sourceApp">
                ${sourceOptions(state.current.sourceApp)}
              </select>
            </label>
          </div>

          <label>
            <span>来源链接</span>
            <input data-field="sourceUrl" value="${escapeHtml(state.current.sourceUrl)}" placeholder="https://..." />
          </label>

          <div class="row two">
            <label>
              <span>知识库</span>
              <input data-field="knowledgeBase" value="${escapeHtml(state.current.knowledgeBase)}" />
            </label>
            <label>
              <span>文件库</span>
              <input data-field="fileRepo" value="${escapeHtml(state.current.fileRepo)}" />
            </label>
          </div>

          <label>
            <span>标签</span>
            <input data-field="tagsText" value="${escapeHtml(state.current.tagsText)}" placeholder="capture,wechat,ocr" />
          </label>

          <label>
            <span>摘要</span>
            <input data-field="description" value="${escapeHtml(state.current.description)}" />
          </label>

          <label class="markdown-label">
            <span>Markdown</span>
            <textarea data-field="markdown" placeholder="# 标题&#10;&#10;正文内容">${escapeHtml(state.current.markdown)}</textarea>
          </label>

          <div class="asset-strip">
            <label class="file-picker">
              <input type="file" data-action="assets" multiple accept="image/*,.pdf" />
              <span>添加附件</span>
            </label>
            <label class="file-picker">
              <input type="file" data-action="ocr-assets" multiple accept="image/*" />
              <span>OCR图片</span>
            </label>
            <div class="asset-count">${state.current.assets.length} 个附件</div>
          </div>

          <div class="actions">
            <button type="button" data-action="clear">清空</button>
            <button type="button" data-action="clipboard">读剪贴板</button>
            <button type="button" data-action="ocr-existing" ${hasImageAssets() ? '' : 'disabled'}>识别附件</button>
            <button type="button" data-action="copy-markdown" ${state.current.markdown.trim() ? '' : 'disabled'}>复制MD</button>
            <button type="button" data-action="export-markdown" ${state.current.markdown.trim() ? '' : 'disabled'}>导出MD</button>
            <button type="button" data-action="save-draft">存入队列</button>
            <button type="submit" class="primary" ${state.busy ? 'disabled' : ''}>发送到 Guaxi</button>
          </div>
        </form>

        <aside class="panel queue-panel">
          <div class="queue-head">
            <h2>同步队列</h2>
            <span>${state.queue.length}</span>
          </div>
          <div class="queue-list">
            ${state.queue.map(renderQueueItem).join('') || '<p class="empty">暂无离线草稿</p>'}
          </div>
        </aside>
      </section>
    </section>
  `

  bindEvents()
}

function bindEvents() {
  app.querySelectorAll('[data-field]').forEach((field) => {
    field.addEventListener('input', (event) => {
      const key = event.currentTarget.dataset.field
      if (key === 'endpoint') {
        state.endpoint = event.currentTarget.value
        saveEndpoint(state.endpoint)
      } else if (key === 'ocrEndpoint') {
        state.ocrEndpoint = event.currentTarget.value
        saveOcrEndpoint(state.ocrEndpoint)
      } else {
        state.current[key] = event.currentTarget.value
      }
    })
  })

  app.querySelector('[data-form="capture"]').addEventListener('submit', async (event) => {
    event.preventDefault()
    await submitCurrent()
  })

  app.querySelector('[data-action="clear"]').addEventListener('click', () => {
    state.current = createCaptureDraft()
    state.status = '已清空'
    render()
  })

  app.querySelector('[data-action="save-draft"]').addEventListener('click', () => {
    enqueueCurrent('pending')
    state.current = createCaptureDraft()
    state.status = '已存入队列'
    persistAndRender()
  })

  app.querySelector('[data-action="retry"]').addEventListener('click', retryQueue)
  app.querySelector('[data-action="clipboard"]').addEventListener('click', importFromClipboard)
  app.querySelector('[data-action="ocr-existing"]').addEventListener('click', recognizeExistingAssets)
  app.querySelector('[data-action="copy-markdown"]').addEventListener('click', copyMarkdown)
  app.querySelector('[data-action="export-markdown"]').addEventListener('click', exportMarkdown)

  app.querySelector('[data-action="assets"]').addEventListener('change', async (event) => {
    const files = Array.from(event.currentTarget.files || [])
    if (!files.length) return
    state.status = '正在读取附件...'
    render()
    const assets = []
    for (const file of files) {
      assets.push(await fileToAsset(file))
    }
    state.current.assets = [...state.current.assets, ...assets]
    state.status = `已添加 ${assets.length} 个附件`
    render()
  })

  app.querySelector('[data-action="ocr-assets"]').addEventListener('change', async (event) => {
    const files = Array.from(event.currentTarget.files || [])
    if (!files.length) return
    await importImagesWithOcr(files)
  })

  app.querySelectorAll('[data-action="load-queue"]').forEach((button) => {
    button.addEventListener('click', () => {
      const item = state.queue.find((entry) => entry.id === button.dataset.id)
      if (!item) return
      state.current = { ...item, status: 'draft' }
      state.status = '已载入草稿'
      render()
    })
  })
}

async function importImagesWithOcr(files) {
  state.busy = true
  state.status = '正在读取图片并 OCR...'
  render()

  try {
    const assets = []
    const markdownParts = []
    for (const file of files) {
      const asset = await fileToAsset(file)
      assets.push(asset)
      const result = await recognizeImageAsset(state.ocrEndpoint, asset)
      markdownParts.push(buildOcrMarkdown(asset, result))
    }

    state.current.assets = [...state.current.assets, ...assets]
    appendMarkdown(markdownParts.join('\n\n'))
    state.current.sourceApp = 'camera'
    state.current.tagsText = ensureTags(state.current.tagsText, ['capture', 'ocr', 'camera'])
    state.current.title = state.current.title || buildOcrTitle(files)
    state.status = `已 OCR ${assets.length} 张图片`
  } catch (error) {
    state.status = `OCR 失败：${error.message}`
    console.warn('[capture-ocr] import image ocr failed', error)
  } finally {
    state.busy = false
    render()
  }
}

async function recognizeExistingAssets() {
  const imageAssets = state.current.assets.filter(isImageAsset)
  if (!imageAssets.length || state.busy) return

  state.busy = true
  state.status = `正在 OCR ${imageAssets.length} 个附件...`
  render()

  try {
    const markdownParts = []
    for (const asset of imageAssets) {
      const result = await recognizeImageAsset(state.ocrEndpoint, asset)
      markdownParts.push(buildOcrMarkdown(asset, result))
    }

    appendMarkdown(markdownParts.join('\n\n'))
    state.current.sourceApp = state.current.sourceApp === 'manual' ? 'camera' : state.current.sourceApp
    state.current.tagsText = ensureTags(state.current.tagsText, ['capture', 'ocr'])
    state.status = `已识别 ${imageAssets.length} 个图片附件`
  } catch (error) {
    state.status = `识别附件失败：${error.message}`
    console.warn('[capture-ocr] existing asset ocr failed', error)
  } finally {
    state.busy = false
    render()
  }
}

async function importFromClipboard() {
  state.status = '正在读取剪贴板...'
  render()

  try {
    const payload = await readClipboardPayload()
    applyIncomingPayload(payload, '已从剪贴板生成草稿')
  } catch (error) {
    state.status = `读取剪贴板失败：${error.message}`
    console.warn('[capture-intake] clipboard import failed', error)
    render()
  }
}

function applyIncomingPayload(payload, status) {
  const draft = createDraftFromIncomingPayload(payload)
  if (!draft) {
    state.status = '没有识别到可采集内容'
    render()
    return
  }

  // 设计说明：外部分享进入时直接替换当前草稿，避免混合旧草稿和新分享内容。
  // 用户需要保留旧草稿时，可以先点“存入队列”。
  state.current = draft
  state.status = status
  console.info('[capture-intake] draft applied', {
    title: draft.title,
    sourceApp: draft.sourceApp,
    sourceUrl: draft.sourceUrl,
  })
  render()
}

async function copyMarkdown() {
  if (!navigator.clipboard?.writeText) {
    state.status = '当前环境不支持写入剪贴板'
    render()
    return
  }

  await navigator.clipboard.writeText(state.current.markdown)
  state.status = '已复制 Markdown'
  console.info('[capture-export] markdown copied', {
    length: state.current.markdown.length,
  })
  render()
}

function exportMarkdown() {
  const fileName = `${sanitizeFileName(state.current.title || 'guaxi-capture')}.md`
  const blob = new Blob([state.current.markdown], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = fileName
  link.click()
  URL.revokeObjectURL(url)
  state.status = `已导出 ${fileName}`
  console.info('[capture-export] markdown exported', {
    fileName,
    length: state.current.markdown.length,
  })
  render()
}

async function submitCurrent() {
  if (!state.current.markdown.trim()) {
    state.status = 'Markdown 内容不能为空'
    render()
    return
  }

  state.busy = true
  state.status = '正在发送...'
  render()

  try {
    await submitCapture(state.endpoint, state.current)
    state.current = createCaptureDraft()
    state.status = '已发送到 Guaxi'
  } catch (error) {
    enqueueCurrent('failed', error.message)
    state.status = `发送失败，已进入队列：${error.message}`
  } finally {
    state.busy = false
    persistAndRender()
  }
}

function enqueueCurrent(status = 'pending', lastError = '') {
  const draft = {
    ...state.current,
    status,
    lastError,
    updatedAt: new Date().toISOString(),
  }
  state.queue = [draft, ...state.queue.filter((item) => item.id !== draft.id)]
}

async function retryQueue() {
  if (!state.queue.length || state.busy) return
  state.busy = true
  state.status = '正在同步队列...'
  render()

  const nextQueue = []
  for (const item of state.queue) {
    try {
      await submitCapture(state.endpoint, item)
    } catch (error) {
      nextQueue.push({ ...item, status: 'failed', lastError: error.message })
    }
  }

  state.queue = nextQueue
  state.busy = false
  state.status = nextQueue.length ? `仍有 ${nextQueue.length} 条失败` : '队列同步完成'
  persistAndRender()
}

function persistAndRender() {
  saveQueue(state.queue)
  render()
}

function appendMarkdown(markdown) {
  const content = String(markdown || '').trim()
  if (!content) return
  state.current.markdown = [state.current.markdown.trim(), content]
    .filter(Boolean)
    .join('\n\n')
}

function hasImageAssets() {
  return state.current.assets.some(isImageAsset)
}

function ensureTags(tagsText, requiredTags) {
  const tags = String(tagsText || '')
    .split(',')
    .map((tag) => tag.trim())
    .filter(Boolean)
  for (const tag of requiredTags) {
    if (!tags.includes(tag)) tags.push(tag)
  }
  return tags.join(',')
}

function buildOcrTitle(files) {
  if (files.length === 1) return files[0].name.replace(/\.[^.]+$/, '') || 'OCR 采集'
  return `OCR 采集 ${new Date().toLocaleString()}`
}

function sourceOptions(current) {
  const options = [
    ['manual', '手动'],
    ['wechat', '微信公众号'],
    ['zhihu', '知乎'],
    ['xiaohongshu', '小红书'],
    ['bilibili', 'B站'],
    ['camera', '照片/OCR'],
    ['browser', '浏览器'],
    ['clipboard', '剪贴板'],
  ]

  return options
    .map(([value, label]) => `<option value="${value}" ${value === current ? 'selected' : ''}>${label}</option>`)
    .join('')
}

function renderQueueItem(item) {
  return `
    <article class="queue-item">
      <strong>${escapeHtml(item.title || '未命名')}</strong>
      <span>${escapeHtml(item.sourceApp)} · ${escapeHtml(item.fileRepo)}</span>
      ${item.lastError ? `<small>${escapeHtml(item.lastError)}</small>` : ''}
      <button type="button" data-action="load-queue" data-id="${escapeHtml(item.id)}">编辑</button>
    </article>
  `
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

function sanitizeFileName(value) {
  return String(value || 'guaxi-capture')
    .replace(/[\\/:*?"<>|]/g, '-')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 80) || 'guaxi-capture'
}

render()

registerNativeCaptureListener((payload) => {
  applyIncomingPayload(payload, '已接收系统分享草稿')
})

readStartupNativePayload().then((payload) => {
  if (payload) {
    applyIncomingPayload(payload, '已接收启动分享草稿')
  }
})
