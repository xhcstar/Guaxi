import MarkdownIt from "markdown-it";
import markdownItMathjax from "markdown-it-mathjax3";
import mermaid from "mermaid";
import hljs from "highlight.js";
import "highlight.js/styles/atom-one-light.css";
import { initMathJax, renderMath } from "./mathjaxConfig";
import { nextTick } from "vue";
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiMarker')

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

  // 查找所有可能的代码块容器（包括 hljs 和 pre code）
  const codeContainers = Array.from(
    container.querySelectorAll(".hljs, pre code")
  );
  const processedBlocks = new Set();

  codeContainers.forEach((codeContainer) => {
    // 确保容器存在且没有重复按钮
    if (!codeContainer || codeContainer.querySelector(".copy-button")) return;

    // 获取实际的代码内容容器（可能是pre或.hljs）
    const codeBlock = codeContainer.classList.contains("hljs")
      ? codeContainer
      : codeContainer.parentElement;

    if (!codeBlock) return;
    if (processedBlocks.has(codeBlock)) return;
    processedBlocks.add(codeBlock);

    if (shouldSkipForNotes(codeBlock)) return;

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
        const wAttr = Number.parseFloat(String(svg?.getAttribute?.('width') || ''));
        const hAttr = Number.parseFloat(String(svg?.getAttribute?.('height') || ''));
        if (Number.isFinite(wAttr) && wAttr > 0 && Number.isFinite(hAttr) && hAttr > 0) {
          return { width: wAttr, height: hAttr };
        }

        try {
          const rect = svg?.getBoundingClientRect?.();
          if (rect && rect.width > 0 && rect.height > 0) return { width: rect.width, height: rect.height };
        } catch {
        }

        const vbBase = svg?.viewBox?.baseVal;
        if (vbBase && vbBase.width > 0 && vbBase.height > 0) return { width: vbBase.width, height: vbBase.height };

        const vbAttr = String(svg?.getAttribute?.('viewBox') || '').trim();
        if (vbAttr) {
          const nums = vbAttr.split(/[\s,]+/).map((v) => Number.parseFloat(v)).filter((n) => Number.isFinite(n));
          if (nums.length === 4 && nums[2] > 0 && nums[3] > 0) return { width: nums[2], height: nums[3] };
        }

        return { width: 800, height: 600 };
      } catch {
        return { width: 800, height: 600 };
      }
    };

    const buildExportSvg = (svg, { background = true } = {}) => {
      try {
        if (!(svg instanceof SVGElement)) return null;
        const { width, height } = resolveSvgSize(svg);
        const w = Math.max(1, Math.floor(width));
        const h = Math.max(1, Math.floor(height));

        const cloned = svg.cloneNode(true);
        if (!(cloned instanceof SVGElement)) return null;

        if (!cloned.getAttribute('xmlns')) cloned.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
        if (!cloned.getAttribute('viewBox')) cloned.setAttribute('viewBox', `0 0 ${w} ${h}`);
        cloned.setAttribute('width', String(w));
        cloned.setAttribute('height', String(h));

        if (background) {
          const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
          rect.setAttribute('x', '0');
          rect.setAttribute('y', '0');
          rect.setAttribute('width', '100%');
          rect.setAttribute('height', '100%');
          rect.setAttribute('fill', '#ffffff');
          cloned.insertBefore(rect, cloned.firstChild);
        }

        return { svg: cloned, width: w, height: h };
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
        toolbar.appendChild(btnMinus);
        toolbar.appendChild(btnPlus);
        toolbar.appendChild(btnClose);

        const stage = document.createElement('div');
        stage.style.cssText = 'width:95vw;height:92vh;max-width:95vw;max-height:92vh;display:flex;align-items:center;justify-content:center;overflow:auto;padding:40px 24px 24px;box-sizing:border-box;background:rgba(255,255,255,0.98);';

        const contentWrap = document.createElement('div');
        contentWrap.style.cssText = 'transform-origin:center center;';
        stage.appendChild(contentWrap);

        let zoom = 1;
        const applyZoom = () => {
          const z = Math.max(0.1, Math.min(8, zoom));
          zoom = z;
          contentWrap.style.transform = `scale(${z})`;
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

        const close = () => {
          try {
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
        btnPlus.addEventListener('click', () => {
          zoom *= 1.1;
          applyZoom();
        });
        btnMinus.addEventListener('click', () => {
          zoom /= 1.1;
          applyZoom();
        });

        overlay.addEventListener('click', (e) => {
          if (e.target === overlay) close();
        });

        stage.addEventListener(
          'wheel',
          (e) => {
            e.preventDefault();
            const delta = e.deltaY > 0 ? 0.9 : 1.1;
            zoom *= delta;
            applyZoom();
          },
          { passive: false }
        );

        document.addEventListener('keydown', onKeyDown, true);

        panel.appendChild(toolbar);
        panel.appendChild(legend);
        panel.appendChild(stage);
        overlay.appendChild(panel);
        document.body.appendChild(overlay);
        applyZoom();
      } catch (e) {
        logger.error('[MediaViewer] 打开预览失败:', e);
      }
    };

    const saveBlob = async (blob, defaultName, filters) => {
      try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        const { writeFile } = await import('@tauri-apps/plugin-fs');
        const path = await save({ defaultPath: defaultName, filters });
        if (!path) return;
        const buf = new Uint8Array(await blob.arrayBuffer());
        await writeFile(path, buf);
      } catch (e) {
        logger.error('[MediaViewer] 保存失败:', e);
      }
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

    const svgToPngBlob = async (svg) => {
      const normalized = buildExportSvg(svg, { background: true });
      const exportSvg = normalized?.svg || svg;
      const exportW = normalized?.width || 800;
      const exportH = normalized?.height || 600;

      const xml = new XMLSerializer().serializeToString(exportSvg);
      const hasXmlns = /xmlns=/.test(xml);
      const svgText = hasXmlns ? xml : xml.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"');
      const svgBlob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' });
      const url = URL.createObjectURL(svgBlob);

      try {
        const img = new Image();
        img.decoding = 'async';
        await new Promise((resolve, reject) => {
          img.onload = resolve;
          img.onerror = reject;
          img.src = url;
        });

        const w = Math.max(1, Math.floor(exportW));
        const h = Math.max(1, Math.floor(exportH));

        const canvas = document.createElement('canvas');
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('canvas ctx is null');
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);

        const blob = await new Promise((resolve, reject) => {
          try {
            canvas.toBlob((b) => resolve(b), 'image/png');
          } catch (e) {
            reject(e);
          }
        });
        if (!blob) throw new Error('toBlob returns null');
        return blob;
      } finally {
        URL.revokeObjectURL(url);
      }
    };

    const saveImage = async (img) => {
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
          const m = src.match(/^http:\/\/localhost:8888\/markdown\/api\/images\/(.+)$/i);
          if (m && m[1]) {
            try {
              const { readFile } = await import('@tauri-apps/plugin-fs');
              let filePath = '';
              try {
                filePath = decodeURIComponent(m[1]);
              } catch {
                filePath = m[1];
              }
              if (/^[a-zA-Z]:\//.test(filePath)) {
                filePath = filePath.replace(/\//g, '\\');
              }
              const bytes = await readFile(filePath);
              blob = new Blob([bytes], { type: `image/${ext === 'jpg' ? 'jpeg' : ext}` });
              logger.info('[MediaViewer] 保存图片: 使用 readFile 读取本地路径:', { filePath });
            } catch (e) {
              logger.warn('[MediaViewer] readFile 兜底失败:', e);
            }
          }
        }

        if (!blob) throw new Error('image blob is null');

        await saveBlob(blob, `image.${ext}`, [{ name: 'Image', extensions: [ext] }]);
      } catch (e) {
        logger.error('[MediaViewer] 保存图片失败:', e);
      }
    };

    const saveMermaid = async (mermaidEl) => {
      try {
        const svg = resolveMermaidSvg(mermaidEl);
        if (!svg) {
          logger.warn('[MediaViewer] 未找到 Mermaid SVG');
          return;
        }

        const normalized = buildExportSvg(svg, { background: true });
        const exportSvg = normalized?.svg || svg;
        const svgText = new XMLSerializer().serializeToString(exportSvg);
        const svgBlob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' });

        const { save } = await import('@tauri-apps/plugin-dialog');
        const path = await save({
          defaultPath: 'mermaid.png',
          filters: [
            { name: 'PNG', extensions: ['png'] },
            { name: 'SVG', extensions: ['svg'] }
          ]
        });
        if (!path) return;

        const lower = String(path).toLowerCase();
        const wantSvg = lower.endsWith('.svg');

        const { writeFile } = await import('@tauri-apps/plugin-fs');

        if (wantSvg) {
          const buf = new Uint8Array(await svgBlob.arrayBuffer());
          await writeFile(path, buf);
          return;
        }

        try {
          const pngBlob = await svgToPngBlob(svg);
          const buf = new Uint8Array(await pngBlob.arrayBuffer());
          await writeFile(path, buf);
        } catch (e) {
          logger.warn('[MediaViewer] Mermaid 转 PNG 失败，回退保存 SVG:', e);
          const fallbackPath = String(path).replace(/\.png$/i, '.svg');
          const buf = new Uint8Array(await svgBlob.arrayBuffer());
          await writeFile(fallbackPath, buf);
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

    container.querySelectorAll('img').forEach((img) => {
      if (shouldSkipForNotes(img)) return;
      const wrap = ensureImgWrap(img);
      if (!wrap) return;
      addBtnsToWrap(
        wrap,
        {
          onView: () => openOverlay({ kind: 'img', src: img.currentSrc || img.src, alt: img.alt }),
          onSave: () => saveImage(img),
        },
        {
          viewTitle: '预览图片（打开查看器）',
          saveTitle: '保存图片到文件',
        }
      );
    });

    container.querySelectorAll('.mermaid').forEach((mermaidEl) => {
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
          onSave: () => saveMermaid(mermaidEl),
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
  } = options;

  try {
    let finalContent = content;
    let mappings = [];

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

    if (enableMath) {
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
          return `<pre><code>${md.utils.escapeHtml(code)}</code></pre>`;
        }
        : undefined,
    });

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
          const cellId = `py-${startLine || 'na'}-${endLine || 'na'}`;

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
    if (enableMath) {
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

    // 处理Mermaid图表
    if (enableMermaid) {
      const mermaidElements = container.querySelectorAll(".mermaid");
      if (mermaidElements.length > 0) {
        try {
          // 处理每个图表
          mermaidElements.forEach((el) => {
            try {
              const content = el.textContent
                .replace(/&gt;/g, ">")
                .replace(/&lt;/g, "<")
                .replace(/&amp;/g, "&");
              el.innerHTML = content;
            } catch (err) {
              logger.error("Mermaid内容处理错误:", err);
            }
          });

          // Vue 的 nextTick() 只是 等 Vue 自己的 DOM 变更完成,
          // 但并不等浏览器真正渲染并解析DOM nextTick()确保Vue更新了Virtual DOM, 并把它提交到浏览器.
          // 但浏览器可能尚未完成布局和样式计算, Mermaid可能无法正确解析 .mermaid 元素.
          // Mermaid可能依赖某些浏览器渲染完成的属性但nextTick()时这些属性可能尚未计算完成,导致Mermaid找不到.mermaid或渲染失败。
          // 尝试requestAnimationFrame()因为它会等到浏览器完成一次帧渲染后再执行.
          nextTick().then(() => {
            requestAnimationFrame(() => {
              try {
                const nodes = Array.from(container.querySelectorAll('.mermaid') || []);
                if (!nodes.length) return;

                Promise.resolve(mermaid.run({ nodes }))
                  .then(() => {
                    logger.debug('Mermaid渲染完成', { count: nodes.length });
                  })
                  .catch((err) => {
                    logger.error('Mermaid渲染失败', err);
                  });
              } catch (err) {
                logger.error('Mermaid渲染失败（同步异常）', err);
              }
            });
          });

          // logger.info("Mermaid渲染完成: ", container.innerHTML);
        } catch (err) {
          logger.error("Mermaid渲染错误:", err);
        }
      }
    }

    if (enableMath) {
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
