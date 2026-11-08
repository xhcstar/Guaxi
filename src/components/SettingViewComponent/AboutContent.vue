<template>
  <div class="about-shell" :class="{ 'is-dark': isDarkTheme }">
    <header class="about-hero">
      <div class="about-mark" aria-hidden="true">
        <span>G</span>
        <small>Guaxi</small>
      </div>

      <div class="about-hero-copy">
        <div class="about-eyebrow">
          <span class="about-dot"></span>
          GUAXI · ABOUT
        </div>
        <h1 class="about-name">Guaxi</h1>
        <p class="about-tagline">AI 驱动的个人知识工作台</p>
        <p class="about-lede">
          把本地文件、Markdown 写作、资料阅读、AI 助手、知识图谱、外部知识源、任务与工具应用放进同一套桌面界面，
          让「收集 → 阅读 → 理解 → 写作 → 沉淀」成为一条连续的路径。
        </p>

        <div class="about-actions">
          <button type="button" class="about-btn is-primary" @click="openManual">
            <Icon :icon="bookOpenIcon" width="15" height="15" />
            打开用户手册
          </button>
          <button type="button" class="about-btn" @click="openExternal(repos.github.url)">
            GitHub
            <Icon :icon="arrowUpRightIcon" width="13" height="13" />
          </button>
          <button type="button" class="about-btn" @click="openExternal(repos.gitee.url)">
            Gitee
            <Icon :icon="arrowUpRightIcon" width="13" height="13" />
          </button>
        </div>
      </div>

      <div class="about-seal" aria-hidden="true">
        <div class="about-seal-ring ring-1"></div>
        <div class="about-seal-ring ring-2"></div>
        <div class="about-seal-ring ring-3"></div>
        <div class="about-seal-core">
          <b>{{ version }}</b>
          <span>DESKTOP</span>
        </div>
      </div>
    </header>

    <div class="about-meta-strip">
      <div class="about-meta-item">
        <span>版本</span>
        <b>v{{ version }}</b>
      </div>
      <div class="about-meta-item">
        <span>运行环境</span>
        <b>{{ platformLabel }}</b>
      </div>
      <div class="about-meta-item">
        <span>许可</span>
        <b>AGPL-3.0</b>
      </div>
      <div class="about-meta-item">
        <span>数据</span>
        <b>本地优先</b>
      </div>
    </div>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">01</span>
        <h2>它是什么</h2>
        <span class="about-section-en">WHAT IT IS</span>
      </div>
      <p class="about-p">
        Guaxi 是一款面向个人知识工作流的桌面软件。它不要求你把资料上传到某个云端工作区，
        而是直接用本机目录组织知识库、资源库和任务库，再在这些真实文件之上叠加编辑、阅读、检索、图谱与 AI 能力。
      </p>
      <p class="about-p">
        它的核心不是“再做一个聊天框”，而是把 AI 放进已有的工作链路里：阅读器里选中一段就能翻译或提问，
        编辑器里可以用插件批量处理表格与链接，Agent 可以读文件、跑命令、给出 diff，
        认知层则在你不在的时候读你已经拥有的材料，攒出可以被推翻的结论。
      </p>
    </section>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">02</span>
        <h2>核心能力</h2>
        <span class="about-section-en">CAPABILITIES</span>
      </div>
      <div class="about-grid">
        <div v-for="(item, index) in capabilities" :key="item.title" class="about-card">
          <span class="about-card-no">{{ String(index + 1).padStart(2, '0') }}</span>
          <div class="about-card-title">{{ item.title }}</div>
          <p class="about-card-text">{{ item.text }}</p>
        </div>
      </div>
    </section>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">03</span>
        <h2>技术栈</h2>
        <span class="about-section-en">STACK</span>
      </div>
      <div class="about-stack">
        <div v-for="group in stackGroups" :key="group.title" class="about-stack-group">
          <div class="about-stack-title">{{ group.title }}</div>
          <div class="about-chips">
            <span v-for="item in group.items" :key="item" class="about-chip">{{ item }}</span>
          </div>
        </div>
      </div>
      <p class="about-note">
        软件本身用 Rust + Tauri 打包，前端是 Vue 3 + Vite；阅读、编辑、图谱、白板与图表分别由独立的渲染内核承担，
        Python 插件与本地语音模型在进程内运行，不依赖额外的云端服务。
      </p>
    </section>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">04</span>
        <h2>数据与隐私</h2>
        <span class="about-section-en">LOCAL FIRST</span>
      </div>
      <ul class="about-list">
        <li><b>文件在你的磁盘上</b>：知识库、资源库、任务库都是普通目录，可以随时用外部编辑器、Git 或文件管理器接管。</li>
        <li><b>模型密钥留在本机</b>：在线模型的 API Key 由软件保存并调用，插件只拿到补全结果，接触不到密钥。</li>
        <li><b>认知层默认安静</b>：剪贴板与屏幕采集出厂关闭；向量、新颖度和矛盾检测优先走本地模型。</li>
        <li><b>同步由你选择</b>：P2P、WebDAV、Syncthing 或 Git 远程仓库，四种方式分别配置，不强制经过同一台服务器。</li>
      </ul>
    </section>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">05</span>
        <h2>版本与许可</h2>
        <span class="about-section-en">VERSION &amp; LICENSE</span>
      </div>
      <dl class="about-defs">
        <div class="about-def">
          <dt>当前版本</dt>
          <dd>v{{ version }} · 桌面端</dd>
        </div>
        <div class="about-def">
          <dt>安装目标</dt>
          <dd>Windows · NSIS 安装包</dd>
        </div>
        <div class="about-def">
          <dt>开源许可</dt>
          <dd>GNU Affero General Public License v3.0（AGPL-3.0）</dd>
        </div>
        <div class="about-def">
          <dt>源码仓库</dt>
          <dd class="about-repo-row">
            <button type="button" class="about-link" @click="openExternal(repos.github.url)">
              <Icon :icon="gitBranchIcon" width="13" height="13" />
              {{ repos.github.label }}
              <Icon :icon="arrowUpRightIcon" width="12" height="12" />
            </button>
            <button type="button" class="about-link" @click="openExternal(repos.gitee.url)">
              <Icon :icon="gitBranchIcon" width="13" height="13" />
              {{ repos.gitee.label }}
              <Icon :icon="arrowUpRightIcon" width="12" height="12" />
            </button>
            <button
              type="button"
              class="about-copy"
              @click="copyText(`${repos.github.url}\n${repos.gitee.url}`)"
            >
              <Icon :icon="copied === 'repos' ? checkIcon : copyIcon" width="12" height="12" />
              {{ copied === 'repos' ? '已复制' : '复制仓库地址' }}
            </button>
          </dd>
        </div>
      </dl>
      <p class="about-note">
        在 AGPL-3.0 下，你可以自由使用、修改和分发本软件；如果你把它作为网络服务提供给他人，
        需要按同一许可公开对应的源代码。完整条款见仓库根目录的 <code>LICENSE</code>。
      </p>
    </section>

    <section class="about-section">
      <div class="about-section-head">
        <span class="about-section-no">06</span>
        <h2>作者与致谢</h2>
        <span class="about-section-en">AUTHOR &amp; CREDITS</span>
      </div>
      <div class="about-author">
        <div class="about-author-mark">xhc</div>
        <div class="about-author-main">
          <div class="about-author-name">xhcstar <span>作者 / 维护者</span></div>
          <div class="about-author-lines">
            <span><Icon :icon="mailIcon" width="13" height="13" /> 联系邮箱：待补充</span>
            <span><Icon :icon="scaleIcon" width="13" height="13" /> 个人主页：待补充</span>
          </div>
          <p class="about-author-note">
            作者信息将在正式发布前补齐。当前如需反馈问题或提交建议，请通过上方仓库的 Issues 入口联系。
          </p>
        </div>
      </div>

      <div class="about-credits">
        <div class="about-credits-title">本软件建立在这些开源项目之上</div>
        <div class="about-chips">
          <span v-for="item in credits" :key="item" class="about-chip">{{ item }}</span>
        </div>
      </div>
    </section>

    <footer class="about-footer">
      <div class="about-footer-rule"></div>
      <div class="about-footer-main">
        <span>Guaxi · 智能知识管理系统</span>
        <span>Built with Rust &amp; Vue · AGPL-3.0</span>
      </div>
      <p>© {{ year }} Guaxi. 资料属于你，工具只是把它们串起来。</p>
    </footer>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { Icon } from '@iconify/vue'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import bookOpenIcon from '@iconify-icons/lucide/book-open'
