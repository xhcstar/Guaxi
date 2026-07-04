const CAPTURE_IMPORT_PORT = '14219'
const CAPTURE_IMPORT_PATH = '/extension/capture/import'

function inferDefaultEndpoint() {
  const envHost = import.meta.env?.VITE_GUAXI_PC_HOST
  const locationHost = typeof window !== 'undefined' ? window.location.hostname : ''
  const host = normalizeEndpointHost(envHost || locationHost)

  if (!host) {
    return `http://127.0.0.1:${CAPTURE_IMPORT_PORT}${CAPTURE_IMPORT_PATH}`
  }

  return `http://${host}:${CAPTURE_IMPORT_PORT}${CAPTURE_IMPORT_PATH}`
}

function normalizeEndpointHost(host) {
  const value = String(host || '').trim()
  if (!value || value === 'tauri.localhost' || value === 'localhost') {
    return '127.0.0.1'
  }
  return value
}

export const DEFAULT_ENDPOINT = inferDefaultEndpoint()

export const STORAGE_KEYS = {
  endpoint: 'guaxi_capture_endpoint',
  queue: 'guaxi_capture_queue',
}

export function loadEndpoint() {
  const stored = localStorage.getItem(STORAGE_KEYS.endpoint)
  if (stored && isUsableStoredEndpoint(stored)) {
    return stored
  }

  console.info('[capture] using default endpoint', {
    endpoint: DEFAULT_ENDPOINT,
    stored,
  })
  return DEFAULT_ENDPOINT
}

export function saveEndpoint(endpoint) {
  localStorage.setItem(STORAGE_KEYS.endpoint, endpoint.trim() || DEFAULT_ENDPOINT)
}

function isUsableStoredEndpoint(endpoint) {
  const value = String(endpoint || '').trim()
  if (!value) return false

  // 设计说明：
  // 旧版本默认值是 127.0.0.1:14219。它在 PC 浏览器里可用，但在 Android/iOS 真机上
  // 会指向手机自身，导致发送到 Guaxi PC 失败。Tauri 移动端下自动迁移到当前
  // dev 脚本探测出的 PC 局域网 IP；用户手动填过的局域网地址则保留。
  const isMobileTauri = Boolean(window.__TAURI_INTERNALS__) && window.location.hostname === 'tauri.localhost'
  if (isMobileTauri && /^http:\/\/(127\.0\.0\.1|localhost):14219\//i.test(value)) {
    console.info('[capture] ignore stale localhost endpoint on mobile', {
      endpoint: value,
      nextEndpoint: DEFAULT_ENDPOINT,
    })
    return false
  }

  return true
}

export function loadQueue() {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.queue)
    return raw ? JSON.parse(raw) : []
  } catch (error) {
    console.warn('[capture] queue parse failed', error)
    return []
  }
}

export function saveQueue(queue) {
  localStorage.setItem(STORAGE_KEYS.queue, JSON.stringify(queue))
}

export function createCaptureDraft(overrides = {}) {
  return {
    id: crypto.randomUUID(),
    title: '',
    sourceUrl: '',
    sourceApp: 'manual',
    description: '',
    knowledgeBase: 'default',
    fileRepo: 'Inbox',
    tagsText: 'capture',
    markdown: '',
    assets: [],
    status: 'draft',
    createdAt: formatLocalTimestamp(new Date()),
    updatedAt: formatLocalTimestamp(new Date()),
    lastError: '',
    ...overrides,
  }
}

export function toCapturePayload(draft) {
  return {
    title: draft.title.trim(),
    sourceUrl: draft.sourceUrl.trim(),
    sourceApp: draft.sourceApp.trim() || 'manual',
    description: draft.description.trim(),
    knowledgeBase: draft.knowledgeBase.trim() || 'default',
    fileRepo: draft.fileRepo.trim() || 'Inbox',
    tags: draft.tagsText
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean),
    createdAt: draft.createdAt,
    markdown: draft.markdown.trim(),
    assets: draft.assets || [],
  }
}

export async function submitCapture(endpoint, draft) {
  const payload = toCapturePayload(draft)
  console.info('[capture] submitting payload', {
    endpoint,
    title: payload.title,
    sourceApp: payload.sourceApp,
    assetCount: payload.assets.length,
  })

  const response = await fetch(endpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  })

  const data = await response.json().catch(() => ({}))
  if (!response.ok || data?.success === false) {
    const message = data?.message || `HTTP ${response.status}`
    throw new Error(message)
  }

  console.info('[capture] submit ok', data)
  return data
}

export async function fileToAsset(file) {
  const dataBase64 = await readFileAsBase64(file)
  return {
    name: file.name,
    mime: file.type || 'application/octet-stream',
    dataBase64,
  }
}

export function formatLocalTimestamp(date) {
  const pad = (value) => String(value).padStart(2, '0')
  return [
    date.getFullYear(),
    pad(date.getMonth() + 1),
    pad(date.getDate()),
  ].join('-') + ` ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

function readFileAsBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result || '')
      resolve(result.includes(',') ? result.slice(result.indexOf(',') + 1) : result)
    }
    reader.onerror = () => reject(reader.error || new Error('读取文件失败'))
    reader.readAsDataURL(file)
  })
}
