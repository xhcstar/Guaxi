// GUAXI 任务块 DSL 的前端解析器（镜像 src-tauri/src/commands/task_block.rs 的 parse_blocks）。
// 用于编辑器分屏预览的实时解析（每次内容变更本地解析，无需 IPC）。

export const BLOCK_START = '##GUAXI_TASK_START'
export const BLOCK_END = '##GUAXI_TASK_END'

export const KNOWN_FIELDS = [
  'MANAGE TASK TYPE', 'INTERNAL ID', 'ID', 'PARENT TASK', 'TYPE', 'KIND', 'DEADLINE', 'FOCUS',
  'CREATE TIME', 'COMPLETED TIME', 'ORIGIN', 'SEVERITY', 'PRIORITY', 'STATUS', 'REPEAT', 'PROGRESS',
  'SPENT', 'CHECKINS',
  'FIX STATUS', 'DESCRIPTION', 'ROOT CAUSE', 'CONDITION OF OCCURRENCE',
  'FIXED DETAILS', 'TESTING SUGGESTIONS', 'AFFECTED RELEASE', 'REVIEW LINK',
  'EXTRA IMPACT', 'CONTENT',
]

const STATUS_KEYWORDS = ['TODO', 'NEXT', 'DOING', 'WAITING', 'PAUSED', 'HOLD', 'CANCELLED', 'DONE']
const DONE_STATUS = ['DONE', 'CANCELLED']

// 状态英文→中文标签（展示用）
export const STATUS_LABELS = {
  TODO: '待办', NEXT: '下一步', DOING: '进行中', WAITING: '等待',
  PAUSED: '暂停', HOLD: '搁置', CANCELLED: '已取消', DONE: '已完成',
}

function emptyBlock() {
  return {
    internalId: '', id: '', parentId: '', manageType: '', taskType: '', kind: 'task',
    deadline: '', focus: '', createTime: '', completedTime: '', origin: '', severity: '', priority: '',
    status: '', done: false, progress: '', repeat: '', spent: 0, checkins: [], fixStatus: '', description: '',
    rootCause: '', conditionOfOccurrence: '', fixedDetails: '', testingSuggestions: '',
    affectedRelease: '', reviewLink: '', extraImpacts: [], content: '',
    notes: { kind: 'none', value: '' },
    customFields: [],
    blockStartLine: 0, blockEndLine: 0,
  }
}

const splitCheckins = (value) => {
  const out = []
  for (const part of String(value || '').split(/[,;，；\s]+/)) {
    const s = part.trim()
    if (s && !out.includes(s)) out.push(s)
  }
  return out
}

function parseFieldLine(line) {
  const trimmed = line.replace(/^\s+/, '')
  if (!trimmed.startsWith('#') || trimmed.startsWith('##')) return null
  const afterHash = trimmed.slice(1)
  const m = /^([^:：]+)(?::|：)(.*)$/.exec(afterHash)
  if (!m) return null
  const nameUpper = m[1].trim().toUpperCase()
  if (KNOWN_FIELDS.includes(nameUpper)) {
    return { name: nameUpper, value: m[2].trim() }
  }
  return null
}

// 未知字段声明 `#NAME:`（大写字母与空格 + 英文冒号，且不在 KNOWN_FIELDS/NOTES 内）→ 自定义字段。
// 全角冒号不视为字段（交由校验器/续行逻辑），与后端 parse_custom_field_line 对齐。
function parseCustomFieldLine(line) {
  const trimmed = line.replace(/^\s+/, '')
  if (!trimmed.startsWith('#') || trimmed.startsWith('##')) return null
  const after = trimmed.slice(1)
  let name = ''
  let restIdx = -1
  for (let i = 0; i < after.length; i++) {
    const ch = after[i]
    if (ch === ':') { restIdx = i + 1; break }
    if ((ch >= 'A' && ch <= 'Z') || ch === ' ') { name += ch } else { return null }
  }
  if (restIdx < 0) return null
  const nm = name.trim()
  if (!nm) return null
  if (KNOWN_FIELDS.includes(nm) || nm === 'NOTES START' || nm === 'NOTES END') return null
  return { name: nm, value: after.slice(restIdx).trim() }
}

