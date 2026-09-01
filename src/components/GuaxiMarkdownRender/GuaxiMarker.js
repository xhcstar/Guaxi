import MarkdownIt from "markdown-it";
import markdownItMathjax from "markdown-it-mathjax3";
import mermaid from "mermaid";
import hljs from "highlight.js";
import "highlight.js/styles/atom-one-light.css";
// 暗色 One Dark 代码配色 + 纯图片段落布局（作用域限定，详见文件内说明）。
import "./guaxiMarkdownExtras.css";
import { initMathJax, renderMath } from "./mathjaxConfig";
import { allowFileProtocolLinks } from "../../utils/markdownFileLink";
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiMarker')
let mermaidRenderCounter = 0

const renderPlainFenceBlock = (code, md) => {
  const escaped = md.utils.escapeHtml(code);
  return `<pre class="hljs hljs-plain"><code class="language-plaintext">${escaped}</code></pre>`;
}

const RenderOptions = {
  mermaid: undefined,
  math: undefined,
  codeHighlight: undefined,
  theme: "default", // 移除类型注解
  basePath: "",
};

export const addCopyButtons = (container, options = {}) => {
  // 移除已有按钮
  container.querySelectorAll(".copy-button").forEach((btn) => btn.remove());

  const theme = String(options?.theme || '').trim().toLowerCase();
  const fromDom =
    !!container?.closest?.('.dark') ||
    !!container?.closest?.('.dark-mode') ||
    !!document?.documentElement?.classList?.contains?.('dark') ||
    !!document?.documentElement?.classList?.contains?.('dark-mode');
  const isDark = theme === 'dark' || fromDom;

  const shouldSkipForNotes = (el) => {
    try {
      if (options?.includeNotes === true) return false;
      return !!el?.closest?.('.notes-panel');
    } catch {
      return false;
    }
  };

  const shouldSkipForInteractiveCells = (el) => {
    try {
      // 设计思路：
      // 1. Python/Code Cell 的主代码块也需要复制按钮，不能再因为外层是 `.guaxi-pycell/.guaxi-codecell` 而整体跳过。
      // 2. 但媒体展示区、PDF 预览壳、以及执行输出区不是“源码输入块”，继续跳过，避免生成无意义或重复的复制按钮。
      // 3. 这里保留日志，方便后续继续判断实际命中的跳过类型。
      const shouldSkip = !!el?.closest?.(
        '.guaxi-codecell-media, .guaxi-pycell-media, .guaxi-pdf-preview-shell'
      ) || el?.classList?.contains?.('guaxi-codecell-output') || el?.classList?.contains?.('guaxi-pycell-output');

      if (shouldSkip) {
        logger.debug('[MarkdownRender] skip copy button for interactive auxiliary block', {
          blockTag: String(el?.tagName || '').toLowerCase(),
          blockClass: String(el?.className || ''),
        });
      }

      return shouldSkip;
    } catch {
      return false;
    }
  };

  // 设计说明：
  // 1. Markdown fenced code block 的稳定块级边界是 `<pre>`，优先从这里挂载复制按钮。
  // 2. 历史渲染结果里 `.hljs` 可能挂在 `<pre>`、`<code>` 或独立块节点上，因此保留 `.hljs` 兜底兼容。
  // 3. 这样能把带语言标记和 plaintext 代码块统一到同一条识别路径，避免复制图标因结构差异漏挂。
  const codeContainers = Array.from(
    container.querySelectorAll("pre, .hljs")
  );
  const processedBlocks = new Set();

  codeContainers.forEach((codeContainer) => {
    // 确保容器存在且没有重复按钮。
    if (!codeContainer || codeContainer.querySelector(".copy-button")) return;

    // 设计说明：
    // 1. 标准 Markdown 代码块优先落到 `<pre>`。
    // 2. 如果命中了 `<code class="hljs">`，则回退到外层 `<pre>`，避免按钮被塞进 `<pre>` 内层。
    // 3. 对旧结构的块级 `.hljs` 节点继续兼容，保证历史消息和不同渲染链都能正常显示复制按钮。
    let codeBlock = null;
    const tagName = String(codeContainer.tagName || "").toUpperCase();
    if (tagName === "PRE") {
      codeBlock = codeContainer;
    } else if (tagName === "CODE" && codeContainer.parentElement?.tagName === "PRE") {
      codeBlock = codeContainer.parentElement;
    } else if (codeContainer.classList.contains("hljs")) {
      codeBlock = codeContainer;
    }

    if (!codeBlock) return;
    if (!codeBlock.querySelector("code")) {
      logger.debug("[MarkdownRender] skip non-code block when adding copy button", {
        blockTag: String(codeBlock.tagName || "").toLowerCase(),
        blockClass: String(codeBlock.className || ""),
      });
      return;
    }
    if (processedBlocks.has(codeBlock)) return;
    processedBlocks.add(codeBlock);

    logger.debug("[MarkdownRender] normalize code block for copy button", {
      containerTag: String(codeContainer.tagName || "").toLowerCase(),
      containerClass: String(codeContainer.className || ""),
      blockTag: String(codeBlock.tagName || "").toLowerCase(),
      blockClass: String(codeBlock.className || ""),
      languageClass: String(codeBlock.querySelector("code")?.className || ""),
    });

    if (shouldSkipForNotes(codeBlock)) return;
    if (shouldSkipForInteractiveCells(codeBlock)) return;

    let anchor = codeBlock.parentElement;
    if (!anchor || !anchor.classList?.contains("guaxi-code-block-anchor")) {
      anchor = document.createElement("div");
      anchor.className = "guaxi-code-block-anchor";
      anchor.style.cssText = `
        position: relative;
      `;
      codeBlock.parentNode?.insertBefore(anchor, codeBlock);
      anchor.appendChild(codeBlock);
    }

    // 创建复制按钮
    const copyButton = document.createElement("button");
    copyButton.className = "copy-button";

    const iconSvgMap = {
      copy: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/>
        </svg>
      `,
      check: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
        </svg>
      `,
      close: `
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M18.3 5.71 12 12l6.3 6.29-1.41 1.42L12 13.41l-6.29 6.3-1.42-1.41L10.59 12 4.29 5.71 5.7 4.29 12 10.59l6.29-6.3z"/>
        </svg>
      `,
    };

    const setCopyButtonState = (state) => {
      const nextState = state || "copy";
      const iconHtml = iconSvgMap[nextState] || iconSvgMap.copy;
      iconSpan.innerHTML = iconHtml;

      const titleMap = {
        copy: "复制",
        check: "已复制",
        close: "复制失败",
      };
      const title = titleMap[nextState] || "复制";
      copyButton.title = title;
      copyButton.setAttribute("aria-label", title);
    };

    // 创建图标元素（替代文案）
    const iconSpan = document.createElement("span");
    iconSpan.className = "copy-button-icon";
    iconSpan.style.cssText = `
      display: inline-flex;
      align-items: center;
      justify-content: center;
      line-height: 0;
    `;

    copyButton.appendChild(iconSpan);
    setCopyButtonState("copy");

    copyButton.style.cssText = `
      position: absolute;
      right: 12px;
      top: 12px;
      height: 24px;
      padding: 0 6px;
      border-radius: 6px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: ${isDark ? 'rgba(255, 255, 255, 0.85)' : 'rgba(0, 0, 0, 0.6)'};
      background: ${isDark ? 'rgba(60, 60, 60, 0.85)' : 'rgba(255, 255, 255, 0.85)'};
      border: 1px solid ${isDark ? 'rgba(255, 255, 255, 0.12)' : 'rgba(0, 0, 0, 0.12)'};
      cursor: pointer;
      z-index: 10;
      gap: 4px;
    `;

    // 添加按钮到锚点容器，避免与代码块横向滚动绑定
    anchor.appendChild(copyButton);

    copyButton.onclick = async (e) => {
      e.stopPropagation();

      if (copyButton.dataset.isCopying === "true") {
        return;
      }

      try {
        const code = codeBlock.querySelector("code")?.textContent || "";
        await import("@tauri-apps/plugin-clipboard-manager").then(
          ({ writeText }) => writeText(code)
        );

        // 保存原始状态
        copyButton.dataset.isCopying = "true";

        // 只修改图标状态
        setCopyButtonState("check");

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始状态
          setCopyButtonState("copy");
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      } catch (err) {
        logger.error("复制失败:", err);

        // 保存原始状态
        copyButton.dataset.isCopying = "true";

        // 只修改图标状态
        setCopyButtonState("close");

        // 清除之前的定时器（如果有）
        if (copyButton.dataset.timeoutId) {
          clearTimeout(parseInt(copyButton.dataset.timeoutId));
        }

        // 设置新的定时器
        const timeoutId = setTimeout(() => {
          // 恢复原始状态
          setCopyButtonState("copy");
          delete copyButton.dataset.isCopying;
          delete copyButton.dataset.timeoutId;
        }, 2000);

        copyButton.dataset.timeoutId = timeoutId.toString();
      }
    };
  });
};

