<template>
  <div class="diff-compare-window">
    <GuaxiDiffPanel :visible="true" @close="closeWindow" />
  </div>
</template>

<script setup>
/**
 * 独立差异对比窗口的宿主页面（路由 /diff-compare）。
 *
 * 这个页面本身几乎不做事，两件关键的事：
 * 1. becomeHost()：告诉 store 本 webview 就是对比窗口，之后各种 compare* 请求原地生效，
 *    不再往 Rust 转发去开窗（否则自己给自己发请求会绕圈）。
 * 2. 取请求：主窗口的请求先落在 Rust 的待取槽位里，冷启动时前端还没挂上监听，
 *    所以这里挂载完主动 take 一次；窗口已经开着时 Rust 会发 diff-compare:request 叫我们再来取。
 */
import { onBeforeUnmount, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import GuaxiDiffPanel from '../components/HomeViewComponent/GuaxiDiffPanel.vue'
import { useDiffCompareStore } from '../stores/diffCompareStore'
import { createLogger } from '../utils/logger'

const logger = createLogger('GuaxiDiffComparePage')
const store = useDiffCompareStore()

// 必须在面板 setup 之前打开这个开关：面板挂载时就会按当前 state 试着跑一次对比。
store.becomeHost()

let unlisten = null

const pullRequest = async () => {
  try {
    const req = await invoke('diff_compare_take_request')
    if (req) store.applyRequest(req)
  } catch (e) {
    logger.warn('[DiffWindow] take request failed', { error: String(e?.message || e || '') })
  }
}

const closeWindow = async () => {
  try {
    await getCurrentWindow().close()
  } catch (e) {
    logger.warn('[DiffWindow] close failed', { error: String(e?.message || e || '') })
  }
}

onMounted(async () => {
  try {
    unlisten = await listen('diff-compare:request', () => { void pullRequest() })
  } catch (e) {
    logger.warn('[DiffWindow] listen failed', { error: String(e?.message || e || '') })
  }
  await pullRequest()
})

onBeforeUnmount(() => {
  if (typeof unlisten === 'function') unlisten()
  unlisten = null
})
</script>

<style scoped>
.diff-compare-window {
  /* 独立窗口根：用 100vh 而非 100% —— 与本项目其它独立窗口一致。
     #app 与 naive 的 config/message provider 之间的包裹层会破坏 height:100% 的百分比链，
     导致整页按内容撑高、虚拟滚动的视口算不出高度。 */
  display: flex;
  flex-direction: column;
  height: 100vh;
  max-height: 100vh;
  overflow: hidden;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

.diff-compare-window :deep(.guaxi-diff-panel) {
  flex: 1 1 auto;
  min-height: 0;
}
</style>
