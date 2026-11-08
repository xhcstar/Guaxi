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
            <h2><TravelIcon name="users" :size="18" /> 个人偏好</h2>
            <p>家庭地址、车辆与默认选择只保存在本机；生成行程后会学习你的选择，用于下次预填。</p>
          </div>
          <span class="tr-source-tag" :class="prefs.autoLearn ? 'is-amap' : 'is-osm'">
            {{ prefs.autoLearn ? '自动学习' : '手动' }}
          </span>
        </div>

        <div class="tr-pref-grid">
          <div class="tr-pref-block">
            <h3><TravelIcon name="pin" :size="15" /> 家庭地址</h3>
            <div class="tr-place-field" :class="{ 'is-open': homeSuggestions.length }">
              <TravelIcon name="pin" :size="16" />
              <input
                v-model="homeQuery"
                type="text"
                placeholder="小区、商圈或车站即可，不必写门牌号"
                autocomplete="off"
                @input="onHomeInput"
                @focus="onHomeFocus"
              />
              <button type="button" class="tr-locate-btn" @click="locateHome">
                <TravelIcon name="navigation" :size="14" /> 定位
              </button>
              <div v-if="homeSuggestions.length" class="tr-suggest">
                <button
                  v-for="place in homeSuggestions"
                  :key="place.id || place.name"
                  type="button"
                  @mousedown.prevent="chooseHome(place)"
                >
                  <TravelIcon name="pin" :size="14" />
                  <span><strong>{{ place.name }}</strong><small>{{ place.address || place.city }}</small></span>
                </button>
              </div>
            </div>
            <p class="tr-field-note">作为默认出发地；规划时可切换成公司、车站或临时定位。</p>
            <button v-if="prefs.home" type="button" class="tr-link-btn" @click="clearHome">清除家庭地址</button>
          </div>

          <div class="tr-pref-block">
            <h3><TravelIcon name="car" :size="15" /> 车辆</h3>
            <div class="tr-setting-row">
              <div>
                <strong>是否有车</strong>
                <p>关闭后默认出行方式会优先改为公共交通。</p>
              </div>
              <button type="button" class="tr-switch" :class="{ on: prefs.hasCar }" @click="prefs.hasCar = !prefs.hasCar">
                <span></span>
              </button>
            </div>
            <div v-if="prefs.hasCar" class="tr-grid-2">
              <label class="tr-field">
                <span>品牌</span>
                <input v-model="prefs.car.brand" type="text" placeholder="例如：大众" />
              </label>
              <label class="tr-field">
                <span>车型</span>
                <input v-model="prefs.car.model" type="text" placeholder="例如：途观 L" />
              </label>
              <label class="tr-field">
                <span>能源</span>
                <select v-model="prefs.car.energy">
                  <option value="gas">汽油</option>
                  <option value="ev">纯电</option>
                  <option value="hybrid">混动</option>
                </select>
              </label>
            </div>
          </div>

          <div class="tr-pref-block is-wide">
            <h3><TravelIcon name="compass" :size="15" /> 默认出行偏好</h3>

            <div class="tr-pref-label">
              <span>出游方式</span>
              <button type="button" class="tr-follow-btn" :class="{ active: !prefs.defaultTransport }" @click="prefs.defaultTransport = ''">
                跟随学习（{{ transportLabel(effective.transport) }}）
              </button>
            </div>
            <div class="tr-segmented is-four">
              <button
                v-for="option in transportOptions"
                :key="option.value"
                type="button"
                :class="{ active: prefs.defaultTransport === option.value }"
                @click="prefs.defaultTransport = option.value"
              >
                <TravelIcon :name="transportIcon(option.value)" :size="15" />
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>

            <div class="tr-pref-label">
              <span>花费档位</span>
              <button type="button" class="tr-follow-btn" :class="{ active: !prefs.defaultBudgetLevel }" @click="prefs.defaultBudgetLevel = ''">
                跟随学习（{{ budgetLabel(effective.budgetLevel) }}）
              </button>
            </div>
            <div class="tr-segmented is-three">
              <button
                v-for="option in budgetLevels"
                :key="option.value"
                type="button"
                :class="{ active: prefs.defaultBudgetLevel === option.value }"
                @click="prefs.defaultBudgetLevel = option.value"
              >
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>

            <div class="tr-pref-label">
              <span>行程节奏</span>
              <button type="button" class="tr-follow-btn" :class="{ active: !prefs.defaultPace }" @click="prefs.defaultPace = ''">
                跟随学习（{{ paceLabel(effective.pace) }}）
              </button>
            </div>
            <div class="tr-segmented is-three">
              <button
                v-for="option in paceOptions"
                :key="option.value"
                type="button"
                :class="{ active: prefs.defaultPace === option.value }"
                @click="prefs.defaultPace = option.value"
              >
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>

            <div class="tr-pref-label"><span>默认人数</span></div>
            <div class="tr-pref-steppers">
              <div v-for="item in travelerFields" :key="item.key" class="tr-stepper">
                <div><strong>{{ item.label }}</strong><small>{{ item.hint }}</small></div>
                <div class="tr-stepper-controls">
                  <button type="button" :disabled="prefs[item.key] <= item.min" @click="stepPref(item.key, -1, item.min)">−</button>
                  <span>{{ prefs[item.key] }}</span>
                  <button type="button" @click="stepPref(item.key, 1, item.min)">＋</button>
                </div>
              </div>
            </div>

            <div class="tr-pref-label">
              <span>兴趣偏好</span>
              <button type="button" class="tr-follow-btn" :class="{ active: !prefs.interests.length }" @click="prefs.interests = []">
                跟随学习{{ effective.interests.length ? `（${effective.interests.slice(0, 3).join('、')}）` : '' }}
              </button>
            </div>
            <div class="tr-pref-chips">
              <button
                v-for="interest in interestOptions"
                :key="interest"
                type="button"
                class="tr-chip"
                :class="{ active: prefs.interests.includes(interest) }"
                @click="togglePrefInterest(interest)"
              >
                {{ interest }}
              </button>
            </div>

            <div class="tr-pref-label"><span>周末周边半径</span></div>
            <div class="tr-segmented is-three">
              <button
                v-for="option in radiusOptions"
                :key="option.value"
                type="button"
                :class="{ active: prefs.weekendRadiusKm === option.value }"
                @click="prefs.weekendRadiusKm = option.value"
              >
                <strong>{{ option.label }}</strong>
                <small>{{ option.hint }}</small>
              </button>
            </div>

            <label class="tr-field" style="margin-top: 12px">
              <span>默认避开（逗号分隔）</span>
              <input v-model="prefs.avoid" type="text" placeholder="例如：爬山、人太多的网红店" />
            </label>
          </div>

          <div class="tr-pref-block is-wide">
            <h3><TravelIcon name="sparkles" :size="15" /> 偏好学习与隐私</h3>
            <div class="tr-setting-row">
              <div>
                <strong>根据每次生成自动学习</strong>
                <p>只在本机累计交通、预算、节奏、兴趣与目的地偏好，不同步、不上传。</p>
              </div>
              <button type="button" class="tr-switch" :class="{ on: prefs.autoLearn }" @click="prefs.autoLearn = !prefs.autoLearn">
                <span></span>
              </button>
            </div>
            <div class="tr-learned">
              <template v-if="learned.trips">
                <span>已学习 <strong>{{ learned.trips }}</strong> 次出行</span>
                <span>常选 <strong>{{ learned.transport }}</strong> · <strong>{{ learned.budget }}</strong> · <strong>{{ learned.pace }}</strong></span>
                <span v-if="learned.interests.length">兴趣 <strong>{{ learned.interests.join('、') }}</strong></span>
              </template>
              <span v-else>还没有学习记录，当前使用默认值；生成一次行程后会自动累计。</span>
            </div>
            <div class="tr-pref-warning">
              家庭地址与车辆信息只写入本机 <code>repos/travel/preferences.json</code>。分享长图、PDF 与手机页面不会包含这份偏好文件；
              但计划里的出发地会出现在分享内容中，建议使用小区/商圈级地址而不是门牌号。
            </div>
            <div class="tr-pref-actions">
              <button type="button" class="tr-btn is-ghost is-small" @click="resetPrefs">清空偏好与学习</button>
              <button type="button" class="tr-btn is-primary is-small" @click="savePrefs">保存个人偏好</button>
            </div>
          </div>
        </div>
      </section>

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

      <section class="tr-settings-card is-wide">
        <div class="tr-settings-card-head">
          <div>
            <h2><TravelIcon name="map" :size="18" /> 地图服务商</h2>
            <p>国内可选高德/百度/腾讯，境外可选 Google/Mapbox/OpenStreetMap。新生成的计划会使用这里的选择。</p>
          </div>
          <span class="tr-source-tag">{{ mapProviderLabel(effectiveMapProvider) }}</span>
        </div>
        <div class="tr-pref-chips">
          <button
            v-for="provider in mapProviders"
            :key="provider.value"
            type="button"
            class="tr-chip"
            :class="{ active: prefs.mapProvider === provider.value }"
            @click="setMapProvider(provider.value)"
          >
            {{ provider.label }}
          </button>
        </div>
        <div class="tr-map-key-grid">
          <div v-for="provider in mapKeyProviders" :key="provider.value" class="tr-map-key-item">
            <div class="tr-pref-label">
              <span>{{ provider.label }} <small>{{ provider.hint }}</small></span>
              <small>{{ mapKeys[provider.value]?.set ? `已配置 …${mapKeys[provider.value].tail}` : '未配置' }}</small>
            </div>
            <div class="tr-key-row">
              <div class="tr-key-input">
                <TravelIcon name="lock" :size="14" />
                <input
                  v-model="mapKeyInputs[provider.value]"
                  type="password"
                  autocomplete="new-password"
                  :placeholder="`粘贴 ${provider.label} Key`"
                />
              </div>
              <button type="button" class="tr-btn is-outline is-small" @click="saveOtherMapKey(provider.value)">保存</button>
            </div>
          </div>
        </div>
        <p class="tr-settings-note">
          高德 Key 在上方单独管理；其它 Key 与高德 Key 一样在本机加密保存，不会写入分享内容。
          百度使用 BD-09 坐标，显示时自动转换为通用地图瓦片；公交路线在部分服务商下会退化为驾车参考。
        </p>
      </section>

      <section class="tr-settings-card is-wide">
        <div class="tr-settings-card-head">
          <div>
            <h2><TravelIcon name="receipt" :size="18" /> 缓存与今日额度</h2>
            <p>同一城市与分类的检索会缓存 7 天，天气缓存 30 分钟，路线缓存 2 小时，避免反复消耗接口额度。</p>
          </div>
          <button type="button" class="tr-btn is-ghost is-small" @click="clearMapCache">清空地点缓存</button>
        </div>
        <div v-if="quota" class="tr-quota-grid">
          <div v-for="row in quotaRows" :key="`${row.provider}-${row.kind}`" class="tr-quota-row">
            <div>
              <strong>{{ row.provider }} · {{ row.kind }}</strong>
              <small>{{ row.limit ? `个人认证参考上限 ${row.limit}/日` : '免费或自建配额' }}</small>
            </div>
            <span class="tr-quota-count">{{ row.count }}{{ row.limit ? `/${row.limit}` : '' }}</span>
            <div class="tr-quota-bar">
              <i :style="{ width: row.limit ? `${Math.min(100, Math.round((row.count / row.limit) * 100))}%` : '8%' }"></i>
            </div>
          </div>
          <div v-if="!quotaRows.length" class="tr-prep-empty">今日还没有发出地图请求。</div>
          <div class="tr-quota-cache">
            <TravelIcon name="book" :size="13" />
            本地缓存 {{ quota.cacheCount || 0 }} 条 · {{ quota.day }}
            <span v-if="quota.preview">（浏览器预览数据）</span>
          </div>
        </div>
        <div v-else class="tr-prep-empty">暂时没有额度记录；完成一次地点检索后会在这里显示。</div>
        <p class="tr-settings-note">
          高德个人认证的关键字/周边搜索约 100 次/天，路线与地理编码约 5000 次/天。
          清空缓存后会重新请求服务并消耗当日额度；日常规划建议保留缓存。路况只缓存 2 小时，出发前重新生成可拿到较新的拥堵信息。
        </p>
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
            <p>{{ mapProviderLabel(effectiveMapProvider) }} · {{ effectiveMapProvider === 'auto' ? '有高德 Key 时优先高德' : '按所选服务商检索与规划' }}</p>
          </div>
          <span class="tr-source-tag" :class="effectiveMapProvider === 'osm' ? 'is-osm' : 'is-amap'">
            {{ effectiveMapProvider.toUpperCase() }}
          </span>
        </div>
        <p class="tr-settings-note">
          OSM 模式不会上传 Key，也不会产生费用；但在国内，景点名称、营业时间和公交路线可能不如高德完整。
        </p>
      </section>

      <section class="tr-settings-card">
        <h2><TravelIcon name="sparkles" :size="18" /> 模型</h2>
        <div class="tr-segmented is-three" style="margin-bottom: 11px">
          <button type="button" :class="{ active: prefs.aiSource === 'api' }" @click="setAiSource('api')">
            <strong>API 供应商</strong>
            <small>使用设置页已配置的 Key</small>
          </button>
          <button type="button" :class="{ active: prefs.aiSource === 'web' }" @click="setAiSource('web')">
            <strong>网页模型</strong>
            <small>登录一次，免 API Key</small>
          </button>
          <button type="button" :class="{ active: prefs.aiSource === 'local' }" @click="setAiSource('local')">
            <strong>本地模型</strong>
            <small>离线，零 API 成本</small>
          </button>
        </div>

        <template v-if="prefs.aiSource === 'api'">
          <div class="tr-setting-row">
            <div>
              <strong>当前 API 供应商</strong>
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
        </template>

        <template v-else-if="prefs.aiSource === 'web'">
          <div class="tr-pref-label">
            <span>网页模型站点</span>
            <button type="button" class="tr-btn is-outline is-small" @click="openWebLogin">
              <TravelIcon name="external" :size="12" /> 打开登录窗口
            </button>
          </div>
          <div class="tr-pref-chips">
            <button
              v-for="provider in webAiProviders"
              :key="provider.value"
              type="button"
              class="tr-chip"
              :class="{ active: prefs.webProvider === provider.value }"
              @click="setWebProvider(provider.value)"
            >
              {{ provider.label }}
            </button>
          </div>
          <p class="tr-settings-note">
            首次使用会在独立窗口打开站点，登录一次后会复用网页会话；只有会话失效时才需要重新登录。
            网页模型不消耗 API 额度，但站点改版可能导致自动发送/读取失效，失败时可切回 API 供应商。
          </p>
        </template>

        <template v-else>
          <div class="tr-pref-label"><span>本地模型</span></div>
          <div class="tr-pref-chips">
            <button
              type="button"
              class="tr-chip"
              :class="{ active: prefs.localModel === 'deepseek' }"
              @click="setLocalModel('deepseek')"
            >
              DeepSeek R1
            </button>
            <button
              type="button"
              class="tr-chip"
              :class="{ active: prefs.localModel === 'qwen' }"
              @click="setLocalModel('qwen')"
            >
              Qwen
            </button>
          </div>
          <p class="tr-settings-note">
            本地模型完全离线、零 API 成本，适合批量生成周末计划；首次加载模型较慢。
            需要先在「设置 → 大模型」里下载并启用对应本地模型，否则会返回启动失败提示。
          </p>
        </template>
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
import { computed, onMounted, reactive, ref, watch } from 'vue'
import TravelIcon from './TravelIcon.vue'
import { api } from './travelApi.js'
import {
  BUDGET_LEVELS,
  INTEREST_OPTIONS,
  PACE_OPTIONS,
  TRANSPORT_OPTIONS,
} from './travelPlan.js'
import {
  effectiveDefaults,
  learnedSummary,
  MAP_PROVIDERS,
  mapProviderLabel,
  mergePreferences,
  WEB_AI_PROVIDERS,
} from './travelPreferences.js'

