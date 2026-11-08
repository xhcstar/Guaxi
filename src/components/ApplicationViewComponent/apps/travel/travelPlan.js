// 行程装配：把「事实 + 模型编排 + 路线」合成可持久化的计划，并计算预算与行前清单。
// 预算全部由确定性规则计算，模型只负责文案和顺序，避免把票价/房价编进计划。

import {
  addDays,
  enumerateDates,
  money,
  parseDate,
  shortDate,
  timeToMinutes,
  todayString,
  uid,
  uniqueBy,
  weekdayCn,
} from './travelUtils.js'

export const TRANSPORT_OPTIONS = [
  { value: 'driving', label: '自驾', hint: '含油费与过路费估算' },
  { value: 'transit', label: '公共交通', hint: '高铁/大巴/地铁换乘' },
  { value: 'tour', label: '跟团游', hint: '按团费另计大交通' },
  { value: 'walking', label: '普通散步', hint: '短途慢行，少赶路' },
]

export const BUDGET_LEVELS = [
  { value: 'economy', label: '省着花', hint: '青旅/民宿 · 本地小馆' },
  { value: 'comfort', label: '舒适', hint: '连锁酒店 · 有代表性的餐厅' },
  { value: 'premium', label: '不将就', hint: '精品酒店 · 预约制餐厅' },
]

export const INTEREST_OPTIONS = [
  '自然风光',
  '人文古迹',
  '博物馆',
  '美食',
  '摄影',
  '亲子',
  '徒步',
  '露营',
  '夜生活',
  '购物',
  '温泉',
  '小众',
]

export const PACE_OPTIONS = [
  { value: 'relaxed', label: '松弛', hint: '每天 1-2 个重点' },
  { value: 'balanced', label: '均衡', hint: '2-3 个重点 + 留白' },
  { value: 'intense', label: '紧凑', hint: '尽量多看，少留白' },
]

export const BUDGET_DEFAULTS = {
  hotel: { economy: 220, comfort: 450, premium: 850 },
  food: { economy: 70, comfort: 150, premium: 300 },
  shopping: { economy: 30, comfort: 80, premium: 180 },
}

const TICKET_DEFAULTS = [
  { match: ['博物馆', '美术馆', '纪念馆'], price: 30 },
  { match: ['公园', '广场', '湿地', '湖'], price: 20 },
  { match: ['古镇', '古城'], price: 50 },
  { match: ['乐园', '游乐'], price: 120 },
  { match: ['寺', '庙', '观'], price: 40 },
  { match: ['山', '峰', '峡谷'], price: 60 },
]

function budgetValue(level) {
  return BUDGET_DEFAULTS.food[level] ? level : 'comfort'
}

function defaultTicket(poi) {
  if (!poi) return 0
  const cost = Number(poi.cost)
  if (Number.isFinite(cost) && cost > 0) return cost
  const haystack = `${poi.name || ''} ${poi.category || ''} ${(poi.tags || []).join(' ')}`
  for (const rule of TICKET_DEFAULTS) {
    if (rule.match.some((keyword) => haystack.includes(keyword))) return rule.price
  }
  return 45
}

function planNights(form, days) {
  if (!days || days.length < 2) return 0
  const start = parseDate(form.startDate)
  const end = parseDate(form.endDate)
  if (start && end && end <= start) return 0
  return days.length - 1
}

