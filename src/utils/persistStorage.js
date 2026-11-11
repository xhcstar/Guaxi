// localStorage 的落盘镜像。
//
// 正式版的 localStorage 住在 WebView2 的数据目录（%LOCALAPPDATA%\<identifier>\EBWebView），
// 卸载程序勾了"删除应用数据"、或者换一个安装方式，这一整块就没了 —— 表现是界面偏好、
// 收藏的聚合源、助手会话全部回默认。这里把有长期价值的那部分 key 额外写一份 JSON 到
// 用户数据根（%APPDATA%\Guaxi\state），启动时补回 localStorage。
//
// 只镜像白名单里的 key：monaco / excalidraw / 各类 cache 体积大且可重建，不碰。

import { invoke } from '@tauri-apps/api/core'

const MIRROR_PATTERNS = [
  /^guaxi[:.]/i,
  /^guaxi_/i,
  /^flipboard_/i,
  /^searchHistory$/,
  /^searchPresets$/,
  /^rh_recent$/,
]

// 单值超过这个体积就不镜像：localStorage 里的大 blob 基本是可重建的缓存。
const MAX_VALUE_BYTES = 256 * 1024

const isTauri = () => typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__)

function shouldMirror(key) {
  if (!key || typeof key !== 'string') return false
  return MIRROR_PATTERNS.some((pattern) => pattern.test(key))
}

let flushTimer = 0
let patched = false

function collect() {
  const data = {}
  for (let i = 0; i < localStorage.length; i += 1) {
    const key = localStorage.key(i)
    if (!shouldMirror(key)) continue
    const value = localStorage.getItem(key)
    if (value == null || value.length > MAX_VALUE_BYTES) continue
    data[key] = value
  }
  return data
}

function flush() {
  if (!isTauri()) return
  try {
    const json = JSON.stringify(collect())
    invoke('storage_snapshot_write', { json }).catch(() => {
      /* 落盘失败不影响界面，下次变更还会再试 */
    })
  } catch {
    /* 序列化失败（脏数据）时静默跳过 */
  }
}

function schedule() {
  clearTimeout(flushTimer)
  flushTimer = setTimeout(flush, 1200)
}

/// 启动时把落盘的快照补回 localStorage。
/// 只补缺失的 key：本次运行期间用户已经改过的值优先级更高。
export async function restorePersistedState() {
  if (!isTauri()) return
  try {
    // 首屏不该等一个读文件：800ms 还没回来就按没有快照处理。
    const raw = await Promise.race([
      invoke('storage_snapshot_read'),
      new Promise((resolve) => setTimeout(() => resolve(null), 800)),
    ])
    if (!raw || typeof raw !== 'string') return
    const data = JSON.parse(raw)
    if (!data || typeof data !== 'object') return
    for (const [key, value] of Object.entries(data)) {
      if (typeof value !== 'string' || !shouldMirror(key)) continue
      if (localStorage.getItem(key) !== null) continue
      localStorage.setItem(key, value)
    }
  } catch {
    /* 读不到快照就按首次启动处理 */
  }
}

/// 挂上写入监听。localStorage 是共享的 Storage 原型，patch 一次即可。
export function installStorageMirror() {
  if (!isTauri() || patched) return
  patched = true
  try {
    const proto = Object.getPrototypeOf(localStorage)
    const originalSet = proto.setItem
    const originalRemove = proto.removeItem
    const originalClear = proto.clear

    proto.setItem = function patchedSetItem(key, value) {
      originalSet.call(this, key, value)
      if (shouldMirror(key)) schedule()
    }
    proto.removeItem = function patchedRemoveItem(key) {
      originalRemove.call(this, key)
      if (shouldMirror(key)) schedule()
    }
    proto.clear = function patchedClear() {
      originalClear.call(this)
      schedule()
    }
  } catch {
    // 某些 WebView 不允许改写 Storage 原型：退化成定时快照。
    patched = false
  }

  // 兜底：页面隐藏/关闭前再刷一次，另外每 60 秒兜一下漏网之鱼。
  const flushOnHide = () => {
    if (document.visibilityState === 'hidden') flush()
  }
  document.addEventListener('visibilitychange', flushOnHide)
  window.addEventListener('beforeunload', flush)
  setInterval(flush, 60_000)
}