const props = defineProps({
  settings: { type: Object, default: () => ({}) },
  preferOsm: Boolean,
  isMock: Boolean,
  preferences: { type: Object, default: () => ({}) },
})
const emit = defineEmits(['save-key', 'set-prefer-osm', 'save-preferences', 'reset-preferences', 'home'])

const keyInput = ref('')
const testing = ref(false)
const notice = ref('')
const noticeType = ref('info')
const modelReady = computed(
  () =>
    Boolean(props.settings.providerLabel) &&
    !/未配置|预览/.test(String(props.settings.providerLabel)),
)

const prefs = reactive(mergePreferences(props.preferences))
const effective = computed(() => effectiveDefaults(prefs))
const learned = computed(() => learnedSummary(prefs))
const transportOptions = TRANSPORT_OPTIONS
const budgetLevels = BUDGET_LEVELS
const paceOptions = PACE_OPTIONS
const interestOptions = INTEREST_OPTIONS
const webAiProviders = WEB_AI_PROVIDERS
const mapProviders = MAP_PROVIDERS
const mapKeyProviders = [
  { value: 'baidu', label: '百度地图', hint: 'Web 服务 AK' },
  { value: 'tencent', label: '腾讯地图', hint: 'WebService Key' },
  { value: 'google', label: 'Google Maps', hint: 'Places / Directions API Key' },
  { value: 'mapbox', label: 'Mapbox', hint: 'Access Token' },
]
const mapKeys = ref({})
const mapKeyInputs = reactive({ baidu: '', tencent: '', google: '', mapbox: '' })
const quota = ref(null)
const effectiveMapProvider = computed(() => (props.preferOsm ? 'osm' : prefs.mapProvider || 'auto'))

