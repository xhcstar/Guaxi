<!-- BookshelfView.vue —— Kindle 风格书架视图
  设计说明：
  · 每个文档渲染为一本“书”，封面上印有文件名；点击行为与表格视图一致（交给父级 handleOpen）。
  · 木质层板用 repeating-linear-gradient 绘制：书高与行间距固定 → 每行周期恒定 → 层板可精准落在每排书下方。
  · 封面配色按文件名哈希 + 文件类型决定，保证同名稳定、整体丰富而克制（书卷气的低饱和深色）。
  · 主题跟随 :root[data-guaxi-theme='dark']，浅色为暖纸/胡桃，深色为深木阅读室。 -->
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
          :style="coverStyle(book, i)"
          :title="book.importFileName"
          @click="$emit('open', book)"
          @contextmenu="$emit('contextmenu', { event: $event, book })"
        >
          <span class="book-cover">
            <span class="cover-spine" />
            <span class="cover-pages" />
            <span class="cover-sheen" />
            <span class="cover-title">{{ displayTitle(book.importFileName) }}</span>
            <span class="cover-foil" />
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

// 文件类型 → 主色相 + 角标文案（书脊配色家族）
const TYPE_ACCENT = {
  pdf: { hue: 354, name: 'PDF' },
  markdown: { hue: 168, name: 'MD' },
  md: { hue: 168, name: 'MD' },
  epub: { hue: 268, name: 'EPUB' },
  html: { hue: 28, name: 'HTML' },
  htm: { hue: 28, name: 'HTML' },
  word: { hue: 214, name: 'DOC' },
  doc: { hue: 214, name: 'DOC' },
  docx: { hue: 214, name: 'DOC' },
  txt: { hue: 205, name: 'TXT' },
}

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
  return TYPE_ACCENT[t]?.name || (t ? t.toUpperCase() : 'DOC')
}

// 生成封面 CSS 变量：渐变两色 + 烫金强调色 + 入场动画延迟
const coverStyle = (book, index) => {
  const t = String(book.type || '').toLowerCase()
  const accent = TYPE_ACCENT[t]
  const h = hashStr(book.importFileName || book.id || '')
  const baseHue = accent ? (accent.hue + (h % 14) - 7 + 360) % 360 : h % 360
  const hue2 = (baseHue + 22) % 360
  const sat = 38 + (h % 16) // 38–53%
  const lig = 28 + (h % 9) //  28–36%
  return {
    '--cover-a': `hsl(${baseHue} ${sat + 4}% ${lig + 9}%)`,
    '--cover-b': `hsl(${hue2} ${sat + 8}% ${lig - 5}%)`,
    '--accent': `hsl(${baseHue} 72% 64%)`,
    '--rise-delay': `${Math.min(index, 26) * 26}ms`,
  }
}
</script>

<script>
export default { name: 'BookshelfView' }
</script>

