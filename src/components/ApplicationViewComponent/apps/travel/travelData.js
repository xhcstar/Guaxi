// 预览与离线演示数据：真实的应用里这些来自高德/OSM；浏览器预览用同一套结构跑通界面。
// 示例行程放在杭州，日期取最近的周六，避免预览里出现“已过期”的观感。

import { addDays, nextSaturday, weekdayCn } from './travelUtils.js'
import { assemblePlan } from './travelPlan.js'
import { fallbackDay, fallbackOutline } from './travelPrompt.js'

export const DEMO_ORIGIN = {
  id: 'demo-origin',
  name: '上海市徐汇区徐家汇',
  address: '上海市徐汇区徐家汇',
  city: '上海',
  adcode: '310104',
  cityCode: '021',
  lng: 121.4365,
  lat: 31.1879,
  source: 'amap',
}

export const DEMO_DESTINATION = {
  id: 'demo-hangzhou',
  name: '杭州西湖风景名胜区',
  address: '浙江省杭州市西湖区龙井路 1 号',
  city: '杭州',
  adcode: '330100',
  cityCode: '0571',
  lng: 120.1302,
  lat: 30.2405,
  source: 'amap',
}

const poi = (id, name, kind, category, lng, lat, rating, cost, address, distance, tags = []) => ({
  id,
  name,
  source: 'amap',
  kind,
  category,
  address,
  city: '杭州',
  lng,
  lat,
  rating,
  cost,
  tel: '',
  distance,
  tags,
})

const ATTRACTIONS = [
  poi('demo-bridge', '断桥残雪', 'sight', '风景名胜 · 西湖十景', 120.1487, 30.2594, 4.7, 0, '西湖区北山街', 1500, ['西湖', '拍照', '步行']),
  poi('demo-lingyin', '灵隐寺', 'sight', '寺庙 · 人文古迹', 120.1012, 30.2408, 4.8, 45, '西湖区法云弄 1 号', 6500, ['古寺', '山林', '需预约']),
  poi('demo-leifeng', '雷峰塔', 'sight', '古塔 · 西湖十景', 120.1489, 30.2312, 4.6, 40, '西湖区南山路 15 号', 2800, ['登塔', '日落']),
  poi('demo-museum', '浙江省博物馆（孤山馆区）', 'sight', '博物馆', 120.1552, 30.2534, 4.7, 0, '西湖区孤山路 25 号', 3200, ['免费', '周一闭馆']),
  poi('demo-hefang', '河坊街', 'sight', '历史文化街区', 120.1694, 30.2422, 4.4, 0, '上城区河坊街', 4300, ['小吃', '夜景']),
  poi('demo-longjing', '龙井村', 'sight', '茶园 · 村落', 120.1065, 30.2193, 4.6, 0, '西湖区龙井村', 7200, ['茶园', '徒步']),
  poi('demo-xixi', '西溪国家湿地公园', 'sight', '湿地公园', 120.0702, 30.2681, 4.6, 80, '西湖区天目山路 518 号', 11000, ['湿地', '摇橹船']),
  poi('demo-jiuxi', '九溪烟树', 'sight', '自然风光 · 溪谷', 120.1174, 30.1963, 4.6, 0, '西湖区龙井村南九溪十八涧', 5800, ['溪水', '徒步', '秋天']),
  poi('demo-liuhe', '六和塔', 'sight', '古塔 · 钱塘江', 120.1398, 30.2026, 4.5, 20, '西湖区之江路 16 号', 5200, ['登塔', '江景']),
  poi('demo-yunhe', '拱宸桥 · 京杭大运河', 'sight', '历史街区 · 运河', 120.1457, 30.3225, 4.5, 0, '拱墅区桥弄街', 9200, ['运河', '夜景', '博物馆']),
  poi('demo-liangzhu', '良渚古城遗址公园', 'sight', '考古遗址 · 公园', 119.9957, 30.3953, 4.6, 60, '余杭区凤都路与 104 国道交叉口', 20500, ['世界遗产', '亲子', '需预约']),
]