const quotaRows = computed(() => {
  const data = quota.value
  if (!data) return []
  const providers = data.providers || {}
  const limits = data.limits || {}
  const labels = {
    amap: '高德',
    baidu: '百度',
    tencent: '腾讯',
    google: 'Google',
    mapbox: 'Mapbox',
    osm: 'OSM',
    'open-meteo': 'Open-Meteo',
  }
  const kinds = {
    poi: '地点检索',
    route: '路线规划',
    geocode: '地理编码',
    weather: '天气',
    static: '静态地图',
    other: '其它',
  }
  const rows = []
  for (const [provider, counts] of Object.entries(providers)) {
    for (const [kind, count] of Object.entries(counts || {})) {
      const limit = limits?.[provider]?.[kind]
      rows.push({
        provider: labels[provider] || provider,
        kind: kinds[kind] || kind,
        count: Number(count) || 0,
        limit: Number(limit) || 0,
      })
    }
  }
  return rows.sort((a, b) => b.count - a.count)
})

onMounted(async () => {
  try {
    mapKeys.value = (await api.getMapKeys()) || {}
  } catch {
    mapKeys.value = {}
  }
  try {
    quota.value = await api.getQuotaUsage()
  } catch {
    quota.value = null
  }
})

async function clearMapCache() {
  if (!window.confirm('清空本地地点/路线缓存？之后会重新请求地图服务，可能消耗当日额度。')) return
  try {
    await api.clearCache()
    quota.value = await api.getQuotaUsage()
    notice.value = '地图缓存已清空。'
    noticeType.value = 'success'
  } catch (error) {
    notice.value = `清空缓存失败：${error?.message || error}`
    noticeType.value = 'error'
  }
}

