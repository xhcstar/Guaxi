<template>
  <section class="mail-pane mail-reader">
    <div v-if="loading" class="mail-reader-placeholder">
      <span class="mail-spin"></span>
    </div>

    <template v-else-if="detail">
      <header class="mail-reader-head">
        <button class="mail-btn" @click="$emit('close')" :title="closeLabel">
          ‹ {{ closeLabel }}
        </button>
        <div class="mail-reader-toolbar">
          <button class="mail-btn is-icon" title="回复" aria-label="回复" @click="$emit('reply', detail)">
            <MailIcon name="reply" />
          </button>
          <button
            class="mail-btn is-icon"
            title="全部回复"
            aria-label="全部回复"
            @click="$emit('reply-all', detail)"
          >
            <MailIcon name="replyAll" />
          </button>
          <button class="mail-btn is-icon" title="转发" aria-label="转发" @click="$emit('forward', detail)">
            <MailIcon name="forward" />
          </button>
          <!-- 默认在下方就地回复；这个按钮把同一封回复搬进独立窗口，
               方便和主窗口并排对照原文。 -->
          <button
            class="mail-btn is-icon"
            title="在新窗口里回复"
            aria-label="在新窗口里回复"
            @click="$emit('reply', detail, { newWindow: true })"
          >
            <MailIcon name="replyWindow" />
          </button>
          <span class="mail-toolbar-sep"></span>
          <button
            class="mail-btn is-icon"
            :class="{ 'is-on': detail.flagged }"
            :title="detail.flagged ? '取消星标' : '加星标'"
            :aria-label="detail.flagged ? '取消星标' : '加星标'"
            @click="$emit('toggle-flag', detail)"
          >
            <MailIcon name="star" :filled="detail.flagged" />
          </button>
          <button
            class="mail-btn is-icon"
            :title="detail.seen ? '标为未读' : '标为已读'"
            :aria-label="detail.seen ? '标为未读' : '标为已读'"
            @click="$emit('toggle-unread', detail)"
          >
            <MailIcon name="unread" :filled="!detail.seen" />
          </button>
          <button class="mail-btn is-icon" title="归档" aria-label="归档" @click="$emit('archive', detail)">
            <MailIcon name="archive" />
          </button>
          <!-- 只有「已发送」里的邮件才需要撤回入口；已投递的邮件在服务端
               无法真正收回，点了会明确说明边界，不做假撤回。 -->
          <button
            v-if="canRecall"
            class="mail-btn is-icon"
            title="撤回这封邮件（仅延迟发送的邮件可真正撤回）"
            aria-label="撤回这封邮件"
            @click="$emit('recall', detail)"
          >
            <MailIcon name="undo" />
          </button>
          <button
            class="mail-btn is-icon is-danger"
            title="删除"
            aria-label="删除"
            @click="$emit('delete', detail)"
          >
            <MailIcon name="trash" />
          </button>
          <span class="mail-toolbar-sep"></span>
          <button
            class="mail-btn is-icon"
            :disabled="tools.translating.value"
            :title="'把正文整封翻译成 ' + tools.targetLang.value"
            aria-label="翻译全文"
            @click="tools.translateAll()"
          >
            <MailIcon name="translate" />
          </button>
          <button
            class="mail-btn is-icon"
            :class="{ 'is-on': tools.readAloud.isPlaying.value }"
            :title="tools.readAloud.isPlaying.value ? '暂停朗读' : '朗读正文'"
            :aria-label="tools.readAloud.isPlaying.value ? '暂停朗读' : '朗读正文'"
            @click="tools.toggleReadAloud()"
          >
            <MailIcon name="speak" />
          </button>
          <select class="mail-select" title="导出为…" aria-label="导出为" @change="onExportChange($event)">
            <option value="">导出…</option>
            <option value="eml">原始邮件 (.eml)</option>
            <option value="md">Markdown</option>
            <option value="html">HTML</option>
            <option value="pdf">PDF</option>
            <option value="docx">Word</option>
            <option value="txt">纯文本</option>
          </select>
          <button class="mail-btn is-icon" title="保存到笔记库" aria-label="保存到笔记库" @click="tools.saveToNotes()">
            <MailIcon name="note" />
          </button>
          <button class="mail-btn is-icon" title="转成一条任务" aria-label="转成一条任务" @click="tools.toTask()">
            <MailIcon name="task" />
          </button>
        </div>

        <h1 class="mail-reader-subject">
          {{ detail.subject || '（无主题）' }}
          <!-- 会话封数：一眼能看出「这条线一共几封往来」，
               不用靠往下滚去数卡片，也不会误以为回复丢了。 -->
          <span v-if="isThread" class="mail-reader-thread-count">
            {{ threadTotal || threadMessages.length }} 封往来
          </span>
        </h1>
      </header>

      <!-- 邮件头信息：与 Outlook 一样把「谁发给谁、什么时候」摊开写清楚。
           每个地址都能悬停出通讯录卡片（复制地址 / 加进通讯录），
           收件人多的时候可以点「展开」。 -->
      <div v-if="!isThread" class="mail-reader-meta">
        <span
          class="mail-avatar mail-reader-avatar"
          :style="{ background: `hsl(${avatarHue} 42% 48%)` }"
        >
          {{ avatarLetter }}
        </span>
        <div class="mail-reader-from">
          <div class="mail-reader-line">
            <span class="mail-reader-key">发件人</span>
            <button
              class="mail-addr"
              @mouseenter="showAddressCard(detail.fromAddr, $event)"
              @mouseleave="addressCard?.scheduleHide()"
              @click="showAddressCard(detail.fromAddr, $event)"
            >
              {{ detail.fromName || detail.fromAddr }}
              <span v-if="detail.fromName" class="mail-addr-raw">&lt;{{ detail.fromAddr }}&gt;</span>
            </button>
          </div>
          <div class="mail-reader-line">
            <span class="mail-reader-key">收件人</span>
            <span class="mail-reader-addrs">
              <button
                v-for="addr in visibleTo"
                :key="'to-' + addr.address"
                class="mail-addr"
                @mouseenter="showAddressCard(addr.address, $event)"
                @mouseleave="addressCard?.scheduleHide()"
                @click="showAddressCard(addr.address, $event)"
              >
                {{ addr.name || addr.address }}
              </button>
              <button
                v-if="hiddenToCount > 0"
                class="mail-addr-more"
                :title="hiddenToNames"
                @click="showAllTo = !showAllTo"
              >
                {{ showAllTo ? '收起' : `还有 ${hiddenToCount} 人` }}
              </button>
            </span>
          </div>
          <div v-if="visibleCc.length || hiddenCcCount > 0" class="mail-reader-line">
            <span class="mail-reader-key">抄送</span>
            <span class="mail-reader-addrs">
              <button
                v-for="addr in visibleCc"
                :key="'cc-' + addr.address"
                class="mail-addr"
                @mouseenter="showAddressCard(addr.address, $event)"
                @mouseleave="addressCard?.scheduleHide()"
                @click="showAddressCard(addr.address, $event)"
              >
                {{ addr.name || addr.address }}
              </button>
              <button
                v-if="hiddenCcCount > 0"
                class="mail-addr-more"
                :title="hiddenCcNames"
                @click="showAllCc = !showAllCc"
              >
                {{ showAllCc ? '收起' : `还有 ${hiddenCcCount} 人` }}
              </button>
            </span>
          </div>
        </div>
        <span class="mail-reader-when" :title="fullDateLabel">{{ dateLabel }}</span>
      </div>

      <MailAddressCard ref="addressCard" :theme-style="themeStyle" />

      <!-- 划词翻译：与 PDF / 电子书 / 认知收件箱同一个面板组件，交互一致。 -->
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
        @translate="selectionTranslate.onTranslate()"
        @copy="selectionTranslate.onCopy()"
        @swap-langs="selectionTranslate.onSwapLangs()"
        @toggle-pin="selectionTranslate.onTogglePin()"
        @update:pos="(v) => { selectionTranslate.pos.value = v }"
        @update:from-lang="(v) => { selectionTranslate.fromLang.value = v }"
        @update:to-lang="(v) => { selectionTranslate.toLang.value = v }"
        @close="selectionTranslate.close()"
      />

      <div
        ref="scrollRef"
        class="mail-reader-scroll"
        @mouseup="onReaderMouseUp"
      >
        <!-- 内联回复卡片：占满一屏正文高度，原文/历史卡片在它下面，滚动查看。 -->
        <slot name="inline-composer" />

        <div class="mail-reader-content" @mouseover="onBodyAddressOver" @mouseout="onBodyAddressOut">
          <!-- 远端图片默认不加载：先问一句，避免打开即给发件人回一个「已读+IP」的信标 -->
          <div v-if="hasRemoteImages && imagesBlocked" class="mail-remote-banner">
            <span>这封邮件包含外部图片，默认没有加载。</span>
            <button class="mail-btn" @click="showImages">显示图片</button>
          </div>

          <div v-if="tools.translateError.value" class="mail-search-error">
            {{ tools.translateError.value }}
          </div>

          <div v-if="tools.showTranslation.value" class="mail-translation">
            <div class="mail-translation-head">
              <span>全文翻译（{{ tools.targetLang.value }}）</span>
              <button class="mail-btn" @click="tools.showTranslation.value = false">收起</button>
            </div>
            <div class="mail-body" v-html="tools.translatedHtml.value"></div>
          </div>

          <div v-if="!isThread && showHeaders" class="mail-headers">
            <pre>{{ headerText }}</pre>
          </div>

          <!-- 会话视图：同一 thread 的每封邮件一张独立卡片，不再把历史塞进当前正文。 -->
          <div v-if="isThread" class="mail-thread">
            <!-- 超出渲染上限时把话说清楚：宁可多一行提示，也不能让用户以为邮件丢了。 -->
            <div v-if="threadTruncated" class="mail-thread-notice">
              这条会话共有 {{ threadTotal }} 封，当前显示最近 {{ threadMessages.length }} 封。
            </div>
            <article
              v-for="msg in threadMessages"
              :key="threadKeyOf(msg)"
              class="mail-thread-card"
            >
              <header class="mail-thread-head">
                <span
                  class="mail-thread-avatar"
                  :style="{ background: `hsl(${threadSeed(msg).hue} 42% 48%)` }"
                >
                  {{ threadSeed(msg).letter }}
                </span>
                <div class="mail-thread-who">
                  <button
                    class="mail-thread-name mail-thread-addr"
                    @mouseenter="showAddressCard(msg.fromAddr, $event)"
                    @mouseleave="addressCard?.scheduleHide()"
                    @click="showAddressCard(msg.fromAddr, $event)"
                  >
                    {{ threadName(msg) }}
                  </button>
                  <span class="mail-thread-subject">{{ msg.subject || '（无主题）' }}</span>
                  <span
                    v-if="threadToList(msg).length || threadCcList(msg).length"
                    class="mail-thread-to"
                  >
                    <template v-if="threadToList(msg).length">
                      发给
                      <button
                        v-for="addr in threadToList(msg)"
                        :key="'to-' + addr.address"
                        class="mail-thread-addr"
                        @mouseenter="showAddressCard(addr.address, $event)"
                        @mouseleave="addressCard?.scheduleHide()"
                        @click="showAddressCard(addr.address, $event)"
                      >
                        {{ addr.name || addr.address }}
                      </button>
                    </template>
                    <template v-if="threadToList(msg).length && threadCcList(msg).length"> · </template>
                    <template v-if="threadCcList(msg).length">
                      抄送
                      <button
                        v-for="addr in threadCcList(msg)"
                        :key="'cc-' + addr.address"
                        class="mail-thread-addr"
                        @mouseenter="showAddressCard(addr.address, $event)"
                        @mouseleave="addressCard?.scheduleHide()"
                        @click="showAddressCard(addr.address, $event)"
                      >
                        {{ addr.name || addr.address }}
                      </button>
                    </template>
                  </span>
                </div>
                <span class="mail-thread-date">{{ threadDate(msg) }}</span>
              </header>
              <div
                class="mail-body mail-thread-body"
                :style="readingStyle"
                v-html="threadBody(msg)"
                @click="onBodyClick"
              ></div>
              <div v-if="msg.attachments?.length" class="mail-attachments">
                <div
                  v-for="att in msg.attachments"
                  :key="att.part + att.filename"
                  class="mail-attachment-item"
                >
                  <button
                    class="mail-attachment"
                    :title="`用系统默认程序打开 ${att.filename}`"
                    @click="$emit('open-attachment', { attachment: att, detail: msg })"
                  >
                    <span aria-hidden="true">{{ iconFor(att.contentType) }}</span>
                    <span class="mail-attachment-name">{{ att.filename }}</span>
                    <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
                  </button>
                  <button
                    class="mail-attachment-save"
                    title="保存到本地"
                    aria-label="保存到本地"
                    @click="$emit('save-attachment', { attachment: att, detail: msg })"
                  >
                    <MailIcon name="download" />
                  </button>
                </div>
              </div>
            </article>
          </div>

          <template v-else>
            <div
              v-if="renderedHtml"
              ref="bodyRef"
              class="mail-body"
              :style="readingStyle"
              v-html="renderedHtml"
              @click="onBodyClick"
            ></div>
            <pre v-else class="mail-body mail-body--plain">{{ detail.text }}</pre>

            <div v-if="detail.attachments?.length" class="mail-attachments">
              <div
                v-for="att in detail.attachments"
                :key="att.part + att.filename"
                class="mail-attachment-item"
              >
                <button
                  class="mail-attachment"
                  :title="`用系统默认程序打开 ${att.filename}`"
                  @click="$emit('open-attachment', { attachment: att, detail })"
                >
                  <span aria-hidden="true">{{ iconFor(att.contentType) }}</span>
                  <span class="mail-attachment-name">{{ att.filename }}</span>
                  <span class="mail-attachment-size">{{ sizeLabel(att.size) }}</span>
                </button>
                <button
                  class="mail-attachment-save"
                  title="保存到本地"
                  aria-label="保存到本地"
                  @click="$emit('save-attachment', { attachment: att, detail })"
                >
                  <MailIcon name="download" />
                </button>
              </div>
            </div>
          </template>
        </div>
      </div>
    </template>
  </section>
