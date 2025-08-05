let selectionModeActive = false;
let selectedElements = []; // 存储已选元素

/**
 * 等待React/Vue应用完全渲染
 * 安全等待页面渲染（兼容SPA和非SPA）
 */
async function safeWaitForPageRender() {
  // 尝试检测SPA应用根节点
  const appRoot =
    document.getElementById("app") ||
    document.querySelector("[data-reactroot], [data-vue-app]");

  if (appRoot) {
    // SPA应用处理逻辑
    await new Promise((resolve) => {
      const observer = new MutationObserver(() => {
        if (
          appRoot.innerHTML.trim() &&
          appRoot.innerHTML.trim() !== "<!---->"
        ) {
          observer.disconnect();
          resolve();
        }
      });

      try {
        observer.observe(appRoot, {
          childList: true,
          subtree: true,
          attributes: true,
        });
      } catch (e) {
        console.warn("MutationObserver启动失败:", e);
        resolve();
      }

      setTimeout(() => {
        observer.disconnect();
        resolve();
      }, 5000);
    });
  } else {
    // 非SPA页面处理逻辑
    await new Promise((resolve) => {
      if (document.readyState === "complete") {
        resolve();
      } else {
        const onLoad = () => {
          window.removeEventListener("load", onLoad);
          resolve();
        };
        window.addEventListener("load", onLoad);
        setTimeout(resolve, 3000);
      }
    });
  }
}

/**
 * 捕获动态样式
 */
function captureDynamicStyles() {
  try {
    // 1. 处理样式表
    Array.from(document.styleSheets).forEach((sheet) => {
      try {
        if (sheet.href && sheet.href.includes(location.origin)) {
          const rules = Array.from(sheet.cssRules || []);
          rules.forEach((rule) => {
            try {
              if (rule.selectorText) {
                const style = document.createElement("style");
                style.textContent = rule.cssText;
                document.head.appendChild(style);
              }
            } catch (e) {
              console.warn("处理CSS规则失败:", e);
            }
          });
        }
      } catch (e) {
        console.warn("处理样式表失败:", e);
      }
    });

    // 2. 处理内联样式
    document.querySelectorAll("style").forEach((style) => {
      try {
        document.head.appendChild(style.cloneNode(true));
      } catch (e) {
        console.warn("克隆样式标签失败:", e);
      }
    });
  } catch (e) {
    console.warn("捕获样式时发生错误:", e);
  }
}

/**
 * 序列化React/Vue状态
 */
function serializeAppState() {
  const stateScript = document.createElement("script");
  stateScript.type = "application/json";
  stateScript.id = "guaxi-serialized-state";

  const state = {
    __NUXT__: window.__NUXT__,
    __NEXT_DATA__: window.__NEXT_DATA__,
    __REDUX_STATE__: window.__REDUX_STATE__,
    __APOLLO_STATE__: window.__APOLLO_STATE__,
    __REACT_CONTEXT__:
      document.querySelector("[data-reactroot]")?._reactRootContainer
        ?._internalRoot?.current,
  };

  stateScript.textContent = JSON.stringify(state);
  document.head.appendChild(stateScript);
}

/**
 * 等待所有动态内容加载完成
 */
async function waitForDynamicContent() {
  try {
    // 1. 滚动页面
    await scrollPageToTriggerLazyLoad();

    // 2. 等待图片
    await waitForImagesToLoad();

    // 3. 等待自定义内容
    await waitForCustomDynamicContent();

    // 4. 等待网络空闲
    await waitForNetworkIdle();
  } catch (e) {
    console.warn("等待动态内容时出错:", e);
    // 即使出错也继续执行
  }
}

/**
 * 滚动页面触发懒加载
 */
