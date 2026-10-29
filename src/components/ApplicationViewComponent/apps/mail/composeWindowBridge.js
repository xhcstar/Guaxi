// 写信窗口与主窗口之间的桥。
//
// 独立窗口是一个**新的 webview**：它有自己的 pinia、自己的 mailStore，看不到主窗口
// 已经加载的账户、模板与设置。所以这里不共享 store，而是：
// 1. 从后端取「任务」（草稿内容由 `mail_take_compose_request` 交接）；
// 2. 取窗口需要用到的只读数据（账户、设置、模板）；
// 3. 保存 / 发送 / 关闭都直接调后端命令，完成后通知主窗口刷新。
//
// 这样两个窗口之间没有共享可变状态，也就不存在「主窗口改了设置、写信窗口看不到」
// 这类同步问题——每次开窗都是现读。
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../../../utils/logger'

const logger = createLogger('MailComposeWindow')

/** 窗口当前这份草稿（结构与主窗口 `mailStore.composing` 一致）。 */
export const composeDraft = ref(null)
/** 账户 / 设置 / 模板：开窗时读一次。 */
export const accounts = ref([])
export const settings = ref({})
export const templates = ref([])
export const options = ref({})
export const loading = ref(true)
export const error = ref('')

export const ready = computed(() => Boolean(composeDraft.value) && !loading.value)

let currentLabel = ''

/** 把后端交接的 ComposeRequest 转成组件用的 draft。 */
function toDraft(request) {
  return {
    mode: request.mode || 'new',
    accountId: request.accountId || accounts.value[0]?.id || '',
    to: request.to || [],
    cc: request.cc || [],
    bcc: request.bcc || [],
    subject: request.subject || '',
    text: request.text || '',
    html: request.html || '',
    htmlMode: request.htmlMode !== false,
    attachments: (request.attachments || []).map((path) => ({ path, name: path.split(/[\\/]/).pop() })),
    inlineImages: (request.inlineImages || []).map((img) => ({
      contentId: img.contentId,
      path: img.path,
      name: img.name || '',
    })),
    cryptoMode: '',
    inReplyTo: request.inReplyTo || null,
    references: request.references || null,
    draftUid: request.draftUid ?? null,
    draftFolder: request.draftFolder || '',
    showCc: (request.cc || []).length > 0,
    showBcc: (request.bcc || []).length > 0,
    sourceMode: Boolean(request.sourceMode),
  }
}

/** 取一次窗口的初始化数据 + 待取草稿。 */
export async function loadComposeWindow(label) {
  currentLabel = label || currentLabel
  loading.value = true
  error.value = ''
  try {
    const [accountList, mailSettings, templateList, request] = await Promise.all([
      invoke('mail_list_accounts').catch(() => []),
      invoke('mail_get_settings').catch(() => ({})),
      invoke('mail_list_templates').catch(() => []),
      invoke('mail_take_compose_request', { label: currentLabel }).catch(() => null),
    ])
    accounts.value = accountList || []
    settings.value = mailSettings || {}
    templates.value = templateList || []
    if (!request) {
      error.value = '没有找到要编辑的草稿。窗口可能已经过期，请从主窗口重新打开。'
      composeDraft.value = null
      return
    }
    composeDraft.value = toDraft(request)
    options.value = request
  } catch (err) {
    logger.error('读取写信数据失败', err)
    error.value = String(err?.message || err)
  } finally {
    loading.value = false
  }
}

/** 窗口已经开着时主窗口再发请求：重新取一次。 */
export function reloadComposeRequest() {
  return loadComposeWindow(currentLabel)
}

/** 组件对草稿的修改：就地合并。 */
export function patchDraft(patch) {
  if (!composeDraft.value) return
  Object.assign(composeDraft.value, patch)
}

/** 组件需要的地址提示（来自通讯录 + 历史地址）。 */
export async function suggestAddress(prefix) {
  try {
    return (await invoke('mail_contact_suggest', { prefix })) || []
  } catch {
    return []
  }
}

/** 附件检查（体积 / 危险类型）。 */
export async function checkAttachments(paths) {
  try {
    return (await invoke('mail_check_attachments', { paths })) || []
  } catch {
    return []
  }
}

const ILLEGAL = new Set([
  String.fromCharCode(92), '/', ':', '*', '?', '"', '<', '>', '|',
  String.fromCharCode(13), String.fromCharCode(10),
])

