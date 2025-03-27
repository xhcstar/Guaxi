<template>
	<div class="pdf-viewer">
		<!-- 顶部操作区域 -->
		<div class="header">
			<!-- 跳转到指定页面 -->
			<div class="page-jump">
				<span style="margin-right: 15px; margin-left: 15px;">页码: ({{ currentPage }} / {{ totalPages }})</span>
				<n-input-number size="tiny" type="number" style="width: 95px;" v-model:value="targetPage" min="1"
					max="totalPages" placeholder="输入页码" @keyup.enter="jumpToPage" />
				<n-tooltip placement="top" trigger="hover">
					<template #trigger>
						<n-icon size="16" style="cursor: pointer; margin-left: 15px;" @click="jumpToPage"
							:disabled="currentPage <= 1">
							<svg t="1740835084057" class="icon" viewBox="0 0 1024 1024" version="1.1"
								xmlns="http://www.w3.org/2000/svg" p-id="13283" width="16" height="16">
								<path
									d="M946.41980934 509.46438067L577.24342834 163.35238067v231.204572c-1.755429-0.073143-3.437714-0.463238-5.217524-0.463238H189.12304734c-37.278476 0-69.241905 18.846476-85.284571 46.153142h-11.142095v161.52381h11.117714c16.042667 27.306667 48.006095 46.153143 85.284571 46.153143h382.902857c1.77981 0 3.437714-0.390095 5.217524-0.463238v231.204571l369.200762-369.200762z"
									fill="" p-id="13284"></path>
							</svg>
						</n-icon>
					</template>
					跳转到指定页面
				</n-tooltip>
			</div>

			<!-- PDF 翻页按钮 -->
			<div class="pagination">
				<n-tooltip placement="top" trigger="hover">
					<template #trigger>
						<n-icon size="18" style="cursor: pointer; margin-right: 15px;" @click="prevPage"
							:disabled="currentPage <= 1">
							<svg t="1740840336642" class="icon" viewBox="0 0 1024 1024" version="1.1"
								xmlns="http://www.w3.org/2000/svg" p-id="82509" width="16" height="16">
								<path
									d="M114.034 477.369c-0.086 0.116-0.165 0.23499999-0.25 0.351a58.33 58.33 0 0 0-1.374 1.972c-0.141 0.212-0.273 0.429-0.40999999 0.641-0.375 0.582-0.747 1.167-1.10200001 1.763-0.156 0.261-0.302 0.527-0.453 0.79-0.322 0.561-0.642 1.124-0.948 1.697-0.148 0.278-0.288 0.561-0.431 0.841-0.292 0.571-0.579 1.142-0.852 1.723-0.128 0.274-0.248 0.548-0.371 0.823-0.27 0.6-0.536 1.201-0.78700001 1.809-0.104 0.254-0.202 0.51-0.30099999 0.764-0.25 0.635-0.495 1.273-0.725 1.919-0.083 0.233-0.159 0.469-0.237 0.704-0.227 0.668-0.447 1.338-0.651 2.017-0.065 0.221-0.125 0.441-0.187 0.661-0.197 0.68999999-0.39 1.382-0.563 2.082-0.05400001 0.224-0.102 0.448-0.156 0.67199999a57.86 57.86 0 0 0-0.457 2.08900001c-0.05 0.254-0.091 0.509-0.136 0.763-0.122 0.669-0.241 1.34-0.341 2.017-0.047 0.32199999-0.081 0.647-0.123 0.97-0.079 0.61-0.16 1.21799999-0.22 1.834-0.046 0.467-0.07100001 0.934-0.106 1.401-0.035 0.479-0.08 0.955-0.103 1.438a59.209 59.209 0 0 0 0 5.777c0.024 0.482 0.069 0.959 0.103 1.439 0.035 0.467 0.06 0.934 0.106 1.401 0.059 0.616 0.141 1.225 0.22 1.835 0.042 0.32300001 0.076 0.647 0.123 0.969a58.615 58.615 0 0 0 0.341 2.019c0.046 0.254 0.086 0.509 0.136 0.762a60.71 60.71 0 0 0 0.457 2.091c0.052 0.224 0.099 0.448 0.155 0.671a56.117 56.117 0 0 0 0.563 2.085c0.063 0.22 0.122 0.44 0.187 0.66a60.05 60.05 0 0 0 0.651 2.018c0.08 0.234 0.155 0.47 0.237 0.70400001a57.334 57.334 0 0 0 0.72500001 1.91899999c0.1 0.255 0.196 0.51 0.30099999 0.763a57.266 57.266 0 0 0 0.788 1.812c0.124 0.273 0.243 0.548 0.37 0.821a57.515 57.515 0 0 0 0.853 1.725c0.143 0.279 0.281 0.561 0.43000001 0.839a56.184 56.184 0 0 0 0.94799999 1.698c0.151 0.263 0.298 0.528 0.453 0.789a57.847 57.847 0 0 0 1.102 1.763c0.138 0.213 0.27 0.429 0.41 0.641a58.33 58.33 0 0 0 1.37400001 1.972c0.085 0.117 0.165 0.23499999 0.24999999 0.351a58.818 58.818 0 0 0 6.147 7.085L353.70599999 787.243c22.841 22.841 59.845 22.841 82.68800001 0s22.841-59.845 0-82.688l-134.081-134.081L862.846 570.474c32.32 0 58.476-26.182 58.475-58.475s-26.15400001-58.475-58.475-58.475l-560.533 0L436.394 319.445c22.841-22.841 22.841-59.845 0-82.688-11.421-11.421-26.381-17.131-41.344-17.131s-29.922 5.711-41.34400001 17.131l-233.527 233.527a58.715 58.715 0 0 0-6.14499999 7.084z"
									fill="" p-id="82510"></path>
							</svg>
						</n-icon>
					</template>
					上一页
				</n-tooltip>

				<n-tooltip placement="top" trigger="hover">
					<template #trigger>
						<n-icon size="18" style="cursor: pointer;" @click="nextPage" :disabled="currentPage >= totalPages">
							<svg t="1740839899631" class="icon" viewBox="0 0 1024 1024" version="1.1"
								xmlns="http://www.w3.org/2000/svg" p-id="39786" width="16" height="16">
								<path
									d="M909.966 546.631c0.086-0.116 0.165-0.235 0.25-0.351a58.33 58.33 0 0 0 1.374-1.972c0.141-0.212 0.273-0.429 0.41-0.641 0.375-0.582 0.747-1.167 1.102-1.763 0.156-0.261 0.302-0.527 0.453-0.79 0.322-0.561 0.642-1.124 0.948-1.697 0.148-0.278 0.288-0.561 0.431-0.841 0.292-0.571 0.579-1.142 0.852-1.723 0.128-0.274 0.248-0.548 0.371-0.823 0.27-0.6 0.536-1.201 0.787-1.809 0.104-0.254 0.202-0.51 0.301-0.764 0.25-0.635 0.495-1.273 0.725-1.919 0.083-0.233 0.159-0.469 0.237-0.704 0.227-0.668 0.447-1.338 0.651-2.017 0.065-0.221 0.125-0.441 0.187-0.661 0.197-0.69 0.39-1.382 0.563-2.082 0.054-0.224 0.102-0.448 0.156-0.672a57.86 57.86 0 0 0 0.457-2.089c0.05-0.254 0.091-0.509 0.136-0.763 0.122-0.669 0.241-1.34 0.341-2.017 0.047-0.322 0.081-0.647 0.123-0.97 0.079-0.61 0.16-1.218 0.22-1.834 0.046-0.467 0.071-0.934 0.106-1.401 0.035-0.479 0.08-0.955 0.103-1.438a59.209 59.209 0 0 0 0-5.777c-0.024-0.482-0.069-0.959-0.103-1.439-0.035-0.467-0.06-0.934-0.106-1.401-0.059-0.616-0.141-1.225-0.22-1.835-0.042-0.323-0.076-0.647-0.123-0.969a58.615 58.615 0 0 0-0.341-2.019c-0.046-0.254-0.086-0.509-0.136-0.762a60.71 60.71 0 0 0-0.457-2.091c-0.052-0.224-0.099-0.448-0.155-0.671a56.117 56.117 0 0 0-0.563-2.085c-0.063-0.22-0.122-0.44-0.187-0.66a60.05 60.05 0 0 0-0.651-2.018c-0.08-0.234-0.155-0.47-0.237-0.704a57.334 57.334 0 0 0-0.725-1.919c-0.1-0.255-0.196-0.51-0.301-0.763a57.266 57.266 0 0 0-0.788-1.812c-0.124-0.273-0.243-0.548-0.37-0.821a57.515 57.515 0 0 0-0.853-1.725c-0.143-0.279-0.281-0.561-0.43-0.839a56.184 56.184 0 0 0-0.948-1.698c-0.151-0.263-0.298-0.528-0.453-0.789a57.847 57.847 0 0 0-1.102-1.763c-0.138-0.213-0.27-0.429-0.41-0.641a58.33 58.33 0 0 0-1.374-1.972c-0.085-0.117-0.165-0.235-0.25-0.351a58.818 58.818 0 0 0-6.147-7.085L670.294 236.757c-22.841-22.841-59.845-22.841-82.688 0s-22.841 59.845 0 82.688l134.081 134.081H161.154c-32.32 0-58.476 26.182-58.475 58.475s26.154 58.475 58.475 58.475h560.533L587.606 704.555c-22.841 22.841-22.841 59.845 0 82.688 11.421 11.421 26.381 17.131 41.344 17.131s29.922-5.711 41.344-17.131l233.527-233.527a58.715 58.715 0 0 0 6.145-7.084z"
									fill="" p-id="39787"></path>
							</svg>
						</n-icon>
					</template>
					下一页
				</n-tooltip>
			</div>

			<n-tooltip placement="top" trigger="hover">
				<template #trigger>
					<n-icon size="18" style="cursor: pointer;" @click="downloadMarkdown">
						<svg t="1740840459613" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
							p-id="103000" width="16" height="16">
							<path
								d="M809.841541 0c55.9274 0 101.283157 45.441012 101.453667 101.453667v190.119058a36.489218 36.489218 0 1 1-73.063691 0V101.453667a28.389976 28.389976 0 0 0-28.389976-28.389975h-708.470569a28.389976 28.389976 0 0 0-28.30472 28.475231v819.984347c0 15.772209 12.703022 28.475231 28.389975 28.475231h296.688036a36.489218 36.489218 0 0 1 0 73.063692H101.456227A101.624178 101.624178 0 0 1 0.00256 921.608526V101.453667C0.00256 45.526268 45.443572 0 101.456227 0h708.385314z m122.596953 563.792524h2.557655l1.960869 0.255765 0.426276 0.17051c1.534593 0.085255 2.557655 0.341021 3.410208 0.511532l1.108317 0.34102a31.885438 31.885438 0 0 1 2.642911 0.767297v0.085255l1.449338 0.511531 1.790359 0.767297 1.108317 0.511531 1.449338 0.767297-0.852552-0.341021 2.898676 1.619848a23.615686 23.615686 0 0 1 3.665973 2.728166l1.023063 0.852552 0.852551 0.852552a33.931563 33.931563 0 0 1 3.836484 4.433269l1.193572 1.534594a36.318708 36.318708 0 0 1 2.4724 4.433269l1.023063 2.4724a37.17126 37.17126 0 0 1 0.682041 1.705104l54.051786 164.371992a36.574473 36.574473 0 0 1-27.707934 47.316627l-6.820415 0.682041a36.489218 36.489218 0 0 1-34.698859-25.406044l-23.104155-70.420781a668.571143 668.571143 0 0 1-205.46499 241.783698 480.071934 480.071934 0 0 1-156.699026 75.195071 36.489218 36.489218 0 0 1-15.857464-71.443843 419.79652 419.79652 0 0 0 133.509616-65.731746 594.228624 594.228624 0 0 0 185.089002-224.562151l-88.66539 30.4361a36.489218 36.489218 0 0 1-46.37882-22.677878 36.574473 36.574473 0 0 1 22.677879-46.037799l163.51944-56.524186 2.13138-0.511531a36.574473 36.574473 0 0 1 1.619848-0.426276l3.410208-0.767297h0.085255a55.586379 55.586379 0 0 1 1.875614-0.17051h1.278828l1.449338-0.085255zM448.189056 474.871368a36.489218 36.489218 0 0 1 0 73.148947H225.502518a36.489218 36.489218 0 0 1 0-73.148947h222.686538z m133.680126-206.061777a36.489218 36.489218 0 0 1 0 73.063692h-356.366664a36.489218 36.489218 0 0 1 0-73.063692h356.366664z"
								p-id="103001"></path>
						</svg>
					</n-icon>
				</template>
				下载笔记文件
			</n-tooltip>
		</div>

		<!-- 显示 PDF 中的句子块 -->
		<div ref="pdfContainer" class="pdf-container">
			<div v-for="(item, index) in pdfContent" :key="index" class="pdf-item" @mouseover="showNumber(item)"
				@mouseleave="hideNumber(item)" @click="addToMarkdown(item)">
				<span class="text-content" v-html="formatText(item.text)"></span>
				<span class="number" :id="item.id" v-show="item.isHovered">{{ index + 1 }}</span>
			</div>
		</div>
	</div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue';