export const addMediaViewerButtons = (container, options = {}) => {
  try {
    if (!(container instanceof HTMLElement)) return;

    container
      .querySelectorAll('.media-viewer-button, .media-save-button')
      .forEach((btn) => btn.remove());

    const theme = String(options?.theme || '').trim().toLowerCase();
    const fromDom =
      !!container?.closest?.('.dark') ||
      !!container?.closest?.('.dark-mode') ||
      !!document?.documentElement?.classList?.contains?.('dark') ||
      !!document?.documentElement?.classList?.contains?.('dark-mode');
    const isDark = theme === 'dark' || fromDom;

    container.dataset.mvTheme = theme;
    container.dataset.mvIncludeNotes = options?.includeNotes === true ? '1' : '0';

    if (container.dataset.mvObserver !== '1') {
      container.dataset.mvObserver = '1';
      try {
        const observer = new MutationObserver((mutations) => {
          const el = container;
          if (!(el instanceof HTMLElement)) return;
          if (el.dataset.mvScheduled === '1') return;

          let relevant = false;
          for (const m of mutations) {
            const nodes = [];
            if (m.addedNodes && m.addedNodes.length) nodes.push(...m.addedNodes);
            if (m.removedNodes && m.removedNodes.length) nodes.push(...m.removedNodes);

            for (const n of nodes) {
              if (!(n instanceof HTMLElement)) continue;
              const cls = n.classList;
              if (
                cls?.contains('media-viewer-button') ||
                cls?.contains('media-save-button') ||
                cls?.contains('media-viewer-wrap')
              ) {
                continue;
              }

              if (n.matches?.('img, .mermaid, svg') || n.querySelector?.('img, .mermaid, svg')) {
                relevant = true;
                break;
              }
            }
            if (relevant) break;
          }

          if (!relevant) return;

          el.dataset.mvScheduled = '1';
          requestAnimationFrame(() => {
            try {
              const t = String(el.dataset.mvTheme || '').trim();
              const includeNotes = el.dataset.mvIncludeNotes === '1';
              addMediaViewerButtons(el, { theme: t, includeNotes });
            } finally {
              delete el.dataset.mvScheduled;
            }
          });
        });

        observer.observe(container, { childList: true, subtree: true });
        container.__mvObserver = observer;
        logger.info('[MediaViewer] observer installed');
      } catch (e) {
        logger.warn('[MediaViewer] observer install failed:', e);
      }
    }

    const ICONS = {
      view: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 5c-7 0-11 7-11 7s4 7 11 7 11-7 11-7-4-7-11-7zm0 12a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/></svg>',
      save: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M5 20h14v-2H5v2zm7-18-5.5 5.5 1.42 1.42L11 6.84V16h2V6.84l3.08 3.08 1.42-1.42L12 2z"/></svg>',
      close: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M18.3 5.71 12 12l6.3 6.29-1.41 1.42L12 13.41l-6.29 6.3-1.42-1.41L10.59 12 4.29 5.71 5.7 4.29 12 10.59l6.29-6.3z"/></svg>',
      plus: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M19 11H13V5h-2v6H5v2h6v6h2v-6h6z"/></svg>',
      minus: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M19 13H5v-2h14v2z"/></svg>',
    };

    const makeBtn = (className, rightPx, title, iconSvg) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = className;
      btn.title = title;
      btn.setAttribute('aria-label', title);

      const icon = document.createElement('span');
      icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;line-height:0;';
      icon.innerHTML = iconSvg;
      btn.appendChild(icon);

      btn.style.cssText = `position:absolute;right:${rightPx}px;top:12px;height:24px;min-width:24px;padding:0 6px;border-radius:6px;display:inline-flex;align-items:center;justify-content:center;color:${isDark ? 'rgba(255,255,255,0.85)' : 'rgba(0,0,0,0.62)'};background:${isDark ? 'rgba(60,60,60,0.85)' : 'rgba(255,255,255,0.92)'};border:1px solid ${isDark ? 'rgba(255,255,255,0.12)' : 'rgba(0,0,0,0.12)'};cursor:pointer;z-index:10;opacity:0;transition:opacity 0.15s ease;`;
      btn.style.setProperty('opacity', '0', 'important');
      return btn;
    };

    const bindHover = (wrap) => {
      if (!wrap || wrap.dataset.mvBound === '1') return;
      wrap.dataset.mvBound = '1';
      const set = (v) => {
        wrap
          .querySelectorAll('.media-viewer-button, .media-save-button')
          .forEach((btn) => btn.style.setProperty('opacity', v ? '1' : '0', 'important'));
      };
      wrap.addEventListener('mouseenter', () => set(true));
      wrap.addEventListener('mouseleave', () => set(false));
    };

    const ensureImgWrap = (img) => {
      if (!(img instanceof HTMLImageElement)) return null;
      const p = img.parentElement;
      if (p && p.classList && p.classList.contains('media-viewer-wrap')) return p;
      const wrap = document.createElement('span');
      wrap.className = 'media-viewer-wrap';
      wrap.style.cssText = 'position:relative;display:inline-block;max-width:100%;';
      const parent = img.parentNode;
      if (!parent) return null;
      parent.insertBefore(wrap, img);
      wrap.appendChild(img);
      bindHover(wrap);
      return wrap;
    };

    const ensureMermaidWrap = (mermaidEl) => {
      if (!(mermaidEl instanceof HTMLElement)) return null;
      const p = mermaidEl.parentElement;
      if (p && p.classList && p.classList.contains('media-viewer-wrap')) return p;

      const wrap = document.createElement('div');
      wrap.className = 'media-viewer-wrap';
      wrap.style.cssText = 'position:relative;display:block;max-width:100%;';

      const parent = mermaidEl.parentNode;
      if (!parent) return null;
      parent.insertBefore(wrap, mermaidEl);
      wrap.appendChild(mermaidEl);
      bindHover(wrap);
      return wrap;
    };

    const resolveMermaidSvg = (mermaidEl) => {
      if (!(mermaidEl instanceof HTMLElement)) return null;
      const direct = mermaidEl.tagName?.toLowerCase() === 'svg' ? mermaidEl : null;
      if (direct) return direct;
      return mermaidEl.querySelector('svg');
    };

    const resolveSvgSize = (svg) => {
      try {
        // 设计说明（对应"导出的 PNG 边缘被裁掉"）：
        // 必须以 viewBox 为准。Mermaid 给根 svg 写的是 width="100%" + style="max-width:...px"，
        // parseFloat("100%") 会得到 100 这种毫无意义的值；而 getBoundingClientRect 拿到的是
        // 被预览容器宽度约束后的"显示尺寸"，比真实内容小，按它开画布就会把右侧/左侧内容切掉。
        // viewBox 才是图形的真实内容包围盒。
        const vbBase = svg?.viewBox?.baseVal;
        if (vbBase && vbBase.width > 0 && vbBase.height > 0) {
          return { width: vbBase.width, height: vbBase.height };
        }

        const vbAttr = String(svg?.getAttribute?.('viewBox') || '').trim();
        if (vbAttr) {
          const nums = vbAttr.split(/[\s,]+/).map((v) => Number.parseFloat(v)).filter((n) => Number.isFinite(n));
          if (nums.length === 4 && nums[2] > 0 && nums[3] > 0) return { width: nums[2], height: nums[3] };
        }

        // 没有 viewBox 时才看 width/height，且只认纯数字/px（排掉 100%、auto、em 等）。
        const parsePx = (raw) => {
          const s = String(raw || '').trim();
          if (!s || /%$/.test(s)) return NaN;
          if (!/^[0-9.]+(px)?$/i.test(s)) return NaN;
          return Number.parseFloat(s);
        };
        const wAttr = parsePx(svg?.getAttribute?.('width'));
        const hAttr = parsePx(svg?.getAttribute?.('height'));
        if (Number.isFinite(wAttr) && wAttr > 0 && Number.isFinite(hAttr) && hAttr > 0) {
          return { width: wAttr, height: hAttr };
        }

        try {
          const rect = svg?.getBoundingClientRect?.();
          if (rect && rect.width > 0 && rect.height > 0) return { width: rect.width, height: rect.height };
        } catch {
        }

        return { width: 800, height: 600 };
      } catch {
        return { width: 800, height: 600 };
      }
    };

    // 读出 viewBox 的四个值（含可能为负的 min-x / min-y）。
    const resolveViewBox = (svg, fallbackW, fallbackH) => {
      const vbBase = svg?.viewBox?.baseVal;
      if (vbBase && vbBase.width > 0 && vbBase.height > 0) {
        return { x: vbBase.x || 0, y: vbBase.y || 0, width: vbBase.width, height: vbBase.height };
      }
      const vbAttr = String(svg?.getAttribute?.('viewBox') || '').trim();
      if (vbAttr) {
        const nums = vbAttr.split(/[\s,]+/).map((v) => Number.parseFloat(v)).filter((n) => Number.isFinite(n));
        if (nums.length === 4 && nums[2] > 0 && nums[3] > 0) {
          return { x: nums[0], y: nums[1], width: nums[2], height: nums[3] };
        }
      }
      return { x: 0, y: 0, width: fallbackW, height: fallbackH };
    };

    const unionBox = (a, b) => {
      const x1 = Math.min(a.x, b.x);
      const y1 = Math.min(a.y, b.y);
      const x2 = Math.max(a.x + a.width, b.x + b.width);
      const y2 = Math.max(a.y + a.height, b.y + b.height);
      return { x: x1, y: y1, width: Math.max(1, x2 - x1), height: Math.max(1, y2 - y1) };
    };

    // flatten=true 时把 foreignObject 拍平成原生 <text>（导出/光栅化路径专用）；
    // 预览遮罩不需要拍平——它插进真实 DOM，浏览器会正常渲染 foreignObject。
    const buildExportSvg = (svg, { background = true, flatten = false } = {}) => {
      try {
        if (!(svg instanceof SVGElement)) return null;
        const { width, height } = resolveSvgSize(svg);
        const w = Math.max(1, Math.ceil(width));
        const h = Math.max(1, Math.ceil(height));
        const vb = resolveViewBox(svg, w, h);

        const cloned = svg.cloneNode(true);
        if (!(cloned instanceof SVGElement)) return null;

        let box = { x: vb.x, y: vb.y, width: vb.width, height: vb.height };
        if (flatten) {
          const measured = applyMeasuredTextFlatten(cloned, svg);
          if (measured.applied) {
            // 实测出来的文字外框要并进 viewBox，否则贴边或轻微出框的文字会被裁掉。
            if (measured.bbox) box = unionBox(box, measured.bbox);
          } else {
            flattenForeignObjects(cloned);
          }
        }

        // 留一点边距：描边和文字正好压在 viewBox 边界上时，光栅化会切掉半个像素到几个像素。
        const pad = flatten ? 8 : 0;
        const outVb = {
          x: box.x - pad,
          y: box.y - pad,
          width: box.width + pad * 2,
          height: box.height + pad * 2,
        };
        const outW = Math.max(1, Math.ceil(outVb.width));
        const outH = Math.max(1, Math.ceil(outVb.height));

        if (!cloned.getAttribute('xmlns')) cloned.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
        cloned.setAttribute('viewBox', `${outVb.x} ${outVb.y} ${outVb.width} ${outVb.height}`);
        // 导出成固定像素尺寸：把 mermaid 写的 width="100%" / max-width 一并覆盖掉，
        // 否则当图片渲染时会按 100% 去适配画布，出现缩放错位或裁剪。
        cloned.setAttribute('width', String(outW));
        cloned.setAttribute('height', String(outH));
        cloned.setAttribute('preserveAspectRatio', 'xMidYMid meet');
        cloned.style.removeProperty('max-width');
        cloned.style.removeProperty('width');
        cloned.style.removeProperty('height');

        if (background) {
          // 背景铺满 viewBox（含负偏移区域），不能用 x=0/100% —— viewBox 起点为负时会漏白。
          const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
          rect.setAttribute('x', String(outVb.x));
          rect.setAttribute('y', String(outVb.y));
          rect.setAttribute('width', String(outVb.width));
          rect.setAttribute('height', String(outVb.height));
          rect.setAttribute('fill', '#ffffff');
          cloned.insertBefore(rect, cloned.firstChild);
        }

        return { svg: cloned, width: outW, height: outH };
      } catch {
        return null;
      }
    };

    const openOverlay = (payload) => {
      try {
        const overlay = document.createElement('div');
        overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.55);z-index:99999;display:flex;align-items:center;justify-content:center;';

        const panel = document.createElement('div');
        panel.style.cssText = 'position:relative;max-width:95vw;max-height:92vh;background:rgba(255,255,255,0.98);border:1px solid rgba(0,0,0,0.12);border-radius:10px;overflow:hidden;';

        const toolbar = document.createElement('div');
        toolbar.style.cssText = 'position:absolute;right:8px;top:8px;display:flex;gap:6px;z-index:2;';

        const legend = document.createElement('div');
        legend.style.cssText = 'position:absolute;left:8px;top:8px;display:flex;gap:10px;align-items:center;padding:6px 8px;border-radius:8px;background:rgba(255,255,255,0.9);border:1px solid rgba(0,0,0,0.12);color:rgba(0,0,0,0.7);font-size:12px;line-height:1;z-index:2;';

        const mkLegendItem = (iconSvg, text) => {
          const item = document.createElement('span');
          item.style.cssText = 'display:inline-flex;align-items:center;gap:6px;';

          const icon = document.createElement('span');
          icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;line-height:0;';
          icon.innerHTML = iconSvg;

          const t = document.createElement('span');
          t.textContent = text;

          item.appendChild(icon);
          item.appendChild(t);
          return item;
        };

        const mkTool = (iconSvg, title) => {
          const b = document.createElement('button');
          b.type = 'button';
          b.title = title;
          b.setAttribute('aria-label', title);
          b.style.cssText = 'height:30px;min-width:30px;padding:0 8px;border-radius:8px;color:rgba(0,0,0,0.75);background:rgba(255,255,255,0.95);border:1px solid rgba(0,0,0,0.12);cursor:pointer;display:inline-flex;align-items:center;justify-content:center;line-height:0;';

          const icon = document.createElement('span');
          icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;line-height:0;';
          icon.innerHTML = iconSvg;
          b.appendChild(icon);

          return b;
        };

        const btnClose = mkTool(ICONS.close, '关闭 (Esc)');
        const btnPlus = mkTool(ICONS.plus, '放大（滚轮/按钮）');
        const btnMinus = mkTool(ICONS.minus, '缩小（滚轮/按钮）');
        const btnReset = mkTool('', '还原（双击画布也可以）');
        btnReset.textContent = '1:1';
        btnReset.style.fontSize = '12px';
        btnReset.style.lineHeight = '1';
        toolbar.appendChild(btnMinus);
        toolbar.appendChild(btnPlus);
        toolbar.appendChild(btnReset);
        toolbar.appendChild(btnClose);

        // 设计说明（对应"放大后无法拖动查看细节"）：
        // 原来只给 contentWrap 加 transform: scale()。transform 不参与布局，
        // stage 的 overflow:auto 根本不会因为放大而产生可滚动区域，于是放大后
        // 超出可视区的部分既滚不到、也拖不动。这里改成自己维护 translate + scale：
        // 平移量放在 scale 前面（screen 像素语义），滚轮按光标位置定点缩放，
        // 鼠标按住拖拽即平移，双击还原。
        const stage = document.createElement('div');
        stage.style.cssText = 'width:95vw;height:92vh;max-width:95vw;max-height:92vh;display:flex;align-items:center;justify-content:center;overflow:hidden;padding:40px 24px 24px;box-sizing:border-box;background:rgba(255,255,255,0.98);cursor:grab;touch-action:none;';

        const contentWrap = document.createElement('div');
        contentWrap.style.cssText = 'transform-origin:center center;will-change:transform;';
        stage.appendChild(contentWrap);

        const ZOOM_MIN = 0.1;
        const ZOOM_MAX = 8;
        let zoom = 1;
        let panX = 0;
        let panY = 0;

        const applyTransform = () => {
          contentWrap.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`;
        };

        // 定点缩放：保持 (anchorX, anchorY) 处的内容像素在屏幕上不动。
        // 推导：屏幕位置 = 未变换中心 C + pan + zoom * 局部偏移，
        // 其中 C = 当前变换后包围盒中心 - pan（缩放不移动中心）。
        const zoomAt = (nextZoom, anchorX, anchorY) => {
          const target = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, nextZoom));
          if (target === zoom) return;
          const rect = contentWrap.getBoundingClientRect();
          const centerX = rect.left + rect.width / 2 - panX;
          const centerY = rect.top + rect.height / 2 - panY;
          const dx = anchorX - centerX;
          const dy = anchorY - centerY;
          const ratio = target / zoom;
          panX = dx - ratio * (dx - panX);
          panY = dy - ratio * (dy - panY);
          zoom = target;
          applyTransform();
        };

        const zoomAtStageCenter = (nextZoom) => {
          const rect = stage.getBoundingClientRect();
          zoomAt(nextZoom, rect.left + rect.width / 2, rect.top + rect.height / 2);
        };

        const resetView = () => {
          zoom = 1;
          panX = 0;
          panY = 0;
          applyTransform();
        };

        if (payload?.kind === 'img') {
          const im = document.createElement('img');
          im.src = payload.src;
          im.alt = payload.alt || '';
          im.style.cssText = 'max-width:none;max-height:none;display:block;';
          contentWrap.appendChild(im);
        } else if (payload?.kind === 'svg') {
          const svg = payload.svg;
          const normalized = buildExportSvg(svg, { background: true });
          const cloned = normalized?.svg || svg.cloneNode(true);
          if (cloned && cloned instanceof SVGElement) {
            cloned.style.display = 'block';
            cloned.style.maxWidth = 'none';
            cloned.style.maxHeight = 'none';
            contentWrap.appendChild(cloned);
          }
        }

        // 拖拽平移。监听挂在 window 上，指针拖出画布甚至拖出窗口也不会中断。
        let dragging = false;
        let dragStartX = 0;
        let dragStartY = 0;
        let dragOriginX = 0;
        let dragOriginY = 0;
        let dragMoved = false;

        const onDragMove = (e) => {
          if (!dragging) return;
          const dx = e.clientX - dragStartX;
          const dy = e.clientY - dragStartY;
          if (!dragMoved && Math.abs(dx) + Math.abs(dy) > 2) dragMoved = true;
          panX = dragOriginX + dx;
          panY = dragOriginY + dy;
          applyTransform();
        };

        const endDrag = () => {
          if (!dragging) return;
          dragging = false;
          stage.style.cursor = 'grab';
          window.removeEventListener('mousemove', onDragMove, true);
          window.removeEventListener('mouseup', endDrag, true);
        };

        const close = () => {
          try {
            endDrag();
            document.removeEventListener('keydown', onKeyDown, true);
            overlay.remove();
          } catch (e) {
          }
        };

        const onKeyDown = (e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            close();
          }
        };

        btnClose.addEventListener('click', close);
        btnPlus.addEventListener('click', () => zoomAtStageCenter(zoom * 1.1));
        btnMinus.addEventListener('click', () => zoomAtStageCenter(zoom / 1.1));
        btnReset.addEventListener('click', resetView);

        overlay.addEventListener('click', (e) => {
          // 拖拽结束时鼠标可能正好落在遮罩上，这一下不该被当成"点空白关闭"。
          if (e.target === overlay && !dragMoved) close();
          dragMoved = false;
        });

        stage.addEventListener('mousedown', (e) => {
          if (e.button !== 0) return;
          e.preventDefault();
          dragging = true;
          dragMoved = false;
          dragStartX = e.clientX;
          dragStartY = e.clientY;
          dragOriginX = panX;
          dragOriginY = panY;
          stage.style.cursor = 'grabbing';
          window.addEventListener('mousemove', onDragMove, true);
          window.addEventListener('mouseup', endDrag, true);
        });

        stage.addEventListener('dblclick', (e) => {
          e.preventDefault();
          resetView();
        });

        stage.addEventListener(
          'wheel',
          (e) => {
            e.preventDefault();
            const delta = e.deltaY > 0 ? 0.9 : 1.1;
            zoomAt(zoom * delta, e.clientX, e.clientY);
          },
          { passive: false }
        );

        document.addEventListener('keydown', onKeyDown, true);

        // 左上角操作提示：之前 mkLegendItem 定义了却没被用过，图例框一直是空的。
        legend.appendChild(mkLegendItem(ICONS.plus, '滚轮缩放（按光标定点）'));
        legend.appendChild(mkLegendItem(ICONS.view, '按住拖动查看细节'));
        legend.appendChild(mkLegendItem(ICONS.close, '双击还原 · Esc 关闭'));

        panel.appendChild(toolbar);
        panel.appendChild(legend);
        panel.appendChild(stage);
        overlay.appendChild(panel);
        document.body.appendChild(overlay);
        applyTransform();
      } catch (e) {
        logger.error('[MediaViewer] 打开预览失败:', e);
      }
    };

    // 设计说明（对应"另存为弹了对话框但文件没保存"）：
    // fs 插件的写权限受 capabilities 里的 scope 限制 —— 当前只放开了 $TEMP 与 $APPDATA，
    // 而"另存为"选的是用户任意目录（桌面、D 盘…），writeFile 会被 ACL 直接拒掉。
    // 这里改走后端命令 write_file_bytes_base64：Rust 侧不受前端 scope 约束，
    // 还会自动补齐父目录。fs 插件只作为非 Tauri 环境（纯浏览器调试）的兜底。
    const writeBytesToPath = async (path, bytes) => {
      const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes || []);
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        // 大图分段转 base64，避免 String.fromCharCode 一次性展开几十万参数爆栈。
        let binary = '';
        const CHUNK = 0x8000;
        for (let i = 0; i < u8.length; i += CHUNK) {
          binary += String.fromCharCode.apply(null, u8.subarray(i, i + CHUNK));
        }
        await invoke('write_file_bytes_base64', { path, base64: btoa(binary) });
        logger.info('[MediaViewer] 已通过后端写入文件:', { path, bytes: u8.length });
        return true;
      } catch (e) {
        logger.warn('[MediaViewer] 后端写入失败，尝试 fs 插件兜底:', {
          path,
          error: String(e?.message || e || ''),
        });
      }
      const { writeFile } = await import('@tauri-apps/plugin-fs');
      await writeFile(path, u8);
      logger.info('[MediaViewer] 已通过 fs 插件写入文件:', { path, bytes: u8.length });
      return true;
    };

    const saveBlob = async (blob, defaultName, filters) => {
      try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        const path = await save({ defaultPath: defaultName, filters });
        if (!path) return;
        const buf = new Uint8Array(await blob.arrayBuffer());
        await writeBytesToPath(path, buf);
      } catch (e) {
        logger.error('[MediaViewer] 保存失败:', e);
        throw e;
      }
    };

    const SVG_NS = 'http://www.w3.org/2000/svg';

    // ───── 导出文件名 ─────
    // 原来 Mermaid 一律叫 mermaid.png、图片一律叫 image.png，同一篇文档里导出几张就互相覆盖。
    // 这里按"最近的上文标题 + 序号"起名，实在取不到标题才退回时间戳。
    const slugifyName = (raw) => {
      const s = String(raw || '')
        .replace(/\s+/g, ' ')
        .trim()
        .replace(/[\\/:*?"<>|]/g, '')     // 文件名非法字符
        .replace(/[\x00-\x1f\x7f]/g, '')
        .replace(/^\.+/, '')
        .replace(/\s/g, '-');
      return s.slice(0, 48);
    };

    const stampNow = () => {
      const d = new Date();
      const p = (n) => String(n).padStart(2, '0');
      return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
    };

    // 往前找最近的标题，作为这张图的语义名字。
    const findNearestHeadingText = (el) => {
      try {
        let node = el;
        while (node && node !== container) {
          let sib = node.previousElementSibling;
          while (sib) {
            if (/^H[1-6]$/.test(sib.tagName || '')) {
              const t = String(sib.textContent || '').trim();
              if (t) return t;
            }
            const nested = sib.querySelectorAll?.('h1,h2,h3,h4,h5,h6');
            if (nested && nested.length) {
              const t = String(nested[nested.length - 1].textContent || '').trim();
              if (t) return t;
            }
            sib = sib.previousElementSibling;
          }
          node = node.parentElement;
        }
      } catch {
      }
      return '';
    };

    const buildExportBaseName = (el, { kind = 'diagram', index = 0 } = {}) => {
      const heading = slugifyName(findNearestHeadingText(el));
      const seq = index > 0 ? `-${index}` : '';
      if (heading) return `${heading}-${kind}${seq}`;
      return `${kind}${seq}-${stampNow()}`;
    };

    const guessExt = (src) => {
      try {
        const u = String(src || '');
        const clean = u.split('#')[0].split('?')[0];
        const m = clean.match(/\.([a-zA-Z0-9]+)$/);
        const ext = (m && m[1] ? m[1] : '').toLowerCase();
        if (!ext) return 'png';
        if (ext === 'jpeg') return 'jpg';
        return ext;
      } catch {
        return 'png';
      }
    };

    // 把 foreignObject 里的 HTML 抽成"按行"的纯文本。
    // 关键点（对应"导出图里文字重叠成一行"）：Mermaid 的多行标签是
    // <p>第一行<br>第二行</p> 这种结构，直接取 textContent 会把几行首尾相连粘成一长串，
    // 于是既溢出方框、又和相邻文字重叠。这里遍历节点，遇到 <br> 和块级元素边界就断行。
    const extractForeignObjectLines = (fo) => {
      const lines = [];
      let current = '';
      const flush = () => {
        const t = current.replace(/\s+/g, ' ').trim();
        if (t) lines.push(t);
        current = '';
      };
      const walk = (node) => {
        if (!node) return;
        if (node.nodeType === 3) {
          current += node.textContent || '';
          return;
        }
        if (node.nodeType !== 1) return;
        const tag = String(node.tagName || '').toLowerCase();
        if (tag === 'br') {
          flush();
          return;
        }
        const isBlock = tag === 'p' || tag === 'div' || tag === 'li' || tag === 'tr' || tag === 'section';
        if (isBlock) flush();
        for (const child of Array.from(node.childNodes)) walk(child);
        if (isBlock) flush();
      };
      walk(fo);
      flush();
      return lines;
    };

    const FLATTEN_FONT_SIZE = 16;
    const FLATTEN_LINE_HEIGHT = 21;

    // Mermaid 的节点标签默认是 <foreignObject> 里的 HTML。把 SVG 当图片喂给 canvas 时，
    // 浏览器按严格 XML 解析、且不渲染 foreignObject —— 轻则标签全丢，重则直接 onerror，
    // 于是 PNG 导出一路失败、被静默降级成 SVG（就是"只能导出 svg"的原因）。
    // 这里在光栅化前把 foreignObject 拍平成等价的 <text>/<tspan>，让文字能画进 PNG。
    const flattenForeignObjects = (root) => {
      let converted = 0;
      try {
        const list = Array.from(root.querySelectorAll('foreignObject'));
        list.forEach((fo) => {
          const num = (name) => Number.parseFloat(fo.getAttribute(name) || '0') || 0;
          const x = num('x');
          const y = num('y');
          const w = num('width');
          const h = num('height');

          // 逐块取文本，尽量保留原来的换行结构。
          const lines = extractForeignObjectLines(fo);

          const text = document.createElementNS(SVG_NS, 'text');
          const cx = w > 0 ? x + w / 2 : x;
          const cy = h > 0 ? y + h / 2 : y;
          text.setAttribute('x', String(cx));
          text.setAttribute('y', String(cy));
          text.setAttribute('text-anchor', 'middle');
          text.setAttribute('dominant-baseline', 'central');
          text.setAttribute('font-size', String(FLATTEN_FONT_SIZE));
          text.setAttribute('font-family', '"Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", system-ui, sans-serif');
          text.setAttribute('fill', '#1f2328');

          const lineHeight = FLATTEN_LINE_HEIGHT;
          const startDy = -((Math.max(1, lines.length) - 1) * lineHeight) / 2;
          (lines.length ? lines : ['']).forEach((line, i) => {
            const tspan = document.createElementNS(SVG_NS, 'tspan');
            tspan.setAttribute('x', String(cx));
            tspan.setAttribute('dy', i === 0 ? String(startDy) : String(lineHeight));
            tspan.textContent = line;
            text.appendChild(tspan);
          });

          fo.parentNode?.replaceChild(text, fo);
          converted += 1;
        });
      } catch (e) {
        logger.warn('[MediaViewer] foreignObject 拍平失败（继续尝试光栅化）:', e);
      }
      return converted;
    };

    // ───── 按预览实测坐标拍平 foreignObject ─────
    // 设计说明（对应"导出 PNG 文字重叠、边界被裁剪"）：
    // 1. 上面 flattenForeignObjects 是按 DOM 结构猜断行（<br>、块级元素边界）。
    //    但 Mermaid 11 的长标签是靠 CSS 自动折行的：DOM 里就是一个文本节点，
    //    结构上看不出任何换行点，于是三行标签被拼成一行 —— 既溢出方框、和相邻
    //    边标签重叠，又把内容顶到 viewBox 之外被裁掉。这正是当前导出图的症状。
    // 2. 预览里的 foreignObject 已经由浏览器排好版，真实行盒是可以"量"出来的：
    //    用 Range.getClientRects() 逐字符测量 → 按行盒 top 分组，得到和预览
    //    完全一致的分行结果（含 CSS 自动折行）。
    // 3. 再用根 svg 的 getScreenCTM() 逆矩阵把屏幕坐标换算回根 svg 用户坐标，
    //    逐行生成 <text>，直接挂到根 svg 上（坐标已是根用户空间，无需再套 transform）。
    const measureTextNodeLines = (textNode) => {
      const raw = String(textNode?.nodeValue || '');
      if (!raw.trim()) return [];

      const range = document.createRange();
      const lines = [];
      let i = 0;

      while (i < raw.length) {
        const code = raw.charCodeAt(i);
        // 代理对要整对取，否则 emoji / 生僻字会被切碎。
        const step = code >= 0xd800 && code <= 0xdbff && i + 1 < raw.length ? 2 : 1;
        const ch = raw.slice(i, i + step);

        let rect = null;
        try {
          range.setStart(textNode, i);
          range.setEnd(textNode, i + step);
          const rects = Array.from(range.getClientRects() || []);
          for (const r of rects) {
            if (!rect || r.width * r.height > rect.width * rect.height) rect = r;
          }
        } catch {
          rect = null;
        }

        const last = lines[lines.length - 1];
        if (rect && rect.height > 0) {
          // 同一行内行盒 top 恒定；top 跳变即认为换行。
          if (last && Math.abs(last.top - rect.top) <= Math.max(1, rect.height * 0.5)) {
            last.text += ch;
            last.left = Math.min(last.left, rect.left);
            last.right = Math.max(last.right, rect.right);
            last.top = Math.min(last.top, rect.top);
            last.bottom = Math.max(last.bottom, rect.bottom);
          } else {
            lines.push({
              text: ch,
              left: rect.left,
              right: rect.right,
              top: rect.top,
              bottom: rect.bottom,
            });
          }
        } else if (last) {
          // 折行处被折叠掉的空白字符量不到矩形，并入当前行后统一 trim。
          last.text += ch;
        }

        i += step;
      }

      return lines
        .map((line) => ({ ...line, text: line.text.replace(/\s+/g, ' ').trim() }))
        .filter((line) => line.text);
    };

    const applyMeasuredTextFlatten = (clonedSvg, liveSvg) => {
      const empty = { applied: false, bbox: null, lines: 0 };
      try {
        if (!(clonedSvg instanceof SVGElement) || !(liveSvg instanceof SVGElement)) return empty;

        const liveForeignObjects = Array.from(liveSvg.querySelectorAll('foreignObject'));
        const clonedForeignObjects = Array.from(clonedSvg.querySelectorAll('foreignObject'));
        // 没有 foreignObject（例如已经是 htmlLabels:false 渲染的图）就不需要拍平。
        if (!liveForeignObjects.length) return { applied: clonedForeignObjects.length === 0, bbox: null, lines: 0 };

        const ctm = typeof liveSvg.getScreenCTM === 'function' ? liveSvg.getScreenCTM() : null;
        // 元素没挂到可见 DOM（或 display:none）时量不出坐标，交给结构猜测兜底。
        if (!ctm) return empty;
        const inverse = ctm.inverse();
        const makePoint = (x, y) => {
          if (typeof DOMPoint === 'function') return new DOMPoint(x, y);
          const p = liveSvg.createSVGPoint();
          p.x = x;
          p.y = y;
          return p;
        };
        const toUser = (x, y) => makePoint(x, y).matrixTransform(inverse);
        // 预览里 mermaid 常被 max-width 缩放过，字号要按同样比例换到用户单位。
        const unitPerPx = Math.hypot(inverse.a, inverse.b) || 1;

        // 预览里看不见的文本不能被拍平出来（mermaid 会留隐藏的测量/占位标签），
        // 否则导出图上会凭空多出一堆字。
        const isVisibleHost = (el) => {
          try {
            const cs = window.getComputedStyle(el);
            if (cs.display === 'none' || cs.visibility !== 'visible') return false;
            if (Number.parseFloat(cs.opacity || '1') <= 0.01) return false;
            return true;
          } catch {
            return true;
          }
        };

        const specs = [];
        for (const foreignObject of liveForeignObjects) {
          if (!(foreignObject.getClientRects?.().length > 0)) continue;

          const walker = document.createTreeWalker(foreignObject, NodeFilter.SHOW_TEXT, null);
          let node = walker.nextNode();
          while (node) {
            const host = node.parentElement;
            const lines = host && isVisibleHost(host) ? measureTextNodeLines(node) : [];
            if (lines.length) {
              const cs = window.getComputedStyle(host);
              const fontPx = Number.parseFloat(cs.fontSize) || FLATTEN_FONT_SIZE;
              const style = {
                fontSize: Math.max(1, fontPx * unitPerPx),
                fontFamily: cs.fontFamily || '"Microsoft YaHei", "PingFang SC", system-ui, sans-serif',
                fontWeight: String(cs.fontWeight || 'normal'),
                fontStyle: cs.fontStyle || 'normal',
                fill: cs.color || '#1f2328',
              };

              for (const line of lines) {
                const center = toUser((line.left + line.right) / 2, (line.top + line.bottom) / 2);
                const p1 = toUser(line.left, line.top);
                const p2 = toUser(line.right, line.bottom);
                specs.push({
                  ...style,
                  text: line.text,
                  cx: center.x,
                  cy: center.y,
                  x1: Math.min(p1.x, p2.x),
                  y1: Math.min(p1.y, p2.y),
                  x2: Math.max(p1.x, p2.x),
                  y2: Math.max(p1.y, p2.y),
                });
              }
            }
            node = walker.nextNode();
          }
        }

        if (!specs.length) return empty;

        clonedForeignObjects.forEach((foreignObject) => foreignObject.parentNode?.removeChild(foreignObject));

        let bbox = null;
        for (const spec of specs) {
          const text = document.createElementNS(SVG_NS, 'text');
          text.setAttribute('x', String(spec.cx));
          text.setAttribute('y', String(spec.cy));
          text.textContent = spec.text;
          // Mermaid 会往 svg 里注入 <style>，样式表优先级高于呈现属性，
          // 所以字体/颜色/对齐必须写成内联 important，否则导出时又被主题样式改掉。
          text.style.setProperty('font-size', `${spec.fontSize}px`, 'important');
          text.style.setProperty('font-family', spec.fontFamily, 'important');
          text.style.setProperty('font-weight', spec.fontWeight, 'important');
          text.style.setProperty('font-style', spec.fontStyle, 'important');
          text.style.setProperty('fill', spec.fill, 'important');
          text.style.setProperty('stroke', 'none', 'important');
          text.style.setProperty('text-anchor', 'middle', 'important');
          text.style.setProperty('dominant-baseline', 'central', 'important');
          text.style.setProperty('white-space', 'pre', 'important');
          clonedSvg.appendChild(text);

          const lineBox = {
            x: spec.x1,
            y: spec.y1,
            width: Math.max(0, spec.x2 - spec.x1),
            height: Math.max(0, spec.y2 - spec.y1),
          };
          bbox = bbox ? unionBox(bbox, lineBox) : lineBox;
        }

        logger.info('[MediaViewer] foreignObject 已按预览实测坐标拍平:', {
          foreignObjects: liveForeignObjects.length,
          lines: specs.length,
        });
        return { applied: true, bbox, lines: specs.length };
      } catch (e) {
        logger.warn('[MediaViewer] 实测拍平失败，回退按结构猜测拍平:', e);
        return empty;
      }
    };

    // 序列化成能被 <img> 当作严格 XML 解析的 SVG 文本。
    const serializeSvgForRaster = (svgEl) => {
      const xml = new XMLSerializer().serializeToString(svgEl);
      let text = /xmlns=/.test(xml)
        ? xml
        : xml.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"');
      // 裸 & 会让 XML 解析直接失败（foreignObject 里的 &nbsp; 之类最常见）。
      text = text.replace(/&(?!#\d+;|#x[0-9a-fA-F]+;|amp;|lt;|gt;|quot;|apos;)/g, '&amp;');
      return text;
    };

    const loadImage = (src) => new Promise((resolve, reject) => {
      const img = new Image();
      img.decoding = 'async';
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error('svg image load failed'));
      img.src = src;
    });

    const svgToPngBlob = async (svg, { scale = 2 } = {}) => {
      const normalized = buildExportSvg(svg, { background: true, flatten: true });
      const exportSvg = normalized?.svg || svg.cloneNode(true);
      const exportW = normalized?.width || 800;
      const exportH = normalized?.height || 600;

      const svgText = serializeSvgForRaster(exportSvg);

      // data: URL 比 blob: 在"SVG 当图片"这条路上更稳（不受 blob 生命周期与部分 CSP 影响）。
      const dataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgText)}`;
      let img = null;
      let revoke = null;
      try {
        img = await loadImage(dataUrl);
      } catch (e) {
        logger.warn('[MediaViewer] data: URL 光栅化失败，回退 blob: URL:', e);
        const blobUrl = URL.createObjectURL(new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' }));
        revoke = () => {
          try {
            URL.revokeObjectURL(blobUrl);
          } catch {
          }
        };
        img = await loadImage(blobUrl);
      }

      try {
        const ratio = Math.max(1, Math.min(4, Number(scale) || 1));
        const w = Math.max(1, Math.floor(exportW));
        const h = Math.max(1, Math.floor(exportH));

        const canvas = document.createElement('canvas');
        canvas.width = Math.max(1, Math.round(w * ratio));
        canvas.height = Math.max(1, Math.round(h * ratio));
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('canvas ctx is null');
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);

        const blob = await new Promise((resolve, reject) => {
          try {
            canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('toBlob returns null'))), 'image/png');
          } catch (e) {
            reject(e);
          }
        });
        logger.info('[MediaViewer] SVG 已光栅化为 PNG:', {
          width: canvas.width,
          height: canvas.height,
          bytes: blob.size,
        });
        return blob;
      } finally {
        revoke?.();
      }
    };

    // 把预览里各种形态的图片 URL 还原成磁盘绝对路径。
    // release 下 CSP 的 connect-src 不含 asset 协议，fetch 这类地址会被拦，
    // 只能落回"解析出真实路径 → 后端读字节"。
    const resolveLocalPathFromSrc = (src) => {
      const raw = String(src || '');
      if (!raw) return '';
      const patterns = [
        /^https?:\/\/localhost:8888\/markdown\/api\/images\/(.+)$/i,
        /^https?:\/\/asset\.localhost\/(.+)$/i,
        /^asset:\/\/localhost\/(.+)$/i,
        /^https?:\/\/tauri\.localhost\/asset\/(.+)$/i,
      ];
      let encoded = '';
      for (const re of patterns) {
        const m = raw.match(re);
        if (m && m[1]) {
          encoded = m[1];
          break;
        }
      }
      if (!encoded && /^file:\/\//i.test(raw)) {
        encoded = raw.replace(/^file:\/\/\/?/i, '');
      }
      if (!encoded) return '';

      let filePath = encoded.split('#')[0].split('?')[0];
      try {
        filePath = decodeURIComponent(filePath);
      } catch {
        /* 已经是明文路径就直接用 */
      }
      // Windows 盘符路径统一回反斜杠，交给后端 normalize。
      if (/^[a-zA-Z]:\//.test(filePath)) filePath = filePath.replace(/\//g, '\\');
      return filePath;
    };

    // 读文件同样优先走后端：fs 插件的 read scope 也只放开了 $TEMP / $APPDATA。
    const readBytesFromPath = async (filePath) => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const base64 = await invoke('read_file_bytes_base64', { path: filePath });
        const bin = atob(String(base64 || ''));
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        return bytes;
      } catch (e) {
        logger.warn('[MediaViewer] 后端读取失败，尝试 fs 插件兜底:', {
          filePath,
          error: String(e?.message || e || ''),
        });
      }
      try {
        const { readFile } = await import('@tauri-apps/plugin-fs');
        return await readFile(filePath);
      } catch (e) {
        logger.warn('[MediaViewer] fs 插件读取也失败:', {
          filePath,
          error: String(e?.message || e || ''),
        });
        return null;
      }
    };

    const saveImage = async (img, { index = 0 } = {}) => {
      try {
        const src = String(img?.currentSrc || img?.src || '');
        if (!src) return;

        const ext = guessExt(src);
        let blob = null;

        if (src.startsWith('data:')) {
          const comma = src.indexOf(',');
          if (comma > 0) {
            const meta = src.slice(0, comma);
            const data = src.slice(comma + 1);
            const isBase64 = /;base64/i.test(meta);
            const mime = (meta.match(/^data:([^;]+)/i)?.[1] || '').trim();

            try {
              if (isBase64) {
                const bin = atob(data);
                const bytes = new Uint8Array(bin.length);
                for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
                blob = new Blob([bytes], { type: mime || `image/${ext}` });
              } else {
                const decoded = decodeURIComponent(data);
                blob = new Blob([decoded], { type: mime || `image/${ext}` });
              }
              logger.info('[MediaViewer] 保存图片: 使用 dataURL');
            } catch (e) {
              logger.warn('[MediaViewer] dataURL 转换失败:', e);
            }
          }
        }

        if (!blob) {
          try {
            const resp = await fetch(src);
            if (!resp.ok) throw new Error(`fetch failed: ${resp.status}`);
            blob = await resp.blob();
          } catch (e) {
            logger.warn('[MediaViewer] fetch 图片失败，尝试本地路径兜底:', e);
          }
        }

        if (!blob) {
          const filePath = resolveLocalPathFromSrc(src);
          if (filePath) {
            const bytes = await readBytesFromPath(filePath);
            if (bytes) {
              blob = new Blob([bytes], { type: `image/${ext === 'jpg' ? 'jpeg' : ext}` });
              logger.info('[MediaViewer] 保存图片: 从本地路径读取:', { filePath, bytes: bytes.length });
            }
          }
        }

        if (!blob) throw new Error('image blob is null');

        // 命名优先用原文件名（最贴近用户预期），拿不到再按"标题 + 序号"生成。
        const localPath = resolveLocalPathFromSrc(src);
        const originalName = String(localPath || src.split('#')[0].split('?')[0] || '')
          .split(/[\\/]/)
          .pop() || '';
        const originalBase = slugifyName(originalName.replace(/\.[a-zA-Z0-9]+$/, ''));
        const baseName = originalBase || buildExportBaseName(img, { kind: 'image', index });

        await saveBlob(blob, `${baseName}.${ext}`, [{ name: 'Image', extensions: [ext] }]);
      } catch (e) {
        logger.error('[MediaViewer] 保存图片失败:', e);
      }
    };

    const saveMermaid = async (mermaidEl, { index = 0 } = {}) => {
      try {
        const svg = resolveMermaidSvg(mermaidEl);
        if (!svg) {
          logger.warn('[MediaViewer] 未找到 Mermaid SVG');
          return;
        }

        // 注意：SVG 也走 flatten。拍平后的 <text> 是按预览实测坐标生成的，
        // 内容与预览一致，同时不再依赖"渲染器是否支持 foreignObject"，
        // 保证导出的 PNG 和 SVG 两种格式看到的是同一张图。
        const normalized = buildExportSvg(svg, { background: true, flatten: true });
        const exportSvg = normalized?.svg || svg.cloneNode(true);
        const svgText = serializeSvgForRaster(exportSvg);
        const svgBlob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' });

        const baseName = buildExportBaseName(mermaidEl, { kind: 'diagram', index });

        const { save } = await import('@tauri-apps/plugin-dialog');
        // 两种格式都放在 filters 里：用户在对话框切换类型即可，扩展名跟着变。
        const path = await save({
          defaultPath: `${baseName}.png`,
          filters: [
            { name: 'PNG 图片', extensions: ['png'] },
            { name: 'SVG 矢量图', extensions: ['svg'] }
          ]
        });
        if (!path) return;

        const lower = String(path).toLowerCase();
        const wantSvg = lower.endsWith('.svg');

        if (wantSvg) {
          const buf = new Uint8Array(await svgBlob.arrayBuffer());
          await writeBytesToPath(path, buf);
          logger.info('[MediaViewer] Mermaid 已导出 SVG:', { path, bytes: buf.length });
          return;
        }

        try {
          const pngBlob = await svgToPngBlob(svg, { scale: 2 });
          const buf = new Uint8Array(await pngBlob.arrayBuffer());
          await writeBytesToPath(path, buf);
          logger.info('[MediaViewer] Mermaid 已导出 PNG:', { path, bytes: buf.length });
        } catch (e) {
          // 光栅化仍然失败时退回 SVG，但必须把真实落盘路径写清楚，
          // 否则用户按 .png 去找会以为"根本没保存"。
          logger.warn('[MediaViewer] Mermaid 转 PNG 失败，回退保存 SVG:', e);
          const fallbackPath = String(path).replace(/\.png$/i, '.svg');
          const buf = new Uint8Array(await svgBlob.arrayBuffer());
          await writeBytesToPath(fallbackPath, buf);
          logger.info('[MediaViewer] Mermaid 已回退保存为 SVG:', { fallbackPath });
        }
      } catch (e) {
        logger.error('[MediaViewer] 保存 Mermaid 失败:', e);
      }
    };

    const addBtnsToWrap = (wrap, handlers, titles = {}) => {
      if (!(wrap instanceof HTMLElement)) return;
      if (!wrap.style.position) wrap.style.position = 'relative';

      const viewBtn = makeBtn('media-viewer-button', 12, titles?.viewTitle || '预览（打开查看器）', ICONS.view);
      const saveBtn = makeBtn('media-save-button', 60, titles?.saveTitle || '保存到文件', ICONS.save);

      viewBtn.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        handlers?.onView?.();
      });

      saveBtn.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        handlers?.onSave?.();
      });

      wrap.appendChild(saveBtn);
      wrap.appendChild(viewBtn);
      bindHover(wrap);
    };

    const shouldSkipForNotes = (el) => {
      try {
        if (options?.includeNotes === true) return false;
        return !!el?.closest?.('.notes-panel');
      } catch {
        return false;
      }
    };

    container.querySelectorAll('img').forEach((img, imgIndex) => {
      if (shouldSkipForNotes(img)) return;
      const wrap = ensureImgWrap(img);
      if (!wrap) return;
      addBtnsToWrap(
        wrap,
        {
          onView: () => openOverlay({ kind: 'img', src: img.currentSrc || img.src, alt: img.alt }),
          onSave: () => saveImage(img, { index: imgIndex + 1 }),
        },
        {
          viewTitle: '预览图片（打开查看器）',
          saveTitle: '保存图片到文件',
        }
      );
    });

    container.querySelectorAll('.mermaid').forEach((mermaidEl, mermaidIndex) => {
      if (!(mermaidEl instanceof HTMLElement)) return;
      if (shouldSkipForNotes(mermaidEl)) return;

      const wrap = ensureMermaidWrap(mermaidEl);
      if (!wrap) return;

      addBtnsToWrap(
        wrap,
        {
          onView: () => {
            const svg = resolveMermaidSvg(mermaidEl);
            if (!svg) {
              logger.warn('[MediaViewer] 未找到 Mermaid SVG');
              return;
            }
            openOverlay({ kind: 'svg', svg });
          },
          onSave: () => saveMermaid(mermaidEl, { index: mermaidIndex + 1 }),
        },
        {
          viewTitle: '预览图表（打开查看器）',
          saveTitle: '保存图表到文件（PNG/SVG）',
        }
      );
    });
  } catch (e) {
    logger.error('[MediaViewer] addMediaViewerButtons 失败:', e);
  }
};