async function scrollPageToTriggerLazyLoad() {
  const scrollStep = 500;
  const scrollDelay = 300;
  let lastPosition = -1;
  let currentPosition = 0;

  window.scrollTo(0, 0);
  await new Promise((resolve) => setTimeout(resolve, scrollDelay));

  // 只向下滚动一次，减少对非SPA页面的影响
  lastPosition = window.scrollY;
  window.scrollBy(0, scrollStep);
  await new Promise((resolve) => setTimeout(resolve, scrollDelay));
  currentPosition = window.scrollY;

  window.scrollTo(0, 0);
}

async function waitForImagesToLoad() {
  const images = Array.from(document.images);
  await Promise.allSettled(
    images.map((img) => {
      return new Promise((resolve) => {
        if (img.complete) resolve();
        img.addEventListener("load", resolve, { once: true });
        img.addEventListener("error", resolve, { once: true });
      });
    })
  );
}

async function waitForCustomDynamicContent() {
  const dynamicSelectors = [
    "[data-src]",
    "[data-srcset]",
    '[data-loaded="false"]',
  ];

  dynamicSelectors.forEach((selector) => {
    document.querySelectorAll(selector).forEach((el) => {
      try {
        if (el.dataset.src) el.src = el.dataset.src;
        if (el.dataset.srcset) el.srcset = el.dataset.srcset;
      } catch (e) {
        console.warn("处理动态元素失败:", e);
      }
    });
  });

  await new Promise((resolve) => setTimeout(resolve, 300));
}

/**
 * 等待图片加载完成
 */
async function waitForImagesToLoad() {
  const images = Array.from(document.images);
  const promises = images.map((img) => {
    if (img.complete) return Promise.resolve();
    return new Promise((resolve) => {
      img.addEventListener("load", resolve);
      img.addEventListener("error", resolve);
    });
  });

  await Promise.all(promises);
  await new Promise((resolve) => setTimeout(resolve, 200));
}

/**
 * 等待自定义动态内容加载
 */
async function waitForCustomDynamicContent() {
  const dynamicSelectors = [
    '[data-loaded="false"]',
    ".lazy-load",
    ".async-content",
    "[data-dynamic]",
    "[data-src]",
    "[data-srcset]",
    ".is-loading",
    ".is-fetching",
  ];

  // 触发自定义元素的加载
  dynamicSelectors.forEach((selector) => {
    document.querySelectorAll(selector).forEach((el) => {
      if (el.dataset.src) el.src = el.dataset.src;
      if (el.dataset.srcset) el.srcset = el.dataset.srcset;
    });
  });

  // 等待一段时间让动态内容加载
  await new Promise((resolve) => setTimeout(resolve, 500));
}

/**
 * 等待网络空闲
 */
async function waitForNetworkIdle() {
  const idleTime = 500; // 500ms内没有新请求视为空闲
  let lastRequestTime = performance.now();
  let timer;

  // 监听fetch请求
  const originalFetch = window.fetch;
  window.fetch = function (...args) {
    lastRequestTime = performance.now();
    return originalFetch.apply(this, args);
  };

  return new Promise((resolve) => {
    function checkIdle() {
      const now = performance.now();
      if (now - lastRequestTime > idleTime) {
        window.fetch = originalFetch; // 恢复原fetch
        resolve();
      } else {
        timer = setTimeout(checkIdle, idleTime);
      }
    }

    timer = setTimeout(checkIdle, idleTime);
  });
}

/**
 * 处理克隆体中的动态元素
 */
async function processDynamicElementsInClone(clone) {
  // 处理图片的data-src属性
  clone.querySelectorAll("img[data-src]").forEach((img) => {
    if (img.dataset.src && !img.src) {
      img.src = img.dataset.src;
    }
  });

  // 处理iframe的data-src属性
  clone.querySelectorAll("iframe[data-src]").forEach((iframe) => {
    if (iframe.dataset.src && !iframe.src) {
      iframe.src = iframe.dataset.src;
    }
  });

  // 处理懒加载背景图
  clone.querySelectorAll("[data-bg]").forEach((el) => {
    if (el.dataset.bg && !el.style.backgroundImage) {
      el.style.backgroundImage = `url(${el.dataset.bg})`;
    }
  });

  // 移除懒加载相关的类名
  clone.querySelectorAll(".lazy-load, .lazy-loaded").forEach((el) => {
    el.classList.remove("lazy-load", "lazy-loaded");
  });
}