import { useFileProcessStore } from '../../stores/fileProcessStore'
import { invoke } from '@tauri-apps/api/core';


const fileProcessStore = useFileProcessStore();
const contentList = computed(() => fileProcessStore.openedCurentfileContent);

const currentPage = ref(1); // 当前页
const totalPages = ref(0);   // 总页数
const targetPage = ref(1);   // 跳转目标页
const pdfContent = ref([]);  // 存储 PDF 中的句子内容
const markdownContent = ref(''); // 存储 Markdown 内容

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

watch(
	() => fileProcessStore.openedCurentfileContent,
	(newContent) => {
		if (newContent && newContent.length > 0) {
			console.log("WWW>>> loadPdfContent contentList", contentList.value)
			loadPdfContent(newContent);
		}
	},
	{ deep: true } // 深度监听
);
// 加载并渲染 PDF 内容
const loadPdfContent = async (contentList) => {
	console.log("loadPdfContent contentList", contentList)
	if (!contentList || contentList.length === 0) {
		console.error("contentList is empty or undefined");
		return;
	} else {
		console.log(">>>>>contentList[0]", contentList[0]);
		totalPages.value = contentList[0].totalPages;
	}

	// 渲染当前页
	console.log("@@@>>> contentList[0].content", contentList[0].content)
	await renderPage(contentList[0].content);
};