import arrowUpRightIcon from '@iconify-icons/lucide/arrow-up-right'
import copyIcon from '@iconify-icons/lucide/copy'
import checkIcon from '@iconify-icons/lucide/check'
import gitBranchIcon from '@iconify-icons/lucide/git-branch'
import mailIcon from '@iconify-icons/lucide/mail'
import scaleIcon from '@iconify-icons/lucide/scale'
import { useSettingsStore } from '../../stores/settingsStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('AboutContent')
const settingsStore = useSettingsStore()

const version = ref('0.1.0')
const platformLabel = ref('Windows · 桌面端')
const copied = ref('')
const year = new Date().getFullYear()

const repos = Object.freeze({
  github: { label: 'github.com/xhcstar/Guaxi', url: 'https://github.com/xhcstar/Guaxi' },
  gitee: { label: 'github.com/xhcstar/Guaxi', url: 'https://github.com/xhcstar/Guaxi' },
})

const capabilities = [
  { title: '知识编辑', text: 'Monaco 编辑器、预览与分屏、模板、表格工具、Markdown 格式化与浏览器静态预览。' },
  { title: '资料阅读', text: 'PDF、Word、EPUB、表格、幻灯片、图片与音视频统一入口；摘录、翻译、朗读与笔记。' },
  { title: 'Agent 工作台', text: '任务目录 + 沙箱；读文件、跑命令、看 diff，逐动作审批，运行中可以转向。' },
  { title: '知识图谱与大脑', text: '3D 图谱、语义检索、社区折叠；认知层在后台攒主张、矛盾与缺口。' },
  { title: '知识源聚合', text: 'RSS、整站抓取、平台订阅与登录队列；收藏和摘录直接插入编辑器。' },
  { title: '工具与应用', text: '邮件、相册、音乐、翻译、传输、OCR、终端、简历、旅行计划等独立窗口。' },
]

