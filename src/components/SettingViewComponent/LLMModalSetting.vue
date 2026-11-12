<template>
  <div class="llm-setting setting-page">
    <n-card class="setting-page-card" title="大模型" size="small">
      <n-space vertical size="large" style="width: 100%">
        <n-alert type="info" :show-icon="true">
          大模型配置分两类：直接填服务商 API Key，或启用 cc-switch 本地代理。
          普通对话与 Agent 共用同一份配置，保存一次即全部生效。
        </n-alert>

        <n-tabs v-model:value="activeTab" type="line" animated>
          <n-tab-pane name="providers" tab="服务商 API">
            <n-form label-placement="left" label-width="auto" :show-require-mark="false">
              <n-form-item label="ChatGPT API Key">
                <n-input v-model:value="form.chatgpt_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="sk-..." />
              </n-form-item>

              <n-form-item label="DeepSeek API Key">
                <n-input v-model:value="form.deepseek_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="请输入 DeepSeek Key" />
              </n-form-item>

              <n-form-item label="Qwen API Key">
                <n-input v-model:value="form.qwen_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="请输入 Qwen Key" />
              </n-form-item>

              <n-form-item label="Kimi API Key">
                <n-input v-model:value="form.kimi_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="请输入 Kimi Key" />
              </n-form-item>

              <n-form-item label="豆包 API Key">
                <n-input v-model:value="form.doubao_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="请输入 豆包 Key" />
              </n-form-item>

              <n-form-item label="硅基流动 API Key">
                <n-input v-model:value="form.siliconflow_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="请输入 硅基流动 Key" />
              </n-form-item>

              <n-divider style="margin: 8px 0">自定义 OpenAI 兼容服务</n-divider>

              <n-form-item label="自定义 Base URL">
                <n-input v-model:value="form.custom_base_url" :disabled="configLoading"
                  placeholder="如 https://openrouter.ai/api/v1 或 https://open.bigmodel.cn/api/paas/v4" />
              </n-form-item>

              <n-form-item label="自定义 API Key">
                <n-input v-model:value="form.custom_api_key" :disabled="configLoading" type="password" show-password-on="click"
                  placeholder="OpenAI 兼容服务的 API Key" />
              </n-form-item>

              <n-form-item label="自定义模型名">
                <n-input v-model:value="form.custom_model" :disabled="configLoading"
                  placeholder="如 deepseek/deepseek-chat、glm-4-plus 等" />
              </n-form-item>
            </n-form>

            <n-alert type="default" :show-icon="false" style="margin-top: 8px; font-size: 12px">
              服务商 API Key 只服务普通对话；Agent 走哪条路见「Agent」标签页：
              {{ agentRoute.text }}
            </n-alert>
          </n-tab-pane>

          <n-tab-pane name="cc-switch" tab="cc-switch 代理">
            <n-alert type="info" :show-icon="false" style="margin-bottom: 12px">
              启用 cc-switch 后，Guaxi 只记住代理地址，具体用哪家供应商由 cc-switch 决定；
              在 cc-switch 里切换供应商，普通对话与 Agent 会同时跟随，无需回到这里改配置。
            </n-alert>

            <n-form label-placement="left" label-width="auto" :show-require-mark="false">
              <n-form-item label="启用 cc-switch 代理">
                <n-space align="center">
                  <n-switch v-model:value="form.cc_switch_enabled" :disabled="configLoading" />
                  <n-button size="small" :loading="detecting" :disabled="detecting" @click="detectCcSwitch">自动探测</n-button>
                  <n-text depth="3">{{ ccSwitchHint }}</n-text>
                </n-space>
              </n-form-item>

              <n-form-item label="cc-switch 代理地址">
                <n-input v-model:value="form.cc_switch_base_url" :disabled="configLoading"
                  placeholder="http://127.0.0.1:15721/v1" />
              </n-form-item>

              <n-form-item label="cc-switch 模型名">
                <n-input v-model:value="form.cc_switch_model" :disabled="configLoading"
                  placeholder="代理转发时使用的模型名，如 gpt-5.1-codex" />
              </n-form-item>

              <n-form-item label="cc-switch API Key">
                <n-input v-model:value="form.cc_switch_api_key" :disabled="configLoading" type="password"
                  show-password-on="click" placeholder="留空即可，代理会用上游供应商的密钥" />
              </n-form-item>

              <n-form-item label="cc-switch 协议">
                <n-select v-model:value="form.cc_switch_wire_api" :disabled="configLoading" :options="wireApiOptions"
                  style="max-width: 260px" />
              </n-form-item>
              <n-text depth="3" style="display: block; margin: -8px 0 12px 0; font-size: 12px">
                该选项只影响“对话”模式；Agent 固定使用 Responses 协议（codex 0.149 起不再支持 chat）。
              </n-text>
            </n-form>
          </n-tab-pane>

          <n-tab-pane name="agent" tab="Agent">
            <n-alert :type="agentRoute.type" :show-icon="true" style="margin-bottom: 12px">
              {{ agentRoute.text }}
            </n-alert>

            <n-form label-placement="left" label-width="auto" :show-require-mark="false">
              <n-form-item label="Agent 引擎">
                <n-select v-model:value="form.agent_engine" :disabled="configLoading" :options="agentEngineOptions"
                  style="max-width: 260px" />
              </n-form-item>

              <n-form-item label="Agent 模型名">
                <n-input v-model:value="form.codex_model" :disabled="configLoading"
                  placeholder="留空则沿用 Agent 供应商的模型名" />
              </n-form-item>

              <n-form-item label="Agent 沙箱">
                <n-select v-model:value="form.codex_sandbox" :disabled="configLoading" :options="codexSandboxOptions"
                  style="max-width: 300px" />
              </n-form-item>

              <n-form-item label="Agent 配置目录">
                <n-input :value="codexStatus.codex_home" readonly placeholder="首次运行 Agent 后生成" />
              </n-form-item>
            </n-form>
          </n-tab-pane>
        </n-tabs>

        <n-space justify="end">
          <n-button :disabled="saving" @click="loadConfig">刷新</n-button>
          <n-button :disabled="saving || configLoading" :loading="saving" type="primary" @click="saveConfig">保存</n-button>
        </n-space>

        <n-divider />

        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="ChatRecord 保存路径">
            <n-input v-model:value="chatRecordRoot" :disabled="configLoading" placeholder="默认：软件同级目录\ChatRecord" />
          </n-form-item>
          <n-form-item label="操作">
            <n-space>
              <n-button :disabled="savingChatRecord" @click="pickChatRecordRoot">浏览文件夹...</n-button>
              <n-button :disabled="savingChatRecord" :loading="savingChatRecord" type="primary" @click="saveChatRecordRoot">保存</n-button>
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
              <n-button :disabled="installingModels || downloadingModels" :loading="installingModels" type="primary" @click="installMarkdownModels">加载</n-button>
              <n-button :disabled="installingModels || downloadingModels" :loading="downloadingModels" @click="downloadMarkdownModels">在线下载</n-button>
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

          <!-- Marker 运行环境：首次启动后台自动装配（uv 建独立 venv + 按 GPU 选 torch） -->
          <div class="status-item" :class="`is-${markerEnvState}`" role="listitem">
            <div class="status-left">
              <n-icon :component="ConstructOutline" />
              <span class="status-name">Marker 运行环境</span>
            </div>
            <div class="status-right">
              <n-select
                size="tiny"
                style="width: 150px"
                :value="markerReadingOrder"
                :options="markerReadingOrderOptions"
                :disabled="configLoading || savingMarkerReadingOrder"
                @update:value="changeMarkerReadingOrder"
              />
              <n-select
                size="tiny"
                style="width: 168px"
                :value="markerOcrMode"
                :options="markerOcrModeOptions"
                :disabled="configLoading || savingMarkerOcr"
                @update:value="changeMarkerOcrMode"
              />
              <n-button
                size="tiny"
                type="primary"
                ghost
                :loading="preparingMarkerEnv"
                :disabled="configLoading || preparingMarkerEnv"
                @click="prepareMarkerEnv"
              >
                准备 / 重试
              </n-button>
              <div class="status-pop-wrap" tabindex="0">
                <span class="status-badge">
                  <n-icon :component="markerEnvBadgeIcon" />
                  <span>{{ markerEnvBadgeText }}</span>
                </span>
                <div class="status-popover" role="tooltip">
                  <div class="status-tip-title">Marker 运行环境</div>
                  <div class="status-tip-desc">首次启动会在后台自动装配（联网即可，无需手动配置）</div>
                  <div class="status-tip-desc">torch：{{ engineStatus.markerCuda ? 'CUDA 版（GPU 加速）' : 'CPU 版' }}</div>
                  <div class="status-tip-desc">OCR 策略：{{ markerOcrModeHint }}</div>
                  <div class="status-tip-desc">阅读顺序：{{ markerReadingOrderHint }}</div>
                  <div v-if="engineStatus.markerDegraded" class="status-tip-desc">
                    降级模式：定制源码未取到，当前使用通用版 marker-pdf（功能不受影响）
                  </div>
                  <div class="status-tip-desc">环境目录: {{ engineStatus.markerEnvDir || '-' }}</div>
                  <div v-if="engineStatus.markerError" class="status-tip-desc">最近错误: {{ engineStatus.markerError }}</div>
                </div>
              </div>
            </div>
          </div>

          <div class="status-item" :class="`is-${localDeepseekState}`" role="listitem">
            <div class="status-left">
              <n-icon :component="CubeOutline" />
              <span class="status-name">LocalDeepseek 模型</span>
            </div>
            <div class="status-right">
              <n-button
                size="tiny"
                type="primary"
                ghost
                :loading="downloadingLocalDeepseek"
                :disabled="configLoading || downloadingLocalDeepseek || localDeepseekStatus.installed"
                @click="downloadLocalDeepseekModel"
              >
                下载
              </n-button>
              <n-button
                size="tiny"
                ghost
                :disabled="configLoading || downloadingLocalDeepseek"
                @click="importLocalDeepseekModel"
              >
                导入
              </n-button>
              <div class="status-pop-wrap" tabindex="0">
                <span class="status-badge">
                  <n-icon :component="localDeepseekBadgeIcon" />
                  <span>{{ localDeepseekBadgeText }}</span>
                </span>
                <div class="status-popover" role="tooltip">
                  <div class="status-tip-title">LocalDeepseek</div>
                  <div class="status-tip-desc">模型路径: {{ localDeepseekStatus.expected_path || '-' }}</div>
                  <div class="status-tip-desc">llama-server: {{ localDeepseekStatus.llama_server_path || '-' }}</div>
                  <div class="status-tip-desc">
                    {{ localDeepseekStatus.llama_server_exists ? 'llama.cpp 服务端可用' : '请将 llama.cpp 构建版本放到 utils/llama' }}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="status-item" :class="`is-${localQwenState}`" role="listitem">
            <div class="status-left">
              <n-icon :component="CubeOutline" />
              <span class="status-name">LocalQwen 模型</span>
            </div>
            <div class="status-right">
              <n-button
                size="tiny"
                type="primary"
                ghost
                :loading="downloadingLocalQwen"
                :disabled="configLoading || downloadingLocalQwen || localQwenStatus.installed"
                @click="downloadLocalQwenModel"
              >
                下载
              </n-button>
              <n-button
                size="tiny"
                ghost
                :disabled="configLoading || downloadingLocalQwen"
                @click="importLocalQwenModel"
              >
                导入
              </n-button>
              <div class="status-pop-wrap" tabindex="0">
                <span class="status-badge">
                  <n-icon :component="localQwenBadgeIcon" />
                  <span>{{ localQwenBadgeText }}</span>
                </span>
                <div class="status-popover" role="tooltip">
                  <div class="status-tip-title">LocalQwen</div>
                  <div class="status-tip-desc">模型路径: {{ localQwenStatus.expected_path || '-' }}</div>
                  <div class="status-tip-desc">llama-server: {{ localQwenStatus.llama_server_path || '-' }}</div>
                  <div class="status-tip-desc">
                    {{ localQwenStatus.llama_server_exists ? 'llama.cpp 服务端可用' : '请将 llama.cpp 构建版本放到 utils/llama' }}
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div class="status-item" :class="`is-${translateModelState}`" role="listitem">
            <div class="status-left">
              <n-icon :component="CubeOutline" />
              <span class="status-name">翻译模型（划词/全文）</span>
            </div>
            <div class="status-right">
              <n-select
                size="tiny"
                style="width: 190px"
                :value="translateLocalStatus.active_model"
                :options="translateModelOptions"
                :disabled="configLoading || downloadingTranslateModel"
                @update:value="changeTranslateModel"
              />
              <n-button
                size="tiny"
                type="primary"
                ghost
                :loading="downloadingTranslateModel || translateLocalStatus.downloading"
                :disabled="configLoading || downloadingTranslateModel || translateLocalStatus.installed"
                @click="downloadTranslateModel"
              >
                下载
              </n-button>
              <n-button
                size="tiny"
                ghost
                :disabled="configLoading || downloadingTranslateModel"
                @click="importTranslateModel"
              >
                导入
              </n-button>
              <div class="status-pop-wrap" tabindex="0">
                <span class="status-badge">
                  <n-icon :component="translateModelBadgeIcon" />
                  <span>{{ translateModelBadgeText }}</span>
                </span>
                <div class="status-popover" role="tooltip">
                  <div class="status-tip-title">翻译专用小模型</div>
                  <div class="status-tip-desc">只服务单词/句子/段落/全文翻译，与大模型对话无关</div>
                  <div class="status-tip-desc">免费翻译接口全部失效或被限流时自动接管</div>
                  <div class="status-tip-desc">模型路径: {{ translateLocalStatus.expected_path || '-' }}</div>
                  <div class="status-tip-desc">
                    推理服务: {{ translateLocalStatus.ready ? '运行中' : '按需启动（未运行不占内存）' }}
                  </div>
                  <div class="status-tip-desc">
                    {{ translateLocalStatus.llama_server_exists ? 'llama.cpp 服务端可用' : '请将 llama.cpp 构建版本放到 utils/llama' }}
                  </div>
                  <div class="status-tip-desc">首次启动会在后台自动下载（优先魔搭），也可在此手动触发</div>
                </div>
              </div>
            </div>
          </div>

          <div class="status-item" :class="`is-${translateCacheState}`" role="listitem">
            <div class="status-left">
              <n-icon :component="CubeOutline" />
              <span class="status-name">翻译缓存</span>
            </div>
            <div class="status-right">
              <n-button
                size="tiny"
                ghost
                :loading="clearingTranslateCache"
                :disabled="configLoading || clearingTranslateCache || (!translateCacheStats.rows && !translateCacheStats.word_rows)"
                @click="clearTranslateCache"
              >
                清空
              </n-button>
              <div class="status-pop-wrap" tabindex="0">
                <span class="status-badge">
                  <n-icon :component="clearingTranslateCache ? TimeOutline : CheckmarkCircleOutline" />
                  <span>{{ translateCacheBadgeText }}</span>
                </span>
                <div class="status-popover" role="tooltip">
                  <div class="status-tip-title">翻译结果持久缓存</div>
                  <div class="status-tip-desc">译文 {{ translateCacheStats.rows }} 段，累计命中 {{ translateCacheStats.total_hits }} 次</div>
                  <div class="status-tip-desc">词条 {{ translateCacheStats.word_rows }} 条，累计命中 {{ translateCacheStats.word_hits }} 次</div>
                  <div class="status-tip-desc">占用 {{ formatBytes(translateCacheStats.db_size_bytes) }}，译文上限 {{ translateCacheStats.max_rows }} 段（超出按最久未用淘汰）</div>
                  <div class="status-tip-desc">路径: {{ translateCacheStats.db_path || '-' }}</div>
                  <div class="status-tip-desc">重开同一篇文档做全文翻译、或重查同一个单词，都直接读缓存</div>
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
import { NCard, NSpace, NAlert, NForm, NFormItem, NInput, NButton, NDivider, NIcon, NText, NSwitch, NSelect, NTabs, NTabPane, useMessage } from 'naive-ui'
import { CheckmarkCircleOutline, CloseCircleOutline, ConstructOutline, CubeOutline, InformationCircleOutline, KeyOutline, TimeOutline } from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const logger = createLogger('LLMModalSetting')

