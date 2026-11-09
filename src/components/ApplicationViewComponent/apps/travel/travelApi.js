// Tauri 命令封装 + 浏览器预览桩。
// 预览桩不是“假界面”：它走同一套事实/路线/装配逻辑，只是不调用高德与模型，
// 所以排版、预算、地图、长图都能在没有桌面运行时的情况下检查。

import { invoke } from '@tauri-apps/api/core'
import { createDemoFacts, createDemoPlan, DEMO_DESTINATION, DEMO_LIBRARY, DEMO_ORIGIN } from './travelData.js'
import { fallbackCities, provinceOf } from './travelRegions.js'
import { addDays, enumerateDates, nextSaturday, seasonFor, uid } from './travelUtils.js'

export const isTauriRuntime = () =>
  typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__)
export const isMock = !isTauriRuntime()

const STORAGE_KEY = 'guaxi.travel.plans.v1'
const SETTINGS_KEY = 'guaxi.travel.settings.v1'
const PREFERENCES_KEY = 'guaxi.travel.preferences.v1'

function readLocalPlans() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function writeLocalPlans(plans) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(plans))
  } catch {
    /* 预览环境 localStorage 满时忽略，界面仍可用 */
  }
}

function readLocalSettings() {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

function writeLocalSettings(settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    /* ignore */
  }
}

function mockSettings() {
  const local = readLocalSettings()
  return {
    amapKeySet: Boolean(local.amapKey),
    amapKeyTail: local.amapKey ? String(local.amapKey).slice(-4) : '',
    ownKeySet: Boolean(local.amapKey),
    albumKeySet: false,
    dataRoot: '浏览器预览 / 本地存储',
    planCount: readLocalPlans().length,
    providerLabel: '浏览器预览（不调用模型）',
    source: local.amapKey ? 'amap' : 'osm',
    preview: true,
  }
}

function mockMapKeys(keys = {}) {
  const local = readLocalSettings()
  const out = {}
  for (const provider of ['amap', 'baidu', 'tencent', 'google', 'mapbox']) {
    const key = String(keys[provider] || (provider === 'amap' ? local.amapKey : '') || '')
    out[provider] = { set: Boolean(key), tail: key ? key.slice(-4) : '' }
  }
  return out
}

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

function haversineKm(a, b) {
  const radius = 6371
  const dLat = ((b.lat - a.lat) * Math.PI) / 180
  const dLng = ((b.lng - a.lng) * Math.PI) / 180
  const lat1 = (a.lat * Math.PI) / 180
  const lat2 = (b.lat * Math.PI) / 180
  const value =
    Math.sin(dLat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLng / 2) ** 2
  return 2 * radius * Math.asin(Math.sqrt(Math.min(1, value)))
}

function mockPlace(name, city, lng, lat, address = '') {
  return {
    id: `mock-${name}`,
    name,
    address: address || `${city}${name}`,
    city,
    adcode: '',
    cityCode: '',
    lng,
    lat,
    source: 'osm',
  }
}

const MOCK_PLACES = [
  mockPlace('徐家汇', '上海', 121.4365, 31.1879, '上海市徐汇区徐家汇'),
  mockPlace('上海虹桥站', '上海', 121.3273, 31.1948, '上海市闵行区申贵路 1500 号'),
  mockPlace('西湖风景名胜区', '杭州', 120.1302, 30.2405, '浙江省杭州市西湖区龙井路 1 号'),
  mockPlace('杭州东站', '杭州', 120.2131, 30.2917, '浙江省杭州市上城区全福桥路 2 号'),
  mockPlace('乌镇西栅', '嘉兴', 120.4878, 30.7444, '浙江省嘉兴市桐乡市乌镇'),
  mockPlace('莫干山风景区', '湖州', 119.8688, 30.5995, '浙江省湖州市德清县莫干山'),
  mockPlace('苏州平江路', '苏州', 120.6298, 31.3195, '江苏省苏州市姑苏区平江路'),
  mockPlace('千岛湖景区', '杭州', 119.0419, 29.6045, '浙江省杭州市淳安县千岛湖镇'),
]

