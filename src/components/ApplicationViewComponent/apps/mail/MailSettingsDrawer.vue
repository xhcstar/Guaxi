<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal" role="dialog" aria-label="外观与设置">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">外观与设置</h2>
        <button class="mail-modal-close" @click="$emit('close')">×</button>
      </div>

      <div class="mail-modal-body">
        <!-- 主题 -->
        <div class="mail-flabel" style="margin-bottom: 8px">主题</div>
        <div class="mail-theme-grid">
          <button
            v-for="theme in themes"
            :key="theme.id"
            class="mail-theme-card"
            :class="{ 'is-active': settings.theme === theme.id }"
            @click="$emit('update', { theme: theme.id })"
          >
            <div class="mail-theme-swatches">
              <span
                v-for="token in ['bg', 'panel', 'text', 'accent']"
                :key="token"
                class="mail-theme-swatch"
                :style="{ background: theme.tokens?.[token] || 'transparent' }"
              ></span>
            </div>
            <div class="mail-theme-name">
              {{ theme.name }}
              <span class="mail-hint">{{ theme.mode === 'dark' ? '深色' : '浅色' }}</span>
            </div>
          </button>
        </div>

        <!-- 密度 -->
        <div class="mail-field">
          <label class="mail-flabel">界面密度</label>
          <div style="display: flex; gap: 6px">
            <button
              v-for="item in densityOptions"
              :key="item.value"
              class="mail-chip"
              :class="{ 'is-on': settings.density === item.value }"
              @click="$emit('update', { density: item.value })"
            >
              {{ item.label }}
            </button>
          </div>
        </div>

        <!-- 字体 -->
        <div class="mail-field-row">
          <div class="mail-field">
            <label class="mail-flabel">阅读字体</label>
            <select
              class="mail-input"
              :value="settings.readingFont"
              @change="$emit('update', { readingFont: $event.target.value })"
            >
              <option value="system">跟随系统</option>
              <option v-for="font in fontOptions" :key="font" :value="font">{{ font }}</option>
            </select>
          </div>
          <div class="mail-field">
            <label class="mail-flabel">阅读字号（{{ settings.readingSize }}px）</label>
            <input
              class="mail-input"
              type="range"
              min="12"
              max="22"
              :value="settings.readingSize"
              @input="$emit('update', { readingSize: Number($event.target.value) })"
            />
          </div>
        </div>

        <div class="mail-field">
          <label class="mail-flabel">主色（留空使用主题自带）</label>
          <div style="display: flex; gap: 6px; align-items: center">
            <input
              type="color"
              :value="settings.accentOverride || themeAccent"
              style="width: 36px; height: 30px; border: 1px solid var(--mail-border); border-radius: 6px; background: transparent"
              @input="$emit('update', { accentOverride: $event.target.value })"
            />
            <input
              class="mail-input"
              :value="settings.accentOverride"
              placeholder="例如 #7aa2f7"
              @change="$emit('update', { accentOverride: $event.target.value.trim() })"
            />
            <button
              v-if="settings.accentOverride"
              class="mail-btn"
              @click="$emit('update', { accentOverride: '' })"
            >
              恢复
            </button>
          </div>
        </div>

        <!-- 代理 -->
        <div class="mail-section-divider"></div>
        <div class="mail-flabel" style="margin-bottom: 8px">网络代理（收发信都走它）</div>
        <div class="mail-field-row">
          <div class="mail-field">
            <label class="mail-flabel">模式</label>
            <select
              class="mail-input"
              :value="settings.proxy?.mode || 'system'"
              @change="updateProxy({ mode: $event.target.value })"
            >
              <option value="none">直连</option>
              <option value="system">跟随系统</option>
              <option value="custom">自定义</option>
            </select>
          </div>
          <div class="mail-field">
            <label class="mail-flabel">代理地址</label>
            <input
              class="mail-input"
              :value="settings.proxy?.url || ''"
              :disabled="settings.proxy?.mode !== 'custom'"
              placeholder="socks5://127.0.0.1:1080"
              @change="updateProxy({ url: $event.target.value.trim() })"
            />
          </div>
        </div>
        <div class="mail-field">
          <label class="mail-flabel">不走代理的主机（逗号分隔）</label>
          <input
            class="mail-input"
            :value="(settings.proxy?.bypass || []).join(', ')"
            placeholder="corp.local, localhost"
            @change="updateBypass($event.target.value)"
          />
        </div>

        <!-- 同步与发送 -->
        <div class="mail-section-divider"></div>
        <div class="mail-field-row">
          <div class="mail-field">
            <label class="mail-flabel">自动同步间隔（秒）</label>
            <input
              class="mail-input"
              type="number"
              min="30"
              :value="settings.pollSeconds"
              @change="$emit('update', { pollSeconds: Math.max(30, Number($event.target.value) || 90) })"
            />
          </div>
          <div class="mail-field">
            <label class="mail-flabel">发送撤回窗口（秒，0 关闭）</label>
            <input
              class="mail-input"
              type="number"
              min="0"
              :value="settings.undoSendSeconds"
              @change="$emit('update', { undoSendSeconds: Math.max(0, Number($event.target.value) || 0) })"
            />
          </div>
        </div>

        <div class="mail-field">
          <label class="mail-flabel">本地邮件保留天数（0 = 永久；只清本地缓存，星标邮件不动）</label>
          <input
            class="mail-input"
            type="number"
            min="0"
            :value="settings.retentionDays"
            @change="$emit('update', { retentionDays: Math.max(0, Number($event.target.value) || 0) })"
          />
        </div>

        <label class="mail-check">
          <input
            type="checkbox"
            :checked="settings.inlineImages"
            @change="$emit('update', { inlineImages: $event.target.checked })"
          />
          默认加载邮件里的外部图片（关闭更安全：不给发件人回执信标）
        </label>
        <label class="mail-check">
          <input
            type="checkbox"
            :checked="settings.showHeaders"
            @change="$emit('update', { showHeaders: $event.target.checked })"
          />
          在阅读区显示原始邮件头
        </label>
        <label class="mail-check">
          <input
            type="checkbox"
            :checked="settings.groupThreads"
            @change="$emit('update', { groupThreads: $event.target.checked })"
          />
          在收件箱里按会话聚合同一主题的邮件
        </label>
        <div class="mail-section-divider"></div>
        <div class="mail-flabel" style="margin-bottom: 6px">本地加密</div>
        <label class="mail-check">
          <input
            type="checkbox"
            :checked="encryptionEnabled"
            @change="toggleEncryption($event.target.checked)"
          />
          正文与附件在本地数据库中加密存储（密钥存系统凭据库）
        </label>
        <p class="mail-hint" style="margin: 0 0 8px">
          关闭后新到的邮件正文明文落盘，已有密文仍可读。开启加密后，<strong>正文搜索</strong>不再可用
          ——密文无法建立全文索引；主题、发件人、收件人、时间的搜索不受影响。
        </p>

        <label class="mail-check">
          <input
            type="checkbox"
            :checked="settings.rulesEnabled"
            @change="$emit('update', { rulesEnabled: $event.target.checked })"
          />
          启用收信规则
        </label>
        <label class="mail-check">
          <input
            type="checkbox"
            :checked="settings.composeHtml"
            @change="$emit('update', { composeHtml: $event.target.checked })"
          />
          新邮件默认使用富文本
        </label>

        <div class="mail-section-divider"></div>
        <div class="mail-flabel" style="margin-bottom: 6px">存储位置</div>
        <p class="mail-hint" style="margin: 0 0 8px">
          当前目录：<code>{{ storage.activeDir || '（读取中）' }}</code>
          <template v-if="storage.dbBytes">
            · 数据库 {{ (storage.dbBytes / 1024 / 1024).toFixed(1) }} MB
          </template>
        </p>
        <div class="mail-field-row">
          <div class="mail-field">
            <label class="mail-flabel">目标目录</label>
            <input
              class="mail-input"
              :value="targetDir"
              placeholder="例如 D:\\Mail\\guaxi"
              @input="targetDir = $event.target.value"
            />
          </div>
          <div class="mail-field">
            <label class="mail-flabel">迁移</label>
            <button class="mail-btn" :disabled="migrating" @click="migrateStorage">
              {{ migrating ? '迁移中…' : '迁移到该目录' }}
            </button>
          </div>
        </div>
        <p class="mail-hint" style="margin: 0 0 10px">
          迁移会先复制并校验大小，成功后才切换；原库保留为 <code>mail.db.bak</code>，确认无误后可自行删除。
        </p>

        <div class="mail-flabel" style="margin-bottom: 6px">备份与恢复</div>
        <div style="display: flex; gap: 8px; flex-wrap: wrap">
          <button class="mail-btn" @click="backupDb">备份到文件…</button>
          <button class="mail-btn is-danger" @click="restoreDb">从备份恢复…</button>
        </div>
        <p class="mail-hint" style="margin: 6px 0 0">
          备份是完整的 SQLite 库（含所有账户的本地邮件）。恢复前会把当前库另存为
          <code>mail.db.before-restore</code>，恢复失败可手工退回去。
        </p>
      </div>

      <div class="mail-modal-foot">
        <button class="mail-btn is-danger" @click="clearCache">清空本地缓存</button>
        <span class="spacer"></span>
        <button class="mail-btn is-primary" @click="$emit('close')">完成</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps({
  settings: { type: Object, required: true },
  themes: { type: Array, default: () => [] },
})
const emit = defineEmits(['close', 'update'])

