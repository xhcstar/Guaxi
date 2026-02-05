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

        <n-form label-placement="left" :label-width="220" :show-require-mark="false">
          <n-form-item label="Markdown转换模型" :label-style="{ whiteSpace: 'normal', lineHeight: '1.2' }">
            <n-input v-model:value="markdownModelsArchivePath" :disabled="installingModels" placeholder="选择Markdown转换模型" />
          </n-form-item>
          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="installingModels" @click="pickMarkdownModelsArchive">选择压缩包...</n-button>
              <n-button :disabled="installingModels" :loading="installingModels" type="primary" @click="installMarkdownModels">加载</n-button>
            </n-space>
          </n-form-item>
        </n-form>

        <n-divider />

        <n-form label-placement="left" :label-width="220" :show-require-mark="false">
          <n-form-item label="Markdown转换模型运行环境" :label-style="{ whiteSpace: 'normal', lineHeight: '1.2' }">
            <n-input v-model:value="markdownConvertorEnvArchivePath" :disabled="copyingEnv" placeholder="选择Markdown转换模型运行环境" />
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
          <div class="status-header">
            <div class="status-title">
              <n-space align="center" size="small">
                <n-icon :component="InformationCircleOutline" />
                <span>当前状态</span>
              </n-space>
            </div>
            <n-text depth="3" class="status-subtitle">鼠标悬停状态查看提示与期望路径</n-text>
          </div>

          <div class="status-grid" role="list">
            <div v-for="item in statusItems" :key="item.id" class="status-item" :class="`is-${item.state}`" role="listitem">
              <div class="status-left">
                <n-icon :component="item.icon" />
                <span class="status-name">{{ item.name }}</span>
              </div>
              <div class="status-right">
                <div class="status-pop-wrap" tabindex="0">
                  <span class="status-badge">
                    <n-icon :component="item.badgeIcon" />
                    <span>{{ item.badgeText }}</span>
                  </span>
                  <div class="status-popover" role="tooltip">
                    <div class="status-tip-title">{{ item.tipTitle }}</div>
                    <div v-for="(line, idx) in item.tipLines" :key="idx" class="status-tip-desc">{{ line }}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { reactive, ref, onMounted, computed } from 'vue'
import { NCard, NSpace, NAlert, NForm, NFormItem, NInput, NButton, NDivider, NIcon, NText, useMessage } from 'naive-ui'
import { CheckmarkCircleOutline, CloseCircleOutline, ConstructOutline, CubeOutline, InformationCircleOutline, KeyOutline, TimeOutline } from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const logger = createLogger('LLMModalSetting')

const loading = ref(false)
const message = useMessage()

const chatRecordRoot = ref('')

const markdownModelsArchivePath = ref('')
const markdownConvertorEnvArchivePath = ref('')
const installingModels = ref(false)
const copyingEnv = ref(false)

const markdownModelsStatus = ref({ installed: false, expected_path: '' })
const markdownEnvStatus = ref({ installed: false, expected_path: '' })

const hasKey = (v) => typeof v === 'string' && v.trim().length > 0

const statusItems = computed(() => {
  const keyItem = (id, name, value) => {
    const ok = hasKey(value)
    return {
      id,
      name,
      icon: KeyOutline,
      state: ok ? 'ok' : 'warn',
      badgeText: ok ? '已配置' : '未配置',
      badgeIcon: ok ? CheckmarkCircleOutline : CloseCircleOutline,
      tipTitle: `${name} API Key`,
      tipLines: ['在上方输入并点击“保存”后生效'],
    }
  }

  const modelsState = loading.value ? 'info' : (markdownModelsStatus.value.installed ? 'ok' : 'warn')
  const envState = loading.value ? 'info' : (markdownEnvStatus.value.installed ? 'ok' : 'warn')

  return [
    keyItem('chatgpt', 'ChatGPT', form.chatgpt_api_key),
    keyItem('deepseek', 'DeepSeek', form.deepseek_api_key),
    keyItem('qwen', 'Qwen', form.qwen_api_key),
    keyItem('kimi', 'Kimi', form.kimi_api_key),
    keyItem('doubao', '豆包', form.doubao_api_key),
    {
      id: 'markdown_models',
      name: 'Markdown模型',
      icon: CubeOutline,
      state: modelsState,
      badgeText: loading.value ? '检测中' : (markdownModelsStatus.value.installed ? '已加载' : '未加载'),
      badgeIcon: loading.value ? TimeOutline : (markdownModelsStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline),
      tipTitle: 'Markdown 转换模型',
      tipLines: [`期望路径：${markdownModelsStatus.value.expected_path || '-'}`, '未加载时请选择 surya_models.zip 并点击“安装”'],
    },
    {
      id: 'markdown_env',
      name: '转换环境',
      icon: ConstructOutline,
      state: envState,
      badgeText: loading.value ? '检测中' : (markdownEnvStatus.value.installed ? '已加载' : '未加载'),
      badgeIcon: loading.value ? TimeOutline : (markdownEnvStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline),
      tipTitle: '模型运行环境',
      tipLines: [`期望路径：${markdownEnvStatus.value.expected_path || '-'}`, '未加载时请选择 MarkdownConvertor.zip 并点击“拷贝”'],
    },
  ]
})

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
    logger.error('加载大模型配置失败:', e)
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
    logger.error('保存大模型配置失败:', e)
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
    logger.error('选择 ChatRecord 目录失败:', e)
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
    logger.error('保存 ChatRecord 路径失败:', e)
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
    logger.error('选择 Markdown 转换模型压缩包失败:', e)
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
    logger.error('安装 Markdown 转换模型失败:', e)
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
    logger.error('选择模型运行环境压缩包失败:', e)
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
    logger.error('拷贝模型运行环境压缩包失败:', e)
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

.status-header {
  margin-bottom: 8px;
}

.status-title {
  font-weight: 600;
  margin-bottom: 2px;
}

.status-subtitle {
  display: block;
  line-height: 1.2;
}

.status-grid {
  display: grid;
  gap: 8px;
}

.status-item {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) auto;
  gap: 12px;
  align-items: center;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(128, 128, 128, 0.22);
}

.status-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.status-name {
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.status-pop-wrap {
  position: relative;
  outline: none;
}

.status-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 12px;
  line-height: 1;
  user-select: none;
  border: 1px solid transparent;
}

.is-ok .status-badge {
  background: rgba(16, 185, 129, 0.14);
  border-color: rgba(16, 185, 129, 0.32);
  color: rgb(16, 185, 129);
}

.is-warn .status-badge {
  background: rgba(245, 158, 11, 0.14);
  border-color: rgba(245, 158, 11, 0.35);
  color: rgb(245, 158, 11);
}

.is-info .status-badge {
  background: rgba(59, 130, 246, 0.14);
  border-color: rgba(59, 130, 246, 0.32);
  color: rgb(59, 130, 246);
}

.status-popover {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  min-width: 280px;
  max-width: 420px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(128, 128, 128, 0.25);
  background: rgba(0, 0, 0, 0.86);
  color: #fff;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.24);
  display: none;
  z-index: 5;
}

.status-pop-wrap:hover .status-popover,
.status-pop-wrap:focus .status-popover,
.status-pop-wrap:focus-within .status-popover {
  display: block;
}

.status :deep(.n-icon) {
  --n-size: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
}

.status :deep(.n-icon svg) {
  display: block;
}

.status-tip-title {
  font-weight: 600;
  margin-bottom: 4px;
}

.status-tip-desc {
  font-size: 12px;
  line-height: 1.3;
  opacity: 0.92;
}
</style>