// 渲染某一页
const renderPage = async (currentPageContent) => {
	console.log("renderPage currentPageContent", currentPageContent)
	// 提取文本内容
	const textContent = currentPageContent;

	// 将文本按句子分割
	let sentences = splitIntoSentences(textContent);

	// 将分割后的句子添加到 pdfContent 中
	pdfContent.value = sentences.map((sentence, index) => ({
		text: sentence,
		id: `page-sentence-${index}`,
		isHovered: false,
	}));
};

const splitIntoSentences = (content) => {
	console.log("splitIntoSentences content", content)
	// const sentenceEndRegex = /[。！？.!?]/; // 中文、英文的句号、问号、感叹号等
	const sentenceEndRegex = /[\n]/; // 中文、英文的句号、问号、感叹号等
	const urlRegex = /(?:https?|ftp):\/\/[^\s]+/; // 匹配 URL
	const emailRegex = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/; // 匹配电子邮件
	const ipRegex = /(\d{1,3}\.){3}\d{1,3}/; // 匹配 IP 地址（简单的 IPv4 格式）

	let sentences = [];
	let currentSentence = '';

	// 按字符遍历内容
	for (let i = 0; i < content.length; i++) {
		const char = content[i];
		currentSentence += char;

		// 如果当前字符是句子结束符号
		if (sentenceEndRegex.test(char)) {
			// 检查当前句子是否包含 URL、电子邮件或 IP 地址
			// if (!urlRegex.test(currentSentence) && !emailRegex.test(currentSentence) && !ipRegex.test(currentSentence)) {
			//     sentences.push(currentSentence.trim()); // 将句子添加到结果中
			//     currentSentence = ''; // 清空当前句子
			// }
			if (!urlRegex.test(currentSentence) && !emailRegex.test(currentSentence) && !ipRegex.test(currentSentence)) {
				sentences.push(currentSentence); // 将句子添加到结果中
				currentSentence = ''; // 清空当前句子
			}
		}
	}

	// 处理最后一句（如果没有句子结束符号）
	if (currentSentence.trim()) {
		sentences.push(currentSentence.trim());
	}

	return sentences;
};

