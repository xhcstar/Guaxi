// 唯一的简历渲染入口：文档数据 + 模板 HTML -> 最终 HTML。
// 预览、导出、打印、缩略图都调用这里，避免三套 DOM 各自漂移。

export function escapeHtml(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

export function escapeAttr(value) {
  return escapeHtml(value).replace(/`/g, '&#96;')
}

export function toLines(text) {
  return String(text || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
}

const hasText = (value) => String(value ?? '').trim().length > 0

const ITEM_TEXT_KEYS = ['period', 'school', 'college', 'major', 'degree', 'company', 'department', 'role', 'name', 'description', 'details']

function isItemEmpty(type, item) {
  if (!item) return true
  if (type === 'skills') return !hasText(item.text)
  return !ITEM_TEXT_KEYS.some((key) => hasText(item[key]))
}

function renderList(lines, className = 'resume-list') {
  if (!lines.length) return ''
  return `<ul class="${className}">${lines.map((line) => `<li>${escapeHtml(line)}</li>`).join('')}</ul>`
}

function renderSectionBody(section) {
  const items = (section.items || []).filter((item) => !isItemEmpty(section.type, item))

  if (section.type === 'skills') {
    const lines = items.map((item) => item.text).filter(Boolean)
    return renderList(lines, 'resume-list skill-list')
  }

  if (section.type === 'education') {
    return items.map((item) => `
      <div class="timeline-row education-row">
        <div class="time-col">${escapeHtml(item.period)}</div>
        <div class="content-col">
          <div class="inline-meta">
            <strong>${escapeHtml(item.school)}</strong>
            <span>${escapeHtml(item.college)}</span>
            <span>${escapeHtml(item.major)}</span>
            <span>${escapeHtml(item.degree)}</span>
          </div>
        </div>
      </div>`).join('')
  }

  if (section.type === 'work') {
    return items.map((item) => `
      <div class="timeline-row">
        <div class="time-col">${escapeHtml(item.period)}</div>
        <div class="content-col">
          <div class="inline-meta">
            <strong>${escapeHtml(item.company)}</strong>
            <span>${escapeHtml(item.department)}</span>
            <span>${escapeHtml(item.role)}</span>
          </div>
          ${renderList(toLines(item.description))}
        </div>
      </div>`).join('')
  }

  // project / custom 使用同一套「标题 + 简介 + 要点」结构。
  return items.map((item) => `
      <div class="timeline-row project-row">
        <div class="time-col">${escapeHtml(item.period)}</div>
        <div class="content-col">
          <div class="project-title">${escapeHtml(item.name)}</div>
          ${hasText(item.description) ? `<p class="project-desc">${escapeHtml(item.description)}</p>` : ''}
          ${renderList(toLines(item.details))}
        </div>
      </div>`).join('')
}

function renderSection(section) {
  if (!section || section.visible === false) return ''
  const body = renderSectionBody(section)
  if (!body.trim()) return ''
  const title = hasText(section.title) ? section.title : '模块'
  return `<section class="resume-section section-${escapeAttr(section.type)}"><h2>${escapeHtml(title)}</h2>${body}</section>`
}

function buildContactItems(profile) {
  return [
    { label: '联系地址', value: profile.city },
    { label: '联系方式', value: profile.phone },
    { label: '电子邮箱', value: profile.email },
    { label: '个人主页', value: profile.website },
  ].filter((item) => hasText(item.value))
}

export function buildTemplateVariables(doc) {
  const data = doc?.data || {}
  const profile = data.profile || {}
  const visibleSections = (data.sections || []).filter((section) => section.visible !== false)

  const profileTitleBlock = hasText(profile.title)
    ? `<div class="resume-role">${escapeHtml(profile.title)}</div>`
    : ''
  const targetBlock = hasText(profile.target)
    ? `<div class="target-line"><strong>求职意向：</strong>${escapeHtml(profile.target)}</div>`
    : ''
  const avatarBlock = hasText(profile.avatar)
    ? `<div class="avatar-box"><img src="${escapeAttr(profile.avatar)}" alt="头像" /></div>`
    : ''
  const contactItemsHtml = buildContactItems(profile)
    .map((item) => `<div class="contact-item"><strong>${escapeHtml(item.label)}：</strong><span>${escapeHtml(item.value)}</span></div>`)
    .join('')

  const sectionsHtml = visibleSections.map(renderSection).filter(Boolean).join('\n')
  const byType = (type) => visibleSections.filter((section) => section.type === type)

  const sectionHtmlOfType = (type) => byType(type).map(renderSection).filter(Boolean).join('\n')
  const summarySection = hasText(profile.summary)
    ? `<section class="resume-section section-summary"><h2>个人优势</h2><p class="summary-text">${escapeHtml(profile.summary)}</p></section>`
    : ''

  return {
    'profile.name': escapeHtml(profile.name || '姓名'),
    'profile.title': escapeHtml(profile.title),
    'profile.city': escapeHtml(profile.city),
    'profile.phone': escapeHtml(profile.phone),
    'profile.email': escapeHtml(profile.email),
    'profile.website': escapeHtml(profile.website),
    'profile.target': escapeHtml(profile.target),
    'profile.summary': escapeHtml(profile.summary),
    'profile.avatar': escapeAttr(profile.avatar),
    profileTitleBlock,
    targetBlock,
    avatarBlock,
    contactItemsHtml,
    summarySection,
    sectionsHtml,
    sections: sectionsHtml,
    educationSection: sectionHtmlOfType('education'),
    worksSection: sectionHtmlOfType('work'),
    projectsSection: sectionHtmlOfType('project'),
    skillsSection: sectionHtmlOfType('skills'),
    customSections: sectionHtmlOfType('custom'),
    documentTitle: escapeHtml(doc?.title || '简历'),
  }
}

export function renderResumeHtml(doc, templateHtml) {
  const template = String(templateHtml || '').trim()
  if (!template) return ''
  const variables = buildTemplateVariables(doc)
  const rendered = template.replace(/\{\{\s*([\w.[\]-]+)\s*\}\}/g, (_, key) => (
    Object.prototype.hasOwnProperty.call(variables, key) ? variables[key] : ''
  ))
  return ensureStandaloneHtml(rendered, doc)
}

export function ensureStandaloneHtml(html, doc) {
  const content = String(html || '').trim()
  if (!content) return ''
  if (/<!doctype html/i.test(content) || /<html[\s>]/i.test(content)) return content
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <title>${escapeHtml(doc?.title || '简历')}</title>
</head>
<body>
${content}
</body>
</html>`
}

export function stripScriptsForPreview(html) {
  const content = String(html || '')
  if (!content) return ''
  const cleanup = (root) => {
    root.querySelectorAll('script').forEach((node) => node.remove())
    root.querySelectorAll('*').forEach((node) => {
      for (const attr of Array.from(node.attributes)) {
        if (/^on/i.test(attr.name)) node.removeAttribute(attr.name)
      }
    })
    return root
  }
  // srcdoc 的 sandbox 不含 allow-scripts，模板里残留的脚本会在控制台刷
  // "Blocked script execution" 报错；这里按 DOM 解析后再剥除，避免正则漏掉
  // 带属性或非标准写法的 <script>。
  if (typeof DOMParser !== 'undefined') {
    try {
      const doc = new DOMParser().parseFromString(content, 'text/html')
      cleanup(doc)
      const doctype = /^\s*<!doctype/i.test(content) ? '<!doctype html>\n' : ''
      return doctype + doc.documentElement.outerHTML
    } catch {
      // 解析失败时退回正则清理。
    }
  }
  return content
    .replace(/<script\b[^>]*>[^]*?<\/script\s*>/gi, '')
    .replace(/<script\b[^>]*\/\s*>/gi, '')
    .replace(/\son[a-z]+\s*=\s*"[^"]*"/gi, '')
    .replace(/\son[a-z]+\s*=\s*'[^']*'/gi, '')
}

export function documentHasContent(doc) {
  const data = doc?.data || {}
  const profile = data.profile || {}
  if (['name', 'title', 'phone', 'email', 'target', 'summary'].some((key) => hasText(profile[key]))) return true
  return (data.sections || []).some((section) => section.visible !== false && renderSectionBody(section).trim())
}

export function buildStandaloneHtml(doc, templateHtml) {
  return renderResumeHtml(doc, templateHtml)
}
