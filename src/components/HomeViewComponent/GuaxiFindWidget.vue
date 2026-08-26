<template>
  <!-- 设计说明：几何与控件组合对齐 Monaco 的 find widget（编辑模式用的就是它），
       但配色走项目 CSS 变量，明暗主题都成立。本组件不含任何搜索逻辑，纯展示 + 事件。 -->
  <div
    v-show="visible"
    class="guaxi-find-widget"
    role="search"
    @mousedown.stop
  >
    <div
      class="guaxi-find-widget__field"
      :class="{ 'is-invalid': regexInvalid, 'is-empty': showNoResults }"
    >
      <input
        ref="inputRef"
        class="guaxi-find-widget__input"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :placeholder="placeholder"
        :value="keyword"
        :title="regexInvalid ? '正则表达式无效' : ''"
        @input="onInput"
        @keydown.enter.exact.prevent="emit('next')"
        @keydown.shift.enter.prevent="emit('prev')"
        @keydown.esc.prevent.stop="emit('close')"
      />
      <div class="guaxi-find-widget__toggles">
        <button
          v-for="toggle in toggles"
          :key="toggle.key"
          type="button"
          class="guaxi-find-widget__toggle"
          :class="{ 'is-active': toggle.active }"
          :title="toggle.title"
          :aria-label="toggle.title"
          :aria-pressed="String(toggle.active)"
          tabindex="-1"
          @click="toggle.onToggle()"
        >{{ toggle.glyph }}</button>
      </div>
    </div>

    <span class="guaxi-find-widget__count" :class="{ 'is-empty': showNoResults }">{{ countText }}</span>

    <div class="guaxi-find-widget__actions">
      <button
        type="button"
        class="guaxi-find-widget__action"
        :disabled="!matchCount"
        title="上一个匹配 (Shift+Enter)"
        aria-label="上一个匹配"
        tabindex="-1"
        @click="emit('prev')"
      >
        <Icon :icon="arrowUpIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-find-widget__action"
        :disabled="!matchCount"
        title="下一个匹配 (Enter)"
        aria-label="下一个匹配"
        tabindex="-1"
        @click="emit('next')"
      >
        <Icon :icon="arrowDownIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-find-widget__action"
        title="关闭 (Esc)"
        aria-label="关闭搜索"
        tabindex="-1"
        @click="emit('close')"
      >
        <Icon :icon="closeIcon" width="16" height="16" />
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { Icon } from '@iconify/vue'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
import closeIcon from '@iconify-icons/lucide/x'

const props = defineProps({
  visible: { type: Boolean, default: false },
  keyword: { type: String, default: '' },
  // 当前命中下标（0 基）；-1 表示没有激活命中
  matchIndex: { type: Number, default: -1 },
  matchCount: { type: Number, default: 0 },
  caseSensitive: { type: Boolean, default: false },
  wholeWord: { type: Boolean, default: false },
  useRegex: { type: Boolean, default: false },
  regexInvalid: { type: Boolean, default: false },
  placeholder: { type: String, default: '查找' },
})

const emit = defineEmits([
  'update:keyword',
  'update:caseSensitive',
  'update:wholeWord',
  'update:useRegex',
  'next',
  'prev',
  'close',
])

const inputRef = ref(null)

// 只有“输入了关键词却没有命中”才算无结果；关键词为空时保持中性，不显示红框。
const showNoResults = computed(() => !!String(props.keyword || '').trim() && !props.matchCount)

// 文案与编辑模式的 Monaco 控件逐字一致，避免四个模式看起来像四个产品。
const countText = computed(() => {
  if (!String(props.keyword || '').trim()) return 'No results'
  const total = Math.max(0, Number(props.matchCount || 0))
  if (!total) return 'No results'
  const index = Math.max(0, Number(props.matchIndex || 0))
  return `${Math.min(index + 1, total)} of ${total}`
})

const toggles = computed(() => [
  {
    key: 'case',
    glyph: 'Aa',
    title: '区分大小写',
    active: props.caseSensitive,
    onToggle: () => emit('update:caseSensitive', !props.caseSensitive),
  },
  {
    key: 'word',
    glyph: 'ab',
    title: '全字匹配',
    active: props.wholeWord,
    onToggle: () => emit('update:wholeWord', !props.wholeWord),
  },
  {
    key: 'regex',
    glyph: '.*',
    title: '使用正则表达式',
    active: props.useRegex,
    onToggle: () => emit('update:useRegex', !props.useRegex),
  },
])

