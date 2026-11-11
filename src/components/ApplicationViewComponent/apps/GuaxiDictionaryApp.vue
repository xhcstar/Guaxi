<template>
  <div class="gx-dict" :class="{ 'is-idle': view === 'search' && !active }">
    <!-- 左侧栏 -->
    <aside class="gx-rail">
      <button type="button" class="gx-brand" title="回到查词" @click="goHome">
        <span class="gx-seal">G</span>
        <span class="gx-brand-text">
          <strong>词典</strong>
          <small>GUAXI LEXICON</small>
        </span>
      </button>

      <nav class="gx-nav">
        <button
          v-for="item in navItems"
          :key="item.key"
          type="button"
          class="gx-nav-item"
          :class="{ active: view === item.key }"
          @click="switchView(item.key)"
        >
          <Icon :icon="item.icon" width="17" height="17" />
          <span>{{ item.label }}</span>
          <em v-if="item.count">{{ item.count }}</em>
        </button>
      </nav>

      <div class="gx-rail-foot">
        <button type="button" class="gx-lib-btn" @click="managerVisible = true">
          <Icon :icon="libraryIcon" width="15" height="15" />
          <span>词典库</span>
        </button>
        <p class="gx-lib-stat">
          <b>{{ enabledPacks.length }}</b> / {{ packs.length }} 本启用
          <br />
          {{ totalEntries.toLocaleString() }} 词条
        </p>
      </div>
    </aside>

    <!-- 主区 -->
    <section class="gx-main">
      <header class="gx-searchbar">
        <div class="gx-field">
          <Icon :icon="searchIcon" width="16" height="16" class="gx-field-icon" />
          <input
            ref="searchInput"
            v-model="query"
            type="text"
            spellcheck="false"
            placeholder="查单词、词组或整段文字…"
            @input="onQueryInput"
            @keydown.enter.prevent="submit"
          />
          <button v-if="query" type="button" class="gx-clear" title="清空" @click="clearQuery">
            <Icon :icon="xIcon" width="13" height="13" />
          </button>
          <span class="gx-enter">↵</span>
        </div>

        <div class="gx-langpair">
          <n-select v-model:value="fromLang" size="small" :options="langOptions" style="width: 104px" />
          <button type="button" class="gx-swap" title="交换语言" @click="swapLangs">
            <Icon :icon="swapIcon" width="14" height="14" />
          </button>
          <n-select v-model:value="toLang" size="small" :options="targetLangOptions" style="width: 104px" />
        </div>
      </header>

      <div ref="scrollRef" class="gx-scroll">
        <!-- 查词 -->
        <div v-if="view === 'search'" class="gx-search-view">
          <!-- 首页 -->
          <template v-if="!active">
            <section class="gx-daily">
              <div class="gx-daily-head">
                <span class="gx-eyebrow">每日一句</span>
                <span class="gx-daily-date">{{ dailyDate }}</span>
              </div>
              <p class="gx-daily-en">{{ daily.en }}</p>
              <p class="gx-daily-zh">{{ daily.zh }}</p>
              <div class="gx-daily-foot">
                <span class="gx-daily-from">{{ daily.from }}</span>
                <button type="button" @click="speak(daily.en, 1)">
                  <Icon :icon="volumeIcon" width="13" height="13" />常速
                </button>
                <button type="button" @click="speak(daily.en, 0.7)">
                  <Icon :icon="volumeIcon" width="13" height="13" />慢速
                </button>
              </div>
            </section>

            <section v-if="recentWords.length" class="gx-recent">
              <h3 class="gx-sec-title">最近查过</h3>
              <div class="gx-chips">
                <button v-for="word in recentWords" :key="word" type="button" class="gx-chip" @click="searchWord(word)">
                  {{ word }}
                </button>
              </div>
            </section>

            <section class="gx-status">
              <div class="gx-status-item">
                <span class="gx-status-num">{{ enabledPacks.length }}</span>
                <span class="gx-status-label">离线词库启用中</span>
              </div>
              <div class="gx-status-item">
                <span class="gx-status-num">{{ totalEntries.toLocaleString() }}</span>
                <span class="gx-status-label">本地词条</span>
              </div>
              <div class="gx-status-item">
                <span class="gx-status-num">{{ vocabCount }}</span>
                <span class="gx-status-label">生词本记录</span>
              </div>
            </section>

            <p class="gx-tip">
              输入单词会同时查本地词库与在线词条；输入句子或段落则翻译并自动记入语言学习。
              支持 MDX / StarDict / DSL 词库，可在「词典库」中导入与管理。
            </p>
          </template>

          <!-- 加载中 -->
          <div v-else-if="loading" class="gx-loading">
            <n-spin size="small" />
            <span>正在查词…</span>
          </div>

          <template v-else>
            <!-- 单词 -->
            <article v-if="isWord" class="gx-word">
              <header class="gx-word-head">
                <div class="gx-word-title-wrap">
                  <h1 class="gx-word-title">{{ headword }}</h1>
                  <div v-if="phonetics.length" class="gx-phons">
                    <span v-for="ph in phonetics" :key="ph.accent" class="gx-phon">
                      <em>{{ ph.accent }}</em>
                      <span class="gx-phon-text">{{ ph.text }}</span>
                      <button type="button" class="gx-phon-play" :title="`${ph.accent}发音`" @click="playWord(ph.accent)">
                        <Icon :icon="volumeIcon" width="13" height="13" />
                      </button>
                    </span>
                  </div>
                </div>
                <div class="gx-word-tools">
                  <button type="button" class="gx-tool" :class="{ 'is-done': recorded }" @click="collectWord">
                    <Icon :icon="recorded ? checkIcon : starIcon" width="14" height="14" />
                    <span>{{ recorded ? '已记入生词本' : '记入生词本' }}</span>
                  </button>
                </div>
              </header>

              <p v-if="leadTranslation" class="gx-word-lead">{{ leadTranslation }}</p>

              <div class="gx-tags">
                <span v-if="wordMeta?.part_of_speech" class="gx-tag is-pos">{{ wordMeta.part_of_speech }}</span>
                <span v-for="pack in packHits" :key="pack.pack_id" class="gx-tag">
                  {{ pack.pack_name }}
                </span>
                <span v-if="!packHits.length" class="gx-tag is-quiet">在线词条</span>
              </div>

              <!-- 离线词库分组 -->
              <section v-for="pack in packHits" :key="pack.pack_id" class="gx-pack">
                <header class="gx-pack-head">
                  <span class="gx-pack-name">{{ pack.pack_name }}</span>
                  <span class="gx-pack-meta">{{ formatLabel(pack.format) }}</span>
                  <button type="button" class="gx-pack-toggle" @click="togglePack(pack.pack_id)">
                    {{ collapsed[pack.pack_id] ? '展开释义' : '收起' }}
                  </button>
                </header>
                <div v-if="!collapsed[pack.pack_id]" class="gx-definition">
                  <div
                    v-for="(entry, index) in packEntries(pack)"
                    :key="`${pack.pack_id}-${index}`"
                    class="gx-definition-block"
                    v-html="entry"
                  ></div>
                </div>
              </section>

              <!-- 在线词条 -->
              <p v-if="!packHits.length && !wordMeta" class="gx-empty-def">
                本地词库与在线词条都没有收录「{{ headword }}」。可以到「词典库」导入更完整的离线词库。
              </p>

              <section v-if="wordMeta" class="gx-online">
                <div v-if="meaningList.length" class="gx-sec">
                  <h3 class="gx-sec-title">释义</h3>
                  <ol class="gx-meanings">
                    <li v-for="(meaning, index) in meaningList" :key="`m-${index}`">{{ meaning }}</li>
                  </ol>
                </div>

                <div v-if="phrasePairs.length" class="gx-sec">
                  <h3 class="gx-sec-title">常用短语</h3>
                  <ul class="gx-pairs">
                    <li v-for="(pair, index) in phrasePairs" :key="`p-${index}`">
                      <span class="gx-pair-src">{{ pair.source }}</span>
                      <span v-if="pair.target" class="gx-pair-dst">{{ pair.target }}</span>
                    </li>
                  </ul>
                </div>

                <div v-if="examplePairs.length" class="gx-sec">
                  <h3 class="gx-sec-title">例句</h3>
                  <ul class="gx-pairs">
                    <li v-for="(pair, index) in examplePairs" :key="`e-${index}`">
                      <span class="gx-pair-src">{{ pair.source }}</span>
                      <span v-if="pair.target" class="gx-pair-dst">{{ pair.target }}</span>
                    </li>
                  </ul>
                </div>

                <div v-for="group in lexGroups" :key="group.label" class="gx-sec">
                  <h3 class="gx-sec-title">{{ group.label }}</h3>
                  <div class="gx-chips">
                    <button
                      v-for="item in group.items"
                      :key="`${group.label}-${item.word}`"
                      type="button"
                      class="gx-chip is-link"
                      @click="searchWord(item.word)"
                    >
                      {{ item.word }}<span v-if="item.translation" class="gx-chip-sub"> · {{ item.translation }}</span>
                    </button>
                  </div>
                </div>
              </section>

              <p class="gx-source-note">
                记录已写入语言学习，可在「生词本」里复习；发音优先使用有道词典音频，离线时退回系统语音。
              </p>
            </article>

            <!-- 句子 / 段落 -->
            <article v-else class="gx-translate">
              <div class="gx-src">
                <header class="gx-src-head">
                  <span class="gx-eyebrow">{{ sourceKind }}</span>
                  <button type="button" class="gx-tool is-quiet" @click="copyText(rawText)">
                    <Icon :icon="copyIcon" width="13" height="13" />
                    <span>复制原文</span>
                  </button>
                </header>
                <p class="gx-src-text">{{ rawText }}</p>
              </div>

              <div class="gx-engines">
                <article v-for="item in engines" :key="item.engine" class="gx-engine">
                  <header>
                    <span class="gx-engine-name">{{ item.engine }}</span>
                    <button
                      v-if="item.success && item.text"
                      type="button"
                      class="gx-tool is-quiet"
                      @click="copyText(item.text)"
                    >
                      <Icon :icon="copyIcon" width="13" height="13" />
                    </button>
                  </header>
                  <p class="gx-engine-text" :class="{ 'is-error': !item.success }">
                    {{ item.success ? item.text : item.error || '翻译失败' }}
                  </p>
                </article>
              </div>

              <p v-if="error" class="gx-error">{{ error }}</p>
              <p v-if="recorded" class="gx-recorded">
                <Icon :icon="checkIcon" width="13" height="13" />
                已记入语言学习，可在「历史」中复习。
              </p>
            </article>
          </template>
        </div>

        <!-- 生词本 / 历史 -->
        <div v-else class="gx-list-view">
          <header class="gx-list-head">
            <div>
              <span class="gx-eyebrow">{{ view === 'vocab' ? 'VOCABULARY' : 'HISTORY' }}</span>
              <h2>{{ view === 'vocab' ? '生词本' : '查词历史' }}</h2>
              <p class="gx-list-sub">{{ view === 'vocab' ? '查过的单词会自动收录，点击即可重新查阅。' : '单词、句子与段落的全部翻译记录。' }}</p>
            </div>
            <div class="gx-list-tools">
              <input v-model="recordKeyword" type="text" placeholder="筛选…" @input="scheduleRecords" />
              <n-select
                v-model:value="recordLang"
                size="small"
                :options="recordLangOptions"
                style="width: 100px"
                @update:value="loadRecords"
              />
            </div>
          </header>

          <div v-if="recordsLoading" class="gx-loading"><n-spin size="small" /><span>读取中…</span></div>
          <div v-else-if="!records.length" class="gx-empty-list">
            <Icon :icon="view === 'vocab' ? starIcon : historyIcon" width="28" height="28" />
            <p>{{ recordKeyword ? '没有匹配的记录' : '还没有记录，先去查一个词吧。' }}</p>
          </div>

          <ul v-else class="gx-records">
            <li v-for="record in records" :key="record.id">
              <button type="button" class="gx-record-main" @click="openRecord(record)">
                <span class="gx-record-word">{{ record.record_type === 'word' ? record.word || record.source_text : record.source_text }}</span>
                <span class="gx-record-text">{{ record.translated_text }}</span>
              </button>
              <div class="gx-record-side">
                <span class="gx-record-engine">{{ record.engine }}</span>
                <span class="gx-record-date">{{ String(record.created_at || '').slice(0, 10) }}</span>
                <button type="button" class="gx-record-del" title="删除" @click.stop="deleteRecord(record)">
                  <Icon :icon="trashIcon" width="13" height="13" />
                </button>
              </div>
            </li>
          </ul>
        </div>
      </div>
    </section>

    <DictionaryManager v-model:show="managerVisible" @changed="onPacksChanged" />
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import searchIcon from '@iconify-icons/lucide/search'
import xIcon from '@iconify-icons/lucide/x'
import swapIcon from '@iconify-icons/lucide/arrow-left-right'
import volumeIcon from '@iconify-icons/lucide/volume-2'
import starIcon from '@iconify-icons/lucide/star'
import checkIcon from '@iconify-icons/lucide/check'
import copyIcon from '@iconify-icons/lucide/copy'
import trashIcon from '@iconify-icons/lucide/trash-2'
import libraryIcon from '@iconify-icons/lucide/library'
import historyIcon from '@iconify-icons/lucide/history'
import bookIcon from '@iconify-icons/lucide/book-open'
import DictionaryManager from './dictionary/DictionaryManager.vue'