const stackGroups = [
  { title: '桌面与界面', items: ['Rust', 'Tauri 2', 'Vue 3', 'Vite', 'Pinia', 'Naive UI'] },
  { title: '知识与阅读', items: ['Monaco Editor', 'PDF.js', 'Markdown', 'ECharts', 'Excalidraw', 'Graphology'] },
  { title: 'AI 与本地能力', items: ['OpenAI 兼容接口', 'LocalDeepseek / LocalQwen', 'llama.cpp', 'sherpa-onnx', 'Python 运行时'] },
  { title: '数据与同步', items: ['Git', 'P2P / iroh', 'WebDAV', 'Syncthing', 'SQLite', 'jieba 分词'] },
]

const credits = [
  'Tauri',
  'Vue.js',
  'Vite',
  'Naive UI',
  'Monaco Editor',
  'PDF.js',
  'ECharts',
  'Excalidraw',
  'Graphology',
  'sherpa-onnx',
  'llama.cpp',
  'jieba',
]

const isDarkTheme = computed(() => {
  const theme = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (theme === 'dark') return true
  if (theme === 'light') return false
  return document?.documentElement?.dataset?.guaxiTheme === 'dark'
})

const openManual = () => {
  logger.info('[About] open manual')
  settingsStore.setSelectedMenu('manual')
}

const openExternal = async (url) => {
  logger.info('[About] open external link', { url })
  try {
    await openUrl(url)
  } catch (error) {
    logger.warn('[About] opener unavailable, fallback to window.open', { url, error })
    window.open(url, '_blank', 'noopener,noreferrer')
  }
}

const copyText = async (text) => {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
  }
  copied.value = 'repos'
  window.setTimeout(() => {
    if (copied.value === 'repos') copied.value = ''
  }, 1600)
}

onMounted(async () => {
  try {
    version.value = await getVersion()
  } catch {
    version.value = '0.1.0'
  }

  try {
    const { platform, arch } = await import('@tauri-apps/plugin-os')
    const platformName = await platform()
    const archName = await arch()
    const platformMap = {
      windows: 'Windows',
      macos: 'macOS',
      linux: 'Linux',
      ios: 'iOS',
      android: 'Android',
    }
    platformLabel.value = `${platformMap[platformName] || platformName || '桌面端'} · ${archName || 'desktop'}`
  } catch {
    const ua = navigator.userAgent || ''
    if (/Windows/i.test(ua)) platformLabel.value = 'Windows · 桌面端'
    else if (/Mac/i.test(ua)) platformLabel.value = 'macOS · 桌面端'
    else if (/Linux/i.test(ua)) platformLabel.value = 'Linux · 桌面端'
    else platformLabel.value = '桌面端'
  }

  logger.info('[About] mounted', {
    version: version.value,
    platform: platformLabel.value,
  })
})
</script>

