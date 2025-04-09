<template>
    <canvas ref="canvas" class="annotation-layer" @mousedown="handleMouseDown" @mousemove="handleMouseMove"
        @mouseup="handleMouseUp" />
</template>

<script setup>
import { ref, onMounted, watch } from 'vue';

const props = defineProps({
    enabled: Boolean
});

const emit = defineEmits(['change']);
const canvas = ref(null);
const ctx = ref(null);
const isDrawing = ref(false);
const currentPath = ref([]);

// 工具配置
const toolConfig = ref({
    color: '#ff0000',
    size: 3,
    type: 'pen' // 'pen' | 'arrow' | 'rectangle'
});

onMounted(() => {
    initCanvas();
    window.addEventListener('resize', initCanvas);
});

const initCanvas = () => {
    const canvasEl = canvas.value;
    canvasEl.width = window.innerWidth;
    canvasEl.height = window.innerHeight;
    ctx.value = canvasEl.getContext('2d');
};

const handleMouseDown = (e) => {
    if (!props.enabled) return;

    isDrawing.value = true;
    currentPath.value = [{ x: e.offsetX, y: e.offsetY }];

    ctx.value.beginPath();
    ctx.value.moveTo(e.offsetX, e.offsetY);
    ctx.value.lineCap = 'round';
    ctx.value.lineJoin = 'round';
    ctx.value.strokeStyle = toolConfig.value.color;
    ctx.value.lineWidth = toolConfig.value.size;
};

const handleMouseMove = (e) => {
    if (!isDrawing.value) return;

    currentPath.value.push({ x: e.offsetX, y: e.offsetY });

    ctx.value.lineTo(e.offsetX, e.offsetY);
    ctx.value.stroke();
    emit('change', currentPath.value);
};

const handleMouseUp = () => {
    isDrawing.value = false;
};

const clear = () => {
    ctx.value.clearRect(0, 0, canvas.value.width, canvas.value.height);
};

defineExpose({
    clear,
    setTool: (type) => toolConfig.value.type = type,
    setColor: (color) => toolConfig.value.color = color
});
</script>

<style scoped>
.annotation-layer {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    z-index: 2147483647;
    pointer-events: auto;
}
</style>