function setMapProvider(value) {
  prefs.mapProvider = value
  persistPreferences()
}

async function saveOtherMapKey(provider) {
  try {
    mapKeys.value = await api.setMapKey(provider, mapKeyInputs[provider] || '')
    mapKeyInputs[provider] = ''
    notice.value = `${mapProviderLabel(provider)} Key 已保存。`
    noticeType.value = 'success'
  } catch (error) {
    notice.value = `保存 Key 失败：${error?.message || error}`
    noticeType.value = 'error'
  }
}
const travelerFields = [
  { key: 'adults', label: '成人', hint: '至少 1 人', min: 1 },
  { key: 'children', label: '儿童', hint: '门票按 0.6 计', min: 0 },
  { key: 'elders', label: '老人', hint: '减少爬坡', min: 0 },
]
const radiusOptions = [
  { value: 30, label: '30 km', hint: '近郊' },
  { value: 60, label: '60 km', hint: '周边区县' },
  { value: 100, label: '100 km', hint: '邻市山野' },
]

const homeQuery = ref(prefs.home?.name || '')
const homeSuggestions = ref([])
let homeTimer = null

watch(
  () => props.preferences,
  (value) => {
    Object.assign(prefs, mergePreferences(value))
    homeQuery.value = prefs.home?.name || ''
  },
  { deep: true },
)

