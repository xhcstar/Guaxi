<template>
    <div class="outline-panel">
        <label class="outline-search">
            <span>搜索</span>
            <input v-model="outlineSearch" type="text" placeholder="搜索标题、LaTeX/Typst 章节或 Cell" />
        </label>
        <div class="outline-scroll">
            <n-tree :data="filteredOutlineTree" key-field="id" label-field="text" :render-label="renderLabel"
                :render-prefix="renderPrefix" :expand-on-click="true" :show-line="true" :virtual-scroll="true"
                :virtual-scroll-item-height="28" :expanded-keys="expandedKeys"
                @update:expanded-keys="handleExpandedKeysUpdate" @update:selected-keys="handleSelect"
                class="outline-tree" />
        </div>
    </div>
</template>

<script setup>
import { NTree, NIcon } from 'naive-ui'
import { TextBulletListTree20Filled, List24Filled } from '@vicons/fluent'
import { Code24Regular } from '@vicons/fluent'
import { storeToRefs } from 'pinia'
import { useOutlineStore } from '../../stores/outlineStore'
import { h, computed, ref, watch } from 'vue'

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
const outlineSearch = ref('')
const filteredOutlineTree = computed(() => {
    const keyword = outlineSearch.value.trim().toLowerCase()
    if (!keyword) return outlineTree.value
    const filterNodes = (nodes) => {
        return nodes.reduce((acc, node) => {
            const children = filterNodes(node.children || [])
            const matched = String(node.text || '').toLowerCase().includes(keyword)
                || String(node.kind || '').toLowerCase().includes(keyword)
            if (matched || children.length) {
                acc.push({ ...node, children })
            }
            return acc
        }, [])
    }
    return filterNodes(outlineTree.value)
})
const expandedKeys = ref([])
const expandedKeysInitialized = ref(false)

function collectAllKeys(nodes, bucket = []) {
    nodes.forEach((node) => {
        bucket.push(node.id)
        if (Array.isArray(node.children) && node.children.length) {
            collectAllKeys(node.children, bucket)
        }
    })
    return bucket
}

function getLevelOneExpandedKeys(tree) {
    return tree.map((node) => node.id)
}

watch(
    filteredOutlineTree,
    (tree) => {
        // 搜索态：整棵展开，才能看清命中落在哪一层。
        if (outlineSearch.value.trim()) {
            expandedKeys.value = collectAllKeys(tree)
            return
        }

        // 设计说明（关键 · 对应“一级标题默认是收起的，每次都要手动展开”）：
        // 大纲面板挂载往往早于大纲生成，watcher 会先以空数组跑一次 immediate。
        // 此前那一次就把 expandedKeysInitialized 置位了，于是真正的大纲到达时走的是
        // “保留用户展开状态”分支——而那时状态是空数组，且旧的重置兜底要求
        // expandedKeys.length > 0，条件不成立，结果一级标题全部收起。
        // 空树阶段必须什么都不认定，把“首次初始化”留给第一份真实大纲。
        if (!tree.length) {
            expandedKeys.value = []
            return
        }

        const allKeys = new Set(collectAllKeys(tree))
        const survived = expandedKeys.value.filter((key) => allKeys.has(key))

        // 节点 id 含行号，切文件 / 切模式（源码大纲 ↔ 探索者大纲）会让整棵树的 id 全变，
        // 一个都不剩说明这是“另一份大纲”，和首次一样按默认展开一级标题处理。
        // 同一份文档内的普通更新（有 key 存活）则保留用户自己的展开状态。
        if (!expandedKeysInitialized.value || survived.length === 0) {
            expandedKeys.value = getLevelOneExpandedKeys(tree)
            expandedKeysInitialized.value = true
            return
        }

        expandedKeys.value = survived
    },
    { immediate: true }
)

// 渲染标签
const renderLabel = ({ option }) => {
    return h('span', {
        class: [
            'outline-label',
            option.kind === 'python-cell' ? 'is-python-cell' : '',
            option.kind === 'math-cell' ? 'is-math-cell' : '',
        ],
        style: {
            fontWeight: option.level === 1 ? 'bold' : 'normal'
        }
    }, option.kind === 'python-cell'
        ? `Python Cell · ${option.text}`
        : option.kind === 'math-cell'
            ? `Math Cell · ${option.text}`
            : option.text)
}

// 渲染前缀图标
const renderPrefix = ({ option }) => {
    if (option.kind === 'python-cell') {
        return h(NIcon, null, { default: () => h(Code24Regular) })
    }
    if (option.kind === 'math-cell') {
        return h(NIcon, null, { default: () => h('span', { class: 'outline-math-icon' }, 'Σ') })
    }
    if (option.level === 1) {
        return h(NIcon, null, { default: () => h(List24Filled) })
    }
    return h(NIcon, null, { default: () => h(TextBulletListTree20Filled) })
}

// 点击跳转
const handleSelect = (keys, options) => {
    if (options && options[0] && jumpToLine.value) {
        // 设计说明：第一个参数继续保持旧的行号/页码数字，兼容 PDF/Reader 等已有调用；
        // 第二个参数补充完整大纲项，供编辑器区分 Markdown、LaTeX、Typst 等不同跳转目标。
        jumpToLine.value(options[0].line, options[0])
    }
}

const handleExpandedKeysUpdate = (keys) => {
    expandedKeys.value = keys
}
</script>

<style scoped>
.outline-panel {
    flex: 1 1 0;
    min-height: 0;
    height: 100%;
    padding: 0;
    font-size: 14px;
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.outline-scroll {
    flex: 1 1 0;
    height: 0;
    min-height: 0;
    overflow: auto;
    padding: 6px 6px 6px 8px;
    box-sizing: border-box;
}

.outline-search {
    flex: 0 0 auto;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    margin: 8px 8px 4px;
    border: 1px solid var(--guaxi-border, rgba(128, 128, 128, 0.22));
    border-radius: 8px;
    padding: 6px 9px;
    background: var(--guaxi-header-bg);
    color: var(--guaxi-text-secondary, #667085);
}

.outline-search span {
    font-size: 12px;
}

.outline-search input {
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--guaxi-text-primary, inherit);
    font: inherit;
}

:deep(.n-tree-node-content) {
    border-radius: 6px;
    transition: background 0.2s ease, box-shadow 0.2s ease;
}

:deep(.n-tree-node-content:hover) {
    background: var(--guaxi-header-bg);
    box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.3),
        0 2px 12px 0 rgba(7, 118, 161, 0.15);
}

.outline-tree {
    width: 100%;
    height: 100%;
    min-height: 0;
}

:deep(.outline-tree .n-tree-node-content__text) {
    white-space: normal;
    word-break: break-word;
    line-height: 1.35;
    font-family: inherit;
}

:deep(.outline-tree .n-tree-node-content) {
    align-items: flex-start;
}

:deep(.outline-label.is-python-cell) {
    color: color-mix(in srgb, var(--guaxi-text-primary, currentColor) 86%, #3b82f6 14%);
}

:deep(.outline-label.is-math-cell) {
    color: color-mix(in srgb, var(--guaxi-text-primary, currentColor) 86%, #0891b2 14%);
}

.outline-math-icon {
    font-weight: 700;
    font-size: 14px;
    color: #0891b2;
}
</style>
