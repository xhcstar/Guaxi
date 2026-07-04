const RECEIVER_URL = 'http://127.0.0.1:14230/capture'

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: 'guaxi-capture-article',
    title: '发送网页到 Guaxi Capture',
    contexts: ['page'],
  })

  chrome.contextMenus.create({
    id: 'guaxi-capture-selection',
    title: '发送选中内容到 Guaxi Capture',
    contexts: ['selection'],
  })
})

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  const mode = info.menuItemId === 'guaxi-capture-selection' ? 'selection' : 'article'
  const result = await sendCurrentTabToCapture(tab.id, mode)
  console.info('[guaxi-extension] context menu send result', result)
})

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message?.type !== 'GUAXI_CAPTURE_SEND_ACTIVE_TAB') return false

  sendCurrentTabToCapture(message.tabId || sender.tab?.id, message.mode || 'article')
    .then((result) => sendResponse(result))
    .catch((error) => sendResponse({ success: false, message: error.message }))

  return true
})

async function sendCurrentTabToCapture(tabId, mode) {
  if (!tabId) {
    throw new Error('无法获取当前标签页')
  }

  const payload = await extractFromTab(tabId, mode)
  const response = await fetch(RECEIVER_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  })

  const data = await response.json().catch(() => ({}))
  if (!response.ok || data?.success === false) {
    throw new Error(data?.message || `Guaxi Capture HTTP ${response.status}`)
  }

  return {
    success: true,
    title: payload.title,
    message: '已发送到 Guaxi Capture',
  }
}

async function extractFromTab(tabId, mode) {
  try {
    const response = await chrome.tabs.sendMessage(tabId, {
      type: 'GUAXI_CAPTURE_EXTRACT',
      mode,
    })
    if (response?.success) return response.payload
    throw new Error(response?.message || '内容脚本提取失败')
  } catch (error) {
    console.info('[guaxi-extension] reinjecting content script', error)
    await chrome.scripting.executeScript({
      target: { tabId },
      files: ['content.js'],
    })
    const response = await chrome.tabs.sendMessage(tabId, {
      type: 'GUAXI_CAPTURE_EXTRACT',
      mode,
    })
    if (response?.success) return response.payload
    throw new Error(response?.message || '内容脚本提取失败')
  }
}
