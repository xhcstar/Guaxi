let selectionModeActive = false;
let selectedElement = null;

// 处理保存请求
async function handleSaveRequest(request) {
  console.log("[Guaxi] Handling save request:", request);

  try {
    if (request.mode === "fullPage") {
      return await saveFullPage();
    }
    // 你可以保留 selection 模式，也可以直接移除
    return { success: false, error: "Only fullPage mode is supported" };
  } catch (error) {
    console.error("[Guaxi] Save request failed:", error);
    return { success: false, error: error.message };
  }
}

// 保存完整页面
async function saveFullPage() {
  console.log("[Guaxi] Saving full page (raw HTML)...");
  const html = document.documentElement.outerHTML;
  const data = {
    url: window.location.href,
    title: document.title,
    html: html,
    timestamp: new Date().toISOString(),
  };

  try {
    // tauri 后端监听在 8888 端口
    const response = await fetch("http://localhost:8888/extension/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    });
    const result = await response.json();
    if (result.success) {
      return { success: true, data: result };
    } else {
      return { success: false, error: result.message || "Save failed" };
    }
  } catch (error) {
    return { success: false, error: error.message };
  }
}

// 保存选中内容
async function saveSelection(element) {
  if (!element) {
    throw new Error("No element selected");
  }

  console.log("[Guaxi] Saving selection:", element);
  const markdown = new TurndownService().turndown(element.innerHTML);

  return {
    success: true,
    data: {
      url: window.location.href,
      title: document.title,
      content: markdown,
      mode: "partial",
      element: {
        id: element.id || null,
        tag: element.tagName,
        class: element.className || null,
      },
      timestamp: new Date().toISOString(),
    },
  };
}

// ==================== 选择模式 ====================

// 激活选择模式
async function activateSelectionMode() {
  return new Promise((resolve) => {
    console.log("[Guaxi] Activating selection mode");
    selectionModeActive = true;
    addHighlightStyles();
    document.body.style.cursor = "pointer";

    // 点击处理函数
    const handleClick = async (e) => {
      e.preventDefault();
      e.stopImmediatePropagation();

      const element = findSelectableElement(e.target);
      if (element) {
        try {
          highlightElement(element);
          const result = await saveSelection(element);
          resolve(result);
        } catch (error) {
          resolve({
            success: false,
            error: error.message,
          });
        } finally {
          deactivateSelectionMode();
        }
      }
    };

    // 键盘处理函数
    const handleKeyDown = (e) => {
      if (e.key === "Escape") {
        console.log("[Guaxi] Selection canceled by ESC");
        resolve({
          success: false,
          error: "Selection canceled",
        });
        deactivateSelectionMode();
      }
    };

    // 添加事件监听
    document.addEventListener("click", handleClick, {
      once: true,
      capture: true,
    });
    document.addEventListener("mouseover", handleMouseOver);
    document.addEventListener("keydown", handleKeyDown);

    showTooltip("Click on the content you want to save. Press ESC to cancel.");
  });
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

// 取消选择模式
function deactivateSelectionMode() {
  if (!selectionModeActive) return;

  console.log("[Guaxi] Deactivating selection mode");
  selectionModeActive = false;

  // 清理样式
  document.body.style.cursor = "";
  document.querySelectorAll('[class*="guaxi-"]').forEach((el) => {
    el.classList.remove("guaxi-hover-highlight", "guaxi-selected-highlight");
  });

  // 移除样式标签
  const styleTag = document.getElementById("guaxi-highlight-styles");
  if (styleTag) styleTag.remove();

  // 移除事件监听
  document.removeEventListener("mouseover", handleMouseOver);
  document.removeEventListener("keydown", handleEscapeKey);

  // 移除提示
  document.querySelectorAll(".guaxi-tooltip").forEach((el) => el.remove());
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
      outline: 2px dashed #4CAF50 !important;
      background-color: rgba(76, 175, 80, 0.1) !important;
      cursor: pointer !important;
    }
    .guaxi-selected-highlight {
      outline: 2px solid #2196F3 !important;
      background-color: rgba(33, 150, 243, 0.2) !important;
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

// ==================== 消息通信 ====================

// 监听来自后台的消息
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "saveContent") {
    console.log("[Guaxi] Received save request:", request);
    handleSaveRequest(request).then(sendResponse);
    return true; // 保持消息通道开放
  }
  return false;
});
