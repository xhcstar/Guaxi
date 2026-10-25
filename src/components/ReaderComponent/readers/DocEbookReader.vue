<template>
  <div ref="ebookRoot" class="guaxi-doc-reader" data-reader="ebook">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ title || name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kindLabel }}</span>
      <span v-if="sections.length" class="guaxi-doc-reader__meta gdr-ebook__section-count">
        {{ activeIndex + 1 }} / {{ sections.length }}
      </span>
      <span class="guaxi-doc-reader__spacer" />

      <!--
        搜索条就放在工具栏中间那段空白里（原先是纯 spacer），不再另起一整行。
        阅读时正文最值钱，为了一个搜索框把正文整体下推 34px 没有道理；
        这里按需展开、按需收起，工具栏高度始终不变。
      -->
      <div v-if="searchOpen" class="gdr-ebook__search">
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          class="gdr-ebook__search-input"
          type="search"
          placeholder="在全书范围内搜索…"
          aria-label="在全书范围内搜索，Enter 下一个，Shift+Enter 上一个，Esc 关闭"
          @input="runSearch"
          @keydown.enter.prevent="nextMatch($event.shiftKey ? -1 : 1)"
          @keydown.esc.prevent="toggleSearch"
        />
        <span class="guaxi-doc-reader__meta gdr-ebook__search-count">
          {{ searchHits.length ? `${searchCursor + 1} / ${searchHits.length}` : (searchQuery ? '无结果' : '') }}
        </span>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="上一个匹配（Shift+Enter）"
          aria-label="上一个匹配"
          :disabled="!searchHits.length"
          @click="nextMatch(-1)"
        >
          <Icon :icon="arrowUpIcon" width="15" height="15" />
        </button>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="下一个匹配（Enter）"
          aria-label="下一个匹配"
          :disabled="!searchHits.length"
          @click="nextMatch(1)"
        >
          <Icon :icon="arrowDownIcon" width="15" height="15" />
        </button>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="关闭搜索（Esc）"
          aria-label="关闭搜索"
          @click="toggleSearch"
        >
          <Icon :icon="closeIcon" width="15" height="15" />
        </button>
      </div>

      <!-- 图标按钮统一带 title + aria-label：鼠标悬停有提示，读屏也能读。 -->
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': searchOpen }"
        title="全文搜索（Ctrl+F）"
        aria-label="全文搜索"
        :aria-pressed="searchOpen"
        @click="toggleSearch"
      >
        <Icon :icon="searchIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'bookmarks' }"
        title="书签（Ctrl+D 添加当前页）"
        aria-label="书签"
        :aria-pressed="panelMode === 'bookmarks'"
        @click="togglePanel('bookmarks')"
      >
        <Icon :icon="bookmarkIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'notes' }"
        :title="`标注与笔记${annotations.length ? `（${annotations.length} 条）` : ''}`"
        aria-label="标注与笔记"
        :aria-pressed="panelMode === 'notes'"
        @click="togglePanel('notes')"
      >
        <Icon :icon="noteIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': panelMode === 'display' }"
        title="阅读设置：字号、字体、主题"
        aria-label="阅读设置"
        :aria-pressed="panelMode === 'display'"
        @click="togglePanel('display')"
      >
        <Icon :icon="typeIcon" width="15" height="15" />
      </button>

      <span class="guaxi-doc-reader__divider" />

      <!--
        缩放：电子书正文是流式的，缩放等同于改字号，所以这里和「显示设置」里的字号
        滑杆共用 zoom —— 读者从工具栏点 + 时，正文立刻变大，不需要先去开面板。
      -->
      <div class="gdr-ebook__zoom" role="group" aria-label="缩放">
        <button
          class="guaxi-doc-reader__icon-btn"
          title="缩小（Ctrl+-）"
          aria-label="缩小"
          :disabled="zoom <= 0.5"
          @click="zoomBy(0.9)"
        >
          <Icon :icon="minusIcon" width="15" height="15" />
        </button>
        <span class="gdr-ebook__zoom-val" :title="`当前缩放 ${Math.round(zoom * 100)}%`">{{ Math.round(zoom * 100) }}%</span>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="放大（Ctrl+=）"
          aria-label="放大"
          :disabled="zoom >= 3"
          @click="zoomBy(1.1)"
        >
          <Icon :icon="plusIcon" width="15" height="15" />
        </button>
      </div>

      <!--
        节导航收成一枚胶囊：← [12] / 340 60% →。
        三样东西是同一件事（我在哪、怎么挪），分组之后工具栏的节奏和 PDF 那边一致，
        也不会因为夹在图标按钮中间而显得零散。搜索展开时导航保持可见 ——
        「搜到一段再翻回上一节对照」是阅读时的常见动作，不能在这时把它藏起来。
      -->
      <span class="gdr-ebook__nav">
        <button
          class="guaxi-doc-reader__icon-btn"
          title="上一节（←）"
          aria-label="上一节"
          :disabled="activeIndex <= 0"
          @click="goSection(activeIndex - 1)"
        >
          <Icon :icon="chevronLeftIcon" width="15" height="15" />
        </button>
        <!--
          节定位：可直接输入节号跳转。电子书常有几百节，只靠 ← → 一节一节挪
          在长篇里根本不可用；输入框比滑杆精确（读者知道自己在第几节，不知道百分比）。
        -->
        <span class="gdr-ebook__section-jump" :title="progressLabel ? `全书进度 ${progressLabel}` : ''">
          <input
            v-model="sectionInput"
            class="gdr-ebook__section-input"
            type="number"
            min="1"
            :max="sections.length || 1"
            aria-label="跳转到第几节"
            @keydown.enter.prevent="commitSectionInput"
            @blur="syncSectionInput"
          />
          <span class="gdr-ebook__section-total">/ {{ sections.length }}</span>
          <span class="gdr-ebook__progress">{{ progressLabel }}</span>
        </span>
        <button
          class="guaxi-doc-reader__icon-btn"
          title="下一节（→）"
          aria-label="下一节"
          :disabled="activeIndex >= sections.length - 1"
          @click="goSection(activeIndex + 1)"
        >
          <Icon :icon="chevronRightIcon" width="15" height="15" />
        </button>
      </span>

      <span class="guaxi-doc-reader__divider" />

      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': tocVisible }"
        title="目录"
        aria-label="目录"
        :aria-pressed="tocVisible"
        @click="tocVisible = !tocVisible"
      >
        <Icon :icon="tocIcon" width="15" height="15" />
      </button>

      <!--
        朗读：从当前位置读起，边读边高亮当前句；再次点击停止。
        「点哪读哪」由正文里的点击接管（见 onFrameLoad 的 dblclick 分流）。
      -->
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': readAloudPlaying }"
        :title="readAloudPlaying ? '停止朗读' : '从当前位置朗读'"
        :aria-label="readAloudPlaying ? '停止朗读' : '朗读'"
        :aria-pressed="readAloudPlaying"
        @click="toggleReadAloud"
      >
        <Icon :icon="readAloudPlaying ? stopIcon : volumeIcon" width="15" height="15" />
      </button>
    </div>


    <!-- 划词译文面板：与编辑器 / PDF 阅读器同一套组件，交互一致。 -->
    <GuaxiSelectionTranslatePanel
      v-if="selectionTranslate.visible.value"
      :pos="selectionTranslate.pos.value"
      :raw="selectionTranslate.raw.value"
      :result="selectionTranslate.result.value"
      :error="selectionTranslate.error.value"
      :busy="selectionTranslate.busy.value"
      :pinned="selectionTranslate.pinned.value"
      :items="selectionTranslate.items.value"
      :from-lang="selectionTranslate.fromLang.value"
      :to-lang="selectionTranslate.toLang.value"
      :lang-options="selectionTranslate.langOptions"
      :word-meta="selectionTranslate.wordMeta.value"
      @translate="selectionTranslate.translate(selectionTranslate.raw.value)"
      @copy="selectionTranslate.copyResult()"
      @swap-langs="selectionTranslate.swapLangs()"
      @toggle-pin="selectionTranslate.togglePin()"
      @update:pos="(v) => { selectionTranslate.pos.value = v }"
      @update:from-lang="(v) => { selectionTranslate.fromLang.value = v }"
      @update:to-lang="(v) => { selectionTranslate.toLang.value = v }"
      @close="selectionTranslate.close()"
    />

    <!--
      划词操作条：浮在选区上方，图标按钮 + title 提示。
      高亮图标直接用默认颜色落笔；调色盘图标才展开颜色，避免常用动作前先挑颜色。
    -->
    <div
      v-if="selectionVisible"
      class="gdr-ebook__selbar"
      :style="{ left: `${selectionPos.left}px`, top: `${selectionPos.top}px` }"
      role="toolbar"
      aria-label="选中文字操作"
      @mousedown.prevent
    >
      <button
        class="gdr-ebook__selbar-btn"
        title="翻译选中的文字"
        aria-label="翻译"
        @click="translateSelectionText"
      >
        <Icon :icon="languagesIcon" width="15" height="15" />
      </button>
      <button
        class="gdr-ebook__selbar-btn"
        title="朗读选中的文字"
        aria-label="朗读"
        @click="speakSelectionText"
      >
        <Icon :icon="volumeIcon" width="15" height="15" />
      </button>
      <span class="gdr-ebook__selbar-sep" />
      <button
        class="gdr-ebook__selbar-btn"
        title="高亮（默认颜色）"
        aria-label="高亮"
        @click="highlightSelection(DEFAULT_HIGHLIGHT_COLOR)"
      >
        <Icon :icon="highlighterIcon" width="15" height="15" />
      </button>
      <button
        class="gdr-ebook__selbar-btn"
        :class="{ 'is-on': selectionPaletteOpen }"
        title="选择高亮颜色"
        aria-label="高亮颜色"
        :aria-expanded="selectionPaletteOpen"
        @click.stop="selectionPaletteOpen = !selectionPaletteOpen"
      >
        <Icon :icon="paletteIcon" width="15" height="15" />
      </button>
      <span v-if="selectionPaletteOpen" class="gdr-ebook__selbar-swatches">
        <button
          v-for="preset in HIGHLIGHT_PRESETS"
          :key="preset.color"
          class="gdr-ebook__selbar-swatch"
          :style="{ background: preset.color }"
          :title="`${preset.label}色高亮`"
          :aria-label="`${preset.label}色高亮`"
          @click.stop="highlightSelection(preset.color)"
        />
      </span>
      <button
        class="gdr-ebook__selbar-btn"
        title="关闭（Esc）"
        aria-label="关闭"
        @click="hideSelectionBar"
      >
        <Icon :icon="closeIcon" width="13" height="13" />
      </button>
    </div>

    <div v-if="loading" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">
        <div>正在解析电子书…</div>
        <div class="guaxi-doc-reader__meta">首次打开大文件可能需要几秒</div>
      </div>
    </div>
    <div v-else-if="errorText" class="guaxi-doc-reader__body">
      <div class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
    </div>
    <div v-else class="guaxi-doc-reader__body gdr-ebook">
      <aside v-if="tocVisible" class="gdr-ebook__toc" :style="{ width: `${tocWidth}px` }">
        <button
          v-for="(item, idx) in toc"
          :key="`${idx}-${item.href}`"
          class="gdr-ebook__toc-item"
          :class="{ 'is-active': tocActiveIdx === idx }"
          :style="{ paddingLeft: `${8 + item.depth * 12}px` }"
          :title="item.label"
          @click="jumpToHref(item.href)"
        >
          {{ item.label || '(未命名章节)' }}
        </button>
        <div v-if="!toc.length" class="guaxi-doc-reader__meta" style="padding: 8px">
          本书没有可用的目录信息
        </div>
      </aside>
      <div
        v-if="tocVisible && toc.length"
        class="gdr-ebook__splitter"
        :class="{ 'is-active': resizing }"
        role="separator"
        aria-orientation="vertical"
        aria-label="拖动调整目录宽度"
        @mousedown.prevent="startTocResize"
      />
      <div class="gdr-ebook__main" :class="{ 'is-resizing': resizing }">
        <!--
          用 blob URL + src 而不是 srcdoc：章节文档一次成型、可直接挂载，
          切章时 revoke 掉即可释放内存；也避开 srcdoc 先创建 about:blank
          过渡文档的那一跳。

          v-if 而不是常驻：iframe 一旦插入就会先创建一个 about:blank 文档；
          只有 frameSrc 就绪才挂载，且挂载时直接 src 到 blob，跳过空白文档阶段。

          不加 sandbox：Tauri 会往每个新文档注入 IPC 初始化脚本，sandbox 一开，
          这份注入脚本就会被 Chromium 拦住并逐帧打印 "Blocked script execution…"；
          书籍脚本的防线改为「送进 iframe 前彻底剥离 + 文档内 CSP script-src 'none'」
          （见 sanitizeSectionHtml 与 buildFrameDoc）。
        -->
        <iframe
          v-if="frameSrc"
          ref="frameRef"
          class="gdr-ebook__frame"
          referrerpolicy="no-referrer"
          :src="frameSrc"
          @load="onFrameLoad"
        />
      </div>

      <!-- 右侧面板：书签 / 笔记 / 显示设置。与左侧目录对称，可独立开关。 -->
      <aside v-if="panelMode" class="gdr-ebook__panel" :style="{ width: `${panelWidth}px` }">
        <div class="gdr-ebook__panel-head">
          <span>{{ panelTitle }}</span>
          <button
            class="guaxi-doc-reader__icon-btn"
            title="关闭面板（Esc）"
            aria-label="关闭面板"
            @click="panelMode = ''"
          >
            <Icon :icon="closeIcon" width="15" height="15" />
          </button>
        </div>

        <!-- 书签 -->
        <div v-if="panelMode === 'bookmarks'" class="gdr-ebook__panel-body">
          <button class="gdr-ebook__panel-action" @click="addBookmarkAtCurrent">
            <Icon :icon="bookmarkIcon" width="14" height="14" />
            在当前位置添加书签
          </button>
          <div v-if="!bookmarks.length" class="guaxi-doc-reader__meta" style="padding: 12px 8px">
            还没有书签。阅读时点「+ 在当前位置添加书签」。
          </div>
          <div
            v-for="bm in bookmarks"
            :key="bm.id"
            class="gdr-ebook__panel-item"
            :title="bm.label"
          >
            <button class="gdr-ebook__panel-item-main" @click="jumpToBookmark(bm)">
              <span class="gdr-ebook__panel-item-title">{{ bm.label || '未命名书签' }}</span>
              <span class="gdr-ebook__panel-item-sub">
                第 {{ bm.sectionIndex + 1 }} 节 · {{ formatTime(bm.createdAt) }}
              </span>
            </button>
            <button class="gdr-ebook__panel-item-del" title="删除书签" aria-label="删除书签" @click="removeBookmark(bm)">
              <Icon :icon="closeIcon" width="13" height="13" />
            </button>
          </div>
        </div>

        <!-- 笔记 / 高亮 -->
        <div v-else-if="panelMode === 'notes'" class="gdr-ebook__panel-body">
          <div class="guaxi-doc-reader__meta" style="padding: 8px 4px">
            在正文中选中文字后点「高亮」即可添加。共 {{ annotations.length }} 条。
          </div>
          <div v-if="!annotations.length" class="guaxi-doc-reader__meta" style="padding: 12px 8px">
            还没有标注。
          </div>
          <div v-for="an in annotations" :key="an.id" class="gdr-ebook__panel-item is-note">
            <button class="gdr-ebook__panel-item-main" @click="jumpToAnnotation(an)">
              <span class="gdr-ebook__note-quote" :style="{ borderLeftColor: an.color }">
                {{ an.text }}
              </span>
              <span v-if="an.note" class="gdr-ebook__note-body">{{ an.note }}</span>
              <span class="gdr-ebook__panel-item-sub">第 {{ an.sectionIndex + 1 }} 节 · {{ formatTime(an.createdAt) }}</span>
            </button>
            <button class="gdr-ebook__panel-item-del" title="删除标注" aria-label="删除标注" @click="removeAnnotation(an)">
              <Icon :icon="closeIcon" width="13" height="13" />
            </button>
          </div>
        </div>

        <!-- 显示设置 -->
        <div v-else-if="panelMode === 'display'" class="gdr-ebook__panel-body">
          <label class="gdr-ebook__setting">
            <span>字号</span>
            <input v-model.number="prefs.fontSize" type="range" min="12" max="30" step="1" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.fontSize }}px</span>
          </label>
          <label class="gdr-ebook__setting">
            <span>行距</span>
            <input v-model.number="prefs.lineHeight" type="range" min="1.2" max="2.6" step="0.1" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.lineHeight.toFixed(1) }}</span>
          </label>
          <label class="gdr-ebook__setting">
            <span>页宽</span>
            <input v-model.number="prefs.maxWidth" type="range" min="500" max="1400" step="20" @change="applyPrefs" />
            <span class="gdr-ebook__setting-val">{{ prefs.maxWidth }}px</span>
          </label>
          <div class="gdr-ebook__setting">
            <span>字体</span>
            <div class="gdr-ebook__seg">
              <button
                v-for="opt in fontOptions"
                :key="opt.value"
                :class="{ 'is-on': prefs.fontFamily === opt.value }"
                @click="setFontFamily(opt.value)"
              >{{ opt.label }}</button>
            </div>
          </div>
          <div class="gdr-ebook__setting">
            <span>主题</span>
            <div class="gdr-ebook__seg">
              <button
                v-for="opt in themeOptions"
                :key="opt.value"
                :class="{ 'is-on': prefs.theme === opt.value }"
                @click="setTheme(opt.value)"
              >{{ opt.label }}</button>
            </div>
          </div>
          <div class="gdr-ebook__setting">
            <span>翻页</span>
            <div class="gdr-ebook__seg">
              <button :class="{ 'is-on': prefs.layoutMode === 'scroll' }" @click="setLayoutMode('scroll')">滚动</button>
              <button :class="{ 'is-on': prefs.layoutMode === 'paginated' }" @click="setLayoutMode('paginated')">分页</button>
            </div>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup>
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
// 图标走「import 对象」路径（不是 iconRegistry 里的名字）：release 包离线可用，
// 名字写错在构建期就报模块找不到，不会变成一片空白。
import searchIcon from '@iconify-icons/lucide/search'
import bookmarkIcon from '@iconify-icons/lucide/bookmark'
import noteIcon from '@iconify-icons/lucide/edit-3'
import typeIcon from '@iconify-icons/lucide/type'
import chevronLeftIcon from '@iconify-icons/lucide/chevron-left'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import tocIcon from '@iconify-icons/lucide/list-tree'
import closeIcon from '@iconify-icons/lucide/x'
import arrowUpIcon from '@iconify-icons/lucide/arrow-up'
import arrowDownIcon from '@iconify-icons/lucide/arrow-down'
import volumeIcon from '@iconify-icons/lucide/volume-2'
import stopIcon from '@iconify-icons/lucide/square'
import minusIcon from '@iconify-icons/lucide/minus'
import plusIcon from '@iconify-icons/lucide/plus'
import highlighterIcon from '@iconify-icons/lucide/highlighter'
import paletteIcon from '@iconify-icons/lucide/palette'
import languagesIcon from '@iconify-icons/lucide/languages'
import { openUrl } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../../utils/logger'
import { useReaderReadAloud } from '../../../composables/useReaderReadAloud'
import { useSelectionTranslate } from '../../../composables/useSelectionTranslate'
import { useSelectionReadAloud } from '../../../composables/useSelectionReadAloud'
import './readerPane.css'
import { sanitizeSectionHtml } from './ebookSanitize.js'
import { normalizeWithMap } from '../readerSearch'

