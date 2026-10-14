import { ref, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/** 档案变更广播事件名（后端在 ssh_save_profile / ssh_delete_profile 里发出）。 */
export const SSH_PROFILES_CHANGED = 'ssh-profiles-changed'

/**
 * SSH 档案列表，**会自动跟随变更**。
 *
 * 为什么需要它：终端窗口的各个标签页是常驻的（`display-directive="show:lazy"`），
 * 组件只在挂载时读一次档案；在「实时终端」里新建/删除连接后，GDB、工具箱、命令下发
 * 那几个页签不重新挂载，就会一直显示旧列表（"新增的会话在这里找不到，除非重开窗口"）。
 *
 * 所以：挂载时加载一次 + 监听后端的档案变更广播，变更即重载。
 * 事件是全局广播，跨窗口（应用独立窗口 ↔ 主窗口）也生效。
 *
 * @param {{ error?: Function }} [logger] 可选日志器（项目里由 createLogger 提供）
 * @returns {{ profiles: import('vue').Ref<Array>, reload: () => Promise<void> }}
 */
export function useSshProfiles(logger) {
  const profiles = ref([])
  let unlisten = null
  let disposed = false

  async function reload() {
    try {
      profiles.value = await invoke('ssh_list_profiles')
    } catch (e) {
      logger?.error?.('list ssh profiles failed', e)
    }
  }

  onMounted(async () => {
    await reload()
    const off = await listen(SSH_PROFILES_CHANGED, () => { if (!disposed) reload() })
    // 注册期间组件可能已经卸载：那就立刻解绑，别把监听留在全局。
    if (disposed) off()
    else unlisten = off
  })

  onBeforeUnmount(() => {
    disposed = true
    unlisten?.()
    unlisten = null
  })

  return { profiles, reload }
}
