import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// 笔记标签搜索
export const useNoteTagsSearch = () => {
  const noteTagOptions = ref([]);
  const isNoteTagLoading = ref(false);

  const handleNoteTagSearch = async (query) => {
    console.log('handleNoteTagSearch 被调用，搜索关键字:', query); // 日志：函数调用和参数
    isNoteTagLoading.value = true;
    try {
      const tags = await invoke('search_notes_tags', { query });
      console.log('handleNoteTagSearch 返回结果:', tags); // 日志：返回结果
      noteTagOptions.value = tags;
    } catch (error) {
      console.error('搜索笔记标签失败:', error); // 日志：错误信息
    } finally {
      isNoteTagLoading.value = false;
    }
  };

  return {
    noteTagOptions,
    isNoteTagLoading,
    handleNoteTagSearch,
  };
};

// 导入文件标签搜索
export const useImportedFileTagsSearch = () => {
  const importedFileTagOptions = ref([]);
  const isImportedFileTagLoading = ref(false);

  const handleImportedFileTagSearch = async (query) => {
    console.log('handleImportedFileTagSearch 被调用，搜索关键字:', query); // 日志：函数调用和参数
    isImportedFileTagLoading.value = true;
    try {
      const tags = await invoke('search_imported_files_tags', { query });
      console.log('handleImportedFileTagSearch 返回结果:', tags); // 日志：返回结果
      importedFileTagOptions.value = tags;
    } catch (error) {
      console.error('搜索导入文件标签失败:', error); // 日志：错误信息
    } finally {
      isImportedFileTagLoading.value = false;
    }
  };

  return {
    importedFileTagOptions,
    isImportedFileTagLoading,
    handleImportedFileTagSearch,
  };
};

// 导入文件搜索
export const useImportedFilesSearch = () => {
  const importedFileOptions = ref([]);
  const isImportedFileLoading = ref(false);

  const handleImportedFileSearch = async (query) => {
    console.log('handleImportedFileSearch 被调用，搜索关键字:', query); // 日志：函数调用和参数
    isImportedFileLoading.value = true;
    try {
      const files = await invoke('search_imported_files', { query });
      console.log('handleImportedFileSearch 返回结果:', files); // 日志：返回结果
      importedFileOptions.value = files;
    } catch (error) {
      console.error('搜索导入文件失败:', error); // 日志：错误信息
    } finally {
      isImportedFileLoading.value = false;
    }
  };

  return {
    importedFileOptions,
    isImportedFileLoading,
    handleImportedFileSearch,
  };
};

// 所属库搜索
export const useLibrariesSearch = () => {
  const libraryOptions = ref([]);
  const isLibraryLoading = ref(false);

  const handleLibrarySearch = async (query) => {
    console.log('handleLibrarySearch 被调用，搜索关键字:', query); // 日志：函数调用和参数
    isLibraryLoading.value = true;
    try {
      const libraries = await invoke('search_libraries', { query });
      console.log('handleLibrarySearch 返回结果:', libraries); // 日志：返回结果
      libraryOptions.value = libraries;
    } catch (error) {
      console.error('搜索所属库失败:', error); // 日志：错误信息
    } finally {
      isLibraryLoading.value = false;
    }
  };

  return {
    libraryOptions,
    isLibraryLoading,
    handleLibrarySearch,
  };
};

// 导入文件所属库搜索
export const useImportedFileLibrarySearch = () => {
  const importedFileLibraryOptions = ref([]);
  const isImportedFileLibraryLoading = ref(false);

  const handleImportedFileLibrarySearch = async (query) => {
    console.log('handleImportedFileLibrarySearch 被调用，搜索关键字:', query); // 日志：函数调用和参数
    isImportedFileLibraryLoading.value = true;
    try {
      const libraries = await invoke('search_imported_file_libraries', { query });
      console.log('handleImportedFileLibrarySearch 返回结果:', libraries); // 日志：返回结果
      importedFileLibraryOptions.value = libraries;
    } catch (error) {
      console.error('搜索导入文件所属库失败:', error); // 日志：错误信息
    } finally {
      isImportedFileLibraryLoading.value = false;
    }
  };

  return {
    importedFileLibraryOptions,
    isImportedFileLibraryLoading,
    handleImportedFileLibrarySearch,
  };
};