const FOOD = [
  poi('demo-louwailou', '楼外楼（孤山路店）', 'food', '杭帮菜', 120.1428, 30.2474, 4.4, 150, '西湖区孤山路 30 号', 1700, ['西湖醋鱼', '老字号']),
  poi('demo-zhiweiguan', '知味观（仁和路总店）', 'food', '杭帮菜 · 小吃', 120.1662, 30.2461, 4.3, 80, '上城区仁和路 83 号', 3900, ['小笼', '片儿川']),
  poi('demo-lvcha', '绿茶餐厅（龙井路店）', 'food', '融合菜', 120.1298, 30.2562, 4.5, 90, '西湖区龙井路 83 号', 2000, ['面包诱惑', '排队']),
  poi('demo-xinbailu', '新白鹿餐厅（西湖银泰店）', 'food', '杭帮菜', 120.1621, 30.2524, 4.4, 70, '上城区延安路 98 号', 3800, ['性价比', '蛋黄鸡翅']),
]

const HOTELS = [
  poi('demo-quanji', '全季酒店（杭州西湖店）', 'hotel', '连锁酒店', 120.1605, 30.2501, 4.5, 450, '上城区延安路 258 号', 3600, ['近地铁', '可取消']),
  poi('demo-guobin', '杭州西湖国宾馆', 'hotel', '精品酒店', 120.1352, 30.2396, 4.8, 1200, '西湖区杨公堤 18 号', 900, ['湖景', '园林']),
  poi('demo-qingzhiwu', '青芝坞民宿', 'hotel', '民宿', 120.1238, 30.2601, 4.6, 300, '西湖区青芝坞', 2400, ['安静', '近植物园']),
]

function weatherFor(dates) {
  const patterns = [
    { dayWeather: '晴', nightWeather: '晴', dayTemp: 20, nightTemp: 12, wind: '东南风', windLevel: '2', icon: 'sun', precipitation: 5 },
    { dayWeather: '多云', nightWeather: '阴', dayTemp: 22, nightTemp: 14, wind: '东风', windLevel: '2', icon: 'cloud', precipitation: 15 },
    { dayWeather: '小雨', nightWeather: '小雨', dayTemp: 18, nightTemp: 11, wind: '北风', windLevel: '3', icon: 'rain', precipitation: 70 },
  ]
  return dates.map((date, index) => ({
    date,
    weekday: weekdayCn(date),
    ...patterns[index % patterns.length],
    source: 'open-meteo',
  }))
}

const step = (instruction, road, distance, duration) => ({ instruction, road, distance, duration })

function routeLeg(from, to, mode, distance, duration, extra = {}) {
  const stepList = extra.steps || [
    step(`从 ${from.name} 出发`, '', 300, 120),
    step(`沿主路向 ${to.name} 方向行驶`, '城市主干道', distance * 0.6 * 1000, duration * 0.6),
    step(`到达 ${to.name} 附近，按导航进入停车场`, '', distance * 0.1 * 1000, duration * 0.1),
  ]
  const points = [
    `${from.lng},${from.lat}`,
    `${(from.lng + to.lng) / 2},${(from.lat + to.lat) / 2}`,
    `${to.lng},${to.lat}`,
  ].join(';')
  return {
    mode,
    distance,
    duration,
    tolls: extra.tolls || 0,
    fare: extra.fare || 0,
    traffic: extra.traffic || '',
    summary:
      extra.summary ||
      `${mode === 'walking' ? '步行' : mode === 'transit' ? '公共交通' : '驾车'} · ${distance.toFixed(1)} km · 约 ${Math.round(duration / 60)} 分钟`,
    steps: stepList,
    polyline: points,
    warning: extra.warning || '',
    fromName: from.name,
    toName: to.name,
  }
}