/**
 * 修改后的inlineResources函数
 */
async function inlineResources(element = document.documentElement) {
  // 1. 处理CSS资源
  const linkNodes = Array.from(
    element.querySelectorAll('link[rel="stylesheet"]')
  );
  await Promise.all(
    linkNodes.map(async (link) => {
      try {
        const response = await fetch(link.href, { credentials: "include" });
        const css = await response.text();
        const style = document.createElement("style");

        // 处理字体和图片引用
        const processedCSS = await processCSSUrls(css, link.href);

        style.textContent = processedCSS;
        link.replaceWith(style);
      } catch (e) {
        console.warn("样式内联失败:", link.href, e);
      }
    })
  );

  // 2. 处理JavaScript模块
  const scriptNodes = Array.from(element.querySelectorAll("script[src]"));
  await Promise.all(
    scriptNodes.map(async (script) => {
      // 保留关键运行时脚本
      if (script.src.includes("runtime") || script.src.includes("vendor")) {
        try {
          const response = await fetch(script.src);
          const js = await response.text();
          const newScript = document.createElement("script");
          newScript.textContent = js;
          script.replaceWith(newScript);
        } catch (e) {
          console.warn("脚本内联失败:", script.src, e);
        }
      } else {
        script.remove(); // 移除非关键脚本
      }
    })
  );

  // 3. 处理Webpack注入的样式
  const styleNodes = Array.from(element.querySelectorAll("style"));
  styleNodes.forEach((style) => {
    if (!style.textContent.includes("#app")) {
      style.remove(); // 移除非关键样式
    }
  });

  return element;
}

/**
 * 处理CSS中的URL引用
 */
async function processCSSUrls(css, baseUrl) {
  const urlRegex = /url\((?!['"]?(?:data|blob):)(['"]?)([^'")]*)\1\)/gi;
  const base = new URL(baseUrl);

  const processed = await Promise.all(
    Array.from(css.matchAll(urlRegex)).map(async (match) => {
      const [fullMatch, quote, url] = match;
      try {
        const absoluteUrl = new URL(url, base).href;
        const dataUrl = await fetchResourceAsDataURL(absoluteUrl);
        return `url(${quote}${dataUrl}${quote})`;
      } catch (e) {
        console.warn(`Failed to inline CSS resource: ${url}`, e);
        return fullMatch; // 失败时返回原URL
      }
    })
  );

  let i = 0;
  return css.replace(urlRegex, () => processed[i++]);
}

/**
 * 获取资源并转换为DataURL
 */
