<template>
  <div class="llm-setting">
    <n-card title="大模型" size="small">
      <n-space vertical size="large" style="width: 100%">
        <n-alert type="info" :show-icon="true">
          配置大模型秘钥，访问秘钥用于调用对应大模型服务。
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

        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="ChatRecord 保存路径">
            <n-input v-model:value="chatRecordRoot" :disabled="loading" placeholder="默认：软件同级目录\ChatRecord" />
          </n-form-item>
          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="loading" @click="pickChatRecordRoot">浏览文件夹...</n-button>
              <n-button :disabled="loading" type="primary" @click="saveChatRecordRoot">保存</n-button>
            </n-space>
          </n-form-item>
        </n-form>

        <n-divider />

        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="Markdown 转换模型">
            <n-input v-model:value="markdownModelsArchivePath" :disabled="installingModels" placeholder="选择模型压缩包（.zip）" />
          </n-form-item>
          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="installingModels" @click="pickMarkdownModelsArchive">选择压缩包...</n-button>
              <n-button :disabled="installingModels" :loading="installingModels" type="primary" @click="installMarkdownModels">安装</n-button>
            </n-space>
          </n-form-item>
        </n-form>

        <n-divider />

        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="模型运行环境">
            <n-input v-model:value="markdownConvertorEnvArchivePath" :disabled="copyingEnv" placeholder="选择运行环境压缩包（将拷贝到 utils/MarkdownConvertor）" />
          </n-form-item>
          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="copyingEnv" @click="pickMarkdownConvertorEnvArchive">选择压缩包...</n-button>
              <n-button :disabled="copyingEnv" :loading="copyingEnv" type="primary" @click="installMarkdownConvertorEnv">拷贝</n-button>
            </n-space>
          </n-form-item>
        </n-form>

        <n-divider />

        <div class="status">
          <div class="status-title">当前状态</div>
          <n-list hoverable style="max-width: 100%">
            <n-list-item>
              <template #prefix><span class="status-prefix">ChatGPT</span></template>
              <template #suffix>{{ form.chatgpt_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">DeepSeek</span></template>
              <template #suffix>{{ form.deepseek_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">Qwen</span></template>
              <template #suffix>{{ form.qwen_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">Kimi</span></template>
              <template #suffix>{{ form.kimi_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">豆包</span></template>
              <template #suffix>{{ form.doubao_api_key?.trim() ? '已配置' : '未配置' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">Markdown模型</span></template>
              <template #suffix>{{ markdownModelsStatus.installed ? '已加载' : '未加载' }}</template>
            </n-list-item>
            <n-list-item>
              <template #prefix><span class="status-prefix">转换环境</span></template>
              <template #suffix>{{ markdownEnvStatus.installed ? '已加载' : '未加载' }}</template>
            </n-list-item>
          </n-list>
        </div>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { reactive, ref, onMounted } from 'vue'
import { NCard, NSpace, NAlert, NForm, NFormItem, NInput, NButton, NDivider, NList, NListItem, useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const loading = ref(false)
const message = useMessage()

const chatRecordRoot = ref('')

const markdownModelsArchivePath = ref('')
const markdownConvertorEnvArchivePath = ref('')
const installingModels = ref(false)
const copyingEnv = ref(false)

const markdownModelsStatus = ref({ installed: false, expected_path: '' })
const markdownEnvStatus = ref({ installed: false, expected_path: '' })

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

    const root = await invoke('get_chat_record_root')
    chatRecordRoot.value = typeof root === 'string' ? root : ''

    const modelsStatus = await invoke('get_markdown_models_status')
    markdownModelsStatus.value = {
      installed: Boolean(modelsStatus?.installed),
      expected_path: typeof modelsStatus?.expected_path === 'string' ? modelsStatus.expected_path : '',
    }

    const envStatus = await invoke('get_markdown_convertor_env_status')
    markdownEnvStatus.value = {
      installed: Boolean(envStatus?.installed),
      expected_path: typeof envStatus?.expected_path === 'string' ? envStatus.expected_path : '',
    }
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
    message.success('已保存大模型秘钥')
  } catch (e) {
    console.error('保存大模型配置失败:', e)
    message.error('保存大模型秘钥失败')
  } finally {
    loading.value = false
  }
}

const pickChatRecordRoot = async () => {
  try {
    const selected = await open({ directory: true, multiple: false })
    if (selected) chatRecordRoot.value = String(selected)
  } catch (e) {
    console.error('选择 ChatRecord 目录失败:', e)
  }
}

const saveChatRecordRoot = async () => {
  loading.value = true
  try {
    await invoke('set_chat_record_root', { path: chatRecordRoot.value })
    const root = await invoke('get_chat_record_root')
    chatRecordRoot.value = typeof root === 'string' ? root : chatRecordRoot.value
    message.success('已保存 ChatRecord 路径')
  } catch (e) {
    console.error('保存 ChatRecord 路径失败:', e)
    message.error('保存 ChatRecord 路径失败')
  } finally {
    loading.value = false
  }
}

const pickMarkdownModelsArchive = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Archive', extensions: ['zip'] }],
    })
    if (selected) markdownModelsArchivePath.value = String(selected)
  } catch (e) {
    console.error('选择 Markdown 转换模型压缩包失败:', e)
  }
}

const installMarkdownModels = async () => {
  const p = markdownModelsArchivePath.value.trim()
  if (!p) {
    message.warning('请先选择模型压缩包')
    return
  }

  installingModels.value = true
  try {
    message.info('开始安装模型（预计约10分钟）')
    const installedTo = await invoke('install_markdown_models_archive', { archivePath: p })
    message.success(`模型已安装到: ${installedTo}`)
  } catch (e) {
    console.error('安装 Markdown 转换模型失败:', e)
    message.error('安装 Markdown 转换模型失败')
  } finally {
    installingModels.value = false
  }
}

const pickMarkdownConvertorEnvArchive = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Archive', extensions: ['zip', '7z', 'rar'] }],
    })
    if (selected) markdownConvertorEnvArchivePath.value = String(selected)
  } catch (e) {
    console.error('选择模型运行环境压缩包失败:', e)
  }
}

const installMarkdownConvertorEnv = async () => {
  const p = markdownConvertorEnvArchivePath.value.trim()
  if (!p) {
    message.warning('请先选择运行环境压缩包')
    return
  }

  copyingEnv.value = true
  try {
    message.info('开始拷贝运行环境（预计约10分钟）')
    const installedTo = await invoke('install_markdown_convertor_env_archive', { archivePath: p })
    message.success(`运行环境压缩包已拷贝到: ${installedTo}`)
  } catch (e) {
    console.error('拷贝模型运行环境压缩包失败:', e)
    message.error('拷贝模型运行环境压缩包失败')
  } finally {
    copyingEnv.value = false
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

.status-prefix {
  display: inline-block;
  min-width: 72px;
  white-space: nowrap;
}
</style>