</template>

<script setup>
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, onUpdated, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { avatarSeed } from './theme'
import { applyBodyTheme } from './colors'
import { useMailReaderTools } from './useMailReaderTools'
import MailAddressCard from './MailAddressCard.vue'
import { useMailSelectionTranslate } from './useMailSelectionTranslate'
import { clearHighlight, highlightHtml, searchTokens } from './highlight'
import { collectQuoteCards, flattenQuoteCards, pruneEmptyQuotes } from './quoteCards'

// 翻译面板是邮件页里唯一依赖 naive-ui 的组件：只在真正划词时加载。
const GuaxiSelectionTranslatePanel = defineAsyncComponent(
  // 本文件在 apps/mail/ 下，到 HomeViewComponent 要上三级（apps → ApplicationViewComponent → components）。
  () => import('../../../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'),
)
import MailIcon from './MailIcon.vue'

const props = defineProps({
  detail: { type: Object, default: null },
  /** 会话内每封邮件的详情（按时间升序）；为空表示普通单封阅读。 */
  thread: { type: Array, default: () => [] },
  /** 这条会话在本地库里的总封数；大于 `thread.length` 说明被渲染上限截断了。 */
  threadTotal: { type: Number, default: 0 },
  loading: { type: Boolean, default: false },
  settings: { type: Object, default: () => ({}) },
  themeMode: { type: String, default: 'dark' },
  /** 邮件主题变量（与 MailApp 根节点同一份）：地址卡片 Teleport 到 body 后要靠它取色。 */
  themeStyle: { type: Object, default: () => ({}) },
  monoFont: { type: String, default: '' },
  /** 列表收起时「返回列表」没有目标，换成「收起邮件」。 */
  closeLabel: { type: String, default: '返回列表' },
  /** 列表里的搜索词：正文命中的文字就地高亮（Outlook 同一套做法）。 */
  searchQuery: { type: String, default: '' },
  /** 当前邮件在「已发送」里：显示撤回入口（已投递的邮件只做说明，不假撤回）。 */
  canRecall: { type: Boolean, default: false },
})
defineEmits([
  'reply',
  'reply-all',
  'forward',
  'delete',
  'archive',
  'recall',
  'toggle-flag',
  'toggle-unread',
  'save-attachment',
  'open-attachment',
  'close',
])

