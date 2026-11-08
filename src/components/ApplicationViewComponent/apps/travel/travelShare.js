// 手机分享页：完全自包含的 HTML（内联 CSS、地图 data URI），不发任何外部请求。
// 这样微信里打开不会因为离线/跨域变成白屏，也方便用户另存为文件。

import { escapeHtml, friendlyDate, money, shortDate, timeAgo, weekdayCn } from './travelUtils.js'
import { TRANSPORT_OPTIONS } from './travelPlan.js'
import { platformSearchLinks } from './travelLinks.js'

const CITY_CODES = {
  上海: 'SHA',
  杭州: 'HGH',
  北京: 'BJS',
  广州: 'CAN',
  深圳: 'SZX',
  成都: 'CTU',
  重庆: 'CKG',
  南京: 'NKG',
  苏州: 'SZV',
  武汉: 'WUH',
  西安: 'SIA',
  厦门: 'XMN',
  青岛: 'TAO',
  长沙: 'CSX',
  昆明: 'KMG',
  大理: 'DLU',
  丽江: 'LJG',
  三亚: 'SYX',
  海口: 'HAK',
  桂林: 'KWL',
  贵阳: 'KWE',
  宁波: 'NGB',
  无锡: 'WUX',
  绍兴: 'SXG',
  嘉兴: 'JXS',
  湖州: 'HZH',
  黄山: 'TXN',
  舟山: 'HSN',
}

export function cityCode(city) {
  const name = String(city || '').replace(/市$/, '')
  return CITY_CODES[name] || name.slice(0, 2).toUpperCase() || '—'
}

export function ticketCities(plan) {
  const fromCity = plan?.origin?.city || plan?.origin?.name || '出发地'
  const toCity = plan?.destination?.city || plan?.destination?.name || '目的地'
  return {
    from: fromCity,
    to: toCity,
    fromCode: cityCode(fromCity),
    toCode: cityCode(toCity),
  }
}

export function transportLabel(value) {
  return TRANSPORT_OPTIONS.find((item) => item.value === value)?.label || '自驾'
}

export function mapSourceLabel(source) {
  return (
    {
      amap: '高德数据',
      osm: 'OSM 数据',
      baidu: '百度数据',
      tencent: '腾讯数据',
      google: 'Google 数据',
      mapbox: 'Mapbox 数据',
    }[source] || '地图数据'
  )
}

export function poiSourceLabel(source) {
  return (
    {
      amap: '高德',
      osm: 'OSM',
      baidu: '百度',
      tencent: '腾讯',
      google: 'Google',
      mapbox: 'Mapbox',
      ai: 'AI 建议',
    }[source] || '来源'
  )
}

export function peopleCount(plan) {
  if (!plan) return 0
  if (Number(plan.people) > 0) return Number(plan.people)
  const travelers = plan.travelers || {}
  return (travelers.adults || 0) + (travelers.children || 0) + (travelers.elders || 0)
}

export function dateRangeText(plan) {
  if (!plan?.startDate) return ''
  if (plan.endDate === plan.startDate) return `${shortDate(plan.startDate)} ${weekdayCn(plan.startDate)}`
  return `${shortDate(plan.startDate)} — ${shortDate(plan.endDate)}`
}

export function daySummaryText(day) {
  const spots = (day.blocks || [])
    .filter((block) => block.poi && (block.kind === 'sight' || block.kind === 'food'))
    .map((block) => block.poi.name)
  return spots.slice(0, 3).join(' · ')
}

function weatherIcon(item) {
  const icon = item?.icon === 'sun' ? '☀' : item?.icon === 'rain' ? '☂' : item?.icon === 'snow' ? '❄' : item?.icon === 'storm' ? '⚡' : item?.icon === 'fog' ? '≋' : '☁'
  return icon
}

function dateLabel(value) {
  return friendlyDate(value)
}

