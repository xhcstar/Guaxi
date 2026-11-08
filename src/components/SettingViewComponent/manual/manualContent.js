import { startChapters } from './chapters/01-start.js'
import { pageChaptersA } from './chapters/02-pages-a.js'
import { pageChaptersB } from './chapters/03-pages-b.js'
import { pageChaptersC } from './chapters/04-pages-c.js'
import { advancedChapters } from './chapters/05-advanced.js'
import { referenceChapters } from './chapters/06-reference.js'

export const manualGroups = ['入门', '页面详解', '扩展与配置', '参考']

export const manualChapters = [
  ...startChapters,
  ...pageChaptersA,
  ...pageChaptersB,
  ...pageChaptersC,
  ...advancedChapters,
  ...referenceChapters,
]

export const manualStats = {
  chapters: manualChapters.length,
  sections: manualChapters.reduce((total, chapter) => total + chapter.sections.length, 0),
  pages: 9,
}

/** 检索结果里显示纯文本，去掉正文里的行内标签。 */
export const stripHtml = (value) =>
  String(value || '')
    .replace(/<br\s*\/?>/gi, ' ')
    .replace(/<\/(p|li|div|tr|h\d)>/gi, ' ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim()

const blockToText = (block) => {
  if (!block) return ''
  switch (block.type) {
    case 'p':
    case 'lead':
    case 'h4':
      return block.text
    case 'list':
      return (block.items || []).join(' ')
    case 'steps':
      return (block.items || []).map((item) => `${item.title || ''} ${item.text || ''}`).join(' ')
    case 'cards':
    case 'flow':
      return (block.items || []).map((item) => (typeof item === 'string' ? item : `${item.title || ''} ${item.text || ''}`)).join(' ')
    case 'table':
      return [...(block.head || []), ...(block.rows || []).flat()].join(' ')
    case 'code':
      return block.text
    case 'callout':
      return `${block.title || ''} ${block.text || ''}`
    case 'defs':
      return (block.items || []).map((item) => `${item.term || ''} ${item.text || ''}`).join(' ')
    case 'qa':
      return (block.items || []).map((item) => `${item.q || ''} ${item.a || ''}`).join(' ')
    case 'shortcuts':
      return (block.groups || [])
        .flatMap((group) => [group.title, ...(group.rows || []).map((row) => row.join(' '))])
        .join(' ')
    case 'figure':
      return block.caption || ''
    default:
      return ''
  }
}

/**
 * 手册检索索引：以「小节」为粒度，把正文、关键词、章节标题一起收进 haystack。
 * 这样搜“同步”能同时命中设置、任务、Git 和 FAQ，搜具体按钮名也能落在对应小节。
 */
export const manualSearchEntries = manualChapters.flatMap((chapter) =>
  chapter.sections.map((section) => {
    const blocksText = section.blocks.map(blockToText).map(stripHtml).join(' ')
    const keywords = (section.keywords || []).join(' ')
    return {
      chapterId: chapter.id,
      chapterNumber: chapter.number,
      chapterTitle: chapter.title,
      sectionId: section.id,
      sectionTitle: section.title,
      haystack: stripHtml(
        `${chapter.number} ${chapter.title} ${chapter.eyebrow} ${chapter.subtitle} ${chapter.summary} ${section.title} ${keywords} ${blocksText}`,
      ),
      body: stripHtml(blocksText),
    }
  }),
)
