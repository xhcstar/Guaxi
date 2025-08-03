document.addEventListener("DOMContentLoaded", () => {
  const elements = {
    masterSwitch: document.getElementById("masterSwitch"),
    saveButton: document.getElementById("saveButton"),
    statusMessage: document.getElementById("statusMessage"),
  };

  // 初始化状态
  chrome.storage.sync.get(["enabled", "saveMode"], (data) => {
    elements.masterSwitch.checked = data.enabled !== false;
    const defaultMode = data.saveMode || "fullPage";
    document.querySelector(
      `input[name="saveMode"][value="${defaultMode}"]`
    ).checked = true;
  });

  // 事件监听
  elements.masterSwitch.addEventListener("change", handleMasterSwitch);
  document.querySelectorAll('input[name="saveMode"]').forEach((radio) => {
    radio.addEventListener("change", handleModeChange);
  });
  elements.saveButton.addEventListener("click", handleSaveClick);

  // 事件处理器
  function handleMasterSwitch(e) {
    chrome.storage.sync.set({ enabled: e.target.checked });
    showStatus(e.target.checked ? "Extension enabled" : "Extension disabled");
  }

  function handleModeChange(e) {
    chrome.storage.sync.set({ saveMode: e.target.value });
  }

  async function handleSaveClick() {
    const { enabled } = await chrome.storage.sync.get(["enabled"]);

    if (!enabled) {
      showStatus("Please enable extension first", true);
      return;
    }

    const [tab] = await chrome.tabs.query({
      active: true,
      currentWindow: true,
    });

    if (!tab?.id) {
      showStatus("No active tab", true);
      return;
    }

    showStatus("Processing...");

    try {
      // 强制 mode 为 fullPage
      const response = await Promise.race([
        chrome.tabs.sendMessage(tab.id, {
          action: "saveContent",
          mode: "fullPage", // 强制使用 fullPage 模式
        }),
        new Promise((_, reject) =>
          setTimeout(() => reject(new Error("Operation timed out")), 10000)
        ),
      ]);

      console.log("Message response:", response);

      if (response.success) {
        showStatus("Saved successfully!");
        // 可以在这里添加保存到本地存储的逻辑
        await chrome.storage.local.set({
          lastSaved: {
            timestamp: new Date().toISOString(),
            url: response.data?.url,
            title: response.data?.title,
          },
        });
      } else {
        showStatus(`Failed: ${response.error || "Unknown error"}`, true);
      }
    } catch (error) {
      let errorMessage = error.message;
      if (error.message.includes("Could not establish connection")) {
        errorMessage = "Please refresh the target page and try again";
      }
      showStatus(`Error: ${errorMessage}`, true);
      console.error("Save error:", error);
    }
  }

  function showStatus(message, isError = false) {
    elements.statusMessage.textContent = message;
    elements.statusMessage.style.color = isError ? "red" : "green";
    setTimeout(() => (elements.statusMessage.textContent = ""), 3000);
  }
});
