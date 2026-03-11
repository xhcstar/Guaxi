<template>
  <n-message-provider>
    <div class="lang-learn-page">
      <div class="manager-header">
        <div class="title-main">学习记录管理器</div>
        <div class="toolbar-row">
          <n-button size="small" @click="message.info('在线同步功能即将提供')">在线同步</n-button>
          <n-button size="small" @click="message.info('导入功能即将提供')">导入</n-button>
          <n-button size="small" @click="exportLearningData">导出</n-button>
          <n-button size="small" @click="message.info('备份恢复功能即将提供')">备份恢复</n-button>
          <n-button size="small" @click="playSelectedPronunciation">单词播放</n-button>
          <n-select
            v-model:value="learningLang"
            size="small"
            :options="learningLangOptions"
            style="width: 140px"
          />
          <n-select
            v-model:value="recordKind"
            size="small"
            :options="recordKindOptions"
            style="width: 120px"
          />
          <n-input
            v-model:value="keyword"
            clearable
            size="small"
            placeholder="搜索单词、释义、例句、笔记..."
            style="width: 260px"
            @keydown.enter="refreshAll"
          />
          <n-button size="small" type="primary" @click="refreshAll">刷新</n-button>
        </div>
      </div>

      <n-tabs v-model:value="activeTab" animated type="line" class="page-tabs">
        <n-tab-pane name="manager" tab="学习管理">
          <div class="manager-layout">
            <n-card size="small" class="left-tree-card">
              <div class="tree-group-title">学习分组</div>
              <div
                v-for="it in leftShelfItems"
                :key="it.key"
                class="tree-item"
                :class="{ active: selectedShelf === it.key }"
                @click="selectedShelf = it.key"
              >
                <span>{{ it.label }}</span>
                <span class="tree-count">({{ it.count }})</span>
              </div>
            </n-card>

            <n-card size="small" class="middle-table-card">
              <div class="table-toolbar">
                <n-button size="small" @click="refreshAll">添加</n-button>
                <n-button size="small" :disabled="!activeAudio" @click="editAudio(activeAudio)">编辑</n-button>
                <n-button size="small" type="error" :disabled="!activeAudio" @click="activeAudio && removeAudio(activeAudio)">删除</n-button>
                <n-button size="small" type="primary" @click="startReviewFromSelection">开始复习</n-button>
              </div>
              <n-data-table
                size="small"
                :columns="managerColumns"
                :data="filteredRows"
                :max-height="610"
                :row-key="(row) => row.id"
                :row-props="managerRowProps"
                :single-line="false"
              />
            </n-card>

            <n-card size="small" class="right-detail-card" title="词条详情 / 学习信息">
              <template v-if="activeWordRecord">
                <div class="word-detail-head">
                  <div class="word-detail-title">{{ activeWordRecord.word || activeWordRecord.source_text }}</div>
                  <div class="word-detail-meta">
                    <div v-if="activeWordPhoneticUk" class="phonetic-row">
                      <n-tag size="small" round>
                        <template #icon>
                          <Icon :icon="speechIcon" width="13" height="13" />
                        </template>
                        英 {{ activeWordPhoneticUk }}
                      </n-tag>
                      <n-button text size="tiny" title="英式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'uk')">
                        <Icon :icon="volumeUkIcon" width="16" height="16" />
                      </n-button>
                    </div>
                    <div v-if="activeWordPhoneticUs" class="phonetic-row">
                      <n-tag size="small" round>
                        <template #icon>
                          <Icon :icon="speechIcon" width="13" height="13" />
                        </template>
                        美 {{ activeWordPhoneticUs }}
                      </n-tag>
                      <n-button text size="tiny" title="美式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'us')">
                        <Icon :icon="volumeUsIcon" width="16" height="16" />
                      </n-button>
                    </div>
                    <n-tag v-if="activeWordRecord.part_of_speech" size="small" round type="info">
                      <template #icon>
                        <Icon :icon="badgeInfoIcon" width="13" height="13" />
                      </template>
                      词性: {{ activeWordRecord.part_of_speech }}
                    </n-tag>
                    <span>掌握度 {{ masteryStars(getMasteryLevel('record', activeWordRecord.id)) }}</span>
                  </div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">译文</div>
                  <div class="detail-content">{{ activeWordCombinedTranslation || '—' }}</div>
                </div>
                <div class="detail-block" v-if="activeWordPhrasePairs.length">
                  <div class="detail-label">常用短语</div>
                  <div class="example-line" v-for="(x, i) in activeWordPhrasePairs" :key="`wd-p-${i}`">
                    <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                    <div class="example-target" v-if="x.target">{{ x.target }}</div>
                  </div>
                </div>
                <div class="detail-block" v-if="activeWordSynonyms.length || activeWordAntonyms.length || activeWordAssociates.length">
                  <div class="detail-label">近义 / 反义 / 联想词</div>
                  <div class="detail-label tiny">近义词</div>
                  <div class="chip-wrap" v-if="activeWordSynonyms.length">
                    <n-tag v-for="(x, i) in activeWordSynonymPairs" :key="`wd-s-${i}`" size="small">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                  <div class="detail-label tiny" v-if="activeWordAntonyms.length">反义词</div>
                  <div class="chip-wrap" v-if="activeWordAntonyms.length">
                    <n-tag v-for="(x, i) in activeWordAntonymPairs" :key="`wd-a-${i}`" size="small" type="warning">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                  <div class="detail-label tiny" v-if="activeWordAssociates.length">联想词</div>
                  <div class="chip-wrap" v-if="activeWordAssociates.length">
                    <n-tag v-for="(x, i) in activeWordAssociatePairs" :key="`wd-r-${i}`" size="small" type="info">{{ x.word }}<span v-if="x.translation"> · {{ x.translation }}</span></n-tag>
                  </div>
                </div>
                <div class="detail-block" v-if="activeWordExamplePairs.length">
                  <div class="detail-label">常用例句库</div>
                  <div class="example-line" v-for="(x, i) in activeWordExamplePairs" :key="`wd-e-${i}`">
                    <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                    <div class="example-target" v-if="x.target">{{ x.target }}</div>
                  </div>
                </div>
                <div class="detail-actions">
                  <n-button size="small" type="primary" @click="refreshWordMeta(activeWordRecord)">刷新词汇拓展</n-button>
                  <n-button size="small" @click="applyReviewResult('known')">我已掌握</n-button>
                  <n-button size="small" @click="applyReviewResult('fuzzy')">还需复习</n-button>
                </div>
              </template>

              <template v-else-if="activeAudio">
                <div class="detail-block">
                  <div class="detail-label">听力标题</div>
                  <div class="detail-content">{{ activeAudio.title }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">资源地址</div>
                  <div class="detail-content">{{ activeAudio.source_uri }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">转写</div>
                  <div class="detail-content">{{ activeAudio.transcript || '—' }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">备注</div>
                  <div class="detail-content">{{ activeAudio.notes || '—' }}</div>
                </div>
                <div class="detail-actions">
                  <n-button size="small" @click="editAudio(activeAudio)">编辑资源</n-button>
                  <n-button size="small" @click="applyReviewResult('known')">听懂了</n-button>
                  <n-button size="small" @click="applyReviewResult('unknown')">听不懂</n-button>
                </div>
              </template>

              <template v-else-if="activeSentenceRecord">
                <div class="detail-block">
                  <div class="detail-label">原文</div>
                  <div class="detail-content">{{ activeSentenceRecord.source_text }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">译文</div>
                  <div class="detail-content">{{ activeSentenceRecord.translated_text }}</div>
                </div>
                <div class="detail-actions">
                  <n-button size="small" @click="applyReviewResult('known')">掌握</n-button>
                  <n-button size="small" @click="applyReviewResult('fuzzy')">模糊</n-button>
                  <n-button size="small" @click="applyReviewResult('unknown')">不会</n-button>
                </div>
              </template>

              <n-empty v-else description="请从中间列表选择学习项" />
            </n-card>
          </div>
        </n-tab-pane>

        <n-tab-pane name="review" tab="复习训练">
          <div class="review-layout">
            <n-card size="small" class="review-stat-card">
              <div class="review-stat-grid">
                <div class="stat-cell">
                  <div class="stat-label">总学习项</div>
                  <div class="stat-value">{{ totalLearningItemCount }}</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">已复习次数</div>
                  <div class="stat-value">{{ totalReviewedCount }}</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">掌握占比</div>
                  <div class="stat-value">{{ masteryPercent }}%</div>
                </div>
                <div class="stat-cell">
                  <div class="stat-label">当前卡组</div>
                  <div class="stat-value">{{ reviewRows.length }}</div>
                </div>
              </div>
              <div class="mastery-bar-track">
                <div class="mastery-bar-fill" :style="{ width: `${masteryPercent}%` }"></div>
              </div>
            </n-card>

            <n-card size="small" class="review-card-panel">
              <div class="review-topbar">
                <n-radio-group v-model:value="reviewType" size="small">
                  <n-radio-button value="mixed">混合</n-radio-button>
                  <n-radio-button value="word">单词</n-radio-button>
                  <n-radio-button value="sentence">例句</n-radio-button>
                  <n-radio-button value="paragraph">段落</n-radio-button>
                  <n-radio-button value="listening">听力</n-radio-button>
                </n-radio-group>
                <n-button size="small" @click="shuffleReviewRows">洗牌</n-button>
              </div>

              <div v-if="currentReviewItem" class="review-card">
                <div class="review-card-type">
                  <n-tag size="small" type="info">{{ rowTypeLabel(currentReviewItem.rowType) }}</n-tag>
                  <span>{{ reviewIndex + 1 }} / {{ reviewRows.length }}</span>
                </div>
                <div class="review-card-front">
                  <template v-if="currentReviewItem.rowType === 'word'">
                    <div class="review-word">{{ currentReviewItem.title }}</div>
                    <div class="review-phonetic">{{ currentReviewItem.phonetic || '—' }}</div>
                  </template>
                  <template v-else>
                    <div class="review-text">{{ currentReviewItem.sourceText }}</div>
                  </template>
                </div>
                <div class="review-card-answer" v-if="reviewShowAnswer">
                  <div class="review-answer-title">答案</div>
                  <template v-if="currentReviewItem.rowType === 'word'">
                    <div class="review-answer-meta">
                      <div class="review-answer-chip">音标：{{ currentReviewItem.phonetic || '—' }}</div>
                      <div class="review-answer-chip">词性：{{ currentReviewItem.partOfSpeech || '—' }}</div>
                    </div>
                    <div class="review-answer-text">{{ currentReviewItem.combinedTranslation || currentReviewItem.translatedText || '—' }}</div>
                    <div v-if="currentReviewItem.examplePairs?.length" class="review-examples">
                      <div class="review-answer-title">例句</div>
                      <div class="review-example-item" v-for="(x, i) in currentReviewItem.examplePairs" :key="`rv-ex-${i}`">
                        <div class="review-answer-text">{{ i + 1 }}. {{ x.source }}</div>
                        <div class="review-answer-text review-answer-target" v-if="x.target">{{ x.target }}</div>
                      </div>
                    </div>
                  </template>
                  <template v-else>
                    <div class="review-answer-text">{{ currentReviewItem.summary || currentReviewItem.translatedText || '—' }}</div>
                    <div v-if="currentReviewItem.examples?.length" class="review-examples">
                      <div class="review-answer-title">例句</div>
                      <div class="review-answer-text" v-for="(x, i) in currentReviewItem.examples" :key="`rv-ex-${i}`">{{ x }}</div>
                    </div>
                  </template>
                </div>
                <div class="review-actions">
                  <n-button size="small" @click="reviewShowAnswer = !reviewShowAnswer">{{ reviewShowAnswer ? '隐藏答案' : '显示答案' }}</n-button>
                  <n-button size="small" type="success" @click="gradeAndNext('known')">认识</n-button>
                  <n-button size="small" type="warning" @click="gradeAndNext('fuzzy')">模糊</n-button>
                  <n-button size="small" type="error" @click="gradeAndNext('unknown')">不认识</n-button>
                  <n-button size="small" @click="gotoPrevReview">上一个</n-button>
                  <n-button size="small" @click="gotoNextReview">下一个</n-button>
                </div>
              </div>
              <n-empty v-else description="当前筛选下暂无可复习内容" />
            </n-card>

            <n-card size="small" class="audio-form-card" title="听力资源维护">
              <n-form :model="audioForm" label-placement="left" label-width="72">
                <n-form-item label="标题">
                  <n-input v-model:value="audioForm.title" placeholder="例如：BBC-News-01" />
                </n-form-item>
                <n-form-item label="类型">
                  <n-select v-model:value="audioForm.source_kind" :options="audioKindOptions" />
                </n-form-item>
                <n-form-item label="地址">
                  <n-input v-model:value="audioForm.source_uri" placeholder="本地路径或在线 URL" />
                </n-form-item>
                <n-form-item label="时长">
                  <n-input-number v-model:value="audioForm.duration_sec" :min="0" />
                </n-form-item>
                <n-form-item label="转写">
                  <n-input v-model:value="audioForm.transcript" type="textarea" :autosize="{ minRows: 2, maxRows: 5 }" />
                </n-form-item>
                <n-form-item label="备注">
                  <n-input v-model:value="audioForm.notes" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
                </n-form-item>
              </n-form>
              <n-space>
                <n-button type="primary" @click="saveAudio">保存资源</n-button>
                <n-button @click="resetAudioForm">清空</n-button>
              </n-space>
            </n-card>
          </div>
        </n-tab-pane>
      </n-tabs>
    </div>
  </n-message-provider>
</template>

<script setup>
import { computed, h, onMounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import speechIcon from '@iconify-icons/lucide/messages-square'
import badgeInfoIcon from '@iconify-icons/lucide/badge-info'
import volumeUsIcon from '@iconify-icons/lucide/volume-2'
import volumeUkIcon from '@iconify-icons/lucide/volume-1'
import {
  NButton,
  NCard,
  NDataTable,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NMessageProvider,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpace,
  NTabPane,
  NTabs,
  NTag,
  useMessage,
} from 'naive-ui'

const message = useMessage()
const activeTab = ref('manager')
const keyword = ref('')
const recordKind = ref('all')
const learningLangStorageKey = 'guaxi_language_learning_target_lang'
const learningLang = ref(window.localStorage.getItem(learningLangStorageKey) || 'zh')
const selectedShelf = ref('my_vocab')
const records = ref([])
const audioResources = ref([])
const activeRow = ref(null)
const reviewType = ref('mixed')
const reviewRows = ref([])
const reviewIndex = ref(0)
const reviewShowAnswer = ref(false)
const masteryStore = ref({ record: {}, audio: {} })

const recordKindOptions = [
  { label: '全部', value: 'all' },
  { label: '单词', value: 'word' },
  { label: '例句', value: 'sentence' },
  { label: '段落', value: 'paragraph' },
  { label: '听力', value: 'listening' },
]

const learningLangOptions = [
  { label: '学习中文', value: 'zh' },
  { label: '学习英语', value: 'en' },
  { label: '学习日语', value: 'ja' },
  { label: '学习韩语', value: 'ko' },
  { label: '学习法语', value: 'fr' },
  { label: '学习德语', value: 'de' },
  { label: '学习西语', value: 'es' },
  { label: '学习俄语', value: 'ru' },
]

const audioKindOptions = [
  { label: '在线音频', value: 'remote' },
  { label: '本地文件', value: 'local' },
  { label: '听写素材', value: 'dictation' },
]

const audioForm = reactive({
  id: '',
  title: '',
  source_kind: 'remote',
  source_uri: '',
  duration_sec: 0,
  transcript: '',
  notes: '',
})

const masteryKey = 'guaxi_language_learning_mastery_v1'

const toNum = (v) => {
  const n = Number(v)
  return Number.isFinite(n) ? n : 0
}

const loadMasteryStore = () => {
  try {
    const raw = window.localStorage.getItem(masteryKey) || ''
    if (!raw) return
    const parsed = JSON.parse(raw)
    masteryStore.value = {
      record: parsed?.record && typeof parsed.record === 'object' ? parsed.record : {},
      audio: parsed?.audio && typeof parsed.audio === 'object' ? parsed.audio : {},
    }
  } catch {
    masteryStore.value = { record: {}, audio: {} }
  }
}

const saveMasteryStore = () => {
  try {
    window.localStorage.setItem(masteryKey, JSON.stringify(masteryStore.value))
  } catch {}
}

const getMasteryEntry = (sourceType, rawId) => {
  const box = masteryStore.value[sourceType] || {}
  const key = String(rawId || '')
  const row = box[key]
  if (!row || typeof row !== 'object') {
    return { level: 0, reviewed: 0, mastered: 0, lastResult: '' }
  }
  return {
    level: toNum(row.level),
    reviewed: toNum(row.reviewed),
    mastered: toNum(row.mastered),
    lastResult: String(row.lastResult || ''),
  }
}

const upsertMasteryEntry = (sourceType, rawId, result) => {
  const key = String(rawId || '')
  if (!key) return
  const current = getMasteryEntry(sourceType, key)
  let level = current.level
  if (result === 'known') level += 1
  if (result === 'fuzzy') level += 0
  if (result === 'unknown') level -= 1
  level = Math.min(5, Math.max(0, level))

  const next = {
    level,
    reviewed: current.reviewed + 1,
    mastered: current.mastered + (result === 'known' ? 1 : 0),
    lastResult: result,
  }
  masteryStore.value = {
    ...masteryStore.value,
    [sourceType]: {
      ...(masteryStore.value[sourceType] || {}),
      [key]: next,
    },
  }
  saveMasteryStore()
}

const masteryStars = (level) => {
  const n = Math.min(5, Math.max(0, toNum(level)))
  return `${'★'.repeat(n)}${'☆'.repeat(5 - n)}`
}

const rowTypeLabel = (t) => {
  if (t === 'word') return '单词'
  if (t === 'paragraph') return '段落'
  if (t === 'listening') return '听力'
  return '例句'
}

const normalizeText = (v) => String(v || '').trim().toLowerCase()
const splitSimpleTranslations = (text) => {
  const src = String(text || '').trim()
  if (!src) return []
  const parts = src.split(/[;；,，、/|｜]/g).map((x) => String(x || '').trim()).filter(Boolean)
  const out = []
  parts.forEach((p) => {
    if (p.length > 16) return
    if (!out.some((x) => x === p)) out.push(p)
  })
  return out.slice(0, 5)
}

const allRows = computed(() => {
  const recRows = (Array.isArray(records.value) ? records.value : []).map((r) => {
    const m = getMasteryEntry('record', r.id)
    return {
      id: `record:${r.id}`,
      rawId: r.id,
      sourceType: 'record',
      rowType: String(r.record_type || 'sentence'),
      title: r.is_word ? String(r.word || r.source_text || '') : String(r.source_text || '').slice(0, 36),
      sourceText: String(r.source_text || ''),
      translatedText: String(r.translated_text || ''),
      summary: String(r.translated_text || ''),
      phonetic: String(r.phonetic || ''),
      phoneticUk: String(r.phonetic_uk || ''),
      phoneticUs: String(r.phonetic_us || ''),
      partOfSpeech: String(r.part_of_speech || ''),
      combinedTranslation: normalizeList(Array.isArray(r.meanings) && r.meanings.length ? r.meanings : splitSimpleTranslations(r.translated_text)).join('；'),
      note: String((r.examples && r.examples[0]) || ''),
      createdAt: String(r.created_at || ''),
      examples: Array.isArray(r.examples) ? r.examples : [],
      examplePairs: (() => {
        const sources = Array.isArray(r.examples) ? r.examples : []
        const targets = Array.isArray(r.example_translations) ? r.example_translations : []
        const list = sources.map((source, idx) => ({
          source: String(source || '').trim(),
          target: String(targets[idx] || '').trim(),
        })).filter((x) => x.source)
        if (list.length) return list
        const fallback = String(((Array.isArray(r.phrases) ? r.phrases : [])[0]) || '').trim() || `This is a ${String(r.word || r.source_text || '').trim()}.`
        const fallbackTarget = String(((Array.isArray(r.phrase_translations) ? r.phrase_translations : [])[0]) || '').trim()
          || normalizeList(Array.isArray(r.meanings) ? r.meanings : splitSimpleTranslations(r.translated_text)).slice(0, 1).join('')
        return [{ source: fallback, target: fallbackTarget }]
      })(),
      phrasePairs: (Array.isArray(r.phrases) ? r.phrases : []).map((source, idx) => ({
        source: String(source || '').trim(),
        target: String((Array.isArray(r.phrase_translations) ? r.phrase_translations : [])[idx] || '').trim(),
      })),
      payload: r,
      masteryLevel: m.level,
      reviewed: m.reviewed,
      mastered: m.mastered,
    }
  })
  const audioRows = (Array.isArray(audioResources.value) ? audioResources.value : []).map((r) => {
    const m = getMasteryEntry('audio', r.id)
    return {
      id: `audio:${r.id}`,
      rawId: r.id,
      sourceType: 'audio',
      rowType: 'listening',
      title: String(r.title || ''),
      sourceText: String(r.transcript || r.notes || r.source_uri || ''),
      translatedText: String(r.notes || ''),
      summary: String(r.transcript || r.notes || ''),
      phonetic: '',
      note: String(r.source_uri || ''),
      createdAt: String(r.created_at || ''),
      examples: [],
      payload: r,
      masteryLevel: m.level,
      reviewed: m.reviewed,
      mastered: m.mastered,
    }
  })
  return [...recRows, ...audioRows]
})

const matchesKeyword = (row) => {
  const kw = normalizeText(keyword.value)
  if (!kw) return true
  return [row.title, row.sourceText, row.summary, row.note].some((x) => normalizeText(x).includes(kw))
}

const shelfFilteredRows = computed(() => {
  const rows = allRows.value
  if (selectedShelf.value === 'my_vocab') return rows.filter((r) => r.rowType === 'word')
  if (selectedShelf.value === 'paused') return rows.filter((r) => r.masteryLevel <= 1)
  if (selectedShelf.value === 'history') return rows.filter((r) => r.sourceType === 'record')
  if (selectedShelf.value === 'notes') return rows.filter((r) => normalizeText(r.note).length > 0)
  if (selectedShelf.value === 'cards') return rows.filter((r) => r.sourceType === 'record')
  if (selectedShelf.value === 'audio') return rows.filter((r) => r.rowType === 'listening')
  return rows
})

const filteredRows = computed(() => {
  let rows = shelfFilteredRows.value.filter(matchesKeyword)
  if (recordKind.value !== 'all') {
    rows = rows.filter((r) => r.rowType === recordKind.value)
  }
  return rows
})

const leftShelfItems = computed(() => {
  const rows = allRows.value
  const count = (fn) => rows.filter(fn).length
  return [
    { key: 'my_vocab', label: '我的生词本', count: count((r) => r.rowType === 'word') },
    { key: 'paused', label: '暂停', count: count((r) => r.masteryLevel <= 1) },
    { key: 'history', label: '历史记录', count: count((r) => r.sourceType === 'record') },
    { key: 'notes', label: '我的笔记', count: count((r) => normalizeText(r.note).length > 0) },
    { key: 'cards', label: '例卡片', count: count((r) => r.sourceType === 'record') },
    { key: 'audio', label: '已掌握听力', count: count((r) => r.rowType === 'listening' && r.masteryLevel >= 4) },
  ]
})

const managerColumns = [
  {
    title: '类型',
    key: 'rowType',
    width: 86,
    render: (row) => h(NTag, { size: 'small', type: row.rowType === 'word' ? 'success' : row.rowType === 'listening' ? 'warning' : 'info' }, { default: () => rowTypeLabel(row.rowType) }),
  },
  { title: '词目/标题', key: 'title', width: 180, ellipsis: { tooltip: true } },
  { title: '音标', key: 'phonetic', width: 120, ellipsis: { tooltip: true } },
  {
    title: '等级',
    key: 'masteryLevel',
    width: 96,
    render: (row) => h('span', { class: 'mastery-stars' }, masteryStars(row.masteryLevel)),
  },
  { title: '简明解释', key: 'summary', ellipsis: { tooltip: true } },
  { title: '我的笔记', key: 'note', width: 220, ellipsis: { tooltip: true } },
  { title: '添加时间', key: 'createdAt', width: 188, ellipsis: { tooltip: true } },
]

const managerRowProps = (row) => ({
  onClick: () => {
    activeRow.value = row
  },
})

const activeWordRecord = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'record' || r.rowType !== 'word') return null
  return r.payload
})

const activeSentenceRecord = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'record' || r.rowType === 'word') return null
  return r.payload
})

