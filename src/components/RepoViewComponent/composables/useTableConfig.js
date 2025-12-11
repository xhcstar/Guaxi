// useTableConfig.js
import { ref, computed, h } from "vue";
import { invoke } from "@tauri-apps/api/core";

export function useTableConfig({ store, message, ShowOrEdit }) {
  const tableRef = ref(null);
  const checkedRowKeys = ref([]);

  // 分页配置
  const pagination = ref({
    pageSize: 10,
    showSizePicker: true,
    pageSizes: [10, 20, 50, 100],
    showQuickJumper: true,
    prefix: (info) => `共 ${info.itemCount} 条记录`,
  });

  // 表格高度计算
  const tableHeight = computed(() => {
    const headerHeight = 85;
    const rowHeight = 100;
    const pageSize = pagination.value.pageSize || 10;
    return headerHeight + rowHeight * pageSize;
  });

  // 计算表格总宽度
  const scrollX = computed(() => {
    let totalWidth = 0;
    columns.value.forEach((column) => {
      totalWidth += column.width || 120;
    });
    return totalWidth;
  });

  // 行键函数
  const rowKey = (row) => row.id;

  // 行高函数
  const heightForRow = () => 40;

  // 标题样式
  const titleStyle = {
    fontSize: "17px",
    fontWeight: "bold",
    color: "#000000",
  };

  // 校验表格单元格
  const validateTableCell = (row) => {
    console.log("validateTableCell", row);

    // 校验标签
    if (!row.importedFileTags || row.importedFileTags.trim() === "") {
      row.importedFileTags = "default";
    } else {
      const tags = row.importedFileTags.split(",").map((tag) => tag.trim());
      if (tags.some((tag) => !tag)) {
        return "多个标签没有以逗号分隔";
      }
    }

    // 校验描述
    if (!row.description || row.description.trim() === "") {
      return "描述不能为空";
    }
    if (row.description.length > 240) {
      return "描述不能超过 240 个字符";
    }

    // 校验所属库
    if (
      !row.library ||
      (typeof row.library === "string" && row.library.trim() === "")
    ) {
      row.library = "default";
      return "所属库不能为空";
    }

    // 校验入库时间
    if (!row.importFileCreateTime || row.importFileCreateTime.trim() === "") {
      return "入库时间不能为空";
    }

    // 校验文件大小
    if (!row.importFileSize || row.importFileSize <= 0) {
      return "文件大小不能为空";
    }

    return null;
  };

  // 更新导入文件
  const updateImportedFile = async (row) => {
    try {
      const importedFileTags =
        typeof row.importedFileTags === "string"
          ? row.importedFileTags.split(",")
          : row.importedFileTags;

      console.log("updateImportedFile row:", row);
      const file = {
        import_file_name: row.importFileName,
        description: row.description,
        library: row.library,
        tags: importedFileTags,
        file_type: row.type,
        import_file_create_time: row.importFileCreateTime,
        import_file_size: row.importFileSize,
        knowledge_repo: row.knowledgeRepo,
      };

      console.log("row.id:", row.id);
      console.log("file:", JSON.stringify(file, null, 2));

      await invoke("update_imported_file", {
        id: row.id,
        file: file,
      });
      console.log("更新导入文件成功:", row);
    } catch (error) {
      console.error("更新导入文件失败:", error);
      if (message) {
        message.error("更新文件失败: " + error.message);
      }
    }
  };

  const tagColorMap = new Map();
  const tagColors = ['success', 'warning', 'error', 'info', 'default'];
  let colorIndex = 0;

  const getTagColor = (tag) => {
    if (!tagColorMap.has(tag)) {
      tagColorMap.set(tag, tagColors[colorIndex % tagColors.length]);
      colorIndex++;
    }
    return tagColorMap.get(tag);
  };

  // 文件大小显示格式化（从字节换算为 KB 或 MB，仅用于展示）
  const formatFileSize = (bytes) => {
    if (bytes == null || isNaN(bytes)) return '';
    const b = Number(bytes);
    const KB = 1024;
    const MB = KB * 1024;
    if (b < MB) return (b / KB).toFixed(2) + ' KB';
    return (b / MB).toFixed(2) + ' MB';
  };

  // 动态生成列配置
  const columns = computed(() => {
    const columnDefinitions = [
      // 选择列
      {
        type: "selection",
        width: 30,
        fixed: "left",
        align: "center",
        key: "selection",
      },
      // 文件名列
      {
        title: () => h("span", { style: titleStyle }, "文件名"),
        key: "importFileName",
        width: 200,
        fixed: "left",
        render: (row) =>
          h(ShowOrEdit, {
            value: row.importFileName,
            onUpdateValue: async (value) => {
              const nv = typeof value === "string" ? value.trim() : value;
              const ov = typeof row.importFileName === "string" ? row.importFileName.trim() : row.importFileName;
              if (nv === ov) return;
              const error = validateTableCell(row, "importFileName", nv);
              if (error) {
                if (message) message.error(error);
                return;
              }
              const updatedRow = { ...row, importFileName: nv };
              try {
                await updateImportedFile(updatedRow);
                Object.assign(row, updatedRow);
              } catch (error) {
                console.error("更新失败:", error);
              }
            },
          }),
      },
      // 文件描述列
      {
        title: () => h("span", { style: titleStyle }, "文件描述"),
        key: "description",
        width: 200,
        render: (row) =>
          h(ShowOrEdit, {
            value: row.description,
            onUpdateValue: async (value) => {
              const nv = typeof value === "string" ? value.trim() : value;
              const ov = typeof row.description === "string" ? row.description.trim() : row.description;
              if (nv === ov) return;
              const error = validateTableCell(row, "description", nv);
              if (error) {
                if (message) message.error(error);
                return;
              }
              const updatedRow = { ...row, description: nv };
              try {
                await updateImportedFile(updatedRow);
                Object.assign(row, updatedRow);
              } catch (error) {
                console.error("更新失败:", error);
              }
            },
            enableAutoComplete: false,
          }),
      },
      {
        title: () => h("span", { style: titleStyle }, "文件标签"),
        key: "importedFileTags",
        width: 200,
        render: (row) =>
          h(ShowOrEdit, {
            value: row.importedFileTags,
            onUpdateValue: async (value) => {
              const normalize = (v) => {
                if (Array.isArray(v)) return v.map(t => String(t).trim()).filter(Boolean).join(",");
                if (typeof v === "string") return v.split(",").map(t => t.trim()).filter(Boolean).join(",");
                return "";
              };
              const nv = normalize(value);
              const ov = normalize(row.importedFileTags);
              if (nv === ov) return;
              const error = validateTableCell(row, "importedFileTags", nv);
              if (error) {
                if (message) message.error(error);
                return;
              }
              const updatedRow = { ...row, importedFileTags: nv };
              try {
                await updateImportedFile(updatedRow);
                Object.assign(row, updatedRow);
              } catch (error) {
                console.error("更新失败:", error);
              }
            },
            enableAutoComplete: true,
            searchType: "importedFileTags",
            displayMode: "tags",
            getTagColor: getTagColor
          }),
      },
      // 文件库列
      {
        title: () => h("span", { style: titleStyle }, "所属文件库"),
        key: "library",
        width: 150,
        render: (row) =>
          h(ShowOrEdit, {
            value: row.library,
            onUpdateValue: async (value) => {
              const nv = typeof value === "string" ? value.trim() : value;
              const ov = typeof row.library === "string" ? row.library.trim() : row.library;
              if (nv === ov) return;
              const error = validateTableCell(row, "library", nv);
              if (error) {
                if (message) message.error(error);
                return;
              }
              const updatedRow = { ...row, library: nv };
              try {
                await updateImportedFile(updatedRow);
                Object.assign(row, updatedRow);
              } catch (error) {
                console.error("更新失败:", error);
              }
            },
            enableAutoComplete: true,
            searchType: "libraries",
          }),
      },
      // 入库时间列 - 只读
      {
        title: () => h("span", { style: titleStyle }, "入库时间"),
        key: "importFileCreateTime",
        width: 170,
        render: (row) =>
          h(ShowOrEdit, {
            value: row.importFileCreateTime,
            readonly: true,
          }),
      },
      // 文件类型列 - 只读
      {
        title: () => h("span", { style: titleStyle }, "文件类型"),
        key: "type",
        width: 120,
        render: (row) =>
          h(ShowOrEdit, {
            value: row.type,
            readonly: true,
          }),
      },
      // 文件大小列 - 只读
      {
        title: () => h("span", { style: titleStyle }, "文件大小"),
        key: "importFileSize",
        width: 120,
        render: (row) =>
          h(ShowOrEdit, {
            value: formatFileSize(row.importFileSize),
            readonly: true,
          }),
      },
      // 笔记相关列 - 全部只读
      // {
      //   title: () => h("span", { style: titleStyle }, "笔记文件名"),
      //   key: "newNoteForm.noteFileName",
      //   width: 200,
      //   render: (row) =>
      //     h(ShowOrEdit, {
      //       value: row.newNoteForm?.noteFileName,
      //       readonly: true,
      //     }),
      // },
      // {
      //   title: () => h("span", { style: titleStyle }, "笔记描述"),
      //   key: "newNoteForm.description",
      //   width: 200,
      //   render: (row) =>
      //     h(ShowOrEdit, {
      //       value: row.newNoteForm?.description,
      //       readonly: true,
      //     }),
      // },
      // {
      //   title: () => h("span", { style: titleStyle }, "笔记标签"),
      //   key: "newNoteForm.notesTags",
      //   width: 150,
      //   render: (row) =>
      //     h(ShowOrEdit, {
      //       value: row.newNoteForm?.notesTags,
      //       readonly: true,
      //     }),
      // },
      // {
      //   title: () => h("span", { style: titleStyle }, "笔记创建时间"),
      //   key: "newNoteForm.noteFileCreateTime",
      //   width: 170,
      //   render: (row) =>
      //     h(ShowOrEdit, {
      //       value: row.newNoteForm?.noteFileCreateTime,
      //       readonly: true,
      //     }),
      // },
      // {
      //   title: () => h("span", { style: titleStyle }, "笔记修改时间"),
      //   key: "newNoteForm.noteFileUpdateTime",
      //   width: 170,
      //   fixed: "right",
      //   align: "left",
      //   render: (row) =>
      //     h(ShowOrEdit, {
      //       value: row.newNoteForm?.noteFileUpdateTime,
      //       readonly: true,
      //     }),
      // },
    ];

    return columnDefinitions;
  });

  // 处理选中行更新
  const handleCheckboxUpdate = (rowKeys) => {
    checkedRowKeys.value = rowKeys;
    console.log("选中的行 keys:", rowKeys);
  };

  // 排序方法
  const sortTitle = () => {
    tableRef.value?.sort("title", "ascend");
  };

  // 清除过滤器
  const clearFilters = () => {
    tableRef.value?.filter(null);
  };

  // 获取排序状态
  const getSortState = () => {
    return tableRef.value?.getSortState();
  };

  // 获取过滤状态
  const getFilterState = () => {
    return tableRef.value?.getFilterState();
  };

  // 重置表格状态
  const resetTableState = () => {
    tableRef.value?.clearFilters();
    tableRef.value?.clearSorter();
    tableRef.value?.clearSelection();
  };

  // 导出表格数据
  const exportTableData = (format = "csv") => {
    const data = store.files || [];
    let content = "";

    if (format === "csv") {
      // 生成CSV格式
      const headers = columns.value
        .filter((col) => col.key !== "selection")
        .map((col) => {
          const title = typeof col.title === "function" ? "标题" : col.title;
          return `"${title}"`;
        })
        .join(",");

      const rows = data.map((row) =>
        columns.value
          .filter((col) => col.key !== "selection")
          .map((col) => {
            const value = getNestedValue(row, col.key);
            return `"${value || ""}"`;
          })
          .join(",")
      );
      content = [headers, ...rows].join("\n");
    } else if (format === "json") {
      content = JSON.stringify(data, null, 2);
    }

    return content;
  };

  // 辅助函数：获取嵌套对象属性
  const getNestedValue = (obj, path) => {
    return path.split(".").reduce((o, p) => o?.[p], obj);
  };

  return {
    // 响应式引用
    tableRef,
    checkedRowKeys,

    // 计算属性
    tableHeight,
    scrollX,
    columns,

    // 配置
    pagination,
    rowKey,
    heightForRow,

    // 方法
    handleCheckboxUpdate,
    sortTitle,
    clearFilters,
    getSortState,
    getFilterState,
    resetTableState,
    exportTableData,
    validateTableCell,
    updateImportedFile,
  };
}
