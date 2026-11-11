// 行迹旅行计划：纯函数与接入契约测试。
// 不启动 Tauri / 浏览器，盯三件事：
//   1. 卡片、路由、窗口尺寸、命令注册是否真的接通；
//   2. 预算明细与合计是否一致、行前日期是否已过期；
//   3. 模型 JSON 容错、POI 白名单与分享 HTML 转义。
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8')
const load = (p) => import(pathToFileURL(path.join(root, p)).href)

const travelDir = 'src/components/ApplicationViewComponent/apps/travel'
const { createDemoPlan, createDemoFacts } = await load(path.join(travelDir, 'travelData.js'))
const { computeBudget, assemblePlan, buildPacking } = await load(
  path.join(travelDir, 'travelPlan.js'),
)
const { normalizeOutline, normalizeDay, fallbackOutline, parseModelJson, buildDayPrompt } =
  await load(path.join(travelDir, 'travelPrompt.js'))
const { buildShareHtml, mapSourceLabel } = await load(path.join(travelDir, 'travelShare.js'))
const { buildContext, enrichCandidates, scoreCandidate, autoPickCandidates, buildExploreFacts } =
  await load(path.join(travelDir, 'travelExplore.js'))
const { platformSearchLinks } = await load(path.join(travelDir, 'travelLinks.js'))
const {
  mergePreferences,
  applyPreferencesToForm,
  learnFromForm,
  mapProviderLabel,
} = await load(path.join(travelDir, 'travelPreferences.js'))
const {
  todayString,
  enumerateDates,
  extractJson,
  wgs84ToGcj02,
  gcj02ToWgs84,
  parsePolyline,
  samplePolyline,
  seasonFor,
} = await load(path.join(travelDir, 'travelUtils.js'))

// ── 接入契约 ───────────────────────────────────────────────

test('应用卡片、路由、窗口与命令都已注册', () => {
  const content = read('src/components/ApplicationViewComponent/GuaxiContentAppView.vue')
  const router = read('src/router/index.js')
  const windowRs = read('src-tauri/src/commands/templateview.rs')
  const mainRs = read('src-tauri/src/main.rs')
  const commandsRs = read('src-tauri/src/commands.rs')

  assert.match(content, /id:\s*'life-travel-planner'/)
  assert.match(content, /category:\s*'life'/)
  assert.match(content, /TravelPlannerApp/)
  assert.match(router, /'life-travel-planner':\s*TravelPlannerApp/)
  assert.match(router, /path:\s*'\/travel-print'/)
  assert.match(windowRs, /"life-travel-planner"/)
  assert.match(commandsRs, /pub mod travel;/)
  for (const command of [
    'travel_collect_facts',
    'travel_explore',
    'travel_build_routes',
    'travel_route_weather',
    'travel_llm_complete',
    'travel_llm_web_complete',
    'travel_llm_local_complete',
    'travel_get_quota_usage',
    'travel_clear_cache',
    'travel_save_plan',
    'travel_share_start',
    'travel_open_print_window',
  ]) {
    assert.match(mainRs, new RegExp(`commands::travel::${command}`), `缺少命令注册：${command}`)
  }
})

// ── 预算与示例数据 ─────────────────────────────────────────

test('示例计划预算：明细合计与总数一致，行前日期不早于今天', () => {
  const plan = createDemoPlan()
  const sum = plan.budget.items.reduce((total, item) => total + item.amount, 0)
  assert.equal(plan.budget.total, sum)
  assert.equal(plan.budget.perPerson, Math.round(sum / 2))
  assert.equal(plan.days.length, 2)
  assert.ok(plan.days.every((day) => day.blocks.length > 0))
  assert.ok(plan.days.every((day) => day.blocks.every((block) => /^\d{2}:\d{2}$/.test(block.start))))
  assert.equal(plan.destination.name, '杭州西湖风景名胜区')
  assert.ok(plan.checklist.every((item) => !item.due || item.due >= todayString()))
  assert.ok(plan.days.flatMap((day) => day.blocks).filter((block) => block.booking?.needed).length >= 3)
})

test('computeBudget 按明细求和，且能识别超预算', () => {
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    transport: 'driving',
    budgetLevel: 'comfort',
    budgetTotal: 100,
    adults: 2,
    children: 0,
    elders: 0,
  }
  const days = [
    {
      blocks: [
        {
          kind: 'sight',
          poi: { name: '测试景点', category: '公园', cost: 20 },
        },
      ],
      legs: [{ mode: 'driving', distance: 10, tolls: 5, fare: 0 }],
    },
    { blocks: [], legs: [] },
  ]
  const budget = computeBudget({ form, days, facts: {} })
  const sum = budget.items.reduce((total, item) => total + item.amount, 0)
  assert.equal(budget.total, sum)
  assert.match(budget.note, /目标预算/)
  assert.equal(budget.nights, 1)
})

// ── 模型 JSON 容错 ─────────────────────────────────────────

test('extractJson 支持代码块与前后解释', () => {
  assert.deepEqual(extractJson('好的，结果如下：\n```json\n{"a":1}\n```\n希望有帮助'), { a: 1 })
  assert.deepEqual(parseModelJson('```json\n{"ok":true}\n```'), { ok: true })
  assert.throws(() => extractJson('没有 JSON'), /不是 JSON/)
})

