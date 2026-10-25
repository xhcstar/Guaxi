<template>
  <div class="preference-wrapper">
    <!-- 页内主题导航:点击平滑滚动到对应分区(锚点为各区块 id) -->
    <nav class="pref-subnav">
      <button
        v-for="s in sections"
        :key="s.id"
        type="button"
        class="pref-subnav-btn"
        :class="{ active: activeSection === s.id }"
        @click="scrollToSection(s.id)"
      >
        {{ s.label }}
      </button>
    </nav>

    <div class="pref-sections">
      <!-- 外观主题 -->
      <n-card id="pref-appearance" class="pref-card" title="外观主题">
        <p class="description">在此切换应用主题, 暗色或者亮色。</p>
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

      <!-- 语音能力:朗读(TTS) + 语音转写(ASR) 同主题合并 -->
      <n-card id="pref-voice" class="pref-card" title="语音能力">
        <div class="pref-block">
          <h3>朗读声音</h3>
          <p class="description">
            划词朗读、逐句朗读、PDF 朗读与有声书默认都使用这里的音色与语速；设置保存到 guaxi_config.json。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <template v-if="ttsProvider === 'sherpa-onnx'">
              <n-form-item label="语言">
                <n-select
                  v-model:value="ttsLanguage"
                  :options="ttsLanguageOptions"
                  :disabled="loading || ttsSaving"
                  style="width: 240px"
                />
              </n-form-item>
              <n-form-item label="性别">
                <n-select
                  v-model:value="ttsGender"
                  :options="ttsGenderOptions"
                  :disabled="loading || ttsSaving"
                  style="width: 240px"
                />
              </n-form-item>
              <n-form-item label="音色">
                <n-space vertical :size="4">
                  <n-select
                    v-model:value="ttsSpeakerId"
                    :options="ttsSpeakerOptions"
                    filterable
                    tag
                    placeholder="选择朗读音色"
                    :disabled="loading || ttsSaving"
                    style="width: 320px"
                  />
                  <span class="description">{{ ttsSpeakerHint }}</span>
                  <span v-if="!ttsSpeakerOptions.length" class="description">
                    当前语言/性别组合下没有可用音色（Kokoro 英文音色只有女声），换一个组合试试。
                  </span>
                </n-space>
              </n-form-item>
            </template>
            <n-form-item label="语速">
              <n-space align="center" :size="12">
                <n-slider
                  v-model:value="ttsSpeed"
                  :min="0.5"
                  :max="2"
                  :step="0.05"
                  :disabled="loading || ttsSaving"
                  style="width: 220px"
                />
                <span class="description">{{ Number(ttsSpeed).toFixed(2) }}×</span>
                <n-button size="tiny" quaternary :disabled="loading || ttsSaving" @click="ttsSpeed = 1">
                  原速
                </n-button>
              </n-space>
            </n-form-item>
            <n-form-item label="试听文本">
              <n-input
                v-model:value="ttsTestText"
                placeholder="输入一行用于试听的文本"
                :disabled="loading || ttsSaving || ttsTesting"
              />
            </n-form-item>
            <n-form-item>
              <n-space>
                <n-button
                  type="primary"
                  size="small"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsConfigSave"
                >
                  保存
                </n-button>
                <n-button
                  size="small"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsConfigReset"
                >
                  恢复默认
                </n-button>
                <n-button
                  secondary
                  size="small"
                  :loading="ttsTesting"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsTestReadAloud"
                >
                  {{ ttsTesting ? '朗读中...' : '测试朗读' }}
                </n-button>
              </n-space>
            </n-form-item>
            <n-collapse>
              <n-collapse-item title="高级：朗读服务与模型路径" name="advanced">
                <n-form-item label="Provider">
                  <n-select
                    v-model:value="ttsProvider"
                    :options="ttsProviderOptions"
                    :disabled="loading || ttsSaving"
                    style="width: 240px"
                  />
                </n-form-item>
                <n-form-item label="Model 路径">
                  <n-input
                    v-model:value="ttsModel"
                    placeholder="utils/Models/tts/kokoro_multi_lang/model.onnx"
                    :disabled="loading || ttsSaving"
                  />
                </n-form-item>
                <n-form-item v-if="ttsProvider === 'sherpa-onnx'" label="Tokens 路径">
                  <n-input
                    v-model:value="ttsTokens"
                    placeholder="utils/Models/tts/kokoro_multi_lang/tokens.txt"
                    :disabled="loading || ttsSaving"
                  />
                </n-form-item>
              </n-collapse-item>
            </n-collapse>
          </n-form>
        </div>

        <n-divider />

        <div class="pref-block">
          <h3>ASR 语音转写配置</h3>
          <p class="description">
            配置语音转文字提供方与本地 sherpa-onnx 路径。设置保存到 guaxi_config.json。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="Provider">
              <n-select
                v-model:value="asrProvider"
                :options="asrProviderOptions"
                :disabled="loading || asrSaving"
                style="width: 240px"
              />
            </n-form-item>
            <n-form-item label="Model 路径">
              <n-input
                v-model:value="asrModel"
                placeholder="可填模型目录或模型文件路径"
                :disabled="loading || asrSaving"
              />
            </n-form-item>
            <n-form-item label="测试音频路径">
              <n-input
                v-model:value="asrTestFilePath"
                placeholder="填写本地音频文件绝对路径（仅支持 wav）"
                :disabled="loading || asrSaving || asrTesting"
              />
            </n-form-item>
            <n-form-item label="转写结果">
              <n-input
                v-model:value="asrTestResult"
                type="textarea"
                :autosize="{ minRows: 3, maxRows: 6 }"
                placeholder="点击“测试转写”后显示结果"
                :disabled="true"
              />
            </n-form-item>
            <n-form-item>
              <n-space>
                <n-button
                  type="primary"
                  size="small"
                  :disabled="loading || asrSaving"
                  @click="handleAsrConfigSave"
                >
                  保存
                </n-button>
                <n-button
                  size="small"
                  :disabled="loading || asrSaving"
                  @click="handleAsrConfigReset"
                >
                  恢复默认
                </n-button>
                <n-button
                  secondary
                  size="small"
                  :loading="asrTesting"
                  :disabled="loading || asrSaving"
                  @click="handleAsrTestTranscribe"
                >
                  {{ asrTesting ? '转写中...' : '测试转写' }}
                </n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>
      </n-card>

      <!-- 编辑器 -->
      <n-card id="pref-editor" class="pref-card" title="编辑器">
        <h3>编辑器外观</h3>
        <p class="description">
          主编辑器（Monaco）的字体、字号和小地图开关。改动即时生效并写入 guaxi_config.json，
          不用重启，也不用重新打开标签页。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="字体">
            <n-space vertical :size="4" style="width: 100%">
              <n-select
                :value="settingsStore.editorDisplay.font_family"
                :options="editorFontFamilyOptions"
                :loading="systemFontsLoading"
                filterable
                tag
                :disabled="loading || editorDisplaySaving"
                placeholder="选择预设或本机字体，也可直接输入 CSS font-family"
                style="width: 420px; max-width: 100%"
                @focus="loadSystemFonts"
                @update:value="(v) => handleEditorDisplayChange('font_family', String(v || ''))"
              />
              <span class="description">
                展开下拉时会读取本机已安装的字体（等宽的排在前面）；想用自己的字体链，
                直接输入整条 CSS font-family 后回车确认即可，字体没装时按链上顺序回退，不会变成方块。
                多数英文等宽字体（Meslo、Consolas、JetBrains Mono…）不含中文字形，
                中文会沿回退链落到 Maple Mono NF CN / 微软雅黑——所以换字体后“中文看着没变、英文变了”是正常现象。
              </span>
            </n-space>
          </n-form-item>
          <n-form-item label="字号">
            <n-space align="center" :size="8">
              <n-input-number
                :value="settingsStore.editorDisplay.font_size"
                :min="EDITOR_FONT_SIZE_MIN"
                :max="EDITOR_FONT_SIZE_MAX"
                :step="1"
                :precision="0"
                :disabled="loading || editorDisplaySaving"
                style="width: 140px"
                @update:value="(v) => handleEditorDisplayChange('font_size', Number(v) || DEFAULT_EDITOR_DISPLAY_OPTIONS.font_size)"
              />
              <span class="description">
                允许 {{ EDITOR_FONT_SIZE_MIN }}–{{ EDITOR_FONT_SIZE_MAX }}。行高按字号的 4/3 自动跟随（默认 18px 字号 / 24px 行高）。
              </span>
            </n-space>
          </n-form-item>
          <n-form-item label="小地图">
            <n-space align="center" :size="8">
              <n-switch
                :value="settingsStore.editorDisplay.minimap_enabled"
                :disabled="loading || editorDisplaySaving"
                @update:value="(v) => handleEditorDisplayChange('minimap_enabled', v)"
              />
              <span class="description">编辑器右侧的代码缩略图，默认关闭。</span>
            </n-space>
          </n-form-item>
          <n-form-item label="预览">
            <div class="editor-display-preview" :style="editorDisplayPreviewStyle">
              <div>
                <span class="tok-kw">def</span> <span class="tok-fn">greet</span>(name):
              </div>
              <div>&nbsp;&nbsp;&nbsp;&nbsp;<span class="tok-kw">return</span> <span class="tok-str">f"你好，{name}"</span> <span class="tok-cmt"># 中英混排 iIlL1 0123</span></div>
            </div>
          </n-form-item>
        </n-form>
        <p class="description">
          提示：编辑器里按住 Ctrl 滚动滚轮也能临时缩放，但那只是当前窗口的临时值，不会写进配置，
          重新打开后会回到这里的字号。
        </p>

        <n-divider />

        <h3>LSP 配置</h3>
        <p class="description">
          LSP 能力统一由下方语言扩展提供；安装对应扩展后，新建或重新打开编辑器即可获得语法高亮和语义补全。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="启用 LSP 扩展能力">
            <n-switch
              v-model:value="lspHighlightEnabled"
              :disabled="loading"
              @update:value="handleLspHighlightChange"
            />
          </n-form-item>
          <n-form-item label="代码配色主题">
            <n-select
              v-model:value="codeTheme"
              :options="codeThemeOptions"
              :disabled="loading"
              style="max-width: 240px"
              @update:value="handleCodeThemeChange"
            />
          </n-form-item>
        </n-form>
        <p class="description">
          代码配色同时作用于编辑器里的代码块、探索者 cell，以及 Markdown 预览/阅读器/导出的代码块。
          每套主题自带亮色与暗色两套配色，随外观主题自动切换。
        </p>

        <h3>Markdown 格式化</h3>
        <p class="description">
          右键菜单里「Markdown 格式化（全文 / 选中）」使用的参数。这组值以前写死在编辑器代码里，
          现在可以按自己的写法调整；改动即时落盘。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="折行宽度（0 = 不折行）">
            <n-input-number
              :value="settingsStore.markdownFormat.max_line_width"
              :min="0"
              :max="400"
              :disabled="loading || markdownFormatSaving"
              style="width: 160px"
              @update:value="(v) => handleMarkdownFormatChange('max_line_width', Number(v) || 0)"
            />
          </n-form-item>
          <n-form-item label="缩进宽度">
            <n-input-number
              :value="settingsStore.markdownFormat.indent_width"
              :min="1"
              :max="8"
              :disabled="loading || markdownFormatSaving"
              style="width: 160px"
              @update:value="(v) => handleMarkdownFormatChange('indent_width', Number(v) || 2)"
            />
          </n-form-item>
          <n-form-item label="列表缩进">
            <n-input-number
              :value="settingsStore.markdownFormat.list_indent"
              :min="1"
              :max="8"
              :disabled="loading || markdownFormatSaving"
              style="width: 160px"
              @update:value="(v) => handleMarkdownFormatChange('list_indent', Number(v) || 2)"
            />
          </n-form-item>
          <n-form-item label="格式化表格">
            <n-switch
              :value="settingsStore.markdownFormat.format_tables"
              :disabled="loading || markdownFormatSaving"
              @update:value="(v) => handleMarkdownFormatChange('format_tables', v)"
            />
          </n-form-item>
          <n-form-item label="格式化列表">
            <n-switch
              :value="settingsStore.markdownFormat.format_lists"
              :disabled="loading || markdownFormatSaving"
              @update:value="(v) => handleMarkdownFormatChange('format_lists', v)"
            />
          </n-form-item>
          <n-form-item label="保留代码块内格式">
            <n-switch
              :value="settingsStore.markdownFormat.preserve_code_blocks"
              :disabled="loading || markdownFormatSaving"
              @update:value="(v) => handleMarkdownFormatChange('preserve_code_blocks', v)"
            />
          </n-form-item>
          <n-form-item label="移除多余空行">
            <n-switch
              :value="settingsStore.markdownFormat.remove_extra_blank_lines"
              :disabled="loading || markdownFormatSaving"
              @update:value="(v) => handleMarkdownFormatChange('remove_extra_blank_lines', v)"
            />
          </n-form-item>
          <n-form-item label="清理行尾空白">
            <n-switch
              :value="settingsStore.markdownFormat.fix_extra_whitespace"
              :disabled="loading || markdownFormatSaving"
              @update:value="(v) => handleMarkdownFormatChange('fix_extra_whitespace', v)"
            />
          </n-form-item>
        </n-form>
        <LspExtensionPanel />

        <h3>AI 代码辅助</h3>
        <p class="description">
          在探索者笔记本的 Python cell 里，用自然语言/注释一键生成代码（复用你已配置的大模型）。
          未配置云端模型时会回退到本地大模型，低配机器上生成较慢，故默认关闭，按需开启。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="启用 Python 代码生成助手">
            <n-switch
              v-model:value="pythonCodegenAssistEnabled"
              :disabled="loading"
              @update:value="handlePythonCodegenAssistChange"
            />
          </n-form-item>
        </n-form>
      </n-card>

      <!-- 插件管理 -->
      <n-card id="pref-plugins" class="pref-card" title="插件管理">
        <p class="description">
          用户可编写的软件级插件(基于内置 Python 引擎)。可启用/禁用、导入/导出、新建、编辑源码。
          编辑器里选中文本后右键“运行插件…”即可调用已启用的 transform 插件。
        </p>
        <GuaxiPluginManager />
      </n-card>

      <!-- 终端 -->
      <n-card id="pref-terminal" class="pref-card" title="终端">
        <p class="description">
          配置全局终端面板的默认终端类型，以及 Windows 下 Git Bash 的可执行文件路径。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="默认终端">
            <n-select
              v-model:value="terminalDefaultProfile"
              :options="terminalProfileOptions"
              :disabled="loading || terminalSaving"
              style="width: 240px"
            />
          </n-form-item>
          <n-form-item label="Git Bash 路径">
            <n-input
              v-model:value="terminalGitBashPath"
              placeholder="例如：D:\\Git\\bin\\bash.exe -i -l"
              :disabled="loading || terminalSaving"
            />
          </n-form-item>
          <n-form-item>
            <n-button
              type="primary"
              size="small"
              :disabled="loading || terminalSaving"
              @click="handleTerminalConfigSave"
            >
              保存终端配置
            </n-button>
          </n-form-item>
        </n-form>
      </n-card>

      <!-- 文件传输 -->
      <n-card id="pref-transfer" class="pref-card" title="文件传输">
        <h3>WebRTC 文件传输</h3>
        <p class="description">
          房间码模式需要一个“信令服务”（只中转 WebRTC 协商信息，不传文件内容）。此处配置后，文件传输助手将不再需要手动填写信令地址。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="信令服务地址">
            <n-input
              v-model:value="webrtcSignalUrl"
              placeholder="例如：wss://your-signal.example.com/ft/signal"
              :disabled="loading || webrtcSignalSaving"
            />
          </n-form-item>
          <n-form-item>
            <n-button
              type="primary"
              size="small"
              :disabled="loading || webrtcSignalSaving"
              @click="handleWebrtcSignalUrlSave"
            >
              保存
            </n-button>
          </n-form-item>
        </n-form>
      </n-card>

      <!-- 系统维护 -->
      <n-card id="pref-system" class="pref-card" title="系统维护">
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
      </n-card>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onBeforeUnmount, onMounted, watch } from 'vue'
