import { createApp, h, ref, onMounted } from 'vue'
import { renderAsync } from 'docx-preview'

// 最小 docx：手工构造一个含 styles.xml 的 OOXML 包（重点是那条 p{margin:0} 级别的全局规则）
async function buildSampleDocx() {
  const { default: JSZip } = await import('jszip')
  const zip = new JSZip()

  zip.file('[Content_Types].xml', `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>`)

  zip.file('_rels/.rels', `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>`)

  zip.file('word/_rels/document.xml.rels', `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>`)

  zip.file('word/styles.xml', `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:styleId="Evil"><w:name w:val="Evil"/>
<w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr>
<w:rPr><w:sz w:val="24"/></w:rPr></w:style>
</w:styles>`)

  zip.file('word/document.xml', `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body><w:p><w:r><w:t>这是 Word 文档正文第一段。</w:t></w:r></w:p>
<w:p><w:r><w:t>第二段用来验证样式是否穿透到页面。</w:t></w:r></w:p></w:body></w:document>`)

  return await zip.generateAsync({ type: 'uint8array' })
}

const App = {
  setup() {
    const host = ref(null)
    const result = ref('running')
    const injectedStyles = ref(0)

    onMounted(async () => {
      const bytes = await buildSampleDocx()
      const sh = host.value.attachShadow({ mode: 'open' })
      const styleHost = document.createElement('div')
      const body = document.createElement('div')
      sh.appendChild(styleHost)
      sh.appendChild(body)

      await renderAsync(bytes, body, styleHost, { inWrapper: true, breakPages: false })

      // 统计：影子树里的 <style> 数量 vs 页面 <head> 里的 <style> 数量
      injectedStyles.value = sh.querySelectorAll('style').length
      const headStyles = document.head.querySelectorAll('style').length
      const shadowText = body.textContent || ''
      // 关键断言：正文是否渲染出来（有格式）
      result.value = JSON.stringify({
        shadowStyles: injectedStyles.value,
        headStylesBefore: 2,
        headStylesAfter: headStyles,
        paragraphs: body.querySelectorAll('p').length,
        hasText: shadowText.includes('第一段'),
        // 页面（影子外）是否被污染：插入一个探针段落量它的 margin
        probeMargin: (() => {
          const p = document.createElement('p')
          p.textContent = 'probe'
          document.body.appendChild(p)
          const m = getComputedStyle(p).margin
          p.remove()
          return m
        })(),
      })
    })

    return () => h('div', { style: 'padding:16px' }, [
      h('div', { id: 'result', style: 'font-family:monospace;font-size:12px;background:#eef;padding:8px;margin-bottom:12px' }, result.value),
      h('p', { style: 'color:#333' }, '这是影子树之外的普通段落，样式不应被 Word 文档影响。'),
      h('div', { ref: host, style: 'border:1px solid #ccc;height:300px;overflow:auto' }),
    ])
  },
}

createApp(App).mount('#app')
