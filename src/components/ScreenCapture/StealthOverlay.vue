<template>
  <teleport to="body">
    <div v-if="isActive" class="stealth-overlay" @mousedown="startSelection" @mousemove="updateSelection"
      @mouseup="endSelection">
      <AnnotationCanvas ref="annotator" :enabled="isPenMode" />

      <div v-if="selection" class="selection-box" :style="boxStyle" />

      <ScreenCaptureToolbar v-if="showToolbar" :position="toolbarPosition" :isPenActive="isPenMode" @save="saveImage"
        @copy="copyToClipboard" @togglePen="togglePenMode" @cancel="reset" />
    </div>
  </teleport>
</template>

<script setup>
import { ref } from 'vue';
import AnnotationCanvas from './AnnotationCanvas.vue';
import ScreenCaptureToolbar from './ScreenCaptureToolbar.vue';
import { useScreenshot } from './useScreenshot';

const annotator = ref(null);
const {
  isActive,
  isPenMode,
  showToolbar,
  selection,
  toolbarPosition,
  boxStyle,
  startSelection,
  updateSelection,
  endSelection,
  saveImage,
  copyToClipboard,
  togglePenMode,
  reset
} = useScreenshot();

defineExpose({
  activate: () => {
    isActive.value = true;
    document.body.style.cursor = 'crosshair';
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
}

.selection-box {
  position: absolute;
  border: 2px dashed rgba(0, 150, 255, 0.8);
  background: rgba(0, 100, 255, 0.1);
  pointer-events: none;
}
</style>