const formatText = (text) => {
	// 替换多个连续空格为单个空格
	// text = text.replace(/\s+/g, ' ');
	// 在每个句子结束后加上换行
	// text = text.replace(/([。！？.!?])/g, '$1<br />');
	return text;
};

const getCurrentPageContent = async (pageNumber, fileProcessStore) => {
	fileProcessStore.curentPage = pageNumber;

	// 从文件路径中提取文件扩展名
	console.log("getCurrentPageContent fileProcessStore.selectedFile", fileProcessStore.openedCurentfileContent[0].fileName);
	const fileExtension = fileProcessStore.openedCurentfileContent[0].fileName.split('.').pop().toLowerCase();
	console.log("getCurrentPageContent fileExtension", fileExtension);

	// 根据文件扩展名设置 filetype
	let filetype;
	switch (fileExtension) {
		case 'pdf':
			filetype = 'pdf';
			break;
		case 'md':
			filetype = 'md';
			break;
		case 'txt':
			filetype = 'txt';
			break;
		case 'docx':
			filetype = 'doc';
			break;
		default:
			filetype = 'unknown';
			break;
	}

	try {
		const request = {
			filetype: filetype,
			filepath: fileProcessStore.selectedFile,
			curpage: fileProcessStore.curentPage,
			lastpage: fileProcessStore.lastPage,
		};
		console.log('getCurrentPageContent request:', request);

		// 调用后端 API 并等待结果
		const response = await invoke('openfile', { request: request });

		// 更新 store 中的数据
		fileProcessStore.getCurentOpenedFileContent(response, 0);
		console.log('getCurrentPageContent Response:', response);

		// 返回响应结果
		return response;
	} catch (error) {
		console.error('Error calling openfile API:', error);
		throw error; // 抛出错误，以便调用方可以处理
	}
};
// 上一页
const prevPage = async () => {
	console.log("111prevPage currentPage.value", currentPage.value);
	if (currentPage.value > 1) {
		currentPage.value -= 1;
		console.log("prevPage currentPage.value", currentPage.value);

		// 等待 getCurrentPageContent 完成
		await getCurrentPageContent(currentPage.value, fileProcessStore);

		// 确保 contentList 已更新后再调用 loadPdfContent
		if (contentList.value && contentList.value.length > 0) {
			loadPdfContent(contentList.value);
		}
	}
};

