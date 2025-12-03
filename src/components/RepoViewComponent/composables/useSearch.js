// useSearch.js
import { ref, computed, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const useSearch = ({ store, message, externalState = {} }) => {
  // 使用外部状态或创建默认状态
  const isLibraryLoading = externalState.isLibraryLoading || ref(false);
  const isImportedFileTagLoading = externalState.isImportedFileTagLoading || ref(false);
  const isKnowledgeRepoLoading = ref(false); // 新增：知识库加载状态

  // 添加库选项和标签选项状态 - 添加默认占位符
  const searchLibraryOptions = ref([
    { label: "输入关键词搜索文件库...", value: "", disabled: true }
  ]);
  const searchImportedFileTagOptions = ref([
    { label: "输入关键词搜索标签...", value: "", disabled: true }
  ]);
  const searchKnowledgeRepoOptions = ref([
    { label: "输入关键词搜索知识库...", value: "", disabled: true }
  ]);

  // 搜索历史状态
  const searchHistory = ref(JSON.parse(localStorage.getItem("searchHistory") || "[]"));

  // 搜索面板状态
  const searchPanelCollapsed = ref(true);

  const searchParams = ref({
    keyword: "",
    fileType: "",
    knowledgeRepo: "",
    library: "",
    importedFileTags: "",
    importFileCreateTime: null,
    importFileSize: null,
  });

  // 文件类型选项 - 确保始终有数据
  const fileTypeOptions = ref([
    { label: "PDF", value: "pdf" },
    { label: "Markdown", value: "md" },
    { label: "HTML", value: "html" },
    { label: "Word", value: "docx" },
    { label: "Text", value: "txt" },
    { label: "EPUB", value: "epub" },
  ]);

  // 知识库选项 - 从 store 获取（静态选项），添加默认占位符
  const knowledgeRepoOptions = computed(() => {
    if (!store.repos || !Array.isArray(store.repos) || store.repos.length === 0) {
      // 如果没有数据，返回默认占位符
      return [{ label: "暂无知识库数据", value: "", disabled: true }];
    }
    return store.repos.map((repo) => ({
      label: repo.name,
      value: repo.name,
    }));
  });

  // 过滤后的数据
  const filteredData = computed(() => {
    let data = store.files || [];

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
    if (searchParams.value.importedFileTags) {
      data = data.filter((item) => {
        const itemTags = item.importedFileTags?.toLowerCase() || "";
        return itemTags.includes(searchParams.value.importedFileTags.toLowerCase());
      });
    }

    // 日期范围筛选
    if (searchParams.value.importFileCreateTime && searchParams.value.importFileCreateTime.length === 2) {
      const [start, end] = searchParams.value.importFileCreateTime;
      data = data.filter((item) => {
        const itemDate = new Date(item.importFileCreateTime).getTime();
        return itemDate >= start && itemDate <= end;
      });
    }

    // 知识库筛选
    if (searchParams.value.knowledgeRepo) {
      data = data.filter((item) => item.knowledgeRepo === searchParams.value.knowledgeRepo);
    }

    // 文件类型筛选
    if (searchParams.value.fileType) {
      data = data.filter((item) => item.type === searchParams.value.fileType);
    }

    return data;
  });

  // 搜索处理
  const handleSearch = () => {
    addToSearchHistory({ ...searchParams.value });
    message.success("搜索完成，找到 " + filteredData.value.length + " 条结果");
  };

  // 切换搜索面板
  const toggleSearchPanel = () => {
    searchPanelCollapsed.value = !searchPanelCollapsed.value;

    // 展开时预加载数据
    if (!searchPanelCollapsed.value) {
      preloadSearchOptions();
    }
  };

  // 预加载搜索选项 - 优化预加载逻辑
  const preloadSearchOptions = async () => {
    console.log("🔄 预加载搜索选项");

    try {
      // 设置加载中的占位符
      if (searchLibraryOptions.value.length === 0 ||
        (searchLibraryOptions.value.length === 1 && searchLibraryOptions.value[0].disabled)) {
        searchLibraryOptions.value = [{ label: "加载文件库中...", value: "", disabled: true }];
      }

      if (searchImportedFileTagOptions.value.length === 0 ||
        (searchImportedFileTagOptions.value.length === 1 && searchImportedFileTagOptions.value[0].disabled)) {
        searchImportedFileTagOptions.value = [{ label: "加载标签中...", value: "", disabled: true }];
      }

      if (searchKnowledgeRepoOptions.value.length === 0 ||
        (searchKnowledgeRepoOptions.value.length === 1 && searchKnowledgeRepoOptions.value[0].disabled)) {
        searchKnowledgeRepoOptions.value = [{ label: "加载知识库中...", value: "", disabled: true }];
      }

      // 异步加载实际数据
      await Promise.all([
        handleLibrarySearch("", true),
        handleImportedFileTagSearch("", true),
        handleKnowledgeRepoSearch("", true)
      ]);

      console.log("✅ 预加载搜索选项完成");
    } catch (error) {
      console.error("❌ 预加载搜索选项失败:", error);
    }
  };

  // 防抖计时器
  let librarySearchTimer = null;
  let tagSearchTimer = null;
  let knowledgeRepoSearchTimer = null;

  // 格式化搜索选项 - 优化格式化逻辑
  const formatSearchOptions = (data, type) => {
    if (!Array.isArray(data)) {
      console.warn(`❌ ${type}数据格式错误:`, data);
      return [{ label: `暂无${type}数据`, value: "", disabled: true }];
    }

    if (data.length === 0) {
      return [{ label: `未找到${type}数据`, value: "", disabled: true }];
    }

    const options = data.map(item => {
      if (typeof item === 'string') {
        return { label: item, value: item };
      } else if (item && typeof item === 'object') {
        return {
          label: item.label || item.name || item.value || item.title || String(item),
          value: item.value || item.id || item.label || item.name || String(item)
        };
      } else {
        return { label: String(item), value: String(item) };
      }
    });

    console.log(`📋 ${type}格式化结果:`, options);
    return options;
  };

  // 库搜索
  const handleLibrarySearch = async (query = "", isPreload = false) => {
    console.log("📚 文件库搜索被调用:", { query, isPreload });

    if (librarySearchTimer) {
      clearTimeout(librarySearchTimer);
    }

    return new Promise((resolve) => {
      librarySearchTimer = setTimeout(async () => {
        isLibraryLoading.value = true;

        try {
          let searchQuery = query;
          if (isPreload && !query) {
            searchQuery = "";
          }

          const libraries = await invoke("search_libraries", { query: searchQuery });
          console.log("✅ 文件库搜索结果:", libraries);

          searchLibraryOptions.value = formatSearchOptions(libraries, "文件库");
          console.log("📋 格式化后的文件库选项:", searchLibraryOptions.value);

          resolve(searchLibraryOptions.value);
        } catch (error) {
          console.error("❌ 搜索文件库失败:", error);
          searchLibraryOptions.value = [{ label: "搜索失败，请重试", value: "", disabled: true }];
          resolve(searchLibraryOptions.value);
        } finally {
          isLibraryLoading.value = false;
        }
      }, isPreload ? 0 : 300);
    });
  };

  // 标签搜索
  const handleImportedFileTagSearch = async (query = "", isPreload = false) => {
    console.log("🏷️ 标签搜索被调用:", { query, isPreload });

    if (tagSearchTimer) {
      clearTimeout(tagSearchTimer);
    }

    return new Promise((resolve) => {
      tagSearchTimer = setTimeout(async () => {
        isImportedFileTagLoading.value = true;

        try {
          let searchQuery = query;
          if (isPreload && !query) {
            searchQuery = "";
          }

          const tags = await invoke("search_imported_files_tags", { query: searchQuery });
          console.log("✅ 标签搜索结果:", tags);

          searchImportedFileTagOptions.value = formatSearchOptions(tags, "标签");
          console.log("📋 格式化后的标签选项:", searchImportedFileTagOptions.value);

          resolve(searchImportedFileTagOptions.value);
        } catch (error) {
          console.error("❌ 搜索标签失败:", error);
          searchImportedFileTagOptions.value = [{ label: "搜索失败，请重试", value: "", disabled: true }];
          resolve(searchImportedFileTagOptions.value);
        } finally {
          isImportedFileTagLoading.value = false;
        }
      }, isPreload ? 0 : 300);
    });
  };

  // 知识库搜索功能
  const handleKnowledgeRepoSearch = async (query = "", isPreload = false) => {
    console.log("📖 知识库搜索被调用:", { query, isPreload });

    if (knowledgeRepoSearchTimer) {
      clearTimeout(knowledgeRepoSearchTimer);
    }

    return new Promise((resolve) => {
      knowledgeRepoSearchTimer = setTimeout(async () => {
        isKnowledgeRepoLoading.value = true;

        try {
          let searchQuery = query;
          if (isPreload && !query) {
            searchQuery = "";
          }

          // 调用后端接口搜索知识库
          const repos = await invoke("get_knowledge_repos", { query: searchQuery });
          console.log("✅ 知识库搜索结果:", repos);

          // 格式化数据
          searchKnowledgeRepoOptions.value = formatSearchOptions(repos, "知识库");
          console.log("📖 格式化后的知识库选项:", searchKnowledgeRepoOptions.value);

          resolve(searchKnowledgeRepoOptions.value);
        } catch (error) {
          console.error("❌ 搜索知识库失败:", error);
          searchKnowledgeRepoOptions.value = [{ label: "搜索失败，请重试", value: "", disabled: true }];
          resolve(searchKnowledgeRepoOptions.value);
        } finally {
          isKnowledgeRepoLoading.value = false;
        }
      }, isPreload ? 0 : 300);
    });
  };

  // 获取知识库选项（合并静态和动态搜索选项）
  const getKnowledgeRepoOptions = computed(() => {
    // 如果搜索选项有有效数据，使用搜索选项
    if (searchKnowledgeRepoOptions.value.length > 0 &&
      !(searchKnowledgeRepoOptions.value.length === 1 && searchKnowledgeRepoOptions.value[0].disabled)) {
      return searchKnowledgeRepoOptions.value;
    }
    // 否则使用静态选项
    return knowledgeRepoOptions.value;
  });

  // 获取文件库选项（确保始终有占位符）
  const getLibraryOptions = computed(() => {
    if (searchLibraryOptions.value.length === 0 ||
      (searchLibraryOptions.value.length === 1 && searchLibraryOptions.value[0].disabled)) {
      return [{ label: "暂无文件库数据", value: "", disabled: true }];
    }
    return searchLibraryOptions.value;
  });

  // 获取标签选项（确保始终有占位符）
  const getImportedFileTagOptions = computed(() => {
    if (searchImportedFileTagOptions.value.length === 0 ||
      (searchImportedFileTagOptions.value.length === 1 && searchImportedFileTagOptions.value[0].disabled)) {
      return [{ label: "暂无标签数据", value: "", disabled: true }];
    }
    return searchImportedFileTagOptions.value;
  });

  // 清除搜索条件
  const clearSearch = () => {
    searchParams.value = {
      keyword: "",
      fileType: "",
      knowledgeRepo: "",
      library: "",
      importedFileTags: "",
      importFileCreateTime: null,
      importFileSize: null,
    };
    message.info("已清除所有搜索条件");
  };

  // 保存搜索条件
  const saveSearchPreset = (name) => {
    try {
      const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
      const existingIndex = presets.findIndex(p => p.name === name);

      if (existingIndex !== -1) {
        presets[existingIndex] = {
          name: name,
          params: { ...searchParams.value },
          timestamp: new Date().toISOString(),
        };
      } else {
        presets.push({
          name: name,
          params: { ...searchParams.value },
          timestamp: new Date().toISOString(),
        });
      }

      localStorage.setItem("searchPresets", JSON.stringify(presets));
      message.success(`搜索条件 "${name}" 已保存`);
    } catch (error) {
      console.error("保存搜索预设失败:", error);
      message.error("保存搜索预设失败");
    }
  };

  // 加载搜索条件
  const loadSearchPreset = (name) => {
    try {
      const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
      const preset = presets.find((p) => p.name === name);
      if (preset) {
        searchParams.value = { ...preset.params };
        message.success(`已加载搜索条件 "${name}"`);

        nextTick(() => {
          handleSearch();
        });
      } else {
        message.warning(`未找到搜索条件 "${name}"`);
      }
    } catch (error) {
      console.error("加载搜索预设失败:", error);
      message.error("加载搜索预设失败");
    }
  };

  // 获取搜索预设列表
  const getSearchPresets = () => {
    try {
      return JSON.parse(localStorage.getItem("searchPresets") || "[]");
    } catch (error) {
      console.error("获取搜索预设列表失败:", error);
      return [];
    }
  };

  // 删除搜索预设
  const deleteSearchPreset = (name) => {
    try {
      const presets = JSON.parse(localStorage.getItem("searchPresets") || "[]");
      const filteredPresets = presets.filter((p) => p.name !== name);
      localStorage.setItem("searchPresets", JSON.stringify(filteredPresets));
      message.success(`已删除搜索条件 "${name}"`);
    } catch (error) {
      console.error("删除搜索预设失败:", error);
      message.error("删除搜索预设失败");
    }
  };

  // 添加到搜索历史
  const addToSearchHistory = (params) => {
    try {
      const history = [...searchHistory.value];

      const isEmptySearch = !params.keyword && !params.fileType && !params.knowledgeRepo &&
        !params.library && !params.importedFileTags;

      if (isEmptySearch) {
        return;
      }

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

      if (history.length > 10) {
        history.splice(10);
      }

      searchHistory.value = history;
      localStorage.setItem("searchHistory", JSON.stringify(history));
    } catch (error) {
      console.error("添加到搜索历史失败:", error);
    }
  };

  // 从搜索历史加载
  const loadFromSearchHistory = (index) => {
    if (index >= 0 && index < searchHistory.value.length) {
      searchParams.value = { ...searchHistory.value[index].params };
      handleSearch();
    }
  };

  // 清空搜索历史
  const clearSearchHistory = () => {
    searchHistory.value = [];
    localStorage.setItem("searchHistory", "[]");
    message.info("已清空搜索历史");
  };

  // 初始化搜索功能
  const initializeSearch = async () => {
    console.log("🚀 初始化搜索功能");
    await preloadSearchOptions();
  };

  return {
    // 状态
    searchParams,
    searchPanelCollapsed,
    searchLibraryOptions,
    searchImportedFileTagOptions,
    searchKnowledgeRepoOptions,
    isLibraryLoading,
    isImportedFileTagLoading,
    isKnowledgeRepoLoading,
    searchHistory,

    // 选项数据 - 使用计算属性确保始终有占位符
    fileTypeOptions,
    knowledgeRepoOptions: getKnowledgeRepoOptions,
    libraryOptions: getLibraryOptions,
    importedFileTagOptions: getImportedFileTagOptions,
    filteredData,

    // 搜索方法
    handleSearch,
    toggleSearchPanel,
    handleLibrarySearch,
    handleImportedFileTagSearch,
    handleKnowledgeRepoSearch,

    // 工具方法
    clearSearch,
    saveSearchPreset,
    loadSearchPreset,
    getSearchPresets,
    deleteSearchPreset,
    addToSearchHistory,
    loadFromSearchHistory,
    clearSearchHistory,

    // 初始化函数
    initializeSearch,
  };
};