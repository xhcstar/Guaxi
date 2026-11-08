// 行迹应用状态与编排流水线：事实 → 大纲 → 路线 → 分日 → 预算 → 保存。
// 所有耗时步骤都有阶段进度；模型失败时自动落到确定性编排，保证永远能出计划。

import { computed, reactive } from 'vue'
import { api, createLocalDraft, createWeekendDraft } from './travelApi.js'
import { createDemoPlan } from './travelData.js'
import { homeGuardKm, orderSights, pickNearAnchor, tooCloseToHome } from './travelEngine.js'
import { assemblePlan, computeBudget, defaultPlanTitle } from './travelPlan.js'
import { formatProblems, validatePlan } from './travelValidate.js'
import {
  buildDayPrompt,
  buildOutlinePrompt,
  fallbackDay,
  fallbackOutline,
  normalizeDay,
  normalizeOutline,
  parseModelJson,
} from './travelPrompt.js'
import {
  applyPreferencesToForm,
  learnFromForm,
  mergePreferences,
  webProviderLabel,
} from './travelPreferences.js'
import {
  addDays,
  deepClone,
  enumerateDates,
  formatDate,
  haversineKm,
  minutesToTime,
  nextSaturday,
  parseDate,
  parsePolyline,
  runWithConcurrency,
  safeText,
  samplePolyline,
  timeToMinutes,
  todayString,
  uid,
  uniqueBy,
  weekdayCn,
} from './travelUtils.js'

const STORAGE_ORIGIN = 'guaxi.travel.origin.v1'
const STORAGE_PREFER_OSM = 'guaxi.travel.preferOsm.v1'
const STORAGE_DEFAULTS = 'guaxi.travel.defaults.v1'
// 网页模型是没有流式的黑盒：一次编排最多等 120 秒，超时立刻走确定性编排，
// 避免整条流水线在某个阶段一停就是几分钟。
const WEB_MODEL_TIMEOUT_MS = 120 * 1000

const state = reactive({
  view: 'home',
  plans: [],
  plan: null,
  settings: {
    amapKeySet: false,
    amapKeyTail: '',
    ownKeySet: false,
    albumKeySet: false,
    dataRoot: '',
    planCount: 0,
    providerLabel: '',
    source: 'osm',
    preview: false,
  },
  defaultOrigin: null,
  preferences: mergePreferences({}),
  preferOsm: false,
  wizard: null,
  weekend: {
    seriesId: '',
    plans: [],
    loading: false,
    refiningId: '',
    lastRun: '',
  },
  snapshots: [],
  busy: false,
  stage: { key: '', label: '', detail: '', percent: 0 },
  error: '',
  warning: '',
  toast: { show: false, text: '', type: 'info' },
  share: {
    status: null,
    qr: '',
    html: '',
    busy: false,
    error: '',
  },
  generationToken: 0,
})

let toastTimer = null

function showToast(text, type = 'info', duration = 2600) {
  state.toast = { show: true, text, type }
  if (toastTimer) clearTimeout(toastTimer)
  toastTimer = setTimeout(() => {
    state.toast = { ...state.toast, show: false }
  }, duration)
}

function setStage(key, label, percent = 0, detail = '') {
  state.stage = { key, label, percent, detail, startedAt: Date.now() }
}

function modelWaitHint() {
  if (state.preferences.aiSource !== 'web') return ''
  const label = webProviderLabel(state.preferences.webProvider)
  return `网页模型 ${label} 最多等 ${WEB_MODEL_TIMEOUT_MS / 1000} 秒，没有响应就改用确定性编排`
}

// 校验不过就不落盘：宁可报错，也不把违反距离/日期约束的计划交给用户。
// 浏览器预览用的是内置示例 POI，与目的地坐标不同源，只校验结构，不校验半径。
function assertPlanConstraints(plan, options) {
  const report = validatePlan(plan, api.isMock ? { ...options, radiusKm: 0 } : options)
  if (report.ok) return
  throw new Error(`生成结果未通过约束校验：${formatProblems(report.problems)}`)
}

function friendlyError(error) {
  const text = String(error?.message || error || '未知错误')
  if (text.includes('未配置高德')) return text
  if (text.includes('浏览器预览不调用模型')) return text
  if (text.includes('没有可用的模型')) return '还没有可用的模型：请到「设置 → 大模型」填写 API Key。'
  if (text.includes('Failed to fetch') || text.includes('network') || text.includes('请求')) {
    return '网络请求失败，请检查网络或代理设置后重试。'
  }
  return text
}

function readJson(key, fallback) {
  try {
    const raw = localStorage.getItem(key)
    return raw ? JSON.parse(raw) : fallback
  } catch {
    return fallback
  }
}

function writeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    /* ignore */
  }
}

