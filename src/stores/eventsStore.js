import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { createLogger } from "../utils/logger";

const logger = createLogger("eventsStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

export const useEventStore = defineStore("event", {
  state: () => ({
    eventQueues: new Map(),
    subscribers: new Map(),
    listeners: new Map(),
    _initialized: false, // 新增初始化状态标志
  }),

  actions: {
    /**
     * 初始化全局事件监听（安全版本）
     */
    async initGlobalListeners() {
      if (this._initialized) {
        console.warn("事件系统已初始化，跳过重复初始化");
        return;
      }

      try {
        // 预注册核心事件类型
        await this._registerCoreEventTypes();
        this._initialized = true;
        console.log("全局事件系统初始化完成");
      } catch (error) {
        console.error("全局事件初始化失败:", error);
        throw error;
      }
    },

    /**
     * 内部方法：注册核心事件类型
     */
    async _registerCoreEventTypes() {
      const coreEvents = ["add_web_clipper_file_event"];

      await Promise.all(
        coreEvents.map(async (eventType) => {
          if (this.listeners.has(eventType)) return;

          try {
            const unlisten = await listen(eventType, (event) => {
              console.debug(`[Event] ${eventType}`, event.payload);

              // 标准化payload处理
              const payload = event.payload || {};
              if (typeof payload !== "object") {
                console.warn(`非对象格式的payload: ${typeof payload}`, payload);
                return;
              }

              this._handleEvent(eventType, payload);
            });

            this.listeners.set(eventType, unlisten);
            this.subscribers.set(eventType, new Set());
            this.eventQueues.set(eventType, []);
          } catch (error) {
            console.error(`注册事件 ${eventType} 失败:`, error);
            throw error;
          }
        })
      );
    },

    /**
     * 处理事件（增强版）
     */
    _handleEvent(eventType, payload) {
      const subscribers = this.subscribers.get(eventType);
      const queue = this.eventQueues.get(eventType);

      if (!subscribers || !queue) {
        console.error(`未注册的事件类型: ${eventType}`);
        return;
      }

      // 有订阅者立即处理
      if (subscribers.size > 0) {
        subscribers.forEach((callback) => {
          try {
            callback(payload);
          } catch (error) {
            console.error(`事件回调执行错误 (${eventType}):`, error);
          }
        });
      }
      // 无订阅者则缓存（限制最大队列长度）
      else if (queue.length < 100) {
        queue.push(payload);
        console.debug(`事件入队: ${eventType} (当前队列: ${queue.length})`);
      } else {
        console.warn(`事件队列已满 (${eventType})，丢弃事件`);
      }
    },

    /**
     * 安全订阅方法
     */
    subscribe(eventType, callback) {
      if (typeof callback !== "function") {
        throw new Error("回调必须是函数");
      }

      // 自动注册未知事件类型
      if (!this.subscribers.has(eventType)) {
        console.warn(`自动注册新事件类型: ${eventType}`);
        this.registerEventType(eventType).catch(console.error);
      }

      const subscriberSet = this.subscribers.get(eventType);
      subscriberSet.add(callback);

      // 处理积压事件（带错误处理）
      const queue = this.eventQueues.get(eventType);
      while (queue.length > 0) {
        try {
          callback(queue.shift());
        } catch (error) {
          console.error("处理积压事件失败:", error);
        }
      }

      return () => {
        subscriberSet.delete(callback);
        console.debug(`取消订阅: ${eventType}`);
      };
    },

    /**
     * 注册新事件类型（增强版）
     */
    async registerEventType(eventType) {
      if (this.listeners.has(eventType)) {
        console.debug(`事件类型已注册: ${eventType}`);
        return;
      }

      try {
        const unlisten = await listen(eventType, (event) => {
          this._handleEvent(eventType, event.payload || {});
        });

        this.listeners.set(eventType, unlisten);
        this.subscribers.set(eventType, new Set());
        this.eventQueues.set(eventType, []);

        console.log(`成功注册事件类型: ${eventType}`);
      } catch (error) {
        console.error(`注册事件类型失败 (${eventType}):`, error);
        throw error;
      }
    },

    /**
     * 安全清理方法
     */
    async cleanup() {
      const cleanupTasks = [];

      for (const [eventType, unlisten] of this.listeners) {
        cleanupTasks.push(
          unlisten().catch((error) => {
            console.error(`取消监听失败 (${eventType}):`, error);
          })
        );
      }

      await Promise.all(cleanupTasks);

      this.listeners.clear();
      this.subscribers.clear();
      this.eventQueues.clear();
      this._initialized = false;

      console.log("事件系统已完全清理");
    },
  },
});