<style scoped>
.about-shell {
  --a-paper: var(--guaxi-panel-bg);
  --a-surface: var(--guaxi-content-bg);
  --a-surface-2: color-mix(in srgb, var(--guaxi-panel-bg) 86%, var(--guaxi-text-primary) 14%);
  --a-ink: var(--guaxi-text-primary);
  --a-muted: var(--guaxi-text-secondary);
  --a-line: var(--guaxi-border);
  --a-line-strong: color-mix(in srgb, var(--guaxi-border) 64%, var(--guaxi-text-secondary) 36%);
  --a-accent: #4078f2;
  --a-accent-ink: #4078f2;
  --a-accent-soft: color-mix(in srgb, var(--a-accent) 14%, transparent);
  --a-blue: var(--a-accent);
  --a-blue-soft: color-mix(in srgb, var(--a-blue) 11%, transparent);
  --a-body-font: 'Noto Sans SC', 'Source Han Sans SC', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif;
  --a-display: 'Iowan Old Style', 'Palatino Linotype', 'Source Han Serif SC', 'Noto Serif SC', 'Songti SC', serif;
  --a-mono: var(--guaxi-editor-font-family, 'Maple Mono NF CN', Consolas, monospace);
  --a-shadow: 0 12px 28px rgba(0, 0, 0, 0.14);

  width: 100%;
  container-type: inline-size;
  color: var(--a-ink);
  font-family: var(--a-body-font);
  background:
    radial-gradient(900px 380px at 8% -12%, var(--a-accent-soft), transparent 62%),
    var(--a-paper);
  box-sizing: border-box;
}

.about-shell.is-dark {
  --a-accent: #61afef;
  --a-accent-ink: #61afef;
  --a-shadow: 0 16px 38px rgba(0, 0, 0, 0.32);
}

/* 封面 */
.about-hero {
  position: relative;
  display: grid;
  grid-template-columns: 132px minmax(0, 1fr) 170px;
  gap: 26px;
  align-items: center;
  padding: 34px 34px 28px;
  border-bottom: 1px solid var(--a-line);
  overflow: hidden;
}

.about-mark {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 124px;
  height: 124px;
  border: 1px solid var(--a-line-strong);
  border-radius: 26px;
  background: var(--a-surface);
  box-shadow: var(--a-shadow);
}

.about-mark::before {
  content: '';
  position: absolute;
  inset: 7px;
  border: 1px solid var(--a-line);
  border-radius: 20px;
}

.about-mark span {
  position: relative;
  font-family: var(--a-display);
  font-size: 64px;
  line-height: 0.85;
  color: var(--a-accent);
}

.about-mark small {
  position: relative;
  margin-top: 8px;
  font-family: var(--a-mono);
  font-size: 9px;
  letter-spacing: 0.24em;
  text-transform: uppercase;
  color: var(--a-muted);
}

.about-eyebrow {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--a-mono);
  font-size: 10.5px;
  letter-spacing: 0.18em;
  color: var(--a-muted);
}

.about-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--a-accent);
  box-shadow: 0 0 0 4px var(--a-accent-soft);
}

.about-name {
  margin: 8px 0 0;
  font-family: var(--a-display);
  font-size: clamp(34px, 4.4vw, 52px);
  line-height: 1.05;
  font-weight: 600;
  letter-spacing: 0.01em;
  color: var(--a-ink);
}

.about-tagline {
  margin: 6px 0 0;
  font-family: var(--a-display);
  font-size: 16px;
  color: var(--a-accent-ink);
}

.about-lede {
  max-width: 680px;
  margin: 12px 0 0;
  font-size: 13.5px;
  line-height: 1.85;
  color: var(--a-muted);
}

.about-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 16px;
}

.about-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 13px;
  border: 1px solid var(--a-line-strong);
  border-radius: 999px;
  background: var(--a-surface);
  color: var(--a-ink);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: transform 0.16s ease, border-color 0.16s ease, background 0.16s ease;
}

.about-btn:hover {
  transform: translateY(-1px);
  border-color: var(--a-accent);
}

.about-btn.is-primary {
  border-color: var(--a-accent);
  background: var(--a-accent);
  color: #ffffff;
}

.about-btn.is-primary:hover {
  background: var(--a-accent-ink);
}

.about-seal {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 150px;
  height: 150px;
  justify-self: end;
}

.about-seal-ring {
  position: absolute;
  border: 1px solid var(--a-line-strong);
  border-radius: 50%;
}

