<template>
  <div class="tr-settings">
    <div class="tr-settings-head">
      <div>
        <div class="tr-eyebrow"><span></span> SETTINGS</div>
        <h1>行迹设置</h1>
        <p>数据源、模型与本地存储。没有高德 Key 也能用，只是周边检索和公交路线会弱一些。</p>
      </div>
      <button type="button" class="tr-btn is-ghost is-small" @click="$emit('home')">
        <TravelIcon name="chevronLeft" :size="14" /> 返回计划库
      </button>
    </div>

    <div class="tr-settings-grid">
      <section class="tr-settings-card is-wide">
        <div class="tr-settings-card-head">
          <div>
            <h2><TravelIcon name="map" :size="18" /> 高德 Web 服务 Key</h2>
            <p>用于 POI 检索、逐段路线、路况与天气。Key 只保存在本机配置文件，不会进入前端页面或分享内容。</p>
          </div>
          <span class="tr-source-tag" :class="settings.amapKeySet ? 'is-amap' : 'is-osm'">
            {{ settings.amapKeySet ? '已连接' : '未连接' }}
          </span>
        </div>
        <div class="tr-key-row">
          <div class="tr-key-input">
            <TravelIcon name="lock" :size="15" />
            <input
              v-model="keyInput"
              type="password"
              show-password
              autocomplete="new-password"
              placeholder="粘贴高德 Web 服务 Key（不是 JS API Key）"
              @keyup.enter="save"
            />
            <span v-if="settings.amapKeyTail" class="tr-key-tail">…{{ settings.amapKeyTail }}</span>
          </div>
          <button type="button" class="tr-btn is-primary is-small" @click="save">保存</button>
          <button type="button" class="tr-btn is-outline is-small" :disabled="testing" @click="test">
            {{ testing ? '测试中…' : '测试连接' }}
          </button>
        </div>
        <div class="tr-settings-status">
          <span v-if="settings.ownKeySet">正在使用旅行专属 Key</span>
          <span v-else-if="settings.albumKeySet">旅行 Key 未填写，已自动复用相册的高德 Key</span>
          <span v-else>未填写 Key，当前使用 OpenStreetMap / OSRM / Open-Meteo 免费数据</span>
        </div>
        <details class="tr-key-help">
          <summary>怎么申请？个人认证后免费额度够用吗？</summary>
          <ol>
            <li>打开 <button type="button" class="tr-link-btn" @click="openAmap">高德开放平台控制台</button>，注册并完成个人实名认证。</li>
            <li>「应用管理 → 我的应用 → 创建新应用」，类型随意；在该应用下「添加 Key」。</li>
            <li>服务平台必须选 <strong>Web 服务</strong>（不是 Web 端 JS API），提交后复制 Key。</li>
            <li>个人认证的免费额度大致为：路径规划 / 地理编码 5000 次/日、15 万次/月；POI 搜索约 100 次/日、5000 次/月；天气 5000 次/月。</li>
            <li>行迹会把 POI 检索按「城市 + 分类」缓存 7 天，正常个人使用不会轻易用完搜索额度。</li>
          </ol>
        </details>
      </section>

      <section class="tr-settings-card">
        <h2><TravelIcon name="globe" :size="18" /> 数据源</h2>
        <div class="tr-setting-row">
          <div>
            <strong>仅使用免费数据源</strong>
            <p>强制使用 OpenStreetMap / OSRM / Open-Meteo，不调用高德。适合没有 Key 或想完全离线免费使用的场景。</p>
          </div>
          <button type="button" class="tr-switch" :class="{ on: preferOsm }" @click="$emit('set-prefer-osm', !preferOsm)">
            <span></span>
          </button>
        </div>
        <div class="tr-setting-row">
          <div>
            <strong>当前地图数据</strong>
            <p>{{ settings.source === 'amap' ? '高德：中文 POI、公交换乘、实时路况' : 'OpenStreetMap：免费，公交与营业信息较弱' }}</p>
          </div>
          <span class="tr-source-tag" :class="settings.source === 'amap' ? 'is-amap' : 'is-osm'">
            {{ settings.source === 'amap' ? 'AMAP' : 'OSM' }}
          </span>
        </div>
        <p class="tr-settings-note">
          OSM 模式不会上传 Key，也不会产生费用；但在国内，景点名称、营业时间和公交路线可能不如高德完整。
        </p>
      </section>

      <section class="tr-settings-card">
        <h2><TravelIcon name="sparkles" :size="18" /> 模型</h2>
        <div class="tr-setting-row">
          <div>
            <strong>当前模型供应商</strong>
            <p>{{ settings.providerLabel || '未配置' }}</p>
          </div>
          <span class="tr-source-tag" :class="modelReady ? 'is-amap' : 'is-osm'">
            {{ settings.preview ? '演示' : modelReady ? '可用' : '未配置' }}
          </span>
        </div>
        <p class="tr-settings-note">
          行迹复用「设置 → 大模型」里的供应商与 Key。模型只负责排序和文案；地点、路线、预算由程序校验和计算。
          <template v-if="settings.preview"> 浏览器预览不会调用模型，会使用内置示例编排。</template>
        </p>
      </section>

      <section class="tr-settings-card is-wide">
        <h2><TravelIcon name="receipt" :size="18" /> 本地数据</h2>
        <div class="tr-data-grid">
          <div><small>计划数量</small><strong>{{ settings.planCount || 0 }}</strong></div>
          <div><small>数据目录</small><span class="tr-data-path">{{ settings.dataRoot || '未初始化' }}</span></div>
          <div><small>计划格式</small><strong>JSON · schema v1</strong></div>
          <div><small>分享</small><strong>局域网只读 · 2 小时自动停止</strong></div>
        </div>
        <p class="tr-settings-note">
          计划只保存在本机，不会自动上传。删除计划会同时移除本地 JSON 和备份；长图与 PDF 需要你主动导出。
        </p>
      </section>

      <section class="tr-settings-card is-wide is-privacy">
        <div class="tr-privacy-icon"><TravelIcon name="info" :size="20" /></div>
        <div>
          <strong>关于隐私与准确性</strong>
          <p>
            定位只用 IP 粗定位到城市，随后由你确认详细出发地，不会持续追踪。路线、营业时间、票价与天气都来自第三方服务，
            出发前请再用高德/官方渠道复核；AI 生成的建议不构成预订承诺。
          </p>
        </div>
      </section>
    </div>

    <div v-if="notice" class="tr-settings-notice" :class="noticeType">{{ notice }}</div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { api } from './travelApi.js'

