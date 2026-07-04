const DEFAULT_ENDPOINT = 'http://127.0.0.1:14219/extension/capture/import'

export const STORAGE_KEYS = {
  endpoint: 'guaxi_capture_endpoint',
  queue: 'guaxi_capture_queue',
}

export function loadEndpoint() {
  return localStorage.getItem(STORAGE_KEYS.endpoint) || DEFAULT_ENDPOINT
}

export function saveEndpoint(endpoint) {
  localStorage.setItem(STORAGE_KEYS.endpoint, endpoint.trim() || DEFAULT_ENDPOINT)
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