const activeAudio = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'audio') return null
  return r.payload
})

const normalizeList = (arr) => {
  const list = Array.isArray(arr) ? arr : []
  const out = []
  list.forEach((x) => {
    const t = String(x || '').trim()
    if (!t) return
    if (!out.some((y) => y.toLowerCase() === t.toLowerCase())) out.push(t)
  })
  return out
}

const activeWordCombinedTranslation = computed(() => {
  const row = activeWordRecord.value
  if (!row) return ''
  const meanings = Array.isArray(row.meanings) && row.meanings.length ? row.meanings : splitSimpleTranslations(row.translated_text)
  return normalizeList(meanings).slice(0, 5).join('；')
})

const activeWordPhoneticUk = computed(() => String(activeWordRecord.value?.phonetic_uk || activeWordRecord.value?.phonetic || '').trim())
const activeWordPhoneticUs = computed(() => String(activeWordRecord.value?.phonetic_us || activeWordRecord.value?.phonetic || '').trim())

const activeWordSynonyms = computed(() => normalizeList(activeWordRecord.value?.synonyms))
const activeWordAntonyms = computed(() => normalizeList(activeWordRecord.value?.antonyms))
const activeWordAssociates = computed(() => normalizeList(activeWordRecord.value?.associates))
const activeWordSynonymPairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.synonyms_translations) ? row.synonyms_translations : []
  return activeWordSynonyms.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordAntonymPairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.antonyms_translations) ? row.antonyms_translations : []
  return activeWordAntonyms.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordAssociatePairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.associates_translations) ? row.associates_translations : []
  return activeWordAssociates.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordPhrasePairs = computed(() => {
  const row = activeWordRecord.value
  if (!row) return []
  const sources = normalizeList(row.phrases)
  const targets = Array.isArray(row.phrase_translations) ? row.phrase_translations.map((x) => String(x || '').trim()) : []
  return sources.slice(0, 12).map((source, idx) => ({ source, target: String(targets[idx] || '').trim() }))
})

