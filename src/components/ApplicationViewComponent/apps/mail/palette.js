/**
 * 写信工具栏与表格菜单共用的「常用色」。
 *
 * 三处选颜色的地方（工具栏的字体颜色 / 突出显示、表格右键菜单的单元格文字 / 填充）
 * 以前各有一套色板：工具栏是 Office 标准色，单元格那套只有一排浅色，同一个红色
 * 在两处对不上。现在只保留这一份：文字色一套、填充色一套，工具栏和单元格菜单都从这里取。
 *
 * 直接弹系统的 `<input type="color">` 也有两个问题：它是取色器而不是「选一个常用色」，
 * 每换一次颜色都要重调；Outlook / Word 的习惯是「一排标准色 + 最近用的颜色」，更顺手。
 */

/** 文字色：3 行 × 5 = 黑灰白 + 常见彩色，深浅都有。 */
export const STANDARD_COLORS = [
  ['#000000', '#404040', '#7f7f7f', '#a6a6a6', '#ffffff'],
  ['#c00000', '#ff0000', '#ed7d31', '#ffc000', '#ffff00'],
  ['#00b050', '#70ad47', '#4472c4', '#0070c0', '#7030a0'],
]

/** 填充色：浅色打底（深色字仍然可读）+ 常见的饱和色。 */
export const HIGHLIGHT_COLORS = [
  ['#ffffff', '#f2f2f2', '#ddebf7', '#e2efda', '#fff2cc'],
  ['#fce4d6', '#ededed', '#ffc000', '#ff7c80', '#00b050'],
  ['#0070c0', '#7030a0', '#c00000', '#548235', '#000000'],
]

/** 拉平成一个个色块（表格菜单里是网格，不需要分行）。 */
export const TEXT_COLOR_LIST = STANDARD_COLORS.flat()
export const FILL_COLOR_LIST = HIGHLIGHT_COLORS.flat()

/** 最近用过的颜色最多记几个。 */
export const RECENT_LIMIT = 5

/**
 * 记一个「最近用过」的颜色：放到最前、去重、截断。
 * 返回新数组而不是就地改，方便直接赋给响应式 ref。
 */
export function pushRecent(list, color, limit = RECENT_LIMIT) {
  const value = String(color || '').trim().toLowerCase()
  if (!/^#[0-9a-f]{6}$/.test(value)) return list
  const next = [value, ...list.filter((item) => item !== value)]
  return next.slice(0, limit)
}