const props = defineProps({
  settings: { type: Object, default: () => ({}) },
  preferOsm: Boolean,
  isMock: Boolean,
})
const emit = defineEmits(['save-key', 'set-prefer-osm', 'home'])

const keyInput = ref('')
const testing = ref(false)
const notice = ref('')
const noticeType = ref('info')
const modelReady = computed(
  () =>
    Boolean(props.settings.providerLabel) &&
    !/未配置|预览/.test(String(props.settings.providerLabel)),
)

async function save() {
  const key = keyInput.value.trim()
  emit('save-key', key)
  notice.value = key ? '已保存。下一次生成会优先使用高德数据。' : '已清除旅行 Key。若相册里配置了 Key，仍会复用相册的 Key。'
  noticeType.value = 'success'
  keyInput.value = ''
}

async function test() {
  testing.value = true
  try {
    const place = await api.ipLocation()
    notice.value = `连接成功：IP 定位到 ${place.city || place.name}。${api.isMock ? '（预览模式）' : ''}`
    noticeType.value = 'success'
  } catch (error) {
    notice.value = `连接失败：${error?.message || error}`
    noticeType.value = 'error'
  } finally {
    testing.value = false
  }
}

async function openAmap() {
  const url = 'https://console.amap.com/dev/key/app'
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}
</script>
