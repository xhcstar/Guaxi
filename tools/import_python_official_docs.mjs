import { createHash } from 'node:crypto'
import { spawn } from 'node:child_process'
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const PROJECT_ROOT = path.resolve(__dirname, '..')
const DEFAULT_PYTHON_DOCS_VERSION = '3.13.13'
const DEFAULT_PYTHON_DOCS_SERIES = '3.13'
const DEFAULT_SOURCE_URL = `https://docs.python.org/zh-cn/${DEFAULT_PYTHON_DOCS_SERIES}/archives/python-${DEFAULT_PYTHON_DOCS_SERIES}-docs-html.zip`
const FALLBACK_SOURCE_URL = `https://docs.python.org/${DEFAULT_PYTHON_DOCS_SERIES}/archives/python-${DEFAULT_PYTHON_DOCS_SERIES}-docs-html.zip`
const DEFAULT_VERSION = DEFAULT_PYTHON_DOCS_VERSION
const DOCS_PUBLIC_ROOT = path.join(PROJECT_ROOT, 'public', 'guaxi-docs', 'python')
const CACHE_ROOT = path.join(PROJECT_ROOT, '.cache', 'guaxi-docs', 'python')
const CHUNK_SIZE = 120

const args = process.argv.slice(2)

const getArgValue = (name, fallback = '') => {
  const index = args.indexOf(name)
  if (index < 0 || index + 1 >= args.length) return fallback
  return String(args[index + 1] || '').trim()
}

const hasArg = (name) => args.includes(name)

const log = (message, extra = null) => {
  if (extra) {
    console.info(`[GuaxiPythonDocs] ${message}`, extra)
    return
  }
  console.info(`[GuaxiPythonDocs] ${message}`)
}

const run = (command, commandArgs, options = {}) => new Promise((resolve, reject) => {
  const child = spawn(command, commandArgs, {
    stdio: 'inherit',
    shell: false,
    ...options,
  })
  child.on('error', reject)
  child.on('exit', (code) => {
    if (code === 0) {
      resolve()
      return
    }
    reject(new Error(`${command} exited with code ${code}`))
  })
})

const ensureDir = async (dir) => {
  await fs.mkdir(dir, { recursive: true })
}

const removeDir = async (dir) => {
  await fs.rm(dir, { recursive: true, force: true })
}

