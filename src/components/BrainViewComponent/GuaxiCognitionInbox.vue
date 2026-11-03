<template>
  <div class="cog-inbox">
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
        <!-- 打分失败不再只写日志：把原因放在最显眼处，点一下就能重试。 -->
        <n-tag
          v-if="store.pendingScoreCount && st?.scoringLastError"
          :bordered="false"
          type="warning"
          size="small"
          style="cursor: pointer;"
          :title="st.scoringLastError"
          @click="scoreNow"
        >
          <template #icon><n-icon :component="Warning20Regular" /></template>
          {{ store.pendingScoreCount }} 条未打分 · 点此重试
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
        <GuaxiIconButton
          icon-only
          quaternary
          :icon="Settings20Regular"
          label="高级设置"
          hint="主张抽取、矛盾检测、策略训练与评测都收在设置页的「认知」里。"
          @click="openAdvancedSettings"
        />
      </div>
    </div>

    <n-tabs v-model:value="tab" type="line" size="small" class="cog-tabs">
      <!-- 今日：把"该干什么"放在第一屏。提醒和值得读的材料合成一张行动清单，
           每条就地处理，不再需要在三个 tab 之间来回找。 -->
      <n-tab-pane name="today">
        <template #tab>
          <span class="cog-tab">
            <n-icon :component="WeatherSunny20Regular" />今日
          </span>
        </template>
        <div class="today-head">
          <span class="today-date">{{ todayLabel }}</span>
          <span class="today-meta">
            提醒 {{ store.openNudgeCount }} · 值得读 {{ store.topPercepts.length }} ·
            <a
              v-if="store.pendingScoreCount"
              class="today-link"
              :title="st?.scoringLastError || '点击立即给待处理内容打分'"
              @click="scoreNow"
            >待打分 {{ store.pendingScoreCount }}</a>
            <span v-else>待打分 0</span>
          </span>
        </div>
        <!-- 简报开场一句话：每日先看结论，再决定展开或直接处理下面的清单。 -->
        <div v-if="briefTeaser" class="today-brief">
          <span class="today-brief-label">今日简报</span>
          <span class="today-brief-text">{{ briefTeaser }}</span>
          <n-button size="tiny" quaternary @click="tab = 'brief'">展开</n-button>
        </div>
        <div v-if="!todayItems.length" class="cog-empty">
          今天没有必须处理的事。采集和提醒仍会在后台继续，有新内容再来这里。
        </div>
        <div v-for="it in todayItems" :key="it.key" class="today-item">
          <span class="today-dot" :style="{ background: it.color }"></span>
          <div class="today-body">
            <div class="today-title">{{ it.title }}</div>
            <div class="today-sub">{{ it.sub }}</div>
          </div>
          <div class="today-actions">
            <template v-if="it.type === 'nudge'">
              <GuaxiIconButton
                v-if="it.data.kind === 'review'"
                icon-only
                type="primary"
                :icon="CheckmarkCircle20Regular"
                label="还记得"
                hint="记得住：拉长这条复习提醒的间隔，并采纳它。"
                @click="reviewAll(it.data, true)"
              />
              <GuaxiIconButton
                v-else
                icon-only
                type="primary"
                :icon="CheckmarkCircle20Regular"
                label="采纳"
                hint="这条有用。系统会据此调高同类提醒的权重。"
                @click="decide(it.data, true)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="DismissCircle20Regular"
                label="忽略"
                hint="这条没用。系统会调低同类提醒，少来烦你。"
                @click="decide(it.data, false)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="Open20Regular"
                label="去看"
                hint="打开这条提醒引用的内容。"
                @click="jumpNudge(it.data)"
              />
              <GuaxiIconButton
                v-if="it.data.kind === 'review'"
                icon-only
                quaternary
                :icon="CalendarClock20Regular"
                label="忘了"
                hint="把这条复习提醒引用的章节重排到明天。"
                @click="reviewAll(it.data, false)"
              />
            </template>
            <template v-else>
              <GuaxiIconButton
                icon-only
                type="primary"
                :icon="BookAdd20Regular"
                label="提炼成笔记"
                hint="用模型提炼摘要与要点，写进选定的知识库。"
                @click="openNoteDialog(it.data)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="DocumentOnePage20Regular"
                label="查看全文"
                hint="在独立阅读窗口里读清洗后的全文：可朗读、整篇对照翻译、按大纲跳转。"
                @click="viewFull(it.data)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="Archive20Regular"
                label="归档"
                hint="留着但不再算「未处理」。"
                @click="setState(it.data, 'archived')"
              />
            </template>
          </div>
        </div>
        <div class="today-foot">
          <n-button size="tiny" quaternary @click="tab = 'brief'">查看今天简报</n-button>
          <n-button size="tiny" quaternary @click="tab = 'percepts'">打开全部收件箱</n-button>
        </div>
      </n-tab-pane>

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
                v-if="n.kind === 'review'"
                icon-only
                type="primary"
                :icon="CheckmarkCircle20Regular"
                label="还记得"
                hint="记得住：拉长这条复习提醒的间隔，并采纳它。"
                @click="reviewAll(n, true)"
              />
              <GuaxiIconButton
                v-else
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
              <GuaxiIconButton
                v-if="n.refs?.length"
                icon-only
                quaternary
                :icon="Open20Regular"
                label="跳到原文"
                hint="打开这条提醒引用的那一处内容。"
                @click="jumpNudge(n)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="Add20Regular"
                label="加入任务"
                hint="把这条提醒写成任务管理里的一条 TODO。"
                @click="addTask(n)"
              />
              <GuaxiIconButton
                icon-only
                quaternary
                :icon="BookQuestionMark20Regular"
                label="生成自测"
                hint="根据引用章节出 3 道自测题，答案折叠，先自己回想。"
                @click="openQuiz(n)"
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
          <div v-for="p in visiblePercepts" :key="p.id" class="cog-card">
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
                新颖 {{ p.novelty < 0 ? '待打分' : p.novelty.toFixed(2) }}
                <i class="cog-meter-bar"><b class="alt" :style="{ width: pct(p.novelty < 0 ? 0 : p.novelty) }"></b></i>
              </span>
              <span v-if="p.anchorNodeId" class="cog-meter" :title="p.anchorNodeId">
                <n-icon :component="Link20Regular" />已锚定到知识库
              </span>
            </div>

            <div class="cog-card-body cog-untrusted markdown-body"
              v-html="perceptHtml[p.id] || escapeText(p.excerpt)"></div>

            <div class="cog-card-actions">
              <GuaxiIconButton
                icon-only
                type="primary"
                :icon="BookAdd20Regular"
                label="提炼成笔记"
                hint="用模型提炼摘要与要点，写进选定的知识库；原文链接会一并保留。"
                @click="openNoteDialog(p)"
              />
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
                hint="在独立阅读窗口里读清洗后的全文（大纲 / 朗读 / 对照翻译）。外部内容只作资料，不会自己进知识库。"
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
          <!-- 默认只露显著性最高的 10 条，避免一屏噪声；其余折叠。 -->
          <div v-if="store.percepts.length > PERCEPT_PAGE" class="cog-more">
            <n-button size="tiny" quaternary @click="showAllPercepts = !showAllPercepts">
              {{ showAllPercepts ? '收起' : `显示全部 ${store.percepts.length} 条` }}
            </n-button>
          </div>
        </n-spin>
      </n-tab-pane>

      <n-tab-pane name="brief">
        <template #tab>
          <span class="cog-tab"><n-icon :component="News20Regular" />简报</span>
        </template>
        <div class="cog-filters">
          <n-button
            v-if="todayItems.length"
            size="tiny"
            type="primary"
            quaternary
            @click="tab = 'today'"
          >去处理今天的 {{ todayItems.length }} 件事</n-button>
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

    </n-tabs>

    <!-- 提炼成笔记：选目标知识库/子目录 → 模型提炼 → 写入并回显落点。 -->
    <n-modal
      v-model:show="noteDialog.show"
      preset="card"
      class="cog-modal"
      title="提炼成笔记"
      style="width: 760px; max-width: 94vw;"
    >
      <div class="note-form">
        <div class="note-row">
          <span class="note-label">目标知识库</span>
          <n-select
            v-model:value="noteDialog.kbName"
            size="small"
            :options="kbOptions"
            placeholder="选择知识库"
            style="width: 240px;"
          />
          <span class="note-label">子目录</span>
          <n-input
            v-model:value="noteDialog.folder"
            size="small"
            placeholder="收件箱"
            style="width: 200px;"
          />
        </div>
        <div v-if="noteDialog.error" class="note-error">{{ noteDialog.error }}</div>
        <div class="note-preview">
          <pre v-if="noteDialog.result" class="note-pre">{{ noteDialog.result.markdown }}</pre>
          <div v-else class="note-hint">
            先本地生成摘要与要点、立即写入知识库；模型在后台提炼完成后自动覆盖，
            不会卡住界面。原文链接与摘录始终保留。
          </div>
        </div>
        <div v-if="noteDialog.result" class="note-path">
          已写入 <code>{{ noteDialog.result.path }}</code>
          <span v-if="noteDialog.result.refining">（本地摘要已写入，正在后台提炼…）</span>
          <span v-else-if="!noteDialog.result.usedLlm">（模型未完成，本次为本地摘要）</span>
        </div>
      </div>
      <template #footer>
        <div class="note-footer">
          <n-button v-if="noteDialog.result" size="small" @click="openNoteFile">打开笔记</n-button>
          <n-button size="small" :disabled="noteDialog.busy" @click="noteDialog.show = false">关闭</n-button>
          <n-button
            size="small"
            type="primary"
            :loading="noteDialog.busy"
            :disabled="!noteDialog.kbName || !!noteDialog.result"
            @click="confirmNote"
          >
            写入知识库
          </n-button>
        </div>
      </template>
    </n-modal>

    <!-- 自测：答案默认折叠，先回想再看。 -->
    <n-modal
      v-model:show="quiz.show"
      preset="card"
      class="cog-modal"
      :title="`自测 · ${quiz.title}`"
      style="width: 700px; max-width: 94vw;"
    >
      <n-spin :show="quiz.busy">
        <div v-if="quiz.error" class="note-error">{{ quiz.error }}</div>
        <n-collapse v-if="quiz.items.length" class="quiz-list">
          <n-collapse-item
            v-for="(q, i) in quiz.items"
            :key="i"
            :name="`q${i}`"
            :title="`${i + 1}. ${q.question}`"
          >
            <div class="quiz-answer">{{ q.answer }}</div>
          </n-collapse-item>
        </n-collapse>
        <div v-else-if="!quiz.busy && !quiz.error" class="cog-empty">没有生成题目。</div>
      </n-spin>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  Add20Regular,
  Alert20Regular,
  Archive20Regular,
  ArrowClockwise20Regular,
  ArrowSync20Regular,
  BookAdd20Regular,
  BookQuestionMark20Regular,
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
  Settings20Regular,
  ShieldKeyhole20Regular,
  ThumbDislike20Regular,
  Warning20Regular,
  WeatherSunny20Regular,
} from '@vicons/fluent'
import GuaxiIconButton from '../GuaxiIconButton.vue'
import {
  useCognitionStore,
  NUDGE_KINDS,
  PERCEPT_SOURCE_LABELS,
} from '../../stores/cognitionStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { channelIcon, nudgeIcon, sourceIcon } from '../../utils/cognitionIcons'
import { onCognitionNudge, onCognitionBrief } from '../../utils/cognitionEvents'
import { createLogger } from '../../utils/logger'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker.js'