const showHeaders = computed(() => Boolean(props.settings?.showHeaders))

// 增强阅读：翻译 / 朗读 / 导出 / 笔记 / 任务
const tools = useMailReaderTools({
  getDetail: () => props.detail,
  notify: (text, isError) => console.warn('[mail-reader]', isError ? 'error:' : '', text),
  settings: props.settings,
})

/** 阅读区字号 / 字体由「外观」控制；正文根节点单独再落一次，避免邮件自带样式覆盖。 */
const readingStyle = computed(() => ({
  fontSize: `${props.settings?.readingSize || 15}px`,
}))
const imagesAllowed = ref(false)
const bodyRef = ref(null)

// 换邮件时重置「显示图片」的选择：一封邮件一次授权，不要跨邮件延续。
watch(
  () => `${props.detail?.accountId}:${props.detail?.folder}:${props.detail?.uid}`,
  () => {
    imagesAllowed.value = false
  },
)

const isThread = computed(() => (props.thread?.length || 0) > 1)
/**
 * 会话卡片按「最新在上」排列（与 Outlook / Gmail 一致）。
 *
 * store 是按时间升序存的（回复时要拼引用、取最新一封都依赖这个顺序），
 * 但阅读时用户第一眼要的是最新那条回复；升序渲染会把最早的一封顶在最上面，
 * 看起来就像「邮件没同步到最新」（真实反馈）。这里只反转展示顺序，不动数据。
 */