test('normalizeOutline 过滤不存在的 POI，并补齐交通日期', () => {
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    pace: 'balanced',
    adults: 2,
    children: 0,
    elders: 0,
  }
  const facts = createDemoFacts(enumerateDates(form.startDate, form.endDate))
  const outline = normalizeOutline(
    {
      title: '测试大纲',
      days: [
        {
          date: form.startDate,
          theme: '第一天',
          stops: [
            { poiId: 'demo-bridge', kind: 'sight' },
            { poiId: '不存在的 id', kind: 'sight' },
          ],
        },
        { date: form.endDate, theme: '第二天', stops: [] },
      ],
    },
    { form, facts },
  )
  assert.equal(outline.days.length, 2)
  const known = new Set(
    [...facts.attractions, ...facts.food, ...facts.hotels].map((poi) => poi.id),
  )
  assert.ok(outline.days[0].stops.every((stop) => known.has(stop.poiId)))
  assert.equal(outline.days[1].hotelId, '')
})

test('normalizeDay 为漏写的路段补 transit 块', () => {
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    startTime: '08:30',
    endTime: '20:00',
    pace: 'balanced',
    adults: 2,
    children: 0,
    elders: 0,
    interests: [],
  }
  const facts = createDemoFacts(enumerateDates(form.startDate, form.endDate))
  const outline = fallbackOutline({ form, facts })
  const outlineDay = outline.days[0]
  const stopRefs = outlineDay.stops
    .map((stop) => [...facts.attractions, ...facts.food, ...facts.hotels].find((poi) => poi.id === stop.poiId))
    .filter(Boolean)
  const legs = stopRefs.slice(1).map((to, index) => ({
    mode: 'driving',
    distance: 2 + index,
    duration: 600,
    summary: '测试路线',
    steps: [],
    polyline: '',
  }))
  const day = normalizeDay(
    {
      theme: '测试日',
      blocks: [
        {
          start: '09:00',
          end: '10:30',
          kind: 'sight',
          poiId: stopRefs[0].id,
          title: '第一站',
        },
      ],
    },
    { form, facts, outlineDay, dayIndex: 0, legs, stopRefs, startName: facts.origin.name },
  )
  assert.ok(day.blocks.some((block) => block.kind === 'transit'))
  assert.ok(day.blocks.every((block) => /^\d{2}:\d{2}$/.test(block.start)))
})

// 回归：乌鲁木齐一天排喀纳斯、禾木这类跨城长路段时，确定性编排曾经一路排过午夜，
// 时间被截成 23:59 之后同一天出现「结束不晚于开始」，整份计划判失败（用户只能看到报错）。
test('确定性编排：当天路太长时少排一站，而不是排出 23:59 的假时间', async () => {
  const { fallbackDay } = await load(path.join(travelDir, 'travelPrompt.js'))
  const { validateDayBlocks } = await load(path.join(travelDir, 'travelValidate.js'))
  const poi = (id, name, kind, lng, lat) => ({ id, name, kind, lng, lat, city: '乌鲁木齐' })
  const origin = poi('origin', '乌鲁木齐站', 'origin', 87.6, 43.8)
  const sights = [
    poi('s1', '大巴扎步行街', 'sight', 87.62, 43.78),
    poi('s2', '喀纳斯景区-卧龙湾', 'sight', 87.03, 48.7),
    poi('s3', '禾木风景区', 'sight', 87.4, 48.6),
  ]
  const food = [poi('f1', '雪莲山餐饮·新疆地方菜(河南路店)', 'food', 87.58, 43.87)]
  const leg = (minutes, toName) => ({
    mode: 'driving',
    distance: minutes * 1.2,
    duration: minutes * 60,
    tolls: 0,
    fare: 0,
    summary: '测试路线',
    steps: [],
    polyline: '',
    toName,
  })
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    startTime: '08:30',
    endTime: '20:30',
    pace: 'balanced',
    transport: 'driving',
    radiusKm: 900,
    adults: 2,
    children: 0,
    elders: 0,
    interests: [],
  }
  const facts = {
    origin,
    destination: { name: '乌鲁木齐', city: '乌鲁木齐', lng: 87.6, lat: 43.8 },
    attractions: sights,
    food,
    hotels: [],
    weather: [],
  }
  const stopRefs = sights.map((item) => ({ ...item, poiId: item.id }))
  const legs = [leg(40, sights[0].name), leg(560, sights[1].name), leg(200, sights[2].name), leg(600, '乌鲁木齐')]
  const day = fallbackDay({
    form,
    facts,
    outlineDay: { date: '2026-11-14', theme: '测试日', area: '', hotelId: '' },
    dayIndex: 0,
    totalDays: 2,
    legs,
    stopRefs,
    startName: origin.name,
  })

  assert.ok(day.blocks.length > 0, '不能因为路太长就把这一天空掉')
  day.blocks.forEach((block) => {
    assert.match(block.start, /^\d{2}:\d{2}$/)
    assert.ok(block.end > block.start, `「${block.title}」${block.start}-${block.end} 时间倒挂`)
  })
  // 时间线必须单调，不能出现两段安排重叠。
  for (let index = 1; index < day.blocks.length; index += 1) {
    assert.ok(
      day.blocks[index].start >= day.blocks[index - 1].end,
      `「${day.blocks[index].title}」与上一段重叠`,
    )
  }
  const problems = validateDayBlocks(day, {
    center: facts.destination,
    radiusKm: 900,
    notBeforeMinutes: 510,
    endMinutes: 1230,
    home: origin,
    requireHotel: true,
  })
  assert.deepEqual(problems.filter((item) => /结束时间不晚于开始时间|时间重叠/.test(item)), [])
  // 排不进去的那站要说清楚，不能悄悄少一站。
  assert.ok(day.tips.some((item) => /没排进来/.test(item)))
  // 时间线对不上的路段不能留在地图/预算里。
  assert.ok(day.legs.length < legs.length)
})

// ── 分享安全 ───────────────────────────────────────────────