const downloadFile = async (url, targetPath) => {
  log('download official Python documentation archive', { url, targetPath })
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Download failed: ${response.status} ${response.statusText}`)
  }
  const buffer = Buffer.from(await response.arrayBuffer())
  await ensureDir(path.dirname(targetPath))
  await fs.writeFile(targetPath, buffer)
  log('download complete', { bytes: buffer.byteLength })
  return targetPath
}

const downloadFileWithFallback = async ({ primaryUrl, fallbackUrl, targetPath }) => {
  try {
    await downloadFile(primaryUrl, targetPath)
    return {
      url: primaryUrl,
      language: primaryUrl.includes('/zh-cn/') ? 'zh-cn' : 'en',
    }
  } catch (primaryError) {
    if (!fallbackUrl || fallbackUrl === primaryUrl) throw primaryError
    log('primary Python docs archive unavailable, fallback to English official docs', {
      primaryUrl,
      fallbackUrl,
      error: String(primaryError?.message || primaryError),
    })
    await downloadFile(fallbackUrl, targetPath)
    return {
      url: fallbackUrl,
      language: fallbackUrl.includes('/zh-cn/') ? 'zh-cn' : 'en',
    }
  }
}

const extractZip = async (zipPath, outDir) => {
  await removeDir(outDir)
  await ensureDir(outDir)
  log('extract Python docs archive', { zipPath, outDir })
  try {
    await run('python', ['-m', 'zipfile', '-e', zipPath, outDir])
    return
  } catch (pythonError) {
    if (process.platform !== 'win32') throw pythonError
    log('python zipfile extractor failed, fallback to PowerShell Expand-Archive', {
      error: String(pythonError?.message || pythonError),
    })
    await run('powershell.exe', [
      '-NoProfile',
      '-Command',
      `Expand-Archive -LiteralPath ${JSON.stringify(zipPath)} -DestinationPath ${JSON.stringify(outDir)} -Force`,
    ])
  }
}

const readDirRecursive = async (dir, files = []) => {
  const entries = await fs.readdir(dir, { withFileTypes: true })
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      await readDirRecursive(fullPath, files)
      continue
    }
    files.push(fullPath)
  }
  return files
}

const findDocsRoot = async (dir) => {
  const files = await readDirRecursive(dir)
  const indexFiles = files.filter((file) => path.basename(file).toLowerCase() === 'index.html')
  if (!indexFiles.length) {
    throw new Error(`No index.html found in extracted docs: ${dir}`)
  }

  // 设计说明：
  // 1. Python 官方 HTML 包里很多子目录都有自己的 index.html，例如 c-api/index.html。
  // 2. 完整离线文档必须选择包含 contents.html、library/、tutorial/ 等入口的顶层目录，
  //    不能拿到第一个 index.html 就停止，否则会只导入一个子手册。
  // 3. 如果未来官方包结构变化，仍用“最短相对路径”作为兜底，尽量靠近解压根目录。
  const candidates = await Promise.all(indexFiles.map(async (indexFile) => {
    const root = path.dirname(indexFile)
    const names = new Set((await fs.readdir(root).catch(() => [])).map((name) => String(name).toLowerCase()))
    const score = [
      names.has('contents.html') ? 6 : 0,
      names.has('library') ? 5 : 0,
      names.has('tutorial') ? 4 : 0,
      names.has('reference') ? 4 : 0,
      names.has('howto') ? 3 : 0,
      names.has('_static') ? 2 : 0,
    ].reduce((total, value) => total + value, 0)
    return {
      root,
      score,
      depth: path.relative(dir, root).split(path.sep).filter(Boolean).length,
    }
  }))

  candidates.sort((a, b) => b.score - a.score || a.depth - b.depth)
  const best = candidates[0]
  log('resolved Python docs root', {
    root: best.root,
    score: best.score,
    depth: best.depth,
  })
  return best.root
}

const decodeHtmlEntities = (value = '') => String(value || '')
  .replace(/&nbsp;/g, ' ')
  .replace(/&amp;/g, '&')
  .replace(/&lt;/g, '<')
  .replace(/&gt;/g, '>')
  .replace(/&quot;/g, '"')
  .replace(/&#39;/g, "'")
  .replace(/&#x27;/g, "'")
  .replace(/&#(\d+);/g, (_, code) => {
    const n = Number(code)
    return Number.isFinite(n) ? String.fromCodePoint(n) : ''
  })
  .replace(/&#x([0-9a-f]+);/gi, (_, code) => {
    const n = Number.parseInt(code, 16)
    return Number.isFinite(n) ? String.fromCodePoint(n) : ''
  })

const stripHtml = (html = '') => decodeHtmlEntities(String(html || '')
  .replace(/<script[\s\S]*?<\/script>/gi, ' ')
  .replace(/<style[\s\S]*?<\/style>/gi, ' ')
  .replace(/<nav[\s\S]*?<\/nav>/gi, ' ')
  .replace(/<header[\s\S]*?<\/header>/gi, ' ')
  .replace(/<footer[\s\S]*?<\/footer>/gi, ' ')
  .replace(/<[^>]+>/g, ' ')
  .replace(/\s+/g, ' ')
  .trim())

const extractTitle = (html = '', relativePath = '') => {
  const titleMatch = String(html).match(/<title[^>]*>([\s\S]*?)<\/title>/i)
  const h1Match = String(html).match(/<h1[^>]*>([\s\S]*?)<\/h1>/i)
  const rawTitle = stripHtml(h1Match?.[1] || titleMatch?.[1] || relativePath)
  return rawTitle.replace(/\s*—\s*Python .* documentation\s*$/i, '').trim() || relativePath
}

const extractHeadings = (html = '') => {
  const headings = []
  const re = /<h([1-3])[^>]*>([\s\S]*?)<\/h\1>/gi
  let match = re.exec(html)
  while (match) {
    const text = stripHtml(match[2])
    if (text) {
      headings.push(text)
    }
    match = re.exec(html)
  }
  return Array.from(new Set(headings)).slice(0, 12)
}

const extractMainText = (html = '') => {
  const bodyMatch = String(html).match(/<div[^>]+class="body"[^>]*>([\s\S]*?)<\/div>\s*<\/div>\s*<\/div>/i)
  const mainMatch = String(html).match(/<main[^>]*>([\s\S]*?)<\/main>/i)
  const source = bodyMatch?.[1] || mainMatch?.[1] || html
  return stripHtml(source)
}

const inferDocKind = (relativePath = '') => {
  const first = String(relativePath || '').split(/[\\/]/)[0] || 'reference'
  if (first === 'tutorial') return '教程'
  if (first === 'library') return '标准库'
  if (first === 'reference') return '语言参考'
  if (first === 'howto') return 'HOWTO'
  if (first === 'faq') return 'FAQ'
  if (first === 'using') return '安装与使用'
  if (first === 'extending') return '扩展与嵌入'
  if (first === 'c-api') return 'C API'
  if (first === 'distutils') return '打包工具'
  return '文档'
}

const buildSearchText = (entry) => [
  entry.title,
  entry.kind,
  entry.path,
  ...(entry.headings || []),
  entry.text,
].join(' ').toLowerCase()

const createDocEntry = async (filePath, docsRoot) => {
  const html = await fs.readFile(filePath, 'utf8')
  const relativePath = path.relative(docsRoot, filePath).replace(/\\/g, '/')
  const text = extractMainText(html)
  if (!text || text.length < 40) return null
  const title = extractTitle(html, relativePath)
  const headings = extractHeadings(html)
  const id = createHash('sha1').update(relativePath).digest('hex').slice(0, 16)
  const entry = {
    id,
    channel: 'python',
    title,
    kind: inferDocKind(relativePath),
    path: relativePath,
    localUrl: `/guaxi-docs/python/html/${relativePath}`,
    headings,
    excerpt: text.slice(0, 260),
    text,
  }
  return {
    ...entry,
    searchText: buildSearchText(entry),
  }
}

const writeJson = async (targetPath, value) => {
  await ensureDir(path.dirname(targetPath))
  await fs.writeFile(targetPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
}

const buildIndex = async ({ docsRoot, version, sourceUrl, language }) => {
  const files = (await readDirRecursive(docsRoot))
    .filter((file) => file.toLowerCase().endsWith('.html'))
    .filter((file) => !path.basename(file).startsWith('_'))

  log('scan Python html files', { count: files.length })
  const entries = []
  for (const file of files) {
    const entry = await createDocEntry(file, docsRoot)
    if (entry) entries.push(entry)
  }

  entries.sort((a, b) => String(a.path).localeCompare(String(b.path), 'en-US'))

  const chunksDir = path.join(DOCS_PUBLIC_ROOT, 'chunks')
  await removeDir(chunksDir)
  await ensureDir(chunksDir)
  const chunks = []
  for (let index = 0; index < entries.length; index += CHUNK_SIZE) {
    const part = entries.slice(index, index + CHUNK_SIZE)
    const file = `python-docs-${String(chunks.length).padStart(3, '0')}.json`
    await writeJson(path.join(chunksDir, file), part)
    chunks.push({ file: `chunks/${file}`, count: part.length })
  }

  const manifest = {
    kind: 'python-official-docs',
    title: language === 'zh-cn' ? 'Python 官方完整文档（简体中文）' : 'Python 官方完整文档（英文）',
    version,
    language,
    sourceUrl,
    generatedAt: new Date().toISOString(),
    totalEntries: entries.length,
    chunks,
  }
  await writeJson(path.join(DOCS_PUBLIC_ROOT, 'index.json'), manifest)
  log('write Python docs search index', {
    entries: entries.length,
    chunks: chunks.length,
    target: path.join(DOCS_PUBLIC_ROOT, 'index.json'),
  })
}

const copyHtmlBundle = async (docsRoot) => {
  const target = path.join(DOCS_PUBLIC_ROOT, 'html')
  await removeDir(target)
  await ensureDir(path.dirname(target))
  await fs.cp(docsRoot, target, { recursive: true })
  log('copy full Python html docs into public assets', { target })
}

const main = async () => {
  const sourceZip = getArgValue('--zip')
  const sourceDir = getArgValue('--src')
  const sourceUrl = getArgValue('--url', DEFAULT_SOURCE_URL)
  const fallbackUrl = getArgValue('--fallback-url', FALLBACK_SOURCE_URL)
  const languageArg = getArgValue('--language', '')
  const version = getArgValue('--version', DEFAULT_VERSION)
  const skipDownload = hasArg('--no-download')

  await ensureDir(CACHE_ROOT)

  let resolvedSourceUrl = sourceUrl
  let resolvedLanguage = languageArg || (sourceUrl.includes('/zh-cn/') ? 'zh-cn' : 'en')
  let docsRoot = ''
  if (sourceDir) {
    docsRoot = path.resolve(PROJECT_ROOT, sourceDir)
    log('use existing Python docs directory', { docsRoot })
  } else {
    const zipPath = sourceZip
      ? path.resolve(PROJECT_ROOT, sourceZip)
      : path.join(CACHE_ROOT, `python-${version}-docs-html.zip`)
    if (!sourceZip && !skipDownload) {
      const result = await downloadFileWithFallback({
        primaryUrl: sourceUrl,
        fallbackUrl,
        targetPath: zipPath,
      })
      resolvedSourceUrl = result.url
      resolvedLanguage = languageArg || result.language
    }
    const extractRoot = path.join(CACHE_ROOT, 'html')
    await extractZip(zipPath, extractRoot)
    docsRoot = await findDocsRoot(extractRoot)
  }

  await copyHtmlBundle(docsRoot)
  await buildIndex({ docsRoot, version, sourceUrl: resolvedSourceUrl, language: resolvedLanguage })
}

main().catch((error) => {
  console.error('[GuaxiPythonDocs] import failed', error)
  process.exitCode = 1
})
