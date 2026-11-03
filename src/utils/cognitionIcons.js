/**
 * 认知层的图标映射。
 *
 * 与 `cognitionStore` 里的 `NUDGE_KINDS` / `PERCEPT_SOURCE_LABELS` 一一对应，但刻意
 * 放在 store 之外：store 是数据层，塞进 Vue 组件对象会让它不能在非组件环境里被复用。
 * 收件箱与左栏都要用同一套图标，所以也不能各写一份——两处不一致比没有图标更糟。
 */
import {
  Board20Regular,
  BookOpen20Regular,
  CalendarClock20Regular,
  Camera20Regular,
  Chat20Regular,
  Clipboard20Regular,
  Desktop20Regular,
  Document20Regular,
  Flash20Regular,
  Globe20Regular,
  Keyboard20Regular,
  Library20Regular,
  News20Regular,
  Notepad20Regular,
  PuzzlePiece20Regular,
  Rss24Regular,
  Sparkle20Regular,
  Timer20Regular,
  Warning20Regular,
  Alert20Regular,
} from '@vicons/fluent'

/** 提醒类型。图标要能单独说明"这是哪一类打扰"，颜色仍由 store 里的 color 决定。 */
export const NUDGE_ICONS = {
  contradiction: Warning20Regular,
  gap: PuzzlePiece20Regular,
  due: CalendarClock20Regular,
  review: BookOpen20Regular,
  emerging: Sparkle20Regular,
  stale: Timer20Regular,
}

/** 感知来源。左栏的筛选行与收件箱卡片共用。 */
export const PERCEPT_SOURCE_ICONS = {
  crawl: Globe20Regular,
  rss: Rss24Regular,
  capture: Camera20Regular,
  file: Document20Regular,
  clipboard: Clipboard20Regular,
  screen: Desktop20Regular,
  manual: Keyboard20Regular,
  chat: Chat20Regular,
  // M2：用户自有材料。摘录与资料库文档都算"你选的资料"，与外部抓取分开。
  note: Notepad20Regular,
  library: Library20Regular,
}

/** 投递渠道：这条提醒当初是怎么找到你的。 */
export const CHANNEL_ICONS = {
  board: Board20Regular,
  brief: News20Regular,
  notify: Alert20Regular,
  interrupt: Flash20Regular,
}

export const nudgeIcon = (kind) => NUDGE_ICONS[kind] || Sparkle20Regular
export const sourceIcon = (kind) => PERCEPT_SOURCE_ICONS[kind] || Document20Regular
export const channelIcon = (kind) => CHANNEL_ICONS[kind] || Board20Regular
