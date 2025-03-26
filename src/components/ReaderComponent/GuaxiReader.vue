<template>
	<div class="guaxi-reader">
		<component :is="readerComponent" v-if="readerComponent" :file="fileData" @close="emit('close')" />
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

const props = defineProps({
	fileType: {
		type: String,
		required: true
	},
	fileData: {
		type: Object,
		required: true,
		validator: (value) => {
			// 验证 fileData 至少包含 name 或 path 属性
			return value.name !== undefined || value.path !== undefined
		}
	}
})

const emit = defineEmits(['close'])

// 根据文件类型和扩展名决定使用哪个阅读器组件
const readerComponent = computed(() => {
	console.log('readerComponent', props.fileType, props.fileData)
	const { fileType, fileData } = props

	// 首先检查明确的文件类型
	switch (fileType.toLowerCase()) {
		case 'pdf':
			return GuaxiPDFReader
		case 'md':
		case 'markdown':
			return GuaxiMarkdownReader
		case 'html':
		case 'htm':
			return GuaxiHtmlReader
		case 'epub':
			return GuaxiEpubReader
		case 'doc':
		case 'docx':
		case 'word':
			return GuaxiWordReader
		case 'txt':
			return GuaxiTextReader
	}

	// 如果文件类型不明确，尝试从文件名中提取扩展名
	if (fileData?.name) {
		const fileName = fileData.name.toLowerCase()
		const extension = fileName.split('.').pop()

		switch (extension) {
			case 'pdf':
				return GuaxiPDFReader
			case 'md':
				return GuaxiMarkdownReader
			case 'html':
			case 'htm':
				return GuaxiHtmlReader
			case 'epub':
				return GuaxiEpubReader
			case 'doc':
			case 'docx':
				return GuaxiWordReader
			case 'txt':
				return GuaxiTextReader
		}
	}

	// 如果还是无法确定，返回 null 显示不支持的消息
	return null
})
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