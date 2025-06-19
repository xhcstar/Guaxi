<template>
    <div class="outline-panel">
        <n-tree :data="outlineTree" key-field="id" label-field="text" :render-label="renderLabel"
            :render-prefix="renderPrefix" :expand-on-click="true" :show-line="true" :virtual-scroll="true"
            :virtual-scroll-item-height="28" @update:selected-keys="handleSelect" class="outline-tree" />
    </div>
</template>

<script setup>
import { NTree, NIcon } from 'naive-ui'
import { TextBulletListTree20Filled, List24Filled } from '@vicons/fluent'
import { storeToRefs } from 'pinia'
import { useOutlineStore } from '../../stores/outlineStore'
import { h, computed } from 'vue'

const outlineStore = useOutlineStore()
const { outline, jumpToLine } = storeToRefs(outlineStore)

// 将 outline 扁平数组转为树结构
function buildTree(flatOutline) {
    const stack = []
    const root = []
    flatOutline.forEach((item, idx) => {
        const node = {
            ...item,
            id: `${item.line}-${idx}`,
            children: []
        }
        while (stack.length && stack[stack.length - 1].level >= node.level) {
            stack.pop()
        }
        if (stack.length === 0) {
            root.push(node)
        } else {
            stack[stack.length - 1].children.push(node)
        }
        stack.push(node)
    })
    return root
}

const outlineTree = computed(() => buildTree(outline.value || []))

// 渲染标签
const renderLabel = ({ option }) => {
    return h('span', {
        style: {
            fontWeight: option.level === 1 ? 'bold' : 'normal'
        }
    }, option.text)
}

// 渲染前缀图标
const renderPrefix = ({ option }) => {
    // 你可以根据不同层级或类型自定义不同图标
    if (option.level === 1) {
        return h(NIcon, null, { default: () => h(List24Filled) })
    }
    return h(NIcon, null, { default: () => h(TextBulletListTree20Filled) })
}

// 点击跳转
const handleSelect = (keys, options) => {
    if (options && options[0] && jumpToLine.value) {
        jumpToLine.value(options[0].line)
    }
}
</script>

<style scoped>
.outline-panel {
    padding: 8px;
    font-size: 14px;
    overflow-y: auto;
    max-height: 100%;
    height: 100%;
}

.outline-tree {
    width: 100%;
    height: 100%;
}
</style>