test('分享 HTML 转义标题与地点，不执行用户输入', () => {
  const plan = createDemoPlan()
  plan.title = '<script>alert(1)</script>'
  plan.destination.name = '<img src=x onerror=alert(1)>'
  const html = buildShareHtml(plan)
  assert.ok(!html.includes('<script>alert(1)</script>'))
  assert.ok(!html.includes('<img src=x onerror=alert(1)>'))
  assert.ok(html.includes('&lt;script&gt;alert(1)&lt;/script&gt;'))
  assert.match(html, /由 Guaxi生成/)
})

// ── 个人偏好与多地图 ───────────────────────────────────────

test('偏好会预填出发地；无车时默认交通改为公共交通', () => {
  const prefs = mergePreferences({
    home: { name: '家附近', city: '上海', lng: 121.4, lat: 31.2 },
    hasCar: false,
    defaultTransport: 'driving',
    interests: ['美食'],
    adults: 2,
    weekendRadiusKm: 30,
  })
  const form = applyPreferencesToForm(
    {
      origin: { name: '' },
      transport: 'driving',
      budgetLevel: 'comfort',
      pace: 'balanced',
      adults: 1,
      children: 0,
      elders: 0,
      interests: [],
      avoid: '',
      radiusKm: 60,
      mapProvider: 'auto',
    },
    prefs,
  )
  assert.equal(form.origin.name, '家附近')
  assert.equal(form.transport, 'transit')
  assert.deepEqual(form.interests, ['美食'])
  assert.equal(form.radiusKm, 30)
})

test('生成后会累计学习计数并记录最近出发地/目的地', () => {
  let prefs = mergePreferences({ autoLearn: true })
  prefs = learnFromForm(prefs, {
    mode: 'trip',
    origin: { name: '上海徐汇', city: '上海', lng: 121.4, lat: 31.2 },
    destination: { name: '杭州西湖', city: '杭州', lng: 120.1, lat: 30.2 },
    transport: 'transit',
    budgetLevel: 'comfort',
    pace: 'balanced',
    interests: ['美食', '自然风光'],
  })
  assert.equal(prefs.learned.transport.transit, 1)
  assert.equal(prefs.learned.interests['美食'], 1)
  assert.equal(prefs.lastOrigin.name, '上海徐汇')
  assert.equal(prefs.learned.destinations[0].name, '杭州西湖')
})

test('地图服务商标签与坐标转换可用', () => {
  assert.equal(mapProviderLabel('baidu'), '百度')
  assert.equal(mapSourceLabel('google'), 'Google 数据')
  const [gLng, gLat] = wgs84ToGcj02(121.4737, 31.2304)
  const [wLng, wLat] = gcj02ToWgs84(gLng, gLat)
  assert.ok(Math.abs(wLng - 121.4737) < 0.0002)
  assert.ok(Math.abs(wLat - 31.2304) < 0.0002)
})

test('平台跳转只生成搜索链接，不抓取第三方数据', () => {
  const links = platformSearchLinks({ name: '楼外楼', city: '杭州' })
  assert.ok(links.length >= 3)
  assert.ok(links.every((link) => link.url.startsWith('https://')))
  assert.ok(links.some((link) => link.url.includes('dianping.com')))
  assert.ok(links.some((link) => link.url.includes('ctrip.com')))
  assert.ok(links.some((link) => link.url.includes('uri.amap.com')))
})

test('示例行程每条路段都有 2-3 条备选，且不再展示逐段步骤', () => {
  const plan = createDemoPlan()
  const legs = plan.days.flatMap((day) => day.legs || [])
  assert.ok(legs.length > 0)
  assert.ok(legs.every((leg) => Array.isArray(leg.alternatives) && leg.alternatives.length >= 2))
  const html = buildShareHtml(plan)
  assert.ok(!html.includes('展开逐段路线'))
  assert.ok(!html.includes('<details'))
  assert.match(html, /route-options/)
})

// ── 沿途天气与季节 ─────────────────────────────────────────

test('路线采样、季节与沿途天气进入计划和提示词', () => {
  assert.equal(seasonFor('2026-11-14').label, '秋季 · 深秋（11 月）')
  const coords = parsePolyline('120,30;122,30', null, null)
  assert.deepEqual(samplePolyline(coords, 0.5), [121, 30])

  const plan = createDemoPlan()
  assert.ok(plan.routeWeather.length >= 6)
  assert.ok(plan.routeWeather.every((point) => point.date && point.time))
  assert.match(plan.season.label, /秋/)
  assert.ok(plan.weatherMeta.total >= plan.routeWeather.length)
  assert.ok(plan.days.every((day) => (day.routeWeather || []).length > 0))
  assert.ok(
    plan.days[0].legs.every((leg) => (leg.weatherPoints || []).length >= 2),
    '每条路段都应有起终点天气',
  )
  const html = buildShareHtml(plan)
  assert.match(html, /沿途天气/)
  assert.match(html, /leg-weather/)
})

