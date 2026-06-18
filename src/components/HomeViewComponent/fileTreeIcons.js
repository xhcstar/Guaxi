// 文件树图标解析器（主编辑器树 / 知识图谱视图树共用）
//
// 设计说明：
// 1. 文件类型使用 vscode-icons 的彩色 SVG 图标，高 DPI 屏缩放不发虚。
// 2. 文件夹改用 lucide 单色描线图标（folder / folder-open，currentColor），由 CSS 统一染成
//    淡蓝等高级色，开/合两态切换；避免 vscode-icons 默认文件夹突兀的纯黄、与主题割裂。
// 3. 文件先按完整文件名识别常见配置文件，再按扩展名识别类型，无扩展名文件（Dockerfile 等）也能命中。
// 4. 未覆盖的类型统一回退到 default-file，保证新格式也有稳定显示。

import defaultFileIcon from '@iconify-icons/vscode-icons/default-file'
// 文件夹改用 lucide 单色描线图标(currentColor),由 CSS 统一染成淡蓝等高级色,
// 避免 vscode-icons 默认文件夹突兀的纯黄、与主题不协调。
import folderIcon from '@iconify-icons/lucide/folder'
import folderOpenIcon from '@iconify-icons/lucide/folder-open'

// ───── 文件类型图标 ─────
import markdownIcon from '@iconify-icons/vscode-icons/file-type-markdown'
import jsonIcon from '@iconify-icons/vscode-icons/file-type-json-official'
import jsIcon from '@iconify-icons/vscode-icons/file-type-js-official'
import tsIcon from '@iconify-icons/vscode-icons/file-type-typescript-official'
import vueIcon from '@iconify-icons/vscode-icons/file-type-vue'
import htmlIcon from '@iconify-icons/vscode-icons/file-type-html'
import cssIcon from '@iconify-icons/vscode-icons/file-type-css'
import scssIcon from '@iconify-icons/vscode-icons/file-type-scss'
import lessIcon from '@iconify-icons/vscode-icons/file-type-less'
import pythonIcon from '@iconify-icons/vscode-icons/file-type-python'
import javaIcon from '@iconify-icons/vscode-icons/file-type-java'
import cIcon from '@iconify-icons/vscode-icons/file-type-c'
import cHeaderIcon from '@iconify-icons/vscode-icons/file-type-cheader'
import cppIcon from '@iconify-icons/vscode-icons/file-type-cpp'
import cppHeaderIcon from '@iconify-icons/vscode-icons/file-type-cppheader'
import csharpIcon from '@iconify-icons/vscode-icons/file-type-csharp'
import goIcon from '@iconify-icons/vscode-icons/file-type-go'
import rustIcon from '@iconify-icons/vscode-icons/file-type-rust'
import luaIcon from '@iconify-icons/vscode-icons/file-type-lua'
import shellIcon from '@iconify-icons/vscode-icons/file-type-shell'
import powershellIcon from '@iconify-icons/vscode-icons/file-type-powershell'
import yamlIcon from '@iconify-icons/vscode-icons/file-type-yaml'
import xmlIcon from '@iconify-icons/vscode-icons/file-type-xml'
import tomlIcon from '@iconify-icons/vscode-icons/file-type-toml'
import iniIcon from '@iconify-icons/vscode-icons/file-type-ini'
import configIcon from '@iconify-icons/vscode-icons/file-type-config'
import dockerIcon from '@iconify-icons/vscode-icons/file-type-docker'
import gitIcon from '@iconify-icons/vscode-icons/file-type-git'
import pdfIcon from '@iconify-icons/vscode-icons/file-type-pdf2'
import imageIcon from '@iconify-icons/vscode-icons/file-type-image'
import svgIcon from '@iconify-icons/vscode-icons/file-type-svg'
import wordIcon from '@iconify-icons/vscode-icons/file-type-word'
import excelIcon from '@iconify-icons/vscode-icons/file-type-excel'
import powerpointIcon from '@iconify-icons/vscode-icons/file-type-powerpoint'
import texIcon from '@iconify-icons/vscode-icons/file-type-tex'
import textIcon from '@iconify-icons/vscode-icons/file-type-text'
import zipIcon from '@iconify-icons/vscode-icons/file-type-zip'
import audioIcon from '@iconify-icons/vscode-icons/file-type-audio'
import videoIcon from '@iconify-icons/vscode-icons/file-type-video'
import fontIcon from '@iconify-icons/vscode-icons/file-type-font'
import sqlIcon from '@iconify-icons/vscode-icons/file-type-sql'
import sqliteIcon from '@iconify-icons/vscode-icons/file-type-sqlite'
import mermaidIcon from '@iconify-icons/vscode-icons/file-type-mermaid'

