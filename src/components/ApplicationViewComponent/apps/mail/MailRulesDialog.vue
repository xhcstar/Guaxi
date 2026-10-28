<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal is-wide mail-rules-modal" role="dialog" aria-label="收信规则">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">收信规则</h2>
        <button class="mail-modal-close" @click="$emit('close')">×</button>
      </div>

      <!-- 两个模块分成两页：发件人名单是「一条规则都不用写」的快速拉黑 / 放行，
           收信规则是条件 + 动作。以前挤在同一列里，加地址的按钮看起来像在给规则加条件。 -->
      <div class="mail-rules-tabs" role="tablist">
        <button
          v-for="tab in TABS"
          :key="tab.key"
          class="mail-rules-tab"
          :class="{ 'is-on': activeTab === tab.key }"
          role="tab"
          :aria-selected="activeTab === tab.key"
          @click="switchTab(tab.key)"
        >
          <MailIcon :name="tab.icon" :size="14" />
          {{ tab.label }}
          <span v-if="tabCount(tab.key)" class="mail-rules-tab-count">{{ tabCount(tab.key) }}</span>
        </button>
      </div>

      <div ref="panelRef" class="mail-modal-body">
        <!-- ============ 收信规则 ============ -->
        <section v-show="activeTab === 'rules'" class="mail-rules-panel">
          <p class="mail-hint" style="margin-top: 0">
            规则在本地同步到新邮件后执行。没有指定文件夹的规则只作用于收件箱。
            想让规则在服务器上生效（关掉客户端也跑），在「外观与设置 → 服务器端规则」里上传。
          </p>

          <div v-for="rule in orderedRules" :key="rule.id" class="mail-rule">
            <label class="mail-check" style="margin: 2px 0 0">
              <input
                type="checkbox"
                :checked="rule.enabled"
                @change="toggleRule(rule, $event.target.checked)"
              />
            </label>
            <div class="mail-rule-main">
              <div class="mail-rule-name">{{ rule.name || '未命名规则' }}</div>
              <div class="mail-rule-desc">{{ describeRule(rule) }}</div>
            </div>
            <button class="mail-btn" @click="editRule(rule)">编辑</button>
            <button class="mail-btn is-danger" @click="removeRule(rule)">删除</button>
          </div>

          <div v-if="!rules.length" class="mail-hint" style="margin-bottom: 12px">
            还没有规则。新建一条，把「来自某个发件人」或「主题包含某个词」的邮件自动归档、打标签或标为已读。
          </div>

          <button class="mail-btn is-primary" @click="startNew">＋ 新建规则</button>

          <!-- 编辑器 -->
          <div v-if="editing" class="mail-rule-editor">
            <h3 class="mail-modal-title" style="font-size: 13px; margin: 18px 0 12px">
              {{ draftIndex >= 0 ? '编辑规则' : '新建规则' }}
            </h3>

            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">规则名称</label>
                <input v-model="draft.name" class="mail-input" placeholder="例如：财务报表自动归档" />
              </div>
              <div class="mail-field">
                <label class="mail-flabel">优先级（数字小的先执行）</label>
                <input v-model.number="draft.priority" class="mail-input" type="number" />
              </div>
            </div>

            <div class="mail-flabel" style="margin-bottom: 6px">满足以下条件（全部满足）</div>
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">
                  发件人包含
                  <span v-if="tokenCount('from')" class="mail-rule-count">{{ tokenCount('from') }} 个</span>
                </label>
                <div class="mail-rule-input-row">
                  <textarea
                    v-model="cond.from"
                    class="mail-input mail-rule-grow"
                    rows="1"
                    :title="cond.from"
                    placeholder="用逗号分隔多个"
                    @input="autoGrow"
                  ></textarea>
                  <button
                    class="mail-btn mail-rule-pick-btn"
                    :class="{ 'is-on': pickerTarget === 'from' }"
                    title="从通讯录选择发件人"
                    @click="togglePicker('from')"
                  >
                    通讯录
                  </button>
                </div>
              </div>
              <div class="mail-field">
                <label class="mail-flabel">
                  收件人包含
                  <span v-if="tokenCount('to')" class="mail-rule-count">{{ tokenCount('to') }} 个</span>
                </label>
                <div class="mail-rule-input-row">
                  <textarea
                    v-model="cond.to"
                    class="mail-input mail-rule-grow"
                    rows="1"
                    :title="cond.to"
                    placeholder="用逗号分隔多个"
                    @input="autoGrow"
                  ></textarea>
                  <button
                    class="mail-btn mail-rule-pick-btn"
                    :class="{ 'is-on': pickerTarget === 'to' }"
                    title="从通讯录选择收件人"
                    @click="togglePicker('to')"
                  >
                    通讯录
                  </button>
                </div>
              </div>
            </div>
            <!-- 内联展开而不是浮层：弹层正文自带滚动容器，浮层会被裁掉一截。 -->
            <div v-if="pickerTarget" class="mail-rule-picker">
              <div class="mail-rule-picker-head">
                选择{{ pickerTarget === 'from' ? '发件人' : '收件人' }}（可多选，选完继续追加）
              </div>
              <AddressPicker
                :exclude="splitList(pickerTarget === 'from' ? cond.from : cond.to)"
                keep-open
                @pick="onPickAddress"
                @pick-group="onPickGroup"
                @close="pickerTarget = ''"
              />
            </div>
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">主题包含</label>
                <textarea
                  v-model="cond.subject"
                  class="mail-input mail-rule-grow"
                  rows="1"
                  placeholder="用逗号分隔多个"
                  @input="autoGrow"
                ></textarea>
              </div>
              <div class="mail-field">
                <label class="mail-flabel">主题不包含</label>
                <textarea
                  v-model="cond.subjectNot"
                  class="mail-input mail-rule-grow"
                  rows="1"
                  placeholder="排除这类邮件"
                  @input="autoGrow"
                ></textarea>
              </div>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">正文包含</label>
              <input v-model="cond.body" class="mail-input" placeholder="匹配正文内容" />
            </div>
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">高级正则（主题 / 正文 / 发件人任一匹配）</label>
                <input v-model="cond.regex" class="mail-input" placeholder="例如 ^\\[发票\\]" spellcheck="false" />
              </div>
              <div class="mail-field">
                <label class="mail-flabel">生效文件夹</label>
                <select v-model="cond.folder" class="mail-input">
                  <option value="inbox">仅收件箱</option>
                  <option value="*">所有文件夹</option>
                </select>
              </div>
            </div>
            <label class="mail-check">
              <input v-model="cond.attachmentOnly" type="checkbox" />
              只处理带附件的邮件
            </label>

            <div class="mail-flabel" style="margin: 14px 0 6px">执行以下动作</div>
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">
                  移到文件夹
                  <span class="mail-rule-scope" :title="accountLabel">· {{ accountLabel }}</span>
                </label>
                <!-- 悬浮的文件夹树（对齐 Outlook 的「规则和通知」）：能挑已有文件夹、
                     能就地新建，全部在面板里完成，不再弹系统对话框。 -->
                <MailFolderPicker
                  v-model="act.moveTo"
                  :folders="targetFolders"
                  :account-label="accountLabel"
                  :create-folder="createFolderFn"
                  placeholder="不移动"
                  title="选择要移动到的文件夹（可以在面板里新建）"
                  @refresh="refreshFolders"
                />
              </div>
              <div class="mail-field">
                <label class="mail-flabel">
                  添加分类标签
                  <span v-if="splitList(act.labels).length" class="mail-rule-count">
                    {{ splitList(act.labels).length }} 个
                  </span>
                </label>
                <textarea
                  v-model="act.labels"
                  class="mail-input mail-rule-grow"
                  rows="1"
                  :title="act.labels"
                  placeholder="用逗号分隔"
                  @input="autoGrow"
                ></textarea>
              </div>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">
                转发到
                <span v-if="splitList(act.forwardTo).length" class="mail-rule-count">
                  {{ splitList(act.forwardTo).length }} 个地址
                </span>
              </label>
              <textarea
                v-model="act.forwardTo"
                class="mail-input mail-rule-grow"
                rows="1"
                :title="act.forwardTo"
                placeholder="可选，多个用逗号分隔"
                @input="autoGrow"
              ></textarea>
            </div>
            <div style="display: flex; gap: 16px; flex-wrap: wrap">
              <label class="mail-check"><input v-model="act.markRead" type="checkbox" />标为已读</label>
              <label class="mail-check"><input v-model="act.star" type="checkbox" />加星标</label>
              <label class="mail-check"><input v-model="act.delete" type="checkbox" />移入垃圾箱</label>
              <label class="mail-check"><input v-model="draft.stopProcessing" type="checkbox" />命中后停止后续规则</label>
            </div>

            <div style="display: flex; gap: 8px; margin-top: 12px; align-items: center">
              <button class="mail-btn is-primary" :disabled="saving" @click="saveDraft">
                {{ savingLabel }}
              </button>
              <button class="mail-btn" @click="editing = false">取消</button>
              <!-- 规则平时只作用于「同步到的新邮件」，新建规则后老邮件纹丝不动，
                   用户建完文件夹点进去是空的。这里给一个「立刻对已有邮件也执行」。 -->
              <label class="mail-check" style="margin-left: auto">
                <input v-model="applyExisting" type="checkbox" />
                保存后立即对已有的邮件执行
              </label>
            </div>
            <!-- 保存/执行失败就地说明，不弹系统对话框（这里刚被吐槽过一次）。 -->
            <p v-if="saveError" class="mail-rule-error">{{ saveError }}</p>
          </div>
        </section>

        <!-- ============ 发件人名单 ============ -->
        <section v-show="activeTab === 'senders'" class="mail-rules-panel">
          <p class="mail-hint" style="margin-top: 0">
            黑名单里的发件人直接进垃圾箱；白名单里的发件人跳过所有规则。
            支持 <code>@域名</code>、完整地址、<code>*</code> 通配符，或直接写一段子串。
            这里的判定优先于下面的收信规则。
          </p>

          <div v-for="item in senderRules" :key="item.id" class="mail-rule">
            <span class="mail-chip" :class="item.kind === 'block' ? 'is-on' : ''">
              {{ item.kind === 'block' ? '黑名单' : '白名单' }}
            </span>
            <div class="mail-rule-main">
              <div class="mail-rule-name">{{ item.pattern }}</div>
              <div v-if="item.note" class="mail-rule-desc">{{ item.note }}</div>
            </div>
            <button class="mail-btn is-danger" @click="$emit('delete-sender-rule', item.id)">删除</button>
          </div>

          <div v-if="!senderRules.length" class="mail-hint" style="margin-bottom: 12px">
            名单还是空的。拦掉某个发件人，或者给重要的人开白名单。
          </div>

          <div class="mail-field-row" style="align-items: flex-end">
            <div class="mail-field">
              <label class="mail-flabel">类型</label>
              <select v-model="senderDraft.kind" class="mail-input">
                <option value="block">黑名单（直接进垃圾箱）</option>
                <option value="allow">白名单（跳过规则）</option>
              </select>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">地址 / 域名 / 通配符</label>
              <input
                v-model="senderDraft.pattern"
                class="mail-input"
                placeholder="@spam.example 或 noreply-*@x.com"
                @keyup.enter="addSenderRule"
              />
            </div>
            <button class="mail-btn" style="margin-bottom: 2px" @click="addSenderRule">添加</button>
          </div>
        </section>
      </div>

      <div class="mail-modal-foot">
        <span class="mail-hint">
          规则只改变本地视图与服务器上的移动/标记，不会替你「代收」或「代回」。
        </span>
        <span class="spacer"></span>
        <button class="mail-btn" @click="$emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import AddressPicker from './AddressPicker.vue'
