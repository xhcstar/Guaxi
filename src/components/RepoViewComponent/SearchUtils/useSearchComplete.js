// useSearchComplete.js - 添加数据格式化函数
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from '../../../utils/logger'

const logger = createLogger('useSearchComplete')
// 添加数据格式化函数
const formatOptions = (data) => {
  if (!Array.isArray(data)) {
    logger.warn('❌ 格式化选项：输入数据不是数组', data);
    return [];
  }

  return data.map(item => {
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
};

export const useSearchComplete = () => {
  const noteTagOptionsComplete = ref([]);
  const isNoteTagLoadingComplete = ref(false);

  // 笔记标签搜索
  const handleNoteTagSearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleNoteTagSearchComplete 被调用，搜索关键字:", query);
    isNoteTagLoadingComplete.value = true;
    try {
      const tags = await invoke("search_notes_tags", { query });
      logger.info("✅ [useSearchComplete] handleNoteTagSearchComplete 返回结果:", tags);
      // 格式化数据
      noteTagOptionsComplete.value = formatOptions(tags);
      logger.info("📝 [useSearchComplete] 格式化后的笔记标签选项:", noteTagOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索笔记标签失败:", error);
    } finally {
      isNoteTagLoadingComplete.value = false;
    }
  };

  // 导入文件标签搜索
  const importedFileTagOptionsComplete = ref([]);
  const isImportedFileTagLoadingComplete = ref(false);

  const handleImportedFileTagSearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleImportedFileTagSearchComplete 被调用，搜索关键字:", query);
    isImportedFileTagLoadingComplete.value = true;
    try {
      const tags = await invoke("search_imported_files_tags", { query });
      logger.info("✅ [useSearchComplete] handleImportedFileTagSearchComplete 返回结果:", tags);
      // 格式化数据
      importedFileTagOptionsComplete.value = formatOptions(tags);
      logger.info("📋 [useSearchComplete] 格式化后的导入文件标签选项:", importedFileTagOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索导入文件标签失败:", error);
    } finally {
      isImportedFileTagLoadingComplete.value = false;
    }
  };

  // 导入文件搜索
  const importedFileOptionsComplete = ref([]);
  const isImportedFileLoadingComplete = ref(false);

  const handleImportedFileSearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleImportedFileSearchComplete 被调用，搜索关键字:", query);
    isImportedFileLoadingComplete.value = true;
    try {
      const files = await invoke("search_imported_files", { query });
      logger.info("✅ [useSearchComplete] handleImportedFileSearchComplete 返回结果:", files);
      // 格式化数据
      importedFileOptionsComplete.value = formatOptions(files);
      logger.info("📁 [useSearchComplete] 格式化后的导入文件选项:", importedFileOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索导入文件失败:", error);
    } finally {
      isImportedFileLoadingComplete.value = false;
    }
  };

  // 所属库搜索
  const libraryOptionsComplete = ref([]);
  const isLibraryLoadingComplete = ref(false);

  const handleLibrarySearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleLibrarySearchComplete 被调用，搜索关键字:", query);
    isLibraryLoadingComplete.value = true;
    try {
      const libraries = await invoke("search_libraries", { query });
      logger.info("✅ [useSearchComplete] handleLibrarySearchComplete 返回结果:", libraries);
      // 格式化数据
      libraryOptionsComplete.value = formatOptions(libraries);
      logger.info("📚 [useSearchComplete] 格式化后的库选项:", libraryOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索所属库失败:", error);
    } finally {
      isLibraryLoadingComplete.value = false;
    }
  };

  // 导入文件所属库搜索
  const importedFileLibraryOptionsComplete = ref([]);
  const isImportedFileLibraryLoadingComplete = ref(false);

  const handleImportedFileLibrarySearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleImportedFileLibrarySearchComplete 被调用，搜索关键字:", query);
    isImportedFileLibraryLoadingComplete.value = true;
    try {
      const libraries = await invoke("search_imported_file_libraries", { query });
      logger.info("✅ [useSearchComplete] handleImportedFileLibrarySearchComplete 返回结果:", libraries);
      // 格式化数据
      importedFileLibraryOptionsComplete.value = formatOptions(libraries);
      logger.info("🏢 [useSearchComplete] 格式化后的导入文件库选项:", importedFileLibraryOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索导入文件所属库失败:", error);
    } finally {
      isImportedFileLibraryLoadingComplete.value = false;
    }
  };

  // 知识库搜索功能
  const knowledgeRepoOptionsComplete = ref([]);
  const isKnowledgeRepoLoadingComplete = ref(false);

  const handleKnowledgeRepoSearchComplete = async (query) => {
    logger.info("🔍 [useSearchComplete] handleKnowledgeRepoSearchComplete 被调用，搜索关键字:", query);
    isKnowledgeRepoLoadingComplete.value = true;
    try {
      const repos = await invoke("get_knowledge_repos", { query });
      logger.info("✅ [useSearchComplete] handleKnowledgeRepoSearchComplete 返回结果:", repos);
      // 格式化数据
      knowledgeRepoOptionsComplete.value = formatOptions(repos);
      logger.info("📖 [useSearchComplete] 格式化后的知识库选项:", knowledgeRepoOptionsComplete.value);
    } catch (error) {
      logger.error("❌ [useSearchComplete] 搜索知识库失败:", error);
    } finally {
      isKnowledgeRepoLoadingComplete.value = false;
    }
  };

  return {
    noteTagOptionsComplete,
    isNoteTagLoadingComplete,
    handleNoteTagSearchComplete,

    importedFileTagOptionsComplete,
    isImportedFileTagLoadingComplete,
    handleImportedFileTagSearchComplete,

    importedFileOptionsComplete,
    isImportedFileLoadingComplete,
    handleImportedFileSearchComplete,

    libraryOptionsComplete,
    isLibraryLoadingComplete,
    handleLibrarySearchComplete,

    importedFileLibraryOptionsComplete,
    isImportedFileLibraryLoadingComplete,
    handleImportedFileLibrarySearchComplete,

    knowledgeRepoOptionsComplete,
    isKnowledgeRepoLoadingComplete,
    handleKnowledgeRepoSearchComplete
  };
};