const activeWordExamplePairs = computed(() => {
  const row = activeWordRecord.value
  if (!row) return []
  const examples = normalizeList(row.examples)
  const targets = Array.isArray(row.example_translations)
    ? row.example_translations.map((x) => String(x || '').trim())
    : []
  const pairs = examples.map((source, idx) => ({
    source,
    target: String(targets[idx] || '').trim(),
  }))
  if (pairs.length) return pairs
  const fallbackSource = String(activeWordPhrasePairs.value[0]?.source || '').trim()
    || `This is a ${String(row.word || row.source_text || '').trim()}.`
  const fallbackTarget = String(activeWordPhrasePairs.value[0]?.target || '').trim() || activeWordCombinedTranslation.value
  return [{ source: fallbackSource, target: String(fallbackTarget || '').trim() }]
})

const getMasteryLevel = (sourceType, rawId) => getMasteryEntry(sourceType, rawId).level

const applyReviewResult = (result) => {
  if (!activeRow.value) return
  upsertMasteryEntry(activeRow.value.sourceType, activeRow.value.rawId, result)
  if (activeRow.value) {
    activeRow.value = allRows.value.find((r) => r.id === activeRow.value.id) || activeRow.value
  }
}

const reviewRowsByType = computed(() => {
  const rows = filteredRows.value
  if (reviewType.value === 'mixed') return rows
  return rows.filter((r) => r.rowType === reviewType.value)
})

