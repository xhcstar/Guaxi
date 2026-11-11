// 模型编排：只负责“去哪、什么顺序、怎么讲”，地点与路线必须来自事实包。
// 校验不过时软件不自己改方案，而是把「哪一条不通过」讲回给模型让它重排；
// 这里提供那段反馈文本，以及最后仍然不通时的确定性兜底。

import {
  addMinutes,
  durationText,
  enumerateDates,
  extractJson,
  minutesToTime,
  normalizeTime,
  safeText,
  seasonFor,
  timeToMinutes,
  uid,
  uniqueBy,
  weekdayCn,
} from './travelUtils.js'
import { PACE_OPTIONS, TRANSPORT_OPTIONS } from './travelPlan.js'
import {
  dailyDetourBudgetKm,
  dailyTransitLimitMinutes,
  homeGuardKm,
  mealGuardKm,
  pickNearAnchor,
  pruneStopsByDetour,
} from './travelEngine.js'
import { validateDayBlocks } from './travelValidate.js'

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

// 一天的可用时长：出发到结束，用来把「在途占比」换算成分钟上限。
function dayWindowMinutes(form) {
  const start = timeToMinutes(form?.startTime || '08:30')
  const end = timeToMinutes(form?.endTime || '20:30')
  return start != null && end != null && end > start ? end - start : 0
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

/**
 * 校验反馈段：把「上一版哪里不通过」原样回传给模型。
 * 软件保持只做校验和提问，怎么改由模型决定；previous 只作对照，截断后附上避免撑爆上下文。
 */
export function buildRepairFeedback(feedback) {
  const { problems = [], previous = '' } = feedback || {}
  const items = (Array.isArray(problems) ? problems : [])
    .map((item) => String(item || '').trim())
    .filter(Boolean)
  if (!items.length) return ''
  const list = items.map((item, index) => `${index + 1}. ${item}`).join('\n')
  const raw = String(previous || '')
  const previousBlock = raw
    ? `\n\n【上一次的输出】（只用来对照问题，不要照抄）\n${raw.length > 2600 ? `${raw.slice(0, 2600)}…` : raw}`
    : ''
  return `\n\n【校验未通过，逐条改正】\n${list}${previousBlock}\n\n请针对上面每一条重新安排，输出完整的 JSON（结构与上面的示例完全一致）。只输出 JSON，不要解释、不要 markdown。`
}

export function buildOutlinePrompt({ form, facts, feedback = null }) {
  const dates = enumerateDates(form.startDate, form.endDate)
  const attractions = (facts.attractions || []).map(compactPoi)
  const food = (facts.food || []).map(compactPoi)
  const hotels = (facts.hotels || []).map(compactPoi)
  const season = seasonFor(form.startDate)
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

【季节】
${season.label}
${season.tips.map((tip) => `- ${tip}`).join('\n')}

【目的地天气】
${weather.join('\n') || '（暂无预报，按季节常识提示）'}

请输出整体大纲：
{"title":"不超过 18 字的目的地主题","subtitle":"一句话说明这趟行程的取舍","days":[{"date":"${form.startDate}","theme":"当天主题","area":"主要活动区域","stops":[{"poiId":"候选 id","kind":"sight"},{"poiId":"候选 id","kind":"food"}],"hotelId":"候选住宿 id 或空字符串","paceNote":"节奏提示"}],"advice":["2-4 条整体建议"],"mustKnow":["2-4 条出发前须知"]}

要求：
- days 必须正好 ${dates.length} 项，日期依次为 ${dates.join('、')}。
- 每天 2-4 个景点/活动、1-2 个餐饮；stops 的顺序就是当天游览顺序，必须按地理位置聚类。
- 最后一天不要再安排住宿；多天行程的中间天从候选住宿里选 hotelId。
- 结合季节与天气安排：雨天减少户外、夏季避开正午暴晒、冬季缩短户外时间，山区早晚加衣。
- 只使用上面出现的 id；没有合适的就留空并说明。`

  return { system: SYSTEM_PROMPT, prompt: `${user}${buildRepairFeedback(feedback)}` }
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

    // 模型常把一天塞到 5-6 个点，真实路线一算就是整天赶路。
    // 先按绕行预算裁掉绕得最远的那一站，再交给路线计算，避免"看着很满、其实没玩到"。
    const startPoint = index === 0 ? facts.origin : facts.destination || facts.origin
    const endPoint = index === dates.length - 1 ? facts.origin : facts.destination || facts.origin
    const ordered = mixed
      .slice(0, 6)
      .map((stop) => ({ stop, poi: map.get(stop.poiId) }))
      .filter((item) => item.poi)
    const pruned = pruneStopsByDetour(
      ordered.map((item) => item.poi),
      {
        start: startPoint,
        end: endPoint,
        budgetKm: dailyDetourBudgetKm(form, dayWindowMinutes(form)),
        keepIds: new Set(ordered.filter((item) => item.poi.must).map((item) => item.poi.id)),
      },
    )
    const keptIds = new Set(pruned.stops.map((poi) => poi.id))
    const trimmed = ordered
      .filter((item) => keptIds.has(item.poi.id))
      .map((item) => item.stop)

    return {
      date,
      weekday: weekdayCn(date),
      theme: safeText(source.theme) || (index === 0 ? '抵达与初见' : '慢慢逛'),
      area: safeText(source.area),
      stops: trimmed,
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

function weatherBrief(point) {
  if (!point) return ''
  if (point.available === false) {
    return `${point.name || '途经'} ${point.time || ''} 临近更新`
  }
  const parts = [point.name || '途经', point.time || '', point.weather || '']
  const temperature = Number(point.temperature)
  if (Number.isFinite(temperature)) parts.push(`${Math.round(temperature)}℃`)
  const rainChance = Number(point.precipitationProbability)
  if (Number.isFinite(rainChance) && rainChance >= 40) parts.push(`降水 ${Math.round(rainChance)}%`)
  const wind = Number(point.windSpeed)
  if (Number.isFinite(wind) && wind >= 30) parts.push(`风 ${Math.round(wind)} km/h`)
  return parts.filter(Boolean).join(' ')
}

function legWeatherLines(legs) {
  return (legs || [])
    .map((leg, index) => {
      const points = leg.weatherPoints || []
      if (!points.length) return ''
      return `- leg ${index}：${points.map(weatherBrief).join(' → ')}`
    })
    .filter(Boolean)
    .join('\n')
}

// 沿途餐馆单独成段：跨城那天午饭本就该在途中城镇解决，混在普通餐饮里模型会忽略。
function alongFoodLines(facts) {
  const along = (facts.food || []).filter((poi) => poi.alongTime)
  if (!along.length) return ''
  return `【沿途顺路餐饮】（跨城路段中途，优先用于这一段路上的午饭/晚饭）
${along
  .map(
    (poi) =>
      `- ${poi.id} | ${poi.name} | ${poi.alongMeal === 'lunch' ? '午餐' : '晚餐'} ${poi.alongTime} 前后 | 评分 ${poi.rating ?? '未知'}`,
  )
  .join('\n')}`
}

export function buildDayPrompt({ form, facts, outlineDay, dayIndex, totalDays, legs, startName, feedback = null }) {
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
  const season = seasonFor(outlineDay.date)
  const food = (facts.food || []).map(compactPoi)
  const hotels = (facts.hotels || []).map(compactPoi)
  // 当天在途预算：给出分钟数，模型才知道“别把一天排成跨城赶路”。
  const transitBudget = dailyTransitLimitMinutes(form, dayWindowMinutes(form))
  const transitUsed = (legs || []).reduce((sum, leg) => sum + (Number(leg?.duration) || 0) / 60, 0)
  // 当天往返（含周末一日）不能出现住宿；多日行程只有非最后一晚才写酒店。
  const allowHotel = form.startDate !== form.endDate && dayIndex < totalDays - 1
  const startTime = form.startTime || '09:00'
  const hotelExample = allowHotel
    ? ` {"start":"19:00","end":"20:00","kind":"hotel","poiId":"候选 id","title":"入住","detail":"位置与入住提示"},\n {"start":"21:00","end":"21:30","kind":"rest","title":"回酒店休息","detail":""},\n`
    : ''

  const user = `【当天信息】
日期：${outlineDay.date} ${outlineDay.weekday || weekdayCn(outlineDay.date)}；第 ${dayIndex + 1} / ${totalDays} 天
季节：${season.label}
当天主题：${outlineDay.theme || ''}；主要区域：${outlineDay.area || ''}
出发：${startName || facts.origin?.name || '酒店'}
目的地天气：${weather ? `${weather.dayWeather || ''}，${weather.nightTemp ?? ''}~${weather.dayTemp ?? ''}℃，${weather.wind || ''}` : '暂无预报'}
用户节奏：${paceLabel(form.pace)}；兴趣：${(form.interests || []).join('、') || '不限定'}
当天结束时间：${form.endTime}
当天的路段（已按真实路线算好，不要再增删停留点）：${legs?.length || 0} 段，在途约 ${Math.round(transitUsed)} 分钟

【当天顺序】（已按地理位置排好）
${stopLines || '（没有候选点）'}

【点到点路线】
${legPromptLines(legs) || '（当天只有一个点，无路段）'}

【沿途天气】（按出发时间估算）
${legWeatherLines(legs) || '（沿途天气暂不可用，按季节与目的地天气判断）'}

【可选餐饮】
${food.map((poi) => `- ${poi.id} | ${poi.name} | 评分 ${poi.rating ?? '未知'}${poi.alongTime ? ` | 沿途 ${poi.alongMeal === 'lunch' ? '午餐' : '晚餐'} ${poi.alongTime} 前后` : ''}`).join('\n') || '（无）'}
${alongFoodLines(facts)}
${allowHotel ? `
【可选住宿】
${hotels.map((poi) => `- ${poi.id} | ${poi.name} | 参考人均 ${poi.cost ?? '未知'}`).join('\n') || '（无）'}
` : ''}

请输出当天的可执行时间表：
{"date":"${outlineDay.date}","theme":"当天主题","blocks":[
 {"start":"${startTime}","end":"${addMinutes(startTime, 30)}","kind":"transit","legIndex":0,"title":"前往第一站","detail":"简短说明"},
 {"start":"${addMinutes(startTime, 30)}","end":"${addMinutes(startTime, 120)}","kind":"sight","poiId":"候选 id","title":"游览标题","detail":"为什么值得、怎么逛","tips":["实用提示"],"booking":{"needed":true,"channel":"公众号/美团","deadline":"${form.startDate}","note":""}},
 {"start":"12:00","end":"13:00","kind":"food","poiId":"候选 id","title":"午餐","detail":"推荐菜与人均感受"},
${hotelExample}]}

要求：
- 必须为上面每一条 leg 输出一个 kind=transit 且带 legIndex 的块，时间要覆盖该段路程。
- 每个景点/餐饮/住宿块尽量带 poiId，且只能来自候选；不要编造新地点。
- 景点 60-150 分钟，餐饮 45-90 分钟；节奏松弛就多留白，紧凑就少留白。
- 结合沿途与目的地天气：有雨把户外点放到雨势较小的时段或准备室内备选；有雾提醒自驾降速；温差大提醒分层穿衣。
- 第一天从 ${startTime} 之后开始，最后的活动在用户结束时间 ${form.endTime} 前结束（返程可以排在这之后）。
- 时间覆盖 11:00-15:00 必须安排午餐；覆盖 17:00-20:30 必须安排晚餐。候选里没有合适的店时，写一顿“附近小店/简餐”，不要省略。
- 午餐/晚餐必须安排在当天其他停留点附近（同城、12 km 内），不要排在家门口或返程终点附近。
- 当天在途预算约 ${transitBudget} 分钟；上面的路段已经算好，不要为了多塞景点再加绕路。
- 当天往返和最后一天不要安排住宿；需要过夜的那几天必须安排住宿。
- 不写预算金额；tips 写排队、预约、停车、穿衣、点菜等真正有用的信息。`

  return { system: SYSTEM_PROMPT, prompt: `${user}${buildRepairFeedback(feedback)}` }
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

function normalizeText(value) {
  return String(value || '')
    .replace(/\s+/g, '')
    .replace(/[·・,，。:：;；\-—_（）()【】\[\]]/g, '')
}

// 模型经常把地点写成“蓝关古道雨中慢行”这种自由文本；
// 按名称包含关系匹配回候选，能救回大部分“不是候选地点”的误判。
function buildPoiMatcher(map) {
  const list = [...map.values()].filter((poi) => poi?.name)
  return (text, kind = '') => {
    const needle = normalizeText(text)
    if (needle.length < 2) return null
    let best = null
    let bestScore = 0
    for (const poi of list) {
      // 餐饮/住宿只和同类匹配，景点也不会被餐厅名字顶掉。
      if (kind === 'food' && poi.kind !== 'food') continue
      if (kind === 'hotel' && poi.kind !== 'hotel') continue
      if (kind !== 'food' && kind !== 'hotel' && (poi.kind === 'food' || poi.kind === 'hotel')) {
        continue
      }
      const name = normalizeText(poi.name)
      if (name.length < 2) continue
      if (needle === name) return poi
      if (needle.includes(name) || name.includes(needle)) {
        const score = Math.min(name.length, needle.length) / Math.max(name.length, needle.length)
        if (score > bestScore) {
          bestScore = score
          best = poi
        }
      }
    }
    return bestScore >= 0.5 ? best : null
  }
}

function normalizeBlock(raw, map, fallbackStart = '09:00', matchPoi = null) {
  const start = normalizeTime(raw?.start, fallbackStart)
  const end = normalizeTime(raw?.end, addMinutes(start, 60))
  let kind = normalizeKind(raw?.kind)
  const poiId = safeText(raw?.poiId)
  let poi = map.get(poiId)
  if (!poi && matchPoi && kind !== 'transit') {
    poi = matchPoi(raw?.title, kind) || null
  }
  // 模型常把景点写成 activity；挂了景点 POI 就归为 sight，别让整份安排被判“没有景点”。
  if (kind === 'activity' && poi && poi.kind !== 'food' && poi.kind !== 'hotel') {
    kind = 'sight'
  }
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

function routeWeatherOf(legs) {
  return uniqueBy(
    (legs || []).flatMap((leg) => leg.weatherPoints || []),
    (point) => `${point.date}|${point.time}|${point.lng}|${point.lat}`,
  )
}

function durationMinutes(leg) {
  return Math.max(5, Math.round((Number(leg?.duration) || 0) / 60))
}

function describeLeg(leg, fromName, toName) {
  if (!leg) return `前往${toName || '下一站'}`
  const mode = leg.mode === 'walking' ? '步行' : leg.mode === 'transit' ? '公共交通' : '驾车'
  return `${mode} ${Number(leg.distance || 0).toFixed(1)} km，约 ${durationText(leg.duration) || '—'}`
}

// 一天能用 HH:mm 表示的最后时刻。分钟数一旦越过这里，开始与结束会被一起截成 23:59，
// 校验就会报「结束时间不晚于开始时间」——所以排到这里还没排完，说明当天装不下，
// 该少排一站，而不是排出假时间。
const DAY_END_MINUTES = 24 * 60 - 1
// 一段安排至少要占这么久；连这点时间都挤不出来，就说明当天已经排满。
const MIN_SLOT_MINUTES = 20
// 过夜那天给「前往酒店 + 入住」留的余量（上路 30 分 + 入住 30 分 + 缓冲）。
const HOTEL_RESERVE_MINUTES = 75

// 把一段安排裁进当天可表示的范围；连最小片段都放不下时返回 null，由调用方跳过。
function clipBlockTimes(start, end, limit = DAY_END_MINUTES, minMinutes = 5) {
  const from = Math.max(0, Math.round(start))
  const to = Math.max(from + minMinutes, Math.round(end))
  if (from >= limit) return null
  return { start: from, end: Math.min(to, limit) }
}

export function normalizeDay(raw, { form, facts, outlineDay, dayIndex, totalDays = 1, legs, stopRefs, startName }) {
  const map = poiMap(facts)
  if (!raw || !Array.isArray(raw.blocks)) {
    throw new Error('模型当天安排缺少 blocks')
  }
  const matchPoi = buildPoiMatcher(map)
  let blocks = raw.blocks
    .map((block) => normalizeBlock(block, map, form.startTime, matchPoi))
    .filter((block) => timeToMinutes(block.start) != null)
    .sort((a, b) => (timeToMinutes(a.start) || 0) - (timeToMinutes(b.start) || 0))
  // 匹配不上的景点/餐饮/住宿块直接丢掉，而不是整份作废；自由活动/休息保留。
  blocks = blocks.filter((block) => {
    if (block.poi) return true
    return block.kind === 'transit' || block.kind === 'rest' || block.kind === 'activity'
  })
  if (!blocks.length) throw new Error('模型当天安排为空')
  if (!blocks.some((block) => block.kind === 'sight' && block.poi)) {
    throw new Error('模型没有给出可用的景点')
  }

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
    const dayStart = timeToMinutes(form.startTime) ?? 9 * 60
    let startMinutes = timeToMinutes(targetBlock?.start || addMinutes(form.startTime, index * 90))
    if (startMinutes == null) startMinutes = dayStart + index * 90
    let endMinutes = startMinutes
    // 不能倒推到当天出发之前：08:30 的第一段路不会从 00:10 开始。
    startMinutes = Math.max(dayStart, startMinutes - minutes)
    if (targetBlock) {
      const previous = blocks
        .filter((block) => timeToMinutes(block.end) <= timeToMinutes(targetBlock.start))
        .map((block) => timeToMinutes(block.end))
        .filter((value) => value != null)
      if (previous.length) startMinutes = Math.max(startMinutes, Math.max(...previous))
      endMinutes = Math.max(startMinutes + 5, timeToMinutes(targetBlock.start))
    } else {
      // 模型没给这一站排块时，按「已有安排的收尾时间」往后接：按序号硬插会插进
      // 别人中间，整天的块一旦重叠，这天就会被判失败退回兜底。
      const settled = Math.max(
        dayStart,
        ...blocks.map((block) => timeToMinutes(block.end) ?? 0),
      )
      startMinutes = Math.max(startMinutes, settled)
      endMinutes = Math.max(startMinutes + minutes, settled + minutes)
    }
    const times = clipBlockTimes(startMinutes, endMinutes)
    if (!times) return
    const block = {
      id: uid('blk'),
      start: minutesToTime(times.start),
      end: minutesToTime(times.end),
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
    const insertAt = targetBlock ? blocks.indexOf(targetBlock) : blocks.length
    blocks.splice(insertAt, 0, block)
  })

  // 模型漏掉的午餐/晚餐：在时间窗里找空档补上，优先用当天附近的真实餐饮。
  const overlapsMealWindow = (block, from, to) => {
    const start = timeToMinutes(block?.start)
    const end = timeToMinutes(block?.end)
    return start != null && end != null && start < to && end > from
  }
  const findFreeSlot = (from, to, duration) => {
    const sorted = [...blocks].sort(
      (a, b) => (timeToMinutes(a.start) || 0) - (timeToMinutes(b.start) || 0),
    )
    let cursor = from
    for (const block of sorted) {
      const start = timeToMinutes(block?.start)
      const end = timeToMinutes(block?.end)
      if (start == null || end == null) continue
      if (end <= cursor) continue
      if (start >= to) break
      if (start - cursor >= duration) return cursor
      cursor = Math.max(cursor, end)
      if (cursor >= to) return null
    }
    return to - cursor >= duration ? cursor : null
  }
  const endMinutes = timeToMinutes(form.endTime) ?? 0
  const mealSpecs = []
  if (endMinutes >= 13 * 60) {
    mealSpecs.push({ key: 'lunch', from: 11 * 60 + 30, to: 15 * 60, label: '午餐' })
  }
  if (endMinutes >= 19 * 60) {
    mealSpecs.push({ key: 'dinner', from: 17 * 60 + 30, to: 20 * 60 + 30, label: '晚餐' })
  }
  const usedFood = new Set(
    blocks.filter((block) => block.poi?.kind === 'food').map((block) => block.poi.id),
  )
  for (const spec of mealSpecs) {
    if (blocks.some((block) => block.kind === 'food' && overlapsMealWindow(block, spec.from, spec.to))) {
      continue
    }
    const slot = findFreeSlot(spec.from, spec.to, 60)
    if (slot == null) continue
    const before = [...blocks]
      .filter((block) => (timeToMinutes(block.end) ?? 0) <= slot)
      .sort((a, b) => (timeToMinutes(b.end) || 0) - (timeToMinutes(a.end) || 0))[0]
    const after = blocks.find((block) => (timeToMinutes(block.start) ?? 0) >= slot + 60)
    const anchor =
      before?.poi ||
      after?.poi ||
      stopRefs.find((poi) => poi?.kind === 'sight') ||
      facts.destination
    const near = pickNearAnchor(facts.food || [], anchor, {
      maxKm: 12,
      exclude: usedFood,
      kind: 'food',
    })
    if (near) usedFood.add(near.id)
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(slot),
      end: minutesToTime(slot + 60),
      kind: 'food',
      title: near ? near.name : `${spec.label} · 附近小店（现场选择）`,
      detail: near
        ? `位于${near.address || near.city || '附近'}，${spec.key === 'lunch' ? '避开 12:00-13:00 的排队高峰' : '就近吃晚饭，不为了网红店折返'}。`
        : '当天附近有多家小馆，按口味现场选即可。',
      poiId: near?.id || '',
      poi: near || null,
      freeMeal: !near,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
  }
  // 需要过夜但模型没排住宿：用候选酒店或占位块补上，保证计划可执行。
  const needHotel = form.startDate !== form.endDate && dayIndex < totalDays - 1
  if (needHotel && !blocks.some((block) => block.kind === 'hotel')) {
    const lastPoi = [...blocks]
      .filter((block) => block.poi)
      .sort((a, b) => (timeToMinutes(b.end) || 0) - (timeToMinutes(a.end) || 0))[0]?.poi
    const hotel =
      (outlineDay.hotelId ? map.get(outlineDay.hotelId) : null) ||
      pickNearAnchor(facts.hotels || [], lastPoi || facts.destination, { maxKm: 15, kind: 'hotel' })
    const latestEnd = Math.max(
      0,
      ...blocks.map((block) => timeToMinutes(block.end) ?? 0),
    )
    const hotelStart = Math.max(19 * 60, latestEnd + 15)
    // 当天的块已经排到 23:59 时，入住只能截在同一分钟上，那会变成「结束不晚于开始」；
    // 这种情况不硬塞住宿，让当天校验退回确定性编排，由那边重新排到装得下为止。
    const hotelTimes = clipBlockTimes(hotelStart, hotelStart + 30, DAY_END_MINUTES, 30)
    if (hotelTimes) {
      blocks.push({
        id: uid('blk'),
        start: minutesToTime(hotelTimes.start),
        end: minutesToTime(hotelTimes.end),
        kind: 'hotel',
        title: hotel ? `入住 ${hotel.name}` : '入住目的地附近酒店（现场预订）',
        detail: hotel?.address || '目的地附近酒店较多，按预算现场预订即可。',
        poiId: hotel?.id || '',
        poi: hotel || null,
        freeHotel: !hotel,
        tips: [],
        booking: null,
        legIndex: null,
        route: null,
      })
    }
  }

  blocks = blocks.sort((a, b) => (timeToMinutes(a.start) || 0) - (timeToMinutes(b.start) || 0))
  // 模型自由发挥的地点、超出半径的地点、时间倒挂的排法都在这里拦下，
  // 由调用方退回确定性编排，避免把不满足约束的计划交给用户。
  // 收尾偏晚、当天在途过长不在这里判失败：这两项靠单日重排消除不了，
  // 判失败会让所有长线行程都退回确定性编排；由最终计划校验给出提示。
  const problems = validateDayBlocks(
    { blocks, weather: null },
    {
      center: facts.destination,
      radiusKm: Number(form.radiusKm) || 20,
      notBeforeMinutes: timeToMinutes(form.startTime),
      allowHotel: form.startDate !== form.endDate,
      home: facts.origin,
      homeGuard: homeGuardKm(form),
      mealGuard: mealGuardKm({
        ...form,
        origin: facts.origin,
        destination: facts.destination,
      }),
      requireLunch: (timeToMinutes(form.endTime) ?? 0) >= 13 * 60,
      requireDinner: (timeToMinutes(form.endTime) ?? 0) >= 19 * 60,
      requireHotel: form.startDate !== form.endDate && dayIndex < totalDays - 1,
    },
  )
  if (problems.length) {
    const error = new Error(`模型当天安排未通过校验：${problems[0]}`)
    // 校验条目原样带出去：调用方要把它们讲回给模型整改，而不是立刻换成确定性编排。
    error.code = 'day-validation'
    error.problems = problems
    error.previous = raw
    throw error
  }
  return {
    date: outlineDay.date,
    weekday: outlineDay.weekday || weekdayCn(outlineDay.date),
    theme: safeText(raw.theme) || outlineDay.theme,
    area: outlineDay.area,
    weather: (facts.weather || []).find((item) => item.date === outlineDay.date) || null,
    routeWeather: routeWeatherOf(legs),
    blocks,
    legs,
    tips: Array.isArray(raw.tips) ? raw.tips.map((item) => safeText(item)).filter(Boolean) : [],
  }
}

export function fallbackDay({ form, facts, outlineDay, dayIndex, totalDays = 1, legs, stopRefs, startName }) {
  const map = poiMap(facts)
  const blocks = []
  const tips = []
  const startMin = timeToMinutes(form.startTime || '08:30') ?? 510
  const endMin = Math.max(timeToMinutes(form.endTime || '20:30') ?? 1230, startMin + 5 * 60)
  // 两个上限分开用：endMin 是用户设的结束时间，超一点只提示；activityLimit 是当天真正
  // 装得下的时间，越过它排出来的 HH:mm 会退化成 23:59 的假时间。
  const lastDay = dayIndex >= Math.max(1, totalDays) - 1
  const needHotel = !lastDay && form.startDate !== form.endDate
  const activityLimit = DAY_END_MINUTES - (needHotel ? HOTEL_RESERVE_MINUTES : 0)
  // 用户设的结束时间说的是「活动结束」，所以新的一站最晚要在 endMin 前开始；
  // 已经开始的这一站可以顺延，返程和入住也可以排在这之后。
  const lastStartMinutes = Math.min(activityLimit, Math.max(endMin, startMin + 120))
  let cursor = startMin
  const startLabel = startName || facts.origin?.name || '酒店'
  // 覆盖范围放宽到 15:00 / 20:30，长行程也能补上“晚一点的午饭/晚饭”。
  const LUNCH = { from: 11 * 60 + 30, to: 15 * 60 }
  const DINNER = { from: 17 * 60 + 30, to: 20 * 60 + 30 }
  let lunchDone = false
  let dinnerDone = false
  let lastAnchor = null
  // 当天排不下的停留点：记下来，最后写进 tips，避免「少了一站却不说」。
  const skipped = []
  const usedFood = new Set()
  const foodPool = uniqueBy(
    [
      ...(facts.food || []),
      ...stopRefs.filter((item) => item.kind === 'food').map((item) => map.get(item.poiId)),
    ].filter(Boolean),
    (poi) => poi.id,
  )

  const addMeal = (kind, anchor) => {
    const windowInfo = kind === 'lunch' ? LUNCH : DINNER
    const start = Math.max(cursor, windowInfo.from)
    // 一顿饭要放得下整块 60 分钟，窗口和当天剩余时间都得够；否则不硬塞，留给下一站。
    if (start > windowInfo.to - 60 || start > Math.min(endMin - 45, activityLimit - 60)) return false
    const near = pickNearAnchor(foodPool, anchor, { maxKm: 8, exclude: usedFood, kind: 'food' })
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(start),
      end: minutesToTime(start + 60),
      kind: 'food',
      title:
        near
          ? near.name
          : kind === 'lunch'
            ? '午餐 · 附近小店（现场选择）'
            : '晚餐 · 附近小店（现场选择）',
      detail: near
        ? `位于${near.address || near.city || '附近'}，${kind === 'lunch' ? '避开 12:00-13:00 的排队高峰，按当地口味点菜' : '就近吃晚饭，不为了网红店折返'}。`
        : '目的地附近有多家小馆，按口味现场选即可；不为了吃饭折返几十公里。',
      poiId: near?.id || '',
      poi: near || null,
      freeMeal: !near,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
    if (near) usedFood.add(near.id)
    cursor = start + 60 + 15
    if (kind === 'lunch') lunchDone = true
    else dinnerDone = true
    return true
  }

  const maybeMeals = (anchor) => {
    if (!lunchDone && cursor >= LUNCH.from && cursor <= LUNCH.to - 60) addMeal('lunch', anchor)
    if (!dinnerDone && cursor >= DINNER.from && cursor <= DINNER.to - 60) addMeal('dinner', anchor)
  }

  for (let index = 0; index < stopRefs.length; index += 1) {
    const poi = stopRefs[index]
    const leg = legs[index]
    const isFood = poi.kind === 'food'
    const dwell = isFood ? 60 : form.pace === 'relaxed' ? 120 : 90
    const legMinutes = leg ? durationMinutes(leg) : 0
    const nextStart = cursor + (legMinutes ? legMinutes + 10 : 0)
    // 下一站本身就是饭点附近的餐饮时，不重复插一顿。
    const nextFoodInLunch =
      isFood && nextStart >= LUNCH.from - 15 && nextStart <= LUNCH.to
    const nextFoodInDinner =
      isFood && nextStart >= DINNER.from - 15 && nextStart <= DINNER.to
    // 这一段路加一小段游览都塞不进当天，剩下的点就别排了：宁可在计划里少一站，
    // 也不能排出 23:59 才开始、23:59 就结束的假时间。
    if (cursor + legMinutes + 10 + MIN_SLOT_MINUTES > lastStartMinutes) {
      skipped.push(...stopRefs.slice(index).map((item) => item.name))
      break
    }
    if (index > 0 && !nextFoodInLunch && !nextFoodInDinner) {
      maybeMeals(lastAnchor || poi)
      // 补了饭之后时间可能又不够了，按新的 cursor 再判一次。
      if (cursor + legMinutes + 10 + MIN_SLOT_MINUTES > lastStartMinutes) {
        skipped.push(...stopRefs.slice(index).map((item) => item.name))
        break
      }
    }

    if (leg) {
      const minutes = durationMinutes(leg)
      const times = clipBlockTimes(cursor, cursor + minutes, activityLimit)
      if (!times) {
        skipped.push(...stopRefs.slice(index).map((item) => item.name))
        break
      }
      blocks.push({
        id: uid('blk'),
        start: minutesToTime(times.start),
        end: minutesToTime(times.end),
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
    // 时间够就按节奏停留，不够就压缩这一站，但不低于最小片段。
    const duration = Math.max(
      MIN_SLOT_MINUTES,
      Math.min(dwell, activityLimit - cursor),
    )
    const blockStart = cursor
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(blockStart),
      end: minutesToTime(blockStart + duration),
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
    if (isFood) {
      // 这一站已经占了这家店，晚饭别再排同一家。
      if (poi.id) usedFood.add(poi.id)
      // 差一点到饭点的餐饮也算这一顿，别在 10:50 吃完又在 12:00 补一顿午饭。
      if (blockStart >= LUNCH.from - 60 && blockStart < LUNCH.to) lunchDone = true
      if (blockStart >= DINNER.from - 60 && blockStart < DINNER.to) dinnerDone = true
    }
    lastAnchor = poi
    cursor += duration + 15
  }

  // 一站都排不进去（当天第一段路就超过一天）：至少给出「在路上」的时间线，
  // 否则这天会变成空安排，整份计划又被判失败。
  let routeOverflow = false
  if (!blocks.length) {
    const first = stopRefs[0]
    const leg = legs[0] || null
    const minutes = leg ? durationMinutes(leg) : 60
    const times = clipBlockTimes(startMin, startMin + Math.max(minutes, MIN_SLOT_MINUTES), activityLimit)
    if (times) {
      blocks.push({
        id: uid('blk'),
        start: minutesToTime(times.start),
        end: minutesToTime(times.end),
        kind: 'transit',
        title: `前往${first?.name || '目的地'}`,
        detail: leg
          ? describeLeg(leg, startLabel, first?.name)
          : '当天路线服务不可用，按直线估算，出发前请复核。',
        poiId: '',
        poi: null,
        tips: leg?.warning ? [leg.warning] : [],
        booking: null,
        legIndex: leg ? 0 : null,
        route: leg,
      })
      // 时间线已经被这一段路占满，后面补饭/补住宿都要按这个终点算。
      cursor = times.end
      routeOverflow = true
    }
    if (stopRefs.length) {
      tips.push('当天路程已经占满整天，建议改乘飞机/高铁，或在途中城镇住一晚再继续。')
    } else {
      tips.push('这天没有排到合适的地点，可以先自由活动，或把想去的点挪到其他天。')
    }
  }

  // 白天很长但景点少：收尾前把没排上的饭补上（睡到自然醒/只逛一个点也能吃上午饭）。
  if (!lunchDone && endMin >= 13 * 60) {
    cursor = Math.max(cursor, startMin)
    addMeal('lunch', lastAnchor)
  }
  if (!dinnerDone && endMin >= 19 * 60) {
    addMeal('dinner', lastAnchor)
  }

  const hotel = outlineDay.hotelId
    ? map.get(outlineDay.hotelId)
    : needHotel
      ? pickNearAnchor(facts.hotels || [], lastAnchor, { maxKm: 15, kind: 'hotel' })
      : null
  // 少了站就不再排返程：剩下那段路通向没排进去的点，接不上当天的时间线。
  const extraLeg = skipped.length ? null : legs[stopRefs.length]
  let returnLegUsed = false
  if (extraLeg) {
    const minutes = durationMinutes(extraLeg)
    const times = clipBlockTimes(cursor, cursor + minutes, activityLimit)
    const targetName = extraLeg.toName || hotel?.name || (needHotel ? '住宿' : '下一站')
    if (times) {
      blocks.push({
        id: uid('blk'),
        start: minutesToTime(times.start),
        end: minutesToTime(times.end),
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
      cursor = times.end + 10
      returnLegUsed = true
    } else {
      tips.push('返程那段路已经排不进当天，建议提前结束游览，或就近住一晚再走。')
    }
  }

  if (needHotel) {
    // 上面已经预留了 HOTEL_RESERVE_MINUTES，这里再夹一次是防手滑：宁可住得早，不排出 23:59。
    const hotelStart = Math.min(Math.max(cursor + 30, 19 * 60), DAY_END_MINUTES - 30)
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(hotelStart),
      end: minutesToTime(hotelStart + 30),
      kind: 'hotel',
      title: hotel ? `入住 ${hotel.name}` : '入住目的地附近酒店（现场预订）',
      detail: hotel?.address || '目的地附近酒店较多，按预算现场预订即可。',
      poiId: hotel?.id || '',
      poi: hotel || null,
      freeHotel: !hotel,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
  }

  // 连一段路都没有（当天没有任何可用地点/路线）：给一段自由活动，
  // 别让这天空着，空安排会把整份计划判失败。
  if (!blocks.length) {
    blocks.push({
      id: uid('blk'),
      start: minutesToTime(startMin),
      end: minutesToTime(Math.min(startMin + 60, DAY_END_MINUTES)),
      kind: 'rest',
      title: '自由活动 / 现场找去处',
      detail: '这天没有可用的候选地点，可以到地方再决定，或回到探索页换一批地点。',
      poiId: '',
      poi: null,
      tips: [],
      booking: null,
      legIndex: null,
      route: null,
    })
  }

  if (skipped.length && !routeOverflow) {
    const names = skipped.slice(0, 2).join('、')
    tips.push(
      `当天时间装不下，${names}${skipped.length > 2 ? ` 等 ${skipped.length} 处` : ''}没排进来，可以挪到其他天或换掉一处目的地。`,
    )
  }

  // 没排进来的站，连它后面那段路一起拿掉：地图、预算和时间线才对得上。
  const usedLegs = routeOverflow
    ? legs.slice(0, legs.length ? 1 : 0)
    : skipped.length
      ? legs.slice(0, Math.max(0, stopRefs.length - skipped.length))
      : returnLegUsed || legs.length <= stopRefs.length
        ? legs
        : legs.slice(0, stopRefs.length)

  return {
    date: outlineDay.date,
    weekday: outlineDay.weekday || weekdayCn(outlineDay.date),
    theme: outlineDay.theme,
    area: outlineDay.area,
    weather: (facts.weather || []).find((item) => item.date === outlineDay.date) || null,
    routeWeather: routeWeatherOf(usedLegs),
    blocks,
    legs: usedLegs,
    tips,
  }
}

export function parseModelJson(text) {
  try {
    return extractJson(text)
  } catch (error) {
    // 把模型的原话带进报错：登录墙/额度用尽时，用户能直接看到“请先登录”之类的提示。
    const snippet = String(text || '').replace(/\s+/g, ' ').trim().slice(0, 80)
    if (snippet) throw new Error(`模型回了说明文字而不是 JSON：${snippet}`)
    throw error
  }
}