test('雾、横风与高海拔会写进行前清单和建议', () => {
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    transport: 'driving',
    adults: 2,
    children: 0,
    elders: 0,
    interests: [],
    pace: 'balanced',
  }
  const routeWeather = [
    {
      name: '山口',
      date: '2026-11-14',
      time: '10:00',
      weather: '有雾',
      temperature: 3,
      windSpeed: 45,
      elevation: 1800,
      visibility: 400,
      available: true,
    },
  ]
  const packing = buildPacking({ form, weather: [], routeWeather })
  const items = packing.flatMap((group) => group.items).join(' ')
  assert.match(items, /雾灯/)
  assert.match(items, /冲锋衣/)
  assert.match(items, /防风镜/)

  const dayPrompt = buildDayPrompt({
    form,
    facts: { weather: [], origin: { name: '上海' }, attractions: [], food: [], hotels: [] },
    outlineDay: { date: form.startDate, weekday: '周六', theme: '山口', area: '', stops: [] },
    dayIndex: 0,
    totalDays: 1,
    legs: [
      {
        mode: 'driving',
        distance: 80,
        duration: 5400,
        fromName: '上海',
        toName: '山口',
        weatherPoints: routeWeather,
      },
    ],
    startName: '上海',
  })
  assert.match(dayPrompt.prompt, /沿途天气/)
  assert.match(dayPrompt.prompt, /有雾/)
  assert.match(dayPrompt.prompt, /季节：秋季/)
})

// ── 目的地候选池 ───────────────────────────────────────────

const exploreForm = {
  startDate: '2026-11-14',
  endDate: '2026-11-15',
  transport: 'driving',
  budgetLevel: 'comfort',
  adults: 2,
  children: 0,
  elders: 0,
  interests: ['自然风光'],
  pace: 'balanced',
}

const exploreCandidate = (overrides = {}) => ({
  id: 'c1',
  name: '西湖',
  kind: 'sight',
  categoryKey: 'nature',
  categoryLabel: '',
  baseName: '杭州',
  baseId: '330100',
  lng: 120.13,
  lat: 30.24,
  distanceBaseKm: 4,
  distanceOriginKm: 180,
  rating: 4.7,
  cost: 0,
  tags: ['风景区', '湖'],
  source: 'amap',
  must: false,
  ...overrides,
})

test('候选池：评分偏向口碑好、离基地近的地点', () => {
  const context = buildContext({ form: exploreForm })
  const good = scoreCandidate(
    exploreCandidate({ id: 'good', rating: 4.9, distanceBaseKm: 2 }),
    context,
  )
  const weak = scoreCandidate(
    exploreCandidate({ id: 'weak', rating: 3.8, distanceBaseKm: 55, name: '远山' }),
    context,
  )
  assert.ok(Number.isInteger(good.score) && good.score >= 0 && good.score <= 100)
  assert.ok(good.score > weak.score)
  assert.match(good.reason, /评分 4\.9/)
  assert.ok(good.metrics.access > weak.metrics.access)
})

test('候选池：雨天室内场馆得分高于纯户外', () => {
  const rainContext = buildContext({
    form: { ...exploreForm, interests: ['博物馆'] },
    weather: [{ date: '2026-11-14', dayWeather: '中雨', dayTemp: 16 }],
    dates: ['2026-11-14', '2026-11-15'],
  })
  const museum = scoreCandidate(
    exploreCandidate({ id: 'museum', categoryKey: 'museum', name: '浙江省博物馆', tags: ['博物馆'] }),
    rainContext,
  )
  const nature = scoreCandidate(
    exploreCandidate({ id: 'nature', categoryKey: 'nature', name: '九溪烟树', tags: ['溪谷'] }),
    rainContext,
  )
  assert.ok(museum.indoor)
  assert.ok(!nature.indoor)
  assert.ok(museum.score > nature.score)
  assert.match(museum.reason, /室内/)
})

test('候选池：必去地点置顶并保留推荐理由', () => {
  const context = buildContext({ form: exploreForm })
  const must = scoreCandidate(exploreCandidate({ id: 'must', must: true, rating: 3.9 }), context)
  assert.ok(must.score >= 96)
  assert.match(must.reason, /必去/)
})

test('候选池：帮我挑按天数凑量、包含必去且不重复', () => {
  const context = buildContext({ form: exploreForm })
  const candidates = enrichCandidates(
    {
      candidates: [
        exploreCandidate({ id: 'must-1', name: '灵隐寺', categoryKey: 'temple', must: true }),
        exploreCandidate({ id: 'n1', name: '西湖', categoryKey: 'nature' }),
        exploreCandidate({ id: 'h1', name: '河坊街', categoryKey: 'street' }),
        exploreCandidate({ id: 'm1', name: '省博', categoryKey: 'museum' }),
        exploreCandidate({ id: 'k1', name: '九溪', categoryKey: 'hike' }),
        exploreCandidate({ id: 'l1', name: '雷峰塔', categoryKey: 'heritage' }),
        exploreCandidate({ id: 'f1', name: '楼外楼', kind: 'food', categoryKey: 'food' }),
        exploreCandidate({ id: 'f2', name: '知味观', kind: 'food', categoryKey: 'food' }),
        exploreCandidate({ id: 'f3', name: '绿茶', kind: 'food', categoryKey: 'food' }),
      ],
      hotels: [{ id: 'hotel-1', name: '全季', kind: 'hotel' }],
    },
    context,
  )
  const ids = autoPickCandidates(candidates.candidates, { form: exploreForm, dayCount: 2 })
  assert.equal(new Set(ids).size, ids.length)
  assert.ok(ids.includes('must-1'))
  assert.ok(ids.length >= 3)
  assert.ok(ids.length <= 2 * 3 + 2 * 2)
})