// 划词翻译面板是阅读器里唯一用到 naive-ui 的组件，且只在真正翻译时才需要出现。
// 异步加载让「打开一本书」不必先付 naive-ui 的解析成本。
const GuaxiSelectionTranslatePanel = defineAsyncComponent(
  () => import('../../HomeViewComponent/GuaxiSelectionTranslatePanel.vue')
)

const logger = createLogger('DocEbookReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'epub' },
})

const frameRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const title = ref('')
const sections = ref([])
const toc = ref([])
const activeIndex = ref(0)
const tocVisible = ref(true)
// 拖拽目录分隔线期间为 true：给正文挂遮罩，避免事件被 iframe 吃掉。
const resizing = ref(false)
const tocWidth = ref(250)
const zoom = ref(1)
const frameDoc = ref('')
/** 绑定给 iframe 的 blob URL（模板用 v-if + :src，先算好再挂载）。 */
const frameSrc = ref('')

const kindLabel = computed(() => String(props.kind || '').toUpperCase())

// ───────────────────────── 阅读设置 ─────────────────────────
//
// 字体与主题只影响 iframe 内部的 CSS，不改任何章节 HTML。

const FONT_STACKS = {
  // 「默认」= 主界面 / 编辑器同一套字体（--guaxi-editor-font-family）。
  // iframe 是独立文档，拿不到父页面的 CSS 变量，只能把同一份字体栈再写一遍。
  system:
    "'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
  serif: '"Noto Serif SC", "Songti SC", "SimSun", Georgia, serif',
  sans: '"Noto Sans SC", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  mono: '"Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", Consolas, monospace',
}