function mockLeg(from, to, index) {
  const distance = Math.max(0.6, haversineKm(from, to) * 1.2)
  const mode = distance > 25 ? 'driving' : index % 3 === 1 ? 'walking' : 'driving'
  const speed = mode === 'walking' ? 4.5 : 38
  const duration = (distance / speed) * 3600
  const summary = `${mode === 'walking' ? '步行' : '驾车'} · ${distance.toFixed(1)} km · 约 ${Math.round(duration / 60)} 分钟`
  const base = {
    mode,
    distance,
    duration,
    tolls: distance > 60 ? Math.round(distance * 0.4) : 0,
    fare: 0,
    traffic: distance > 30 ? '缓行 1 段' : '畅通',
    summary,
    steps: [
      {
        instruction: `从 ${from.name} 出发，向 ${to.name} 方向行驶`,
        road: '',
        distance: distance * 1000 * 0.2,
        duration: duration * 0.2,
      },
      {
        instruction: `沿主路继续行驶，按导航前往 ${to.name}`,
        road: '城市主干道',
        distance: distance * 1000 * 0.6,
        duration: duration * 0.6,
      },
      {
        instruction: `到达 ${to.name} 附近，按指引停车或步行进入`,
        road: '',
        distance: distance * 1000 * 0.2,
        duration: duration * 0.2,
      },
    ],
    polyline: `${from.lng},${from.lat};${(from.lng + to.lng) / 2},${(from.lat + to.lat) / 2};${to.lng},${to.lat}`,
    warning: '',
    fromLng: from.lng,
    fromLat: from.lat,
    toLng: to.lng,
    toLat: to.lat,
    fromName: from.name,
    toName: to.name,
  }
  base.alternatives = [
    { label: '推荐', summary, distance, duration, tolls: base.tolls, fare: 0, traffic: base.traffic, polyline: base.polyline },
    {
      label: '躲避拥堵',
      summary: `${summary} · 绕行`,
      distance: Number((distance * 1.07).toFixed(1)),
      duration: Math.round(duration * 1.1),
      tolls: base.tolls,
      fare: 0,
      traffic: '缓行 1 段',
      polyline: base.polyline,
    },
    {
      label: '距离优先',
      summary: `${summary} · 更短`,
      distance: Number((distance * 0.95).toFixed(1)),
      duration: Math.round(duration * 1.06),
      tolls: Math.max(0, base.tolls - 10),
      fare: 0,
      traffic: '',
      polyline: base.polyline,
    },
  ]
  return base
}

function mockCollectFacts(args) {
  const request = args?.request || {}
  const dates = enumerateDates(request.startDate, request.endDate)
  const facts = createDemoFacts(dates.length ? dates : undefined)
  const dest = request.destination || {}
  const isHangzhou = `${dest.name || ''}${dest.city || ''}`.includes('杭州') || `${dest.name || ''}`.includes('西湖')
  if (!isHangzhou && dest.lng && dest.lat) {
    facts.destination = {
      ...DEMO_DESTINATION,
      name: dest.name || DEMO_DESTINATION.name,
      address: dest.address || DEMO_DESTINATION.address,
      city: dest.city || DEMO_DESTINATION.city,
      lng: Number(dest.lng),
      lat: Number(dest.lat),
      source: 'osm',
    }
    // 预览只换目的地坐标，POI 仍是杭州示例；界面上会明确标注“示例数据”。
    facts.notes = ['浏览器预览：POI 为杭州示例数据，接入真实后端后会替换为目的地周边检索结果。']
  }
  if (request.origin?.lng && request.origin?.lat) {
    facts.origin = {
      ...DEMO_ORIGIN,
      name: request.origin.name || DEMO_ORIGIN.name,
      address: request.origin.address || DEMO_ORIGIN.address,
      city: request.origin.city || DEMO_ORIGIN.city,
      lng: Number(request.origin.lng),
      lat: Number(request.origin.lat),
      source: 'osm',
    }
  }
  if (request.source === 'osm') {
    facts.source = 'osm'
  }
  facts.weather = facts.weather.map((item, index) => ({
    ...item,
    ...(dates[index] ? { date: dates[index], weekday: '' } : {}),
  }))
  return facts
}