// 辅助函数：转义正则表达式特殊字符
function escapeRegExp(string) {
  return string.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// 流式输出场景下，数学公式经常会被拆成多个 token 分段返回。
// 如果在公式还没闭合时立刻执行数学渲染，
// 就可能出现“前半段被渲染、后半段仍显示源码”的混合状态。
// 这里做一个轻量级检测：只要发现当前文本里存在明显未闭合的数学定界符，
// 本轮先跳过数学渲染，等后续 token 补齐后再恢复正常渲染，
// 这样视觉上会稳定很多。
const stripCodeSegmentsForMathCheck = (content) => {
  const text = String(content || "");
  return text
    .replace(/```[\s\S]*?```/g, "\n")
    .replace(/`[^`\n]*`/g, " ");
};

const countRegexMatches = (content, regex) => {
  const matches = String(content || "").match(regex);
  return Array.isArray(matches) ? matches.length : 0;
};

// 某些模型会把数学公式写成 "**\\vec{E}**"、"`\\oint`" 这种
// “Markdown 装饰 + LaTeX” 混合形式。对普通文本无害，但会让 MathJax 把公式视为非法输入。
// 这里仅在数学定界符内部做最小清洗，不动正文中的 Markdown 标记。
const sanitizeMathBody = (mathBody) => {
  let normalized = String(mathBody || "");
  normalized = normalized.replace(/`+/g, "");
  normalized = normalized.replace(/\*\*([\s\S]*?)\*\*/g, "$1");
  normalized = normalized.replace(/__([\s\S]*?)__/g, "$1");
  normalized = normalized.replace(/\*([\s\S]*?)\*/g, "$1");
  normalized = normalized.replace(/_([\s\S]*?)_/g, "$1");
  normalized = normalized.replace(/~~([\s\S]*?)~~/g, "$1");
  return normalized;
};

