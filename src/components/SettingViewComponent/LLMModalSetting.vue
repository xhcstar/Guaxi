<template>
  <div class="llm-setting">
    <n-card title="大模型" size="small">
      <n-space vertical size="large" style="width: 100%">
        <n-alert type="info" :show-icon="true">
          访问秘钥将写入本地配置文件（`config/guaxi_config.json`），用于后端调用对应大模型服务。
        </n-alert>

        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="ChatGPT API Key">
            <n-input v-model:value="form.chatgpt_api_key" :disabled="loading" type="password" show-password-on="click"
              placeholder="sk-..." />
          </n-form-item>

          <n-form-item label="DeepSeek API Key">
            <n-input v-model:value="form.deepseek_api_key" :disabled="loading" type="password" show-password-on="click"
              placeholder="请输入 DeepSeek Key" />
          </n-form-item>

          <n-form-item label="Qwen API Key">
            <n-input v-model:value="form.qwen_api_key" :disabled="loading" type="password" show-password-on="click"
              placeholder="请输入 Qwen Key" />
          </n-form-item>

          <n-form-item label="Kimi API Key">
            <n-input v-model:value="form.kimi_api_key" :disabled="loading" type="password" show-password-on="click"
              placeholder="请输入 Kimi Key" />
          </n-form-item>

          <n-form-item label="豆包 API Key">
            <n-input v-model:value="form.doubao_api_key" :disabled="loading" type="password" show-password-on="click"
              placeholder="请输入 豆包 Key" />
          </n-form-item>

          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="loading" @click="loadConfig">刷新</n-button>
              <n-button :disabled="loading" type="primary" @click="saveConfig">保存</n-button>
            </n-space>
          </n-form-item>
        </n-form>

        <n-divider />

        <div class="status">
          <div class="status-title">当前状态</div>
          <n-list hoverable style="max-width: 100%">
            <n-list-item>
              <template #prefix>ChatGPT</template>
              <template #suffix>{{ form.chatgpt_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix>DeepSeek</template>
              <template #suffix>{{ form.deepseek_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix>Qwen</template>
              <template #suffix>{{ form.qwen_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix>Kimi</template>
              <template #suffix>{{ form.kimi_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix>豆包</template>
              <template #suffix>{{ form.doubao_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
          </n-list>
        </div>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { reactive, ref, onMounted } from 'vue'
import { NCard, NSpace, NAlert, NForm, NFormItem, NInput, NButton, NDivider, NList, NListItem } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const loading = ref(false)

const form = reactive({
  chatgpt_api_key: '',
  deepseek_api_key: '',
  qwen_api_key: '',
  kimi_api_key: '',
  doubao_api_key: '',
})

const loadConfig = async () => {
  loading.value = true
  try {
    const cfg = await invoke('get_llm_config')
    form.chatgpt_api_key = typeof cfg?.chatgpt_api_key === 'string' ? cfg.chatgpt_api_key : ''
    form.deepseek_api_key = typeof cfg?.deepseek_api_key === 'string' ? cfg.deepseek_api_key : ''
    form.qwen_api_key = typeof cfg?.qwen_api_key === 'string' ? cfg.qwen_api_key : ''
    form.kimi_api_key = typeof cfg?.kimi_api_key === 'string' ? cfg.kimi_api_key : ''
    form.doubao_api_key = typeof cfg?.doubao_api_key === 'string' ? cfg.doubao_api_key : ''
  } catch (e) {
    console.error('加载大模型配置失败:', e)
  } finally {
    loading.value = false
  }
}

const saveConfig = async () => {
  loading.value = true
  try {
    await invoke('set_llm_config', {
      llm_config: {
        chatgpt_api_key: form.chatgpt_api_key,
        deepseek_api_key: form.deepseek_api_key,
        qwen_api_key: form.qwen_api_key,
        kimi_api_key: form.kimi_api_key,
        doubao_api_key: form.doubao_api_key,
      },
    })
  } catch (e) {
    console.error('保存大模型配置失败:', e)
  } finally {
    loading.value = false
  }
}

onMounted(loadConfig)
</script>

<style scoped>
.llm-setting {
  width: 100%;
  box-sizing: border-box;
}

.status-title {
  font-weight: 600;
  margin-bottom: 8px;
}
</style>