import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { createLogger } from '../../../utils/logger'

const logger = createLogger('useFileOperations')
export function useFileOperations({ store, message }) {
  // 备份数据库
  const backupDatabase = async (scope = "current") => {
    try {
      message.info("开始备份资料库...");
      const backupDir = await open({ directory: true, multiple: false });
      if (!backupDir) {
        message.warning("请选择备份目录");
        return;
      }
      const loading = null;
      const databaseType = scope === "all" ? "ALL" : store.currentDatabase;
      invoke(
        "backup_database",
        {
          databaseType,
          outputDir: backupDir || null,
        },
        {
          target: "main",
        }
      )
        .then((result) => {
          if (result.success) {
            message.success("资料库备份成功");
            logger.info("备份文件路径:", result.backupPath);
          } else {
            throw new Error(result.error || "备份失败");
          }
        })
        .catch((error) => {
          logger.error("备份失败:", error);
          message.error(`资料库备份失败: ${error.message || error}`);
        });
    } catch (error) {
      logger.error("备份失败:", error);
      message.error(`资料库备份失败: ${error.message || error}`);
    }
  };

  // 恢复数据库
  const recoverDatabase = async () => {
    try {
      message.info("开始从备份文件中恢复...");
      const backupPath = await open({ multiple: false, directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
      if (!backupPath) {
        message.warning("请选择备份文件");
        return;
      }
      const result = await invoke(
        "recover_database",
        {
          databaseType: store.currentDatabase,
          backupPath: backupPath,
        },
        {
          target: "main",
        }
      );
      if (result.success) {
        message.success("资料库恢复成功");
        await store.fetchFiles(store.currentDatabase);
        await store.fetchRepos();
      } else {
        throw new Error(result.error || "恢复失败");
      }
    } catch (error) {
      logger.error("恢复失败:", error);
      message.error(`资料库恢复失败: ${error.message || error}`);
    }
  };

  // 导入条目
  const importItems = async () => {
    try {
      message.info("开始导入指定条目...");
      const importPath = await open({ multiple: false });
      if (!importPath) {
        message.warning("请选择导入包或目录");
        return;
      }
      const result = await invoke("import_items", {
        databaseType: store.currentDatabase,
        importPath: importPath,
      });
      if (result.success) {
        message.success(`成功导入 ${result.importedCount} 个条目`);
        await store.fetchFiles(store.currentDatabase);
      } else {
        throw new Error(result.error || "导入失败");
      }
    } catch (error) {
      logger.error("导入失败:", error);
      message.error(`条目导入失败: ${error.message || error}`);
    }
  };

  // 导出条目
  const exportItems = async (itemIds) => {
    try {
      message.info("开始导出指定条目...");
      if (!itemIds || itemIds.length === 0) {
        message.warning("请选择要导出的条目");
        return;
      }
      const exportPath = await open({ directory: true, multiple: false });
      if (!exportPath) {
        message.warning("请选择导出目录");
        return;
      }
      const result = await invoke("export_items", {
        databaseType: store.currentDatabase,
        exportPath: exportPath,
        itemIds: itemIds,
      });
      if (result.success) {
        message.success(`成功导出 ${result.exportedCount} 个条目`);
        logger.info("导出文件路径:", result.exportPath);
      } else {
        throw new Error(result.error || "导出失败");
      }
    } catch (error) {
      logger.error("导出失败:", error);
      message.error(`条目导出失败: ${error.message || error}`);
    }
  };

  // 批量删除文件
  const batchDeleteFiles = async (fileIds) => {
    try {
      if (!fileIds || fileIds.length === 0) {
        message.warning("请选择要删除的文件");
        return;
      }

      const result = await invoke("batch_delete_files", {
        fileIds: fileIds,
      });

      if (result.success) {
        message.success(`成功删除 ${result.deletedCount} 个文件`);

        // 刷新数据
        await store.fetchFiles(store.currentDatabase);
        await store.fetchRepos();
      } else {
        throw new Error(result.error || "批量删除失败");
      }
    } catch (error) {
      logger.error("批量删除失败:", error);
      message.error(`批量删除失败: ${error.message || error}`);
    }
  };

  // 批量移动文件
  const batchMoveFiles = async (fileIds, targetLibrary) => {
    try {
      if (!fileIds || fileIds.length === 0) {
        message.warning("请选择要移动的文件");
        return;
      }

      if (!targetLibrary) {
        message.warning("请选择目标文件库");
        return;
      }

      const result = await invoke("batch_move_files", {
        fileIds: fileIds,
        targetLibrary: targetLibrary,
      });

      if (result.success) {
        message.success(
          `成功移动 ${result.movedCount} 个文件到 ${targetLibrary}`
        );

        // 刷新数据
        await store.fetchFiles(store.currentDatabase);
      } else {
        throw new Error(result.error || "批量移动失败");
      }
    } catch (error) {
      logger.error("批量移动失败:", error);
      message.error(`批量移动失败: ${error.message || error}`);
    }
  };

  // 批量导出文件
  const batchExportFiles = async (fileIds, exportPath) => {
    try {
      if (!fileIds || fileIds.length === 0) {
        message.warning("请选择要导出的文件");
        return;
      }

      const result = await invoke("batch_export_files", {
        fileIds: fileIds,
        exportPath: exportPath,
      });

      if (result.success) {
        message.success(
          `成功导出 ${result.exportedCount} 个文件到 ${exportPath}`
        );
      } else {
        throw new Error(result.error || "批量导出失败");
      }
    } catch (error) {
      logger.error("批量导出失败:", error);
      message.error(`批量导出失败: ${error.message || error}`);
    }
  };

  // 获取文件统计信息
  const getFileStatistics = async () => {
    try {
      const stats = await invoke("get_file_statistics", {
        databaseType: store.currentDatabase,
      });

      return stats;
    } catch (error) {
      logger.error("获取文件统计信息失败:", error);
      return null;
    }
  };

  // 清理临时文件
  const cleanupTempFiles = async () => {
    try {
      const result = await invoke("cleanup_temp_files");

      if (result.success) {
        message.success(`清理了 ${result.cleanedCount} 个临时文件`);
      } else {
        throw new Error(result.error || "清理失败");
      }
    } catch (error) {
      logger.error("清理临时文件失败:", error);
      message.error(`清理临时文件失败: ${error.message || error}`);
    }
  };

  // 验证文件完整性
  const validateFileIntegrity = async () => {
    try {
      message.info("开始验证文件完整性...");

      const result = await invoke("validate_file_integrity", {
        databaseType: store.currentDatabase,
      });

      if (result.success) {
        if (result.corruptedFiles.length === 0) {
          message.success("所有文件完整性验证通过");
        } else {
          message.warning(`发现 ${result.corruptedFiles.length} 个损坏文件`);
          logger.info("损坏文件列表:", result.corruptedFiles);
        }
        return result;
      } else {
        throw new Error(result.error || "验证失败");
      }
    } catch (error) {
      logger.error("验证文件完整性失败:", error);
      message.error(`验证文件完整性失败: ${error.message || error}`);
      return null;
    }
  };

  const gcDeletedItems = async () => {
    try {
      message.info("开始执行垃圾回收...");
      await invoke("gc_deleted_items");
      message.success("垃圾回收完成");
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      logger.error("垃圾回收失败:", error);
      message.error(`垃圾回收失败: ${error.message || error}`);
    }
  };

  // P2P 在线同步：文件库根目录
  const syncLibrariesRoot = async () => {
    try {
      message.info("开始同步文件库根目录...");
      const result = await invoke("p2p_sync_libraries_root_now");
      message.success(typeof result === "string" ? result : "文件库同步完成");
    } catch (error) {
      logger.error("文件库同步失败:", error);
      message.error(`文件库同步失败: ${error.message || error}`);
    }
  };

  // P2P 在线同步：当前资料库（资源库文件 + 当前 knowledge_repo 的数据库记录）
  const syncCurrentRepo = async () => {
    try {
      message.info("开始同步当前资料库（文件 + 记录）...");
      const knowledgeRepo = store.currentDatabase || null;
      const result = await invoke("p2p_sync_resource_repo_now", { knowledgeRepo });
      message.success(typeof result === "string" ? result : "资料库同步完成");
      // 记录可能已变化，刷新当前库数据
      await store.fetchFiles(store.currentDatabase);
      await store.fetchRepos();
    } catch (error) {
      logger.error("资料库同步失败:", error);
      message.error(`资料库同步失败: ${error.message || error}`);
    }
  };

  return {
    backupDatabase,
    recoverDatabase,
    importItems,
    exportItems,
    batchDeleteFiles,
    batchMoveFiles,
    batchExportFiles,
    getFileStatistics,
    cleanupTempFiles,
    validateFileIntegrity,
    gcDeletedItems,
    syncLibrariesRoot,
    syncCurrentRepo,
  };
}