const threadMessages = computed(() => (isThread.value ? [...props.thread].reverse() : []))
const threadTruncated = computed(
  () => isThread.value && props.threadTotal > threadMessages.value.length,
)

const hasRemoteImages = computed(() => {
  const html = isThread.value
    ? threadMessages.value.map((message) => message.html || '').join('')
    : props.detail?.html || ''
  return /data-guaxi-remote-src=/.test(html)
})

const imagesBlocked = computed(() => hasRemoteImages.value && !imagesAllowed.value)

/**
 * 把暂存的远端图片地址放回 `src`。
 *
 * 不能假设 `src=""` 一定还在：二次清洗（老缓存带 `<style>` 时会重新过一遍
 * sanitize）会把空的 src 当成非法图片地址丢掉，只剩 `data-guaxi-remote-src`。
 * 两种形态都要能恢复，否则点了「显示图片」只会留下碎图图标。
 */
function restoreRemoteImages(html) {
  return String(html || '')
    .replace(/data-guaxi-remote-src="([^"]*)"\s+src=""/g, 'src="$1"')
    .replace(/data-guaxi-remote-src="([^"]*)"\s+src="data:image\/gif;base64,[^"]*"/g, 'src="$1"')
    .replace(/data-guaxi-remote-src="([^"]*)"/g, 'src="$1"')
    .replace(/\sclass="guaxi-noimg"/g, '')
}

