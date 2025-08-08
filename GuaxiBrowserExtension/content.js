let selectionModeActive = false;
let selectedElements = []; // 存储已选元素

// 定义需要特殊处理的域名列表
const SPECIAL_DOMAINS = [
  "github.githubassets.com",
  "www.gstatic.com",
  "fonts.googleapis.com",
  "fonts.gstatic.com",
];
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
 * 改进的captureDynamicStyles函数
 * 增强对静态页面的样式捕获能力
 */
function captureDynamicStyles() {
  try {
    // 1. 捕获所有样式表（包括跨域样式表）
    Array.from(document.styleSheets).forEach((sheet) => {
      try {
        let cssRules = [];
        try {
          // 尝试获取所有CSS规则（可能因跨域限制失败）
          cssRules = Array.from(sheet.cssRules || []);
        } catch (e) {
          // 跨域样式表处理：通过AJAX获取样式内容
          if (sheet.href) {
            fetch(sheet.href, { credentials: "omit" })
              .then((res) => res.text())
              .then((css) => {
                const style = document.createElement("style");
                style.textContent = css;
                document.head.appendChild(style);
              })
              .catch((e) => console.warn("获取跨域样式失败:", sheet.href, e));
          }
          return;
        }

        // 处理获取到的规则
        cssRules.forEach((rule) => {
          try {
            if (
              rule.type === CSSRule.STYLE_RULE ||
              rule.type === CSSRule.MEDIA_RULE ||
              rule.type === CSSRule.SUPPORTS_RULE
            ) {
              const style = document.createElement("style");
              style.textContent = rule.cssText;
              document.head.appendChild(style);
            }
          } catch (e) {
            console.warn("处理CSS规则失败:", e);
          }
        });
      } catch (e) {
        console.warn("处理样式表失败:", e);
      }
    });

    // 2. 捕获内联样式和计算样式
    document.querySelectorAll("*").forEach((el) => {
      try {
        // 捕获内联样式
        if (el.hasAttribute("style")) {
          const style = document.createElement("style");
          style.textContent = `[data-capture-id="${
            el.id || el.className
          }"] { ${el.getAttribute("style")} }`;
          document.head.appendChild(style);
        }

        // 捕获计算样式（关键元素）
        if (
          [
            "body",
            "html",
            "main",
            "header",
            "footer",
            "article",
            "div",
            "section",
          ].includes(el.tagName.toLowerCase())
        ) {
          const computed = getComputedStyle(el);
          let styleText = "";
          for (let i = 0; i < computed.length; i++) {
            const prop = computed[i];
            if (
              prop.startsWith("background") ||
              prop.startsWith("border") ||
              prop.startsWith("font") ||
              prop.startsWith("padding") ||
              prop.startsWith("margin")
            ) {
              styleText += `${prop}: ${computed.getPropertyValue(prop)};`;
            }
          }
          if (styleText) {
            const style = document.createElement("style");
            style.textContent = `[data-capture-id="${
              el.id || el.className
            }"] { ${styleText} }`;
            document.head.appendChild(style);
          }
        }
      } catch (e) {
        console.warn("捕获元素样式失败:", e);
      }
    });

    // 3. 捕获特殊样式（伪元素、媒体查询等）
    const extraStyles = `
      /* 捕获伪元素样式 */
      body::before, body::after, 
      div::before, div::after,
      main::before, main::after {
        ${getComputedStyle(document.body, "::before").cssText}
      }
      
      /* 捕获媒体查询 */
      @media (max-width: 768px) {
        body {
          ${getComputedStyle(document.body).cssText}
        }
      }
    `;
    const style = document.createElement("style");
    style.textContent = extraStyles;
    document.head.appendChild(style);
  } catch (e) {
    console.warn("捕获样式时发生错误:", e);
  }
}

/**
 * 改进的cloneDocument函数
 * 更好地保留静态页面的结构和样式
 */
