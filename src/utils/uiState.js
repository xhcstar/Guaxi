/**
 * 面板级 UI 状态的本地记忆。
 *
 * 用途：让各面板"记住上次的选择"（档案、模式、源码根、分栏宽度、字号…），
 * 下次开窗接着用。**不含任何凭据**（密码/口令一律不落这里）。
 *
 * 为什么用 localStorage 而不是后端文件：本仓库已有 34 处这么用（如
 * `GuaxiDiffPanel` 记分栏比例、`ResumeBuilderApp` 记模板），各窗口同源共享一份，
 * 不需要 IPC 往返，也不需要新增命令。
 *
 * 写入按 JSON 编码（布尔/数字才能原样还原）；读取带默认值与可选校验，
 * 任何异常（隐私模式禁用、脏数据、类型不符）一律退回默认值，绝不抛给调用方。
 */

/** 读取；`validate` 返回 false 时视为无效，退回 `fallback`。 */
export function readUiState(key, fallback, validate) {
  try {
    const raw = localStorage.getItem(key)
    if (raw === null) return fallback
    const value = JSON.parse(raw)
    if (typeof validate === 'function' && !validate(value)) return fallback
    return value
  } catch {
    return fallback
  }
}

/** 写入；存不下（配额/禁用）只影响"下次回到默认"，不影响当前使用。 */
export function persistUiState(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    /* 记不住就下次用默认值 */
  }
}

/** 常用校验器：限定在若干取值内。 */
export const oneOf = (...allowed) => (v) => allowed.includes(v)

/** 常用校验器：数值落在闭区间内。 */
export const within = (min, max) => (v) => Number.isFinite(v) && v >= min && v <= max
