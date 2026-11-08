// 行迹个人偏好：默认值 + 本地学习。
// 偏好只存在本机；这里不包含任何上传逻辑。

import { clamp } from './travelUtils.js'

export const DEFAULT_PREFERENCES = {
  home: null,
  lastOrigin: null,
  hasCar: true,
  car: { brand: '', model: '', energy: 'gas' },
  defaultTransport: '',
  defaultBudgetLevel: '',
  defaultPace: '',
  adults: 2,
  children: 0,
  elders: 0,
  interests: [],
  avoid: '',
  weekendRadiusKm: 60,
  autoLearn: true,
  mapProvider: 'auto',
  aiSource: 'api',
  webProvider: 'deepseek',
  learned: {
    transport: {},
    budget: {},
    pace: {},
    interests: {},
    origins: [],
    destinations: [],
  },
}

export const WEB_AI_PROVIDERS = [
  { value: 'chatgpt', label: 'ChatGPT' },
  { value: 'deepseek', label: 'DeepSeek' },
  { value: 'kimi', label: 'Kimi' },
  { value: 'qwen', label: '通义千问' },
  { value: 'doubao', label: '豆包' },
  { value: 'yuanbao', label: '腾讯元宝' },
]

export const MAP_PROVIDERS = [
  { value: 'auto', label: '自动' },
  { value: 'amap', label: '高德' },
  { value: 'baidu', label: '百度' },
  { value: 'tencent', label: '腾讯' },
  { value: 'google', label: 'Google' },
  { value: 'mapbox', label: 'Mapbox' },
  { value: 'osm', label: 'OpenStreetMap' },
]

export function mapProviderLabel(value) {
  return MAP_PROVIDERS.find((item) => item.value === value)?.label || value
}

export function webProviderLabel(value) {
  return WEB_AI_PROVIDERS.find((item) => item.value === value)?.label || value
}

function place(value) {
  if (!value || typeof value !== 'object') return null
  const name = String(value.name || '').trim()
  if (!name) return null
  return {
    name,
    address: String(value.address || ''),
    city: String(value.city || ''),
    lng: Number(value.lng) || 0,
    lat: Number(value.lat) || 0,
    adcode: String(value.adcode || ''),
    cityCode: String(value.cityCode || value.city_code || ''),
    source: String(value.source || ''),
  }
}

function counts(value) {
  if (!value || typeof value !== 'object') return {}
  const out = {}
  for (const [key, count] of Object.entries(value)) {
    const number = Number(count)
    if (key && Number.isFinite(number) && number > 0) out[key] = Math.min(999, number)
  }
  return out
}

export function mergePreferences(raw = {}) {
  const source = raw && typeof raw === 'object' ? raw : {}
  const learned = source.learned && typeof source.learned === 'object' ? source.learned : {}
  const car = source.car && typeof source.car === 'object' ? source.car : {}
  return {
    home: place(source.home),
    lastOrigin: place(source.lastOrigin),
    hasCar: source.hasCar !== false,
    car: {
      brand: String(car.brand || ''),
      model: String(car.model || ''),
      energy: ['gas', 'ev', 'hybrid'].includes(car.energy) ? car.energy : 'gas',
    },
    defaultTransport: String(source.defaultTransport || ''),
    defaultBudgetLevel: String(source.defaultBudgetLevel || ''),
    defaultPace: String(source.defaultPace || ''),
    adults: clamp(Number(source.adults) || 2, 1, 20),
    children: clamp(Number(source.children) || 0, 0, 20),
    elders: clamp(Number(source.elders) || 0, 0, 20),
    interests: Array.isArray(source.interests)
      ? source.interests.map((item) => String(item)).filter(Boolean).slice(0, 12)
      : [],
    avoid: String(source.avoid || ''),
    weekendRadiusKm: clamp(Number(source.weekendRadiusKm) || 60, 10, 200),
    autoLearn: source.autoLearn !== false,
    mapProvider: ['auto', 'amap', 'baidu', 'tencent', 'google', 'mapbox', 'osm'].includes(
      source.mapProvider,
    )
      ? source.mapProvider
      : 'auto',
    aiSource: source.aiSource === 'web' ? 'web' : 'api',
    webProvider: WEB_AI_PROVIDERS.some((item) => item.value === source.webProvider)
      ? source.webProvider
      : 'deepseek',
    learned: {
      transport: counts(learned.transport),
      budget: counts(learned.budget),
      pace: counts(learned.pace),
      interests: counts(learned.interests),
      origins: Array.isArray(learned.origins)
        ? learned.origins.map(place).filter(Boolean).slice(0, 10)
        : [],
      destinations: Array.isArray(learned.destinations)
        ? learned.destinations
            .map((item) => (item && item.name ? { name: String(item.name), city: String(item.city || ''), at: String(item.at || '') } : null))
            .filter(Boolean)
            .slice(0, 10)
        : [],
    },
  }
}

