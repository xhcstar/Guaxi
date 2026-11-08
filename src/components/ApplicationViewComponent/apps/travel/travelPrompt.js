// 模型编排：只负责“去哪、什么顺序、怎么讲”，地点与路线必须来自事实包。
// 这里同时提供确定性兜底，模型不可用/输出不合法时仍能生成可执行的行程。

import {
  addMinutes,
  durationText,
  enumerateDates,
  extractJson,
  minutesToTime,
  normalizeTime,
  safeText,
  timeToMinutes,
  uid,
  uniqueBy,
  weekdayCn,
} from './travelUtils.js'
import { PACE_OPTIONS, TRANSPORT_OPTIONS } from './travelPlan.js'

const SYSTEM_PROMPT = `你是一名资深中文旅行规划师，擅长把真实 POI 与真实路线编排成可执行的行程。
硬性规则：
1. 只能使用用户给出的候选地点 id，绝不编造地点、坐标、营业时间或价格。
2. 只输出一个 JSON 对象，不要 markdown 代码块，不要解释，不要前后缀。
3. 时间用 24 小时制 HH:mm，按时间递增，不重叠；景点之间必须留出交通时间。
4. 不写预算金额，预算由程序计算；可以写“人均约”以外的定性建议。
5. 信息不足时用 area / tips 说明，不要虚构事实。
6. 同一天不要来回横穿城市；先按地理位置聚类，再排顺序。`

function poiMap(facts) {
  const map = new Map()
  for (const poi of facts.attractions || []) map.set(poi.id, { ...poi, kind: 'sight' })
  for (const poi of facts.food || []) map.set(poi.id, { ...poi, kind: 'food' })
  for (const poi of facts.hotels || []) map.set(poi.id, { ...poi, kind: 'hotel' })
  return map
}

function compactPoi(poi) {
  return {
    id: poi.id,
    name: poi.name,
    kind: poi.kind,
    category: poi.category || '',
    rating: poi.rating ?? null,
    distanceKm: poi.distance != null ? Number((poi.distance / 1000).toFixed(1)) : null,
    address: poi.address || '',
    cost: poi.cost ?? null,
  }
}

function topPois(items, count, used = new Set()) {
  return (items || [])
    .filter((poi) => !used.has(poi.id))
    .sort((a, b) => {
      const rating = (Number(b.rating) || 0) - (Number(a.rating) || 0)
      if (rating !== 0) return rating
      return (Number(a.distance) || 0) - (Number(b.distance) || 0)
    })
    .slice(0, count)
}

function transportLabel(value) {
  return TRANSPORT_OPTIONS.find((item) => item.value === value)?.label || '自驾'
}

function paceLabel(value) {
  return PACE_OPTIONS.find((item) => item.value === value)?.label || '均衡'
}

function travelersText(form) {
  const parts = [`成人 ${form.adults || 0}`]
  if (form.children) parts.push(`儿童 ${form.children}`)
  if (form.elders) parts.push(`老人 ${form.elders}`)
  return parts.join('、')
}