const THEME_VARS = {
  light: { bg: '#ffffff', fg: '#1f2328', link: '#2f6fdf', border: 'rgba(128,128,128,.35)' },
  sepia: { bg: '#f5ecd9', fg: '#3b3226', link: '#8b5e34', border: 'rgba(139,94,52,.35)' },
  gray: { bg: '#e9ecef', fg: '#2b2f33', link: '#2f6fdf', border: 'rgba(43,47,51,.25)' },
  dark: { bg: '#1c1f22', fg: '#d6d9dc', link: '#6cb6ff', border: 'rgba(214,217,220,.25)' },
}

const fontOptions = [
  { value: 'system', label: '默认' },
  { value: 'serif', label: '衬线' },
  { value: 'sans', label: '黑体' },
  { value: 'mono', label: '等宽' },
]
const themeOptions = [
  { value: 'light', label: '白' },
  { value: 'sepia', label: '护眼' },
  { value: 'gray', label: '灰' },
  { value: 'dark', label: '深色' },
]

/** 与后端 ReaderPrefs::default() 保持一致；prefsVersion 用于迁移旧默认（16px 衬线）。 */
const PREFS_VERSION = 2
const prefs = ref({
  prefsVersion: PREFS_VERSION,
  fontSize: 18,
  lineHeight: 1.8,
  fontFamily: 'system',
  theme: 'light',
  layoutMode: 'scroll',
  maxWidth: 860,
})

/** 一本书的稳定标识：规范化路径。跨窗口/跨重启一致，才能恢复进度。 */
const bookKey = computed(() => String(props.path || '').replace(/\\/g, '/').trim())

// ───────────────────────── 面板状态 ─────────────────────────

const panelMode = ref('')
const panelWidth = ref(320)
const panelTitle = computed(() => {
  if (panelMode.value === 'bookmarks') return '书签'
  if (panelMode.value === 'notes') return '标注与笔记'
  if (panelMode.value === 'display') return '显示设置'
  return ''
})

const togglePanel = (mode) => {
  panelMode.value = panelMode.value === mode ? '' : mode
}

const bookmarks = ref([])
const annotations = ref([])

const formatTime = (ms) => {
  if (!ms) return ''
  try {
    return new Date(ms).toLocaleDateString('zh-CN')
  } catch {
    return ''
  }
}

// ───────────────────────── 搜索 ─────────────────────────

const searchOpen = ref(false)
const searchQuery = ref('')
const searchHits = ref([])
const searchCursor = ref(0)
const searchInputRef = ref(null)

/**
 * 正文里的文本节点（按文档顺序），搜索与高亮共用这一份。
 *
 * 为什么不拿原始 HTML 字符串做正则剥标签来算偏移：浏览器解析后的 DOM 会处理实体
 * （`&nbsp;` `&amp;`）、注释、`<br>`、空白折叠，正则剥标签得到的是另一套文本，
 * 两套文本的偏移一旦混用，就会出现「搜的是中文、高亮落在别处」。
 *
 * 只排除 script/style —— 它们不是正文。阅读器自己插入的 <mark> 不用排除：
 * 包标记只是把文本节点切碎，拼接结果不变，偏移因此保持稳定。
 */
const collectTextNodes = (doc) => {
  if (!doc?.body) return []
  const out = []
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      if (!node.nodeValue) return NodeFilter.FILTER_REJECT
      if (node.parentElement?.closest?.('script, style, head, title')) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })
  let node = walker.nextNode()
  while (node) {
    out.push(node)
    node = walker.nextNode()
  }
  return out
}

/** 把一段 HTML 解析成正文文本（与 iframe 渲染走同一条 HTML→DOM 路径）。 */
const sectionText = (html) => {
  try {
    const parsed = new DOMParser().parseFromString(String(html || ''), 'text/html')
    return collectTextNodes(parsed).map((n) => n.nodeValue || '').join('')
  } catch {
    return ''
  }
}

/** 在规范化串里找出全部匹配，返回原文下标区间 [{ start, end }]。 */
const findMatches = (normalized, needle) => {
  const out = []
  if (!needle) return out
  let from = 0
  while (out.length < 2000) {
    const at = normalized.text.indexOf(needle, from)
    if (at < 0) break
    const last = at + needle.length - 1
    const start = normalized.map[at]
    const end = normalized.map[last] + 1
    if (Number.isInteger(start) && Number.isInteger(end) && end > start) out.push({ start, end })
    from = at + Math.max(1, needle.length)
  }
  return out
}

const toggleSearch = () => {
  searchOpen.value = !searchOpen.value
  if (searchOpen.value) {
    nextTick(() => searchInputRef.value?.focus?.())
    return
  }
  searchQuery.value = ''
  searchHits.value = []
  searchCursor.value = 0
  clearSearchHighlights()
}

