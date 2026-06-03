import { defineStore } from "pinia";
import { createLogger } from "../utils/logger";

const logger = createLogger("mathWorkspaceStore");

// 设计说明：
// 1. 这是「数学块」承载层（右侧面板 / 独立工作台）专用的独立 store，
//    与 Python Cell 的 rightSiderStore.pythonSessionInspector 完全解耦——
//    互不读写、互不复用，仅同样借用右侧区域与新窗口这两处通用位置。
// 2. inspector 描述「当前选中的数学块」及其会话里的工作区符号 / 可复跑 carrier，
//    数据由 GuaxiNotebookView 在运行 / 选中数学块时通过一次无状态 python_exec 探测后写入。
const EMPTY_INSPECTOR = {
  filePath: "",
  cellId: "",
  cellKey: "",
  stableId: "",
  cellTitle: "",
  sessionId: "",
  hasActiveCell: false,
  workspaceSymbols: [], // [{ name, type }]
  carriers: [], // [{ key, kind, command, label, params, summary, solutionVar, seq }]
  updatedAt: 0,
};

export const useMathWorkspaceStore = defineStore("mathWorkspace", {
  state: () => ({
    inspector: { ...EMPTY_INSPECTOR },
  }),
  actions: {
    setInspector(payload) {
      const next = payload && typeof payload === "object" ? payload : {};
      this.inspector = {
        filePath: String(next.filePath || ""),
        cellId: String(next.cellId || ""),
        cellKey: String(next.cellKey || ""),
        stableId: String(next.stableId || ""),
        cellTitle: String(next.cellTitle || ""),
        sessionId: String(next.sessionId || ""),
        hasActiveCell: !!next.hasActiveCell,
        workspaceSymbols: Array.isArray(next.workspaceSymbols) ? next.workspaceSymbols : [],
        carriers: Array.isArray(next.carriers) ? next.carriers : [],
        updatedAt: Date.now(),
      };
      logger.debug("[MathWorkspaceStore] set inspector", {
        cellId: this.inspector.cellId,
        hasActiveCell: this.inspector.hasActiveCell,
        symbolCount: this.inspector.workspaceSymbols.length,
        carrierCount: this.inspector.carriers.length,
      });
    },
    clear() {
      this.inspector = { ...EMPTY_INSPECTOR, updatedAt: Date.now() };
      logger.info("[MathWorkspaceStore] cleared inspector");
    },
  },
});