function classifyDemoPoi(poi) {
  if (poi.kind === 'food') return 'food'
  const haystack = `${poi.name || ''} ${poi.category || ''} ${(poi.tags || []).join(' ')}`
  const rules = [
    [/夜市/, 'night'],
    [/市场|集市|菜场/, 'market'],
    [/博物馆|美术馆|展览|纪念馆/, 'museum'],
    [/剧院|剧场|演出|音乐厅/, 'show'],
    [/温泉/, 'hotspring'],
    [/乐园|动物园|海洋|水族|亲子/, 'family'],
    [/寺|庙|道观/, 'temple'],
    [/遗址|古塔|塔|桥|运河|故居|古城/, 'heritage'],
    [/街|商场|商圈|步行/, 'street'],
    [/湿地|溪|湖|山|古道|公园|茶园|村|烟树|岛/, 'nature'],
  ]
  for (const [pattern, key] of rules) {
    if (pattern.test(haystack)) return key
  }
  return 'landmark'
}

function mockExplore(args) {
  const request = args?.request || {}
  const facts = mockCollectFacts({ request })
  const destination = facts.destination
  const province = /省|自治区/.test(String(destination.name || ''))
  const bases = (request.bases || []).filter((item) => item?.name)
  if (province && !bases.length) {
    return {
      scope: 'province',
      source: 'osm',
      coordSystem: 'gcj02',
      origin: facts.origin,
      destination,
      bases: [],
      baseCandidates: fallbackCities(provinceOf(destination.name), 8).map((city, index) => ({
        id: city.adcode || `demo-base-${index}`,
        name: city.name,
        city: city.name,
        adcode: city.adcode || '',
        lng: city.lng,
        lat: city.lat,
        distanceKm: 0,
      })),
      candidates: [],
      hotels: [],
      weather: facts.weather,
      notes: ['浏览器预览：省级目的地先选落脚城市，再按基地扫描候选。'],
      collectedAt: new Date().toISOString(),
      total: 0,
      truncated: false,
    }
  }
  const baseList = bases.length
    ? bases.map((item, index) => ({
        id: item.adcode || `demo-base-${index}`,
        name: item.name,
        city: item.city || item.name,
        adcode: item.adcode || '',
        lng: Number(item.lng) || destination.lng,
        lat: Number(item.lat) || destination.lat,
        distanceKm: 0,
      }))
    : [
        {
          id: destination.adcode || 'demo-base',
          name: destination.name,
          city: destination.city,
          adcode: destination.adcode || '',
          lng: destination.lng,
          lat: destination.lat,
          distanceKm: Math.round(
            haversineKm(facts.origin, destination) * 10,
          ) / 10,
        },
      ]
  const base = baseList[0]
  const candidates = [...(facts.attractions || []), ...(facts.food || [])]
    .map((poi, index) => {
      const categoryKey = classifyDemoPoi(poi)
      const distanceKm = Math.round(haversineKm(base, poi) * 10) / 10
      return {
        ...poi,
        categoryKey,
        categoryLabel: '',
        baseId: base.id,
        baseName: base.name,
        distanceBaseKm: distanceKm,
        distanceOriginKm: Math.round(haversineKm(facts.origin, poi) * 10) / 10,
        must: index === 0 && (request.mustVisit || []).length > 0,
      }
    })
    .filter((poi) => poi.lng && poi.lat)
  return {
    scope: province ? 'province' : /市/.test(String(destination.name || '')) ? 'city' : 'poi',
    source: 'osm',
    coordSystem: 'gcj02',
    origin: facts.origin,
    destination,
    bases: baseList,
    baseCandidates: [],
    candidates,
    hotels: (facts.hotels || []).map((poi) => ({ ...poi })),
    weather: facts.weather,
    notes: [
      '浏览器预览：候选为杭州示例地点，接入桌面端后会替换为高德/OSM 的真实检索结果。',
      '评分、人均与距离为示例值，用于检查筛选与勾选流程。',
    ],
    collectedAt: new Date().toISOString(),
    total: candidates.length,
    truncated: false,
  }
}

