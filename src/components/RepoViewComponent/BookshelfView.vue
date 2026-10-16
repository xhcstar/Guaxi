<!-- BookshelfView.vue —— Kindle 风格书架视图
  设计说明：
  · 每个文档渲染为一本“书”，封面上印有文件名；点击行为与表格视图一致（交给父级 handleOpen）。
  · 木质层板用 repeating-linear-gradient 绘制：书高与行间距固定 → 每行周期恒定 → 层板可精准落在每排书下方。
  · 封面是一套低饱和的“书衣”色板，八个色相家族统一下来；浅色主题走纸面（浅底深字），
    深色主题走布面精装（深底浅字），同一本书在两个主题下是同一个家族、同一种气质的两种材质。
  · 主题跟随 :root[data-guaxi-theme='dark']，浅色为纸面＋灰木，深色为布面＋深木阅读室。 -->
<template>
  <div class="bookshelf">
    <div class="shelf-head">
      <span class="shelf-head-badge">书架</span>
      <span class="shelf-head-title" :title="library">{{ library || '全部文档' }}</span>
      <span class="shelf-head-count">{{ books.length }} 本</span>
      <span class="shelf-head-rule" />
    </div>

    <div class="shelf-scroll">
      <div v-if="books.length" class="shelf-grid">
        <button
          v-for="(book, i) in books"
          :key="book.id ?? (book.importFileName + i)"
          class="book"
          :data-cover="coverFamily(book)"
          :style="{ '--rise-delay': `${Math.min(i, 26) * 26}ms` }"
          :title="book.importFileName"
          @click="$emit('open', book)"
          @contextmenu="$emit('contextmenu', { event: $event, book })"
        >
          <span class="book-cover">
            <span class="cover-spine" />
            <span class="cover-pages" />
            <span class="cover-title">{{ displayTitle(book.importFileName) }}</span>
            <span class="cover-rule" />
            <span class="cover-meta">
              <span class="cover-type">{{ typeLabel(book) }}</span>
              <span v-if="book.library" class="cover-lib">{{ book.library }}</span>
            </span>
          </span>
        </button>
      </div>

      <div v-else class="shelf-empty">
        <div class="shelf-empty-art">
          <span /><span /><span /><span />
        </div>
        <p class="shelf-empty-title">这层书架还空着</p>
        <p class="shelf-empty-sub">导入文件后，它们会像书一样陈列在这里。</p>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  books: { type: Array, default: () => [] },
  library: { type: String, default: '' },
})
defineEmits(['open', 'contextmenu'])

// 文件类型 → 角标文案
const TYPE_LABEL = {
  pdf: 'PDF',
  markdown: 'MD',
  md: 'MD',
  epub: 'EPUB',
  html: 'HTML',
  htm: 'HTML',
  word: 'DOC',
  doc: 'DOC',
  docx: 'DOC',
  txt: 'TXT',
}

// 封面家族：只按文件名哈希取色，保证同名稳定、整架错落有致。
// 具体色值全部写在 CSS 里（明暗各一套），这里只负责挑家族。
const COVER_FAMILIES = ['ink', 'indigo', 'slate', 'pine', 'moss', 'chestnut', 'plum', 'wisteria']

const hashStr = (s) => {
  let h = 0
  const str = String(s || '')
  for (let i = 0; i < str.length; i += 1) h = (h * 31 + str.charCodeAt(i)) >>> 0
  return h
}

const displayTitle = (name) => {
  const n = String(name || '')
  const i = n.lastIndexOf('.')
  return i > 0 ? n.slice(0, i) : n
}

const typeLabel = (b) => {
  const t = String(b.type || '').toLowerCase()
  return TYPE_LABEL[t] || (t ? t.toUpperCase() : 'DOC')
}

const coverFamily = (book) =>
  COVER_FAMILIES[hashStr(book.importFileName || book.id || '') % COVER_FAMILIES.length]
</script>

<script>
export default { name: 'BookshelfView' }
</script>

