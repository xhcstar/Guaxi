// 简历的文件出口：HTML、PDF（独立打印 iframe）、项目文件（JSON 打包）。
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { createDiscreteApi } from 'naive-ui'
import { createLogger } from '../../../../utils/logger'
import { renderResumeHtml, stripScriptsForPreview } from './render/renderResumeHtml'

const logger = createLogger('ResumeExport')

const safeBaseName = (title) => String(title || 'resume')
  .replace(/[\\/:*?"<>|]/g, '')
  .trim()
  .replace(/\s+/g, '-')
  .slice(0, 60) || 'resume'

export function useResumeExport(store) {
  const { message } = createDiscreteApi(['message'])

  const fileName = computed(() => safeBaseName(store.doc?.title))

  function fullHtml() {
    return renderResumeHtml(store.doc, store.activeTemplateHtml)
  }

  async function writeFile(path, content) {
    await invoke('write_file_content', { path, content })
  }

  async function exportHtml() {
    if (!store.doc) return
    try {
      const path = await save({
        title: '导出简历 HTML',
        defaultPath: `${fileName.value}.html`,
        filters: [{ name: 'HTML', extensions: ['html'] }],
      })
      if (!path) return
      await writeFile(String(path), fullHtml())
      logger.info('[Resume] html exported', { path })
      message.success('HTML 已导出')
    } catch (error) {
      logger.error('[Resume] html export failed', error)
      message.error('HTML 导出失败')
    }
  }

  function waitForIframeLoad(iframe) {
    return new Promise((resolve) => {
      let settled = false
      const done = () => {
        if (settled) return
        settled = true
        resolve()
      }
      iframe.addEventListener('load', done, { once: true })
      setTimeout(done, 1200)
    })
  }

  async function exportPdf() {
    if (!store.doc) return
    const html = fullHtml()
    const iframe = document.createElement('iframe')
    iframe.setAttribute('aria-hidden', 'true')
    iframe.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0;opacity:0;pointer-events:none'
    document.body.appendChild(iframe)

    let cleaned = false
    const cleanup = () => {
      if (cleaned) return
      cleaned = true
      iframe.remove()
      logger.debug('[Resume] print iframe removed')
    }

    try {
      message.info('正在打开打印对话框，选择「另存为 PDF」')
      iframe.srcdoc = html
      await waitForIframeLoad(iframe)
      await new Promise((resolve) => setTimeout(resolve, 120))

      const win = iframe.contentWindow
      if (!win) throw new Error('打印窗口不可用')
      win.addEventListener('afterprint', cleanup, { once: true })
      win.focus()
      win.print()
      setTimeout(cleanup, 60_000)
      logger.info('[Resume] print invoked', { bytes: html.length })
    } catch (error) {
      cleanup()
      logger.error('[Resume] pdf export failed', error)
      message.error('PDF 导出失败')
    }
  }

  async function exportBundle() {
    if (!store.doc) return
    try {
      const path = await save({
        title: '导出简历项目文件',
        defaultPath: `${fileName.value}.guaxiresume.json`,
        filters: [{ name: 'Guaxi 简历', extensions: ['json'] }],
      })
      if (!path) return
      const bundle = store.serializeBundle({ includeVersions: true })
      await writeFile(String(path), JSON.stringify(bundle, null, 2))
      logger.info('[Resume] bundle exported', { path, versions: bundle?.versions?.length || 0 })
      message.success('简历项目文件已导出')
    } catch (error) {
      logger.error('[Resume] bundle export failed', error)
      message.error('导出项目文件失败')
    }
  }

  async function importBundleFile() {
    try {
      const path = await open({
        title: '导入简历项目文件',
        multiple: false,
        filters: [{ name: 'Guaxi 简历', extensions: ['json'] }],
      })
      if (!path) return
      const raw = await invoke('read_file_content', { path: String(path) })
      const bundle = JSON.parse(String(raw))
      await store.importBundle(bundle)
      logger.info('[Resume] bundle imported', { path })
    } catch (error) {
      logger.error('[Resume] bundle import failed', error)
      message.error('导入失败：文件格式不正确')
    }
  }

  function previewHtml() {
    return stripScriptsForPreview(fullHtml())
  }

  return {
    fileName,
    fullHtml,
    previewHtml,
    exportHtml,
    exportPdf,
    exportBundle,
    importBundleFile,
  }
}
