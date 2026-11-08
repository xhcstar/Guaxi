// 行迹旅行计划的纯函数工具：日期、金额、并发、JSON 解析与转义。
// 不依赖 Vue / Tauri，方便在预览与 Node 测试里单独使用。

export const DAY_MS = 24 * 60 * 60 * 1000

export const pad2 = (value) => String(value).padStart(2, '0')

export function parseDate(value) {
  if (!value) return null
  const [year, month, day] = String(value).slice(0, 10).split('-').map(Number)
  if (!year || !month || !day) return null
  return new Date(year, month - 1, day, 12, 0, 0)
}

export function formatDate(date) {
  if (!date) return ''
  const d = date instanceof Date ? date : parseDate(date)
  if (!d) return ''
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
}

export function addDays(value, count) {
  const date = parseDate(value)
  if (!date) return ''
  date.setDate(date.getDate() + count)
  return formatDate(date)
}

export function enumerateDates(start, end) {
  const startDate = parseDate(start)
  const endDate = parseDate(end)
  if (!startDate) return []
  const last = endDate && endDate >= startDate ? endDate : startDate
  const dates = []
  const cursor = new Date(startDate)
  let guard = 0
  while (cursor <= last && guard < 31) {
    dates.push(formatDate(cursor))
    cursor.setDate(cursor.getDate() + 1)
    guard += 1
  }
  return dates
}

export function weekdayCn(value) {
  const date = parseDate(value)
  if (!date) return ''
  return ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][date.getDay()]
}

export function shortDate(value) {
  const date = parseDate(value)
  if (!date) return ''
  return `${date.getMonth() + 1}月${date.getDate()}日`
}

/** 把 YYYY-MM-DD 或 ISO 时间转成中文短日期；其它文案原样返回。 */
export function friendlyDate(value) {
  const text = String(value || '').trim()
  return /^\d{4}-\d{2}-\d{2}/.test(text) ? shortDate(text.slice(0, 10)) : text
}

export function diffDays(a, b) {
  const start = parseDate(a)
  const end = parseDate(b)
  if (!start || !end) return 0
  return Math.round((end - start) / DAY_MS)
}

export function todayString() {
  return formatDate(new Date())
}

export function nextSaturday(from = new Date()) {
  const date = new Date(from)
  date.setHours(12, 0, 0, 0)
  const day = date.getDay()
  const delta = (6 - day + 7) % 7
  date.setDate(date.getDate() + (delta === 0 ? 0 : delta))
  return formatDate(date)
}

export function timeToMinutes(value) {
  const [hours, minutes] = String(value || '')
    .split(':')
    .map(Number)
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) return null
  return hours * 60 + minutes
}

export function minutesToTime(total) {
  const safe = Math.max(0, Math.min(24 * 60 - 1, Math.round(total)))
  return `${pad2(Math.floor(safe / 60))}:${pad2(safe % 60)}`
}

export function addMinutes(value, minutes) {
  const base = timeToMinutes(value)
  if (base == null) return value
  return minutesToTime(base + minutes)
}

export function normalizeTime(value, fallback = '09:00') {
  const raw = String(value || '').trim()
  const match = raw.match(/^(\d{1,2}):(\d{2})/)
  if (!match) return fallback
  const hours = Number(match[1])
  const minutes = Number(match[2])
  if (hours > 23 || minutes > 59) return fallback
  return `${pad2(hours)}:${pad2(minutes)}`
}

export function durationText(seconds) {
  const total = Number(seconds) || 0
  if (total <= 0) return ''
  const minutes = Math.round(total / 60)
  if (minutes < 60) return `${minutes} 分钟`
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  return rest ? `${hours} 小时 ${rest} 分` : `${hours} 小时`
}

export function distanceText(km) {
  const value = Number(km) || 0
  if (value <= 0) return ''
  return value < 10 ? `${value.toFixed(1)} km` : `${Math.round(value)} km`
}

export function money(value) {
  const number = Number(value) || 0
  return number.toLocaleString('zh-CN', {
    maximumFractionDigits: number % 1 === 0 ? 0 : 1,
  })
}