const densityOptions = [
  { value: 'compact', label: '紧凑' },
  { value: 'comfortable', label: '舒适' },
  { value: 'spacious', label: '宽松' },
]

const fonts = ref([])
const encryptionEnabled = ref(true)
const storage = ref({ activeDir: '', customDir: '', dbPath: '', dbBytes: 0 })
const migrating = ref(false)
const targetDir = ref('')

async function loadStorage() {
  try {
    storage.value = await invoke('mail_storage_info')
    targetDir.value = storage.value.customDir || ''
  } catch {
    /* 设置页拿不到存储信息不算致命，保持默认展示 */
  }
}

async function migrateStorage() {
  const target = (targetDir.value || '').trim()
  if (!target) {
    window.alert('请先填写目标目录')
    return
  }
  if (
    !window.confirm(
      `把邮件库迁移到：
${target}

会复制并校验，成功后切换；原库保留为 .bak。继续？`,
    )
  ) {
    return
  }
  migrating.value = true
  try {
    const result = await invoke('mail_migrate_storage', { targetDir: target })
    window.alert(`迁移完成：
${result.dbPath}

原库已备份为：
${result.backupPath}`)
    await loadStorage()
  } catch (error) {
    window.alert(String(error?.message || error))
  } finally {
    migrating.value = false
  }
}

