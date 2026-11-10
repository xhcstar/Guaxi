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

// 异地行程里「快到家了才吃午饭」是反常识安排，所以餐饮另设一道离家门槛。
// 只约束吃饭、不约束景点：出发前在家附近吃早饭、返程顺路买点东西都是合理的。
// 目的地离出发点不到 20 km 时视为本地行程，不设门槛。
export function mealGuardKm(target) {
  if (target?.mode === 'weekend' || target?.kind === 'weekend') return 0
  if (!target?.origin || !target?.destination) return 0
  if (!(distanceKm(target.origin, target.destination) > 20)) return 0
  return 2
}

// 单日在途上限：按白天的比例估，再压一个绝对上限。比例保证短日不硬塞，
// 上限保证长日不会被「12 小时里 6 小时在车上」放过。
export function dailyTransitLimitMinutes(target, dayWindowMinutes = 0) {
  const window = Number(dayWindowMinutes) > 0 ? Number(dayWindowMinutes) : 9 * 60
  const pace = target?.pace || 'balanced'
  const share = pace === 'relaxed' ? 0.35 : pace === 'intense' ? 0.65 : 0.5
  const cap =
    target?.transport === 'walking'
      ? 120
      : target?.transport === 'transit'
        ? 210
        : pace === 'relaxed'
          ? 150
          : 240
  return Math.max(90, Math.min(cap, Math.round(window * share)))
}

/**
 * 找出跨饭点的长路段：这类路段会把午饭拖到终点城市，或在起点附近随便吃一顿。
 * 判据是「这一段路本身压过饭点」，而不是看全天有没有排饭——后者会让模型随便塞一顿就算过关。
 * 返回路段索引、饭点类型与沿线采样比例（0-1），供沿途检索餐饮使用。
 */
export function mealStopsAlongLegs(legs = [], { startTime = '08:30' } = {}) {
  const toMinutes = (value) => {
    const [hours, minutes] = String(value || '').split(':').map(Number)
    return Number.isFinite(hours) && Number.isFinite(minutes) ? hours * 60 + minutes : null
  }
  const windows = [
    { key: 'lunch', label: '午餐', from: 11 * 60, to: 14 * 60 },
    { key: 'dinner', label: '晚餐', from: 17 * 60, to: 20 * 60 },
  ]
  let cursor = toMinutes(startTime)
  if (cursor == null) return []
  const found = []
  ;(legs || []).forEach((leg, index) => {
    const minutes = Math.max(0, Math.round((Number(leg?.duration) || 0) / 60))
    const depart = cursor
    const arrive = cursor + minutes
    cursor = arrive
    // 两条判据缺一不可：路段要够长，且真的压过饭点；短途市区移动不算。
    const km = Number(leg?.distance) || 0
    if (!(minutes >= 90 || km >= 80)) return
    for (const window of windows) {
      if (arrive <= window.from || depart >= window.to) continue
      const at = Math.min(Math.max(window.from, depart) + 30, arrive - 15)
      const ratio = minutes > 0 ? Math.min(0.9, Math.max(0.15, (at - depart) / minutes)) : 0.5
      found.push({ legIndex: index, meal: window.key, label: window.label, time: at, ratio })
      break
    }
  })
  return found
}

export function tooCloseToHome(poi, origin, minKm) {
  if (!(minKm > 0) || !origin || !poi) return false
  return distanceKm(origin, poi) < minKm
}

// 绕行里程：start → 各停留点 → end 的累计，减去「本来就要走的那段」。
// 本来就要走的那段：起终点不同时是两地直连；起终点相同（当天往返一个目的地）时，
// 是到最远一站的往返——去和回都躲不掉，只有站点之间的折返才算绕。
export function detourKm(stops = [], start = null, end = null) {
  const chain = [start, ...stops, end].filter(
    (poi) => poi && Number.isFinite(Number(poi.lat)) && Number.isFinite(Number(poi.lng)),
  )
  let total = 0
  for (let index = 1; index < chain.length; index += 1) {
    total += distanceKm(chain[index - 1], chain[index])
  }
  let baseline = 0
  if (start && end) {
    baseline = distanceKm(start, end)
    if (!(baseline > 1) && stops.length) {
      baseline =
        2 *
        Math.max(
          ...stops
            .filter((poi) => Number.isFinite(Number(poi.lat)) && Number.isFinite(Number(poi.lng)))
            .map((poi) => distanceKm(start, poi)),
        )
    }
  }
  const detour = total - (Number.isFinite(baseline) ? baseline : 0)
  return Number.isFinite(detour) ? Math.max(0, detour) : 0
}

// 把分钟上限换算成绕行里程：按交通方式的有效速度，再乘 1.3 的路网系数。
export function dailyDetourBudgetKm(target, dayWindowMinutes = 0) {
  const speed = target?.transport === 'walking' ? 4.5 : target?.transport === 'transit' ? 40 : 60
  return ((dailyTransitLimitMinutes(target, dayWindowMinutes) / 60) * speed) / 1.3
}

/**
 * 按绕行预算裁掉最不值的一站：同样一天，宁可少看一个点，也不要全程赶路。
 * 餐饮与住宿不删（删了会漏饭/漏住），必去点和评分最高的一站保留，至少留两站。
 */
export function pruneStopsByDetour(
  stops = [],
  { start = null, end = null, budgetKm = 0, keepIds = new Set() } = {},
) {
  let current = stops.filter(Boolean)
  if (!(budgetKm > 0) || !Number.isFinite(budgetKm) || current.length < 3) {
    return { stops: current, removed: [] }
  }
  const sightsOf = (list) => list.filter((poi) => poi?.kind !== 'food' && poi?.kind !== 'hotel')
  const protectedIds = new Set(keepIds)
  const sights = sightsOf(current)
  if (sights.length) {
    const best = sights.reduce((top, poi) => ((Number(poi.rating) || 0) > (Number(top.rating) || 0) ? poi : top), sights[0])
    if (best?.id) protectedIds.add(best.id)
  }

  const removed = []
  while (sightsOf(current).length > 2 && detourKm(current, start, end) > budgetKm) {
    let dropIndex = -1
    let bestSaving = 0
    current.forEach((poi, index) => {
      if (poi?.kind === 'food' || poi?.kind === 'hotel' || poi?.must) return
      if (poi?.id && protectedIds.has(poi.id)) return
      const next = current.filter((_, at) => at !== index)
      const saving = detourKm(current, start, end) - detourKm(next, start, end)
      if (saving > bestSaving) {
        bestSaving = saving
        dropIndex = index
      }
    })
    // 删掉任何一站都省不下路，说明预算卡不住这天的结构，交给提示说明即可。
    if (dropIndex < 0) break
    removed.push(current[dropIndex])
    current = current.filter((_, at) => at !== dropIndex)
  }
  return { stops: current, removed }
}
