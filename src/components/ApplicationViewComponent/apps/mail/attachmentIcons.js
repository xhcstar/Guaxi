// 附件图标：按文件类型挑图标，而不是所有附件共用一个「📄」。
//
// 附件清单里最需要一眼看出的是「这是什么文件」——.xls 和 .pdf 都画成同一张纸，
// 用户只能逐个读文件名（用户反馈：「对应类型的文件要用对应的 icon」）。
// 图标沿用文件树那一套 vscode-icons 彩色图标（见 fileTreeIcons.js），
// 先按扩展名、再按 MIME 兜底：很多客户端给出的 content-type 是
// application/octet-stream，只能靠文件名判断。
import defaultFileIcon from '@iconify-icons/vscode-icons/default-file.js'
import wordIcon from '@iconify-icons/vscode-icons/file-type-word.js'
import excelIcon from '@iconify-icons/vscode-icons/file-type-excel.js'
import powerpointIcon from '@iconify-icons/vscode-icons/file-type-powerpoint.js'
import pdfIcon from '@iconify-icons/vscode-icons/file-type-pdf2.js'
import zipIcon from '@iconify-icons/vscode-icons/file-type-zip.js'
import imageIcon from '@iconify-icons/vscode-icons/file-type-image.js'
import svgIcon from '@iconify-icons/vscode-icons/file-type-svg.js'
import audioIcon from '@iconify-icons/vscode-icons/file-type-audio.js'
import videoIcon from '@iconify-icons/vscode-icons/file-type-video.js'
import textIcon from '@iconify-icons/vscode-icons/file-type-text.js'
import markdownIcon from '@iconify-icons/vscode-icons/file-type-markdown.js'
import jsonIcon from '@iconify-icons/vscode-icons/file-type-json-official.js'
import xmlIcon from '@iconify-icons/vscode-icons/file-type-xml.js'
import htmlIcon from '@iconify-icons/vscode-icons/file-type-html.js'
import cssIcon from '@iconify-icons/vscode-icons/file-type-css.js'
import jsIcon from '@iconify-icons/vscode-icons/file-type-js-official.js'
import pythonIcon from '@iconify-icons/vscode-icons/file-type-python.js'
import javaIcon from '@iconify-icons/vscode-icons/file-type-java.js'
import sqlIcon from '@iconify-icons/vscode-icons/file-type-sql.js'
import fontIcon from '@iconify-icons/vscode-icons/file-type-font.js'
import binaryIcon from '@iconify-icons/vscode-icons/file-type-binary.js'
import mailIcon from '@iconify-icons/vscode-icons/file-type-mailing.js'

const EXT_ICONS = new Map([
  ['doc', wordIcon], ['docx', wordIcon], ['dot', wordIcon], ['dotx', wordIcon],
  ['rtf', wordIcon], ['odt', wordIcon],
  ['xls', excelIcon], ['xlsx', excelIcon], ['xlsm', excelIcon], ['csv', excelIcon],
  ['ods', excelIcon], ['et', excelIcon],
  ['ppt', powerpointIcon], ['pptx', powerpointIcon], ['pps', powerpointIcon],
  ['ppsx', powerpointIcon], ['odp', powerpointIcon],
  ['pdf', pdfIcon],
  ['zip', zipIcon], ['rar', zipIcon], ['7z', zipIcon], ['tar', zipIcon],
  ['gz', zipIcon], ['tgz', zipIcon], ['bz2', zipIcon], ['xz', zipIcon], ['zst', zipIcon],
  ['png', imageIcon], ['jpg', imageIcon], ['jpeg', imageIcon], ['gif', imageIcon],
  ['webp', imageIcon], ['bmp', imageIcon], ['ico', imageIcon], ['avif', imageIcon],
  ['tif', imageIcon], ['tiff', imageIcon], ['heic', imageIcon],
  ['svg', svgIcon],
  ['mp3', audioIcon], ['wav', audioIcon], ['flac', audioIcon], ['ogg', audioIcon],
  ['m4a', audioIcon], ['aac', audioIcon], ['wma', audioIcon], ['amr', audioIcon],
  ['mp4', videoIcon], ['mkv', videoIcon], ['webm', videoIcon], ['avi', videoIcon],
  ['mov', videoIcon], ['wmv', videoIcon], ['flv', videoIcon],
  ['txt', textIcon], ['log', textIcon], ['ini', textIcon], ['conf', textIcon],
  ['cfg', textIcon], ['md', markdownIcon], ['markdown', markdownIcon],
  ['json', jsonIcon], ['json5', jsonIcon], ['xml', xmlIcon],
  ['html', htmlIcon], ['htm', htmlIcon], ['css', cssIcon],
  ['js', jsIcon], ['mjs', jsIcon], ['ts', jsIcon],
  ['py', pythonIcon], ['java', javaIcon],
  ['sql', sqlIcon], ['db', sqlIcon], ['sqlite', sqlIcon],
  ['ttf', fontIcon], ['otf', fontIcon], ['woff', fontIcon], ['woff2', fontIcon],
  ['exe', binaryIcon], ['msi', binaryIcon], ['dll', binaryIcon], ['so', binaryIcon],
  ['jar', binaryIcon], ['apk', binaryIcon], ['bin', binaryIcon], ['iso', binaryIcon],
  ['eml', mailIcon], ['msg', mailIcon],
])

/** content-type → 图标：文件名没有可用扩展名时按类型兜底。 */
function iconForContentType(contentType) {
  const type = String(contentType || '').toLowerCase()
  if (!type) return null
  if (type.includes('spreadsheet') || type.includes('ms-excel')) return excelIcon
  if (type.includes('wordprocessing') || type.includes('msword')) return wordIcon
  if (type.includes('presentation') || type.includes('ms-powerpoint')) return powerpointIcon
  if (type.includes('pdf')) return pdfIcon
  if (type.includes('zip') || type.includes('compressed') || type.includes('x-tar')) return zipIcon
  if (type.includes('svg')) return svgIcon
  if (type.startsWith('image/')) return imageIcon
  if (type.startsWith('audio/')) return audioIcon
  if (type.startsWith('video/')) return videoIcon
  if (type.startsWith('font/')) return fontIcon
  if (type.startsWith('text/html')) return htmlIcon
  if (type.startsWith('text/css')) return cssIcon
  if (type.startsWith('text/markdown')) return markdownIcon
  if (type.includes('json') || type.includes('xml')) return jsonIcon
  if (type.startsWith('text/')) return textIcon
  if (type.includes('message/rfc822')) return mailIcon
  return null
}

export function attachmentIcon(attachment) {
  const name = String(attachment?.filename || '').trim().toLowerCase()
  const dot = name.lastIndexOf('.')
  const ext = dot > 0 ? name.slice(dot + 1) : ''
  const byExt = ext ? EXT_ICONS.get(ext) : null
  return byExt || iconForContentType(attachment?.contentType) || defaultFileIcon
}

export { defaultFileIcon }