function renderRoute(block) {
  const leg = block.route
  if (!leg) return ''
  const options =
    Array.isArray(leg.alternatives) && leg.alternatives.length
      ? leg.alternatives.slice(0, 3)
      : [leg]
  const optionHtml = options
    .map((option, index) => {
      const minutes = Number(option.duration) ? Math.round(Number(option.duration) / 60) : null
      const distance = Number(option.distance) ? Number(option.distance).toFixed(1) : ''
      const extra = [
        option.traffic,
        option.tolls ? `过路费 ¥${money(option.tolls)}` : '',
        option.fare ? `票价 ¥${money(option.fare)}` : '',
      ]
        .filter(Boolean)
        .join(' · ')
      return `<div class="route-option${index === 0 ? ' is-primary' : ''}"><div><span>${escapeHtml(option.label || (index === 0 ? '推荐' : `备选 ${index}`))}</span><strong>${minutes ? `${minutes} 分钟` : '—'}</strong></div><small>${escapeHtml(distance ? `${distance} km` : '')}${extra ? ` · ${escapeHtml(extra)}` : ''}</small></div>`
    })
    .join('')
  const routeWeather = (leg.weatherPoints || [])
    .map((point) =>
      point.available === false
        ? `<span class="unknown">${escapeHtml(point.name || '途经')} 临近更新</span>`
        : `<span><b>${escapeHtml(point.name || '途经')}</b> ${escapeHtml(point.time || '')} ${weatherIcon(point)} ${escapeHtml(point.weather || '')}${point.temperature != null ? ` ${Math.round(Number(point.temperature))}℃` : ''}</span>`,
    )
    .join(' → ')
  return `<div class="route"><div class="route-title">${escapeHtml(leg.summary || '路线方案')}</div><div class="route-options">${optionHtml}</div>${leg.warning ? `<div class="route-warning">${escapeHtml(leg.warning)}</div>` : ''}</div>${routeWeather ? `<div class="leg-weather">${routeWeather}</div>` : ''}`
}

function renderBlock(block) {
  const time = `${escapeHtml(block.start || '')}${block.end ? `–${escapeHtml(block.end)}` : ''}`
  const category =
    block.poi?.category || (block.kind === 'food' ? '餐饮' : block.kind === 'hotel' ? '住宿' : block.kind === 'transit' ? '交通' : '')
  const tips = (block.tips || []).map((tip) => `<li>${escapeHtml(tip)}</li>`).join('')
  const booking = block.booking?.needed
    ? `<div class="booking">需预订${block.booking.channel ? ` · ${escapeHtml(block.booking.channel)}` : ''}${block.booking.deadline ? ` · 截止 ${escapeHtml(dateLabel(block.booking.deadline))}` : ''} · 以官方渠道实时信息为准</div>`
    : ''
  const source = block.poi?.source
    ? `<span class="tag source">${escapeHtml(poiSourceLabel(block.poi.source))}</span>`
    : ''
  const links = block.poi
    ? `<div class="poi-links">${platformSearchLinks(block.poi)
        .map(
          (link) =>
            `<a href="${escapeHtml(link.url)}" target="_blank" rel="noopener noreferrer">${escapeHtml(link.label)}</a>`,
        )
        .join('')}<small>跳转平台搜索，价格/房态以实时为准</small></div>`
    : ''
  return `<article class="block block-${escapeHtml(block.kind)}">
  <div class="time">${time}</div>
  <div class="body">
    <div class="block-head"><h3>${escapeHtml(block.title || '')}</h3>${category ? `<span class="tag">${escapeHtml(category)}</span>` : ''}${source}</div>
    ${block.detail ? `<p>${escapeHtml(block.detail)}</p>` : ''}
    ${block.poi ? `<div class="poi-meta">${block.poi.rating ? `<span>★ ${escapeHtml(block.poi.rating)}</span>` : ''}${block.poi.address ? `<span>${escapeHtml(block.poi.address)}</span>` : ''}${block.poi.cost ? `<span>参考 ¥${escapeHtml(block.poi.cost)}</span>` : ''}</div>` : ''}
    ${links}
    ${tips ? `<ul class="tips">${tips}</ul>` : ''}
    ${booking}
    ${renderRoute(block)}
  </div>
</article>`
}

function renderDay(day, index) {
  const weather = day.weather
  const weatherText = weather
    ? `${weatherIcon(weather)} ${escapeHtml(weather.dayWeather || '')} ${weather.nightTemp ?? ''}~${weather.dayTemp ?? ''}℃`
    : '临近再确认天气'
  return `<section class="day">
  <div class="day-head">
    <div class="day-index">DAY ${String(index + 1).padStart(2, '0')}</div>
    <div class="day-title">
      <h2>${escapeHtml(day.theme || shortDate(day.date))}</h2>
      <p>${escapeHtml(shortDate(day.date))} · ${escapeHtml(day.weekday || weekdayCn(day.date))} · ${weatherText}</p>
    </div>
  </div>
  <div class="timeline">${(day.blocks || []).map(renderBlock).join('')}</div>
</section>`
}

