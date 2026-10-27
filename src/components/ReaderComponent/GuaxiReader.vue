<template>
  <n-message-provider>
    <div class="guaxi-reader">
      <!--
        Word 走 Guaxi 自己的双栏阅读器（左 Word 版式 + 右 Markdown），看书、做笔记、翻译用的；
        「用 Office 打开」是同一个工具栏上的另一个入口，不替代阅读。

        其余 Office（表格 / 幻灯片）没有对应的双栏阅读器，交给 DocOfficeReader，
        由它把文件交给本机 Office/WPS —— 这两类文件在 Guaxi 里自绘版式只会「差不多但不对」。
      -->
      <DocOfficeReader
        v-if="isSpreadsheetOrSlides"
        :path="filePath"
        :name="fileName"
        :kind="fileType"
      />
      <component :is="readerComponent" v-else-if="readerComponent" :filePath="filePath" :fileType="fileType"
        :fileName="fileName" :library="library" :importFileId="Number(importFileId) || 0" @close=" emit('close')" />
      <div v-else class="unknown-type">
        不支持的文件类型: {{ fileType }}
        <p>文件名: {{ fileData?.name || '未知' }}</p>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed } from 'vue'
import GuaxiConvertPDFReader from './GuaxiConvertPDFReader.vue'
import GuaxiMarkdownReader from './GuaxiMarkdownReader.vue'
import GuaxiHtmlReader from './GuaxiHtmlReader.vue'
import GuaxiEpubReader from './GuaxiEpubReader.vue'
import GuaxiTextReader from './GuaxiTextReader.vue'
import GuaxiWordReader from './GauxiWordReader.vue'
import DocOfficeReader from './readers/DocOfficeReader.vue'

const logger = createLogger('GuaxiReader')
// import { Base64 } from 'js-base64'

const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['pdf', 'md', 'markdown', 'html', 'htm', 'epub', 'mobi', 'azw', 'azw3', 'doc', 'txt', 'docx', 'word', 'rtf', 'odt', 'xls', 'xlsx', 'ods', 'ppt', 'pptx', 'odp'].includes(value)
  },
  fileName: {
    type: String,
    required: true,
  },
  library: {
    type: String,
    required: true,
  },
  importFileId: {
    type: [Number, String],
    default: null,
  }
})

// Base64 解码文件路径
// const decodedFilePath = computed (() => {
//   try {
//     // 先URL解码，再Base64解码
//     const urlDecoded = decodeURIComponent(props.filePath)
// 	logger.info('urlDecoded', urlDecoded)
//     return Base64.decode(urlDecoded)
//   } catch (error) {
//     logger.error('路径解码失败:', error)
//     return null
//   }
// })

// 更严格的组件选择逻辑
/**
 * 只有「表格 / 幻灯片」交给 DocOfficeReader（本机 Office/WPS）。
 *
 * Word 不在此列：它有 Guaxi 自己的双栏阅读器（左 Word 版式 + 右 Markdown），
 * 那是用来读书、做笔记、翻译的，与「用 Office 编辑原文件」是两件事。
 * 把 word 划进来等于把双栏阅读器整个下线 —— 这次就是那么丢掉的，别再合并。
 */
const isSpreadsheetOrSlides = computed(() =>
  ['xls', 'xlsx', 'ods', 'ppt', 'pptx', 'odp'].includes(
    String(props.fileType || '').toLowerCase()
  )
)

const readerComponent = computed(() => {
  const { fileType, filePath, fileName, library } = props
  logger.info('readerComponent', fileType, filePath, fileName, library)

  if (!fileType || !fileName) {
    logger.error('Missing required props')
    return null
  }

  const typeMap = {
    pdf: GuaxiConvertPDFReader,
    md: GuaxiMarkdownReader,
    markdown: GuaxiMarkdownReader,
    html: GuaxiHtmlReader,
    htm: GuaxiHtmlReader,
    epub: GuaxiEpubReader,
    // Kindle 系电子书复用 EPUB 阅读器：它是「源文件 + 转换后 Markdown」双栏的通用电子书壳，
    // 章节与目录由后端解析器产出；后端 read_document_for_reader 已经认得 mobi / azw / azw3，
    // 前端只需把真实类型透传下去。
    mobi: GuaxiEpubReader,
    azw: GuaxiEpubReader,
    azw3: GuaxiEpubReader,
    txt: GuaxiTextReader,
    // Word：双栏阅读器（左版式 + 右 Markdown）。旧格式 doc 与 docx/rtf/odt 都走它。
    doc: GuaxiWordReader,
    docx: GuaxiWordReader,
    word: GuaxiWordReader,
    rtf: GuaxiWordReader,
    odt: GuaxiWordReader,
  }

  return typeMap[fileType] || null
})

const emit = defineEmits(['close'])

</script>

<style scoped>
.guaxi-reader {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  z-index: 1000;
  overflow: auto;
  display: flex;
  flex-direction: column;
}

.unknown-type {
  padding: 20px;
  color: var(--n-color-error, #e06c75);
  font-size: 18px;
  text-align: center;
  margin: auto;
}

.unknown-type p {
  color: var(--guaxi-text-secondary);
  font-size: 14px;
  margin-top: 10px;
}
</style>