export function buildOutlinePrompt({ form, facts }) {
  const dates = enumerateDates(form.startDate, form.endDate)
  const attractions = (facts.attractions || []).map(compactPoi)
  const food = (facts.food || []).map(compactPoi)
  const hotels = (facts.hotels || []).map(compactPoi)
  const weather = (facts.weather || [])
    .filter((item) => dates.includes(item.date))
    .map(
      (item) =>
        `${item.date} ${item.weekday || weekdayCn(item.date)} ${item.dayWeather || ''} ${item.nightTemp ?? ''}~${item.dayTemp ?? ''}℃`,
    )

  const user = `【用户需求】
出发地：${facts.origin?.name || form.origin?.name}（${facts.origin?.city || ''}）
目的地：${facts.destination?.name || form.destination?.name}（${facts.destination?.city || ''}）
日期：${form.startDate} 至 ${form.endDate}，${dates.length} 天；每天 ${form.startTime} 出发，${form.endTime} 结束
人数：${travelersText(form)}
出游方式：${transportLabel(form.transport)}
花费档位：${form.budgetLevel}
节奏：${paceLabel(form.pace)}
兴趣：${(form.interests || []).join('、') || '不限定'}
补充要求：${form.keywords || '无'}
必去：${(form.mustVisit || []).join('、') || '无'}
避开：${(form.avoid || []).join('、') || '无'}

【候选景点】
${attractions.map((poi) => `- ${poi.id} | ${poi.name} | ${poi.category} | 评分 ${poi.rating ?? '未知'} | ${poi.distanceKm ?? '?'} km | ${poi.address}`).join('\n') || '（无，允许只写区域建议，但不要编造具体地点）'}

【候选餐饮】
${food.map((poi) => `- ${poi.id} | ${poi.name} | ${poi.category} | 评分 ${poi.rating ?? '未知'} | ${poi.distanceKm ?? '?'} km`).join('\n') || '（无）'}

【候选住宿】
${hotels.map((poi) => `- ${poi.id} | ${poi.name} | 评分 ${poi.rating ?? '未知'} | 参考人均 ${poi.cost ?? '未知'}`).join('\n') || '（无）'}

【天气】
${weather.join('\n') || '（暂无预报，按季节常识提示）'}

请输出整体大纲：
{"title":"不超过 18 字的目的地主题","subtitle":"一句话说明这趟行程的取舍","days":[{"date":"${form.startDate}","theme":"当天主题","area":"主要活动区域","stops":[{"poiId":"候选 id","kind":"sight"},{"poiId":"候选 id","kind":"food"}],"hotelId":"候选住宿 id 或空字符串","paceNote":"节奏提示"}],"advice":["2-4 条整体建议"],"mustKnow":["2-4 条出发前须知"]}

要求：
- days 必须正好 ${dates.length} 项，日期依次为 ${dates.join('、')}。
- 每天 2-4 个景点/活动、1-2 个餐饮；stops 的顺序就是当天游览顺序，必须按地理位置聚类。
- 最后一天不要再安排住宿；多天行程的中间天从候选住宿里选 hotelId。
- 只使用上面出现的 id；没有合适的就留空并说明。`

  return { system: SYSTEM_PROMPT, prompt: user }
}

export function normalizeOutline(raw, { form, facts }) {
  const map = poiMap(facts)
  const dates = enumerateDates(form.startDate, form.endDate)
  if (!raw || !Array.isArray(raw.days) || raw.days.length === 0) {
    throw new Error('模型大纲缺少 days')
  }
  const used = new Set()
  const attractions = facts.attractions || []
  const food = facts.food || []
  const hotels = facts.hotels || []

  const days = dates.map((date, index) => {
    const source = raw.days[index] || {}
    let stops = []
    if (Array.isArray(source.stops)) {
      stops = source.stops
        .map((item) => {
          const poiId = safeText(item?.poiId || item?.id || item)
          const poi = map.get(poiId)
          if (!poi) return null
          return { poiId, kind: poi.kind === 'food' ? 'food' : 'sight' }
        })
        .filter(Boolean)
    } else if (Array.isArray(source.poiIds)) {
      stops = source.poiIds
        .map((poiId) => map.get(poiId))
        .filter(Boolean)
        .map((poi) => ({ poiId: poi.id, kind: poi.kind === 'food' ? 'food' : 'sight' }))
    }

    stops = uniqueBy(stops, (item) => item.poiId)
    const sights = stops.filter((item) => item.kind === 'sight')
    const meals = stops.filter((item) => item.kind === 'food')
    if (sights.length === 0) {
      const fillCount = form.pace === 'relaxed' ? 1 : form.pace === 'intense' ? 3 : 2
      for (const poi of topPois(attractions, fillCount, used)) {
        sights.push({ poiId: poi.id, kind: 'sight' })
        used.add(poi.id)
      }
    }
    if (meals.length === 0) {
      const poi = topPois(food, 1, used)[0]
      if (poi) {
        meals.push({ poiId: poi.id, kind: 'food' })
        used.add(poi.id)
      }
    }
    for (const item of [...sights, ...meals]) used.add(item.poiId)

    const mixed = []
    const maxSights = Math.max(sights.length, meals.length)
    for (let i = 0; i < maxSights; i += 1) {
      if (sights[i]) mixed.push(sights[i])
      if (meals[i]) mixed.push(meals[i])
    }

    let hotelId = safeText(source.hotelId)
    if (!map.has(hotelId)) {
      hotelId = ''
    }
    if (!hotelId && index < dates.length - 1 && hotels.length) {
      hotelId = topPois(hotels, 1, used)[0]?.id || ''
    }
    if (index === dates.length - 1) hotelId = ''

    return {
      date,
      weekday: weekdayCn(date),
      theme: safeText(source.theme) || (index === 0 ? '抵达与初见' : '慢慢逛'),
      area: safeText(source.area),
      stops: mixed.slice(0, 6),
      hotelId,
      paceNote: safeText(source.paceNote),
    }
  })

  return {
    title: safeText(raw.title) || '',
    subtitle: safeText(raw.subtitle) || '',
    days,
    advice: Array.isArray(raw.advice)
      ? raw.advice.map((item) => safeText(item)).filter(Boolean).slice(0, 6)
      : [],
    mustKnow: Array.isArray(raw.mustKnow)
      ? raw.mustKnow.map((item) => safeText(item)).filter(Boolean).slice(0, 6)
      : [],
  }
}

