<template>
  <div class="contact-editor">
    <h3 class="contact-editor-title">{{ contact.id ? '编辑联系人' : '新建联系人' }}</h3>

    <div class="mail-field-row">
      <div class="mail-field">
        <label class="mail-flabel">显示名</label>
        <input v-model="form.displayName" class="mail-input" placeholder="张三" />
      </div>
      <div class="mail-field">
        <label class="mail-flabel">公司 / 组织</label>
        <input v-model="form.organization" class="mail-input" placeholder="某某公司" />
      </div>
    </div>

    <div class="mail-field-row">
      <div class="mail-field">
        <label class="mail-flabel">姓</label>
        <input v-model="form.familyName" class="mail-input" placeholder="张" />
      </div>
      <div class="mail-field">
        <label class="mail-flabel">名</label>
        <input v-model="form.givenName" class="mail-input" placeholder="三" />
      </div>
    </div>

    <div class="mail-field">
      <label class="mail-flabel">邮箱</label>
      <div v-for="(email, index) in form.emails" :key="`e${index}`" class="contact-row-input">
        <select v-model="email.label" class="mail-input contact-label">
          <option value="work">工作</option>
          <option value="home">私人</option>
          <option value="other">其他</option>
        </select>
        <input v-model="email.value" class="mail-input" placeholder="name@example.com" spellcheck="false" />
        <button
          v-if="form.emails.length > 1"
          class="mail-icon-btn"
          title="删除这个邮箱"
          @click="form.emails.splice(index, 1)"
        >
          ×
        </button>
      </div>
      <button class="mail-btn contact-add-row" @click="form.emails.push({ label: 'other', value: '' })">
        ＋ 添加邮箱
      </button>
    </div>

    <div class="mail-field">
      <label class="mail-flabel">电话</label>
      <div v-for="(phone, index) in form.phones" :key="`p${index}`" class="contact-row-input">
        <select v-model="phone.label" class="mail-input contact-label">
          <option value="phone">手机</option>
          <option value="work">工作</option>
          <option value="home">私人</option>
          <option value="other">其他</option>
        </select>
        <input v-model="phone.value" class="mail-input" placeholder="13800000000" />
        <button class="mail-icon-btn" title="删除" @click="form.phones.splice(index, 1)">×</button>
      </div>
      <button class="mail-btn contact-add-row" @click="form.phones.push({ label: 'phone', value: '' })">
        ＋ 添加电话
      </button>
    </div>

    <div class="mail-field-row">
      <div class="mail-field">
        <label class="mail-flabel">职位</label>
        <input v-model="form.title" class="mail-input" placeholder="财务经理" />
      </div>
      <div class="mail-field">
        <label class="mail-flabel">生日（YYYY-MM-DD）</label>
        <input v-model="form.birthday" class="mail-input" placeholder="1990-01-01" />
      </div>
    </div>

    <div class="mail-field-row">
      <div class="mail-field">
        <label class="mail-flabel">地址</label>
        <input v-model="form.address" class="mail-input" placeholder="城市 / 街道" />
      </div>
      <div class="mail-field">
        <label class="mail-flabel">网址</label>
        <input v-model="form.website" class="mail-input" placeholder="https://" spellcheck="false" />
      </div>
    </div>

    <div class="mail-field">
      <label class="mail-flabel">备注</label>
      <textarea v-model="form.note" class="mail-textarea" rows="3"></textarea>
    </div>

    <div class="mail-field" v-if="groups.length">
      <label class="mail-flabel">所属通讯组</label>
      <div class="contact-group-chips">
        <button
          v-for="group in groups"
          :key="group.id"
          class="mail-chip"
          :class="{ 'is-on': form.groups.includes(group.id) }"
          @click="toggleGroup(group.id)"
        >
          {{ group.name }}
        </button>
      </div>
    </div>

    <label class="mail-check">
      <input v-model="form.starred" type="checkbox" />
      标记为星标联系人（排到补全列表前面）
    </label>

    <div class="contact-editor-foot">
      <button class="mail-btn" @click="$emit('cancel')">取消</button>
      <button class="mail-btn is-primary" :disabled="saving || !canSave" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, watch } from 'vue'

const props = defineProps({
  contact: { type: Object, required: true },
  groups: { type: Array, default: () => [] },
  saving: { type: Boolean, default: false },
})
const emit = defineEmits(['save', 'cancel'])

const form = reactive(JSON.parse(JSON.stringify(props.contact)))

watch(
  () => props.contact,
  (next) => Object.assign(form, JSON.parse(JSON.stringify(next))),
)

// 至少要有一个非空邮箱：没有地址的联系人在写信时用不上。
const canSave = computed(() => form.emails.some((e) => e.value.trim()))

function toggleGroup(id) {
  const index = form.groups.indexOf(id)
  if (index >= 0) form.groups.splice(index, 1)
  else form.groups.push(id)
}

function save() {
  const cleaned = {
    ...form,
    emails: form.emails.filter((e) => e.value.trim()).map((e) => ({ ...e, value: e.value.trim() })),
    phones: form.phones.filter((p) => p.value.trim()).map((p) => ({ ...p, value: p.value.trim() })),
  }
  if (!cleaned.displayName.trim()) {
    cleaned.displayName =
      [cleaned.familyName, cleaned.givenName].filter(Boolean).join('') ||
      cleaned.emails[0]?.value ||
      ''
  }
  emit('save', cleaned)
}
</script>

<style scoped>
.contact-editor {
  display: flex;
  flex-direction: column;
  padding: 14px 16px 24px;
  overflow-y: auto;
}

.contact-editor-title {
  margin: 0 0 14px;
  font-size: 14px;
  font-weight: 650;
}

.contact-row-input {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}

.contact-label {
  width: 84px;
  flex: 0 0 auto;
}

.contact-add-row {
  margin-top: 2px;
  font-size: 11.5px;
}

.contact-group-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.contact-editor-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--mail-border);
}
</style>