.ring-1 {
  inset: 0;
  opacity: 0.5;
}

.ring-2 {
  inset: 18px;
  border-style: dashed;
  opacity: 0.65;
}

.ring-3 {
  inset: 38px;
  border-color: var(--a-accent);
  opacity: 0.45;
}

.about-seal-core {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 10px 14px;
  border-radius: 12px;
  background: var(--a-surface);
  box-shadow: var(--a-shadow);
}

.about-seal-core b {
  font-family: var(--a-display);
  font-size: 20px;
  color: var(--a-accent);
}

.about-seal-core span {
  font-family: var(--a-mono);
  font-size: 8.5px;
  letter-spacing: 0.16em;
  color: var(--a-muted);
}

/* 元信息条 */
.about-meta-strip {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  border-bottom: 1px solid var(--a-line);
  background: var(--a-surface);
}

.about-meta-item {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 12px 18px;
  border-right: 1px solid var(--a-line);
}

.about-meta-item:last-child {
  border-right: 0;
}

.about-meta-item span {
  font-family: var(--a-mono);
  font-size: 9.5px;
  letter-spacing: 0.12em;
  color: var(--a-muted);
}

.about-meta-item b {
  font-size: 13px;
  font-weight: 600;
  color: var(--a-ink);
}

/* 章节 */
.about-section {
  padding: 30px 34px 4px;
}

.about-section-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding-bottom: 12px;
  border-bottom: 2px solid var(--a-ink);
}

.about-section-no {
  font-family: var(--a-mono);
  font-size: 10.5px;
  color: var(--a-accent);
}

.about-section-head h2 {
  margin: 0;
  font-family: var(--a-display);
  font-size: 23px;
  font-weight: 600;
  color: var(--a-ink);
}

.about-section-en {
  margin-left: auto;
  font-family: var(--a-mono);
  font-size: 9.5px;
  letter-spacing: 0.16em;
  color: var(--a-muted);
}

.about-p {
  margin: 12px 0 0;
  font-size: 13.8px;
  line-height: 1.95;
  color: var(--a-ink);
}

.about-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin-top: 14px;
}

.about-card {
  padding: 14px;
  border: 1px solid var(--a-line);
  border-radius: 12px;
  background: var(--a-surface);
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
}

.about-card:hover {
  transform: translateY(-2px);
  border-color: var(--a-line-strong);
  box-shadow: var(--a-shadow);
}

.about-card-no {
  font-family: var(--a-display);
  font-size: 19px;
  color: var(--a-accent);
}

.about-card-title {
  margin-top: 6px;
  font-size: 13.5px;
  font-weight: 700;
  color: var(--a-ink);
}

.about-card-text {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.75;
  color: var(--a-muted);
}

/* 技术栈 */
.about-stack {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 14px;
}

.about-stack-group {
  padding: 13px 15px;
  border: 1px solid var(--a-line);
  border-radius: 12px;
  background: var(--a-surface);
}

.about-stack-title {
  margin-bottom: 8px;
  font-size: 12.5px;
  font-weight: 700;
  color: var(--a-ink);
}

.about-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.about-chip {
  padding: 3px 9px;
  border: 1px solid var(--a-line);
  border-radius: 999px;
  background: var(--a-surface-2);
  font-family: var(--a-mono);
  font-size: 10.5px;
  color: var(--a-muted);
}

.about-note {
  margin: 12px 0 0;
  font-size: 12.2px;
  line-height: 1.85;
  color: var(--a-muted);
}

.about-note code {
  padding: 1px 5px;
  border: 1px solid var(--a-line);
  border-radius: 4px;
  background: var(--a-surface-2);
  font-family: var(--a-mono);
  font-size: 0.94em;
}

/* 列表与定义 */
.about-list {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
}

.about-list li {
  position: relative;
  padding: 8px 0 8px 18px;
  border-bottom: 1px dashed var(--a-line);
  font-size: 13px;
  line-height: 1.85;
  color: var(--a-muted);
}

.about-list li::before {
  content: '';
  position: absolute;
  left: 2px;
  top: 1.02em;
  width: 6px;
  height: 6px;
  border-radius: 1px;
  background: var(--a-accent);
  transform: rotate(45deg);
}

.about-list b {
  color: var(--a-ink);
}

.about-defs {
  display: grid;
  gap: 0;
  margin: 14px 0 0;
  border-top: 1px solid var(--a-line);
}

