// 内置简历模板：完整的 HTML 文档，用 {{ 变量 }} 接收表单数据。
// 表单、预览、导出、打印都走同一套模板，改这里即时生效。

export const BUILTIN_TEMPLATE_PREFIX = 'builtin:'

export const RESUME_TEMPLATE_CSS = `
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body {
  background: #eceff3;
  color: #22201d;
  font-family: "Microsoft YaHei", "PingFang SC", "Segoe UI", Arial, sans-serif;
  -webkit-print-color-adjust: exact;
  print-color-adjust: exact;
}
.resume-paper {
  width: 210mm;
  min-height: 297mm;
  margin: 0 auto;
  padding: 18mm 20mm 16mm;
  background: #fff;
  --resume-accent: #2f6fb0;
  --resume-text: #22201d;
  --resume-muted: #6f7680;
  --resume-line: #bcd5ec;
  --resume-soft: #f5f8fc;
}
.resume-theme-graphite {
  --resume-accent: #33363b;
  --resume-text: #1d1e20;
  --resume-muted: #71757c;
  --resume-line: #cfd2d6;
  --resume-soft: #f6f6f6;
}
.resume-theme-emerald {
  --resume-accent: #0e7c6b;
  --resume-text: #172723;
  --resume-muted: #647873;
  --resume-line: #b7ddd6;
  --resume-soft: #f2f8f7;
}

.resume-header {
  display: grid;
  grid-template-columns: 1fr 30mm;
  gap: 12mm;
  align-items: start;
  border-bottom: 1.5px solid var(--resume-line);
  padding-bottom: 7mm;
}
.resume-profile h1 {
  margin: 0 0 3mm;
  color: var(--resume-accent);
  font-family: Georgia, "Songti SC", "STSong", "SimSun", serif;
  font-size: 32px;
  font-weight: 700;
  line-height: 1.08;
  letter-spacing: .01em;
}
.resume-role {
  margin-bottom: 4mm;
  font-size: 14px;
  font-weight: 700;
  color: var(--resume-text);
  letter-spacing: .02em;
}
.contact-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1.8mm 7mm;
  font-size: 12px;
  color: var(--resume-text);
}
.contact-item strong,
.target-line strong { color: #161513; font-weight: 700; }
.target-line { margin-top: 2.6mm; font-size: 12.5px; }
.avatar-box {
  width: 28mm;
  height: 36mm;
  border: 1px solid var(--resume-line);
  overflow: hidden;
  background: var(--resume-soft);
}
.avatar-box img { width: 100%; height: 100%; object-fit: cover; display: block; }

.resume-section { margin-top: 7mm; break-inside: avoid; page-break-inside: avoid; }
.resume-section h2 {
  display: flex;
  align-items: center;
  gap: 4mm;
  margin: 0 0 4mm;
  color: var(--resume-accent);
  font-size: 16px;
  font-weight: 700;
  line-height: 1.2;
  letter-spacing: .06em;
}
.resume-section h2::after {
  content: "";
  flex: 1;
  border-top: 1.2px solid var(--resume-line);
}
.summary-text,
.project-desc {
  margin: 0 0 1.6mm;
  font-size: 12.5px;
  line-height: 1.8;
  color: var(--resume-text);
}
.timeline-row {
  display: grid;
  grid-template-columns: 34mm 1fr;
  gap: 6mm;
  margin-bottom: 3.6mm;
  break-inside: avoid;
  page-break-inside: avoid;
}
.time-col {
  color: var(--resume-muted);
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.inline-meta {
  display: grid;
  grid-template-columns: 1.2fr 1fr 1fr;
  gap: 3mm;
  color: var(--resume-muted);
  font-size: 12.5px;
  font-weight: 600;
}
.education-row .inline-meta { grid-template-columns: 1.1fr 1fr 1fr .5fr; }
.inline-meta strong { color: var(--resume-text); font-weight: 700; }
.resume-list {
  margin: 1.6mm 0 0;
  padding-left: 4.5mm;
  font-size: 12.5px;
  line-height: 1.78;
  color: var(--resume-text);
}
.resume-list li { margin: .4mm 0; }
.resume-list li::marker { color: var(--resume-accent); }
.skill-list { columns: 1; margin-left: 34mm; }
.project-title { margin-bottom: 1.4mm; font-size: 13px; font-weight: 700; color: var(--resume-text); }
.page-break { display: block; height: 0; margin: 0; padding: 0; break-before: page; page-break-before: always; }
.avoid-break { break-inside: avoid; page-break-inside: avoid; }

@page { size: A4; margin: 0; }
@media print {
  body { background: #fff; }
  .resume-paper { margin: 0; box-shadow: none; }
  .page-break { break-before: page; page-break-before: always; }
}
`

const BUILTIN_THEMES = [
  {
    key: 'blueprint',
    name: '蓝白商务',
    description: '清晰的蓝色分隔线，适合互联网与工程岗位',
    accent: '#2f6fb0',
    paper: '#ffffff',
  },
  {
    key: 'graphite',
    name: '墨灰极简',
    description: '低饱和灰阶，排版克制，适合研究与设计岗位',
    accent: '#33363b',
    paper: '#ffffff',
  },
  {
    key: 'emerald',
    name: '青绿',
    description: '沉静的绿色强调，适合产品与运营岗位',
    accent: '#0e7c6b',
    paper: '#ffffff',
  },
]

export const BUILTIN_THEMES_LIST = BUILTIN_THEMES

export function buildBuiltinTemplate(themeKey = 'blueprint') {
  const theme = BUILTIN_THEMES.find((item) => item.key === themeKey) || BUILTIN_THEMES[0]
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{{ profile.name }}</title>
  <style>${RESUME_TEMPLATE_CSS}</style>
</head>
<body>
  <!-- 想改配色：改 .resume-paper 上的 --resume-accent / --resume-line；
       想调模块顺序：移动下面几个 {{ }} 占位符；
       想隐藏模块：在编辑器的模块大纲里关闭显示。 -->
  <article class="resume-paper resume-theme-${theme.key}">
    <header class="resume-header">
      <div class="resume-profile">
        <h1>{{ profile.name }}</h1>
        {{ profileTitleBlock }}
        <div class="contact-grid">{{ contactItemsHtml }}</div>
        {{ targetBlock }}
      </div>
      {{ avatarBlock }}
    </header>

    {{ summarySection }}

    {{ sectionsHtml }}
  </article>
</body>
</html>`
}

export const BUILTIN_TEMPLATES = BUILTIN_THEMES.map((theme) => ({
  id: `${BUILTIN_TEMPLATE_PREFIX}${theme.key}`,
  key: theme.key,
  name: theme.name,
  description: theme.description,
  accent: theme.accent,
  html: buildBuiltinTemplate(theme.key),
}))

export function findBuiltinTemplate(id) {
  return BUILTIN_TEMPLATES.find((item) => item.id === id) || null
}

export function isBuiltinTemplateId(id) {
  return String(id || '').startsWith(BUILTIN_TEMPLATE_PREFIX)
}