const currentReviewItem = computed(() => {
  if (!reviewRows.value.length) return null
  const idx = Math.max(0, Math.min(reviewRows.value.length - 1, reviewIndex.value))
  return reviewRows.value[idx] || null
})

const rebuildReviewRows = () => {
  reviewRows.value = [...reviewRowsByType.value]
  reviewIndex.value = 0
  reviewShowAnswer.value = false
}

const shuffleReviewRows = () => {
  const next = [...reviewRowsByType.value]
  for (let i = next.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1))
    const t = next[i]
    next[i] = next[j]
    next[j] = t
  }
  reviewRows.value = next
  reviewIndex.value = 0
  reviewShowAnswer.value = false
}

const gotoNextReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value + 1) % reviewRows.value.length
  reviewShowAnswer.value = false
}

const gotoPrevReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value - 1 + reviewRows.value.length) % reviewRows.value.length
  reviewShowAnswer.value = false
}

const gradeAndNext = (result) => {
  const cur = currentReviewItem.value
  if (!cur) return
  upsertMasteryEntry(cur.sourceType, cur.rawId, result)
  gotoNextReview()
}

const startReviewFromSelection = () => {
  activeTab.value = 'review'
  rebuildReviewRows()
}

const totalLearningItemCount = computed(() => allRows.value.length)

