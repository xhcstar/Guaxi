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

      <!--
        学习看板：主题掌握度 + 复习队列 + 最近读完。
        数据来自后端 learning::overview（阅读活动、读完情节、FSRS 复习队列三处），
        这里只负责展示；「刷新」会重算掌握度。
      -->
      <n-tab-pane name="learning">
        <template #tab>
          <span class="cog-tab"><n-icon :component="Gauge20Regular" />学习</span>
        </template>
        <div ref="learningRootEl" class="learn-root">
          <div class="learn-stats">
            <div class="learn-stat">
              <b>{{ learn?.finishedBooks ?? 0 }}</b>
              <span>近 {{ learn?.days ?? 30 }} 天读完</span>
            </div>
            <div class="learn-stat">
              <b>{{ learn?.dueReviews ?? 0 }}</b>
              <span>待复习</span>
            </div>
            <div class="learn-stat">
              <b>{{ learn?.streakDays ?? 0 }}</b>
              <span>连续阅读天数</span>
            </div>
            <div class="learn-stat">
              <b>{{ learn?.mastered ?? 0 }}/{{ learn?.topicsTotal ?? 0 }}</b>
              <span>已掌握主题</span>
            </div>
          </div>
          <div class="learn-bands">
            <span class="learn-band is-weak">待加强 {{ learn?.weak ?? 0 }}</span>
            <span class="learn-band is-learning">进行中 {{ learn?.learning ?? 0 }}</span>
            <span class="learn-band is-mastered">已掌握 {{ learn?.mastered ?? 0 }}</span>
            <span class="learn-updated">更新于 {{ shortTime(learn?.updatedAt) || '—' }}</span>
          </div>

          <!-- 操作放在首屏：列表会很长，放到底部时用户要先滚到最后才找得到刷新。 -->
          <div class="learn-actions">
            <GuaxiIconButton
              :icon="ArrowSync20Regular"
              label="刷新学习状态"
              hint="重算主题掌握度并读取看板。阅读活动与复习反馈会自动带进来。"
              :loading="store.learningRefreshing"
              @click="refreshLearning"
            />
            <n-button size="tiny" quaternary @click="tab = 'brief'">去看简报</n-button>
            <span class="learn-actions-hint">阅读进度、读完情节与复习反馈会自动计入</span>
          </div>

          <n-spin :show="store.learningLoading">
            <div class="learn-section-title">该补的</div>
            <div v-if="weakTopics.length" class="learn-topic-list">
              <div v-for="t in weakTopics" :key="t.key" class="learn-topic">
                <span class="learn-topic-name" :title="t.topic">{{ t.topic }}</span>
                <span class="learn-topic-bar"><b :style="{ width: pct(t.mastery) }"></b></span>
                <span class="learn-topic-pct">{{ Math.round(t.mastery * 100) }}%</span>
                <span class="learn-topic-meta">
                  {{ t.source === 'node' ? '复习' : '阅读' }} · {{ t.sessions }} 次
                </span>
              </div>
            </div>
            <div v-else class="cog-empty">还没有可统计的主题。读一本书或复习一处笔记后就会出现在这里。</div>

            <div class="learn-section-title">今天该复习</div>
            <div v-if="learn?.due?.length" class="learn-due">
              <div v-for="d in learn.due" :key="d.nodeId" class="learn-due-row">
                <span class="learn-due-name" :title="d.label">{{ d.label }}</span>
                <span class="learn-due-meta">{{ d.kbName }} · 已复习 {{ d.reps }} 次 · {{ d.dueDate }}</span>
              </div>
            </div>
            <div v-else class="cog-empty">今天没有到期的复习。</div>

            <div class="learn-section-title">最近读完</div>
            <div v-if="learn?.recentFinished?.length" class="learn-finished">
              <div v-for="(f, i) in learn.recentFinished" :key="i" class="learn-finished-row">
                <span class="learn-finished-title" :title="f.title">{{ f.title }}</span>
                <span class="learn-finished-meta">
                  {{ shortTime(f.endedAt) }}<template v-if="f.minutes"> · {{ f.minutes }} 分钟</template>
                </span>
              </div>
            </div>
            <div v-else class="cog-empty">还没有读完的书。</div>
          </n-spin>
        </div>
      </n-tab-pane>

      <!--
        任务：只读接入白名单里的分类（设置里可逐类关闭，比如把 work 留在大脑之外）。
        到期提醒也走同一份白名单，因此这里的开关同时决定"大脑会不会为工作待办提醒你"。
      -->
      <n-tab-pane name="tasks">
        <template #tab>
          <span class="cog-tab"><n-icon :component="CalendarClock20Regular" />任务</span>
        </template>
        <div ref="taskRootEl" class="task-root">
          <div class="task-intro">
            <div class="task-cats">
              <button
                v-for="c in taskCategories?.categories || []"
                :key="c.key"
                type="button"
                class="task-cat"
                :class="{ on: c.enabled }"
                :title="c.enabled ? `已接入：${c.label}任务会出现在大脑与到期提醒里` : `未接入：${c.label}任务不会进入大脑`"
                @click="toggleTaskCategory(c.key)"
              >
                {{ c.label }}<i>{{ c.enabled ? '接入' : '不接入' }}</i>
              </button>
            </div>
            <n-button size="tiny" quaternary @click="openTaskManager">打开任务管理</n-button>
            <GuaxiIconButton
              icon-only
              quaternary
              :icon="ArrowSync20Regular"
              label="刷新任务"
              hint="重新读取接入分类里的到期任务。分类开关一改，到期提醒也会按新白名单重算。"
              :loading="store.taskLoading"
              @click="store.loadTasks()"
            />
          </div>
          <div class="task-summary">
            <span><b>{{ taskOverview?.overdue ?? 0 }}</b>已过期</span>
            <span><b>{{ taskOverview?.dueToday ?? 0 }}</b>今天</span>
            <span><b>{{ taskOverview?.dueSoon ?? 0 }}</b>接下来 {{ taskOverview?.windowDays ?? 7 }} 天</span>
          </div>
          <n-spin :show="store.taskLoading">
            <template v-if="taskGroups.length">
              <section v-for="g in taskGroups" :key="g.key" class="task-group">
                <div class="task-group-title" :class="g.tone">
                  {{ g.label }}<i>{{ g.items.length }}</i>
                </div>
                <div v-for="t in g.items" :key="t.internalId" class="task-row">
                  <span class="task-date">{{ shortDate(t.deadline) }}</span>
                  <span class="task-title" :class="{ overdue: t.overdue }" :title="t.title">{{ t.title }}</span>
                  <span class="task-meta">
                    {{ t.categoryLabel }}<template v-if="t.priority"> · P{{ t.priority }}</template> · {{ t.status }}
                  </span>
                </div>
              </section>
            </template>
            <div v-else-if="!taskAllowed.length" class="cog-empty">
              任务接入已全部关闭。打开上面的分类开关，或到设置 → 认知层 → 任务接入里调整。
            </div>
            <div v-else class="cog-empty">
              接入的分类里，未来 {{ taskOverview?.windowDays ?? 7 }} 天没有到期任务；无截止日期的条目留在任务管理里。
            </div>
          </n-spin>
        </div>
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
import { computed, nextTick, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
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

// ── 学习看板
const learn = computed(() => store.learningOverview)
const learningRootEl = ref(null)
const weakTopics = computed(() =>
  (store.learningOverview?.topics || []).filter((t) => t.mastery < 0.7).slice(0, 8),
)
const shortTime = (iso) => {
  if (!iso) return ''
  const s = String(iso)
  return s.length > 19 ? s.slice(5, 19).replace('T', ' ') : s
}
const refreshLearning = async () => {
  try {
    await store.refreshLearning(30)
  } catch (e) {
    message.error(String(e))
  }
}

// ── 任务接入
const taskRootEl = ref(null)
const taskCategories = computed(() => store.taskCategories)
const taskOverview = computed(() => store.taskOverview)
const taskAllowed = computed(() => store.taskCategories?.allowed || [])
const todayKey = () => {
  const d = new Date()
  const pad = (n) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}
const shortDate = (iso) => String(iso || '').slice(5)
const taskGroups = computed(() => {
  const items = store.taskOverview?.items || []
  const today = todayKey()
  const overdue = items.filter((t) => t.overdue)
  const dueToday = items.filter((t) => !t.overdue && t.deadline === today)
  const soon = items.filter((t) => !t.overdue && t.deadline !== today)
  return [
    { key: 'overdue', label: '已过期', tone: 'is-overdue', items: overdue },
    { key: 'today', label: '今天', tone: '', items: dueToday },
    { key: 'soon', label: '接下来', tone: '', items: soon },
  ].filter((g) => g.items.length)
})
const toggleTaskCategory = async (key) => {
  const allowed = new Set(store.taskCategories?.allowed || [])
  if (allowed.has(key)) allowed.delete(key)
  else allowed.add(key)
  try {
    await store.setTaskCategories([...allowed])
  } catch (e) {
    message.error(String(e))
  }
}
const openTaskManager = () => {
  window.location.hash = '/task-manager'
}

watch(tab, async (value) => {
  await nextTick()
  // 页签共用滚动容器：切换后回到顶部，避免上一条长列表的 scrollTop 带过来。
  const root =
    value === 'learning' ? learningRootEl.value : value === 'tasks' ? taskRootEl.value : null
  const pane = root?.closest('.n-tab-pane') || root?.parentElement
  if (pane) pane.scrollTop = 0
  if (value === 'learning') {
    void store.refreshLearningIfStale(30)
  } else if (value === 'tasks') {
    if (!store.taskCategories) void store.loadTaskCategories()
    void store.loadTasks()
  }
})

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
let offWindowFocus = null

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
  // 阅读器是独立窗口：从那边读完回来，路由没变，onActivated 不会触发。
  // 主窗口重新获得焦点时，如果正停在「学习」页签，就顺手刷新一次概览。
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    offWindowFocus = await getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload && tab.value === 'learning') {
        void store.refreshLearningIfStale(30, 15_000)
      }
    })
  } catch {
    offWindowFocus = null
  }
})