<style scoped>
.bookshelf {
  /* ── 主题色板（浅色：与应用浅色面 #ffffff 协调的中性画布＋灰木层板） ── */
  --shelf-bg1: var(--guaxi-app-bg, #ffffff);
  --shelf-bg2: #f3f5f8;
  --shelf-vignette: rgba(52, 60, 76, 0.06);
  --shelf-top: #ded6c9;
  --shelf-face: #c6bba8;
  --shelf-edge: #a79b87;
  --shelf-under: rgba(58, 52, 40, 0.30);
  --head-text: #3b3a35;
  --badge-bg: #3a3833;
  --badge-fg: #f5f2ec;
  --book-shadow-cast: rgba(34, 40, 52, 0.15);
  --book-shadow-core: rgba(34, 40, 52, 0.33);
  --book-ring: rgba(255, 255, 255, 0.5);
  /* 纸面材质：顶部一层薄高光，底部极轻的压暗 */
  --cover-light: linear-gradient(180deg, rgba(255, 255, 255, 0.46), rgba(255, 255, 255, 0) 48%, rgba(0, 0, 0, 0.05));
  --cover-grain: 0.42;
  --c-frame: rgba(0, 0, 0, 0.13);
  --c-spine-a: rgba(0, 0, 0, 0.17);
  --c-spine-b: rgba(0, 0, 0, 0);
  --c-hair: rgba(0, 0, 0, 0.10);
  --page-a: #f2eee3;
  --page-b: #dcd4c4;
  --book-serif: Georgia, 'Noto Serif SC', 'Source Han Serif SC', 'Songti SC', 'STSong', 'SimSun', Cambria, 'Times New Roman', serif;
  --book-mono: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, monospace;

  position: relative;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background:
    radial-gradient(120% 80% at 50% -10%, rgba(255, 255, 255, 0.35), transparent 60%),
    radial-gradient(140% 120% at 50% 120%, var(--shelf-vignette), transparent 55%),
    linear-gradient(180deg, var(--shelf-bg1), var(--shelf-bg2));
}

/* 细腻纸纹噪点叠层 */
.bookshelf::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  opacity: 0.5;
  mix-blend-mode: multiply;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.05'/%3E%3C/svg%3E");
}

/* ── 顶部书架铭牌 ── */
.shelf-head {
  position: relative;
  z-index: 1;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 26px 12px;
}
.shelf-head-badge {
  font-family: var(--book-mono);
  font-size: 11px;
  letter-spacing: 3px;
  color: var(--badge-fg);
  background: var(--badge-bg);
  padding: 4px 12px;
  border-radius: 3px;
}
.shelf-head-title {
  font-family: var(--book-serif);
  font-size: 26px;
  font-weight: 600;
  line-height: 1;
  color: var(--head-text);
  letter-spacing: 0.3px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 46%;
}
.shelf-head-count {
  font-family: var(--book-mono);
  font-size: 12px;
  color: var(--head-text);
  opacity: 0.6;
}
.shelf-head-rule {
  flex: 1;
  height: 1px;
  background: linear-gradient(90deg, var(--shelf-edge), transparent);
  opacity: 0.7;
}

/* ── 滚动区 ── */
.shelf-scroll {
  position: relative;
  z-index: 1;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 30px 30px 8px;
}

/* ── 书格栅 + 木层板 ──
   书高 244 + 行距 56 = 周期 300；层板绘制在每个周期的 244–268 区间，
   底部 padding 补一个行距，使最后一排书也有层板。 */
.shelf-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, 172px);
  justify-content: space-between;
  column-gap: 22px;
  row-gap: 56px;
  padding-bottom: 56px;
  background-image: repeating-linear-gradient(
    to bottom,
    transparent 0,
    transparent 244px,
    rgba(255, 255, 255, 0.30) 244px,
    rgba(255, 255, 255, 0.30) 245px,
    var(--shelf-top) 245px,
    var(--shelf-face) 258px,
    var(--shelf-edge) 266px,
    var(--shelf-under) 267px,
    var(--shelf-under) 271px,
    transparent 272px,
    transparent 300px
  );
}