// 配置读写只锁自己那一小段：读/写配置时禁用表单，状态探测全程在后台跑。
const configLoading = ref(false)
const saving = ref(false)
const savingChatRecord = ref(false)
const detecting = ref(false)
const activeTab = ref('providers')
const message = useMessage()

const chatRecordRoot = ref('')

const markdownModelsArchivePath = ref('')
const installingModels = ref(false)
const downloadingModels = ref(false)

const markdownModelsStatus = ref({ installed: false, expected_path: '' })

// 转换引擎装配状态（后端 `conversion_engine_status`）。
// marker 的运行环境与模型都是**启动时后台自动装配**的，这里只做展示 + 手动重试；
// 运行环境不再提供离线压缩包加载（已改为自动装配），模型仍保留离线加载。
const engineStatus = ref({
  markerReady: false,
  markerProvisioning: false,
  markerError: '',
  markerCuda: false,
  markerDegraded: false,
  markerModelsReady: false,
  markerModelsError: '',
  markerModelsDir: '',
  markerEnvDir: '',
})
const preparingMarkerEnv = ref(false)
const codexStatus = ref({
  binary_path: '',
  binary_exists: false,
  version: '',
  codex_home: '',
  provider_base_url: '',
  provider_model: '',
  provider_ready: false,
  sandbox: 'workspace-write',
  hint: '',
})
const ccSwitchDetection = ref({ found: false, proxy_enabled: false, current_provider_name: '', hint: '' })
const ccSwitchHint = computed(() => {
  const detection = ccSwitchDetection.value
  if (!detection.hint) return '点击"自动探测"读取本机 cc-switch 配置'
  if (detection.current_provider_name) {
    return `${detection.hint}（当前供应商：${detection.current_provider_name}）`
  }
  return detection.hint
})
const localDeepseekStatus = ref({
  installed: false,
  expected_path: '',
  llama_server_exists: false,
  llama_server_path: '',
})
const downloadingLocalDeepseek = ref(false)