function cloneDocument() {
  const clone = document.documentElement.cloneNode(true);

  // 1. 特殊处理GitHub等常见网站
  if (window.location.hostname.includes("github.com")) {
    // 保留GitHub特有的样式和结构
    const githubStyles = document.querySelectorAll(
      'head style, head link[rel="stylesheet"]'
    );
    githubStyles.forEach((style) => {
      clone.querySelector("head").appendChild(style.cloneNode(true));
    });

    // 保留GitHub的页面结构
    const mainContent = document.querySelector(".repository-content");
    if (mainContent) {
      clone.querySelector(".repository-content").innerHTML =
        mainContent.innerHTML;
    }
  }

  // 2. 保留body的所有属性
  const bodyAttrs = document.body.attributes;
  const cloneBody = clone.querySelector("body");
  Array.from(bodyAttrs).forEach((attr) => {
    cloneBody.setAttribute(attr.name, attr.value);
  });

  // 3. 保留关键meta标签
  const metaTags = document.querySelectorAll(
    'meta[name^="viewport"], meta[charset]'
  );
  metaTags.forEach((meta) => {
    clone.querySelector("head").appendChild(meta.cloneNode(true));
  });

  return clone;
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
 * 改进的inlinePageResources函数
 * 针对静态页面优化资源内联
 */
async function inlinePageResources(element = document.documentElement) {
  // 1. 内联所有CSS资源（增强对静态页面的支持）
  const linkNodes = Array.from(
    element.querySelectorAll('link[rel="stylesheet"]')
  );
  await Promise.all(
    linkNodes.map(async (link) => {
      try {
        if (!link.href) return;

        // 特殊处理常见CDN资源
        const isSpecialDomain = SPECIAL_DOMAINS.some((domain) =>
          link.href.includes(domain)
        );
        const fetchOptions = {
          credentials: isSpecialDomain ? "omit" : "include",
        };

        const response = await fetch(link.href, fetchOptions);
        if (!response.ok) return;

        let css = await response.text();

        // 修复CSS中的相对路径
        const baseUrl = new URL(link.href);
        css = css.replace(
          /url\((?!['"]?(?:data|http))['"]?([^'")]*)['"]?\)/g,
          (match, p1) => {
            return `url('${new URL(p1, baseUrl).href}')`;
          }
        );

        const style = document.createElement("style");
        style.textContent = css;
        link.replaceWith(style);
      } catch (e) {
        console.warn(`样式内联失败: ${link.href}`, e);
      }
    })
  );

  // 2. 内联关键图片和背景
  await inlineCriticalAssets(element);

  // 3. 保留页面结构的关键脚本
  const scriptNodes = Array.from(element.querySelectorAll("script[src]"));
  await Promise.all(
    scriptNodes.map(async (script) => {
      try {
        if (!script.src) return;

        // 只内联关键脚本（如布局相关）
        const isCritical =
          script.src.includes("main") ||
          script.src.includes("app") ||
          script.src.includes("runtime") ||
          script.getAttribute("type") === "application/json";

        if (isCritical) {
          const response = await fetch(script.src, { credentials: "include" });
          if (!response.ok) return;

          const js = await response.text();
          const newScript = document.createElement("script");
          newScript.textContent = js;

          // 保留原属性
          Array.from(script.attributes).forEach((attr) => {
            if (attr.name !== "src")
              newScript.setAttribute(attr.name, attr.value);
          });

          script.replaceWith(newScript);
        } else {
          script.remove();
        }
      } catch (e) {
        console.warn(`脚本内联失败: ${script.src}`, e);
      }
    })
  );

  return element;
}

/**
 * 内联关键静态资源（图片、背景等）
 */
async function inlineCriticalAssets(element) {
  // 1. 内联视口内图片
  const images = Array.from(element.querySelectorAll("img")).filter((img) => {
    const rect = img.getBoundingClientRect();
    return rect.top < window.innerHeight * 2;
  });

  await Promise.all(
    images.map(async (img) => {
      try {
        if (!img.src || img.src.startsWith("data:")) return;

        const response = await fetch(img.src, { credentials: "include" });
        if (!response.ok) return;

        const blob = await response.blob();
        const dataUrl = await new Promise((resolve) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result);
          reader.readAsDataURL(blob);
        });

        img.src = dataUrl;
      } catch (e) {
        console.warn("图片内联失败:", img.src, e);
      }
    })
  );

  // 2. 内联关键背景图
  const elementsWithBg = Array.from(element.querySelectorAll("*")).filter(
    (el) => {
      const style = getComputedStyle(el);
      return style.backgroundImage && !style.backgroundImage.includes("none");
    }
  );

  await Promise.all(
    elementsWithBg.map(async (el) => {
      try {
        const style = getComputedStyle(el);
        const bgUrl = style.backgroundImage
          .replace(/^url\(['"]?/, "")
          .replace(/['"]?\)$/, "");

        if (!bgUrl.startsWith("http")) return;

        const response = await fetch(bgUrl, { credentials: "include" });
        if (!response.ok) return;

        const blob = await response.blob();
        const dataUrl = await new Promise((resolve) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result);
          reader.readAsDataURL(blob);
        });

        el.style.backgroundImage = `url(${dataUrl})`;
      } catch (e) {
        console.warn("背景图内联失败:", e);
      }
    })
  );
}

/**
 * 针对特定网站的优化处理
 */
function optimizeForSpecificSites(clone) {
  const hostname = window.location.hostname;

  // GitHub优化
  if (hostname.includes("github.com")) {
    // 保留GitHub的导航和文件结构
    const header = document.querySelector("header");
    const repoNav = document.querySelector(
      ".repository-content .file-navigation"
    );
    const fileContent = document.querySelector(".repository-content .Box");

    if (header) clone.querySelector("header").innerHTML = header.innerHTML;
    if (repoNav)
      clone.querySelector(".file-navigation").innerHTML = repoNav.innerHTML;
    if (fileContent)
      clone.querySelector(".Box").innerHTML = fileContent.innerHTML;

    // 保留GitHub的样式
    const githubStyles = document.querySelectorAll(
      'head style, head link[rel="stylesheet"]'
    );
    githubStyles.forEach((style) => {
      clone.querySelector("head").appendChild(style.cloneNode(true));
    });
  }

  // Google搜索结果优化
  if (hostname.includes("google.com")) {
    // 保留搜索结果结构
    const main = document.querySelector("#main");
    if (main) clone.querySelector("#main").innerHTML = main.innerHTML;

    // 保留Google的样式
    const googleStyles = document.querySelectorAll(
      'head style, head link[rel="stylesheet"]'
    );
    googleStyles.forEach((style) => {
      clone.querySelector("head").appendChild(style.cloneNode(true));
    });
  }

  // 通用博客优化
  if (document.querySelector("article.post, article.blog-post")) {
    // 保留文章主体和样式
    const article = document.querySelector("article");
    if (article) clone.querySelector("article").innerHTML = article.innerHTML;

    // 保留代码高亮样式
    const codeStyles = document.querySelectorAll(
      'head link[href*="highlight"], head style'
    );
    codeStyles.forEach((style) => {
      clone.querySelector("head").appendChild(style.cloneNode(true));
    });
  }

  return clone;
}
/**
 * 改进的processCSSUrls函数
 */
async function processCSSUrls(css, baseUrl) {
  const urlRegex = /url\((?!['"]?(?:data|blob):)(['"]?)([^'")]*)\1\)/gi;

  try {
    const base = new URL(baseUrl);
    const processed = await Promise.all(
      Array.from(css.matchAll(urlRegex)).map(async (match) => {
        const [fullMatch, quote, url] = match;
        try {
          const absoluteUrl = new URL(url, base).href;

          // 自动判断是否第三方资源
          const isThirdParty = !absoluteUrl.includes(location.origin);
          const fetchOptions = {
            credentials: isThirdParty ? "omit" : "include",
          };

          const dataUrl = await fetchResourceAsDataURL(
            absoluteUrl,
            fetchOptions
          );
          return `url(${quote}${dataUrl}${quote})`;
        } catch (e) {
          console.warn(`CSS资源内联失败: ${url}`, e);
          return fullMatch; // 失败时返回原URL
        }
      })
    );

    let i = 0;
    return css.replace(urlRegex, () => processed[i++]);
  } catch (e) {
    console.warn(`处理CSS URL失败: ${baseUrl}`, e);
    return css; // 失败时返回原始CSS
  }
}

/**
 * 获取资源并转换为DataURL
 */
async function fetchResourceAsDataURL(url, options = {}) {
  try {
    const isSpecialDomain = SPECIAL_DOMAINS.some((domain) =>
      url.includes(domain)
    );

    const fetchOptions = {
      credentials: isSpecialDomain ? "omit" : "include",
      ...options,
    };

    const response = await fetch(url, fetchOptions);

    if (!response.ok) {
      throw new Error(`HTTP错误: ${response.status}`);
    }

    // 检查内容类型
    const contentType = response.headers.get("content-type");
    if (!contentType) {
      throw new Error("缺少Content-Type头");
    }

    const blob = await response.blob();
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result);
      reader.onerror = () => reject(new Error("文件读取失败"));
      reader.readAsDataURL(blob);
    });
  } catch (e) {
    console.warn(`获取DataURL失败: ${url}`, e);
    throw e;
  }
}