import MailIcon from './MailIcon.vue'
import MailFolderPicker from './MailFolderPicker.vue'
import { useContactsStore, expandGroupMembers } from '../../../../stores/contactsStore'

const props = defineProps({
  rules: { type: Array, default: () => [] },
  accounts: { type: Array, default: () => [] },
  foldersByAccount: { type: Object, default: () => ({}) },
  activeAccountId: { type: String, default: null },
  // 收/发件人名单（黑名单 / 白名单）。以前模板里用了却没声明，
  // 渲染时读 `item.kind` 直接抛 TypeError，整个弹层（连同上层的设置页）都白屏。
  senderRules: { type: Array, default: () => [] },
  /** 新建文件夹（规则里选「不存在的文件夹」时用）。返回 `{ name }` 或 null。 */
  createFolderFn: { type: Function, default: null },
  /** 从服务器刷新文件夹表：列表要能随其它客户端新建的文件夹更新。 */
  refreshFoldersFn: { type: Function, default: null },
  /**
   * 保存规则（可选立即对已有邮件执行）。必须是函数 prop：
   * emit 拿不到返回值，而「立即执行」要先知道后端给这条规则分配的 id。
   */
  saveRuleFn: { type: Function, default: null },
})
const emit = defineEmits([
  'close',
  'save',
  'delete',
  'save-sender-rule',
  'delete-sender-rule',
])