const localQwenStatus = ref({
  installed: false,
  expected_path: '',
  llama_server_exists: false,
  llama_server_path: '',
})
const downloadingLocalQwen = ref(false)

// 翻译专用小模型（llama.cpp）：只服务划词/句子/段落/全文翻译，和上面的大模型对话无关。
// 免费翻译接口全挂 / 被风控 / 额度用完时由它兜底，因此首次启动会在后台自动下载。
const translateLocalStatus = ref({
  enabled: true,
  ready: false,
  downloading: false,
  active_model: '',
  installed: false,
  expected_path: '',
  llama_server_exists: false,
  llama_server_path: '',
  models: [],
})
const downloadingTranslateModel = ref(false)

const translateModelOptions = computed(() =>
  (translateLocalStatus.value.models || []).map((m) => ({
    label: `${m.key}（${m.note}）`,
    value: m.key,
  }))
)

const translateModelState = computed(() => {
  if (downloadingTranslateModel.value || translateLocalStatus.value.downloading) return 'info'
  if (!translateLocalStatus.value.enabled) return 'warn'
  return translateLocalStatus.value.installed ? 'ok' : 'warn'
})

const translateModelBadgeText = computed(() => {
  if (downloadingTranslateModel.value || translateLocalStatus.value.downloading) return 'Downloading'
  if (!translateLocalStatus.value.enabled) return 'Disabled'
  return translateLocalStatus.value.installed ? 'Installed' : 'Not installed'
})

