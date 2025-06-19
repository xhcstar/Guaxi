<template>
    <div class="outline-panel">
        <div v-for="item in outline" :key="item.line"
            :style="{ paddingLeft: (item.level - 1) * 16 + 'px', cursor: 'pointer' }"
            @click="jumpToLine && jumpToLine(item.line)">
            <span :style="{ fontWeight: item.level === 1 ? 'bold' : 'normal' }">
                {{ item.text }}
            </span>
        </div>
    </div>
</template>

<script setup>
import { useOutlineStore } from '../../stores/outlineStore'
const outlineStore = useOutlineStore()
const outline = outlineStore.outline
const jumpToLine = outlineStore.jumpToLine
</script>

<style scoped>
.outline-panel {
    padding: 8px;
    font-size: 14px;
    overflow-y: auto;
    max-height: 100%;
}
</style>