const TABS = [
  { key: 'rules', label: '收信规则', icon: 'filter' },
  { key: 'senders', label: '发件人名单', icon: 'shield' },
]
const activeTab = ref('rules')
const panelRef = ref(null)

function tabCount(key) {
  return key === 'rules' ? props.rules.length : props.senderRules.length
}

function switchTab(key) {
  activeTab.value = key
  // 换页后把滚动位置收回顶部，否则新页一进来就停在中间。
  nextTick(() => {
    if (panelRef.value) panelRef.value.scrollTop = 0
    resizeGrowables()
  })
}

/** 新增一条发件人名单的草稿。提交后清空，方便连着录入几条。 */
const senderDraft = reactive({ kind: 'block', pattern: '' })

function addSenderRule() {
  const pattern = senderDraft.pattern.trim()
  if (!pattern) return
  emit('save-sender-rule', {
    id: '',
    kind: senderDraft.kind,
    pattern,
    note: '',
    createdAt: 0,
  })
  senderDraft.pattern = ''
}

const editing = ref(false)
const draftIndex = ref(-1)
/** 保存后是否立刻对库里已有的邮件执行一遍。 */
const applyExisting = ref(false)
const saving = ref(false)
/** 保存或「立即执行」失败的说明：就地显示，不弹系统对话框。 */
const saveError = ref('')
const savingLabel = computed(() => {
  if (!saving.value) return '保存规则'
  // 扫已有邮件是逐封判断 + 逐封移动，几百封要跑一会儿，得说清在忙什么。
  return applyExisting.value ? '正在对已有邮件执行…' : '保存中…'
})

