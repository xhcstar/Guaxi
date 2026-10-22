<template>
  <!--
    「在外部编辑器打开」按钮。

    Office 文档（docx / xlsx / pptx）在 Guaxi 里是**只读预览**，真正的编辑交给
    WordCraft / GridCraft / DeckCraft。这样做的理由不是偷懒：

    - 那三个项目是各自格式的原生实现（修订、样式、公式、母版、图表），要把它们的能力
      复刻进 Guaxi 是数人月的工作，而复用只需一次进程启动；
    - 「编辑并保存 Office 文件」最容易出错的地方是**无损回写**，用它们自己的读写实现
      比我们自己拼 OOXML 安全得多。

    按钮是否显示由 probe_craft_apps 决定：没装对应工具时给出明确提示，
    而不是点了没反应。
  -->
  <button
    v-if="available"
    type="button"
    class="guaxi-doc-reader__icon-btn craft-edit-btn"
    :disabled="launching"
    :title="`在 ${appName} 中编辑`"
    :aria-label="`在 ${appName} 中编辑`"
    @click.stop="open"
  >
    <Icon :icon="editIcon" width="15" height="15" />
  </button>
  <button
    v-else
    type="button"
    class="guaxi-doc-reader__icon-btn craft-edit-btn is-unavailable"
    :title="`未检测到 ${appName}。点击查看安装说明`"
    :aria-label="`${appName} 未安装，点击查看安装说明`"
    @click.stop="showMissing"
  >
    <Icon :icon="editIcon" width="15" height="15" />
  </button>

  <!--
    用 CLI 把当前文档转成 PDF 并打开。

    这条路径的价值：Office 文档在 Guaxi 里是只读预览，缺搜索/大纲/批注；
    转成 PDF 后可以复用完整的 PDF 阅读器。对「只想读、不想编辑」的场景，
    它比启动一个完整的 Office GUI 轻得多。
  -->
  <button
    v-if="cliAvailable"
    type="button"
    class="guaxi-doc-reader__icon-btn"
    :disabled="exporting"
    :title="exporting ? '正在导出 PDF…' : '导出为 PDF 并用阅读器打开'"
    :aria-label="exporting ? '正在导出 PDF' : '导出为 PDF 并用阅读器打开'"
    @click.stop="exportPdf"
  >
    <Icon :icon="exporting ? spinnerIcon : pdfIcon" width="15" height="15" />
  </button>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import editIcon from '@iconify-icons/lucide/edit-2'
import pdfIcon from '@iconify-icons/lucide/file-down'
import spinnerIcon from '@iconify-icons/lucide/loader-2'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('CraftEditButton')

const props = defineProps({
  path: { type: String, required: true },
  /** 'docx' / 'xlsx' / 'pptx' 等扩展名，或 'word' / 'grid' / 'deck' */
  kind: { type: String, default: '' },
})

// 与后端 CraftApp::exe_stem() 对齐
const EXE_BY_KIND = {
  docx: 'wordcraft', doc: 'wordcraft', word: 'wordcraft',
  xlsx: 'gridcraft', xls: 'gridcraft', grid: 'gridcraft',
  pptx: 'deckcraft', ppt: 'deckcraft', deck: 'deckcraft',
}
const NAME_BY_KIND = {
  wordcraft: 'WordCraft',
  gridcraft: 'GridCraft',
  deckcraft: 'DeckCraft',
}

const exeStem = computed(() => EXE_BY_KIND[String(props.kind || '').toLowerCase()] || '')
const appName = computed(() => NAME_BY_KIND[exeStem.value] || '外部编辑器')

const available = ref(false)
const cliAvailable = ref(false)
const launching = ref(false)
const exporting = ref(false)

onMounted(async () => {
  if (!exeStem.value) return
  try {
    const result = await invoke('probe_craft_apps')
    const entry = result?.[exeStem.value]
    available.value = !!entry?.available
    logger.info('[CraftEdit] probe', {
      kind: props.kind,
      exe: exeStem.value,
      available: available.value,
      path: entry?.path || '',
    })
  } catch (error) {
    logger.warn('[CraftEdit] probe failed', { error: String(error?.message || error) })
  }

  try {
    const clis = await invoke('probe_craft_clis')
    cliAvailable.value = !!clis?.[`${exeStem.value}-cli`]?.available
  } catch (error) {
    logger.debug('[CraftEdit] probe cli failed', { error: String(error?.message || error) })
  }
})

/**
 * 导出 PDF 并用阅读器打开。
 *
 * 输出到「原文件同目录 / 同名 .pdf」，重名时加序号，绝不覆盖用户已有文件。
 * 导出成功后直接跳到 PDF 阅读器 —— 用户本来就只是想读，这一跳是符合预期的。
 */
const exportPdf = async () => {
  if (exporting.value) return
  exporting.value = true
  try {
    const { join, dirname } = await import('@tauri-apps/api/path')
    const { exists } = await import('@tauri-apps/plugin-fs')

    const dir = await dirname(props.path)
    const base = String(props.path).replace(/\\/g, '/').split('/').pop() || 'document'
    const stem = base.replace(/\.[^.]+$/, '')

    // 不覆盖：同名已存在就依次试 -1、-2…
    let output = await join(dir, `${stem}.pdf`)
    let seq = 1
    while (await exists(output).catch(() => false)) {
      output = await join(dir, `${stem}-${seq}.pdf`)
      seq += 1
      if (seq > 200) throw new Error('同名 PDF 过多，请先清理')
    }

    logger.info('[CraftEdit] exporting pdf', { input: props.path, output })
    const result = await invoke('craft_cli_export_pdf', {
      path: props.path,
      kind: props.kind || exeStem.value,
      output,
    })

    if (!result?.ok) {
      throw new Error(result?.message || '导出失败')
    }

    // 交给宿主打开：阅读器自己不知道 PDF 阅读器挂在哪儿。
    const { emit } = await import('@tauri-apps/api/event')
    await emit('doc-reader:open-pdf', { path: output })
    logger.info('[CraftEdit] pdf exported', { output })
  } catch (error) {
    logger.warn('[CraftEdit] export pdf failed', { error: String(error?.message || error) })
    window.alert?.(`导出 PDF 失败：${String(error?.message || error)}`)
  } finally {
    exporting.value = false
  }
}

const open = async () => {
  if (launching.value) return
  launching.value = true
  try {
    const result = await invoke('open_in_craft_app', {
      path: props.path,
      kind: props.kind || exeStem.value,
    })
    logger.info('[CraftEdit] launched', result)
  } catch (error) {
    logger.warn('[CraftEdit] launch failed', { error: String(error?.message || error) })
    window.alert?.(String(error?.message || error))
  } finally {
    launching.value = false
  }
}

const showMissing = () => {
  const hint = `未检测到 ${appName.value}。\n\n` +
    `把 ${appName.value} 安装到 Guaxi 程序目录下的 craft/ 子目录，\n` +
    `或设置环境变量 GUAXI_CRAFT_DIR 指向它的安装目录。\n\n` +
    `下载地址：https://getartcraft.com/`
  window.alert?.(hint)
}
</script>

<style scoped>
.craft-edit-btn.is-unavailable {
  opacity: 0.55;
}
</style>
