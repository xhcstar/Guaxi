<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="编辑器">
      <h3>编辑器外观</h3>
      <p class="setting-page-desc">
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
            <span class="setting-page-desc">
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
            <span class="setting-page-desc">
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
            <span class="setting-page-desc">编辑器右侧的代码缩略图，默认关闭。</span>
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
      <p class="setting-page-desc">
        提示：编辑器里按住 Ctrl 滚动滚轮也能临时缩放，但那只是当前窗口的临时值，不会写进配置，
        重新打开后会回到这里的字号。
      </p>

      <n-divider />

      <h3>Markdown 图片</h3>
      <p class="setting-page-desc">
        预览与探索者里图片的显示口径。宽度与圆角通过 CSS 变量全局下发，改完立刻生效，
        不用重开文档，也不用重新渲染。
      </p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="宽度上限">
          <n-space align="center" :size="8">
            <n-input-number
              :value="settingsStore.previewImage.max_width"
              :min="PREVIEW_IMAGE_MAX_WIDTH_MIN"
              :max="PREVIEW_IMAGE_MAX_WIDTH_MAX"
              :step="20"
              :precision="0"
              :disabled="loading || previewImageSaving"
              style="width: 140px"
              @update:value="(v) => handlePreviewImageChange('max_width', Number(v) || DEFAULT_PREVIEW_IMAGE_OPTIONS.max_width)"
            />
            <span class="setting-page-desc">
              允许 {{ PREVIEW_IMAGE_MAX_WIDTH_MIN }}–{{ PREVIEW_IMAGE_MAX_WIDTH_MAX }}px，默认 1400。
              这是单图的上限：原图更宽时等比收缩到该宽度，更窄时按原始尺寸显示（低于 460px 的小图会
              放大到 460px，免得截图小图看不清）。图集按列宽平分，不受这项限制。
            </span>
          </n-space>
        </n-form-item>
        <n-form-item label="圆角">
          <n-space align="center" :size="8">
            <n-input-number
              :value="settingsStore.previewImage.radius"
              :min="0"
              :max="PREVIEW_IMAGE_RADIUS_MAX"
              :step="1"
              :precision="0"
              :disabled="loading || previewImageSaving"
              style="width: 140px"
              @update:value="(v) => handlePreviewImageChange('radius', Number(v) || 0)"
            />
            <span class="setting-page-desc">
              0–{{ PREVIEW_IMAGE_RADIUS_MAX }}px，默认 12。设为 0 是直角，截图的四个角完全方正。
            </span>
          </n-space>
        </n-form-item>
        <n-form-item label="显示图注">
          <n-space align="center" :size="8">
            <n-switch
              :value="settingsStore.previewImage.show_caption"
              :disabled="loading || previewImageSaving"
              @update:value="(v) => handlePreviewImageChange('show_caption', !!v)"
            />
            <span class="setting-page-desc">
              把图片的 alt 文字显示在图片下方（13px、居中、弱化色）。alt 为空的图片不占位，
              图集里图注最多两行。
            </span>
          </n-space>
        </n-form-item>
        <n-form-item label="预览">
          <div class="preview-image-sample">
            <div
              class="preview-image-sample__frame"
              :style="previewImageSampleStyle"
            >
              <span class="preview-image-sample__hint">示例图片</span>
            </div>
            <span v-if="settingsStore.previewImage.show_caption" class="preview-image-sample__caption">
              示例图注：这里就是 alt 文字
            </span>
          </div>
        </n-form-item>
      </n-form>

      <n-divider />

      <h3>LSP 配置</h3>
      <p class="setting-page-desc">
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
            :value="settingsStore.codeTheme"
            :options="codeThemeOptions"
            :disabled="loading"
            style="max-width: 240px"
            @update:value="handleCodeThemeChange"
          />
        </n-form-item>
      </n-form>
      <p class="setting-page-desc">
        代码配色同时作用于编辑器里的代码块、探索者 cell，以及 Markdown 预览/阅读器/导出的代码块。
        每套主题自带亮色与暗色两套配色，随外观主题自动切换。
      </p>

      <n-divider />

      <h3>Typst 包下载</h3>
      <p class="setting-page-desc">
        在 .typ 文档里 <code>#import "@preview/..."</code> 引用包时，首次编译会联网下载。
        官方源在境外，不配代理时可能很慢。这里可以指定镜像地址与代理，
        已下载的包会缓存到本地，之后不再联网。留空表示使用默认值。
      </p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="下载地址">
          <n-input
            v-model:value="typstPackage.registry"
            placeholder="https://packages.typst.org"
            :disabled="loading || typstPackageSaving"
            style="max-width: 420px"
          />
        </n-form-item>
        <n-form-item label="缓存目录">
          <n-input
            v-model:value="typstPackage.cache_dir"
            placeholder="留空使用系统默认缓存目录"
            :disabled="loading || typstPackageSaving"
            style="max-width: 420px"
          />
        </n-form-item>
        <n-form-item label="代理">
          <n-input
            v-model:value="typstPackage.proxy"
            placeholder="例如 http://127.0.0.1:7890"
            :disabled="loading || typstPackageSaving"
            style="max-width: 420px"
          />
        </n-form-item>
        <n-form-item label="离线模式">
          <n-space align="center" :size="8">
            <n-switch
              v-model:value="typstPackage.offline"
              :disabled="loading || typstPackageSaving"
              @update:value="saveTypstPackage"
            />
            <span class="setting-page-desc">打开后只用本地已缓存的包，缺包会直接报错而不是联网下载。</span>
          </n-space>
        </n-form-item>
        <n-form-item label=" ">
          <n-space align="center" :size="12">
            <n-button size="small" type="primary" :loading="typstPackageSaving" @click="saveTypstPackage">
              保存
            </n-button>
            <span class="setting-page-desc">{{ typstPackageHint }}</span>
          </n-space>
        </n-form-item>
      </n-form>

      <n-divider />

      <h3>Markdown 格式化</h3>
      <p class="setting-page-desc">
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
      <p class="setting-page-desc">
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
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onMounted } from 'vue'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInput, NInputNumber, NSwitch, NSelect, NButton } from 'naive-ui'
import { GUAXI_CODE_THEMES } from '../../monaco/codeThemes'
import { invoke } from '@tauri-apps/api/core'
import {
  useSettingsStore,
  DEFAULT_EDITOR_DISPLAY_OPTIONS,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_FALLBACK_STACK,
  resolveEditorFontFamily,
  resolveEditorLineHeight,
  DEFAULT_PREVIEW_IMAGE_OPTIONS,
  PREVIEW_IMAGE_MAX_WIDTH_MIN,
  PREVIEW_IMAGE_MAX_WIDTH_MAX,
  PREVIEW_IMAGE_RADIUS_MAX,
} from '../../stores/settingsStore'
import LspExtensionPanel from './LspExtensionPanel.vue'

