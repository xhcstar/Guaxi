import { ref, computed, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import * as pdfjsLib from "pdfjs-dist/build/pdf.mjs";
import { createLogger } from '../../utils/logger'

const logger = createLogger('usePdfRenderer')

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url
).toString();

export default function usePdfRenderer(props) {
  const isDebug = () => {
    try {
      const v = String(window?.localStorage?.getItem("guaxi_debug_pdfjs") || "").trim().toLowerCase();
      return v === "1" || v === "true" || v === "yes" || v === "on";
    } catch {
      return false;
    }
  };

  const dlog = (...args) => {
    if (!isDebug()) return;
    logger.info("[PDFJS]", ...args);
  };

  // 增强props验证
  if (!props || typeof props !== "object") {
    throw new Error("usePdfRenderer: props必须是对象");
  }
  if (!props.filePath || typeof props.filePath !== "string") {
    throw new Error("usePdfRenderer: filePath必须是字符串且不能为空");
  }

  const renderMode = ref("image");
  const currentPageData = ref(null);
  const textLayer = ref(null);
  const isLoading = ref(false);
  const error = ref(null);

  let pdfDoc = null;
  let pdfDataKey = "";
  let outlineFlatCache = null;
  const pageTextCache = new Map();

  let annotationEditorUiManager = null;
  let annotationEditorEventBus = null;
  let annotationEditorDataKey = "";
  let annotationEditorContainerEl = null;
  let annotationEditorViewerEl = null;
  let lastAnnotationEditorLayer = null;

  const resetDocumentState = () => {
    pdfDoc = null;
    pdfDataKey = "";
    outlineFlatCache = null;
    pageTextCache.clear();
    currentPageData.value = null;
    error.value = null;

    try {
      lastAnnotationEditorLayer?.destroy?.();
    } catch {
    }
    lastAnnotationEditorLayer = null;

    try {
      annotationEditorUiManager?.destroy?.();
    } catch {
    }
    annotationEditorUiManager = null;
    annotationEditorEventBus = null;
    annotationEditorDataKey = "";
    annotationEditorContainerEl = null;
    annotationEditorViewerEl = null;
  };

  watch(
    () => props.filePath,
    (nextPath) => {
      const fp = String(nextPath || "");
      if (!fp) return;
      if (pdfDataKey && pdfDataKey !== fp) {
        resetDocumentState();
      }
    }
  );

  const decodeBase64ToBytes = (b64) => {
    const bin = window.atob(String(b64 || ""));
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
    return out;
  };

  const ensurePdfDocLoaded = async () => {
    const fp = String(props.filePath || "");
    if (!fp) throw new Error("Missing filePath");

    if (pdfDoc && pdfDataKey === fp) return pdfDoc;

    const b64 = await invoke("read_file_bytes_base64", { path: fp });
    const bytes = decodeBase64ToBytes(b64);
    const loadingTask = pdfjsLib.getDocument({ data: bytes });
    pdfDoc = await loadingTask.promise;
    pdfDataKey = fp;
    outlineFlatCache = null;
    pageTextCache.clear();
    return pdfDoc;
  };

  const getPageNumberFromExplicitDest = async (explicitDest) => {
    if (!pdfDoc) return null;
    if (!Array.isArray(explicitDest) || !explicitDest.length) return null;
    const [destRef] = explicitDest;
    if (destRef && typeof destRef === "object") {
      try {
        return (await pdfDoc.getPageIndex(destRef)) + 1;
      } catch {
        return null;
      }
    }
    if (Number.isInteger(destRef)) return destRef + 1;
    return null;
  };

  const resolveDestToPageNumber = async (dest) => {
    await ensurePdfDocLoaded();
    if (!pdfDoc) return null;

    try {
      if (typeof dest === "string" && dest) {
        const explicit = await pdfDoc.getDestination(dest);
        return await getPageNumberFromExplicitDest(explicit);
      }
      const explicit = await dest;
      return await getPageNumberFromExplicitDest(explicit);
    } catch {
      return null;
    }
  };

  const findFirstDestPage = async (items) => {
    const list = Array.isArray(items) ? items : [];
    for (let i = 0; i < list.length; i += 1) {
      const it = list[i];
      const p = await resolveDestToPageNumber(it?.dest);
      if (p) return p;
      const sub = await findFirstDestPage(it?.items);
      if (sub) return sub;
    }
    return null;
  };

  const getOutlineFlat = async () => {
    await ensurePdfDocLoaded();
    if (!pdfDoc) return [];
    if (outlineFlatCache) return outlineFlatCache;

    const raw = await pdfDoc.getOutline();
    const result = [];

    const walk = async (items, level) => {
      const list = Array.isArray(items) ? items : [];
      for (let i = 0; i < list.length; i += 1) {
        const it = list[i];
        const text = String(it?.title || "").trim();
        const line = (await resolveDestToPageNumber(it?.dest)) || (await findFirstDestPage(it?.items));
        if (text && line) {
          result.push({ level, text, line });
        }
        if (it?.items && it.items.length) {
          await walk(it.items, Math.min(6, level + 1));
        }
      }
    };

    await walk(raw, 1);
    outlineFlatCache = result;
    return outlineFlatCache;
  };

  const getPageText = async (pageNum) => {
    await ensurePdfDocLoaded();
    const p = Number(pageNum);
    if (!pdfDoc || !Number.isFinite(p) || p < 1 || p > pdfDoc.numPages) return "";

    const cached = pageTextCache.get(p);
    if (typeof cached === "string") return cached;

    const page = await pdfDoc.getPage(p);
    const tc = await page.getTextContent();
    const items = Array.isArray(tc?.items) ? tc.items : [];
    const text = items.map((it) => String(it?.str || "")).join(" ");
    pageTextCache.set(p, text);
    return text;
  };

  const searchText = async (query) => {
    const q = String(query || "").trim();
    if (!q) return [];

    const doc = await ensurePdfDocLoaded();
    if (!doc) return [];

    const qLower = q.toLowerCase();
    const hits = [];

    for (let p = 1; p <= doc.numPages; p += 1) {
      const textLower = String(await getPageText(p)).toLowerCase();
      let idx = textLower.indexOf(qLower);
      if (idx === -1) continue;

      let count = 0;
      while (idx !== -1) {
        count += 1;
        idx = textLower.indexOf(qLower, idx + qLower.length);
      }

      hits.push({ page: p, count });
    }

    return hits;
  };

  const imageResourcesPath = new URL("pdfjs-dist/web/images/", import.meta.url).toString();

  const createLinkService = (onGoToPage) => {
    const onGo = typeof onGoToPage === "function" ? onGoToPage : null;

    const addLinkAttributes = (link, url, newWindow = false) => {
      if (!url || typeof url !== "string") return;
      link.href = url;
      link.title = url;
      if (newWindow) link.target = "_blank";
      link.rel = "noopener noreferrer";
    };

    const getAnchorUrl = (hash) => {
      const h = String(hash || "");
      return h ? `#${h.replace(/^#/, "")}` : "#";
    };

    const getDestinationHash = (dest) => {
      if (typeof dest === "string" && dest) return `#${encodeURIComponent(dest)}`;
      return "#";
    };

    const getPageNumberFromExplicitDest = async (explicitDest) => {
      if (!pdfDoc) return null;
      if (!Array.isArray(explicitDest) || !explicitDest.length) return null;
      const [destRef] = explicitDest;
      if (destRef && typeof destRef === "object") {
        try {
          return (await pdfDoc.getPageIndex(destRef)) + 1;
        } catch {
          return null;
        }
      }
      if (Number.isInteger(destRef)) return destRef + 1;
      return null;
    };

    const goToDestination = async (dest) => {
      if (!pdfDoc || !onGo) return;
      let explicitDest = null;
      if (typeof dest === "string") {
        explicitDest = await pdfDoc.getDestination(dest);
      } else {
        explicitDest = await dest;
      }
      const pageNumber = await getPageNumberFromExplicitDest(explicitDest);
      if (pageNumber) onGo(pageNumber);
    };

    const executeNamedAction = (action) => {
      if (!onGo) return;
      const cur = Number(currentPageData.value?.page_number) || 1;
      const total = Number(pdfDoc?.numPages) || 1;
      switch (String(action || "")) {
        case "NextPage":
          return onGo(Math.min(total, cur + 1));
        case "PrevPage":
          return onGo(Math.max(1, cur - 1));
        case "FirstPage":
          return onGo(1);
        case "LastPage":
          return onGo(total);
        default:
          return;
      }
    };

    const executeSetOCGState = () => {
    };

    const eventBus = { dispatch: () => {
    } };

    return {
      eventBus,
      externalLinkEnabled: true,
      externalLinkTarget: null,
      externalLinkRel: "noopener noreferrer",
      addLinkAttributes,
      getAnchorUrl,
      getDestinationHash,
      goToDestination,
      executeNamedAction,
      executeSetOCGState,
    };
  };

  let renderTextLayerFn = pdfjsLib.renderTextLayer || null;
  let TextLayerCtor = pdfjsLib.TextLayer || null;

  const ensureTextLayerApis = async () => {
    if (TextLayerCtor || renderTextLayerFn) {
      return { TextLayerCtor, renderTextLayerFn };
    }

    try {
      const mod = await import("pdfjs-dist/web/pdf_viewer.mjs");
      TextLayerCtor = mod?.TextLayer || null;
      renderTextLayerFn = renderTextLayerFn || mod?.renderTextLayer || null;
    } catch (err) {
      dlog("textLayer:apiImportFailed", { name: err?.name, message: err?.message });
      TextLayerCtor = null;
    }

    return { TextLayerCtor, renderTextLayerFn };
  };

  let canvasRenderTask = null;
  let canvasRenderTarget = null;
  let renderSeq = 0;
  let activeRenderToken = 0;

  const isRenderCancelled = (err) => {
    const name = String(err?.name || "");
    const msg = String(err?.message || "");
    return name === "RenderingCancelledException" || /rendering cancelled/i.test(msg);
  };

  const renderInto = async (page, viewport, canvasEl, textLayerEl, annotationLayerEl, editorLayerEl, onGoToPage, token = 0) => {
    if (token) activeRenderToken = token;
    if (token && token !== activeRenderToken) return false;

    dlog("renderInto:start", {
      token,
      hasCanvas: Boolean(canvasEl),
      hasTextLayer: Boolean(textLayerEl),
      hasAnnotationLayer: Boolean(annotationLayerEl),
      viewport: {
        width: Math.round(Number(viewport?.width) || 0),
        height: Math.round(Number(viewport?.height) || 0),
        scale: Number(viewport?.scale) || 0,
        rotation: Number(viewport?.rotation) || 0,
      },
    });

    if (canvasEl) {
      const prevTask = canvasRenderTask;
      if (prevTask && canvasRenderTarget === canvasEl) {
        try {
          prevTask.cancel();
        } catch {
        }
        try {
          await prevTask.promise;
        } catch {
        }
      }

      if (token && token !== activeRenderToken) return false;

      const dpr = Math.max(1, Number(window.devicePixelRatio) || 1);
      const ctx = canvasEl.getContext("2d", { alpha: false });
      if (!ctx) throw new Error("Failed to create canvas context");

      canvasEl.width = Math.max(1, Math.floor(viewport.width * dpr));
      canvasEl.height = Math.max(1, Math.floor(viewport.height * dpr));
      canvasEl.style.width = `${Math.floor(viewport.width)}px`;
      canvasEl.style.height = `${Math.floor(viewport.height)}px`;

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const task = page.render({ canvasContext: ctx, viewport });
      canvasRenderTask = task;
      canvasRenderTarget = canvasEl;

      try {
        await task.promise;
      } catch (err) {
        if (isRenderCancelled(err)) return false;
        throw err;
      } finally {
        if (canvasRenderTask === task) {
          canvasRenderTask = null;
        }
      }
    }

    if (token && token !== activeRenderToken) return false;

    if (textLayerEl) {
      try {
        textLayerEl.classList.add("textLayer");
        textLayerEl.style.pointerEvents = "auto";
      } catch {
      }

      textLayerEl.innerHTML = "";
      textLayerEl.style.width = `${Math.floor(viewport.width)}px`;
      textLayerEl.style.height = `${Math.floor(viewport.height)}px`;

      const { TextLayerCtor, renderTextLayerFn } = await ensureTextLayerApis();
      if (token && token !== activeRenderToken) return false;

      dlog("textLayer:api", {
        hasTextLayerCtor: Boolean(TextLayerCtor),
        hasRenderTextLayer: Boolean(renderTextLayerFn),
      });

      const useStream = typeof page?.streamTextContent === "function";
      const textContentSource = useStream
        ? page.streamTextContent({ includeMarkedContent: true })
        : await page.getTextContent();

      if (token && token !== activeRenderToken) return false;

      dlog("textLayer:textContentSource", {
        mode: useStream ? "stream" : "object",
        items: !useStream && Array.isArray(textContentSource?.items) ? textContentSource.items.length : null,
        styles: !useStream && textContentSource?.styles ? Object.keys(textContentSource.styles).length : null,
      });

      const textViewport = viewport.clone({ dontFlip: true });
      try {
        textLayerEl.style.setProperty("--scale-factor", String(textViewport.scale));
        textLayerEl.style.setProperty("--total-scale-factor", String(textViewport.scale));
      } catch {
      }

      try {
        if (TextLayerCtor) {
          const layer = new TextLayerCtor({
            textContentSource,
            container: textLayerEl,
            viewport: textViewport,
          });
          await layer.render();
          dlog("textLayer:rendered", { mode: "TextLayer" });
        } else if (renderTextLayerFn) {
          const task = renderTextLayerFn({
            textContentSource,
            container: textLayerEl,
            viewport: textViewport,
          });
          await (task?.promise || task);
          dlog("textLayer:rendered", { mode: "renderTextLayer" });
        } else {
          dlog("textLayer:skipped", { reason: "no api" });
        }
      } catch (err) {
        if (isRenderCancelled(err)) return false;
        dlog("textLayer:error", { name: err?.name, message: err?.message });
        throw err;
      }

      try {
        const spanCount = textLayerEl.querySelectorAll("span").length;
        const rect = textLayerEl.getBoundingClientRect();
        const pe = window.getComputedStyle(textLayerEl).pointerEvents;
        dlog("textLayer:dom", {
          spanCount,
          rect: { w: Math.round(rect.width), h: Math.round(rect.height) },
          pointerEvents: pe,
        });
      } catch {
      }
    }

    if (token && token !== activeRenderToken) return false;

    let annotationLayerObj = null;

    if (annotationLayerEl) {
      try {
        annotationLayerEl.classList.add("annotationLayer");
        annotationLayerEl.style.pointerEvents = "auto";
      } catch {
      }

      annotationLayerEl.innerHTML = "";
      annotationLayerEl.style.width = `${Math.floor(viewport.width)}px`;
      annotationLayerEl.style.height = `${Math.floor(viewport.height)}px`;

      const annViewport = viewport.clone({ dontFlip: true });
      try {
        const annotations = await page.getAnnotations({ intent: "display" });
        if (token && token !== activeRenderToken) return false;

        const layer = new pdfjsLib.AnnotationLayer({
          div: annotationLayerEl,
          page,
          viewport: annViewport,
        });
        annotationLayerObj = layer;

        await layer.render({
          annotations,
          linkService: createLinkService(onGoToPage),
          annotationStorage: pdfDoc?.annotationStorage,
          imageResourcesPath,
          renderForms: true,
        });

        dlog("annotationLayer:rendered", {
          count: Array.isArray(annotations) ? annotations.length : null,
        });
      } catch (err) {
        if (isRenderCancelled(err)) return false;
        dlog("annotationLayer:error", { name: err?.name, message: err?.message });
        throw err;
      }
    }

    if (token && token !== activeRenderToken) return false;

    if (editorLayerEl) {
      editorLayerEl.innerHTML = "";
      editorLayerEl.style.width = `${Math.floor(viewport.width)}px`;
      editorLayerEl.style.height = `${Math.floor(viewport.height)}px`;

      const annEditorViewport = viewport.clone({ dontFlip: true });
      try {
        const textLayerObj = textLayerEl ? { div: textLayerEl } : null;

        try {
          lastAnnotationEditorLayer?.destroy?.();
        } catch {
        }
        lastAnnotationEditorLayer = null;

        if (annotationEditorUiManager) {
          const pageIndex = (Number(page?.pageNumber) || 1) - 1;
          lastAnnotationEditorLayer = new pdfjsLib.AnnotationEditorLayer({
            uiManager: annotationEditorUiManager,
            pageIndex,
            div: editorLayerEl,
            structTreeLayer: null,
            accessibilityManager: null,
            annotationLayer: annotationLayerObj,
            drawLayer: null,
            textLayer: textLayerObj,
            viewport: annEditorViewport,
            l10n: null,
          });

          lastAnnotationEditorLayer.render({
            viewport: annEditorViewport,
            div: editorLayerEl,
            annotations: null,
            intent: "display",
          });
        } else {
          editorLayerEl.innerHTML = "";
        }
      } catch (err) {
        if (isRenderCancelled(err)) return false;
        dlog("annotationEditorLayer:error", { name: err?.name, message: err?.message });
        throw err;
      }
    }

    dlog("renderInto:done", { token });
    return true;
  };

  const loadPage = async (pageNum, options = {}) => {
    isLoading.value = true;
    error.value = null;

    try {
      dlog("loadPage", {
        pageNum,
        scale: options?.scale,
        rotation: options?.rotation,
        hasCanvasEl: Boolean(options?.canvasEl),
        hasTextLayerEl: Boolean(options?.textLayerEl),
        hasAnnotationLayerEl: Boolean(options?.annotationLayerEl),
        hasOnGoToPage: typeof options?.onGoToPage === "function",
        updateState: options?.updateState,
      });
      const targetPage = Number(pageNum);
      if (!Number.isFinite(targetPage) || targetPage < 1) {
        throw new Error("Invalid page number");
      }

      await ensurePdfDocLoaded();

      if (targetPage > pdfDoc.numPages) {
        throw new Error(`页码 ${targetPage} 超出范围(总页数: ${pdfDoc.numPages})`);
      }

      const page = await pdfDoc.getPage(targetPage);

      const baseViewport = page.getViewport({ scale: 1, rotation: 0 });
      const baseRotation = baseViewport.width > baseViewport.height ? 90 : 0;

      const scale = Number(options?.scale) || 1;
      const rotation = Number(options?.rotation) || 0;
      const viewport = page.getViewport({
        scale,
        rotation: (baseRotation + rotation) % 360,
      });

      const wantsRender = Boolean(options?.canvasEl || options?.textLayerEl || options?.annotationLayerEl);
      const token = wantsRender ? (renderSeq += 1) : 0;

      const didRender = await renderInto(
        page,
        viewport,
        options?.canvasEl || null,
        options?.textLayerEl || null,
        options?.annotationLayerEl || null,
        options?.editorLayerEl || null,
        options?.onGoToPage || null,
        token
      );

      const data = {
        page_number: targetPage,
        total_pages: pdfDoc.numPages,
        svg: null,
        image_data: null,
        text_blocks: [],
        error: null,
        page_width: viewport.width,
        page_height: viewport.height,
        page_rotation: baseRotation,
      };

      if (options?.updateState !== false) {
        if (!wantsRender || didRender) {
          currentPageData.value = data;
        }
      }
      return data;
    } catch (err) {
      if (isRenderCancelled(err)) {
        return currentPageData.value;
      }
      error.value = err?.message || "加载PDF失败";
      throw err;
    } finally {
      isLoading.value = false;
    }
  };

  // 优化的渲染内容计算
  const renderContent = computed(() => {
    if (!currentPageData.value) return null;

    const { svg, image_data, text_blocks = [] } = currentPageData.value;

    let type = "image";
    let content = null;

    if (svg) {
      type = "vector";
      content = svg;
    } else if (image_data) {
      type = "image";
      content = `data:image/png;base64,${image_data}`;
    } else if (text_blocks.length > 0) {
      type = "hybrid";
      content = null;
    } else {
      type = "pdfjs";
      content = null;
    }

    return {
      type,
      content,
      textBlocks: text_blocks,
      meta: {
        page: currentPageData.value.page_number,
        total: currentPageData.value.total_pages,
      },
    };
  });

  // 改进的文本层初始化
  const initTextLayer = () => {
    if (!textLayer.value || !renderContent.value?.textBlocks?.length) return;

    const layer = textLayer.value;
    layer.innerHTML = "";

    const container = document.querySelector(".pdf-image-container");
    if (!container) return;

    const { width: containerWidth } = container.getBoundingClientRect();
    const scale = containerWidth / currentPageData.value.page_width;

    renderContent.value.textBlocks.forEach((block, index) => {
      const div = document.createElement("div");
      div.className = "text-block";
      div.dataset.blockId = `block-${index}`;

      Object.assign(div.style, {
        position: "absolute",
        left: `${block.bounds.x * scale}px`,
        top: `${block.bounds.y * scale}px`,
        width: `${block.bounds.w * scale}px`,
        height: `${block.bounds.h * scale}px`,
        fontSize: `${Math.max(10, 12 * scale)}px`,
        lineHeight: "1.2",
        cursor: "text",
        pointerEvents: "auto",
      });

      div.textContent = block.text;
      layer.appendChild(div);
    });
  };

  // 增强的文本选择处理
  const handleTextSelection = (event) => {
    const selection = window.getSelection();
    if (!selection.toString().trim()) return;

    const blockElement = event.target.closest(".text-block");
    const blockId = blockElement?.dataset.blockId;

    return {
      text: selection.toString(),
      blockId,
      page: currentPageData.value?.page_number,
    };
  };

  // 优化的监听逻辑
  watch(
    () => renderContent.value?.type,
    (newType) => {
      if (newType === "hybrid") {
        nextTick(initTextLayer);
      }
    }
  );

  const ensureAnnotationEditorUi = async ({ containerEl, viewerEl } = {}) => {
    await ensurePdfDocLoaded();
    if (!pdfDoc) return false;

    if (annotationEditorUiManager && annotationEditorDataKey === pdfDataKey) {
      if (containerEl instanceof HTMLElement) annotationEditorContainerEl = containerEl;
      if (viewerEl instanceof HTMLElement) annotationEditorViewerEl = viewerEl;
      return true;
    }

    if (!(containerEl instanceof HTMLElement) || !(viewerEl instanceof HTMLElement)) {
      return false;
    }

    let EventBusCtor = null;
    try {
      const mod = await import("pdfjs-dist/web/pdf_viewer.mjs");
      EventBusCtor = mod?.EventBus || null;
    } catch {
      EventBusCtor = null;
    }

    if (!EventBusCtor) return false;

    annotationEditorEventBus = new EventBusCtor();
    annotationEditorContainerEl = containerEl;
    annotationEditorViewerEl = viewerEl;

    annotationEditorUiManager = new pdfjsLib.AnnotationEditorUIManager(
      annotationEditorContainerEl,
      annotationEditorViewerEl,
      null,
      annotationEditorEventBus,
      pdfDoc,
      null,
      null,
      false,
      false,
      false,
      null,
      null,
      true
    );

    annotationEditorDataKey = pdfDataKey;
    return true;
  };

  const setAnnotationEditorMode = async (mode) => {
    const m = String(mode || "none").toLowerCase();
    if (!annotationEditorUiManager) return false;

    const t =
      m === "highlight"
        ? pdfjsLib.AnnotationEditorType.HIGHLIGHT
        : m === "freetext" || m === "text"
          ? pdfjsLib.AnnotationEditorType.FREETEXT
          : pdfjsLib.AnnotationEditorType.NONE;

    try {
      annotationEditorUiManager.updateMode(t);
      return true;
    } catch {
      return false;
    }
  };

  const saveDocumentBase64 = async () => {
    await ensurePdfDocLoaded();
    if (!pdfDoc || typeof pdfDoc.saveDocument !== "function") {
      throw new Error("PDF.js saveDocument is not available");
    }

    const bytes = await pdfDoc.saveDocument();
    if (!(bytes instanceof Uint8Array)) {
      throw new Error("saveDocument did not return Uint8Array");
    }

    let binary = "";
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
      binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    return window.btoa(binary);
  };

  const getAnnotationEditorIdsForPage = (pageIndex) => {
    if (!annotationEditorUiManager) return [];
    const idx = Math.max(0, Number(pageIndex) || 0);
    try {
      return (annotationEditorUiManager.getEditors(idx) || [])
        .map((e) => (e && typeof e.id === "string" ? e.id : null))
        .filter(Boolean);
    } catch {
      return [];
    }
  };

  const removeAnnotationEditorById = async (id) => {
    if (!annotationEditorUiManager) return false;
    const eid = String(id || "");
    if (!eid) return false;

    try {
      const editor = annotationEditorUiManager.getEditor(eid);
      if (!editor) return false;
      annotationEditorUiManager.removeEditor(editor);
      await nextTick();
      return true;
    } catch {
      return false;
    }
  };

  const highlightSelectionAsAnnotation = async ({ color } = {}) => {
    if (!annotationEditorUiManager) return null;

    const pageIndex = Math.max(0, (Number(currentPageData.value?.page_number) || 1) - 1);
    const before = new Set(getAnnotationEditorIdsForPage(pageIndex));

    const c = typeof color === "string" && color.trim() ? color.trim() : null;

    try {
      if (c && pdfjsLib.AnnotationEditorParamsType) {
        annotationEditorUiManager.updateParams(pdfjsLib.AnnotationEditorParamsType.HIGHLIGHT_DEFAULT_COLOR, c);
      }
    } catch {
    }

    try {
      await annotationEditorUiManager.updateMode(pdfjsLib.AnnotationEditorType.HIGHLIGHT);
    } catch {
    }

    try {
      annotationEditorUiManager.highlightSelection("button");
    } catch {
      return null;
    } finally {
      try {
        await annotationEditorUiManager.updateMode(pdfjsLib.AnnotationEditorType.NONE);
      } catch {
      }
    }

    await nextTick();
    await new Promise((r) => setTimeout(r, 0));

    const after = getAnnotationEditorIdsForPage(pageIndex);
    const created = after.filter((x) => !before.has(x));
    return created.length ? created[created.length - 1] : null;
  };

  return {
    renderMode,
    loadPage,
    renderContent,
    textLayer,
    isLoading,
    error,
    currentPageData,
    getOutlineFlat,
    getPageText,
    searchText,
    ensureAnnotationEditorUi,
    setAnnotationEditorMode,
    saveDocumentBase64,
    getAnnotationEditorIdsForPage,
    removeAnnotationEditorById,
    highlightSelectionAsAnnotation,
  };
}
