// 目的地候选池的评分层：全部是确定性规则，便于测试，也不受模型波动影响。
// 模型只负责把选中地点编排成行程；推荐理由由指标拼出来，缺数据就明说“未核实”。

import { haversineKm } from './travelUtils.js'

export const EXPLORE_CATEGORIES = [
  {
    key: 'nature',
    label: '自然风光',
    icon: 'camera',
    interests: ['自然风光', '摄影', '徒步'],
    duration: 150,
    indoor: 0.1,
    best: [3, 4, 5, 6, 9, 10, 11],
    experience: 0.8,
  },
  {
    key: 'heritage',
    label: '人文古迹',
    icon: 'book',
    interests: ['人文古迹', '摄影'],
    duration: 90,
    indoor: 0.3,
    best: [3, 4, 5, 9, 10, 11],
    experience: 0.8,
  },
  {
    key: 'museum',
    label: '博物馆',
    icon: 'book',
    interests: ['博物馆', '亲子', '人文古迹'],
    duration: 120,
    indoor: 0.95,
    best: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    experience: 0.75,
  },
  {
    key: 'landmark',
    label: '地标',
    icon: 'flag',
    interests: ['摄影', '自然风光'],
    duration: 45,
    indoor: 0.2,
    best: [3, 4, 5, 9, 10],
    experience: 0.65,
  },
  {
    key: 'street',
    label: '街区商业',
    icon: 'map',
    interests: ['购物', '美食', '摄影'],
    duration: 90,
    indoor: 0.35,
    best: [],
    experience: 0.6,
  },
  {
    key: 'night',
    label: '夜市美食街',
    icon: 'food',
    interests: ['美食', '夜生活'],
    duration: 90,
    indoor: 0.4,
    best: [4, 5, 6, 7, 8, 9, 10],
    experience: 0.65,
  },
  {
    key: 'market',
    label: '市场集市',
    icon: 'food',
    interests: ['美食', '小众', '购物'],
    duration: 60,
    indoor: 0.5,
    best: [],
    experience: 0.55,
  },
  {
    key: 'hike',
    label: '徒步户外',
    icon: 'backpack',
    interests: ['徒步', '自然风光', '露营'],
    duration: 210,
    indoor: 0.02,
    best: [3, 4, 5, 9, 10, 11],
    experience: 0.85,
  },
  {
    key: 'family',
    label: '亲子',
    icon: 'users',
    interests: ['亲子'],
    duration: 180,
    indoor: 0.4,
    best: [4, 5, 7, 8, 10],
    experience: 0.65,
  },
  {
    key: 'temple',
    label: '寺庙',
    icon: 'compass',
    interests: ['人文古迹', '小众'],
    duration: 60,
    indoor: 0.2,
    best: [],
    experience: 0.6,
  },
  {
    key: 'show',
    label: '演出剧场',
    icon: 'ticket',
    interests: ['夜生活', '亲子'],
    duration: 120,
    indoor: 0.95,
    best: [],
    experience: 0.7,
  },
  {
    key: 'hotspring',
    label: '温泉',
    icon: 'sun',
    interests: ['温泉'],
    duration: 180,
    indoor: 0.6,
    best: [11, 12, 1, 2, 3],
    experience: 0.75,
  },
  {
    key: 'food',
    label: '特色餐饮',
    icon: 'food',
    interests: ['美食'],
    duration: 75,
    indoor: 0.8,
    best: [],
    experience: 0.55,
  },
  {
    key: 'must',
    label: '必去',
    icon: 'heart',
    interests: [],
    duration: 120,
    indoor: 0.3,
    best: [],
    experience: 0.8,
  },
]

const FALLBACK_CATEGORY = {
  key: 'landmark',
  label: '地点',
  icon: 'pin',
  interests: [],
  duration: 90,
  indoor: 0.3,
  best: [],
  experience: 0.55,
}

