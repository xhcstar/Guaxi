import { ref, computed } from "vue";

export function useSearch({ store, message }) {
  // 搜索参数
  const searchParams = ref({
    keyword: "",
    library: null,
    tags: [],
    dateRange: null,
    knowledgeRepo: null,
    fileType: null,
  });

  // 搜索面板状态
  const searchPanelCollapsed = ref(true);

  // 文件类型选项
  const fileTypeOptions = [
    { label: "PDF", value: "pdf" },
    { label: "Markdown", value: "md" },
    { label: "HTML", value: "html" },
    { label: "Word", value: "docx" },
    { label: "Text", value: "txt" },
    { label: "EPUB", value: "epub" },
  ];

  // 知识库选项
  const knowledgeRepoOptions = computed(() => {
    return store.repos.map((repo) => ({
      label: repo.name,
      value: repo.name,
    }));
  });

  // 过滤后的数据
  const filteredData = computed(() => {
    let data = store.files;

    // 关键字搜索
    if (searchParams.value.keyword) {
      const keyword = searchParams.value.keyword.toLowerCase();
      data = data.filter(
        (item) =>
          item.importFileName?.toLowerCase().includes(keyword) ||
          item.description?.toLowerCase().includes(keyword) ||
          item.importedFileTags?.toLowerCase().includes(keyword) ||
          item.newNoteForm?.description?.toLowerCase().includes(keyword) ||
          item.newNoteForm?.notesTags?.toLowerCase().includes(keyword)
      );
    }

    // 文件库筛选
    if (searchParams.value.library) {
      data = data.filter((item) => item.library === searchParams.value.library);
    }

    // 标签筛选
    if (searchParams.value.tags && searchParams.value.tags.length > 0) {
      data = data.filter((item) => {
        const itemTags = item.importedFileTags?.split(",") || [];
        return searchParams.value.tags.some((tag) => itemTags.includes(tag));
      });
    }

    // 日期范围筛选
    if (
      searchParams.value.dateRange &&
      searchParams.value.dateRange.length === 2
    ) {
      const [start, end] = searchParams.value.dateRange;
      data = data.filter((item) => {
        const itemDate = new Date(item.importFileCreateTime).getTime();
        return itemDate >= start && itemDate <= end;
      });
    }

    // 知识库筛选
    if (searchParams.value.knowledgeRepo) {
      data = data.filter(
        (item) => item.knowledgeRepo === searchParams.value.knowledgeRepo
      );
    }

    // 文件类型筛选
    if (searchParams.value.fileType) {
      data = data.filter((item) => item.type === searchParams.value.fileType);
    }

    return data;
  });

  // 搜索处理
  const handleSearch = () => {
    message.success("搜索完成，找到 " + filteredData.value.length + " 条结果");
  };

  // 切换搜索面板
  const toggleSearchPanel = () => {
    searchPanelCollapsed.value = !searchPanelCollapsed.value;
  };

  // 库搜索
  const handleLibrarySearch = (query) => {
    console.log("库搜索:", query);
    // 这里可以调用后端搜索接口
  };

  // 标签搜索
  const handleImportedFileTagSearch = (query) => {
    console.log("标签搜索:", query);
    // 这里可以调用后端搜索接口
  };

  // 清除搜索条件
  const clearSearch = () => {
    searchParams.value = {
      keyword: "",
      library: null,
      tags: [],
      dateRange: null,
      knowledgeRepo: null,
      fileType: null,
    };
    message.info("已清除所有搜索条件");
  };

  // 保存搜索条件
  const saveSearchPreset = (name) => {
    const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
    presets.push({
      name: name,
      params: { ...searchParams.value },
      timestamp: new Date().toISOString(),
    });
    localStorage.setItem("searchPresets", JSON.stringify(presets));
    message.success(`搜索条件 "${name}" 已保存`);
  };

  // 加载搜索条件
  const loadSearchPreset = (name) => {
    const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
    const preset = presets.find((p) => p.name === name);
    if (preset) {
      searchParams.value = { ...preset.params };
      message.success(`已加载搜索条件 "${name}"`);
    } else {
      message.warning(`未找到搜索条件 "${name}"`);
    }
  };

  // 获取搜索预设列表
  const getSearchPresets = () => {
    return JSON.parse(localStorage.getItem("searchPresets") || "[]");
  };

  // 删除搜索预设
  const deleteSearchPreset = (name) => {
    const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
    const filteredPresets = presets.filter((p) => p.name !== name);
    localStorage.setItem("searchPresets", JSON.stringify(filteredPresets));
    message.success(`已删除搜索条件 "${name}"`);
  };

  // 高级搜索：组合多个条件
  const advancedSearch = (conditions) => {
    searchParams.value = { ...searchParams.value, ...conditions };
    handleSearch();
  };

  // 搜索历史
  const searchHistory = ref(
    JSON.parse(localStorage.getItem("searchHistory") || "[]")
  );

  // 添加到搜索历史
  const addToSearchHistory = (params) => {
    const history = searchHistory.value;
    const existingIndex = history.findIndex(
      (item) => JSON.stringify(item.params) === JSON.stringify(params)
    );

    if (existingIndex !== -1) {
      history.splice(existingIndex, 1);
    }

    history.unshift({
      params: { ...params },
      timestamp: new Date().toISOString(),
    });

    // 只保留最近10条记录
    if (history.length > 10) {
      history.splice(10);
    }

    searchHistory.value = history;
    localStorage.setItem("searchHistory", JSON.stringify(history));
  };

  // 从搜索历史加载
  const loadFromSearchHistory = (index) => {
    if (index >= 0 && index < searchHistory.value.length) {
      searchParams.value = { ...searchHistory.value[index].params };
      handleSearch();
    }
  };

  return {
    searchParams,
    searchPanelCollapsed,
    fileTypeOptions,
    knowledgeRepoOptions,
    filteredData,
    handleSearch,
    toggleSearchPanel,
    handleLibrarySearch,
    handleImportedFileTagSearch,
    clearSearch,
    saveSearchPreset,
    loadSearchPreset,
    getSearchPresets,
    deleteSearchPreset,
    advancedSearch,
    searchHistory,
    addToSearchHistory,
    loadFromSearchHistory,
  };
}