const LANG_OPTIONS = [
  { label: '自动检测', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' },
]
const RECORD_LANG_OPTIONS = LANG_OPTIONS.filter((item) => item.value !== 'auto')
const langOptions = LANG_OPTIONS
const targetLangOptions = RECORD_LANG_OPTIONS

const DAILY_QUOTES = [
  { en: 'A word after a word after a word is power.', zh: '一个词接一个词，再接着一个词，这就是力量。', from: 'Margaret Atwood' },
  { en: 'The limits of my language mean the limits of my world.', zh: '我的语言的界限，意味着我的世界的界限。', from: 'Ludwig Wittgenstein' },
  { en: 'To have another language is to possess a second soul.', zh: '掌握另一门语言，就是拥有第二个灵魂。', from: 'Charlemagne' },
  { en: 'Language is the road map of a culture.', zh: '语言是一种文化的地图。', from: 'Rita Mae Brown' },
  { en: 'One language sets you in a corridor for life. Two languages open every door along the way.', zh: '一门语言让你走上一条走廊，两门语言为你打开沿途每一扇门。', from: 'Frank Smith' },
  { en: 'Learning another language is not only learning different words for the same things, but learning another way to think about things.', zh: '学另一门语言不只是记住同一件事的不同说法，而是学会另一种思考方式。', from: 'Flora Lewis' },
  { en: 'The more that you read, the more things you will know.', zh: '你读得越多，知道得就越多。', from: 'Dr. Seuss' },
  { en: 'Words are, of course, the most powerful drug used by mankind.', zh: '词语当然是人类使用过的最有力的药物。', from: 'Rudyard Kipling' },
  { en: 'You can never understand one language until you understand at least two.', zh: '除非你至少懂两门语言，否则你无法真正理解其中任何一门。', from: 'Geoffrey Willans' },
  { en: 'A different language is a different vision of life.', zh: '不同的语言，是不同的生活视野。', from: 'Federico Fellini' },
  { en: 'Language shapes the way we think, and determines what we can think about.', zh: '语言塑造我们的思维方式，并决定我们能思考什么。', from: 'Benjamin Lee Whorf' },
]

const view = ref('search')
const query = ref('')
const fromLang = ref('auto')
const toLang = ref('zh')
const active = ref(false)
const loading = ref(false)
const isWord = ref(false)
const headword = ref('')
const rawText = ref('')
const packHits = ref([])
const wordMeta = ref(null)
const engines = ref([])
const error = ref('')
const recorded = ref(false)
const collapsed = ref({})
const managerVisible = ref(false)
const packs = ref([])
const searchInput = ref(null)
const scrollRef = ref(null)

const records = ref([])
const recordsLoading = ref(false)
const recordKeyword = ref('')
const recordLang = ref('en')
const vocabCount = ref(0)
const recentWords = ref([])

let debounceTimer = 0
let recordTimer = 0
let activeAudio = null
let queryToken = 0

const RECENT_KEY = 'guaxi_dictionary_recent'
const LANG_KEY = 'guaxi_dictionary_langs'

const navItems = computed(() => [
  { key: 'search', label: '查词', icon: searchIcon, count: '' },
  { key: 'vocab', label: '生词本', icon: bookIcon, count: vocabCount.value || '' },
  { key: 'history', label: '历史', icon: historyIcon, count: '' },
])

const enabledPacks = computed(() => packs.value.filter((pack) => pack.enabled !== false))
const totalEntries = computed(() => packs.value.reduce((sum, pack) => sum + Number(pack.entry_count || 0), 0))

const daily = computed(() => {
  const now = new Date()
  const start = new Date(now.getFullYear(), 0, 0)
  const day = Math.floor((now - start) / 86400000)
  return DAILY_QUOTES[day % DAILY_QUOTES.length]
})
const dailyDate = computed(() => {
  const now = new Date()
  return `${now.getMonth() + 1} 月 ${now.getDate()} 日`
})

const phonetics = computed(() => {
  const meta = wordMeta.value || {}
  const list = []
  const uk = String(meta.phonetic_uk || '').trim()
  const us = String(meta.phonetic_us || '').trim()
  const plain = String(meta.phonetic || '').trim()
  if (uk) list.push({ accent: '英', text: uk.startsWith('/') ? uk : `/${uk}/`, kind: 'uk' })
  if (us) list.push({ accent: '美', text: us.startsWith('/') ? us : `/${us}/`, kind: 'us' })
  if (!list.length && plain) list.push({ accent: '音标', text: plain.startsWith('/') ? plain : `/${plain}/`, kind: 'us' })
  return list
})

const leadTranslation = computed(() => {
  const meta = wordMeta.value || {}
  const translation = String(meta.translation || '').trim()
  if (translation) return translation
  const meanings = Array.isArray(meta.meanings) ? meta.meanings : []
  return meanings.slice(0, 3).join('；')
})

const meaningList = computed(() => {
  const meta = wordMeta.value || {}
  const meanings = Array.isArray(meta.meanings) ? meta.meanings.map((item) => String(item || '').trim()).filter(Boolean) : []
  const translation = String(meta.translation || '').trim()
  if (meanings.length) return meanings.slice(0, 10)
  // 离线词库只有一行行释义时，把 translation 按分隔符拆开
  return translation
    .split(/[;；|]/)
    .map((item) => item.trim())
    .filter(Boolean)
    .slice(0, 10)
})

const pairList = (sources, translations) => {
  const src = Array.isArray(sources) ? sources : []
  const dst = Array.isArray(translations) ? translations : []
  return src
    .slice(0, 12)
    .map((source, index) => ({ source: String(source || '').trim(), target: String(dst[index] || '').trim() }))
    .filter((pair) => pair.source)
}
const phrasePairs = computed(() => {
  const meta = wordMeta.value || {}
  return pairList(meta.phrases, meta.phrase_translations)
})
const examplePairs = computed(() => {
  const meta = wordMeta.value || {}
  return pairList(meta.examples, meta.example_translations)
})

const lexGroups = computed(() => {
  const meta = wordMeta.value || {}
  const build = (label, words, translations) => {
    const list = Array.isArray(words) ? words : []
    const trans = Array.isArray(translations) ? translations : []
    const items = list
      .slice(0, 16)
      .map((word, index) => ({ word: String(word || '').trim(), translation: String(trans[index] || '').trim() }))
      .filter((item) => item.word)
    return items.length ? { label, items } : null
  }
  return [
    build('近义词', meta.synonyms, meta.synonyms_translations),
    build('反义词', meta.antonyms, meta.antonyms_translations),
    build('联想词', meta.associates, meta.associates_translations),
  ].filter(Boolean)
})

const sourceKind = computed(() => {
  const text = rawText.value.trim()
  if (!text) return '原文'
  if (/[\n。！？.!?]/.test(text) || text.length > 40) return '段落'
  if (/\s/.test(text)) return '句子'
  return '词语'
})

const formatLabel = (format) => {
  const map = {
    mdx: 'MDict',
    stardict: 'StarDict',
    dsl: 'DSL',
    wiktionary: 'Wiktionary',
    ecdict_csv: 'ECDICT',
    cedict_zip: 'CC-CEDICT',
    zip: 'ZIP',
    csv: 'CSV',
    json: 'JSON',
  }
  return map[String(format || '')] || '词库'
}

const isWordLike = (text) => {
  const value = String(text || '').trim()
  if (!value || /\s/.test(value)) return false
  return /^[A-Za-z][A-Za-z'’-]{0,47}$/.test(value) || /^[\u3040-\u30ff\uac00-\ud7af\u4e00-\u9fff]{1,12}$/.test(value)
}

const persistRecent = (word) => {
  const value = String(word || '').trim()
  if (!value) return
  const list = [value, ...recentWords.value.filter((item) => item !== value)].slice(0, 18)
  recentWords.value = list
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list))
  } catch {}
}