const translateModelBadgeIcon = computed(() => {
  if (downloadingTranslateModel.value || translateLocalStatus.value.downloading) return TimeOutline
  return translateLocalStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline
})

// 翻译结果持久缓存：全文翻译一次上百段，缓存住之后重开同一篇基本不再打接口。
const translateCacheStats = ref({
  available: false,
  db_path: '',
  rows: 0,
  total_hits: 0,
  word_rows: 0,
  word_hits: 0,
  db_size_bytes: 0,
  max_rows: 0,
})
const clearingTranslateCache = ref(false)

const formatBytes = (n) => {
  const bytes = Number(n) || 0
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const translateCacheState = computed(() => {
  if (clearingTranslateCache.value) return 'info'
  return translateCacheStats.value.available ? 'ok' : 'warn'
})

const translateCacheBadgeText = computed(() => {
  if (clearingTranslateCache.value) return 'Clearing'
  if (!translateCacheStats.value.available) return 'Unavailable'
  return `${translateCacheStats.value.rows} 段 / ${translateCacheStats.value.word_rows} 词`
})

const localDeepseekState = computed(() => {
  if (downloadingLocalDeepseek.value) return 'info'
  return localDeepseekStatus.value.installed ? 'ok' : 'warn'
})

const localDeepseekBadgeText = computed(() => {
  if (downloadingLocalDeepseek.value) return 'Downloading'
  return localDeepseekStatus.value.installed ? 'Installed' : 'Not installed'
})

const localDeepseekBadgeIcon = computed(() => {
  if (downloadingLocalDeepseek.value) return TimeOutline
  return localDeepseekStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline
})

const localQwenState = computed(() => {
  if (downloadingLocalQwen.value) return 'info'
  return localQwenStatus.value.installed ? 'ok' : 'warn'
})

const localQwenBadgeText = computed(() => {
  if (downloadingLocalQwen.value) return 'Downloading'
  return localQwenStatus.value.installed ? 'Installed' : 'Not installed'
})

const localQwenBadgeIcon = computed(() => {
  if (downloadingLocalQwen.value) return TimeOutline
  return localQwenStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline
})

const hasKey = (v) => typeof v === 'string' && v.trim().length > 0

const codexEngineState = computed(() => {
  if (!codexStatus.value.binary_exists) return 'warn'
  return codexStatus.value.provider_ready ? 'ok' : 'warn'
})

const codexEngineBadgeText = computed(() => {
  if (!codexStatus.value.binary_exists) return '未内置'
  return codexStatus.value.version || '已内置'
})

const ccSwitchState = computed(() => {
  if (!form.cc_switch_enabled) return 'warn'
  return codexStatus.value.provider_ready ? 'ok' : 'warn'
})

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

  const modelsState = configLoading.value ? 'info' : (markdownModelsStatus.value.installed ? 'ok' : 'warn')

  return [
    keyItem('chatgpt', 'ChatGPT', form.chatgpt_api_key),
    keyItem('deepseek', 'DeepSeek', form.deepseek_api_key),
    keyItem('qwen', 'Qwen', form.qwen_api_key),
    keyItem('kimi', 'Kimi', form.kimi_api_key),
    keyItem('doubao', '豆包', form.doubao_api_key),
    keyItem('siliconflow', '硅基流动', form.siliconflow_api_key),
    {
      id: 'markdown_models',
      name: 'Markdown模型',
      icon: CubeOutline,
      state: modelsState,
      badgeText: configLoading.value ? '检测中' : (markdownModelsStatus.value.installed ? '已加载' : '未加载'),
      badgeIcon: configLoading.value ? TimeOutline : (markdownModelsStatus.value.installed ? CheckmarkCircleOutline : CloseCircleOutline),
      tipTitle: 'Markdown 转换模型',
      tipLines: [
        `期望路径：${markdownModelsStatus.value.expected_path || '-'}`,
        '启动时会自动在线下载；无网络时可选择 surya_models.zip 并点击“加载”',
      ],
    },
    {
      id: 'codex_engine',
      name: 'Codex 引擎',
      icon: ConstructOutline,
      state: codexEngineState.value,
      badgeText: codexEngineBadgeText.value,
      badgeIcon: codexStatus.value.binary_exists ? CheckmarkCircleOutline : CloseCircleOutline,
      tipTitle: 'Codex Agent 引擎',
      tipLines: [
        `可执行文件：${codexStatus.value.binary_path || '未找到'}`,
        `配置目录：${codexStatus.value.codex_home || '-'}（与用户的 ~/.codex 无关）`,
        `沙箱：${codexStatus.value.sandbox || '-'}`,
        codexStatus.value.hint || '缺失时 Agent 会自动回退到内置 Python 引擎；可执行 npm run codex:fetch 获取',
      ],
    },
    {
      id: 'cc_switch',
      name: 'cc-switch 代理',
      icon: KeyOutline,
      state: ccSwitchState.value,
      badgeText: form.cc_switch_enabled ? (codexStatus.value.provider_ready ? '已启用' : '配置不全') : '未启用',
      badgeIcon: form.cc_switch_enabled && codexStatus.value.provider_ready ? CheckmarkCircleOutline : CloseCircleOutline,
      tipTitle: 'cc-switch 本地代理',
      tipLines: [
        `代理地址：${form.cc_switch_base_url || '-'}`,
        `供应商模型：${codexStatus.value.provider_model || form.cc_switch_model || '-'}`,
        ccSwitchHint.value,
      ],
    },
  ]
})

const form = reactive({
  chatgpt_api_key: '',
  deepseek_api_key: '',
  qwen_api_key: '',
  kimi_api_key: '',
  doubao_api_key: '',
  siliconflow_api_key: '',
  custom_base_url: '',
  custom_api_key: '',
  custom_model: '',
  // cc-switch 本地代理
  cc_switch_enabled: false,
  cc_switch_base_url: 'http://127.0.0.1:15721/v1',
  cc_switch_api_key: '',
  cc_switch_model: '',
  cc_switch_wire_api: 'chat',
  // Codex Agent
  codex_model: '',
  codex_sandbox: 'workspace-write',
  agent_engine: 'codex',
})

