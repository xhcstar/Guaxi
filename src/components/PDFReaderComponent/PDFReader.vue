<template>
    <div class="pdf-viewer">
        <!-- 顶部操作区域 -->
        <div class="header">
            <!-- 跳转到指定页面 -->
            <div class="page-jump">
                <input type="number" v-model="targetPage" min="1" max="totalPages" placeholder="输入页码"
                    @keyup.enter="jumpToPage" />
                <button @click="jumpToPage">跳转</button>
            </div>

            <!-- PDF 翻页按钮 -->
            <div class="pagination">
                <button @click="prevPage" :disabled="currentPage <= 1">上一页</button>
                <span>页码: {{ currentPage }} / {{ totalPages }}</span>
                <button @click="nextPage" :disabled="currentPage >= totalPages">下一页</button>
            </div>

            <!-- 下载 Markdown 文件按钮 -->
            <button @click="downloadMarkdown" class="download-btn">下载Markdown</button>
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
import { ref, onMounted } from 'vue';
import * as pdfjsLib from 'pdfjs-dist';

pdfjsLib.GlobalWorkerOptions.workerSrc = '../../node_modules/pdfjs-dist/build/pdf.worker.mjs';

const pdfUrl = '/rfc5246.pdf'; // PDF 文件路径
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
        const word = item.str.trim();
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
    text = text.replace(/\s+/g, ' ');
    // 在每个句子结束后加上换行
    text = text.replace(/([。！？.!?])/g, '$1<br />');
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
</script>

<style scoped>
.pdf-viewer {
    position: relative;
    display: flex;
    flex-direction: column;
    max-height: 95vh; /* 最大高度为视口的80%，你可以根据需要调整 */
    overflow-y: auto; /* 当内容超出时触发滚动 */
}

.header {
    flex: 0 0 auto;
    top: 18px;
    /* width: 95%; */
    padding: 10px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
}

.page-jump {
    display: flex;
    align-items: center;
}

.page-jump input {
    width: 80px;
    padding: 5px;
    margin-right: 10px;
}

.download-btn {
    padding: 5px 15px;
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
    flex-grow: 1; /* 让翻页区域伸展，占据剩余空间 */
}

.pagination button {
    padding: 5px 10px;
    margin: 0 10px;
    cursor: pointer;
}

.pagination button:disabled {
    cursor: not-allowed;
}

.pdf-container {
    flex: 1 1 auto;
    flex-wrap: wrap; /* 自动换行 */
    gap: 5px;  /* 控制句子之间的间距 */
    padding: 10px;
    margin-top: 60px;
    max-width: 900px; /* 限制容器的最大宽度 */
    margin: 0 auto; /* 居中显示 */
    justify-content: flex-start; /* 左对齐 */
    height: auto; /* 让容器根据内容高度自动调整 */
}

.pdf-item {
    display: inline-block; /* 每个句子保持在一行 */
    word-wrap: break-word; /* 自动换行 */
    padding: 5px;
    background-color: #f9f9f9;
    border-radius: 4px;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
    cursor: pointer;
    min-width: 150px; /* 设置最小宽度，避免过小 */
    max-width: 100%; /* 确保句子在容器内自适应 */
    white-space: normal; /* 允许自动换行 */
    overflow: hidden; /* 隐藏溢出的文本 */
    line-height: 1.6; /* 行高设置为1.6 */
    font-size: 14px;
    transition: box-shadow 0.3s ease, transform 0.3s ease; /* 添加过渡效果 */
}

.pdf-item:hover {
    box-shadow: 0 0 10px rgba(0, 0, 255, 0.5); /* 蓝色光效 */
    transform: scale(1.02); /* 轻微放大效果 */
}

.text-content {
    display: inline-block;
    word-break: break-word; /* 保证长单词能够换行 */
    text-align: left;
}

.number {
    position: absolute;
    top: -5px;
    right: -5px;
    background-color: rgba(255, 0, 0, 0.6);
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
