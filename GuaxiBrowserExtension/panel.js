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
    const radio = document.querySelector(
      `input[name="saveMode"][value="${defaultMode}"]`
    );
    if (radio) radio.checked = true;

    if (defaultMode === "partial") {
      chrome.tabs.query({ active: true, currentWindow: true }, ([tab]) => {
        if (tab?.id) {
          chrome.tabs.sendMessage(tab.id, { action: "startSelectionMode" });
        }
      });
    }
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

// 在 handleModeChange 中添加检查
function handleModeChange(e) {
  chrome.storage.sync.set({ saveMode: e.target.value });
  chrome.tabs.query({ active: true, currentWindow: true }, ([tab]) => {
    if (!tab?.id) return;
    
    // 只在模式确实改变时发送消息
    chrome.storage.sync.get(['saveMode'], (result) => {
      if (result.saveMode !== e.target.value) {
        if (e.target.value === "partial") {
          chrome.tabs.sendMessage(tab.id, { action: "startSelectionMode" });
        } else {
          chrome.tabs.sendMessage(tab.id, { action: "exitSelectionMode" });
        }
      }
    });
  });
}

  async function handleSaveClick() {
    const { enabled, saveMode } = await chrome.storage.sync.get([
      "enabled",
      "saveMode",
    ]);

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
      let response;
      if (saveMode === "partial") {
        response = await chrome.tabs.sendMessage(tab.id, {
          action: "saveAllSelections",
        });
      } else {
        response = await chrome.tabs.sendMessage(tab.id, {
          action: "saveContent",
          mode: "fullPage",
        });
      }

      if (response?.success) {
        showStatus("Saved successfully!");
      } else {
        showStatus(`Failed: ${response?.error || "Unknown error"}`, true);
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