const normalizeWordMeta = (word, basic, details) => {
  if (!basic && !details) return null
  const base = {
    word: String(basic?.word || word || '').trim(),
    phonetic: String(basic?.phonetic || '').trim(),
    phonetic_uk: String(basic?.phonetic_uk || '').trim(),
    phonetic_us: String(basic?.phonetic_us || '').trim(),
    part_of_speech: String(basic?.part_of_speech || '').trim(),
    translation: String(basic?.translation || '').trim(),
    meanings: Array.isArray(basic?.meanings) ? basic.meanings : [],
    examples: Array.isArray(basic?.examples) ? basic.examples : [],
    synonyms: [],
    antonyms: [],
    associates: [],
    synonyms_translations: [],
    antonyms_translations: [],
    associates_translations: [],
    phrases: [],
    phrase_translations: [],
    example_translations: [],
  }
  if (!details) return base
  return {
    ...base,
    synonyms: Array.isArray(details.synonyms) ? details.synonyms : [],
    antonyms: Array.isArray(details.antonyms) ? details.antonyms : [],
    associates: Array.isArray(details.associates) ? details.associates : [],
    synonyms_translations: Array.isArray(details.synonyms_translations) ? details.synonyms_translations : [],
    antonyms_translations: Array.isArray(details.antonyms_translations) ? details.antonyms_translations : [],
    associates_translations: Array.isArray(details.associates_translations) ? details.associates_translations : [],
    phrases: Array.isArray(details.phrases) ? details.phrases : [],
    phrase_translations: Array.isArray(details.phrase_translations) ? details.phrase_translations : [],
    examples: Array.isArray(details.examples) && details.examples.length ? details.examples : base.examples,
    example_translations: Array.isArray(details.example_translations) ? details.example_translations : [],
  }
}