.about-def {
  display: grid;
  grid-template-columns: 104px minmax(0, 1fr);
  gap: 14px;
  padding: 10px 0;
  border-bottom: 1px solid var(--a-line);
}

.about-def dt {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--a-accent-ink);
}

.about-def dd {
  margin: 0;
  font-size: 12.8px;
  line-height: 1.8;
  color: var(--a-muted);
}

.about-repo-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.about-link,
.about-copy {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 9px;
  border: 1px solid var(--a-line);
  border-radius: 999px;
  background: var(--a-surface);
  color: var(--a-accent-ink);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: border-color 0.16s ease, background 0.16s ease;
}

.about-link:hover,
.about-copy:hover {
  border-color: var(--a-accent);
  background: var(--a-accent-soft);
}

.about-copy {
  color: var(--a-muted);
}

/* 作者 */
.about-author {
  display: flex;
  gap: 16px;
  margin-top: 14px;
  padding: 16px;
  border: 1px solid var(--a-line);
  border-radius: 14px;
  background: var(--a-surface);
}

.about-author-mark {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 58px;
  height: 58px;
  border-radius: 50%;
  background: var(--a-accent-soft);
  border: 1px solid var(--a-accent);
  font-family: var(--a-display);
  font-size: 19px;
  color: var(--a-accent-ink);
}

.about-author-main {
  min-width: 0;
}

.about-author-name {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 15px;
  font-weight: 700;
  color: var(--a-ink);
}

.about-author-name span {
  padding: 1px 7px;
  border: 1px solid var(--a-line);
  border-radius: 999px;
  font-family: var(--a-mono);
  font-size: 9.5px;
  font-weight: 500;
  letter-spacing: 0.08em;
  color: var(--a-muted);
}

.about-author-lines {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 18px;
  margin-top: 8px;
  font-size: 12px;
  color: var(--a-muted);
}

.about-author-lines span {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.about-author-note {
  margin: 8px 0 0;
  font-size: 12px;
  line-height: 1.8;
  color: var(--a-muted);
}

.about-credits {
  margin-top: 14px;
  padding: 14px 16px;
  border: 1px dashed var(--a-line-strong);
  border-radius: 12px;
}

.about-credits-title {
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 700;
  color: var(--a-ink);
}

/* 页脚 */
.about-footer {
  margin-top: 34px;
  padding: 0 34px 30px;
  color: var(--a-muted);
}

.about-footer-rule {
  height: 10px;
  margin-bottom: 14px;
  border-top: 1px solid var(--a-line-strong);
  border-bottom: 1px solid var(--a-line);
}

.about-footer-main {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px;
  font-family: var(--a-mono);
  font-size: 10.5px;
  letter-spacing: 0.04em;
}

.about-footer p {
  margin: 8px 0 0;
  font-size: 11.5px;
}

/* 响应式：设置页侧栏会吃掉宽度，按“关于页自身宽度”而不是视口宽度换布局。 */
@container (max-width: 900px) {
  .about-hero {
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 20px;
    padding: 28px 24px 24px;
  }

  .about-mark {
    width: 104px;
    height: 104px;
  }

  .about-mark span {
    font-size: 54px;
  }

  .about-seal {
    display: none;
  }

  .about-meta-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .about-meta-item:nth-child(2) {
    border-right: 0;
  }

  .about-meta-item:nth-child(-n + 2) {
    border-bottom: 1px solid var(--a-line);
  }

  .about-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .about-section {
    padding: 26px 24px 2px;
  }

  .about-footer {
    padding: 0 24px 26px;
  }
}

@container (max-width: 620px) {
  .about-hero {
    grid-template-columns: minmax(0, 1fr);
    padding: 24px 20px 20px;
  }

  .about-mark {
    width: 84px;
    height: 84px;
    border-radius: 20px;
  }

  .about-mark span {
    font-size: 42px;
  }

  .about-mark small {
    display: none;
  }

  .about-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .about-stack {
    grid-template-columns: minmax(0, 1fr);
  }

  .about-meta-strip {
    grid-template-columns: minmax(0, 1fr);
  }

  .about-meta-item {
    border-right: 0;
    border-bottom: 1px solid var(--a-line);
  }

  .about-def {
    grid-template-columns: minmax(0, 1fr);
    gap: 3px;
  }

  .about-section {
    padding: 24px 20px 2px;
  }

  .about-footer {
    padding: 0 20px 24px;
  }

  .about-author {
    flex-direction: column;
  }

  .about-section-en {
    display: none;
  }
}
</style>