async function fetchResourceAsDataURL(url) {
  const response = await fetch(url, { credentials: "include" });
  if (!response.ok) throw new Error(`Failed to fetch ${url}`);

  const blob = await response.blob();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

/**
 * 内联媒体资源
 */
async function inlineMediaResources(element) {
  // 处理图片
  const images = Array.from(
    element.querySelectorAll('img[src]:not([src^="data:"])')
  );
  await Promise.all(
    images.map(async (img) => {
      try {
        const dataUrl = await fetchResourceAsDataURL(img.src);
        img.src = dataUrl;
      } catch (e) {
        console.warn(`Failed to inline image: ${img.src}`, e);
      }
    })
  );

  // 处理视频和音频
  const mediaElements = Array.from(
    element.querySelectorAll("video source, audio source")
  );
  await Promise.all(
    mediaElements.map(async (media) => {
      try {
        const dataUrl = await fetchResourceAsDataURL(media.src);
        media.src = dataUrl;
      } catch (e) {
        console.warn(`Failed to inline media: ${media.src}`, e);
      }
    })
  );
}

/**
 * 内联计算样式
 */
function inlineComputedStyles(element) {
  Array.from(element.querySelectorAll("*")).forEach((el) => {
    const computed = getComputedStyle(el);
    let style = "";
    for (let i = 0; i < computed.length; i++) {
      const prop = computed[i];
      if (!prop.startsWith("-")) {
        style += `${prop}:${computed.getPropertyValue(prop)};`;
      }
    }
    el.setAttribute("style", style);
  });
}

/**
 * 递归隐藏目标元素的同级节点，保留父级结构
 * @param {Element} el - 目标元素（在克隆体中）
 */
function hideSiblingsRecursive(el) {
  if (!el.parentElement) return;
  Array.from(el.parentElement.children).forEach((sib) => {
    if (sib !== el) {
      sib.style.display = "none";
    }
  });
  hideSiblingsRecursive(el.parentElement);
}

// 处理保存请求
async function handleSaveRequest(request) {
  console.log("[Guaxi] Handling save request:", request);

  try {
    if (request.mode === "fullPage") {
      return await saveFullPage();
    } else if (request.mode === "partial") {
      // 激活选择模式，等待用户选择
      return await activateSelectionMode();
    }
    return { success: false, error: "Unknown mode" };
  } catch (error) {
    console.error("[Guaxi] Save request failed:", error);
    return { success: false, error: error.message };
  }
}

// 保存完整页面
async function saveFullPage() {
  let clone;

  try {
    // 阶段1：等待基础加载
    await new Promise((resolve) => {
      if (document.readyState === "complete") {
        resolve();
      } else {
        const onLoad = () => {
          window.removeEventListener("load", onLoad);
          resolve();
        };
        window.addEventListener("load", onLoad);
        setTimeout(resolve, 10000);
      }
    });

    // 阶段2：等待动态内容（不中断流程）
    try {
      await waitForDynamicContent();
    } catch (e) {
      console.warn("等待动态内容时出错:", e);
    }

    // 阶段3：等待页面渲染（兼容SPA和非SPA）
    try {
      await safeWaitForPageRender();
    } catch (e) {
      console.warn("等待页面渲染时出错:", e);
    }

    // 阶段4：捕获样式（如果有）
    try {
      captureDynamicStyles();
    } catch (e) {
      console.warn("捕获样式时出错:", e);
    }

    // 阶段5：克隆文档
    clone = document.documentElement.cloneNode(true);

    // 阶段6：处理SPA内容（如果存在）
    const appRoot =
      document.getElementById("app") ||
      document.querySelector("[data-reactroot], [data-vue-app]");
    if (appRoot) {
      try {
        const cloneAppRoot =
          clone.querySelector("#app") ||
          clone.querySelector("[data-reactroot], [data-vue-app]");
        if (cloneAppRoot) {
          cloneAppRoot.innerHTML = appRoot.innerHTML;
        }
      } catch (e) {
        console.warn("处理SPA内容时出错:", e);
      }
    }

    // 阶段7：处理资源（不中断流程）
    try {
      await inlineResources(clone);
    } catch (e) {
      console.warn("资源内联时出错:", e);
    }
  } catch (mainError) {
    console.error("保存过程中发生主要错误:", mainError);
    if (!clone) {
      clone = document.documentElement.cloneNode(true);
    }
  }

  // 最终保存
  const data = {
    url: location.href,
    title: document.title,
    html: clone.outerHTML,
    timestamp: new Date().toISOString(),
    mode: "fullPage",
  };

  try {
    const result = await sendToBackend(data);
    showTooltip(result.success ? "页面保存成功!" : `错误: ${result.error}`);
    return result;
  } catch (e) {
    showTooltip(`保存失败: ${e.message}`, true);
    return { success: false, error: e.message };
  }
}
async function saveSelection(element) {
  if (!element) throw new Error("No element selected");

  // 等待动态内容加载
  await waitForDynamicContent();

  // 先移除所有高亮class
  document
    .querySelectorAll(".guaxi-selected-highlight, .guaxi-hover-highlight")
    .forEach((el) => {
      el.classList.remove("guaxi-selected-highlight", "guaxi-hover-highlight");
    });

  // 克隆整个页面
  const docClone = document.documentElement.cloneNode(true);

  // 获取选中元素选择器
  const selector = getUniqueSelector(element);

  // 在克隆体中查找对应元素
  const targetInClone = docClone.querySelector(selector);

  // 递归隐藏同级元素
  if (targetInClone) {
    hideSiblingsRecursive(targetInClone);
    targetInClone.style.display = "";
    targetInClone.scrollIntoView();
  }

  // 处理克隆体中的动态内容
  await processDynamicElementsInClone(docClone);

  // 内联资源
  const processed = await inlineResources(docClone);

  // 保留原<html>和<body>的属性
  const htmlAttrs = getAttrs(document.documentElement);
  const bodyAttrs = getAttrs(document.body);

  // 拼接完整HTML
  const headHtml = document.head.innerHTML;
  const html = `<!DOCTYPE html>
<html ${htmlAttrs}>
  <head>${headHtml}</head>
  <body ${bodyAttrs}>
    ${processed.querySelector("body").innerHTML}
  </body>
</html>`;

  const data = {
    url: window.location.href,
    title: document.title,
    html,
    timestamp: new Date().toISOString(),
    mode: "partial",
  };

  return await sendToBackend(data);
}

async function saveAllSelections() {
  if (!selectedElements.length) {
    showTooltip("No elements selected", true);
    return { success: false, error: "No elements selected" };
  }

  // 先移除所有高亮class，避免保存时带上
  document
    .querySelectorAll(".guaxi-selected-highlight, .guaxi-hover-highlight")
    .forEach((el) => {
      el.classList.remove("guaxi-selected-highlight", "guaxi-hover-highlight");
    });

  // 多选时，保留页面结构和样式，只显示所有选中元素
  // 1. 克隆整个页面
  const docClone = document.documentElement.cloneNode(true);

  // 2. 获取所有选中元素在原页面中的唯一选择器
  function getUniqueSelector(el) {
    if (el.id) return `#${el.id}`;
    let path = [];
    while (el && el.nodeType === 1 && el !== document.body) {
      let selector = el.nodeName.toLowerCase();
      if (el.className) selector += "." + [...el.classList].join(".");
      path.unshift(selector);
      el = el.parentElement;
    }
    return path.length ? path.join(" > ") : null;
  }
  const selectors = selectedElements.map(getUniqueSelector);

  // 3. 在克隆体中查找对应元素
  const targetsInClone = selectors
    .map((selector) => docClone.querySelector(selector))
    .filter(Boolean);

  // 4. 递归隐藏同级元素，只显示所有选中元素
  targetsInClone.forEach((target) => {
    hideSiblingsRecursive(target);
    target.style.display = "";
    target.scrollIntoView();
  });

  const data = {
    url: window.location.href,
    title: document.title,
    html: docClone.outerHTML,
    timestamp: new Date().toISOString(),
    mode: "partial",
  };

  const result = await sendToBackend(data);
  if (result.success) {
    selectedElements = [];
  }

  deactivateSelectionMode();
  return result;
}

async function sendToBackend(data) {
  try {
    const response = await fetch("http://localhost:8888/extension/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    });
    return await response.json();
  } catch (error) {
    return { success: false, error: error.message };
  }
}

// ==================== 选择模式 ====================
function deactivateSelectionMode() {
  selectionModeActive = false;
  document.body.style.cursor = "";
  document.removeEventListener("mouseover", handleMouseOver);
  document.querySelectorAll(".guaxi-hover-highlight").forEach((el) => {
    el.classList.remove("guaxi-hover-highlight");
  });
  document.querySelectorAll(".guaxi-selected-highlight").forEach((el) => {
    el.classList.remove("guaxi-selected-highlight");
  });
  const styleTag = document.getElementById("guaxi-highlight-styles");
  if (styleTag) styleTag.remove();
  removeExistingTooltips();
  document.removeEventListener("mousedown", handleAltClick, true);
  document.removeEventListener("keydown", handleKeyDown);
}

// 激活选择模式
async function activateSelectionMode() {
  console.log("Selection mode activated");
  if (selectionModeActive) return;
  selectionModeActive = true;

  // 移除旧的监听器（如果存在）
  document.removeEventListener("mousedown", window.handleAltClick, true);
  document.removeEventListener("keydown", window.handleKeyDown);

  addHighlightStyles();
  document.body.style.cursor = "pointer";

  document.addEventListener("mouseover", handleMouseOver);

  window.handleAltClick = (e) => {
    console.log("Alt click event triggered");
    if (!selectionModeActive) return;
    if (e.altKey && e.button === 0) {
      e.preventDefault();
      e.stopImmediatePropagation();
      e.stopPropagation();

      const element = findSelectableElement(e.target);
      if (element && !selectedElements.includes(element)) {
        highlightElement(element);
        selectedElements.push(element);
        showTooltip("已选中一个模块，可继续选中其它模块", false);
      }
    }
  };

  window.handleKeyDown = (e) => {
    if (e.key === "Escape") {
      deactivateSelectionMode();
      showTooltip("已退出选中模式", false);
    }
  };

  document.addEventListener("mousedown", window.handleAltClick, true);
  document.addEventListener("keydown", window.handleKeyDown);

  showTooltip("按住 Alt 并点击模块可选中，ESC 退出", false);
}

// 高亮选中元素
function highlightElement(element) {
  document.querySelectorAll(".guaxi-selected-highlight").forEach((el) => {
    el.classList.remove("guaxi-selected-highlight");
  });
  element.classList.add("guaxi-selected-highlight");
}

// 鼠标悬停处理
function handleMouseOver(e) {
  if (!selectionModeActive) return;
  document.querySelectorAll(".guaxi-hover-highlight").forEach((el) => {
    el.classList.remove("guaxi-hover-highlight");
  });
  const element = findSelectableElement(e.target);
  if (element) {
    element.classList.add("guaxi-hover-highlight");
  }
}

// 查找可选择的元素
function findSelectableElement(startElement) {
  let element = startElement;
  while (element && element !== document.body) {
    if (isSelectableElement(element)) {
      return element;
    }
    element = element.parentElement;
  }
  return null;
}

// 判断元素是否可选
function isSelectableElement(element) {
  return (
    element.textContent.trim().length > 0 &&
    element.offsetWidth > 0 &&
    element.offsetHeight > 0 &&
    getComputedStyle(element).visibility !== "hidden" &&
    getComputedStyle(element).display !== "none" &&
    !element.classList.contains("guaxi-tooltip")
  );
}

// 添加高亮样式
function addHighlightStyles() {
  const style = document.createElement("style");
  style.id = "guaxi-highlight-styles";
  style.textContent = `
    .guaxi-hover-highlight {
      background-color: rgba(33, 150, 243, 0.2) !important;
      outline: 2px dashed #2196F3 !important;
      cursor: pointer !important;
    }
    .guaxi-selected-highlight {
      background-color: rgba(33, 150, 243, 0.4) !important;
      outline: 2px solid #1976D2 !important;
    }
  `;
  document.head.appendChild(style);
}

// 显示工具提示
function showTooltip(message, isError = false) {
  removeExistingTooltips();

  const tooltip = document.createElement("div");
  tooltip.className = "guaxi-tooltip";
  tooltip.style.cssText = `
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    padding: 10px 20px;
    border-radius: 4px;
    z-index: 999999;
    background-color: ${isError ? "#f44336" : "#333"};
    color: white;
    font-family: sans-serif;
    box-shadow: 0 2px 10px rgba(0,0,0,0.2);
    max-width: 80vw;
    text-align: center;
    pointer-events: none;
  `;
  tooltip.textContent = message;
  document.body.appendChild(tooltip);

  setTimeout(() => {
    tooltip.style.transition = "opacity 0.5s ease";
    tooltip.style.opacity = "0";
    setTimeout(() => tooltip.remove(), 500);
  }, 2000);
}

// 移除现有提示
function removeExistingTooltips() {
  document.querySelectorAll(".guaxi-tooltip").forEach((t) => t.remove());
}

// ==================== 样式与资源处理 ====================
/**
 * 内联页面所有样式和脚本资源，尽量还原浏览器效果
 * @param {HTMLElement} element - 需要处理的根节点（通常是克隆的document.documentElement）
 */
async function inlineResources(element = document.documentElement) {
  // 1. 内联所有 <link rel="stylesheet">（包括动态注入的）
  const linkNodes = Array.from(
    element.querySelectorAll('link[rel="stylesheet"]')
  );
  await Promise.all(
    linkNodes.map(async (link) => {
      const href = link.href;
      if (!href) return;
      try {
        const resp = await fetch(href, { credentials: "include" });
        if (resp.ok) {
          const css = await resp.text();
          const style = document.createElement("style");
          style.textContent = `/* Inlined: ${href} */\n${css}`;
          link.parentNode.replaceChild(style, link);
        }
      } catch (e) {
        // fetch 失败则保留原link
        console.warn("Failed to inline stylesheet:", href, e);
      }
    })
  );

  // 2. 保留所有 <style> 标签（包括动态注入的）
  // 克隆体已包含所有 <style>，无需特殊处理

  // 3. 内联所有 <script src="">
  const scriptNodes = Array.from(element.querySelectorAll("script[src]"));
  await Promise.all(
    scriptNodes.map(async (script) => {
      const src = script.src;
      if (!src) return;
      try {
        const resp = await fetch(src, { credentials: "include" });
        if (resp.ok) {
          const js = await resp.text();
          const inlineScript = document.createElement("script");
          inlineScript.textContent = `/* Inlined: ${src} */\n${js}`;
          // 保留原属性（如type等）
          for (const attr of script.attributes) {
            if (attr.name !== "src")
              inlineScript.setAttribute(attr.name, attr.value);
          }
          script.parentNode.replaceChild(inlineScript, script);
        }
      } catch (e) {
        // fetch 失败则保留原script
        console.warn("Failed to inline script:", src, e);
      }
    })
  );

  // 4. 内联同源样式表（styleSheets），包括动态注入的 <style>
  // 注意：这里遍历克隆体的 <style>，而不是原页面的 document.styleSheets
  const head = element.querySelector("head");
  if (head) {
    const styleNodes = Array.from(head.querySelectorAll("style"));
    styleNodes.forEach((styleNode) => {
      // 可选：处理 @import 语句
      // 这里假设 <style> 已经包含了动态注入的内容
    });
  }

  // 5. 内联计算样式（可选，若要最大还原视觉效果）
  Array.from(element.querySelectorAll("*")).forEach((el) => {
    const computed = getComputedStyle(el);
    let style = "";
    for (let i = 0; i < computed.length; i++) {
      const prop = computed[i];
      if (!prop.startsWith("-")) {
        style += `${prop}:${computed.getPropertyValue(prop)};`;
      }
    }
    el.setAttribute("style", style);
  });

  return element;
}

// ==================== 消息通信 ====================

// 监听来自后台的消息
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "saveContent") {
    handleSaveRequest(request).then(sendResponse);
    return true;
  }
  if (request.action === "startSelectionMode") {
    activateSelectionMode();
    sendResponse({ success: true });
    return true;
  }
  if (request.action === "saveAllSelections") {
    saveAllSelections().then(sendResponse);
    return true;
  }
  if (request.action === "exitSelectionMode") {
    deactivateSelectionMode();
    sendResponse({ success: true });
    return true;
  }
  return false;
});
