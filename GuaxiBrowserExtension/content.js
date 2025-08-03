let selectionModeActive = false;
let selectedElements = []; // 存储已选元素

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
  console.log("[Guaxi] Saving full page (raw HTML)...");
  const html = document.documentElement.outerHTML;
  const data = {
    url: window.location.href,
    title: document.title,
    html: html,
    timestamp: new Date().toISOString(),
    mode: "fullPage",
  };

  try {
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

  // 只发送 html 字段，与后端 ExtensionRequest 保持一致
  const data = {
    url: window.location.href,
    title: document.title,
    html: element.outerHTML,
    timestamp: new Date().toISOString(),
    mode: "partial",
  };

  try {
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
  if (selectionModeActive) return;
  selectionModeActive = true;
  addHighlightStyles();
  document.body.style.cursor = "pointer";

  document.addEventListener("mouseover", handleMouseOver);

  window.handleAltClick = (e) => {
    if (!selectionModeActive) return;
    if (e.altKey && e.button === 0) {
      e.preventDefault();
      e.stopImmediatePropagation();
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

// 保存所有已选内容
async function saveAllSelections() {
  if (!selectedElements.length) {
    return { success: false, error: "No elements selected" };
  }
  const htmlArr = selectedElements.map((el) => el.outerHTML);
  const data = {
    url: window.location.href,
    title: document.title,
    html: htmlArr.join("\n"),
    timestamp: new Date().toISOString(),
    mode: "partial",
  };

  try {
    const response = await fetch("http://localhost:8888/extension/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    });
    const result = await response.json();
    if (result.success) {
      selectedElements = []; // 清空已选
      showTooltip("所有选中模块已保存", false);
      deactivateSelectionMode(); // 保存后退出选中模式
      return { success: true, data: result };
    } else {
      return { success: false, error: result.message || "Save failed" };
    }
  } catch (error) {
    return { success: false, error: error.message };
  }
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
