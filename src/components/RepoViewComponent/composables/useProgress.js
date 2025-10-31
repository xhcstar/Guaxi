import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export function useProgress({ store, message }) {
  const progresses = ref([]);
  const showTaskList = ref(false);

  // 切换任务列表显示
  const toggleTaskList = () => {
    showTaskList.value = !showTaskList.value;
  };

  // 开始监听进度事件
  const startProgressListener = async (filePath) => {
    const fileName = filePath.split(/[\\/]/).pop() || "未知文件";

    const newProgress = {
      show: true,
      current: 0,
      total: 100,
      message: "开始转换...",
      fileName,
    };

    progresses.value.push(newProgress);
    const progressIndex = progresses.value.length - 1;

    const unlisten = await listen("conversion_progress", (event) => {
      progresses.value[progressIndex] = {
        ...progresses.value[progressIndex],
        current: event.payload.current,
        message: event.payload.message,
      };

      // 转换完成后3秒自动移除
      if (event.payload.current >= 100) {
        setTimeout(() => {
          if (progresses.value[progressIndex]?.current >= 100) {
            progresses.value.splice(progressIndex, 1);
          }
        }, 3000);
      }
    });

    return unlisten;
  };

  // 转换文件
  const convertFile = async (inputPath, outputDir) => {
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
    } catch (error) {
      console.error("Conversion failed:", error);
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
      console.error("取消转换失败:", error);
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
      tasks: progresses.value.map((p) => ({
        fileName: p.fileName,
        progress: p.current,
        message: p.message,
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
    progresses,
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
  };
}