/**
 * 全文搜索。
 *
 * 每一节都走 sectionText()（DOMParser）→ 同一套规范化，因此「计数」与「能跳过去、
 * 跳过去能高亮」是同一个判断。命中只存 `{ sectionIndex, localIndex }`：
 * 切到那一节之后重新在该节正文里定位第 localIndex 个匹配再包高亮，
 * 不跨文档搬运偏移 —— 这是旧实现错位的另一条来源。
 */
const runSearch = () => {
  const query = String(searchQuery.value || '').trim()
  if (!query) {
    searchHits.value = []
    searchCursor.value = 0
    clearSearchHighlights()
    return
  }
  const needle = normalizeWithMap(query).text
  const hits = []
  sections.value.forEach((section, sectionIndex) => {
    if (hits.length >= 2000) return
    const normalized = normalizeWithMap(sectionText(section.html))
    findMatches(normalized, needle).forEach((range, localIndex) => {
      if (hits.length >= 2000) return
      hits.push({
        sectionIndex,
        localIndex,
        snippet: normalized.text.slice(Math.max(0, range.start - 30), range.end + 30).trim(),
      })
    })
  })
  searchHits.value = hits
  // 当前节里的第一个命中优先，读者不必被拽去第一章。
  const here = hits.findIndex((h) => h.sectionIndex === activeIndex.value)
  searchCursor.value = here >= 0 ? here : 0
  logger.info('[DocEbookReader] search done', { query, hits: hits.length })
  if (hits.length) void goToHit(searchCursor.value)
}

const nextMatch = (delta) => {
  if (!searchHits.value.length) return
  const next = (searchCursor.value + delta + searchHits.value.length) % searchHits.value.length
  void goToHit(next)
}

/** 跳到第 n 个匹配：必要时切章，等正文就绪后在当前文档里重新定位并高亮。 */
const goToHit = async (n) => {
  searchCursor.value = n
  const hit = searchHits.value[n]
  if (!hit) return
  if (hit.sectionIndex !== activeIndex.value) {
    activeIndex.value = hit.sectionIndex
    await nextTick()
    await waitForFrame()
  }
  applySearchHighlight(hit.localIndex)
}

const waitForFrame = async () => {
  for (let i = 0; i < 40; i += 1) {
    await new Promise((resolve) => setTimeout(resolve, 50))
    if (frameRef.value?.contentDocument?.body) return
  }
}

/**
 * 在当前 iframe 文档里高亮第 n 个匹配。
 *
 * 偏移在这里**现算**（同一个 normalizeWithMap + findMatches），因此与页面上的文字
 * 严格对应；命中跨多个文本节点时（一句话被 <em>/<a> 切开）逐节点包，
 * 视觉上仍是连续的一段。
 */
const applySearchHighlight = (localIndex) => {
  const doc = frameRef.value?.contentDocument
  if (!doc?.body) return
  clearSearchHighlights()
  const nodes = collectTextNodes(doc)
  if (!nodes.length) return

  const starts = []
  let raw = ''
  for (const node of nodes) {
    starts.push(raw.length)
    raw += node.nodeValue || ''
  }
  const normalized = normalizeWithMap(raw)
  const needle = normalizeWithMap(searchQuery.value).text
  const match = findMatches(normalized, needle)[localIndex]
  if (!match) {
    nodes[0]?.parentElement?.scrollIntoView?.({ block: 'center', behavior: 'smooth' })
    return
  }

  // 原文区间 → 每个节点的交集 → 逐节点包 <mark>。
  let firstMark = null
  for (let i = 0; i < nodes.length; i += 1) {
    const node = nodes[i]
    const text = node.nodeValue || ''
    const nodeStart = starts[i]
    const from = Math.max(match.start, nodeStart) - nodeStart
    const to = Math.min(match.end, nodeStart + text.length) - nodeStart
    if (to <= from) continue
    try {
      const range = doc.createRange()
      range.setStart(node, from)
      range.setEnd(node, to)
      const mark = doc.createElement('mark')
      mark.className = 'gx-hit is-current'
      range.surroundContents(mark)
      if (!firstMark) firstMark = mark
    } catch (error) {
      logger.debug('[DocEbookReader] search highlight segment failed', {
        error: String(error?.message || error),
      })
    }
  }
  firstMark?.scrollIntoView({ block: 'center', behavior: 'smooth' })
}

const clearSearchHighlights = () => {
  const doc = frameRef.value?.contentDocument
  if (!doc) return
  doc.querySelectorAll('mark.gx-hit').forEach((el) => {
    const parent = el.parentNode
    if (!parent) return
    while (el.firstChild) parent.insertBefore(el.firstChild, el)
    parent.removeChild(el)
  })
}

const tocActiveIdx = computed(() => {
  // 目录项与 section 通过 href 关联；找不到就退回按索引映射。
  const current = sections.value[activeIndex.value]
  if (!current) return -1
  const idx = toc.value.findIndex((item) => item.href && current.href && item.href.endsWith(current.href))
  if (idx >= 0) return idx
  return Math.min(activeIndex.value, Math.max(0, toc.value.length - 1))
})

let loadToken = 0

const zoomBy = (factor) => {
  zoom.value = Math.min(3, Math.max(0.5, +(zoom.value * factor).toFixed(3)))
}

/**
 * 章节文档的 CSP。iframe 不设 sandbox（原因见模板注释），书籍脚本的防线在这里：
 * script/object/frame 全部禁止；样式允许内联与 data:（书籍 CSS 已内联），
 * 图片/媒体允许 data: 与 blob:（资源在本进程内联成 data URI）。
 * 这条 meta 必须放在文档 <head> 的最前面，浏览器只认先到达的那一条。
 * 与 sanitizeSectionHtml 的「剥离 + 自检」是两层独立防线，任何一层都不该被当成可选。
 */
const FRAME_CSP =
  "default-src 'none'; img-src data: blob:; media-src data: blob:; font-src data:; " +
  "style-src 'unsafe-inline' data:; script-src 'none'; object-src 'none'; " +
  "frame-src 'none'; form-action 'none'; base-uri 'none'"
const FRAME_CSP_META = `<meta http-equiv="Content-Security-Policy" content="${FRAME_CSP}">`

/** 净化并自检（剥离逻辑与判据见 ebookSanitize.js）；退化时记一条日志便于排查。 */
const sanitizeSection = (html) => {
  const result = sanitizeSectionHtml(html)
  if (result.degraded) {
    logger.warn('[DocEbookReader] script survived stripping, degraded to plain text', {
      path: props.path,
      section: activeIndex.value,
    })
  }
  return result.html
}

/**
 * 把当前章节 HTML 组装成完整的 iframe 文档串。
 *
 * 负责三件事：
 * 1. 净化（剥离脚本与嵌套容器 + 自检）；
 * 2. 注入阅读器基础样式（字号随缩放联动、正文最大宽度、图片自适应）；
 * 3. 处理「完整文档 / 片段」两种输入形态。
 *
 * 注意：这里只更新 frameDoc 这个 ref，真正写进 iframe 由 writeFrameDocument 负责。
 */
