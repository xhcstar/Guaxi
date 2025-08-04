// 初始化设置
chrome.runtime.onInstalled.addListener(() => {
  chrome.storage.sync.set({ enabled: true, defaultMode: "fullPage" });
});

// 命令处理
chrome.commands.onCommand.addListener(async (command) => {
  try {
    const [tab] = await chrome.tabs.query({
      active: true,
      currentWindow: true,
    });
    if (!tab?.id) throw new Error("No active tab");

    // 确保页面完全加载
    await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: () => {
        return new Promise((resolve) => {
          if (document.readyState === "complete") resolve();
          else window.addEventListener("load", resolve);
        });
      },
    });

    // 发送保存指令
    await chrome.tabs.sendMessage(tab.id, {
      action: "saveContent",
      mode: command.includes("full") ? "fullPage" : "partial",
    });
  } catch (error) {
    console.error("Command error:", error);
  }
});

// 监听页面加载完成（备用）
chrome.webNavigation.onCompleted.addListener(
  (details) => {
    console.log(`Page loaded: ${details.url}`);
  },
  { url: [{ schemes: ["http", "https"] }] }
);