const WEIGHTS = {
  rating: 0.22,
  experience: 0.14,
  interest: 0.14,
  access: 0.12,
  cost: 0.1,
  duration: 0.08,
  people: 0.08,
  weather: 0.06,
  season: 0.06,
}

const STAIR_WORDS = ['山', '峡', '古道', '栈道', '登', '索道', '徒步', '绝壁', '峰']
const CHILD_WORDS = ['乐园', '动物园', '海洋', '水族', '科技馆', '亲子', '植物园']

const clamp = (value, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, value))
const text = (value) => String(value ?? '').trim()

export function categoryMeta(key) {
  return EXPLORE_CATEGORIES.find((item) => item.key === key) || FALLBACK_CATEGORY
}

function ratingScore(item) {
  const rating = Number(item.rating)
  if (!Number.isFinite(rating) || rating <= 0) return { value: 0.5, known: false }
  return { value: clamp((rating - 3.5) / 1.5), known: true }
}

function accessScore(km, transport) {
  const distance = Math.max(0, Number(km) || 0)
  let value =
    distance <= 2
      ? 1
      : distance <= 6
        ? 0.9
        : distance <= 15
          ? 0.75
          : distance <= 30
            ? 0.55
            : distance <= 50
              ? 0.35
              : 0.15
  if (transport === 'walking') value *= distance <= 5 ? 1 : 0.45
  if (transport === 'transit') value *= distance <= 10 ? 1 : distance <= 30 ? 0.85 : 0.7
  return clamp(value)
}

function costScore(cost, level, kind) {
  const target = { economy: 80, comfort: 180, premium: 400 }[level] || 180
  const value = Number(cost)
  if (!Number.isFinite(value) || value < 0) return 0.6
  if (value === 0) return 1
  if (kind === 'sight') {
    return value <= target ? 0.9 : value <= target * 2 ? 0.65 : 0.4
  }
  const ratio = Math.max(value / target, 0.2)
  return clamp(1 - Math.abs(Math.log(ratio)) / 1.4, 0.15, 1)
}

function durationScore(minutes, pace) {
  const budget = pace === 'relaxed' ? 120 : pace === 'intense' ? 300 : 180
  return clamp(1 - Math.max(0, minutes - budget) / 240, 0.35, 1)
}

function peopleScore(item, meta, form) {
  const haystack = `${item.name || ''} ${item.category || ''} ${(item.tags || []).join(' ')}`
  const hasStairs = STAIR_WORDS.some((word) => haystack.includes(word))
  const isChildFriendly = CHILD_WORDS.some((word) => haystack.includes(word)) || meta.key === 'family'
  const scores = []
  if (Number(form.children) > 0) {
    let value = 0.85
    if (isChildFriendly) value = 1
    else if (meta.key === 'night' || meta.key === 'hike') value = 0.55
    else if (meta.key === 'hotspring') value = 0.75
    scores.push(value)
  }
  if (Number(form.elders) > 0) {
    let value = 0.85
    if (meta.key === 'hike' || hasStairs) value = 0.3
    else if (['street', 'market', 'museum', 'temple', 'show'].includes(meta.key)) value = 0.95
    else if (meta.key === 'night') value = 0.8
    scores.push(value)
  }
  if (!scores.length) return 0.85
  return scores.reduce((sum, value) => sum + value, 0) / scores.length
}

function weatherScore(meta, context) {
  const rain = clamp(Number(context.rainShare) || 0)
  let value = meta.indoor >= 0.6 ? 0.75 + 0.25 * rain : 1 - 0.55 * rain
  if (context.hot && meta.indoor < 0.6) value *= 0.85
  if (context.cold && meta.indoor < 0.4) value *= 0.8
  return clamp(value)
}

function seasonScore(meta, month) {
  if (!meta.best.length) return 0.8
  return meta.best.includes(month) ? 1 : 0.6
}

