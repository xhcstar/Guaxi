<template>
    <div class="toolbar" :style="positionStyle">
        <button @click="$emit('save')" title="保存 (Ctrl+S)">
            <svg class="icon">
                <use href="#icon-save" />
            </svg>
        </button>
        <button @click="$emit('copy')" title="复制 (Ctrl+C)">
            <svg class="icon">
                <use href="#icon-copy" />
            </svg>
        </button>
        <button @click="$emit('togglePen')" :class="{ active: isPenActive }" title="标注 (D)">
            <svg class="icon">
                <use href="#icon-pen" />
            </svg>
        </button>
        <button @click="$emit('cancel')" title="取消 (ESC)">
            <svg class="icon">
                <use href="#icon-close" />
            </svg>
        </button>
    </div>
</template>

<script setup>
defineProps({
    position: {
        type: Object,
        required: true
    },
    isPenActive: Boolean
});

const positionStyle = computed(() => ({
    left: `${props.position.x + 10}px`,
    top: `${props.position.y - 50}px`
}));
</script>

<style scoped>
.toolbar {
    position: absolute;
    display: flex;
    gap: 4px;
    background: rgba(40, 40, 40, 0.9);
    border-radius: 6px;
    padding: 6px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(4px);
    z-index: 2147483648;
}

button {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: 4px;
    background: transparent;
    border: none;
    color: white;
    cursor: pointer;
}

button:hover {
    background: rgba(255, 255, 255, 0.1);
}

button.active {
    background: rgba(0, 150, 255, 0.3);
}

.icon {
    width: 18px;
    height: 18px;
    fill: currentColor;
}
</style>