export function learnedTop(learned, key, minCount = 1) {
  const entries = Object.entries(learned?.[key] || {})
    .filter(([, count]) => count >= minCount)
    .sort((a, b) => b[1] - a[1])
  return entries[0]?.[0] || ''
}

export function learnedInterests(prefs, limit = 4) {
  return Object.entries(prefs?.learned?.interests || {})
    .sort((a, b) => b[1] - a[1])
    .map(([name]) => name)
    .slice(0, limit)
}

export function effectiveDefaults(prefs) {
  const safe = mergePreferences(prefs)
  return {
    transport: safe.defaultTransport || learnedTop(safe.learned, 'transport') || 'driving',
    budgetLevel: safe.defaultBudgetLevel || learnedTop(safe.learned, 'budget') || 'comfort',
    pace: safe.defaultPace || learnedTop(safe.learned, 'pace') || 'balanced',
    interests: safe.interests.length ? safe.interests : learnedInterests(safe, 4),
    weekendRadiusKm: safe.weekendRadiusKm,
  }
}

export function applyPreferencesToForm(form, preferences) {
  const prefs = mergePreferences(preferences)
  const defaults = effectiveDefaults(prefs)
  let transport = defaults.transport
  if (!prefs.hasCar && transport === 'driving') transport = 'transit'
  return {
    ...form,
    origin: prefs.home || prefs.lastOrigin || form.origin,
    transport,
    budgetLevel: defaults.budgetLevel,
    pace: defaults.pace,
    adults: prefs.adults,
    children: prefs.children,
    elders: prefs.elders,
    interests: defaults.interests.length ? [...defaults.interests] : [...(form.interests || [])],
    avoid: prefs.avoid || form.avoid,
    radiusKm: prefs.weekendRadiusKm || form.radiusKm,
    mapProvider: prefs.mapProvider || form.mapProvider || 'auto',
  }
}

function increment(map, key) {
  if (!key) return
  map[key] = Math.min(999, (Number(map[key]) || 0) + 1)
}

export function learnFromForm(preferences, form) {
  if (!preferences?.autoLearn) return mergePreferences(preferences)
  const next = mergePreferences(preferences)
  increment(next.learned.transport, form.transport)
  increment(next.learned.budget, form.budgetLevel)
  increment(next.learned.pace, form.pace)
  for (const interest of form.interests || []) increment(next.learned.interests, interest)
  if (form.origin?.name) {
    next.lastOrigin = place(form.origin)
    const origins = next.learned.origins.filter((item) => item.name !== form.origin.name)
    origins.unshift(place(form.origin))
    next.learned.origins = origins.slice(0, 10)
  }
  if (form.mode !== 'weekend' && form.destination?.name) {
    const destinations = next.learned.destinations.filter(
      (item) => item.name !== form.destination.name,
    )
    destinations.unshift({
      name: String(form.destination.name),
      city: String(form.destination.city || ''),
      at: new Date().toISOString(),
    })
    next.learned.destinations = destinations.slice(0, 10)
  }
  return next
}

export function learnedSummary(preferences) {
  const prefs = mergePreferences(preferences)
  const defaults = effectiveDefaults(prefs)
  const transportLabel =
    { driving: '自驾', transit: '公共交通', tour: '跟团', walking: '散步' }[defaults.transport] ||
    defaults.transport
  const budgetLabel =
    { economy: '省着花', comfort: '舒适', premium: '不将就' }[defaults.budgetLevel] ||
    defaults.budgetLevel
  const paceLabel =
    { relaxed: '松弛', balanced: '均衡', intense: '紧凑' }[defaults.pace] || defaults.pace
  return {
    transport: transportLabel,
    budget: budgetLabel,
    pace: paceLabel,
    interests: defaults.interests,
    trips: Object.values(prefs.learned.transport).reduce((sum, count) => sum + count, 0),
  }
}
