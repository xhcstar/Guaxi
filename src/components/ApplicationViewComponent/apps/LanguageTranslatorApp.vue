<template>
  <div class="translator-root">
    <div class="translator-header">
      <div class="translator-title">翻译助手</div>
      <div class="translator-langs">
        <n-select v-model:value="from" :options="langOptions" size="small" style="width: 110px" />
        <span class="translator-arrow">→</span>
        <n-select v-model:value="to" :options="langOptions" size="small" style="width: 110px" />
      </div>
    </div>

    <div class="translator-main">
      <div class="translator-panel">
        <div class="panel-label">输入内容</div>
        <n-input v-model:value="source" type="textarea" rows="6" placeholder="输入要翻译的文本..." />
      </div>
      <div class="translator-panel">
        <div class="panel-label">翻译结果</div>
        <n-input :value="result" type="textarea" rows="6" readonly />
      </div>
    </div>

    <div class="translator-actions">
      <n-button type="primary" @click="translate">翻译</n-button>
      <n-button quaternary size="small" @click="swap">互换语言</n-button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'

const from = ref('zh')
const to = ref('en')
const source = ref('')
const langOptions = [
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' }
]

const result = computed(() => {
  if (!source.value) return ''
  return `【示例】将「${source.value}」从 ${from.value} 翻译为 ${to.value} 的结果占位。`
})

const translate = () => {
}

const swap = () => {
  const tmp = from.value
  from.value = to.value
  to.value = tmp
}
</script>

<style scoped>
.translator-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 16px 18px;
  box-sizing: border-box;
}

.translator-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.translator-title {
  font-size: 18px;
  font-weight: 600;
}

.translator-langs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.translator-arrow {
  font-size: 14px;
  color: #6b7280;
}

.translator-main {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 12px;
}

.translator-panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.panel-label {
  font-size: 12px;
  color: #6b7280;
}

.translator-actions {
  display: flex;
  gap: 8px;
}
</style>