async function backupDb() {
  const { save } = await import('@tauri-apps/plugin-dialog')
  const path = await save({
    title: '备份邮件库',
    defaultPath: `mail-backup-${new Date().toISOString().slice(0, 10)}.db`,
    filters: [{ name: '邮件库', extensions: ['db'] }],
  })
  if (!path) return
  try {
    await invoke('mail_backup', { targetPath: path })
    window.alert(`已备份到：
${path}`)
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

async function restoreDb() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const path = await open({
    multiple: false,
    title: '选择备份文件',
    filters: [{ name: '邮件库', extensions: ['db'] }],
  })
  if (!path) return
  if (!window.confirm('恢复会用备份覆盖当前本地邮件库（账户与设置也一起回到备份时的状态）。继续？')) {
    return
  }
  try {
    await invoke('mail_restore', { sourcePath: path })
    window.alert('恢复完成，即将重新加载。')
    window.location.reload()
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

onMounted(async () => {
  try {
    encryptionEnabled.value = await invoke('mail_get_encryption_enabled')
  } catch {
    encryptionEnabled.value = true
  }
})

async function toggleEncryption(enabled) {
  try {
    await invoke('mail_set_encryption_enabled', { enabled })
    encryptionEnabled.value = enabled
  } catch (error) {
    window.alert(String(error?.message || error))
    encryptionEnabled.value = !enabled
  }
}

onMounted(async () => {
  try {
    const families = await invoke('list_system_fonts')
    // 排到最前面的是等宽字体，阅读场景更需要衬线/正文类字体，所以再按名字排一次。
    fonts.value = (families || []).map((f) => f.name).slice(0, 200)
  } catch {
    fonts.value = []
  }
})

const fontOptions = computed(() => {
  const common = ['Source Han Serif SC', '思源宋体', '霞鹜文楷', 'LXGW WenKai', 'Microsoft YaHei', 'SimSun']
  const seen = new Set()
  return [...common, ...fonts.value].filter((name) => {
    if (!name || seen.has(name)) return false
    seen.add(name)
    return true
  })
})

const themeAccent = computed(
  () => props.themes.find((t) => t.id === props.settings.theme)?.tokens?.accent || '#5b7cfa',
)

function updateProxy(patch) {
  emit('update', { proxy: { ...(props.settings.proxy || {}), ...patch } })
}

function updateBypass(text) {
  const bypass = String(text || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
  updateProxy({ bypass })
}

async function clearCache() {
  if (!window.confirm('清空本地缓存的邮件？服务器上的邮件不会被删除，下次同步会重新拉取。')) return
  try {
    await invoke('mail_clear_cache', { accountId: null })
    window.location.reload()
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}
</script>

<style scoped>
.mail-section-divider {
  height: 1px;
  background: var(--mail-border);
  margin: 20px 0 16px;
}
</style>
