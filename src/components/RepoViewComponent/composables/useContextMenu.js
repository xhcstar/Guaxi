import { ref, nextTick, h } from "vue";
import { NIcon } from "naive-ui";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { useSettingsStore } from "../../../stores/settingsStore";

export function useContextMenu({ store, message }) {
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
      icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
    },
    {
      label: "打开笔记文件所在目录",
      key: "openDirNote",
      icon: () => h(NIcon, { component: Open20Regular, size: 16 }),
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
      label: "删除",
      key: "delete",
      icon: () => h(NIcon, { component: Delete20Regular, size: 16 }),
    },
    {
      label: "删除该条目及相关文件",
      key: "deleteAll",
      icon: () => h(NIcon, { component: Delete20Regular, size: 16 }),
    },
    {
      label: "移动",
      key: "move",
      icon: () => h(NIcon, { component: ArrowMove24Regular, size: 16 }),
    },
    {
      label: "导出",
      key: "export",
      icon: () => h(NIcon, { component: ArrowMove24Regular, size: 16 }),
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
        console.warn("未知操作:", key);
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
        console.warn("未知文件类型:", rowFileType, row);
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

      console.log("store.librariesRoot:", store.librariesRoot);

      if (store.currentDatabase.length > 0) {
        if (osPlatform === "windows") {
          readFilePath = `${store.librariesRoot}\\Libraries\\${row.library}\\${baseFileName}\\${fileType}\\${rowFileName}`;
        } else if (osPlatform === "linux") {
          readFilePath = `${store.librariesRoot}/Libraries/${row.library}/${baseFileName}/${fileType}/${rowFileName}`;
        } else {
          console.error("未知的操作系统平台:", osPlatform);
          return;
        }
      } else {
        console.error("未知的数据库类型:", store.currentDatabase);
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

      console.log("窗口创建成功:", {
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
      console.error("创建窗口失败:", error);
      message.error(`无法打开文件: ${error.message || "未知错误"}`);
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
      console.error("编辑失败:", error);
      message.error("编辑失败");
    }
  };

  // 方法：删除文件
  const handleDelete = async (row) => {
    try {
      await invoke("delete_imported_file", { id: row.id });
      message.success("删除导入文件成功");
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      console.error("删除失败:", error);
      message.error("删除失败");
    }
  };

  // 方法：删除所有相关文件
  const handleDeleteAll = async (row) => {
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

    try {
      await invoke("delete_imported_all_file", { id: row.id, path: dirPath });
      message.success("删除导入文件及相关文件成功");
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      console.error("删除失败:", error);
      message.error("删除失败");
    }
  };

  // 方法：分享文件
  const handleShare = (row) => {
    console.log("分享", row);
    message.info("分享功能待实现");
  };

  // 方法：移动文件
  const handleMove = (row) => {
    console.log("移动", row);
    message.info("移动功能待实现");
  };

  // 方法：导出文件
  const handleExport = (row) => {
    console.log("导出", row);
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
      console.log("在文件资源管理器中打开导入文件目录:", dirPath);
    } catch (error) {
      console.error("打开导入文件目录失败:", error);
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

      console.log("笔记文件所在目录:", dirPath);
      await invoke("open_in_file_explorer", { path: dirPath });
      console.log("在文件资源管理器中打开笔记文件目录:", dirPath);
    } catch (error) {
      console.error("打开笔记文件目录失败:", error);
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

// 图标组件（需要在实际使用中导入）
const Open20Regular = {
  render() {
    return h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        viewBox: "0 0 20 20",
        fill: "currentColor",
      },
      [
        h("path", {
          d: "M4.5 2A2.5 2.5 0 0 0 2 4.5v11A2.5 2.5 0 0 0 4.5 18h11a2.5 2.5 0 0 0 2.5-2.5v-11A2.5 2.5 0 0 0 15.5 2h-11zm0 1h11A1.5 1.5 0 0 1 17 4.5v11a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 15.5v-11A1.5 1.5 0 0 1 4.5 3zm2.5 4a.5.5 0 0 0 0 1h6a.5.5 0 0 0 0-1H7zm0 3a.5.5 0 0 0 0 1h6a.5.5 0 0 0 0-1H7zm0 3a.5.5 0 0 0 0 1h6a.5.5 0 0 0 0-1H7z",
        }),
      ]
    );
  },
};

const Edit20Regular = {
  render() {
    return h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        viewBox: "0 0 20 20",
        fill: "currentColor",
      },
      [
        h("path", {
          d: "M13.89 3.39a2 2 0 0 1 2.83 0l.88.88a2 2 0 0 1 0 2.83l-9.03 9.03a1.5 1.5 0 0 1-.66.39l-3.83.97a.5.5 0 0 1-.62-.62l.97-3.83a1.5 1.5 0 0 1 .39-.66l9.03-9.03zm1.42 1.42l-.88-.88a1 1 0 0 0-1.42 0l-8.58 8.58a.5.5 0 0 0-.13.22l-.78 3.07 3.07-.78a.5.5 0 0 0 .22-.13l8.58-8.58a1 1 0 0 0 0-1.42z",
        }),
      ]
    );
  },
};

const Share20Regular = {
  render() {
    return h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        viewBox: "0 0 20 20",
        fill: "currentColor",
      },
      [
        h("path", {
          d: "M14.5 4a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zm-9 6a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zm9 1a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm-9-7a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm9.5 3.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0zm-9-6a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z",
        }),
      ]
    );
  },
};

const Delete20Regular = {
  render() {
    return h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        viewBox: "0 0 20 20",
        fill: "currentColor",
      },
      [
        h("path", {
          d: "M8.5 4h3a1.5 1.5 0 0 0-3 0zm-1 0a2.5 2.5 0 0 1 5 0h5a.5.5 0 0 1 0 1h-1v9.5a2.5 2.5 0 0 1-2.5 2.5h-5A2.5 2.5 0 0 1 4.5 14.5V5h-1a.5.5 0 0 1 0-1h5zm2 4.5a.5.5 0 0 0-1 0v5a.5.5 0 0 0 1 0v-5zm3 0a.5.5 0 0 0-1 0v5a.5.5 0 0 0 1 0v-5z",
        }),
      ]
    );
  },
};

const ArrowMove24Regular = {
  render() {
    return h(
      "svg",
      {
        xmlns: "http://www.w3.org/2000/svg",
        viewBox: "0 0 24 24",
        fill: "currentColor",
      },
      [
        h("path", {
          d: "M12 2a1 1 0 0 1 1 1v4.586l1.293-1.293a1 1 0 1 1 1.414 1.414l-3 3a1 1 0 0 1-1.414 0l-3-3a1 1 0 0 1 1.414-1.414L11 7.586V3a1 1 0 0 1 1-1zm0 20a1 1 0 0 1-1-1v-4.586l-1.293 1.293a1 1 0 0 1-1.414-1.414l3-3a1 1 0 0 1 1.414 0l3 3a1 1 0 0 1-1.414 1.414L13 16.414V21a1 1 0 0 1-1 1zm10-10a1 1 0 0 1-1 1h-4.586l1.293-1.293a1 1 0 0 1-1.414-1.414l-3 3a1 1 0 0 1-1.414 0l-3-3a1 1 0 0 1 1.414-1.414L10 12.586V8a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1zm-20 0a1 1 0 0 1 1-1h4.586l-1.293 1.293a1 1 0 1 1-1.414-1.414l3-3a1 1 0 0 1 1.414 0l3 3a1 1 0 0 1-1.414 1.414L14 12.586V17a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z",
        }),
      ]
    );
  },
};