function buildDemoRoutes() {
  const day1 = [
    routeLeg(DEMO_ORIGIN, ATTRACTIONS[0], 'driving', 176.4, 9300, {
      tolls: 70,
      traffic: '缓行 1 段',
      steps: [
        step('从徐家汇出发，进入沪闵高架路', '沪闵高架路', 4200, 480),
        step('沿沪昆高速向杭州方向行驶，经过枫泾收费站', '沪昆高速', 68000, 3000),
        step('转入杭州绕城高速，从留下出口驶出', '杭州绕城高速', 22000, 1200),
        step('沿曙光路进入西湖景区，按停车场指示停车', '曙光路', 5200, 900),
      ],
    }),
    routeLeg(ATTRACTIONS[0], FOOD[0], 'walking', 1.8, 1500, {
      steps: [step('沿北山街向西步行，经过断桥', '北山街', 900, 720), step('到达孤山路楼外楼', '孤山路', 900, 780)],
    }),
    routeLeg(FOOD[0], ATTRACTIONS[3], 'walking', 1.1, 900, {
      steps: [step('沿孤山路向南步行至浙江省博物馆', '孤山路', 1100, 900)],
    }),
    routeLeg(ATTRACTIONS[3], HOTELS[0], 'driving', 4.6, 1200, {
      traffic: '缓行 1 段',
      steps: [step('沿北山街向东行驶', '北山街', 1800, 480), step('转入延安路，前往酒店', '延安路', 2800, 720)],
    }),
  ]
  const day2 = [
    routeLeg(HOTELS[0], ATTRACTIONS[1], 'driving', 8.2, 1800, {
      traffic: '畅通',
      steps: [step('从酒店出发，沿延安路向南', '延安路', 2200, 420), step('转入灵隐路，前往灵隐景区', '灵隐路', 6000, 1380)],
    }),
    routeLeg(ATTRACTIONS[1], FOOD[1], 'driving', 7.4, 1500, {
      steps: [step('从灵隐停车场驶出，沿灵隐路向东', '灵隐路', 5200, 1080), step('转入仁和路，到达知味观', '仁和路', 2200, 420)],
    }),
    routeLeg(FOOD[1], ATTRACTIONS[2], 'driving', 3.2, 900, {
      steps: [step('沿南山路向南行驶', '南山路', 3200, 900)],
    }),
    routeLeg(ATTRACTIONS[2], DEMO_ORIGIN, 'driving', 175.8, 9300, {
      tolls: 70,
      traffic: '缓行 2 段',
      steps: [
        step('从雷峰塔停车场出发，沿南山路向北', '南山路', 3000, 600),
        step('进入杭州绕城高速，向上海方向行驶', '杭州绕城高速', 24000, 1500),
        step('沿沪昆高速返回上海，经过枫泾收费站', '沪昆高速', 130000, 6300),
        step('从沪闵高架路返回徐家汇', '沪闵高架路', 18000, 900),
      ],
    }),
  ]
  return [day1, day2]
}

export function createDemoFacts(dates) {
  const tripDates = dates || [nextSaturday(), addDays(nextSaturday(), 1)]
  return {
    source: 'amap',
    origin: DEMO_ORIGIN,
    destination: DEMO_DESTINATION,
    attractions: ATTRACTIONS.map((item) => ({ ...item })),
    food: FOOD.map((item) => ({ ...item })),
    hotels: HOTELS.map((item) => ({ ...item })),
    weather: weatherFor(tripDates),
    notes: ['示例数据：杭州西湖及周边真实地点，路线与时间为演示用估算。'],
    collectedAt: new Date().toISOString(),
  }
}

