// 行程合理性引擎：就近排序、把午餐锚定在当天位置附近、拒绝家门口的“景点”。
// 只做与数据无关的几何/距离判断，不编造地点和价格。

import { haversineKm } from './travelUtils.js'

export function distanceKm(a, b) {
  const km = haversineKm(a, b)
  return Number.isFinite(km) ? km : Infinity
}

// 最近邻排序：从 anchor 出发依次取最近的未访问点，减少折返。
export function orderSights(sights = [], anchor) {
  const left = sights.filter((poi) => Number.isFinite(Number(poi?.lat)) && Number.isFinite(Number(poi?.lng)))
  const passthrough = sights.filter((poi) => !left.includes(poi))
  if (!anchor || left.length < 2) return [...sights]
  const out = []
  let current = anchor
  const remaining = [...left]
  while (remaining.length) {
    let bestIndex = 0
    let bestKm = Infinity
    remaining.forEach((poi, index) => {
      const km = distanceKm(current, poi)
      if (km < bestKm) {
        bestKm = km
        bestIndex = index
      }
    })
    const [next] = remaining.splice(bestIndex, 1)
    out.push(next)
    current = next
  }
  return [...out, ...passthrough]
}

// 选离 anchor 最近的餐饮/住宿；超出 maxKm 就返回 null，宁可不推荐也不让人折返几十公里。
export function pickNearAnchor(candidates = [], anchor, { maxKm = 10, exclude = new Set(), kind = '' } = {}) {
  if (!anchor) return null
  let best = null
  let bestKm = Infinity
  for (const poi of candidates) {
    if (!poi || !Number.isFinite(Number(poi.lat)) || !Number.isFinite(Number(poi.lng))) continue
    if (exclude.has(poi.id)) continue
    if (kind && poi.kind && poi.kind !== kind) continue
    const km = distanceKm(anchor, poi)
    if (km < bestKm) {
      bestKm = km
      best = poi
    }
  }
  return best && bestKm <= maxKm ? best : null
}

// 周末自驾/坐车出去，家门口的公园不该当景点或用餐点。
export function homeGuardKm(target) {
  const weekend = target?.mode === 'weekend' || target?.kind === 'weekend'
  if (!weekend) return 0
  const radius = Number(target?.radiusKm) || 60
  if (target?.transport === 'driving') return Math.min(10, Math.max(5, radius * 0.1))
  if (target?.transport === 'transit') return Math.min(6, Math.max(3, radius * 0.08))
  return 0
}

export function tooCloseToHome(poi, origin, minKm) {
  if (!(minKm > 0) || !origin || !poi) return false
  return distanceKm(origin, poi) < minKm
}
