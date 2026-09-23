<template>
  <div class="cog-inbox" @mouseup="onInboxMouseUp">
    <!-- 一键就绪引导：认知层关闭或语义服务未就绪时显示。
         替代"自己去图谱视图给知识库生成向量"的绕路。 -->
    <div v-if="!store.enabled || !store.embeddingReady" class="cog-setup">
      <n-icon class="cog-setup-icon" :component="Lightbulb20Regular" />
      <div class="cog-setup-text">
        <b>大脑还没准备好</b>
        <span v-if="!store.enabled">认知层已关闭——开启后它才会观察抓取内容、算新颖度、生成简报。</span>
        <span v-else>语义服务未就绪——打分、新颖度、召回、矛盾检测都依赖它。点右侧一键就绪（首次会下载本地模型）。</span>
      </div>
      <div class="cog-setup-actions">
        <n-button v-if="!store.enabled" size="small" type="primary" :loading="busy.enable" @click="enableCognition">
          开启认知层
        </n-button>
        <n-button v-else size="small" type="primary" :loading="busy.embed" @click="readyEmbedding">
          一键就绪语义服务
        </n-button>
      </div>
    </div>

    <!-- 顶部：状态与操作。刻意把"今日已打断 / 阈值 / 采纳率"放在最显眼处——
         用户要能一眼看出系统在多大程度上打扰了自己，以及为什么。 -->
    <div class="cog-bar">
      <div class="cog-bar-left">
        <n-tag :bordered="false" :type="store.enabled ? 'success' : 'default'" size="small">
          <template #icon><n-icon :component="BrainCircuit20Regular" /></template>
          {{ store.enabled ? '认知层运行中' : '认知层已关闭' }}
        </n-tag>
        <n-tag v-if="!store.embeddingReady" :bordered="false" type="warning" size="small">
          <template #icon><n-icon :component="Warning20Regular" /></template>
          语义服务未就绪，召回与打分暂停
        </n-tag>
        <span class="cog-metric" title="等你处理的提醒条数">
          <n-icon :component="Alert20Regular" />待处理 <b>{{ store.openNudgeCount }}</b>
        </span>
        <span class="cog-metric" title="收件箱里未处理的感知条数">
          <n-icon :component="Mail20Regular" />收件箱 <b>{{ st?.percept?.newCount ?? 0 }}</b>
          <i v-if="store.pendingScoreCount">（{{ store.pendingScoreCount }} 待打分）</i>
        </span>
        <span class="cog-metric" title="今日已用掉的打断次数 / 剩余额度">
          <n-icon :component="Flash20Regular" />打断 <b>{{ st?.nudge?.deliveredToday ?? 0 }}</b>
          / 剩 {{ st?.nudge?.interruptsLeftToday ?? 0 }}
        </span>
        <span class="cog-metric" :title="thresholdHint">
          <n-icon :component="Gauge20Regular" />阈值
          <b>{{ (st?.nudge?.threshold ?? 0).toFixed(2) }}</b>
          ｜采纳率 {{ acceptPct }}
        </span>
      </div>
      <div class="cog-bar-right">
        <GuaxiIconButton
          icon-only
          :icon="Lightbulb20Regular"
          label="立即审议"
          hint="现在就扫一遍新感知与图谱变化，产出提醒并按额度投递。不必等下一轮自动审议。"
          :loading="busy.deliberate"
          @click="deliberate"
        />
        <GuaxiIconButton
          icon-only
          :icon="News20Regular"
          label="生成简报"
          hint="把够不上打断门槛的内容汇总成今天的简报。会消耗少量 token。"
          :loading="store.briefLoading"
          @click="makeBrief"
        />
        <GuaxiIconButton
          icon-only
          quaternary
          :icon="ArrowSync20Regular"
          label="刷新"
          hint="重新读取状态、提醒、收件箱与简报列表。"
          :loading="store.statusLoading"
          @click="reload"
        />
      </div>
    </div>

    <n-tabs v-model:value="tab" type="line" size="small" class="cog-tabs">
      <n-tab-pane name="nudges">
        <template #tab>
          <span class="cog-tab">
            <n-icon :component="Alert20Regular" />提醒 ({{ store.openNudgeCount }})
          </span>
        </template>
        <div class="cog-filters">
          <n-tag
            :bordered="false"
            size="small"
            :checked="store.nudgeFilterKind === ''"
            checkable
            @update:checked="store.nudgeFilterKind = ''"
          >全部</n-tag>
          <n-tag
            v-for="(meta, kind) in KINDS"
            :key="kind"
            :bordered="false"
            size="small"
            checkable
            :checked="store.nudgeFilterKind === kind"
            :title="meta.hint"
            @update:checked="store.nudgeFilterKind = store.nudgeFilterKind === kind ? '' : kind"
          >
            <template #icon>
              <n-icon :component="nudgeIcon(kind)" :style="{ color: meta.color }" />
            </template>
            {{ meta.label }}
            <i v-if="countOf(kind)">{{ countOf(kind) }}</i>
          </n-tag>
        </div>

        <n-spin :show="store.nudgesLoading">
          <div v-if="!store.filteredNudges.length" class="cog-empty">
            没有待处理的提醒。系统仍在后台观察感知收件箱与知识库的变化。
          </div>
          <div v-for="n in store.filteredNudges" :key="n.id" class="cog-card">
            <div class="cog-card-head">
              <n-icon
                class="cog-kind-icon"
                :component="nudgeIcon(n.kind)"
                :style="{ color: kindColor(n.kind) }"
                :title="kindHint(n.kind)"
              />
              <span class="cog-card-title">{{ n.title }}</span>
              <n-tag size="tiny" :bordered="false" :title="kindHint(n.kind)">
                {{ kindLabel(n.kind) }}
              </n-tag>
              <n-tag size="tiny" :bordered="false" :title="channelHint(n.channel)">
                <template #icon><n-icon :component="channelIcon(n.channel)" /></template>
                {{ channelLabel(n.channel) }}
              </n-tag>
              <span class="cog-card-score" :title="`策略分 ${n.policyScore.toFixed(3)}｜效用 ${n.utility.toFixed(2)}｜成本 ${n.cost.toFixed(2)}`">
                {{ n.policyScore.toFixed(2) }}
              </span>
            </div>

            <div class="cog-card-body">{{ n.body }}</div>

            <!-- 证据链：为什么现在拿这件事打扰你。没有它，提醒就只是断言。 -->
            <ul v-if="n.why?.length" class="cog-why">
              <li v-for="(w, i) in n.why" :key="i">{{ w }}</li>
            </ul>

            <div v-if="n.refs?.length" class="cog-refs">
              <a
                v-for="(r, i) in n.refs"
                :key="i"
                class="cog-ref"
                :title="refTitle(r)"
                @click="openRef(r)"
              >{{ r.label || r.id }}</a>
            </div>

            <div class="cog-card-actions">
              <GuaxiIconButton
                icon-only
                type="primary"
                :icon="CheckmarkCircle20Regular"
                label="采纳"
                hint="这条有用。系统会据此调高同类提醒的权重，下次更早告诉你。"
                @click="decide(n, true)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="DismissCircle20Regular"
                label="忽略"
                hint="这条没用。系统会调低同类提醒并抬高打断门槛，少来烦你。"
                @click="decide(n, false)"
              />
              <GuaxiIconButton
                v-if="n.kind === 'review'"
                icon-only
                quaternary
                :icon="CalendarClock20Regular"
                label="忘了，明天再来"
                hint="把这条提醒引用的章节全部重排到明天再问，同时缩短它们的复习间隔。"
                @click="reviewAll(n, false)"
              />
            </div>
          </div>
        </n-spin>
      </n-tab-pane>

      <n-tab-pane name="percepts">
        <template #tab>
          <span class="cog-tab">
            <n-icon :component="Mail20Regular" />收件箱 ({{ st?.percept?.newCount ?? 0 }})
          </span>
        </template>
        <div class="cog-filters">
          <n-select
            v-model:value="store.perceptFilter.state"
            size="tiny"
            class="cog-select"
            :options="stateOptions"
            @update:value="() => store.refreshPercepts()"
          />
          <n-select
            v-model:value="store.perceptFilter.sourceKind"
            size="tiny"
            class="cog-select"
            :options="sourceOptions"
            @update:value="() => store.refreshPercepts()"
          />
          <!-- 总数提示：解释"当前筛选只显示 N 条，但累计抓了 M 条"，避免"东西丢了"的错觉。 -->
          <span class="cog-count-hint" :title="`当前筛选 ${store.percepts.length} 条，累计入库 ${st?.percept?.total ?? '—'} 条`">
            显示 {{ store.percepts.length }} / 累计 {{ st?.percept?.total ?? '—' }}
          </span>
          <GuaxiIconButton
            v-if="store.pendingScoreCount"
            icon-only
            :icon="Gauge20Regular"
            label="立即打分"
            :hint="`给 ${store.pendingScoreCount} 条待处理内容求向量、算新颖度与显著性。全程本地模型，不花 token。`"
            :loading="busy.score"
            @click="scoreNow"
          />
        </div>

        <n-spin :show="store.perceptsLoading">
          <div v-if="!store.percepts.length" class="cog-empty">
            这一档没有内容。抓取、采集、本地文件都会自动流入这里。
          </div>
          <div v-for="p in store.percepts" :key="p.id" class="cog-card">
            <div class="cog-card-head">
              <span class="cog-card-title">{{ p.title || '（无标题）' }}</span>
              <n-tag size="tiny" :bordered="false">
                <template #icon><n-icon :component="sourceIcon(p.sourceKind)" /></template>
                {{ sourceLabel(p.sourceKind) }}
              </n-tag>
              <n-tag
                v-if="p.neutralized > 0"
                size="tiny"
                type="warning"
                :bordered="false"
                title="这条材料里出现了试图指挥 AI 的内容，已被中和。它仍然只能当资料读。"
              >
                <template #icon><n-icon :component="ShieldKeyhole20Regular" /></template>
                已中和 {{ p.neutralized }} 处
              </n-tag>
            </div>

            <!-- 两条量尺：显著性决定它会不会进简报，新颖度说明它与你已知的距离。 -->
            <div class="cog-meters">
              <span class="cog-meter" title="显著性：兼顾新颖度、与你关注的贴合度、来源可信度">
                显著 {{ p.salience.toFixed(2) }}
                <i class="cog-meter-bar"><b :style="{ width: pct(p.salience) }"></b></i>
              </span>
              <span class="cog-meter" title="新颖度：1 减去与知识库最相近内容的余弦相似度">
                新颖 {{ p.novelty.toFixed(2) }}
                <i class="cog-meter-bar"><b class="alt" :style="{ width: pct(p.novelty) }"></b></i>
              </span>
              <span v-if="p.anchorNodeId" class="cog-meter" :title="p.anchorNodeId">
                <n-icon :component="Link20Regular" />已锚定到知识库
              </span>
            </div>

            <div class="cog-card-body cog-untrusted markdown-body"
              v-html="perceptHtml[p.id] || escapeText(p.excerpt)"></div>

            <div class="cog-card-actions">
              <GuaxiIconButton
                v-if="p.uri"
                icon-only
                quaternary
                :icon="Open20Regular"
                label="打开原文"
                hint="在内置浏览窗口里打开来源链接。窗口里双击段落可以直接把它采集回收件箱。"
                @click="openUri(p.uri)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="DocumentOnePage20Regular"
                label="查看全文"
                hint="在弹窗里读清洗后的全文。外部内容只作资料，不会自己进知识库。"
                @click="viewFull(p)"
              />
              <GuaxiIconButton
                icon-only
                :icon="Archive20Regular"
                label="归档"
                hint="留着但不再算「未处理」。归档过的条目不会被每日清理带走。"
                @click="setState(p, 'archived')"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="ThumbDislike20Regular"
                label="不感兴趣"
                hint="从收件箱移出。它仍留在库里，可按「不感兴趣」筛出来，只是不再计入未处理。"
                @click="setState(p, 'rejected')"
              />
            </div>
          </div>
        </n-spin>
      </n-tab-pane>

      <n-tab-pane name="brief">
        <template #tab>
          <span class="cog-tab"><n-icon :component="News20Regular" />简报</span>
        </template>
        <div class="cog-filters">
          <n-select
            v-model:value="briefDay"
            size="tiny"
            class="cog-select-wide"
            :options="briefOptions"
            placeholder="选择日期"
            @update:value="(d) => store.loadBrief(d)"
          />
          <GuaxiIconButton
            icon-only
            :icon="ArrowClockwise20Regular"
            label="重新生成今天"
            hint="覆盖今天的简报再出一份。适合上午出完之后又读了不少东西的时候。"
            :loading="store.briefLoading"
            @click="makeBrief"
          />
        </div>
        <n-spin :show="store.briefLoading">
          <div v-if="briefHtml" class="cog-brief markdown-body" v-html="briefHtml"></div>
          <div v-else class="cog-empty">
            今天还没有简报。它会在你设定的时刻自动生成（画像里的「简报时刻」），也可以手动生成。
          </div>
        </n-spin>
      </n-tab-pane>

      <!-- 知识：从材料抽出的主张（信念）、矛盾检测，以及进化闩等高级维护。
           这些能力后端一直存在，之前大脑页没有入口，这里补上。 -->
      <n-tab-pane name="knowledge">
        <template #tab>
          <span class="cog-tab"><n-icon :component="Lightbulb20Regular" />知识</span>
        </template>

        <div class="cog-filters">
          <span class="cog-count-hint">
            活跃主张 {{ st?.belief?.active ?? 0 }}｜已被反驳 {{ st?.belief?.contradicted ?? 0 }}
          </span>
          <GuaxiIconButton
            icon-only
            :icon="Lightbulb20Regular"
            label="抽取主张"
            :hint="kbName ? `从知识库「${kbName}」里抽取可被反驳的结论（消耗少量 token）。` : '先在图谱视图选中一个知识库。'"
            :loading="busy.belief"
            @click="extractBeliefs"
          />
          <GuaxiIconButton
            icon-only
            :icon="Warning20Regular"
            label="检测矛盾"
            hint="两两比对主张，找出互相冲突的结论并标记（消耗少量 token）。"
            :loading="busy.contradict"
            @click="detectContradictions"
          />
        </div>

        <n-spin :show="store.beliefsLoading">
          <div v-if="!store.beliefs.length" class="cog-empty">
            还没有主张。点「抽取主张」从选中的知识库里提取，之后新读到的材料若与它冲突会自动标记。
          </div>
          <div v-for="b in store.beliefs" :key="b.id" class="cog-card">
            <div class="cog-card-head">
              <span class="cog-card-title">{{ b.statement }}</span>
              <n-tag size="tiny" :bordered="false"
                :type="b.status === 'contradicted' ? 'warning' : (b.status === 'active' ? 'success' : 'default')">
                {{ beliefStatusLabel(b.status) }}
              </n-tag>
              <span class="cog-card-score" :title="`置信度 ${(b.confidence ?? 0).toFixed(2)}`">
                {{ (b.confidence ?? 0).toFixed(2) }}
              </span>
            </div>
            <div v-if="b.kb_name" class="cog-belief-kb">来源库：{{ b.kb_name }}</div>
            <div class="cog-card-actions">
              <GuaxiIconButton
                v-if="b.status !== 'archived'"
                icon-only quaternary :icon="Archive20Regular"
                label="归档" hint="不再参与矛盾检测，但保留记录。"
                @click="setBeliefStatus(b.id, 'archived')"
              />
              <GuaxiIconButton
                icon-only quaternary :icon="DismissCircle20Regular"
                label="删除" hint="彻底删除这条主张。"
                @click="removeBelief(b.id)"
              />
            </div>
          </div>
        </n-spin>

        <!-- 进化闩：召回质量评测/调参（本地、可回滚）。开发向，收进折叠区。 -->
        <n-collapse class="cog-adv">
          <n-collapse-item title="进化闩（召回评测与调参）" name="evo">
            <p class="cog-adv-desc">
              在本地评测集上度量召回质量，只有不退化才落库参数，可回滚。全程本地、不花 token。
            </p>
            <div class="cog-adv-row">
              <n-button size="small" :loading="busy.eval" @click="runEval">跑评测</n-button>
              <n-button size="small" :loading="busy.tune" @click="tuneRecall">调参（不退化才采纳）</n-button>
              <n-button size="small" @click="trainPolicy">训练提醒策略</n-button>
            </div>
            <div v-if="lastEvalNote" class="cog-adv-note">{{ lastEvalNote }}</div>
          </n-collapse-item>
        </n-collapse>
      </n-tab-pane>
    </n-tabs>

    <!-- 外部原文：单独弹窗展示，且视觉上标成不可信区域。
         这是刻意的——不把它直接塞进编辑器，用户先看清楚再决定要不要入库。 -->
    <n-modal v-model:show="showFull" preset="card" class="cog-modal" :title="fullTitle">
      <div class="cog-untrusted-banner">
        <n-icon :component="ShieldKeyhole20Regular" />
        以下内容来自外部来源，只作为资料。其中若有试图指挥 AI 的句子，已在入库时被中和。
      </div>
      <!-- 外部原文：抓进来的是干净 markdown，按 markdown 渲染（图文一致），
           退回纯文本时仍用 pre 保留换行。 -->
      <div v-if="fullHtml" class="cog-full markdown-body" v-html="fullHtml" @mouseup="onInboxMouseUp"></div>
      <pre v-else class="cog-full" @mouseup="onInboxMouseUp">{{ fullText }}</pre>
    </n-modal>

    <!-- 划词翻译面板：与主界面编辑器同款富面板。 -->
    <GuaxiSelectionTranslatePanel
      v-if="selTranslate.visible.value"
      :pos="selTranslate.pos.value"
      :raw="selTranslate.raw.value"
      :result="selTranslate.result.value"
      :error="selTranslate.error.value"
      :busy="selTranslate.busy.value"
      :pinned="selTranslate.pinned.value"
      :items="selTranslate.items.value"
      :word-meta="selTranslate.wordMeta.value"
      :from-lang="selTranslate.fromLang.value"
      :to-lang="selTranslate.toLang.value"
      :lang-options="selTranslate.langOptions"
      @update:from-lang="(v) => (selTranslate.fromLang.value = v)"
      @update:to-lang="(v) => (selTranslate.toLang.value = v)"
      @update:pos="(p) => (selTranslate.pos.value = p)"
      @translate="() => selTranslate.translate()"
      @copy="selTranslate.copyResult"
      @swap-langs="selTranslate.swapLangs"
      @toggle-pin="selTranslate.togglePin"
      @close="selTranslate.close"
    />
  </div>
