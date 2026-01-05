<template>
  <div class="preference-wrapper">
    <n-card title="外观与系统">
      <n-space vertical size="large" style="width: 100%">
        <div>
          <h3>外观</h3>
          <p class="description">这里是外观的设置内容，后续可在此扩展主题、字号等个性化配置。</p>
        </div>

        <n-divider />

        <div>
          <h3>垃圾回收配置</h3>
          <p class="description">
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
          <n-alert type="info" :show-icon="true" class="tip">
            修改后将在下一次周期生效，最低间隔为 1 小时。若未配置则默认每 1 小时执行一次垃圾回收。
          </n-alert>
        </div>

        <n-divider />

        <div>
          <h3>LSP 配置</h3>
          <p class="description">
            关闭后将不会初始化 LSP 语法高亮和代码智能，重新打开后新建编辑器才会生效。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="启用 LSP 语法高亮">
              <n-switch
                v-model:value="lspHighlightEnabled"
                :disabled="loading"
                @update:value="handleLspHighlightChange"
              />
            </n-form-item>
          </n-form>
        </div>


      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInputNumber, NAlert, NSwitch } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const gcCycleHours = ref(1)
const lspHighlightEnabled = ref(false)
const loading = ref(false)

onMounted(async () => {
  try {
    loading.value = true
    const [seconds, enabled] = await Promise.all([
      invoke('get_gc_cycle_seconds'),
      invoke('get_lsp_syntax_highlight_enabled')
    ])

    if (typeof seconds === 'number' && seconds > 0) {
      gcCycleHours.value = Math.max(1, Math.round(seconds / 3600))
    } else {
      gcCycleHours.value = 1
    }

    lspHighlightEnabled.value = !!enabled
  } catch (error) {
    console.error('加载偏好设置失败:', error)
    gcCycleHours.value = 1
    lspHighlightEnabled.value = false
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
    console.error('更新 GC 周期配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleLspHighlightChange = async (value) => {
  try {
    loading.value = true
    await invoke('set_lsp_syntax_highlight_enabled', { enabled: value })
  } catch (error) {
    console.error('更新 LSP 语法高亮配置失败:', error)
  } finally {
    loading.value = false
  }
}


</script>

<style scoped>
.preference-wrapper {
  width: 100%;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.description {
  margin-top: 4px;
  color: #666;
  font-size: 13px;
}

.tip {
  margin-top: 8px;
}
</style>