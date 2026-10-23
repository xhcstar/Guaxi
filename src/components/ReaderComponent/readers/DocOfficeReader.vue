<template>
  <!--
    Office 文档的统一入口。

    【为什么不再提供只读渲染】
    过去 Guaxi 自己渲染 docx/xlsx/pptx：效果永远是「差不多但不对」——分页、母版、图表、
    修订都还原不了，于是阅读用一个阅读器、编辑又要换另一个程序，同一份文件在用户眼里
    变成两种不同的东西。这既不值得维护，也不可能做到生产质量。

    现在的唯一职责是**把文件交给真正会编辑它的程序**，按优先级：

      1. 本机装了 Microsoft Office / WPS → 用它打开。这是用户已有工作流的延伸，
         Office 自己就是最权威的读写实现，存回原文件天然无损。
      2. 没装 → 用 WordCraft / GridCraft / DeckCraft。它们是各自格式的原生实现，
         同样直接编辑原文件，不是「预览」。
      3. 两个都没有 → 明确说明缺什么、去哪装，不假装能打开。

    选完之后本组件就退场：文件在外部程序里编辑，Guaxi 不参与、也不复制一份中间产物。
  -->

  <div class="gdoc-office">
    <div class="gdoc-office__bar">
      <span class="gdoc-office__title" :title="path">{{ name }}</span>
      <span class="gdoc-office__badge">{{ kind.toUpperCase() }}</span>
      <span class="gdoc-office__meta">{{ sizeLabel }}</span>
      <span class="gdoc-office__spacer" />
      <button
        class="guaxi-doc-reader__btn"
        type="button"
        :disabled="launching"
        :title="primaryHint"
        @click="openPrimary"
      >
        {{ primaryLabel }}
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        type="button"
        title="用系统默认程序打开（等同于在资源管理器里双击）"
        aria-label="用系统默认程序打开"
        @click="openDefault"
      >
        <Icon :icon="folderIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        type="button"
        title="重新检测本机办公套件"
        aria-label="重新检测办公套件"
        @click="probe"
      >
        <Icon :icon="refreshIcon" width="15" height="15" />
      </button>
    </div>

    <div class="gdoc-office__body">
      <div class="gdoc-office__card">
        <Icon :icon="docIcon" width="34" height="34" class="gdoc-office__icon" />

        <div class="gdoc-office__headline">{{ headline }}</div>
        <div class="gdoc-office__sub">{{ subline }}</div>

        <ul v-if="!probePending" class="gdoc-office__routes">
          <li v-for="r in routes" :key="r.key" :class="['gdoc-office__route', { 'is-ready': r.ready }]">
            <span class="gdoc-office__route-main">
              <span class="gdoc-office__route-name">{{ r.name }}</span>
              <span class="gdoc-office__route-note">{{ r.note }}</span>
            </span>
            <span class="gdoc-office__route-state">{{ r.ready ? '可用' : '未安装' }}</span>
          </li>
        </ul>

        <div v-if="errorText" class="gdoc-office__error">{{ errorText }}</div>

        <div class="gdoc-office__actions">
          <button
            v-if="!probePending"
            class="gdoc-office__btn gdoc-office__btn--primary"
            type="button"
            :disabled="launching"
            @click="openPrimary"
          >
            {{ primaryLabel }}
          </button>
          <button class="gdoc-office__btn" type="button" @click="openDefault">
            用系统默认程序
          </button>
        </div>

        <p v-if="!officeReady && !craftReady && !probePending" class="gdoc-office__hint">
          Guaxi 不再内置 Office 只读渲染：它无法忠实还原分页、母版与图表，反而会让同一份文件
          出现两种样子。安装上面任意一个即可直接打开并编辑。
        </p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import folderIcon from '@iconify-icons/lucide/folder-open'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import docIcon from '@iconify-icons/lucide/file-text'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocOffice')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  /** docx / doc / rtf / odt / xlsx / xls / pptx / ppt … */
  kind: { type: String, default: '' },
  size: { type: Number, default: 0 },
})

/** 把扩展名收敛到后端认识的三类。 */
const slot = computed(() => {
  const k = String(props.kind || '').toLowerCase()
  if (['xlsx', 'xls', 'csv', 'ods', 'grid'].includes(k)) return 'grid'
  if (['pptx', 'ppt', 'odp', 'deck'].includes(k)) return 'deck'
  return 'word'
})