// 下一页
const nextPage = async () => {
	console.log("111nextPage currentPage.value", currentPage.value);

	// 确保 contentList 存在且不为空
	if (!contentList.value || contentList.value.length === 0) {
		console.error("contentList is empty or undefined");
		return;
	}

	// 获取总页数
	totalPages.value = contentList.value[0].totalPages;

	if (currentPage.value < totalPages.value) {
		currentPage.value += 1;
		console.log("nextPage currentPage.value", currentPage.value);

		// 等待 getCurrentPageContent 完成
		await getCurrentPageContent(currentPage.value, fileProcessStore);

		// 确保 contentList 已更新后再调用 loadPdfContent
		if (contentList.value && contentList.value.length > 0) {
			loadPdfContent(contentList.value);
		}
	}
};

// 跳转到指定页面
const jumpToPage = async () => {
	try {
		// 确保 contentList 存在且不为空
		if (!contentList.value || contentList.value.length === 0) {
			console.error("contentList is empty or undefined");
			return;
		}

		// 获取总页数
		totalPages.value = contentList.value[0].totalPages;
		console.log(`jumpToPage targetPage.value: ${targetPage.value}, totalPages.value: ${totalPages.value}`);

		// 检查目标页码是否有效
		if (targetPage.value >= 1 && targetPage.value <= totalPages.value) {
			console.log("2jumpToPage targetPage.value", targetPage.value);
			currentPage.value = targetPage.value;

			// 等待 getCurrentPageContent 完成
			console.log("Calling getCurrentPageContent...");
			await getCurrentPageContent(currentPage.value, fileProcessStore);
			console.log("getCurrentPageContent completed.");

			// 确保 contentList 已更新后再调用 loadPdfContent
			if (contentList.value && contentList.value.length > 0) {
				loadPdfContent(contentList.value);
			}
		}
	} catch (error) {
		console.error("Error in jumpToPage:", error);
	}
};