const recordLookup = async (payload) => {
  try {
    await invoke('language_learning_record_translation', { input: payload })
    recorded.value = true
    if (payload.record_type === 'word') refreshVocabCount()
  } catch (err) {
    console.warn('[Dictionary] record failed', err)
  }
}

const searchWord = async (text) => {
  const value = String(text || '').trim()
  if (!value) return
  query.value = value
  await submit()
}

const submit = async () => {
  const value = String(query.value || '').trim()
  if (!value) return
  const token = ++queryToken
  active.value = true
  loading.value = true
  error.value = ''
  recorded.value = false
  engines.value = []
  packHits.value = []
  wordMeta.value = null
  rawText.value = value
  collapsed.value = {}
  isWord.value = isWordLike(value)
  headword.value = value
  persistRecent(value)

  try {
    if (isWord.value) {
      const [hits, basic, details] = await Promise.all([
        invoke('dict_lookup_grouped', {
          word: value,
          sourceLang: fromLang.value,
          targetLang: toLang.value,
          perPackLimit: 3,
          maxPacks: 6,
        }).catch(() => []),
        invoke('language_learning_enrich_word_basic', {
          word: value,
          sourceLang: fromLang.value,
          targetLang: toLang.value,
        }).catch(() => null),
        invoke('language_learning_enrich_word_details', {
          word: value,
          sourceLang: fromLang.value,
          targetLang: toLang.value,
        }).catch(() => null),
      ])
      if (token !== queryToken) return
      packHits.value = Array.isArray(hits) ? hits : []
      wordMeta.value = normalizeWordMeta(value, basic, details)
      const lead = leadTranslation.value || packHits.value?.[0]?.entries?.[0]?.meanings?.join('；') || ''
      if (lead) {
        await recordLookup({
          source_text: value,
          translated_text: lead,
          source_lang: fromLang.value,
          target_lang: toLang.value,
          engine: packHits.value.length ? `离线词典:${packHits.value[0].pack_name}` : '在线词条',
          record_type: 'word',
          word_meta: wordMeta.value || undefined,
        })
      }
    } else {
      let items = []
      try {
        const response = await invoke('selection_translate_free_multi', {
          text: value,
          from: fromLang.value,
          to: toLang.value,
        })
        if (token !== queryToken) return
        items = (Array.isArray(response?.items) ? response.items : []).map((item) => ({
          engine: String(item?.engine || ''),
          success: Boolean(item?.success),
          text: String(item?.text || ''),
          error: item?.error ? String(item.error) : '',
        }))
        if (!items.some((item) => item.success && item.text)) {
          throw new Error(String(response?.error || '翻译结果为空'))
        }
      } catch (firstError) {
        try {
          const response = await invoke('translate_text_free', {
            texts: [value],
            from: fromLang.value,
            to: toLang.value,
          })
          if (token !== queryToken) return
          const text = String(response?.translated_texts?.[0] || '').trim()
          if (!text || text.includes('[Translation Failed]')) {
            throw new Error(String(response?.error || '翻译结果为空'))
          }
          items = [{ engine: 'Free', success: true, text, error: '' }]
        } catch (secondError) {
          if (token !== queryToken) return
          error.value = String(secondError?.message || secondError || firstError?.message || '翻译失败')
          items = [{ engine: 'Free', success: false, text: '', error: error.value }]
        }
      }
      engines.value = items
      const primary = items.find((item) => item.success && item.text)
      if (primary) {
        await recordLookup({
          source_text: value,
          translated_text: primary.text,
          source_lang: fromLang.value,
          target_lang: toLang.value,
          engine: primary.engine || 'Free',
        })
      }
    }
  } finally {
    if (token === queryToken) loading.value = false
  }
}