function interestScore(item, meta, interests) {
  const list = Array.isArray(interests) ? interests : []
  if (!list.length) return 0.6
  if (meta.interests.some((interest) => list.includes(interest))) return 1
  const haystack = `${item.name || ''} ${item.category || ''} ${(item.tags || []).join(' ')}`
  if (list.some((interest) => haystack.includes(interest))) return 0.9
  return 0.55
}

function experienceScore(item, meta, rating) {
  const tags = Array.isArray(item.tags) ? item.tags.length : 0
  return clamp(0.4 + Math.min(tags, 8) * 0.04 + Number(meta.experience || 0.5) * 0.35 + rating.value * 0.2)
}

function accessText(km, transport) {
  const distance = Number(km) || 0
  if (distance <= 2) return '步行可达'
  if (transport === 'transit') return `距基地 ${distance.toFixed(1)} km，需换乘`
  const minutes = Math.max(5, Math.round((distance * 1.25) / 35 * 60))
  return `距基地 ${distance.toFixed(1)} km，车程约 ${minutes} 分钟`
}

function buildReasons(item, meta, flags) {
  const reasons = []
  if (flags.must) reasons.push('你标记的必去')
  if (flags.ratingKnown) reasons.push(`评分 ${Number(item.rating).toFixed(1)}`)
  else reasons.push('评分未核实')
  reasons.push(accessText(item.distanceBaseKm, flags.transport))
  reasons.push(`建议留 ${meta.duration >= 60 ? `${(meta.duration / 60).toFixed(meta.duration % 60 ? 1 : 0)} 小时` : `${meta.duration} 分钟`}`)
  if (meta.indoor >= 0.6) reasons.push('室内，雨天也能去')
  if (Number(item.cost) > 0) reasons.push(`人均约 ¥${Math.round(Number(item.cost))}`)
  return reasons
}

function buildCaution(item, meta, form, flags, context) {
  const haystack = `${item.name || ''} ${item.category || ''} ${(item.tags || []).join(' ')}`
  if (Number(form.elders) > 0 && (meta.key === 'hike' || STAIR_WORDS.some((word) => haystack.includes(word)))) {
    return '台阶/爬坡较多，老人同行需量力'
  }
  if (Number(form.children) > 0 && ['night', 'hike'].includes(meta.key)) {
    return '不太适合带小孩，注意时间与体力'
  }
  if ((context.rainShare || 0) >= 0.5 && meta.indoor < 0.6) {
    return '行程日可能下雨，户外安排建议留一个室内备选'
  }
  if (
    flags.ratingKnown &&
    Number(item.rating) >= 4.6 &&
    ['landmark', 'street', 'night'].includes(meta.key)
  ) {
    return '热门时段人多，建议早到或错峰'
  }
  if (!flags.ratingKnown) return '数据来自 OSM，开放时间与票价请出发前确认'
  return ''
}

/**
 * 给候选逐条打分。context 由调用方从表单、天气与基地信息算好：
 * { form, month, rainShare, hot, cold, interests, transport, budgetLevel, pace }
 */
export function scoreCandidate(item, context) {
  const meta = categoryMeta(item.categoryKey)
  const form = context.form || {}
  const rating = ratingScore(item)
  const metrics = {
    rating: rating.value,
    experience: experienceScore(item, meta, rating),
    interest: interestScore(item, meta, context.interests || form.interests),
    access: accessScore(item.distanceBaseKm, context.transport || form.transport),
    cost: costScore(item.cost, context.budgetLevel || form.budgetLevel, item.kind),
    duration: durationScore(meta.duration, context.pace || form.pace),
    people: peopleScore(item, meta, form),
    weather: weatherScore(meta, context),
    season: seasonScore(meta, context.month),
  }
  let total = 0
  for (const [key, weight] of Object.entries(WEIGHTS)) {
    total += (metrics[key] ?? 0.5) * weight
  }
  let score = Math.round(clamp(total, 0, 1) * 100)
  if (item.must) score = Math.max(score, 96)
  const flags = {
    must: Boolean(item.must),
    ratingKnown: rating.known,
    transport: context.transport || form.transport,
  }
  const reasons = buildReasons(item, meta, flags)
  return {
    ...item,
    score,
    metrics,
    confidence: item.source === 'amap' ? 0.9 : 0.5,
    suggestedDuration: meta.duration,
    indoor: meta.indoor >= 0.6,
    reason: reasons.join('；'),
    caution: buildCaution(item, meta, form, flags, context),
  }
}