const buildFrameDoc = () => {
  const section = sections.value[activeIndex.value]
  if (!section) {
    frameDoc.value = '<!doctype html><html><body></body></html>'
    return
  }

  const raw = sanitizeSection(section.html)
  const isFullDoc = /<html[\s>]/i.test(raw)
  // 显示设置全部落到这里：字号 / 行距 / 页宽 / 字体 / 主题。
  // 主题用 CSS 变量注入到 iframe 内部，避免每章正文写死颜色。
  const p = prefs.value
  const fontStack = FONT_STACKS[p.fontFamily] || FONT_STACKS.system
  const themeVars = THEME_VARS[p.theme] || THEME_VARS.light
  const baseCss = `
    html, body { margin: 0 !important; padding: 0 !important; background: transparent; }
    :root {
      --gx-bg: ${themeVars.bg};
      --gx-fg: ${themeVars.fg};
      --gx-link: ${themeVars.link};
      --gx-quote: ${themeVars.border};
    }
    /*
     * 正文排版。
     *
     * 只改「读者会用眼睛判断」的那几项：行宽、行距、段距、首行缩进、标题层级。
     * 不覆盖书籍自带的字体与颜色 —— 那本书的排版是它的身份，阅读器只负责让它
     * 在屏幕上站得住。段距用 em 而不是 px：字号一改，段与段的呼吸感按比例跟着变。
     */
    body {
      font-family: ${fontStack} !important;
      line-height: ${p.lineHeight} !important;
      font-size: ${p.fontSize * zoom.value}px !important;
      color: var(--gx-fg);
      background: var(--gx-bg);
      /* 版式三件套必须带 !important：电子书自带的样式表几乎都给 <body> 写了
         margin/padding（Calibre 的 .calibre 规则），类选择器的优先级高于元素
         选择器，不带 !important 时「页宽 + 居中」会被整段覆盖 —— 表现就是正文
         贴左、右侧留一大片空白。 */
      box-sizing: border-box !important;
      padding: 40px 48px 96px !important;
      max-width: ${p.maxWidth}px !important;
      margin: 0 auto !important;
      word-wrap: break-word;
      /* 中英混排：允许在汉字之间断行，但别把西文单词拦腰截断。 */
      overflow-wrap: break-word;
      hyphens: none;
      text-rendering: optimizeLegibility;
    }
    /* 段距按字号缩放；标题与正文拉开层次，让翻页时一眼能看出「这是新的一节」。 */
    p { margin: 0 0 0.9em; }
    p + p { text-indent: 2em; }
    h1, h2, h3, h4, h5, h6 { line-height: 1.35; margin: 1.6em 0 0.7em; font-weight: 600; }
    h1 { font-size: 1.6em; }
    h2 { font-size: 1.3em; }
    h3 { font-size: 1.12em; }
    h1 + p, h2 + p, h3 + p, h4 + p { text-indent: 0; }
    img, svg, video { max-width: 100%; height: auto; }
    table { border-collapse: collapse; max-width: 100%; margin: 1em 0; }
    td, th { border: 1px solid var(--gx-quote); padding: 4px 8px; }
    pre { white-space: pre-wrap; word-break: break-word; }
    blockquote { border-left: 3px solid var(--gx-quote); margin-left: 0; padding-left: 12px; opacity: .85; }
    a { color: var(--gx-link); }
    /* 正文里的选中色跟随主题，而不是浏览器默认的蓝。 */
    ::selection { background: color-mix(in srgb, var(--gx-link) 30%, transparent); }
    /* 搜索命中：整章内所有匹配用浅黄底，当前匹配用橙色描边。 */
    mark.gx-hit { background: rgba(255, 214, 0, .55); color: inherit; padding: 0; }
    mark.gx-hit.is-current { background: rgba(255, 140, 0, .75); outline: 2px solid rgba(255, 110, 0, .9); }
    /* 高亮标注 */
    mark.gx-annot { color: inherit; padding: 0; border-radius: 2px; }
    ${p.layoutMode === 'paginated' ? `
      html, body { height: 100%; overflow: hidden; }
      body { height: 100vh; box-sizing: border-box; overflow-y: auto; column-width: ${p.maxWidth}px; }
    ` : ''}
  `

  if (isFullDoc) {
    // 完整文档：CSP 与本阅读器的基础样式都插进 head。
    // CSP meta 必须尽量靠前，否则浏览器可能已经完成策略选择。
    let next = raw
    if (/<head[^>]*>/i.test(next)) {
      next = next
        .replace(/<head([^>]*)>/i, `<head$1>${FRAME_CSP_META}`)
        .replace(/<\/head>/i, `<style>${baseCss}</style></head>`)
    } else if (/<html[^>]*>/i.test(next)) {
      next = next.replace(
        /<html([^>]*)>/i,
        `<html$1><head>${FRAME_CSP_META}<style>${baseCss}</style></head>`
      )
    } else {
      next = `<!doctype html><html><head><meta charset="utf-8">${FRAME_CSP_META}<style>${baseCss}</style></head><body>${next}</body></html>`
    }
    frameDoc.value = next
    return
  }

  frameDoc.value = `<!doctype html><html><head><meta charset="utf-8">${FRAME_CSP_META}<style>${baseCss}</style></head><body>${raw}</body></html>`
}

/** 切到指定章节（越界自动收敛）。 */
const goSection = (index) => {
  const next = Math.max(0, Math.min(sections.value.length - 1, index))
  if (next === activeIndex.value) return
  activeIndex.value = next
}

// ───────────────────────── 节跳转 ─────────────────────────
// 输入框显示的是 1-based 节号，内部状态一律 0-based，转换只发生在这一层，
// 免得别处（目录、书签、搜索）混用两种编号。

const sectionInput = ref('1')

const syncSectionInput = () => {
  sectionInput.value = String(activeIndex.value + 1)
}

const commitSectionInput = () => {
  const n = Math.round(Number(sectionInput.value))
  if (!Number.isFinite(n) || n < 1) {
    syncSectionInput()
    return
  }
  goSection(n - 1)
  syncSectionInput()
}

/** 去掉 `../`、`./` 与开头的 `/`，让不同来源的路径能直接比较。 */
const normalizeHrefPath = (raw) => {
  const parts = []
  for (const seg of String(raw || '').split('\\').join('/').split('/')) {
    if (!seg || seg === '.') continue
    if (seg === '..') { parts.pop(); continue }
    parts.push(seg)
  }
  return parts.join('/')
}

/**
 * 把章节 href 解析成 sections 下标（-1 表示解析不到）。
 *
 * 目录项与章节内的相对链接都走这里。匹配分三层，从严到松：
 *   1. 归一化后整串相等；
 *   2. 一方以另一方结尾（章节文件可能在不同目录层级，如 `part0006.html` vs `text/part0006.html`）；
 *   3. 只比文件名（`OEBPS/text/part0006.html` 与 `part0006.html`）。
 */
const resolveSectionIndex = (href) => {
  const raw = String(href || '').split('#')[0]
  if (!raw) return -1
  const target = normalizeHrefPath(raw)
  if (!target) return -1
  const list = sections.value
  const base = (h) => h.split('?')[0]
  let idx = list.findIndex((s) => normalizeHrefPath(base(String(s.href))) === target)
  if (idx < 0) {
    idx = list.findIndex((s) => {
      const sh = normalizeHrefPath(base(String(s.href)))
      return sh.endsWith('/' + target) || target.endsWith('/' + sh)
    })
  }
  if (idx < 0) {
    const file = target.split('/').pop()
    idx = list.findIndex((s) => normalizeHrefPath(base(String(s.href))).split('/').pop() === file)
  }
  return idx
}

/** 目录项点击：切到对应章节（目录项若带 `#锚点`，切过去后再滚到锚点）。 */
const jumpToHref = (href) => {
  const fragment = String(href || '').includes('#')
    ? String(href).slice(String(href).indexOf('#') + 1)
    : ''
  const idx = resolveSectionIndex(href)
  if (idx < 0) {
    // 找不到对应章节：可能是书内不存在的资源，或某种内部位置引用。
    logger.debug('[DocEbookReader] unresolvable in-book link', { href })
    return
  }
  followSectionJump(idx, fragment)
}

// ───────────────────────── 书内链接与位置锚点 ─────────────────────────
// 后端在生成章节 HTML 时已经把 MOBI 的 `filepos`、KF8 的 `kindle:pos` 统一改写成
// `#gx-pos-<正文流字节偏移>`，并在目标处插入了空锚点。这里只做三件事：
//   1. `#gx-pos-N` → 用每节的 startOffset/byteLen 找到所属节，必要时先切节；
//   2. `<a href="partXXXX.html#anchor">` → 切节 + 滚动到锚点（EPUB 的脚注/引用）；
//   3. 纯 `#anchor` → 当前节内滚动。

/** 跨节跳转时待消费的锚点 id；iframe 重新加载后执行一次。 */
let pendingAnchor = ''

const frameDocElement = (id) => frameRef.value?.contentDocument?.getElementById?.(id) || null

/** 在当前章节里滚到某个锚点；找不到返回 false。 */
const scrollToAnchor = (id) => {
  const el = frameDocElement(id)
  if (!el) return false
  try {
    el.scrollIntoView({ block: 'start', behavior: 'smooth' })
  } catch {
    el.scrollIntoView()
  }
  return true
}

/** 正文流字节偏移 → 所在节下标（没有偏移信息的章节跳过）。 */
const sectionIndexForPosition = (pos) => {
  const list = sections.value
  for (let i = 0; i < list.length; i += 1) {
    const start = Number(list[i]?.startOffset)
    const len = Number(list[i]?.byteLen)
    if (!Number.isFinite(start) || !Number.isFinite(len) || len <= 0) continue
    if (pos >= start && pos < start + len) return i
  }
  return -1
}

/** 跳到正文流里的某个字节位置。 */
const jumpToPosition = (pos) => {
  const idx = sectionIndexForPosition(pos)
  if (idx < 0) {
    logger.debug('[DocEbookReader] position not mapped to any section', { pos })
    return
  }
  const anchor = `gx-pos-${pos}`
  if (idx === activeIndex.value) {
    scrollToAnchor(anchor)
    return
  }
  pendingAnchor = anchor
  goSection(idx)
}

/** 先切节再滚锚点；同节则直接滚。 */
const followSectionJump = (idx, fragment) => {
  if (idx === activeIndex.value) {
    if (fragment) scrollToAnchor(fragment)
    return
  }
  pendingAnchor = fragment || ''
  goSection(idx)
}