const onQueryInput = () => {
  if (debounceTimer) window.clearTimeout(debounceTimer)
  const value = String(query.value || '').trim()
  if (!value) {
    active.value = false
    loading.value = false
    queryToken += 1
    return
  }
  debounceTimer = window.setTimeout(() => {
    debounceTimer = 0
    submit()
  }, 420)
}

const clearQuery = () => {
  query.value = ''
  active.value = false
  loading.value = false
  error.value = ''
  engines.value = []
  packHits.value = []
  wordMeta.value = null
  queryToken += 1
  searchInput.value?.focus()
}

const goHome = () => {
  view.value = 'search'
  clearQuery()
}

const swapLangs = () => {
  const from = fromLang.value
  fromLang.value = toLang.value
  toLang.value = from === 'auto' ? 'zh' : from
}

const togglePack = (id) => {
  collapsed.value = { ...collapsed.value, [id]: !collapsed.value[id] }
}

const plainDefinition = (pack) => {
  const entry = pack?.entries?.[0]
  const meanings = Array.isArray(entry?.meanings) ? entry.meanings : []
  if (!meanings.length) return '<p class="gx-def-empty">该词库未给出释义</p>'
  return meanings.map((item) => `<p>${escapeHtml(item)}</p>`).join('')
}
// 一个词在同一本词典里可能有多条记录（多义项被拆开），全部展开给用户看
const packEntries = (pack) => {
  const entries = Array.isArray(pack?.entries) ? pack.entries : []
  const blocks = entries
    .map((entry) => String(entry?.definition_html || '').trim())
    .filter((html) => html.length > 0)
  if (blocks.length) return blocks
  return [plainDefinition(pack)]
}
const escapeHtml = (text) =>
  String(text || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

const copyText = async (text) => {
  const value = String(text || '').trim()
  if (!value) return
  try {
    await navigator.clipboard.writeText(value)
  } catch {}
}

const playWord = (accent = 'us') => {
  const word = String(headword.value || '').trim()
  if (!word) return
  playUrl(`https://dict.youdao.com/dictvoice?audio=${encodeURIComponent(word)}&type=${accent === 'uk' ? 1 : 0}`)
}

const playUrl = (url) => {
  try {
    if (activeAudio) {
      activeAudio.pause()
      activeAudio = null
    }
    activeAudio = new Audio(url)
    activeAudio.play().catch(() => speak(headword.value))
  } catch {
    speak(headword.value)
  }
}

const speak = (text, rate = 1) => {
  const value = String(text || '').trim()
  if (!value) return
  try {
    if (!('speechSynthesis' in window)) return
    window.speechSynthesis.cancel()
    const utterance = new SpeechSynthesisUtterance(value)
    utterance.rate = rate
    utterance.lang = /[\u4e00-\u9fff]/.test(value) ? 'zh-CN' : 'en-US'
    window.speechSynthesis.speak(utterance)
  } catch {}
}

const collectWord = async () => {
  if (recorded.value) return
  const lead = leadTranslation.value
  if (!lead) return
  await recordLookup({
    source_text: headword.value,
    translated_text: lead,
    source_lang: fromLang.value,
    target_lang: toLang.value,
    engine: packHits.value.length ? `离线词典:${packHits.value[0].pack_name}` : '在线词条',
    record_type: 'word',
    word_meta: wordMeta.value || undefined,
  })
}

const switchView = (next) => {
  view.value = next
  if (next === 'search') return
  recordLang.value = inferRecordLang()
  loadRecords()
}

const inferRecordLang = () => {
  const sample = String(headword.value || query.value || '').trim()
  if (/[\u3040-\u30ff]/.test(sample)) return 'ja'
  if (/[\uac00-\ud7af]/.test(sample)) return 'ko'
  if (/[\u0400-\u04ff]/.test(sample)) return 'ru'
  if (/[\u4e00-\u9fff]/.test(sample)) return 'zh'
  return 'en'
}

const loadRecords = async () => {
  recordsLoading.value = true
  try {
    const kind = view.value === 'vocab' ? 'word' : 'all'
    const list = await invoke('language_learning_list_records', {
      keyword: String(recordKeyword.value || '').trim(),
      kind,
      learningLang: recordLang.value,
      limit: 200,
      offset: 0,
    })
    records.value = Array.isArray(list) ? list : []
    if (view.value === 'vocab') vocabCount.value = records.value.length
  } catch (err) {
    console.warn('[Dictionary] load records failed', err)
  } finally {
    recordsLoading.value = false
  }
}

const scheduleRecords = () => {
  if (recordTimer) window.clearTimeout(recordTimer)
  recordTimer = window.setTimeout(() => {
    recordTimer = 0
    loadRecords()
  }, 320)
}

const openRecord = (record) => {
  const text = record.record_type === 'word' ? record.word || record.source_text : record.source_text
  view.value = 'search'
  searchWord(text)
}

const deleteRecord = async (record) => {
  try {
    await invoke('language_learning_delete_records', {
      ids: [String(record.id)],
      learningLang: recordLang.value,
    })
    records.value = records.value.filter((item) => item.id !== record.id)
    if (view.value === 'vocab') vocabCount.value = Math.max(0, vocabCount.value - 1)
  } catch (err) {
    console.warn('[Dictionary] delete record failed', err)
  }
}

const refreshPacks = async () => {
  try {
    const list = await invoke('language_learning_dict_list')
    packs.value = Array.isArray(list) ? list : []
  } catch {}
}

const onPacksChanged = () => {
  refreshPacks()
  loadRecords()
}

const refreshVocabCount = async () => {
  try {
    const counts = await invoke('language_learning_get_counts', {
      keyword: '',
      kind: 'word',
      learningLang: recordLang.value,
    })
    const value = Number(counts?.records ?? 0)
    if (Number.isFinite(value) && value > 0) vocabCount.value = value
  } catch {}
}

onMounted(() => {
  try {
    const raw = localStorage.getItem(RECENT_KEY)
    if (raw) {
      const list = JSON.parse(raw)
      if (Array.isArray(list)) recentWords.value = list.slice(0, 18)
    }
    const langs = JSON.parse(localStorage.getItem(LANG_KEY) || 'null')
    if (langs && typeof langs === 'object') {
      if (langs.from) fromLang.value = langs.from
      if (langs.to) toLang.value = langs.to
    }
  } catch {}
  refreshPacks()
  recordLang.value = 'en'
  refreshVocabCount()
  searchInput.value?.focus()
})

onBeforeUnmount(() => {
  if (debounceTimer) window.clearTimeout(debounceTimer)
  if (recordTimer) window.clearTimeout(recordTimer)
  if (activeAudio) {
    activeAudio.pause()
    activeAudio = null
  }
  try {
    window.speechSynthesis?.cancel()
  } catch {}
})

watch([fromLang, toLang], () => {
  try {
    localStorage.setItem(LANG_KEY, JSON.stringify({ from: fromLang.value, to: toLang.value }))
  } catch {}
})
</script>

<style scoped>
.gx-dict {
  --gx-paper: var(--guaxi-app-bg, #ffffff);
  --gx-panel: var(--guaxi-panel-bg, #ffffff);
  --gx-ink: var(--guaxi-text-primary, #1f2328);
  --gx-muted: var(--guaxi-text-secondary, #57606a);
  --gx-rule: var(--guaxi-border, #d0d7de);
  --gx-jade: #2f7d6d;
  --gx-amber: #a9702a;
  --gx-jade-soft: color-mix(in srgb, var(--gx-jade) 10%, transparent);
  --gx-serif: 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, 'Songti SC', 'Noto Serif SC', serif;

  display: flex;
  height: 100%;
  min-height: 0;
  background: var(--gx-paper);
  color: var(--gx-ink);
  overflow: hidden;
}

/* ── 侧栏 ── */
.gx-rail {
  width: 176px;
  flex: 0 0 176px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 18px 14px 14px;
  border-right: 1px solid var(--gx-rule);
  background: linear-gradient(180deg, color-mix(in srgb, var(--gx-panel) 92%, var(--gx-jade) 8%), var(--gx-panel));
}

.gx-brand {
  display: flex;
  align-items: center;
  gap: 9px;
  border: 0;
  background: transparent;
  padding: 0;
  cursor: pointer;
  text-align: left;
  color: inherit;
}

.gx-seal {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: var(--gx-jade);
  color: #fff;
  font-family: var(--gx-serif);
  font-size: 18px;
  font-weight: 700;
  box-shadow: inset 0 -2px 0 rgba(0, 0, 0, 0.18);
}

.gx-brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.15;
}
.gx-brand-text strong {
  font-family: var(--gx-serif);
  font-size: 17px;
  letter-spacing: 0.14em;
}
.gx-brand-text small {
  font-size: 8px;
  letter-spacing: 0.16em;
  color: var(--gx-muted);
}

.gx-nav {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.gx-nav-item {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 9px 10px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--gx-muted);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.gx-nav-item:hover {
  background: var(--gx-jade-soft);
  color: var(--gx-ink);
}
.gx-nav-item.active {
  background: color-mix(in srgb, var(--gx-jade) 16%, transparent);
  color: var(--gx-ink);
  font-weight: 600;
}
.gx-nav-item em {
  margin-left: auto;
  font-style: normal;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--gx-jade) 18%, transparent);
  color: var(--gx-jade);
}

.gx-rail-foot {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.gx-lib-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px;
  border-radius: 9px;
  border: 1px solid var(--gx-rule);
  background: transparent;
  color: var(--gx-ink);
  font-size: 12px;
  cursor: pointer;
}
.gx-lib-btn:hover {
  border-color: color-mix(in srgb, var(--gx-jade) 45%, var(--gx-rule));
  background: var(--gx-jade-soft);
}

.gx-lib-stat {
  margin: 0;
  font-size: 10px;
  line-height: 1.7;
  color: var(--gx-muted);
}
.gx-lib-stat b {
  color: var(--gx-ink);
}

/* ── 主区 ── */
.gx-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.gx-searchbar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 22px;
  border-bottom: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 96%, var(--gx-jade) 4%);
}

.gx-field {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  height: 38px;
  border-radius: 10px;
  border: 1px solid var(--gx-rule);
  background: var(--gx-paper);
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.gx-field:focus-within {
  border-color: color-mix(in srgb, var(--gx-jade) 55%, var(--gx-rule));
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--gx-jade) 12%, transparent);
}
.gx-field-icon {
  color: var(--gx-muted);
  flex: 0 0 auto;
}
.gx-field input {
  flex: 1;
  min-width: 0;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--gx-ink);
  font-size: 14.5px;
  font-family: var(--gx-serif);
}
.gx-field input::placeholder {
  color: var(--gx-muted);
  opacity: 0.75;
}
.gx-clear {
  border: 0;
  background: transparent;
  color: var(--gx-muted);
  cursor: pointer;
  line-height: 0;
}
.gx-clear:hover {
  color: var(--gx-ink);
}
.gx-enter {
  font-size: 10px;
  color: var(--gx-muted);
  border: 1px solid var(--gx-rule);
  border-radius: 4px;
  padding: 0 4px;
}