/**
 * 内联媒体资源
 */
/**
 * 改进的inlineMediaResources函数
 */
async function inlineMediaResources(element) {
  // 处理图片 - 只处理视口内或关键区域的图片
  const images = Array.from(
    element.querySelectorAll('img[src]:not([src^="data:"])')
  ).filter((img) => {
    const rect = img.getBoundingClientRect();
    return (
      rect.width > 50 &&
      rect.height > 50 && // 尺寸较大的图片
      (rect.top < window.innerHeight * 2 || // 视口内或下方不远
        img.classList.contains("logo") || // 或者是logo
        img.id === "app-icon") // 或者是应用图标
    );
  });

  await Promise.all(
    images.map(async (img) => {
      try {
        const isThirdParty = !img.src.includes(location.origin);
        const fetchOptions = {
          credentials: isThirdParty ? "omit" : "include",
        };

        const dataUrl = await fetchResourceAsDataURL(img.src, fetchOptions);
        img.src = dataUrl;
      } catch (e) {
        console.warn(`内联图片失败: ${img.src}`, e);
      }
    })
  );

  // 处理关键背景图
  const elementsWithBg = Array.from(
    element.querySelectorAll('[style*="background-image"]')
  ).filter((el) => {
    const style = getComputedStyle(el);
    return (
      style.backgroundImage &&
      style.backgroundImage !== "none" &&
      !style.backgroundImage.includes("data:") &&
      el.getBoundingClientRect().width > 100
    );
  });

  await Promise.all(
    elementsWithBg.map(async (el) => {
      try {
        const style = getComputedStyle(el);
        const bgUrl = style.backgroundImage
          .replace(/^url\(["']?/, "")
          .replace(/["']?\)$/, "");

        if (!bgUrl.startsWith("http")) return;

        const isThirdParty = !bgUrl.includes(location.origin);
        const fetchOptions = {
          credentials: isThirdParty ? "omit" : "include",
        };

        const dataUrl = await fetchResourceAsDataURL(bgUrl, fetchOptions);
        el.style.backgroundImage = `url(${dataUrl})`;
      } catch (e) {
        console.warn(`内联背景图失败`, e);
      }
    })
  );

  // 处理视频和音频源
  const mediaElements = Array.from(
    element.querySelectorAll("video source, audio source")
  );
  await Promise.all(
    mediaElements.map(async (media) => {
      try {
        if (!media.src || media.src.startsWith("data:")) return;

        const isThirdParty = !media.src.includes(location.origin);
        const fetchOptions = {
          credentials: isThirdParty ? "omit" : "include",
        };

        const dataUrl = await fetchResourceAsDataURL(media.src, fetchOptions);
        media.src = dataUrl;
      } catch (e) {
        console.warn(`内联媒体源失败: ${media.src}`, e);
      }
    })
  );
}

/**
 * 内联计算样式
 */
function inlineComputedStyles(element) {
  // 只处理关键元素，减少不必要的处理
  const criticalSelectors = [
    "#app",
    ".app",
    "#root",
    ".root",
    "body",
    "html",
    "main",
    ".main",
    "header",
    ".header",
    "footer",
    ".footer",
  ];

  criticalSelectors.forEach((selector) => {
    element.querySelectorAll(selector).forEach((el) => {
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
  console.log("[Guaxi] Current selection mode state:", selectionModeActive);
  console.log("[Guaxi] Selected elements count:", selectedElements.length);

  try {
    if (request.mode === "fullPage") {
      console.log("[Guaxi] Starting full page save process");
      return await saveFullPage();
    } else if (request.mode === "partial") {
      console.log("[Guaxi] Starting partial save/selection process");
      return await activateSelectionMode();
    }
    console.warn("[Guaxi] Unknown mode requested:", request.mode);
    return { success: false, error: "Unknown mode" };
  } catch (error) {
    console.error("[Guaxi] Save request failed:", error);
    return { success: false, error: error.message };
  }
}

/**
 * 改进的saveFullPage函数
 * 针对静态页面优化的完整保存流程
 */
async function saveFullPage() {
  let clone;

  try {
    // 1. 等待页面完全加载
    await safeWaitForPageRender();

    // 2. 捕获动态样式
    captureDynamicStyles();

    // 3. 克隆文档
    clone = cloneDocument();

    // 4. 针对特定网站优化
    clone = optimizeForSpecificSites(clone);

    // 5. 内联资源
    await inlinePageResources(clone);
    await inlineCriticalAssets(clone);

    // 6. 内联计算样式（关键元素）
    inlineComputedStyles(clone);

    // 7. 处理动态内容
    await processDynamicElementsInClone(clone);
  } catch (error) {
    console.error("保存过程中出错:", error);
    if (!clone) clone = document.documentElement.cloneNode(true);
  }

  // 准备最终数据
  const data = {
    url: location.href,
    title: document.title,
    html: clone.outerHTML,
    timestamp: new Date().toISOString(),
    mode: "fullPage",
  };

  try {
    const result = await sendToBackend(data);
    showTooltip(result.success ? "页面保存成功!" : `错误: ${result.message}`);
    return result;
  } catch (error) {
    showTooltip(`保存失败: ${error.message}`, true);
    throw error;
  }
}

/**
 * 改进的saveSelection函数
 */
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

  // 内联资源 - 只内联必要资源
  await inlinePageResources(docClone);
  await inlineMediaResources(docClone);

  // 保留原<html>和<body>的属性
  const htmlAttrs = getAttrs(document.documentElement);
  const bodyAttrs = getAttrs(document.body);

  // 拼接完整HTML
  const headHtml = document.head.innerHTML;
  const html = `<!DOCTYPE html>
<html ${htmlAttrs}>
  <head>${headHtml}</head>
  <body ${bodyAttrs}>
    ${docClone.querySelector("body").innerHTML}
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

/**
 * 改进的saveAllSelections函数
 */
async function saveAllSelections() {
  if (!selectedElements.length) {
    showTooltip("No elements selected", true);
    return { success: false, error: "No elements selected" };
  }

  // 先移除所有高亮class
  document
    .querySelectorAll(".guaxi-selected-highlight, .guaxi-hover-highlight")
    .forEach((el) => {
      el.classList.remove("guaxi-selected-highlight", "guaxi-hover-highlight");
    });

  // 克隆整个页面
  const docClone = document.documentElement.cloneNode(true);

  // 获取所有选中元素的选择器
  const selectors = selectedElements.map((el) => {
    if (el.id) return `#${el.id}`;

    const path = [];
    while (el && el.nodeType === 1 && el !== document.body) {
      let selector = el.nodeName.toLowerCase();
      if (el.className) selector += `.${[...el.classList].join(".")}`;
      path.unshift(selector);
      el = el.parentElement;
    }
    return path.join(" > ");
  });

  // 在克隆体中查找对应元素
  const targetsInClone = selectors
    .map((selector) => docClone.querySelector(selector))
    .filter(Boolean);

  // 递归隐藏同级元素，只显示所有选中元素
  targetsInClone.forEach((target) => {
    hideSiblingsRecursive(target);
    target.style.display = "";
  });

  // 处理动态内容
  await processDynamicElementsInClone(docClone);

  // 内联必要资源
  await inlinePageResources(docClone);

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
    // 1. 验证数据格式
    const validationErrors = validateRequestData(data);
    if (validationErrors) {
      const errorMsg = "无效的请求数据: " + validationErrors.join(", ");
      showTooltip(errorMsg, true);
      throw new Error(errorMsg);
    }

    // 2. 准备请求数据
    const requestData = {
      url: data.url,
      title: data.title,
      html: data.html,
      timestamp: data.timestamp || new Date().toISOString(),
      mode: data.mode || "fullPage",
    };

    console.log("发送数据到后端:", {
      url: requestData.url,
      title: requestData.title,
      htmlLength: formatBytes(requestData.html.length),
      mode: requestData.mode,
    });

    // 显示上传中提示
    showTooltip("正在上传数据...");

    // 3. 根据内容大小选择传输方式
    let result;
    if (requestData.html.length <= 2 * 1024 * 1024) {
      // 小于2MB直接发送
      result = await sendNormalRequest(requestData);
    } else {
      // 大文件分块上传
      result = await uploadInChunks(requestData);
    }

    // 检查结果并显示相应提示
    if (result && result.success) {
      showTooltip("保存成功！");
    } else {
      const errorMsg = result?.message || "保存失败，未知错误";
      showTooltip(errorMsg, true);
      throw new Error(errorMsg);
    }

    return result;
  } catch (error) {
    console.error("上传失败:", error);
    showTooltip(`上传失败: ${error.message}`, true);
    throw error; // 继续向上抛出错误以便调用方处理
  }
}

async function uploadInChunks(data) {
  const CHUNK_SIZE = 4 * 1024 * 1024; // 4M分块
  const fileId = generateFileId();
  const htmlBytes = new TextEncoder().encode(data.html);
  const totalChunks = Math.ceil(htmlBytes.length / CHUNK_SIZE);

  console.log(`开始分块上传 (共 ${totalChunks} 块)`);
  showTooltip(`开始上传 (共 ${totalChunks} 块)`);

  // 1. 发送元数据
  try {
    const metaResponse = await fetch(
      "http://localhost:8888/extension/save_meta",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          url: data.url,
          title: data.title,
          timestamp: data.timestamp,
          mode: data.mode,
          fileId,
          totalChunks,
          originalLength: htmlBytes.length,
        }),
      }
    );

    if (!metaResponse.ok) {
      throw new Error(`元数据上传失败: ${metaResponse.status}`);
    }
  } catch (error) {
    showTooltip("元数据上传失败", true);
    throw error;
  }

  // 2. 分块上传
  for (let i = 0; i < totalChunks; i++) {
    const chunkStart = i * CHUNK_SIZE;
    const chunkEnd = Math.min((i + 1) * CHUNK_SIZE, htmlBytes.length);
    const chunk = htmlBytes.slice(chunkStart, chunkEnd);

    try {
      const chunkResponse = await fetch(
        "http://localhost:8888/extension/save_chunk",
        {
          method: "POST",
          headers: {
            "Content-Type": "application/octet-stream",
            "X-File-ID": fileId,
            "X-Chunk-Index": i.toString(),
            "X-Total-Chunks": totalChunks.toString(),
            "X-Chunk-Start": chunkStart.toString(),
            "X-Chunk-End": chunkEnd.toString(),
          },
          body: chunk,
        }
      );

      if (!chunkResponse.ok) {
        throw new Error(`分块上传失败: ${chunkResponse.status}`);
      }

      // 更新进度提示
      const progress = Math.round(((i + 1) / totalChunks) * 100);
      showTooltip(`上传中... ${progress}%`);
    } catch (error) {
      showTooltip(`分块 ${i + 1}/${totalChunks} 上传失败`, true);
      throw error;
    }
  }

  // 3. 完成上传
  try {
    const completeResponse = await fetch(
      "http://localhost:8888/extension/complete",
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-File-ID": fileId,
          "X-Expected-Length": htmlBytes.length.toString(),
          "X-Checksum": await calculateChecksum(htmlBytes),
        },
      }
    );

    if (!completeResponse.ok) {
      throw new Error("完成上传失败");
    }

    return await completeResponse.json();
  } catch (error) {
    showTooltip("完成上传时出错", true);
    throw error;
  }
}