export function uid(prefix = 'trp') {
  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`
}

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value))
}

export async function runWithConcurrency(items, limit, worker) {
  const results = new Array(items.length)
  let cursor = 0
  const runners = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (cursor < items.length) {
      const index = cursor
      cursor += 1
      results[index] = await worker(items[index], index)
    }
  })
  await Promise.all(runners)
  return results
}

/** 模型经常把 JSON 包在 ```json 里或在前后加解释，这里取第一个完整对象。 */
export function extractJson(text) {
  if (!text || typeof text !== 'string') {
    throw new Error('模型没有返回内容')
  }
  let source = text.trim()
  source = source.replace(/^```(?:json)?\s*/i, '').replace(/```\s*$/i, '').trim()
  const start = source.indexOf('{')
  const end = source.lastIndexOf('}')
  if (start === -1 || end <= start) {
    throw new Error('模型返回的内容不是 JSON')
  }
  source = source.slice(start, end + 1)
  return JSON.parse(source)
}

export function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

export function safeText(value, fallback = '') {
  if (typeof value === 'string') return value.trim()
  if (typeof value === 'number') return String(value)
  if (Array.isArray(value)) return value.filter((item) => typeof item === 'string').join('')
  return fallback
}

export function uniqueBy(items, keyFn) {
  const seen = new Set()
  return items.filter((item) => {
    const key = keyFn(item)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

export function deepClone(value) {
  return JSON.parse(JSON.stringify(value))
}

// ── 坐标系转换 ─────────────────────────────────────────────
// 国内地图服务分别使用 WGS84（OSM/Google）、GCJ-02（高德/腾讯）和 BD-09（百度）。
// 计划内统一尽量保留数据源坐标；跨服务商打开导航/地图时按需转换。

const PI = Math.PI
const AXIS = 6378245.0
const ECCENTRICITY = 0.00669342162296594323

function outOfChina(lng, lat) {
  return lng < 72.004 || lng > 137.8347 || lat < 0.8293 || lat > 55.8271
}

function transformLat(lng, lat) {
  let ret = -100.0 + 2.0 * lng + 3.0 * lat + 0.2 * lat * lat + 0.1 * lng * lat + 0.2 * Math.sqrt(Math.abs(lng))
  ret += ((20.0 * Math.sin(6.0 * lng * PI) + 20.0 * Math.sin(2.0 * lng * PI)) * 2.0) / 3.0
  ret += ((20.0 * Math.sin(lat * PI) + 40.0 * Math.sin((lat / 3.0) * PI)) * 2.0) / 3.0
  ret += ((160.0 * Math.sin((lat / 12.0) * PI) + 320 * Math.sin((lat * PI) / 30.0)) * 2.0) / 3.0
  return ret
}

function transformLng(lng, lat) {
  let ret = 300.0 + lng + 2.0 * lat + 0.1 * lng * lng + 0.1 * lng * lat + 0.1 * Math.sqrt(Math.abs(lng))
  ret += ((20.0 * Math.sin(6.0 * lng * PI) + 20.0 * Math.sin(2.0 * lng * PI)) * 2.0) / 3.0
  ret += ((20.0 * Math.sin(lng * PI) + 40.0 * Math.sin((lng / 3.0) * PI)) * 2.0) / 3.0
  ret += ((150.0 * Math.sin((lng / 12.0) * PI) + 300.0 * Math.sin((lng / 30.0) * PI)) * 2.0) / 3.0
  return ret
}

export function wgs84ToGcj02(lng, lat) {
  if (outOfChina(lng, lat)) return [lng, lat]
  let dLat = transformLat(lng - 105.0, lat - 35.0)
  let dLng = transformLng(lng - 105.0, lat - 35.0)
  const radLat = (lat / 180.0) * PI
  let magic = Math.sin(radLat)
  magic = 1 - ECCENTRICITY * magic * magic
  const sqrtMagic = Math.sqrt(magic)
  dLat = (dLat * 180.0) / (((AXIS * (1 - ECCENTRICITY)) / (magic * sqrtMagic)) * PI)
  dLng = (dLng * 180.0) / ((AXIS / sqrtMagic) * Math.cos(radLat) * PI)
  return [lng + dLng, lat + dLat]
}

export function gcj02ToWgs84(lng, lat) {
  if (outOfChina(lng, lat)) return [lng, lat]
  const [gLng, gLat] = wgs84ToGcj02(lng, lat)
  return [lng * 2 - gLng, lat * 2 - gLat]
}

export function gcj02ToBd09(lng, lat) {
  const z = Math.sqrt(lng * lng + lat * lat) + 0.00002 * Math.sin(lat * PI * 3000.0 / 180.0)
  const theta = Math.atan2(lat, lng) + 0.000003 * Math.cos(lng * PI * 3000.0 / 180.0)
  return [z * Math.cos(theta) + 0.0065, z * Math.sin(theta) + 0.006]
}

export function bd09ToGcj02(lng, lat) {
  const x = lng - 0.0065
  const y = lat - 0.006
  const z = Math.sqrt(x * x + y * y) - 0.00002 * Math.sin(y * PI * 3000.0 / 180.0)
  const theta = Math.atan2(y, x) - 0.000003 * Math.cos(x * PI * 3000.0 / 180.0)
  return [z * Math.cos(theta), z * Math.sin(theta)]
}

export function bd09ToWgs84(lng, lat) {
  const [gLng, gLat] = bd09ToGcj02(lng, lat)
  return gcj02ToWgs84(gLng, gLat)
}

