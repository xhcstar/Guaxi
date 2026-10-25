<template>
  <svg
    class="mail-icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    :fill="filled ? 'currentColor' : 'none'"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <path v-for="(d, index) in paths" :key="index" :d="d" />
  </svg>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  name: { type: String, required: true },
  size: { type: [Number, String], default: 16 },
  /** 星标这类开关型图标需要实心。 */
  filled: { type: Boolean, default: false },
})

// 统一 24×24 线性图标，描边用 currentColor，按钮悬停/激活只改颜色即可。
const ICONS = {
  reply: ['M9 14 4 9l5-5', 'M20 20v-7a4 4 0 0 0-4-4H4'],
  replyAll: ['M7 14 2 9l5-5', 'M12 14 7 9l5-5', 'M22 20v-7a4 4 0 0 0-4-4H7'],
  forward: ['M15 14l5-5-5-5', 'M4 20v-7a4 4 0 0 1 4-4h12'],
  star: ['M12 2.6l2.9 5.9 6.5.9-4.7 4.6 1.1 6.5L12 17.4 6.2 20.5l1.1-6.5L2.6 9.4l6.5-.9z'],
  unread: ['M4 5h16v14H4z', 'M4 7.5l8 5.5 8-5.5', 'M18.5 3.5a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4'],
  archive: ['M3 8h18v12H3z', 'M2 4h20v4H2z', 'M9.5 12h5'],
  trash: ['M3 6h18', 'M8 6V4h8v2', 'M6 6l1.2 14h9.6L18 6', 'M10 10.5v6', 'M14 10.5v6'],
  translate: [
    'M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19',
    'M2.8 9.5h18.4',
    'M2.8 14.5h18.4',
    'M12 2.5c2.6 2.6 3.9 5.8 3.9 9.5s-1.3 6.9-3.9 9.5c-2.6-2.6-3.9-5.8-3.9-9.5S9.4 5.1 12 2.5',
  ],
  speak: ['M11 5 6 9H2.5v6H6l5 4z', 'M15.5 8.5a5 5 0 0 1 0 7', 'M18.5 5.5a9 9 0 0 1 0 13'],
  download: ['M12 3v12', 'M7 10l5 5 5-5', 'M4 20h16'],
  note: ['M14 2.5H6a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8.5z', 'M14 2.5v6h6', 'M8 13h8', 'M8 17h5'],
  task: ['M9 11.5l3 3L22 4.5', 'M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11'],
  chevronLeft: ['M15 18l-6-6 6-6'],
  chevronRight: ['M9 18l6-6-6-6'],
}

const paths = computed(() => ICONS[props.name] || [])
</script>

<style scoped>
.mail-icon {
  display: block;
  flex: 0 0 auto;
}
</style>
