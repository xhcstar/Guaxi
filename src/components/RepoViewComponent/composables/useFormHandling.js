// composables/useFormHandling.js
import { ref, computed } from "vue";
import { platform } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export function useFormHandling({
  store,
  message,
  startProgressListener,
  convertFile,
}) {
  // 模态框状态
  const isAddFileModalVisible = ref(false);

  // 表单数据
  const currentForm = ref({
    id: 0,
    filePath: "",
    importFileName: "",
    description: "",
    importedFileTags: "",
    type: "",
    library: "",
    importFileCreateTime: "",
    importFileSize: 0,
    convetToMarkdownFilename: "",
    knowledgeRepo: "",
    newNoteForm: {
      id: 0,
      description: "",
      noteFileName: "",
      notesTags: "",
      fromImportedFile: "",
      fromImportedFileLibrary: "",
      noteFileType: "",
      noteFileCreateTime: "",
      noteFileUpdateTime: "",
      knowledgeRepo: "",
    },
  });

  // 校验文件路径函数
  const validateFilePath = async (rule, value) => {
    console.log("validateFilePath", value);

    if (!value || value.trim() === "") {
      return new Error("文件路径不能为空");
    }

    return new Promise(async (resolve, reject) => {
      try {
        const osPlatform = await platform();

        const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
        const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
        const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

        console.log("osPlatform", osPlatform);
        if (osPlatform === "windows") {
          if (!windowsPathRegex.test(value)) {
            resolve(new Error("Windows 文件路径不合法，请检查路径格式"));
            return;
          }
        } else if (osPlatform === "linux") {
          if (!linuxPathRegex.test(value)) {
            resolve(new Error("Linux 文件路径不合法，请检查路径格式"));
            return;
          }
        } else if (osPlatform === "macos") {
          if (!macPathRegex.test(value)) {
            resolve(new Error("macOS 文件路径不合法，请检查路径格式"));
            return;
          }
        } else {
          resolve(new Error("不支持的操作系统"));
          return;
        }

        resolve(true);
      } catch (error) {
        resolve(new Error("校验文件路径失败: " + error.message));
      }
    });
  };

  // 校验标签函数
  const validateTags = (rule, value) => {
    console.log("validateTags", value);

    if (!value || value.trim() === "") {
      return new Error("标签不能为空");
    }

    const tags = value.split(",").map((tag) => tag.trim());
    if (tags.some((tag) => !tag)) {
      return new Error("标签不能为空或以多个逗号分隔");
    }

    return true;
  };

  // 表单校验规则
  const rules = {
    filePath: [
      {
        validator: validateFilePath,
        required: true,
        message: "文件路径不能为空",
        trigger: ["input", "blur"],
      },
    ],
    importedFileTags: [
      {
        validator: validateTags,
        required: true,
        message: "标签不能为空",
        trigger: ["input", "blur"],
      },
    ],
    description: [
      { required: true, message: "描述不能为空", trigger: ["input", "blur"] },
      {
        max: 240,
        message: "描述不能超过240个字符",
        trigger: ["input", "blur"],
      },
    ],
    library: [
      { required: true, message: "所属库不能为空", trigger: ["input", "blur"] },
    ],
    importFileCreateTime: [
      {
        required: true,
        message: "入库时间不能为空",
        trigger: ["input", "blur"],
      },
    ],
    importFileSize: [
      {
        required: true,
        message: "文件大小不能为空",
        trigger: ["input", "blur"],
      },
    ],
    knowledgeRepo: [
      { required: true, message: "知识库不能为空", trigger: ["input", "blur"] },
    ],
  };

  // 动态生成表单列配置
  const formColumns = computed(() => {
    const columns = [
      { key: "filePath", title: "文件路径" },
      { key: "importFileName", title: "文件名" },
      { key: "description", title: "文件描述" },
      { key: "knowledgeRepo", title: "知识库" },
      { key: "library", title: "文件库" },
      { key: "importedFileTags", title: "文件标签" },
      { key: "type", title: "文件类型" },
      { key: "importFileCreateTime", title: "入库时间" },
      { key: "importFileSize", title: "文件大小" },
    ];
    return columns;
  });

  // 库选项和加载状态
  const libraryOptions = ref([]);
  const isLibraryLoading = ref(false);

  // 标签选项和加载状态
  const importedFileTagOptions = ref([]);
  const isImportedFileTagLoading = ref(false);

  // 显示添加文件对话框
  const showAddFileModal = () => {
    isAddFileModalVisible.value = true;
  };

  // 打开文件浏览器
  const openFileBrowser = async () => {
    try {
      const filePath = await open({
        filters: [{ name: "All Files", extensions: ["*"] }],
      });
      if (filePath) {
        currentForm.value.filePath = filePath;

        // 调用后台接口获取文件信息
        const fileInfo = await invoke("get_file_info", { filePath });
        console.log("File Info:", fileInfo);

        // 根据当前数据库类型更新表单数据
        currentForm.value.importFileName = fileInfo.file_name;
        currentForm.value.type = fileInfo.file_type;
        currentForm.value.importFileCreateTime = fileInfo.create_time;
        currentForm.value.importFileSize = fileInfo.file_size;
      }
    } catch (error) {
      console.error("Error opening file browser:", error);
    }
  };

  // 校验表单
  const validateForm = async () => {
    const form = currentForm.value;
    console.log(">>>>>>>>>>>> validateForm", currentForm.value);

    if (!form.filePath || form.filePath.trim() === "") {
      return "文件路径不能为空";
    }

    const osPlatform = await platform();

    const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
    const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

    console.log("osPlatform", osPlatform);
    if (osPlatform === "windows") {
      if (!windowsPathRegex.test(form.filePath)) {
        return "Windows 文件路径不合法，请检查路径格式";
      }
    } else if (osPlatform === "linux") {
      if (!linuxPathRegex.test(form.filePath)) {
        return "Linux 文件路径不合法，请检查路径格式";
      }
    } else if (osPlatform === "macos") {
      if (!macPathRegex.test(form.filePath)) {
        return "macOS 文件路径不合法，请检查路径格式";
      }
    } else {
      return "不支持的操作系统";
    }

    if (!form.importedFileTags || form.importedFileTags.trim() === "") {
      console.log(">>>>>>>>>>>>> form", form.importedFileTags);
      console.log(">>>>>>>>>>>>> form.importedFileTags", form.importedFileTags);
      currentForm.value.importedFileTags = "default";
    } else {
      const tags = form.importedFileTags.split(",").map((tag) => tag.trim());
      if (tags.some((tag) => !tag)) {
        return "多个标签没有以逗号分隔";
      }
    }

    if (!form.description || form.description.trim() === "") {
      return "描述不能为空";
    }
    if (form.description.length > 240) {
      return "描述不能超过 240 个字符";
    }

    if (
      !form.library ||
      (typeof form.library === "string" && form.library.trim() === "")
    ) {
      currentForm.value.library = "default";
      return "所属库不能为空";
    }

    if (!form.importFileCreateTime || form.importFileCreateTime.trim() === "") {
      return "入库时间不能为空";
    }

    if (!form.importFileSize || form.importFileSize <= 0) {
      return "文件大小不能为空";
    }

    return null;
  };

  // 添加文件
  const addFile = async () => {
    try {
      const validationError = await validateForm();
      if (validationError) {
        message.error(validationError);
        return;
      }

      if (!currentForm.value.library || !currentForm.value.importFileName) {
        message.error(
          "Library or importFileName path is missing: ",
          currentForm.value
        );
        return;
      }

      // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
      const addResultInfo = await invoke("add_repo_import_file", {
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      }).catch((error) => {
        let errorMessage = "Failed to import file";
        if (error.includes("Unsupported file type")) {
          errorMessage = "Unsupported file type";
        }
        message.error(errorMessage);
        throw new Error(errorMessage);
      });

      // 创建 importfileList
      const importfileList = createImportFileList(
        currentForm.value
      );

      // 将 importfileList 写入数据库
      console.log("importfileList:", importfileList);
      const fileId = await addFileToDatabase(importfileList);

      // 添加导入的文件到 store
      store.addImportedFile({
        id: fileId,
        importFileName: currentForm.value.importFileName,
        description: currentForm.value.description,
        importedFileTags: currentForm.value.importedFileTags,
        type: currentForm.value.type,
        library: currentForm.value.library,
        importFileCreateTime: currentForm.value.importFileCreateTime,
        importFileSize: currentForm.value.importFileSize,
        knowledgeRepo: currentForm.value.knowledgeRepo,
      });

      // 转换完成后刷新数据
      console.log("add_file currentDatabase", store.currentDatabase);
      await store.fetchRepos();
      await store.fetchFiles(store.currentDatabase);

      try {
        // 构建输出目录路径
        const { outputDir, fileToCheck } = await buildConvertOutputFilePath(
          currentForm.value.importFileName,
          currentForm.value.library
        );

        // 检查文件是否已转换
        const isConverted = await invoke("check_file_converted", {
          outputDir: fileToCheck,
        });

        console.log("isConverted", isConverted);
        console.log("addResultInfo", addResultInfo);
        console.log("currentForm: ", currentForm.value);

        if (isConverted || addResultInfo.notConverted) {
          console.log("isConverted", isConverted);
          console.log(
            "convetToMarkdownFilename: ",
            currentForm.value.convetToMarkdownFilename
          );
          message.success("已存在转换文件，无需转换");
          isAddFileModalVisible.value = false;
          resetForm();
          return;
        }

        // 开始监听进度事件
        const unlisten = await startProgressListener(
          currentForm.value.filePath
        );

        // 立即关闭表单对话框
        isAddFileModalVisible.value = false;

        // 开始转换流程
        await convertFile(currentForm.value.filePath, outputDir);

        // 转换完成后刷新数据
        console.log("add_file currentDatabase", store.currentDatabase);
        await store.fetchFiles(store.currentDatabase);
        resetForm();

        // 转换完成后取消监听
        unlisten();
      } catch (error) {
        // 更新进度条状态为失败
        const fileName =
          currentForm.value.filePath.split(/[\\/]/).pop() || "未知文件";
        const progressIndex = progresses.value.findIndex(
          (p) => p.fileName === fileName
        );
        if (progressIndex !== -1) {
          progresses.value[progressIndex] = {
            ...progresses.value[progressIndex],
            message: `转换失败: ${error.message || error}`,
            current: 100,
          };
          setTimeout(() => {
            progresses.value = progresses.value.filter(
              (_, i) => i !== progressIndex
            );
          }, 3000);
        }
        message.error(`添加文件失败: ${error}`);
      }

      isAddFileModalVisible.value = false;
      resetForm();
    } catch (error) {
      console.error("Error adding file:", error);
    }
  };

  const addFileFromWeb = async () => {
    try {
      // 校验表单
      const validationError = await validateForm(); // 等待 validateForm 的结果
      if (validationError) {
        message.error(validationError); // 确保 validationError 是字符串
        return; // 终止添加
      }

      if (!currentForm.value.library || !currentForm.value.importFileName) {
        message.error(
          "Library or importFileName path is missing: ",
          currentForm.value
        );
        return;
      }

      // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
      const addResultInfo = await invoke("add_web_clipper_import_file", {
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      });

      // 创建 importfileList
      const importfileList = createImportFileList(
        currentForm.value
      );

      // 将 importfileList 写入数据库
      console.log("importfileList:", importfileList);
      const fileId = await addFileToDatabase(importfileList);

      // 添加导入的文件到 store
      store.addImportedFile({
        id: fileId,
        importFileName: currentForm.value.importFileName,
        description: currentForm.value.description,
        importedFileTags: currentForm.value.importedFileTags,
        type: currentForm.value.type,
        library: currentForm.value.library,
        importFileCreateTime: currentForm.value.importFileCreateTime,
        importFileSize: currentForm.value.importFileSize,
        knowledgeRepo: currentForm.value.knowledgeRepo,
      });

      // 转换完成后刷新数据
      console.log("add_file currentDatabase", store.currentDatabase);
      await store.fetchRepos();
      await store.fetchFiles(store.currentDatabase);

      // 关闭对话框并清空表单
      isAddFileModalVisible.value = false;
      resetForm();
    } catch (error) {
      console.error("Error adding file:", error);
    }
  };

  const handleFileInfoEvent = async (payload) => {
    if (!payload) {
      console.error("事件 payload 为空");
      return;
    }

    const fileInfo = payload;
    console.log("handleFileInfoEvent", payload);

    // 构建表单数据
    currentForm.value = {
      id: 0,
      filePath: fileInfo.file_path,
      importFileName: fileInfo.file_name,
      description: fileInfo.description,
      importedFileTags: fileInfo.tags.join(","),
      type: fileInfo.file_type,
      library: fileInfo.file_repo,
      importFileCreateTime: fileInfo.create_time,
      importFileSize: fileInfo.file_size,
      knowledgeRepo: fileInfo.knowledge_repo,
    };

    // 调用 addFile 方法
    try {
      await addFileFromWeb();
      message.success("文件添加成功");
    } catch (error) {
      console.error("添加文件失败:", error);
      message.error(`添加文件失败: ${error.message || error}`);
    }
  };

  // 清空表单
  const resetForm = () => {
    currentForm.value = {
      id: 0,
      importFileName: "",
      description: "",
      importedFileTags: "",
      type: "",
      library: "",
      importFileCreateTime: "",
      importFileSize: 0,
      convetToMarkdownFilename: "",
      knowledgeRepo: "",
    };
  };

  // 创建导入文件列表
  const createImportFileList = (currentForm) => {
    return {
      import_file_name: currentForm.importFileName,
      description: currentForm.description,
      library: currentForm.library,
      tags: currentForm.importedFileTags.split(","),
      file_type: currentForm.type,
      import_file_create_time: currentForm.importFileCreateTime,
      import_file_size: currentForm.importFileSize,
      knowledge_repo: currentForm.knowledgeRepo,
    };
  };

  // 添加文件到数据库
  const addFileToDatabase = async (fileInfo) => {
    try {
      console.log("Adding info into table imported_files database:", fileInfo);
      const fileId = await invoke("add_imported_file", {
        file: fileInfo,
      });
      console.log("File info successfully added to database, id: ", fileId);
      return fileId;
    } catch (error) {
      console.error(
        "Error adding info into table imported_files database:",
        error
      );
      throw error;
    }
  };

  // 构建转换输出文件路径
  const buildConvertOutputFilePath = async (fileName, library) => {
    const fileNameWithoutExt = fileName.replace(/\.[^/.]+$/, "");
    const osPlatform = await platform();

    const basePath =
      osPlatform === "windows"
        ? `${store.librariesRoot}\\Libraries\\${library}\\${fileNameWithoutExt}\\markdown`
        : `${store.librariesRoot}/Libraries/${library}/${fileNameWithoutExt}/markdown`;

    const fullPath =
      osPlatform === "windows"
        ? `${basePath}\\${fileNameWithoutExt}\\${fileNameWithoutExt}.md`
        : `${basePath}/${fileNameWithoutExt}/${fileNameWithoutExt}.md`;

    console.log("basePath:", basePath);
    console.log("fullPath:", fullPath);

    return {
      outputDir: basePath,
      fileToCheck: fullPath,
    };
  };

  // 修复缺失的方法
  const handleFormUpdate = (newForm) => {
    currentForm.value = newForm;
  };

  const handleModalCancel = () => {
    isAddFileModalVisible.value = false;
    resetForm();
    message.info("已取消添加文件");
  };

  // 返回所有需要的函数和状态
  return {
    // 状态
    isAddFileModalVisible,
    currentForm,
    rules,
    formColumns,
    libraryOptions,
    importedFileTagOptions,
    isLibraryLoading,
    isImportedFileTagLoading,

    // 方法
    showAddFileModal,
    openFileBrowser,
    addFile,
    handleFileInfoEvent,
    resetForm,
    validateForm,
    validateFilePath,
    validateTags,
    createImportFileList,
    addFileToDatabase,
    handleFormUpdate,
    handleModalCancel
  };
}
