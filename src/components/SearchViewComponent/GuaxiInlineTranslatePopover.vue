<template>
  <!-- 划词翻译浮层：teleport 到 body，fixed 定位，避免被容器 overflow 裁剪。 -->
  <teleport to="body">
    <div
      v-if="visible"
      class="inline-translate-pop"
      :style="{ left: pos.left + 'px', top: pos.top + 'px' }"
      @mousedown.stop
      @mouseup.stop
      @click.stop
    >
      <div class="itp-head">
        <span class="itp-title">翻译</span>
        <button class="itp-close" title="关闭" @click="$emit('close')">×</button>
      </div>
      <div class="itp-body">
        <div v-if="loading" class="itp-loading">翻译中…</div>
        <div v-else-if="errorText" class="itp-error">{{ errorText }}</div>
        <div v-else class="itp-result">{{ resultText }}</div>
      </div>
      <div class="itp-actions" v-if="!loading && resultText">
        <button class="itp-btn" @click="copyResult">复制译文</button>
      </div>
    </div>
  </teleport>
</template>

<script setup>
const props = defineProps({
  visible: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
  resultText: { type: String, default: '' },
  errorText: { type: String, default: '' },
  pos: { type: Object, default: () => ({ left: 0, top: 0 }) },
})
defineEmits(['close'])

const copyResult = async () => {
  try {
    await navigator.clipboard.writeText(props.resultText || '')
  } catch {
    /* 忽略复制失败 */
  }
}
</script>

<style scoped>
.inline-translate-pop {
  position: fixed;
  z-index: 10050;
  width: 320px;
  max-width: calc(100vw - 16px);
  background: var(--guaxi-panel-bg, #fff);
  border: 1px solid var(--guaxi-border, #e5e7eb);
  border-radius: 10px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.18);
  overflow: hidden;
  font-size: 14px;
  line-height: 1.6;
}

.itp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-bottom: 1px solid var(--guaxi-border, #e5e7eb);
  background: var(--guaxi-header-bg, #f8fafc);
}

.itp-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-secondary, #64748b);
}

.itp-close {
  border: none;
  background: transparent;
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  color: var(--guaxi-text-secondary, #94a3b8);
  padding: 0 4px;
}

.itp-close:hover {
  color: var(--guaxi-text-primary, #1f2328);
}

.itp-body {
  padding: 10px 12px;
  max-height: 200px;
  overflow-y: auto;
  color: var(--guaxi-text-primary, #1f2328);
  white-space: pre-wrap;
  word-break: break-word;
}

.itp-loading,
.itp-error {
  color: var(--guaxi-text-secondary, #64748b);
  font-size: 13px;
}

.itp-error {
  color: #e04f5f;
}

.itp-actions {
  display: flex;
  justify-content: flex-end;
  padding: 0 10px 8px;
}

.itp-btn {
  border: 1px solid var(--guaxi-border, #e5e7eb);
  background: transparent;
  color: var(--guaxi-text-secondary, #64748b);
  border-radius: 6px;
  padding: 3px 10px;
  font-size: 12px;
  cursor: pointer;
}

.itp-btn:hover {
  color: var(--guaxi-primary, #2563eb);
  border-color: var(--guaxi-primary, #2563eb);
}
</style>