// 完整文件名 → 图标
const FILE_NAME_ICON_MAP = new Map([
  ['dockerfile', dockerIcon],
  ['compose.yaml', dockerIcon],
  ['compose.yml', dockerIcon],
  ['docker-compose.yaml', dockerIcon],
  ['docker-compose.yml', dockerIcon],
  ['makefile', configIcon],
  ['cmakelists.txt', configIcon],
  ['package.json', jsonIcon],
  ['package-lock.json', jsonIcon],
  ['pnpm-lock.yaml', yamlIcon],
  ['yarn.lock', yamlIcon],
  ['vite.config.js', jsIcon],
  ['vite.config.ts', tsIcon],
  ['vue.config.js', vueIcon],
  ['tsconfig.json', tsIcon],
  ['jsconfig.json', jsIcon],
  ['cargo.toml', rustIcon],
  ['cargo.lock', rustIcon],
  ['readme', markdownIcon],
  ['license', textIcon],
  ['.gitignore', gitIcon],
  ['.gitattributes', gitIcon],
  ['.gitmodules', gitIcon],
  ['.env', configIcon],
  ['.env.local', configIcon],
])

// 扩展名 → 图标
const EXTENSION_ICON_MAP = new Map([
  ['md', markdownIcon], ['markdown', markdownIcon], ['mdx', markdownIcon],
  ['json', jsonIcon], ['jsonc', jsonIcon], ['json5', jsonIcon], ['cellmeta', jsonIcon],
  ['js', jsIcon], ['mjs', jsIcon], ['cjs', jsIcon], ['jsx', jsIcon],
  ['ts', tsIcon], ['tsx', tsIcon], ['d.ts', tsIcon],
  ['vue', vueIcon], ['html', htmlIcon], ['htm', htmlIcon],
  ['css', cssIcon], ['scss', scssIcon], ['sass', scssIcon], ['less', lessIcon],
  ['py', pythonIcon], ['pyw', pythonIcon], ['ipynb', pythonIcon],
  ['java', javaIcon], ['c', cIcon], ['h', cHeaderIcon], ['cpp', cppIcon], ['cc', cppIcon], ['cxx', cppIcon], ['hpp', cppHeaderIcon], ['hh', cppHeaderIcon],
  ['cs', csharpIcon], ['go', goIcon], ['rs', rustIcon], ['lua', luaIcon],
  ['sh', shellIcon], ['bash', shellIcon], ['zsh', shellIcon], ['fish', shellIcon], ['bat', shellIcon], ['cmd', shellIcon],
  ['ps1', powershellIcon], ['psm1', powershellIcon], ['psd1', powershellIcon],
  ['yml', yamlIcon], ['yaml', yamlIcon], ['xml', xmlIcon], ['toml', tomlIcon], ['ini', iniIcon], ['conf', configIcon], ['config', configIcon],
  ['dockerfile', dockerIcon], ['gitignore', gitIcon],
  ['pdf', pdfIcon], ['tex', texIcon], ['txt', textIcon], ['log', textIcon],
  ['png', imageIcon], ['jpg', imageIcon], ['jpeg', imageIcon], ['gif', imageIcon], ['webp', imageIcon], ['bmp', imageIcon], ['ico', imageIcon], ['avif', imageIcon],
  ['svg', svgIcon],
  ['doc', wordIcon], ['docx', wordIcon],
  ['xls', excelIcon], ['xlsx', excelIcon], ['csv', excelIcon],
  ['ppt', powerpointIcon], ['pptx', powerpointIcon],
  ['zip', zipIcon], ['rar', zipIcon], ['7z', zipIcon], ['tar', zipIcon], ['gz', zipIcon], ['tgz', zipIcon],
  ['mp3', audioIcon], ['wav', audioIcon], ['flac', audioIcon], ['ogg', audioIcon], ['m4a', audioIcon],
  ['mp4', videoIcon], ['mkv', videoIcon], ['webm', videoIcon], ['avi', videoIcon], ['mov', videoIcon],
  ['ttf', fontIcon], ['otf', fontIcon], ['woff', fontIcon], ['woff2', fontIcon],
  ['sql', sqlIcon], ['sqlite', sqliteIcon], ['sqlite3', sqliteIcon], ['db', sqliteIcon],
  ['mmd', mermaidIcon], ['mermaid', mermaidIcon],
])

function getExtension(name) {
  const lower = String(name || '').toLowerCase()
  if (lower.endsWith('.d.ts')) return 'd.ts'
  const dot = lower.lastIndexOf('.')
  return dot > 0 ? lower.slice(dot + 1) : lower
}

/**
 * 解析文件树节点图标。
 * @param {string} name 节点名称（文件名或目录名）
 * @param {boolean} isDir 是否目录
 * @param {boolean} [expanded] 目录是否展开（用于切换开/合图标）
 * @returns iconify 图标对象
 */
export function getFileTreeIcon(name, isDir, expanded = false) {
  if (isDir) {
    // 单色文件夹图标:开/合两态,颜色由 CSS（淡蓝等高级色）统一控制
    return expanded ? folderOpenIcon : folderIcon
  }

  const fileName = String(name || '').trim().toLowerCase()
  if (!fileName) return defaultFileIcon

  const byName = FILE_NAME_ICON_MAP.get(fileName)
  if (byName) return byName

  return EXTENSION_ICON_MAP.get(getExtension(fileName)) || defaultFileIcon
}

export { defaultFileIcon, folderIcon, folderOpenIcon }