export function fallbackOutline({ form, facts }) {
  const dates = enumerateDates(form.startDate, form.endDate)
  const attractions = [...(facts.attractions || [])]
  const food = [...(facts.food || [])]
  const hotels = [...(facts.hotels || [])]
  const used = new Set()
  const sightsPerDay = form.pace === 'relaxed' ? 1 : form.pace === 'intense' ? 3 : 2
  const days = dates.map((date, index) => {
    const sights = topPois(attractions, sightsPerDay, used)
    sights.forEach((poi) => used.add(poi.id))
    const meal = topPois(food, 1, used)[0]
    if (meal) used.add(meal.id)
    const stops = []
    if (sights[0]) stops.push({ poiId: sights[0].id, kind: 'sight' })
    if (meal) stops.push({ poiId: meal.id, kind: 'food' })
    for (const poi of sights.slice(1)) stops.push({ poiId: poi.id, kind: 'sight' })
    const hotel = index < dates.length - 1 ? topPois(hotels, 1, used)[0] : null
    if (hotel) used.add(hotel.id)
    return {
      date,
      weekday: weekdayCn(date),
      theme: sights[0]?.category || (index === 0 ? '抵达与初见' : '城市漫步'),
      area: sights[0]?.city || facts.destination?.city || '',
      stops,
      hotelId: hotel?.id || '',
      paceNote: '',
    }
  })

  return {
    title: `${facts.destination?.city || facts.destination?.name || '目的地'} · ${dates.length}日行程`,
    subtitle: '按地理顺序排列的真实地点，先把最想看的放在上午。',
    days,
    advice: [
      '热门景点尽量在 9:00 前到达；排队超过 40 分钟就先去附近备选，下午再回来。',
      '午餐安排在两个景点之间，避免为了吃饭横穿城市。',
    ],
    mustKnow: [
      '出发前一天再确认开放时间与预约政策，周一很多博物馆闭馆。',
      '把本行程保存为长图或 PDF，离线也能查看。',
    ],
  }
}

function legPromptLines(legs) {
  return legs
    .map((leg, index) => {
      const steps = (leg.steps || [])
        .slice(0, 3)
        .map((step) => step.instruction)
        .filter(Boolean)
        .join(' → ')
      return `- leg ${index}：${leg.fromName || ''} → ${leg.toName || ''}，${leg.mode === 'walking' ? '步行' : leg.mode === 'transit' ? '公共交通' : '驾车'} ${Number(leg.distance || 0).toFixed(1)} km，约 ${Math.round((Number(leg.duration) || 0) / 60)} 分钟${leg.traffic ? `，${leg.traffic}` : ''}${steps ? `；${steps}` : ''}`
    })
    .join('\n')
}