const blankDraft = () => ({
  id: '',
  name: '',
  enabled: true,
  priority: 10,
  matchMode: 'all',
  condition: {},
  action: {},
  stopProcessing: false,
  createdAt: 0,
})

const draft = reactive(blankDraft())
const cond = reactive({
  from: '',
  to: '',
  subject: '',
  subjectNot: '',
  body: '',
  regex: '',
  folder: 'inbox',
  attachmentOnly: false,
})
const act = reactive({
  moveTo: '',
  labels: '',
  forwardTo: '',
  markRead: false,
  star: false,
  delete: false,
})

/** 通讯录选择器当前挂在哪个条件上：'from' | 'to' | ''。 */
const pickerTarget = ref('')
const contacts = useContactsStore()

function togglePicker(target) {
  pickerTarget.value = pickerTarget.value === target ? '' : target
}

// ---------------- 多值输入框：内容多了向下长，而不是把后面的内容截断 ----------------

/**
 * 地址 / 主题这类「逗号分隔的多个值」用单行输入框时，从通讯录加几个人就被截断，
 * 用户看不到自己到底加了谁。这里用 textarea 让它向下长（最多 5 行），
 * 超出后自身滚动——膨胀只影响这一格的高度，不会把整个弹层撑开。
 */
function autoGrow(event) {
  const el = event?.target
  if (!el || el.tagName !== 'TEXTAREA') return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight + 2, 96)}px`
}

function resizeGrowables() {
  const root = panelRef.value
  if (!root) return
  root.querySelectorAll('textarea.mail-rule-grow').forEach((el) => {
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight + 2, 96)}px`
  })
}