const wireApiOptions = [
  { value: 'chat', label: 'chat（/chat/completions，“对话”模式用这个）' },
  { value: 'responses', label: 'responses（/responses，“对话”模式不可用）' },
]

const agentEngineOptions = [
  { value: 'app-server', label: 'App-Server（工作台默认：可转向、逐动作审批、看 diff）' },
  { value: 'codex', label: 'Codex exec（一轮一进程，兼容旧行为）' },
  { value: 'builtin', label: '内置 Python Agent（回退方案）' },
]

const codexSandboxOptions = [
  { value: 'read-only', label: 'read-only（只读，模型不能改文件）' },
  { value: 'workspace-write', label: 'workspace-write（可改知识库内文件，推荐）' },
  { value: 'danger-full-access', label: 'danger-full-access（不设限，慎用）' },
]

// Agent 与普通对话共用同一份配置，但只认 cc-switch 或自定义 Base URL
// （codex 只支持 Responses 协议，内置服务商多半只有 Chat Completions）。
// 这里把“Agent 现在实际会走哪条路”直接算出来，避免设置页看起来比实际能做的一团乱。
const agentRoute = computed(() => {
  if (form.cc_switch_enabled && form.cc_switch_base_url.trim()) {
    const model = form.codex_model.trim() || form.cc_switch_model.trim()
    return {
      type: 'success',
      text: `Agent 走 cc-switch 代理：${form.cc_switch_base_url.trim()}（模型 ${model || '未填写'}）`,
    }
  }
  if (form.custom_base_url.trim()) {
    const model = form.codex_model.trim() || form.custom_model.trim()
    return {
      type: 'success',
      text: `Agent 走自定义供应商：${form.custom_base_url.trim()}（模型 ${model || '未填写'}）`,
    }
  }
  return {
    type: 'warning',
    text: 'Agent 当前没有可用供应商：需要启用 cc-switch 代理，或填写自定义 Base URL。服务商 API Key 只服务普通对话。',
  }
})

const refreshCodexStatus = async () => {
  try {
    const status = await invoke('codex_agent_status')
    codexStatus.value = {
      binary_path: typeof status?.binary_path === 'string' ? status.binary_path : '',
      binary_exists: Boolean(status?.binary_exists),
      version: typeof status?.version === 'string' ? status.version : '',
      codex_home: typeof status?.codex_home === 'string' ? status.codex_home : '',
      provider_base_url: typeof status?.provider_base_url === 'string' ? status.provider_base_url : '',
      provider_model: typeof status?.provider_model === 'string' ? status.provider_model : '',
      provider_ready: Boolean(status?.provider_ready),
      sandbox: typeof status?.sandbox === 'string' ? status.sandbox : 'workspace-write',
      hint: typeof status?.hint === 'string' ? status.hint : '',
    }
  } catch (e) {
    logger.error('读取 Codex 引擎状态失败:', e)
  }
}

const refreshChatRecordRoot = async () => {
  try {
    const root = await invoke('get_chat_record_root')
    chatRecordRoot.value = typeof root === 'string' ? root : ''
  } catch (e) {
    logger.error('读取 ChatRecord 路径失败:', e)
  }
}

const refreshMarkdownModelsStatus = async () => {
  try {
    const modelsStatus = await invoke('get_markdown_models_status')
    markdownModelsStatus.value = {
      installed: Boolean(modelsStatus?.installed),
      expected_path: typeof modelsStatus?.expected_path === 'string' ? modelsStatus.expected_path : '',
    }
  } catch (e) {
    logger.error('读取 Markdown 转换模型状态失败:', e)
  }
}

const refreshLocalDeepseekStatus = async () => {
  try {
    const localStatus = await invoke('get_local_deepseek_status')
    localDeepseekStatus.value = {
      installed: Boolean(localStatus?.installed),
      expected_path: typeof localStatus?.expected_path === 'string' ? localStatus.expected_path : '',
      llama_server_exists: Boolean(localStatus?.llama_server_exists),
      llama_server_path: typeof localStatus?.llama_server_path === 'string' ? localStatus.llama_server_path : '',
    }
  } catch (e) {
    logger.error('读取 LocalDeepseek 状态失败:', e)
  }
}

const refreshLocalQwenStatus = async () => {
  try {
    const localQwen = await invoke('get_local_qwen_status')
    localQwenStatus.value = {
      installed: Boolean(localQwen?.installed),
      expected_path: typeof localQwen?.expected_path === 'string' ? localQwen.expected_path : '',
      llama_server_exists: Boolean(localQwen?.llama_server_exists),
      llama_server_path: typeof localQwen?.llama_server_path === 'string' ? localQwen.llama_server_path : '',
    }
  } catch (e) {
    logger.error('读取 LocalQwen 状态失败:', e)
  }
}

// 只读探测本机 cc-switch：回填代理地址，并提示当前激活的供应商。
// 探测本身在后端的 blocking 线程里跑，这里只负责按钮 loading，不锁表单。
const detectCcSwitch = async () => {
  if (detecting.value) return
  detecting.value = true
  try {
    const detection = await invoke('detect_cc_switch_config')
    ccSwitchDetection.value = {
      found: Boolean(detection?.found),
      proxy_enabled: Boolean(detection?.proxy_enabled),
      current_provider_name: typeof detection?.current_provider_name === 'string' ? detection.current_provider_name : '',
      hint: typeof detection?.hint === 'string' ? detection.hint : '',
    }
    const baseUrl = typeof detection?.base_url === 'string' ? detection.base_url : ''
    if (baseUrl) {
      form.cc_switch_base_url = baseUrl
    }
    if (detection?.proxy_enabled) {
      message.success('已回填 cc-switch 代理地址，别忘了保存')
    } else {
      message.warning(ccSwitchDetection.value.hint || '未检测到可用的 cc-switch 本地代理')
    }
  } catch (e) {
    logger.error('探测 cc-switch 配置失败:', e)
    message.error('探测 cc-switch 配置失败')
  } finally {
    detecting.value = false
  }
}

