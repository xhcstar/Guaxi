<template>
  <div class="guaxi-notebook-html-output">
    <iframe
      ref="iframeRef"
      class="guaxi-notebook-html-output__frame"
      :title="title"
      sandbox="allow-scripts allow-same-origin"
      scrolling="no"
      @load="handleFrameLoad"
    ></iframe>
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  html: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
  },
  title: {
    type: String,
    default: 'Notebook HTML Output',
  },
})

const logger = createLogger('GuaxiNotebookHtmlOutput')
const iframeRef = ref(null)
let frameHeightSyncTimer = 0

const buildSrcDoc = () => {
  const rawHtml = String(props.html || '')
  const isDark = String(props.theme || '').toLowerCase() === 'dark'
  const background = isDark ? '#1f232b' : '#ffffff'
  const color = isDark ? '#e6edf7' : '#172033'

  // 设计说明：
  // 1. Plotly 这类富 HTML 输出通常依赖内嵌 script 初始化，直接 v-html 不会执行脚本。
  // 2. 这里改成 iframe + srcdoc，把第三方脚本隔离到独立文档里执行，既能渲染交互图，也不会污染主页面。
  // 3. 通过 ResizeObserver + postMessage 把内容高度回传出来，避免 iframe 固定高度导致底部留白或裁切。
  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      html, body {
        margin: 0;
        padding: 0;
        background: ${background};
        color: ${color};
        overflow: hidden;
      }
      body {
        font-family: "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace, "Microsoft YaHei", "Courier New";
      }
      #guaxi-notebook-html-root {
        width: 100%;
        min-height: 1px;
      }
      #guaxi-notebook-html-root > * {
        max-width: 100%;
      }
    </style>
  </head>
  <body>
    <div id="guaxi-notebook-html-root">${rawHtml}</div>
    <script>
      (function () {
        const reportHeight = function () {
          const root = document.getElementById('guaxi-notebook-html-root');
          const body = document.body;
          const html = document.documentElement;
          const nextHeight = Math.max(
            root ? root.scrollHeight : 0,
            body ? body.scrollHeight : 0,
            html ? html.scrollHeight : 0,
            root ? root.getBoundingClientRect().height : 0,
            body ? body.getBoundingClientRect().height : 0,
            24
          );
          parent.postMessage({
            type: 'guaxi-notebook-html-height',
            height: Math.ceil(nextHeight)
          }, '*');
        };

        if (typeof ResizeObserver === 'function') {
          const observer = new ResizeObserver(function () {
            reportHeight();
          });
          observer.observe(document.documentElement);
          const root = document.getElementById('guaxi-notebook-html-root');
          if (root) observer.observe(root);
        }

        window.addEventListener('load', function () {
          reportHeight();
          setTimeout(reportHeight, 60);
          setTimeout(reportHeight, 180);
          setTimeout(reportHeight, 420);
        });
        window.addEventListener('resize', reportHeight);
        setTimeout(reportHeight, 0);
      })();
    <\/script>
  </body>
</html>`
}

const syncFrameContent = async (reason = 'unknown') => {
  await nextTick()
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  const srcdoc = buildSrcDoc()
  iframe.srcdoc = srcdoc
  iframe.style.height = '48px'
  logger.info('[NotebookHtmlOutput] synced iframe srcdoc', {
    reason,
    htmlLength: String(props.html || '').length,
  })
}

const syncFrameHeight = (height, reason = 'message') => {
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  const nextHeight = Math.max(48, Number(height || 0))
  iframe.style.height = `${nextHeight}px`
  logger.debug('[NotebookHtmlOutput] synced iframe height', {
    reason,
    height: nextHeight,
  })
}

const probeFrameHeight = (reason = 'probe') => {
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  try {
    const doc = iframe.contentDocument
    const root = doc?.getElementById('guaxi-notebook-html-root')
    const nextHeight = Math.max(
      Number(root?.scrollHeight || 0),
      Number(doc?.body?.scrollHeight || 0),
      Number(doc?.documentElement?.scrollHeight || 0),
      48,
    )
    syncFrameHeight(nextHeight, reason)
  } catch (error) {
    logger.debug('[NotebookHtmlOutput] probe frame height skipped', {
      reason,
      error: String(error?.message || error),
    })
  }
}

const scheduleFrameHeightProbe = (reason = 'schedule-probe') => {
  if (frameHeightSyncTimer) {
    window.clearTimeout(frameHeightSyncTimer)
  }
  frameHeightSyncTimer = window.setTimeout(() => {
    frameHeightSyncTimer = 0
    probeFrameHeight(reason)
  }, 120)
}

const handleFrameLoad = () => {
  scheduleFrameHeightProbe('iframe-load')
}

const handleWindowMessage = (event) => {
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  if (event?.source !== iframe.contentWindow) return
  if (String(event?.data?.type || '') !== 'guaxi-notebook-html-height') return
  syncFrameHeight(event?.data?.height, 'post-message')
}

watch(
  () => [props.html, props.theme],
  () => {
    syncFrameContent('props-watch')
  },
  { immediate: true },
)

onMounted(() => {
  window.addEventListener('message', handleWindowMessage)
})

onBeforeUnmount(() => {
  window.removeEventListener('message', handleWindowMessage)
  if (frameHeightSyncTimer) {
    window.clearTimeout(frameHeightSyncTimer)
    frameHeightSyncTimer = 0
  }
})
</script>

<style scoped>
.guaxi-notebook-html-output {
  width: 100%;
  min-width: 0;
}

.guaxi-notebook-html-output__frame {
  display: block;
  width: 100%;
  min-height: 48px;
  border: 0;
  background: transparent;
}
</style>
