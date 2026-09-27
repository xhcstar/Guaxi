/* ============================================================
   挂系 · 库网站前端引擎
   - 虚拟滚动：DOM 里永远只有可视窗口 + 缓冲，十万份不卡
   - 稀疏分页缓存：按需拉页，滚到哪拉到哪
   - 动画只作用于刚进入视口的少量行 (transform/opacity)
   API 契约（后端 actix 实现，见 library_site.rs）：
     GET  ./api/libraries                → { local:[Lib], peers:[Lib] }
     GET  ./api/list?lib&offset&limit&sort → { total, items:[Doc] }
     GET  ./api/search?q&offset&limit      → { total, items:[Doc] }
     GET  ./api/file?lib&path              → 原始文件（预览/下载）
     POST ./api/discover                   → 触发联邦发现
   Doc = { path,name,ext,size,mtime,lib,source,band,snippet? }
   ============================================================ */
(() => {
  "use strict";

  const ROW_H = 84;          // 行高:容纳 名称+描述/片段+标签行 三行,须与 CSS --row-h 一致
  const PAGE = 200;          // 每页条数
  const BUFFER = 8;          // 视口上下各多渲染的行数
  const TOKEN = new URLSearchParams(location.search).get("t") || "";
  // 宿主(Guaxi)内嵌时通过 ?theme= 传入主题;有则进入"跟随宿主"模式。
  const THEME_PARAM = new URLSearchParams(location.search).get("theme");
  const EMBEDDED = THEME_PARAM === "dark" || THEME_PARAM === "light";
  if (EMBEDDED) document.documentElement.dataset.theme = THEME_PARAM;

  const $ = (id) => document.getElementById(id);
  const el = {
    rows: $("rows"), spacer: $("vpSpacer"), viewport: $("viewport"),
    empty: $("vpEmpty"), loading: $("vpLoading"),
    localLibs: $("localLibs"), peerLibs: $("peerLibs"),
    q: $("q"), mastTitle: $("mastTitle"), mastKicker: $("mastKicker"),
    mastMeta: $("mastMeta"), tbMode: $("tbMode"), tbSort: $("tbSort"),
    tbScope: $("tbScope"),
    themeToggle: $("themeToggle"), discoverBtn: $("discoverBtn"),
  };

  const state = {
    lib: null, libName: "资料库", mode: "browse", sort: "mtime",
    query: "", total: 0, pages: new Map(), inflight: new Set(),
    firstRender: true, lastRange: [-1, -1], selected: -1,
    searchMode: "meta", // 检索匹配:meta(元数据) | fulltext(正文)
  };

  // —— API 层（带 token；失败时回退演示数据，便于离线预览设计稿） ——
  async function api(path, params) {
    const u = new URL("./api/" + path, location.href);
    if (params) for (const k in params) if (params[k] != null) u.searchParams.set(k, params[k]);
    if (TOKEN) u.searchParams.set("t", TOKEN);
    const r = await fetch(u, { headers: TOKEN ? { "X-Guaxi-Token": TOKEN } : {} });
    if (!r.ok) throw new Error("HTTP " + r.status);
    return r.json();
  }
  // —— 稀疏分页缓存：一行属于哪一页就拉哪一页，滚到哪拉到哪 ——
  function pageOf(i) { return Math.floor(i / PAGE); }

  async function ensurePage(p) {
    if (state.pages.has(p) || state.inflight.has(p)) return;
    state.inflight.add(p);
    try {
      const params = { offset: p * PAGE, limit: PAGE };
      let data;
      if (state.mode === "search") {
        data = await api("search", { ...params, q: state.query, lib: state.lib, mode: state.searchMode });
      } else {
        data = await api("list", { ...params, lib: state.lib, sort: state.sort });
      }
      state.total = data.total ?? (data.items || []).length;
      state.pages.set(p, data.items || []);
      el.spacer.style.height = state.total * ROW_H + "px";
      updateCount();
      render(true);
    } catch (e) {
      // 后端未就绪时退回演示数据，方便离线看设计稿
      if (state.pages.size === 0) loadDemo();
    } finally {
      state.inflight.delete(p);
      toggleLoading(false);
    }
  }

  function docAt(i) {
    const p = state.pages.get(pageOf(i));
    return p ? p[i % PAGE] : null;
  }

  // —— 虚拟滚动核心：只渲染 [start,end) 窗口 ——
  function render(force) {
    const scroll = el.viewport.scrollTop;
    const vh = el.viewport.clientHeight;
    let start = Math.max(0, Math.floor(scroll / ROW_H) - BUFFER);
    let end = Math.min(state.total, Math.ceil((scroll + vh) / ROW_H) + BUFFER);

    if (!force && start === state.lastRange[0] && end === state.lastRange[1]) return;
    state.lastRange = [start, end];

    // 预取当前窗口涉及的页
    for (let p = pageOf(start); p <= pageOf(Math.max(start, end - 1)); p++) ensurePage(p);

    const frag = document.createDocumentFragment();
    const seen = new Set();
    for (let i = start; i < end; i++) {
      const doc = docAt(i);
      let node = el.rows.querySelector(`[data-i="${i}"]`);
      if (!node) {
        node = doc ? buildRow(doc, i) : buildSkeleton(i);
        if (state.firstRender && doc) {
          node.classList.add("enter");
          node.style.animationDelay = (i - start) * 22 + "ms";
        }
        frag.appendChild(node);
      }
      seen.add(String(i));
    }
    el.rows.appendChild(frag);
    // 回收离开窗口的行
    for (const child of Array.from(el.rows.children)) {
      if (!seen.has(child.dataset.i)) child.remove();
    }
    if (state.firstRender && state.total) state.firstRender = false;

    el.empty.hidden = state.total !== 0;
  }

  function buildRow(doc, i) {
    const li = document.createElement("li");
    li.className = "row"; li.dataset.i = i;
    if (state.selected === i) li.classList.add("selected");
    li.style.top = i * ROW_H + "px";
    if (doc.band) li.style.setProperty("--band", doc.band);
    // 副行:检索命中片段优先,否则显示文件描述
    const sub = doc.snippet
      ? `<div class="row-snippet">${doc.snippet}</div>`
      : (doc.desc ? `<div class="row-desc">${escapeHtml(doc.desc)}</div>` : "");
    // 标签
    const tags = (doc.tags && doc.tags.length)
      ? `<span class="row-tags">${doc.tags.slice(0, 4).map((t) => `<span class="row-tag">${escapeHtml(t)}</span>`).join("")}</span>`
      : "";
    // 所属文件库 / 联邦来源
    const libTag = doc.library ? `<span class="row-lib">${escapeHtml(doc.library)}</span>` : "";
    const src = doc.source && doc.source !== "local"
      ? `<span class="row-src">@${escapeHtml(doc.source)}</span>` : "";
    li.innerHTML = `
      <span class="row-band"></span>
      <span class="row-ext">${escapeHtml((doc.ext || "·").slice(0, 4))}</span>
      <div class="row-main">
        <div class="row-name">${escapeHtml(doc.name)}</div>
        ${sub}
        <div class="row-tagline">${libTag}${tags}</div>
      </div>
      <div class="row-meta">
        <span class="rm-size">${fmtSize(doc.size)}</span>
        <span class="rm-time">${doc.time_text ? escapeHtml(doc.time_text) : fmtTime(doc.mtime)} ${src}</span>
      </div>
      <div class="row-actions">
        <button class="row-act" data-act="view" title="查看">查看</button>
        ${doc.note ? `<button class="row-act" data-act="note" title="摘录笔记">笔记</button>` : ""}
        <button class="row-act" data-act="dl" title="下载">下载</button>
      </div>`;
    li.querySelector('[data-act="view"]').addEventListener("click", (e) => {
      e.stopPropagation(); selectRow(i); openDoc(doc);
    });
    const noteBtn = li.querySelector('[data-act="note"]');
    if (noteBtn) noteBtn.addEventListener("click", (e) => { e.stopPropagation(); selectRow(i); openNote(doc); });
    li.querySelector('[data-act="dl"]').addEventListener("click", (e) => {
      e.stopPropagation(); downloadDoc(doc);
    });
    li.addEventListener("click", () => selectRow(i));
    li.addEventListener("dblclick", () => openDoc(doc));
    return li;
  }

  function buildSkeleton(i) {
    const li = document.createElement("li");
    li.className = "row skeleton"; li.dataset.i = i;
    li.style.top = i * ROW_H + "px";
    li.innerHTML = `<span class="row-band"></span><span class="row-ext"></span>
      <div class="row-main"><div class="row-name"></div></div>`;
    return li;
  }

  // 构造 /api/file 的 URL(可选 dl=1 触发下载)
  function fileUrl(doc, download) {
    const u = new URL("./api/file", location.href);
    u.searchParams.set("lib", doc.lib || state.lib || "");
    u.searchParams.set("path", doc.path);
    if (download) u.searchParams.set("dl", "1");
    if (TOKEN) u.searchParams.set("t", TOKEN);
    return u.toString();
  }

  // 查看:本机联邦来源无 import_file_id → 走浏览器/系统打开;本机文件库条目 → 请求宿主用本地阅读器打开。
  function openDoc(doc) {
    const isPeer = doc.source && doc.source !== "local";
    if (EMBEDDED && !isPeer && doc.import_file_id != null) {
      // Guaxi 内嵌:交给宿主用本地阅读器打开文件库条目。
      try {
        parent.postMessage({
          type: "guaxi-open-reader",
          library: doc.library || "",
          fileName: doc.name,
          fileType: doc.ftype || "",
          importFileId: doc.import_file_id,
        }, "*");
        return;
      } catch {}
    }
    const url = fileUrl(doc, false);
    if (EMBEDDED) {
      // 内嵌但无法用阅读器(联邦来源等):交给宿主用系统默认程序打开。
      try { parent.postMessage({ type: "guaxi-open-file", url, name: doc.name }, "*"); } catch {}
    } else {
      window.open(url, "_blank");
    }
  }

  // 打开摘录笔记(本机文件库条目才有)
  function openNote(doc) {
    if (!doc.note) return;
    const u = new URL("./api/file", location.href);
    u.searchParams.set("lib", doc.lib || state.lib || "");
    u.searchParams.set("path", doc.note);
    if (TOKEN) u.searchParams.set("t", TOKEN);
    const url = u.toString();
    if (EMBEDDED) {
      try { parent.postMessage({ type: "guaxi-open-file", url, name: doc.note_name || doc.name }, "*"); } catch {}
    } else {
      window.open(url, "_blank");
    }
  }

  // 下载:内嵌交给宿主(webview 会拦截 attachment 下载),浏览器直接触发。
  function downloadDoc(doc) {
    const url = fileUrl(doc, true);
    if (EMBEDDED) {
      try { parent.postMessage({ type: "guaxi-download", url, name: doc.name }, "*"); } catch {}
    } else {
      const a = document.createElement("a");
      a.href = url; a.download = doc.name || "";
      document.body.appendChild(a); a.click(); a.remove();
    }
  }

  function selectRow(i) {
    state.selected = i;
    el.rows.querySelectorAll(".row.selected").forEach((n) => n.classList.remove("selected"));
    const node = el.rows.querySelector(`[data-i="${i}"]`);
    if (node) node.classList.add("selected");
  }

  // —— 库切换 / 检索 ——
  function resetAndLoad() {
    state.pages.clear(); state.inflight.clear();
    state.total = 0; state.firstRender = true; state.lastRange = [-1, -1]; state.selected = -1;
    el.rows.innerHTML = ""; el.viewport.scrollTop = 0;
    el.spacer.style.height = "0px";
    toggleLoading(true);
    ensurePage(0);
  }

  function selectLib(lib, name, band) {
    state.lib = lib; state.libName = name || "资料库"; state.mode = "browse";
    el.mastTitle.textContent = state.libName;
    el.mastKicker.textContent = band ? "联邦来源" : "本机库";
    document.querySelectorAll(".lib-item").forEach((n) =>
      n.classList.toggle("is-active", n.dataset.lib === lib));
    setModeUI("browse");
    resetAndLoad();
  }

  let debounce;
  el.q.addEventListener("input", () => {
    clearTimeout(debounce);
    debounce = setTimeout(() => {
      state.query = el.q.value.trim();
      if (state.query) { state.mode = "search"; setModeUI("search"); el.mastKicker.textContent = "检索结果"; el.mastTitle.textContent = `“${state.query}”`; }
      else { state.mode = "browse"; setModeUI("browse"); el.mastTitle.textContent = state.libName; el.mastKicker.textContent = "本机库"; }
      resetAndLoad();
    }, 220);
  });

  function setModeUI(mode) {
    el.tbMode.querySelectorAll(".tb-seg").forEach((b) =>
      b.classList.toggle("is-on", b.dataset.mode === mode));
    // 检索模式才显示"元数据/正文"匹配范围切换
    if (el.tbScope) el.tbScope.hidden = mode !== "search";
  }
  el.tbMode.addEventListener("click", (e) => {
    const seg = e.target.closest(".tb-seg"); if (!seg) return;
    if (seg.dataset.mode === "search") el.q.focus();
    else { el.q.value = ""; state.query = ""; state.mode = "browse"; setModeUI("browse"); el.mastTitle.textContent = state.libName; resetAndLoad(); }
  });
  // 检索匹配范围:元数据(文件名/描述/标签) | 正文(全文索引)
  if (el.tbScope) el.tbScope.addEventListener("click", (e) => {
    const seg = e.target.closest(".tb-seg"); if (!seg) return;
    el.tbScope.querySelectorAll(".tb-seg").forEach((b) => b.classList.toggle("is-on", b === seg));
    state.searchMode = seg.dataset.search === "fulltext" ? "fulltext" : "meta";
    if (state.mode === "search" && state.query) resetAndLoad();
  });
  el.tbSort.addEventListener("click", (e) => {
    const seg = e.target.closest(".tb-seg"); if (!seg) return;
    el.tbSort.querySelectorAll(".tb-seg").forEach((b) => b.classList.toggle("is-on", b === seg));
    state.sort = seg.dataset.sort; resetAndLoad();
  });

  // —— 库列表 / 联邦发现 ——
  async function loadLibraries() {
    let data;
    try { data = await api("libraries"); }
    catch { data = DEMO_LIBS; }
    renderLibList(el.localLibs, data.local || [], false);
    renderLibList(el.peerLibs, data.peers || [], true);
    const first = (data.local || [])[0];
    if (first) selectLib(first.id, first.name, null);
    else loadDemo();
  }

  function renderLibList(ul, libs, isPeer) {
    ul.innerHTML = "";
    libs.forEach((lib) => {
      const li = document.createElement("li");
      li.className = "lib-item" + (isPeer ? " lib-peer" : "");
      li.dataset.lib = lib.id;
      if (lib.band) li.style.setProperty("--band", lib.band);
      li.innerHTML = `<span class="lib-name"${isPeer ? ` data-peer="${escapeHtml(lib.peer || "")}"` : ""}>${escapeHtml(lib.name)}</span>
        <span class="lib-count">${lib.count ?? ""}</span>`;
      li.addEventListener("click", () => selectLib(lib.id, lib.name, lib.band));
      ul.appendChild(li);
    });
  }

  el.discoverBtn.addEventListener("click", async () => {
    el.discoverBtn.textContent = "发现中…";
    try { await api("discover"); await loadLibraries(); }
    catch {}
    el.discoverBtn.textContent = "发现";
  });

  // —— 主题 ——
  // 内嵌模式(Guaxi 内 iframe，URL 带 ?theme=)跟随宿主主题，不用自己的开关/localStorage，
  // 避免站点主题与 Guaxi 界面各走各的、出现半亮半暗。独立浏览器打开时才用站点自己的开关。
  if (EMBEDDED) {
    document.documentElement.dataset.theme = THEME_PARAM;
    el.themeToggle.style.display = "none";
    // 宿主切换主题时通过 postMessage 通知,实时跟随。
    window.addEventListener("message", (e) => {
      const t = e.data && e.data.type === "guaxi-theme" ? e.data.theme : null;
      if (t === "dark" || t === "light") document.documentElement.dataset.theme = t;
    });
  } else {
    el.themeToggle.addEventListener("click", () => {
      const cur = document.documentElement.dataset.theme;
      document.documentElement.dataset.theme = cur === "dark" ? "light" : "dark";
      try { localStorage.setItem("guaxi-lib-theme", document.documentElement.dataset.theme); } catch {}
    });
    try {
      const saved = localStorage.getItem("guaxi-lib-theme");
      if (saved) document.documentElement.dataset.theme = saved;
    } catch {}
  }

  // —— 工具函数 ——
  function updateCount() {
    const c = el.mastMeta.querySelector(".mm-count");
    if (c) c.textContent = state.total.toLocaleString("zh-CN");
  }
  function toggleLoading(on) { el.loading.hidden = !on; }
  function escapeHtml(s) { return String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c])); }
  function fmtSize(b) {
    if (b == null) return "";
    const u = ["B", "KB", "MB", "GB"]; let i = 0; let n = b;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return (i ? n.toFixed(1) : n) + u[i];
  }
  function fmtTime(ts) {
    if (!ts) return "";
    const d = new Date(ts * 1000);
    const p = (x) => String(x).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }

  el.viewport.addEventListener("scroll", () => requestAnimationFrame(() => render(false)), { passive: true });
  window.addEventListener("resize", () => render(true));

  // —— 离线演示数据（后端未接通时看设计稿用） ——
  const DEMO_LIBS = {
    local: [
      { id: "kb", name: "知识库", count: 1284, band: "#1890ff" },
      { id: "repo", name: "文件库", count: 9612, band: "#c07a2b" },
    ],
    peers: [
      { id: "peer-a-kb", name: "书房知识库", count: 542, peer: "书房台式机", band: "#22863a" },
      { id: "peer-b-repo", name: "笔记本文件库", count: 3310, peer: "MacBook", band: "#a626a4" },
    ],
  };
  function loadDemo() {
    state.total = 100000;
    el.spacer.style.height = state.total * ROW_H + "px";
    updateCount();
    const exts = ["md", "pdf", "docx", "epub", "txt", "pptx"];
    const names = ["深度学习综述", "项目会议纪要", "季度财报", "论文精读笔记", "架构设计稿", "读书摘录"];
    const bands = ["#1890ff", "#c07a2b", "#22863a", "#a626a4"];
    for (let p = 0; p < 2; p++) {
      const items = [];
      for (let k = 0; k < PAGE; k++) {
        const i = p * PAGE + k;
        items.push({
          path: `demo/${i}.md`, name: `${names[i % names.length]} ${String(i).padStart(5, "0")}`,
          ext: exts[i % exts.length], size: 2048 + (i * 733) % 4_000_000,
          mtime: 1_750_000_000 - i * 3600, lib: "demo", source: i % 7 === 0 ? "书房台式机" : "local",
          band: bands[i % bands.length],
          snippet: i % 3 === 0 ? "……在<mark>检索</mark>时结合中文分词与 BM25 排序……" : "",
        });
      }
      state.pages.set(p, items);
    }
    toggleLoading(false);
    render(true);
  }

  // —— 启动 ——
  loadLibraries();
})();