/** 1×1 透明 GIF：给被封锁的远端图当占位 src，浏览器就不会画「破图」图标。 */
const BLANK_PIXEL = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'

/**
 * 未加载（被封锁）的远端图片：`src=""` 会让浏览器画出破图图标，
 * 深色纸面上那就是一块深灰/纯黑。换成透明像素 + 占位类，尺寸仍由 width/height 撑住。
 *
 * 高度必须写成行内样式：`.mail-body img { height: auto }`（防止大图撑破版心）
 * 会让占位图按 1×1 透明像素的原始比例变成正方形，把邮件版式挤坏。
 */
function maskBlockedImages(html) {
  return String(html || '').replace(/<img\b[^>]*>/gi, (tag) => {
    const remote = tag.match(/data-guaxi-remote-src="([^"]*)"/i)
    if (!remote) return tag
    const src = tag.match(/\ssrc="([^"]*)"/i)
    // 已经有真实地址（用户点过「显示图片」）：原样返回。
    if (src && src[1].trim() && !src[1].startsWith(BLANK_PIXEL)) return tag
    const height = tag.match(/\sheight="(\d+)"/i)
    const style = tag.match(/\sstyle="([^"]*)"/i)
    const extra = height ? `height:${height[1]}px;` : ''
    const mergedStyle = extra
      ? ` style="${extra}${style ? style[1] : ''}"`
      : style
        ? ` style="${style[1]}"`
        : ''
    const cleaned = tag
      .replace(/\ssrc="[^"]*"/gi, '')
      .replace(/\sclass="[^"]*"/gi, '')
      .replace(/\sstyle="[^"]*"/gi, '')
      .replace(/^<img/i, '<img class="guaxi-noimg"')
    return cleaned.replace(/>$/, `${mergedStyle} src="${BLANK_PIXEL}">`)
  })
}

const scrollRef = ref(null)

/**
 * 换邮件 / 换会话时回到顶部。
 *
 * 滚动容器是复用的，上一封读到一半的位置会留下来——打开新邮件先看到中间一段，
 * 用户会以为「信的开头丢了」。会话视图里最新一封排在最上面，回顶就是最新回复。
 */
watch(
  () =>
    `${props.detail?.accountId}:${props.detail?.folder}:${props.detail?.uid}:${
      props.thread?.length || 0
    }`,
  () => {
    const el = scrollRef.value
    if (el) el.scrollTop = 0
  },
  { flush: 'post' },
)

/**
 * 划词翻译：拖选正文里的文字即出译文面板。
 *
 * 用阅读区当容器（而不是整个窗口）：邮件正文是「读」的对象，
 * 列表 / 工具栏上的划选不该触发翻译。
 */
const selectionTranslate = useMailSelectionTranslate(() => scrollRef.value)

/**
 * 阅读区的 Alt+划词翻译。
 *
 * 内联回复卡片就在这个滚动容器里，而它有自己的取词范围：这里放行，
 * 否则同一次划词会同时弹出两个翻译面板。
 */
function onReaderMouseUp(event) {
  if (event.target?.closest?.('.mail-inline-composer')) return
  selectionTranslate.handleMouseUp(event)
}

/**
 * 暗色纸面上，邮件里写死的颜色要逐元素映射（浅底→深表面、深字→浅字）。
 * 映射逻辑与判定规则都在 colors.js：这里是唯一调用点。
 *
 * 亮色也要调一次：映射会先把元素还原成邮件原值，切回亮色时正文才能回到原样，
 * 不留下暗色模式的痕迹。
 */
function applyThemeColors() {
  applyBodyTheme(scrollRef.value, props.themeMode === 'light' ? 'light' : 'dark')
}

const renderedHtml = computed(() => {
  const html = props.detail?.html
  if (!html) return ''
  // 用户点了「显示图片」：把暂存地址放回 src；否则把空 src 换成透明占位，
  // 免得浏览器在深色纸面上画出一块「破图」。
  const prepared = imagesBlocked.value ? maskBlockedImages(html) : restoreRemoteImages(html)
  return upgradeQuoteCards(prepared)
})

/**
 * 正文里命中的搜索词就地高亮。
 *
 * 只在有搜索词时动手；每次重跑先摘掉上一次的 <mark>，所以换信、改关键词、
 * 切换主题都不会留残影。与颜色映射一样放在每次 patch 之后补一遍——
 * v-html 会重置 innerHTML，watch 里做的那次会被冲掉。
 */
