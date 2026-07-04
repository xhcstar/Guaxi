const DEFAULT_OCR_ENDPOINT = 'http://127.0.0.1:9876/ocr'

export const OCR_STORAGE_KEYS = {
  endpoint: 'guaxi_capture_ocr_endpoint',
}

export function loadOcrEndpoint() {
  return localStorage.getItem(OCR_STORAGE_KEYS.endpoint) || DEFAULT_OCR_ENDPOINT
}

export function saveOcrEndpoint(endpoint) {
  localStorage.setItem(OCR_STORAGE_KEYS.endpoint, endpoint.trim() || DEFAULT_OCR_ENDPOINT)
}

// 设计说明：
// Guaxi 主程序的 OCR command 最终也是调用 agent_server 的 /ocr 端点。
// Capture App 保持独立，不直接依赖主程序 Rust crate；只复用同一个本机 OCR HTTP 接口。
export async function recognizeImageAsset(endpoint, asset) {
  if (!asset?.dataBase64) {
    throw new Error('图片数据为空，无法 OCR')
  }

  console.info('[capture-ocr] recognizing image', {
    endpoint,
    name: asset.name,
    mime: asset.mime,
  })

  const data = await callOcr(endpoint, asset.dataBase64)

  console.info('[capture-ocr] recognition ok', {
    name: asset.name,
    textLength: String(data?.text || '').length,
    lineCount: Array.isArray(data?.lines) ? data.lines.length : 0,
  })

  return {
    text: String(data?.text || '').trim(),
    lines: Array.isArray(data?.lines) ? data.lines : [],
    raw: data,
  }
}

async function callOcr(endpoint, imageBase64) {
  if (window.__TAURI_INTERNALS__) {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke('capture_ocr_recognize', {
      endpoint,
      imageBase64,
    })
  }

  console.info('[capture-ocr] tauri proxy unavailable, using fetch fallback')
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      image_path: '',
      image_base64: imageBase64,
    }),
  })

  const data = await response.json().catch(() => ({}))
  if (!response.ok) {
    throw new Error(data?.message || `OCR HTTP ${response.status}`)
  }
  return data
}

export function isImageAsset(asset) {
  return String(asset?.mime || '').startsWith('image/')
}

export function buildOcrMarkdown(asset, result) {
  const title = asset?.name ? `## OCR：${asset.name}` : '## OCR 识别结果'
  const text = String(result?.text || '').trim()
  return text ? `${title}\n\n${text}` : `${title}\n\n> 未识别到文字`
}