const logger = createLogger('GuaxiCognitionInbox')
const emit = defineEmits(['open-ref'])

// 朗读 / 译文 / 大纲不再在收件箱的悬浮层里实现：全文交给独立的 Markdown 阅读窗口
// （create_article_reader_window），那里的工具栏才是这些能力该待的地方。

const store = useCognitionStore()
const message = useMessage()
const KINDS = NUDGE_KINDS

const tab = ref('today')
const briefDay = ref(null)
const showAllPercepts = ref(false)
const PERCEPT_PAGE = 10
const busy = ref({
  deliberate: false, score: false, enable: false, embed: false,
})
const kbStore = useKnowledgeBaseStore()
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const settingsStore = useSettingsStore()
const kbName = computed(() => kbStore.selectedKnowledgeBase?.knowledge_base_name || '')
const kbOptions = computed(() =>
  (kbStore.knowledgeBases || []).map((k) => ({
    label: k.knowledge_base_name,
    value: k.knowledge_base_name,
  })),
)

// ── 今日看板：提醒 + 值得读，合成一张行动清单
const todayLabel = computed(() => {
  const d = new Date()
  const wd = ['日', '一', '二', '三', '四', '五', '六'][d.getDay()]
  return `${d.getMonth() + 1} 月 ${d.getDate()} 日 · 周${wd}`
})
const visiblePercepts = computed(() =>
  showAllPercepts.value ? store.percepts : store.percepts.slice(0, PERCEPT_PAGE),
)
/** 简报的第一段正文：优先开场引用，其次第一段正文；跳过标题/表格/列表符号。 */
const briefTeaser = computed(() => {
  const md = String(store.brief?.markdown || '')
  if (!md.trim()) return ''
  const lines = md.split('\n').map((l) => l.trim())
  const quote = lines.find((l) => l.startsWith('> '))
  const body = lines.find(
    (l) =>
      l &&
      !l.startsWith('#') &&
      !l.startsWith('>') &&
      !l.startsWith('-') &&
      !l.startsWith('*') &&
      !l.startsWith('|') &&
      l.length > 12,
  )
  const line = (quote ? quote.replace(/^>\s*/, '') : body || '').trim()
  if (!line) return ''
  return line.length > 120 ? `${line.slice(0, 120)}…` : line
})
const todayItems = computed(() => {
  const nudges = store.nudges.slice(0, 3).map((n) => ({
    type: 'nudge',
    key: `n-${n.id}`,
    data: n,
    title: n.title,
    sub: n.body || kindHint(n.kind),
    color: kindColor(n.kind),
  }))
  const percepts = (store.topPercepts || []).map((p) => ({
    type: 'percept',
    key: `p-${p.id}`,
    data: p,
    title: p.title || '（无标题）',
    sub:
      Number(p.salience) > 0.25
        ? `显著 ${Number(p.salience).toFixed(2)} · ${sourceLabel(p.sourceKind)}`
        : `待打分 · ${sourceLabel(p.sourceKind)}`,
    color: '#3b82c4',
  }))
  return [...nudges, ...percepts]
})

