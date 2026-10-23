<template>
  <n-message-provider>
    <div class="guaxi-reader">
      <!-- Office 文档走统一入口：交给本机 Office/WPS，否则 WordCraft 系列，不再自绘版式。 -->
      <DocOfficeReader
        v-if="isOfficeType"
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
    validator: (value) => ['pdf', 'md', 'markdown', 'html', 'htm', 'epub', 'doc', 'txt', 'docx', 'word'].includes(value)
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
/** Office 三件套（含旧格式）不再自绘，统一交给 DocOfficeReader。 */
const isOfficeType = computed(() =>
  ['doc', 'docx', 'word', 'xls', 'xlsx', 'ods', 'ppt', 'pptx', 'odp', 'rtf', 'odt'].includes(
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
    txt: GuaxiTextReader
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