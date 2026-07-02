// composables/useFormHandling.js
import { ref, computed } from "vue";
import { platform } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { createLogger } from '../../../utils/logger'

const logger = createLogger('useFormHandling')
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
    convertEngine: "auto",
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
    logger.info("validateFilePath", value);

    if (!value || value.trim() === "") {
      return new Error("文件路径不能为空");
    }

    return new Promise(async (resolve, reject) => {
      try {
        const osPlatform = await platform();

        const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
        const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
        const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

        logger.info("osPlatform", osPlatform);
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
    logger.info("validateTags", value);

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
        logger.info("File Info:", fileInfo);

        // 根据当前数据库类型更新表单数据
        currentForm.value.importFileName = fileInfo.file_name;
        currentForm.value.type = fileInfo.file_type;
        currentForm.value.importFileCreateTime = fileInfo.create_time;
        currentForm.value.importFileSize = fileInfo.file_size;
      }
    } catch (error) {
      logger.error("Error opening file browser:", error);
    }
  };

  // 校验表单
  const validateForm = async () => {
    const form = currentForm.value;
    logger.info(">>>>>>>>>>>> validateForm", currentForm.value);

    if (!form.filePath || form.filePath.trim() === "") {
      return "文件路径不能为空";
    }

    const osPlatform = await platform();

    const windowsPathRegex = /^([a-zA-Z]:\\|\\\\)([^\\/:*?"<>|\r\n]+\\?)*$/;
    const linuxPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;
    const macPathRegex = /^\/([^\/:*?"<>|\r\n]+\/?)*$/;

    logger.info("osPlatform", osPlatform);
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
      logger.info(">>>>>>>>>>>>> form", form.importedFileTags);
      logger.info(">>>>>>>>>>>>> form.importedFileTags", form.importedFileTags);
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
    const flowId = `${Date.now()}-${Math.random().toString(16).slice(2)}`;

    logger.info("[import] addFile start", {
      flowId,
      filePath: currentForm.value.filePath,
      importFileName: currentForm.value.importFileName,
      library: currentForm.value.library,
      knowledgeRepo: currentForm.value.knowledgeRepo,
    });

    try {
      const validationError = await validateForm();
      if (validationError) {
        logger.warn("[import] validateForm failed", { flowId, validationError });
        message.error(validationError);
        return;
      }

      logger.info("[import] validateForm ok", { flowId });

      if (!currentForm.value.library || !currentForm.value.importFileName) {
        message.error(
          "Library or importFileName path is missing: ",
          currentForm.value
        );
        return;
      }

      // 调用 Tauri 后端接口，将文件拷贝到 Libraries 目录下
      logger.info("[import] invoke add_repo_import_file", {
        flowId,
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      });

      const addResultInfo = await invoke("add_repo_import_file", {
        library: currentForm.value.library,
        filePath: currentForm.value.filePath,
      }).catch((error) => {
        logger.error("[import] add_repo_import_file failed", { flowId, error });
        let errorMessage = "Failed to import file";
        if (error.includes("Unsupported file type")) {
          errorMessage = "Unsupported file type";
        }
        message.error(errorMessage);
        throw new Error(errorMessage);
      });

      logger.info("[import] add_repo_import_file ok", { flowId, addResultInfo });

      // 创建 importfileList
      const importfileList = createImportFileList(
        currentForm.value
      );

      // 将 importfileList 写入数据库
      logger.info("importfileList:", importfileList);
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
      logger.info("add_file currentDatabase", store.currentDatabase);
      await store.fetchRepos();
      await store.fetchFiles(store.currentDatabase);

      let unlisten = null;

      try {
        // 构建输出目录路径
        const { outputDir, fileToCheck } = await buildConvertOutputFilePath(
          currentForm.value.importFileName,
          currentForm.value.library
        );

        logger.info("[import] buildConvertOutputFilePath", {
          flowId,
          outputDir,
          fileToCheck,
        });

        // 检查文件是否已转换
        logger.info("[import] invoke check_file_converted", {
          flowId,
          outputDir: fileToCheck,
        });

        const isConverted = await invoke("check_file_converted", {
          outputDir: fileToCheck,
        });

        logger.info("[import] check_file_converted result", {
          flowId,
          isConverted,
        });

        logger.info("isConverted", isConverted);
        logger.info("addResultInfo", addResultInfo);
        logger.info("currentForm: ", currentForm.value);

        if (isConverted || addResultInfo.notConverted) {
          logger.info("isConverted", isConverted);
          logger.info(
            "convetToMarkdownFilename: ",
            currentForm.value.convetToMarkdownFilename
          );
          message.success("已存在转换文件，无需转换");
          isAddFileModalVisible.value = false;
          resetForm();
          return;
        }

        // 开始监听进度事件
        logger.info("[import] startProgressListener", {
          flowId,
          filePath: currentForm.value.filePath,
        });

        unlisten = await startProgressListener(
          currentForm.value.filePath
        );

        // 立即关闭表单对话框
        isAddFileModalVisible.value = false;

        // 入队转换任务（异步）。进度由持久化队列驱动，
        // startProgressListener 会在任务终态时自动清理监听。
        const selectedEngine = currentForm.value.convertEngine || "auto";
        logger.info("[import] convertFile(enqueue) start", {
          flowId,
          filePath: currentForm.value.filePath,
          outputDir,
          engine: selectedEngine,
        });

        await convertFile(currentForm.value.filePath, outputDir, selectedEngine);

        logger.info("[import] convertFile(enqueue) ok", { flowId });

        // 入队成功即视为流程完成；实际 markdown 就绪后由进度监听刷新文件表。
        resetForm();

        // 注意：不在此处 unlisten，交由 startProgressListener 在任务终态时自清理。
        unlisten = null;
      } catch (error) {
        logger.error("[import] conversion flow failed", { flowId, error });

        if (unlisten) {
          unlisten();
          unlisten = null;
        }

        message.error(`添加文件失败: ${error}`);
      }

      isAddFileModalVisible.value = false;
      resetForm();
    } catch (error) {
      logger.error("Error adding file:", error);
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
      logger.info("importfileList:", importfileList);
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
      logger.info("add_file currentDatabase", store.currentDatabase);
      await store.fetchRepos();
      await store.fetchFiles(store.currentDatabase);

      // 关闭对话框并清空表单
      isAddFileModalVisible.value = false;
      resetForm();
    } catch (error) {
      logger.error("Error adding file:", error);
    }
  };

  const handleFileInfoEvent = async (payload) => {
    if (!payload) {
      logger.error("事件 payload 为空");
      return;
    }

    const fileInfo = payload;
    logger.info("handleFileInfoEvent", payload);
    if (!fileInfo || !fileInfo.file_path) {
      logger.error("fileInfo 或 file_path 为空:", fileInfo)
      message.error("Web Clipper 传来的文件路径为空，无法导入")
      return
    }

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
      logger.error("添加文件失败:", error);
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
      convertEngine: "auto",
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
      logger.info("Adding info into table imported_files database:", fileInfo);
      const fileId = await invoke("add_imported_file", {
        file: fileInfo,
      });
      logger.info("File info successfully added to database, id: ", fileId);
      return fileId;
    } catch (error) {
      logger.error(
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

    logger.info("basePath:", basePath);
    logger.info("fullPath:", fullPath);

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