function transportLabel(value) {
  return TRANSPORT_OPTIONS.find((item) => item.value === value)?.label || value
}
function budgetLabel(value) {
  return BUDGET_LEVELS.find((item) => item.value === value)?.label || value
}
function paceLabel(value) {
  return PACE_OPTIONS.find((item) => item.value === value)?.label || value
}
function transportIcon(value) {
  return value === 'transit' ? 'train' : value === 'walking' ? 'walk' : value === 'tour' ? 'users' : 'car'
}
function stepPref(key, delta, min) {
  prefs[key] = Math.max(min, (Number(prefs[key]) || 0) + delta)
}
function togglePrefInterest(interest) {
  const list = prefs.interests || (prefs.interests = [])
  const index = list.indexOf(interest)
  if (index >= 0) list.splice(index, 1)
  else list.push(interest)
}
function onHomeInput() {
  prefs.home = {
    ...(prefs.home || {}),
    name: homeQuery.value,
    lng: 0,
    lat: 0,
    city: '',
  }
  if (homeTimer) clearTimeout(homeTimer)
  homeTimer = setTimeout(async () => {
    const keyword = homeQuery.value.trim()
    if (!keyword) {
      homeSuggestions.value = []
      return
    }
    try {
      homeSuggestions.value = (await api.searchPlaces(keyword)).slice(0, 6)
    } catch {
      homeSuggestions.value = []
    }
  }, 260)
}
function onHomeFocus() {
  if (homeQuery.value.trim() && !homeSuggestions.value.length) onHomeInput()
  if (homeQuery.value.trim() && !homeSuggestions.value.length) {
    api.searchPlaces(homeQuery.value).then((items) => {
      homeSuggestions.value = (items || []).slice(0, 6)
    }).catch(() => {})
  }
}
function chooseHome(place) {
  prefs.home = { ...place }
  homeQuery.value = place.name
  homeSuggestions.value = []
}
async function locateHome() {
  try {
    const place = await api.ipLocation()
    prefs.home = { ...place }
    homeQuery.value = place.name
    homeSuggestions.value = []
    notice.value = `已定位到 ${place.city || place.name}，可再选择更精确的小区/商圈。`
    noticeType.value = 'success'
  } catch (error) {
    notice.value = `定位失败：${error?.message || error}`
    noticeType.value = 'error'
  }
}
function clearHome() {
  prefs.home = null
  homeQuery.value = ''
  homeSuggestions.value = []
}

