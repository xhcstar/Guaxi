<template>
  <div class="guaxi-notebook-html-output">
    <iframe
      ref="iframeRef"
      class="guaxi-notebook-html-output__frame"
      :title="title"
      :sandbox="iframeSandbox"
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
let frameDiagnosticsTimer = 0
let syncToken = 0
let lastFrameDiagnostics = null
let hasHeightMessageForCurrentFrame = false
let frameBlobUrl = ''

// 设计说明：
// 1. HTML/动画输出需要 allow-scripts，否则 Plotly、Matplotlib HTMLWriter 的交互脚本不会执行。
// 2. 这里刻意不加 allow-same-origin：Chromium 会对 `allow-scripts + allow-same-origin` 发安全警告，
//    且 srcdoc 文档获得同源能力后理论上可以解除 sandbox。
// 3. 高度同步改走 iframe 内部 postMessage；父页面只校验 event.source，不依赖 contentDocument 同源访问。
const iframeSandbox = 'allow-scripts'

const collectHtmlDiagnostics = (html = '', { sourcePath = '', sourceUrl = '', reason = '' } = {}) => {
  const rawHtml = String(html || '')
  return {
    reason: String(reason || ''),
    sourcePath: String(sourcePath || ''),
    sourceUrl: String(sourceUrl || ''),
    htmlLength: rawHtml.length,
    inlineScriptCount: (rawHtml.match(/<script\b/gi) || []).length,
    dataUrlCount: (rawHtml.match(/data:/gi) || []).length,
    blobUrlCount: (rawHtml.match(/blob:/gi) || []).length,
    hasMatplotlibAnimation: /matplotlib|animation_frames|anim-controls|FuncAnimation|frames\s*=|new\s+Array\(/i.test(rawHtml),
    hasHtml5Video: /<video\b|video\/mp4|video\/webm/i.test(rawHtml),
  }
}

const rememberFrameDiagnostics = (diagnostics) => {
  lastFrameDiagnostics = diagnostics && typeof diagnostics === 'object' ? { ...diagnostics } : null
  hasHeightMessageForCurrentFrame = false
}

const scheduleFrameDiagnosticsProbe = (reason = 'diagnostics-probe') => {
  if (frameDiagnosticsTimer) {
    window.clearTimeout(frameDiagnosticsTimer)
  }
  frameDiagnosticsTimer = window.setTimeout(() => {
    frameDiagnosticsTimer = 0
    const iframe = iframeRef.value
    if (!(iframe instanceof HTMLIFrameElement) || !lastFrameDiagnostics) return
    const currentHeight = Number.parseFloat(String(iframe.style.height || '0')) || 0
    // 设计思路：
    // 1. release 模式通常不能打开 DevTools，因此这里把“脚本可能被 CSP 拦截”的可观测信号写入前端日志。
    // 2. matplotlib 动画 iframe 正常执行时会通过 postMessage 回传高度；如果长时间没有回传且仍停在初始高度，
    //    基本可以判断 iframe 内联脚本没有运行，最常见原因就是 release CSP。
    // 3. 这里只记录摘要，不写入完整 HTML，避免日志文件被 base64 动画帧撑爆。
    if (!hasHeightMessageForCurrentFrame && lastFrameDiagnostics.inlineScriptCount > 0 && currentHeight <= 80) {
      logger.warn('[NotebookHtmlOutput] iframe html scripts may be blocked before height handshake', {
        reason,
        currentHeight,
        sandbox: iframeSandbox,
        ...lastFrameDiagnostics,
      })
    } else {
      logger.info('[NotebookHtmlOutput] iframe html diagnostics probe completed', {
        reason,
        currentHeight,
        hasHeightMessage: hasHeightMessageForCurrentFrame,
        sandbox: iframeSandbox,
        ...lastFrameDiagnostics,
      })
    }
  }, 1600)
}

const base64ToText = (base64 = '') => {
  const binary = window.atob(String(base64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return new TextDecoder('utf-8').decode(bytes)
}

const revokeFrameBlobUrl = (reason = 'unknown') => {
  if (!frameBlobUrl) return
  try {
    URL.revokeObjectURL(frameBlobUrl)
    logger.debug('[NotebookHtmlOutput] revoked iframe blob url', { reason })
  } catch (error) {
    logger.warn('[NotebookHtmlOutput] failed to revoke iframe blob url', {
      reason,
      error: String(error?.message || error),
    })
  }
  frameBlobUrl = ''
}

const writeFrameHtmlDocument = (iframe, html, reason = 'html-output') => {
  if (!(iframe instanceof HTMLIFrameElement)) return
  const htmlText = String(html || '')
  // 设计说明：
  // 1. release WebView2 中 iframe srcdoc 会继承主页面 CSP，matplotlib HTMLWriter 的内联脚本容易被拦截。
  // 2. data:text/html;base64 能绕过 srcdoc，但动画帧内联后 URL 可能过长；Blob URL 没有这个长度压力。
  // 3. iframe 仍保持 sandbox="allow-scripts"，第三方 HTML 只在隔离文档中执行；日志只记录长度，不写完整 HTML。
  revokeFrameBlobUrl(`${reason}:before-write`)
  const blob = new Blob([htmlText], { type: 'text/html;charset=utf-8' })
  frameBlobUrl = URL.createObjectURL(blob)
  iframe.removeAttribute('srcdoc')
  iframe.src = frameBlobUrl
  logger.info('[NotebookHtmlOutput] wrote iframe html through blob url', {
    reason,
    htmlLength: htmlText.length,
    blobSize: blob.size,
  })
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
        width: 100%;
        max-width: 100%;
        box-sizing: border-box;
        background: ${background};
        color: ${color};
        overflow-x: hidden;
        overflow-y: auto;
      }
      body {
        font-family: "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace, "Microsoft YaHei", "Courier New";
        overflow-wrap: anywhere;
      }
      /* 设计说明：
       * 1. Matplotlib HTMLWriter 生成的结构通常是 animation + img + controls，但不同版本会混用
       *    inline style、table-like div 或普通 form controls，默认靠左显示。
       * 2. iframe 内样式不会污染主应用，因此这里用较强选择器把动画画面、进度条、按钮区统一居中。
       * 3. 只约束动画 HTML 文档，不影响 Plotly 等普通 HTML 输出。
       */
      .animation {
        width: 100%;
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: flex-start;
        gap: 12px;
        padding: 8px 12px 16px;
        text-align: center;
        overflow-x: hidden;
      }
      .animation > *,
      .animation div,
      .animation form,
      .animation table {
        max-width: 100%;
        margin-left: auto !important;
        margin-right: auto !important;
        text-align: center;
        box-sizing: border-box;
      }
      .animation img,
      .animation canvas,
      .animation video {
        width: auto;
        height: clamp(520px, 62vh, 760px);
        max-width: 100%;
        max-height: none;
        object-fit: contain;
        background: #ffffff;
        display: block;
        margin-left: auto !important;
        margin-right: auto !important;
      }
      .animation .anim-controls,
      .animation [class*="controls"],
      .animation [id*="controls"] {
        width: min(100%, 760px);
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
      }
      .animation .anim-slider,
      .animation input[type="range"] {
        width: min(100%, 620px);
        display: block;
        margin-left: auto !important;
        margin-right: auto !important;
      }
      .animation .anim-buttons,
      .animation [class*="button"],
      .animation [id*="button"] {
        display: flex;
        justify-content: center;
        align-items: center;
        flex-wrap: wrap;
        gap: 6px;
      }
      .animation .anim-state,
      .animation [class*="state"] {
        display: flex;
        justify-content: center;
        align-items: center;
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
        width: 100%;
        max-width: 100%;
        box-sizing: border-box;
        background: ${background};
        color: ${color};
        overflow: hidden;
      }
      body {
        font-family: "Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace, "Microsoft YaHei", "Courier New";
        overflow-wrap: anywhere;
      }
      #guaxi-notebook-html-root {
        width: 100%;
        min-height: 1px;
      }
      #guaxi-notebook-html-root > * {
        max-width: 100%;
      }
      /* 设计说明：
       * 1. 有些用户会直接输出 FuncAnimation.to_jshtml()，此时没有 sourcePath，也会走普通 HTML srcdoc。
       * 2. 这里复用动画居中约束，让内联 HTML 动画和本地 artifact 动画保持一致。
       */
      #guaxi-notebook-html-root .animation {
        width: 100% !important;
        box-sizing: border-box;
        display: flex !important;
        flex-direction: column;
        align-items: center;
        justify-content: flex-start;
        gap: 12px;
        text-align: center;
        overflow-x: hidden;
      }
      #guaxi-notebook-html-root .animation > *,
      #guaxi-notebook-html-root .animation div,
      #guaxi-notebook-html-root .animation form,
      #guaxi-notebook-html-root .animation table {
        max-width: 100%;
        margin-left: auto !important;
        margin-right: auto !important;
        text-align: center;
        box-sizing: border-box;
      }
      #guaxi-notebook-html-root .animation img,
      #guaxi-notebook-html-root .animation canvas,
      #guaxi-notebook-html-root .animation video {
        display: block;
        max-width: 100%;
        height: auto;
        object-fit: contain;
        margin-left: auto !important;
        margin-right: auto !important;
      }
      #guaxi-notebook-html-root .animation .anim-controls,
      #guaxi-notebook-html-root .animation [class*="controls"],
      #guaxi-notebook-html-root .animation [id*="controls"] {
        width: min(100%, 760px);
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
      }
      #guaxi-notebook-html-root .animation .anim-slider,
      #guaxi-notebook-html-root .animation input[type="range"] {
        width: min(100%, 620px);
        display: block;
        margin-left: auto !important;
        margin-right: auto !important;
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
      const diagnostics = collectHtmlDiagnostics(inlinedHtml, { sourcePath, reason })
      rememberFrameDiagnostics(diagnostics)
      writeFrameHtmlDocument(iframe, buildArtifactSrcDoc(inlinedHtml), 'local-artifact-blob-url')
      logger.info('[NotebookHtmlOutput] synced local artifact html into iframe blob url', {
        reason,
        sourcePath,
        htmlLength: inlinedHtml.length,
        diagnostics,
      })
      scheduleFrameDiagnosticsProbe('local-artifact-blob-url')
    } catch (error) {
      logger.error('[NotebookHtmlOutput] sync local artifact html failed', {
        reason,
        sourcePath,
        error: String(error?.message || error),
      })
      const fallbackHtml = buildSrcDoc()
      rememberFrameDiagnostics(collectHtmlDiagnostics(props.html, { sourcePath, reason: `${reason}:fallback` }))
      writeFrameHtmlDocument(iframe, fallbackHtml, 'local-artifact-fallback-blob-url')
      scheduleFrameDiagnosticsProbe('local-artifact-fallback-blob-url')
    }
    return
  }
  const sourceUrl = String(props.src || '').trim()
  if (sourceUrl) {
    rememberFrameDiagnostics(collectHtmlDiagnostics('', { sourceUrl, reason }))
    revokeFrameBlobUrl('external-source')
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
  const diagnostics = collectHtmlDiagnostics(props.html, { reason })
  rememberFrameDiagnostics(diagnostics)
  writeFrameHtmlDocument(iframe, srcdoc, 'inline-blob-url')
  iframe.style.height = '48px'
  logger.info('[NotebookHtmlOutput] synced iframe blob url', {
    reason,
    htmlLength: String(props.html || '').length,
    diagnostics,
  })
  scheduleFrameDiagnosticsProbe('inline-blob-url')
}

const syncFrameHeight = (height, reason = 'message') => {
  const iframe = iframeRef.value
  if (!(iframe instanceof HTMLIFrameElement)) return
  const nextHeight = Math.max(48, Number(height || 0))
  hasHeightMessageForCurrentFrame = true
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
    logger.debug('[NotebookHtmlOutput] skipped direct iframe height probe because sandbox is origin-isolated', {
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
  if (frameDiagnosticsTimer) {
    window.clearTimeout(frameDiagnosticsTimer)
    frameDiagnosticsTimer = 0
  }
  revokeFrameBlobUrl('before-unmount')
})
</script>

<style scoped>
.guaxi-notebook-html-output {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  box-sizing: border-box;
  /* 设计说明：
   * 1. 动画 HTML 运行在 iframe 内，横向滚动应由 iframe 文档里的内容约束解决。
   * 2. 外层输出组件只负责占满当前输出模块宽度；这里阻断 iframe/内联控件的 scrollWidth 向 Notebook 外层泄漏。
   */
  overflow-x: hidden;
}

.guaxi-notebook-html-output__frame {
  display: block;
  width: 100%;
  max-width: 100%;
  min-height: 48px;
  box-sizing: border-box;
  border: 0;
  background: transparent;
}
</style>