export function computeBudget({ form, days, facts }) {
  const level = budgetValue(form.budgetLevel)
  const adults = Math.max(1, Number(form.adults) || 1)
  const children = Math.max(0, Number(form.children) || 0)
  const elders = Math.max(0, Number(form.elders) || 0)
  const people = adults + children + elders
  const weight = adults + children * 0.6 + elders * 0.8
  const ticketWeight = adults + children * 0.5 + elders * 0.5
  const nights = planNights(form, days)

  let transport = 0
  if (form.transport === 'tour') {
    transport = 40 * weight * days.length
  } else {
    const seen = new Set()
    for (const day of days) {
      for (const leg of day.legs || []) {
        const key = `${leg.mode}|${leg.distance}|${leg.tolls}|${leg.fare}|${leg.polyline}`
        if (seen.has(key)) continue
        seen.add(key)
        const distance = Number(leg.distance) || 0
        if (leg.mode === 'driving') {
          transport += distance * 0.85 + (Number(leg.tolls) || 0)
        } else if (leg.mode === 'transit') {
          const fare = Number(leg.fare) || 0
          transport += fare > 0 ? fare * weight : 4 * weight
        }
      }
    }
  }

  let tickets = 0
  const ticketNotes = []
  for (const day of days) {
    for (const block of day.blocks || []) {
      if (block.kind !== 'sight' && block.kind !== 'activity') continue
      if (!block.poi) continue
      const price = defaultTicket(block.poi)
      if (price <= 0) continue
      tickets += price * ticketWeight
      ticketNotes.push(`${block.poi.name} ¥${price}`)
    }
  }

  const food = BUDGET_DEFAULTS.food[level] * weight * Math.max(1, days.length)

  let lodging = 0
  let hotelName = ''
  if (nights > 0) {
    const hotelBlock = days
      .flatMap((day) => day.blocks || [])
      .find((block) => block.kind === 'hotel' && block.poi)
    const hotelCost =
      Number(hotelBlock?.poi?.cost) >= 100
        ? Number(hotelBlock.poi.cost)
        : BUDGET_DEFAULTS.hotel[level]
    hotelName = hotelBlock?.poi?.name || ''
    const rooms = Math.max(1, Math.ceil((adults + elders) / 2))
    lodging = rooms * nights * hotelCost
  }

  const subtotal = transport + tickets + food + lodging
  const shopping = BUDGET_DEFAULTS.shopping[level] * weight * Math.max(1, days.length)
  const other = subtotal * 0.08

  const items = [
    {
      category: '交通',
      label:
        form.transport === 'tour'
          ? '当地交通 / 停车（大交通按团费另计）'
          : form.transport === 'walking'
            ? '步行 / 零星公交'
            : '油费、过路费或车票',
      amount: Math.round(transport),
      note:
        form.transport === 'driving'
          ? '按 0.85 元/km + 高德返回的过路费估算'
          : form.transport === 'transit'
            ? '按高德公交票价 × 人数估算'
            : '按人数与天数估算',
    },
    {
      category: '门票',
      label: '景点门票 / 体验',
      amount: Math.round(tickets),
      note: ticketNotes.slice(0, 4).join(' · ') || '未选择收费景点',
    },
    {
      category: '餐饮',
      label: `每日餐饮（${BUDGET_LEVELS.find((item) => item.value === level)?.label}）`,
      amount: Math.round(food),
      note: `约 ¥${BUDGET_DEFAULTS.food[level]} / 人 / 天，儿童按 0.6 计`,
    },
    {
      category: '住宿',
      label: nights > 0 ? `住宿 ${nights} 晚 × ${Math.max(1, Math.ceil((adults + elders) / 2))} 间` : '住宿',
      amount: Math.round(lodging),
      note: nights > 0 ? hotelName || '按预算档位估算' : '当日往返，无住宿',
    },
    {
      category: '机动',
      label: '零食、购物与机动',
      amount: Math.round(shopping + other),
      note: `购物约 ¥${BUDGET_DEFAULTS.shopping[level]} / 人 / 天 + 8% 机动`,
    },
  ]

  // 明细各自取整后再求和，避免页面顶部与清单底部差 1 元。
  const total = items.reduce((sum, item) => sum + item.amount, 0)

  return {
    currency: 'CNY',
    items,
    total,
    perPerson: Math.round(total / Math.max(1, people)),
    people,
    nights,
    note:
      form.budgetTotal && total > Number(form.budgetTotal)
        ? `当前估算比目标预算高 ¥${money(total - Number(form.budgetTotal))}，可优先调整住宿档位或减少收费景点。`
        : '',
  }
}

