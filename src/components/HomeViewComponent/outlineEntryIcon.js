// 大纲/面包屑条目图标的**唯一**解析处。
//
// 背景：面包屑导航、面包屑里的大纲选择列表、以及右侧大纲面板都要给同一批条目画图标。
// 以前这份逻辑只活在 GuaxiEditor.vue 里，大纲面板另用一套（一级用 List24、其余用 TextBulletListTree），
// 于是同一个标题在两处长得不一样。这里抽出来共用，改一处三处都变。
//
// 设计说明：
// 1. 标题按 level 给 heading-1..heading-6；超出范围（LaTeX 的 subparagraph 等）用 pilcrow 表示"段落"，
//    拿不到 level 时用 hash。
// 2. .org 任务块按类型 + 完成态：Bug 用 bug；未完成用 list-todo，已完成用 check-square。
// 3. 这里只负责"给什么图标"，不负责颜色——颜色由各处的 CSS 决定。
import heading1Icon from '@iconify-icons/lucide/heading-1'
import heading2Icon from '@iconify-icons/lucide/heading-2'
import heading3Icon from '@iconify-icons/lucide/heading-3'
import heading4Icon from '@iconify-icons/lucide/heading-4'
import heading5Icon from '@iconify-icons/lucide/heading-5'
import heading6Icon from '@iconify-icons/lucide/heading-6'
import hashIcon from '@iconify-icons/lucide/hash'
import pilcrowIcon from '@iconify-icons/lucide/pilcrow'
import bugIcon from '@iconify-icons/lucide/bug'
import listTodoIcon from '@iconify-icons/lucide/list-todo'
import checkSquareIcon from '@iconify-icons/lucide/check-square'

const HEADING_ICON_BY_LEVEL = {
  1: heading1Icon,
  2: heading2Icon,
  3: heading3Icon,
  4: heading4Icon,
  5: heading5Icon,
  6: heading6Icon,
}

/**
 * 按“标题层级 / 任务类型”取图标（iconify 图标对象，直接给 <Icon :icon="..."> 用）。
 * @param {{kind?:string, level?:number, taskType?:string, done?:boolean}} entry
 */
export const resolveOutlineEntryIcon = (entry) => {
  const kind = String(entry?.kind || '')
  if (kind === 'org-task') {
    if (String(entry?.taskType || '') === 'bug') return bugIcon
    return entry?.done ? checkSquareIcon : listTodoIcon
  }

  const level = Number(entry?.level || 0)
  if (HEADING_ICON_BY_LEVEL[level]) return HEADING_ICON_BY_LEVEL[level]
  return level > 6 ? pilcrowIcon : hashIcon
}
