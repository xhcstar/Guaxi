// 统一的插件运行器:加载插件入口脚本、调用 run(ctx)、返回结构化结果。
// editor/repo/knowledge-base 各作用面都用它,只是 ctx 形状与结果渲染不同。
import { invoke } from '@tauri-apps/api/core'

const SCAFFOLD = [
  'import importlib.util as _ilu',
  '_spec = _ilu.spec_from_file_location("guaxi_user_plugin", _plugin_entry_path)',
  '_mod = _ilu.module_from_spec(_spec)',
  '_spec.loader.exec_module(_mod)',
  '_result = _mod.run(_plugin_ctx)',
].join('\n')

/**
 * 运行一个插件。
 * @param {string} entryPath 插件入口 .py 的绝对路径(来自 list_all_plugins 的 entry_path)
 * @param {object} ctx 传给 run(ctx) 的上下文(经 python_exec vars 注入为全局变量)
 * @returns {Promise<object>} 插件返回的 result_json(如 {text} / {report} / ...)
 */
export async function runGuaxiPlugin(entryPath, ctx = {}) {
  const res = await invoke('python_exec', {
    request: {
      code: SCAFFOLD,
      persist: false,
      expect_json: true,
      include_session_vars: false,
      vars: {
        _plugin_entry_path: String(entryPath || ''),
        _plugin_ctx: ctx || {},
      },
    },
  })
  if (!res?.ok) {
    throw new Error(String(res?.stderr || '插件运行失败').slice(0, 600))
  }
  return res?.result_json || {}
}

/** 拉取指定作用面、已启用的插件列表。 */
export async function listPluginsForSurface(surface) {
  const all = (await invoke('list_all_plugins')) || []
  return all.filter(
    (p) => p.enabled && Array.isArray(p.surfaces) && p.surfaces.includes(surface)
  )
}