// 先读配置本身：这段结束就解锁表单，状态类探测全部丢到后台并行跑。
const loadConfig = async () => {
  configLoading.value = true
  try {
    const cfg = await invoke('get_llm_config')
    form.chatgpt_api_key = typeof cfg?.chatgpt_api_key === 'string' ? cfg.chatgpt_api_key : ''
    form.deepseek_api_key = typeof cfg?.deepseek_api_key === 'string' ? cfg.deepseek_api_key : ''
    form.qwen_api_key = typeof cfg?.qwen_api_key === 'string' ? cfg.qwen_api_key : ''
    form.kimi_api_key = typeof cfg?.kimi_api_key === 'string' ? cfg.kimi_api_key : ''
    form.doubao_api_key = typeof cfg?.doubao_api_key === 'string' ? cfg.doubao_api_key : ''
    form.siliconflow_api_key = typeof cfg?.siliconflow_api_key === 'string' ? cfg.siliconflow_api_key : ''
    form.custom_base_url = typeof cfg?.custom_base_url === 'string' ? cfg.custom_base_url : ''
    form.custom_api_key = typeof cfg?.custom_api_key === 'string' ? cfg.custom_api_key : ''
    form.custom_model = typeof cfg?.custom_model === 'string' ? cfg.custom_model : ''
    form.cc_switch_enabled = Boolean(cfg?.cc_switch_enabled)
    form.cc_switch_base_url = typeof cfg?.cc_switch_base_url === 'string' && cfg.cc_switch_base_url
      ? cfg.cc_switch_base_url
      : 'http://127.0.0.1:15721/v1'
    form.cc_switch_api_key = typeof cfg?.cc_switch_api_key === 'string' ? cfg.cc_switch_api_key : ''
    form.cc_switch_model = typeof cfg?.cc_switch_model === 'string' ? cfg.cc_switch_model : ''
    form.cc_switch_wire_api = cfg?.cc_switch_wire_api === 'responses' ? 'responses' : 'chat'
    form.codex_model = typeof cfg?.codex_model === 'string' ? cfg.codex_model : ''
    form.codex_sandbox = typeof cfg?.codex_sandbox === 'string' && cfg.codex_sandbox
      ? cfg.codex_sandbox
      : 'workspace-write'
    // 三档引擎：只有在这三个值里才照搬，其余（含老配置里的空串）一律落到工作台默认档。
    form.agent_engine = ['app-server', 'codex', 'builtin'].includes(cfg?.agent_engine)
      ? cfg.agent_engine
      : 'app-server'
  } catch (e) {
    logger.error('加载大模型配置失败:', e)
  } finally {
    configLoading.value = false
  }

  // 状态探测互不依赖，并行刷新；慢或失败都不该再锁住表单。
  void refreshAllStatus()
}

const refreshAllStatus = async () => {
  await Promise.allSettled([
    refreshCodexStatus(),
    refreshChatRecordRoot(),
    refreshMarkdownModelsStatus(),
    refreshEngineStatus(),
    refreshMarkerOcrMode(),
    refreshMarkerReadingOrder(),
    refreshLocalDeepseekStatus(),
    refreshLocalQwenStatus(),
    refreshTranslateLocalStatus(),
    refreshTranslateCacheStats(),
  ])
}

