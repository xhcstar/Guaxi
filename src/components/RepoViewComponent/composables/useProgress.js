import { ref, computed } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";

export function useProgress({ store, message }) {
  const progresses = ref([]);
  const showTaskList = ref(false);

  listen("backup_progress", (event) => {
    const db = event.payload.databaseType || "资料库";
    let idx = progresses.value.findIndex(
      (p) => p.type === "backup" && p.fileName === db
    );
    if (idx === -1) {
      progresses.value.push({
        show: true,
        current: 0,
        total: 100,
        message: "开始备份...",
        fileName: db,
        type: "backup",
      });
      idx = progresses.value.length - 1;
    }
    progresses.value[idx] = {
      ...progresses.value[idx],
      current: event.payload.current,
      message: event.payload.message,
    };
    if (event.payload.current >= 100) {
      setTimeout(() => {
        const i = progresses.value.findIndex(
          (p) => p.type === "backup" && p.fileName === db
        );
        if (i !== -1 && progresses.value[i]?.current >= 100) {
          progresses.value.splice(i, 1);
        }
      }, 3000);
    }
  });

  listen("recovery_progress", (event) => {
    const db = event.payload.databaseType || "资料库";
    let idx = progresses.value.findIndex(
      (p) => p.type === "recovery" && p.fileName === db
    );
    if (idx === -1) {
      progresses.value.push({
        show: true,
        current: 0,
        total: 100,
        message: "开始恢复...",
        fileName: db,
        type: "recovery",
      });
      idx = progresses.value.length - 1;
    }
    progresses.value[idx] = {
      ...progresses.value[idx],
      current: event.payload.current,
      message: event.payload.message,
    };
    if (event.payload.current >= 100) {
      setTimeout(() => {
        const i = progresses.value.findIndex(
          (p) => p.type === "recovery" && p.fileName === db
        );
        if (i !== -1 && progresses.value[i]?.current >= 100) {
          progresses.value.splice(i, 1);
        }
      }, 3000);
    }
  });

  // 新增：计算属性 - 按任务类型分类
  const categorizedProgresses = computed(() => {
    const tasks = progresses.value || [];

    return {
      // 所有任务（保持原有接口兼容性）
      all: tasks,
      // 转换任务
      conversion: tasks.filter(
        (progress) =>
          progress.type === "conversion" ||
          progress.taskType === "conversion" ||
          progress.name?.includes("转换") ||
          progress.message?.includes("转换") ||
          progress.fileName?.includes("转换")
      ),
      // 备份任务
      backup: tasks.filter(
        (progress) =>
          progress.type === "backup" ||
          progress.taskType === "backup" ||
          progress.name?.includes("备份") ||
          progress.message?.includes("备份") ||
          progress.fileName?.includes("备份")
      ),
      // 恢复任务
      recovery: tasks.filter(
        (progress) =>
          progress.type === "recovery" ||
          progress.taskType === "recovery" ||
          progress.name?.includes("恢复") ||
          progress.message?.includes("恢复") ||
          progress.fileName?.includes("恢复")
      ),
      // 导入任务
      import: tasks.filter(
        (progress) =>
          progress.type === "import" ||
          progress.taskType === "import" ||
          progress.name?.includes("导入") ||
          progress.message?.includes("导入") ||
          progress.fileName?.includes("导入")
      ),
      // 导出任务
      export: tasks.filter(
        (progress) =>
          progress.type === "export" ||
          progress.taskType === "export" ||
          progress.name?.includes("导出") ||
          progress.message?.includes("导出") ||
          progress.fileName?.includes("导出")
      ),
    };
  });

  // 新增：获取特定类型任务数量的辅助函数
  const getProgressCountByType = (type) => {
    return categorizedProgresses.value[type]?.length || 0;
  };

  // 切换任务列表显示
  const toggleTaskList = () => {
    showTaskList.value = !showTaskList.value;
  };

  // 开始监听进度事件
  const startProgressListener = async (filePath) => {
    const fileName = filePath.split(/[\\/]/).pop() || "未知文件";

    console.info("[conversion] startProgressListener", {
      fileName,
      filePath,
      ts: new Date().toISOString(),
    });

    const newProgress = {
      show: true,
      current: 0,
      total: 100,
      message: "开始转换...",
      fileName,
      type: "conversion", // 新增：标记任务类型
    };

    progresses.value.push(newProgress);
    const progressIndex = progresses.value.length - 1;

    const unlisten = await listen("conversion_progress", (event) => {
      if (event.payload?.fileName && event.payload.fileName !== fileName) {
        console.debug("[conversion] progress ignored", {
          listeningFor: fileName,
          got: event.payload.fileName,
        });
        return;
      }

      console.debug("[conversion] progress", {
        fileName,
        current: event.payload.current,
        message: event.payload.message,
      });

      progresses.value[progressIndex] = {
        ...progresses.value[progressIndex],
        current: event.payload.current,
        message: event.payload.message,
      };

      // 转换完成后3秒自动移除
      if (event.payload.current >= 100) {
        console.info("[conversion] completed", { fileName });
        setTimeout(() => {
          if (progresses.value[progressIndex]?.current >= 100) {
            progresses.value.splice(progressIndex, 1);
          }
        }, 3000);
      }
    });

    return () => {
      console.info("[conversion] stopProgressListener", { fileName });
      return unlisten();
    };
  };

  // 转换文件
  const convertFile = async (inputPath, outputDir) => {
    const fileName = inputPath.split(/[\\/]/).pop() || "未知文件";
    const startedAt = Date.now();

    console.info("[conversion] invoke convert_to_markdown", {
      fileName,
      inputPath,
      outputDir,
    });

    try {
      await invoke(
        "convert_to_markdown",
        {
          request: {
            input_path: inputPath,
            output_dir: outputDir,
          },
        },
        {
          target: "main",
        }
      );

      console.info("[conversion] convert_to_markdown ok", {
        fileName,
        durationMs: Date.now() - startedAt,
      });
    } catch (error) {
      console.error("[conversion] convert_to_markdown failed", {
        fileName,
        durationMs: Date.now() - startedAt,
        error,
      });
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

  // 批量转换文件
  const batchConvertFiles = async (filePaths, outputDir) => {
    try {
      const results = [];

      for (const filePath of filePaths) {
        try {
          const fileName = filePath.split(/[\\/]/).pop() || "未知文件";

          // 添加进度条
          const progressIndex = progresses.value.length;
          progresses.value.push({
            show: true,
            current: 0,
            total: 100,
            message: `开始转换 ${fileName}...`,
            fileName,
            type: "conversion", // 新增：标记任务类型
          });

          // 监听该文件的进度
          const unlisten = await listen("conversion_progress", (event) => {
            if (event.payload.fileName === fileName) {
              progresses.value[progressIndex] = {
                ...progresses.value[progressIndex],
                current: event.payload.current,
                message: event.payload.message,
              };
            }
          });

          // 开始转换
          await convertFile(filePath, outputDir);

          // 转换完成
          progresses.value[progressIndex].current = 100;
          progresses.value[progressIndex].message = `转换完成: ${fileName}`;

          results.push({
            fileName,
            success: true,
            message: "转换成功",
          });

          // 3秒后移除进度条
          setTimeout(() => {
            progresses.value.splice(progressIndex, 1);
          }, 3000);

          unlisten();
        } catch (error) {
          results.push({
            fileName: filePath.split(/[\\/]/).pop() || "未知文件",
            success: false,
            message: error.message || "转换失败",
          });
        }
      }

      return results;
    } catch (error) {
      console.error("批量转换失败:", error);
      throw error;
    }
  };

  // 取消转换任务
  const cancelConversion = async (fileName) => {
    console.info("[conversion] cancelConversion", { fileName });

    try {
      await invoke("cancel_conversion", {
        fileName: fileName,
      });

      // 从进度列表中移除
      const index = progresses.value.findIndex((p) => p.fileName === fileName);
      if (index !== -1) {
        progresses.value.splice(index, 1);
      }

      message.info(`已取消转换任务: ${fileName}`);
    } catch (error) {
      console.error("[conversion] cancelConversion failed", { fileName, error });
      message.error(`取消转换失败: ${error.message || error}`);
    }
  };

  // 获取转换统计信息
  const getConversionStats = () => {
    const total = progresses.value.length;
    const completed = progresses.value.filter((p) => p.current >= 100).length;
    const inProgress = progresses.value.filter((p) => p.current < 100).length;
    const failed = progresses.value.filter((p) =>
      p.message.includes("失败")
    ).length;

    return {
      total,
      completed,
      inProgress,
      failed,
      progress: total > 0 ? Math.round((completed / total) * 100) : 0,
    };
  };

  // 新增：获取分类统计信息
  const getCategorizedStats = () => {
    const categorized = categorizedProgresses.value;
    return {
      conversion: {
        total: categorized.conversion.length,
        completed: categorized.conversion.filter((p) => p.current >= 100)
          .length,
        inProgress: categorized.conversion.filter((p) => p.current < 100)
          .length,
      },
      backup: {
        total: categorized.backup.length,
        completed: categorized.backup.filter((p) => p.current >= 100).length,
        inProgress: categorized.backup.filter((p) => p.current < 100).length,
      },
      recovery: {
        total: categorized.recovery.length,
        completed: categorized.recovery.filter((p) => p.current >= 100).length,
        inProgress: categorized.recovery.filter((p) => p.current < 100).length,
      },
      import: {
        total: categorized.import.length,
        completed: categorized.import.filter((p) => p.current >= 100).length,
        inProgress: categorized.import.filter((p) => p.current < 100).length,
      },
      export: {
        total: categorized.export.length,
        completed: categorized.export.filter((p) => p.current >= 100).length,
        inProgress: categorized.export.filter((p) => p.current < 100).length,
      },
    };
  };

  // 清除所有已完成的任务
  const clearCompletedTasks = () => {
    progresses.value = progresses.value.filter((p) => p.current < 100);
    message.info("已清除所有已完成的任务");
  };

  // 导出转换报告
  const exportConversionReport = () => {
    const report = {
      timestamp: new Date().toISOString(),
      stats: getConversionStats(),
      categorizedStats: getCategorizedStats(), // 新增：分类统计
      tasks: progresses.value.map((p) => ({
        fileName: p.fileName,
        progress: p.current,
        message: p.message,
        type: p.type || "unknown", // 新增：任务类型
        status: p.current >= 100 ? "completed" : "in-progress",
      })),
    };

    return JSON.stringify(report, null, 2);
  };

  // 重试失败的任务
  const retryFailedTasks = async () => {
    const failedTasks = progresses.value.filter((p) =>
      p.message.includes("失败")
    );

    for (const task of failedTasks) {
      try {
        // 重新开始转换
        const index = progresses.value.findIndex(
          (p) => p.fileName === task.fileName
        );
        if (index !== -1) {
          progresses.value[index] = {
            ...task,
            current: 0,
            message: "重新开始转换...",
          };
        }

        // 这里需要重新调用转换逻辑
        // await convertFile(task.filePath, task.outputDir)

        message.info(`重新开始转换: ${task.fileName}`);
      } catch (error) {
        console.error(`重试失败: ${task.fileName}`, error);
      }
    }
  };

  return {
    // 原有返回属性（保持兼容性）
    progresses, // 仍然返回原始数组
    showTaskList,
    toggleTaskList,
    startProgressListener,
    convertFile,
    buildConvertOutputFilePath,
    batchConvertFiles,
    cancelConversion,
    getConversionStats,
    clearCompletedTasks,
    exportConversionReport,
    retryFailedTasks,

    // 新增：分类任务相关属性
    categorizedProgresses, // 分类后的任务
    getProgressCountByType, // 按类型获取任务数量
    getCategorizedStats, // 获取分类统计
  };
}