export function buildContext({ form = {}, weather = [], dates = [], month } = {}) {
  const tripWeather = (weather || []).filter((item) => !dates.length || dates.includes(item.date))
  const rainy = tripWeather.filter((item) => /雨|雪|雷/.test(String(item.dayWeather || ''))).length
  const rainShare = tripWeather.length ? rainy / tripWeather.length : 0.25
  const temps = tripWeather
    .map((item) => Number(item.dayTemp))
    .filter((value) => Number.isFinite(value))
  const avgTemp = temps.length ? temps.reduce((sum, value) => sum + value, 0) / temps.length : 20
  return {
    form,
    month: month || (Number(String(form.startDate || '').slice(5, 7)) || new Date().getMonth() + 1),
    rainShare,
    hot: avgTemp >= 32,
    cold: avgTemp <= 0,
    avgTemp,
    interests: form.interests || [],
    transport: form.transport,
    budgetLevel: form.budgetLevel,
    pace: form.pace,
  }
}

export function enrichCandidates(result, context) {
  const candidates = (result?.candidates || [])
    .filter((item) => item.kind !== 'hotel')
    .map((item) => scoreCandidate(item, context))
    .sort((a, b) => {
      if (a.must !== b.must) return a.must ? -1 : 1
      return b.score - a.score
    })
  const groups = []
  const counts = new Map()
  for (const item of candidates) {
    const meta = categoryMeta(item.categoryKey)
    if (!counts.has(meta.key)) {
      counts.set(meta.key, { key: meta.key, label: meta.label, icon: meta.icon, count: 0 })
    }
    counts.get(meta.key).count += 1
  }
  for (const meta of EXPLORE_CATEGORIES) {
    const group = counts.get(meta.key)
    if (group) groups.push(group)
  }
  const selected = candidates.filter((item) => item.must).map((item) => item.id)
  return {
    ...result,
    candidates,
    groups,
    selectedIds: selected,
    hotels: (result?.hotels || []).map((item) => ({ ...item, kind: 'hotel' })),
  }
}