const saveConfig = async () => {
  if (saving.value) return
  saving.value = true
  try {
    await invoke('set_llm_config', {
      llmConfig: {
        chatgpt_api_key: form.chatgpt_api_key,
        deepseek_api_key: form.deepseek_api_key,
        qwen_api_key: form.qwen_api_key,
        kimi_api_key: form.kimi_api_key,
        doubao_api_key: form.doubao_api_key,
        siliconflow_api_key: form.siliconflow_api_key,
        custom_base_url: form.custom_base_url,
        custom_api_key: form.custom_api_key,
        custom_model: form.custom_model,
        // set_llm_config 是整体替换，所有字段都必须回传，漏一个就会被重置成默认值。
        cc_switch_enabled: form.cc_switch_enabled,
        cc_switch_base_url: form.cc_switch_base_url,
        cc_switch_api_key: form.cc_switch_api_key,
        cc_switch_model: form.cc_switch_model,
        cc_switch_wire_api: form.cc_switch_wire_api,
        codex_model: form.codex_model,
        codex_sandbox: form.codex_sandbox,
        agent_engine: form.agent_engine,
      },
    })
    message.success('已保存大模型配置')
    // Agent 读到的是同一份配置，保存后顺手刷新它的状态，避免显示停留在旧供应商。
    void refreshCodexStatus()
  } catch (e) {
    logger.error('保存大模型配置失败:', e)
    message.error('保存大模型配置失败')
  } finally {
    saving.value = false
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
  if (savingChatRecord.value) return
  savingChatRecord.value = true
  try {
    await invoke('set_chat_record_root', { path: chatRecordRoot.value })
    await refreshChatRecordRoot()
    message.success('已保存 ChatRecord 路径')
  } catch (e) {
    logger.error('保存 ChatRecord 路径失败:', e)
    message.error('保存 ChatRecord 路径失败')
  } finally {
    savingChatRecord.value = false
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

// ── Marker 运行环境（自动装配）状态 ──
const markerEnvState = computed(() => {
  if (preparingMarkerEnv.value || engineStatus.value.markerProvisioning) return 'info'
  return engineStatus.value.markerReady ? 'ok' : 'warn'
})

const markerEnvBadgeText = computed(() => {
  if (preparingMarkerEnv.value || engineStatus.value.markerProvisioning) return '装配中'
  if (!engineStatus.value.markerReady) return '未就绪'
  return engineStatus.value.markerCuda ? '已就绪 · GPU' : '已就绪 · CPU'
})

const markerEnvBadgeIcon = computed(() => {
  if (preparingMarkerEnv.value || engineStatus.value.markerProvisioning) return TimeOutline
  return engineStatus.value.markerReady ? CheckmarkCircleOutline : CloseCircleOutline
})

// ── marker 的 OCR 策略 ──
// 为什么需要它：CPU 上 VLM 解码只有约 6 token/s，一次整页 OCR 要数分钟，上千页跑不完；
// 但直接关掉 OCR 又会让扫描件彻底没内容。所以给三档，默认“自动”：
// 有 GPU 时按“全质量”处理，没 GPU 时才让后端按文档类型决定（数字文档跳过、扫描件保留）。
const markerOcrMode = ref('auto')
const savingMarkerOcr = ref(false)
const markerOcrModeOptions = [
  { value: 'auto', label: '自动（推荐）' },
  { value: 'full', label: '全质量（始终允许 OCR）' },
  { value: 'text_only', label: '仅文本层（最快）' },
]

const markerOcrModeHint = computed(() => {
  switch (markerOcrMode.value) {
    case 'text_only':
      return '永不调用 OCR：最快，但扫描件会几乎没有内容'
    case 'full':
      return '始终允许 OCR：质量最高；无 GPU 时扫描件每页可能要数分钟'
    default:
      return engineStatus.value.markerCuda
        ? '已检测到 GPU：按「全质量」处理（OCR 很快，不牺牲质量）'
        : '未检测到 GPU：数字文档自动跳过 OCR（快），扫描件仍走 OCR（较慢但可用）'
  }
})

const refreshMarkerOcrMode = async () => {
  try {
    const mode = await invoke('get_marker_ocr_mode')
    markerOcrMode.value = typeof mode === 'string' && mode ? mode : 'auto'
  } catch (e) {
    logger.error('读取 marker OCR 策略失败:', e)
  }
}

const changeMarkerOcrMode = async (value) => {
  savingMarkerOcr.value = true
  try {
    const mode = String(value || 'auto')
    await invoke('set_marker_ocr_mode', { mode })
    markerOcrMode.value = mode
    message.success('已保存 OCR 策略，下次转换生效')
  } catch (e) {
    logger.error('保存 marker OCR 策略失败:', e)
    message.error(`保存 OCR 策略失败: ${String(e?.message || e || '')}`)
  } finally {
    savingMarkerOcr.value = false
  }
}

// ── marker 的阅读顺序策略 ──
// marker 2.x 默认按 PDF 字符流排块（多栏页面上更准），但"打印成 PDF"的文档
// 字符流顺序常与版面不一致——标题会被写在正文之后，于是标题与正文分家。
// 默认取版面顺序，需要多栏优化时可切回字符流。
const markerReadingOrder = ref('layout')
const savingMarkerReadingOrder = ref(false)
const markerReadingOrderOptions = [
  { value: 'layout', label: '顺序：版面（推荐）' },
  { value: 'pdftext', label: '顺序：PDF 字符流' },
]

const markerReadingOrderHint = computed(() => (
  markerReadingOrder.value === 'pdftext'
    ? '按 PDF 字符流排块：多栏论文通常更准，但浏览器打印/导出的 PDF 可能把标题排到文末'
    : '按版面位置排块：网页打印/导出型 PDF 更稳，标题不会与正文分家'
))

const refreshMarkerReadingOrder = async () => {
  try {
    const order = await invoke('get_marker_reading_order')
    markerReadingOrder.value = order === 'pdftext' ? 'pdftext' : 'layout'
  } catch (e) {
    logger.error('读取 marker 阅读顺序失败:', e)
  }
}

const changeMarkerReadingOrder = async (value) => {
  savingMarkerReadingOrder.value = true
  try {
    const order = String(value || 'layout')
    await invoke('set_marker_reading_order', { order })
    markerReadingOrder.value = order
    message.success('已保存阅读顺序，下次转换生效')
  } catch (e) {
    logger.error('保存 marker 阅读顺序失败:', e)
    message.error(`保存阅读顺序失败: ${String(e?.message || e || '')}`)
  } finally {
    savingMarkerReadingOrder.value = false
  }
}

/// 读取转换引擎装配状态（marker 运行环境 / 模型）。
const refreshEngineStatus = async () => {
  try {
    const status = await invoke('conversion_engine_status')
    engineStatus.value = {
      markerReady: Boolean(status?.markerReady),
      markerProvisioning: Boolean(status?.markerProvisioning),
      markerError: typeof status?.markerError === 'string' ? status.markerError : '',
      markerCuda: Boolean(status?.markerCuda),
      markerDegraded: Boolean(status?.markerDegraded),
      markerModelsReady: Boolean(status?.markerModelsReady),
      markerModelsError: typeof status?.markerModelsError === 'string' ? status.markerModelsError : '',
      markerModelsDir: typeof status?.markerModelsDir === 'string' ? status.markerModelsDir : '',
      markerEnvDir: typeof status?.markerEnvDir === 'string' ? status.markerEnvDir : '',
    }
  } catch (e) {
    logger.error('读取转换引擎状态失败:', e)
  }
}

/// 手动触发 marker 运行环境装配（首次启动后台已自动进行，这里是“重试”入口）。
const prepareMarkerEnv = async () => {
  preparingMarkerEnv.value = true
  try {
    message.info('开始准备 Marker 运行环境（首次可能需要数分钟，取决于网络）')
    await invoke('ensure_marker_engine_ready')
    await refreshEngineStatus()
    if (engineStatus.value.markerReady) {
      message.success('Marker 运行环境已就绪')
    } else {
      message.warning(engineStatus.value.markerError || 'Marker 运行环境仍未就绪，请稍后重试')
    }
  } catch (e) {
    logger.error('准备 Marker 运行环境失败:', e)
    message.error(`准备 Marker 运行环境失败: ${String(e?.message || e || '')}`)
  } finally {
    preparingMarkerEnv.value = false
  }
}

/// 在线下载 marker 模型（离线加载仍保留在上面的“加载”按钮）。
const downloadMarkdownModels = async () => {
  downloadingModels.value = true
  try {
    message.info('开始下载 Markdown 转换模型（数 GB，请耐心等待）')
    await invoke('ensure_marker_models_ready')
    await refreshEngineStatus()
    const modelsStatus = await invoke('get_markdown_models_status')
    markdownModelsStatus.value = {
      installed: Boolean(modelsStatus?.installed),
      expected_path: typeof modelsStatus?.expected_path === 'string' ? modelsStatus.expected_path : '',
    }
    if (markdownModelsStatus.value.installed) {
      message.success('Markdown 转换模型已就绪')
    } else {
      message.warning(engineStatus.value.markerModelsError || '模型仍未下载完整，请稍后重试')
    }
  } catch (e) {
    logger.error('下载 Markdown 转换模型失败:', e)
    message.error(`下载 Markdown 转换模型失败: ${String(e?.message || e || '')}`)
  } finally {
    downloadingModels.value = false
  }
}

const downloadLocalDeepseekModel = async () => {
  if (localDeepseekStatus.value.installed) {
    logger.info('LocalDeepseek 模型已存在，跳过下载')
    message.info('模型已存在，无需重复下载')
    return
  }

  downloadingLocalDeepseek.value = true
  try {
    message.info('开始下载 LocalDeepseek 模型，请稍候')
    const installedTo = await invoke('download_local_deepseek_model')
    message.success(`模型已下载到: ${installedTo}`)

    await refreshLocalDeepseekStatus()
  } catch (e) {
    logger.error('下载 LocalDeepseek 模型失败:', e)
    message.error('下载 LocalDeepseek 模型失败')
  } finally {
    downloadingLocalDeepseek.value = false
  }
}

const importLocalDeepseekModel = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'GGUF Model', extensions: ['gguf'] }],
    })
    if (!selected) return

    downloadingLocalDeepseek.value = true
    const installedTo = await invoke('install_local_deepseek_model_from_file', {
      sourcePath: String(selected),
    })
    message.success(`模型已导入到: ${installedTo}`)

    await refreshLocalDeepseekStatus()
  } catch (e) {
    logger.error('导入 LocalDeepseek 模型失败:', e)
    message.error('导入 LocalDeepseek 模型失败')
  } finally {
    downloadingLocalDeepseek.value = false
  }
}

