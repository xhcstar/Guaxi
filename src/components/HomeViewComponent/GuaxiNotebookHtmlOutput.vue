<template>
  <div class="guaxi-notebook-html-output">
    <iframe
      ref="iframeRef"
      class="guaxi-notebook-html-output__frame"
      :title="title"
      sandbox="allow-scripts allow-same-origin"
      :scrolling="src ? 'auto' : 'no'"
      @load="handleFrameLoad"
    ></iframe>
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  html: {
    type: String,
    default: '',
  },
  src: {
    type: String,
    default: '',
  },
  sourcePath: {
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
let syncToken = 0

const base64ToText = (base64 = '') => {
  const binary = window.atob(String(base64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return new TextDecoder('utf-8').decode(bytes)
}

const normalizeLocalPath = (path = '') => String(path || '').replace(/\\/g, '/')

const dirname = (path = '') => {
  const normalized = normalizeLocalPath(path)
  const index = normalized.lastIndexOf('/')
  return index >= 0 ? normalized.slice(0, index) : ''
}

const joinLocalPath = (basePath = '', relativePath = '') => {
  const base = normalizeLocalPath(basePath).replace(/\/+$/, '')
  const relative = normalizeLocalPath(relativePath).replace(/^\/+/, '')
  return base ? `${base}/${relative}` : relative
}

const readFileBase64 = async (path, reason = 'html-output') => {
  const normalizedPath = String(path || '').trim()
  if (!normalizedPath) return ''
  const base64 = await invoke('read_file_bytes_base64', { path: normalizedPath })
  logger.debug('[NotebookHtmlOutput] read local artifact file', {
    reason,
    path: normalizedPath,
    base64Length: String(base64 || '').length,
  })
  return String(base64 || '')
}

const inlineLocalAnimationFrames = async (html, htmlPath, token) => {
  const rawHtml = String(html || '')
  const baseDir = dirname(htmlPath)
  const generatedFrameCount = Number(rawHtml.match(/var\s+frames\s*=\s*new\s+Array\((\d+)\)/)?.[1] || 0)
  const generatedFramePaths = generatedFrameCount > 0
    ? Array.from({ length: generatedFrameCount }, (_, index) => `animation_frames/frame${String(index).padStart(7, '0')}.png`)
    : []
  const literalFrameMatches = [...rawHtml.matchAll(/(["'])([^"']*animation_frames\/frame\d+\.(?:png|jpg|jpeg|webp|gif))\1/gi)]
  const literalFramePaths = literalFrameMatches.map((match) => String(match[2] || '').trim()).filter(Boolean)
  const framePaths = [...new Set([...generatedFramePaths, ...literalFramePaths])]
  if (!framePaths.length || !baseDir) return rawHtml

  const replacements = new Map()
  await Promise.all(framePaths.map(async (relativePath) => {
    if (token !== syncToken) return
    const absolutePath = joinLocalPath(baseDir, relativePath)
    const ext = String(relativePath.split('.').pop() || 'png').toLowerCase()
    const mime = ext === 'jpg' || ext === 'jpeg' ? 'image/jpeg' : ext === 'webp' ? 'image/webp' : ext === 'gif' ? 'image/gif' : 'image/png'
    const frameBase64 = await readFileBase64(absolutePath, 'matplotlib-animation-frame')
    replacements.set(relativePath, `data:${mime};base64,${frameBase64}`)
  }))

  let nextHtml = rawHtml
  if (generatedFrameCount > 0) {
    const dataFrames = generatedFramePaths
      .map((relativePath) => replacements.get(relativePath))
      .filter(Boolean)
    if (dataFrames.length === generatedFrameCount) {
      const dataFrameAssignment = `\n    frames = ${JSON.stringify(dataFrames)};\n\n    `
      nextHtml = nextHtml.replace(
        /(\s*\/\* set a timeout to make sure all the above elements are created before)/,
        dataFrameAssignment + '$1',
      )
      logger.info('[NotebookHtmlOutput] replaced generated matplotlib frame array with data urls', {
        htmlPath,
        frameCount: dataFrames.length,
      })
    } else {
      logger.warn('[NotebookHtmlOutput] generated matplotlib frame array was incomplete', {
        htmlPath,
        expectedFrameCount: generatedFrameCount,
        resolvedFrameCount: dataFrames.length,
      })
    }
  }
  for (const [relativePath, dataUrl] of replacements.entries()) {
    nextHtml = nextHtml.split(relativePath).join(dataUrl)
  }
  logger.info('[NotebookHtmlOutput] inlined matplotlib animation frames', {
    htmlPath,
    frameCount: replacements.size,
  })
  return nextHtml
}

const buildArtifactSrcDoc = (artifactHtml) => {
  const rawHtml = String(artifactHtml || '')
  const isDark = String(props.theme || '').toLowerCase() === 'dark'
  const background = isDark ? '#1f232b' : '#ffffff'
  const color = isDark ? '#e6edf7' : '#172033'

  // 设计说明：
  // 1. Matplotlib HTMLWriter 会生成完整的动画控件和脚本，但原始文件引用本地帧目录；
  //    在 Tauri iframe 中直接用 asset URL 读取运行时目录会被 403 拦截。
  // 2. 这里先由后端授权命令读取 HTML 与帧图片，再把帧图片内联为 data URL，最终仍使用 srcdoc 隔离执行脚本。
  // 3. 同时注入高度回传脚本，避免动画图片加载后 iframe 仍停留在 48px 的空白条。
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
        overflow: auto;
      }
      body {
        font-family: "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace, "Microsoft YaHei", "Courier New";
      }
      .animation {
        width: 100%;
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: flex-start;
        gap: 12px;
        padding: 8px 12px 16px;
      }
      .animation img {
        width: auto;
        height: clamp(520px, 62vh, 760px);
        max-width: 100%;
        max-height: none;
        object-fit: contain;
        background: #ffffff;
        display: block;
      }
      .animation .anim-controls {
        width: min(100%, 760px);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
      }
      .animation .anim-slider {
        width: min(100%, 620px);
      }
      .animation .anim-buttons {
        display: flex;
        justify-content: center;
        flex-wrap: wrap;
        gap: 6px;
      }
      .animation .anim-state {
        display: flex;
        justify-content: center;
        flex-wrap: wrap;
        gap: 10px;
      }
    </style>
  </head>
  <body>
    ${rawHtml}
    <script>
      (function () {
        const reportHeight = function () {
          const body = document.body;
          const html = document.documentElement;
          const nextHeight = Math.max(
            body ? body.scrollHeight : 0,
            html ? html.scrollHeight : 0,
            body ? body.getBoundingClientRect().height : 0,
            320
          );
          parent.postMessage({
            type: 'guaxi-notebook-html-height',
            height: Math.ceil(nextHeight)
          }, '*');
        };
        if (typeof ResizeObserver === 'function') {
          const observer = new ResizeObserver(reportHeight);
          observer.observe(document.documentElement);
          if (document.body) observer.observe(document.body);
        }
        window.addEventListener('load', function () {
          reportHeight();
          setTimeout(reportHeight, 80);
          setTimeout(reportHeight, 240);
          setTimeout(reportHeight, 600);
        });
        window.addEventListener('resize', reportHeight);
        setTimeout(reportHeight, 0);
      })();
    <\/script>
  </body>
</html>`
}

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
  const token = syncToken + 1
  syncToken = token
  await nextTick()
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  const sourcePath = String(props.sourcePath || '').trim()
  if (sourcePath) {
    iframe.removeAttribute('src')
    iframe.style.height = '760px'
    try {
      const htmlBase64 = await readFileBase64(sourcePath, 'matplotlib-animation-html')
      if (token !== syncToken) return
      const html = base64ToText(htmlBase64)
      const inlinedHtml = await inlineLocalAnimationFrames(html, sourcePath, token)
      if (token !== syncToken) return
      iframe.srcdoc = buildArtifactSrcDoc(inlinedHtml)
      logger.info('[NotebookHtmlOutput] synced local artifact html into iframe srcdoc', {
        reason,
        sourcePath,
        htmlLength: inlinedHtml.length,
      })
    } catch (error) {
      logger.error('[NotebookHtmlOutput] sync local artifact html failed', {
        reason,
        sourcePath,
        error: String(error?.message || error),
      })
      iframe.srcdoc = buildSrcDoc()
    }
    return
  }
  const sourceUrl = String(props.src || '').trim()
  if (sourceUrl) {
    iframe.removeAttribute('srcdoc')
    iframe.src = sourceUrl
    iframe.style.height = '520px'
    logger.info('[NotebookHtmlOutput] synced iframe external html source', {
      reason,
      sourceUrl,
    })
    return
  }
  const srcdoc = buildSrcDoc()
  iframe.removeAttribute('src')
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
  () => [props.html, props.src, props.sourcePath, props.theme],
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