const slotLabel = computed(() => ({ word: '文字文档', grid: '表格', deck: '演示文稿' })[slot.value])

const sizeLabel = computed(() => {
  const n = Number(props.size || 0)
  if (!n) return ''
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
})

const probePending = ref(true)
const launching = ref(false)
const errorText = ref('')
/** probe_office_suites 的结果：{ word|grid|deck: { suite, displayName, executable } } */
const suites = ref(null)
/** probe_craft_apps 的结果：[{ name, available, path }] */
const crafts = ref([])

const SUITE_KEY = { word: 'word', grid: 'grid', deck: 'deck' }
const CRAFT_NAME = { word: 'WordCraft', grid: 'GridCraft', deck: 'DeckCraft' }
/** craft_app.rs 里 exe_stem 的显示名是小写比较。 */
const craftForSlot = computed(() => {
  const want = CRAFT_NAME[slot.value].toLowerCase()
  return crafts.value.find((c) => String(c?.name || '').toLowerCase() === want) || null
})

const officeInfo = computed(() => suites.value?.[SUITE_KEY[slot.value]] || null)
const officeReady = computed(() => {
  const s = officeInfo.value?.suite
  return Boolean(s && s !== 'none')
})
const craftReady = computed(() => Boolean(craftForSlot.value?.available))

const headline = computed(() => {
  if (probePending.value) return '正在检测可用的办公程序…'
  if (officeReady.value) return `已检测到 ${officeInfo.value.displayName}`
  if (craftReady.value) return `已检测到 ${CRAFT_NAME[slot.value]}`
  return `没有找到能编辑${slotLabel.value}的程序`
})

const subline = computed(() => {
  if (probePending.value) return ''
  if (officeReady.value) {
    return `${officeInfo.value.displayName} 会直接打开并保存原文件，Guaxi 不做转换`
  }
  if (craftReady.value) {
    return `${CRAFT_NAME[slot.value]} 直接编辑原文件；装上 Microsoft Office 或 WPS 后会自动优先使用它们`
  }
  return '安装 Microsoft Office、WPS，或 WordCraft / GridCraft / DeckCraft 任意一个'
})

const routes = computed(() => {
  if (probePending.value) return []
  const ms = officeInfo.value
  return [
    {
      key: 'office',
      name: ms?.suite === 'microsoftoffice' ? 'Microsoft Office' : ms?.suite === 'wps' ? 'WPS Office' : 'Microsoft Office / WPS',
      note: ms?.suite && ms.suite !== 'none' ? ms.executable : '未检测到安装',
      ready: officeReady.value,
    },
    {
      key: 'craft',
      name: CRAFT_NAME[slot.value],
      note: craftForSlot.value?.path || '未检测到安装',
      ready: craftReady.value,
    },
  ]
})

const primaryLabel = computed(() => {
  if (launching.value) return '正在打开…'
  if (officeReady.value) return `用 ${officeInfo.value.displayName} 打开`
  if (craftReady.value) return `用 ${CRAFT_NAME[slot.value]} 打开`
  return '打开失败：本机没有可用的办公程序'
})

const primaryHint = computed(() => primaryLabel.value)

const probe = async () => {
  probePending.value = true
  errorText.value = ''
  try {
    const [s, c] = await Promise.all([
      invoke('probe_office_suites'),
      invoke('probe_craft_apps'),
    ])
    suites.value = s || null
    crafts.value = Array.isArray(c) ? c : []
    logger.info('[DocOffice] probed', {
      slot: slot.value,
      office: officeInfo.value?.suite || 'unknown',
      craft: craftForSlot.value?.name || 'none',
    })
  } catch (error) {
    // 探测失败不该让人以为「什么都没装」：如实说明，并允许手动重试与默认程序兜底。
    errorText.value = `检测失败：${String(error?.message || error)}`
    logger.warn('[DocOffice] probe failed', { error: String(error?.message || error) })
  } finally {
    probePending.value = false
  }
}

