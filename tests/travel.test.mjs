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
const { computeBudget, assemblePlan } = await load(path.join(travelDir, 'travelPlan.js'))
const { normalizeOutline, normalizeDay, fallbackOutline, parseModelJson } = await load(
  path.join(travelDir, 'travelPrompt.js'),
)
const { buildShareHtml, mapSourceLabel } = await load(path.join(travelDir, 'travelShare.js'))
const { platformSearchLinks } = await load(path.join(travelDir, 'travelLinks.js'))
const {
  mergePreferences,
  applyPreferencesToForm,
  learnFromForm,
  mapProviderLabel,
} = await load(path.join(travelDir, 'travelPreferences.js'))
const { todayString, enumerateDates, extractJson, wgs84ToGcj02, gcj02ToWgs84 } = await load(
  path.join(travelDir, 'travelUtils.js'),
)

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
    'travel_build_routes',
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

// ── 分享安全 ───────────────────────────────────────────────

test('分享 HTML 转义标题与地点，不执行用户输入', () => {
  const plan = createDemoPlan()
  plan.title = '<script>alert(1)</script>'
  plan.destination.name = '<img src=x onerror=alert(1)>'
  const html = buildShareHtml(plan)
  assert.ok(!html.includes('<script>alert(1)</script>'))
  assert.ok(!html.includes('<img src=x onerror=alert(1)>'))
  assert.ok(html.includes('&lt;script&gt;alert(1)&lt;/script&gt;'))
  assert.match(html, /由行迹生成/)
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
