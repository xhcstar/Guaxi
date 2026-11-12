// 删除确认弹窗（应用内）。
//
// 为什么不用 `@tauri-apps/plugin-dialog` 的 confirm：那是**系统原生**对话框，
// 无法换主题、无法放图标与自定义文案，与界面其余部分（naive-ui 深色主题）割裂。
//
// 为什么不用 `useDialog()`：应用根节点只挂了 `<n-message-provider>`，没有
// `<n-dialog-provider>`，`useDialog()` 会直接抛错（DictionaryManager 里踩过同一个坑）。
// 所以这里用 `NModal preset="dialog"` 自己封一个，主题仍由外层 config-provider 决定。
//
// 语义：删除一律进系统回收站，因此文案要说「移到回收站」而不是「不可撤销」——
// 用户能在文件管理器里还原，这决定了按钮措辞与语气。
import { defineComponent, h } from 'vue'
import { NModal } from 'naive-ui'

// 弹窗的少量外观修正。
//
// 为什么需要：`preset="dialog"` 用的是 naive-ui 的默认尺寸（圆角 3px、按钮 28px 高），
// 放在本应用里比周围的卡片（8px 圆角）更硬。这里只调整尺寸与圆角，
// 颜色、明暗主题、字体全部交给 n-config-provider 与全局浮层规则，不在这里写死任何颜色。
const DIALOG_SCOPE_CLASS = 'guaxi-confirm-dialog'

const ensureScopedStyle = () => {
  if (typeof document === 'undefined') return
  if (document.getElementById('guaxi-confirm-dialog-style')) return
  const style = document.createElement('style')
  style.id = 'guaxi-confirm-dialog-style'
  style.textContent = `
.${DIALOG_SCOPE_CLASS}.n-dialog { border-radius: 8px; }
.${DIALOG_SCOPE_CLASS} .n-dialog__action { margin-top: 18px; }
.${DIALOG_SCOPE_CLASS} .guaxi-confirm-dialog__text { font-size: 14px; }
.${DIALOG_SCOPE_CLASS} .guaxi-confirm-dialog__positive.n-button { height: 32px; padding: 0 18px; border-radius: 6px; }
.${DIALOG_SCOPE_CLASS} .guaxi-confirm-dialog__negative.n-button { height: 32px; padding: 0 18px; border-radius: 6px; }
`
  document.head.appendChild(style)
}

export default defineComponent({
  name: 'GuaxiConfirmDialog',
  props: {
    show: { type: Boolean, default: false },
    title: { type: String, default: '确认操作' },
    content: { type: String, default: '' },
    /** 说明性副文案（例如「可以在系统回收站中还原」）。 */
    hint: { type: String, default: '' },
    positiveText: { type: String, default: '确定' },
    negativeText: { type: String, default: '取消' },
    /** naive-ui 的 dialog 语义色：warning / error / info / success。 */
    type: { type: String, default: 'warning' },
    positiveDisabled: { type: Boolean, default: false },
  },
  emits: ['update:show', 'positive', 'negative'],
  setup(props, { emit }) {
    const close = () => emit('update:show', false)
    const onPositive = () => {
      close()
      emit('positive')
    }
    const onNegative = () => {
      close()
      emit('negative')
    }
    // 设计说明（关键 · 这是实测抓到的 bug）：
    // NModal 的 `preset="dialog"` 里，`content` 属性渲染的是 dialog 的**默认插槽**。
    // 一旦我们给 `default` 插槽传了 hint，就会把 content 顶掉 —— 弹窗上只剩副文案，
    // 真正要确认的那句「确定删除「xxx」吗？」反而消失。
    // 因此正文与副文案一起交给 default 插槽渲染，不再使用 content 属性。
    // 排版说明：正文直接继承 `.n-dialog__content` 的文字色（naive-ui 按明暗主题给的
    // --n-text-color-2），副文案用透明度降一级。两处都不写死颜色，主题切换自动跟随。
    const renderBody = () => h('div', { class: 'guaxi-confirm-dialog__body' }, [
      props.content
        ? h('div', {
          class: 'guaxi-confirm-dialog__text',
          style: { lineHeight: '1.7', wordBreak: 'break-word' },
        }, props.content)
        : null,
      props.hint
        ? h('div', {
          class: 'guaxi-confirm-dialog__hint',
          style: {
            marginTop: props.content ? '8px' : '0',
            fontSize: '12px',
            lineHeight: '1.6',
            opacity: '0.72',
          },
        }, props.hint)
        : null,
    ])
    return () => {
      ensureScopedStyle()
      return h(
        NModal,
        {
          show: props.show,
          'onUpdate:show': (v) => emit('update:show', v),
          class: DIALOG_SCOPE_CLASS,
          preset: 'dialog',
          type: props.type,
          title: props.title,
          positiveText: props.positiveText,
          negativeText: props.negativeText,
          positiveButtonProps: {
            disabled: props.positiveDisabled,
            class: 'guaxi-confirm-dialog__positive',
          },
          negativeButtonProps: {
            class: 'guaxi-confirm-dialog__negative',
          },
          maskClosable: false,
          autoFocus: false,
          onPositiveClick: onPositive,
          onNegativeClick: onNegative,
          onClose: onNegative,
          onMaskClick: onNegative,
          onEsc: onNegative,
        },
        (props.content || props.hint) ? { default: renderBody } : undefined,
      )
    }
  },
})