const openPrimary = async () => {
  if (launching.value) return
  errorText.value = ''
  if (officeReady.value) {
    launching.value = true
    try {
      await invoke('open_in_office', { path: props.path, kind: slot.value })
      logger.info('[DocOffice] opened with office', { suite: officeInfo.value?.suite })
      return
    } catch (error) {
      errorText.value = String(error?.message || error)
      logger.warn('[DocOffice] open_in_office failed', { error: String(error?.message || error) })
    } finally {
      launching.value = false
    }
  }
  if (craftReady.value) {
    launching.value = true
    try {
      await invoke('open_in_craft_app', { path: props.path, kind: slot.value })
      logger.info('[DocOffice] opened with craft')
      return
    } catch (error) {
      errorText.value = String(error?.message || error)
      logger.warn('[DocOffice] open_in_craft_app failed', { error: String(error?.message || error) })
    } finally {
      launching.value = false
    }
  }
  if (!errorText.value) {
    errorText.value = '本机没有可用的办公程序。请先安装 Microsoft Office、WPS，或对应的 Craft 应用。'
  }
}

const openDefault = async () => {
  errorText.value = ''
  try {
    await invoke('open_with_system_default', { path: props.path })
    logger.info('[DocOffice] opened with system default')
  } catch (error) {
    errorText.value = String(error?.message || error)
    logger.warn('[DocOffice] open_with_system_default failed', {
      error: String(error?.message || error),
    })
  }
}

onMounted(() => {
  void probe()
})
</script>

<style scoped>
.gdoc-office {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--guaxi-content-bg, #fff);
  color: var(--guaxi-text-primary, #1f2328);
  z-index: 8;
}

.gdoc-office__bar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 10px;
  border-bottom: 1px solid var(--guaxi-border, #e1e4e8);
  font-size: 12px;
}
.gdoc-office__title {
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 42%;
}
.gdoc-office__badge {
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--guaxi-hover-bg, rgba(120, 120, 120, 0.12));
  color: var(--guaxi-text-secondary, #6a737d);
  font-size: 11px;
  white-space: nowrap;
}
.gdoc-office__meta {
  color: var(--guaxi-text-secondary, #6a737d);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.gdoc-office__spacer { flex: 1 1 auto; }

.gdoc-office__body {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  overflow: auto;
}

.gdoc-office__card {
  width: min(520px, 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  text-align: center;
}
.gdoc-office__icon {
  opacity: 0.55;
  margin-bottom: 2px;
}
.gdoc-office__headline {
  font-size: 15px;
  font-weight: 600;
}
.gdoc-office__sub {
  font-size: 12.5px;
  line-height: 1.65;
  max-width: 46ch;
  color: var(--guaxi-text-secondary, #6a737d);
}

.gdoc-office__routes {
  width: 100%;
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
  text-align: left;
}
.gdoc-office__route {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border, #e1e4e8);
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 3%, transparent);
}
.gdoc-office__route.is-ready {
  border-color: color-mix(in srgb, var(--n-color-primary, #61afef) 45%, transparent);
}
.gdoc-office__route-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1 1 auto;
}
.gdoc-office__route-name { font-size: 12.5px; font-weight: 600; }
.gdoc-office__route-note {
  font-size: 11px;
  color: var(--guaxi-text-secondary, #6a737d);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}
.gdoc-office__route-state {
  flex: 0 0 auto;
  font-size: 11px;
  color: var(--guaxi-text-secondary, #6a737d);
}
.gdoc-office__route.is-ready .gdoc-office__route-state {
  color: #2da44e;
}

.gdoc-office__error {
  width: 100%;
  padding: 8px 12px;
  border-radius: 8px;
  background: rgba(207, 34, 46, 0.08);
  color: #cf222e;
  font-size: 12px;
  text-align: left;
}

.gdoc-office__actions {
  display: flex;
  gap: 8px;
  margin-top: 4px;
  flex-wrap: wrap;
  justify-content: center;
}
.gdoc-office__btn {
  padding: 7px 14px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border, #e1e4e8);
  background: transparent;
  color: inherit;
  font-size: 12.5px;
  cursor: pointer;
}
.gdoc-office__btn:hover:not(:disabled) {
  background: var(--guaxi-hover-bg, rgba(120, 120, 120, 0.12));
}
.gdoc-office__btn--primary {
  border-color: color-mix(in srgb, var(--n-color-primary, #61afef) 55%, transparent);
  color: var(--n-color-primary, #61afef);
  font-weight: 600;
}
.gdoc-office__btn:disabled { opacity: 0.5; cursor: not-allowed; }

.gdoc-office__hint {
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  max-width: 48ch;
  color: var(--guaxi-text-secondary, #6a737d);
  opacity: 0.8;
}
</style>