function persistPreferences() {
  emit('save-preferences', JSON.parse(JSON.stringify(prefs)))
}

function setAiSource(value) {
  prefs.aiSource = value
  persistPreferences()
}

function setWebProvider(value) {
  prefs.webProvider = value
  prefs.aiSource = 'web'
  persistPreferences()
}

function setLocalModel(value) {
  prefs.localModel = value
  prefs.aiSource = 'local'
  persistPreferences()
}

async function openWebLogin() {
  try {
    const result = await api.openWebModelWindow(prefs.webProvider)
    if (result?.preview) {
      notice.value = '浏览器预览无法打开网页模型登录窗口，请在桌面端使用。'
      noticeType.value = 'info'
      return
    }
    notice.value = '已打开登录窗口。登录完成后关闭窗口即可，后续生成会自动复用会话。'
    noticeType.value = 'success'
  } catch (error) {
    notice.value = `打开登录窗口失败：${error?.message || error}`
    noticeType.value = 'error'
  }
}
function savePrefs() {
  emit('save-preferences', JSON.parse(JSON.stringify(prefs)))
  notice.value = '个人偏好已保存，下次规划会优先使用。'
  noticeType.value = 'success'
}
function resetPrefs() {
  if (!window.confirm('清空个人偏好与学习记录？家庭地址、车辆和默认选择都会移除。')) return
  emit('reset-preferences')
  Object.assign(prefs, mergePreferences({}))
  homeQuery.value = ''
  homeSuggestions.value = []
  notice.value = '已清空个人偏好与学习记录。'
  noticeType.value = 'success'
}

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
