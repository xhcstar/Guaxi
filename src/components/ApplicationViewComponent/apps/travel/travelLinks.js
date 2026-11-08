// 平台跳转：只打开搜索结果/详情页，不抓取、不缓存第三方平台的实时价格、评分与评论。
// 使用搜索引擎的 site: 限定跳转，避免依赖各平台经常变化的搜索 URL 结构。

export function platformSearchLinks(poi) {
  if (!poi?.name) return []
  const query = `${poi.name}${poi.city ? ` ${poi.city}` : ''}`.trim()
  const site = (domain) =>
    `https://www.bing.com/search?q=${encodeURIComponent(`site:${domain} ${query}`)}`
  return [
    { label: '大众点评', hint: '实时评分 / 评论', url: site('dianping.com') },
    { label: '携程', hint: '酒店房态 / 价格', url: site('ctrip.com') },
    { label: '美团', hint: '餐饮 / 酒店', url: site('meituan.com') },
    { label: '马蜂窝', hint: '攻略 / 游记', url: site('mafengwo.cn') },
    { label: '高德', hint: '地点与营业时间', url: amapSearchUrl(poi) },
  ]
}

export function amapSearchUrl(poi) {
  const keyword = encodeURIComponent(poi?.name || '')
  const city = encodeURIComponent(poi?.city || '')
  return `https://uri.amap.com/search?keyword=${keyword}&city=${city}&src=guaxi&coordinate=gaode&callnative=0`
}

export async function openExternalUrl(url) {
  if (!url) return
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}