/* ── 封面配色家族 ──
   八个家族只在色相上有别，明度与饱和度统一：浅色主题是同一批纸的八种底色，
   深色主题是同一批布面的八种底色，整架看起来是一个系列，而不是一堆色块。 */
.book[data-cover='ink'] {
  --c-bg: hsl(220 10% 82%);
  --c-ink: hsl(220 12% 20%);
  --c-rule: hsl(220 10% 44%);
  --c-soft: hsl(220 9% 41%);
}
.book[data-cover='indigo'] {
  --c-bg: hsl(228 24% 82%);
  --c-ink: hsl(228 26% 21%);
  --c-rule: hsl(228 36% 50%);
  --c-soft: hsl(228 20% 41%);
}
.book[data-cover='slate'] {
  --c-bg: hsl(202 22% 81%);
  --c-ink: hsl(204 26% 20%);
  --c-rule: hsl(202 34% 46%);
  --c-soft: hsl(202 20% 40%);
}
.book[data-cover='pine'] {
  --c-bg: hsl(158 20% 80%);
  --c-ink: hsl(162 24% 19%);
  --c-rule: hsl(160 28% 40%);
  --c-soft: hsl(160 16% 38%);
}
.book[data-cover='moss'] {
  --c-bg: hsl(72 22% 81%);
  --c-ink: hsl(74 24% 20%);
  --c-rule: hsl(72 26% 40%);
  --c-soft: hsl(72 18% 38%);
}
.book[data-cover='chestnut'] {
  --c-bg: hsl(26 32% 82%);
  --c-ink: hsl(24 30% 21%);
  --c-rule: hsl(24 40% 48%);
  --c-soft: hsl(24 24% 41%);
}
.book[data-cover='plum'] {
  --c-bg: hsl(342 26% 84%);
  --c-ink: hsl(342 26% 22%);
  --c-rule: hsl(342 34% 52%);
  --c-soft: hsl(342 20% 43%);
}
.book[data-cover='wisteria'] {
  --c-bg: hsl(268 24% 84%);
  --c-ink: hsl(268 24% 22%);
  --c-rule: hsl(268 28% 54%);
  --c-soft: hsl(268 18% 44%);
}

/* ── 单本书 ── */
.book {
  position: relative;
  width: 172px;
  height: 244px;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
  perspective: 900px;
  outline: none;
}
/* 落在层板上的接地投影 */
.book::after {
  content: '';
  position: absolute;
  left: 10px;
  right: 6px;
  bottom: -8px;
  height: 16px;
  z-index: 0;
  background: radial-gradient(ellipse at 42% 50%, var(--book-shadow-core), transparent 72%);
  filter: blur(2.5px);
  opacity: 0.75;
  transition: opacity 0.35s ease, transform 0.35s ease;
}

.book-cover {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  flex-direction: column;
  padding: 26px 20px 18px 30px;
  text-align: left;
  border-radius: 2px 5px 5px 2px;
  background-color: var(--c-bg);
  background-image: var(--cover-light);
  box-shadow:
    0 10px 20px var(--book-shadow-cast),
    0 2px 5px rgba(20, 24, 34, 0.10),
    inset 0 0 0 1px var(--book-ring);
  overflow: hidden;
  transform-origin: left center;
  transition: transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.4s ease;
  animation: book-rise 0.55s both;
  animation-delay: var(--rise-delay);
}
/* 布面/纸纹质感：极淡的一层噪点，让平涂的底色不至于像一块色板 */
.book-cover::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  opacity: var(--cover-grain);
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='90' height='90'%3E%3Cfilter id='g'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23g)' opacity='0.07'/%3E%3C/svg%3E");
}
/* 藏书票式的内框细线 */
.book-cover::after {
  content: '';
  position: absolute;
  inset: 10px 10px 10px 14px;
  pointer-events: none;
  border: 1px solid var(--c-frame);
  border-radius: 1px;
}

