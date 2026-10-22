// 通讯录的前端状态。
//
// 与 mailStore 分开是有意的：通讯录的生命周期比「当前选中的邮件」长，
// 写信时的地址补全又要在没有打开通讯录页的情况下工作。放在 mailStore 里会让
// 邮件页的每次刷新都连带重查联系人。
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('ContactsStore')

const emptyQuery = () => ({
  text: '',
  groupIds: [],
  starredOnly: false,
  source: null,
  frequentOnly: false,
  includeUnused: true,
  limit: 500,
  offset: 0,
})

export const useContactsStore = defineStore('contacts', {
  state: () => ({
    ready: false,
    loading: false,
    contacts: [],
    groups: [],
    stats: { total: 0, auto: 0, starred: 0, withUnread: 0 },
    query: emptyQuery(),
    /** 当前打开的联系人详情。 */
    active: null,
    activeMessages: [],
    activeLoading: false,
    /** 当前选中的组（左栏）。 */
    activeGroupId: '',
    error: '',
  }),

  getters: {
    total: (state) => state.stats.total || state.contacts.length,
    /** 按首字母/拼音首字母分组，供右侧列表画分组标题。 */
    sections(state) {
      const buckets = new Map()
      const sorted = [...state.contacts].sort((a, b) =>
        sortKey(a).localeCompare(sortKey(b), 'zh-CN'),
      )
      for (const contact of sorted) {
        const letter = initialOf(contact)
        if (!buckets.has(letter)) buckets.set(letter, [])
        buckets.get(letter).push(contact)
      }
      return [...buckets.entries()]
        .sort((a, b) => a[0].localeCompare(b[0], 'zh-CN'))
        .map(([letter, items]) => ({ letter, items }))
    },
    groupsById(state) {
      return Object.fromEntries(state.groups.map((g) => [g.id, g]))
    },
  },

  actions: {
    async init() {
      if (this.ready) return
      this.loading = true
      try {
        await Promise.all([this.reload(), this.reloadGroups(), this.reloadStats()])
        this.ready = true
      } catch (error) {
        logger.error('通讯录初始化失败', error)
        this.error = String(error?.message || error)
      } finally {
        this.loading = false
      }
    },

    async reload() {
      this.loading = true
      try {
        const rows = await invoke('mail_list_contacts', { query: this.query })
        this.contacts = rows || []
        this.error = ''
      } catch (error) {
        logger.error('加载联系人失败', error)
        this.error = String(error?.message || error)
        this.contacts = []
      } finally {
        this.loading = false
      }
    },

    async reloadStats() {
      try {
        this.stats = (await invoke('mail_contact_stats')) || this.stats
      } catch (error) {
        logger.warn('加载通讯录统计失败', error)
      }
    },

    async reloadGroups() {
      try {
        this.groups = (await invoke('mail_list_contact_groups')) || []
      } catch (error) {
        logger.warn('加载通讯组失败', error)
      }
    },

    /** 搜索：输入即查，走库层 LIKE + 邮箱索引。 */
    setSearch(text) {
      this.query.text = text
      this.query.offset = 0
      return this.reload()
    },

    selectGroup(id) {
      this.activeGroupId = id
      this.query.groupIds = id ? [id] : []
      this.query.offset = 0
      return this.reload()
    },

    toggleStarFilter(on) {
      this.query.starredOnly = Boolean(on)
      return this.reload()
    },

    toggleFrequent(on) {
      this.query.frequentOnly = Boolean(on)
      return this.reload()
    },

    async open(id) {
      this.activeLoading = true
      this.activeMessages = []
      try {
        const [contact, messages] = await Promise.all([
          invoke('mail_get_contact', { id }),
          invoke('mail_contact_messages', { id, limit: 40 }),
        ])
        this.active = contact || null
        this.activeMessages = messages || []
      } catch (error) {
        logger.error('打开联系人失败', error)
        this.error = String(error?.message || error)
      } finally {
        this.activeLoading = false
      }
    },

    closeActive() {
      this.active = null
      this.activeMessages = []
    },

    async save(contact) {
      const saved = await invoke('mail_save_contact', { contact })
      const index = this.contacts.findIndex((c) => c.id === saved.id)
      if (index >= 0) this.contacts.splice(index, 1, saved)
      else this.contacts.unshift(saved)
      if (this.active?.id === saved.id) this.active = saved
      await Promise.all([this.reload(), this.reloadStats()])
      return saved
    },

    async remove(id) {
      await invoke('mail_delete_contact', { id })
      this.contacts = this.contacts.filter((c) => c.id !== id)
      if (this.active?.id === id) this.closeActive()
      await this.reloadStats()
    },

    // ---------------- 通讯组 ----------------

    async saveGroup(group) {
      const saved = await invoke('mail_save_contact_group', { group })
      const index = this.groups.findIndex((g) => g.id === saved.id)
      if (index >= 0) this.groups.splice(index, 1, saved)
      else this.groups.push(saved)
      this.groups.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
      return saved
    },

    async removeGroup(id) {
      await invoke('mail_delete_contact_group', { id })
      this.groups = this.groups.filter((g) => g.id !== id)
      if (this.activeGroupId === id) this.selectGroup('')
      await this.reload()
    },

    async expandGroup(id) {
      return invoke('mail_expand_contact_group', { id })
    },

    // ---------------- 导入导出 ----------------

    async importFromFile() {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const path = await open({
        multiple: false,
        title: '选择 vCard 文件',
        filters: [{ name: 'vCard', extensions: ['vcf', 'vcard', 'txt'] }],
      })
      if (!path) return null
      const report = await invoke('mail_import_contacts', { path })
      await Promise.all([this.reload(), this.reloadStats()])
      return report
    },

    async exportToFile(groupId = null) {
      const { save } = await import('@tauri-apps/plugin-dialog')
      const path = await save({
        defaultPath: groupId
          ? `${this.groupsById[groupId]?.name || '通讯组'}.vcf`
          : '通讯录.vcf',
        filters: [{ name: 'vCard', extensions: ['vcf'] }],
      })
      if (!path) return 0
      return invoke('mail_export_contacts', { path, groupId })
    },

    async rebuildFromMail() {
      const touched = await invoke('mail_contacts_rebuild_from_mail')
      await Promise.all([this.reload(), this.reloadStats()])
      return touched
    },

    // ---------------- 补全（写信时用） ----------------

    async suggest(prefix) {
      if (!prefix || prefix.trim().length < 1) return []
      try {
        return (await invoke('mail_contact_suggest', { prefix: prefix.trim() })) || []
      } catch (error) {
        logger.warn('地址补全失败', error)
        return []
      }
    },
  },
})

/** 排序键：中文按拼音，英文按字母（localeCompare 带 zh-CN 即按拼音）。 */
function sortKey(contact) {
  return String(contact.displayName || contact.emails?.[0]?.value || '').trim()
}

/** 首字母（A-Z，非字母归到 #）。 */
function initialOf(contact) {
  const name = sortKey(contact)
  if (!name) return '#'
  const first = name[0].toUpperCase()
  return /[A-Z]/.test(first) ? first : '#'
}

/** 联系人新增/编辑的空壳。 */
export function blankContact() {
  return {
    id: '',
    displayName: '',
    givenName: '',
    familyName: '',
    emails: [{ label: 'work', value: '' }],
    phones: [],
    organization: '',
    title: '',
    note: '',
    birthday: '',
    website: '',
    address: '',
    avatar: '',
    groups: [],
    starred: false,
    source: 'manual',
    firstSeenAt: 0,
    lastSeenAt: 0,
    createdAt: 0,
    updatedAt: 0,
  }
}

export function blankGroup() {
  return {
    id: '',
    name: '',
    description: '',
    members: [],
    createdAt: 0,
    updatedAt: 0,
  }
}