export function buildDayPrompt({ form, facts, outlineDay, dayIndex, totalDays, legs, startName }) {
  const map = poiMap(facts)
  const stopLines = (outlineDay.stops || [])
    .map((item, index) => {
      const poi = map.get(item.poiId)
      if (!poi) return ''
      return `${index + 1}. ${poi.name}（${poi.kind === 'food' ? '餐饮' : poi.category || '景点'}，评分 ${poi.rating ?? '未知'}）${poi.address ? `，${poi.address}` : ''}`
    })
    .filter(Boolean)
    .join('\n')
  const weather = (facts.weather || []).find((item) => item.date === outlineDay.date)
  const food = (facts.food || []).map(compactPoi)
  const hotels = (facts.hotels || []).map(compactPoi)

  const user = `【当天信息】
日期：${outlineDay.date} ${outlineDay.weekday || weekdayCn(outlineDay.date)}；第 ${dayIndex + 1} / ${totalDays} 天
当天主题：${outlineDay.theme || ''}；主要区域：${outlineDay.area || ''}
出发：${startName || facts.origin?.name || '酒店'}
天气：${weather ? `${weather.dayWeather || ''}，${weather.nightTemp ?? ''}~${weather.dayTemp ?? ''}℃，${weather.wind || ''}` : '暂无预报'}
用户节奏：${paceLabel(form.pace)}；兴趣：${(form.interests || []).join('、') || '不限定'}
当天结束时间：${form.endTime}

【当天顺序】（已按地理位置排好）
${stopLines || '（没有候选点）'}

【点到点路线】
${legPromptLines(legs) || '（当天只有一个点，无路段）'}

【可选餐饮】
${food.map((poi) => `- ${poi.id} | ${poi.name} | 评分 ${poi.rating ?? '未知'}`).join('\n') || '（无）'}

【可选住宿】
${hotels.map((poi) => `- ${poi.id} | ${poi.name} | 参考人均 ${poi.cost ?? '未知'}`).join('\n') || '（无）'}

请输出当天的可执行时间表：
{"date":"${outlineDay.date}","theme":"当天主题","blocks":[
 {"start":"08:30","end":"09:00","kind":"transit","legIndex":0,"title":"前往第一站","detail":"简短说明"},
 {"start":"09:00","end":"10:30","kind":"sight","poiId":"候选 id","title":"游览标题","detail":"为什么值得、怎么逛","tips":["实用提示"],"booking":{"needed":true,"channel":"公众号/美团","deadline":"${form.startDate}","note":""}},
 {"start":"12:00","end":"13:00","kind":"food","poiId":"候选 id","title":"午餐","detail":"推荐菜与人均感受"},
 {"start":"19:00","end":"20:00","kind":"hotel","poiId":"候选 id","title":"入住","detail":"位置与入住提示"},
 {"start":"21:00","end":"21:30","kind":"rest","title":"回酒店休息","detail":""}
]}

要求：
- 必须为上面每一条 leg 输出一个 kind=transit 且带 legIndex 的块，时间要覆盖该段路程。
- 每个景点/餐饮/住宿块尽量带 poiId，且只能来自候选；不要编造新地点。
- 景点 60-150 分钟，餐饮 45-90 分钟；节奏松弛就多留白，紧凑就少留白。
- 最后一天按用户结束时间收尾，不要再安排入住。
- 不写预算金额；tips 写排队、预约、停车、穿衣、点菜等真正有用的信息。`

  return { system: SYSTEM_PROMPT, prompt: user }
}

const KIND_ALIAS = {
  transport: 'transit',
  traffic: 'transit',
  move: 'transit',
  meal: 'food',
  eat: 'food',
  restaurant: 'food',
  sightseeing: 'sight',
  attraction: 'sight',
  activity: 'activity',
  stay: 'hotel',
  lodging: 'hotel',
  break: 'rest',
}

function normalizeKind(value) {
  const kind = safeText(value, 'activity').toLowerCase()
  return KIND_ALIAS[kind] || (['transit', 'sight', 'food', 'hotel', 'rest', 'activity'].includes(kind) ? kind : 'activity')
}

function normalizeBlock(raw, map, fallbackStart = '09:00') {
  const start = normalizeTime(raw?.start, fallbackStart)
  const end = normalizeTime(raw?.end, addMinutes(start, 60))
  const kind = normalizeKind(raw?.kind)
  const poiId = safeText(raw?.poiId)
  const poi = map.get(poiId)
  return {
    id: uid('blk'),
    start,
    end: timeToMinutes(end) <= timeToMinutes(start) ? addMinutes(start, 60) : end,
    kind,
    title: safeText(raw?.title) || poi?.name || (kind === 'transit' ? '前往下一站' : '自由安排'),
    detail: safeText(raw?.detail),
    poiId: poi ? poi.id : '',
    poi: poi || null,
    tips: Array.isArray(raw?.tips) ? raw.tips.map((item) => safeText(item)).filter(Boolean).slice(0, 5) : [],
    booking: raw?.booking && typeof raw.booking === 'object'
      ? {
          needed: Boolean(raw.booking.needed),
          channel: safeText(raw.booking.channel),
          deadline: safeText(raw.booking.deadline),
          note: safeText(raw.booking.note),
        }
      : null,
    legIndex: Number.isInteger(Number(raw?.legIndex)) ? Number(raw.legIndex) : null,
    route: null,
  }
}

