<template>
  <section class="tr-share">
    <div class="tr-panel-head">
      <div>
        <div class="tr-eyebrow"><span></span> TAKE IT WITH YOU</div>
        <h2>分享与带走</h2>
        <p>长图、PDF、局域网链接三种方式，覆盖“在微信里看”和“到了当地离线看”。</p>
      </div>
      <span class="tr-share-source">{{ plan.source === 'amap' ? '高德数据' : 'OSM 数据' }}</span>
    </div>

    <div class="tr-share-layout">
      <div class="tr-share-actions">
        <div class="tr-share-card is-primary">
          <div class="tr-share-icon"><TravelIcon name="copy" :size="22" /></div>
          <div>
            <strong>复制长图，直接发微信</strong>
            <p>生成 1080px 宽的完整行程长图，复制后到微信聊天框 Ctrl+V 即可。</p>
            <div class="tr-share-btns">
              <button type="button" class="tr-btn is-primary is-small" :disabled="!!busyAction" @click="copyPoster">
                <TravelIcon name="copy" :size="14" /> {{ busyAction === 'copy' ? '生成中…' : '复制长图' }}
              </button>
              <button type="button" class="tr-btn is-outline is-small" :disabled="!!busyAction" @click="savePoster">
                <TravelIcon name="download" :size="14" /> 保存 PNG
              </button>
            </div>
          </div>
        </div>

        <div class="tr-share-card">
          <div class="tr-share-icon"><TravelIcon name="print" :size="22" /></div>
          <div>
            <strong>打印 / 保存为 PDF</strong>
            <p>打开 A4 打印页，选择“另存为 PDF”，再发到微信文件传输助手，手机上可翻页阅读。</p>
            <div class="tr-share-btns">
              <button type="button" class="tr-btn is-outline is-small" @click="printPdf">
                <TravelIcon name="print" :size="14" /> 打开打印页
              </button>
            </div>
          </div>
        </div>

        <div class="tr-share-card" :class="{ 'is-running': shareStatus?.running }">
          <div class="tr-share-icon"><TravelIcon name="qr" :size="22" /></div>
          <div>
            <strong>手机扫码查看</strong>
            <p>启动只读分享服务，手机与电脑在同一 Wi-Fi 下扫码，可看完整时间轴和路线；2 小时后自动停止。</p>
            <div class="tr-share-btns">
              <button v-if="!shareStatus?.running" type="button" class="tr-btn is-primary is-small" :disabled="!!busyAction" @click="startShare">
                <TravelIcon name="qr" :size="14" /> {{ busyAction === 'share' ? '启动中…' : '生成手机链接' }}
              </button>
              <button v-else type="button" class="tr-btn is-outline is-small" @click="stopShare">
                <TravelIcon name="x" :size="14" /> 停止分享
              </button>
              <button v-if="shareStatus?.running" type="button" class="tr-btn is-ghost is-small" @click="copyLink">
                <TravelIcon name="copy" :size="14" /> 复制链接
              </button>
            </div>
          </div>
        </div>

        <div v-if="shareStatus?.running" class="tr-link-box">
          <div class="tr-link-qr">
            <img v-if="qr" :src="qr" alt="手机扫码查看" />
          </div>
          <div class="tr-link-info">
            <strong>手机扫码查看</strong>
            <span class="tr-link-url">{{ shareStatus.url }}</span>
            <small>同一 Wi-Fi 下有效 · 电脑关闭或手动停止后失效 · 只读</small>
            <small v-if="shareStatus.preview" class="is-accent">浏览器预览返回的是演示地址</small>
          </div>
        </div>

        <div v-if="notice" class="tr-share-notice">{{ notice }}</div>
      </div>

      <div class="tr-share-preview">
        <div class="tr-share-preview-head">
          <span>长图预览</span>
          <button type="button" class="tr-link-btn" :disabled="posterBusy" @click="refreshPosterMap">
            <TravelIcon name="refresh" :size="12" /> {{ posterBusy ? '更新地图…' : '更新地图' }}
          </button>
        </div>
        <div ref="frameRef" class="tr-poster-viewport" :style="{ height: `${viewportHeight}px` }">
          <div class="tr-poster-scaler" :style="{ transform: `scale(${scale})` }">
            <PlanPoster ref="posterRef" :plan="posterPlan" :qr="qr" />
          </div>
        </div>
        <p class="tr-share-preview-note">
          长图会包含路线、逐日时间表、预算和行前重点；分享链接是动态页面，适合在手机上查看。
        </p>
      </div>
    </div>

    <div class="tr-share-guide">
      <div><span>01</span><strong>发微信</strong><p>复制长图后直接粘贴到聊天框，或发 PDF 文件；不用对方装任何应用。</p></div>
      <div><span>02</span><strong>在手机上</strong><p>同一 Wi-Fi 扫码看交互版；出门后长图和 PDF 可离线查看。</p></div>
      <div><span>03</span><strong>隐私</strong><p>计划只保存在本机；局域网分享只读、带随机 token，不会上传到第三方。</p></div>
    </div>
  </section>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import QRCode from 'qrcode'