const highlightTokens = computed(() => searchTokens(props.searchQuery))

function applySearchHighlight() {
  const root = scrollRef.value
  if (!root) return
  if (!highlightTokens.value.length) {
    clearHighlight(root)
    return
  }
  for (const body of root.querySelectorAll('.mail-body')) {
    highlightHtml(body, highlightTokens.value)
  }
}

// 正文 DOM 更新后（换信、显示图片、切主题、换搜索词）重新映射一次颜色并高亮。
watch(
  [renderedHtml, () => props.themeMode, () => props.thread, imagesAllowed, highlightTokens],
  () => {
    applyThemeColors()
    applySearchHighlight()
  },
  { flush: 'post' },
)
onMounted(() => {
  applyThemeColors()
  applySearchHighlight()
})
// Vue 在每次组件更新时都会无条件重设 v-html 的 innerHTML；颜色映射与高亮写在 DOM 上，
// 会被下一次更新（标记已读、工具栏状态变化等）整个冲掉。只在 watch 里跑不够，
// 这里在每次 patch 之后补一遍。两者都是幂等的，重复执行不会越改越暗 / 越包越深。
onUpdated(() => {
  applyThemeColors()
  applySearchHighlight()
})

/** 点面板以外的地方收起译文（「固定」时不收，用户要它留着对照看）。 */
function onTranslateOutside(event) {
  selectionTranslate.handleDocumentMouseDown(event)
}