test('省级目的地：给出保底城市，省会排第一且直辖市不拆', async () => {
  const { provinceOf, fallbackCities } = await load(path.join(travelDir, 'travelRegions.js'))
  const xinjiang = provinceOf('新疆维吾尔自治区')
  assert.ok(xinjiang, '应识别出新疆')
  const cities = fallbackCities(xinjiang, 8)
  assert.ok(cities.length >= 6)
  assert.equal(cities[0].name, '乌鲁木齐市')
  // 其余城市也必须是地级行政区（市 / 地区 / 自治州），不能是「北京城区」这类虚拟节点。
  assert.ok(cities.slice(1).every((city) => /市$|地区$|自治州$|盟$/.test(city.name)))
  // 全路径地名也要能落回省份，而不是把「新疆维吾尔自治区乌鲁木齐市」当成一个省。
  assert.equal(provinceOf('新疆维吾尔自治区乌鲁木齐市')?.name, '新疆维吾尔自治区')
  // 用户常只写简称，必须也能命中。
  assert.equal(provinceOf('新疆')?.name, '新疆维吾尔自治区')
  assert.equal(provinceOf('内蒙')?.name, '内蒙古自治区')
  // 直辖市自我兜底：当作城市目的地，不拆分。
  assert.equal(provinceOf('北京市')?.name, '北京市')
})

test('候选池：事实包只带选中地点，住宿自动附上', () => {
  const context = buildContext({ form: exploreForm })
  const result = enrichCandidates(
    {
      source: 'amap',
      coordSystem: 'gcj02',
      origin: { name: '上海', city: '上海', lng: 121.43, lat: 31.18 },
      destination: { name: '杭州', city: '杭州', lng: 120.13, lat: 30.24 },
      candidates: [
        exploreCandidate({ id: 'a1', name: '西湖', score: 90 }),
        exploreCandidate({ id: 'a2', name: '灵隐寺', categoryKey: 'temple' }),
        exploreCandidate({ id: 'f1', name: '楼外楼', kind: 'food', categoryKey: 'food' }),
      ],
      hotels: [{ id: 'hotel-1', name: '全季', kind: 'hotel' }],
      weather: [],
      notes: ['示例'],
    },
    context,
  )
  const facts = buildExploreFacts(result, ['a1', 'f1'], exploreForm)
  assert.deepEqual(facts.attractions.map((poi) => poi.id), ['a1'])
  assert.deepEqual(facts.food.map((poi) => poi.id), ['f1'])
  assert.equal(facts.hotels[0].kind, 'hotel')
  assert.equal(facts.attractions[0].kind, 'sight')
  assert.ok(!('score' in facts.attractions[0]))
  assert.match(facts.notes.join(''), /勾选的才会/)
})