const masteryAggregate = computed(() => {
  let reviewed = 0
  let mastered = 0
  allRows.value.forEach((r) => {
    const m = getMasteryEntry(r.sourceType, r.rawId)
    reviewed += m.reviewed
    mastered += m.mastered
  })
  return { reviewed, mastered }
})

const totalReviewedCount = computed(() => masteryAggregate.value.reviewed)
const masteryPercent = computed(() => {
  const reviewed = masteryAggregate.value.reviewed
  const mastered = masteryAggregate.value.mastered
  if (reviewed <= 0) return 0
  return Math.min(100, Math.max(0, Math.round((mastered / reviewed) * 100)))
})

const refreshRecords = async () => {
  try {
    const kind = recordKind.value === 'listening' ? 'all' : recordKind.value
    const data = await invoke('language_learning_list_records', {
      keyword: keyword.value,
      kind,
      learningLang: learningLang.value,
      limit: 1200,
      offset: 0,
    })
    records.value = Array.isArray(data) ? data : []
  } catch (err) {
    message.error(`读取学习记录失败: ${String(err?.message || err || '')}`)
  }
}

const refreshAudio = async () => {
  try {
    const data = await invoke('language_learning_list_audio_resources', { keyword: keyword.value, learningLang: learningLang.value })
    audioResources.value = Array.isArray(data) ? data : []
  } catch (err) {
    message.error(`读取听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const refreshAll = async () => {
  await refreshRecords()
  await refreshAudio()
  if (!activeRow.value && filteredRows.value.length) {
    activeRow.value = filteredRows.value[0]
  }
}

const refreshWordMeta = async (row) => {
  try {
    const payload = await invoke('language_learning_enrich_word', {
      word: row.word || row.source_text,
      sourceLang: row.source_lang || 'auto',
      targetLang: row.target_lang || 'zh',
    })
    await invoke('language_learning_record_translation', {
      input: {
        source_text: row.source_text,
        translated_text: row.translated_text,
        source_lang: row.source_lang,
        target_lang: row.target_lang,
        engine: row.engine,
        record_type: 'word',
        word_meta: {
          word: payload?.word || row.word || row.source_text,
          phonetic: payload?.phonetic || '',
          phonetic_uk: payload?.phonetic_uk || '',
          phonetic_us: payload?.phonetic_us || '',
          part_of_speech: payload?.part_of_speech || '',
          meanings: payload?.meanings || [],
          synonyms: payload?.synonyms || [],
          antonyms: payload?.antonyms || [],
          associates: payload?.associates || [],
          synonyms_translations: payload?.synonyms_translations || [],
          antonyms_translations: payload?.antonyms_translations || [],
          associates_translations: payload?.associates_translations || [],
          phrases: payload?.phrases || [],
          phrase_translations: payload?.phrase_translations || [],
          examples: payload?.examples || [],
          example_translations: payload?.example_translations || [],
        },
      },
    })
    message.success('词汇拓展已更新')
    await refreshRecords()
    const hit = allRows.value.find((x) => x.sourceType === 'record' && x.rawId === row.id)
    if (hit) activeRow.value = hit
  } catch (err) {
    message.error(`刷新词汇拓展失败: ${String(err?.message || err || '')}`)
  }
}

const resetAudioForm = () => {
  audioForm.id = ''
  audioForm.title = ''
  audioForm.source_kind = 'remote'
  audioForm.source_uri = ''
  audioForm.duration_sec = 0
  audioForm.transcript = ''
  audioForm.notes = ''
}

const editAudio = (row) => {
  if (!row) return
  audioForm.id = row.id || ''
  audioForm.title = row.title || ''
  audioForm.source_kind = row.source_kind || 'remote'
  audioForm.source_uri = row.source_uri || ''
  audioForm.duration_sec = Number(row.duration_sec || 0)
  audioForm.transcript = row.transcript || ''
  audioForm.notes = row.notes || ''
  activeTab.value = 'review'
}

const saveAudio = async () => {
  try {
    await invoke('language_learning_upsert_audio_resource', {
      input: { ...audioForm },
      learningLang: learningLang.value,
    })
    message.success('听力资源已保存')
    resetAudioForm()
    await refreshAudio()
  } catch (err) {
    message.error(`保存听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const removeAudio = async (row) => {
  try {
    await invoke('language_learning_delete_audio_resource', { id: row.id, learningLang: learningLang.value })
    message.success('听力资源已删除')
    if (activeRow.value?.sourceType === 'audio' && activeRow.value?.rawId === row.id) {
      activeRow.value = null
    }
    await refreshAudio()
  } catch (err) {
    message.error(`删除听力资源失败: ${String(err?.message || err || '')}`)
  }
}

let learningPronounceAudio = null
const playWordPronunciation = (rawWord, accent = 'us') => {
  const text = String(rawWord || '').trim()
  if (!text) return
  const type = accent === 'uk' ? '1' : '0'
  const url = `https://dict.youdao.com/dictvoice?audio=${encodeURIComponent(text)}&type=${type}`
  try {
    if (learningPronounceAudio) {
      learningPronounceAudio.pause()
      learningPronounceAudio = null
    }
    learningPronounceAudio = new Audio(url)
    learningPronounceAudio.play().catch(() => {})
  } catch {}
}

const playSelectedPronunciation = () => {
  const text = activeWordRecord.value?.word || activeWordRecord.value?.source_text || ''
  if (!text) {
    message.warning('请先选择一个单词条目')
    return
  }
  playWordPronunciation(text, 'us')
}

const exportLearningData = () => {
  try {
    const payload = {
      exported_at: new Date().toISOString(),
      records: records.value,
      audio_resources: audioResources.value,
      mastery: masteryStore.value,
    }
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `language-learning-export-${Date.now()}.json`
    a.click()
    URL.revokeObjectURL(url)
    message.success('学习数据已导出')
  } catch {
    message.error('导出失败')
  }
}

watch([recordKind, selectedShelf], () => {
  if (!filteredRows.value.length) {
    activeRow.value = null
    return
  }
  if (!activeRow.value || !filteredRows.value.some((x) => x.id === activeRow.value.id)) {
    activeRow.value = filteredRows.value[0]
  }
})

watch(learningLang, async (v) => {
  try {
    window.localStorage.setItem(learningLangStorageKey, String(v || 'zh'))
  } catch {}
  activeRow.value = null
  await refreshAll()
  rebuildReviewRows()
})

watch([reviewType, filteredRows], () => {
  rebuildReviewRows()
})

watch(activeTab, (tab) => {
  if (tab === 'review') {
    rebuildReviewRows()
  }
})

onMounted(async () => {
  loadMasteryStore()
  await refreshAll()
  if (filteredRows.value.length) {
    activeRow.value = filteredRows.value[0]
  }
  rebuildReviewRows()
})
</script>

<style scoped>
.lang-learn-page {
  height: 100vh;
  box-sizing: border-box;
  padding: 12px;
  background: var(--guaxi-app-bg);
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
}

.manager-header {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.title-main {
  font-size: 22px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.toolbar-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.page-tabs {
  flex: 1 1 auto;
  min-height: 0;
}

.manager-layout {
  height: calc(100vh - 150px);
  display: grid;
  grid-template-columns: 260px 1fr 420px;
  gap: 10px;
}

.left-tree-card,
.middle-table-card,
.right-detail-card {
  min-height: 0;
  overflow: hidden;
}

.left-tree-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: auto;
}

.tree-group-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 4px;
}

.tree-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 82%, transparent);
}

.tree-item.active {
  border-color: var(--primary-color, #4acadb);
  background: color-mix(in srgb, var(--primary-color, #4acadb) 14%, var(--guaxi-panel-bg));
}

.tree-count {
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.middle-table-card {
  display: flex;
  flex-direction: column;
}

.table-toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.mastery-stars {
  font-size: 12px;
  color: #f5a524;
  letter-spacing: 1px;
}

.right-detail-card {
  overflow: auto;
}

.word-detail-head {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px 12px;
  margin-bottom: 10px;
  background: color-mix(in srgb, var(--guaxi-header-bg) 70%, transparent);
}

.word-detail-title {
  font-size: 28px;
  font-weight: 700;
  line-height: 1.05;
}

.word-detail-meta {
  margin-top: 8px;
  color: var(--guaxi-text-secondary);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.phonetic-row {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.detail-block {
  margin-bottom: 12px;
}

.detail-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.detail-label.tiny {
  font-size: 12px;
  opacity: 0.85;
  margin-top: 6px;
}

.detail-content {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 8px 10px;
  line-height: 1.56;
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--guaxi-panel-bg);
}

.chip-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.example-line {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  padding: 8px 10px;
  margin-bottom: 6px;
  line-height: 1.5;
}

.example-src {
  line-height: 1.55;
}

.example-target {
  margin-top: 4px;
  line-height: 1.55;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 70%, var(--guaxi-text-primary));
}

.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.review-layout {
  height: calc(100vh - 150px);
  display: grid;
  grid-template-columns: 340px 1fr 360px;
  gap: 10px;
}

.review-stat-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.review-stat-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.stat-cell {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 86%, transparent);
}

.stat-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.stat-value {
  margin-top: 4px;
  font-size: 24px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.mastery-bar-track {
  height: 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-border) 75%, transparent);
  overflow: hidden;
}

.mastery-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #4acadb 0%, #6ed4a1 100%);
}

.review-card-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.review-topbar {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}

.review-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 14px;
  padding: 16px;
  background: linear-gradient(160deg, color-mix(in srgb, var(--guaxi-panel-bg) 95%, #7cc8ff 5%) 0%, color-mix(in srgb, var(--guaxi-panel-bg) 95%, #87f6c9 5%) 100%);
  min-height: 420px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.review-card-type {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.review-card-front {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 16px;
  min-height: 150px;
  background: var(--guaxi-panel-bg);
}

.review-word {
  font-size: 40px;
  font-weight: 700;
  line-height: 1.08;
}

.review-phonetic {
  margin-top: 10px;
  font-size: 14px;
  color: var(--guaxi-text-secondary);
}

.review-text {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.7;
  font-size: 18px;
}

.review-card-answer {
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 14px;
  background: linear-gradient(160deg, color-mix(in srgb, var(--guaxi-header-bg) 74%, transparent) 0%, color-mix(in srgb, var(--guaxi-panel-bg) 92%, #8fb8ff 8%) 100%);
}

.review-answer-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.review-answer-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 8px;
}

.review-answer-chip {
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  padding: 3px 10px;
  font-size: 12px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, transparent);
}

.review-answer-text {
  line-height: 1.65;
  font-size: 15px;
  white-space: pre-wrap;
  word-break: break-word;
}

.review-examples {
  margin-top: 10px;
}

.review-example-item {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 8px 10px;
  margin-top: 6px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
}

.review-answer-target {
  margin-top: 4px;
  color: color-mix(in srgb, var(--primary-color, #4acadb) 72%, var(--guaxi-text-primary));
}

.review-actions {
  margin-top: auto;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.audio-form-card {
  overflow: auto;
}
</style>
