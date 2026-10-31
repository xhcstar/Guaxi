// 邮箱客户端的主题层。
//
// 设计说明：客户端只认 CSS 变量（--mail-*），不认具体主题。内置两套（墨 / 纸）与用户
// 自定义主题都是同一组 token 的不同取值。这样「换主题」= 换一组变量，组件里不出现
// 任何条件判断，也不会出现某个面板忘了跟主题走的情况。
//
// token 清单必须与后端 `commands/mail/theme.rs::TOKENS` 一致：后端负责提供内置主题的
// 值（保证安装包里的两套主题不会因为前端版本不同而变样），前端负责把它们写到 DOM 上。

/** CSS 变量名前缀。变量统一挂在阅读器根节点上，不影响应用其余部分。 */
const PREPEND = '--mail-'

/** 与后端 TOKENS 对应；这里只用于「后端还没应答时的兜底」。 */
export const THEME_TOKENS = [
  'bg',
  'bgElevated',
  'bgSunken',
  'panel',
  'panelHover',
  'sidebar',
  'sidebarActive',
  'border',
  'borderStrong',
  'text',
  'textSecondary',
  'textMuted',
  'accent',
  'accentText',
  'accentSoft',
  'unreadDot',
  'star',
  'danger',
  'warning',
  'success',
  'selection',
  'codeBg',
  'quoteBar',
  'shadow',
  'readerBg',
]

/** 密度 → 行高与间距。 */
export const DENSITY_PRESETS = {
  compact: { rowHeight: 60, gap: 6, pad: 10, fontDelta: -1 },
  comfortable: { rowHeight: 78, gap: 8, pad: 14, fontDelta: 0 },
  spacious: { rowHeight: 96, gap: 10, pad: 18, fontDelta: 1 },
}

/** 后端不可用时的两套内置主题（与 backend theme.rs 同步维护）。 */
export const FALLBACK_THEMES = {
  ink: {
    id: 'ink',
    name: '墨',
    mode: 'dark',
    tokens: {
      bg: '#14171c',
      bgElevated: '#1a1e25',
      bgSunken: '#101317',
      panel: '#1a1e25',
      panelHover: '#222730',
      sidebar: '#171a20',
      sidebarActive: '#262c36',
      border: '#2a3038',
      borderStrong: '#3a424d',
      text: '#e2e6ec',
      textSecondary: '#a4adba',
      textMuted: '#6f7885',
      accent: '#7aa2f7',
      accentText: '#0d1014',
      accentSoft: 'rgba(122,162,247,0.14)',
      unreadDot: '#7dcfff',
      star: '#e0af68',
      danger: '#f7768e',
      warning: '#e0af68',
      success: '#9ece6a',
      selection: 'rgba(122,162,247,0.22)',
      codeBg: '#101317',
      quoteBar: '#3d4654',
      shadow: '0 12px 32px rgba(0,0,0,0.45)',
      readerBg: '#171a20',
    },
  },
  paper: {
    id: 'paper',
    name: '纸',
    mode: 'light',
    tokens: {
      bg: '#f7f6f3',
      bgElevated: '#ffffff',
      bgSunken: '#eeede9',
      panel: '#ffffff',
      panelHover: '#f2f1ed',
      sidebar: '#f1f0ec',
      sidebarActive: '#e6e4de',
      border: '#dedbd4',
      borderStrong: '#c6c2b9',
      text: '#22262c',
      textSecondary: '#5a616b',
      textMuted: '#8a9099',
      accent: '#2f6df6',
      accentText: '#ffffff',
      accentSoft: 'rgba(47,109,246,0.10)',
      unreadDot: '#2f6df6',
      star: '#c98a18',
      danger: '#cf3d4a',
      warning: '#b8791a',
      success: '#2f8a4a',
      selection: 'rgba(47,109,246,0.16)',
      codeBg: '#f2f1ed',
      quoteBar: '#c6c2b9',
      shadow: '0 10px 28px rgba(30,34,40,0.10)',
      readerBg: '#fbfaf7',
    },
  },
}

/** 把主题的 token 表转成 style 对象；`accentOverride` 非空时覆盖主色。 */
export function themeToStyle(theme, accentOverride = '') {
  const style = {}
  if (!theme?.tokens) return style
  for (const [key, value] of Object.entries(theme.tokens)) {
    if (value) style[`${PREPEND}${kebab(key)}`] = value
  }
  if (accentOverride && accentOverride.trim()) {
    style[`${PREPEND}accent`] = accentOverride.trim()
  }
  return style
}

function kebab(name) {
  return name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)
}

/** 阅读字体：`system` 时用系统 UI 字体栈。 */
export function readingFontStack(readingFont) {
  if (!readingFont || readingFont === 'system') {
    return "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', 'PingFang SC', system-ui, sans-serif"
  }
  return `'${readingFont}', -apple-system, 'Segoe UI', 'Microsoft YaHei', 'PingFang SC', sans-serif`
}

/** 等宽字体栈（源码视图 / 编辑器）。 */
export function monoFontStack(monoFont) {
  const family = monoFont?.trim() || 'Maple Mono NF CN'
  return `'${family}', 'JetBrainsMono Nerd Font Mono', Consolas, 'Courier New', monospace`
}

/** 按密度给出行高与字号。 */
export function densityMetrics(density, readingSize) {
  const preset = DENSITY_PRESETS[density] || DENSITY_PRESETS.comfortable
  return {
    rowHeight: `${preset.rowHeight}px`,
    gap: `${preset.gap}px`,
    pad: `${preset.pad}px`,
    readingFontSize: `${Math.max(11, Number(readingSize || 16) + preset.fontDelta)}px`,
  }
}

/** 选中的主题对象：自定义主题优先，其次内置，最后兜底。 */
export function resolveTheme(settings, builtinThemes, fallbackId = 'ink') {
  const list = builtinThemes?.length ? builtinThemes : Object.values(FALLBACK_THEMES)
  const all = [...list, ...(settings?.customThemes || [])]
  return (
    all.find((t) => t.id === settings?.theme) ||
    all.find((t) => t.id === fallbackId) ||
    list[0] ||
    FALLBACK_THEMES.ink
  )
}

/** 生成一封邮件头部的占位字母与配色（头像）。 */
export function avatarSeed(name, address) {
  const source = (name || address || '?').trim()
  const letter = (source[0] || '?').toUpperCase()
  let hash = 0
  for (let i = 0; i < source.length; i += 1) {
    hash = (hash * 31 + source.charCodeAt(i)) % 360
  }
  return { letter, hue: hash }
}