const logger = createLogger('EditorContent')

const settingsStore = useSettingsStore()

const lspHighlightEnabled = ref(false)
// 主题清单是前端资产（每套自带明暗两套调色板 + 上游 highlight.js 样式表），后端只存 id 字符串。
const codeThemeOptions = GUAXI_CODE_THEMES.map((theme) => ({ label: theme.label, value: theme.id }))
const pythonCodegenAssistEnabled = ref(false)
const markdownFormatSaving = ref(false)
const editorDisplaySaving = ref(false)
const previewImageSaving = ref(false)
const loading = ref(false)

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

/**
 * 图片示例框：宽度、圆角按当前设置 1:1 渲染，面板放不下时才收缩。
 * 只用真实的 px 值，不做等比缩放——否则把 720 调到 860 在窄面板里看不出变化，
 * 用户会以为设置没生效。
 */
const previewImageSampleStyle = computed(() => {
  const image = settingsStore.previewImage || {}
  const width = Number(image.max_width) || DEFAULT_PREVIEW_IMAGE_OPTIONS.max_width
  const radius = Number(image.radius)
  return {
    width: `min(100%, ${width}px)`,
    borderRadius: `${Number.isFinite(radius) && radius >= 0 ? radius : DEFAULT_PREVIEW_IMAGE_OPTIONS.radius}px`,
  }
})