const normalizeMathSegmentsForRender = (content) => {
  let normalizedContent = String(content || "");

  normalizedContent = normalizedContent.replace(/\$\$([\s\S]*?)\$\$/g, (match, body) => {
    const cleanedBody = sanitizeMathBody(body);
    return `$$${cleanedBody}$$`;
  });

  normalizedContent = normalizedContent.replace(/(^|[^\\$])\$(?!\$)([\s\S]*?)(^|[^\\$])\$(?!\$)/gm, (match, prefix, body, suffix) => {
    const cleanedBody = sanitizeMathBody(body);
    return `${prefix}$${cleanedBody}${suffix}$`;
  });

  normalizedContent = normalizedContent.replace(/\\\(([\s\S]*?)\\\)/g, (match, body) => {
    const cleanedBody = sanitizeMathBody(body);
    return `\\(${cleanedBody}\\)`;
  });

  normalizedContent = normalizedContent.replace(/\\\[([\s\S]*?)\\\]/g, (match, body) => {
    const cleanedBody = sanitizeMathBody(body);
    return `\\[${cleanedBody}\\]`;
  });

  return normalizedContent;
};

const normalizeLatexMathEnvironments = (content) => {
  let normalizedContent = String(content || "");

  const blockEnvNames = [
    "equation",
    "equation*",
    "align",
    "align*",
    "aligned",
    "gather",
    "gather*",
    "multline",
    "multline*",
  ];

  blockEnvNames.forEach((envName) => {
    const escapedEnvName = escapeRegExp(envName);
    const envRegex = new RegExp(
      `\\\\begin\\{${escapedEnvName}\\}([\\s\\S]*?)\\\\end\\{${escapedEnvName}\\}`,
      "g"
    );

    normalizedContent = normalizedContent.replace(envRegex, (match, body) => {
      const cleanedBody = sanitizeMathBody(body).trim();
      return `$$\n${cleanedBody}\n$$`;
    });
  });

  return normalizedContent;
};