/** 点击章节内链接的统一入口。 */
const followInBookLink = (href) => {
  if (href.startsWith('#gx-pos-')) {
    const pos = Number(href.slice('#gx-pos-'.length))
    if (Number.isFinite(pos)) jumpToPosition(pos)
    return
  }
  if (href.startsWith('#')) {
    scrollToAnchor(href.slice(1))
    return
  }
  const hashAt = href.indexOf('#')
  const path = hashAt >= 0 ? href.slice(0, hashAt) : href
  const fragment = hashAt >= 0 ? href.slice(hashAt + 1) : ''
  const idx = resolveSectionIndex(path)
  if (idx < 0) {
    if (fragment) scrollToAnchor(fragment)
    return
  }
  followSectionJump(idx, fragment)
}

/** 当前 iframe 的 blob URL，切换章节时先回收，避免泄漏。 */
let frameBlobUrl = ''

const revokeFrameBlob = () => {
  if (!frameBlobUrl) return
  try {
    URL.revokeObjectURL(frameBlobUrl)
  } catch {
    /* 忽略 */
  }
  frameBlobUrl = ''
}

/**
 * 生成章节文档的 blob URL，驱动 iframe 挂载。
 *
 * 先把 frameSrc 置空让 v-if 销毁旧 iframe，再挂新的：
 * 直接改 src 会让同一个浏览上下文经历一次导航，旧文档的脚本会被重新解析并再报一次拦截。
 */
const writeFrameDocument = () => {
  revokeFrameBlob()
  frameSrc.value = ''
  const blob = new Blob([frameDoc.value], { type: 'text/html;charset=utf-8' })
  frameBlobUrl = URL.createObjectURL(blob)
  nextTick(() => {
    frameSrc.value = frameBlobUrl
  })
}

const onFrameLoad = () => {
  // 换文档后旧的选区与操作条都不再成立。
  hideSelectionBar()
  // 读一次内部文档，挂上链接拦截：`<a href="ch2.xhtml">` 要在阅读器内跳转，
  // 而不是把 iframe 导航到不存在的地址。（blob 文档与宿主同源，可直接访问。）
  try {
    const doc = frameRef.value?.contentDocument
    if (!doc) return
    // iframe 内的按键不会冒泡到宿主窗口，Esc 收起操作条要在这里单独听一次。
    doc.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && selectionVisible.value) {
        event.preventDefault()
        hideSelectionBar()
      }
    })
    doc.addEventListener('click', (event) => {
      const anchor = event.target?.closest?.('a')
      if (!anchor) return
      const href = String(anchor.getAttribute('href') || '').trim()
      // `javascript:` 一律吞掉：书内不应有，真有也不能在 blob 文档里执行。
      if (!href || /^javascript:/i.test(href)) {
        event.preventDefault()
        return
      }
      // 外链交给系统浏览器。不能让它在这个 blob 文档里自行导航 —— 那会把整本书
      // 的 iframe 变成那个网页，读者除了关掉重开没有别的退路。
      if (/^(https?:|mailto:|tel:)/i.test(href)) {
        event.preventDefault()
        openUrl(href).catch((error) => {
          logger.warn('[DocEbookReader] open external link failed', {
            href,
            error: String(error?.message || error),
          })
        })
        return
      }
      // 书内链接统一由阅读器接管：切节 + 滚到锚点/正文位置。
      // 不能交给浏览器默认导航 —— blob 文档没有对应的基址，点了只会白跑一趟。
      event.preventDefault()
      followInBookLink(href)
    })
    // 滚动进度跟踪：用于保存阅读位置。
    doc.addEventListener('scroll', onFrameScroll, { passive: true })
    // 划词操作条：鼠标选择、Shift+方向键选择（selectionchange）都要跟手；
    // 章节滚动时收起，避免操作条悬在已经离开的段落上。
    doc.addEventListener('mouseup', scheduleSelectionSync)
    doc.addEventListener('keyup', scheduleSelectionSync)
    doc.addEventListener('selectionchange', scheduleSelectionSync)
    doc.addEventListener('scroll', hideSelectionBar, { passive: true })
    // 恢复本节滚动位置（首次进书或跳转书签后由调用方设置）。
    applyPendingRestore()
    // 跨节链接的落点：新文档就绪后滚到锚点（优先于上面恢复的滚动位置）。
    if (pendingAnchor) {
      const anchor = pendingAnchor
      pendingAnchor = ''
      scrollToAnchor(anchor)
    }
    // 本章已有标注画上底色。
    applyAnnotationHighlights()
  } catch (error) {
    logger.debug('[DocEbookReader] frame hook skipped', {
      error: String(error?.message || error),
    })
  }
  // 搜索跳转后若落在本章，需要重新打高亮（新文档里的 mark 已被替换掉）。
  if (searchHits.value.length && searchQuery.value) {
    const hit = searchHits.value[searchCursor.value]
    if (hit && hit.sectionIndex === activeIndex.value) {
      applySearchHighlight(hit.localIndex)
    }
  }
}

// ───────────────────────── 进度持久化 ─────────────────────────

let saveStateTimer = null
/** 切章/跳书签时待恢复的滚动比例；iframe load 后消费一次。 */
let pendingRestoreRatio = null

const onFrameScroll = () => {
  if (saveStateTimer) clearTimeout(saveStateTimer)
  saveStateTimer = setTimeout(() => {
    saveStateTimer = null
    void persistState()
    updateProgressLabel()
  }, 600)
}

const currentScrollRatio = () => {
  const el = frameRef.value?.contentDocument?.documentElement
  if (!el) return 0
  const max = el.scrollHeight - el.clientHeight
  if (max <= 0) return 0
  return Math.min(1, Math.max(0, el.scrollTop / max))
}

const applyPendingRestore = () => {
  if (pendingRestoreRatio == null) return
  const ratio = pendingRestoreRatio
  pendingRestoreRatio = null
  const el = frameRef.value?.contentDocument?.documentElement
  if (!el) return
  const max = el.scrollHeight - el.clientHeight
  if (max > 0) el.scrollTop = ratio * max
  updateProgressLabel()
}

const persistState = async () => {
  if (!bookKey.value || loading.value) return
  try {
    await invoke('reader_save_state', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      scrollRatio: currentScrollRatio(),
    })
  } catch (error) {
    logger.debug('[DocEbookReader] save state failed', {
      error: String(error?.message || error),
    })
  }
}

const progressLabel = ref('')

const updateProgressLabel = () => {
  if (!sections.value.length) {
    progressLabel.value = ''
    return
  }
  const ratio = currentScrollRatio()
  const overall = Math.min(1, Math.max(0, (activeIndex.value + ratio) / sections.value.length))
  progressLabel.value = `${Math.round(overall * 100)}%`
}

// ───────────────────────── 书签 ─────────────────────────

const addBookmarkAtCurrent = async () => {
  if (!bookKey.value) return
  const section = sections.value[activeIndex.value]
  const heading = sectionText(section?.html || '').trim().slice(0, 40) || `第 ${activeIndex.value + 1} 节`
  try {
    const bm = await invoke('reader_add_bookmark', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      scrollRatio: currentScrollRatio(),
      label: heading,
    })
    bookmarks.value = [bm, ...bookmarks.value]
    logger.info('[DocEbookReader] bookmark added', { label: heading })
  } catch (error) {
    logger.warn('[DocEbookReader] add bookmark failed', { error: String(error?.message || error) })
  }
}

const jumpToBookmark = (bm) => {
  if (bm.sectionIndex !== activeIndex.value) {
    pendingRestoreRatio = bm.scrollRatio
    activeIndex.value = bm.sectionIndex
  } else {
    const el = frameRef.value?.contentDocument?.documentElement
    const max = el ? el.scrollHeight - el.clientHeight : 0
    if (el && max > 0) el.scrollTop = bm.scrollRatio * max
  }
}

const removeBookmark = async (bm) => {
  try {
    await invoke('reader_delete_bookmark', { id: bm.id })
    bookmarks.value = bookmarks.value.filter((item) => item.id !== bm.id)
  } catch (error) {
    logger.warn('[DocEbookReader] delete bookmark failed', { error: String(error?.message || error) })
  }
}

// ───────────────────────── 划词操作条 ─────────────────────────

/**
 * 划词后的悬浮操作条（属于**宿主文档**，不是 iframe 里）。
 *
 * 为什么从 iframe 内挪出来：
 *  1. iframe 内的 `position:absolute` 是相对章节文档定位的，滚动后坐标会飘；
 *  2. 在宿主里才能复用 @iconify 图标与 title 提示，不用往书籍文档里塞按钮；
 *  3. 选区变化（`selectionchange`）与鼠标抬起都能驱动，按住 Shift 选词也能即时出现。
 *
 * 位置换算：iframe 内的 range 矩形是 iframe 视口坐标，加上 iframe 在阅读器容器里的
 * 偏移即得宿主坐标；横向以选区中心对齐（CSS translateX(-50%)）。
 */