// Typst 包下载配置。默认值留空：后端对空串的解释就是「用默认」，
// 所以界面不需要知道默认的 registry 常量是什么，也不会把默认值写进配置。
const typstPackage = ref({ registry: '', cache_dir: '', proxy: '', offline: false })
const typstPackageSaving = ref(false)
const typstPackageHint = ref('')

const loadTypstPackageConfig = async () => {
  try {
    const cfg = await invoke('get_typst_package_config')
    typstPackage.value = {
      registry: cfg?.registry || '',
      cache_dir: cfg?.cache_dir || '',
      proxy: cfg?.proxy || '',
      offline: !!cfg?.offline,
    }
  } catch (error) {
    logger.warn('加载 Typst 包下载配置失败:', error)
  }
}

const saveTypstPackage = async () => {
  try {
    typstPackageSaving.value = true
    const res = await invoke('set_typst_package_config', {
      registry: typstPackage.value.registry || '',
      cacheDir: typstPackage.value.cache_dir || '',
      proxy: typstPackage.value.proxy || '',
      offline: !!typstPackage.value.offline,
    })
    // applied=false 表示本进程已经初始化过配置，需要重启应用才生效——
    // 这一点必须如实告诉用户，否则他会以为保存了却没作用。
    typstPackageHint.value = res?.applied ? '已保存' : '已保存，重启应用后生效'
    if (res?.current) {
      typstPackage.value = {
        registry: res.current.registry || '',
        cache_dir: res.current.cache_dir || '',
        proxy: res.current.proxy || '',
        offline: !!res.current.offline,
      }
    }
  } catch (error) {
    logger.error('保存 Typst 包下载配置失败:', error)
    typstPackageHint.value = `保存失败：${String(error?.message || error || '')}`
  } finally {
    typstPackageSaving.value = false
  }
}

onMounted(async () => {
  try {
    loading.value = true
    const [enabled, codegenAssist] = await Promise.all([
      invoke('get_lsp_syntax_highlight_enabled'),
      invoke('get_python_codegen_assist_enabled'),
      settingsStore.loadEditorDisplayConfig(),
      settingsStore.loadCodeThemeConfig(),
      settingsStore.loadMarkdownFormatConfig(),
      loadTypstPackageConfig(),
    ])
    lspHighlightEnabled.value = !!enabled
    pythonCodegenAssistEnabled.value = !!codegenAssist
  } catch (error) {
    logger.error('加载编辑器设置失败:', error)
    lspHighlightEnabled.value = false
    pythonCodegenAssistEnabled.value = false
  } finally {
    loading.value = false
  }
})

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

/**
 * 图片显示三项：先让 store 把 CSS 变量刷到根节点（预览/探索者立刻变化），再落盘。
 * 落盘失败时 store 已经把值改了，这里只记日志不回滚——回滚会让界面在
 * 「已经看到变化」和「设置项跳回旧值」之间来回闪，体验更差。
 */
const handlePreviewImageChange = async (key, value) => {
  try {
    previewImageSaving.value = true
    await settingsStore.setPreviewImageOption(key, value)
  } catch (error) {
    logger.error('更新 Markdown 图片设置失败:', error)
  } finally {
    previewImageSaving.value = false
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
</script>

<style scoped>
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

/* 图片示例框：宽度与圆角由行内 style 跟着 store 走，
   这里只给底色/描边，让它看起来像一张“截图”而不是一块色块。 */
.preview-image-sample {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  max-width: 100%;
}

.preview-image-sample__frame {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 132px;
  /* 与真实图片同一道极淡描边，圆角变化才看得出来。 */
  border: 1px solid var(--guaxi-border-weak, rgba(15, 23, 42, 0.12));
  background: repeating-linear-gradient(
    135deg,
    var(--guaxi-content-bg) 0 12px,
    var(--guaxi-hover-bg, rgba(15, 23, 42, 0.04)) 12px 24px
  );
  color: var(--guaxi-text-tertiary, rgba(107, 114, 128, 0.9));
  font-size: 12px;
}

.preview-image-sample__caption {
  max-width: 100%;
  font-size: 13px;
  line-height: 1.55;
  color: var(--guaxi-text-tertiary, rgba(107, 114, 128, 0.92));
  text-align: center;
}
</style>
