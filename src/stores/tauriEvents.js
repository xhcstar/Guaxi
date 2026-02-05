import { useEventStore } from "./eventsStore";
import { createLogger } from "../utils/logger";

const logger = createLogger("tauriEvents");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

/**
 * 初始化Tauri事件监听系统
 */
export const initTauriEvents = async () => {
  const eventStore = useEventStore();
  await eventStore.initGlobalListeners();

  // 在开发环境下打印初始化信息
  if (import.meta.env.DEV) {
    console.log("Tauri事件系统初始化完成");
  }
};

/**
 * 清理Tauri事件监听系统
 */
export const cleanupTauriEvents = async () => {
  const eventStore = useEventStore();
  await eventStore.cleanup();

  if (import.meta.env.DEV) {
    console.log("Tauri事件系统已清理");
  }
};