export function useTravelStore() {
  const hasPlans = computed(() => state.plans.length > 0)
  const activePlan = computed(() => state.plan)
  const settingsReady = computed(() => Boolean(state.settings.providerLabel !== undefined))

  async function init() {
    api.seedPreview()
    state.preferOsm = localStorage.getItem(STORAGE_PREFER_OSM) === '1'
    state.defaultOrigin = readJson(STORAGE_ORIGIN, null)
    const defaults = readJson(STORAGE_DEFAULTS, {})
    try {
      state.preferences = mergePreferences(await api.getPreferences())
      if (state.preferences.home || state.preferences.lastOrigin) {
        state.defaultOrigin = state.preferences.home || state.preferences.lastOrigin
      }
    } catch {
      state.preferences = mergePreferences({})
    }
    try {
      state.settings = await api.getSettings()
    } catch {
      /* 设置读取失败不阻断计划库 */
    }
    await refreshPlans()
    return defaults
  }

  async function refreshPlans() {
    try {
      const list = await api.listPlans()
      state.plans = list.filter((item) => item.kind !== 'snapshot')
      state.snapshots = list.filter((item) => item.kind === 'snapshot')
      state.settings.planCount = state.plans.length
    } catch (error) {
      state.error = friendlyError(error)
    }
  }

  async function openPlan(planId) {
    state.error = ''
    try {
      state.plan = await api.getPlan(planId)
      state.view = 'plan'
      state.share = { status: null, qr: '', html: '', busy: false, error: '' }
      const status = await api.shareStatus().catch(() => null)
      if (status?.running && status.planId === planId) {
        state.share.status = status
      }
    } catch (error) {
      state.error = friendlyError(error)
    }
  }

  function showHome() {
    state.view = 'home'
    state.error = ''
  }

  function newTrip(mode = 'trip') {
    const defaults = readJson(STORAGE_DEFAULTS, {})
    const draft =
      mode === 'weekend'
        ? createWeekendDraft({
            ...defaults,
            origin: state.defaultOrigin || defaults.origin || createLocalDraft().origin,
          })
        : createLocalDraft({
            ...defaults,
            origin: state.defaultOrigin || defaults.origin || createLocalDraft().origin,
          })
    draft.mode = mode
    state.wizard = applyPreferencesToForm(draft, state.preferences)
    state.defaultOrigin = state.wizard.origin
    state.view = 'wizard'
    state.error = ''
    state.warning = ''
    state.stage = { key: '', label: '', detail: '', percent: 0 }
  }

  function editWizard(patch = {}) {
    if (!state.wizard) state.wizard = createLocalDraft()
    Object.assign(state.wizard, patch)
  }

  async function locateOrigin() {
    try {
      const place = await api.ipLocation()
      if (!state.wizard) state.wizard = createLocalDraft()
      state.wizard.origin = place
      rememberOrigin(place)
      showToast(`已定位到 ${place.city || place.name}，请确认详细出发地址`, 'success')
      return place
    } catch (error) {
      showToast(`定位失败：${friendlyError(error)}`, 'error')
      return null
    }
  }

  async function searchPlaces(keyword, city = '') {
    try {
      return await api.searchPlaces(keyword, city)
    } catch {
      return []
    }
  }

  function rememberOrigin(origin) {
    if (!origin) return
    state.defaultOrigin = origin
    writeJson(STORAGE_ORIGIN, origin)
  }

  // 家庭地址存的可能是“只有名字和 adcode、没有坐标”的旧数据，
  // 一旦按名字解析出了坐标，就把它补回去，避免每次生成都重新猜城市。
  async function repairHomeOrigin(resolved) {
    if (!resolved?.lng || !resolved?.lat) return
    const home = state.preferences.home
    if (!home || home.lng || !home.adcode) return
    if (String(home.adcode).slice(0, 4) !== String(resolved.adcode || '').slice(0, 4)) return
    await savePreferences({
      ...state.preferences,
      home: { ...home, ...resolved, name: home.name || resolved.name },
    })
  }

  function rememberDefaults(form) {
    writeJson(STORAGE_DEFAULTS, {
      interests: form.interests,
      pace: form.pace,
      transport: form.transport,
      budgetLevel: form.budgetLevel,
      adults: form.adults,
      children: form.children,
      elders: form.elders,
      source: form.source,
    })
  }

  function splitList(value) {
    return String(value || '')
      .split(/[,，、\n]/)
      .map((item) => item.trim())
      .filter(Boolean)
  }

  function validationError(form) {
    if (!form.origin?.name) return '请先确认出发地'
    if (!form.destination?.name) return '请选择目的地'
    if (!form.startDate) return '请选择出发日期'
    if (!form.endDate) return '请选择返回日期'
    if (form.endDate < form.startDate) return '返回日期不能早于出发日期'
    if (!form.startTime || !form.endTime) return '请选择出发和结束时间'
    if (!form.adults && !form.children && !form.elders) return '至少需要一位出行人'
    return ''
  }

  function toRequest(form) {
    return {
      origin: {
        name: form.origin?.name || '',
        address: form.origin?.address || '',
        city: form.origin?.city || '',
        adcode: form.origin?.adcode || '',
        cityCode: form.origin?.cityCode || '',
        lng: Number(form.origin?.lng) || 0,
        lat: Number(form.origin?.lat) || 0,
      },
      destination: {
        name: form.destination?.name || '',
        address: form.destination?.address || '',
        city: form.destination?.city || '',
        adcode: form.destination?.adcode || '',
        cityCode: form.destination?.cityCode || '',
        lng: Number(form.destination?.lng) || 0,
        lat: Number(form.destination?.lat) || 0,
      },
      startDate: form.startDate,
      endDate: form.endDate,
      startTime: form.startTime,
      endTime: form.endTime,
      transport: form.transport,
      mode: form.mode,
      budgetLevel: form.budgetLevel,
      adults: Number(form.adults) || 1,
      children: Number(form.children) || 0,
      elders: Number(form.elders) || 0,
      interests: form.interests || [],
      pace: form.pace,
      radiusKm: Number(form.radiusKm) || 20,
      keywords: form.keywords || '',
      mustVisit: splitList(form.mustVisit),
      avoid: splitList(form.avoid),
      source: state.preferOsm ? 'osm' : form.source || 'auto',
      mapProvider: form.mapProvider || state.preferences.mapProvider || 'auto',
    }
  }

  function poiMap(facts) {
    const map = new Map()
    for (const poi of facts.attractions || []) map.set(poi.id, { ...poi, kind: 'sight' })
    for (const poi of facts.food || []) map.set(poi.id, { ...poi, kind: 'food' })
    for (const poi of facts.hotels || []) map.set(poi.id, { ...poi, kind: 'hotel' })
    return map
  }

  function buildStopsForDay({ form, facts, outline, outlineDay, dayIndex, totalDays, map }) {
    const startPoint =
      dayIndex === 0
        ? facts.origin
        : map.get(outline.days[dayIndex - 1]?.hotelId) ||
          facts.destination ||
          facts.origin
    const stopRefs = (outlineDay.stops || [])
      .map((item) => map.get(item.poiId))
      .filter(Boolean)
      .map((poi) => ({ ...poi }))
    const hotel = outlineDay.hotelId ? map.get(outlineDay.hotelId) : null
    const points = []
    const push = (poi) => {
      if (!poi) return
      const last = points[points.length - 1]
      if (last && Math.abs(last.lng - poi.lng) < 0.00001 && Math.abs(last.lat - poi.lat) < 0.00001) {
        return
      }
      points.push(poi)
    }
    push(startPoint)
    for (const poi of stopRefs) push(poi)
    if (dayIndex < totalDays - 1 && hotel) push(hotel)
    if (dayIndex === totalDays - 1) push(facts.origin)
    const routeStops = points.map((poi) => ({
      id: poi.id || '',
      name: poi.name || '',
      city: poi.city || (poi === facts.origin ? facts.origin.city : facts.destination?.city) || '',
      cityCode:
        poi.cityCode ||
        (poi === facts.origin ? facts.origin.cityCode : facts.destination?.cityCode) ||
        '',
      lng: Number(poi.lng) || 0,
      lat: Number(poi.lat) || 0,
    }))
    return {
      startName: startPoint?.name || '',
      stopRefs,
      routeStops,
    }
  }

  function fallbackLeg(from, to) {
    const toRad = (value) => (value * Math.PI) / 180
    const dLat = toRad(to.lat - from.lat)
    const dLng = toRad(to.lng - from.lng)
    const a =
      Math.sin(dLat / 2) ** 2 +
      Math.cos(toRad(from.lat)) * Math.cos(toRad(to.lat)) * Math.sin(dLng / 2) ** 2
    const distance = Math.max(0.3, 2 * 6371 * Math.asin(Math.sqrt(Math.min(1, a))) * 1.2)
    const duration = (distance / 40) * 3600
    return {
      mode: 'driving',
      distance,
      duration,
      tolls: 0,
      fare: 0,
      traffic: '',
      summary: `驾车 · ${distance.toFixed(1)} km · 约 ${Math.round(duration / 60)} 分钟`,
      steps: [
        {
          instruction: `从 ${from.name || '起点'} 前往 ${to.name || '下一站'}（未获取到逐段导航，按直线估算）`,
          road: '',
          distance: distance * 1000,
          duration,
        },
      ],
      polyline: `${from.lng},${from.lat};${to.lng},${to.lat}`,
      warning: '路线服务暂不可用，已按直线估算，出发前请复核。',
      fromName: from.name || '',
      toName: to.name || '',
    }
  }

  function routeWeatherKey(point) {
    return `${point.date}|${String(point.time || '').slice(0, 2)}|${Number(point.lng).toFixed(3)}|${Number(point.lat).toFixed(3)}`
  }

  function evenIndexes(length, count) {
    if (length <= count) return Array.from({ length }, (_, index) => index)
    const picked = new Set()
    for (let index = 0; index < count; index += 1) {
      picked.add(Math.round((index * (length - 1)) / (count - 1)))
    }
    return [...picked]
  }

  /** 沿真实路线折线采样出发地、途经地区与目的地，再批量取逐时天气。 */
  async function attachRouteWeather({ form, facts, routeInfo }) {
    const requestPoints = []
    const indexByKey = new Map()
    const pushPoint = (point) => {
      const key = routeWeatherKey(point)
      if (indexByKey.has(key)) return indexByKey.get(key)
      const index = requestPoints.length
      indexByKey.set(key, index)
      requestPoints.push(point)
      return index
    }

    const forecastHorizon = formatDate(addDays(todayString(), 16))
    routeInfo.forEach((info) => {
      let cursor = timeToMinutes(form.startTime)
      if (cursor == null) cursor = 9 * 60
      const offline = Boolean(info.date && info.date > forecastHorizon)
      const daySamples = []
      const samplesByLeg = []
      ;(info.legs || []).forEach((leg, legIndex) => {
        const duration = Math.max(5, Math.round((Number(leg.duration) || 0) / 60))
        const distance = Number(leg.distance) || 0
        const mode = leg.mode || form.transport
        const corridor = ['driving', 'walking', 'transit'].includes(mode) && distance >= 25
        const fractions = corridor
          ? distance >= 120
            ? [0, 0.25, 0.5, 0.75, 1]
            : [0, 0.5, 1]
          : [0, 1]
        const coords = parsePolyline(
          leg.polyline,
          { lng: leg.fromLng, lat: leg.fromLat },
          { lng: leg.toLng, lat: leg.toLat },
        )
        const samples = []
        for (const fraction of fractions) {
          const position = samplePolyline(coords, fraction)
          if (!position) continue
          const [lng, lat] = position
          const time = minutesToTime(cursor + Math.round(duration * fraction))
          const name =
            fraction === 0
              ? leg.fromName || info.startName || '出发地'
              : fraction === 1
                ? leg.toName || '目的地'
                : ''
          const point = { name, lng, lat, date: info.date, time }
          const sample = offline
            ? {
                ...point,
                local: {
                  ...point,
                  weather: '',
                  icon: 'cloud',
                  available: false,
                  message: '超过 16 天预报窗口，临近出发再更新',
                  source: 'open-meteo',
                  fetchedAt: new Date().toISOString(),
                },
                fraction,
              }
            : { ...point, index: pushPoint(point), fraction }
          samples.push(sample)
          daySamples.push(sample)
        }
        samplesByLeg.push(samples)
        const stop = info.stopRefs?.[legIndex]
        const dwell = stop ? (stop.kind === 'food' ? 60 : stop.kind === 'hotel' ? 20 : 90) : 0
        cursor += duration + 10 + dwell + (legIndex < info.legs.length - 1 ? 15 : 0)
      })
      info.weatherSamples = daySamples
      info.weatherByLeg = samplesByLeg
    })

    let result = null
    let errorNote = ''
    const resolved = new Map()
    if (requestPoints.length) {
      const selected = evenIndexes(requestPoints.length, 40)
      try {
        result = await api.routeWeather({
          provider: facts.source,
          seasonDate: form.startDate,
          points: selected.map((index) => requestPoints[index]),
        })
        ;(result?.points || []).forEach((point, index) => {
          resolved.set(selected[index], point)
        })
      } catch (error) {
        errorNote = `沿途天气暂不可用：${friendlyError(error)}`
      }
    }
    routeInfo.forEach((info) => {
      ;(info.legs || []).forEach((leg, legIndex) => {
        const samples = info.weatherByLeg?.[legIndex] || []
        const weatherPoints = samples
          .map((sample, index) => {
            const point = sample.local || resolved.get(sample.index)
            return point ? { ...point, fraction: sample.fraction, sampleIndex: index } : null
          })
          .filter(Boolean)
        if (weatherPoints.length) {
          leg.weatherPoints = uniqueBy(weatherPoints, (item) => routeWeatherKey(item))
        }
      })
      info.routeWeather = uniqueBy(
        (info.weatherSamples || [])
          .map((sample) => sample.local || resolved.get(sample.index))
          .filter(Boolean),
        routeWeatherKey,
      )
    })
    const notes = [...(result?.notes || [])]
    if (errorNote) notes.push(errorNote)
    if (!requestPoints.length) {
      notes.push('所选日期超出 16 天预报窗口，临近出发再更新')
    } else if (requestPoints.length > 40) {
      notes.push('采样点较多，已均匀抽取 40 个代表点')
    }
    return {
      points: routeInfo.flatMap((info) => info.routeWeather || []),
      season: result?.season || '',
      seasonTips: result?.seasonTips || [],
      notes,
      fetchedAt: result?.fetchedAt || new Date().toISOString(),
      provider: result?.provider || '',
    }
  }

  async function buildRoutesForOutline({ form, facts, outline, totalDays, map, alternatives = true }) {
    const days = outline.days.map((outlineDay, index) =>
      buildStopsForDay({ form, facts, outline, outlineDay, dayIndex: index, totalDays, map }),
    )
    let routeDays = []
    try {
      routeDays = await api.buildRoutes({
        transport: form.transport,
        provider: facts.source,
        alternatives,
        days: days.map((item, index) => ({
          date: outline.days[index].date,
          stops: item.routeStops,
        })),
      })
    } catch {
      routeDays = days.map((item, index) => ({
        date: outline.days[index].date,
        legs: item.routeStops
          .slice(1)
          .map((stop, legIndex) =>
            fallbackLeg(item.routeStops[legIndex], stop),
          ),
      }))
    }
    const output = days.map((item, index) => {
      const legs = (routeDays[index]?.legs || []).map((leg, legIndex) => ({
        ...leg,
        fromName: item.routeStops[legIndex]?.name || leg.fromName || '',
        toName: item.routeStops[legIndex + 1]?.name || leg.toName || '',
      }))
      return {
        date: outline.days[index].date,
        legs,
        stopRefs: item.stopRefs,
        startName: item.startName,
      }
    })
    const weatherMeta = await attachRouteWeather({ form, facts, routeInfo: output })
    output.weatherMeta = weatherMeta
    return output
  }

  async function createOutline({ form, facts }) {
    try {
      const { system, prompt } = buildOutlinePrompt({ form, facts })
      const { raw, provider } = await completeJsonLlm({
        prompt,
        system,
        temperature: 0.35,
        maxTokens: 8192,
      })
      const outline = normalizeOutline(raw, { form, facts })
      return {
        outline,
        provider: provider || '',
        usedFallback: false,
        warning: '',
      }
    } catch (error) {
      return {
        outline: fallbackOutline({ form, facts }),
        provider: api.isMock ? '内置编排' : '确定性兜底',
        usedFallback: true,
        warning: api.isMock ? '' : `模型这次没有返回可用的大纲 JSON，已用确定性编排兜底：${friendlyError(error)}`,
      }
    }
  }

  async function completeLlm(payload) {
    if (state.preferences.aiSource === 'web') {
      const response = await api.webLlmComplete({
        provider: state.preferences.webProvider,
        prompt: payload.prompt,
        system: payload.system,
        timeoutMs: WEB_MODEL_TIMEOUT_MS,
      })
      return {
        text: response.text,
        provider: `网页 · ${webProviderLabel(state.preferences.webProvider)}`,
      }
    }
    if (state.preferences.aiSource === 'local') {
      const response = await api.localLlmComplete({
        provider: state.preferences.localModel,
        prompt: payload.prompt,
        system: payload.system,
      })
      return { text: response.text, provider: response.provider || '本地模型' }
    }
    return api.llmComplete({ ...payload, jsonMode: true, disableReasoning: true })
  }

  // 模型偶尔回一段说明而不是 JSON：强制 JSON 模式，再给一次“只输出 JSON”的机会。
  async function completeJsonLlm(payload) {
    const attempts = [
      payload,
      {
        ...payload,
        temperature: 0.2,
        prompt: `${payload.prompt}\n\n再次强调：只输出一个 JSON 对象，不要任何解释、标题或 markdown。`,
      },
    ]
    let lastError = null
    for (let index = 0; index < attempts.length; index += 1) {
      let response = null
      try {
        response = await completeLlm({ ...attempts[index], jsonMode: true })
        return { raw: parseModelJson(response.text), provider: response.provider || '' }
      } catch (error) {
        const message = String(error?.message || error)
        const label = response?.provider ? `（模型：${response.provider}）` : ''
        lastError = new Error(`${message}${label}`)
        // 配置/网络类错误重试也没用，直接抛出真实原因。
        if (/没有可用的模型|API Key|不调用模型|Failed to fetch|网络请求失败/.test(message)) break
      }
    }
    throw lastError || new Error('模型没有返回可解析的 JSON')
  }

  async function createDay({ form, facts, outlineDay, dayIndex, totalDays, legs, stopRefs, startName, useModel = true }) {
    const buildFallback = () => {
      const day = fallbackDay({
        form,
        facts,
        outlineDay,
        dayIndex,
        legs,
        stopRefs,
        startName,
      })
      day.source = 'fallback'
      return day
    }
    // 大纲阶段模型已经失败时不再逐日重试：网页模型每次都要等满超时，代价太高。
    if (!useModel) return buildFallback()
    try {
      const { system, prompt } = buildDayPrompt({
        form,
        facts,
        outlineDay,
        dayIndex,
        totalDays,
        legs,
        startName,
      })
      const { raw } = await completeJsonLlm({
        prompt,
        system,
        temperature: 0.3,
        maxTokens: 8192,
      })
      const day = normalizeDay(raw, {
        form,
        facts,
        outlineDay,
        dayIndex,
        legs,
        stopRefs,
        startName,
      })
      day.source = 'ai'
      return day
    } catch (error) {
      console.warn('[travel] 模型当天安排未通过校验，已改用确定性编排：', error?.message || error)
      return buildFallback()
    }
  }

  async function generateTrip(form) {
    const error = validationError(form)
    if (error) {
      showToast(error, 'error')
      return null
    }
    const token = ++state.generationToken
    state.busy = true
    state.error = ''
    state.warning = ''
    state.stage = { key: 'collect', label: '正在解析地点与检索周边…', detail: '', percent: 4 }
    try {
      const request = toRequest(form)
      rememberDefaults(form)
      const facts = await api.collectFacts(request)
      if (token !== state.generationToken) return null
      // 用后端解析后的坐标回填出发点，避免下一次又拿着 0 坐标按名字乱查城市。
      if (facts.origin?.lng && facts.origin?.lat) {
        form.origin = { ...form.origin, ...facts.origin }
        rememberOrigin(form.origin)
        if (state.wizard) state.wizard.origin = form.origin
        await repairHomeOrigin(form.origin)
      }
      const waitHint = modelWaitHint()
      setStage(
        'outline',
        '正在编排整体行程…',
        22,
        `已找到 ${facts.attractions.length} 个景点、${facts.food.length} 家餐饮、${facts.hotels.length} 家住宿${waitHint ? `；${waitHint}` : ''}`,
      )
      const outlineResult = await createOutline({ form, facts })
      if (token !== state.generationToken) return null
      if (outlineResult.warning) state.warning = outlineResult.warning
      setStage('routes', '正在计算路线并获取沿途实时天气…', 42)
      const map = poiMap(facts)
      const totalDays = outlineResult.outline.days.length
      const routeInfo = await buildRoutesForOutline({
        form,
        facts,
        outline: outlineResult.outline,
        totalDays,
        map,
      })
      if (token !== state.generationToken) return null
      setStage('days', '正在细化每天的游览节奏与用餐…', 58, modelWaitHint())
      const days = await runWithConcurrency(
        outlineResult.outline.days,
        2,
        async (outlineDay, index) => {
          const info = routeInfo[index]
          return createDay({
            form,
            facts,
            outlineDay,
            dayIndex: index,
            totalDays,
            legs: info.legs,
            stopRefs: info.stopRefs,
            startName: info.startName,
            useModel: !outlineResult.usedFallback,
          })
        },
      )
      if (token !== state.generationToken) return null
      setStage('save', '正在核算预算并保存…', 90)
      const fallbackDays = days.filter((day) => day.source === 'fallback').length
      if (fallbackDays > 0 && !api.isMock) {
        state.warning = state.warning
          ? `${state.warning}；其中 ${fallbackDays} 天使用了确定性编排。`
          : `其中 ${fallbackDays} 天的模型细化失败，已使用确定性编排，可在计划页重新生成。`
      }
      const plan = assemblePlan({
        form,
        facts,
        outline: outlineResult.outline,
        days,
        provider: outlineResult.provider,
        routeWeather: routeInfo.weatherMeta,
      })
      assertPlanConstraints(plan, {
        center: facts.destination,
        radiusKm: Number(form.radiusKm) || 20,
        expectedDates: enumerateDates(form.startDate, form.endDate),
      })
      const saved = await api.savePlan(plan)
      if (token !== state.generationToken) return null
      state.plan = saved
      await refreshPlans()
      await learnPreferences(form)
      state.view = 'plan'
      state.stage = { key: 'done', label: '行程已生成', detail: '', percent: 100 }
      showToast('行程已生成，可以查看地图、预算和分享', 'success')
      return saved
    } catch (error) {
      state.error = friendlyError(error)
      state.stage = { key: '', label: '', detail: '', percent: 0 }
      return null
    } finally {
      if (token === state.generationToken) {
        setTimeout(() => {
          if (state.stage.key === 'done') {
            state.stage = { key: '', label: '', detail: '', percent: 0 }
          }
        }, 800)
        state.busy = false
      }
    }
  }

  // ── 周末班次 ───────────────────────────────────────────────

  const WEEKEND_PROMPT_SYSTEM = `你是一名熟悉城市周边出游的中文旅行规划师。你只能使用给出的候选地点 id，不得编造地点。
只输出一个 JSON 对象，不要 markdown，不要解释。每个周末安排周六、周日两天，节奏松弛。
周末出游是“出门”而不是在小区附近转：目的地优先选在半径外圈，类型尽量多样——峪口、古道、徒步点、观景台、水库、古镇古村、县城集镇、集市、农家乐、公园、商场、博物馆、寺庙都可以作为目的地；餐饮优先选目的地附近的小店。`

  function weekendDates(form) {
    let first = nextSaturday()
    const start = parseDate(form?.startDate)
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    if (start && start >= today && (start.getDay() === 6 || start.getDay() === 0)) {
      first = formatDate(start)
    }
    return Array.from({ length: 8 }, (_, index) => addDays(first, index * 7))
  }

  function buildWeekendPrompt({ form, facts, dates }) {
    const ring = weekendRing(form)
    // 按距离从远到近列给模型：目的地本来就要选外圈，先看到远处的候选。
    const attractions = [...(facts.attractions || [])]
      .sort((a, b) => (Number(b.distance) || 0) - (Number(a.distance) || 0))
      .slice(0, 36)
      .map(
        (poi) =>
          `- ${poi.id} | ${poi.name} | ${poi.category} | 评分 ${poi.rating ?? '未知'} | ${Math.round((Number(poi.distance) || 0) / 1000)} km`,
      )
      .join('\n')
    const food = [...(facts.food || [])]
      .sort((a, b) => (Number(b.distance) || 0) - (Number(a.distance) || 0))
      .slice(0, 24)
      .map(
        (poi) =>
          `- ${poi.id} | ${poi.name} | ${poi.category || '餐饮'} | 评分 ${poi.rating ?? '未知'} | ${Math.round((Number(poi.distance) || 0) / 1000)} km`,
      )
      .join('\n')
    const hotels = [...(facts.hotels || [])]
      .sort((a, b) => (Number(b.distance) || 0) - (Number(a.distance) || 0))
      .slice(0, 10)
      .map((poi) => `- ${poi.id} | ${poi.name} | 参考人均 ${poi.cost ?? '未知'}`)
      .join('\n')
    const weather = (facts.weather || [])
      .map((item) => `${item.date} ${item.dayWeather || ''} ${item.nightTemp ?? ''}~${item.dayTemp ?? ''}℃`)
      .join('\n')
    const user = `【出发点】${facts.origin?.name || form.origin?.name}（${facts.origin?.city || ''}）
【周边半径】${form.radiusKm} km（目的地要真的出门：不要选 ${Math.round(ring.min)} km 以内的地点，优先 ${Math.round(ring.prefer)}-${Math.round(ring.radius)} km）
【人数】成人 ${form.adults}、儿童 ${form.children}、老人 ${form.elders}
【偏好】${(form.interests || []).join('、') || '不限定'}；节奏 ${form.pace}
【未来 8 个周六】${dates.join('、')}

【候选景点与周边目的地】
${attractions || '（无）'}
【候选餐饮】
${food || '（无）'}
【候选住宿】
${hotels || '（无）'}
【天气（最多 16 天，后面的周末可能没有）】
${weather || '（暂无）'}

请输出：
{"title":"这组周末计划的主题","weekends":[
 {"weekendStart":"${dates[0]}","basePoiId":"候选景点 id","theme":"周末主题","stops1":["候选景点 id"],"food1":"候选餐饮 id","stops2":["候选景点 id"],"food2":"候选餐饮 id","hotelId":"候选住宿 id 或空","note":"一句提醒"}
]}
要求：
- weekends 正好 8 项，顺序对应上面的 8 个周六。
- 每个周末只围绕一个 base 目的地，周六、周日各 1-2 个景点；不要每天都跨城。
- base 必须是候选里距离出发地 ${Math.round(ring.min)} km 以外的地点；能选 ${Math.round(ring.prefer)} km 以外的就不要选近的。
- 8 个周末的目的地和类型尽量不重复（峪口/山野、古镇古村、县城集市、公园湿地、商场博物馆、亲子农场等都算不同类型）。
- 餐饮优先选 base 附近的小店/农家乐，不要选出发地附近的连锁店。
- 只使用候选 id；住宿只在需要过夜时填写。`

    return { system: WEEKEND_PROMPT_SYSTEM, prompt: user }
  }

  // 周末出游应当“出门”：目的地放在半径外圈，而不是家门口。
  // min = 至少要离开家多远；prefer = 优先从这之外选（半径的一半左右）。
  function weekendRing(form) {
    const radius = Number(form?.radiusKm) || 60
    if (form?.transport === 'walking') {
      return { radius, min: 2, prefer: Math.max(3, radius * 0.15) }
    }
    if (form?.transport === 'transit') {
      return {
        radius,
        min: Math.max(5, radius * 0.15),
        prefer: Math.max(10, radius * 0.45),
      }
    }
    return {
      radius,
      min: Math.max(8, radius * 0.25),
      prefer: Math.max(15, radius * 0.55),
    }
  }

  // 粗分类只用于“8 个周末别都是公园”，不是数据字段。
  function weekendBaseKind(poi) {
    const text = `${poi?.name || ''}${poi?.category || ''}`
    if (/峪|谷|峡|山|森林|湿地|湖|水库|沙滩|岛/.test(text)) return 'nature'
    if (/古镇|古城|村|镇|寨|集|圩/.test(text)) return 'village'
    if (/徒步|营地|古道|观景|栈道|步道/.test(text)) return 'outdoor'
    if (/公园/.test(text)) return 'park'
    if (/商场|购物|广场|街区/.test(text)) return 'mall'
    if (/寺|庙|观|教堂|遗址/.test(text)) return 'temple'
    if (/博物馆|美术馆|纪念馆|展览/.test(text)) return 'museum'
    return String(poi?.category || 'other')
  }

  function normalizeWeekendOutline(raw, { form, facts, dates }) {
    const map = poiMap(facts)
    const attractions = [...(facts.attractions || [])].sort((a, b) => {
      const rating = (Number(b.rating) || 0) - (Number(a.rating) || 0)
      return rating || (Number(a.distance) || 0) - (Number(b.distance) || 0)
    })
    const ring = weekendRing(form)
    const distanceKm = (poi) => {
      const meters = Number(poi?.distance)
      return Number.isFinite(meters) ? meters / 1000 : NaN
    }
    const inRing = (poi) => !Number.isFinite(distanceKm(poi)) || distanceKm(poi) >= ring.min
    const inOuter = (poi) => Number.isFinite(distanceKm(poi)) && distanceKm(poi) >= ring.prefer
    const food = [...(facts.food || [])]
    const hotels = [...(facts.hotels || [])]
    const used = new Set()
    const usedBases = new Set()
    const usedKinds = new Set()
    // 目的地优先外圈；同样条件下同类目的地不连续重复（8 个周末别都是公园）。
    const pickBase = () => {
      const free = attractions.filter((poi) => !usedBases.has(poi.id))
      const groups = [
        free.filter((poi) => inOuter(poi) && !usedKinds.has(weekendBaseKind(poi))),
        free.filter((poi) => inRing(poi) && !usedKinds.has(weekendBaseKind(poi))),
        free.filter((poi) => inOuter(poi)),
        free.filter((poi) => inRing(poi)),
        free,
      ]
      const chosen = groups.find((group) => group.length)?.[0]
      if (chosen) usedKinds.add(weekendBaseKind(chosen))
      return chosen
    }
    const rawItems = Array.isArray(raw?.weekends) ? raw.weekends : []
    return dates.map((date, index) => {
      const source = rawItems[index] || {}
      let base = map.get(safeText(source.basePoiId))
      // 模型也可能挑一个家门口的公园：外圈还有得选时不采纳。
      const outerAvailable = attractions.some(
        (poi) => !usedBases.has(poi.id) && inOuter(poi),
      )
      if (
        base &&
        (!inRing(base) ||
          (outerAvailable && Number.isFinite(distanceKm(base)) && distanceKm(base) < ring.prefer))
      ) {
        base = null
      }
      if (!base) {
        base = pickBase() || attractions[index % Math.max(1, attractions.length)]
      }
      if (base) {
        used.add(base.id)
        usedBases.add(base.id)
      }
      const pickSights = (ids) =>
        (Array.isArray(ids) ? ids : [])
          .map((id) => map.get(safeText(id)))
          .filter(
            (poi) =>
              poi &&
              poi.kind !== 'food' &&
              poi.kind !== 'hotel' &&
              poi.id !== base?.id &&
              // 目的地离出发点十几公里，就别把家门口的公园排进同一天。
              (!base || haversineKm(base, poi) <= Math.max(15, ring.min)),
          )
          .slice(0, 2)
      let stops1 = orderSights(pickSights(source.stops1), base)
      let stops2 = orderSights(pickSights(source.stops2), base)
      if (!stops1.length && base) stops1 = [base]
      if (!stops2.length) {
        const next = attractions.find(
          (poi) =>
            !used.has(poi.id) &&
            poi.id !== base?.id &&
            (!base || haversineKm(base, poi) <= Math.max(15, ring.min)),
        )
        if (next) {
          stops2 = [next]
          used.add(next.id)
        } else if (base) {
          stops2 = [base]
        }
      }
      // 餐饮/住宿只取目的地附近：宁可不推荐，也不要让人开回几十公里外的家门口吃饭。
      const pickMeal = (rawId, exclude) => {
        const named = map.get(safeText(rawId))
        if (named && base && haversineKm(base, named) <= 12 && !exclude.has(named.id)) return named
        return pickNearAnchor(food, base, { maxKm: 12, exclude, kind: 'food' })
      }
      const food1 = pickMeal(source.food1, used)
      const foodUsed = new Set(used)
      if (food1) {
        foodUsed.add(food1.id)
        used.add(food1.id)
      }
      const food2 = pickMeal(source.food2, foodUsed)
      if (food2) used.add(food2.id)
      const namedHotel = map.get(safeText(source.hotelId))
      const hotel =
        (namedHotel && base && haversineKm(base, namedHotel) <= 15 && !used.has(namedHotel.id)
          ? namedHotel
          : null) || pickNearAnchor(hotels, base, { maxKm: 15, exclude: used, kind: 'hotel' })
      if (hotel) used.add(hotel.id)
      return {
        date,
        basePoiId: base?.id || '',
        theme: safeText(source.theme) || base?.category || '周边慢游',
        stops1: stops1.map((poi) => poi.id),
        stops2: stops2.map((poi) => poi.id),
        food1: food1?.id || '',
        food2: food2?.id || '',
        hotelId: hotel?.id || '',
        note: safeText(source.note),
      }
    })
  }

  function fallbackWeekendOutline({ form, facts, dates }) {
    return normalizeWeekendOutline({ weekends: [] }, { form, facts, dates })
  }

  function weekendOutlineDays(item, map, oneDay = false) {
    const base = map.get(item.basePoiId)
    const stops1 = uniqueBy(
      [base, ...item.stops1.map((id) => map.get(id))].filter(Boolean),
      (poi) => poi.id,
    )
    const stops2 = uniqueBy(
      item.stops2.map((id) => map.get(id)).filter(Boolean),
      (poi) => poi.id,
    )
    const food1 = map.get(item.food1)
    const food2 = map.get(item.food2)
    const hotel = map.get(item.hotelId)
    const day1Stops = []
    stops1.forEach((poi, index) => {
      day1Stops.push({ poiId: poi.id, kind: 'sight' })
      if (index === 0 && food1) day1Stops.push({ poiId: food1.id, kind: 'food' })
    })
    const day2Stops = []
    stops2.forEach((poi, index) => {
      day2Stops.push({ poiId: poi.id, kind: 'sight' })
      if (index === 0 && food2) day2Stops.push({ poiId: food2.id, kind: 'food' })
    })
    const firstDay = {
      date: item.date,
      weekday: weekdayCn(item.date),
      theme: `${item.theme} · ${weekdayCn(item.date)}`,
      area: base?.category || '',
      stops: day1Stops.slice(0, 4),
      hotelId: hotel?.id || '',
      paceNote: '',
    }
    if (oneDay) {
      return [{ ...firstDay, hotelId: '' }]
    }
    return [
      firstDay,
      {
        date: addDays(item.date, 1),
        weekday: weekdayCn(addDays(item.date, 1)),
        theme: `${item.theme} · ${weekdayCn(addDays(item.date, 1))}`,
        area: base?.category || '',
        stops: day2Stops.slice(0, 4),
        hotelId: '',
        paceNote: '',
      },
    ]
  }

  async function buildWeekendPlan({ form, facts, item, outline, map, seriesId, index, useModel = true }) {
    const oneDay = form.startDate === form.endDate
    const weekendForm = {
      ...form,
      mode: 'weekend',
      destination: item.basePoiId ? map.get(item.basePoiId) : form.destination,
      startDate: item.date,
      endDate: oneDay ? item.date : addDays(item.date, 1),
      startTime: form.startTime || '09:30',
      endTime: form.endTime || '17:00',
      budgetTotal: null,
    }
    const totalDays = oneDay ? 1 : 2
    const routeInfo = await buildRoutesForOutline({
      form: weekendForm,
      facts,
      outline,
      totalDays,
      map,
      alternatives: index === 0,
    })
    const detailed = useModel && index === 0
    const days = []
    for (let dayIndex = 0; dayIndex < totalDays; dayIndex += 1) {
      const outlineDay = outline.days[dayIndex]
      const info = routeInfo[dayIndex]
      if (detailed) {
        days.push(
          await createDay({
            form: weekendForm,
            facts,
            outlineDay,
            dayIndex,
            totalDays,
            legs: info.legs,
            stopRefs: info.stopRefs,
            startName: info.startName,
          }),
        )
      } else {
        const day = fallbackDay({
          form: weekendForm,
          facts,
          outlineDay,
          dayIndex,
          legs: info.legs,
          stopRefs: info.stopRefs,
          startName: info.startName,
        })
        day.source = 'fallback'
        days.push(day)
      }
    }
    const baseName = item.basePoiId ? map.get(item.basePoiId)?.name : ''
    const plan = assemblePlan({
      form: weekendForm,
      facts,
      outline: {
        ...outline,
        title: `${baseName || item.theme} · ${oneDay ? '周末一日' : '周末两日'}`,
        subtitle: item.theme + (item.note ? ` · ${item.note}` : ''),
      },
      days,
      provider: detailed ? '模型 + 确定性' : '班次表',
      detailLevel: detailed ? 'full' : 'draft',
      seriesId,
      weekendStart: item.date,
      routeWeather: routeInfo.weatherMeta,
    })
    plan.id = `wk_${seriesId}_${item.date.replaceAll('-', '')}`
    plan.kind = 'weekend'
    plan.locked = false
    plan.weekend = {
      basePoiId: item.basePoiId,
      theme: item.theme,
      note: item.note,
      index,
    }
    assertPlanConstraints(plan, {
      center: facts.destination,
      radiusKm: Number(weekendForm.radiusKm) || 20,
      expectedDates: oneDay ? [item.date] : [item.date, addDays(item.date, 1)],
    })
    return plan
  }

  async function generateWeekend(form) {
    const error = validationError({ ...form, destination: form.origin })
    if (error) {
      showToast(error, 'error')
      return []
    }
    if (!form.origin) {
      showToast('请先定位或选择出发点', 'error')
      return []
    }
    const token = ++state.generationToken
    state.busy = true
    state.error = ''
    state.warning = ''
    state.weekend.loading = true
    setStage('collect', '正在检索周边适合周末去的地方…', 5)
    try {
      const oneDay = form.startDate === form.endDate
      const dates = weekendDates(form)
      const baseForm = {
        ...form,
        mode: 'weekend',
        destination: form.origin,
        startDate: dates[0],
        endDate: oneDay ? dates[0] : addDays(dates[0], 1),
        startTime: form.startTime || '09:30',
        endTime: form.endTime || '17:00',
        budgetTotal: null,
        source: state.preferOsm ? 'osm' : form.source || 'auto',
      }
      const facts = await api.collectFacts(toRequest(baseForm))
      if (token !== state.generationToken) return []
      // 用后端解析后的坐标回填出发点，避免下一次又拿着 0 坐标按名字乱查城市。
      if (facts.origin?.lng && facts.origin?.lat) {
        form.origin = { ...form.origin, ...facts.origin }
        rememberOrigin(form.origin)
        if (state.wizard) state.wizard.origin = form.origin
        baseForm.origin = form.origin
        baseForm.destination = form.origin
        await repairHomeOrigin(form.origin)
      }
      setStage('outline', '正在把周边地点排成 8 个周末班次…', 30, modelWaitHint())
      let outlineItems
      let usedFallback = false
      let modelError = null
      try {
        const { system, prompt } = buildWeekendPrompt({ form: baseForm, facts, dates })
        const { raw } = await completeJsonLlm({ prompt, system, temperature: 0.5, maxTokens: 8192 })
        outlineItems = normalizeWeekendOutline(raw, { form: baseForm, facts, dates })
      } catch (error) {
        outlineItems = fallbackWeekendOutline({ form: baseForm, facts, dates })
        usedFallback = true
        modelError = error
      }
      const map = poiMap(facts)
      const seriesId = uid('wks')
      const plans = []
      for (let index = 0; index < outlineItems.length; index += 1) {
        if (token !== state.generationToken) return []
        const item = outlineItems[index]
        setStage(
          'days',
          `正在编排 ${item.date} 的周末…`,
          38 + Math.round((index / outlineItems.length) * 52),
        )
        const outline = {
          title: `${item.theme} · ${oneDay ? '周末一日' : '周末两日'}`,
          subtitle: item.note,
          days: weekendOutlineDays(item, map, oneDay),
          advice: [],
          mustKnow: [],
        }
        const plan = await buildWeekendPlan({
          form: baseForm,
          facts,
          item,
          outline,
          map,
          seriesId,
          index,
          useModel: !usedFallback,
        })
        const saved = await api.savePlan(plan)
        plans.push(saved)
      }
      state.weekend.seriesId = seriesId
      state.weekend.plans = plans
      state.weekend.lastRun = new Date().toISOString()
      await refreshPlans()
      await learnPreferences(form)
      state.view = 'weekend'
      if (plans[0]) {
        state.plan = plans[0]
      }
      if (usedFallback && !api.isMock) {
        const reason = modelError ? friendlyError(modelError) : '模型没有返回可解析的班次'
        state.warning = `模型这次没有返回可用的班次 JSON，已用确定性方案兜底：${reason}；可在班次表里逐个细化。`
      }
      showToast(
        usedFallback
          ? '已生成未来 8 个周末的班次，使用确定性编排'
          : '已生成未来 8 个周末的班次，最近一周已细化',
        'success',
      )
      state.stage = { key: 'done', label: '周末班次已生成', detail: '', percent: 100 }
      return plans
    } catch (error) {
      state.error = friendlyError(error)
      return []
    } finally {
      if (token === state.generationToken) {
        state.busy = false
        state.weekend.loading = false
        setTimeout(() => {
          if (state.stage.key === 'done') state.stage = { key: '', label: '', detail: '', percent: 0 }
        }, 800)
      }
    }
  }

  // 只是放弃这次结果：在途请求仍在后端跑，token 已变，回来时会被丢弃。
  function cancelGeneration() {
    if (!state.busy) return
    state.generationToken += 1
    state.busy = false
    state.weekend.loading = false
    state.weekend.refiningId = ''
    state.stage = { key: '', label: '', detail: '', percent: 0 }
    showToast('已取消本次生成', 'info')
  }

  async function openWeekendBoard() {
    state.view = 'weekend'
    const weekendPlans = state.plans
      .filter((item) => item.kind === 'weekend')
      .sort((a, b) => String(a.weekendStart).localeCompare(String(b.weekendStart)))
    const newest = [...weekendPlans].sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)))[0]
    state.weekend.seriesId = newest?.seriesId || ''
    const selected = state.weekend.seriesId
      ? weekendPlans.filter((item) => item.seriesId === state.weekend.seriesId)
      : weekendPlans
    state.weekend.plans = selected
    if (selected.length) {
      const full = await Promise.all(
        selected.map((item) => api.getPlan(item.id).catch(() => item)),
      )
      state.weekend.plans = full
    }
  }

  async function refineWeekend(planId) {
    const summary = state.plans.find((item) => item.id === planId)
    if (!summary) return
    if (state.weekend.refiningId) return
    state.weekend.refiningId = planId
    showToast('正在细化这个周末，模型和路线都需要重新核对…', 'info', 3200)
    try {
      const plan = await api.getPlan(planId)
      const form = {
        ...createWeekendDraft(),
        origin: plan.origin,
        destination: plan.destination,
        startDate: plan.startDate,
        endDate: plan.endDate,
        startTime: plan.startTime,
        endTime: plan.endTime,
        transport: plan.transport,
        budgetLevel: plan.budgetLevel,
        adults: plan.travelers?.adults || 1,
        children: plan.travelers?.children || 0,
        elders: plan.travelers?.elders || 0,
        interests: plan.interests || [],
        pace: plan.pace || 'balanced',
        // 沿用在生成时用过的半径，避免细化时把约束悄悄放宽。
        radiusKm: Number(plan.radiusKm) || createWeekendDraft().radiusKm,
        source: state.preferOsm ? 'osm' : 'auto',
      }
      const facts = await api.collectFacts(toRequest(form))
      const map = poiMap(facts)
      const days = plan.days || []
      // 旧计划里可能存在“家门口的公园/餐厅”，细化时先剔掉，再用目的地附近的候选补齐。
      const guard = homeGuardKm({ ...form, mode: 'weekend' })
      const outline = {
        title: plan.title,
        subtitle: plan.subtitle,
        days: days.map((day) => {
          const blocks = day.blocks || []
          const sights = orderSights(
            blocks
              .filter((block) => block.poi && (block.kind === 'sight' || block.kind === 'activity'))
              .filter((block) => !tooCloseToHome(block.poi, plan.origin, guard))
              .map((block) => map.get(block.poiId))
              .filter(Boolean),
            plan.destination,
          ).map((poi) => ({ poiId: poi.id, kind: 'sight' }))
          if (!sights.length) {
            const nearby = pickNearAnchor(facts.attractions || [], plan.destination, { maxKm: 20 })
            if (nearby) sights.push({ poiId: nearby.id, kind: 'sight' })
          }
          // 一天只排一个点太单薄：从目的地附近再补一个，避免 13 点就返程。
          if (sights.length === 1) {
            const extra = pickNearAnchor(
              (facts.attractions || []).filter((poi) => poi.id !== sights[0].poiId),
              plan.destination,
              { maxKm: 20 },
            )
            if (extra) sights.push({ poiId: extra.id, kind: 'sight' })
          }
          const hasFood = blocks.some(
            (block) =>
              block.poi && block.kind === 'food' && !tooCloseToHome(block.poi, plan.origin, guard),
          )
          const meal = hasFood
            ? null
            : pickNearAnchor(facts.food || [], plan.destination, { maxKm: 12, kind: 'food' })
          const stops = [...sights]
          if (meal) stops.splice(Math.min(1, stops.length), 0, { poiId: meal.id, kind: 'food' })
          return {
            date: day.date,
            weekday: day.weekday,
            theme: day.theme,
            area: day.area || '',
            stops,
            hotelId: blocks.find((block) => block.kind === 'hotel')?.poiId || '',
          }
        }),
        advice: plan.advice || [],
        mustKnow: plan.mustKnow || [],
      }
      const routeInfo = await buildRoutesForOutline({
        form,
        facts,
        outline,
        totalDays: outline.days.length,
        map,
      })
      const refinedDays = []
      for (let index = 0; index < outline.days.length; index += 1) {
        const info = routeInfo[index]
        refinedDays.push(
          await createDay({
            form,
            facts,
            outlineDay: outline.days[index],
            dayIndex: index,
            totalDays: outline.days.length,
            legs: info.legs,
            stopRefs: info.stopRefs,
            startName: info.startName,
          }),
        )
      }
      const rebuilt = assemblePlan({
        form,
        facts,
        outline,
        days: refinedDays,
        provider: '模型 + 确定性',
        detailLevel: 'full',
        seriesId: plan.seriesId,
        weekendStart: plan.weekendStart,
        routeWeather: routeInfo.weatherMeta,
      })
      rebuilt.id = plan.id
      rebuilt.createdAt = plan.createdAt
      rebuilt.locked = plan.locked
      rebuilt.weekend = plan.weekend
      assertPlanConstraints(rebuilt, {
        center: facts.destination,
        radiusKm: Number(form.radiusKm) || 20,
        expectedDates: days.map((day) => day.date),
      })
      const saved = await api.savePlan(rebuilt)
      state.weekend.plans = state.weekend.plans.map((item) =>
        item.id === planId ? saved : item,
      )
      state.plan = saved
      await refreshPlans()
      const aiDays = refinedDays.filter((day) => day.source === 'ai').length
      showToast(
        aiDays > 0
          ? '这个周末已细化完成'
          : 'AI 细化未通过校验，已保留确定性编排',
        aiDays > 0 ? 'success' : 'info',
        3200,
      )
    } catch (error) {
      showToast(`细化失败：${friendlyError(error)}`, 'error')
    } finally {
      state.weekend.refiningId = ''
    }
  }

  async function toggleWeekendLock(planId) {
    try {
      const plan = await api.getPlan(planId)
      plan.locked = !plan.locked
      const saved = await api.savePlan(plan)
      state.weekend.plans = state.weekend.plans.map((item) =>
        item.id === planId ? saved : item,
      )
      await refreshPlans()
      showToast(saved.locked ? '已锁定这个周末' : '已解锁', 'success')
    } catch (error) {
      showToast(friendlyError(error), 'error')
    }
  }

  async function replaceWeekend(planId) {
    try {
      const plan = await api.getPlan(planId)
      const usedNames = new Set(
        (state.weekend.plans || []).map((item) => item.destinationName).filter(Boolean),
      )
      usedNames.add(plan.destination?.name)
      const origin = plan.origin
      const form = {
        ...createWeekendDraft(),
        origin,
        startDate: plan.startDate,
        endDate: plan.endDate,
        transport: plan.transport,
        budgetLevel: plan.budgetLevel,
        adults: plan.travelers?.adults || 1,
        children: plan.travelers?.children || 0,
        elders: plan.travelers?.elders || 0,
        interests: plan.interests || [],
        pace: plan.pace || 'balanced',
        source: state.preferOsm ? 'osm' : 'auto',
      }
      const facts = await api.collectFacts({
        ...toRequest({ ...form, destination: origin }),
        radiusKm: 60,
      })
      const ring = weekendRing({ ...form, radiusKm: 60 })
      const candidates = (facts.attractions || []).filter(
        (poi) =>
          !usedNames.has(poi.name) &&
          Number(poi.distance) / 1000 >= ring.min,
      )
      const pool = candidates.length ? candidates : (facts.attractions || []).filter((poi) => !usedNames.has(poi.name))
      const base = pool[Math.floor(Math.random() * Math.max(1, pool.length))] || facts.attractions?.[0]
      if (!base) {
        showToast('周边暂时没有新的候选地点', 'error')
        return
      }
      const map = poiMap(facts)
      const nearby = orderSights(
        (facts.attractions || []).filter(
          (poi) => poi.id !== base.id && haversineKm(base, poi) <= Math.max(15, ring.min),
        ),
        base,
      ).slice(0, 3)
      const food = pickNearAnchor(facts.food || [], base, { maxKm: 12, kind: 'food' })
      const food2 = pickNearAnchor(facts.food || [], base, {
        maxKm: 12,
        kind: 'food',
        exclude: new Set(food ? [food.id] : []),
      })
      const hotel = pickNearAnchor(facts.hotels || [], base, { maxKm: 15, kind: 'hotel' })
      const item = {
        date: plan.weekendStart || plan.startDate,
        basePoiId: base.id,
        theme: base.category || '周边慢游',
        stops1: nearby.slice(0, 2).map((poi) => poi.id),
        stops2: nearby.slice(2, 3).map((poi) => poi.id),
        food1: food?.id || '',
        food2: food2?.id || food?.id || '',
        hotelId: hotel?.id || '',
        note: '换一个目的地后，时间与路线已重新计算。',
      }
      const oneDay = plan.startDate === plan.endDate
      const outline = {
        title: `${base.name} · ${oneDay ? '周末一日' : '周末两日'}`,
        subtitle: item.note,
        days: weekendOutlineDays(item, map, oneDay),
        advice: plan.advice || [],
        mustKnow: plan.mustKnow || [],
      }
      const rebuilt = await buildWeekendPlan({
        form,
        facts,
        item,
        outline,
        map,
        seriesId: plan.seriesId,
        index: 1,
      })
      rebuilt.id = plan.id
      rebuilt.createdAt = plan.createdAt
      rebuilt.locked = plan.locked
      rebuilt.weekend = { ...plan.weekend, basePoiId: base.id, theme: item.theme, note: item.note }
      const saved = await api.savePlan(rebuilt)
      state.weekend.plans = state.weekend.plans.map((entry) =>
        entry.id === planId ? saved : entry,
      )
      state.plan = saved
      await refreshPlans()
      showToast(`已换成 ${base.name}`, 'success')
    } catch (error) {
      showToast(`换目的地失败：${friendlyError(error)}`, 'error')
    }
  }

  // ── 计划维护 ───────────────────────────────────────────────

  async function updatePlan(plan, patch = {}) {
    const next = { ...state.plan, ...patch, updatedAt: new Date().toISOString() }
    state.plan = next
    try {
      const saved = await api.savePlan(next)
      state.plan = saved
      await refreshPlans()
    } catch (error) {
      state.error = friendlyError(error)
    }
    return next
  }

  async function updateBudget(items) {
    if (!state.plan) return
    const total = items.reduce((sum, item) => sum + (Number(item.amount) || 0), 0)
    const people =
      (state.plan.travelers?.adults || 0) +
      (state.plan.travelers?.children || 0) +
      (state.plan.travelers?.elders || 0)
    const next = {
      ...state.plan,
      budget: {
        ...state.plan.budget,
        items,
        total: Math.round(total),
        perPerson: Math.round(total / Math.max(1, people)),
      },
    }
    await updatePlan(next)
  }

  async function toggleChecklist(itemId) {
    if (!state.plan) return
    const checklist = (state.plan.checklist || []).map((item) =>
      item.id === itemId ? { ...item, done: !item.done } : item,
    )
    await updatePlan({ ...state.plan, checklist })
  }

  async function deletePlan(planId) {
    try {
      await api.deletePlan(planId)
      state.plans = state.plans.filter((item) => item.id !== planId)
      state.snapshots = state.snapshots.filter((item) => item.id !== planId)
      state.weekend.plans = state.weekend.plans.filter((item) => item.id !== planId)
      if (state.plan?.id === planId) {
        state.plan = null
        state.view = 'home'
      }
      showToast('计划已删除', 'success')
    } catch (error) {
      showToast(friendlyError(error), 'error')
    }
  }

  async function renamePlan(planId, title) {
    const nextTitle = String(title || '').trim()
    if (!nextTitle) return false
    try {
      const plan = await api.getPlan(planId)
      plan.title = nextTitle
      const saved = await api.savePlan(plan)
      if (state.plan?.id === planId) state.plan = saved
      await refreshPlans()
      showToast('已重命名', 'success')
      return true
    } catch (error) {
      showToast(`重命名失败：${friendlyError(error)}`, 'error')
      return false
    }
  }

  async function duplicatePlan(planId) {
    try {
      const plan = await api.getPlan(planId)
      const copy = deepClone(plan)
      const now = new Date().toISOString()
      copy.id = uid('trp')
      copy.title = `${plan.title || '未命名行程'} · 副本`
      copy.createdAt = now
      copy.updatedAt = now
      copy.adoptedAt = ''
      copy.snapshotOf = ''
      copy.originalKind = ''
      const saved = await api.savePlan(copy)
      await refreshPlans()
      state.plan = saved
      state.view = 'plan'
      showToast('已复制一份计划', 'success')
      return saved
    } catch (error) {
      showToast(`复制失败：${friendlyError(error)}`, 'error')
      return null
    }
  }

  // 采纳：给当前版本打上采纳时间，并留一份不可变快照；之后的修改不会覆盖历史。
  async function adoptPlan(planId) {
    try {
      const plan = await api.getPlan(planId)
      const now = new Date().toISOString()
      const snapshot = {
        ...deepClone(plan),
        id: uid('hist'),
        kind: 'snapshot',
        originalKind: plan.kind || 'trip',
        snapshotOf: plan.id,
        adoptedAt: now,
        createdAt: now,
        updatedAt: now,
      }
      await api.savePlan(snapshot)
      plan.adoptedAt = now
      const saved = await api.savePlan(plan)
      if (state.plan?.id === planId) state.plan = saved
      await refreshPlans()
      showToast('已采纳，并记录了一份历史版本', 'success')
      return saved
    } catch (error) {
      showToast(`采纳失败：${friendlyError(error)}`, 'error')
      return null
    }
  }

  async function restoreSnapshot(snapshotId) {
    try {
      const snapshot = await api.getPlan(snapshotId)
      const now = new Date().toISOString()
      const restored = {
        ...deepClone(snapshot),
        id: uid('trp'),
        kind: snapshot.originalKind || 'trip',
        originalKind: '',
        snapshotOf: '',
        adoptedAt: '',
        title: `${snapshot.title || '行程'} · 恢复`,
        createdAt: now,
        updatedAt: now,
        createdFromHistory: snapshot.id,
      }
      const saved = await api.savePlan(restored)
      await refreshPlans()
      state.plan = saved
      state.view = 'plan'
      showToast('历史版本已恢复为新的计划', 'success')
      return saved
    } catch (error) {
      showToast(`恢复失败：${friendlyError(error)}`, 'error')
      return null
    }
  }

  async function loadDemo() {
    const demo = createDemoPlan()
    demo.id = 'demo-hangzhou-weekend'
    try {
      const saved = await api.savePlan(demo)
      state.plan = saved
      await refreshPlans()
      state.view = 'plan'
      showToast('已载入杭州两日示例，可直接体验完整流程', 'success')
      return saved
    } catch (error) {
      showToast(friendlyError(error), 'error')
      return null
    }
  }

  async function saveSettings(key) {
    try {
      state.settings = await api.setAmapKey(key)
      showToast(key ? '高德 Key 已保存' : '已清除高德 Key，使用免费数据源', 'success')
      return state.settings
    } catch (error) {
      showToast(friendlyError(error), 'error')
      return state.settings
    }
  }

  async function savePreferences(next) {
    try {
      const merged = mergePreferences(next)
      state.preferences = mergePreferences(await api.setPreferences(merged))
      state.defaultOrigin =
        state.preferences.home || state.preferences.lastOrigin || state.defaultOrigin
      if (state.wizard && state.view === 'wizard') {
        state.wizard = applyPreferencesToForm(state.wizard, state.preferences)
      }
      showToast('个人偏好已保存，本次规划会自动使用', 'success')
      return state.preferences
    } catch (error) {
      showToast(`保存偏好失败：${friendlyError(error)}`, 'error')
      return state.preferences
    }
  }

  async function resetPreferences() {
    try {
      await api.resetPreferences()
      state.preferences = mergePreferences({})
      showToast('已清空个人偏好与学习记录', 'success')
      return state.preferences
    } catch (error) {
      showToast(`清空偏好失败：${friendlyError(error)}`, 'error')
      return state.preferences
    }
  }

  async function learnPreferences(form) {
    if (!state.preferences.autoLearn) return
    try {
      const next = learnFromForm(state.preferences, form)
      state.preferences = next
      await api.setPreferences(next)
    } catch {
      /* 学习失败不阻断行程生成 */
    }
  }

  function setPreferOsm(value) {
    state.preferOsm = Boolean(value)
    localStorage.setItem(STORAGE_PREFER_OSM, state.preferOsm ? '1' : '0')
  }

  function resetShare() {
    state.share.status = null
    state.share.qr = ''
    state.share.html = ''
    state.share.error = ''
  }

  function sortPlansForLibrary() {
    return [...state.plans].sort((a, b) => {
      const dateA = a.kind === 'weekend' ? a.weekendStart || a.startDate : a.startDate
      const dateB = b.kind === 'weekend' ? b.weekendStart || b.startDate : b.startDate
      return String(dateB).localeCompare(String(dateA))
    })
  }

  return {
    state,
    hasPlans,
    activePlan,
    settingsReady,
    init,
    refreshPlans,
    openPlan,
    showHome,
    newTrip,
    editWizard,
    locateOrigin,
    searchPlaces,
    generateTrip,
    generateWeekend,
    cancelGeneration,
    openWeekendBoard,
    refineWeekend,
    toggleWeekendLock,
    replaceWeekend,
    updatePlan,
    updateBudget,
    toggleChecklist,
    deletePlan,
    renamePlan,
    duplicatePlan,
    adoptPlan,
    restoreSnapshot,
    loadDemo,
    saveSettings,
    savePreferences,
    resetPreferences,
    setPreferOsm,
    resetShare,
    showToast,
    sortPlansForLibrary,
    upcomingPlans: () =>
      state.plans.filter((plan) => plan.startDate >= new Date().toISOString().slice(0, 10)),
  }
}
