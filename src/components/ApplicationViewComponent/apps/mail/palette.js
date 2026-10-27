/**
 * 写信工具栏的「文字颜色 / 突出显示」调色板。
 *
 * 直接弹系统的 `<input type="color">` 有两个问题：
 * - 它是取色器而不是「选一个常用色」，用户每换一次颜色都要调一遍；
 * - Outlook / Word 的习惯是「一排标准色 + 最近用的颜色」，这更顺手。
 *
 * 这里的色板与 Office 的主题色同一套顺序（白/黑/灰 + 暖冷两组 + 深色一组），
 * 让从 Outlook 迁过来的用户不用重新找颜色。
 */

/** 标准色：4 行 × 5 列，与 Office 的「标准色」面板一致。 */
export const STANDARD_COLORS = [
  ['#ffffff', '#000000', '#e7e6e6', '#44546a', '#4472c4'],
  ['#ed7d31', '#a5a5a5', '#ffc000', '#5b9bd5', '#70ad47'],
  ['#264478', '#9e480e', '#636363', '#997300', '#255e91'],
  ['#43682b', '#f7cbac', '#d6dce4', '#fff2cc', '#ddebf7'],
]

/** 突出显示的常用底色：比文字色更浅，保证深色字仍然可读。 */
export const HIGHLIGHT_COLORS = [
  ['#ffff00', '#00ff00', '#00ffff', '#ff00ff', '#0000ff'],
  ['#ff0000', '#000080', '#008080', '#008000', '#800080'],
  ['#800000', '#808000', '#808080', '#c0c0c0', '#ffffff'],
  ['#fff2cc', '#ddebf7', '#e2efda', '#fce4d6', '#ededed'],
]

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