import { toPng } from 'html-to-image'
import TravelIcon from './TravelIcon.vue'
import PlanPoster from './PlanPoster.vue'
import { api } from './travelApi.js'
import { buildShareHtml } from './travelShare.js'

const props = defineProps({
  plan: { type: Object, required: true },
  isMock: Boolean,
})
const emit = defineEmits(['update-plan', 'toast'])

const posterRef = ref(null)
const frameRef = ref(null)
const scale = ref(0.42)
const viewportHeight = ref(620)
const qr = ref('')
const localPosterMap = ref('')
const shareStatus = ref(null)
const busyAction = ref('')
const posterBusy = ref(false)
const notice = ref('')
let resizeObserver = null
let noticeTimer = null

const posterPlan = computed(() => ({
  ...props.plan,
  posterMap: localPosterMap.value || props.plan.posterMap || '',
}))

function notify(text, type = 'info') {
  notice.value = text
  emit('toast', text, type)
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    notice.value = ''
  }, 3600)
}

function updateScale() {
  if (!frameRef.value) return
  const width = frameRef.value.clientWidth - 24
  scale.value = Math.max(0.24, Math.min(1, width / 1080))
  const node = posterRef.value?.getNode?.()
  const height = node?.offsetHeight || 1500
  viewportHeight.value = Math.max(420, Math.round(height * scale.value))
}

function toBlob(dataUrl) {
  const [meta, base64] = String(dataUrl).split(',')
  const mime = meta.match(/data:(.*?);/)?.[1] || 'image/png'
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return new Blob([bytes], { type: mime })
}