/**
 * 输入框里已有几个值。
 *
 * 框最多长到 5 行，从通讯录连着加二十个人时看不到底——标签上先报个数，
 * 悬停输入框还能看到完整清单（textarea 的 title）。
 */
function tokenCount(field) {
  return splitList(cond[field]).length
}

/** 把选中的地址追加进「发件人 / 收件人包含」，去重后仍用逗号分隔。 */
function appendAddresses(target, addresses) {
  const field = target === 'from' ? 'from' : 'to'
  const existing = new Set(splitList(cond[field]).map((item) => item.toLowerCase()))
  const additions = addresses
    .map((item) => String(item || '').trim())
    .filter((item) => item && !existing.has(item.toLowerCase()))
  if (!additions.length) return
  const base = String(cond[field] || '').trim().replace(/[,;，；]\s*$/, '')
  cond[field] = base ? `${base}, ${additions.join(', ')}` : additions.join(', ')
  // 追加后立刻长高：用户马上能看到新加进去的地址。
  nextTick(resizeGrowables)
}

function onPickAddress(address) {
  const target = pickerTarget.value
  if (!target) return
  appendAddresses(target, [address])
}

/** 通讯组：规则里存具体的成员地址，匹配才生效（组名本身不是邮箱）。 */
function onPickGroup(groupId) {
  const target = pickerTarget.value
  if (!target) return
  const members = expandGroupMembers(contacts.groups, groupId)
  appendAddresses(target, members.map((member) => member.email))
}

function onDocMouseDown(event) {
  if (!pickerTarget.value) return
  if (event.target.closest?.('.mail-rule-picker, .mail-rule-pick-btn')) return
  pickerTarget.value = ''
}

onMounted(() => {
  document.addEventListener('mousedown', onDocMouseDown)
  nextTick(resizeGrowables)
})
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocMouseDown))

const orderedRules = computed(() =>
  [...props.rules].sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0)),
)

// ---------------- 目标文件夹 ----------------

/** 规则用哪个账户的文件夹：优先当前账户，聚合视图下退回第一个账户。 */
const ruleAccountId = computed(() => props.activeAccountId || props.accounts[0]?.id || '')
const accountLabel = computed(() => {
  const account = props.accounts.find((a) => a.id === ruleAccountId.value)
  return account?.email || '未选择账户'
})

/**
 * 「移到文件夹」的候选：**只列当前账户**的文件夹。
 *
 * 以前这里取的是 `accounts[0]`（不是当前账户），多账户时会把别的账户的文件夹
 * 摆进来，选中后规则移动自然失败。聚合视图（没有当前账户）下也不该跨账户混列，
 * 所以退回第一个账户并明确标注账户名。
 * 排序与层级交给 MailFolderPicker（它按角色与名字排，并把带分隔符的名字组成树）。
 */
const targetFolders = computed(() => {
  const list = props.foldersByAccount[ruleAccountId.value] || []
  return list
    .filter((folder) => folder.selectable !== false)
    // 发件箱是本地队列，不是可归档的目的地。
    .filter((folder) => folder.role !== 'outbox')
})

/** 列表随文件夹表实时变化：面板打开时再问一次服务器，别让用户以为建了却不存在。 */
function refreshFolders() {
  props.refreshFoldersFn?.()
}

