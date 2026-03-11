<template>
  <n-message-provider>
  <div class="lang-learn-page">
    <div class="page-header">
      <div class="title-wrap">
        <div class="title-main">语言学习中心</div>
        <div class="title-sub">词汇积累、例句回放与听力资源管理</div>
      </div>
      <div class="header-actions">
        <n-input v-model:value="keyword" clearable placeholder="搜索原文、译文、单词..." size="small" @keydown.enter="refreshRecords" />
        <n-button size="small" @click="refreshRecords">刷新</n-button>
      </div>
    </div>

    <n-tabs type="line" animated class="page-tabs">
      <n-tab-pane name="records" tab="学习记录">
        <div class="records-layout">
          <div class="records-left">
            <n-radio-group v-model:value="recordKind" size="small">
              <n-radio-button v-for="it in recordKindOptions" :key="it.value" :value="it.value">
                {{ it.label }}
              </n-radio-button>
            </n-radio-group>
            <n-data-table
              size="small"
              :columns="recordColumns"
              :data="records"
              :max-height="620"
              :row-key="(row) => row.id"
              :row-props="recordRowProps"
              :single-line="false"
            />
          </div>
          <div class="records-right">
            <n-card title="学习详情" size="small" class="detail-card">
              <template v-if="activeRecord">
                <div class="detail-line">
                  <n-tag size="small" type="info">{{ typeLabel(activeRecord.record_type) }}</n-tag>
                  <span class="detail-time">{{ activeRecord.created_at }}</span>
                </div>
                <div class="detail-block">
                  <div class="detail-label">原文</div>
                  <div class="detail-content">{{ activeRecord.source_text }}</div>
                </div>
                <div class="detail-block">
                  <div class="detail-label">译文</div>
                  <div class="detail-content">{{ activeRecord.translated_text }}</div>
                </div>
                <template v-if="activeRecord.is_word">
                  <div class="dict-head">
                    <div class="dict-word">{{ activeRecord.word || activeRecord.source_text }}</div>
                    <div class="dict-meta">
                      <span v-if="activeRecord.phonetic">{{ activeRecord.phonetic }}</span>
                      <span v-if="activeRecord.part_of_speech">· {{ activeRecord.part_of_speech }}</span>
                    </div>
                  </div>
                  <div class="detail-block" v-if="activeRecord.meanings?.length">
                    <div class="detail-label">释义</div>
                    <n-space vertical :size="6">
                      <n-tag v-for="(x, i) in activeRecord.meanings" :key="`m-${i}`" size="small" type="success">{{ x }}</n-tag>
                    </n-space>
                  </div>
                  <div class="detail-block" v-if="activeRecord.synonyms?.length">
                    <div class="detail-label">近义词</div>
                    <n-space :size="6" wrap>
                      <n-tag v-for="(x, i) in activeRecord.synonyms" :key="`s-${i}`" size="small">{{ x }}</n-tag>
                    </n-space>
                  </div>
                  <div class="detail-block" v-if="activeRecord.antonyms?.length">
                    <div class="detail-label">反义词</div>
                    <n-space :size="6" wrap>
                      <n-tag v-for="(x, i) in activeRecord.antonyms" :key="`a-${i}`" size="small" type="warning">{{ x }}</n-tag>
                    </n-space>
                  </div>
                  <div class="detail-block" v-if="activeRecord.examples?.length">
                    <div class="detail-label">示例</div>
                    <n-space vertical :size="6">
                      <div v-for="(x, i) in activeRecord.examples" :key="`e-${i}`" class="detail-content">{{ x }}</div>
                    </n-space>
                  </div>
                  <n-button size="small" type="primary" @click="refreshWordMeta(activeRecord)">刷新词汇拓展</n-button>
                </template>
              </template>
              <n-empty v-else description="请选择一条学习记录" />
            </n-card>
          </div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="audio" tab="听力资源">
        <div class="audio-layout">
          <n-card size="small" title="新增/编辑听力资源" class="audio-form-card">
            <n-form :model="audioForm" label-placement="left" label-width="72">
              <n-form-item label="标题">
                <n-input v-model:value="audioForm.title" placeholder="例如：TED-Climate-01" />
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

          <n-card size="small" title="资源列表" class="audio-list-card">
            <n-data-table size="small" :columns="audioColumns" :data="audioResources" :row-key="(row) => row.id" :max-height="620" />
          </n-card>
        </div>
      </n-tab-pane>
    </n-tabs>
  </div>
  </n-message-provider>
</template>

<script setup>
import { h, onMounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NButton, NCard, NDataTable, NEmpty, NForm, NFormItem, NInput, NInputNumber, NMessageProvider, NRadioButton, NRadioGroup, NSelect, NSpace, NTabPane, NTabs, NTag, useMessage } from 'naive-ui'

const message = useMessage()
const keyword = ref('')
const recordKind = ref('all')
const records = ref([])
const activeRecord = ref(null)
const audioResources = ref([])

const recordKindOptions = [
  { label: '全部', value: 'all' },
  { label: '单词', value: 'word' },
  { label: '句子', value: 'sentence' },
  { label: '段落', value: 'paragraph' }
]

const audioKindOptions = [
  { label: '在线音频', value: 'remote' },
  { label: '本地文件', value: 'local' },
  { label: '听写素材', value: 'dictation' }
]

const audioForm = reactive({
  id: '',
  title: '',
  source_kind: 'remote',
  source_uri: '',
  duration_sec: 0,
  transcript: '',
  notes: ''
})