/* 书脊（左侧暗带 + 装订压线） */
.cover-spine {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 9px;
  background: linear-gradient(to right, var(--c-spine-a), var(--c-spine-b));
}
.cover-spine::after {
  content: '';
  position: absolute;
  left: 12px;
  top: 12px;
  bottom: 12px;
  width: 1px;
  background: var(--c-hair);
}
/* 右侧书页层叠 */
.cover-pages {
  position: absolute;
  right: 0;
  top: 4px;
  bottom: 4px;
  width: 4px;
  border-radius: 0 3px 3px 0;
  background: repeating-linear-gradient(to bottom, var(--page-a) 0 1px, var(--page-b) 1px 2px);
  box-shadow: inset -1px 0 2px rgba(0, 0, 0, 0.22);
}

.cover-title {
  position: relative;
  z-index: 1;
  font-family: var(--book-serif);
  font-size: 17px;
  line-height: 1.4;
  font-weight: 600;
  letter-spacing: 0.3px;
  color: var(--c-ink);
  max-width: 122px;
  display: -webkit-box;
  -webkit-line-clamp: 5;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}
/* 唯一的一点颜色：标题下的一小段细线 */
.cover-rule {
  position: relative;
  z-index: 1;
  margin-top: 13px;
  width: 34px;
  height: 2px;
  border-radius: 1px;
  background: var(--c-rule);
}

.cover-meta {
  position: absolute;
  left: 30px;
  right: 16px;
  bottom: 17px;
  z-index: 1;
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 6px;
}
.cover-type {
  font-family: var(--book-mono);
  font-size: 10px;
  letter-spacing: 2px;
  color: var(--c-soft);
  white-space: nowrap;
}
.cover-lib {
  font-family: var(--book-mono);
  font-size: 10px;
  color: var(--c-soft);
  opacity: 0.85;
  max-width: 76px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
}

/* 悬停：抬起 + 微开本，露出书脊厚度 */
.book:hover .book-cover {
  transform: translateY(-12px) rotateY(-16deg);
  box-shadow:
    -14px 20px 36px var(--book-shadow-core),
    inset 0 0 0 1px var(--book-ring);
}
.book:hover::after {
  opacity: 0.45;
  transform: translateY(-6px) scaleX(0.9);
}
.book:focus-visible .book-cover {
  outline: 2px solid var(--c-rule);
  outline-offset: 3px;
}

@keyframes book-rise {
  from { opacity: 0; transform: translateY(16px); }
  to { opacity: 1; transform: none; }
}

/* ── 空书架 ── */
.shelf-empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--head-text);
}
.shelf-empty-art {
  display: flex;
  align-items: flex-end;
  gap: 7px;
  height: 78px;
  padding: 0 16px 12px;
  border-bottom: 5px solid var(--shelf-face);
  border-radius: 2px;
  box-shadow: 0 8px 14px -8px var(--shelf-under);
  margin-bottom: 18px;
  opacity: 0.65;
}
.shelf-empty-art span {
  width: 17px;
  border-radius: 2px 3px 0 0;
  background: linear-gradient(180deg, var(--shelf-top), var(--shelf-edge));
  box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.14);
}
.shelf-empty-art span:nth-child(1) { height: 58px; }
.shelf-empty-art span:nth-child(2) { height: 70px; transform: rotate(4deg); transform-origin: bottom; }
.shelf-empty-art span:nth-child(3) { height: 50px; }
.shelf-empty-art span:nth-child(4) { height: 64px; }
.shelf-empty-title {
  font-family: var(--book-serif);
  font-size: 22px;
  font-weight: 600;
  margin: 0;
}
.shelf-empty-sub {
  font-family: var(--book-mono);
  font-size: 12px;
  margin: 0;
  opacity: 0.7;
}