function parseNotesStart(line) {
  const trimmed = line.replace(/^\s+/, '')
  if (!/^#NOTES\s+START/i.test(trimmed)) return undefined
  const lp = trimmed.indexOf('(')
  if (lp >= 0) {
    const rp = trimmed.indexOf(')', lp)
    if (rp > lp) {
      const inside = trimmed.slice(lp + 1, rp).trim().replace(/^"|"$/g, '').trim()
      if (inside) return { path: inside }
    }
  }
  return { path: null }
}

function isNotesEnd(line) {
  return /^#NOTES\s+END/i.test(line.replace(/^\s+/, ''))
}

function canonicalStatus(value) {
  const upper = (value || '').toUpperCase()
  for (const kw of STATUS_KEYWORDS) if (upper.includes(kw)) return kw
  return ''
}

function canonicalPriority(value) {
  const upper = (value || '').toUpperCase()
  const idx = upper.indexOf('[#')
  if (idx >= 0) {
    const c = upper[idx + 2]
    if (c >= 'A' && c <= 'Z') return c
  }
  const t = (value || '').trim()
  if (t.length === 1) {
    const c = t.toUpperCase()
    if (c >= 'A' && c <= 'C') return c
  }
  return ''
}

function setField(b, name, rawVal) {
  const val = String(rawVal).replace(/\n+$/, '')
  const trimmed = val.trim()
  switch (name) {
    case 'MANAGE TASK TYPE': break // 目录推导优先
    case 'INTERNAL ID': b.internalId = trimmed; break
    case 'ID': b.id = trimmed; break
    case 'PARENT TASK': b.parentId = trimmed; break
    case 'TYPE': b.taskType = trimmed.toLowerCase(); break
    case 'KIND': b.kind = trimmed ? trimmed.toLowerCase() : 'task'; break
    case 'DEADLINE': b.deadline = trimmed; break
    case 'FOCUS': b.focus = trimmed; break
    case 'CREATE TIME': b.createTime = trimmed; break
    case 'COMPLETED TIME': b.completedTime = trimmed; break
    case 'ORIGIN': b.origin = trimmed; break
    case 'SEVERITY': b.severity = trimmed; break
    case 'PRIORITY': b.priority = canonicalPriority(trimmed); break
    case 'STATUS':
      b.status = canonicalStatus(trimmed)
      b.done = DONE_STATUS.includes(b.status)
      break
    case 'REPEAT': b.repeat = trimmed; break
    case 'PROGRESS': b.progress = trimmed; break
    case 'SPENT': b.spent = parseInt(trimmed, 10) || 0; break
    case 'CHECKINS': b.checkins = splitCheckins(val); break
    case 'FIX STATUS': b.fixStatus = trimmed; break
    case 'DESCRIPTION': b.description = trimmed; break
    case 'ROOT CAUSE': b.rootCause = trimmed; break
    case 'CONDITION OF OCCURRENCE': b.conditionOfOccurrence = val; break
    case 'FIXED DETAILS': b.fixedDetails = val; break
    case 'TESTING SUGGESTIONS': b.testingSuggestions = val; break
    case 'AFFECTED RELEASE': b.affectedRelease = trimmed; break
    case 'REVIEW LINK': b.reviewLink = trimmed; break
    case 'EXTRA IMPACT': if (val.trim()) b.extraImpacts.push(val); break
    case 'CONTENT': b.content = val; break
    default: break
  }
}

function parseOneBlock(bodyLines, category) {
  const b = emptyBlock()
  b.manageType = category || ''
  let current = null       // 已知字段名
  let customName = null    // 自定义字段名
  let buf = ''
  const flush = () => {
    if (current !== null) setField(b, current, buf)
    else if (customName !== null) b.customFields.push({ name: customName, value: buf.replace(/\n+$/, '') })
  }

  let k = 0
  while (k < bodyLines.length) {
    const line = bodyLines[k]
    const notes = parseNotesStart(line)
    if (notes !== undefined) {
      flush(); current = null; customName = null; buf = ''
      if (notes.path) {
        b.notes = { kind: 'file', value: notes.path }
        k++
        if (k < bodyLines.length && isNotesEnd(bodyLines[k])) k++
      } else {
        const noteLines = []
        k++
        while (k < bodyLines.length && !isNotesEnd(bodyLines[k])) { noteLines.push(bodyLines[k]); k++ }
        if (k < bodyLines.length && isNotesEnd(bodyLines[k])) k++
        b.notes = { kind: 'inline', value: noteLines.join('\n').replace(/^\n+|\n+$/g, '') }
      }
      continue
    }
    const field = parseFieldLine(line)
    if (field) {
      flush(); buf = ''
      current = field.name
      customName = null
      buf += field.value + '\n'
      k++
      continue
    }
    const custom = parseCustomFieldLine(line)
    if (custom) {
      flush(); buf = ''
      current = null
      customName = custom.name
      buf += custom.value + '\n'
      k++
      continue
    }
    if (current !== null || customName !== null) { buf += line + '\n' }
    k++
  }
  flush()
  return b
}

/**
 * 解析整个文件文本为任务块数组。
 * @param {string} content
 * @param {string} category work/life/study（由目录推导）
 * @returns {Array<object>} blocks
 */
export function parseTaskBlocks(content, category = '') {
  const lines = String(content || '').split(/\r?\n/)
  const blocks = []
  let i = 0
  while (i < lines.length) {
    if (lines[i].replace(/^\s+/, '').startsWith(BLOCK_START)) {
      const startLine = i + 1
      let j = i + 1
      const body = []
      let endLine = lines.length
      while (j < lines.length) {
        if (lines[j].replace(/^\s+/, '').startsWith(BLOCK_END)) { endLine = j + 1; break }
        body.push(lines[j]); j++
      }
      const block = parseOneBlock(body, category)
      block.blockStartLine = startLine
      block.blockEndLine = endLine
      blocks.push(block)
      i = j + 1
      continue
    }
    i++
  }
  return blocks
}

// ───────── 序列化 / 回写（镜像 task_block.rs::serialize_block / replace_block_in_content）─────────

const MANAGE_LABELS = { work: 'work 工作', life: 'life 生活', study: 'study 学习' }

/** 把一个块对象序列化为 DSL 文本（保持稳定字段顺序）。 */
export function serializeBlock(b) {
  const out = []
  out.push(BLOCK_START)
  const f = (name, val) => out.push(`#${name}: ${val ?? ''}`)
  const fmulti = (name, val) => {
    const v = String(val ?? '')
    if (v.includes('\n')) {
      const lines = v.split('\n')
      out.push(`#${name}: ${lines[0]}`)
      for (let i = 1; i < lines.length; i++) out.push(lines[i])
    } else {
      out.push(`#${name}: ${v}`)
    }
  }
  f('MANAGE TASK TYPE', MANAGE_LABELS[b.manageType] || b.manageType || '')
  f('INTERNAL ID', b.internalId)
  f('ID', b.id)
  f('PARENT TASK', b.parentId)
  f('TYPE', b.taskType)
  if (b.kind && b.kind !== 'task') f('KIND', b.kind)
  f('DEADLINE', b.deadline)
  if (b.focus) f('FOCUS', b.focus)
  f('CREATE TIME', b.createTime)
  f('COMPLETED TIME', b.completedTime)
  f('ORIGIN', b.origin)
  f('SEVERITY', b.severity)
  f('PRIORITY', b.priority ? `[#${b.priority}]` : '')
  f('STATUS', b.status)
  if (b.repeat) f('REPEAT', b.repeat)
  f('PROGRESS', b.progress)
  if (b.spent) f('SPENT', b.spent)
  if (b.checkins && b.checkins.length) f('CHECKINS', b.checkins.join(', '))
  f('FIX STATUS', b.fixStatus)
  f('DESCRIPTION', b.description)
  f('ROOT CAUSE', b.rootCause)
  fmulti('CONDITION OF OCCURRENCE', b.conditionOfOccurrence)
  fmulti('FIXED DETAILS', b.fixedDetails)
  fmulti('TESTING SUGGESTIONS', b.testingSuggestions)
  f('AFFECTED RELEASE', b.affectedRelease)
  f('REVIEW LINK', b.reviewLink)
  for (const ei of (b.extraImpacts || [])) fmulti('EXTRA IMPACT', ei)
  fmulti('CONTENT', b.content)
  // 自定义字段：原样写回（CONTENT 之后、NOTES 之前）。
  for (const cf of (b.customFields || [])) {
    if (cf && cf.name && String(cf.name).trim()) fmulti(cf.name, cf.value)
  }
  if (b.notes && b.notes.kind === 'inline') {
    out.push('#NOTES START:')
    out.push(...String(b.notes.value || '').split('\n'))
    out.push('#NOTES END:')
  } else if (b.notes && b.notes.kind === 'file') {
    out.push(`#NOTES START("${b.notes.value}"):`)
    out.push('#NOTES END:')
  }
  out.push(BLOCK_END)
  return out.join('\n')
}

/** 用新块文本替换 content 中 [startLine, endLine]（1 基）的旧块，返回新全文。 */
export function replaceBlockInContent(content, startLine, endLine, newBlockText) {
  const lines = String(content || '').split(/\r?\n/)
  const s = Math.max(0, startLine - 1)
  const e = Math.max(s, endLine - 1)
  const before = lines.slice(0, s)
  const after = lines.slice(e + 1)
  const mid = String(newBlockText).replace(/\n+$/, '').split('\n')
  return [...before, ...mid, ...after].join('\n')
}

/** 新建块默认的两条 Extra Impact 模板（内容不含 "Extra Impact:" 关键字）。 */
export const DEFAULT_EXTRA_IMPACTS = [
  [
    'SSL算法安全性：',
    '1.算法在业界是否安全（在算法选择时考虑）不涉及',
    '2.对算法的使用是否安全（code review、设计、代码编写时考虑）不涉及',
  ].join('\n'),
  [
    'Webui Bug: If this bug causes WebUI changes, please provide the WebUI bug ID in the text field WebUIBugID',
    'Doc Bug: If this bug needs to be documented, please provide the document bug ID in the text field DocBugID',
    'Xinchuang Multi-OS Platform Impact(MUST provide).',
    '1. Code is probably not OS-related (99%);',
    '2. Not sure after analysis (must provide possibly-related points);',
    '3. Code is OS-Related (must provide related points)',
    'Choice: 1',
  ].join('\n'),
]

/** 构造一个新块对象（用于创建子任务/子bug）。 */
export function buildNewBlock({ category = 'work', taskType = 'task', id = '', parentId = '', createTime = '' } = {}) {
  return {
    internalId: '', id, parentId, manageType: category, taskType, kind: 'task',
    deadline: '', focus: '', createTime, completedTime: '', origin: '', severity: 'normal', priority: '',
    status: 'TODO', done: false, progress: taskType === 'bug' ? '' : '0%', repeat: '', spent: 0, checkins: [], fixStatus: '',
    description: '', rootCause: '', conditionOfOccurrence: '', fixedDetails: '',
    testingSuggestions: '', affectedRelease: '', reviewLink: '',
    extraImpacts: DEFAULT_EXTRA_IMPACTS.slice(),
    content: '', notes: { kind: 'inline', value: '' },
    customFields: [],
    blockStartLine: 0, blockEndLine: 0,
  }
}


const SEVERITY_TOKENS = ['critical', 'major', 'normal', 'minor', 'trivial']
const ORIGIN_TOKENS = ['customer', 'qa', 'dev', 'other']
const FIX_STATUS_TOKENS = ['debug', 'test', 'review', 'approval', 'fixed', 'other']
const TYPE_TOKENS = ['bug', 'task', 'other']
const KIND_TOKENS = ['task', 'habit', 'goal', 'review']
const MANAGE_TYPE_TOKENS = ['work', 'life', 'study']

// 检测字段声明行 #NAME: 或 #NAME：（NAME 仅大写字母与空格）
function detectFieldDecl(line) {
  const t = line.replace(/^\s+/, '')
  if (!t.startsWith('#') || t.startsWith('##')) return null
  const after = t.slice(1)
  let name = ''
  let found = false, full = false, restIdx = 0
  for (let i = 0; i < after.length; i++) {
    const ch = after[i]
    if (ch === ':') { restIdx = i + 1; found = true; break }
    if (ch === '：') { restIdx = i + 1; found = true; full = true; break }
    if ((ch >= 'A' && ch <= 'Z') || ch === ' ') { name += ch }
    else return null
  }
  if (!found) return null
  return { name: name.trim(), full, value: after.slice(restIdx).trim() }
}

function containsTokenCi(value, tokens) {
  const words = String(value).toLowerCase().split(/[^a-z0-9]+/)
  return tokens.some((t) => words.includes(t))
}
function looksLikeDate(v) {
  const s = String(v).trim()
  if (!s) return true
  // 接受 ISO(2026-06-20)、斜杠(2026/6/20) 或 中文(2026年06月20日)
  return /\d{4}\s*[-/年]\s*\d{1,2}\s*[-/月]\s*\d{1,2}/.test(s)
}

/**
 * 校验任务块语法，返回诊断数组 [{line, severity:'error'|'warning', message}]。
 */
export function validateTaskBlocks(content, category = '') {
  const lines = String(content || '').split(/\r?\n/)
  const diags = []
  let inBlock = false, blockStart = 0
  const push = (line, severity, message) => diags.push({ line, severity, message })

  const validateBlock = (startIdx, endIdx) => {
    const fields = new Map() // name -> {line, value, count}
    let notesOpen = 0, notesClosed = false, inNotes = false
    for (let k = startIdx + 1; k < Math.min(endIdx, lines.length); k++) {
      const lt = lines[k].replace(/^\s+/, '')
      const up = lt.toUpperCase()
      if (up.startsWith('#NOTES START')) {
        notesOpen = k + 1
        const fileref = lt.includes('(') && lt.includes(')')
        inNotes = !fileref
        if (fileref) notesClosed = true
        continue
      }
      if (up.startsWith('#NOTES END')) { inNotes = false; notesClosed = true; continue }
      if (inNotes) continue
      const decl = detectFieldDecl(lines[k])
      if (decl) {
        if (decl.full) {
          push(k + 1, 'error', `字段「${decl.name}」请使用英文冒号 ':'（不要用全角 '：'）`)
        } else if (!KNOWN_FIELDS.includes(decl.name)) {
          // 未知字段视为用户自定义字段：合法，不再告警。
        } else {
          const e = fields.get(decl.name) || { line: k + 1, value: decl.value, count: 0 }
          e.count += 1
          if (e.count === 1) { e.line = k + 1; e.value = decl.value }
          fields.set(decl.name, e)
        }
      }
    }
    const g = (n) => fields.get(n)
    const typeVal = (g('TYPE')?.value || '').toLowerCase()
    if (g('TYPE')) {
      const lv = (g('TYPE').value || '').toLowerCase()
      if (lv && !TYPE_TOKENS.includes(lv)) push(g('TYPE').line, 'error', 'TYPE 必须为 bug / task / other 之一')
      if (lv === 'bug' && (category === 'life' || category === 'study')) push(g('TYPE').line, 'error', '生活/学习 分类不支持 bug 类型，仅 task / other')
    }
    if (g('STATUS') && g('STATUS').value.trim() && !canonicalStatus(g('STATUS').value)) {
      push(g('STATUS').line, 'error', 'STATUS 无法识别，应包含 TODO/NEXT/DOING/WAITING/PAUSED/HOLD/CANCELLED/DONE 之一')
    }
    if (g('KIND') && g('KIND').value.trim() && !KIND_TOKENS.includes(g('KIND').value.trim().toLowerCase())) {
      push(g('KIND').line, 'warning', 'KIND 建议为 task / habit / goal / review')
    }
    if (g('SPENT') && g('SPENT').value.trim() && !/^\d+$/.test(g('SPENT').value.trim())) {
      push(g('SPENT').line, 'warning', 'SPENT 请填写分钟数（纯数字）')
    }
    if (g('FOCUS') && !looksLikeDate(g('FOCUS').value)) {
      push(g('FOCUS').line, 'warning', 'FOCUS 建议形如 2026-11-06')
    }
    if (g('PRIORITY') && g('PRIORITY').value.trim() && !canonicalPriority(g('PRIORITY').value)) {
      push(g('PRIORITY').line, 'error', 'PRIORITY 仅支持 [#A] / [#B] / [#C] 或留空')
    }
    if (g('SEVERITY') && g('SEVERITY').value.trim() && !containsTokenCi(g('SEVERITY').value, SEVERITY_TOKENS)) {
      push(g('SEVERITY').line, 'warning', 'SEVERITY 建议为 critical/major/normal/minor/trivial 之一')
    }
    if (g('ORIGIN') && g('ORIGIN').value.trim() && !containsTokenCi(g('ORIGIN').value, ORIGIN_TOKENS)) {
      push(g('ORIGIN').line, 'warning', 'ORIGIN 建议为 customer/QA/dev/other 之一')
    }
    if (g('FIX STATUS') && g('FIX STATUS').value.trim() && !containsTokenCi(g('FIX STATUS').value, FIX_STATUS_TOKENS)) {
      push(g('FIX STATUS').line, 'warning', 'FIX STATUS 建议为 debug/test/review/approval/fixed/other 之一')
    }
    if (g('PROGRESS') && g('PROGRESS').value.trim()) {
      const t = g('PROGRESS').value.trim()
      if (!/^\d{1,3}%$/.test(t)) push(g('PROGRESS').line, 'warning', 'PROGRESS 建议形如 50%')
    }
    if (g('MANAGE TASK TYPE') && g('MANAGE TASK TYPE').value.trim() && !containsTokenCi(g('MANAGE TASK TYPE').value, MANAGE_TYPE_TOKENS)) {
      push(g('MANAGE TASK TYPE').line, 'warning', 'MANAGE TASK TYPE 建议为 work/life/study')
    }
    for (const f of ['DEADLINE', 'CREATE TIME']) {
      if (g(f) && !looksLikeDate(g(f).value)) push(g(f).line, 'warning', `${f} 建议形如 2026年06月13日（CREATE TIME 可含 时:分:秒）`)
    }
    if (g('DESCRIPTION')) { if (!g('DESCRIPTION').value.trim()) push(g('DESCRIPTION').line, 'warning', '建议填写 DESCRIPTION') }
    else push(startIdx + 1, 'warning', '缺少 DESCRIPTION 字段')
    void typeVal
    for (const [name, e] of fields) {
      if (e.count > 1 && name !== 'EXTRA IMPACT') push(e.line, 'warning', `字段「${name}」重复出现 ${e.count} 次`)
    }
    if (notesOpen && !notesClosed) push(notesOpen, 'warning', '#NOTES START 没有对应的 #NOTES END')
  }

  let i = 0
  while (i < lines.length) {
    const t = lines[i].replace(/^\s+/, '')
    if (t.startsWith(BLOCK_START)) {
      if (inBlock) { push(i + 1, 'error', '上一个任务块未用 ##GUAXI_TASK_END 结束'); validateBlock(blockStart, i) }
      inBlock = true; blockStart = i; i++; continue
    }
    if (t.startsWith(BLOCK_END)) {
      if (!inBlock) push(i + 1, 'error', '##GUAXI_TASK_END 没有对应的 ##GUAXI_TASK_START')
      else { validateBlock(blockStart, i); inBlock = false }
      i++; continue
    }
    i++
  }
  if (inBlock) { push(blockStart + 1, 'error', '任务块未闭合：缺少 ##GUAXI_TASK_END'); validateBlock(blockStart, lines.length) }
  return diags
}