import { buildSpeakerOptions, describeSpeaker, speakerMeta } from '../../utils/ttsSpeakers'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInputNumber, NInput, NButton, NAlert, NSwitch, NSelect, NSlider, NCollapse, NCollapseItem } from 'naive-ui'
import { DEFAULT_CODE_THEME_ID, GUAXI_CODE_THEMES } from '../../monaco/codeThemes'
import { invoke } from '@tauri-apps/api/core'
import {
  useSettingsStore,
  DEFAULT_EDITOR_DISPLAY_OPTIONS,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_FALLBACK_STACK,
  resolveEditorFontFamily,
  resolveEditorLineHeight,
} from '../../stores/settingsStore'
import LspExtensionPanel from './LspExtensionPanel.vue'
import GuaxiPluginManager from './GuaxiPluginManager.vue'

const logger = createLogger('PreferenceContent')

const settingsStore = useSettingsStore()

// 页内主题分区与锚点导航
const sections = [
  { id: 'pref-appearance', label: '外观主题' },
  { id: 'pref-voice', label: '语音能力' },
  { id: 'pref-editor', label: '编辑器' },
  { id: 'pref-terminal', label: '终端' },
  { id: 'pref-transfer', label: '文件传输' },
  { id: 'pref-system', label: '系统维护' },
]
const activeSection = ref('pref-appearance')
function scrollToSection(id) {
  activeSection.value = id
  const el = document.getElementById(id)
  if (el && typeof el.scrollIntoView === 'function') {
    el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

const themeOptions = [
  { label: '亮色', value: 'light' },
  { label: '暗色', value: 'dark' },
]

const themeValue = ref('light')
const gcCycleHours = ref(1)
const lspHighlightEnabled = ref(false)
const codeTheme = ref(DEFAULT_CODE_THEME_ID)
// 主题清单是前端资产（每套自带明暗两套调色板 + 上游 highlight.js 样式表），后端只存 id 字符串。
const codeThemeOptions = GUAXI_CODE_THEMES.map((theme) => ({ label: theme.label, value: theme.id }))
const pythonCodegenAssistEnabled = ref(false)
// Markdown 格式化选项存在 settingsStore 里（启动时已 load），这里只跟一个"正在保存"的开关。
const markdownFormatSaving = ref(false)
// 编辑器外观（字体 / 字号 / 小地图）同理，值在 settingsStore 里。
const editorDisplaySaving = ref(false)
// 预设字体链。标签只写字体名，选中后落盘的是整条 CSS font-family，
// 所以用户没装这个字体时仍有稳定回退（前两项是项目自己在用的那份栈）。
const editorFontPresetOptions = [
  { label: 'Maple Mono NF CN（默认）', value: DEFAULT_EDITOR_DISPLAY_OPTIONS.font_family },
  { label: 'JetBrains Mono', value: "'JetBrainsMono Nerd Font Mono', Consolas, monospace" },
  { label: 'Cascadia Code', value: "'Cascadia Code', 'Cascadia Mono', Consolas, monospace" },
  { label: 'Fira Code', value: "'Fira Code', 'JetBrainsMono Nerd Font Mono', Consolas, monospace" },
  { label: 'Consolas', value: "Consolas, 'Courier New', monospace" },
  { label: '中文优先（Noto Sans CJK）', value: "'Noto Sans CJK SC', 'Microsoft YaHei', Consolas, monospace" },
  { label: '系统等宽', value: 'monospace' },
]

// 本机已安装字体由后端枚举（WebView 没有可用的系统字体 API），第一次展开下拉时才去读，
// 不让设置页首屏等一次全量字体扫描。
const systemFontFamilies = ref([])
const systemFontsLoading = ref(false)
let systemFontsRequested = false
const loadSystemFonts = async () => {
  if (systemFontsRequested) return
  systemFontsRequested = true
  systemFontsLoading.value = true
  try {
    const families = await invoke('list_system_fonts')
    systemFontFamilies.value = Array.isArray(families)
      ? families.filter((font) => font && typeof font.name === 'string' && font.name.trim())
      : []
    logger.info('读取系统字体列表完成', { count: systemFontFamilies.value.length })
  } catch (error) {
    // 读不到系统字体不影响预设与手输字体链，退化成只有预设即可。
    logger.error('读取系统字体列表失败:', error)
    systemFontFamilies.value = []
  } finally {
    systemFontsLoading.value = false
  }
}

// 字体名里的单引号在 CSS font-family 里无法转义，直接去掉；其余字符原样放进引号即可。
const quoteEditorFontFamily = (name) => `'${String(name || '').replace(/'/g, '')}'`

// 选系统字体时补一段固定回退链。多数英文等宽字体（Meslo、Consolas、JetBrains Mono…）没有中文字形，
// 不补的话中文会交给浏览器默认回退，同一份文档在换字体前后中文观感会悄悄变一次；
// 补上之后语义很明确：英文/数字用你选的字体，缺字形时按这条链回退。
// 回退段必须包含一个「中文正好占 2 格」的等宽字体，否则源码里对齐好的表格会看起来歪掉
// （原因见 settingsStore 里 EDITOR_FONT_FALLBACK_STACK 的注释）。
const buildSystemFontValue = (name) => `${quoteEditorFontFamily(name)}, ${EDITOR_FONT_FALLBACK_STACK}`

const editorFontFamilyOptions = computed(() => {
  const presetValues = new Set(editorFontPresetOptions.map((option) => option.value))
  const systemOptions = systemFontFamilies.value
    // 标签就用家族名本身：选中后下拉框显示的就是这个名字，别让“· 等宽”这类注释跑进选中值旁边。
    .map((font) => ({ label: font.name, value: buildSystemFontValue(font.name) }))
    // 预设里已有的字体不再在系统分组里重复出现。
    .filter((option) => !presetValues.has(option.value))

  // 早期版本存的是单字体值（如 `'MesloLGSDZ Nerd Font'`），它不在上面任何选项里；
  // 补一条“当前”项，否则下拉框只能把整串原值当标签显示。
  const currentFontFamily = String(settingsStore.editorDisplay.font_family || '').trim()
  const knownValues = new Set([...editorFontPresetOptions, ...systemOptions].map((option) => option.value))
  if (currentFontFamily && !knownValues.has(currentFontFamily)) {
    systemOptions.unshift({ label: `当前：${currentFontFamily}`, value: currentFontFamily })
  }

  const systemGroupLabel = systemFontsLoading.value
    ? '系统已安装字体（读取中…）'
    : `系统已安装字体（${systemOptions.length}，等宽在前）`

  return [
    { type: 'group', label: '常用', key: 'preset', children: editorFontPresetOptions },
    { type: 'group', label: systemGroupLabel, key: 'system', children: systemOptions },
  ]
})
const editorDisplayPreviewStyle = computed(() => {
  const display = settingsStore.editorDisplay
  const size = Number(display.font_size) || DEFAULT_EDITOR_DISPLAY_OPTIONS.font_size
  return {
    fontFamily: resolveEditorFontFamily(display.font_family),
    fontSize: `${size}px`,
    // 与 GuaxiEditor 共用同一个换算，预览的行距/换行位置才和实机一致。
    lineHeight: `${resolveEditorLineHeight(size)}px`,
  }
})
const webrtcSignalUrl = ref('')
const terminalDefaultProfile = ref('powershell')
const terminalGitBashPath = ref('')
const ttsProvider = ref('sherpa-onnx')
const ttsModel = ref('')
const ttsTokens = ref('')
const ttsExe = ref('')
/// 默认音色（Speaker ID）。各朗读入口在未显式指定时都会用它，
/// 所以这里是"全局音色"的唯一入口。3 号 = zf_001（中文女声）。
const ttsSpeakerId = ref('3')
/// 全局朗读语速：0.5–2.0，1.0 为原速。划词/逐句朗读没显式传语速时由后端取它。
const ttsSpeed = ref(1)
/// 设置页的筛选条件：只影响下拉列表，不单独保存；音色本身仍以 speaker id 为准。
const ttsLanguage = ref('auto')
const ttsGender = ref('auto')
const ttsLanguageOptions = [
  { label: '不限', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
]
const ttsGenderOptions = [
  { label: '不限', value: 'auto' },
  { label: '女声', value: 'female' },
  { label: '男声', value: 'male' },
]
const ttsSpeakerOptions = computed(() =>
  buildSpeakerOptions(ttsModel.value, { language: ttsLanguage.value, gender: ttsGender.value })
)
const ttsSpeakerHint = computed(() => describeSpeaker(ttsModel.value, ttsSpeakerId.value))

const speakerValuesOf = (options) =>
  (Array.isArray(options) ? options : []).flatMap((option) =>
    option?.type === 'group' ? (option.children || []).map((child) => child.value) : [option?.value]
  )

/** 当前音色不在筛选结果里时，自动落到第一个匹配项，避免下拉显示空值。 */
watch([ttsLanguage, ttsGender], () => {
  const values = speakerValuesOf(ttsSpeakerOptions.value)
  if (!values.length) return
  if (!values.includes(String(ttsSpeakerId.value ?? ''))) {
    ttsSpeakerId.value = values[0]
  }
})

/** 从已保存的 speaker id 反推语言/性别筛选，打开设置页时展示对应的那一组。 */
const syncSpeakerFiltersFromId = () => {
  const meta = speakerMeta(ttsModel.value, ttsSpeakerId.value)
  ttsLanguage.value = meta.language || 'auto'
  ttsGender.value = meta.gender || 'auto'
}

// 直接在下拉里换了一个音色时，筛选条件跟着它走（否则会出现「筛选是中文、
// 选中的却是英文音色」的错位）。筛选 watch 可能回写 id，两边的终态一致，不会循环。
watch(ttsSpeakerId, () => {
  const meta = speakerMeta(ttsModel.value, ttsSpeakerId.value)
  if (meta.language && meta.language !== ttsLanguage.value) ttsLanguage.value = meta.language
  if (meta.gender && meta.gender !== ttsGender.value) ttsGender.value = meta.gender
})
const ttsTestText = ref('你好，这是测试朗读。')
const asrProvider = ref('sherpa-onnx')
const asrModel = ref('')
const asrExe = ref('')
const asrTestFilePath = ref('')
const asrTestResult = ref('')
const loading = ref(false)
const themeSaving = ref(false)
const webrtcSignalSaving = ref(false)
const ttsSaving = ref(false)
const ttsTesting = ref(false)
const asrSaving = ref(false)
const asrTesting = ref(false)
const terminalSaving = ref(false)
const ttsProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
const asrProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
const terminalProfileOptions = [
  { label: 'IPython', value: 'ipython' },
  { label: 'PowerShell', value: 'powershell' },
  { label: 'Git Bash', value: 'git-bash' },
  { label: 'CMD', value: 'cmd' },
  ...(typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
    ? []
    : [
        { label: 'Bash', value: 'bash' },
        { label: 'Zsh', value: 'zsh' },
      ]),
]
let ttsTestAudioObjectUrl = ''
let ttsTestAudioElement = null

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
    await Promise.all([
      settingsStore.loadAppearanceConfig(),
      settingsStore.loadEditorDisplayConfig(),
      (async () => {
        const [seconds, enabled, signalUrl, ttsConfig, asrConfig, terminalConfig, codegenAssist] = await Promise.all([
          invoke('get_gc_cycle_seconds'),
          invoke('get_lsp_syntax_highlight_enabled'),
          invoke('get_webrtc_signal_url'),
          invoke('get_tts_config'),
          invoke('get_asr_config'),
          invoke('get_terminal_config'),
          invoke('get_python_codegen_assist_enabled'),
        ])

        if (typeof seconds === 'number' && seconds > 0) {
          gcCycleHours.value = Math.max(1, Math.round(seconds / 3600))
        } else {
          gcCycleHours.value = 1
        }

        lspHighlightEnabled.value = !!enabled
        pythonCodegenAssistEnabled.value = !!codegenAssist
        webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
        ttsProvider.value = String(ttsConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
        ttsModel.value = String(ttsConfig?.model || '').trim()
        ttsTokens.value = String(ttsConfig?.tokens || '').trim()
        ttsExe.value = String(ttsConfig?.exe || '').trim()
        ttsSpeakerId.value = String(ttsConfig?.speaker_id || '3').trim() || '3'
        ttsSpeed.value = Number(ttsConfig?.speed) > 0 ? Number(ttsConfig.speed) : 1
        syncSpeakerFiltersFromId()
        asrProvider.value = String(asrConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
        asrModel.value = String(asrConfig?.model || '').trim()
        asrExe.value = String(asrConfig?.exe || '').trim()
        terminalDefaultProfile.value = String(terminalConfig?.default_profile || 'powershell').trim() || 'powershell'
        terminalGitBashPath.value = String(terminalConfig?.git_bash_path || '').trim()
      })(),
    ])

    themeValue.value = settingsStore.appearanceTheme
    codeTheme.value = settingsStore.codeTheme
  } catch (error) {
    logger.error('加载偏好设置失败:', error)
    gcCycleHours.value = 1
    lspHighlightEnabled.value = false
    pythonCodegenAssistEnabled.value = false
    themeValue.value = 'light'
    ttsProvider.value = 'sherpa-onnx'
    ttsModel.value = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
    ttsTokens.value = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
    ttsExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
    asrProvider.value = 'sherpa-onnx'
    asrModel.value = 'utils/Models/asr/sense-voice'
    asrExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline.exe'
    terminalDefaultProfile.value = 'powershell'
    terminalGitBashPath.value = ''
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

    logger.info('[Preference] 切换主题:', {
      next,
      current: settingsStore.appearanceTheme,
    })

    await settingsStore.setAppearanceTheme(next)

    try {
      const appearance = await invoke('get_appearance_config')
      logger.info('[Preference] 后端返回的 appearance（用于确认是否写入配置文件）:', appearance)
    } catch (e) {
      logger.error('[Preference] 获取后端 appearance 失败（无法确认是否写入配置文件）:', e)
    }

    logger.info('[Preference] 主题切换完成（已应用并已尝试写入配置文件）:', next)
  } catch (error) {
    logger.error('[Preference] 主题切换失败（可能未写入配置文件）:', error)
  } finally {
    themeSaving.value = false
  }
}

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

const handleLspHighlightChange = async (value) => {
  try {
    loading.value = true
    await invoke('set_lsp_syntax_highlight_enabled', { enabled: value })
    window.dispatchEvent(
      new CustomEvent('guaxi:lsp-setting-changed', {
        detail: { enabled: !!value }
      })
    )
  } catch (error) {
    logger.error('更新 LSP 语法高亮配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleCodeThemeChange = async (value) => {
  try {
    loading.value = true
    // 走 store：它一次性完成「落库 + 换掉 Markdown 代码块样式表 + 让 Monaco 重新应用主题」，
    // 避免这里再单独 invoke 一次后端。
    await settingsStore.setCodeTheme(value)
    codeTheme.value = settingsStore.codeTheme
  } catch (error) {
    logger.error('更新代码配色主题失败:', error)
  } finally {
    loading.value = false
  }
}

const handleMarkdownFormatChange = async (key, value) => {
  try {
    markdownFormatSaving.value = true
    await settingsStore.setMarkdownFormatOption(key, value)
  } catch (error) {
    logger.error('更新 Markdown 格式化选项失败:', error)
  } finally {
    markdownFormatSaving.value = false
  }
}

/**
 * 编辑器外观的三项都走同一条保存路径：先改 store（编辑器上的 watcher 立刻 updateOptions，
 * 所见即所得），再由 store 落盘。字体链是自由输入的，空串挡在这里——
 * 清空等于没配置，放过去会让编辑器回退到浏览器默认字体，和"恢复默认"的语义也不一样。
 */
const handleEditorDisplayChange = async (key, value) => {
  if (key === 'font_family' && !String(value || '').trim()) return
  try {
    editorDisplaySaving.value = true
    await settingsStore.setEditorDisplayOption(key, value)
  } catch (error) {
    logger.error('更新编辑器外观配置失败:', error)
  } finally {
    editorDisplaySaving.value = false
  }
}

const handlePythonCodegenAssistChange = async (value) => {
  try {
    loading.value = true
    await invoke('set_python_codegen_assist_enabled', { enabled: value })
    window.dispatchEvent(
      new CustomEvent('guaxi:python-codegen-setting-changed', {
        detail: { enabled: !!value }
      })
    )
  } catch (error) {
    logger.error('更新 Python 代码生成助手配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleWebrtcSignalUrlSave = async () => {
  try {
    webrtcSignalSaving.value = true
    const url = String(webrtcSignalUrl.value || '').trim()

    logger.info('[Preference] 保存 WebRTC 信令服务地址:', url)

    await invoke('set_webrtc_signal_url', { url })

    try {
      const saved = await invoke('get_webrtc_signal_url')
      logger.info('[Preference] WebRTC 信令服务地址已保存（后端读取确认）:', saved)
      webrtcSignalUrl.value = typeof saved === 'string' ? saved : url
    } catch (e) {
      logger.error('[Preference] 读取 WebRTC 信令服务地址失败（无法确认是否写入配置文件）:', e)
    }
  } catch (error) {
    logger.error('保存 WebRTC 信令服务地址失败:', error)
  } finally {
    webrtcSignalSaving.value = false
  }
}

const handleTerminalConfigSave = async () => {
  try {
    terminalSaving.value = true
    const payload = {
      default_profile: String(terminalDefaultProfile.value || '').trim() || 'powershell',
      git_bash_path: String(terminalGitBashPath.value || '').trim(),
    }
    logger.info('[Preference] 保存终端配置', payload)
    await invoke('set_terminal_config', {
      terminalConfig: payload,
    })
    const saved = await invoke('get_terminal_config')
    terminalDefaultProfile.value = String(saved?.default_profile || payload.default_profile).trim()
      || payload.default_profile
    terminalGitBashPath.value = String(saved?.git_bash_path || payload.git_bash_path).trim()
    logger.info('[Preference] 终端配置已保存', {
      defaultProfile: terminalDefaultProfile.value,
      gitBashPath: terminalGitBashPath.value,
    })
  } catch (error) {
    logger.error('[Preference] 保存终端配置失败:', error)
  } finally {
    terminalSaving.value = false
  }
}

const handleTtsConfigSave = async () => {
  try {
    ttsSaving.value = true
    const current = await invoke('get_tts_config')
    await invoke('set_tts_config', {
      ttsConfig: {
        provider: String(ttsProvider.value || '').trim(),
        model: String(ttsModel.value || '').trim(),
        tokens: String(ttsTokens.value || '').trim(),
        exe: String(ttsExe.value || '').trim(),
        speaker_id: String(ttsSpeakerId.value || '3').trim() || '3',
        speed: Number(ttsSpeed.value) || 1,
        output_dir: String(current?.output_dir || '').trim(),
      }
    })
    const saved = await invoke('get_tts_config')
    ttsProvider.value = String(saved?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    ttsModel.value = String(saved?.model || '').trim()
    ttsTokens.value = String(saved?.tokens || '').trim()
    ttsExe.value = String(saved?.exe || '').trim()
    ttsSpeakerId.value = String(saved?.speaker_id || '3').trim() || '3'
    ttsSpeed.value = Number(saved?.speed) > 0 ? Number(saved.speed) : 1
    syncSpeakerFiltersFromId()
  } catch (error) {
    logger.error('保存 TTS 配置失败:', error)
  } finally {
    ttsSaving.value = false
  }
}

const handleTtsConfigReset = async () => {
  ttsProvider.value = 'sherpa-onnx'
  ttsModel.value = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
  ttsTokens.value = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
  ttsExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
  ttsSpeakerId.value = '3'
  ttsSpeed.value = 1
  syncSpeakerFiltersFromId()
  await handleTtsConfigSave()
}

const handleAsrConfigSave = async () => {
  try {
    asrSaving.value = true
    await invoke('set_asr_config', {
      asrConfig: {
        provider: String(asrProvider.value || '').trim(),
        model: String(asrModel.value || '').trim(),
        exe: String(asrExe.value || '').trim(),
      }
    })
    const saved = await invoke('get_asr_config')
    asrProvider.value = String(saved?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    asrModel.value = String(saved?.model || '').trim()
    asrExe.value = String(saved?.exe || '').trim()
    logger.info('保存 ASR 配置完成', {
      provider: asrProvider.value,
      model: asrModel.value,
      exe: asrExe.value,
    })
  } catch (error) {
    logger.error('保存 ASR 配置失败:', error)
  } finally {
    asrSaving.value = false
  }
}

const handleAsrConfigReset = async () => {
  asrProvider.value = 'sherpa-onnx'
  asrModel.value = 'utils/Models/asr/sense-voice'
  asrExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline.exe'
  await handleAsrConfigSave()
}

const handleAsrTestTranscribe = async () => {
  const filePath = String(asrTestFilePath.value || '').trim()
  if (!filePath || asrTesting.value) return
  try {
    asrTesting.value = true
    asrTestResult.value = ''
    const provider = String(asrProvider.value || '').trim() || 'sherpa-onnx'
    const model = String(asrModel.value || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(asrExe.value || '').trim() : ''
    logger.info('开始测试 ASR 转写', {
      provider,
      model,
      endpoint,
      filePath,
    })
    const response = await invoke('audio_speech_to_text', {
      request: {
        common: {
          provider,
          model,
          endpoint,
        },
        file_path: filePath,
        file_name: filePath.split(/[\\/]/).pop() || 'audio.wav',
        mime_type: 'application/octet-stream',
      }
    })
    asrTestResult.value = String(response?.text || '').trim()
    logger.info('ASR 测试完成', {
      textLength: asrTestResult.value.length,
      traceId: String(response?.trace_id || ''),
      model: String(response?.model || ''),
    })
  } catch (error) {
    logger.error('测试 ASR 转写失败:', error)
    asrTestResult.value = ''
  } finally {
    asrTesting.value = false
  }
}

const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

const stopTtsTestAudioPlayback = () => {
  if (ttsTestAudioElement) {
    ttsTestAudioElement.pause()
    ttsTestAudioElement.src = ''
    ttsTestAudioElement.onended = null
    ttsTestAudioElement = null
  }
  if (ttsTestAudioObjectUrl) {
    URL.revokeObjectURL(ttsTestAudioObjectUrl)
    ttsTestAudioObjectUrl = ''
  }
}

const handleTtsTestReadAloud = async () => {
  const text = String(ttsTestText.value || '').trim()
  if (!text || ttsTesting.value) return
  try {
    ttsTesting.value = true
    const provider = String(ttsProvider.value || '').trim() || 'sherpa-onnx'
    const model = String(ttsModel.value || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(ttsExe.value || '').trim() : ''
    const tokens = provider === 'sherpa-onnx' ? String(ttsTokens.value || '').trim() : ''
    const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'
    logger.info('[PreferenceContent] 测试朗读配置', {
      provider,
      model,
      endpoint,
      responseFormat,
      isSherpaOnnx: provider === 'sherpa-onnx',
    })
    const response = await invoke('audio_text_to_speech', {
      request: {
        common: {
          provider,
          model,
          endpoint,
          tokens,
        },
        input: text,
        // 试听直接用当前选中的音色与语速，这样不用先保存就能比较。
        voice: provider === 'sherpa-onnx' ? String(ttsSpeakerId.value || '3').trim() : 'alex',
        speed: Number(ttsSpeed.value) || 1,
        response_format: responseFormat,
        stream: false,
      },
    })
    const audioBase64 = String(response?.audio_base64 || '')
    if (!audioBase64) {
      throw new Error('语音数据为空')
    }
    const contentType = String(response?.content_type || (responseFormat === 'wav' ? 'audio/wav' : 'audio/mpeg'))
    const bytes = base64ToUint8Array(audioBase64)
    const blob = new Blob([bytes], { type: contentType })
    stopTtsTestAudioPlayback()
    ttsTestAudioObjectUrl = URL.createObjectURL(blob)
    // 设计说明：release 版 WebView 会严格执行 CSP；这里记录 Blob 音频元信息，
    // 便于区分“后端合成异常”和“前端 media-src/解码层拒绝加载”。
    logger.info('[PreferenceContent] 测试朗读音频已准备', {
      contentType,
      audioBytes: bytes.byteLength,
      responseFormat,
      objectUrlProtocol: ttsTestAudioObjectUrl.split(':')[0],
    })
    const audio = new Audio(ttsTestAudioObjectUrl)
    ttsTestAudioElement = audio
    const waitForPlaybackEnd = new Promise((resolve, reject) => {
      audio.onended = () => {
        stopTtsTestAudioPlayback()
        resolve()
      }
      audio.onerror = () => {
        const mediaError = audio.error
        logger.error('[PreferenceContent] 测试朗读音频元素加载失败', {
          contentType,
          audioBytes: bytes.byteLength,
          mediaErrorCode: mediaError?.code || 0,
          mediaErrorMessage: mediaError?.message || '',
          networkState: audio.networkState,
          readyState: audio.readyState,
          currentSrcProtocol: String(audio.currentSrc || '').split(':')[0],
        })
        stopTtsTestAudioPlayback()
        reject(new Error('音频播放失败'))
      }
    })
    logger.info('[PreferenceContent] 开始播放测试朗读音频', {
      contentType,
      audioBytes: bytes.byteLength,
    })
    await audio.play()
    await waitForPlaybackEnd
  } catch (error) {
    logger.error('测试朗读失败:', error)
  } finally {
    ttsTesting.value = false
  }
}

onBeforeUnmount(() => {
  stopTtsTestAudioPlayback()
})

</script>

<style scoped>
.preference-wrapper {
  width: 100%;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

/* 页内主题导航:吸顶,跟随滚动 */
.pref-subnav {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 2px 10px;
  margin-bottom: 4px;
  background: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  backdrop-filter: blur(4px);
}

.pref-subnav-btn {
  appearance: none;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font: inherit;
  font-size: 12.5px;
  line-height: 1.2;
  padding: 5px 12px;
  border-radius: 999px;
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease, border-color 0.16s ease, transform 0.12s ease;
}
.pref-subnav-btn:hover {
  color: var(--guaxi-text-primary);
  border-color: var(--n-color-primary, #2dd4bf);
  transform: translateY(-1px);
}
.pref-subnav-btn.active {
  color: #fff;
  background: var(--n-color-primary, #2dd4bf);
  border-color: var(--n-color-primary, #2dd4bf);
  box-shadow: 0 2px 8px rgba(45, 212, 191, 0.28);
}

.pref-sections {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-top: 6px;
}

/* 主题卡片:统一描边、圆角、留白;锚点滚动留出吸顶导航高度 */
.pref-card {
  scroll-margin-top: 64px;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
  transition: box-shadow 0.18s ease, border-color 0.18s ease;
}
.pref-card:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.07);
}
.pref-card :deep(.n-card-header__main) {
  font-weight: 600;
  letter-spacing: 0.3px;
}

/* 卡片内的次级小节(语音页 TTS / ASR) */
.pref-block + .pref-block,
.pref-block {
  width: 100%;
}
.pref-card h3 {
  margin: 0 0 2px;
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.description {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

/* 编辑器外观的实时预览：字体/字号/行高由行内 style 跟着 store 走，这里只管排版与描边。
   代码配色用主题变量，和「代码配色主题」那两项的实际效果保持同一套色。 */
.editor-display-preview {
  max-width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  white-space: pre;
  overflow-x: auto;
  font-variant-ligatures: contextual;
}

.editor-display-preview .tok-kw {
  color: var(--guaxi-syntax-keyword);
}

.editor-display-preview .tok-fn {
  color: var(--guaxi-syntax-function);
}

.editor-display-preview .tok-str {
  color: var(--guaxi-syntax-string);
}

.editor-display-preview .tok-cmt {
  color: var(--guaxi-syntax-comment);
}

.lsp-status-panel {
  margin-top: 12px;
  padding: 12px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
}

.lsp-status-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.lsp-status-title {
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.lsp-status-loading,
.lsp-status-empty {
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.lsp-status-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.lsp-status-item {
  padding: 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-content-bg);
}

.lsp-status-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.lsp-status-name {
  font-weight: 500;
  color: var(--guaxi-text-primary);
}

.lsp-status-chip {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 12px;
  background: var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

.lsp-status-chip.ok {
  background: rgba(25, 113, 194, 0.12);
  color: #2b6cb0;
}

.lsp-status-chip.warn {
  background: rgba(245, 159, 0, 0.15);
  color: #c77d00;
}

.lsp-status-chip.info {
  background: rgba(132, 94, 194, 0.15);
  color: #6b46c1;
}

.lsp-status-chip.loading {
  background: rgba(0, 0, 0, 0.08);
  color: var(--guaxi-text-secondary);
}

.lsp-status-path {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  word-break: break-all;
}

.lsp-status-message {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.lsp-status-actions {
  margin-top: 6px;
}

.tip {
  margin-top: 8px;
}
</style>