const onInput = (event) => {
  emit('update:keyword', String(event?.target?.value ?? ''))
}

const focusInput = ({ select = true } = {}) => {
  const input = inputRef.value
  if (!input) return
  input.focus()
  if (select) input.select?.()
}

defineExpose({ focusInput })
</script>

<style scoped>
/* 尺寸与控件排布照 Monaco find widget 复刻：30px 高、20px 方形按钮、
   toggle 放在输入框内部右侧、计数固定宽度防抖动。颜色一律走主题变量。 */
.guaxi-find-widget {
  --gfw-border: var(--guaxi-border, rgba(128, 128, 128, 0.32));
  --gfw-bg: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 94%, var(--guaxi-content-bg, #f5f7fa));
  --gfw-field-bg: color-mix(in srgb, var(--guaxi-content-bg, #ffffff) 88%, var(--guaxi-panel-bg, #ffffff));
  --gfw-accent: var(--primary-color, #007fd4);
  --gfw-danger: #f14c4c;
  --gfw-muted: var(--guaxi-text-secondary, rgba(128, 128, 128, 0.9));

  display: flex;
  align-items: center;
  gap: 4px;
  width: fit-content;
  max-width: 100%;
  height: 30px;
  padding: 0 4px 0 6px;
  border: 1px solid var(--gfw-border);
  border-radius: 4px;
  background: var(--gfw-bg);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.26);
  color: var(--guaxi-text-primary, inherit);
  font-size: 12px;
  box-sizing: border-box;
}

.guaxi-find-widget__field {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 22px;
  padding: 0 2px 0 4px;
  border: 1px solid transparent;
  border-radius: 2px;
  background: var(--gfw-field-bg);
  box-sizing: border-box;
}

.guaxi-find-widget__field:focus-within {
  border-color: var(--gfw-accent);
}

.guaxi-find-widget__field.is-empty {
  border-color: color-mix(in srgb, var(--gfw-danger) 70%, transparent);
}

.guaxi-find-widget__field.is-invalid {
  border-color: var(--gfw-danger);
}

.guaxi-find-widget__input {
  width: 190px;
  min-width: 90px;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  color: inherit;
  font-size: 12px;
  font-family: inherit;
}

.guaxi-find-widget__input::placeholder {
  color: var(--gfw-muted);
  opacity: 0.8;
}

.guaxi-find-widget__toggles {
  display: flex;
  align-items: center;
  gap: 1px;
  flex: none;
}

.guaxi-find-widget__toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 3px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  /* Aa / ab / .* 三个 glyph 用等宽字体，才能和 Monaco 的图标观感对齐 */
  font-family: var(--editor-font-family, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
  font-size: 11px;
  line-height: 1;
  letter-spacing: -0.2px;
  opacity: 0.72;
}

.guaxi-find-widget__toggle:hover {
  background: color-mix(in srgb, currentColor 12%, transparent);
  opacity: 1;
}

.guaxi-find-widget__toggle.is-active {
  border-color: var(--gfw-accent);
  background: color-mix(in srgb, var(--gfw-accent) 34%, transparent);
  opacity: 1;
}

.guaxi-find-widget__count {
  flex: none;
  min-width: 66px;
  padding: 0 2px;
  color: var(--gfw-muted);
  text-align: left;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.guaxi-find-widget__count.is-empty {
  color: color-mix(in srgb, var(--gfw-danger) 88%, var(--gfw-muted));
}

.guaxi-find-widget__actions {
  display: flex;
  align-items: center;
  gap: 1px;
  flex: none;
}

.guaxi-find-widget__action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  opacity: 0.78;
}

.guaxi-find-widget__action:hover:not(:disabled) {
  background: color-mix(in srgb, currentColor 12%, transparent);
  opacity: 1;
}

.guaxi-find-widget__action:disabled {
  opacity: 0.32;
  cursor: default;
}
</style>
