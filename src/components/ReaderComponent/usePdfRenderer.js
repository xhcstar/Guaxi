import { ref, computed, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";

export default function usePdfRenderer(props) {
  // 增强props验证
  if (!props || typeof props !== "object") {
    throw new Error("usePdfRenderer: props必须是对象");
  }
  if (!props.filePath || typeof props.filePath !== "string") {
    throw new Error("usePdfRenderer: filePath必须是字符串且不能为空");
  }

  const renderMode = ref("image");
  const currentPageData = ref(null);
  const textLayer = ref(null);
  const isLoading = ref(false);
  const error = ref(null);

  const loadPage = async (pageNum, mode = renderMode.value) => {
    isLoading.value = true;
    error.value = null;

    try {
      // 构造完全匹配后端的请求结构
      const request = {
        filepath: props.filePath, // 注意后端使用filepath(小写)
        curpage: Number(pageNum), // 确保是数字类型
        mode: String(mode), // 确保是字符串
        scale: 1.0, // 默认缩放
        lastpage: 1,
        // 后端不需要file_type参数，已移除
      };

      console.log("发送请求:", { request }); // 调试日志

      // 直接传递request对象
      const response = await invoke("get_converted_pdf_page", {request: request});

      // 响应处理
      const data = JSON.parse(response);

      if (!data?.page_number || !data?.total_pages) {
        throw new Error("Invalid response structure");
      }

      currentPageData.value = data;
      return data;
    } catch (err) {
      console.error("PDF加载失败:", err);
      error.value = err.message || "加载PDF失败";
      throw err;
    } finally {
      isLoading.value = false;
    }
  };

  // 优化的渲染内容计算
  const renderContent = computed(() => {
    if (!currentPageData.value) return null;

    const { svg, image_data, text_blocks = [] } = currentPageData.value;

    // 自动检测渲染类型
    let type = "image";
    if (svg) type = "vector";
    else if (text_blocks.length > 0) type = "hybrid";

    return {
      type,
      content: type === "vector" ? svg : `data:image/png;base64,${image_data}`,
      textBlocks: text_blocks,
      meta: {
        page: currentPageData.value.page_number,
        total: currentPageData.value.total_pages,
      },
    };
  });

  // 改进的文本层初始化
  const initTextLayer = () => {
    if (!textLayer.value || !renderContent.value?.textBlocks?.length) return;

    const layer = textLayer.value;
    layer.innerHTML = "";

    const container = document.querySelector(".pdf-image-container");
    if (!container) return;

    const { width: containerWidth } = container.getBoundingClientRect();
    const scale = containerWidth / currentPageData.value.page_width;

    renderContent.value.textBlocks.forEach((block, index) => {
      const div = document.createElement("div");
      div.className = "text-block";
      div.dataset.blockId = `block-${index}`;

      Object.assign(div.style, {
        position: "absolute",
        left: `${block.bounds.x * scale}px`,
        top: `${block.bounds.y * scale}px`,
        width: `${block.bounds.w * scale}px`,
        height: `${block.bounds.h * scale}px`,
        fontSize: `${Math.max(10, 12 * scale)}px`,
        lineHeight: "1.2",
        cursor: "text",
        pointerEvents: "auto",
      });

      div.textContent = block.text;
      layer.appendChild(div);
    });
  };

  // 增强的文本选择处理
  const handleTextSelection = (event) => {
    const selection = window.getSelection();
    if (!selection.toString().trim()) return;

    const blockElement = event.target.closest(".text-block");
    const blockId = blockElement?.dataset.blockId;

    return {
      text: selection.toString(),
      blockId,
      page: currentPageData.value?.page_number,
    };
  };

  // 优化的监听逻辑
  watch(
    () => renderContent.value?.type,
    (newType) => {
      if (newType === "hybrid") {
        nextTick(initTextLayer);
      }
    }
  );

  return {
    renderMode,
    loadPage,
    renderContent,
    textLayer,
    isLoading,
    error,
    currentPageData,
  };
}