// 显示编号
const showNumber = (item) => {
	item.isHovered = true;
};

// 隐藏编号
const hideNumber = (item) => {
	item.isHovered = false;
};

// 添加内容到 Markdown
const addToMarkdown = (item) => {
	markdownContent.value += `\n## ${item.id}\n${item.text}\n`;
};

// 下载 Markdown 文件
const downloadMarkdown = () => {
	const blob = new Blob([markdownContent.value], { type: 'text/markdown' });
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = 'pdf-content.md';
	link.click();
	URL.revokeObjectURL(url);
};

const openFileSelectedNewWindows = async (filePath) => {
	console.log('openFileSelectedNewWindows', filePath);
	if (filePath) {
		fileProcessStore.setSelectedFile(filePath);

		// 从文件路径中提取文件扩展名
		const fileExtension = filePath.split('.').pop().toLowerCase();

		// 根据文件扩展名设置 filetype
		let filetype;
		switch (fileExtension) {
			case 'pdf':
				filetype = 'pdf';
				break;
			case 'md':
				filetype = 'md';
				break;
			case 'txt':
				filetype = 'txt';
				break;
			case 'docx':
				filetype = 'doc';
				break;
			case 'doc':
				filetype = 'doc';
				break;
			case 'html':
				filetype = 'html';
				break;
			case 'epub':
				filetype = 'epub';
				break;
			default:
				filetype = 'unknown';
				break;
		}

		try {
			const openfile_request = {
				filetype: filetype,
				filepath: fileProcessStore.selectedFile,
				curpage: fileProcessStore.curentPage,
				lastpage: fileProcessStore.lastPage,
			};
			console.log('openfile_request:', openfile_request);
			const response = await invoke('openfile', { request: openfile_request });
			fileProcessStore.getCurentOpenedFileContent(response, 0);
			console.log('Response:', response);
		} catch (error) {
			console.error('Error calling openfile API:', error);
		}
	} else {
		console.log('未选择文件');
	}
}

// 页面加载时调用
onMounted(() => {
	if (props.filePath) {
		openFileSelectedNewWindows(props.filePath)
	} else {
		console.error('filePath is undefined', props.filePath)
	}

	if (contentList.value) {
		loadPdfContent(contentList.value)
	}
});
</script>

<style scoped>
.pdf-viewer {
	position: relative;
	display: flex;
	flex-direction: column;
	max-height: 95vh;
	/* 最大高度为视口的80%，你可以根据需要调整 */
	overflow-y: auto;
	/* 当内容超出时触发滚动 */
}