function weatherRange(weather = []) {
  const mins = weather.map((item) => Number(item.nightTemp ?? item.dayTemp)).filter(Number.isFinite)
  const maxs = weather.map((item) => Number(item.dayTemp ?? item.nightTemp)).filter(Number.isFinite)
  return {
    min: mins.length ? Math.min(...mins) : null,
    max: maxs.length ? Math.max(...maxs) : null,
    rain: weather.some((item) =>
      `${item.dayWeather || ''}${item.nightWeather || ''}`.match(/雨|雪|雷/),
    ),
    strongSun: maxs.length ? Math.max(...maxs) >= 28 : false,
    cold: mins.length ? Math.min(...mins) <= 8 : false,
  }
}

export function buildPacking({ form, weather }) {
  const range = weatherRange(weather)
  const groups = [
    {
      category: '证件与预订',
      items: [
        '身份证',
        form.transport === 'driving' ? '驾驶证、行驶证' : '车票 / 登机牌',
        '酒店与门票预订确认单',
        form.children > 0 ? '儿童证件 / 户口本' : '',
        form.elders > 0 ? '老年证 / 医保卡' : '',
      ].filter(Boolean),
    },
    {
      category: '衣物',
      items: [
        '舒适步行鞋',
        range.cold ? '保暖外套 / 薄羽绒' : range.min != null && range.min < 18 ? '薄外套' : '透气短袖',
        range.strongSun ? '遮阳帽 / 防晒衣' : '',
        range.rain ? '折叠伞 / 轻便雨衣' : '',
        '换洗衣物（按天数 + 1 套）',
      ].filter(Boolean),
    },
    {
      category: '电子与路上',
      items: [
        '手机、充电器',
        '充电宝（随身携带）',
        form.transport === 'driving' ? '车充、车载支架' : '',
        form.interests?.includes('摄影') ? '相机、备用电池、存储卡' : '',
        '离线地图 / 行程截图',
      ].filter(Boolean),
    },
    {
      category: '药品与户外',
      items: [
        '常用药、创可贴',
        form.transport === 'driving' || form.transport === 'transit' ? '晕车药' : '',
        range.strongSun ? '防晒霜、润唇膏' : '',
        form.interests?.includes('徒步') ? '登山杖、护膝' : '',
        form.children > 0 ? '儿童湿巾、小零食' : '',
        '水杯 / 保温杯',
      ].filter(Boolean),
    },
  ]
  return groups.filter((group) => group.items.length)
}

function dueBefore(date, days) {
  const due = addDays(date, -Math.max(0, days))
  const today = todayString()
  return due && due < today ? today : due
}

export function buildChecklist({ form, days }) {
  const start = form.startDate
  const nights = planNights(form, days)
  const items = [
    {
      label: form.transport === 'transit' ? '购买往返车票 / 机票' : '确认往返交通与油量',
      due: dueBefore(start, 7),
      note: form.transport === 'transit' ? '节假日车票提前 15 天开售' : '',
    },
    nights > 0
      ? {
          label: '预订酒店并确认取消政策',
          due: dueBefore(start, 5),
          note: '优先选可免费取消的房型',
        }
      : null,
    {
      label: '预约热门景点 / 博物馆门票',
      due: dueBefore(start, 3),
      note: '周一闭馆的馆要避开',
    },
    {
      label: '查看出发前 48 小时天气与路况',
      due: dueBefore(start, 2),
      note: '有雨则启用备选方案',
    },
    {
      label: '下载离线地图与行程长图',
      due: dueBefore(start, 1),
      note: '山区/景区信号可能较弱',
    },
    {
      label: '安排宠物、植物与家中水电',
      due: dueBefore(start, 1),
      note: '',
    },
    {
      label: '把行程分享给家人',
      due: dueBefore(start, 1),
      note: '尤其是独自出行或长途自驾',
    },
    form.budgetLevel === 'premium' || days.length >= 4
      ? {
          label: '确认旅行保险',
          due: dueBefore(start, 3),
          note: '自驾/户外建议含紧急救援',
        }
      : null,
  ].filter(Boolean)

  return items.map((item) => ({
    id: uid('chk'),
    label: item.label,
    due: item.due,
    note: item.note,
    done: false,
  }))
}