function splitList(text) {
  return String(text || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
}

function joinList(list) {
  return (list || []).join(', ')
}

function describeRule(rule) {
  const c = rule.condition || {}
  const a = rule.action || {}
  const parts = []
  if (c.fromContains?.length) parts.push(`发件人含 ${c.fromContains.join(' / ')}`)
  if (c.toContains?.length) parts.push(`收件人含 ${c.toContains.join(' / ')}`)
  if (c.subjectContains?.length) parts.push(`主题含 ${c.subjectContains.join(' / ')}`)
  if (c.subjectNotContains?.length) parts.push(`主题不含 ${c.subjectNotContains.join(' / ')}`)
  if (c.bodyContains?.length) parts.push(`正文含 ${c.bodyContains.join(' / ')}`)
  if (c.anyMatchesRegex) parts.push(`正则 ${c.anyMatchesRegex}`)
  if (c.hasAttachment) parts.push('带附件')
  const actions = []
  if (a.moveTo) actions.push(`移到 ${a.moveTo}`)
  if (a.addLabels?.length) actions.push(`标签 ${a.addLabels.join('/')}`)
  if (a.forwardTo?.length) actions.push(`转发 ${a.forwardTo.join('/')}`)
  if (a.markRead) actions.push('标已读')
  if (a.star) actions.push('加星标')
  if (a.delete) actions.push('移入垃圾箱')
  const where = (rule.condition?.folderRoles || []).length
    ? `【${rule.condition.folderRoles.join('/')}】`
    : '【收件箱】'
  return `${where}${parts.join('，') || '（无条件）'} → ${actions.join('，') || '（无动作）'}`
}

function startNew() {
  pickerTarget.value = ''
  Object.assign(draft, blankDraft())
  Object.assign(cond, {
    from: '',
    to: '',
    subject: '',
    subjectNot: '',
    body: '',
    regex: '',
    folder: 'inbox',
    attachmentOnly: false,
  })
  Object.assign(act, {
    moveTo: '',
    labels: '',
    forwardTo: '',
    markRead: false,
    star: false,
    delete: false,
  })
  draftIndex.value = -1
  saveError.value = ''
  editing.value = true
  refreshFolders()
  nextTick(resizeGrowables)
}

function editRule(rule) {
  pickerTarget.value = ''
  Object.assign(draft, JSON.parse(JSON.stringify(rule)))
  const c = rule.condition || {}
  Object.assign(cond, {
    from: joinList(c.fromContains),
    to: joinList(c.toContains),
    subject: joinList(c.subjectContains),
    subjectNot: joinList(c.subjectNotContains),
    body: joinList(c.bodyContains),
    regex: c.anyMatchesRegex || '',
    folder: (c.folderRoles || []).includes('*') ? '*' : 'inbox',
    attachmentOnly: Boolean(c.hasAttachment),
  })
  const a = rule.action || {}
  Object.assign(act, {
    moveTo: a.moveTo || '',
    labels: joinList(a.addLabels),
    forwardTo: joinList(a.forwardTo),
    markRead: Boolean(a.markRead),
    star: Boolean(a.star),
    delete: Boolean(a.delete),
  })
  draftIndex.value = props.rules.findIndex((r) => r.id === rule.id)
  saveError.value = ''
  editing.value = true
  refreshFolders()
  nextTick(resizeGrowables)
}

async function saveDraft() {
  pickerTarget.value = ''
  const payload = {
    ...draft,
    condition: {
      fromContains: splitList(cond.from),
      toContains: splitList(cond.to),
      subjectContains: splitList(cond.subject),
      subjectNotContains: splitList(cond.subjectNot),
      bodyContains: splitList(cond.body),
      anyMatchesRegex: cond.regex.trim() || null,
      hasAttachment: cond.attachmentOnly ? true : null,
      accountIds: [],
      // 后端语义：空 = 只处理收件箱（Outlook 默认），`*` = 所有文件夹。
      folderRoles: cond.folder === 'inbox' ? [] : ['*'],
      minSize: null,
    },
    action: {
      moveTo: act.moveTo || null,
      copyTo: null,
      addLabels: splitList(act.labels),
      removeLabels: [],
      forwardTo: splitList(act.forwardTo),
      markRead: act.markRead,
      star: act.star,
      delete: act.delete,
      skipInbox: false,
    },
  }
  if (!payload.name.trim()) payload.name = '未命名规则'
  const applyNow = applyExisting.value
  saving.value = true
  saveError.value = ''
  try {
    // 走函数 prop 而不是 emit：保存后要拿到这条规则的 id 才能「立即对已有邮件执行」，
    // 并把处理条数如实回报（0 封命中也要说一声，否则用户以为按钮没生效）。
    if (props.saveRuleFn) await props.saveRuleFn(payload, applyNow)
    else emit('save', payload)
    editing.value = false
    applyExisting.value = false
  } catch (error) {
    saveError.value = `保存失败：${String(error?.message || error)}`
  } finally {
    saving.value = false
  }
}

async function toggleRule(rule, enabled) {
  emit('save', { ...rule, enabled })
}

function removeRule(rule) {
  if (!window.confirm(`删除规则「${rule.name}」？`)) return
  emit('delete', rule.id)
  if (editing.value && draft.id === rule.id) editing.value = false
}
</script>

<style scoped>
/* 两个模块的切页：与「外观与设置」同一套样式，但不再 sticky——
   弹层正文本身就是滚动容器，贴顶的页签会把下面的说明文字压住。 */
.mail-rules-tabs {
  display: flex;
  gap: 4px;
  padding: 10px 18px 0;
  border-bottom: 1px solid var(--mail-border);
}

.mail-rules-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-bottom: -1px;
  padding: 7px 12px;
  border: 1px solid transparent;
  border-bottom: 0;
  border-radius: 8px 8px 0 0;
  background: transparent;
  color: var(--mail-text-secondary);
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
}
.mail-rules-tab:hover {
  background: var(--mail-panel-hover);
  color: var(--mail-text);
}
.mail-rules-tab.is-on {
  border-color: var(--mail-border);
  background: var(--mail-panel);
  color: var(--mail-text);
  font-weight: 600;
}
.mail-rules-tab-count {
  min-width: 18px;
  padding: 0 5px;
  border-radius: 999px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
  font-size: 10.5px;
  font-weight: 500;
  line-height: 16px;
  text-align: center;
}