// 计算SHA-256校验和
async function calculateChecksum(arrayBuffer) {
  const hashBuffer = await crypto.subtle.digest("SHA-256", arrayBuffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

// 增强的fetch函数（带超时）
function fetchWithTimeout(url, options, timeout = 30000) {
  return Promise.race([
    fetch(url, options),
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error("Request timeout")), timeout)
    ),
  ]);
}

// 带重试的fetch函数
async function fetchWithRetry(url, options, maxRetries = 3) {
  let lastError = null;

  for (let i = 0; i < maxRetries; i++) {
    try {
      const response = await fetchWithTimeout(url, options);
      return response;
    } catch (error) {
      lastError = error;
      if (i < maxRetries - 1) {
        await new Promise((resolve) => setTimeout(resolve, 1000 * (i + 1)));
      }
    }
  }

  throw lastError || new Error("Unknown error");
}

// 辅助函数
function generateFileId() {
  return `${Date.now().toString(36)}-${Math.random()
    .toString(36)
    .substr(2, 8)}`;
}

function generateRequestId() {
  return crypto.randomUUID?.() || Math.random().toString(36).substr(2, 16);
}

// 格式化字节大小显示
function formatBytes(bytes) {
  if (bytes === 0) return "0 Bytes";
  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

// 发送普通请求
async function sendNormalRequest(data) {
  const response = await fetch("http://localhost:8888/extension/save", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(data),
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`服务器响应错误: ${response.status}: ${errorText}`);
  }

  return await response.json();
}

function validateRequestData(data) {
  const errors = [];

  if (!data.url || typeof data.url !== "string") {
    errors.push("Invalid or missing URL");
  }

  if (!data.title || typeof data.title !== "string") {
    errors.push("Invalid or missing title");
  }

  if (!data.html || typeof data.html !== "string") {
    errors.push("Invalid or missing HTML content");
  }

  if (data.mode && !["fullPage", "partial"].includes(data.mode)) {
    errors.push('Invalid mode, must be "fullPage" or "partial"');
  }

  return errors.length ? errors : null;
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
async function inlineAllResources(element = document.documentElement) {
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