// ── 提醒的三种行动：跳转 / 加入任务 / 生成自测
function jumpNudge(n) {
  const r = (n.refs || [])[0]
  if (!r) {
    message.info('这条提醒没有可跳转的引用')
    return
  }
  openRef(r)
}

async function addTask(n) {
  try {
    const path = await store.nudgeToTask(n.id, 'work')
    message.success(`已加入任务：${path}`)
  } catch (e) {
    message.error(String(e))
  }
}

const quiz = ref({ show: false, busy: false, title: '', items: [], error: '' })
async function openQuiz(n) {
  quiz.value = { show: true, busy: true, title: n.title, items: [], error: '' }
  try {
    quiz.value.items = await store.nudgeQuiz(n.id)
  } catch (e) {
    quiz.value.error = String(e)
  } finally {
    quiz.value.busy = false
  }
}

// ── 提炼成笔记
const noteDialog = ref({
  show: false,
  percept: null,
  kbName: '',
  folder: '收件箱',
  busy: false,
  result: null,
  error: '',
})
function openNoteDialog(p) {
  noteDialog.value = {
    show: true,
    percept: p,
    // 优先用这条资料锚定的知识库,其次当前选中的,最后列表第一个。
    kbName: p.kbName || kbName.value || kbOptions.value[0]?.value || '',
    folder: '收件箱',
    busy: false,
    result: null,
    error: '',
  }
}
async function confirmNote() {
  const d = noteDialog.value
  if (!d.percept || !d.kbName) return
  d.busy = true
  d.error = ''
  try {
    const r = await store.perceptToNote(d.percept.id, d.kbName, d.folder)
    d.result = r
    message.success(r.refining ? '已写入，摘要会在后台补全' : '已写入')
  } catch (e) {
    d.error = String(e)
  } finally {
    d.busy = false
  }
}

