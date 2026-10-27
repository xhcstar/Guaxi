<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal is-wide" role="dialog" aria-label="收信规则">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">收信规则</h2>
        <button class="mail-modal-close" @click="$emit('close')">×</button>
      </div>

      <div class="mail-modal-body">
        <p class="mail-hint" style="margin-top: 0">
          规则在本地同步到新邮件后执行。没有指定文件夹的规则只作用于收件箱。
          想让规则在服务器上生效（关掉客户端也跑），在「外观与设置 → 服务器端规则」里上传。
        </p>

        <!-- 收发件人名单：比规则更简单的「一键拉黑 / 放行」，判定顺序也更靠前 -->
        <div class="mail-section-divider"></div>
        <div class="mail-flabel" style="margin-bottom: 6px">发件人名单（优先于规则）</div>
        <p class="mail-hint" style="margin: 0 0 8px">
          黑名单里的发件人直接进垃圾箱；白名单里的发件人跳过所有规则。
          支持 <code>@域名</code>、完整地址、<code>*</code> 通配符，或直接写一段子串。
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

        <div class="mail-section-divider"></div>
        <div class="mail-flabel" style="margin-bottom: 8px">收信规则</div>

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
              <label class="mail-flabel">发件人包含</label>
              <input v-model="cond.from" class="mail-input" placeholder="用逗号分隔多个" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">收件人包含</label>
              <input v-model="cond.to" class="mail-input" placeholder="用逗号分隔多个" />
            </div>
          </div>
          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">主题包含</label>
              <input v-model="cond.subject" class="mail-input" placeholder="用逗号分隔多个" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">主题不包含</label>
              <input v-model="cond.subjectNot" class="mail-input" placeholder="排除这类邮件" />
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
                <option value="">所有文件夹</option>
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
              <label class="mail-flabel">移到文件夹</label>
              <select v-model="act.moveTo" class="mail-input">
                <option value="">不移动</option>
                <option v-for="folder in folderOptions" :key="folder.name" :value="folder.name">
                  {{ folder.displayName }}
                </option>
              </select>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">添加分类标签</label>
              <input v-model="act.labels" class="mail-input" placeholder="用逗号分隔" />
            </div>
          </div>
          <div class="mail-field">
            <label class="mail-flabel">转发到</label>
            <input v-model="act.forwardTo" class="mail-input" placeholder="可选，多个用逗号分隔" />
          </div>
          <div style="display: flex; gap: 16px; flex-wrap: wrap">
            <label class="mail-check"><input v-model="act.markRead" type="checkbox" />标为已读</label>
            <label class="mail-check"><input v-model="act.star" type="checkbox" />加星标</label>
            <label class="mail-check"><input v-model="act.delete" type="checkbox" />移入垃圾箱</label>
            <label class="mail-check"><input v-model="draft.stopProcessing" type="checkbox" />命中后停止后续规则</label>
          </div>

          <div style="display: flex; gap: 8px; margin-top: 12px">
            <button class="mail-btn is-primary" @click="saveDraft">保存规则</button>
            <button class="mail-btn" @click="editing = false">取消</button>
          </div>
        </div>
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
import { computed, reactive, ref } from 'vue'

const props = defineProps({
  rules: { type: Array, default: () => [] },
  accounts: { type: Array, default: () => [] },
  foldersByAccount: { type: Object, default: () => ({}) },
  activeAccountId: { type: String, default: null },
  // 收/发件人名单（黑名单 / 白名单）。以前模板里用了却没声明，
  // 渲染时读 `item.kind` 直接抛 TypeError，整个弹层（连同上层的设置页）都白屏。
  senderRules: { type: Array, default: () => [] },
})
const emit = defineEmits([
  'close',
  'save',
  'delete',
  'save-sender-rule',
  'delete-sender-rule',
])

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

const orderedRules = computed(() =>
  [...props.rules].sort((a, b) => (a.priority ?? 0) - (b.priority ?? 0)),
)

const folderOptions = computed(() => {
  const accountId = props.activeAccountId || props.accounts[0]?.id
  return props.foldersByAccount[accountId] || []
})

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
  editing.value = true
}

function editRule(rule) {
  Object.assign(draft, JSON.parse(JSON.stringify(rule)))
  const c = rule.condition || {}
  Object.assign(cond, {
    from: joinList(c.fromContains),
    to: joinList(c.toContains),
    subject: joinList(c.subjectContains),
    subjectNot: joinList(c.subjectNotContains),
    body: joinList(c.bodyContains),
    regex: c.anyMatchesRegex || '',
    folder: (c.folderRoles || []).length ? '' : 'inbox',
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
  editing.value = true
}

function saveDraft() {
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
      folderRoles: cond.folder ? [cond.folder] : [],
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
  emit('save', payload)
  editing.value = false
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
.mail-rule-editor {
  margin-top: 10px;
  padding: 14px;
  border: 1px solid var(--mail-border);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-bg-sunken);
}
</style>
