<template>
  <teleport to="body">
    <div 
      v-if="isActive" 
      class="stealth-overlay" 
      @mousedown="handleMouseDown"
    >
      <AnnotationCanvas 
        ref="annotator" 
        :enabled="isPenMode" 
        @change="handleAnnotationChange"
      />

      <div v-if="selection && !isPenMode" class="selection-box" :style="boxStyle" />

      <ScreenCaptureToolbar 
        v-if="showToolbar"
        :position="toolbarPosition"
        :isPenActive="isPenMode"
        @save="saveImage"
        @copy="copyToClipboard"
        @togglePen="togglePenMode"
        @cancel="reset"
      />
    </div>
  </teleport>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import AnnotationCanvas from './AnnotationCanvas.vue';
import ScreenCaptureToolbar from './ScreenCaptureToolbar.vue';

// 状态管理
const isActive = ref(false);
const isPenMode = ref(false);
const showToolbar = ref(false);
const selection = ref(null);
const toolbarPosition = ref({ x: 0, y: 0 });
const annotator = ref(null);
const appWindow = getCurrentWindow();

// 计算属性
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

// 鼠标事件处理
const handleMouseDown = async (e) => {
  if (isPenMode.value) return;
  
  const pos = await getPreciseMousePosition(e);
  selection.value = {
    start: { x: pos.x, y: pos.y },
    end: { x: pos.x, y: pos.y }
  };
  
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', handleMouseUp, { once: true });
};

const handleMouseMove = async (e) => {
  if (!selection.value || isPenMode.value) return;
  const pos = await getPreciseMousePosition(e);
  selection.value.end = { x: pos.x, y: pos.y };
  toolbarPosition.value = { x: pos.x, y: pos.y };
};

const handleMouseUp = async () => {
  document.removeEventListener('mousemove', handleMouseMove);
  if (isPenMode.value || !selection.value) return;
  
  showToolbar.value = true;
  await appWindow.show().catch(console.error);
  await appWindow.setFocus().catch(console.error);
};

// 辅助方法
const getPreciseMousePosition = async (e) => {
  try {
    const pos = await invoke('get_mouse_position');
    return { x: pos[0], y: pos[1] };
  } catch {
    return { x: e.screenX, y: e.screenY };
  }
};

const handleAnnotationChange = (path) => {
  // 处理标注变化事件
  emit('annotation-change', path);
};

// 核心功能方法
const activate = async () => {
  isActive.value = true;
  document.body.style.cursor = 'crosshair';
  await appWindow.hide().catch(console.error);
  await invoke('start_global_capture').catch(console.error);
  emit('screenshot-activated');
};

const reset = async () => {
  isActive.value = false;
  isPenMode.value = false;
  showToolbar.value = false;
  selection.value = null;
  document.body.style.cursor = '';
  
  await invoke('stop_global_capture').catch(console.error);
  await appWindow.show().catch(console.error);
  emit('screenshot-closed');
};

const captureSelectedArea = async () => {
  if (!selection.value) throw new Error("No selection made");
  
  const { start, end } = selection.value;
  return await invoke('capture_region', {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  });
};

// 工具栏功能
const saveImage = async () => {
  try {
    const imageData = await captureSelectedArea();
    const path = await invoke('show_save_dialog', {
      defaultPath: `screenshot_${Date.now()}.png`,
      filters: [{ name: 'PNG', extensions: ['png'] }]
    });
    
    if (path) {
      await writeFile(path, imageData);
      emit('screenshot-saved', { path });
    }
  } finally {
    await reset();
  }
};

const copyToClipboard = async () => {
  try {
    const imageData = await captureSelectedArea();
    await writeImage(imageData);
    emit('screenshot-copied');
  } finally {
    await reset();
  }
};

const togglePenMode = () => {
  isPenMode.value = !isPenMode.value;
  document.body.style.cursor = isPenMode.value ? 'crosshair' : 'default';
  emit('tool-changed', { tool: isPenMode.value ? 'pen' : 'select' });
};

// 生命周期
onMounted(() => {
  listen('global-mouse-move', (event) => {
    if (!isActive.value || isPenMode.value) return;
    toolbarPosition.value = { x: event.payload[0], y: event.payload[1] };
  });
});

defineExpose({
  activate,
  reset,
  togglePenMode,
  setupGlobalShortcut: async () => {
    await listen('trigger-screenshot', activate);
  }
});
</script>

<style scoped>
.stealth-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 2147483647;
  background-color: rgba(0, 0, 0, 0.01);
  cursor: crosshair;
}

.selection-box {
  position: absolute;
  border: 2px dashed rgba(0, 150, 255, 0.8);
  background: rgba(0, 100, 255, 0.1);
  pointer-events: none;
}
</style>