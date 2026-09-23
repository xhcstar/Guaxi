<template>
  <div class="vn">
    <div class="vn-row" :style="{ paddingLeft: depth * 14 + 'px' }" @click="onClick">
      <span class="vn-tw">{{ node.numchild > 0 ? (node.expanded ? '▾' : '▸') : '·' }}</span>
      <span class="vn-exp">{{ node.expr || node.name }}</span>
      <span class="vn-type" v-if="node.type">: {{ node.type }}</span>
      <span class="vn-val">= {{ node.value }}</span>
    </div>
    <template v-if="node.expanded">
      <GdbVarNode
        v-for="c in node.children"
        :key="c.name"
        :node="c"
        :depth="depth + 1"
        @expand="$emit('expand', $event)"
      />
    </template>
  </div>
</template>

<script setup>
defineOptions({ name: 'GdbVarNode' })
const props = defineProps({ node: { type: Object, required: true }, depth: { type: Number, default: 0 } })
const emit = defineEmits(['expand'])
function onClick() {
  if (props.node.numchild > 0) emit('expand', props.node)
}
</script>

<style scoped>
.vn-row { display: flex; gap: 6px; align-items: baseline; padding: 2px 0; cursor: default; white-space: nowrap; }
.vn-row:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.vn-tw { width: 12px; color: var(--guaxi-text-secondary, #9ca3af); cursor: pointer; }
.vn-exp { color: #79c0ff; }
.vn-type { color: var(--guaxi-text-secondary, #9ca3af); }
.vn-val { color: var(--guaxi-code-fg, #d4d4d4); word-break: break-all; white-space: pre-wrap; }
</style>
