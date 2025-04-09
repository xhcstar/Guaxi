import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import { writeFile } from "@tauri-apps/plugin-fs";
import { writeImage } from "@tauri-apps/plugin-clipboard-manager";

export function useScreenshot() {
  // --- 状态管理 ---
  const isActive = ref(false);
  const isPenMode = ref(false);
  const showToolbar = ref(false);
  const selection = ref(null);
  const toolbarPosition = ref({ x: 0, y: 0 });
  let unlistenShortcut = null;
  const appWindow = getCurrent(); // 获取当前窗口实例

  // --- 计算属性 ---
  const boxStyle = computed(() => {
    if (!selection.value) return {};
    const { start, end } = selection.value;
    return {
      left: `${Math.min(start.x, end.x)}px`,
      top: `${Math.min(start.y, end.y)}px`,
      width: `${Math.abs(end.x - start.x)}px`,
      height: `${Math.abs(end.y - start.y)}px`,
    };
  });

  // --- 核心方法 ---
  const startSelection = (e) => {
    if (isPenMode.value) return;
    selection.value = {
      start: { x: e.clientX, y: e.clientY },
      end: { x: e.clientX, y: e.clientY },
    };
  };

  const updateSelection = (e) => {
    if (!selection.value || isPenMode.value) return;
    selection.value.end = { x: e.clientX, y: e.clientY };
    toolbarPosition.value = selection.value.end;
  };

  const endSelection = () => {
    if (isPenMode.value || !selection.value) return;
    showToolbar.value = true;
  };

  const captureSelectedArea = async () => {
    if (!selection.value) throw new Error("No selection made");

    const { start, end } = selection.value;
    return await invoke("capture_region", {
      x: Math.min(start.x, end.x),
      y: Math.min(start.y, end.y),
      width: Math.abs(end.x - start.x),
      height: Math.abs(end.y - start.y),
    });
  };

  // --- 工具栏功能 ---
  const saveImage = async () => {
    try {
      const imageData = await captureSelectedArea();
      const path = await invoke("show_save_dialog", {
        app: appWindow, // 传递窗口实例作为AppHandle
        defaultPath: `screenshot_${Date.now()}.png`,
        filters: [{ name: "PNG", extensions: ["png"] }],
      });
      if (path) {
        await writeFile(path, imageData);
        emit("screenshot-saved", { path });
      }
    } finally {
      reset();
    }
  };

  const copyToClipboard = async () => {
    try {
      const imageData = await captureSelectedArea();
      await writeImage(imageData);
      emit("screenshot-copied");
    } finally {
      reset();
    }
  };

  const togglePenMode = () => {
    isPenMode.value = !isPenMode.value;
    document.body.style.cursor = isPenMode.value ? "crosshair" : "default";
    emit("tool-changed", { tool: isPenMode.value ? "pen" : "select" });
  };

  // --- 生命周期 ---
  const setupGlobalShortcut = async () => {
    unlistenShortcut = await listen("trigger-screenshot", () => {
      activate();
    });
  };

  const cleanup = () => {
    unlistenShortcut?.();
    reset();
  };

  // --- 公开方法 ---
  const activate = () => {
    isActive.value = true;
    document.body.style.cursor = "crosshair";
    emit("screenshot-activated");
  };

  const reset = () => {
    isActive.value = false;
    isPenMode.value = false;
    showToolbar.value = false;
    selection.value = null;
    document.body.style.cursor = "";
    emit("screenshot-closed");
  };

  // --- 初始化 ---
  onMounted(() => {
    setupGlobalShortcut();
    document.addEventListener("keydown", handleKeyDown);
  });

  onUnmounted(() => {
    cleanup();
    document.removeEventListener("keydown", handleKeyDown);
  });

  // --- 快捷键处理 ---
  const handleKeyDown = (e) => {
    if (!isActive.value) return;

    // ESC 取消
    if (e.key === "Escape") {
      reset();
      e.preventDefault();
    }

    // 工具切换快捷键
    if (e.ctrlKey && e.key === "d") {
      togglePenMode();
      e.preventDefault();
    }
  };

  return {
    // 状态
    isActive,
    isPenMode,
    showToolbar,
    selection,
    toolbarPosition,

    // 计算属性
    boxStyle,

    // 方法
    activate,
    reset,
    startSelection,
    updateSelection,
    endSelection,
    saveImage,
    copyToClipboard,
    togglePenMode,

    // 生命周期
    setupGlobalShortcut,
    cleanup,
  };
}
