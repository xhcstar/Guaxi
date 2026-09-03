<template>
  <n-tooltip
    :delay="220"
    :disabled="!hint && !iconOnly"
    :keep-alive-on-hover="false"
    placement="top"
  >
    <template #trigger>
      <n-button
        class="gib"
        :size="size"
        :type="type"
        :quaternary="quaternary"
        :secondary="secondary"
        :circle="round"
        :block="block"
        :loading="loading"
        :disabled="disabled"
        :aria-label="label"
        @click="emit('click')"
      >
        <template #icon><n-icon :component="icon" /></template>
        <template v-if="!iconOnly">{{ label }}</template>
      </n-button>
    </template>
    <div class="gib-tip">
      <div class="gib-tip-title">{{ label }}</div>
      <div v-if="hint" class="gib-tip-hint">{{ hint }}</div>
    </div>
  </n-tooltip>
</template>

<script setup>
/**
 * 带提示的图标按钮。
 *
 * 存在的理由：认知层的按钮很多，而"图标 + 悬浮解释"比一排文字按钮省地方、也更容易扫。
 * 但图标本身是有歧义的，所以提示不是可选装饰——`label` 一定会出现在提示的第一行，
 * `hint` 补一句"点下去会发生什么"。窄处（看板卡片、顶栏、侧栏）用 `icon-only`，
 * 设置页里改数据的动作保留文字，免得用户要靠悬停才敢点。
 */
import { computed } from 'vue'

const props = defineProps({
  /** @vicons 组件。 */
  icon: { type: [Object, Function], required: true },
  /** 按钮语义。icon-only 时它是提示的标题，同时作为 aria-label。 */
  label: { type: String, required: true },
  /** 点下去会发生什么。空字符串则只显示标题。 */
  hint: { type: String, default: '' },
  size: { type: String, default: 'small' },
  type: { type: String, default: 'default' },
  quaternary: { type: Boolean, default: false },
  secondary: { type: Boolean, default: false },
  iconOnly: { type: Boolean, default: false },
  block: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
})

const emit = defineEmits(['click'])

/** 只有图标又不是通栏按钮时才做成圆形，通栏圆角矩形更像"一行操作"。 */
const round = computed(() => props.iconOnly && !props.block)
</script>

<style scoped>
.gib {
  font-family: var(--guaxi-editor-font-family);
}

/* 提示被 teleport 到 body，拿不到调用方的继承，字体与排版在这里自带一份。 */
.gib-tip {
  max-width: 268px;
  font-family: var(--guaxi-editor-font-family);
}
.gib-tip-title {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.65;
}
.gib-tip-hint {
  margin-top: 2px;
  font-size: 12.5px;
  line-height: 1.75;
  color: rgba(255, 255, 255, 0.76);
}
</style>
