<template>
  <div v-if="isCapturing" class="screen-capture-overlay">
    <div class="screen-capture-toolbar">
      <n-button @click="saveImage">保存</n-button>
      <n-button @click="copyToClipboard">复制</n-button>
      <n-button @click="cancelCapture">取消</n-button>
    </div>
    <canvas ref="canvas" class="screen-capture-canvas"></canvas>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { NButton } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';
import { writeFile } from '@tauri-apps/plugin-fs';
import { writeImage } from '@tauri-apps/plugin-clipboard-manager';

const isCapturing = ref(false);
const canvas = ref(null);
const screenshotData = ref(null);

// 初始化截图功能
const initCapture = async () => {
  try {
    isCapturing.value = true;
    const base64Data = await invoke('capture_screen');
    
    const img = new Image();
    img.onload = () => {
      canvas.value.width = img.width;
      canvas.value.height = img.height;
      const ctx = canvas.value.getContext('2d');
      ctx.drawImage(img, 0, 0);
    };
    img.src = `data:image/png;base64,${base64Data}`;
    screenshotData.value = base64Data;
  } catch (error) {
    console.error('截图失败:', error);
    isCapturing.value = false;
  }
};

// 保存图片（使用官方推荐的 writeFile API）
const saveImage = async () => {
  try {
    const filePath = await invoke('open_dialog', {
      defaultPath: 'screenshot.png',
      filters: [{ name: 'Images', extensions: ['png'] }]
    });
    
    if (filePath) {
      // 将 base64 转换为 Uint8Array
      const binaryData = Uint8Array.from(
        atob(screenshotData.value),
        c => c.charCodeAt(0)
      );
      
      // 使用官方推荐的 writeFile 写入二进制文件
      await writeFile(filePath, binaryData);
    }
    isCapturing.value = false;
  } catch (error) {
    console.error('保存失败:', error);
  }
};

// 复制到剪贴板（保持使用 writeImage）
const copyToClipboard = async () => {
  try {
    const binaryData = Uint8Array.from(
      atob(screenshotData.value),
      c => c.charCodeAt(0)
    );
    await writeImage(binaryData);
    isCapturing.value = false;
  } catch (error) {
    console.error('复制失败:', error);
  }
};

// 取消截图
const cancelCapture = () => {
  isCapturing.value = false;
};

// 键盘事件处理函数
const handleKeyDown = (e) => {
  if (e.ctrlKey && e.key === 'F6') {
    initCapture();
  }
};

// 注册全局快捷键
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<style scoped>
/* 样式保持不变 */
.screen-capture-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(0, 0, 0, 0.5);
  z-index: 9999;
}

.screen-capture-toolbar {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 10px;
  z-index: 10000;
}

.screen-capture-canvas {
  position: absolute;
  top: 0;
  left: 0;
  cursor: crosshair;
}
</style>