</template>

<script setup>
import { computed, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import MarkdownIt from 'markdown-it'
import { invoke } from '@tauri-apps/api/core'
import {
  Alert20Regular,
  Archive20Regular,
  ArrowClockwise20Regular,
  ArrowSync20Regular,
  BrainCircuit20Regular,
  CalendarClock20Regular,
  CheckmarkCircle20Regular,
  DismissCircle20Regular,
  DocumentOnePage20Regular,
  Flash20Regular,
  Gauge20Regular,
  Lightbulb20Regular,
  Link20Regular,
  Mail20Regular,
  News20Regular,
  Open20Regular,
  ShieldKeyhole20Regular,
  ThumbDislike20Regular,
  Warning20Regular,
} from '@vicons/fluent'
import GuaxiIconButton from '../GuaxiIconButton.vue'
import {
  useCognitionStore,
  NUDGE_KINDS,
  PERCEPT_SOURCE_LABELS,
} from '../../stores/cognitionStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { channelIcon, nudgeIcon, sourceIcon } from '../../utils/cognitionIcons'
import { onCognitionNudge, onCognitionBrief } from '../../utils/cognitionEvents'
import { createLogger } from '../../utils/logger'
import { useSelectionTranslate } from '../../composables/useSelectionTranslate.js'
import GuaxiSelectionTranslatePanel from '../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker.js'

const logger = createLogger('GuaxiCognitionInbox')
const emit = defineEmits(['open-ref'])

// 划词翻译：选中收件箱/简报/外部原文里的文字即弹富面板（与编辑器同款）。
const selTranslate = useSelectionTranslate({
  getContainer: () => document.querySelector('.cog-inbox'),
})
const onInboxMouseUp = (event) => { selTranslate.handleMouseUp(event) }

const store = useCognitionStore()
const message = useMessage()
const KINDS = NUDGE_KINDS

const tab = ref('nudges')
const briefDay = ref(null)
const busy = ref({
  deliberate: false, score: false, enable: false, embed: false,
  belief: false, contradict: false, eval: false, tune: false,
})
const kbStore = useKnowledgeBaseStore()
const kbName = computed(() => kbStore.selectedKnowledgeBase?.knowledge_base_name || '')
const lastEvalNote = ref('')

const beliefStatusLabel = (s) => ({
  active: '活跃', contradicted: '已被反驳', archived: '已归档',
}[s] || s || '')

async function extractBeliefs() {
  if (!kbName.value) { message.warning('请先在图谱视图选中一个知识库'); return }
  busy.value.belief = true
  try {
    const r = await store.extractBeliefs(kbName.value)
    message.success(`已抽取 ${r?.extracted ?? r?.count ?? ''} 条主张`)
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.belief = false
  }
}

async function detectContradictions() {
  busy.value.contradict = true
  try {
    const found = await store.detectContradictions()
    message.success(found ? `发现 ${found} 处矛盾` : '未发现新的矛盾')
    await store.refreshBeliefs({ kbName: kbName.value })
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.contradict = false
  }
}

async function setBeliefStatus(id, status) {
  try { await store.setBeliefStatus(id, status) } catch (e) { message.error(String(e)) }
}
async function removeBelief(id) {
  try { await store.removeBelief(id) } catch (e) { message.error(String(e)) }
}

async function runEval() {
  busy.value.eval = true
  try {
    const r = await store.runEval()
    lastEvalNote.value = r?.note || '评测完成'
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.eval = false
  }
}
async function tuneRecall() {
  busy.value.tune = true
  try {
    const r = await store.tuneRecall()
    lastEvalNote.value = r?.note || '调参完成'
    message.success(lastEvalNote.value)
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.tune = false
  }
}
async function trainPolicy() {
  try {
    await store.trainPolicy()
    message.success('提醒策略已训练')
  } catch (e) {
    message.error(String(e))
  }
}

// 一键开启认知层。
async function enableCognition() {
  busy.value.enable = true
  try {
    await store.setEnabled(true)
    await store.bootstrap()
    message.success('认知层已开启')
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.enable = false
  }
}

// 一键让语义服务就绪（首次下载本地嵌入模型）。
async function readyEmbedding() {
  busy.value.embed = true
  try {
    const ok = await store.ensureEmbeddingReady()
    message.success(ok ? '语义服务已就绪' : '已尝试启动，稍后自动生效')
    await store.bootstrap()
  } catch (e) {
    message.error(`语义服务启动失败：${e}`)
  } finally {
    busy.value.embed = false
  }
}
const st = computed(() => store.status)

const md = new MarkdownIt({ html: false, linkify: true, breaks: true })
// 简报：用项目统一的 GuaxiMarker 渲染（与阅读页/编辑器一致）。异步，用 ref 承载。
const briefHtml = ref('')
watch(() => store.brief?.markdown, async (mdText) => {
  const t = String(mdText || '').trim()
  briefHtml.value = t ? await renderMarkdown(t, { buttons: false, mermaid: true, math: true }) : ''
}, { immediate: true })

// 外部原文：同样走 GuaxiMarker 渲染（含图片/标题/列表）。
const fullHtml = ref('')
watch(() => fullText.value, async (t) => {
  const s = String(t || '').trim()
  fullHtml.value = s ? await renderMarkdown(s, { buttons: false, mermaid: true, math: true }) : ''
})

// 收件箱卡片正文：逐条用 GuaxiMarker 渲染 excerpt（含图片/链接/标题），id→html 缓存，
// 未渲染完成前先显示转义后的纯文本，避免闪原始 markdown 或注入。
const perceptHtml = ref({})
const escapeText = (s) => String(s || '')
  .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
watch(() => store.percepts, async (list) => {
  const arr = Array.isArray(list) ? list : []
  for (const p of arr) {
    if (!p || perceptHtml.value[p.id] != null) continue
    const raw = String(p.excerpt || '').trim()
    if (!raw) { perceptHtml.value[p.id] = ''; continue }
    try {
      const html = await renderMarkdown(raw, { buttons: false, mermaid: false, math: false })
      perceptHtml.value = { ...perceptHtml.value, [p.id]: html }
    } catch {
      perceptHtml.value = { ...perceptHtml.value, [p.id]: escapeText(raw) }
    }
  }
}, { immediate: true, deep: false })

const acceptPct = computed(() => {
  const r = st.value?.nudge?.acceptRate14d
  return r == null ? '—' : `${Math.round(r * 100)}%`
})
const thresholdHint =
  '投递阈值按近期采纳率自适应：你越少采纳，门槛越高，打扰越少。可在设置里回滚到基线。'

const stateOptions = [
  { label: '未处理', value: 'new' },
  { label: '已分流', value: 'triaged' },
  { label: '已入库', value: 'linked' },
  { label: '已归档', value: 'archived' },
  { label: '不感兴趣', value: 'rejected' },
]
const sourceOptions = computed(() => [
  { label: '全部来源', value: '' },
  ...Object.entries(PERCEPT_SOURCE_LABELS).map(([value, label]) => ({ label, value })),
])
const briefOptions = computed(() =>
  store.briefHistory.map((b) => ({ label: `${b.day}（${b.kind}）`, value: b.day })),
)

// 左侧边栏点击"简报日期 / 感知来源 / 待处理分类"时，只更新了 store，
// 中间收件箱不会自动跳到对应板块。这里监听 store 变化，把内部 tab 同步过去，
// 让点击左栏条目 = 中间区域直接切到该内容。
watch(
  () => store.brief?.day,
  (day) => {
    if (day) {
      briefDay.value = day
      tab.value = 'brief'
    }
  }
)
watch(
  () => store.perceptFilter.sourceKind,
  (kind, prev) => {
    // 来源筛选变化（含点"全部"清空）都意味着用户在看感知收件箱。
    if (kind !== prev) tab.value = 'percepts'
  }
)
watch(
  () => store.nudgeFilterKind,
  () => { tab.value = 'nudges' }
)
// 切到"知识"tab 时按需拉取主张列表。
watch(tab, (t) => {
  if (t === 'knowledge' && !store.beliefs.length && !store.beliefsLoading) {
    store.refreshBeliefs({ kbName: kbName.value })
  }
})

const kindLabel = (k) => KINDS[k]?.label || k
const kindColor = (k) => KINDS[k]?.color || '#888'
const kindHint = (k) => KINDS[k]?.hint || ''
const sourceLabel = (k) => PERCEPT_SOURCE_LABELS[k] || k
const countOf = (kind) => store.nudges.filter((n) => n.kind === kind).length
const pct = (v) => `${Math.round(Math.max(0, Math.min(1, Number(v) || 0)) * 100)}%`

const CHANNELS = {
  board: ['看板', '只在这里显示，不打扰你'],
  brief: ['简报', '进入下一份简报'],
  notify: ['已通知', '曾以系统通知推送'],
  interrupt: ['已打断', '曾以应用内卡片打断'],
}
const channelLabel = (c) => CHANNELS[c]?.[0] || c
const channelHint = (c) => CHANNELS[c]?.[1] || ''

const refTitle = (r) => {
  if (r.path) return r.line ? `${r.path}:${r.line}` : r.path
  return r.uri || r.id
}

/** 引用跳转：交给宿主决定在哪个视图打开，同时把点击上报给学习回路。 */
function openRef(r) {
  emit('open-ref', r)
  if (store.lastGrounded?.episodeId) {
    store.reportCitationClick(store.lastGrounded.episodeId, r.id)
  }
  if (!r.path && r.uri) openUri(r.uri)
}

/**
 * 打开外部原文。
 *
 * 走内置浏览窗口（`open_article_in_browser`）而不是 opener 插件：
 * 一是 capabilities 里 `opener:allow-open-url` 的 scope 只放了 `vscode://file/*`，
 * 任意 http(s) 会被拒；二是内置窗口注入了双击采集脚本，读到有用的段落可以直接回流成感知。
 */
async function openUri(uri) {
  try {
    await invoke('open_article_in_browser', { url: uri })
  } catch (e) {
    logger.warn('open uri failed', e)
    message.warning('无法打开该链接')
  }
}

async function decide(n, accepted) {
  try {
    await store.decideNudge(n.id, accepted)
    message.success(accepted ? '已采纳' : '已忽略，系统会据此调低同类提醒')
  } catch (e) {
    message.error(String(e))
  }
}

/** 复习提醒里的"忘了"：把该提醒引用的每个章节都重排到明天。 */
async function reviewAll(n, remembered) {
  const nodes = (n.refs || []).filter((r) => r.kind === 'kg_node')
  for (const r of nodes) {
    try {
      await store.reviewFeedback(r.id, remembered)
    } catch (e) {
      logger.debug('review feedback failed', e)
    }
  }
  await store.decideNudge(n.id, true)
  message.success(remembered ? '已拉长复习间隔' : '已重排到明天')
}

async function setState(p, state) {
  try {
    await store.setPerceptState(p.id, state)
  } catch (e) {
    message.error(String(e))
  }
}

async function viewFull(p) {
  try {
    const full = await store.getPerceptText(p.id)
    // 用对话框展示而不是直接塞进编辑器：这是外部内容，先看清楚再决定要不要入库。
    fullText.value = full?.text || ''
    fullTitle.value = full?.title || p.title || '（无标题）'
    showFull.value = true
  } catch (e) {
    message.error(String(e))
  }
}

const showFull = ref(false)
const fullText = ref('')
const fullTitle = ref('')

async function deliberate() {
  busy.value.deliberate = true
  try {
    const r = await store.deliberateNow()
    message.success(
      `审议完成：新增 ${r.produced} 条，投递 ${r.delivered} 条` +
        (r.budgetExhausted ? '（今日 token 预算已用尽，已跳过需要模型的部分）' : ''),
    )
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.deliberate = false
  }
}

async function scoreNow() {
  busy.value.score = true
  try {
    const n = await store.scorePercepts(24)
    message.success(n ? `已打分 ${n} 条` : '没有待打分的条目')
  } catch (e) {
    message.warning(`打分暂不可用：${e}`)
  } finally {
    busy.value.score = false
  }
}

async function makeBrief() {
  try {
    const b = await store.generateBrief('morning')
    briefDay.value = b.day
    tab.value = 'brief'
    message.success('简报已生成')
  } catch (e) {
    message.error(String(e))
  }
}

async function reload() {
  await store.bootstrap()
  await store.refreshBriefHistory()
  briefDay.value = store.brief?.day ?? null
}

let offNudge = null
let offBrief = null

onMounted(async () => {
  await reload()
  offNudge = await onCognitionNudge((evt) => {
    store.refreshNudges()
    store.refreshStatus()
    if (evt.push) {
      const meta = KINDS[evt.kind]
      message.info(`新提醒：${meta?.label || evt.kind}`, { duration: 6000 })
    }
  })
  offBrief = await onCognitionBrief(async () => {
    await store.loadBrief()
    await store.refreshBriefHistory()
  })
})

// 中间区被 <KeepAlive> 缓存：从"读文章"切回大脑页时不会重跑 onMounted，
// 导致收件箱停留在旧数据、看不到刚读/刚抓进来的新内容。onActivated 里重新拉一遍。
onActivated(() => {
  reload()
  // 当前在收件箱页时，顺带刷新感知列表，确保新入库的 percept 立即出现。
  if (tab.value === 'percepts') {
    store.refreshPercepts()
  }
})

onUnmounted(() => {
  if (offNudge) offNudge()
  if (offBrief) offBrief()
})
</script>

<style scoped>
/* 排版基线与主界面一致：同一套字体、14px 正文、次要文字走
   --guaxi-text-secondary 而不是 opacity。这一页要长时间盯着读，
   压暗一档的代价比省下的层次感大得多。 */
.cog-inbox {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 10px 12px;
  box-sizing: border-box;
  overflow: hidden;
  font-family: var(--guaxi-editor-font-family);
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-primary);
}