.gx-langpair {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}

.gx-swap {
  border: 0;
  background: transparent;
  color: var(--gx-muted);
  cursor: pointer;
  padding: 4px;
  line-height: 0;
  border-radius: 6px;
}
.gx-swap:hover {
  color: var(--gx-jade);
  background: var(--gx-jade-soft);
}

.gx-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 22px 26px 34px;
}

/* ── 首页 ── */
.gx-search-view {
  max-width: 760px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.gx-daily {
  padding: 20px 22px;
  border-radius: 14px;
  border: 1px solid var(--gx-rule);
  background:
    radial-gradient(120% 120% at 0% 0%, color-mix(in srgb, var(--gx-jade) 9%, transparent), transparent 60%),
    linear-gradient(180deg, color-mix(in srgb, var(--gx-panel) 96%, var(--gx-amber) 4%), var(--gx-panel));
}
.gx-daily-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 10px;
}
.gx-eyebrow {
  font-size: 10px;
  letter-spacing: 0.2em;
  text-transform: uppercase;
  color: var(--gx-muted);
}
.gx-daily-date {
  font-family: var(--gx-serif);
  font-size: 13px;
  color: var(--gx-jade);
}
.gx-daily-en {
  margin: 0;
  font-family: var(--gx-serif);
  font-size: 22px;
  line-height: 1.5;
  letter-spacing: 0.01em;
}
.gx-daily-zh {
  margin: 8px 0 0;
  font-size: 13px;
  line-height: 1.8;
  color: var(--gx-muted);
}
.gx-daily-foot {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
}
.gx-daily-from {
  margin-right: auto;
  font-size: 11px;
  color: var(--gx-muted);
}
.gx-daily-foot button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 10px;
  font-size: 11px;
  border-radius: 999px;
  border: 1px solid var(--gx-rule);
  background: transparent;
  color: var(--gx-ink);
  cursor: pointer;
}
.gx-daily-foot button:hover {
  border-color: color-mix(in srgb, var(--gx-jade) 45%, var(--gx-rule));
  color: var(--gx-jade);
}

