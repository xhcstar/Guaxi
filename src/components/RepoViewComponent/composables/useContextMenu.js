import { ref, nextTick, h } from "vue";
import { NIcon } from "naive-ui";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { useSettingsStore } from "../../../stores/settingsStore";
import { createLogger } from '../../../utils/logger'
import { useConfirm } from './useConfirm.js'
import {
  Open20Regular,
  FolderOpen20Regular,
  Notepad20Regular,
  ArrowSync20Regular,
  Edit20Regular,
  Share20Regular,
  Delete20Regular,
  DeleteDismiss20Regular,
  ArrowExportLtr20Regular,
} from '@vicons/fluent'

const logger = createLogger('useContextMenu')
export function useContextMenu({ store, message, reconvert = null }) {
  const { confirm } = useConfirm();
  const showDropdown = ref(false);
  const x = ref(0);
  const y = ref(0);
  const selectedRow = ref(null);

  // 右键菜单选项
  const contextMenuOptions = [
    {
      label: "打开",
      key: "open",
      icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
    },
    {
      label: "打开导入文件所在目录",
      key: "openDirImport",
      icon: () => h(NIcon, { component: FolderOpen20Regular, size: 16 }),
    },
    {
      label: "打开笔记文件所在目录",
      key: "openDirNote",
      icon: () => h(NIcon, { component: Notepad20Regular, size: 16 }),
    },
    {
      label: "重新转换(生成Markdown)",
      key: "reconvert",
      icon: () => h(NIcon, { component: ArrowSync20Regular, size: 16 }),
    },
    {
      label: "编辑",
      key: "edit",
      icon: () => h(NIcon, { component: Edit20Regular, size: 16 }),
    },
    {
      label: "分享",
      key: "share",
      icon: () => h(NIcon, { component: Share20Regular, size: 16 }),
    },
    {
      type: "divider",
      key: "d1",
    },
    {
      label: "导出",
      key: "export",
      icon: () => h(NIcon, { component: ArrowExportLtr20Regular, size: 16 }),
    },
    {
      label: "删除",
      key: "delete",
      icon: () => h(NIcon, { component: Delete20Regular, size: 16 }),
    },
    {
      label: "删除该条目及相关文件",
      key: "deleteAll",
      icon: () => h(NIcon, { component: DeleteDismiss20Regular, size: 16 }),
    },
  ];

  // 处理右键菜单选择
  const handleContextMenuSelect = (key) => {
    if (!selectedRow.value) return;

    switch (key) {
      case "open":
        handleOpen(selectedRow.value);
        break;
      case "openDirImport":
        handleOpenDirImport(selectedRow.value);
        break;
      case "openDirNote":
        handleOpenDirNote(selectedRow.value);
        break;
      case "reconvert":
        handleReconvert(selectedRow.value);
        break;
      case "edit":
        handleEdit(selectedRow.value);
        break;
      case "share":
        handleShare(selectedRow.value);
        break;
      case "delete":
        handleDelete(selectedRow.value);
        break;
      case "deleteAll":
        handleDeleteAll(selectedRow.value);
        break;
      case "move":
        handleMove(selectedRow.value);
        break;
      case "export":
        handleExport(selectedRow.value);
        break;
      default:
        logger.warn("未知操作:", key);
    }

    showDropdown.value = false;
  };

  // 处理点击外部关闭菜单
  const onClickoutside = () => {
    showDropdown.value = false;
  };

  // 处理行右键菜单
  const handleRowContextMenu = (e, row) => {
    e.preventDefault();
    selectedRow.value = row;
    showDropdown.value = false;
    nextTick(() => {
      showDropdown.value = true;
      x.value = e.clientX;
      y.value = e.clientY;
    });
  };

  // 文件类型映射函数
  const mapFileType = (rawType) => {
    const typeMap = {
      pdf: "pdf",
      md: "markdown",
      markdown: "markdown",
      html: "html",
      htm: "html",
      epub: "epub",
      doc: "word",
      docx: "word",
      word: "word",
      txt: "txt",
    };
    return typeMap[rawType] || null;
  };

  // 方法：打开文件
  const handleOpen = async (row) => {
    try {
      const rowFileType = row.type?.toLowerCase() || "";
      const rowFileName = row.importFileName || "";
      const importFileId = row.id || null;

      const fileType = mapFileType(rowFileType);
      if (!fileType) {
        logger.warn("未知文件类型:", rowFileType, row);
        message.warning("不支持的文件类型");
        return;
      }

      const baseFileName = rowFileName.includes(".")
        ? rowFileName.substring(0, rowFileName.lastIndexOf("."))
        : rowFileName;

      const windowLabel = `reader-${row.id}-${Date.now()}`;
      const windowTitle = `${rowFileName || "未命名文件"}`;

      const osPlatform = await platform();
      let readFilePath = "";

      logger.info("store.librariesRoot:", store.librariesRoot);

      if (store.currentDatabase.length > 0) {
        if (osPlatform === "windows") {
          readFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\${fileType}\\${rowFileName}`;
        } else if (osPlatform === "linux") {
          readFilePath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/${fileType}/${rowFileName}`;
        } else {
          logger.error("未知的操作系统平台:", osPlatform);
          return;
        }
      } else {
        logger.error("未知的数据库类型:", store.currentDatabase);
        return;
      }

      const settingsStore = useSettingsStore();
      const theme =
        settingsStore.appearanceTheme ||
        document?.documentElement?.dataset?.guaxiTheme ||
        "";

      await invoke("create_reader_window", {
        label: windowLabel,
        title: windowTitle,
        fileData: {
          file_type: fileType,
          file_name: rowFileName,
          file_path: readFilePath,
          library: row.library,
          import_file_id: importFileId,
        },
        theme,
      });

      logger.info("窗口创建成功:", {
        label: windowLabel,
        title: windowTitle,
        fileData: {
          type: fileType,
          name: rowFileName,
          path: readFilePath,
          library: row.library,
          import_file_id: importFileId,
        },
      });
    } catch (error) {
      logger.error("创建窗口失败:", error);
      message.error(`无法打开文件: ${error.message || "未知错误"}`);
    }
  };

  // 方法：重新转换（为缺少 Markdown / 转换失败或中断的条目重新入队）
  const handleReconvert = async (row) => {
    if (typeof reconvert !== "function") {
      message.warning("当前视图不支持重新转换");
      return;
    }
    if (!row?.importFileName || !row?.library) {
      message.error("缺少必要信息：文件名或库名（笔记条目不支持重新转换）");
      return;
    }
    try {
      await reconvert({
        library: row.library,
        fileName: row.importFileName,
        importedFileId: row.id ?? null,
        engine: "auto",
      });
    } catch (error) {
      logger.error("重新转换失败:", error);
    }
  };

  // 方法：编辑文件
  const handleEdit = async (row) => {
    try {
      if (store.currentDatabase === "importFileDB") {
        const updatedFile = {
          id: row.id,
          import_file_name: row.importFileName,
          description: row.description,
          library: row.library,
          file_type: row.type,
          import_file_create_time: row.importFileCreateTime,
          import_file_size: row.importFileSize,
          tags: row.importedFileTags.split(","),
        };
        await invoke("update_imported_file", { id: row.id, file: updatedFile });
        message.success("更新导入文件成功");
      } else {
        const updatedNote = {
          id: row.id,
          description: row.description,
          note_file_name: row.noteFileName,
          note_file_type: row.type,
          from_imported_file: row.fromImportedFile,
          from_imported_file_library: row.fromImportedFileLibrary,
          note_file_create_time: row.noteFileCreateTime,
          note_file_update_time: row.noteFileUpdateTime,
          tags: row.notesTags.split(","),
        };
        await invoke("update_note", { id: row.id, note: updatedNote });
        message.success("更新笔记成功");
      }

      await store.fetchFiles(store.currentDatabase);
    } catch (error) {
      logger.error("编辑失败:", error);
      message.error("编辑失败");
    }
  };

  // 方法：删除文件（软删除，删除前二次确认）
  const handleDelete = async (row) => {
    const name = row?.importFileName || row?.noteFileName || "该条目";
    const ok = await confirm({
      title: `删除「${name}」？`,
      description: "该条目将被标记为删除（不会立即移除磁盘上的原始文件与已转换内容）。",
      confirmLabel: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await invoke("delete_imported_file", { id: row.id });
      message.success("删除导入文件成功");
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      logger.error("删除失败:", error);
      message.error("删除失败");
    }
  };

  // 方法：删除所有相关文件（条目 + 磁盘文件，二次确认，提示更明确）
  const handleDeleteAll = async (row) => {
    if (!row.importFileName || !row.library) {
      message.error("缺少必要信息：文件名或库名");
      return;
    }

    const ok = await confirm({
      title: `删除「${row.importFileName}」及相关文件？`,
      description:
        "将删除该条目，并一并移除磁盘上的原始文件、已转换的 Markdown、图片与笔记等相关文件。此操作不可恢复。",
      confirmLabel: "彻底删除",
      danger: true,
    });
    if (!ok) return;

    const baseFileName = row.importFileName.includes(".")
      ? row.importFileName.substring(0, row.importFileName.lastIndexOf("."))
      : row.importFileName;

    const osPlatform = await platform();
    let dirPath = "";

    if (osPlatform === "windows") {
      dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}`;
    } else {
      dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}`;
    }

    try {
      await invoke("delete_imported_all_file", { id: row.id, path: dirPath });
      message.success("删除导入文件及相关文件成功");
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      logger.error("删除失败:", error);
      message.error("删除失败");
    }
  };

  // 方法：分享文件
  const handleShare = (row) => {
    logger.info("分享", row);
    message.info("分享功能待实现");
  };

  // 方法：移动文件
  const handleMove = (row) => {
    logger.info("移动", row);
    message.info("移动功能待实现");
  };

  // 方法：导出文件
  const handleExport = (row) => {
    logger.info("导出", row);
    message.info("导出功能待实现");
  };

  // 方法：在系统文件资源管理器中打开导入文件所在目录
  const handleOpenDirImport = async (row) => {
    try {
      if (!row.importFileName || !row.library) {
        message.error("缺少必要信息：文件名或库名");
        return;
      }

      const baseFileName = row.importFileName.includes(".")
        ? row.importFileName.substring(0, row.importFileName.lastIndexOf("."))
        : row.importFileName;

      const osPlatform = await platform();
      let dirPath = "";

      if (osPlatform === "windows") {
        dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}`;
      } else {
        dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}`;
      }

      await invoke("open_in_file_explorer", { path: dirPath });
      logger.info("在文件资源管理器中打开导入文件目录:", dirPath);
    } catch (error) {
      logger.error("打开导入文件目录失败:", error);
      message.error(`打开目录失败: ${error.message || error}`);
    }
  };

  // 方法：在系统文件资源管理器中打开笔记文件所在目录
  const handleOpenDirNote = async (row) => {
    try {
      if (!row.importFileName || !row.library) {
        message.error("缺少必要信息：笔记文件名或库名");
        return;
      }

      const baseFileName = row.importFileName.includes(".")
        ? row.importFileName.substring(0, row.importFileName.lastIndexOf("."))
        : row.importFileName;

      const osPlatform = await platform();
      let dirPath = "";

      if (osPlatform === "windows") {
        dirPath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\notes`;
      } else {
        dirPath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/notes`;
      }

      logger.info("笔记文件所在目录:", dirPath);
      await invoke("open_in_file_explorer", { path: dirPath });
      logger.info("在文件资源管理器中打开笔记文件目录:", dirPath);
    } catch (error) {
      logger.error("打开笔记文件目录失败:", error);
      message.error(`打开目录失败: ${error.message || error}`);
    }
  };

  // 行属性配置：右键点击触发菜单
  const rowProps = (row) => {
    return {
      onContextmenu: (e) => {
        e.preventDefault();
        selectedRow.value = row;
        showDropdown.value = false;
        nextTick(() => {
          showDropdown.value = true;
          x.value = e.clientX;
          y.value = e.clientY;
        });
      },
    };
  };

  return {
    showDropdown,
    x,
    y,
    selectedRow,
    contextMenuOptions,
    handleContextMenuSelect,
    onClickoutside,
    handleRowContextMenu,
    rowProps,
    handleOpen,
    handleEdit,
    handleShare,
    handleDelete,
    handleDeleteAll,
    handleMove,
    handleExport,
    handleOpenDirImport,
    handleOpenDirNote,
  };
}
