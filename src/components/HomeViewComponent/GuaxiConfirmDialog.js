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
    return () => h(
      NModal,
      {
        show: props.show,
        'onUpdate:show': (v) => emit('update:show', v),
        preset: 'dialog',
        type: props.type,
        title: props.title,
        content: props.content,
        positiveText: props.positiveText,
        negativeText: props.negativeText,
        positiveButtonProps: { disabled: props.positiveDisabled },
        maskClosable: false,
        autoFocus: false,
        onPositiveClick: onPositive,
        onNegativeClick: onNegative,
        onClose: onNegative,
        onMaskClick: onNegative,
        onEsc: onNegative,
      },
      props.hint ? { default: () => h('div', {
        style: {
          marginTop: '8px',
          fontSize: '12px',
          lineHeight: '1.6',
          opacity: '0.72',
        },
      }, props.hint) } : undefined,
    )
  },
})