.header {
	/* position: relative; 固定定位 */
	position: sticky;
	top: 0px;
	/* 固定在顶部 */
	width: 99%;
	/* 占满父组件的宽度 */
	max-width: inherit;
	/* 继承父组件的最大宽度 */
	z-index: 1000;
	/* 确保 header 在最上层 */
	background-color: white;
	/* 背景颜色，避免内容透出 */
	padding: 0px 0px;
	display: flex;
	justify-content: flex-start;
	align-items: center;
	box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
}

.page-jump {
	display: flex;
	align-items: center;
}

.page-jump n-input-number {
	width: 40px;
	padding: 5px;
	margin-right: 10px;
	margin-left: 50;
}

.download-btn {
	padding: 5px 5px;
	background-color: #4CAF50;
	color: white;
	border: none;
	cursor: pointer;
}

.download-btn:hover {
	background-color: #45a049;
}

.pagination {
	display: flex;
	align-items: center;
	justify-content: center;
	margin-top: 0;
	margin-left: 10;
	flex-grow: 1;
	/* 让翻页区域伸展，占据剩余空间 */
	/* gap: 15px; */
}

.pagination button {
	padding: 5px 10px;
	margin: 5 10px;
	cursor: pointer;
}

.pagination button:disabled {
	cursor: not-allowed;
}

.pdf-container {
	flex: 1 1 auto;
	flex-wrap: wrap;
	/* 自动换行 */
	gap: 8px;
	/* 控制句子之间的间距 */
	padding: 10px;
	margin-top: 60px;
	max-width: 900px;
	/* 限制容器的最大宽度 */
	margin: 0 auto;
	/* 居中显示 */
	justify-content: flex-start;
	/* 左对齐 */
	height: auto;
	/* 让容器根据内容高度自动调整 */
	width: calc(100% - 20px);
}

.pdf-item {
	/* display: inline-block; */
	display: block;
	/* 每个句子保持在一行 */
	word-wrap: break-word;
	/* 自动换行 */
	padding: 5px;
	background-color: #f9f9f9;
	border-radius: 4px;
	box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
	cursor: pointer;
	min-width: 100px;
	/* 设置最小宽度，避免过小 */
	max-width: 100%;
	/* 确保句子在容器内自适应 */
	white-space: normal;
	/* 允许自动换行 */
	/* overflow: hidden; */
	/* 隐藏溢出的文本 */
	line-height: 1.6;
	/* 行高设置为1.6 */
	font-size: 17px;
	transition: box-shadow 0.3s ease, transform 0.3s ease;
	/* 添加过渡效果 */
}

.pdf-item:hover {
	box-shadow: 0 0 10px rgba(140, 3, 194, 0.5);
	/* 蓝色光效 */
	transform: scale(1.02);
	/* 轻微放大效果 */
}

.text-content {
	display: inline-block;
	word-break: break-word;
	/* 保证长单词能够换行 */
	text-align: left;
}

.number {
	position: absolute;
	top: -5px;
	left: -5px;
	background-color: rgba(14, 152, 231, 0.6);
	color: white;
	padding: 2px 6px;
	font-size: 12px;
	border-radius: 50%;
	cursor: pointer;
	pointer-events: none;
	display: none;
}

.pdf-item:hover .number {
	display: inline-block;
}
</style>



<!-- <script setup>
import { ref, onMounted } from 'vue';
import * as pdfjsLib from 'pdfjs-dist';

pdfjsLib.GlobalWorkerOptions.workerSrc = '../../node_modules/pdfjs-dist/build/pdf.worker.mjs';

const pdfUrl = '/book.pdf'; // PDF 文件路径
const currentPage = ref(1); // 当前页
const totalPages = ref(0);   // 总页数
const targetPage = ref(1);   // 跳转目标页
const pdfContent = ref([]);  // 存储 PDF 中的句子内容
const markdownContent = ref(''); // 存储 Markdown 内容