// 设计思路：
// 一些模型会把参数说明写成 Markdown 行内代码，例如 `\Phi_E`、`I_{enc}`、`\mu_0`。
// 这类内容会被 markdown-it 当作 code span，数学渲染器不会接管，所以最终只显示 LaTeX 源码。
// 这里在进入 Markdown 渲染前，把“明显是数学符号”的 code span 轻量转换为行内数学，
// 但尽量不碰真正的代码片段。
const looksLikeInlineMathCode = (codeContent) => {
  const text = String(codeContent || "").trim();
  if (!text) return false;
  if (/\r|\n/.test(text)) return false;
  if (text.length > 48) return false;
  if (/[*[\]{}<>]/.test(text.replace(/\\[{}]/g, ""))) return false;
  if (/[=;]/.test(text)) return false;
  if (/\s{2,}/.test(text)) return false;

  const hasLatexCommand = /\\[A-Za-z]+/.test(text);
  const hasSubOrSup = /(^|[A-Za-z0-9\\])[ _]?\{?[^}]*\}?/.test(text) && (/_|\^/.test(text));
  const isSimpleMathIdentifier = /^[A-Za-z](?:_[A-Za-z0-9]+|\^\{?[A-Za-z0-9+\-]+\}?|_[{][^}]+[}]|\^\{[^}]+\})*$/.test(text);
  const isParenWrappedMathIdentifier = /^\([A-Za-z]\)$/.test(text);
  const isUnitLikeMath = /^[A-Za-z](?:_[A-Za-z0-9]+|\^\{?[A-Za-z0-9+\-]+\}?)?$/.test(text);

  return hasLatexCommand || hasSubOrSup || isSimpleMathIdentifier || isParenWrappedMathIdentifier || isUnitLikeMath;
};