function renderBudget(plan) {
  const budget = plan.budget || {}
  const rows = (budget.items || [])
    .map(
      (item) => `<tr><td>${escapeHtml(item.category || '')}<small>${escapeHtml(item.label || '')}</small></td><td>¥${money(item.amount)}</td></tr>`,
    )
    .join('')
  return `<section class="panel budget">
  <h2>花费清单</h2>
  <table>
    <tbody>${rows}</tbody>
    <tfoot>
      <tr><th>合计</th><th>¥${money(budget.total)}</th></tr>
      <tr><td colspan="2">人均 ¥${money(budget.perPerson)}${budget.note ? ` · ${escapeHtml(budget.note)}` : ''}</td></tr>
    </tfoot>
  </table>
</section>`
}

function renderPacking(plan) {
  const groups = (plan.packing || [])
    .map(
      (group) => `<div class="pack-group"><h3>${escapeHtml(group.category)}</h3><ul>${(group.items || []).map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul></div>`,
    )
    .join('')
  return `<section class="panel"><h2>行前准备</h2><div class="pack-grid">${groups}</div></section>`
}

function renderChecklist(plan) {
  const items = (plan.checklist || [])
    .map(
      (item) => `<li class="${item.done ? 'done' : ''}"><span class="check">${item.done ? '☑' : '☐'}</span><span>${escapeHtml(item.label)}</span>${item.due ? `<small>${escapeHtml(dateLabel(item.due))} 前</small>` : ''}</li>`,
    )
    .join('')
  return `<section class="panel"><h2>出发前确认</h2><ul class="checklist">${items}</ul></section>`
}

function renderAdvice(plan) {
  const advice = [...(plan.advice || []), ...(plan.alternatives || [])]
  if (!advice.length) return ''
  return `<section class="panel"><h2>旅行建议与备选</h2><ul class="advice">${advice.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul></section>`
}

function mapBlock(plan) {
  const map = String(plan.posterMap || '')
  if (map.startsWith('data:image/')) {
    return `<div class="map-frame"><img src="${map}" alt="路线示意图"/></div>`
  }
  return `<div class="map-frame map-fallback">
    <svg viewBox="0 0 800 260" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="800" height="260" fill="#efe7d7"/>
      <path d="M40 200 C180 40, 300 230, 430 110 S650 60, 760 160" fill="none" stroke="#0f7a3d" stroke-width="3" stroke-dasharray="9 8"/>
      <circle cx="40" cy="200" r="9" fill="#1d2a28"/><circle cx="430" cy="110" r="7" fill="#0f7a3d"/><circle cx="760" cy="160" r="9" fill="#4f6b57"/>
      <text x="56" y="225" font-size="17" fill="#1d2a28">出发</text><text x="700" y="195" font-size="17" fill="#1d2a28">抵达</text>
    </svg>
  </div>`
}