/* 一键就绪引导条 */
.cog-setup {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  margin-bottom: 10px;
  border-radius: 8px;
  border-left: 3px solid var(--n-color-primary, #2dd4bf);
  background: color-mix(in srgb, var(--n-color-primary, #2dd4bf) 8%, transparent);
}
.cog-setup-icon {
  font-size: 22px;
  flex: 0 0 auto;
  color: var(--n-color-primary, #2dd4bf);
}
.cog-setup-text {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 13px;
  line-height: 1.6;
}
.cog-setup-text b {
  font-size: 14px;
}
.cog-setup-text span {
  color: var(--guaxi-text-secondary);
}
.cog-setup-actions {
  flex: 0 0 auto;
}

.cog-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
  padding-bottom: 8px;
}
.cog-bar-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  font-size: 13px;
  color: var(--guaxi-text-secondary);
}
.cog-bar-right {
  display: flex;
  gap: 6px;
}
.cog-metric {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.cog-metric b {
  font-weight: 600;
  color: var(--guaxi-text-primary);
  font-variant-numeric: tabular-nums;
}
.cog-metric i {
  font-style: normal;
}
/* 指标前的图标压小一档：它是标签而不是主角，与数字同高会抢注意力。 */
.cog-metric :deep(.n-icon) {
  font-size: 14.5px;
  color: var(--guaxi-text-secondary);
}

.cog-tabs {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

/* naive n-tabs（type=line，非 animated）不生成 pane-wrapper：tab-pane 直接是 .n-tabs 的子节点。
   所以要让 .n-tabs 成为撑满高度的 flex 列，nav 固定，当前 tab-pane 吃掉剩余高度并自己滚动。
   之前挂在不存在的 pane-wrapper 上，所以内容多时无法滚动。 */
.cog-tabs :deep(.n-tabs) {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.cog-tabs :deep(.n-tabs-nav) {
  flex: 0 0 auto;
}
/* naive 会把 nav 之后的内容包一层 .n-tabs .n-tab-pane（或直接是 pane）；两种都撑满并滚动。 */
.cog-tabs :deep(.n-tabs > .n-tab-pane),
.cog-tabs :deep(.n-tabs-pane-wrapper) {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}
.cog-tabs :deep(.n-tab-pane) {
  min-height: 0;
}
/* 页签标题里的图标：naive 的页签是纯文本容器，要自己做对齐。 */
.cog-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.cog-tab :deep(.n-icon) {
  font-size: 16px;
}

.cog-filters {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  padding: 4px 0 10px;
}
.cog-count-hint {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
}
.cog-belief-kb {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin: 2px 0 4px;
}
.cog-adv {
  margin-top: 14px;
  border-top: 1px dashed var(--guaxi-border);
  padding-top: 8px;
}
.cog-adv-desc {
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
  margin: 0 0 8px;
}
.cog-adv-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.cog-adv-note {
  margin-top: 8px;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  white-space: pre-wrap;
}
.cog-select {
  width: 128px;
}
.cog-select-wide {
  width: 210px;
}

/* 提醒卡片左侧的类型图标：颜色由 NUDGE_KINDS 给，形状由 NUDGE_ICONS 给。
   两者一起做视觉锚点——只有颜色时色盲用户读不出类型。 */
.cog-kind-icon {
  font-size: 17px;
  flex: 0 0 auto;
}

.cog-empty {
  padding: 30px 12px;
  text-align: center;
  font-size: 13.5px;
  line-height: 1.9;
  color: var(--guaxi-text-secondary);
}

.cog-card {
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  padding: 11px 13px;
  margin-bottom: 9px;
}
.cog-card-head {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.cog-card-title {
  font-weight: 600;
  font-size: 14.5px;
  line-height: 1.6;
  flex: 1 1 auto;
  min-width: 0;
}
.cog-card-score {
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  font-variant-numeric: tabular-nums;
}

.cog-card-body {
  font-size: 13.5px;
  line-height: 1.85;
  margin: 7px 0;
  white-space: pre-wrap;
  word-break: break-word;
}
/* 外部内容用左侧竖线标记，与用户自己的内容视觉分开。
   刻意不降低对比度：它是需要被认真读的资料，只是来源不可信。 */
.cog-untrusted {
  border-left: 3px solid rgba(217, 138, 43, 0.65);
  padding-left: 9px;
}

.cog-why {
  margin: 5px 0 7px;
  padding-left: 18px;
  font-size: 12.5px;
  line-height: 1.8;
  color: var(--guaxi-text-secondary);
}

.cog-refs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 6px 0;
}
.cog-ref {
  font-size: 12.5px;
  padding: 1px 8px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  cursor: pointer;
  max-width: 280px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cog-ref:hover {
  border-color: #3b82c4;
  color: #3b82c4;
}

.cog-card-actions {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}

.cog-meters {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  margin-top: 5px;
}
.cog-meter {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-variant-numeric: tabular-nums;
}
.cog-meter :deep(.n-icon) {
  font-size: 15px;
}
.cog-meter-bar {
  display: inline-block;
  width: 54px;
  height: 4px;
  border-radius: 2px;
  background: rgba(128, 128, 128, 0.22);
  overflow: hidden;
}
.cog-meter-bar b {
  display: block;
  height: 100%;
  background: #5b9e6a;
}
.cog-meter-bar b.alt {
  background: #8b6bc9;
}

.cog-brief {
  font-size: 14px;
  line-height: 1.95;
  padding: 4px 2px 24px;
}
.cog-brief :deep(h1) {
  font-size: 19px;
  margin: 6px 0 14px;
}
.cog-brief :deep(h2) {
  font-size: 16px;
  margin: 20px 0 9px;
  padding-bottom: 5px;
  border-bottom: 1px solid var(--guaxi-border);
}
.cog-brief :deep(h3) {
  font-size: 14.5px;
  margin: 13px 0 7px;
}
.cog-brief :deep(blockquote) {
  margin: 9px 0;
  padding: 7px 13px;
  border-left: 3px solid #3b82c4;
  color: var(--guaxi-text-secondary);
}
.cog-brief :deep(code) {
  font-size: 12.5px;
  padding: 1px 4px;
  border-radius: 3px;
  background: rgba(128, 128, 128, 0.16);
}

/* 弹窗被 teleport 到 body，拿不到 .cog-inbox 的继承，字体要单独声明。 */
.cog-modal {
  width: 760px;
  max-width: 92vw;
  font-family: var(--guaxi-editor-font-family);
  font-size: 14px;
}
.cog-untrusted-banner {
  display: flex;
  align-items: flex-start;
  gap: 7px;
  font-size: 13px;
  line-height: 1.8;
  padding: 7px 11px;
  margin-bottom: 9px;
  border-radius: 4px;
  border-left: 3px solid rgba(217, 138, 43, 0.8);
  background: rgba(217, 138, 43, 0.1);
}
.cog-untrusted-banner :deep(.n-icon) {
  font-size: 17px;
  flex: 0 0 auto;
  margin-top: 2px;
}
.cog-full {
  max-height: 58vh;
  overflow: auto;
  font-size: 13.5px;
  line-height: 1.9;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  font-family: var(--guaxi-editor-font-family);
}

/* naive-ui 的页签与控件默认字号偏小，且不继承外层字体，这里统一拉回。 */
.cog-inbox :deep(.n-tabs-tab) {
  font-size: 14px;
}
.cog-inbox :deep(.n-base-selection-label),
.cog-inbox :deep(.n-button),
.cog-inbox :deep(.n-tag) {
  font-family: var(--guaxi-editor-font-family);
}
</style>