test('候选池：后端命令与扫描模块已接通', () => {
  assert.match(read('src-tauri/src/main.rs'), /commands::travel::travel_explore/)
  assert.ok(fs.existsSync(path.join(root, 'src-tauri/src/commands/travel/explore.rs')))
  assert.match(
    read('src-tauri/src/commands/travel/explore.rs'),
    /pub async fn explore\(/,
  )
})

// 高德 inputtips 的 city 字段恒为空、district 是「省+市+区」连写；原样塞进
// place/text 的 city 参数会静默返回 0 条候选——截图里四个基地全扫空的根因。
test('候选池：基地城市名必须收敛到城市级，扫描需有坐标兜底', () => {
  const amap = read('src-tauri/src/commands/travel/amap.rs')
  assert.match(amap, /fn city_from_district\(/)
  assert.match(amap, /city_from_district\(&district, city\)/)
  // 文本检索空手时按基地坐标再扫一遍，不能依赖城市名解析成功。
  const explore = read('src-tauri/src/commands/travel/explore.rs')
  assert.match(explore, /place_around\(/)
  // 省级目的地未选基地时要给出可点的保底城市。
  assert.match(explore, /base_candidates/)
  assert.match(read('src-tauri/src/main.rs'), /commands::travel::travel_suggest_bases/)
})

// ── 行程合理性：在途预算与到家门口吃饭 ─────────────────────

test('单日在途上限：短日按比例收紧，长日有绝对上限', async () => {
  const { dailyTransitLimitMinutes } = await load(
    path.join(travelDir, 'travelEngine.js'),
  )
  // 8 小时的一天、均衡节奏：一半在车上就已经很多。
  assert.equal(dailyTransitLimitMinutes({ pace: 'balanced' }, 8 * 60), 240)
  // 同样节奏，只有 5 小时可用时上限随之下降，不能硬塞 4 小时车程。
  assert.ok(dailyTransitLimitMinutes({ pace: 'balanced' }, 5 * 60) < 240)
  // 松弛比紧凑更早触顶。
  assert.ok(
    dailyTransitLimitMinutes({ pace: 'relaxed' }, 10 * 60) <
      dailyTransitLimitMinutes({ pace: 'intense' }, 10 * 60),
  )
  // 再长的一天也封顶，避免「12 小时里 6 小时在车上」被当成合理。
  assert.ok(dailyTransitLimitMinutes({ pace: 'intense' }, 16 * 60) <= 240)
})

test('离家餐饮门槛：异地行程管、周末和本地行程不管', async () => {
  const { mealGuardKm } = await load(path.join(travelDir, 'travelEngine.js'))
  const shanghai = { lng: 121.4365, lat: 31.1879 }
  const hangzhou = { lng: 120.1302, lat: 30.2405 }
  const nearHome = { lng: 121.44, lat: 31.19 }
  assert.ok(mealGuardKm({ origin: shanghai, destination: hangzhou }) > 0)
  // 目的地就在家附近：不设门槛，否则家门口的餐馆全被判反常识。
  assert.equal(mealGuardKm({ origin: shanghai, destination: nearHome }), 0)
  // 周末本来就有 homeGuard 管景点与餐饮，这里不重复加码。
  assert.equal(
    mealGuardKm({ origin: shanghai, destination: hangzhou, mode: 'weekend' }),
    0,
  )
})

test('校验：在途超预算和收尾偏晚只提示，不改判失败', async () => {
  const { validatePlan } = await load(path.join(travelDir, 'travelValidate.js'))
  const plan = createDemoPlan()
  plan.startTime = '08:30'
  plan.endTime = '12:00'
  const report = validatePlan(plan, { center: plan.destination, radiusKm: 0 })
  const text = [...report.problems, ...report.advisories].join('；')
  assert.match(text, /在途|结束于/)
  // 这两项不能进 problems：否则整份计划会被退回重建，长线行程永远出不来。
  assert.ok(!report.problems.some((item) => /在途|结束于/.test(item)))
})

test('校验：返程日把午饭排回家门口会被拦下', async () => {
  const { validateDayBlocks } = await load(path.join(travelDir, 'travelValidate.js'))
  const origin = { name: '乌鲁木齐', city: '乌鲁木齐', lng: 87.62, lat: 43.82 }
  const destination = { name: '喀纳斯景区', city: '布尔津', lng: 87.04, lat: 48.69 }
  const day = {
    blocks: [
      {
        kind: 'transit',
        title: '前往喀纳斯',
        start: '08:30',
        end: '14:00',
        route: { distance: 600, duration: 330 * 60 },
      },
      {
        kind: 'sight',
        title: '喀纳斯湖',
        start: '14:00',
        end: '17:00',
        poi: { name: '喀纳斯湖', lng: 87.04, lat: 48.69, rating: 4.8 },
      },
      {
        kind: 'food',
        title: '家门口拌面',
        start: '19:00',
        end: '20:00',
        poi: { name: '家门口拌面', lng: 87.63, lat: 43.83 },
      },
    ],
  }
  const problems = validateDayBlocks(day, {
    center: destination,
    radiusKm: 200,
    home: origin,
    mealGuard: 2,
  })
  assert.ok(problems.some((item) => /快到家才吃饭/.test(item)))
  // 没有异地离家门槛时（本地行程），这条规则不该出现。
  const localProblems = validateDayBlocks(day, { center: destination, radiusKm: 3000, home: origin })
  assert.ok(!localProblems.some((item) => /快到家才吃饭/.test(item)))
})

// ── 候选池：额度与检索范围 ─────────────────────────────────

test('候选池：兴趣收窄检索类别，餐饮始终保留', async () => {
  const { categoriesForInterests } = await load(path.join(travelDir, 'travelExplore.js'))
  // 没选兴趣：返回空数组表示扫全部类别，不能退化成「一类都不扫」。
  assert.deepEqual(categoriesForInterests([]), [])
  const natural = categoriesForInterests(['自然风光'])
  assert.ok(natural.includes('nature'))
  assert.ok(natural.includes('food'), '餐饮决定午饭晚饭，必须始终检索')
  assert.ok(!natural.includes('must'), '必去由后端单独处理')
  // 兴趣越少，类别越少：这正是省额度的来源。
  assert.ok(natural.length < categoriesForInterests(['自然风光', '人文古迹', '亲子']).length)
})

test('候选池：扫描要按额度缩短基地数，并如实标注数据来源', () => {
  const explore = read('src-tauri/src/commands/travel/explore.rs')
  // 扫描前看今日 POI 额度，不够时缩基地而不是扫到一半静默失败。
  assert.match(explore, /poi_usage_today\(\)/)
  assert.match(explore, /scan_places\.truncate\(affordable\)/)
  // 退到 OSM 后来源必须跟着变，否则页面会把无评分地点当成高德数据。
  assert.match(explore, /let used_amap = use_amap && !scan_places\.is_empty\(\)/)
  assert.match(explore, /if used_amap \{ "amap" \} else \{ "osm" \}/)
})

test('候选池：类别检索要并发，但有限流', () => {
  const explore = read('src-tauri/src/commands/travel/explore.rs')
  assert.match(explore, /SWEEP_CONCURRENCY/)
  assert.match(explore, /tokio::sync::Semaphore::new\(SWEEP_CONCURRENCY\)/)
  assert.match(explore, /join_all\(tasks\)/)
  // 前端必须真的把类别发出去，否则后端过滤是空的。
  assert.match(
    read(path.join(travelDir, 'travelStore.js')),
    /categoriesForInterests\(form\.interests/,
  )
})

// ── 沿途顺路餐饮 ───────────────────────────────────────────

test('沿途餐饮：只有跨饭点的长路段才需要沿线找饭', async () => {
  const { mealStopsAlongLegs } = await load(path.join(travelDir, 'travelEngine.js'))
  // 乌鲁木齐 → 布尔津：08:30 出发、600 km，午饭只能在路上解决。
  const long = mealStopsAlongLegs([{ distance: 600, duration: 330 * 60 }], { startTime: '08:30' })
  assert.equal(long.length, 1)
  assert.equal(long[0].meal, 'lunch')
  assert.ok(long[0].ratio > 0.2 && long[0].ratio < 0.9)
  // 市区 8 km 的移动不该触发沿线找餐。
  assert.equal(mealStopsAlongLegs([{ distance: 8, duration: 30 * 60 }], { startTime: '11:30' }).length, 0)
  // 傍晚出发的长路段对应晚饭。
  const evening = mealStopsAlongLegs([{ distance: 200, duration: 150 * 60 }], { startTime: '16:00' })
  assert.equal(evening[0]?.meal, 'dinner')
})

test('沿途餐饮：离景点远但贴着路线，不该被判「用餐不顺路」', async () => {
  const { validateDayBlocks } = await load(path.join(travelDir, 'travelValidate.js'))
  const along = { name: '途中服务区餐厅', lng: 87.10, lat: 46.0, alongTime: '12:30', alongMeal: 'lunch' }
  const day = {
    blocks: [
      {
        kind: 'transit',
        title: '前往布尔津',
        start: '08:30',
        end: '14:00',
        legIndex: 0,
        route: {
          distance: 600,
          duration: 330 * 60,
          fromLng: 87.62,
          fromLat: 43.82,
          toLng: 86.87,
          toLat: 47.70,
          polyline: '87.62,43.82;87.10,46.00;86.87,47.70',
        },
      },
      {
        kind: 'sight',
        title: '五彩滩',
        start: '14:30',
        end: '16:30',
        poi: { name: '五彩滩', lng: 86.87, lat: 47.70 },
      },
      { kind: 'food', title: '途中服务区餐厅', start: '12:30', end: '13:30', poi: along },
    ],
  }
  const problems = validateDayBlocks(day, { center: { lng: 86.87, lat: 47.7 }, radiusKm: 200 })
  assert.ok(!problems.some((item) => /用餐不顺路/.test(item)))
  // 同样的位置但不带沿途标记：照旧按「离其他安排太远」拦下。
  const plain = {
    ...day,
    blocks: day.blocks.map((block) =>
      block.kind === 'food' ? { ...block, poi: { ...block.poi, alongTime: '' } } : block,
    ),
  }
  const strict = validateDayBlocks(plain, { center: { lng: 86.87, lat: 47.7 }, radiusKm: 200 })
  assert.ok(strict.some((item) => /用餐不顺路/.test(item)))
})

test('沿途餐饮：命令已注册且前端会调用', () => {
  assert.match(read('src-tauri/src/main.rs'), /commands::travel::travel_along_meals/)
  const mod = read('src-tauri/src/commands/travel/mod.rs')
  assert.match(mod, /pub async fn travel_along_meals\(/)
  // 必须走 050000 餐饮分类并限制半径，避免沿线把景点也拉进来。
  assert.match(mod, /"050000"/)
  assert.match(read(path.join(travelDir, 'travelStore.js')), /api\.alongMeals\(/)
})

// ── 候选池：默认不勾 / 批量 / 基地切换 / 指令条 ─────────────

test('候选池指令：常见说法本地解析，不耗模型', async () => {
  const { parseExploreCommand } = await load(path.join(travelDir, 'travelExplore.js'))
  const bases = [{ id: '654000', name: '伊犁哈萨克自治州' }, { id: '654300', name: '阿勒泰地区' }]
  assert.equal(parseExploreCommand('重新检索', bases).action, 'refresh')
  assert.equal(parseExploreCommand('扩大到 150 公里', bases).km, 150)
  assert.equal(parseExploreCommand('切换到伊犁', bases).base, '654000')
  assert.equal(parseExploreCommand('看看阿勒泰', bases).base, '654300')
  assert.equal(parseExploreCommand('只要博物馆', bases).key, 'museum')
  assert.equal(parseExploreCommand('帮我挑', bases).action, 'pick')
  assert.equal(parseExploreCommand('清空', bases).action, 'clear')
  assert.equal(parseExploreCommand('全选', bases).mode, 'add')
  // 解析不了的返回 null，交给模型兜底，而不是瞎猜一个动作。
  assert.equal(parseExploreCommand('今天天气怎么样', bases), null)
})

test('候选池指令：模型返回被收敛到合法动作', async () => {
  const { normalizeExploreCommand } = await load(path.join(travelDir, 'travelExplore.js'))
  const bases = [{ id: '654000', name: '伊犁哈萨克自治州' }]
  assert.equal(normalizeExploreCommand({ action: ' bogus' }, bases).action, 'unknown')
  assert.equal(normalizeExploreCommand({ action: 'switch', base: '伊犁' }, bases).base, '654000')
  assert.equal(normalizeExploreCommand({ action: 'category', category: '博物馆' }, bases).key, 'museum')
  assert.equal(normalizeExploreCommand({ action: 'expand', km: -5 }, bases).km, 0)
  assert.equal(normalizeExploreCommand({ action: 'batch', mode: 'xx' }, bases).mode, 'add')
})

test('候选池：默认不自动勾选，基地增量检索且切换不重搜', () => {
  const store = read(path.join(travelDir, 'travelStore.js'))
  // 进入候选池不再 autoPick，只保留必去（重搜时并回仍存在的旧勾选）。
  assert.match(store, /默认不替用户勾选/)
  assert.match(store, /const kept = previousSelected\.filter/)
  assert.match(store, /\[\.\.\.new Set\(\[\.\.\.\(enriched\.selectedIds \|\| \[\]\), \.\.\.kept\]\)\]/)
  // 新增基地只检索该基地（bases 单元素），合并进总池。
  assert.match(store, /bases: \[placeRef\(place\)\]/)
  assert.match(store, /mergeExploreData\(state\.explore\.data, incoming\)/)
  // 切换基地是纯前端赋值，不调用 api.explore。
  assert.match(store, /function switchExploreBase\(id\) \{\s*state\.explore\.activeBase = id \|\| 'all'\s*\}/)
  // 批量与指令入口已导出。
  for (const fn of ['batchExploreSelection', 'switchExploreBase', 'expandExploreRange', 'interpretExploreCommand']) {
    assert.ok(store.includes(`${fn},`), `store 应导出 ${fn}`)
  }
  const board = read(path.join(travelDir, 'ExploreBoard.vue'))
  assert.match(board, /tr-explore-batch/)
  assert.match(board, /tr-explore-command/)
  assert.match(board, /switch-base/)
})

// ── 横向卡片条滚动与模型整改回路 ──────────────────────────

test('横向卡片条：竖向滚轮转横向，到头交还页面', async () => {
  const { resolveWheelScroll } = await load(path.join(travelDir, 'travelScrollAssist.js'))
  const base = { scrollWidth: 1000, clientWidth: 400 }
  // 竖向滚轮在可滚区间内被接管，并换算成横向位移。
  const mid = resolveWheelScroll({ ...base, deltaY: 120, scrollLeft: 0 })
  assert.equal(mid.handled, true)
  assert.equal(mid.next, 120)
  // 滚到头（差值小于 1px）就不再拦，页面继续上下滚。
  const atEnd = resolveWheelScroll({ ...base, deltaY: 120, scrollLeft: 600 })
  assert.equal(atEnd.handled, false)
  const atStart = resolveWheelScroll({ ...base, deltaY: -120, scrollLeft: 0 })
  assert.equal(atStart.handled, false)
  // 触控板横向手势、Ctrl+滚轮（缩放）、装不下的条都交回浏览器。
  assert.equal(resolveWheelScroll({ ...base, deltaX: 200, deltaY: 10, scrollLeft: 100 }).handled, false)
  assert.equal(resolveWheelScroll({ ...base, deltaY: 120, ctrlKey: true, scrollLeft: 100 }).handled, false)
  assert.equal(
    resolveWheelScroll({ scrollWidth: 400, clientWidth: 400, deltaY: 120, scrollLeft: 0 }).handled,
    false,
  )
  // 行模式的 delta 按行数放大，且不会越界。
  const rows = resolveWheelScroll({ ...base, deltaY: 3, deltaMode: 1, scrollLeft: 500 })
  assert.equal(rows.handled, true)
  assert.equal(rows.next, 554)
})

test('横向卡片条：五个卡片条都接上了 v-hscroll 指令', () => {
  const assist = read(path.join(travelDir, 'travelScrollAssist.js'))
  assert.match(assist, /addEventListener\('wheel', handleWheel, \{ passive: false \}\)/)
  assert.match(assist, /addEventListener\('keydown', handleKeydown\)/)
  const wired = [
    ['PlanWorkspace.vue', ['tr-weather-strip', 'tr-route-weather-track', 'tr-day-switcher']],
    ['PlanMap.vue', ['tr-map-days']],
    ['ExploreBoard.vue', ['tr-explore-toolbar']],
    ['WeekendBoard.vue', ['tr-board-list']],
    ['PrepPanel.vue', ['tr-mini-weather']],
  ]
  for (const [file, classes] of wired) {
    const content = read(path.join(travelDir, file))
    for (const cls of classes) {
      assert.match(
        content,
        new RegExp(`v-hscroll[^>]*class="${cls}"|class="${cls}"[^>]*v-hscroll`),
        `${file} 的 ${cls} 应带 v-hscroll`,
      )
      assert.match(content, /import \{ vHscroll \} from '\.\/travelScrollAssist\.js'/, `${file} 应导入 vHscroll`)
    }
  }
})

test('模型整改：校验失败会把问题清单和上一版输出讲回给模型', async () => {
  const { buildRepairFeedback } = await load(path.join(travelDir, 'travelPrompt.js'))
  // 没有反馈时是普通请求，feedback=null 也不能炸。
  assert.equal(buildRepairFeedback(null), '')
  assert.equal(buildRepairFeedback({}), '')
  const text = buildRepairFeedback({
    problems: ['第 2 天：断桥残雪 距离市中心 45 km，超出半径 20 km', '', '时间倒挂：19:00 的块早于 14:00'],
    previous: '{"days":[]}',
  })
  assert.match(text, /【校验未通过，逐条改正】/)
  // 空条目会被过滤掉后重新编号，问题清单保持紧凑。
  assert.match(text, /1\. 第 2 天：断桥残雪/)
  assert.match(text, /2\. 时间倒挂/)
  assert.doesNotMatch(text, /3\. /)
  assert.match(text, /【上一次的输出】/)
})

test('模型整改：normalizeDay 校验失败带出 problems 和上一版输出', () => {
  const form = {
    startDate: '2026-11-14',
    endDate: '2026-11-15',
    startTime: '08:30',
    endTime: '12:00',
    transport: 'driving',
    budgetLevel: 'comfort',
    radiusKm: 20,
    adults: 2,
    children: 0,
    elders: 0,
    interests: [],
    pace: 'balanced',
  }
  const facts = {
    weather: [],
    origin: { name: '上海市徐汇区徐家汇', lng: 121.44, lat: 31.19 },
    destination: { name: '杭州西湖风景名胜区', lng: 120.15, lat: 30.27 },
    attractions: [
      // 离西湖约 70 km：模型挑了超出半径的景点，校验应当拦下并给出可整改的问题。
      { id: 'poi-far', name: '幻影景点', lng: 120.9, lat: 30.3, rating: 4.5, category: '景区' },
    ],
    food: [],
    hotels: [],
    notes: [],
  }
  const outlineDay = { date: form.startDate, weekday: '周六', theme: '示例', area: '', stops: [] }
  const badDay = {
    day: '2026-11-14',
    theme: '示例',
    blocks: [
      { start: '09:00', end: '10:30', kind: 'sight', poiId: 'poi-far', title: '幻影景点' },
    ],
  }
  assert.throws(
    () =>
      normalizeDay(badDay, {
        form,
        facts,
        outlineDay,
        dayIndex: 0,
        totalDays: 1,
        legs: [],
        stopRefs: [],
        startName: '上海',
      }),
    (error) => {
      assert.equal(error.code, 'day-validation')
      assert.ok(Array.isArray(error.problems) && error.problems.length > 0)
      assert.ok(error.problems.some((item) => /超出 20 km 范围/.test(String(item))))
      assert.ok(error.previous)
      return true
    },
  )
})
