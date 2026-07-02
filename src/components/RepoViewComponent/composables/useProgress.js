import { ref, computed } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";
import { createLogger } from '../../../utils/logger'

const logger = createLogger('useProgress')
const sharedProgresses = ref([]);
const sharedShowTaskList = ref(false);
const sharedConversionTasks = ref([]);
let baseListenersInitialized = false;
let taskUpdateListenerInitialized = false;

export function useProgress({ store, message }) {
  const progresses = sharedProgresses;
  const showTaskList = sharedShowTaskList;

  const findLatestConversionProgressIndex = (fileName) => {
    for (let i = progresses.value.length - 1; i >= 0; i -= 1) {
      const p = progresses.value[i];
      if (p?.type === "conversion" && p?.fileName === fileName) {
        return i;
      }
    }
    return -1;
  };

  const markConversionFailed = (fileName, error) => {
    const idx = findLatestConversionProgressIndex(fileName);
    const errorMessage =
      (typeof error === "string" && error) ||
      error?.message ||
      error?.toString?.() ||
      "未知错误";

    const payload = {
      current: 100,
      total: 100,
      message: `转换失败: ${errorMessage}`,
      fileName,
      type: "conversion",
      failed: true,
      finishedAt: new Date().toISOString(),
    };

    if (idx !== -1) {
      progresses.value[idx] = {
        ...progresses.value[idx],
        ...payload,
      };
      return;
    }

    progresses.value.push({ show: true, ...payload });
  };

  const clearFailedTasks = () => {
    const before = progresses.value.length;
    progresses.value = progresses.value.filter(
      (p) => !(p?.type === "conversion" && (p?.failed || p?.message?.includes("失败")))
    );
    const removed = before - progresses.value.length;
    message.info(`已清除失败任务: ${removed} 个`);
  };

  // 终止正在进行的同步任务（按 run_id）。后端在检查点干净停止，已完成部分保留。
  const cancelSyncTask = async (runId) => {
    if (!runId) return;
    const idx = progresses.value.findIndex(
      (p) => p.type === "sync" && p.id === runId
    );
    if (idx !== -1) {
      // 即时反馈"终止中"。
      progresses.value[idx] = { ...progresses.value[idx], cancelling: true };
    }
    try {
      await invoke("p2p_sync_cancel", { runId });
      message.info("已请求终止同步，将在当前文件完成后停止");
    } catch (error) {
      logger.error("[sync] 终止同步失败", error);
      message.error(`终止同步失败: ${error.message || error}`);
      if (idx !== -1) {
        progresses.value[idx] = { ...progresses.value[idx], cancelling: false };
      }
    }
  };

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

  if (!baseListenersInitialized) {
    baseListenersInitialized = true;

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

    // 资源库/资料库 P2P 同步进度。后端字段：run_id/target/phase/status/message/current/total。
    const syncTargetLabels = {
      resource_repo: "资源库",
      knowledge_base: "知识库",
      libraries_root: "文件库",
      template: "模板库",
      slidev: "Slidev",
    };
    // 各阶段映射到一个基线百分比，传输阶段按 current/total 细化。
    const syncPhaseBaseline = {
      connect: 5,
      scan: 15,
      plan: 30,
      transfer: 40,
      records: 90,
      complete: 100,
    };
    const computeSyncPercent = (payload) => {
      const phase = String(payload.phase || "");
      const status = String(payload.status || "");
      if (
        phase === "complete" ||
        status === "success" ||
        status === "error" ||
        status === "skipped" ||
        status === "waiting"
      ) {
        return 100;
      }
      if (phase === "transfer" && Number(payload.total) > 0) {
        const pct = Math.round((Number(payload.current) / Number(payload.total)) * 100);
        // 传输阶段限定在 40..89，避免与基线阶段重叠或过早显示 100。
        return Math.min(89, Math.max(40, pct));
      }
      return syncPhaseBaseline[phase] ?? 10;
    };

    listen("resource_repo_sync_progress", async (event) => {
      const payload = event.payload || {};
      const runId = payload.run_id || "sync";
      const label = syncTargetLabels[payload.target] || payload.target || "同步";
      const fileName = `${label} 同步`;
      const percent = computeSyncPercent(payload);
      const status = String(payload.status || "");
      const failed = status === "error";
      const cancelled = status === "cancelled";

      let idx = progresses.value.findIndex(
        (p) => p.type === "sync" && p.id === runId
      );
      if (idx === -1) {
        progresses.value.push({
          show: true,
          current: 0,
          total: 100,
          message: payload.message || "开始同步...",
          fileName,
          type: "sync",
          id: runId,
        });
        idx = progresses.value.length - 1;
      }
      progresses.value[idx] = {
        ...progresses.value[idx],
        current: percent,
        message: payload.message || progresses.value[idx].message,
        fileName,
        failed,
        cancelled,
        // 收到终止回执后清除"终止中"瞬时态。
        cancelling: cancelled ? false : progresses.value[idx].cancelling,
      };

      // 同步成功完成：刷新左侧知识库列表与中间文件表，完成视图刷新。
      if (payload.phase === "complete" && payload.status === "success") {
        try {
          await store.fetchRepos();
          await store.fetchFiles(store.currentDatabase);
        } catch (e) {
          logger.error("[sync] 同步后刷新视图失败", e);
        }
      }

      // 成功完成 3 秒后自动移除；失败或被终止则保留供查看。
      if (percent >= 100 && !failed && !cancelled) {
        setTimeout(() => {
          const i = progresses.value.findIndex(
            (p) => p.type === "sync" && p.id === runId
          );
          if (
            i !== -1 &&
            progresses.value[i]?.current >= 100 &&
            !progresses.value[i]?.failed &&
            !progresses.value[i]?.cancelled
          ) {
            progresses.value.splice(i, 1);
          }
        }, 3000);
      }
    });
  }

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
      // 同步任务
      sync: tasks.filter(
        (progress) =>
          progress.type === "sync" ||
          progress.taskType === "sync" ||
          progress.fileName?.includes("同步")
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

    logger.info("[conversion] startProgressListener", {
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

    const taskId = `${Date.now()}-${Math.random().toString(16).slice(2)}`;

    progresses.value.push({ ...newProgress, id: taskId });

    let unlisten = null;
    let disposed = false;
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      if (typeof unlisten === "function") {
        try {
          unlisten();
        } catch (e) {
          logger.warn("[conversion] unlisten error", e);
        }
      }
    };

    unlisten = await listen("conversion_progress", (event) => {
      if (event.payload?.fileName && event.payload.fileName !== fileName) {
        logger.debug("[conversion] progress ignored", {
          listeningFor: fileName,
          got: event.payload.fileName,
        });
        return;
      }

      const idx = progresses.value.findIndex(
        (p) => p?.type === "conversion" && p?.fileName === fileName && p?.id === taskId
      );
      if (idx === -1) {
        return;
      }

      logger.debug("[conversion] progress", {
        fileName,
        current: event.payload.current,
        message: event.payload.message,
        failed: event.payload?.failed,
      });

      const failed =
        event.payload?.failed ||
        progresses.value[idx]?.failed ||
        event.payload?.message?.includes("失败");

      progresses.value[idx] = {
        ...progresses.value[idx],
        current: event.payload.current,
        message: event.payload.message,
        failed,
      };

      const shouldAutoRemove =
        event.payload.current >= 100 &&
        !failed &&
        !event.payload?.message?.includes("失败");

      if (shouldAutoRemove) {
        logger.info("[conversion] completed", { fileName });
        // 转换成功完成后刷新文件表，让 markdown 立即可读。
        (async () => {
          try {
            await store.fetchFiles(store.currentDatabase);
          } catch (e) {
            logger.warn("[conversion] refresh after complete failed", e);
          }
        })();
        setTimeout(() => {
          const i = progresses.value.findIndex(
            (p) => p?.type === "conversion" && p?.fileName === fileName && p?.id === taskId
          );
          if (i !== -1 && progresses.value[i]?.current >= 100 && !progresses.value[i]?.failed) {
            progresses.value.splice(i, 1);
          }
          dispose();
        }, 3000);
      }

      // 终态失败：停止监听（保留失败条目供查看/重试）。
      if (failed) {
        dispose();
      }
    });

    return () => {
      logger.info("[conversion] stopProgressListener", { fileName });
      dispose();
    };
  };

  // 转换文件：入队到持久化转换队列（支持多引擎自动路由 / 断点续跑 / 重试）。
  // engine 取值：auto（默认，自动检测）| fast | marker | mineru。
  // 入队立即返回，实际进度由 conversion_progress 事件驱动（startProgressListener 已在监听）。
  const convertFile = async (inputPath, outputDir, engine = "auto") => {
    const fileName = inputPath.split(/[\\/]/).pop() || "未知文件";
    const startedAt = Date.now();

    logger.info("[conversion] invoke enqueue_conversion", {
      fileName,
      inputPath,
      outputDir,
      engine,
    });

    try {
      const resp = await invoke("enqueue_conversion", {
        request: {
          inputPath,
          outputDir,
          engine,
        },
      });

      logger.info("[conversion] enqueue_conversion ok", {
        fileName,
        taskId: resp?.taskId,
        reused: resp?.reused,
        durationMs: Date.now() - startedAt,
      });

      return resp;
    } catch (error) {
      logger.error("[conversion] enqueue_conversion failed", {
        fileName,
        durationMs: Date.now() - startedAt,
        error,
      });

      markConversionFailed(fileName, error);
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
      logger.error("批量转换失败:", error);
      throw error;
    }
  };

  // 取消转换任务
  const cancelConversion = async (fileName) => {
    logger.info("[conversion] cancelConversion", { fileName });

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
      logger.error("[conversion] cancelConversion failed", { fileName, error });
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
      sync: {
        total: categorized.sync.length,
        completed: categorized.sync.filter((p) => p.current >= 100).length,
        inProgress: categorized.sync.filter((p) => p.current < 100).length,
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
        logger.error(`重试失败: ${task.fileName}`, error);
      }
    }
  };

  // ───────────────── 持久化转换任务面板（SQLite 队列） ─────────────────
  // 从后端 conversion_tasks 表加载的完整任务列表，支持断点续跑/重试/取消。
  const conversionTasks = sharedConversionTasks;

  const refreshConversionTasks = async (status = null, limit = 200) => {
    try {
      const tasks = await invoke("list_conversion_tasks", { status, limit });
      conversionTasks.value = Array.isArray(tasks) ? tasks : [];
      return conversionTasks.value;
    } catch (error) {
      logger.error("[conversion] list_conversion_tasks failed", error);
      return [];
    }
  };

  const retryConversionTask = async (taskId) => {
    try {
      const changed = await invoke("retry_conversion_task", { taskId });
      if (changed) {
        message.info("已重新排入转换队列");
        await refreshConversionTasks();
      } else {
        message.warning("该任务当前状态不可重试");
      }
      return changed;
    } catch (error) {
      logger.error("[conversion] retry_conversion_task failed", error);
      message.error(`重试失败: ${error.message || error}`);
      return false;
    }
  };

  const cancelConversionTask = async (taskId) => {
    try {
      const changed = await invoke("cancel_conversion_task", { taskId });
      if (changed) {
        message.info("已请求取消转换任务");
        await refreshConversionTasks();
      } else {
        message.warning("该任务已结束，无法取消");
      }
      return changed;
    } catch (error) {
      logger.error("[conversion] cancel_conversion_task failed", error);
      message.error(`取消失败: ${error.message || error}`);
      return false;
    }
  };

  const getConversionEngineStatus = async () => {
    try {
      return await invoke("conversion_engine_status");
    } catch (error) {
      logger.error("[conversion] conversion_engine_status failed", error);
      return { mineruAvailable: false };
    }
  };

  if (!taskUpdateListenerInitialized) {
    taskUpdateListenerInitialized = true;
    // 监听后端任务状态变更，增量更新任务面板。
    listen("conversion_task_update", (event) => {
      const task = event.payload;
      if (!task || task.id == null) return;
      const idx = conversionTasks.value.findIndex((t) => t.id === task.id);
      if (idx === -1) {
        conversionTasks.value.unshift(task);
      } else {
        conversionTasks.value[idx] = task;
      }
    });
    // 初次加载一次现存任务（含重启后恢复的任务）。
    refreshConversionTasks();
  }

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
    clearFailedTasks,
    exportConversionReport,
    retryFailedTasks,
    cancelSyncTask, // 终止同步任务

    // 新增：分类任务相关属性
    categorizedProgresses, // 分类后的任务
    getProgressCountByType, // 按类型获取任务数量
    getCategorizedStats, // 获取分类统计

    // 新增：持久化转换任务面板（多引擎队列）
    conversionTasks, // 后端持久化任务列表
    refreshConversionTasks, // 刷新任务列表
    retryConversionTask, // 重试指定任务
    cancelConversionTask, // 取消指定任务
    getConversionEngineStatus, // 查询强引擎(MinerU)是否可用
  };
}