/* 滚动条 */
.shelf-scroll::-webkit-scrollbar { width: 10px; }
.shelf-scroll::-webkit-scrollbar-thumb {
  background: var(--shelf-under);
  border-radius: 10px;
  border: 3px solid transparent;
  background-clip: padding-box;
}

/* ── 深色主题：布面精装。封面比应用底（One Dark #282c34）略亮一档才立得住，
   层板换成同色系的深木，铭牌收成一层淡淡的浮起。 ── */
:root[data-guaxi-theme='dark'] .bookshelf {
  --shelf-bg1: #2f343d;
  --shelf-bg2: #282c34;
  --shelf-vignette: rgba(0, 0, 0, 0.18);
  --shelf-top: #565159;
  --shelf-face: #423e47;
  --shelf-edge: #302d35;
  --shelf-under: rgba(0, 0, 0, 0.46);
  --head-text: #d8dee9;
  --badge-bg: rgba(255, 255, 255, 0.12);
  --badge-fg: #dfe3ea;
  --book-shadow-cast: rgba(0, 0, 0, 0.40);
  --book-shadow-core: rgba(0, 0, 0, 0.52);
  --book-ring: rgba(255, 255, 255, 0.07);
  --cover-light: linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0) 46%, rgba(0, 0, 0, 0.20));
  --cover-grain: 0.34;
  --c-frame: rgba(255, 255, 255, 0.13);
  --c-spine-a: rgba(0, 0, 0, 0.38);
  --c-spine-b: rgba(0, 0, 0, 0.02);
  --c-hair: rgba(255, 255, 255, 0.11);
  --page-a: #efe7d4;
  --page-b: #c9bba0;
  background:
    radial-gradient(120% 80% at 50% -10%, rgba(255, 244, 222, 0.06), transparent 60%),
    linear-gradient(180deg, var(--shelf-bg1), var(--shelf-bg2));
}
:root[data-guaxi-theme='dark'] .bookshelf::before {
  mix-blend-mode: soft-light;
  opacity: 0.35;
}

:root[data-guaxi-theme='dark'] .book[data-cover='ink'] {
  --c-bg: hsl(220 8% 27%);
  --c-ink: hsl(220 8% 91%);
  --c-rule: hsl(220 8% 62%);
  --c-soft: hsl(220 7% 70%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='indigo'] {
  --c-bg: hsl(228 22% 27%);
  --c-ink: hsl(228 18% 92%);
  --c-rule: hsl(228 34% 66%);
  --c-soft: hsl(228 16% 72%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='slate'] {
  --c-bg: hsl(202 20% 26%);
  --c-ink: hsl(202 16% 91%);
  --c-rule: hsl(202 32% 62%);
  --c-soft: hsl(202 14% 70%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='pine'] {
  --c-bg: hsl(160 20% 25%);
  --c-ink: hsl(158 14% 91%);
  --c-rule: hsl(158 26% 58%);
  --c-soft: hsl(158 12% 68%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='moss'] {
  --c-bg: hsl(74 18% 25%);
  --c-ink: hsl(74 14% 91%);
  --c-rule: hsl(74 24% 56%);
  --c-soft: hsl(74 12% 68%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='chestnut'] {
  --c-bg: hsl(26 22% 26%);
  --c-ink: hsl(26 20% 92%);
  --c-rule: hsl(26 34% 62%);
  --c-soft: hsl(26 16% 70%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='plum'] {
  --c-bg: hsl(342 20% 27%);
  --c-ink: hsl(342 16% 92%);
  --c-rule: hsl(342 30% 66%);
  --c-soft: hsl(342 14% 72%);
}
:root[data-guaxi-theme='dark'] .book[data-cover='wisteria'] {
  --c-bg: hsl(268 20% 28%);
  --c-ink: hsl(268 16% 92%);
  --c-rule: hsl(268 26% 68%);
  --c-soft: hsl(268 14% 73%);
}
</style>
