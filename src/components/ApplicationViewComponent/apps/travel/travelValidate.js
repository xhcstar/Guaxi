// 行程硬约束校验：地点必须来自候选池，且落在用户要求的半径、日期与出发时间内。
// 模型输出先过这里，不过关就退回确定性编排；装配完成后再整体校验一次，避免把错误计划交给用户。

import {
  dailyTransitLimitMinutes,
  distanceKm,
  homeGuardKm,
  mealGuardKm,
  tooCloseToHome,
} from './travelEngine.js'
import { haversineKm, minutesToTime, parsePolyline, timeToMinutes } from './travelUtils.js'

const POI_KINDS = new Set(['sight', 'food', 'hotel'])
// 后端按「半径 + 500 m」裁剪候选，这里用同一口径，避免边界点误判。
const RADIUS_SLACK_KM = 0.5
const START_SLACK_MINUTES = 30
// 收尾可以稍有弹性，排过半小时才算真超时。
const END_SLACK_MINUTES = 30

function distanceText(km) {
  return km >= 10 ? km.toFixed(0) : km.toFixed(1)
}

// 当天的真实行驶折线：把所有带 route 的 transit 块按 legIndex 串起来。
function routePolyline(day) {
  const legs = (day?.blocks || [])
    .filter((block) => block?.kind === 'transit' && block?.route)
    .sort((a, b) => (a.legIndex ?? 0) - (b.legIndex ?? 0))
  const points = []
  const push = (lng, lat) => {
    if (!Number.isFinite(lng) || !Number.isFinite(lat)) return
    const last = points[points.length - 1]
    if (last && Math.abs(last.lng - lng) < 1e-5 && Math.abs(last.lat - lat) < 1e-5) return
    points.push({ lng, lat })
  }
  for (const block of legs) {
    const coords = parsePolyline(
      block.route.polyline,
      { lng: block.route.fromLng, lat: block.route.fromLat },
      { lng: block.route.toLng, lat: block.route.toLat },
    )
    for (const [lng, lat] of coords) push(lng, lat)
  }
  return points
}

// 点到折线的最短距离：顺不顺路取决于「离这条线多近」，与离景点多远无关。
function distanceToPolylineKm(poi, points) {
  if (!poi || points.length < 2) return Infinity
  let best = Infinity
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1]
    const to = points[index]
    // 用两端点到目标点的距离近似该线段：折线点足够密，误差远小于 3 km 判定阈值。
    best = Math.min(best, distanceKm(from, poi), distanceKm(to, poi))
  }
  return best
}

