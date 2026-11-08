// 行程硬约束校验：地点必须来自候选池，且落在用户要求的半径、日期与出发时间内。
// 模型输出先过这里，不过关就退回确定性编排；装配完成后再整体校验一次，避免把错误计划交给用户。

import { haversineKm, timeToMinutes } from './travelUtils.js'

const POI_KINDS = new Set(['sight', 'food', 'hotel'])
// 后端按「半径 + 500 m」裁剪候选，这里用同一口径，避免边界点误判。
const RADIUS_SLACK_KM = 0.5
const START_SLACK_MINUTES = 30

function distanceText(km) {
  return km >= 10 ? km.toFixed(0) : km.toFixed(1)
}

export function validateDayBlocks(
  day,
  { center, radiusKm = 0, notBeforeMinutes = null, allowHotel = true } = {},
) {
  const problems = []
  const blocks = Array.isArray(day?.blocks) ? day.blocks : []
  if (!blocks.length) {
    problems.push('没有任何安排')
    return problems
  }

  if (day?.weather && day.weather.date !== day?.date) {
    problems.push('天气日期与行程日期不一致')
  }

  const first = blocks[0]
  if (notBeforeMinutes != null) {
    const start = timeToMinutes(first?.start)
    if (start != null && start < notBeforeMinutes - START_SLACK_MINUTES) {
      problems.push(`「${first.title || '第一站'}」早于出发时间 ${first.start}`)
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

    if (POI_KINDS.has(block?.kind) && !block?.poi) {
      problems.push(`「${label}」不是候选地点里的地点`)
      return
    }
    const poi = block?.poi
    if (!poi || !center || !(radiusKm > 0)) return
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
  const days = Array.isArray(plan?.days) ? plan.days : []
  if (!days.length) problems.push('计划里没有任何一天的安排')
  if (expectedDates.length && days.length !== expectedDates.length) {
    problems.push(`天数不符：应为 ${expectedDates.length} 天，实际 ${days.length} 天`)
  }

  const sameDay = !plan?.startDate || !plan?.endDate || plan.startDate === plan.endDate
  const allowHotel = !sameDay && days.length > 1
  const notBeforeMinutes = timeToMinutes(plan?.startTime)
  days.forEach((day, index) => {
    const prefix = `第 ${index + 1} 天`
    if (expectedDates[index] && day?.date !== expectedDates[index]) {
      problems.push(`${prefix}日期应为 ${expectedDates[index]}，实际 ${day?.date || '缺失'}`)
    }
    validateDayBlocks(day, { center, radiusKm, notBeforeMinutes, allowHotel }).forEach((item) => {
      problems.push(`${prefix}：${item}`)
    })
  })

  validateBudget(plan).forEach((item) => problems.push(`预算：${item}`))

  return { ok: problems.length === 0, problems }
}

export function formatProblems(problems, limit = 2) {
  const list = problems.slice(0, limit)
  const rest = problems.length - list.length
  return `${list.join('；')}${rest > 0 ? ` 等 ${problems.length} 项` : ''}`
}
