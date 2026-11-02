// 邮件页的键盘快捷键。
//
// 目标是对齐 Outlook / Gmail 的肌肉记忆：重度用户一天要按几十次，缺了这些
// 效率会有断崖式下降。设计上有两条必须遵守的规则：
//
// 1. **输入框里不触发**：用户在写信或搜索时按 `j` 就该输入字母 j。
//    这里统一检查 `event.target` 是否是可编辑元素。
// 2. **有修饰键时不劫持**：`Ctrl+R` 之类的组合要留给浏览器/系统，
//    只在用户明确配置过的组合上 preventDefault。
//
// 键位（与 Outlook 尽量一致）：
//   j / k        下 / 上一封
//   Enter        打开
//   r            回复        a        全部回复        f      转发
//   Ctrl+R       回复        Ctrl+Shift+R  全部回复   Ctrl+N 新邮件
//   Delete / Backspace  删除       e        归档
//   Shift+↑ / ↓  扩展选择    Ctrl+A   全选当前列表   Ctrl+Z  撤销删除
//   s            星标        u        标为未读
//   Ctrl+Enter   发送（在写信框里）
//   Esc          关闭阅读 / 关闭弹层
//   /            聚焦搜索框
import { onBeforeUnmount, onMounted } from 'vue'

/** 判断事件是否发生在「用户正在输入」的地方。 */
function isTyping(event) {
  const target = event.target
  if (!target || target.nodeType !== 1) return false
  const tag = String(target.tagName || '').toUpperCase()
  if (tag === 'TEXTAREA' || tag === 'SELECT') return true
  if (tag === 'INPUT') {
    // 复选框 / 单选 / 按钮不是文本输入：点过「全选」之后焦点停在 checkbox 上，
    // 这时 Ctrl+A 仍然应该全选邮件，而不是被当成「正在输入」直接放行。
    const type = String(target.type || 'text').toLowerCase()
    return !['checkbox', 'radio', 'button', 'submit', 'reset', 'file', 'range', 'color'].includes(type)
  }
  if (target.isContentEditable) return true
  // 富文本编辑器把焦点放在子节点上时 target 可能不是 contenteditable 本身。
  return Boolean(target.closest?.('[contenteditable="true"]'))
}

/**
 * @param {object} handlers 动作表，见文件头的键位说明。
 * @param {object} options
 * @param {import('vue').Ref<boolean>} [options.enabled] 只有邮件页可见时才绑定。
 * @param {(selector: string) => void} [options.focusSearch]
 */
export function useMailShortcuts(handlers, options = {}) {
  const { enabled, focusSearch } = options

  function run(name, event) {
    const fn = handlers[name]
    if (typeof fn !== 'function') return false
    // 处理函数可以返回 false 表示「这次不吃这个键」——例如 Ctrl+A 在阅读区
    // 应该交给浏览器选中正文，而不是全选邮件列表。
    if (fn(event) === false) return false
    event.preventDefault()
    return true
  }

  function onKeydown(event) {
    if (enabled && !enabled.value) return
    // 输入框里的按键一律放行：这是「不劫持」的底线。
    if (isTyping(event)) {
      // 唯一的例外是 Esc：写信框里按 Esc 应该关闭（由编辑器自己处理），
      // 这里不介入，避免和编辑器的「取消」语义打架。
      return
    }
    const key = String(event.key || '')
    const lower = key.toLowerCase()
    const mod = event.ctrlKey || event.metaKey

    if (mod) {
      switch (lower) {
        case 'r':
          run(event.shiftKey ? 'replyAll' : 'reply', event)
          return
        case 'n':
          run('compose', event)
          return
        case 'a':
          // 阅读区里的 Ctrl+A 是「选中这段正文」，不是「全选邮件」。
          // 列表 / 复选框上的焦点则应该全选整个文件夹（包括点过全选之后）。
          if (event.target?.closest?.('.mail-reader')) return
          run('selectAll', event)
          return
        case 'z':
          run('undo', event)
          return
        case 'enter':
          run('send', event)
          return
        default:
          return
      }
    }
    // 无修饰键的组合
    switch (key) {
      case 'j':
      case 'ArrowDown':
        run('next', event)
        break
      case 'k':
      case 'ArrowUp':
        run('prev', event)
        break
      case 'Enter':
        run('open', event)
        break
      case 'r':
        run('reply', event)
        break
      case 'a':
        run('replyAll', event)
        break
      case 'f':
        run('forward', event)
        break
      case 'e':
        run('archive', event)
        break
      case 's':
        run('star', event)
        break
      case 'u':
        run('unread', event)
        break
      case 'Delete':
      case 'Backspace':
        run('delete', event)
        break
      case 'Escape':
        run('escape', event)
        break
      case '/':
        if (focusSearch) {
          event.preventDefault()
          focusSearch()
        }
        break
      case '?':
        run('help', event)
        break
      default:
        break
    }
  }

  onMounted(() => document.addEventListener('keydown', onKeydown))
  onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))
}

/** 快捷键清单（帮助面板与 tooltip 共用，避免两处文案走样）。 */
export const SHORTCUT_LIST = [
  { keys: 'J / K', label: '下一封 / 上一封' },
  { keys: 'Enter', label: '打开选中的邮件' },
  { keys: 'R', label: '回复' },
  { keys: 'A', label: '全部回复' },
  { keys: 'F', label: '转发' },
  { keys: 'Ctrl+R / Ctrl+Shift+R', label: '回复 / 全部回复' },
  { keys: 'Ctrl+N', label: '新邮件' },
  { keys: 'Ctrl+Enter', label: '发送（在写信框）' },
  { keys: 'S', label: '加 / 取消星标' },
  { keys: 'U', label: '标为未读' },
  { keys: 'E', label: '归档' },
  { keys: 'Delete', label: '删除（选中多封时删整批）' },
  { keys: 'Shift + ↑ / ↓', label: '扩展选择' },
  { keys: 'Shift + 点击', label: '连续选择一段邮件' },
  { keys: 'Ctrl+A', label: '全选当前列表' },
  { keys: 'Ctrl+Z', label: '撤销上一步（删除 5 秒内）' },
  { keys: '/', label: '聚焦搜索框' },
  { keys: 'Alt + 划词', label: '划词翻译（正文与写信正文都可用）' },
  { keys: 'Esc', label: '关闭阅读 / 弹层' },
  { keys: '?', label: '显示快捷键帮助' },
]