export function validateDayBlocks(
  day,
  {
    center,
    radiusKm = 0,
    notBeforeMinutes = null,
    endMinutes = null,
    allowHotel = true,
    home = null,
    homeGuard = 0,
    mealGuard = 0,
    transitLimitMinutes = 0,
    requireLunch = false,
    requireDinner = false,
    requireHotel = false,
    advisories = null,
  } = {},
) {
  const problems = []
  // 产线类问题（收尾偏晚、当天在途过长）走这里：编排单天时没有收集数组，按硬约束退回
  // 确定性编排；整份计划复核时传入 advisories，只作页面横幅提示，不作废整份计划。
  const soft = (text) => {
    if (Array.isArray(advisories)) advisories.push(text)
    else problems.push(text)
  }
  const blocks = Array.isArray(day?.blocks) ? day.blocks : []
  if (!blocks.length) {
    problems.push('没有任何安排')
    return problems
  }

  if (day?.weather && day.weather.date !== day?.date) {
    problems.push('天气日期与行程日期不一致')
  }

  const foodBlocks = []
  const otherPois = []
  const overlapsMeal = (block, from, to) => {
    const start = timeToMinutes(block?.start)
    const end = timeToMinutes(block?.end)
    return start != null && end != null && start < to && end > from
  }
  const hasMeal = (from, to) =>
    blocks.some((block) => block?.kind === 'food' && overlapsMeal(block, from, to))
  // 只有时间线里真的腾得出 40 分钟，才算“漏排了饭”；全程在路上/排满时不能强求。
  const mealGap = (from, to) => {
    const sorted = [...blocks].sort(
      (a, b) => (timeToMinutes(a?.start) ?? 0) - (timeToMinutes(b?.start) ?? 0),
    )
    let cursor = from
    for (const block of sorted) {
      const start = timeToMinutes(block?.start)
      const end = timeToMinutes(block?.end)
      if (start == null || end == null) continue
      if (start > cursor && Math.min(start, to) - Math.max(cursor, from) >= 40) return true
      cursor = Math.max(cursor, end)
      if (cursor >= to) return false
    }
    return to - Math.max(cursor, from) >= 40
  }
  if (requireLunch && !hasMeal(11 * 60, 15 * 60) && mealGap(11 * 60, 15 * 60)) {
    problems.push('时间跨过午饭时段，但没有午餐安排')
  }
  if (requireDinner && !hasMeal(17 * 60, 20 * 60 + 30) && mealGap(17 * 60, 20 * 60 + 30)) {
    problems.push('时间跨过晚饭时段，但没有晚餐安排')
  }
  if (requireHotel && !blocks.some((block) => block?.kind === 'hotel')) {
    problems.push('当天需要过夜，但没有住宿安排')
  }

  const first = blocks[0]
  if (notBeforeMinutes != null) {
    const start = timeToMinutes(first?.start)
    if (start != null && start < notBeforeMinutes - START_SLACK_MINUTES) {
      problems.push(`「${first.title || '第一站'}」早于出发时间 ${first.start}`)
    }
  }

  // 收尾只看游览/用餐，不看最后返程：结束时间是「当天活动结束」，回家路上的时间不算超时。
  if (endMinutes != null) {
    const lastActivity = [...blocks]
      .filter((block) => block?.kind !== 'transit' && block?.kind !== 'rest')
      .map((block) => ({ block, end: timeToMinutes(block?.end) }))
      .filter((item) => item.end != null)
      .sort((a, b) => b.end - a.end)[0]
    if (lastActivity && lastActivity.end > endMinutes + END_SLACK_MINUTES) {
      soft(
        `「${lastActivity.block.title || '最后一项安排'}」结束于 ${lastActivity.block.end}，晚于每天结束时间 ${minutesToTime(endMinutes)}`,
      )
    }
  }

  // 单日全在车上就没有游览价值：超过在途上限说明这一天几乎都在赶路。
  if (transitLimitMinutes > 0) {
    const transitMinutes = blocks
      .filter((block) => block?.kind === 'transit')
      .reduce((sum, block) => sum + (Number(block?.route?.duration) || 0) / 60, 0)
    if (transitMinutes > transitLimitMinutes) {
      soft(
        `当天在途约 ${Math.round(transitMinutes)} 分钟，超过 ${Math.round(transitLimitMinutes)} 分钟上限，建议减少一处目的地或拉长日期`,
      )
    }
  }

  let previousEnd = null
  blocks.forEach((block) => {
    const label = block?.title || '未命名安排'
    const start = timeToMinutes(block?.start)
    const end = timeToMinutes(block?.end)
    if (start == null || end == null) {
      problems.push(`「${label}」时间格式无效`)
      return
    }
    if (end <= start) problems.push(`「${label}」结束时间不晚于开始时间`)
    if (previousEnd != null && start < previousEnd) {
      problems.push(`「${label}」与上一段安排时间重叠`)
    }
    previousEnd = end

    if (block?.kind === 'hotel' && !allowHotel) {
      problems.push(`「${label}」是住宿安排，但这次是当天往返`)
      return
    }

    if (block?.kind === 'transit') {
      const km = Number(block.route?.distance)
      const seconds = Number(block.route?.duration)
      if (block.route && (!(km > 0) || !(seconds > 0))) {
        problems.push(`「${label}」缺少有效的路线距离或耗时`)
      } else if (block.route) {
        const speed = km / (seconds / 3600)
        if (speed > 160) {
          problems.push(
            `「${label}」的路线耗时与距离对不上（${km.toFixed(1)} km / ${Math.round(seconds / 60)} 分钟）`,
          )
        }
      } else if (block.legIndex != null) {
        problems.push(`「${label}」没有匹配到路线`)
      }
      return
    }

    if (POI_KINDS.has(block?.kind) && !block?.poi && !block?.freeMeal && !block?.freeHotel) {
      problems.push(`「${label}」不是候选地点里的地点`)
      return
    }
    const poi = block?.poi
    if (!poi) return
    if (block.kind === 'food') foodBlocks.push({ label, poi })
    else otherPois.push(poi)
    if (homeGuard > 0 && tooCloseToHome(poi, home, homeGuard)) {
      problems.push(
        `「${poi.name || label}」离出发点不到 ${Math.round(homeGuard)} km，不像出游安排`,
      )
    }
    // 异地行程的餐饮另设离家门槛：快到家了才吃饭，通常是返程顺手排的，没有出游意义。
    // 只看午/晚饭时段，出发前在楼下吃早饭这类安排不算问题。
    if (
      block.kind === 'food' &&
      mealGuard > 0 &&
      tooCloseToHome(poi, home, mealGuard) &&
      (overlapsMeal(block, 11 * 60, 15 * 60) || overlapsMeal(block, 17 * 60, 21 * 60))
    ) {
      problems.push(
        `「${poi.name || label}」离出发点不到 ${Math.round(mealGuard)} km，像是快到家才吃饭`,
      )
    }
    if (!center || !(radiusKm > 0)) return
    const km = haversineKm(center, poi)
    if (!Number.isFinite(km)) {
      problems.push(`「${poi.name || label}」缺少坐标，无法核对距离`)
      return
    }
    if (km > radiusKm + RADIUS_SLACK_KM) {
      problems.push(
        `「${poi.name || label}」距检索中心 ${distanceText(km)} km，超出 ${radiusKm} km 范围`,
      )
    }
  })

  // 午餐/晚餐要落在当天行程附近，不能人在 30 km 外却回家门口吃饭。
  // 沿途餐馆例外：它本来就该离当天景区很远（在跨城路段中间），顺不顺路要看
  // 它离当天路线折线有多近，而不是离哪个景点有多近。
  if (foodBlocks.length && otherPois.length) {
    const route = routePolyline(day)
    for (const { label, poi } of foodBlocks) {
      const detour = poi.alongTime && route.length >= 2 ? distanceToPolylineKm(poi, route) : null
      if (detour != null && detour <= 3) continue
      const nearest = Math.min(...otherPois.map((other) => distanceKm(other, poi)))
      if (Number.isFinite(nearest) && nearest > 12) {
        problems.push(`「${poi.name || label}」离当天其他安排超过 12 km，用餐不顺路`)
      }
    }
  }

  return problems
}