function savedPlans() {
  const plans = readLocalPlans()
  if (plans.length) return plans
  return []
}

function seedPreview() {
  if (!isMock) return
  if (savedPlans().length === 0) {
    writeLocalPlans(DEMO_LIBRARY.map((plan) => ({ ...plan })))
  }
}

function hashText(text) {
  let value = 0
  for (const char of String(text || '')) {
    value = (value * 31 + char.charCodeAt(0)) % 100000
  }
  return value
}

function mockRouteWeather({ provider = 'osm', seasonDate = '', points = [] }) {
  const season = seasonFor(seasonDate || points[0]?.date)
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return {
    provider: '浏览器预览 · Open-Meteo 模拟',
    coordSystem: 'wgs84',
    fetchedAt: new Date().toISOString(),
    season: season.label,
    seasonTips: season.tips,
    notes: ['浏览器预览使用本地模拟天气；桌面端会按路线坐标请求 Open-Meteo 实时数据。'],
    points: points.map((point) => {
      const date = point.date || ''
      const time = point.time || '12:00'
      const hour = Number(time.slice(0, 2)) || 12
      const target = date ? new Date(`${date}T12:00:00`) : today
      const daysAhead = Math.round((target.getTime() - today.getTime()) / 86400000)
      const base = {
        name: point.name || '途经',
        lng: Number(point.lng) || 0,
        lat: Number(point.lat) || 0,
        date,
        time,
        source: 'open-meteo',
        fetchedAt: new Date().toISOString(),
      }
      if (!date || daysAhead > 16) {
        return {
          ...base,
          available: false,
          message: '超过 16 天预报窗口，临近出发再更新',
          weather: '',
          icon: 'cloud',
        }
      }
      const month = Number(date.slice(5, 7)) || new Date().getMonth() + 1
      const seasonal = { 1: 2, 2: 5, 3: 11, 4: 17, 5: 22, 6: 26, 7: 30, 8: 29, 9: 25, 10: 19, 11: 12, 12: 5 }[month] ?? 18
      const hash = hashText(`${date}|${time}|${point.name}|${point.lng}|${point.lat}`)
      const hourAdjust = hour < 8 ? -4 : hour < 11 ? 0 : hour < 16 ? 3 : hour < 19 ? 0 : -4
      const temperature = seasonal + hourAdjust + (hash % 5) - 2
      const code = hash % 10
      const [weather, icon] =
        code <= 3
          ? ['晴', 'sun']
          : code <= 6
            ? ['多云', 'cloud']
            : code <= 8
              ? ['小雨', 'rain']
              : ['有雾', 'fog']
      const elevation = 20 + (hash % 900)
      return {
        ...base,
        weather,
        icon,
        temperature,
        feelsLike: temperature - (hash % 3),
        precipitationProbability:
          icon === 'rain' ? 55 + (hash % 35) : icon === 'fog' ? 20 : icon === 'cloud' ? 15 : 5,
        precipitation: icon === 'rain' ? 1 + (hash % 4) * 0.5 : 0,
        windSpeed: 6 + (hash % 22),
        windDirection: (hash * 37) % 360,
        visibility: icon === 'fog' ? 350 + (hash % 500) : 9000 + (hash % 9000),
        elevation,
        available: true,
        message: '',
        source: 'open-meteo',
      }
    }),
  }
}