function base64Bytes(dataUrl) {
  const base64 = String(dataUrl).split(',')[1] || ''
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

async function capture() {
  const node = posterRef.value?.getNode?.()
  if (!node) throw new Error('长图还没有准备好')
  await nextTick()
  if (document.fonts?.ready) await document.fonts.ready
  return toPng(node, {
    pixelRatio: 1.35,
    backgroundColor: '#f4eee1',
    cacheBust: true,
    skipAutoScale: true,
  })
}

async function copyPoster() {
  if (busyAction.value) return
  busyAction.value = 'copy'
  try {
    const dataUrl = await capture()
    if (api.isTauriRuntime()) {
      const canvas = document.createElement('canvas')
      const image = new Image()
      image.src = dataUrl
      await image.decode()
      canvas.width = image.naturalWidth
      canvas.height = image.naturalHeight
      const context = canvas.getContext('2d')
      context.drawImage(image, 0, 0)
      const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data
      const [{ Image: TauriImage }, { writeImage }] = await Promise.all([
        import('@tauri-apps/api/image'),
        import('@tauri-apps/plugin-clipboard-manager'),
      ])
      const tauriImage = await TauriImage.new(rgba, canvas.width, canvas.height)
      await writeImage(tauriImage)
      tauriImage.close?.()
      notify('长图已复制。到微信聊天框 Ctrl+V 即可。', 'success')
    } else if (navigator.clipboard?.write && window.ClipboardItem) {
      try {
        await navigator.clipboard.write([new ClipboardItem({ 'image/png': toBlob(dataUrl) })])
        notify('长图已复制到剪贴板', 'success')
      } catch {
        // 浏览器可能因为窗口未聚焦/权限拒绝剪贴板，退回下载，不让用户卡住。
        downloadDataUrl(dataUrl)
        notify('浏览器未允许图片剪贴板，已改为下载 PNG', 'info')
      }
    } else {
      downloadDataUrl(dataUrl)
      notify('当前浏览器不支持图片剪贴板，已改为下载 PNG', 'info')
    }
  } catch (error) {
    notify(`生成长图失败：${error?.message || error}`, 'error')
  } finally {
    busyAction.value = ''
  }
}

async function savePoster() {
  if (busyAction.value) return
  busyAction.value = 'save'
  try {
    const dataUrl = await capture()
    if (api.isTauriRuntime()) {
      const [{ save }, { writeFile }] = await Promise.all([
        import('@tauri-apps/plugin-dialog'),
        import('@tauri-apps/plugin-fs'),
      ])
      const path = await save({
        defaultPath: `${safeFileName(props.plan.title)}-${props.plan.startDate || ''}.png`,
        filters: [{ name: 'PNG 图片', extensions: ['png'] }],
      })
      if (path) {
        await writeFile(path, base64Bytes(dataUrl))
        notify('长图已保存', 'success')
      }
    } else {
      downloadDataUrl(dataUrl)
      notify('长图已下载', 'success')
    }
  } catch (error) {
    notify(`保存长图失败：${error?.message || error}`, 'error')
  } finally {
    busyAction.value = ''
  }
}

function downloadDataUrl(dataUrl) {
  const link = document.createElement('a')
  link.href = dataUrl
  link.download = `${safeFileName(props.plan.title)}-${props.plan.startDate || ''}.png`
  document.body.appendChild(link)
  link.click()
  link.remove()
}

function safeFileName(value) {
  return String(value || '行迹行程').replace(/[\\/:*?"<>|]/g, '-').slice(0, 60)
}

async function printPdf() {
  try {
    if (api.isTauriRuntime()) {
      const theme = document.documentElement.dataset.guaxiTheme === 'dark' ? 'dark' : 'light'
      await api.openPrintWindow(props.plan.id, theme)
    } else {
      const popup = window.open('', '_blank')
      if (!popup) {
        notify('浏览器拦截了新窗口，请允许弹窗后重试', 'error')
        return
      }
      popup.document.open()
      popup.document.write(buildShareHtml(props.plan))
      popup.document.close()
      popup.onload = () => popup.print()
    }
  } catch (error) {
    notify(`打开打印页失败：${error?.message || error}`, 'error')
  }
}

async function startShare() {
  if (busyAction.value) return
  busyAction.value = 'share'
  try {
    const html = buildShareHtml(props.plan)
    const status = await api.shareStart(props.plan.id, html)
    shareStatus.value = status
    qr.value = await QRCode.toDataURL(status.url, {
      margin: 1,
      width: 260,
      color: { dark: '#1c2a27', light: '#fffaf0' },
    })
    emit('update-plan', {
      ...props.plan,
      share: { startedAt: status.startedAt, url: status.url },
    })
    notify(status.preview ? '预览模式返回了演示链接' : '手机链接已生成，扫码即可查看', 'success')
  } catch (error) {
    notify(`启动手机链接失败：${error?.message || error}`, 'error')
  } finally {
    busyAction.value = ''
  }
}

async function stopShare() {
  try {
    await api.shareStop()
    shareStatus.value = null
    qr.value = ''
    emit('update-plan', { ...props.plan, share: { startedAt: '', url: '' } })
    notify('已停止分享', 'info')
  } catch (error) {
    notify(`停止分享失败：${error?.message || error}`, 'error')
  }
}

async function copyLink() {
  if (!shareStatus.value?.url) return
  try {
    await navigator.clipboard.writeText(shareStatus.value.url)
    notify('链接已复制', 'success')
  } catch {
    notify(`链接：${shareStatus.value.url}`, 'info')
  }
}

async function refreshPosterMap() {
  if (posterBusy.value) return
  posterBusy.value = true
  try {
    const day = (props.plan.days || []).find((item) =>
      (item.blocks || []).some((block) => block.route?.polyline),
    )
    const block = day?.blocks?.find((item) => item.route?.polyline)
    if (!block) {
      notify('这份行程还没有路线数据，暂时用装饰路线图', 'info')
      return
    }
    const points = String(block.route.polyline)
      .split(';')
      .map((point) => point.split(',').map(Number))
      .filter((pair) => pair.length === 2 && pair.every(Number.isFinite))
    const destination = props.plan.destination
    if (!destination?.lng || !destination?.lat) return
    const dataUrl = await api.staticMap({
      centerLng: Number(destination.lng),
      centerLat: Number(destination.lat),
      zoom: 11,
      width: 900,
      height: 380,
      path: points.slice(0, 120),
    })
    if (dataUrl) {
      localPosterMap.value = dataUrl
      emit('update-plan', { ...props.plan, posterMap: dataUrl })
      await nextTick()
      updateScale()
      notify('已更新长图上的路线地图', 'success')
    } else {
      notify('没有高德 Key，使用装饰路线图', 'info')
    }
  } catch (error) {
    notify(`生成路线地图失败：${error?.message || error}`, 'error')
  } finally {
    posterBusy.value = false
  }
}

watch(
  () => props.plan.id,
  () => {
    localPosterMap.value = ''
    shareStatus.value = null
    qr.value = ''
    notice.value = ''
    nextTick(updateScale)
  },
)

onMounted(async () => {
  await nextTick()
  updateScale()
  resizeObserver = new ResizeObserver(() => updateScale())
  if (frameRef.value) resizeObserver.observe(frameRef.value)
  try {
    const status = await api.shareStatus()
    if (status?.running && status.planId === props.plan.id) {
      shareStatus.value = status
      qr.value = await QRCode.toDataURL(status.url, {
        margin: 1,
        width: 260,
        color: { dark: '#1c2a27', light: '#fffaf0' },
      })
    }
  } catch {
    /* 分享状态读取失败不影响页面 */
  }
  if (!api.isMock && props.plan.source === 'amap' && !props.plan.posterMap) {
    refreshPosterMap()
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  if (noticeTimer) clearTimeout(noticeTimer)
})
</script>