function openAdvancedSettings() {
  settingsStore.setSelectedMenu('cognition')
  window.location.hash = '/setting'
}

/** 笔记写完后直接在主编辑器打开,不用自己去文件树里找。 */
async function openNoteFile() {
  const path = noteDialog.value.result?.path
  if (!path) return
  try {
    await fileTreeStore.openFileInTab(path)
    const content = fileTreeStore.selectedFile?.content || noteDialog.value.result?.markdown || ''
    editorStore.setFile({ path, content })
    noteDialog.value.show = false
    window.location.hash = '/'
  } catch (e) {
    message.error(`打开失败：${e}`)
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
// 首次 bootstrap 也会写 store.brief,不能把默认的「今日」顶掉;
// 只有挂载之后用户点左栏日期/新一天简报到达时才切到简报 tab。
let briefWatchReady = false
watch(
  () => store.brief?.day,
  (day) => {
    if (!briefWatchReady) return
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
// 换筛选条件时回到前 10 条,避免"显示全部"的展开状态沿用上一次筛选。
watch(
  () => [store.perceptFilter.state, store.perceptFilter.sourceKind],
  () => { showAllPercepts.value = false },
)

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

/**
 * 打开外部原文。
 *
 * 走独立阅读窗口而不是弹窗：朗读、译文、大纲都是「阅读器」这一层的能力，
 * 塞进弹窗只会让它越长越像半个阅读器，而正文又只能挤在一小块可视区里。
 * 窗口自己按 id 取正文（cognition_percept_get）——正文动辄几十 KB，
 * 不适合塞进 window 参数；主界面只负责把 id 与标题递过去。
 */
async function viewFull(p) {
  try {
    await invoke('create_article_reader_window', {
      id: p.id,
      title: String(p.title || ''),
      theme: settingsStore.appearanceTheme,
    })
  } catch (e) {
    logger.warn('open article reader window failed', { error: String(e) })
    message.error(`打开阅读器失败：${e}`)
  }
}

// GuaxiMarker 渲染（与阅读页/编辑器一致）。
// 简报正文。
const briefHtml = ref('')
watch(() => store.brief?.markdown, async (mdText) => {
  const t = String(mdText || '').trim()
  briefHtml.value = t ? await renderMarkdown(t, { buttons: false, mermaid: true, math: true }) : ''
}, { immediate: true })

// 收件箱卡片正文：逐条用 GuaxiMarker 渲染 excerpt，id→html 缓存；
// 未渲染完成前显示转义纯文本，避免闪原始 markdown 或注入。
const perceptHtml = ref({})
const escapeText = (s) => String(s || '')
  .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
watch(() => store.percepts, async (list) => {
  const arr = Array.isArray(list) ? list : []
  for (const p of arr) {
    if (!p || perceptHtml.value[p.id] != null) continue
    const raw = String(p.excerpt || '').trim()
    if (!raw) { perceptHtml.value = { ...perceptHtml.value, [p.id]: '' }; continue }
    try {
      const html = await renderMarkdown(raw, { buttons: false, mermaid: false, math: false })
      perceptHtml.value = { ...perceptHtml.value, [p.id]: html }
    } catch {
      perceptHtml.value = { ...perceptHtml.value, [p.id]: escapeText(raw) }
    }
  }
}, { immediate: true, deep: false })

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
let offNoteRefined = null

onMounted(async () => {
  await reload()
  briefWatchReady = true
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
  // 笔记的后台模型提炼完成：若弹窗还停在同一篇，实时替换预览。
  offNoteRefined = await listen('cognition_note_refined', (evt) => {
    const payload = evt?.payload || {}
    const cur = noteDialog.value.result
    if (!cur || payload.path !== cur.path) return
    noteDialog.value.result = {
      ...cur,
      markdown: payload.markdown || cur.markdown,
      usedLlm: true,
      refining: false,
    }
    message.success('摘要已提炼完成')
  })
})

// 中间区被 <KeepAlive> 缓存：从"读文章"切回大脑页时不会重跑 onMounted，
// 导致收件箱停留在旧数据、看不到刚读/刚抓进来的新内容。onActivated 里重新拉一遍。
onActivated(() => {
  reload()
  // 当前在收件箱页时，顺带刷新感知列表，确保新入库的 percept 立即出现。
  if (tab.value === 'percepts' || tab.value === 'today') {
    store.refreshPercepts()
  }
})

onUnmounted(() => {
  if (offNudge) offNudge()
  if (offBrief) offBrief()
  if (offNoteRefined) offNoteRefined()
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

/* class="cog-tabs" 直接落在 n-tabs 根节点上，所以 .cog-tabs 本身就是 .n-tabs，
   flex 列布局要写在它自己身上，而不是它的后代（之前挂在不存在的后代 .n-tabs 上，
   规则从未生效，pane 拿不到高度也就无法滚动）。 */
.cog-tabs {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.cog-tabs :deep(.n-tabs-nav) {
  flex: 0 0 auto;
}
/* type=line 非 animated 时 tab-pane 直接是根节点的子节点；animated 时会包一层
   pane-wrapper。两种都让它吃掉剩余高度并自己滚动。 */
.cog-tabs :deep(.n-tab-pane),
.cog-tabs :deep(.n-tabs-pane-wrapper) {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
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

/* naive-ui 的页签与控件默认字号偏小，且不继承外层字体，这里统一拉回。 */
.cog-inbox :deep(.n-tabs-tab) {
  font-size: 14px;
}
.cog-inbox :deep(.n-base-selection-label),
.cog-inbox :deep(.n-button),
.cog-inbox :deep(.n-tag) {
  font-family: var(--guaxi-editor-font-family);
}

/* ── 今日看板 */
.today-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin: 8px 0 10px;
}
.today-date { font-weight: 600; }
.today-meta { font-size: 12px; color: var(--guaxi-text-secondary); }
.today-item {
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 10px 12px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  margin-bottom: 8px;
  background: var(--guaxi-panel-bg);
}
.today-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: 0 0 auto;
}
.today-body { flex: 1; min-width: 0; }
.today-title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.today-sub {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.today-actions { display: flex; gap: 4px; flex: 0 0 auto; }

/* ── 提炼成笔记 / 自测 */
.cog-more { display: flex; justify-content: center; padding: 6px 0 12px; }
.note-form { display: flex; flex-direction: column; gap: 10px; }
.note-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.note-label { font-size: 12px; color: var(--guaxi-text-secondary); }
.note-error { color: #e04f5f; font-size: 12.5px; white-space: pre-wrap; }
.note-preview {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-app-bg);
  max-height: 42vh;
  overflow: auto;
}
.note-pre {
  margin: 0;
  padding: 12px;
  white-space: pre-wrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 13px;
  line-height: 1.7;
}
.note-hint {
  padding: 16px;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  line-height: 1.7;
}
.note-path { font-size: 12px; color: var(--guaxi-text-secondary); word-break: break-all; }
.note-path code { font-family: var(--guaxi-editor-font-family); color: var(--guaxi-text-primary); }
.note-footer { display: flex; justify-content: flex-end; gap: 8px; }
.quiz-list { max-height: 60vh; overflow: auto; }
.quiz-answer { white-space: pre-wrap; line-height: 1.8; }

.today-foot {
  display: flex;
  gap: 8px;
  justify-content: center;
  padding: 4px 0 12px;
}

.today-brief {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  margin-bottom: 10px;
  border-left: 3px solid var(--n-color-primary, #2dd4bf);
  border-radius: 6px;
  background: color-mix(in srgb, var(--n-color-primary, #2dd4bf) 7%, transparent);
}
.today-brief-label {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  flex: 0 0 auto;
}
.today-brief-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.today-link {
  color: var(--n-color-primary, #2dd4bf);
  cursor: pointer;
  text-decoration: underline dotted;
}
.today-link:hover { text-decoration-style: solid; }
</style>