.gx-recent {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.gx-sec-title {
  margin: 0;
  font-size: 11px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--gx-muted);
  font-weight: 600;
}

.gx-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.gx-chip {
  padding: 5px 11px;
  border-radius: 999px;
  border: 1px solid var(--gx-rule);
  background: transparent;
  color: var(--gx-ink);
  font-size: 12.5px;
  cursor: pointer;
  max-width: 100%;
  overflow-wrap: anywhere;
  text-align: left;
}
.gx-chip:hover {
  border-color: color-mix(in srgb, var(--gx-jade) 50%, var(--gx-rule));
  background: var(--gx-jade-soft);
}
.gx-chip-sub {
  color: var(--gx-muted);
  font-size: 11px;
}

.gx-status {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 94%, var(--gx-jade) 3%);
}
.gx-status-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.gx-status-num {
  font-family: var(--gx-serif);
  font-size: 20px;
  color: var(--gx-ink);
}
.gx-status-label {
  font-size: 10.5px;
  color: var(--gx-muted);
}

.gx-tip {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.85;
  color: var(--gx-muted);
}

/* ── 词条 ── */
.gx-word {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.gx-word-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--gx-rule);
}
.gx-word-title-wrap {
  min-width: 0;
}
.gx-word-title {
  margin: 0;
  font-family: var(--gx-serif);
  font-size: 40px;
  line-height: 1.15;
  font-weight: 600;
  letter-spacing: 0.01em;
  overflow-wrap: anywhere;
}

.gx-phons {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 8px;
  flex-wrap: wrap;
}
.gx-phon {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 13px;
  color: var(--gx-muted);
}
.gx-phon em {
  font-style: normal;
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--gx-amber) 14%, transparent);
  color: var(--gx-amber);
}
.gx-phon-text {
  color: var(--gx-ink);
}
.gx-phon-play {
  border: 0;
  background: transparent;
  color: var(--gx-muted);
  cursor: pointer;
  line-height: 0;
}
.gx-phon-play:hover {
  color: var(--gx-jade);
}

.gx-word-tools {
  display: flex;
  gap: 6px;
  flex: 0 0 auto;
}

.gx-tool {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 7px 12px;
  font-size: 12px;
  border-radius: 8px;
  border: 1px solid var(--gx-rule);
  background: transparent;
  color: var(--gx-ink);
  cursor: pointer;
  white-space: nowrap;
}
.gx-tool:hover {
  border-color: color-mix(in srgb, var(--gx-jade) 45%, var(--gx-rule));
  background: var(--gx-jade-soft);
}
.gx-tool.is-done {
  color: var(--gx-jade);
  border-color: color-mix(in srgb, var(--gx-jade) 45%, var(--gx-rule));
  background: var(--gx-jade-soft);
}
.gx-tool.is-quiet {
  padding: 4px 6px;
  border-color: transparent;
  color: var(--gx-muted);
}

.gx-word-lead {
  margin: 0;
  font-size: 15.5px;
  line-height: 1.85;
  color: var(--gx-ink);
}

.gx-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.gx-tag {
  font-size: 10.5px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--gx-jade) 35%, var(--gx-rule));
  color: var(--gx-jade);
  max-width: 100%;
  overflow-wrap: anywhere;
}
.gx-tag.is-pos {
  color: var(--gx-amber);
  border-color: color-mix(in srgb, var(--gx-amber) 40%, var(--gx-rule));
}
.gx-tag.is-quiet {
  color: var(--gx-muted);
  border-color: var(--gx-rule);
}