.mail-rules-panel {
  min-height: 0;
}

.mail-rule-editor {
  margin-top: 10px;
  padding: 14px;
  border: 1px solid var(--mail-border);
  border-radius: var(--mail-radius-sm);
  /* 与弹层同一块底面：--mail-bg-sunken 比面板暗一档，在暗色主题下像贴了一块黑板。 */
  background: var(--mail-panel);
}

.mail-rule-input-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.mail-rule-input-row .mail-input {
  flex: 1 1 auto;
  min-width: 0;
}
.mail-rule-pick-btn {
  flex: 0 0 auto;
  height: 31px;
}

/* 多值输入框：内容多了往下长（上限 5 行），之后自身滚动。
   不用单行 input——从通讯录加几个人就被横向截断，用户看不到加了谁。 */
.mail-rule-grow {
  min-height: 31px;
  max-height: 96px;
  padding: 7px 9px;
  line-height: 1.45;
  resize: none;
  overflow-y: auto;
}

/* 「移到文件夹」旁边标出用的是哪个账户的文件夹：多账户时这一栏容易误读。 */
.mail-rule-scope {
  margin-left: 4px;
  color: var(--mail-text-muted);
  font-size: 11px;
  font-weight: 400;
}

/* 条件里已经填了几个值：框最多长到 5 行，靠这个计数知道后面还有没有。 */
.mail-rule-count {
  margin-left: 4px;
  padding: 0 5px;
  border-radius: 999px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
  font-size: 10.5px;
  font-weight: 500;
}

/* 就地报错：保存 / 立即执行失败时显示在按钮行下面，不弹系统对话框。 */
.mail-rule-error {
  margin: 8px 0 0;
  padding: 6px 10px;
  border-radius: var(--mail-radius-sm);
  background: color-mix(in srgb, var(--mail-danger) 12%, transparent);
  color: var(--mail-danger);
  font-size: 12px;
  line-height: 1.45;
}

/* 通讯录面板：内联在条件行下方，标题写清楚这次是往哪一栏里加人。 */
.mail-rule-picker {
  margin: 2px 0 14px;
  border: 1px solid var(--mail-border-strong);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-panel);
  overflow: hidden;
}
.mail-rule-picker-head {
  padding: 7px 12px;
  border-bottom: 1px solid var(--mail-border);
  color: var(--mail-text-muted);
  font-size: 11.5px;
}
/* AddressPicker 本身是内联面板配色（凹陷底 + 底边线），嵌进来后跟着弹层底面走。 */
.mail-rule-picker :deep(.address-picker) {
  background: var(--mail-panel);
  border-bottom: 0;
}
</style>