/**
 * 预算核对：晚数按日期算，当天往返不允许有住宿；合计必须等于明细之和。
 */
export function validateBudget(plan) {
  const problems = []
  const budget = plan?.budget
  if (!budget || !Array.isArray(budget.items)) return ['缺少预算明细']
  const days = Array.isArray(plan?.days) ? plan.days : []
  const sameDay = !plan?.startDate || !plan?.endDate || plan.startDate === plan.endDate
  const expectedNights = sameDay || days.length < 2 ? 0 : days.length - 1

  if (Number(budget.nights) !== expectedNights) {
    problems.push(`住宿晚数应为 ${expectedNights}，实际 ${budget.nights ?? 0}`)
  }
  const sum = budget.items.reduce((acc, item) => acc + (Number(item?.amount) || 0), 0)
  if (Math.abs(sum - (Number(budget.total) || 0)) > 1) {
    problems.push(`预算合计与明细不一致（明细 ¥${Math.round(sum)}，合计 ¥${budget.total}`)
  }
  if (expectedNights === 0) {
    const lodging = budget.items.find(
      (item) => item?.category === '住宿' && Number(item?.amount) > 0,
    )
    if (lodging) problems.push('当天往返不应计入住宿费用')
  }
  return problems
}

export function validatePlan(plan, { center, radiusKm = 0, expectedDates = [] } = {}) {
  const problems = []
  const advisories = []
  const days = Array.isArray(plan?.days) ? plan.days : []
  if (!days.length) problems.push('计划里没有任何一天的安排')
  if (expectedDates.length && days.length !== expectedDates.length) {
    problems.push(`天数不符：应为 ${expectedDates.length} 天，实际 ${days.length} 天`)
  }

  const sameDay = !plan?.startDate || !plan?.endDate || plan.startDate === plan.endDate
  const allowHotel = !sameDay && days.length > 1
  const notBeforeMinutes = timeToMinutes(plan?.startTime)
  const homeGuard = homeGuardKm(plan)
  const mealGuard = mealGuardKm(plan)
  const endMinutes = timeToMinutes(plan?.endTime) ?? 0
  const dayWindowMinutes =
    notBeforeMinutes != null && endMinutes > notBeforeMinutes ? endMinutes - notBeforeMinutes : 0
  const transitLimitMinutes = dailyTransitLimitMinutes(plan, dayWindowMinutes)
  const requireLunch = endMinutes >= 13 * 60
  const requireDinner = endMinutes >= 19 * 60
  days.forEach((day, index) => {
    const prefix = `第 ${index + 1} 天`
    if (expectedDates[index] && day?.date !== expectedDates[index]) {
      problems.push(`${prefix}日期应为 ${expectedDates[index]}，实际 ${day?.date || '缺失'}`)
    }
    const soft = []
    validateDayBlocks(day, {
      center,
      radiusKm,
      notBeforeMinutes,
      endMinutes,
      allowHotel,
      home: plan?.origin,
      homeGuard,
      mealGuard,
      transitLimitMinutes,
      requireLunch,
      requireDinner,
      requireHotel: !sameDay && index < days.length - 1,
      advisories: soft,
    }).forEach((item) => {
      problems.push(`${prefix}：${item}`)
    })
    soft.forEach((item) => advisories.push(`${prefix}：${item}`))
  })

  validateBudget(plan).forEach((item) => problems.push(`预算：${item}`))

  return { ok: problems.length === 0, problems, advisories }
}

export function formatProblems(problems, limit = 2) {
  const list = problems.slice(0, limit)
  const rest = problems.length - list.length
  return `${list.join('；')}${rest > 0 ? ` 等 ${problems.length} 项` : ''}`
}