/** 帮我挑：按天数和节奏凑够景点，类别不过度重复；餐饮尽量靠近已选景点。 */
export function autoPickCandidates(candidates, { form = {}, dayCount = 1 } = {}) {
  const sights = candidates.filter((item) => item.kind !== 'food' && item.kind !== 'hotel')
  const foods = candidates.filter((item) => item.kind === 'food')
  const perDay = form.pace === 'relaxed' ? 2 : form.pace === 'intense' ? 4 : 3
  const targetSights = Math.min(sights.length, Math.max(1, dayCount * perDay))
  const ids = []
  const used = new Set()
  const picked = []
  const categories = new Map()

  const take = (item) => {
    ids.push(item.id)
    used.add(item.id)
    picked.push(item)
    categories.set(item.categoryKey, (categories.get(item.categoryKey) || 0) + 1)
  }

  for (const item of sights.filter((entry) => entry.must).slice(0, 4)) {
    if (!used.has(item.id)) take(item)
  }

  while (picked.length < targetSights) {
    let best = null
    let bestScore = -1
    for (const item of sights) {
      if (used.has(item.id)) continue
      const repeated = categories.get(item.categoryKey) || 0
      const diversity = 1 - Math.min(0.25, repeated * 0.08)
      const nearest = picked.reduce(
        (min, entry) =>
          Math.min(min, haversineKm(entry, item)),
        Number.POSITIVE_INFINITY,
      )
      const cluster = !Number.isFinite(nearest) || nearest <= 35 ? 1 : nearest <= 60 ? 0.92 : 0.84
      const adjusted = item.score * diversity * cluster
      if (adjusted > bestScore) {
        bestScore = adjusted
        best = item
      }
    }
    if (!best) break
    take(best)
  }

  const foodTarget = Math.min(foods.length, Math.max(1, dayCount * (form.pace === 'relaxed' ? 1 : 2)))
  const nearby = (item) =>
    picked.length
      ? picked.reduce((min, entry) => Math.min(min, haversineKm(entry, item)), Number.POSITIVE_INFINITY) <= 8
      : true
  const rankedFood = [...foods].sort((a, b) => {
    const aNear = nearby(a) ? 1 : 0
    const bNear = nearby(b) ? 1 : 0
    if (aNear !== bNear) return bNear - aNear
    return b.score - a.score
  })
  for (const item of rankedFood) {
    if (ids.length >= targetSights + foodTarget) break
    if (!used.has(item.id)) take(item)
  }
  return ids
}

function pickPoi(item) {
  return {
    id: item.id || '',
    name: item.name || '',
    source: item.source || '',
    kind: item.kind || 'sight',
    category: item.category || '',
    address: item.address || '',
    city: item.city || '',
    lng: Number(item.lng) || 0,
    lat: Number(item.lat) || 0,
    rating: item.rating ?? null,
    cost: item.cost ?? null,
    tel: item.tel || '',
    distance: item.distance ?? null,
    tags: Array.isArray(item.tags) ? item.tags.slice(0, 8) : [],
  }
}

/** 把用户勾选的候选装配成既有流水线认识的事实包。 */
export function buildExploreFacts(result, selectedIds, form = {}) {
  const ids = new Set(selectedIds || [])
  const candidates = result?.candidates || []
  const selected = candidates.filter((item) => ids.has(item.id))
  let attractions = selected.filter((item) => item.kind !== 'food' && item.kind !== 'hotel')
  let food = selected.filter((item) => item.kind === 'food')
  const notes = [...(result?.notes || [])]
  if (!food.length) {
    food = candidates.filter((item) => item.kind === 'food').slice(0, 2)
    if (food.length) notes.push(`没有勾选餐饮，已自动补 ${food.length} 个高分备选，可在计划页替换。`)
  }
  if (!attractions.length) {
    attractions = candidates.filter((item) => item.kind !== 'food' && item.kind !== 'hotel').slice(0, 4)
    if (attractions.length) notes.push(`没有勾选景点，已按评分补 ${attractions.length} 个备选，保证能排出完整一天。`)
  }
  const hotels = (result?.hotels || []).map((item) => ({ ...pickPoi(item), kind: 'hotel' }))
  return {
    source: result?.source || 'osm',
    coordSystem: result?.coordSystem || 'gcj02',
    origin: result?.origin || form.origin,
    destination: result?.destination || form.destination,
    attractions: attractions.map(pickPoi),
    food: food.map(pickPoi),
    hotels,
    weather: result?.weather || [],
    notes: [...notes, '地点由你在「目的地候选」里挑选；勾选的才会按你的选择进入行程。'],
    collectedAt: result?.collectedAt || new Date().toISOString(),
  }
}

export function exploreStats(candidates) {
  const list = candidates || []
  const minutes = list.reduce((sum, item) => sum + (Number(item.suggestedDuration) || 90), 0)
  const categories = new Set(list.map((item) => item.categoryKey)).size
  return {
    count: list.length,
    hours: Math.round((minutes / 60) * 10) / 10,
    categories,
    indoor: list.filter((item) => item.indoor).length,
  }
}
