<template>
	<div class="guaxi-reader">
		<component :is="readerComponent" v-if="readerComponent" :filePath="filePath" :fileType="fileType" 
			:fileName="fileName" @close=" emit('close')" />
		<div v-else class="unknown-type">
			不支持的文件类型: {{ fileType }}
			<p>文件名: {{ fileData?.name || '未知' }}</p>
		</div>
	</div>
</template>

<script setup>
import { computed } from 'vue'
import GuaxiPDFReader from './GuaxiPDFReader.vue'
import GuaxiMarkdownReader from './GuaxiMarkdownReader.vue'
import GuaxiHtmlReader from './GuaxiHtmlReader.vue'
import GuaxiEpubReader from './GuaxiEpubReader.vue'
import GuaxiWordReader from './GauxiWordReader.vue'
import GuaxiTextReader from './GuaxiTextReader.vue'
// import { Base64 } from 'js-base64'

const props = defineProps({
	filePath: {
		type: String,
		required: true,
	},
	fileType: {
		type: String,
		required: true,
		validator: (value) => ['pdf', 'md', 'html', 'epub', 'doc', 'txt'].includes(value)
	},
	fileName: {
		type: String,
		required: true,
	}
})

// Base64 解码文件路径
// const decodedFilePath = computed (() => {
//   try {
//     // 先URL解码，再Base64解码
//     const urlDecoded = decodeURIComponent(props.filePath)
// 	console.log('urlDecoded', urlDecoded)
//     return Base64.decode(urlDecoded)
//   } catch (error) {
//     console.error('路径解码失败:', error)
//     return null
//   }
// })

// 更严格的组件选择逻辑
const readerComponent = computed(() => {
	const { fileType, filePath, fileName } = props
	console.log('readerComponent', fileType, filePath, fileName)

	if (!fileType || !fileName) {
		console.error('Missing required props')
		return null
	}

	const typeMap = {
		pdf: GuaxiPDFReader,
		md: GuaxiMarkdownReader,
		markdown: GuaxiMarkdownReader,
		html: GuaxiHtmlReader,
		htm: GuaxiHtmlReader,
		epub: GuaxiEpubReader,
		doc: GuaxiWordReader,
		docx: GuaxiWordReader,
		word: GuaxiWordReader,
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
	background: white;
	z-index: 1000;
	overflow: auto;
	display: flex;
	flex-direction: column;
}

.unknown-type {
	padding: 20px;
	color: red;
	font-size: 18px;
	text-align: center;
	margin: auto;
}

.unknown-type p {
	color: #666;
	font-size: 14px;
	margin-top: 10px;
}
</style>