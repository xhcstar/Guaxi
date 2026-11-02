<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="系统维护">
      <h3>垃圾回收配置</h3>
      <p class="setting-page-desc">
        后台会定期清理已删除的数据和临时冲突文件，以减少空间占用并保持库的整洁。
      </p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="GC 周期（小时）">
          <n-input-number
            v-model:value="gcCycleHours"
            :min="1"
            :max="72"
            :step="1"
            :precision="0"
            :disabled="loading"
            style="width: 200px"
            @update:value="handleGcCycleChange"
          />
        </n-form-item>
      </n-form>
      <n-alert type="info" :show-icon="true" class="setting-page-tip">
        修改后将在下一次周期生效，最低间隔为 1 小时。若未配置则默认每 1 小时执行一次垃圾回收。
      </n-alert>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted } from 'vue'
import { NCard, NForm, NFormItem, NInputNumber, NAlert } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('MaintenanceContent')

const loading = ref(false)
const gcCycleHours = ref(1)

onMounted(async () => {
  try {
    loading.value = true
    const seconds = await invoke('get_gc_cycle_seconds')
    if (typeof seconds === 'number' && seconds > 0) {
      gcCycleHours.value = Math.max(1, Math.round(seconds / 3600))
    } else {
      gcCycleHours.value = 1
    }
  } catch (error) {
    logger.error('加载系统维护设置失败:', error)
    gcCycleHours.value = 1
  } finally {
    loading.value = false
  }
})

const handleGcCycleChange = async (value) => {
  if (!value || value < 1) {
    gcCycleHours.value = 1
  }
  try {
    loading.value = true
    const seconds = gcCycleHours.value * 3600
    await invoke('set_gc_cycle_seconds', { seconds })
  } catch (error) {
    logger.error('更新 GC 周期配置失败:', error)
  } finally {
    loading.value = false
  }
}
</script>