const normalizeInlineMathCodeSpans = (content) => {
  return String(content || "").replace(/`([^`\r\n]+)`/g, (match, codeContent) => {
    const trimmed = String(codeContent || "").trim();
    if (!looksLikeInlineMathCode(trimmed)) {
      return match;
    }

    const cleaned = sanitizeMathBody(trimmed);
    return `\\(${cleaned}\\)`;
  });
};

export const detectIncompleteMathSyntax = (content) => {
  const sanitized = stripCodeSegmentsForMathCheck(content);
  const reasons = [];

  if (!sanitized.trim()) {
    return {
      hasIncompleteMath: false,
      reasons,
    };
  }

  const blockDollarCount = countRegexMatches(sanitized, /(^|[^\\])\$\$(?!\$)/gm);
  if (blockDollarCount % 2 !== 0) {
    reasons.push("odd-double-dollar");
  }

  const inlineDollarCount = countRegexMatches(sanitized, /(^|[^\\$])\$(?!\$)/gm);
  if (inlineDollarCount % 2 !== 0) {
    reasons.push("odd-inline-dollar");
  }

  const squareOpenCount = countRegexMatches(sanitized, /\\\[/g);
  const squareCloseCount = countRegexMatches(sanitized, /\\\]/g);
  if (squareOpenCount !== squareCloseCount) {
    reasons.push("unbalanced-square-bracket-math");
  }

  const roundOpenCount = countRegexMatches(sanitized, /\\\(/g);
  const roundCloseCount = countRegexMatches(sanitized, /\\\)/g);
  if (roundOpenCount !== roundCloseCount) {
    reasons.push("unbalanced-round-bracket-math");
  }

  const beginEnvCounter = new Map();
  const endEnvCounter = new Map();
  Array.from(sanitized.matchAll(/\\begin\{([^}]+)\}/g)).forEach((match) => {
    const envName = String(match?.[1] || "").trim();
    if (!envName) return;
    beginEnvCounter.set(envName, (beginEnvCounter.get(envName) || 0) + 1);
  });
  Array.from(sanitized.matchAll(/\\end\{([^}]+)\}/g)).forEach((match) => {
    const envName = String(match?.[1] || "").trim();
    if (!envName) return;
    endEnvCounter.set(envName, (endEnvCounter.get(envName) || 0) + 1);
  });

  const allEnvNames = new Set([...beginEnvCounter.keys(), ...endEnvCounter.keys()]);
  for (const envName of allEnvNames) {
    if ((beginEnvCounter.get(envName) || 0) !== (endEnvCounter.get(envName) || 0)) {
      reasons.push(`unbalanced-environment:${envName}`);
    }
  }

  return {
    hasIncompleteMath: reasons.length > 0,
    reasons,
  };
};

// interface ImageMapping {
//   original_path: string;
//   served_path: string;
// }

export const renderMarkdown = async (content, options) => {
  // logger.info("原始内容:", content);
  const {
    preserveOriginal = false, // 新增参数，控制是否保留原始内容
    mermaid: enableMermaid = true,
    math: enableMath = true,
    codeHighlight: enableCodeHighlight = true,
    buttons: enableButtons = true,
    pythonCells: enablePythonCells = false,
    codeCells: enableCodeCells = false,
    chatPreprocess = false,
    theme = "default",
    deferMathUntilComplete = false,
    smartInlineMathFromCode = false,
  } = options;

  try {
    let finalContent = content;
    let mappings = [];
    let shouldEnableMath = enableMath;

    if (enableMath && deferMathUntilComplete) {
      const mathState = detectIncompleteMathSyntax(content);
      if (mathState.hasIncompleteMath) {
        shouldEnableMath = false;
        logger.info("[MarkdownRender] defer math render for streaming chunk", {
          reasons: mathState.reasons,
          preview: String(content || "").slice(-160),
        });
      }
    }

    if (shouldEnableMath) {
      const normalizedMathEnvironmentContent = normalizeLatexMathEnvironments(finalContent);
      if (normalizedMathEnvironmentContent !== finalContent) {
        logger.info("[MarkdownRender] normalize latex math environments", {
          beforeLength: String(finalContent || "").length,
          afterLength: normalizedMathEnvironmentContent.length,
        });
        finalContent = normalizedMathEnvironmentContent;
      }

      const normalizedMathContent = normalizeMathSegmentsForRender(finalContent);
      if (normalizedMathContent !== finalContent) {
        logger.info("[MarkdownRender] sanitize markdown markers inside math", {
          beforeLength: String(finalContent || "").length,
          afterLength: normalizedMathContent.length,
        });
        finalContent = normalizedMathContent;
      }
    }

    if (shouldEnableMath && smartInlineMathFromCode) {
      const normalizedInlineMathCodeContent = normalizeInlineMathCodeSpans(finalContent);
      if (normalizedInlineMathCodeContent !== finalContent) {
        logger.info("[MarkdownRender] normalize inline math-like code spans", {
          beforeLength: String(finalContent || "").length,
          afterLength: normalizedInlineMathCodeContent.length,
        });
        finalContent = normalizedInlineMathCodeContent;
      }
    }

    // 调用后端处理图片路径（接口不可用时，不影响后续 Markdown 本地渲染）
    try {
      const preprocessApi = chatPreprocess
        ? "http://localhost:8888/api/markdown/chat_process"
        : "http://localhost:8888/api/markdown/image_process";
      const response = await fetch(
        preprocessApi,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            content,
            base_path: options.basePath,
          }),
        }
      );

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();
      finalContent = result.processed_content;
      mappings = result.mappings || [];

      // 根据mappings替换图片路径
      mappings.forEach((mapping) => {
        finalContent = finalContent.replace(
          new RegExp(`\\]\\(${escapeRegExp(mapping.original_path)}\\)`, "g"),
          `](${mapping.served_path})`
        );
      });
    } catch (err) {
      logger.warn("[MarkdownRender] 图片路径处理失败，继续本地渲染:", err);
    }

    // logger.info("result:", result);
    // logger.info("处理后的内容:", finalContent);

    // 初始化各组件
    if (enableMermaid) {
      const fromDom =
        !!document?.documentElement?.classList?.contains?.("dark") ||
        !!document?.documentElement?.classList?.contains?.("dark-mode");
      const isDark = String(theme || "").trim().toLowerCase() === "dark" || fromDom;
      const keepMermaidLight = true;

      mermaid.initialize({
        startOnLoad: false,
        theme: keepMermaidLight ? "default" : isDark ? "dark" : "default",
        securityLevel: "loose",
        ...(keepMermaidLight
          ? {
            themeVariables: {
              background: "#ffffff",
            },
          }
          : {}),
      });
    }

    if (shouldEnableMath) {
      // logger.info("初始化MathJax");
      initMathJax();
    }

    // 配置Markdown-it
    const md = new MarkdownIt({
      html: true,
      linkify: true,
      typographer: true,
      breaks: false, // 禁用单换行转换为 <br>
      xhtmlOut: true, // 使用XHTML兼容的闭合标签
      highlight: enableCodeHighlight
        ? (code, lang) => {
          if (lang && hljs.getLanguage(lang)) {
            try {
              return `<pre class="hljs"><code class="language-${lang}">${hljs.highlight(code, { language: lang }).value
                }</code></pre>`;
            } catch (err) {
              logger.warn(`代码高亮错误 (${lang}):`, err);
            }
          }
          // Keep unlabeled fenced code blocks on the same structural path as highlighted blocks.
          // That preserves the shared copy-button enhancement and lets dark mode reuse the same code-block shell.
          return renderPlainFenceBlock(code, md);
        }
        : undefined,
    });

    // 放行 [label](file:///...) 这类本地文件链接（markdown-it 默认把 file: 拦掉）。
    allowFileProtocolLinks(md);

    // 段落渲染规则
    const defaultParagraphOpen =
      md.renderer.rules.paragraph_open || ((tokens, idx) => "<p>");
    const defaultParagraphClose =
      md.renderer.rules.paragraph_close || ((tokens, idx) => "</p>");

    md.renderer.rules.paragraph_open = (tokens, idx) => {
      const token = tokens[idx];
      const attrs = ['class="md-paragraph"'];

      if (token && Array.isArray(token.map)) {
        const startLine = token.map[0] + 1;
        const endLine = token.map[1] + 1;
        attrs.push(`data-md-start-line="${startLine}"`);
        attrs.push(`data-md-end-line="${endLine}"`);

        if (preserveOriginal) {
          const originalLines = content.split(/\r?\n|\r/);
          const original = originalLines[token.map[0]] || "";
          attrs.push(`data-md-original="${safeEncode(original)}"`);
        }
      }

      return `<p ${attrs.join(" ")}>`;
    };

    md.renderer.rules.paragraph_close = (tokens, idx) => {
      return defaultParagraphClose(tokens, idx);
    };

    // 标题渲染规则：为标题添加源码行号，便于编辑/预览联动
    const defaultHeadingOpen =
      md.renderer.rules.heading_open ||
      ((tokens, idx) => `<${tokens[idx].tag}>`);
    const defaultHeadingClose =
      md.renderer.rules.heading_close ||
      ((tokens, idx) => `</${tokens[idx].tag}>`);

    md.renderer.rules.heading_open = (tokens, idx, options, env, self) => {
      const token = tokens[idx];
      const tag = token.tag || "h1";
      const attrs = [`class="md-heading md-heading-${tag}"`];

      const headingId = token?.meta?.guaxiHeadingId;
      if (headingId) {
        attrs.push(`id="${md.utils.escapeHtml(String(headingId))}"`);
      }

      if (token && Array.isArray(token.map)) {
        const startLine = token.map[0] + 1;
        const endLine = token.map[1] + 1;
        attrs.push(`data-md-start-line="${startLine}"`);
        attrs.push(`data-md-end-line="${endLine}"`);
      }

      return `<${tag} ${attrs.join(" ")}>`;
    };

    md.renderer.rules.heading_close = (tokens, idx, options, env, self) => {
      return defaultHeadingClose(tokens, idx, options, env, self);
    };

    // ==================== GFM 任务列表（Task List）支持 ====================
    // 支持语法：
    // - [ ] 未完成
    // - [x] 已完成
    // 说明：
    // - 这里不引入第三方 markdown-it-task-lists 依赖，而是通过 core ruler 直接改写 token。
    // - 渲染结果包含：
    //   - <li class="task-list-item ..." data-md-task-line="..."> ...
    //   - <input class="task-list-item-checkbox" type="checkbox" ...>
    //   - <span class="task-list-item-text">...</span> 便于 CSS 做删除线
    const installTaskListSupport = () => {
      const taskPrefixRe = /^\s*\[( |x|X)\]\s+/;

      const addTokenClass = (token, cls) => {
        if (!token) return;
        const prev = token.attrGet("class") || "";
        const parts = String(prev).split(/\s+/).filter(Boolean);
        if (!parts.includes(cls)) parts.push(cls);
        token.attrSet("class", parts.join(" "));
      };

      const stripPrefixFromInlineChildren = (inlineToken, prefixLen) => {
        if (!inlineToken || !Array.isArray(inlineToken.children) || prefixLen <= 0) return;
        let remain = prefixLen;
        for (let i = 0; i < inlineToken.children.length && remain > 0;) {
          const c = inlineToken.children[i];
          if (!c || c.type !== "text") break;

          const txt = String(c.content || "");
          if (!txt) {
            i += 1;
            continue;
          }

          if (txt.length <= remain) {
            remain -= txt.length;
            inlineToken.children.splice(i, 1);
            continue;
          }

          c.content = txt.slice(remain);
          remain = 0;
          break;
        }
      };

      md.core.ruler.after("inline", "guaxi_task_list", (state) => {
        const tokens = state.tokens || [];
        let hit = 0;

        for (let i = 0; i < tokens.length; i += 1) {
          const t = tokens[i];
          if (!t || t.type !== "list_item_open") continue;

          const startLine = Array.isArray(t.map) ? t.map[0] + 1 : 0;

          // 找到当前 list item 内的第一个 inline token
          let inline = null;
          for (let j = i + 1; j < tokens.length && tokens[j].type !== "list_item_close"; j += 1) {
            if (tokens[j].type === "inline") {
              inline = tokens[j];
              break;
            }
          }
          if (!inline) continue;

          const m = taskPrefixRe.exec(String(inline.content || ""));
          if (!m) continue;

          const checked = String(m[1] || "").toLowerCase() === "x";
          const prefix = String(m[0] || "");

          // 去掉文本中的 "[ ] " / "[x] " 前缀
          inline.content = String(inline.content || "").slice(prefix.length);
          stripPrefixFromInlineChildren(inline, prefix.length);

          addTokenClass(t, "task-list-item");
          if (checked) addTokenClass(t, "task-list-item-checked");
          if (startLine) t.attrSet("data-md-task-line", String(startLine));
          t.attrSet("data-md-task", checked ? "1" : "0");

          // 在行内内容最前插入 checkbox；并用 span 包裹正文，便于 CSS 统一加删除线
          const mkHtml = (html) => {
            const tok = new state.Token("html_inline", "", 0);
            tok.content = html;
            return tok;
          };

          const checkboxHtml = `<input class="task-list-item-checkbox" type="checkbox"${checked ? " checked" : ""}${startLine ? ` data-md-task-line="${startLine}"` : ""} /> `;

          const children = Array.isArray(inline.children) ? inline.children : [];
          inline.children = [
            mkHtml(checkboxHtml),
            mkHtml('<span class="task-list-item-text">'),
            ...children,
            mkHtml("</span>"),
          ];

          hit += 1;
        }

        if (hit) {
          logger.info("[MarkdownRender] task list parsed", { items: hit });
        }

        return false;
      });
    };

    const installTocSupport = () => {
      const tocMarkerRe = /^\s*(?:\[\[\s*(?:toc|\u76ee\u5f55)\s*\]\]|\[\s*(?:toc|\u76ee\u5f55)\s*\])\s*$/i;

      const slugify = (text) => {
        const raw0 = String(text || "").trim().toLowerCase();
        const raw = raw0.replace(/(\d)[\.\uFF0E](\d)/g, "$1$2");
        const normalized = raw.normalize ? raw.normalize("NFKD") : raw;
        const cleaned = normalized
          .replace(/[^\p{L}\p{N}\s-]/gu, " ")
          .replace(/\s+/g, " ")
          .trim();
        const slug = cleaned.replace(/\s+/g, "-").replace(/-+/g, "-");
        return slug || "section";
      };

      md.core.ruler.after("inline", "guaxi_toc", (state) => {
        const tokens = state.tokens || [];
        const slugCount = new Map();
        const headings = [];

        for (let i = 0; i < tokens.length; i += 1) {
          const t = tokens[i];
          if (!t || t.type !== "heading_open") continue;
          const next = tokens[i + 1];
          if (!next || next.type !== "inline") continue;

          const level = Number(String(t.tag || "h1").slice(1)) || 1;
          const text = String(next.content || "").trim();
          if (!text) continue;

          const base = slugify(text);
          const n = (slugCount.get(base) || 0) + 1;
          slugCount.set(base, n);
          const id = n > 1 ? `${base}-${n}` : base;

          t.meta = { ...(t.meta || {}), guaxiHeadingId: id };

          const startLine = Array.isArray(t.map) ? t.map[0] + 1 : 0;
          headings.push({ level, text, id, startLine });
        }

        if (!headings.length) return false;

        let injected = 0;

        const minLevel = Math.min(...headings.map((h) => h.level));
        const buildTocHtml = () => {
          let html = '<nav class="md-toc" data-md-toc="1"><ul class="md-toc-list">';

          for (const h of headings) {
            const lvl = Math.max(1, (h.level - minLevel) + 1);
            html += `<li class="md-toc-item md-toc-level-${lvl}"><a class="md-toc-link" href="#${encodeURIComponent(String(h.id))}"${h.startLine ? ` data-md-start-line="${h.startLine}"` : ""}>${md.utils.escapeHtml(String(h.text))}</a></li>`;
          }

          html += "</ul></nav>";
          return html;
        };

        for (let i = 0; i < tokens.length - 2; i += 1) {
          const t0 = tokens[i];
          const t1 = tokens[i + 1];
          const t2 = tokens[i + 2];
          if (!t0 || !t1 || !t2) continue;
          if (t0.type !== "paragraph_open" || t1.type !== "inline" || t2.type !== "paragraph_close") continue;

          const raw = String(t1.content || "");
          if (!tocMarkerRe.test(raw)) continue;

          const tok = new state.Token("html_block", "", 0);
          tok.content = buildTocHtml();
          tok.map = t0.map;

          tokens.splice(i, 3, tok);
          injected += 1;
        }

        if (injected) {
          logger.info("[MarkdownRender] toc generated", { items: headings.length, markers: injected });
        }

        return false;
      });
    };

    try {
      installTaskListSupport();
    } catch (e) {
      logger.warn("[MarkdownRender] installTaskListSupport failed", e);
    }

    try {
      installTocSupport();
    } catch (e) {
      logger.warn("[MarkdownRender] installTocSupport failed", e);
    }

    // 可执行代码块 / Mermaid：统一改写 fence，避免多次覆盖 renderer.rules.fence
    if (enableMermaid || enablePythonCells || enableCodeCells) {
      const defaultFence = md.renderer.rules.fence;
      md.renderer.rules.fence = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const info = String(token?.info || '').trim();

        if (enableMermaid && info === "mermaid") {
          const content = String(token.content || '')
            .replace(/&gt;/g, ">")
            .replace(/&lt;/g, "<")
            .replace(/&amp;/g, "&");

          if (preserveOriginal) {
            const original = `\`\`\`mermaid\n${content}\n\`\`\``;
            return `<pre class="mermaid" data-md-original="${safeEncode(original)}">${content}</pre>`;
          }
          return `<pre class="mermaid">${content}</pre>`;
        }

        if (enablePythonCells && /^(python|py)(\s|$)/i.test(info)) {
          const startLine = Array.isArray(token.map) ? token.map[0] + 1 : null;
          const endLine = Array.isArray(token.map) ? token.map[1] + 1 : null;
          const customCellIdMatch = String(info || '').match(/\bguaxi-cell=([^\s`]+)/i);
          const cellId = customCellIdMatch?.[1]
            ? String(customCellIdMatch[1]).trim()
            : `py-${startLine || 'na'}-${endLine || 'na'}`;

          const original = preserveOriginal
            ? `\`\`\`${info || 'python'}\n${String(token.content || '')}\n\`\`\``
            : '';

          const codeHtml = defaultFence ? defaultFence(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);
          const attrs = [
            `class="guaxi-pycell"`,
            `data-guaxi-py="1"`,
            `data-guaxi-py-lang="${safeEncode(info || 'python')}"`,
            `data-guaxi-py-code="${safeEncode(String(token.content || ''))}"`,
            `data-guaxi-py-cell="${safeEncode(cellId)}"`,
            startLine ? `data-guaxi-md-start-line="${startLine}"` : '',
            endLine ? `data-guaxi-md-end-line="${endLine}"` : '',
            preserveOriginal ? `data-md-original="${safeEncode(original)}"` : '',
          ].filter(Boolean).join(' ');

          return `
            <div ${attrs}>
              <div class="guaxi-pycell-toolbar" style="display:flex;align-items:center;gap:8px;margin:8px 0;">
                <button type="button" class="guaxi-pycell-btn guaxi-pycell-run" data-guaxi-py-action="run" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">
                  运行
                </button>
                <button type="button" class="guaxi-pycell-btn guaxi-pycell-run-save" data-guaxi-py-action="run-save" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">
                  运行并写回
                </button>
                <button type="button" class="guaxi-pycell-btn guaxi-pycell-output-toggle" data-guaxi-py-action="toggle-output" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">
                  显示结果
                </button>
                <span class="guaxi-pycell-hint" style="opacity:.7;font-size:12px;">输出会显示在下方（写回会保存到笔记 Markdown）</span>
              </div>
              ${codeHtml}
              <pre class="hljs guaxi-pycell-output" style="margin:8px 0 0;display:none;">
                <code class="language-text" data-guaxi-py-output="${safeEncode(cellId)}"></code>
              </pre>
            </div>
          `;
        }

        if (enableCodeCells) {
          const kind =
            /^(javascript|js)(\s|$)/i.test(info) ? 'javascript'
              : /^(latex|tex)(\s|$)/i.test(info) ? 'latex'
                : /^c(\s|$)/i.test(info) ? 'c'
                  : /^(cpp|c\+\+)(\s|$)/i.test(info) ? 'cpp'
                    : /^(rust|rs)(\s|$)/i.test(info) ? 'rust'
                      : '';

          if (kind) {
            const startLine = Array.isArray(token.map) ? token.map[0] + 1 : null;
            const endLine = Array.isArray(token.map) ? token.map[1] + 1 : null;
            const cellId = `cell-${kind}-${startLine || 'na'}-${endLine || 'na'}`;

            const original = preserveOriginal
              ? `\`\`\`${info || kind}\n${String(token.content || '')}\n\`\`\``
              : '';

            const codeHtml = defaultFence ? defaultFence(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);

            const attrs = [
              `class="guaxi-codecell"`,
              `data-guaxi-cell="1"`,
              `data-guaxi-cell-kind="${safeEncode(kind)}"`,
              `data-guaxi-cell-lang="${safeEncode(info || kind)}"`,
              `data-guaxi-cell-code="${safeEncode(String(token.content || ''))}"`,
              `data-guaxi-cell-id="${safeEncode(cellId)}"`,
              startLine ? `data-guaxi-md-start-line="${startLine}"` : '',
              endLine ? `data-guaxi-md-end-line="${endLine}"` : '',
              preserveOriginal ? `data-md-original="${safeEncode(original)}"` : '',
            ].filter(Boolean).join(' ');

            return `
              <div ${attrs}>
                <div class="guaxi-codecell-toolbar" style="display:flex;align-items:center;gap:8px;margin:8px 0;">
                  <button type="button" class="guaxi-codecell-btn guaxi-codecell-run" data-guaxi-cell-action="run" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">运行</button>
                  <button type="button" class="guaxi-codecell-btn guaxi-codecell-run-save" data-guaxi-cell-action="run-save" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">运行并写回</button>
                  <button type="button" class="guaxi-codecell-btn guaxi-codecell-output-toggle" data-guaxi-cell-action="toggle-output" style="padding:4px 8px;border-radius:6px;border:1px solid var(--guaxi-border);background:var(--guaxi-panel-bg);color:var(--guaxi-text-primary);cursor:pointer;">显示结果</button>
                  <span class="guaxi-codecell-hint" style="opacity:.7;font-size:12px;">输出会显示在下方（写回会保存到笔记 Markdown）</span>
                </div>
                ${codeHtml}
                <pre class="hljs guaxi-codecell-output" style="margin:8px 0 0;display:none;">
                  <code class="language-text" data-guaxi-cell-output="${safeEncode(cellId)}"></code>
                </pre>
              </div>
            `;
          }
        }

        return defaultFence ? defaultFence(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);
      };
    }

    // 添加数学公式支持（完整修改部分）
    if (shouldEnableMath) {
      md.use(markdownItMathjax);

      // 保存原始渲染函数
      const originalInlineMath = md.renderer.rules.math_inline;
      const originalBlockMath = md.renderer.rules.math_block;

      // 重写行内数学公式渲染
      md.renderer.rules.math_inline = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const originalLatex = `$${token.content}$`;

        // 创建容器元素并添加原始内容
        const container = document.createElement("span");
        container.className = "math-inline-container";
        container.setAttribute("data-md-original", safeEncode(originalLatex));

        // 使用原始渲染器生成内容
        const mathContent = originalInlineMath
          ? originalInlineMath(tokens, idx, options, env, self)
          : `<span class="math-inline">${token.content}</span>`;

        container.innerHTML = mathContent;
        return container.outerHTML;
      };

      // 重写块级数学公式渲染
      md.renderer.rules.math_block = (tokens, idx, options, env, self) => {
        const token = tokens[idx];
        const originalLatex = `$$${token.content}$$`;

        // 创建容器元素并添加原始内容
        const container = document.createElement("div");
        container.className = "math-block-container";
        container.setAttribute("data-md-original", safeEncode(originalLatex));

        // 使用原始渲染器生成内容
        const mathContent = originalBlockMath
          ? originalBlockMath(tokens, idx, options, env, self)
          : `<div class="math-block">${token.content}</div>`;

        container.innerHTML = mathContent;
        return container.outerHTML;
      };
    }

    // 执行渲染
    const container = document.createElement("div");
    container.innerHTML = md.render(finalContent);

    // 处理 Mermaid 图表。这里直接在离屏容器里生成最终 SVG，
    // 避免对尚未挂载到真实 DOM 的节点调用 mermaid.run() 导致所有调用方一起失败。
    if (enableMermaid) {
      const mermaidElements = Array.from(container.querySelectorAll(".mermaid"));
      for (const el of mermaidElements) {
        try {
          const source = String(el.textContent || '')
            .replace(/&gt;/g, ">")
            .replace(/&lt;/g, "<")
            .replace(/&amp;/g, "&");

          if (!source.trim()) continue;

          const originalSource = el.getAttribute("data-md-original");
          const renderId = `guaxi-mermaid-${Date.now()}-${mermaidRenderCounter++}`;
          const rendered = await Promise.resolve(mermaid.render(renderId, source));
          const svg = typeof rendered === 'string' ? rendered : String(rendered?.svg || '');
          if (!svg.trim()) continue;

          const wrapper = document.createElement("div");
          wrapper.className = "mermaid";
          wrapper.setAttribute("data-processed", "true");
          wrapper.setAttribute("data-mermaid-source", source);
          if (originalSource) {
            wrapper.setAttribute("data-md-original", originalSource);
          }
          wrapper.innerHTML = svg;
          el.replaceWith(wrapper);

          if (typeof rendered?.bindFunctions === 'function') {
            try {
              rendered.bindFunctions(wrapper);
            } catch (bindError) {
              logger.debug("Mermaid bindFunctions 执行失败", bindError);
            }
          }
        } catch (err) {
          logger.error("Mermaid渲染失败", err);
        }
      }
    }

    if (shouldEnableMath) {
      // 处理已渲染的数学公式
      container
        .querySelectorAll(".math-inline-container, .math-block-container")
        .forEach((el) => {
          const mjxContainer = el.querySelector("mjx-container");
          if (mjxContainer) {
            // 将原始内容转移到mjx-container
            mjxContainer.setAttribute(
              "data-md-original",
              el.getAttribute("data-md-original")
            );

            // 保留容器类名
            mjxContainer.classList.add(
              el.classList.contains("math-inline-container")
                ? "math-inline"
                : "math-block"
            );

            // 替换容器
            el.replaceWith(mjxContainer);
          }
        });
    }

    if (enableButtons) {
      addCopyButtons(container, { theme });
    }
    // logger.info("渲染后的HTML:", container.innerHTML);
    return container.innerHTML;
  } catch (err) {
    logger.error("Markdown处理失败:", err);
    return content;
  }
};

// 安全编码函数（完整实现）
function safeEncode(content) {
  if (!content) {
    logger.warn("安全编码: 空内容");
    return "";
  }

  const encoded = encodeURIComponent(content)
    .replace(/'/g, "%27")
    .replace(/"/g, "%22")
    .replace(/</g, "%3C")
    .replace(/>/g, "%3E")
    .replace(/&/g, "%26")
    .replace(/\//g, "%2F")
    .replace(/\\/g, "%5C");

  // logger.info("安全编码结果:", { original: content, encoded });
  return encoded;
}