// 中间区被 <KeepAlive> 缓存：从"读文章"切回大脑页时不会重跑 onMounted，
// 导致收件箱停留在旧数据、看不到刚读/刚抓进来的新内容。onActivated 里重新拉一遍。
onActivated(() => {
  reload()
  // 当前在收件箱页时，顺带刷新感知列表，确保新入库的 percept 立即出现。
  if (tab.value === 'percepts' || tab.value === 'today') {
    store.refreshPercepts()
  }
  if (tab.value === 'learning') {
    void store.refreshLearningIfStale(30)
  }
  if (tab.value === 'tasks') {
    if (!store.taskCategories) void store.loadTaskCategories()
    void store.loadTasks()
  }
})

onUnmounted(() => {
  if (offNudge) offNudge()
  if (offBrief) offBrief()
  if (offNoteRefined) offNoteRefined()
  if (offWindowFocus) offWindowFocus()
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

/* ── 学习看板 ── */
.learn-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin: 12px 0 8px;
}

/* 学习页签的操作行：放在首屏、用 flex 对齐。之前这排按钮落在一个没有样式的
   普通 div 里，图标与文字按 inline 基线排，视觉上就是错位的。 */
.learn-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin: 2px 0 12px;
}

.learn-actions-hint {
  margin-left: auto;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
}

