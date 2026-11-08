// 行迹应用状态与编排流水线：事实 → 大纲 → 路线 → 分日 → 预算 → 保存。
// 所有耗时步骤都有阶段进度；模型失败时自动落到确定性编排，保证永远能出计划。

import { computed, reactive } from 'vue'
import { api, createLocalDraft, createWeekendDraft } from './travelApi.js'
import { createDemoPlan } from './travelData.js'
import { assemblePlan, computeBudget, defaultPlanTitle } from './travelPlan.js'
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
  addDays,
  deepClone,
  enumerateDates,
  nextSaturday,
  runWithConcurrency,
  safeText,
  uid,
  uniqueBy,
  weekdayCn,
} from './travelUtils.js'

const STORAGE_ORIGIN = 'guaxi.travel.origin.v1'
const STORAGE_PREFER_OSM = 'guaxi.travel.preferOsm.v1'
const STORAGE_DEFAULTS = 'guaxi.travel.defaults.v1'

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
  preferOsm: false,
  wizard: null,
  weekend: {
    seriesId: '',
    plans: [],
    loading: false,
    refiningId: '',
    lastRun: '',
  },
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
  state.stage = { key, label, percent, detail }
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
      state.settings = await api.getSettings()
    } catch {
      /* 设置读取失败不阻断计划库 */
    }
    await refreshPlans()
    return defaults
  }

  async function refreshPlans() {
    try {
      state.plans = await api.listPlans()
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
    state.wizard = draft
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
        lng: Number(form.origin?.lng) || 0,
        lat: Number(form.origin?.lat) || 0,
      },
      destination: {
        name: form.destination?.name || '',
        address: form.destination?.address || '',
        city: form.destination?.city || '',
        lng: Number(form.destination?.lng) || 0,
        lat: Number(form.destination?.lat) || 0,
      },
      startDate: form.startDate,
      endDate: form.endDate,
      startTime: form.startTime,
      endTime: form.endTime,
      transport: form.transport,
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

  async function buildRoutesForOutline({ form, facts, outline, totalDays, map }) {
    const days = outline.days.map((outlineDay, index) =>
      buildStopsForDay({ form, facts, outline, outlineDay, dayIndex: index, totalDays, map }),
    )
    let routeDays = []
    try {
      routeDays = await api.buildRoutes({
        transport: form.transport,
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
    return days.map((item, index) => {
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
  }

  async function createOutline({ form, facts }) {
    try {
      const { system, prompt } = buildOutlinePrompt({ form, facts })
      const response = await api.llmComplete({
        prompt,
        system,
        temperature: 0.35,
        maxTokens: 4096,
      })
      const raw = parseModelJson(response.text)
      const outline = normalizeOutline(raw, { form, facts })
      return {
        outline,
        provider: response.provider || '',
        usedFallback: false,
        warning: '',
      }
    } catch (error) {
      return {
        outline: fallbackOutline({ form, facts }),
        provider: api.isMock ? '内置编排' : '确定性兜底',
        usedFallback: true,
        warning: api.isMock ? '' : `模型编排不可用，已使用确定性编排：${friendlyError(error)}`,
      }
    }
  }

  async function createDay({ form, facts, outlineDay, dayIndex, totalDays, legs, stopRefs, startName }) {
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
      const response = await api.llmComplete({
        prompt,
        system,
        temperature: 0.3,
        maxTokens: 4096,
      })
      const raw = parseModelJson(response.text)
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
    } catch {
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
      rememberOrigin(form.origin)
      rememberDefaults(form)
      const facts = await api.collectFacts(request)
      if (token !== state.generationToken) return null
      setStage(
        'outline',
        '正在编排整体行程…',
        22,
        `已找到 ${facts.attractions.length} 个景点、${facts.food.length} 家餐饮、${facts.hotels.length} 家住宿`,
      )
      const outlineResult = await createOutline({ form, facts })
      if (token !== state.generationToken) return null
      if (outlineResult.warning) state.warning = outlineResult.warning
      setStage('routes', '正在计算点到点路线与路况…', 42)
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
      setStage('days', '正在细化每天的游览节奏与用餐…', 58)
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
      })
      const saved = await api.savePlan(plan)
      if (token !== state.generationToken) return null
      state.plan = saved
      await refreshPlans()
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

  const WEEKEND_PROMPT_SYSTEM = `你是一名熟悉城市周边游的中文旅行规划师。你只能使用给出的候选地点 id，不得编造地点。
只输出一个 JSON 对象，不要 markdown，不要解释。每个周末安排周六、周日两天，节奏松弛，适合从所在城市出发的短途游。`

  function weekendDates() {
    const first = nextSaturday()
    return Array.from({ length: 8 }, (_, index) => addDays(first, index * 7))
  }

  function buildWeekendPrompt({ form, facts, dates }) {
    const attractions = (facts.attractions || [])
      .slice(0, 24)
      .map(
        (poi) =>
          `- ${poi.id} | ${poi.name} | ${poi.category} | 评分 ${poi.rating ?? '未知'} | ${Math.round((Number(poi.distance) || 0) / 1000)} km`,
      )
      .join('\n')
    const food = (facts.food || [])
      .slice(0, 12)
      .map((poi) => `- ${poi.id} | ${poi.name} | 评分 ${poi.rating ?? '未知'}`)
      .join('\n')
    const hotels = (facts.hotels || [])
      .slice(0, 8)
      .map((poi) => `- ${poi.id} | ${poi.name} | 参考人均 ${poi.cost ?? '未知'}`)
      .join('\n')
    const weather = (facts.weather || [])
      .map((item) => `${item.date} ${item.dayWeather || ''} ${item.nightTemp ?? ''}~${item.dayTemp ?? ''}℃`)
      .join('\n')
    const user = `【出发点】${facts.origin?.name || form.origin?.name}（${facts.origin?.city || ''}）
【周边半径】${form.radiusKm} km
【人数】成人 ${form.adults}、儿童 ${form.children}、老人 ${form.elders}
【偏好】${(form.interests || []).join('、') || '不限定'}；节奏 ${form.pace}
【未来 8 个周六】${dates.join('、')}

【候选景点】
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
- 尽量让 8 个周末的目的地和主题有变化（自然/人文/美食/亲子/露营等），不要重复同一个 base。
- 只使用候选 id；住宿只在需要过夜时填写。`

    return { system: WEEKEND_PROMPT_SYSTEM, prompt: user }
  }

  function normalizeWeekendOutline(raw, { form, facts, dates }) {
    const map = poiMap(facts)
    const attractions = [...(facts.attractions || [])].sort((a, b) => {
      const rating = (Number(b.rating) || 0) - (Number(a.rating) || 0)
      return rating || (Number(a.distance) || 0) - (Number(b.distance) || 0)
    })
    const food = [...(facts.food || [])]
    const hotels = [...(facts.hotels || [])]
    const used = new Set()
    const usedBases = new Set()
    const rawItems = Array.isArray(raw?.weekends) ? raw.weekends : []
    return dates.map((date, index) => {
      const source = rawItems[index] || {}
      let base = map.get(safeText(source.basePoiId))
      if (!base) {
        base =
          attractions.find((poi) => !usedBases.has(poi.id)) ||
          attractions[index % Math.max(1, attractions.length)]
      }
      if (base) {
        used.add(base.id)
        usedBases.add(base.id)
      }
      const pickSights = (ids) =>
        (Array.isArray(ids) ? ids : [])
          .map((id) => map.get(safeText(id)))
          .filter((poi) => poi && poi.kind !== 'food' && poi.kind !== 'hotel')
          .slice(0, 2)
      let stops1 = pickSights(source.stops1)
      let stops2 = pickSights(source.stops2)
      if (!stops1.length && base) stops1 = [base]
      if (!stops2.length) {
        const next = attractions.find((poi) => !used.has(poi.id) && poi.id !== base?.id)
        if (next) {
          stops2 = [next]
          used.add(next.id)
        } else if (base) {
          stops2 = [base]
        }
      }
      const food1 = map.get(safeText(source.food1)) || food.find((poi) => !used.has(poi.id)) || food[0]
      const food2 = map.get(safeText(source.food2)) || food.find((poi) => !used.has(poi.id) && poi.id !== food1?.id) || food[1] || food[0]
      if (food1) used.add(food1.id)
      if (food2) used.add(food2.id)
      const hotel =
        map.get(safeText(source.hotelId)) ||
        hotels.find((poi) => !used.has(poi.id)) ||
        hotels[0]
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

  function weekendOutlineDays(item, map) {
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
    return [
      {
        date: item.date,
        weekday: weekdayCn(item.date),
        theme: `${item.theme} · 周六`,
        area: base?.category || '',
        stops: day1Stops.slice(0, 4),
        hotelId: hotel?.id || '',
        paceNote: '',
      },
      {
        date: addDays(item.date, 1),
        weekday: weekdayCn(addDays(item.date, 1)),
        theme: `${item.theme} · 周日`,
        area: base?.category || '',
        stops: day2Stops.slice(0, 4),
        hotelId: '',
        paceNote: '',
      },
    ]
  }

  async function buildWeekendPlan({ form, facts, item, outline, map, seriesId, index }) {
    const weekendForm = {
      ...form,
      mode: 'weekend',
      destination: item.basePoiId ? map.get(item.basePoiId) : form.destination,
      startDate: item.date,
      endDate: addDays(item.date, 1),
      startTime: '08:30',
      endTime: '19:30',
      budgetTotal: null,
    }
    const totalDays = 2
    const routeInfo = await buildRoutesForOutline({
      form: weekendForm,
      facts,
      outline,
      totalDays,
      map,
    })
    const detailed = index === 0
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
        title: `${baseName || item.theme} · 周末两日`,
        subtitle: item.theme + (item.note ? ` · ${item.note}` : ''),
      },
      days,
      provider: detailed ? '模型 + 确定性' : '班次表',
      detailLevel: detailed ? 'full' : 'draft',
      seriesId,
      weekendStart: item.date,
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
      rememberOrigin(form.origin)
      const dates = weekendDates()
      const baseForm = {
        ...form,
        mode: 'weekend',
        destination: form.origin,
        startDate: dates[0],
        endDate: addDays(dates[0], 1),
        startTime: '08:30',
        endTime: '19:30',
        budgetTotal: null,
        source: state.preferOsm ? 'osm' : form.source || 'auto',
      }
      const facts = await api.collectFacts(toRequest(baseForm))
      if (token !== state.generationToken) return []
      setStage('outline', '正在把周边地点排成 8 个周末班次…', 30)
      let outlineItems
      let usedFallback = false
      try {
        const { system, prompt } = buildWeekendPrompt({ form: baseForm, facts, dates })
        const response = await api.llmComplete({ prompt, system, temperature: 0.5, maxTokens: 4096 })
        const raw = parseModelJson(response.text)
        outlineItems = normalizeWeekendOutline(raw, { form: baseForm, facts, dates })
      } catch {
        outlineItems = fallbackWeekendOutline({ form: baseForm, facts, dates })
        usedFallback = true
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
          title: `${item.theme} · 周末两日`,
          subtitle: item.note,
          days: weekendOutlineDays(item, map),
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
        })
        const saved = await api.savePlan(plan)
        plans.push(saved)
      }
      state.weekend.seriesId = seriesId
      state.weekend.plans = plans
      state.weekend.lastRun = new Date().toISOString()
      await refreshPlans()
      state.view = 'weekend'
      if (plans[0]) {
        state.plan = plans[0]
      }
      if (usedFallback && !api.isMock) {
        state.warning = '模型班次编排不可用，已使用确定性方案；可在班次表里逐个细化。'
      }
      showToast('已生成未来 8 个周末的班次，最近一周已细化', 'success')
      state.stage = { key: 'done', label: '周末班次已生成', detail: '', percent: 100 }
      return plans
    } catch (error) {
      state.error = friendlyError(error)
      return []
    } finally {
      state.busy = false
      state.weekend.loading = false
      setTimeout(() => {
        if (state.stage.key === 'done') state.stage = { key: '', label: '', detail: '', percent: 0 }
      }, 800)
    }
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
    state.weekend.refiningId = planId
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
        source: state.preferOsm ? 'osm' : 'auto',
      }
      const facts = await api.collectFacts(toRequest(form))
      const map = poiMap(facts)
      const days = plan.days || []
      const outline = {
        title: plan.title,
        subtitle: plan.subtitle,
        days: days.map((day) => ({
          date: day.date,
          weekday: day.weekday,
          theme: day.theme,
          area: day.area || '',
          stops: (day.blocks || [])
            .filter((block) => block.poi && (block.kind === 'sight' || block.kind === 'food'))
            .map((block) => ({ poiId: block.poiId, kind: block.kind })),
          hotelId: (day.blocks || []).find((block) => block.kind === 'hotel')?.poiId || '',
        })),
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
      })
      rebuilt.id = plan.id
      rebuilt.createdAt = plan.createdAt
      rebuilt.locked = plan.locked
      rebuilt.weekend = plan.weekend
      const saved = await api.savePlan(rebuilt)
      state.weekend.plans = state.weekend.plans.map((item) =>
        item.id === planId ? saved : item,
      )
      state.plan = saved
      await refreshPlans()
      showToast('这个周末已细化完成', 'success')
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
      const candidates = (facts.attractions || []).filter((poi) => !usedNames.has(poi.name))
      const base = candidates[Math.floor(Math.random() * Math.max(1, candidates.length))] || facts.attractions?.[0]
      if (!base) {
        showToast('周边暂时没有新的候选地点', 'error')
        return
      }
      const map = poiMap(facts)
      const nearby = (facts.attractions || [])
        .filter((poi) => poi.id !== base.id)
        .slice(0, 3)
      const food = facts.food?.[0]
      const hotel = facts.hotels?.[0]
      const item = {
        date: plan.weekendStart || plan.startDate,
        basePoiId: base.id,
        theme: base.category || '周边慢游',
        stops1: nearby.slice(0, 2).map((poi) => poi.id),
        stops2: nearby.slice(2, 3).map((poi) => poi.id),
        food1: food?.id || '',
        food2: facts.food?.[1]?.id || food?.id || '',
        hotelId: hotel?.id || '',
        note: '换一个目的地后，时间与路线已重新计算。',
      }
      const outline = {
        title: `${base.name} · 周末两日`,
        subtitle: item.note,
        days: weekendOutlineDays(item, map),
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
    openWeekendBoard,
    refineWeekend,
    toggleWeekendLock,
    replaceWeekend,
    updatePlan,
    updateBudget,
    toggleChecklist,
    deletePlan,
    loadDemo,
    saveSettings,
    setPreferOsm,
    resetShare,
    showToast,
    sortPlansForLibrary,
    upcomingPlans: () =>
      state.plans.filter((plan) => plan.startDate >= new Date().toISOString().slice(0, 10)),
  }
}