function durationMinutes(leg) {
  return Math.max(5, Math.round((Number(leg?.duration) || 0) / 60))
}

function describeLeg(leg, fromName, toName) {
  if (!leg) return `前往${toName || '下一站'}`
  const mode = leg.mode === 'walking' ? '步行' : leg.mode === 'transit' ? '公共交通' : '驾车'
  return `${mode} ${Number(leg.distance || 0).toFixed(1)} km，约 ${durationText(leg.duration) || '—'}`
}

export function normalizeDay(raw, { form, facts, outlineDay, dayIndex, legs, stopRefs, startName }) {
  const map = poiMap(facts)
  if (!raw || !Array.isArray(raw.blocks)) {
    throw new Error('模型当天安排缺少 blocks')
  }
  let blocks = raw.blocks
    .map((block) => normalizeBlock(block, map, form.startTime))
    .filter((block) => timeToMinutes(block.start) != null)
    .sort((a, b) => (timeToMinutes(a.start) || 0) - (timeToMinutes(b.start) || 0))
  if (!blocks.length) throw new Error('模型当天安排为空')

  // 把路线挂到 transit 块；模型漏掉的，按目标景点前插一段。
  legs.forEach((leg, index) => {
    const matched = blocks.find(
      (block) => block.kind === 'transit' && block.legIndex === index,
    )
    const target = stopRefs[index + 1]
    if (matched) {
      matched.route = leg
      matched.title = matched.title || `前往${target?.name || '下一站'}`
      matched.detail = matched.detail || describeLeg(leg, stopRefs[index]?.name, target?.name)
      return
    }
    const targetBlock = blocks.find(
      (block) =>
        block.kind !== 'transit' &&
        ((target?.id && block.poiId === target.id) || block.title === target?.name),
    )
    const minutes = durationMinutes(leg)
    let startMinutes = timeToMinutes(targetBlock?.start || addMinutes(form.startTime, index * 90))
    if (startMinutes == null) startMinutes = timeToMinutes(form.startTime) + index * 90
    let endMinutes = startMinutes
    startMinutes = Math.max(0, startMinutes - minutes)
    if (targetBlock) {
      const previous = blocks
        .filter((block) => timeToMinutes(block.end) <= timeToMinutes(targetBlock.start))
        .map((block) => timeToMinutes(block.end))
        .filter((value) => value != null)
      if (previous.length) startMinutes = Math.max(startMinutes, Math.max(...previous))
      endMinutes = Math.max(startMinutes + 5, timeToMinutes(targetBlock.start))
    }
    const block = {
      id: uid('blk'),
      start: minutesToTime(startMinutes),
      end: minutesToTime(endMinutes),
      kind: 'transit',
      title: `前往${target?.name || '下一站'}`,
      detail: describeLeg(leg, stopRefs[index]?.name, target?.name),
      poiId: '',
      poi: null,
      tips: [],
      booking: null,
      legIndex: index,
      route: leg,
    }
    const insertAt = targetBlock
      ? blocks.indexOf(targetBlock)
      : Math.min(blocks.length, index + 1)
    blocks.splice(insertAt, 0, block)
  })

  // 模型可能漏掉餐饮/住宿块：按 POI 补齐，保证行程完整。
  const hasFood = blocks.some((block) => block.kind === 'food')
  if (!hasFood) {
    const foodStop = stopRefs.find((poi) => poi?.kind === 'food')
    if (foodStop) {
      const start = '12:00'
      blocks.push({
        id: uid('blk'),
        start,
        end: '13:00',
        kind: 'food',
        title: foodStop.name,
        detail: foodStop.category || '当地餐饮',
        poiId: foodStop.id,
        poi: foodStop,
        tips: [],
        booking: null,
        legIndex: null,
        route: null,
      })
    }
  }
  const hasHotel = blocks.some((block) => block.kind === 'hotel')
  if (!hasHotel && outlineDay.hotelId) {
    const hotel = map.get(outlineDay.hotelId)
    if (hotel) {
      blocks.push({
        id: uid('blk'),
        start: '19:30',
        end: '20:00',
        kind: 'hotel',
        title: `入住 ${hotel.name}`,
        detail: hotel.address || '放下行李，附近吃晚饭',
        poiId: hotel.id,
        poi: hotel,
        tips: [],
        booking: null,
        legIndex: null,
        route: null,
      })
    }
  }

  blocks = blocks.sort((a, b) => (timeToMinutes(a.start) || 0) - (timeToMinutes(b.start) || 0))
  return {
    date: outlineDay.date,
    weekday: outlineDay.weekday || weekdayCn(outlineDay.date),
    theme: safeText(raw.theme) || outlineDay.theme,
    area: outlineDay.area,
    weather: (facts.weather || []).find((item) => item.date === outlineDay.date) || null,
    blocks,
    legs,
    tips: Array.isArray(raw.tips) ? raw.tips.map((item) => safeText(item)).filter(Boolean) : [],
  }
}