export function buildAdvice({ form, facts, outline, weather }) {
  const range = weatherRange(weather)
  const advice = [
    ...(outline?.advice || []),
    ...(outline?.mustKnow || []),
  ].filter((item) => typeof item === 'string' && item.trim())

  if (form.pace === 'relaxed') {
    advice.push('每天只安排 1-2 个重点，把午后留白；排队超过 40 分钟的景点直接换备选。')
  } else if (form.pace === 'intense') {
    advice.push('紧凑行程最怕晚起：把第一站定在 8:30 前，午餐选景点附近以减少往返。')
  }
  if (range.rain) {
    advice.push('预报有雨：把户外观景改到上午，午后安排博物馆、茶馆或商场作为室内备选。')
  }
  if (range.strongSun) {
    advice.push('日间紫外线强：11:00-15:00 减少暴晒，帽子和防晒霜随身带。')
  }
  if (form.transport === 'driving') {
    advice.push('自驾请提前查景区停车场；节假日 9:00 后容易满位，优先停外围换乘停车场。')
    advice.push('连续驾驶不超过 2 小时，服务区休息 15 分钟；夜间尽量不走山路。')
  }
  if (form.transport === 'transit') {
    advice.push('公共交通预留 20 分钟换乘缓冲；跨城高铁注意车站与景点之间的距离。')
  }
  if (form.transport === 'tour') {
    advice.push('跟团游先确认集合点、购物点与自费项目；自由活动时间按导游给的时间提前 10 分钟回集合。')
  }
  if (facts?.source === 'osm') {
    advice.push('当前使用 OpenStreetMap 免费数据，出发前请在高德/大众点评复核营业时间与门票。')
  }
  advice.push('把本行程保存为长图或 PDF，离线也能查看；分享链接只在同一 Wi-Fi 下有效。')
  return uniqueBy(advice, (item) => item)
}

export function buildAlternatives({ form, facts, weather }) {
  const rainy = weather.some((item) =>
    `${item.dayWeather || ''}${item.nightWeather || ''}`.match(/雨|雪|雷/),
  )
  const museums = (facts?.attractions || []).filter((poi) =>
    `${poi.name}${poi.category}`.match(/博物馆|美术馆|纪念馆|展/),
  )
  const alternatives = []
  if (rainy) {
    alternatives.push(
      museums.length
        ? `雨天优先改去 ${museums[0].name}，室内且通常需要预约。`
        : '雨天把户外景点换成商场、咖啡馆、书店或当地博物馆。',
    )
  } else {
    alternatives.push('如果第一站排队过长，先去同区域的第二备选，下午再回来。')
  }
  if (form.transport === 'driving') {
    alternatives.push('景区停车位满时，停到外围换乘点，坐接驳车进入。')
  } else {
    alternatives.push('末班车较早，最后一站不要安排离车站太远的景点。')
  }
  return alternatives
}

export function defaultPlanTitle(form, days) {
  const city = form.destination?.city || form.destination?.name || '目的地'
  if (form.mode === 'weekend') {
    return `${city} · 周末两日`
  }
  return `${city} · ${Math.max(1, days?.length || 1)}日行程`
}