<style scoped>
.bookshelf {
  /* ── 主题色板（浅色：与应用浅色面 #ffffff 协调的中性画布＋胡桃木层板） ── */
  --shelf-bg1: var(--guaxi-app-bg, #ffffff);
  --shelf-bg2: #f3f5f8;
  --shelf-vignette: rgba(60, 72, 96, 0.07);
  --shelf-top: #e8caa0;
  --shelf-face: #cda067;
  --shelf-edge: #b9874f;
  --shelf-under: rgba(60, 38, 14, 0.42);
  --book-text: #f6efe2;
  --head-text: #5a4326;
  --book-serif: 'Cormorant Garamond', 'Noto Serif CJK SC', 'Source Han Serif SC', 'Songti SC', Georgia, 'Times New Roman', serif;
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
  color: var(--book-text);
  background: linear-gradient(135deg, #b9874f, #8a5e2c);
  padding: 4px 12px;
  border-radius: 4px;
  box-shadow: 0 2px 6px rgba(80, 50, 18, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.25);
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
  opacity: 0.7;
}
.shelf-head-rule {
  flex: 1;
  height: 2px;
  border-radius: 2px;
  background: linear-gradient(90deg, var(--shelf-edge), transparent);
  opacity: 0.55;
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
    rgba(255, 255, 255, 0.28) 244px,
    rgba(255, 255, 255, 0.28) 245px,
    var(--shelf-top) 245px,
    var(--shelf-face) 258px,
    var(--shelf-edge) 266px,
    var(--shelf-under) 267px,
    var(--shelf-under) 271px,
    transparent 272px,
    transparent 300px
  );
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
  background: radial-gradient(ellipse at 42% 50%, rgba(0, 0, 0, 0.42), transparent 72%);
  filter: blur(2.5px);
  opacity: 0.8;
  transition: opacity 0.35s ease, transform 0.35s ease;
}

.book-cover {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  flex-direction: column;
  padding: 20px 16px 16px 24px;
  text-align: left;
  border-radius: 3px 7px 7px 3px;
  background: linear-gradient(135deg, var(--cover-a), var(--cover-b));
  box-shadow:
    0 12px 24px rgba(40, 24, 8, 0.32),
    inset 0 0 0 1px rgba(255, 255, 255, 0.07),
    inset 2px 0 5px rgba(0, 0, 0, 0.22);
  overflow: hidden;
  transform-origin: left center;
  transition: transform 0.4s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.4s ease;
  animation: book-rise 0.55s both;
  animation-delay: var(--rise-delay);
}

/* 书脊（左侧暗带 + 装订压线） */
.cover-spine {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 11px;
  background: linear-gradient(to right, rgba(0, 0, 0, 0.42), rgba(0, 0, 0, 0.04));
}
.cover-spine::after {
  content: '';
  position: absolute;
  left: 13px;
  top: 10px;
  bottom: 10px;
  width: 1px;
  background: rgba(255, 255, 255, 0.16);
}
/* 右侧书页层叠 */
.cover-pages {
  position: absolute;
  right: 0;
  top: 4px;
  bottom: 4px;
  width: 5px;
  border-radius: 0 4px 4px 0;
  background: repeating-linear-gradient(to bottom, #efe7d4 0 1px, #c9bba0 1px 2px);
  box-shadow: inset -1px 0 2px rgba(0, 0, 0, 0.25);
}
/* 顶部高光 */
.cover-sheen {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(125deg, rgba(255, 255, 255, 0.24), rgba(255, 255, 255, 0) 44%);
}

.cover-title {
  font-family: var(--book-serif);
  font-size: 19px;
  line-height: 1.22;
  font-weight: 600;
  letter-spacing: 0.2px;
  color: var(--book-text);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
  max-width: 128px;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}
.cover-foil {
  margin-top: 10px;
  width: 46px;
  height: 2px;
  border-radius: 2px;
  background: linear-gradient(90deg, var(--accent), transparent);
  box-shadow: 0 0 6px var(--accent);
  opacity: 0.85;
}

.cover-meta {
  position: absolute;
  left: 24px;
  right: 13px;
  bottom: 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}
.cover-type {
  font-family: var(--book-mono);
  font-size: 10px;
  letter-spacing: 1px;
  color: var(--book-text);
  background: rgba(0, 0, 0, 0.26);
  border: 1px solid rgba(255, 255, 255, 0.2);
  padding: 2px 8px;
  border-radius: 20px;
  white-space: nowrap;
}
.cover-lib {
  font-family: var(--book-mono);
  font-size: 10px;
  color: rgba(255, 255, 255, 0.72);
  max-width: 88px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
}

/* 悬停：抬起 + 微开本，露出书脊厚度 */
.book:hover .book-cover {
  transform: translateY(-12px) rotateY(-16deg);
  box-shadow:
    -16px 20px 38px rgba(30, 18, 6, 0.46),
    inset 0 0 0 1px rgba(255, 255, 255, 0.1);
}
.book:hover::after {
  opacity: 0.5;
  transform: translateY(-6px) scaleX(0.9);
}
.book:focus-visible .book-cover {
  outline: 2px solid var(--accent);
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
  border-bottom: 6px solid var(--shelf-face);
  border-radius: 2px;
  box-shadow: 0 8px 14px -8px var(--shelf-under);
  margin-bottom: 18px;
  opacity: 0.65;
}
.shelf-empty-art span {
  width: 17px;
  border-radius: 2px 3px 0 0;
  background: linear-gradient(135deg, #b9874f, #8a5e2c);
  box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.18);
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

/* ── 深色主题：与应用深色面（One Dark #282c34）协调，只加一层很淡的暖木，
   不再叠 multiply 噪点 / 大面积暗角，避免书架区比其它页面明显更黑。 ── */
:root[data-guaxi-theme='dark'] .bookshelf {
  --shelf-bg1: #2f343d;
  --shelf-bg2: #282c34;
  --shelf-vignette: rgba(0, 0, 0, 0.18);
  --shelf-top: #6b5236;
  --shelf-face: #564023;
  --shelf-edge: #3a2c18;
  --shelf-under: rgba(0, 0, 0, 0.40);
  --book-text: #f3ead9;
  --head-text: #d8dee9;
  background:
    radial-gradient(120% 80% at 50% -10%, rgba(255, 244, 222, 0.06), transparent 60%),
    linear-gradient(180deg, var(--shelf-bg1), var(--shelf-bg2));
}
</style>
