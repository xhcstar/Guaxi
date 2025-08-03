// 初始化
chrome.runtime.onInstalled.addListener(async () => {
  await chrome.storage.sync.set({
    enabled: true,
    defaultMode: "fullPage",
  });
  console.log("Extension initialized");
});

// 命令监听
chrome.commands.onCommand.addListener(async (command) => {
  console.log(`Command: ${command}`);

  try {
    const [tab] = await chrome.tabs.query({
      active: true,
      currentWindow: true,
    });

    if (!tab?.id) throw new Error("No active tab");

    await chrome.tabs.sendMessage(tab.id, {
      action: "saveContent",
      mode: command.includes("full") ? "fullPage" : "partial",
    });
  } catch (error) {
    console.error("Command error:", error);
  }
});

// 消息接收
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.type === "saveResult") {
    console.log("Save result:", request.success);
    sendResponse({ received: true });
  }
  return true;
});
