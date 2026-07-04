const statusElement = document.querySelector('#status')

document.querySelectorAll('button[data-mode]').forEach((button) => {
  button.addEventListener('click', async () => {
    await send(button.dataset.mode)
  })
})

async function send(mode) {
  setStatus('正在发送...')
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
    const result = await chrome.runtime.sendMessage({
      type: 'GUAXI_CAPTURE_SEND_ACTIVE_TAB',
      tabId: tab.id,
      mode,
    })

    if (!result?.success) {
      throw new Error(result?.message || '发送失败')
    }

    setStatus(`已发送：${result.title || '当前内容'}`)
  } catch (error) {
    setStatus(`失败：${error.message}`)
  }
}

function setStatus(message) {
  statusElement.textContent = message
}