onMounted(() => document.addEventListener('mousedown', onTranslateOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', onTranslateOutside))

// ---------------- 会话卡片 ----------------

function threadKeyOf(message) {
  return `${message.accountId}:${message.folder}:${message.uid}`
}

function threadSeed(message) {
  return avatarSeed(message.fromName, message.fromAddr)
}

function threadName(message) {
  return (message.fromName || '').trim() || message.fromAddr || '（未知发件人）'
}

function threadDate(message) {
  if (!message.date) return ''
  return new Date(message.date * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

/** 会话卡片里的收件人 / 抄送：拆成一个个可悬停的地址，而不是一整串文字。 */
function threadToList(message) {
  return (message.to || []).filter((item) => item?.address).slice(0, 4)
}

function threadCcList(message) {
  return (message.cc || []).filter((item) => item?.address).slice(0, 4)
}

/** 会话里每封邮件的正文：历史和嵌套引用已经由上方卡片呈现，这里直接去掉。 */
function threadBody(message) {
  if (message.html) {
    const prepared = imagesBlocked.value
      ? maskBlockedImages(message.html)
      : restoreRemoteImages(message.html)
    return upgradeQuoteCards(prepared, { dropQuotes: true })
  }
  return `<pre class="mail-body--plain" style="white-space:pre-wrap;margin:0">${escapeHtml(
    message.text || '',
  )}</pre>`
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

/**
 * 把引用历史重排成 Apple Mail 式的消息卡片。
 *
 * 收件时的清洗层会剥掉 class，但保留 `data-*` 与内联样式：
 * - 新邮件靠 `data-guaxi-quote-*` 找结构；
 * - 旧邮件没有标记，按「10/14px 圆角外框 + 内含 blockquote」识别。
 * 处理方式：去掉旧的内联样式、补头像与主题占位，再交给 mail.css 的卡片样式。
 * DOMParser 解析出来的是惰性文档，不会执行脚本；正文本身也已在清洗层过滤过。
 */
function upgradeQuoteCards(html, { dropQuotes = false } = {}) {
  if (!html || !html.includes('blockquote')) return html
  try {
    const doc = new DOMParser().parseFromString(html, 'text/html')
    // 先把嵌套的卡片提到同一层：回信套回信时每一层都把上一层的历史整块包进自己里，
    // 卡片就会一层叠一层、越往里缩得越深（用户反馈「按时间从上到下列出即可」）。
    flattenQuoteCards(doc.body)
    const cards = collectQuoteCards(doc.body)
    // 会话视图：历史邮件已经是上方的独立卡片，正文里的引用块直接去掉，避免重复。
    if (dropQuotes) for (const card of cards) card.remove()
    // 卡片摘走之后，外面那几层 `<blockquote>` 壳会留在原地，每层一条引用竖线；
    // 回信套回信攒出的几十层嵌套，就是正文底部那一片竖线。空壳收掉。
    pruneEmptyQuotes(doc.body)
    if (dropQuotes) return doc.body.innerHTML
    for (const card of cards) {
      card.classList.add('mail-quote--card')
      delete card.dataset.guaxiQuoteCard
      card.removeAttribute('style')
      const head = card.querySelector('[data-guaxi-quote-head]') || card.firstElementChild
      const body =
        card.querySelector('[data-guaxi-quote-body]') || card.querySelector('blockquote')
      if (head instanceof HTMLElement && head.tagName === 'DIV') {
        head.classList.add('mail-quote-head--card')
        head.removeAttribute('style')
      }
      if (body instanceof HTMLElement) {
        body.classList.add('mail-quote-body--card')
        body.removeAttribute('style')
      }
      if (head instanceof HTMLElement) {
        for (const [selector, className] of QUOTE_PARTS) {
          const part = card.querySelector(selector)
          if (part) part.classList.add(className)
        }
        ensureQuoteHeader(doc, head)
      }
    }
    return doc.body.innerHTML
  } catch {
    return html
  }
}

const QUOTE_PARTS = [
  ['[data-guaxi-quote-avatar]', 'mail-quote-avatar'],
  ['[data-guaxi-quote-meta]', 'mail-quote-meta'],
  ['[data-guaxi-quote-from]', 'mail-quote-from'],
  ['[data-guaxi-quote-subject]', 'mail-quote-subject'],
  ['[data-guaxi-quote-date]', 'mail-quote-date'],
]

/** 旧卡片没有头像/主题行：用发件人名字首字补一个头像，主题给个占位。 */
function ensureQuoteHeader(doc, head) {
  if (head.querySelector('.mail-quote-avatar')) return
  const spans = [...head.children].filter((el) => el.tagName === 'SPAN')
  if (!spans.length) return
  const fromSpan = spans[0]
  const dateSpan = spans.length > 1 ? spans[spans.length - 1] : null
  const text = (fromSpan.textContent || '').trim()
  const avatar = doc.createElement('span')
  avatar.className = 'mail-quote-avatar'
  avatar.textContent = text ? [...text][0].toUpperCase() : '✉'
  head.insertBefore(avatar, head.firstChild)
  if (dateSpan && dateSpan !== fromSpan) {
    fromSpan.classList.add('mail-quote-from')
    dateSpan.classList.add('mail-quote-date')
    const meta = doc.createElement('span')
    meta.className = 'mail-quote-meta'
    head.insertBefore(meta, fromSpan)
    meta.appendChild(fromSpan)
    const subject = doc.createElement('span')
    subject.className = 'mail-quote-subject'
    subject.textContent = '历史消息'
    meta.appendChild(subject)
  }
}

const avatar = computed(() => avatarSeed(props.detail?.fromName, props.detail?.fromAddr))
const avatarLetter = computed(() => avatar.value.letter)
const avatarHue = computed(() => avatar.value.hue)

// ---------------- 邮件头信息 ----------------

/** 收件人 / 抄送过多时默认只显示前几个，其余折叠——Outlook 的「+ N」同一套思路。 */
const showAllTo = ref(false)
const showAllCc = ref(false)
const ADDRESS_PREVIEW = 3

const visibleTo = computed(() => {
  const list = props.detail?.to || []
  return showAllTo.value ? list : list.slice(0, ADDRESS_PREVIEW)
})
const hiddenToCount = computed(() =>
  Math.max(0, (props.detail?.to?.length || 0) - ADDRESS_PREVIEW),
)
const hiddenToNames = computed(() =>
  (props.detail?.to || [])
    .slice(ADDRESS_PREVIEW)
    .map((a) => a.name || a.address)
    .join('、'),
)

const visibleCc = computed(() => {
  const list = props.detail?.cc || []
  return showAllCc.value ? list : list.slice(0, ADDRESS_PREVIEW)
})
const hiddenCcCount = computed(() =>
  Math.max(0, (props.detail?.cc?.length || 0) - ADDRESS_PREVIEW),
)
const hiddenCcNames = computed(() =>
  (props.detail?.cc || [])
    .slice(ADDRESS_PREVIEW)
    .map((a) => a.name || a.address)
    .join('、'),
)

/** 换邮件时收起展开状态，否则上一封「展开的收件人」会带到下一封。 */
watch(
  () => `${props.detail?.accountId}:${props.detail?.folder}:${props.detail?.uid}`,
  () => {
    showAllTo.value = false
    showAllCc.value = false
  },
)

/** 日期：列表里放短格式，悬停给出完整时间（含秒与时区偏移）。 */
const dateLabel = computed(() => {
  const unix = props.detail?.date
  if (!unix) return ''
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
})

const fullDateLabel = computed(() => {
  const unix = props.detail?.date
  if (!unix) return ''
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    weekday: 'long',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
})

// ---------------- 地址悬停卡片 ----------------
//
// 卡片信息一次批量取（发件人 + 全部收件人 + 抄送），悬停时按地址从缓存里拿：
// 鼠标划过整行地址不会触发一串 IPC，也不会因为某次请求慢而「卡片迟迟不出来」。
const addressCard = ref(null)
const addressCache = ref({})

async function loadAddressCards() {
  // 会话里每封邮件的发件人 / 收件人 / 抄送都要预取：卡片按地址命中缓存，
  // 只有把整条会话的地址都算进来，悬停任意一封的地址才不会退化成「空卡片」。
  const docs = [props.detail, ...(props.thread || [])].filter(Boolean)
  const addresses = docs
    .flatMap((d) => [
      d.fromAddr,
      ...(d.to || []).map((a) => a.address),
      ...(d.cc || []).map((a) => a.address),
    ])
    .map((a) => String(a || '').trim())
    .filter(Boolean)
  if (!addresses.length) return
  try {
    const cards = await invoke('mail_address_cards', { addresses })
    const next = { ...addressCache.value }
    for (const card of cards || []) next[String(card.email).toLowerCase()] = card
    addressCache.value = next
  } catch {
    /* 通讯录查询失败不该挡住阅读：卡片会退回只显示地址本身 */
  }
}

watch(
  () => `${props.detail?.accountId}:${props.detail?.folder}:${props.detail?.uid}`,
  () => loadAddressCards(),
  { immediate: true },
)

function showAddressCard(address, event) {
  showAddressCardFor(address, event?.currentTarget, event?.type === 'click')
}

/**
 * 显示地址卡片；缓存里没有的地址（正文里的 mailto 链接）现查一次。
 * `allowUnhovered` 用于点击：点完鼠标不一定还停在元素上，仍要出卡片。
 */
async function showAddressCardFor(address, element, allowUnhovered = false) {
  const target = String(address || '').trim()
  if (!target || !element || !addressCard.value) return
  const key = target.toLowerCase()
  let data = addressCache.value[key]
  if (!data) {
    data = await fetchAddressCard(key, target)
    if (!element.isConnected) return
    if (!allowUnhovered && !element.matches?.(':hover')) return
  }
  addressCard.value.show(target, data, element.getBoundingClientRect())
}

async function fetchAddressCard(key, address) {
  try {
    const cards = await invoke('mail_address_cards', { addresses: [address] })
    const found = (cards || []).find(
      (item) => String(item.email || '').toLowerCase() === key,
    )
    if (!found) return null
    addressCache.value = { ...addressCache.value, [key]: found }
    return found
  } catch {
    return null
  }
}

/** 正文里的 mailto 链接：委托到内容区，悬停同样出卡片。 */
let bodyAddressAnchor = null

function mailtoAddress(anchor) {
  const href = anchor?.getAttribute?.('href') || ''
  if (!/^mailto:/i.test(href)) return ''
  const raw = href.slice('mailto:'.length).split('?')[0]
  try {
    return decodeURIComponent(raw).trim()
  } catch {
    return raw.trim()
  }
}

function onBodyAddressOver(event) {
  const anchor = event.target?.closest?.('a[href]')
  if (!anchor || anchor === bodyAddressAnchor) return
  const address = mailtoAddress(anchor)
  if (!address.includes('@')) return
  bodyAddressAnchor = anchor
  showAddressCardFor(address, anchor)
}

function onBodyAddressOut(event) {
  const anchor = event.target?.closest?.('a[href]')
  if (!anchor || anchor !== bodyAddressAnchor) return
  if (event.relatedTarget && anchor.contains(event.relatedTarget)) return
  bodyAddressAnchor = null
  addressCard.value?.scheduleHide()
}

const headerText = computed(() =>
  (props.detail?.headers || []).map(([name, value]) => `${name}: ${value}`).join('\n'),
)

function showImages() {
  imagesAllowed.value = true
}

/** 正文里的链接：交给系统浏览器打开，不在应用内导航。 */
async function onBodyClick(event) {
  const anchor = event.target?.closest?.('a')
  if (!anchor) return
  const href = anchor.getAttribute('href')
  if (!href) return
  event.preventDefault()
  if (!/^https?:/i.test(href)) return
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(href)
  } catch {
    window.open(href, '_blank')
  }
}

/** 导出下拉：选完就复位，避免下次打开还停在上次那一项。 */
function onExportChange(event) {
  const kind = event.target.value
  event.target.value = ''
  if (kind) tools.exportAs(kind).catch((error) => console.warn('[mail-reader] 导出失败', error))
}

function sizeLabel(bytes) {
  const n = Number(bytes) || 0
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

function iconFor(contentType) {
  const type = String(contentType || '').toLowerCase()
  if (type.startsWith('image/')) return '🖼'
  if (type.startsWith('audio/')) return '♪'
  if (type.startsWith('video/')) return '▶'
  if (type.includes('pdf')) return 'PDF'
  if (type.includes('zip') || type.includes('compressed')) return '⌸'
  if (type.includes('word') || type.includes('document')) return 'W'
  if (type.includes('sheet') || type.includes('excel')) return 'X'
  return '📄'
}
</script>
