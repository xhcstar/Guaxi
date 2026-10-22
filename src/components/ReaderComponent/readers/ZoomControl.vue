<template>
  <!--
    缩放控件：图标 + 百分比读数。

    抽成组件的理由不是省代码，而是**保证五个阅读器的缩放行为一致**：
    单看每个阅读器都自己写一遍 −/＋/100%，很容易在不同文档里出现
    「有的点一下就回到 100%，有的要拖两次」这种不一致。

    百分比读数保留文字：它是**状态**而不是动作，用图标表达不了。
    点击读数本身回到 100%，与多数阅读器一致。
  -->
  <div class="gdr-zoom" role="group" aria-label="缩放">
    <button
      class="guaxi-doc-reader__icon-btn"
      :disabled="disabled || modelValue <= min"
      title="缩小"
      aria-label="缩小"
      @click="step(1 / ratio)"
    >
      <Icon :icon="zoomOutIcon" width="15" height="15" />
    </button>
    <button
      class="gdr-zoom__value"
      :disabled="disabled"
      title="重置为 100%"
      aria-label="重置缩放为 100%"
      @click="reset"
    >
      {{ Math.round(modelValue * 100) }}%
    </button>
    <button
      class="guaxi-doc-reader__icon-btn"
      :disabled="disabled || modelValue >= max"
      title="放大"
      aria-label="放大"
      @click="step(ratio)"
    >
      <Icon :icon="zoomInIcon" width="15" height="15" />
    </button>
  </div>
</template>

<script setup>
import { Icon } from '@iconify/vue'
import zoomInIcon from '@iconify-icons/lucide/zoom-in'
import zoomOutIcon from '@iconify-icons/lucide/zoom-out'

const props = defineProps({
  /** 当前缩放倍率（1 = 100%） */
  modelValue: { type: Number, default: 1 },
  min: { type: Number, default: 0.5 },
  max: { type: Number, default: 3 },
  /** 每次点击的缩放系数 */
  ratio: { type: Number, default: 1.15 },
  disabled: { type: Boolean, default: false },
})

const emit = defineEmits(['update:modelValue'])

const step = (factor) => {
  const next = Math.min(props.max, Math.max(props.min, +(props.modelValue * factor).toFixed(3)))
  if (next !== props.modelValue) emit('update:modelValue', next)
}

const reset = () => {
  if (props.modelValue !== 1) emit('update:modelValue', 1)
}
</script>

<style scoped>
.gdr-zoom {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.gdr-zoom__value {
  min-width: 46px;
  height: 24px;
  padding: 0 6px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: background 0.12s ease, border-color 0.12s ease;
}

.gdr-zoom__value:hover:not(:disabled) {
  background: var(--guaxi-hover-bg, rgba(15, 23, 42, 0.06));
  border-color: var(--guaxi-border, #e1e4e8);
}

.gdr-zoom__value:focus-visible {
  outline: 2px solid #2f6fdf;
  outline-offset: 1px;
}

.gdr-zoom__value:disabled {
  opacity: 0.45;
  cursor: default;
}
</style>