export function createDemoPlan(overrides = {}) {
  const saturday = overrides.startDate || nextSaturday()
  const sunday = overrides.endDate || addDays(saturday, 1)
  const form = {
    mode: 'trip',
    origin: DEMO_ORIGIN,
    destination: DEMO_DESTINATION,
    startDate: saturday,
    endDate: sunday,
    startTime: '08:30',
    endTime: '20:30',
    transport: 'driving',
    budgetLevel: 'comfort',
    budgetTotal: 2600,
    adults: 2,
    children: 0,
    elders: 0,
    interests: ['自然风光', '美食', '博物馆', '摄影'],
    pace: 'balanced',
    keywords: '想看一眼西湖日落，也想吃一顿正经杭帮菜',
    mustVisit: '',
    avoid: '',
    radiusKm: 60,
    source: 'auto',
    ...overrides,
  }
  const dates = [saturday, sunday]
  const facts = createDemoFacts(dates)
  const outline = fallbackOutline({ form, facts })
  outline.title = '西湖两日 · 把时间浪费在湖风里'
  outline.subtitle = '断桥、孤山、灵隐与一顿杭帮菜，节奏不赶，给日落留位置。'
  outline.advice = [
    '第一天把西湖东岸留给步行，车停酒店后不再挪车。',
    '灵隐寺尽量 8:30 前到，避开旅行团；下午回程前留 40 分钟买龙井茶。',
    '楼外楼排队较长，可 11:00 前到店，或改去孤山附近的绿茶餐厅。',
  ]
  outline.mustKnow = [
    '浙江省博物馆孤山馆区周一闭馆，需提前在公众号预约。',
    '西湖景区周末单双号限行，出发前查当天规则。',
    '返程沪昆高速傍晚易堵，建议 19:00 前上高速。',
  ]
  outline.days = [
    {
      date: saturday,
      weekday: weekdayCn(saturday),
      theme: '西湖东岸 · 初见',
      area: '西湖 · 孤山',
      stops: [
        { poiId: 'demo-bridge', kind: 'sight' },
        { poiId: 'demo-louwailou', kind: 'food' },
        { poiId: 'demo-museum', kind: 'sight' },
      ],
      hotelId: 'demo-quanji',
    },
    {
      date: sunday,
      weekday: weekdayCn(sunday),
      theme: '灵隐与南山路 · 返程',
      area: '灵隐 · 南山路',
      stops: [
        { poiId: 'demo-lingyin', kind: 'sight' },
        { poiId: 'demo-zhiweiguan', kind: 'food' },
        { poiId: 'demo-leifeng', kind: 'sight' },
      ],
      hotelId: '',
    },
  ]

  const map = new Map(
    [...facts.attractions, ...facts.food, ...facts.hotels].map((item) => [item.id, item]),
  )
  const routes = buildDemoRoutes()
  const days = outline.days.map((outlineDay, index) => {
    const stopRefs = outlineDay.stops.map((item) => map.get(item.poiId)).filter(Boolean)
    return fallbackDay({
      form,
      facts,
      outlineDay,
      dayIndex: index,
      legs: routes[index],
      stopRefs,
      startName: index === 0 ? facts.origin.name : map.get(outline.days[index - 1].hotelId)?.name,
    })
  })
  // 最后一天的返程 leg 由 fallbackDay 统一补成 transit 块；这里只把完整 legs 挂到当天。
  const lastDay = days[days.length - 1]
  if (lastDay) {
    lastDay.legs = routes[routes.length - 1]
  }

  const book = (block, booking) => {
    if (block) block.booking = booking
  }
  book(days[0].blocks.find((block) => block.poiId === 'demo-museum'), {
    needed: true,
    channel: '浙江省博物馆公众号',
    deadline: addDays(saturday, -2),
    note: '孤山馆区周一闭馆，免费但需预约',
  })
  book(days[0].blocks.find((block) => block.kind === 'hotel'), {
    needed: true,
    channel: '酒店小程序 / 携程',
    deadline: addDays(saturday, -3),
    note: '选可免费取消的房型，西湖周末房价会上浮',
  })
  book(days[1].blocks.find((block) => block.poiId === 'demo-lingyin'), {
    needed: true,
    channel: '灵隐寺公众号',
    deadline: addDays(sunday, -1),
    note: '香花券 45 元，建议 8:30 前入园',
  })

  const plan = assemblePlan({
    form,
    facts,
    outline,
    days,
    provider: '内置示例',
    detailLevel: 'full',
  })
  plan.id = overrides.id || 'demo-hangzhou-weekend'
  plan.createdAt = new Date(Date.now() - 36 * 3600 * 1000).toISOString()
  plan.updatedAt = new Date().toISOString()
  return plan
}

export const DEMO_LIBRARY = [createDemoPlan()]