.gx-pack {
  border: 1px solid var(--gx-rule);
  border-radius: 12px;
  overflow: hidden;
  background: color-mix(in srgb, var(--gx-panel) 96%, var(--gx-jade) 2%);
}
.gx-pack-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 90%, var(--gx-jade) 6%);
}
.gx-pack-name {
  font-family: var(--gx-serif);
  font-size: 13px;
  font-weight: 600;
}
.gx-pack-meta {
  font-size: 10px;
  color: var(--gx-muted);
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px solid var(--gx-rule);
}
.gx-pack-toggle {
  margin-left: auto;
  border: 0;
  background: transparent;
  color: var(--gx-muted);
  font-size: 11px;
  cursor: pointer;
}
.gx-pack-toggle:hover {
  color: var(--gx-jade);
}

.gx-definition {
  padding: 12px 14px;
  font-size: 14px;
  line-height: 1.8;
  max-height: 420px;
  overflow: auto;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.gx-definition :deep(img) {
  max-width: 100%;
  height: auto;
}
.gx-definition-block + .gx-definition-block {
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px dashed var(--gx-rule);
}
.gx-definition :deep(table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.gx-definition :deep(td),
.gx-definition :deep(th) {
  border: 1px solid var(--gx-rule);
  padding: 4px 6px;
  vertical-align: top;
}
.gx-definition :deep(a) {
  color: var(--gx-jade);
}
.gx-definition :deep(div),
.gx-definition :deep(p) {
  margin: 0 0 4px;
}
.gx-definition :deep(.dsl-trn) {
  color: var(--gx-ink);
}
.gx-definition :deep(.dsl-ex) {
  color: var(--gx-muted);
  font-style: italic;
}
.gx-definition :deep(.dsl-pos) {
  color: var(--gx-amber);
  font-size: 12px;
}
.gx-definition :deep(.gx-def-empty) {
  color: var(--gx-muted);
}

.gx-online {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.gx-sec {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.gx-meanings {
  margin: 0;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 5px;
  font-size: 14px;
  line-height: 1.8;
}
.gx-meanings li::marker {
  color: var(--gx-jade);
  font-family: var(--guaxi-editor-font-family, monospace);
  font-size: 11px;
}

.gx-pairs {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.gx-pairs li {
  padding: 8px 11px;
  border-radius: 9px;
  border: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 94%, var(--gx-jade) 3%);
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.gx-pair-src {
  font-family: var(--gx-serif);
  font-size: 13.5px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.gx-pair-dst {
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--gx-muted);
  overflow-wrap: anywhere;
}

.gx-empty-def {
  margin: 0;
  padding: 18px 16px;
  border: 1px dashed var(--gx-rule);
  border-radius: 12px;
  font-size: 13px;
  line-height: 1.8;
  color: var(--gx-muted);
}

.gx-source-note {
  margin: 0;
  padding-top: 10px;
  border-top: 1px dashed var(--gx-rule);
  font-size: 11px;
  line-height: 1.8;
  color: var(--gx-muted);
}

/* ── 翻译 ── */
.gx-translate {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.gx-src {
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 92%, var(--gx-amber) 4%);
}
.gx-src-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}
.gx-src-text {
  margin: 0;
  font-size: 15px;
  line-height: 1.85;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.gx-engines {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.gx-engine {
  border: 1px solid var(--gx-rule);
  border-radius: 12px;
  overflow: hidden;
}
.gx-engine header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  border-bottom: 1px solid var(--gx-rule);
  background: color-mix(in srgb, var(--gx-panel) 90%, var(--gx-jade) 5%);
}
.gx-engine-name {
  font-size: 11px;
  letter-spacing: 0.08em;
  color: var(--gx-muted);
  text-transform: uppercase;
}
.gx-engine-text {
  margin: 0;
  padding: 12px 14px;
  font-size: 15px;
  line-height: 1.9;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.gx-engine-text.is-error {
  color: #c2534b;
}

.gx-recorded {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  font-size: 11.5px;
  color: var(--gx-jade);
}
.gx-error {
  margin: 0;
  font-size: 12.5px;
  color: #c2534b;
}

.gx-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 40px 0;
  font-size: 12px;
  color: var(--gx-muted);
  justify-content: center;
}

/* ── 列表 ── */
.gx-list-view {
  max-width: 820px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.gx-list-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--gx-rule);
}
.gx-list-head h2 {
  margin: 4px 0 0;
  font-family: var(--gx-serif);
  font-size: 24px;
  font-weight: 600;
}
.gx-list-sub {
  margin: 4px 0 0;
  font-size: 11.5px;
  color: var(--gx-muted);
}
.gx-list-tools {
  display: flex;
  align-items: center;
  gap: 8px;
}
.gx-list-tools input {
  height: 30px;
  padding: 0 10px;
  width: 150px;
  border-radius: 8px;
  border: 1px solid var(--gx-rule);
  background: var(--gx-paper);
  color: var(--gx-ink);
  font-size: 12px;
  outline: none;
}

.gx-empty-list {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 52px 0;
  color: var(--gx-muted);
  font-size: 12px;
}
.gx-empty-list p {
  margin: 0;
}

.gx-records {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.gx-records li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border: 1px solid var(--gx-rule);
  border-radius: 10px;
  background: color-mix(in srgb, var(--gx-panel) 95%, var(--gx-jade) 3%);
}
.gx-record-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
  color: inherit;
  padding: 0;
}
.gx-record-word {
  font-family: var(--gx-serif);
  font-size: 15px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gx-record-text {
  font-size: 12px;
  color: var(--gx-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gx-record-main:hover .gx-record-word {
  color: var(--gx-jade);
}
.gx-record-side {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--gx-muted);
}
.gx-record-engine {
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px solid var(--gx-rule);
  max-width: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gx-record-del {
  border: 0;
  background: transparent;
  color: var(--gx-muted);
  cursor: pointer;
  line-height: 0;
}
.gx-record-del:hover {
  color: #c2534b;
}

@media (max-width: 860px) {
  .gx-rail {
    width: 64px;
    flex-basis: 64px;
    padding: 14px 8px;
  }
  .gx-brand-text,
  .gx-nav-item span,
  .gx-lib-btn span,
  .gx-lib-stat {
    display: none;
  }
  .gx-nav-item {
    justify-content: center;
  }
  .gx-searchbar {
    flex-wrap: wrap;
    padding: 12px 14px;
  }
  .gx-langpair {
    width: 100%;
    justify-content: flex-end;
  }
  .gx-scroll {
    padding: 16px 14px 26px;
  }
}
</style>
