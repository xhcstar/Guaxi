<template>
  <div ref="rootRef" class="wb-combobox" :class="{ 'wb-combobox--open': open }">
    <input
      ref="inputRef"
      class="wb-combobox__input"
      :class="{ 'wb-combobox__input--select': selectOnly }"
      :value="displayValue"
      :placeholder="placeholder"
      :readonly="selectOnly"
      autocomplete="off"
      @input="onInput"
      @focus="onFocus"
      @mousedown="onInputMousedown"
      @keydown="onKeydown"
      @blur="onBlur"
    />
    <span class="wb-combobox__caret" aria-hidden="true"></span>
    <ul v-if="open && filteredOptions.length" class="wb-combobox__menu" role="listbox">
      <li
        v-for="(item, index) in filteredOptions"
        :key="item.value"
        class="wb-combobox__option"
        :class="{ 'is-active': index === activeIndex, 'is-selected': item.value === modelValue }"
        role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent="selectOption(item)"
        @mouseenter="activeIndex = index"
      >
        <span class="wb-combobox__option-value">{{ item.label || item.value }}</span>
        <span v-if="!selectOnly && item.label && item.label !== item.value" class="wb-combobox__option-label">{{ item.value }}</span>
      </li>
    </ul>
  </div>
</template>

<script setup>
import { computed, nextTick, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  placeholder: { type: String, default: '' },
  selectOnly: { type: Boolean, default: false },
})

const emit = defineEmits(['update:modelValue'])

const rootRef = ref(null)
const inputRef = ref(null)
const open = ref(false)
const activeIndex = ref(-1)

const displayValue = computed(() => {
  if (!props.selectOnly) return props.modelValue
  const list = Array.isArray(props.options) ? props.options : []
  const matched = list.find((item) => String(item?.value) === String(props.modelValue))
  return matched ? (matched.label || matched.value) : props.modelValue
})

const filteredOptions = computed(() => {
  const list = Array.isArray(props.options) ? props.options : []
  if (props.selectOnly) return list
  const keyword = String(props.modelValue || '').trim().toLowerCase()
  if (!keyword) return list
  const matched = list.filter((item) => {
    const value = String(item?.value || '').toLowerCase()
    const label = String(item?.label || '').toLowerCase()
    return value.includes(keyword) || label.includes(keyword)
  })
  return matched.length ? matched : list
})

const onInput = (event) => {
  if (props.selectOnly) return
  emit('update:modelValue', event.target.value)
  open.value = true
  activeIndex.value = -1
}

const onInputMousedown = () => {
  if (props.selectOnly) open.value = !open.value
}

const syncActiveToSelection = () => {
  const idx = filteredOptions.value.findIndex((item) => String(item?.value) === String(props.modelValue))
  activeIndex.value = idx
}

const onFocus = () => {
  open.value = true
  if (props.selectOnly) syncActiveToSelection()
  else activeIndex.value = -1
}

const onBlur = () => {
  open.value = false
}

const selectOption = (item) => {
  emit('update:modelValue', item.value)
  open.value = false
  nextTick(() => inputRef.value?.focus())
}

const onKeydown = (event) => {
  if (!open.value && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
    open.value = true
    if (props.selectOnly) syncActiveToSelection()
    return
  }
  if (!open.value) return
  const count = filteredOptions.value.length
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    if (count) activeIndex.value = (activeIndex.value + 1) % count
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    if (count) activeIndex.value = (activeIndex.value - 1 + count) % count
  } else if (event.key === 'Enter') {
    if (activeIndex.value >= 0 && activeIndex.value < count) {
      event.preventDefault()
      selectOption(filteredOptions.value[activeIndex.value])
    }
  } else if (event.key === 'Escape') {
    open.value = false
  }
}
</script>

<style scoped>
.wb-combobox {
  position: relative;
  display: block;
  width: 100%;
  min-width: 0;
}

.wb-combobox__input {
  width: 100%;
  min-width: 0;
  min-height: 36px;
  padding: 0 28px 0 10px;
  box-sizing: border-box;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
  color: var(--dl-text);
  font-family: inherit;
  font-size: 13px;
  outline: none;
}

.wb-combobox__input::placeholder {
  color: var(--dl-muted);
}

.wb-combobox__input--select {
  cursor: pointer;
  user-select: none;
}

.wb-combobox__input:focus {
  border-color: color-mix(in srgb, var(--dl-accent) 55%, var(--dl-border));
}

.wb-combobox__caret {
  position: absolute;
  top: 50%;
  right: 12px;
  width: 7px;
  height: 7px;
  border-right: 1.5px solid var(--dl-muted);
  border-bottom: 1.5px solid var(--dl-muted);
  transform: translateY(-65%) rotate(45deg);
  pointer-events: none;
}

.wb-combobox__menu {
  position: absolute;
  z-index: 30;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  margin: 0;
  padding: 4px;
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--dl-border);
  border-radius: 8px;
  background: var(--dl-panel);
  color: var(--dl-text);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.18);
}

.wb-combobox__option {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  color: var(--dl-text);
}

.wb-combobox__option.is-active,
.wb-combobox__option:hover {
  background: var(--dl-accent-soft);
}

.wb-combobox__option.is-selected {
  color: var(--dl-accent);
  font-weight: 600;
}

.wb-combobox__option-value {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.wb-combobox__option-label {
  margin-left: auto;
  font-size: 12px;
  color: var(--dl-muted);
  white-space: nowrap;
}
</style>