export const api = {
  isMock,
  isTauriRuntime,
  seedPreview,

  async getSettings() {
    if (!isTauriRuntime()) return mockSettings()
    return invoke('travel_get_settings')
  },

  async setAmapKey(key) {
    if (!isTauriRuntime()) {
      const local = readLocalSettings()
      writeLocalSettings({ ...local, amapKey: String(key || '').trim() })
      return mockSettings()
    }
    return invoke('travel_set_amap_key', { key })
  },

  async ipLocation() {
    if (!isTauriRuntime()) {
      await delay(240)
      return { ...DEMO_ORIGIN, source: 'osm' }
    }
    return invoke('travel_ip_location')
  },

  async searchPlaces(keyword, city = '') {
    if (!isTauriRuntime()) {
      await delay(180)
      const key = String(keyword || '').trim()
      if (!key) return MOCK_PLACES.slice(0, 6)
      return MOCK_PLACES.filter(
        (item) => item.name.includes(key) || item.city.includes(key) || item.address.includes(key),
      ).slice(0, 8)
    }
    return invoke('travel_search_places', { keyword, city })
  },

  async collectFacts(request) {
    if (!isTauriRuntime()) {
      await delay(520)
      return mockCollectFacts({ request })
    }
    return invoke('travel_collect_facts', { request })
  },

  async explore(request) {
    if (!isTauriRuntime()) {
      await delay(620)
      return mockExplore({ request })
    }
    return invoke('travel_explore', { request })
  },

  async suggestBases(destination) {
    if (!isTauriRuntime()) {
      await delay(160)
      const name = destination?.name || destination?.city || ''
      const province = provinceOf(name)
      return fallbackCities(province, 8).map((city) => ({
        ...city,
        id: `region-${city.adcode}`,
        address: `${province?.name || ''}${city.name}`,
        city: city.name,
        source: 'region',
      }))
    }
    return invoke('travel_suggest_bases', { destination })
  },

  async buildRoutes({ transport, days, provider, alternatives = true }) {
    if (!isTauriRuntime()) {
      await delay(380)
      return days.map((day) => ({
        date: day.date,
        legs: (day.stops || []).slice(1).map((stop, index) => mockLeg(day.stops[index], stop, index)),
      }))
    }
    return invoke('travel_build_routes', {
      request: { transport, days, provider: provider || 'auto', alternatives },
    })
  },

  async routeWeather({ provider, seasonDate, points }) {
    if (!isTauriRuntime()) {
      await delay(220)
      return mockRouteWeather({ provider, seasonDate, points })
    }
    return invoke('travel_route_weather', {
      request: { provider: provider || 'auto', seasonDate: seasonDate || '', points },
    })
  },

  async llmComplete(request) {
    if (!isTauriRuntime()) {
      await delay(300)
      throw new Error('浏览器预览不调用模型')
    }
    return invoke('travel_llm_complete', { request })
  },

  async webLlmComplete(request) {
    if (!isTauriRuntime()) {
      await delay(300)
      throw new Error('浏览器预览不调用网页模型')
    }
    return invoke('travel_llm_web_complete', { request })
  },

  async localLlmComplete(request) {
    if (!isTauriRuntime()) {
      await delay(300)
      throw new Error('浏览器预览不调用本地模型')
    }
    return invoke('travel_llm_local_complete', { request })
  },

  async openWebModelWindow(provider) {
    if (!isTauriRuntime()) {
      await delay(120)
      return { preview: true }
    }
    return invoke('open_web_model_window', { provider })
  },

  async staticMap(request) {
    if (!isTauriRuntime()) return ''
    return invoke('travel_static_map', { request })
  },

  async listPlans() {
    if (!isTauriRuntime()) {
      await delay(80)
      return savedPlans()
    }
    return invoke('travel_list_plans')
  },

  async getPlan(planId) {
    if (!isTauriRuntime()) {
      await delay(60)
      const plan = savedPlans().find((item) => item.id === planId)
      if (!plan) throw new Error('计划不存在')
      return plan
    }
    return invoke('travel_get_plan', { planId })
  },

  async savePlan(plan) {
    if (!isTauriRuntime()) {
      const next = { ...plan, updatedAt: new Date().toISOString() }
      const plans = savedPlans().filter((item) => item.id !== next.id)
      plans.unshift(next)
      writeLocalPlans(plans)
      return next
    }
    return invoke('travel_save_plan', { plan })
  },

  async deletePlan(planId) {
    if (!isTauriRuntime()) {
      writeLocalPlans(savedPlans().filter((item) => item.id !== planId))
      return
    }
    return invoke('travel_delete_plan', { planId })
  },

  async setMapKey(provider, key) {
    if (!isTauriRuntime()) {
      const local = readLocalSettings()
      const keys = { ...(local.mapKeys || {}) }
      if (String(key || '').trim()) keys[provider] = String(key).trim()
      else delete keys[provider]
      writeLocalSettings({ ...local, mapKeys: keys })
      return mockMapKeys(keys)
    }
    return invoke('travel_set_map_key', { provider, key })
  },

  async getMapKeys() {
    if (!isTauriRuntime()) {
      const local = readLocalSettings()
      return mockMapKeys(local.mapKeys || {})
    }
    return invoke('travel_get_map_keys')
  },

  async getQuotaUsage() {
    if (!isTauriRuntime()) {
      return {
        day: new Date().toISOString().slice(0, 10),
        providers: { amap: { poi: 2, route: 3, geocode: 1 } },
        cacheCount: 8,
        limits: { amap: { poi: 100, route: 5000, geocode: 5000, weather: 5000 } },
        preview: true,
      }
    }
    return invoke('travel_get_quota_usage')
  },

  async clearCache() {
    if (!isTauriRuntime()) return
    return invoke('travel_clear_cache')
  },

  async getPreferences() {
    if (!isTauriRuntime()) {
      try {
        return JSON.parse(localStorage.getItem(PREFERENCES_KEY) || '{}')
      } catch {
        return {}
      }
    }
    return invoke('travel_get_preferences')
  },

  async setPreferences(preferences) {
    if (!isTauriRuntime()) {
      try {
        localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences || {}))
      } catch {
        /* ignore */
      }
      return preferences
    }
    return invoke('travel_set_preferences', { preferences })
  },

  async resetPreferences() {
    if (!isTauriRuntime()) {
      try {
        localStorage.removeItem(PREFERENCES_KEY)
      } catch {
        /* ignore */
      }
      return {}
    }
    return invoke('travel_reset_preferences')
  },

  async shareStart(planId, html) {
    if (!isTauriRuntime()) {
      await delay(260)
      const token = uid('demo')
      const url = `http://192.168.1.100:39210/${token}/`
      return {
        running: true,
        url,
        urls: [url, `http://127.0.0.1:39210/${token}/`],
        port: 39210,
        planId,
        startedAt: new Date().toISOString(),
        preview: true,
      }
    }
    return invoke('travel_share_start', { planId, html })
  },

  async shareStop() {
    if (!isTauriRuntime()) return
    return invoke('travel_share_stop')
  },

  async shareStatus() {
    if (!isTauriRuntime()) return { running: false, url: '', urls: [], port: 0, planId: '', startedAt: '' }
    return invoke('travel_share_status')
  },

  async openPrintWindow(planId, theme = 'light') {
    if (!isTauriRuntime()) {
      window.print()
      return { preview: true }
    }
    return invoke('travel_open_print_window', { planId, theme })
  },

  async copyImage(dataUrl) {
    if (!isTauriRuntime()) throw new Error('浏览器预览不支持写入剪贴板')
    return invoke('travel_copy_image', { dataUrl })
  },
}

export function createLocalDraft(overrides = {}) {
  const startDate = overrides.startDate || nextSaturday()
  return {
    mode: 'trip',
    origin: { ...DEMO_ORIGIN },
    destination: { ...DEMO_DESTINATION },
    startDate,
    endDate: overrides.endDate || addDays(startDate, 1),
    startTime: '08:30',
    endTime: '20:30',
    transport: 'driving',
    budgetLevel: 'comfort',
    budgetTotal: null,
    adults: 2,
    children: 0,
    elders: 0,
    interests: ['自然风光', '美食', '摄影'],
    pace: 'balanced',
    keywords: '',
    mustVisit: '',
    avoid: '',
    radiusKm: 60,
    source: 'auto',
    mapProvider: 'auto',
    ...overrides,
  }
}

export function createWeekendDraft(overrides = {}) {
  const startDate = overrides.startDate || nextSaturday()
  return createLocalDraft({
    mode: 'weekend',
    startDate,
    // 周末默认当天往返；把返回日期改到次日即可生成两天一夜。
    endDate: overrides.endDate || startDate,
    startTime: overrides.startTime || '09:30',
    endTime: overrides.endTime || '17:00',
    ...overrides,
  })
}

export { createDemoPlan }