function fileStem(text) {
  return [...String(text || '邮件')]
    .map((ch) => (ILLEGAL.has(ch) ? '_' : ch))
    .join('')
    .slice(0, 80)
}

/** 把窗口里的 draft 转成后端 `mail_send` / `mail_save_draft` 的请求体。 */
function draftRequest(draft, { saveToSent = true } = {}) {
  return {
    accountId: draft.accountId,
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    text: draft.text,
    html: draft.htmlMode ? draft.html : null,
    inReplyTo: draft.inReplyTo,
    references: draft.references,
    attachments: (draft.attachments || []).map((a) => a.path),
    inlineImages: (draft.inlineImages || []).map((img) => ({
      contentId: img.contentId,
      path: img.path,
      filename: img.name || '',
    })),
    cryptoMode: draft.cryptoMode || '',
    saveToSent,
  }
}

/** 发送（或定时 / 延迟发送）。返回 `{ sent }` / `{ queued }`。 */
export async function sendDraft({ useOutbox = false, scheduledAt = 0 } = {}) {
  const draft = composeDraft.value
  if (!draft) return null
  const request = draftRequest(draft, { saveToSent: true })
  if (scheduledAt || (useOutbox && Number(settings.value?.undoSendSeconds) > 0)) {
    const sendAt = scheduledAt ? Math.floor(Number(scheduledAt) / 1000) : null
    const item = await invoke('mail_queue_send', { request, sendAt })
    await discardDraftsOf(draft)
    composeDraft.value = null
    return { queued: item, scheduled: Boolean(scheduledAt) }
  }
  await invoke('mail_send', { request })
  await discardDraftsOf(draft)
  composeDraft.value = null
  return { sent: true }
}

/** 编辑器定时自动保存：真正写进草稿箱（用户在别的客户端也能看到这封半成品）。 */
export async function saveDraftToMailbox(request) {
  if (!request) return null
  return invoke('mail_save_draft', { request })
}

/** 存草稿。只要还开着一封编辑中的邮件就写一条，哪怕是空的（还能继续编辑）。 */
export async function saveDraft({ syncFolder = true } = {}) {
  const draft = composeDraft.value
  if (!draft) return false
  await invoke('mail_save_draft', {
    request: draftRequest(draft, { saveToSent: false }),
    // 关窗时不等同步：APPEND 成功就算存好了，同步只为拿 UID。
    syncAfterAppend: syncFolder,
  })
  await discardDraftsOf(draft)
  return true
}

/**
 * 写回 / 发送成功后，把这条内容在草稿箱里的旧样子清掉。
 *
 * 两个来源：
 * - `draftUid` / `draftFolder`：这封是从草稿箱续写的，原稿不能留；
 * - `mailboxDraft`：编辑器自动保存的那份，留着就会和最终版本重复。
 */
async function discardDraftsOf(draft) {
  if (!draft) return
  const targets = []
  if (draft.draftUid && draft.draftFolder) {
    targets.push({ folder: draft.draftFolder, uid: draft.draftUid })
  }
  const auto = draft.mailboxDraft
  if (auto?.uid && auto?.folder) {
    targets.push({ folder: auto.folder, uid: auto.uid })
    draft.mailboxDraft = null
  }
  for (const target of targets) {
    try {
      await invoke('mail_discard_draft', {
        accountId: draft.accountId,
        folder: target.folder,
        uid: target.uid,
      })
    } catch (err) {
      logger.warn('删除草稿失败', err)
    }
  }
}

/**
 * 关闭窗口。
 *
 * 先把内容存进草稿箱（有内容时），再通知主窗口刷新，最后真正关窗。
 * 「不论什么原因关窗，只要没发出去就留一份草稿」是这里的硬约束。
 */
export async function closeWindow({ save = true } = {}) {
  let saved = false
  if (save) {
    try {
      // 关窗前这一次不等草稿箱同步：否则「保存并关闭」要等网络往返。
      saved = await saveDraft({ syncFolder: false })
    } catch (err) {
      logger.error('关窗前保存草稿失败', err)
    }
  }
  await invoke('mail_compose_closed', { label: currentLabel, saved }).catch(() => {})
  await invoke('mail_drop_compose_request', { label: currentLabel }).catch(() => {})
  await invoke('mail_close_compose_window', { label: currentLabel }).catch(() => {
    window.close()
  })
}