const typeLabel = (v) => {
  if (v === 'word') return '单词'
  if (v === 'paragraph') return '段落'
  return '句子'
}

const recordColumns = [
  { title: '类型', key: 'record_type', width: 72, render: (row) => h(NTag, { size: 'small', type: row.record_type === 'word' ? 'success' : 'info' }, { default: () => typeLabel(row.record_type) }) },
  { title: '原文', key: 'source_text', ellipsis: { tooltip: true } },
  { title: '译文', key: 'translated_text', ellipsis: { tooltip: true } },
  { title: '时间', key: 'created_at', width: 200, ellipsis: { tooltip: true } }
]

const audioColumns = [
  { title: '标题', key: 'title', width: 220, ellipsis: { tooltip: true } },
  { title: '类型', key: 'source_kind', width: 100 },
  { title: '地址/路径', key: 'source_uri', ellipsis: { tooltip: true } },
  { title: '时长(秒)', key: 'duration_sec', width: 100 },
  {
    title: '操作',
    key: 'actions',
    width: 120,
    render: (row) =>
      h(NSpace, { size: 6 }, () => [
        h(NButton, { size: 'tiny', onClick: () => editAudio(row) }, { default: () => '编辑' }),
        h(
          NButton,
          {
            size: 'tiny',
            type: 'error',
            onClick: () => removeAudio(row)
          },
          { default: () => '删除' }
        )
      ])
  }
]

const recordRowProps = (row) => ({
  onClick: () => {
    activeRecord.value = row
  }
})

const refreshRecords = async () => {
  try {
    const data = await invoke('language_learning_list_records', {
      keyword: keyword.value,
      kind: recordKind.value,
      limit: 500,
      offset: 0
    })
    records.value = Array.isArray(data) ? data : []
    if (records.value.length === 0) {
      activeRecord.value = null
      return
    }
    if (!activeRecord.value || !records.value.find((x) => x.id === activeRecord.value.id)) {
      activeRecord.value = records.value[0]
    }
  } catch (err) {
    message.error(`读取学习记录失败: ${String(err?.message || err || '')}`)
  }
}

const refreshAudio = async () => {
  try {
    const data = await invoke('language_learning_list_audio_resources', { keyword: '' })
    audioResources.value = Array.isArray(data) ? data : []
  } catch (err) {
    message.error(`读取听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const refreshWordMeta = async (row) => {
  try {
    const payload = await invoke('language_learning_enrich_word', {
      word: row.word || row.source_text,
      sourceLang: row.source_lang || 'auto',
      targetLang: row.target_lang || 'zh'
    })
    const next = await invoke('language_learning_record_translation', {
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
          part_of_speech: payload?.part_of_speech || '',
          meanings: payload?.meanings || [],
          synonyms: payload?.synonyms || [],
          antonyms: payload?.antonyms || [],
          examples: payload?.examples || []
        }
      }
    })
    activeRecord.value = next
    message.success('词汇拓展已更新')
    await refreshRecords()
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
  audioForm.id = row.id || ''
  audioForm.title = row.title || ''
  audioForm.source_kind = row.source_kind || 'remote'
  audioForm.source_uri = row.source_uri || ''
  audioForm.duration_sec = Number(row.duration_sec || 0)
  audioForm.transcript = row.transcript || ''
  audioForm.notes = row.notes || ''
}

const saveAudio = async () => {
  try {
    await invoke('language_learning_upsert_audio_resource', {
      input: { ...audioForm }
    })
    message.success('听力资源已保存')
    await refreshAudio()
    resetAudioForm()
  } catch (err) {
    message.error(`保存听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const removeAudio = async (row) => {
  try {
    await invoke('language_learning_delete_audio_resource', { id: row.id })
    message.success('听力资源已删除')
    await refreshAudio()
  } catch (err) {
    message.error(`删除听力资源失败: ${String(err?.message || err || '')}`)
  }
}

watch(recordKind, () => {
  refreshRecords()
})

onMounted(async () => {
  await refreshRecords()
  await refreshAudio()
})
</script>

<style scoped>
.lang-learn-page {
  height: 100vh;
  box-sizing: border-box;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--guaxi-app-bg);
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.title-wrap {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.title-main {
  font-size: 22px;
  font-weight: 700;
  color: var(--guaxi-text-primary);
}

.title-sub {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.header-actions {
  display: flex;
  gap: 10px;
  min-width: 360px;
}

.page-tabs {
  flex: 1 1 auto;
  min-height: 0;
}

.records-layout {
  height: calc(100vh - 140px);
  display: grid;
  grid-template-columns: minmax(540px, 1fr) minmax(380px, 460px);
  gap: 12px;
}

.records-left,
.records-right {
  min-height: 0;
}

.records-left {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.detail-card {
  height: 100%;
  overflow: auto;
}

.detail-line {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.detail-time {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.detail-block {
  margin-bottom: 12px;
}

.detail-label {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.detail-content {
  padding: 8px 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.dict-head {
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 90%, transparent);
  margin-bottom: 12px;
}

.dict-word {
  font-size: 24px;
  font-weight: 700;
}

.dict-meta {
  margin-top: 6px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.audio-layout {
  height: calc(100vh - 140px);
  display: grid;
  grid-template-columns: minmax(420px, 460px) 1fr;
  gap: 12px;
}

.audio-form-card,
.audio-list-card {
  height: 100%;
  overflow: auto;
}
</style>