const DEFAULT_HIGHLIGHT_COLOR = '#ffd600'
const HIGHLIGHT_PRESETS = [
  { color: '#ffd600', label: '黄' },
  { color: '#a5d6a7', label: '绿' },
  { color: '#90caf9', label: '蓝' },
  { color: '#f48fb1', label: '粉' },
  { color: '#ffab91', label: '橙' },
]

const selectionVisible = ref(false)
const selectionPaletteOpen = ref(false)
const selectionText = ref('')
/** 选区矩形（iframe 视口坐标），翻译面板定位仍用它。 */
const selectionRect = ref({ left: 0, top: 0, bottom: 0, width: 0, height: 0 })
/** 操作条中心点（相对阅读器容器）。 */
const selectionPos = ref({ left: 0, top: 0 })

const hideSelectionBar = () => {
  selectionVisible.value = false
  selectionPaletteOpen.value = false
}

/** 点击宿主界面（工具栏、目录、侧栏）时收起操作条；iframe 内的点击由 mouseup 流程负责。 */
const onHostMouseDown = (event) => {
  if (!selectionVisible.value) return
  if (event.target?.closest?.('.gdr-ebook__selbar')) return
  hideSelectionBar()
}

/** 读取 iframe 当前的选区并更新操作条位置；没有有效选区就收起。 */
const syncSelectionBar = () => {
  const frame = frameRef.value
  const doc = frame?.contentDocument
  if (!doc) return
  const selection = doc.getSelection?.()
  const text = String(selection?.toString?.() || '').trim()
  if (!text || text.length < 2) {
    hideSelectionBar()
    selectionText.value = ''
    return
  }
  const range = selection.getRangeAt?.(0)
  if (!range) return
  const rect = range.getBoundingClientRect()
  if (!rect || (!rect.width && !rect.height)) return

  const root = ebookRoot.value
  if (!root) return
  const fr = frame.getBoundingClientRect()
  const cr = root.getBoundingClientRect()
  const BAR_H = 36
  const BAR_W = 320
  let left = fr.left - cr.left + rect.left + rect.width / 2
  left = Math.max(BAR_W / 2 - 40, Math.min(left, cr.width - BAR_W / 2 + 40))
  let top = fr.top - cr.top + rect.top - BAR_H - 8
  if (top < 8) top = fr.top - cr.top + rect.bottom + 8

  selectionText.value = text
  selectionRect.value = {
    left: rect.left,
    top: rect.top,
    bottom: rect.bottom,
    width: rect.width,
    height: rect.height,
  }
  selectionPos.value = { left: Math.round(left), top: Math.round(top) }
  selectionVisible.value = true
}

/** selectionchange / mouseup / keyup 统一走一个短防抖：既跟手又不重复计算。 */
let selectionSyncTimer = null
const scheduleSelectionSync = () => {
  if (selectionSyncTimer) clearTimeout(selectionSyncTimer)
  selectionSyncTimer = setTimeout(() => {
    selectionSyncTimer = null
    syncSelectionBar()
  }, 20)
}

const translateSelectionText = () => {
  const text = selectionText.value
  const rect = selectionRect.value
  hideSelectionBar()
  if (text) openTranslate(text, rect)
}

const speakSelectionText = () => {
  const text = selectionText.value
  hideSelectionBar()
  if (text) speakSelection(text)
}

/** 高亮选中的文字（默认颜色或调色盘里挑的颜色）。 */
const highlightSelection = (color) => {
  const text = selectionText.value
  const chosen = color || DEFAULT_HIGHLIGHT_COLOR
  hideSelectionBar()
  if (!text) return
  void addAnnotationFromSelection(text, chosen)
}

const addAnnotationFromSelection = async (text, color) => {
  if (!bookKey.value) return
  try {
    const an = await invoke('reader_add_annotation', {
      bookKey: bookKey.value,
      sectionIndex: activeIndex.value,
      text,
      note: '',
      color,
    })
    annotations.value = [...annotations.value, an]
    applyAnnotationHighlights()
    // 标注是在正文里完成的，不再顺手把右侧「标注与笔记」面板拉开 ——
    // 高亮只是一个阅读动作，读者的视线不应该被整块侧栏打断。
    logger.info('[DocEbookReader] annotation added', { chars: text.length })
  } catch (error) {
    logger.warn('[DocEbookReader] add annotation failed', { error: String(error?.message || error) })
  }
}

/** 把本章已有标注在正文里画成底色。 */
const applyAnnotationHighlights = () => {
  const doc = frameRef.value?.contentDocument
  if (!doc?.body) return
  doc.querySelectorAll('mark.gx-annot').forEach((el) => {
    const parent = el.parentNode
    if (!parent) return
    while (el.firstChild) parent.insertBefore(el.firstChild, el)
    parent.removeChild(el)
  })
  const mine = annotations.value.filter((an) => an.sectionIndex === activeIndex.value)
  if (!mine.length) return
  try {
    for (const an of mine) {
      const needle = String(an.text || '').trim()
      if (!needle) continue
      const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => (node.nodeValue && node.nodeValue.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
      })
      let node = walker.nextNode()
      while (node) {
        const value = node.nodeValue || ''
        const at = value.indexOf(needle)
        if (at >= 0) {
          const range = doc.createRange()
          range.setStart(node, at)
          range.setEnd(node, at + needle.length)
          const mark = doc.createElement('mark')
          mark.className = 'gx-annot'
          mark.style.background = an.color || '#ffd600'
          mark.title = an.note || ''
          try {
            range.surroundContents(mark)
          } catch {
            /* 跨节点时不强求 */
          }
          break
        }
        node = walker.nextNode()
      }
    }
  } catch (error) {
    logger.debug('[DocEbookReader] annotation highlight failed', {
      error: String(error?.message || error),
    })
  }
}

const jumpToAnnotation = (an) => {
  activeIndex.value = an.sectionIndex
}

const removeAnnotation = async (an) => {
  try {
    await invoke('reader_delete_annotation', { id: an.id })
    annotations.value = annotations.value.filter((item) => item.id !== an.id)
    applyAnnotationHighlights()
  } catch (error) {
    logger.warn('[DocEbookReader] delete annotation failed', { error: String(error?.message || error) })
  }
}

// ───────────────────────── 设置持久化 ─────────────────────────

let persistPrefsTimer = null

const applyPrefs = () => {
  buildFrameDoc()
  if (persistPrefsTimer) clearTimeout(persistPrefsTimer)
  persistPrefsTimer = setTimeout(async () => {
    persistPrefsTimer = null
    if (!bookKey.value) return
    try {
      await invoke('reader_save_prefs', { bookKey: bookKey.value, prefs: prefs.value })
    } catch (error) {
      logger.debug('[DocEbookReader] save prefs failed', {
        error: String(error?.message || error),
      })
    }
  }, 400)
}

const setFontFamily = (value) => {
  prefs.value.fontFamily = value
  applyPrefs()
}

const setTheme = (value) => {
  prefs.value.theme = value
  applyPrefs()
}

const setLayoutMode = (value) => {
  prefs.value.layoutMode = value
  applyPrefs()
}

const loadReaderSettings = async () => {
  if (!bookKey.value) return null
  try {
    const [savedPrefs, savedState, savedBookmarks, savedAnnotations] = await Promise.all([
      invoke('reader_get_prefs', { bookKey: bookKey.value }),
      invoke('reader_get_state', { bookKey: bookKey.value }),
      invoke('reader_list_bookmarks', { bookKey: bookKey.value }),
      invoke('reader_list_annotations', { bookKey: bookKey.value }),
    ])
    if (savedPrefs) {
      const merged = { ...prefs.value, ...savedPrefs }
      // 旧版默认是「16px + 衬线」；升级到 v2 后统一改为「界面默认字体 + 18px」。
      // 只在旧默认上迁移，用户自己调过的字号/字体保持不动。
      const version = Number(savedPrefs.prefsVersion) || 0
      if (version < PREFS_VERSION) {
        if (!merged.fontFamily || merged.fontFamily === 'serif') merged.fontFamily = 'system'
        if (!(Number(merged.fontSize) > 16)) merged.fontSize = 18
        merged.prefsVersion = PREFS_VERSION
        // 把迁移结果写回，避免每次打开都重跑一次迁移。
        invoke('reader_save_prefs', { bookKey: bookKey.value, prefs: merged }).catch((error) => {
          logger.debug('[DocEbookReader] persist migrated prefs failed', {
            error: String(error?.message || error),
          })
        })
      }
      prefs.value = merged
    }
    bookmarks.value = Array.isArray(savedBookmarks) ? savedBookmarks : []
    annotations.value = Array.isArray(savedAnnotations) ? savedAnnotations : []
    logger.info('[DocEbookReader] reader settings loaded', {
      bookmarks: bookmarks.value.length,
      annotations: annotations.value.length,
      hasState: !!savedState,
    })
    return savedState || null
  } catch (error) {
    logger.warn('[DocEbookReader] load reader settings failed', {
      error: String(error?.message || error),
    })
    return null
  }
}