const downloadLocalQwenModel = async () => {
  if (localQwenStatus.value.installed) {
    logger.info('LocalQwen 模型已存在，跳过下载')
    message.info('模型已存在，无需重复下载')
    return
  }

  downloadingLocalQwen.value = true
  try {
    message.info('开始下载 LocalQwen 模型，请稍候')
    const installedTo = await invoke('download_local_qwen_model')
    message.success(`模型已下载到: ${installedTo}`)

    await refreshLocalQwenStatus()
  } catch (e) {
    logger.error('下载 LocalQwen 模型失败:', e)
    message.error('下载 LocalQwen 模型失败')
  } finally {
    downloadingLocalQwen.value = false
  }
}

const importLocalQwenModel = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'GGUF Model', extensions: ['gguf'] }],
    })
    if (!selected) return

    downloadingLocalQwen.value = true
    const installedTo = await invoke('install_local_qwen_model_from_file', {
      sourcePath: String(selected),
    })
    message.success(`模型已导入到: ${installedTo}`)

    await refreshLocalQwenStatus()
  } catch (e) {
    logger.error('导入 LocalQwen 模型失败:', e)
    message.error('导入 LocalQwen 模型失败')
  } finally {
    downloadingLocalQwen.value = false
  }
}

const refreshTranslateLocalStatus = async () => {
  try {
    const status = await invoke('translate_local_status')
    translateLocalStatus.value = {
      enabled: Boolean(status?.enabled),
      ready: Boolean(status?.ready),
      downloading: Boolean(status?.downloading),
      active_model: typeof status?.active_model === 'string' ? status.active_model : '',
      installed: Boolean(status?.installed),
      expected_path: typeof status?.expected_path === 'string' ? status.expected_path : '',
      llama_server_exists: Boolean(status?.llama_server_exists),
      llama_server_path: typeof status?.llama_server_path === 'string' ? status.llama_server_path : '',
      models: Array.isArray(status?.models) ? status.models : [],
    }
  } catch (e) {
    logger.error('读取翻译模型状态失败:', e)
  }
}

const refreshTranslateCacheStats = async () => {
  try {
    const stats = await invoke('translate_cache_stats')
    translateCacheStats.value = {
      available: Boolean(stats?.available),
      db_path: typeof stats?.db_path === 'string' ? stats.db_path : '',
      rows: Number(stats?.rows) || 0,
      total_hits: Number(stats?.total_hits) || 0,
      word_rows: Number(stats?.word_rows) || 0,
      word_hits: Number(stats?.word_hits) || 0,
      db_size_bytes: Number(stats?.db_size_bytes) || 0,
      max_rows: Number(stats?.max_rows) || 0,
    }
  } catch (e) {
    logger.error('读取翻译缓存统计失败:', e)
  }
}

const downloadTranslateModel = async () => {
  if (translateLocalStatus.value.installed) {
    message.info('翻译模型已存在，无需重复下载')
    return
  }
  downloadingTranslateModel.value = true
  try {
    message.info('开始下载翻译模型（优先走魔搭），请稍候')
    const installedTo = await invoke('translate_local_download_model', { modelKey: null })
    message.success(`翻译模型已下载到: ${installedTo}`)
    await refreshTranslateLocalStatus()
  } catch (e) {
    logger.error('下载翻译模型失败:', e)
    message.error(`下载翻译模型失败: ${String(e?.message || e || '')}`)
  } finally {
    downloadingTranslateModel.value = false
  }
}

const importTranslateModel = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'GGUF Model', extensions: ['gguf'] }],
    })
    if (!selected) return

    downloadingTranslateModel.value = true
    const installedTo = await invoke('translate_local_import_model', {
      sourcePath: String(selected),
      modelKey: null,
    })
    message.success(`翻译模型已导入到: ${installedTo}`)
    await refreshTranslateLocalStatus()
  } catch (e) {
    logger.error('导入翻译模型失败:', e)
    message.error(`导入翻译模型失败: ${String(e?.message || e || '')}`)
  } finally {
    downloadingTranslateModel.value = false
  }
}

const changeTranslateModel = async (key) => {
  try {
    await invoke('translate_local_set_model', { modelKey: String(key || '') })
    message.success('已切换翻译模型，下次翻译时自动加载')
    await refreshTranslateLocalStatus()
  } catch (e) {
    logger.error('切换翻译模型失败:', e)
    message.error('切换翻译模型失败')
  }
}

const clearTranslateCache = async () => {
  clearingTranslateCache.value = true
  try {
    const removed = await invoke('translate_cache_clear')
    message.success(`已清空 ${removed} 段翻译缓存`)
    await refreshTranslateCacheStats()
  } catch (e) {
    logger.error('清空翻译缓存失败:', e)
    message.error('清空翻译缓存失败')
  } finally {
    clearingTranslateCache.value = false
  }
}

onMounted(loadConfig)
</script>

<style scoped>
.llm-setting {
  width: 100%;
  box-sizing: border-box;
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui);
  font-size: 13.5px;
  line-height: 1.7;
}

/* 顶部状态区的小标题成组出现，字号比小节标题再小半档，靠字重区分。 */
.status-header {
  margin-bottom: 8px;
}

.status-title {
  font-size: 13.5px;
  font-weight: 600;
  margin-bottom: 3px;
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
  gap: 8px;
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
  /* 状态面板位于设置页底部,提示卡片向上弹出,避免落到屏幕外被裁切看不到 */
  bottom: calc(100% + 8px);
  right: 0;
  min-width: 280px;
  max-width: 420px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(128, 128, 128, 0.25);
  background: rgba(0, 0, 0, 0.86);
  color: #fff;
  box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.24);
  display: none;
  z-index: 20;
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
  font-size: 12.5px;
  line-height: 1.5;
  opacity: 0.92;
}
</style>