export function assemblePlan({
  form,
  facts,
  outline,
  days,
  provider = '',
  detailLevel = 'full',
  seriesId = '',
  weekendStart = '',
}) {
  const tripDates = enumerateDates(form.startDate, form.endDate)
  const matchedWeather = (facts.weather || []).filter((item) => tripDates.includes(item.date))
  const weather =
    matchedWeather.length > 0
      ? matchedWeather
      : (facts.weather || [])[0]?.date === tripDates[0]
        ? (facts.weather || []).slice(0, Math.max(1, tripDates.length))
        : []

  const destination = {
    ...(facts.destination || {}),
    ...(form.destination || {}),
    name: form.destination?.name || facts.destination?.name || '',
    address: form.destination?.address || facts.destination?.address || '',
    city: form.destination?.city || facts.destination?.city || '',
    lng: Number(form.destination?.lng) || facts.destination?.lng || 0,
    lat: Number(form.destination?.lat) || facts.destination?.lat || 0,
    source: facts.source || form.destination?.source || 'osm',
  }

  const normalizedDays = days.map((day, index) => ({
    ...day,
    date: day.date || tripDates[index] || form.startDate,
    weekday: day.weekday || weekdayCn(day.date || tripDates[index] || form.startDate),
    weather:
      day.weather ||
      weather.find((item) => item.date === (day.date || tripDates[index])) ||
      null,
  }))

  const budget = computeBudget({ form, days: normalizedDays, facts })
  const packing = buildPacking({ form, weather })
  const checklist = buildChecklist({ form, days: normalizedDays })
  const advice = buildAdvice({ form, facts, outline, weather })
  const alternatives = buildAlternatives({ form, facts, weather })
  const now = new Date().toISOString()
  const title = (outline?.title || '').trim() || defaultPlanTitle(form, normalizedDays)

  return {
    schemaVersion: 1,
    id: uid('trp'),
    kind: form.mode === 'weekend' ? 'weekend' : 'trip',
    seriesId,
    weekendStart,
    detailLevel,
    locked: false,
    title,
    subtitle: (outline?.subtitle || '').trim(),
    createdAt: now,
    updatedAt: now,
    origin: facts.origin,
    destination,
    startDate: form.startDate,
    endDate: form.endDate,
    startTime: form.startTime,
    endTime: form.endTime,
    transport: form.transport,
    budgetLevel: form.budgetLevel,
    budgetTotal: form.budgetTotal || null,
    travelers: {
      adults: Math.max(1, Number(form.adults) || 1),
      children: Math.max(0, Number(form.children) || 0),
      elders: Math.max(0, Number(form.elders) || 0),
    },
    interests: [...(form.interests || [])],
    pace: form.pace,
    source: facts.source || 'osm',
    coordSystem:
      facts.coordSystem ||
      facts.coord_system ||
      (['amap', 'tencent'].includes(facts.source)
        ? 'gcj02'
        : facts.source === 'baidu'
          ? 'bd09'
          : 'wgs84'),
    weather,
    days: normalizedDays,
    budget,
    packing,
    checklist,
    advice,
    alternatives,
    mustKnow: (outline?.mustKnow || []).filter((item) => typeof item === 'string'),
    factsMeta: {
      collectedAt: facts.collectedAt || now,
      poiCount:
        (facts.attractions?.length || 0) +
        (facts.food?.length || 0) +
        (facts.hotels?.length || 0),
      provider,
      notes: facts.notes || [],
    },
    posterMap: '',
    share: {
      startedAt: '',
      url: '',
    },
  }
}

export function itinerarySummary(plan) {
  if (!plan) return ''
  const start = plan.startDate ? shortDate(plan.startDate) : ''
  const end = plan.endDate && plan.endDate !== plan.startDate ? ` - ${shortDate(plan.endDate)}` : ''
  const people =
    (plan.travelers?.adults || 0) + (plan.travelers?.children || 0) + (plan.travelers?.elders || 0)
  return `${start}${end} · ${people} 人 · ¥${money(plan.budget?.total || 0)}`
}

export function blockEndMinutes(block) {
  return timeToMinutes(block.end)
}