const onKeydown = (event) => {
  if (event.target && /input|textarea/i.test(event.target.tagName || '')) return
  // Ctrl/Cmd+F：全文搜索
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
    event.preventDefault()
    if (!searchOpen.value) toggleSearch()
    else searchInputRef.value?.focus()
    return
  }
  // Ctrl/Cmd+D：加书签
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {
    event.preventDefault()
    void addBookmarkAtCurrent()
    return
  }
  if (event.key === 'Escape') {
    // 操作条开着时，Esc 只负责收起它，不顺手关掉搜索/笔记面板。
    if (selectionVisible.value) {
      hideSelectionBar()
      return
    }
    if (searchOpen.value) toggleSearch()
    else if (panelMode.value) panelMode.value = ''
    return
  }
  // 空格：朗读开关。放在翻节之前 —— 阅读时按空格「开始读」比「翻到下一节」更常用。
  if (event.key === ' ') {
    event.preventDefault()
    toggleReadAloud()
    return
  }
  if (event.key === 'ArrowLeft') {
    event.preventDefault()
    goSection(activeIndex.value - 1)
  } else if (event.key === 'ArrowRight') {
    event.preventDefault()
    goSection(activeIndex.value + 1)
  }
}

let tocResize = null

/**
 * 开始拖拽目录分隔线。
 *
 * 关键在 iframe：右侧正文是 <iframe>，鼠标一旦移进去，mousemove 就由 iframe 自己的
 * 文档接收，父窗口再也收不到事件 —— 表现就是「拖到一半就断、没法改宽度」。
 * 拖拽期间给正文盖一层透明遮罩（.is-resizing），事件始终落在父窗口上。
 */
const startTocResize = (event) => {
  event.preventDefault()
  tocResize = { startX: event.clientX, startWidth: tocWidth.value }
  resizing.value = true
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onTocResize)
  window.addEventListener('mouseup', stopTocResize)
}

const onTocResize = (event) => {
  if (!tocResize) return
  tocWidth.value = Math.max(150, Math.min(520, tocResize.startWidth + event.clientX - tocResize.startX))
}

const stopTocResize = () => {
  tocResize = null
  resizing.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onTocResize)
  window.removeEventListener('mouseup', stopTocResize)
}

const loadBook = async () => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  sections.value = []
  toc.value = []
  activeIndex.value = 0
  const startedAt = performance.now()

  try {
    // 设置/进度与正文解析并行：四次 sqlite 读取没有理由串在整本书的解析后面。
    // 两者都在首帧之前不能省，但并行之后首帧只等较慢的那个。
    const settingsPromise = loadReaderSettings()
    const payload = await invoke('read_document_for_reader', {
      path: props.path,
      kind: props.kind,
    })
    const parsedAt = performance.now()
    if (token !== loadToken) return

    if (!payload || payload.format !== 'ebook') {
      throw new Error('后端未返回电子书结构')
    }
    title.value = payload.title || ''
    sections.value = Array.isArray(payload.sections) ? payload.sections : []
    toc.value = Array.isArray(payload.toc) ? payload.toc : []
    if (!sections.value.length) throw new Error('这本书没有可显示的正文')

    // 先读回设置与进度：prefs 会影响 buildFrameDoc 的排版，必须先于渲染。
    const savedState = await settingsPromise
    const settingsAt = performance.now()
    if (token !== loadToken) return
    if (savedState && Number.isInteger(savedState.sectionIndex)) {
      activeIndex.value = Math.min(
        Math.max(0, savedState.sectionIndex),
        sections.value.length - 1
      )
      pendingRestoreRatio = Number(savedState.scrollRatio) || 0
    }

    buildFrameDoc()
    syncSectionInput()
    loading.value = false
    logger.info('[DocEbookReader] loaded', {
      path: props.path,
      sections: sections.value.length,
      toc: toc.value.length,
      resumedSection: activeIndex.value,
      parseMs: Math.round(parsedAt - startedAt),
      settingsMs: Math.round(settingsAt - parsedAt),
      totalMs: Math.round(performance.now() - startedAt),
    })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocEbookReader] load failed', {
      path: props.path,
      kind: props.kind,
      error: String(error?.message || error),
    })
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('mousedown', onHostMouseDown)
  void loadBook()
})

// ───────────────────────── 朗读 / 翻译 ─────────────────────────
// 电子书的正文在 iframe 里，所以「读什么、选什么」都必须走 iframe 自己的 document/window，
// 主文档的 window.getSelection() 永远是空的 —— 这是最容易踩的一个坑。

/** 当前 iframe 的滚动容器（逐句朗读依赖它把高亮句滚进视口）。 */
const frameScrollRoot = () => frameRef.value?.contentDocument?.scrollingElement || null

const readAloud = useReaderReadAloud({
  getContext: () => {
    const doc = frameRef.value?.contentDocument
    return {
      root: doc?.body || null,
      scrollRoot: frameScrollRoot(),
      win: frameRef.value?.contentWindow || window,
      doc: doc || null,
    }
  },
  getIdentity: () => ({ library: '', fileName: props.path }),
  // 换节后旧句索引失效，必须重建会话，否则会从上一章的进度继续读。
  speechIndex: computed(() => activeIndex.value),
  onIndexChange: () => {},
  ownerLabel: 'DocEbookReader',
})
const readAloudPlaying = readAloud.isPlaying
const toggleReadAloud = () => readAloud.toggle()

/** 读一小段选中文字（一次性合成，不进逐句会话，互不干扰）。 */
const selectionSpeech = useSelectionReadAloud({
  visible: computed(() => !readAloudPlaying.value),
})
const speakSelection = (text) => {
  const t = String(text || '').trim()
  if (!t) return
  void selectionSpeech.toggle(t)
}

// 划词翻译：选区来源必须指向 iframe 的 document。
const ebookRoot = ref(null)
const selectionTranslate = useSelectionTranslate({
  getContainer: () => ebookRoot.value || null,
  getSelection: () => frameRef.value?.contentDocument?.getSelection?.() || null,
})

/**
 * 打开译文面板。
 *
 * 位置用「选区矩形相对阅读器容器」的坐标：iframe 里的 rect 是 iframe 视口坐标，
 * 要再加上 iframe 元素自身在容器里的偏移，否则面板会飘到左上角。
 */
const openTranslate = (text, rect) => {
  const t = String(text || '').trim()
  if (!t) return
  const frame = frameRef.value
  const container = ebookRoot.value
  if (!frame || !container) return
  const fr = frame.getBoundingClientRect()
  const cr = container.getBoundingClientRect()
  const PW = 460
  let left = fr.left - cr.left + (rect?.left ?? 0)
  left = Math.max(8, Math.min(left, Math.max(8, cr.width - PW - 8)))
  let top = fr.top - cr.top + (rect?.bottom ?? 0) + 8
  if (top + 300 > cr.height) {
    top = Math.max(8, fr.top - cr.top + (rect?.top ?? 0) - 308)
  }
  selectionTranslate.pos.value = { left: Math.round(left), top: Math.round(top) }
  selectionTranslate.visible.value = true
  selectionTranslate.translate(t)
}

onBeforeUnmount(() => {
  loadToken += 1
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('mousedown', onHostMouseDown)
  if (selectionSyncTimer) {
    clearTimeout(selectionSyncTimer)
    selectionSyncTimer = null
  }
  stopTocResize()
  revokeFrameBlob()
  // 卸载前把最后位置写回，否则「读完关窗」会丢掉这段进度。
  if (saveStateTimer) {
    clearTimeout(saveStateTimer)
    saveStateTimer = null
  }
  void persistState()
})

watch(frameDoc, () => {
  nextTick(() => writeFrameDocument())
})

watch(activeIndex, () => {
  syncSectionInput()
  buildFrameDoc()
  nextTick(() => {
    // 切节后回到顶部，否则会停在上一节的滚动位置。
    const body = frameRef.value?.contentDocument?.documentElement
    if (body && pendingRestoreRatio == null) body.scrollTop = 0
  })
})

watch(zoom, () => {
  buildFrameDoc()
})

watch(
  () => props.path,
  () => {
    void loadBook()
  }
)
</script>