// 加载并渲染 PDF 内容
const loadPdfContent = async () => {
    const loadingTask = pdfjsLib.getDocument(pdfUrl);
    const pdf = await loadingTask.promise;
    totalPages.value = pdf.numPages;

    // 渲染当前页
    await renderPage(currentPage.value, pdf);
};

// 渲染某一页
const renderPage = async (pageNum, pdf) => {
    const page = await pdf.getPage(pageNum);
    const scale = 1.5;
    const viewport = page.getViewport({ scale });

    // 提取文本内容
    const textContent = await page.getTextContent();
    const items = textContent.items;

    // 将文本按句子分割
    let sentences = splitIntoSentences(items);

    // 将分割后的句子添加到 pdfContent 中
    pdfContent.value = sentences.map((sentence, index) => ({
        text: sentence,
        id: `page-${pageNum}-sentence-${index}`,
        position: { x: 0, y: 0 },  // 可以使用位置来进一步控制显示
        isHovered: false,
    }));
};

const splitIntoSentences = (items) => {
    const sentenceEndRegex = /[。！？.!?]/;  // 中文、英文的句号、问号、感叹号等
    const urlRegex = /(?:https?|ftp):\/\/[^\s]+/; // 匹配 URL
    const emailRegex = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/; // 匹配电子邮件
    const ipRegex = /(\d{1,3}\.){3}\d{1,3}/; // 匹配 IP 地址（简单的 IPv4 格式）

    let sentences = [];
    let currentSentence = '';

    items.forEach(item => {
        // const word = item.str.trim();
        const word = item.str;
        currentSentence += word;

        // 如果当前词是 URL、电子邮件或 IP 地址，跳过此点符号，并继续追加到当前句子
        if (urlRegex.test(word) || emailRegex.test(word) || ipRegex.test(word)) {
            currentSentence += ' ';  // 保持句子的连续性
            return;
        }

        // 判断是否遇到句子结束标点符号，并且不在 URL、电子邮件或 IP 地址之后
        if (sentenceEndRegex.test(currentSentence.charAt(currentSentence.length - 1))) {
            // 如果当前句子以句号结尾且不是 URL 或电子邮件的一部分，认为句子结束
            if (!urlRegex.test(currentSentence) && !emailRegex.test(currentSentence) && !ipRegex.test(currentSentence)) {
                sentences.push(currentSentence);
                currentSentence = '';  // 清空缓存，准备下一句
            }
        }
    });

    // 如果最后有剩余文本，处理最后一句
    if (currentSentence.trim()) {
        sentences.push(currentSentence);
    }

    return sentences;
};



const formatText = (text) => {
    // 替换多个连续空格为单个空格
    // text = text.replace(/\s+/g, ' ');
    // 在每个句子结束后加上换行
    // text = text.replace(/([。！？.!?])/g, '$1<br />');
    return text;
};

// 上一页
const prevPage = () => {
    if (currentPage.value > 1) {
        currentPage.value -= 1;
        loadPdfContent();
    }
};

// 下一页
const nextPage = () => {
    if (currentPage.value < totalPages.value) {
        currentPage.value += 1;
        loadPdfContent();
    }
};

// 跳转到指定页面
const jumpToPage = () => {
    if (targetPage.value >= 1 && targetPage.value <= totalPages.value) {
        currentPage.value = targetPage.value;
        loadPdfContent();
    }
};

// 显示编号
const showNumber = (item) => {
    item.isHovered = true;
};

// 隐藏编号
const hideNumber = (item) => {
    item.isHovered = false;
};

// 添加内容到 Markdown
const addToMarkdown = (item) => {
    markdownContent.value += `\n## ${item.id}\n${item.text}\n`;
};

// 下载 Markdown 文件
const downloadMarkdown = () => {
    const blob = new Blob([markdownContent.value], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'pdf-content.md';
    link.click();
    URL.revokeObjectURL(url);
};

// 页面加载时调用
onMounted(() => {
    loadPdfContent();
});
</script> -->