.learn-stat {
  flex: 1 1 120px;
  min-width: 110px;
  padding: 10px 12px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.learn-stat b {
  font-size: 22px;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}

.learn-stat span {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.learn-bands {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin: 6px 0 12px;
  font-size: 12px;
}

.learn-band {
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-secondary);
}

.learn-band.is-weak { color: #d98a2b; }
.learn-band.is-learning { color: #3b82c4; }
.learn-band.is-mastered { color: #5b9e6a; }

.learn-updated {
  margin-left: auto;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.learn-section-title {
  margin: 14px 0 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.learn-topic-list,
.learn-due,
.learn-finished {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.learn-topic {
  display: grid;
  grid-template-columns: minmax(120px, 1.2fr) minmax(80px, 1fr) 44px auto;
  gap: 10px;
  align-items: center;
  font-size: 12.5px;
}

.learn-topic-name,
.learn-due-name,
.learn-finished-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.learn-topic-bar {
  height: 6px;
  border-radius: 999px;
  background: var(--guaxi-hover-bg);
  overflow: hidden;
}

.learn-topic-bar b {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, #e0a13b, #4acadb);
}

.learn-topic-pct {
  text-align: right;
  font-variant-numeric: tabular-nums;
  color: var(--guaxi-text-secondary);
}

.learn-topic-meta {
  color: var(--guaxi-text-secondary);
  font-size: 11.5px;
  white-space: nowrap;
}

.learn-due-row,
.learn-finished-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 5px 0;
  border-bottom: 1px solid var(--guaxi-border);
  font-size: 12.5px;
}

.learn-due-meta,
.learn-finished-meta {
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
  font-size: 11.5px;
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

/* ════════════════════════════════════════════════════════════════
 * 「观测台账本」皮肤（仅大脑页）
 *
 * 这一页的内容都是"按时间发生的记录"：提醒、读完、到期、摘录。卡片会把它
 * 切成互不相关的方块；台账本用横向细线与留白表达同一条时间轴上的条目。
 * 标题用衬线（阅读与记录感），数据与元信息用等宽（仪器感），黄铜色只表示
 * "当前与注意力"，其余一律石墨与纸面。圆角只保留 2px 给标签。
 * ════════════════════════════════════════════════════════════════ */
.cog-inbox {
  --cog-rule: color-mix(in srgb, var(--guaxi-text-primary) 12%, transparent);
  --cog-rule-strong: color-mix(in srgb, var(--guaxi-text-primary) 22%, transparent);
  --cog-brass: #8f7226;
  --cog-alert: #a63d40;
  --cog-wash: color-mix(in srgb, var(--guaxi-text-primary) 4%, transparent);
  --cog-display: 'Noto Serif SC', 'Source Han Serif SC', 'Songti SC', 'SimSun', Georgia, 'Times New Roman', serif;
  --cog-mono: var(--guaxi-editor-font-family);
  padding: 12px 16px;
}
:global(:root[data-guaxi-theme='dark']) .cog-inbox,
:global(html.dark-mode) .cog-inbox {
  --cog-brass: #d2ae63;
  --cog-alert: #d37862;
}

/* ── 顶部信号条：标签压平，指标之间用竖线分隔 ── */
.cog-bar {
  padding: 0 0 10px;
  margin-bottom: 2px;
  border-bottom: 1px solid var(--cog-rule);
  gap: 10px;
}
.cog-bar-left {
  gap: 0;
  font-size: 11.5px;
}
.cog-bar-left :deep(.n-tag) {
  margin-right: 10px;
  border: 1px solid var(--cog-rule-strong);
  border-radius: 2px;
  background: transparent;
  font-family: var(--cog-mono);
  font-size: 10.5px;
}
.cog-metric {
  padding: 0 12px;
  border-left: 1px solid var(--cog-rule);
  line-height: 1.5;
}
.cog-bar-left :deep(.n-tag) + .cog-metric {
  border-left: 0;
  padding-left: 0;
}
.cog-metric b {
  font-family: var(--cog-mono);
  font-size: 13px;
  color: var(--guaxi-text-primary);
  margin: 0 2px;
}
.cog-metric :deep(.n-icon) {
  font-size: 13px;
  margin-right: 4px;
  opacity: 0.55;
}
.cog-bar-right :deep(.n-button) {
  border-radius: 2px;
}

/* ── 页签：细底线 + 黄铜指示条 ── */
.cog-tabs :deep(.n-tabs-nav) {
  border-bottom: 1px solid var(--cog-rule);
}
.cog-tabs :deep(.n-tabs-tab) {
  padding: 6px 2px 9px;
  margin-right: 24px;
  font-size: 13px;
  color: var(--guaxi-text-secondary);
  transition: color 0.15s ease;
}
.cog-tabs :deep(.n-tabs-tab.n-tabs-tab--active) {
  color: var(--guaxi-text-primary);
}
.cog-tabs :deep(.n-tabs-bar) {
  height: 2px;
  background: var(--cog-brass);
}
.cog-tabs :deep(.n-tab-pane) {
  padding-top: 10px;
}

/* ── 今日：日期做刊头，条目是台账行 ── */
.today-head {
  margin: 14px 0 8px;
}
.today-date {
  font-family: var(--cog-display);
  font-size: 22px;
  font-weight: 600;
  letter-spacing: 0.01em;
  line-height: 1.25;
}
.today-meta {
  font-family: var(--cog-mono);
  font-size: 11px;
}
.today-brief {
  align-items: baseline;
  gap: 12px;
  padding: 6px 0 6px 12px;
  margin: 10px 0 14px;
  border: 0;
  border-left: 2px solid var(--cog-brass);
  border-radius: 0;
  background: transparent;
}
.today-brief-label {
  font-family: var(--cog-mono);
  font-size: 10.5px;
  letter-spacing: 0.04em;
}
.today-brief-text {
  font-size: 13px;
  line-height: 1.7;
}
.today-item {
  gap: 12px;
  padding: 13px 0;
  margin: 0;
  border: 0;
  border-bottom: 1px solid var(--cog-rule);
  border-radius: 0;
  background: transparent;
}
.today-item:hover {
  background: var(--cog-wash);
}
.today-dot {
  width: 2px;
  height: 30px;
  border-radius: 0;
  opacity: 0.9;
}
.today-title {
  font-family: var(--cog-display);
  font-size: 15.5px;
  font-weight: 600;
}
.today-sub {
  font-family: var(--cog-mono);
  font-size: 10.5px;
  margin-top: 2px;
}
.today-actions {
  opacity: 0.72;
  transition: opacity 0.15s ease;
}
.today-item:hover .today-actions,
.today-item:focus-within .today-actions {
  opacity: 1;
}
.today-foot {
  justify-content: flex-start;
  padding: 10px 0 14px;
  border-top: 1px solid var(--cog-rule);
}

/* ── 提醒 / 感知：无卡片，只有一条条带日期的记录 ── */
.cog-card {
  border: 0;
  border-bottom: 1px solid var(--cog-rule);
  border-radius: 0;
  padding: 14px 0 13px;
  margin: 0;
}
.cog-card:hover {
  background: var(--cog-wash);
}
.cog-card-title {
  font-family: var(--cog-display);
  font-size: 15.5px;
}
.cog-card-head :deep(.n-tag) {
  border: 1px solid var(--cog-rule);
  border-radius: 2px;
  background: transparent;
  font-family: var(--cog-mono);
  font-size: 10.5px;
}
.cog-card-score {
  font-family: var(--cog-mono);
  font-size: 11.5px;
}
.cog-card-body {
  font-size: 13px;
  line-height: 1.8;
}
.cog-untrusted {
  border-left: 2px solid var(--cog-alert);
  padding-left: 10px;
}
.cog-why {
  border-left: 1px solid var(--cog-rule);
  padding-left: 12px;
  font-size: 12px;
}
.cog-refs {
  gap: 14px;
}
.cog-ref {
  padding: 0;
  border: 0;
  border-bottom: 1px dotted var(--cog-rule-strong);
  border-radius: 0;
  color: var(--guaxi-text-secondary);
  font-family: var(--cog-mono);
  font-size: 11.5px;
}
.cog-ref:hover {
  border-color: var(--cog-brass);
  color: var(--cog-brass);
}
.cog-card-actions {
  margin-top: 10px;
  flex-wrap: wrap;
  opacity: 0.72;
  transition: opacity 0.15s ease;
}
.cog-card:hover .cog-card-actions,
.cog-card:focus-within .cog-card-actions {
  opacity: 1;
}
.cog-card-actions :deep(.n-button),
.cog-actions :deep(.n-button),
.today-actions :deep(.n-button) {
  border-radius: 2px;
}
.cog-meters {
  font-family: var(--cog-mono);
  font-size: 11px;
  gap: 14px;
}

/* ── 学习：统计变成一条仪表带，主题是带刻度的心得清单 ── */
.learn-root {
  padding-bottom: 24px;
}
.learn-stats {
  flex-wrap: nowrap;
  gap: 0;
  margin: 14px 0 8px;
  border-top: 1px solid var(--cog-rule);
  border-bottom: 1px solid var(--cog-rule);
}
.learn-stat {
  flex: 1 1 0;
  min-width: 0;
  padding: 12px 16px;
  border: 0;
  border-radius: 0;
  background: transparent;
}
.learn-stat + .learn-stat {
  border-left: 1px solid var(--cog-rule);
}
.learn-stat b {
  font-family: var(--cog-mono);
  font-size: 26px;
  font-weight: 500;
  letter-spacing: -0.01em;
}
.learn-stat span {
  font-size: 11px;
}
.learn-bands {
  gap: 16px;
  margin: 8px 0 2px;
  font-family: var(--cog-mono);
  font-size: 10.5px;
}
.learn-band {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0;
  border-radius: 0;
  background: none;
}
.learn-band::before {
  content: '';
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.72;
}
.learn-updated {
  font-size: 10.5px;
}
.learn-actions {
  margin: 8px 0 16px;
}
.learn-actions :deep(.n-button) {
  border-radius: 2px;
}
.learn-actions-hint {
  font-family: var(--cog-mono);
  font-size: 10.5px;
}
.learn-section-title {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 20px 0 8px;
  font-family: var(--cog-display);
  font-size: 15px;
  font-weight: 600;
}
.learn-section-title::after {
  content: '';
  flex: 1 1 auto;
  height: 1px;
  background: var(--cog-rule);
}
.learn-topic-name {
  font-family: var(--cog-display);
  font-size: 13.5px;
}
.learn-topic-bar {
  height: 3px;
  border-radius: 0;
  background: var(--cog-rule);
}
.learn-topic-bar b {
  border-radius: 0;
  background: var(--cog-brass);
  transition: width 0.4s ease;
}
.learn-topic-pct {
  font-family: var(--cog-mono);
  font-size: 11.5px;
}
.learn-topic-meta {
  font-family: var(--cog-mono);
  font-size: 10.5px;
}
.learn-due-row,
.learn-finished-row {
  border-bottom: 1px solid var(--cog-rule);
}
.learn-due-name,
.learn-finished-title {
  font-family: var(--cog-display);
  font-size: 13.5px;
}
.learn-due-meta,
.learn-finished-meta {
  font-family: var(--cog-mono);
  font-size: 10.5px;
}

/* ── 任务：分类是开关，条目与学习同一套台账行 ── */
.task-root {
  padding-bottom: 24px;
}
.task-intro {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  margin: 14px 0 10px;
}
.task-cats {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.task-cat {
  appearance: none;
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 10px;
  border: 1px solid var(--cog-rule-strong);
  border-radius: 2px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: var(--cog-mono);
  font-size: 11px;
  cursor: pointer;
}
.task-cat i {
  font-style: normal;
  font-size: 10px;
  opacity: 0.65;
}
.task-cat:hover {
  border-color: var(--cog-brass);
}
.task-cat.on {
  border-color: var(--cog-brass);
  color: var(--guaxi-text-primary);
  box-shadow: inset 0 -2px 0 var(--cog-brass);
}
.task-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--cog-rule);
  font-family: var(--cog-mono);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}
.task-summary b {
  margin-right: 4px;
  font-size: 15px;
  font-weight: 500;
  color: var(--guaxi-text-primary);
}
.task-group-title {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 18px 0 4px;
  font-family: var(--cog-display);
  font-size: 14.5px;
  font-weight: 600;
}
.task-group-title::after {
  content: '';
  flex: 1 1 auto;
  height: 1px;
  background: var(--cog-rule);
}
.task-group-title i {
  font-family: var(--cog-mono);
  font-style: normal;
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}
.task-group-title.is-overdue {
  color: var(--cog-alert);
}
.task-row {
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr) auto;
  gap: 14px;
  align-items: baseline;
  padding: 9px 0;
  border-bottom: 1px solid var(--cog-rule);
}
.task-row:hover {
  background: var(--cog-wash);
}
.task-date {
  font-family: var(--cog-mono);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
}
.task-title {
  font-family: var(--cog-display);
  font-size: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.task-title.overdue {
  color: var(--cog-alert);
}
.task-meta {
  font-family: var(--cog-mono);
  font-size: 10.5px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
}

/* ── 空状态：给方向，不营造气氛 ── */
.cog-empty {
  padding: 16px 0;
  text-align: left;
  font-family: var(--cog-mono);
  font-size: 11.5px;
  line-height: 1.8;
  color: var(--guaxi-text-secondary);
  border-bottom: 1px solid var(--cog-rule);
}

/* ── 一键就绪引导：与台账同一套边线 ── */
.cog-setup {
  border: 1px solid var(--cog-rule);
  border-left: 2px solid var(--cog-brass);
  border-radius: 0;
  background: transparent;
}
.cog-setup-icon {
  color: var(--cog-brass);
}

@media (max-width: 720px) {
  .learn-stats {
    flex-wrap: wrap;
  }
  .learn-stat {
    flex-basis: 45%;
  }
  .learn-stat + .learn-stat {
    border-left: 0;
  }
  .task-row {
    grid-template-columns: 52px minmax(0, 1fr);
  }
  .task-meta {
    grid-column: 2;
  }
}

@media (prefers-reduced-motion: reduce) {
  .learn-topic-bar b {
    transition: none;
  }
}
</style>
