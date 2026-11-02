<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="外观主题">
      <p class="setting-page-desc">在此切换应用主题，暗色或者亮色。</p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="主题">
          <n-select
            v-model:value="themeValue"
            :options="themeOptions"
            :disabled="loading || themeSaving"
            style="width: 240px"
            @update:value="handleThemeChange"
          />
        </n-form-item>
      </n-form>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, watch, onMounted } from 'vue'
import { NCard, NForm, NFormItem, NSelect } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '../../stores/settingsStore'

const logger = createLogger('AppearanceContent')

const settingsStore = useSettingsStore()

const themeOptions = [
  { label: '亮色', value: 'light' },
  { label: '暗色', value: 'dark' },
]

const themeValue = ref('light')
const loading = ref(false)
const themeSaving = ref(false)

watch(
  () => settingsStore.appearanceTheme,
  (v) => {
    themeValue.value = v === 'dark' ? 'dark' : 'light'
  },
  { immediate: true }
)

onMounted(async () => {
  try {
    loading.value = true
    await settingsStore.loadAppearanceConfig()
    themeValue.value = settingsStore.appearanceTheme
  } catch (error) {
    logger.error('加载外观主题失败:', error)
    themeValue.value = 'light'
  } finally {
    loading.value = false
  }
})

const handleThemeChange = async (value) => {
  if (loading.value || themeSaving.value) return

  const next = value === 'dark' ? 'dark' : 'light'
  themeValue.value = next

  try {
    themeSaving.value = true

    logger.info('[Appearance] 切换主题:', {
      next,
      current: settingsStore.appearanceTheme,
    })

    await settingsStore.setAppearanceTheme(next)

    try {
      const appearance = await invoke('get_appearance_config')
      logger.info('[Appearance] 后端返回的 appearance（用于确认是否写入配置文件）:', appearance)
    } catch (e) {
      logger.error('[Appearance] 获取后端 appearance 失败（无法确认是否写入配置文件）:', e)
    }

    logger.info('[Appearance] 主题切换完成（已应用并已尝试写入配置文件）:', next)
  } catch (error) {
    logger.error('[Appearance] 主题切换失败（可能未写入配置文件）:', error)
  } finally {
    themeSaving.value = false
  }
}
</script>