export function fallbackDay({ form, facts, outlineDay, dayIndex, legs, stopRefs, startName }) {
  const map = poiMap(facts)
  const blocks = []
  let cursor = timeToMinutes(form.startTime || '08:30') ?? 510
  const startLabel = startName || facts.origin?.name || '酒店'
  stopRefs.forEach((poi, index) => {
    const leg = legs[index]
    if (leg) {
      const minutes = durationMinutes(leg)
      blocks.push({
        id: uid('blk'),
        start: minutesToTime(cursor),
        end: minutesToTime(cursor + minutes),
        kind: 'transit',
        title: `前往${poi.name}`,
        detail: describeLeg(leg, index === 0 ? startLabel : stopRefs[index - 1]?.name, poi.name),
        poiId: '',
        poi: null,
        tips: leg.warning ? [leg.warning] : [],
        booking: null,
        legIndex: index,
        route: leg,
      })
      cursor += minutes + 10
    }
    const isFood = poi.kind === 'food'
    const duration = isFood ? 60 : form.pace === 'relaxed' ? 120 : 90
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(cursor),
      end: minutesToTime(cursor + duration),
      kind: isFood ? 'food' : 'sight',
      title: poi.name,
      detail: isFood
        ? `建议避开 12:00-13:00 的排队高峰，按 ${poi.category || '本地口味'} 点菜。`
        : `位于${poi.address || poi.city || '目的地'}，${poi.category || '值得慢慢逛'}。`,
      poiId: poi.id,
      poi,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
    cursor += duration + 15
  })

  const hotel = outlineDay.hotelId ? map.get(outlineDay.hotelId) : null
  const extraLeg = legs[stopRefs.length]
  if (extraLeg) {
    const minutes = durationMinutes(extraLeg)
    const targetName = extraLeg.toName || hotel?.name || '下一站'
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(cursor),
      end: minutesToTime(cursor + minutes),
      kind: 'transit',
      title: `前往${targetName}`,
      detail: describeLeg(extraLeg, stopRefs[stopRefs.length - 1]?.name, targetName),
      poiId: '',
      poi: null,
      tips: extraLeg.warning ? [extraLeg.warning] : [],
      booking: null,
      legIndex: stopRefs.length,
      route: extraLeg,
    })
    cursor += minutes + 10
  }

  if (hotel) {
    const hotelStart = Math.max(cursor + 30, 19 * 60)
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(hotelStart),
      end: minutesToTime(hotelStart + 30),
      kind: 'hotel',
      title: `入住 ${hotel.name}`,
      detail: hotel.address || '办理入住，放下行李后附近吃晚饭',
      poiId: hotel.id,
      poi: hotel,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
  }

  return {
    date: outlineDay.date,
    weekday: outlineDay.weekday || weekdayCn(outlineDay.date),
    theme: outlineDay.theme,
    area: outlineDay.area,
    weather: (facts.weather || []).find((item) => item.date === outlineDay.date) || null,
    blocks,
    legs,
    tips: [],
  }
}

export function parseModelJson(text) {
  return extractJson(text)
}