export function buildShareHtml(plan, options = {}) {
  const cities = ticketCities(plan)
  const shareUrl = options.shareUrl || ''
  const source = mapSourceLabel(plan.source)
  const title = plan.title || '旅行计划'
  const updated = plan.factsMeta?.collectedAt ? timeAgo(plan.factsMeta.collectedAt) : '未知'
  const meta = `${dateRangeText(plan)} · ${peopleCount(plan)} 人 · ${transportLabel(plan.transport)} · 人均 ¥${money(plan.budget?.perPerson)}`
  const days = (plan.days || []).map(renderDay).join('')
  const notes = (plan.factsMeta?.notes || []).map((item) => `<li>${escapeHtml(item)}</li>`).join('')
  const routeWeather = (plan.routeWeather || []).length
    ? `<section class="panel route-panel">
  <h2>沿途天气${plan.season?.label ? ` · ${escapeHtml(plan.season.label)}` : ''}</h2>
  <div class="weather">${plan.routeWeather
    .map((point) =>
      point.available === false
        ? `<div class="unknown"><b>${escapeHtml(point.name || '途经')}</b>${escapeHtml(shortDate(point.date))} ${escapeHtml(point.time || '')}<br/>临近更新<br/><small>${escapeHtml(point.message || '')}</small></div>`
        : `<div><b>${weatherIcon(point)} ${escapeHtml(point.weather || '')}</b>${escapeHtml(point.name || '途经')} · ${escapeHtml(shortDate(point.date))} ${escapeHtml(point.time || '')}<br/>${point.temperature != null ? ` ${Math.round(Number(point.temperature))}℃` : ''}${Number(point.precipitationProbability) >= 30 ? ` · 降水 ${Math.round(Number(point.precipitationProbability))}%` : ''}</div>`,
    )
    .join('')}</div>
  ${plan.weatherMeta?.notes?.length ? `<small class="meta">${plan.weatherMeta.notes.map((item) => escapeHtml(item)).join(' · ')}${plan.weatherMeta?.fetchedAt ? ` · 更新于 ${escapeHtml(timeAgo(plan.weatherMeta.fetchedAt))}` : ''}</small>` : ''}
</section>`
    : ''
  const shareNote = shareUrl
    ? `<div class="share-note">手机查看：${escapeHtml(shareUrl)}<br/><small>同一 Wi-Fi 下有效，电脑关闭或分享停止后失效。</small></div>`
    : ''

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1"/>
<title>${escapeHtml(title)} · Guaxi</title>
<style>
:root{color-scheme:light}
*{box-sizing:border-box}
html,body{margin:0;padding:0;background:#efe7d7;color:#1d2a28;overflow-x:hidden}
body{font-family:"PingFang SC","Microsoft YaHei",system-ui,sans-serif;line-height:1.6}
.page{max-width:820px;margin:0 auto;padding:18px 14px 40px;overflow-x:hidden}
.ticket{background:#fffaf0;border:1px solid rgba(29,42,40,.22);border-radius:18px;padding:22px 20px;position:relative;overflow:hidden}
.ticket:before{content:"";position:absolute;inset:8px;border:1px dashed rgba(29,42,40,.18);border-radius:12px;pointer-events:none}
.brand{font-size:12px;letter-spacing:.24em;color:#7c8a83;text-transform:uppercase}
.route{display:flex;align-items:center;gap:14px;margin:18px 0 6px;font-family:"Songti SC","Noto Serif SC",serif}
.route b{font-size:34px;font-weight:600;letter-spacing:.04em}
.route i{font-style:normal;color:#c4552d;font-size:24px}
.route small{display:block;font-family:ui-monospace,Consolas,monospace;font-size:11px;color:#7c8a83;letter-spacing:.18em}
.ticket h1{font-family:"Songti SC","Noto Serif SC",serif;font-size:26px;line-height:1.3;margin:12px 0 8px;overflow-wrap:anywhere}
.ticket .meta{font-size:13px;color:#4a5a55}
.ticket .source{display:inline-block;margin-top:12px;font-size:11px;padding:3px 9px;border:1px solid rgba(29,42,40,.25);border-radius:999px;color:#4a5a55}
.weather{display:flex;gap:10px;overflow-x:auto;padding:16px 2px 4px}
.weather div{min-width:96px;background:#fffaf0;border:1px solid rgba(29,42,40,.16);border-radius:12px;padding:10px 12px;font-size:12px}
.weather b{display:block;font-size:15px;margin-bottom:2px}
.map-frame{margin-top:16px;border-radius:16px;overflow:hidden;border:1px solid rgba(29,42,40,.16);background:#efe7d7}
.map-frame img{display:block;width:100%;height:auto}
.map-fallback svg{display:block;width:100%;height:auto}
.day{margin-top:26px;background:#fffaf0;border:1px solid rgba(29,42,40,.18);border-radius:18px;overflow:hidden}
.day-head{display:flex;flex-wrap:wrap;gap:14px;padding:18px 18px 12px;border-bottom:1px dashed rgba(29,42,40,.18)}
.day-index{font-family:ui-monospace,Consolas,monospace;font-size:12px;letter-spacing:.12em;color:#c4552d;padding-top:4px}
.day-title{min-width:0;flex:1}
.day-title h2{font-family:"Songti SC","Noto Serif SC",serif;font-size:21px;margin:0 0 3px;overflow-wrap:anywhere}
.day-title p{margin:0;font-size:12px;color:#7c8a83;overflow-wrap:anywhere}
.timeline{padding:8px 18px 18px}
.block{display:grid;grid-template-columns:56px minmax(0,1fr);gap:10px;padding:14px 0;border-bottom:1px dashed rgba(29,42,40,.13)}
.block:last-child{border-bottom:0}
.time{font-family:ui-monospace,Consolas,monospace;font-size:12px;color:#7c8a83;padding-top:2px;overflow-wrap:anywhere}
.body{min-width:0;overflow-wrap:anywhere}
.block-head{display:flex;align-items:baseline;gap:8px;flex-wrap:wrap}
.block h3{font-size:16px;margin:0;overflow-wrap:anywhere}
.block p{font-size:13px;color:#4a5a55;margin:5px 0 0;overflow-wrap:anywhere}
.tag{font-size:10px;letter-spacing:.06em;color:#4f6b57;border:1px solid rgba(79,107,87,.4);padding:1px 7px;border-radius:999px;white-space:nowrap}
.tag.source{color:#7c8a83;border-color:rgba(29,42,40,.2)}
.poi-links{display:flex;flex-wrap:wrap;align-items:center;gap:5px;margin-top:7px}
.poi-links a{padding:2px 7px;border:1px solid rgba(29,42,40,.16);border-radius:999px;color:#4f6b57;font-size:9.5px;text-decoration:none}
.poi-links a:hover{background:#efe7d7}
.poi-links small{width:100%;font-size:8.5px;color:#7c8a83}
.poi-meta{display:flex;gap:10px;flex-wrap:wrap;font-size:11px;color:#7c8a83;margin-top:6px;overflow-wrap:anywhere}
.poi-meta span{overflow-wrap:anywhere}
.tips{margin:7px 0 0;padding-left:17px;font-size:12px;color:#4a5a55;overflow-wrap:anywhere}
.tips li{margin:2px 0}
.booking{margin-top:7px;font-size:12px;background:#f3e7d6;border-left:3px solid #c4552d;padding:6px 9px;border-radius:4px}
.route{margin-top:8px;font-size:12px;overflow-wrap:anywhere}
.route-title{color:#4f6b57;font-size:11px;font-weight:600;overflow-wrap:anywhere}
.route-options{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:6px;margin-top:6px}
.route-option{min-width:0;padding:7px 8px;border:1px solid rgba(29,42,40,.14);border-radius:7px;background:#efe7d7}
.route-option.is-primary{border-color:rgba(196,85,45,.42);background:#f3e2d6}
.route-option>div{display:flex;align-items:baseline;justify-content:space-between;gap:5px}
.route-option span{font-size:9px;color:#7c8a83;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.route-option strong{font-family:ui-monospace,Consolas,monospace;font-size:10px;white-space:nowrap}
.route-option small{display:block;margin-top:2px;font-size:8.5px;line-height:1.5;color:#7c8a83;overflow-wrap:anywhere}
.route-warning{margin-top:5px;font-size:10px;line-height:1.5;color:#c4552d;overflow-wrap:anywhere}
.leg-weather{margin-top:6px;font-size:10.5px;line-height:1.7;color:#7c8a83;overflow-wrap:anywhere}
.leg-weather b{color:#4f6b57;font-weight:600}
.leg-weather .unknown{opacity:.6}
.weather .unknown{opacity:.6}
.route-panel{padding:14px 16px}
.route-panel h2{font-size:15px;margin-bottom:2px}
.block-transit h3{font-size:14px;color:#4f6b57}
.block-transit .time{color:#4f6b57}
.block-food h3{color:#8a4a2a}
.panel{margin-top:22px;background:#fffaf0;border:1px solid rgba(29,42,40,.18);border-radius:18px;padding:18px}
.panel h2{font-family:"Songti SC","Noto Serif SC",serif;font-size:19px;margin:0 0 12px}
.panel table{width:100%;border-collapse:collapse;font-size:13px}
.panel td,.panel th{padding:8px 0;border-bottom:1px dashed rgba(29,42,40,.14);text-align:left;vertical-align:top}
.panel td:last-child,.panel th:last-child{text-align:right;white-space:nowrap}
.panel td small{display:block;color:#7c8a83;font-size:11px}
.panel tfoot th{font-size:16px;padding-top:12px}
.pack-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:14px}
.pack-group h3{font-size:13px;margin:0 0 5px;color:#4f6b57}
.pack-group ul{margin:0;padding-left:16px;font-size:12px;color:#4a5a55}
.checklist,.advice{list-style:none;margin:0;padding:0;font-size:13px}
.checklist li{display:flex;gap:8px;align-items:baseline;padding:7px 0;border-bottom:1px dashed rgba(29,42,40,.12)}
.checklist li.done{color:#7c8a83;text-decoration:line-through}
.checklist small{margin-left:auto;color:#7c8a83;font-size:11px;white-space:nowrap}
.check{color:#c4552d}
.advice li{padding:6px 0 6px 16px;position:relative}
.advice li:before{content:"·";position:absolute;left:2px;color:#c4552d;font-size:20px;line-height:1}
.share-note{margin-top:18px;font-size:12px;color:#4a5a55;background:#f3e7d6;border-radius:12px;padding:10px 12px;word-break:break-all}
.footer{margin-top:26px;text-align:center;font-size:11px;color:#7c8a83}
.notes{margin:8px 0 0;padding-left:18px;font-size:12px;color:#4a5a55}
@media(max-width:420px){
  .route b{font-size:28px}
  .ticket h1{font-size:22px}
  .block{grid-template-columns:52px minmax(0,1fr);gap:8px}
  .route-options{grid-template-columns:1fr}
  .page{padding:12px 10px 30px}
}
@media print{
  body{background:#fff}
  .page{max-width:none;padding:0}
  .ticket,.day,.panel{break-inside:avoid;box-shadow:none}
}
</style>
</head>
<body>
<main class="page">
  <header class="ticket">
    <div class="brand">Guaxi · Travel Journal</div>
    <div class="route">
      <div><b>${escapeHtml(cities.from)}</b><small>${escapeHtml(cities.fromCode)}</small></div>
      <i>→</i>
      <div><b>${escapeHtml(cities.to)}</b><small>${escapeHtml(cities.toCode)}</small></div>
    </div>
    <h1>${escapeHtml(title)}</h1>
    ${plan.subtitle ? `<p class="meta">${escapeHtml(plan.subtitle)}</p>` : ''}
    <div class="meta">${escapeHtml(meta)}</div>
    <div class="source">${escapeHtml(source)} · ${escapeHtml(plan.factsMeta?.provider || 'Guaxi')} · 资料更新于 ${escapeHtml(updated)}</div>
    ${mapBlock(plan)}
  </header>

  ${(plan.weather || []).length ? `<div class="weather">${plan.weather
    .map(
      (item) => `<div><b>${weatherIcon(item)} ${escapeHtml(item.dayWeather || '')}</b>${escapeHtml(shortDate(item.date))} ${escapeHtml(weekdayCn(item.date))}<br/>${item.nightTemp ?? ''}~${item.dayTemp ?? ''}℃</div>`,
    )
    .join('')}</div>` : ''}

  ${routeWeather}
  ${days}
  ${renderBudget(plan)}
  ${renderPacking(plan)}
  ${renderChecklist(plan)}
  ${renderAdvice(plan)}
  ${notes ? `<section class="panel"><h2>数据与说明</h2><ul class="notes">${notes}</ul></section>` : ''}
  ${shareNote}
  <div class="footer">由 Guaxi生成 · ${escapeHtml(new Date().toLocaleString('zh-CN'))} · ${escapeHtml(source)}</div>
</main>
</body>
</html>`
}

/**
 * 生成长图/打印用的静态地图（后端绘制绿色路线 + 白色描边）。
 * 返回 data URL；没有可用的路线或服务商时返回空字符串。
 */
export async function buildPlanPosterMap(plan, api) {
  const coordSystem = plan?.coordSystem || (plan?.source === 'amap' ? 'gcj02' : 'wgs84')
  if (coordSystem !== 'gcj02') return ''
  const day = (plan?.days || []).find((item) =>
    (item.blocks || []).some((block) => block.route?.polyline),
  )
  const block = day?.blocks?.find((item) => item.route?.polyline)
  const destination = plan?.destination
  if (!block || !destination?.lng || !destination?.lat) return ''
  const points = String(block.route.polyline)
    .split(';')
    .map((point) => point.split(',').map(Number))
    .filter((pair) => pair.length === 2 && pair.every(Number.isFinite))
  if (!points.length) return ''
  return (
    (await api.staticMap({
      centerLng: Number(destination.lng),
      centerLat: Number(destination.lat),
      zoom: 11,
      width: 900,
      height: 380,
      path: points.slice(0, 120),
    })) || ''
  )
}
