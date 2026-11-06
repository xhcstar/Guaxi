<template>
  <div class="task-manager-page">
    <!-- 顶部工具栏 -->
    <header class="tm-toolbar">
      <div class="tm-toolbar-left">
        <n-icon size="22" class="tm-logo"><ClipboardTaskListLtr20Filled /></n-icon>
        <span class="tm-title">任务管理</span>
        <n-tabs v-model:value="category" type="segment" size="small" class="tm-cat-tabs" @update:value="onCategoryChange">
          <n-tab name="all">全部</n-tab>
          <n-tab name="work">工作</n-tab>
          <n-tab name="life">生活</n-tab>
          <n-tab name="study">学习</n-tab>
        </n-tabs>
      </div>
      <div class="tm-toolbar-right">
        <span class="tm-sync" :title="'最后同步 ' + (lastSync || '—')">已同步 {{ lastSync || '—' }}</span>
        <n-input ref="searchInputRef" v-model:value="keyword" placeholder="搜索任务…" clearable size="small" class="tm-search" @update:value="onSearchInput">
          <template #prefix><n-icon :component="Search20Regular" /></template>
        </n-input>
        <n-button-group size="small" class="tm-view-switch">
          <n-button :type="view === 'today' ? 'primary' : 'default'" @click="view = 'today'">
            <template #icon><n-icon :component="ClipboardTaskListLtr20Filled" /></template>
            今日
          </n-button>
          <n-button :type="view === 'plan' ? 'primary' : 'default'" @click="view = 'plan'">
            <template #icon><n-icon :component="Board20Regular" /></template>
            计划
          </n-button>
          <n-button :type="view === 'year' ? 'primary' : 'default'" @click="view = 'year'">
            <template #icon><n-icon :component="DataPie20Regular" /></template>
            年轮
          </n-button>
        </n-button-group>
        <n-popover trigger="click" placement="bottom-end" :show-arrow="false">
          <template #trigger>
            <n-button size="small" circle :type="reminderEnabled ? 'primary' : 'default'" title="每日摘要提醒">
              <template #icon><n-icon :component="Alert20Regular" /></template>
            </n-button>
          </template>
          <div class="tm-reminder-pop">
            <div class="tm-reminder-row">
              <span>每日摘要</span>
              <n-switch v-model:value="reminderEnabled" size="small" @update:value="saveReminder" />
            </div>
            <div class="tm-reminder-row">
              <span>发送时间</span>
              <n-time-picker
                v-model:formatted-value="reminderTime"
                value-format="HH:mm"
                format="HH:mm"
                size="small"
                :disabled="!reminderEnabled"
                @update:formatted-value="saveReminder"
              />
            </div>
            <div class="tm-reminder-hint">逾期与今天到期的任务会合成一条通知，每天只发一次。</div>
          </div>
        </n-popover>
        <n-button size="small" circle title="导入任务" @click="showImport = true">
          <template #icon><n-icon :component="ArrowImport20Regular" /></template>
        </n-button>
        <n-button size="small" circle title="导出报表" @click="openExport">
          <template #icon><n-icon :component="ArrowExportLtr20Regular" /></template>
        </n-button>
      </div>
    </header>

    <main class="tm-body">
        <!-- 今日 -->
        <section v-show="view === 'today'" class="tm-today">
          <div class="tm-today-main">
            <div class="tm-today-head">
              <div class="tm-today-date">
                <span class="tm-today-day">{{ todayParts.day }}</span>
                <span class="tm-today-meta">
                  <b>{{ todayParts.weekday }}</b>
                  <span>{{ todayParts.yearMonth }}</span>
                </span>
              </div>
              <div class="tm-today-counts">
                <span><b>{{ todayDue.length }}</b> 今天</span>
                <span v-if="overdueItems.length" class="danger"><b>{{ overdueItems.length }}</b> 逾期</span>
                <span><b>{{ doneToday.length }}</b> 完成</span>
              </div>
            </div>

            <form class="tm-capture" @submit.prevent="quickCapture">
              <span class="tm-capture-plus">＋</span>
              <input
                v-model="captureText"
                class="tm-capture-input"
                placeholder="记一件…  支持 明天 / @生活 / !A / #习惯"
              />
              <select v-model="captureCategory" class="tm-capture-sel">
                <option value="work">工作</option>
                <option value="life">生活</option>
                <option value="study">学习</option>
              </select>
              <button class="tm-capture-btn" :disabled="!captureText.trim() || capturing">
                {{ capturing ? '…' : '记下' }}
              </button>
            </form>
            <div v-if="captureHint" class="tm-capture-hint">{{ captureHint }}</div>

            <div v-if="focusItems.length" class="tm-block">
              <div class="tm-block-head"><span>今天的重点</span><span class="tm-block-sub">{{ focusItems.length }}</span></div>
              <div
                v-for="(it, idx) in focusItems"
                :key="it.id"
                class="tm-row tm-row-focus"
                :class="{ selected: selectedId === it.id }"
                :data-task-id="it.id"
                @click="openDetail(it)"
              >
                <span class="tm-focus-no">{{ idx + 1 }}</span>
                <button class="tm-check" :class="{ on: it.done }" title="完成" @click.stop="toggleDone(it)">✓</button>
                <div class="tm-row-main">
                  <div class="tm-row-title">{{ it.title }}</div>
                  <div class="tm-row-meta">{{ rowMeta(it) }}</div>
                </div>
                <button class="tm-row-act" title="移出今日" @click.stop="unfocus(it)">×</button>
              </div>
            </div>

            <div v-if="overdueItems.length" class="tm-block">
              <div class="tm-block-head"><span class="danger">逾期</span><span class="tm-block-sub">{{ overdueItems.length }}</span></div>
              <div
                v-for="it in overdueItems"
                :key="it.id"
                class="tm-row"
                :class="{ overdue: true, selected: selectedId === it.id }"
                :data-task-id="it.id"
                @click="openDetail(it)"
              >
                <button class="tm-check" :class="{ on: it.done }" title="完成" @click.stop="toggleDone(it)">✓</button>
                <div class="tm-row-main">
                  <div class="tm-row-title">{{ it.title }}</div>
                  <div class="tm-row-meta">{{ rowMeta(it) }}</div>
                </div>
                <button class="tm-row-act" title="顺延到明天" @click.stop="snooze(it)">→</button>
              </div>
            </div>

            <div class="tm-block">
              <div class="tm-block-head"><span>今天</span><span class="tm-block-sub">{{ todayDue.length }}</span></div>
              <div v-if="!todayDue.length" class="tm-hint">今天没有到期任务，按 N 记一件。</div>
              <div
                v-for="it in todayDue"
                :key="it.id"
                class="tm-row"
                :class="{ selected: selectedId === it.id }"
                :data-task-id="it.id"
                @click="openDetail(it)"
              >
                <button class="tm-check" :class="{ on: it.done }" title="完成" @click.stop="toggleDone(it)">✓</button>
                <div class="tm-row-main">
                  <div class="tm-row-title">{{ it.title }}</div>
                  <div class="tm-row-meta">{{ rowMeta(it) }}</div>
                </div>
                <button class="tm-row-act" title="顺延到明天" @click.stop="snooze(it)">→</button>
              </div>
            </div>

            <div v-if="todayDoing.length" class="tm-block">
              <div class="tm-block-head"><span>进行中</span><span class="tm-block-sub">{{ todayDoing.length }}</span></div>
              <div
                v-for="it in todayDoing"
                :key="it.id"
                class="tm-row"
                :class="{ selected: selectedId === it.id }"
                :data-task-id="it.id"
                @click="openDetail(it)"
              >
                <button class="tm-check" :class="{ on: it.done }" title="完成" @click.stop="toggleDone(it)">✓</button>
                <div class="tm-row-main">
                  <div class="tm-row-title">{{ it.title }}</div>
                  <div class="tm-row-meta">{{ rowMeta(it) }}</div>
                </div>
              </div>
            </div>

            <div v-if="todayHabits.length" class="tm-block">
              <div class="tm-block-head"><span>今日习惯</span><span class="tm-block-sub">{{ habitDoneCount }}/{{ todayHabits.length }}</span></div>
              <div class="tm-habits">
                <button
                  v-for="h in todayHabits"
                  :key="h.id"
                  class="tm-habit"
                  :class="{ done: isCheckedToday(h) }"
                  @click="toggleHabit(h)"
                >
                  <span class="tm-habit-mark">{{ isCheckedToday(h) ? '✓' : '○' }}</span>
                  <span class="tm-habit-name">{{ h.title }}</span>
                  <span class="tm-habit-streak">{{ habitStreak(h) }} 天</span>
                </button>
              </div>
            </div>

            <div class="tm-block">
              <div class="tm-block-head"><span>今日复盘</span></div>
              <div v-if="todayReview" class="tm-review" @click="openDetail(todayReview)">
                <div class="tm-review-text">{{ reviewExcerpt(todayReview) }}</div>
                <span class="tm-review-edit">继续写</span>
              </div>
              <button v-else class="tm-review-start" @click="startReview">写三行，今天就算收尾了</button>
            </div>
          </div>

          <aside class="tm-today-side">
            <div class="tm-side-card">
              <div class="tm-side-title">本周</div>
              <div class="tm-side-stats">
                <div><b>{{ weekStats.done }}</b><span>完成</span></div>
                <div><b>{{ weekStats.checkins }}</b><span>打卡</span></div>
                <div><b>{{ formatSpent(weekStats.spent) }}</b><span>投入</span></div>
              </div>
            </div>
            <div class="tm-side-card">
              <div class="tm-side-title">最近完成</div>
              <div v-if="!recentDone.length" class="tm-hint">还没有完成记录。</div>
              <div v-for="it in recentDone" :key="it.id" class="tm-recent" @click="openDetail(it)">
                <span class="tm-recent-title">{{ it.title }}</span>
                <span class="tm-recent-date">{{ shortDate(it.completedIso) }}</span>
              </div>
            </div>
            <div class="tm-side-card">
              <div class="tm-side-title">提醒</div>
              <div class="tm-side-reminder">{{ reminderEnabled ? '每天 ' + reminderTime + ' 摘要' : '已关闭' }}</div>
              <div class="tm-side-hint">逾期与今天到期合成一条，不逐条打扰。</div>
            </div>
          </aside>
        </section>

        <!-- 计划 -->
        <section v-show="view === 'plan'" class="tm-plan">
          <div class="tm-plan-bar">
            <n-button-group size="small">
              <n-button :type="planMode === 'week' ? 'primary' : 'default'" @click="setPlanMode('week')">本周</n-button>
              <n-button :type="planMode === 'kanban' ? 'primary' : 'default'" @click="setPlanMode('kanban')">看板</n-button>
              <n-button :type="planMode === 'calendar' ? 'primary' : 'default'" @click="setPlanMode('calendar')">日历</n-button>
              <n-button :type="planMode === 'goals' ? 'primary' : 'default'" @click="setPlanMode('goals')">目标</n-button>
            </n-button-group>
            <select v-model="captureCategory" class="tm-capture-sel" title="新建分类">
              <option value="work">工作</option>
              <option value="life">生活</option>
              <option value="study">学习</option>
            </select>
            <input
              v-model="captureText"
              class="tm-plan-capture"
              :placeholder="planCapturePlaceholder"
              @keydown.enter.prevent="quickCapture"
            />
            <span class="tm-plan-hint">{{ planMode === 'week' ? '按截止日期自动归入本周 / 下周 · J/K 选择 · Enter 打开 · Space 完成' : planMode === 'goals' ? '目标进度由子任务完成比自动汇总' : '卡片内可直接改状态/截止 · J/K 选择 · Enter 打开 · Space 完成' }}</span>
          </div>

          <div v-if="planMode === 'week'" class="tm-week">
            <section v-for="lane in weekLanes" :key="lane.key" class="tm-week-lane">
              <header class="tm-week-head" :class="lane.key">
                <span>{{ lane.label }}</span>
                <span class="tm-block-sub">{{ lane.items.length }}</span>
              </header>
              <div class="tm-week-body">
                <div
                  v-for="it in lane.items"
                  :key="it.id"
                  class="tm-card"
                  :class="{ overdue: lane.key === 'overdue', selected: selectedId === it.id }"
                  :data-task-id="it.id"
                  @click="openDetail(it)"
                >
                  <div class="tm-card-top">
                    <span v-if="it.priority" class="tm-pri" :class="'pri-' + it.priority">{{ it.priority }}</span>
                    <span class="tm-card-title">{{ it.title }}</span>
                    <span class="tm-card-acts">
                      <button title="完成" @click.stop="toggleDone(it)">✓</button>
                      <button title="明天" @click.stop="snooze(it)">→</button>
                    </span>
                  </div>
                  <div class="tm-card-meta">
                    <span class="tm-cat" :class="'cat-' + it.category">{{ catLabel(it.category) }}</span>
                    <span v-if="it.kind !== 'task'" class="tm-kind">{{ kindLabel(it.kind) }}</span>
                    <span v-if="it.deadlineIso" class="tm-date deadline">{{ shortDate(it.deadlineIso) }}</span>
                    <span v-if="it.spent" class="tm-did">{{ formatSpent(it.spent) }}</span>
                  </div>
                </div>
                <div v-if="!lane.items.length" class="tm-empty-col">{{ lane.empty }}</div>
              </div>
            </section>
          </div>

          <div v-show="planMode === 'kanban'" class="tm-kanban">
            <div v-for="col in columns" :key="col.key" class="tm-column">
              <div class="tm-column-head" :style="{ borderColor: col.color }">
                <span class="tm-col-dot" :style="{ background: col.color }" />
                <span class="tm-col-name">{{ col.label }}</span>
                <n-tag size="tiny" round :bordered="false" class="tm-col-count">{{ grouped[col.key].length }}</n-tag>
                <button
                  v-if="col.key === 'done' && doneCount > 0"
                  class="tm-col-more"
                  @click.stop="toggleDoneCollapse"
                >{{ showAllDone ? '收起' : `全部 ${doneCount}` }}</button>
                <button class="tm-col-add" :title="'新建到' + col.label" @click="newTask(col.key)">＋</button>
              </div>
              <draggable
                :list="grouped[col.key]"
                group="tasks"
                item-key="id"
                class="tm-cards"
                :animation="160"
                @change="(e) => onDragChange(col.key, e)"
              >
                <template #item="{ element }">
                  <div
                    class="tm-card"
                    :class="{ overdue: isOverdue(element), selected: selectedId === element.id }"
                    :data-task-id="element.id"
                    @click="openDetail(element)"
                  >
                    <div class="tm-card-top">
                      <span v-if="element.priority" class="tm-pri" :class="'pri-' + element.priority">{{ element.priority }}</span>
                      <input
                        v-if="renamingId === element.id"
                        v-model="renameDraft"
                        class="tm-card-rename"
                        @keydown.enter.prevent="commitRename(element)"
                        @keydown.esc.prevent="renamingId = ''"
                        @blur="commitRename(element)"
                        @click.stop
                      />
                      <span v-else class="tm-card-title" @dblclick.stop="inlineRename(element)">{{ element.title || '(无标题)' }}</span>
                      <span class="tm-card-acts">
                        <button title="完成" @click.stop="toggleDone(element)">✓</button>
                        <button title="明天" @click.stop="snooze(element)">→</button>
                      </span>
                    </div>
                    <div class="tm-card-meta">
                      <span class="tm-cat" :class="'cat-' + element.category">{{ catLabel(element.category) }}</span>
                      <span v-if="element.kind !== 'task'" class="tm-kind">{{ kindLabel(element.kind) }}</span>
                      <span class="tm-type" :class="'tt-' + element.taskType">{{ typeLabel(element.taskType) }}</span>
                      <span class="tm-status-wrap" @click.stop>
                        <span class="tm-status-chip" :class="'st-' + element.status">{{ statusZh(element.status) }}</span>
                        <select
                          class="tm-status-overlay"
                          :value="element.status"
                          title="快速改状态"
                          @change="(e) => setStatus(element, e.target.value)"
                        >
                          <option v-for="s in statusKeys" :key="s" :value="s">{{ statusZh(s) }}</option>
                        </select>
                      </span>
                      <span class="tm-date-wrap" :class="{ empty: !element.deadlineIso }" @click.stop="openDatePicker">
                        <span class="tm-date-chip"><n-icon :component="Flag16Regular" />{{ element.deadlineIso ? shortDate(element.deadlineIso) : '＋截止' }}</span>
                        <input
                          type="date"
                          class="tm-date-overlay"
                          :value="element.deadlineIso || ''"
                          title="设置截止日期"
                          @change="(e) => setDeadline(element, e.target.value)"
                        />
                        <button v-if="element.deadlineIso" class="tm-date-clear" title="清除截止" @click.stop="clearDeadline(element)">×</button>
                      </span>
                      <span v-if="element.spent" class="tm-did">{{ formatSpent(element.spent) }}</span>
                      <span v-if="element.displayId" class="tm-did">{{ element.displayId }}</span>
                      <button
                        v-if="mailOriginOf(element)"
                        class="tm-mail-origin"
                        title="回到原邮件"
                        @click.stop="openMailOrigin(element)"
                      >
                        ✉ 邮件
                      </button>
                    </div>
                  </div>
                </template>
              </draggable>
              <div v-if="!grouped[col.key].length" class="tm-empty-col">暂无任务</div>
            </div>
          </div>

          <div v-show="planMode === 'calendar'" class="tm-calendar">
            <FullCalendar ref="calRef" :options="calendarOptions" />
          </div>

          <div v-if="planMode === 'goals'" class="tm-goals">
            <div class="tm-goals-head">
              <span>目标进度</span>
              <span class="tm-spacer2" />
              <n-button size="small" @click="newGoal">＋ 新建目标</n-button>
            </div>
            <div v-if="!goals.length" class="tm-hint">还没有目标。新建一个目标，再把任务填到它的「父任务」里，进度会自动汇总。</div>
            <div v-for="g in goals" :key="g.id" class="tm-goal" @click="openDetail(g)">
              <div class="tm-goal-head">
                <span class="tm-goal-title">{{ g.title }}</span>
                <span class="tm-goal-pct">{{ goalProgress(g).pct }}%</span>
              </div>
              <div class="tm-goal-bar"><span :style="{ width: goalProgress(g).pct + '%' }" /></div>
              <div class="tm-goal-meta">{{ goalProgress(g).done }}/{{ goalProgress(g).total }} 个子任务 · {{ catLabel(g.category) }}</div>
              <div v-if="goalProgress(g).total" class="tm-goal-children">
                <span
                  v-for="c in goalChildren(g)"
                  :key="c.id"
                  class="tm-goal-child"
                  :class="{ done: c.done }"
                  @click.stop="openDetail(c)"
                >{{ c.title }}</span>
              </div>
            </div>
          </div>
        </section>

        <!-- 日历悬浮详情卡片 -->
        <Teleport to="body">
          <div
            v-if="hover.visible && hover.item"
            class="tm-cal-tip"
            :class="isDark ? 'dark' : 'light'"
            :style="{ left: hover.x + 'px', top: hover.y + 'px' }"
          >
            <div class="tip-head">
              <span class="tip-type" :class="'tt-' + hover.item.taskType">{{ typeLabel(hover.item.taskType) }}</span>
              <span v-if="hover.item.priority" class="tip-pri" :class="'pri-' + hover.item.priority">#{{ hover.item.priority }}</span>
              <span class="tip-title">{{ hover.item.title }}</span>
            </div>
            <div class="tip-row"><span class="tip-k">状态</span><span class="tip-v">{{ statusZh(hover.item.status) }}</span></div>
            <div v-if="hover.item.displayId" class="tip-row"><span class="tip-k">ID</span><span class="tip-v">{{ hover.item.displayId }}</span></div>
            <div class="tip-row"><span class="tip-k">分类</span><span class="tip-v">{{ catLabel(hover.item.category) }}</span></div>
            <div v-if="hover.item.severity" class="tip-row"><span class="tip-k">严重度</span><span class="tip-v">{{ hover.item.severity }}</span></div>
            <div v-if="hover.item.deadline" class="tip-row"><span class="tip-k">截止</span><span class="tip-v">{{ hover.item.deadline }}</span></div>
            <div v-if="hover.item.createTime" class="tip-row"><span class="tip-k">创建</span><span class="tip-v">{{ hover.item.createTime }}</span></div>
          </div>
        </Teleport>

        <!-- 年轮 -->
        <section v-show="view === 'year'" class="tm-year">
          <div class="tm-year-head">
            <span class="tm-stats-title">年轮</span>
            <span class="tm-year-sub">
              <button class="tm-year-nav" title="上一年" @click="heatmapYear -= 1">‹</button>
              <b>{{ heatmapYear }}</b>
              <button class="tm-year-nav" title="下一年" :disabled="heatmapYear >= currentYear" @click="heatmapYear += 1">›</button>
              · 长期记录与成长
            </span>
            <span class="tm-spacer2" />
            <n-button size="small" type="primary" ghost @click="openFieldExport">
              <template #icon><n-icon :component="ArrowExportLtr20Regular" /></template>
              按字段筛选导出
            </n-button>
            <n-button size="small" @click="openExport">生成报表</n-button>
          </div>

          <div class="tm-year-grid">
            <div class="tm-year-main">
              <div class="tm-year-card">
                <TaskYearHeatmap :items="items" :year="heatmapYear" @select="onSelectDay" />
                <div v-if="selectedDay" class="tm-day-detail">
                  <div class="tm-day-head">
                    <span>{{ selectedDay }}</span>
                    <span class="tm-muted">{{ dayDetail.done.length }} 完成 · {{ dayDetail.checkins.length }} 打卡</span>
                  </div>
                  <div v-for="it in dayDetail.done" :key="it.id" class="tm-day-row" @click="openDetail(it)">
                    <span class="tm-day-dot" />{{ it.title }}
                  </div>
                  <div v-for="h in dayDetail.checkins" :key="'h-' + h.id" class="tm-day-row habit">
                    <span class="tm-day-dot" />{{ h.title }} 打卡
                  </div>
                  <div v-if="!dayDetail.done.length && !dayDetail.checkins.length" class="tm-hint">这一天没有记录。</div>
                </div>
              </div>

              <div class="tm-year-stats">
                <div class="tm-stat-card">
                  <div class="tm-stat-num">{{ items.length }}</div>
                  <div class="tm-stat-label">任务总数</div>
                </div>
                <div class="tm-stat-card">
                  <div class="tm-stat-num" style="color:#18a058">{{ yearDone }}</div>
                  <div class="tm-stat-label">本年完成</div>
                </div>
                <div class="tm-stat-card">
                  <div class="tm-stat-num" style="color:#3f6ea8">{{ doingCount }}</div>
                  <div class="tm-stat-label">进行中</div>
                </div>
                <div class="tm-stat-card">
                  <div class="tm-stat-num">{{ completionRate }}%</div>
                  <div class="tm-stat-label">完成率</div>
                </div>
                <div v-if="showBugStat" class="tm-stat-card">
                  <div class="tm-stat-num" style="color:#d03050">{{ bugCount }}</div>
                  <div class="tm-stat-label">Bug 数</div>
                </div>
              </div>

              <div class="tm-year-card">
                <div class="tm-side-title">分类完成情况</div>
                <div class="tm-cat-bars">
                  <div v-for="c in categoryStats" :key="c.key" class="tm-cat-bar">
                    <span class="tm-cat-bar-label">{{ c.label }}</span>
                    <span class="tm-cat-bar-track"><i :style="{ width: c.pct + '%' }" /></span>
                    <span class="tm-cat-bar-num">{{ c.done }}/{{ c.total }}</span>
                  </div>
                </div>
              </div>
            </div>

            <aside class="tm-year-side">
              <div class="tm-side-card">
                <div class="tm-side-title">习惯</div>
                <div v-if="!habits.length" class="tm-hint">还没有习惯。新建一个「形态=习惯」的任务即可。</div>
                <div v-for="h in habits" :key="h.id" class="tm-habit-row" @click="openDetail(h)">
                  <span class="tm-habit-name">{{ h.title }}</span>
                  <span class="tm-habit-streak">{{ habitStreak(h) }} 天</span>
                </div>
              </div>
              <div class="tm-side-card">
                <div class="tm-side-title">目标</div>
                <div v-if="!goals.length" class="tm-hint">还没有目标。</div>
                <div v-for="g in goals" :key="g.id" class="tm-goal-mini" @click="openDetail(g)">
                  <div class="tm-goal-mini-head">
                    <span>{{ g.title }}</span>
                    <span>{{ goalProgress(g).pct }}%</span>
                  </div>
                  <div class="tm-goal-bar"><span :style="{ width: goalProgress(g).pct + '%' }" /></div>
                </div>
              </div>
              <div class="tm-side-card">
                <div class="tm-side-title">最近复盘</div>
                <div v-if="!reviews.length" class="tm-hint">还没有复盘记录。</div>
                <div v-for="r in reviews.slice(0, 6)" :key="r.id" class="tm-review-mini" @click="openDetail(r)">
                  <span class="tm-review-date">{{ shortDate(r.completedIso) }}</span>
                  <span class="tm-review-excerpt">{{ reviewExcerpt(r) }}</span>
                </div>
              </div>
            </aside>
          </div>
        </section>

        <div v-if="loading" class="tm-loading-overlay"><n-spin :size="28" /></div>
    </main>

    <!-- 导入抽屉 -->
    <n-drawer v-model:show="showImport" :width="460" placement="right">
      <n-drawer-content title="导入任务" closable>
        <n-form label-placement="top" size="small">
          <n-form-item label="目标分类">
            <n-select v-model:value="importForm.category" :options="categoryOptions" />
          </n-form-item>
          <n-form-item label="目标文件名">
            <n-input v-model:value="importForm.target_file_name" placeholder="inbox.org" />
          </n-form-item>
          <n-form-item label="源格式">
            <n-radio-group v-model:value="importForm.source_kind">
              <n-radio value="org">Org 文本</n-radio>
              <n-radio value="json">JSON (TaskItem[])</n-radio>
            </n-radio-group>
          </n-form-item>
          <n-form-item label="内容">
            <n-input v-model:value="importForm.content" type="textarea" :autosize="{ minRows: 6, maxRows: 16 }" placeholder="粘贴 .org 文本或 JSON…" />
          </n-form-item>
          <n-button size="small" tertiary @click="pickImportFile">从文件读取…</n-button>
        </n-form>
        <template #footer>
          <n-space>
            <n-button @click="showImport = false">取消</n-button>
            <n-button type="primary" :loading="importing" @click="doImport">导入并扫描</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <!-- 导出抽屉 -->
    <n-drawer v-model:show="showExport" :width="640" placement="right">
      <n-drawer-content title="导出报表" closable>
        <n-form label-placement="left" label-width="auto" size="small" class="tm-export-form">
          <n-form-item label="报表范围">
            <n-select v-model:value="exportForm.scope" :options="reportScopeOptions" style="width:160px" @update:value="genPreview" />
          </n-form-item>
          <n-form-item label="报表类型">
            <n-select v-model:value="exportForm.kind" :options="reportKindOptions" style="width:160px" @update:value="genPreview" />
          </n-form-item>
          <n-form-item label="开始日期">
            <n-date-picker v-model:formatted-value="exportForm.from" value-format="yyyy-MM-dd" type="date" clearable style="width:200px" placeholder="默认按类型回溯" @update:formatted-value="genPreview" />
          </n-form-item>
          <n-form-item label="结束日期">
            <n-date-picker v-model:formatted-value="exportForm.to" value-format="yyyy-MM-dd" type="date" clearable style="width:200px" placeholder="默认今天" @update:formatted-value="genPreview" />
          </n-form-item>
        </n-form>
        <div class="tm-export-preview-wrap">
          <div class="tm-export-preview-label">报表内容（纯文本，可编辑）</div>
          <textarea v-model="exportPreview" class="tm-export-textarea" placeholder="点击「生成」后在此编辑纯文本报表…" spellcheck="false" />
        </div>
        <template #footer>
          <n-space>
            <n-button @click="genPreviewNow" :loading="exporting">生成</n-button>
            <n-button type="primary" :disabled="!exportPreview" @click="saveReport">保存到文件…</n-button>
            <n-button :disabled="!exportPreview" @click="copyReport">复制</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <!-- 按字段筛选导出 -->
    <n-drawer v-model:show="showFieldExport" :width="760" placement="right">
      <n-drawer-content title="按字段筛选导出" closable>
        <div class="tm-fx">
          <div class="tm-fx-filters">
            <div class="tm-fx-fields-head"><span>快捷筛选</span></div>
            <div class="tm-fx-row">
              <label>分类
                <n-select v-model:value="fxFilter.category" size="small" :options="fxCategoryOptions" @update:value="recomputeFx" />
              </label>
              <label>类型
                <n-select v-model:value="fxFilter.taskType" size="small" :options="fxTypeOptions" @update:value="recomputeFx" />
              </label>
              <label>状态
                <n-select v-model:value="fxFilter.status" size="small" :options="fxStatusOptions" @update:value="recomputeFx" />
              </label>
              <label>严重度
                <n-select v-model:value="fxFilter.severity" size="small" :options="fxSeverityOptions" @update:value="recomputeFx" />
              </label>
              <label>优先级
                <n-select v-model:value="fxFilter.priority" size="small" :options="fxPriorityOptions" @update:value="recomputeFx" />
              </label>
            </div>
            <div class="tm-fx-row">
              <label>创建起<n-date-picker v-model:formatted-value="fxFilter.createFrom" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>创建止<n-date-picker v-model:formatted-value="fxFilter.createTo" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>完成起<n-date-picker v-model:formatted-value="fxFilter.completedFrom" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>完成止<n-date-picker v-model:formatted-value="fxFilter.completedTo" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label class="grow">描述含<n-input v-model:value="fxFilter.keyword" size="small" clearable placeholder="关键字" @update:value="recomputeFx" /></label>
            </div>

            <!-- 字段条件：覆盖全部默认字段与自定义字段（来源/修复状态/进度/根本原因/影响版本…） -->
            <div class="tm-fx-conds">
              <div class="tm-fx-fields-head">
                <span>字段条件</span>
                <span class="tm-spacer2" />
                <a @click="addCond">+ 添加条件</a>
                <template v-if="fxConds.length"> · <a @click="clearConds">清空</a></template>
              </div>
              <div v-for="(c, i) in fxConds" :key="c.uid" class="tm-fx-cond">
                <n-select v-model:value="c.field" size="small" class="fx-cond-field" :options="fxFieldOptions" placeholder="字段" @update:value="(v) => onCondFieldChange(c, v)" />
                <n-select v-model:value="c.op" size="small" class="fx-cond-op" :options="condOpOptions(c)" @update:value="recomputeFx" />
                <template v-if="!condNoValue(c)">
                  <n-select v-if="condMulti(c)" v-model:value="c.values" size="small" multiple class="fx-cond-val" :options="condEnumOptions(c)" placeholder="多个值" @update:value="recomputeFx" />
                  <n-select v-else-if="condEnumSingle(c)" v-model:value="c.value" size="small" clearable class="fx-cond-val" :options="condEnumOptions(c)" placeholder="值" @update:value="recomputeFx" />
                  <template v-else-if="condIsDate(c)">
                    <n-date-picker v-model:formatted-value="c.value" value-format="yyyy-MM-dd" type="date" size="small" clearable class="fx-cond-date" placeholder="日期" @update:formatted-value="recomputeFx" />
                    <n-date-picker v-if="c.op === 'range'" v-model:formatted-value="c.value2" value-format="yyyy-MM-dd" type="date" size="small" clearable class="fx-cond-date" placeholder="至" @update:formatted-value="recomputeFx" />
                  </template>
                  <n-input v-else v-model:value="c.value" size="small" clearable class="fx-cond-val" placeholder="值" @update:value="recomputeFx" />
                </template>
                <n-button size="tiny" quaternary title="删除条件" @click="removeCond(i)">✕</n-button>
              </div>
              <div v-if="!fxConds.length" class="tm-fx-hint">
                可对任意默认字段（来源 / 修复状态 / 进度 / 根本原因 / 影响版本 / 复现步骤…）与自定义字段添加条件。
              </div>
            </div>
          </div>

          <div class="tm-fx-stat">
            匹配 <b>{{ fxMatched.length }}</b> 条 · Bug {{ fxCount('bug') }} / 任务 {{ fxCount('task') }} / 其他 {{ fxCount('other') }} · 已完成 {{ fxDoneCount }} / 未完成 {{ fxMatched.length - fxDoneCount }}
          </div>

          <div class="tm-fx-fields">
            <div class="tm-fx-fields-head">
              <span>导出字段</span>
              <span class="tm-fx-fields-count">默认 {{ FIELD_DEFS.length }} 项 · 自定义 {{ customFieldKeys.length }} 项 · 已选 {{ fxFields.length }}</span>
              <span class="tm-spacer2" />
              <a @click="fxSelectAll(true)">全选</a> · <a @click="fxSelectAll(false)">清空</a>
            </div>
            <label v-for="f in allExportFields" :key="f.key" class="tm-fx-chk">
              <input type="checkbox" :value="f.key" v-model="fxFields" @change="buildFxText" /> {{ f.label }}
            </label>
          </div>

          <div class="tm-fx-opts">
            <label class="tm-fx-chk"><input type="checkbox" v-model="fxOneLine" @change="buildFxText" /> 单行紧凑(字段用 " | " 分隔)</label>
          </div>

          <div class="tm-fx-preview-wrap">
            <div class="tm-export-preview-label">导出预览（可编辑）</div>
            <textarea v-model="fxText" class="tm-export-textarea" spellcheck="false" />
          </div>
        </div>
        <template #footer>
          <n-space>
            <n-button @click="buildFxText">重新生成</n-button>
            <n-button type="primary" :disabled="!fxText" @click="fxSaveFile">保存到文件…</n-button>
            <n-button :disabled="!fxText" @click="fxCopy">复制</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <TaskDetailDrawer
      v-model:show="detailVisible"
      :task="detailTask"
      :goals="goals"
      @saved="onDetailSaved"
      @deleted="onDetailSaved"
    />
  </div>
</template>

<script setup>
import { createLogger } from '../utils/logger'
import { ref, shallowRef, reactive, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import {
  NTabs, NTab, NInput, NButton, NButtonGroup, NIcon, NTag, NSpin, NDrawer, NDrawerContent,
  NForm, NFormItem, NSelect, NDatePicker, NSpace, NRadioGroup, NRadio, useMessage,
  NPopover, NSwitch, NTimePicker,
} from 'naive-ui'
import {
  ClipboardTaskListLtr20Filled, Search20Regular, Board20Regular,
  DataPie20Regular, ArrowImport20Regular, ArrowExportLtr20Regular, Alert20Regular,
  Flag16Regular,
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import draggable from 'vuedraggable'
import FullCalendar from '@fullcalendar/vue3'
import dayGridPlugin from '@fullcalendar/daygrid'
import timeGridPlugin from '@fullcalendar/timegrid'
import listPlugin from '@fullcalendar/list'
import interactionPlugin from '@fullcalendar/interaction'
import TaskDetailDrawer from '../components/TaskManagerComponent/TaskDetailDrawer.vue'
import TaskYearHeatmap from '../components/TaskManagerComponent/TaskYearHeatmap.vue'

const logger = createLogger('TaskManagerPage')
const message = useMessage()
const route = useRoute()

const isDark = ref(false)
const applyTheme = () => {
  const t = String(route.query.theme || '').toLowerCase()
  const theme = t === 'dark' || t === 'light' ? t : (document.documentElement.dataset.guaxiTheme || 'light')
  isDark.value = theme === 'dark'
  document.documentElement.dataset.guaxiTheme = theme
  document.documentElement.classList.toggle('dark-mode', theme === 'dark')
  document.body?.setAttribute?.('data-guaxi-theme', theme)
}

// ── 状态 ──
const view = ref('today')
const planMode = ref('kanban')
const category = ref('all')
const keyword = ref('')
const items = ref([])
const loading = ref(false)
const lastSync = ref('')
const searchInputRef = ref(null)

const reminderEnabled = ref(true)
const reminderTime = ref('09:00')

const captureText = ref('')
const captureCategory = ref('work')
const captureKind = ref('task')
const pendingStatus = ref('TODO')
const capturing = ref(false)

const detailVisible = ref(false)
const detailTask = ref(null)
const selectedId = ref('')

const renamingId = ref('')
const renameDraft = ref('')

const heatmapYear = ref(new Date().getFullYear())
const currentYear = new Date().getFullYear()
const selectedDay = ref('')

const CAT_MAP = { work: '工作', life: '生活', study: '学习' }
const catLabel = (c) => CAT_MAP[c] || c || '未分类'
const typeLabel = (t) => ({ bug: 'Bug', task: '任务', other: '其他' }[t] || '任务')
const KIND_LABELS = { task: '任务', habit: '习惯', goal: '目标', review: '复盘' }
const kindLabel = (k) => KIND_LABELS[k] || k || '任务'
const STATUS_ZH = { TODO: '待办', NEXT: '下一步', DOING: '进行中', WAITING: '等待', PAUSED: '暂停', HOLD: '搁置', CANCELLED: '已取消', DONE: '已完成' }
const statusZh = (s) => STATUS_ZH[s] || s || '待办'
const statusKeys = ['TODO', 'NEXT', 'DOING', 'WAITING', 'PAUSED', 'HOLD', 'CANCELLED', 'DONE']
const categoryOptions = [
  { label: '工作', value: 'work' }, { label: '生活', value: 'life' }, { label: '学习', value: 'study' },
]

// 看板列定义：状态关键字 → 列
const columns = [
  { key: 'todo', label: '待办', color: '#909399', statuses: ['TODO', ''], canonical: 'TODO' },
  { key: 'doing', label: '进行中', color: '#2080f0', statuses: ['DOING', 'NEXT'], canonical: 'DOING' },
  { key: 'paused', label: '暂停', color: '#f0a020', statuses: ['PAUSED', 'WAITING', 'HOLD'], canonical: 'PAUSED' },
  { key: 'done', label: '已完成', color: '#18a058', statuses: ['DONE', 'CANCELLED'], canonical: 'DONE' },
]
const colOf = (status) => {
  const s = String(status || '').toUpperCase()
  for (const c of columns) if (c.statuses.includes(s)) return c.key
  return 'todo'
}

const grouped = reactive({ todo: [], doing: [], paused: [], done: [] })
const showAllDone = ref(false)
const regroup = () => {
  grouped.todo = []; grouped.doing = []; grouped.paused = []; grouped.done = []
  const doneCutoff = addDaysIso(todayStr(), -14)
  for (const it of items.value) {
    // 目标是容器而不是待办；它只出现在「目标」视图，避免看板里堆积不可执行的条目。
    if (it.kind === 'goal') continue
    // 已完成默认只留最近两周，旧记录仍在年轮里，不占看板滚动。
    if (!showAllDone.value && it.done && it.completedIso && it.completedIso < doneCutoff) continue
    grouped[colOf(it.status)].push(it)
  }
}

const toggleDoneCollapse = () => {
  showAllDone.value = !showAllDone.value
  regroup()
}

const kanbanOrder = computed(() => columns.flatMap((c) => grouped[c.key]))

const todayStr = () => {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}
// 任意日期 → ISO "YYYY-MM-DD"：支持 ISO(2026-06-20)、斜杠(2026/6/20)、中文(2026年06月20日)，无法解析返回 ''
const cnDateToIso = (s) => {
  const str = String(s || '')
  const pad = (n) => String(n).padStart(2, '0')
  let m = /(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/.exec(str)
  if (m) return `${m[1]}-${pad(m[2])}-${pad(m[3])}`
  return ''
}
const isOverdue = (it) => it.deadlineIso && !it.done && it.deadlineIso < todayStr()

// TaskBlock(原始) → 视图使用的归一化项；同时保留 raw 供详情抽屉直接使用。
const normalizeBlock = (b) => ({
  id: b.internalId || '',
  displayId: b.id || '',
  title: b.description || b.id || '(未命名)',
  status: (b.status || 'TODO').toUpperCase(),
  done: !!b.done,
  priority: b.priority || '',
  category: b.manageType || '',
  taskType: b.taskType || 'task',
  kind: b.kind || 'task',
  severity: b.severity || '',
  deadline: b.deadline || '',
  deadlineIso: cnDateToIso(b.deadline),
  focus: b.focus || '',
  focusIso: cnDateToIso(b.focus),
  repeat: b.repeat || '',
  spent: Number(b.spent) || 0,
  checkins: Array.isArray(b.checkins) ? b.checkins : [],
  parentId: b.parentId || '',
  createTime: b.createTime || '',
  createTimeIso: cnDateToIso(b.createTime),
  completedTime: b.completedTime || '',
  completedIso: cnDateToIso(b.completedTime),
  content: b.content || '',
  notes: b.notes || { kind: 'none', value: '' },
  customFields: b.customFields || [],
  filePath: b.filePath || '',
  line: b.blockStartLine || 0,
  raw: b,
})

// ── 数据加载 ──
let searchTimer = null
const onSearchInput = () => {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(reload, 300)
}

const onCategoryChange = async () => {
  if (category.value !== 'all') captureCategory.value = category.value
  // 生活/学习默认按时间排；工作默认按状态看板。用户手动选的日历/目标不被覆盖。
  if (category.value === 'life' || category.value === 'study') {
    if (planMode.value === 'kanban') planMode.value = 'week'
  } else if (planMode.value === 'week') {
    planMode.value = 'kanban'
  }
  await reload()
}

const reload = async (fresh = false) => {
  loading.value = true
  try {
    const query = {
      category: category.value === 'all' ? null : category.value,
      keyword: keyword.value || null,
    }
    const res = await invoke(fresh ? 'task_query_fresh' : 'task_query', { query })
    items.value = (Array.isArray(res) ? res : []).map(normalizeBlock)
    regroup()
    lastSync.value = timeNow()
  } catch (e) {
    logger.error('加载任务失败:', e)
    message.error('加载任务失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

// ── 拖拽改状态 → 回写 .org ──
const onDragChange = async (colKey, evt) => {
  const added = evt?.added
  if (!added) return
  const el = added.element
  const target = columns.find((c) => c.key === colKey)
  if (!target || !el) return
  const newStatus = target.canonical
  if (String(el.status || '').toUpperCase() === newStatus) return
  try {
    const updated = await invoke('task_update_item', { input: { id: el.id, status: newStatus } })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === el.id) : null
    if (fresh) patchLocal(el, fresh)
    else {
      el.status = newStatus
      el.done = newStatus === 'DONE' || newStatus === 'CANCELLED'
    }
    message.success('已更新任务状态')
  } catch (e) {
    logger.error('更新状态失败:', e)
    message.error('更新失败：' + (e?.message || e))
    reload()
  }
}

// ── 详情抽屉 + 快捷操作 ──
const openDetail = (it) => {
  selectedId.value = it?.id || ''
  detailTask.value = it?.raw || it || null
  detailVisible.value = true
}
const onDetailSaved = async () => {
  detailVisible.value = false
  await reload()
}
/** 任务是从邮件转来的：取 :mailAccount/:mailFolder/:mailUid 自定义属性。 */
function mailOriginOf(it) {
  const fields = it.customFields || []
  const pick = (name) => String(fields.find((f) => f && f.name === name)?.value || '')
  const accountId = pick('mailAccount')
  const folder = pick('mailFolder')
  const uid = Number(pick('mailUid')) || 0
  return accountId && folder && uid ? { accountId, folder, uid } : null
}

/**
 * 回到原邮件：邮件窗口是独立窗口（label 固定为 app-tools-mail）。
 * 这里不能跨窗口直接开某封邮件，退回「把定位写进剪贴板并在应用里提示」太绕，
 * 改为发给邮件窗口一个事件——它监听后自己定位并打开。
 */
async function openMailOrigin(it) {
  const origin = mailOriginOf(it)
  if (!origin) return
  try {
    await invoke('mail_open_origin', origin)
    message.success('已在邮件窗口中打开')
  } catch (e) {
    logger.warn('打开原邮件失败', e)
    message.warning('邮件窗口没有打开，或这封邮件已不在本地')
  }
}

// ── 今日 / 年轮：派生数据 ──
const pad = (n) => String(n).padStart(2, '0')
const timeNow = () => {
  const d = new Date()
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}
const addDaysIso = (iso, n) => {
  const d = new Date(`${iso}T00:00:00`)
  d.setDate(d.getDate() + n)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}
const todayParts = computed(() => {
  const d = new Date()
  const weekdays = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
  return {
    day: pad(d.getDate()),
    weekday: weekdays[d.getDay()],
    yearMonth: `${d.getFullYear()} 年 ${d.getMonth() + 1} 月`,
  }
})
const shortDate = (iso) => {
  const s = cnDateToIso(iso)
  if (!s) return ''
  const today = todayStr()
  if (s === today) return '今天'
  if (s === addDaysIso(today, 1)) return '明天'
  if (s === addDaysIso(today, -1)) return '昨天'
  return s.slice(5).replace('-', '/')
}
const formatSpent = (m) => {
  const n = Number(m) || 0
  if (!n) return '0m'
  return n >= 60 ? `${Math.floor(n / 60)}h${n % 60 ? ` ${n % 60}m` : ''}` : `${n}m`
}
const reviewExcerpt = (it) => String(it.content || it.notes?.value || '').replace(/\s+/g, ' ').slice(0, 72) || '（空白复盘）'
const rowMeta = (it) => {
  const bits = [catLabel(it.category)]
  if (it.deadlineIso) bits.push(`截止 ${shortDate(it.deadlineIso)}`)
  if (it.priority) bits.push(`#${it.priority}`)
  if (it.kind !== 'task') bits.push(kindLabel(it.kind))
  if (it.repeat) bits.push(it.repeat)
  return bits.join(' · ')
}

const today = computed(() => todayStr())
const overdueItems = computed(() => {
  const t = today.value
  return items.value.filter((i) => !i.done && i.deadlineIso && i.deadlineIso < t && i.focusIso !== t)
    .sort((a, b) => a.deadlineIso.localeCompare(b.deadlineIso))
})
const todayDue = computed(() => items.value.filter((i) => !i.done && i.deadlineIso === today.value && i.focusIso !== today.value))
const todayDoing = computed(() => items.value.filter((i) => !i.done && ['DOING', 'NEXT'].includes(i.status)
  && i.deadlineIso !== today.value && !(i.deadlineIso && i.deadlineIso < today.value) && i.focusIso !== today.value))
const doneToday = computed(() => items.value.filter((i) => i.done && i.completedIso === todayStr()))
const focusItems = computed(() => items.value.filter((i) => !i.done && i.focusIso === todayStr()))
const todayHabits = computed(() => items.value.filter((i) => i.kind === 'habit'))
const habitDoneCount = computed(() => todayHabits.value.filter((h) => isCheckedToday(h)).length)
const todayReview = computed(() => items.value.find((i) => i.kind === 'review'
  && (i.completedIso === todayStr() || i.createTimeIso === todayStr())) || null)
const recentDone = computed(() => items.value.filter((i) => i.done && i.completedIso)
  .sort((a, b) => b.completedIso.localeCompare(a.completedIso)).slice(0, 5))
const habits = computed(() => items.value.filter((i) => i.kind === 'habit'))
const goals = computed(() => items.value.filter((i) => i.kind === 'goal'))
const reviews = computed(() => items.value
  .filter((i) => i.kind === 'review' && (i.completedIso || i.createTimeIso).startsWith(String(heatmapYear.value)))
  .sort((a, b) => (b.completedIso || b.createTimeIso).localeCompare(a.completedIso || a.createTimeIso)))

const isCheckedToday = (h) => (h.checkins || []).some((d) => cnDateToIso(d) === todayStr())
const habitStreak = (h) => {
  const days = new Set((h.checkins || []).map(cnDateToIso).filter(Boolean))
  let cur = todayStr()
  if (!days.has(cur)) cur = addDaysIso(cur, -1)
  let n = 0
  while (days.has(cur)) {
    n += 1
    cur = addDaysIso(cur, -1)
  }
  return n
}

const weekStart = computed(() => {
  const d = new Date(`${todayStr()}T00:00:00`)
  const offset = (d.getDay() + 6) % 7
  return addDaysIso(todayStr(), -offset)
})
const weekStats = computed(() => {
  const start = weekStart.value
  const end = todayStr()
  let done = 0
  let checkins = 0
  let spent = 0
  for (const it of items.value) {
    const completedIn = it.completedIso && it.completedIso >= start && it.completedIso <= end
    const createdIn = it.createTimeIso && it.createTimeIso >= start && it.createTimeIso <= end
    if (completedIn && it.done) done += 1
    if ((completedIn || createdIn) && it.spent) spent += it.spent
    for (const c of it.checkins || []) {
      const iso = cnDateToIso(c)
      if (iso && iso >= start && iso <= end) checkins += 1
    }
  }
  return { done, checkins, spent }
})

const weekLanes = computed(() => {
  const t = today.value
  const weekEnd = addDaysIso(weekStart.value, 6)
  const nextWeekEnd = addDaysIso(weekEnd, 7)
  const list = items.value.filter((i) => !i.done && !['goal', 'habit', 'review'].includes(i.kind))
  const lane = (pred) => list
    .filter(pred)
    .sort((a, b) => (a.deadlineIso || '9999-12-31').localeCompare(b.deadlineIso || '9999-12-31'))
  return [
    { key: 'overdue', label: '逾期', empty: '没有逾期', items: lane((i) => i.deadlineIso && i.deadlineIso < t) },
    { key: 'this', label: '本周', empty: '本周还空着', items: lane((i) => i.deadlineIso && i.deadlineIso >= t && i.deadlineIso <= weekEnd) },
    { key: 'next', label: '下周', empty: '下周还没排', items: lane((i) => i.deadlineIso && i.deadlineIso > weekEnd && i.deadlineIso <= nextWeekEnd) },
    { key: 'later', label: '以后 / 无日期', empty: '没有积压', items: lane((i) => !i.deadlineIso || i.deadlineIso > nextWeekEnd) },
  ]
})

const uniqueById = (list) => {
  const map = new Map()
  for (const it of list) if (it?.id && !map.has(it.id)) map.set(it.id, it)
  return [...map.values()]
}
// 键盘 J/K 只在当前视图真正可见的任务之间移动。
const visibleTaskList = computed(() => {
  if (view.value === 'today') {
    return uniqueById([...focusItems.value, ...overdueItems.value, ...todayDue.value, ...todayDoing.value])
  }
  if (view.value === 'plan') {
    if (planMode.value === 'kanban') return kanbanOrder.value
    if (planMode.value === 'week') return uniqueById(weekLanes.value.flatMap((l) => l.items))
  }
  return []
})
const ensureSelectedVisible = async () => {
  await nextTick()
  if (!selectedId.value) return
  const scope = view.value === 'today'
    ? '.tm-today'
    : planMode.value === 'kanban'
      ? '.tm-kanban'
      : planMode.value === 'week'
        ? '.tm-week'
        : ''
  if (!scope) return
  const el = document.querySelector(scope)?.querySelector(`[data-task-id="${selectedId.value}"]`)
  el?.scrollIntoView({ block: 'nearest' })
}
const moveSelection = async (delta) => {
  const list = visibleTaskList.value
  if (!list.length) return
  let idx = list.findIndex((it) => it.id === selectedId.value)
  if (idx < 0) idx = delta > 0 ? 0 : list.length - 1
  else idx = Math.min(list.length - 1, Math.max(0, idx + delta))
  selectedId.value = list[idx].id
  await ensureSelectedVisible()
}
const selectedVisibleItem = computed(() => visibleTaskList.value.find((it) => it.id === selectedId.value) || null)

const categoryStats = computed(() => ['work', 'life', 'study'].map((key) => {
  const list = items.value.filter((i) => i.category === key)
  const done = list.filter((i) => i.done).length
  return { key, label: catLabel(key), done, total: list.length, pct: list.length ? Math.round((done / list.length) * 100) : 0 }
}))

const goalChildren = (g) => items.value.filter((i) => i.parentId && (i.parentId === g.id || i.parentId === g.displayId))
const goalProgress = (g) => {
  const kids = goalChildren(g)
  const done = kids.filter((k) => k.done).length
  return { done, total: kids.length, pct: kids.length ? Math.round((done / kids.length) * 100) : 0 }
}

const dayDetail = computed(() => {
  const day = selectedDay.value
  if (!day) return { done: [], checkins: [] }
  return {
    done: items.value.filter((i) => i.done && i.completedIso === day),
    checkins: items.value.filter((i) => i.kind === 'habit' && (i.checkins || []).some((d) => cnDateToIso(d) === day)),
  }
})
const onSelectDay = (day) => { selectedDay.value = day }

// ── 快捷操作（写好即回写 .org）──
const patchLocal = (it, raw) => {
  if (raw) Object.assign(it, normalizeBlock(raw))
  regroup()
  lastSync.value = timeNow()
}

const setStatus = async (it, next) => {
  const prev = { status: it.status, done: it.done, completedIso: it.completedIso }
  it.status = next
  it.done = next === 'DONE' || next === 'CANCELLED'
  try {
    const updated = await invoke('task_update_item', { input: { id: it.id, status: next } })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === it.id) : null
    if (fresh) patchLocal(it, fresh)
    else regroup()
  } catch (e) {
    Object.assign(it, prev)
    regroup()
    logger.error('更新状态失败:', e)
    message.error('更新失败：' + (e?.message || e))
  }
}
const toggleDone = (it) => setStatus(it, it.done ? 'TODO' : 'DONE')

const isoToCnDate = (iso) => {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''))
  return m ? `${m[1]}年${m[2]}月${m[3]}日` : ''
}
const openDatePicker = (ev) => {
  const input = ev.currentTarget?.querySelector?.('input[type="date"]')
  if (!input) return
  try { input.showPicker?.() } catch { /* WebView 不支持时退回原生焦点 */ }
}
const updateDeadline = async (it, iso, okText) => {
  try {
    const updated = await invoke('task_update_item', {
      input: { id: it.id, fields: { deadline: iso ? isoToCnDate(iso) : '' } },
    })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === it.id) : null
    if (fresh) patchLocal(it, fresh)
    message.success(okText)
  } catch (e) {
    logger.error('更新截止失败:', e)
    message.error('更新截止失败：' + (e?.message || e))
  }
}
const snooze = (it) => updateDeadline(it, addDaysIso(todayStr(), 1), '已顺延到明天')
const setDeadline = (it, iso) => updateDeadline(it, iso, iso ? '已设置截止' : '已清除截止')
const clearDeadline = (it) => setDeadline(it, '')

const unfocus = async (it) => {
  try {
    const updated = await invoke('task_update_item', { input: { id: it.id, fields: { focus: '' } } })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === it.id) : null
    if (fresh) patchLocal(it, fresh)
  } catch (e) {
    logger.error('移出今日失败:', e)
    message.error('操作失败：' + (e?.message || e))
  }
}

const toggleHabit = async (h) => {
  const today = todayStr()
  const next = isCheckedToday(h)
    ? h.checkins.filter((d) => cnDateToIso(d) !== today)
    : [...h.checkins, today]
  try {
    const updated = await invoke('task_update_item', { input: { id: h.id, fields: { checkins: next } } })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === h.id) : null
    if (fresh) patchLocal(h, fresh)
    else { h.checkins = next; lastSync.value = timeNow() }
  } catch (e) {
    logger.error('打卡失败:', e)
    message.error('打卡失败：' + (e?.message || e))
  }
}

// 快速捕获：解析 @分类 / !优先级 / #形态 / 今天·明天 / 重复
const parseCapture = (text) => {
  let s = String(text || '').trim()
  const out = { title: s, category: captureCategory.value, kind: captureKind.value, priority: '', repeat: '', deadline: '' }
  let m = s.match(/@(工作|生活|学习)/)
  if (m) {
    out.category = { 工作: 'work', 生活: 'life', 学习: 'study' }[m[1]]
    s = s.replace(m[0], '')
  }
  m = s.match(/#(习惯|目标|复盘)/)
  if (m) {
    out.kind = { 习惯: 'habit', 目标: 'goal', 复盘: 'review' }[m[1]]
    s = s.replace(m[0], '')
  }
  m = s.match(/!([ABC])/i)
  if (m) {
    out.priority = m[1].toUpperCase()
    s = s.replace(m[0], '')
  }
  m = s.match(/(每天|工作日|每周|每月)/)
  if (m) {
    out.repeat = m[1]
    s = s.replace(m[0], '')
  }
  if (/今天/.test(s)) { out.deadline = todayStr(); s = s.replace(/今天/g, '') }
  else if (/明天/.test(s)) { out.deadline = addDaysIso(todayStr(), 1); s = s.replace(/明天/g, '') }
  else if (/后天/.test(s)) { out.deadline = addDaysIso(todayStr(), 2); s = s.replace(/后天/g, '') }
  else {
    m = s.match(/(\d{4})-(\d{1,2})-(\d{1,2})/)
    if (!m) m = s.match(/(\d{1,2})月(\d{1,2})日/)
    if (m) {
      const y = m.length === 4 ? m[1] : String(new Date().getFullYear())
      const mo = m.length === 4 ? m[2] : m[1]
      const d = m.length === 4 ? m[3] : m[2]
      out.deadline = `${y}-${pad(mo)}-${pad(d)}`
      s = s.replace(m[0], '')
    }
  }
  out.title = s.replace(/\s+/g, ' ').trim()
  return out
}
const captureHint = computed(() => {
  const raw = captureText.value.trim()
  if (!raw) return pendingStatus.value !== 'TODO' ? `新建到「${columns.find((c) => c.canonical === pendingStatus.value)?.label || '待办'}」` : ''
  const p = parseCapture(raw)
  const bits = [catLabel(p.category), kindLabel(p.kind)]
  if (p.deadline) bits.push(`截止 ${shortDate(p.deadline)}`)
  else if (view.value === 'today' && !['habit', 'goal', 'review'].includes(p.kind)) bits.push('置顶今日')
  if (p.priority) bits.push(`#${p.priority}`)
  if (p.repeat) bits.push(p.repeat)
  return bits.join(' · ')
})

const planCapturePlaceholder = computed(() => {
  if (captureKind.value === 'goal') return '＋ 新建目标，回车保存'
  const label = columns.find((c) => c.canonical === pendingStatus.value)?.label || '待办'
  return `＋ 新建到「${label}」，回车保存`
})

const focusCapture = async () => {
  await nextTick()
  const sel = view.value === 'plan' ? '.tm-plan-capture' : '.tm-capture-input'
  document.querySelector(sel)?.focus()
}

const quickCapture = async () => {
  const p = parseCapture(captureText.value)
  if (!p.title) return
  capturing.value = true
  try {
    const created = await invoke('task_create', {
      input: {
        category: p.category,
        title: p.title,
        kind: p.kind,
        priority: p.priority,
        repeat: p.repeat,
        deadline: p.deadline ? `${p.deadline.slice(0, 4)}年${p.deadline.slice(5, 7)}月${p.deadline.slice(8, 10)}日` : '',
        focus: view.value === 'today' && !p.deadline && !['habit', 'goal', 'review'].includes(p.kind) ? todayStr() : '',
        status: p.kind === 'review' ? 'DONE' : pendingStatus.value,
      },
    })
    items.value.unshift(normalizeBlock(created))
    regroup()
    lastSync.value = timeNow()
    if (p.kind === 'goal') planMode.value = 'goals'
    captureText.value = ''
    pendingStatus.value = 'TODO'
    captureKind.value = 'task'
    message.success('已记下')
  } catch (e) {
    logger.error('快速记录失败:', e)
    message.error('记录失败：' + (e?.message || e))
  } finally {
    capturing.value = false
  }
}

// task_create 目前固定 TODO；列内新建需要列状态，这里建好后立刻改状态。
const newTask = async (colKey) => {
  const col = columns.find((c) => c.key === colKey)
  pendingStatus.value = col?.canonical || 'TODO'
  captureKind.value = 'task'
  if (view.value !== 'plan') view.value = 'plan'
  if (planMode.value !== 'kanban') planMode.value = 'kanban'
  await focusCapture()
}

const newGoal = async () => {
  captureKind.value = 'goal'
  pendingStatus.value = 'TODO'
  if (view.value !== 'plan') view.value = 'plan'
  if (planMode.value !== 'kanban') planMode.value = 'kanban'
  await focusCapture()
}

const startReview = async () => {
  try {
    const created = await invoke('task_create', {
      input: {
        category: captureCategory.value,
        title: `复盘 · ${todayStr()}`,
        kind: 'review',
        content: '',
        status: 'DONE',
      },
    })
    await reload()
    detailTask.value = created
    detailVisible.value = true
  } catch (e) {
    logger.error('创建复盘失败:', e)
    message.error('创建复盘失败：' + (e?.message || e))
  }
}

const inlineRename = async (it) => {
  renamingId.value = it.id
  renameDraft.value = it.title
  await nextTick()
  document.querySelector('.tm-card-rename')?.focus()
}
const commitRename = async (it) => {
  const value = renameDraft.value.trim()
  renamingId.value = ''
  if (!value || value === it.title) return
  try {
    const updated = await invoke('task_update_item', { input: { id: it.id, fields: { description: value } } })
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === it.id) : null
    it.title = value
    if (fresh) patchLocal(it, fresh)
    message.success('已改名')
  } catch (e) {
    logger.error('改名失败:', e)
    message.error('改名失败：' + (e?.message || e))
  }
}

const setPlanMode = async (m) => {
  planMode.value = m
  await nextTick()
  if (m === 'calendar') {
    try { calRef.value?.getApi()?.updateSize() } catch {}
    try { window.dispatchEvent(new Event('resize')) } catch {}
  }
}

// ── 提醒设置 ──
const loadReminderConfig = async () => {
  try {
    const cfg = await invoke('task_get_config')
    reminderEnabled.value = cfg?.reminder_enabled !== false
    reminderTime.value = cfg?.reminder_time || '09:00'
  } catch (e) {
    logger.warn('读取提醒配置失败', e)
  }
}
const saveReminder = async () => {
  try {
    const cfg = await invoke('task_set_reminder', {
      enabled: reminderEnabled.value,
      time: reminderTime.value || '09:00',
    })
    reminderEnabled.value = cfg?.reminder_enabled !== false
    reminderTime.value = cfg?.reminder_time || '09:00'
    message.success(reminderEnabled.value ? `已开启每日 ${reminderTime.value} 摘要` : '已关闭每日摘要')
  } catch (e) {
    logger.error('保存提醒设置失败:', e)
    message.error('保存提醒设置失败：' + (e?.message || e))
  }
}

// ── 导入 ──
const showImport = ref(false)
const importing = ref(false)
const importForm = reactive({ category: 'work', target_file_name: 'inbox.org', source_kind: 'org', content: '' })
const pickImportFile = async () => {
  try {
    const picked = await openDialog({ multiple: false, filters: [{ name: 'Org/JSON', extensions: ['org', 'json', 'txt'] }] })
    const path = Array.isArray(picked) ? picked[0] : picked
    if (!path) return
    importForm.content = await invoke('task_read_file', { path })
    if (path.toLowerCase().endsWith('.json')) importForm.source_kind = 'json'
  } catch (e) {
    logger.error('读取导入文件失败:', e)
    message.error('读取文件失败')
  }
}
const doImport = async () => {
  importing.value = true
  try {
    const count = await invoke('task_import', {
      input: {
        category: importForm.category,
        target_file_name: importForm.target_file_name,
        source_kind: importForm.source_kind,
        content: importForm.content,
      },
    })
    message.success(`导入完成，共索引 ${count} 个任务`)
    showImport.value = false
    await reload()
  } catch (e) {
    logger.error('导入失败:', e)
    message.error('导入失败：' + (e?.message || e))
  } finally {
    importing.value = false
  }
}

// ── 导出报表（日/周/月/半年/年，纯文本可编辑）──
const showExport = ref(false)
const exporting = ref(false)
const exportPreview = ref('')
const exportForm = reactive({ kind: 'weekly', from: null, to: null, scope: 'work' })
const reportKindOptions = [
  { label: '日报', value: 'daily' },
  { label: '周报', value: 'weekly' },
  { label: '月报', value: 'monthly' },
  { label: '半年报', value: 'halfyear' },
  { label: '年报', value: 'yearly' },
]
const reportScopeOptions = [
  { label: '工作', value: 'work' },
  { label: '生活', value: 'life' },
  { label: '学习', value: 'study' },
]
const reportKindLabel = () => {
  const kind = reportKindOptions.find((o) => o.value === exportForm.kind)?.label || '报表'
  const scope = reportScopeOptions.find((o) => o.value === exportForm.scope)?.label || '工作'
  return `${scope}${kind}`
}
// 打开导出抽屉并立即生成一次预览
const openExport = () => {
  if (category.value !== 'all') exportForm.scope = category.value
  showExport.value = true
  genPreviewNow()
}
// 日期/报表类型变化时只防抖地生成预览：日期选择器在键盘输入过程中会连续发出
// update:formatted-value，逐次触发一次后端往返会把抽屉拖得一顿一顿的。
let previewSeq = 0
let previewTimer = null
const genPreview = () => {
  // 立刻作废仍在途的旧请求，避免它回来时把刚改好的日期对应的预览盖掉。
  previewSeq++
  if (previewTimer) clearTimeout(previewTimer)
  previewTimer = setTimeout(() => { previewTimer = null; genPreviewNow() }, 260)
}
const genPreviewNow = async () => {
  if (previewTimer) { clearTimeout(previewTimer); previewTimer = null }
  const seq = ++previewSeq
  exporting.value = true
  try {
    // 重扫已在 task_export_report 内部做完（增量，内容没变的文件直接跳过），
    // 不再单独发一次 task_scan_and_index —— 那会让每次改日期都多付一轮全量扫描。
    const text = await invoke('task_export_report', {
      input: { kind: exportForm.kind, date: '', from: exportForm.from || null, to: exportForm.to || null, format: 'text', scope: exportForm.scope },
    })
    if (seq !== previewSeq) return
    exportPreview.value = text
  } catch (e) {
    if (seq !== previewSeq) return
    logger.error('生成报表失败:', e)
    message.error('生成失败：' + (e?.message || e))
  } finally {
    if (seq === previewSeq) exporting.value = false
  }
}
const saveReport = async () => {
  if (!exportPreview.value) return
  try {
    const path = await saveDialog({ defaultPath: `${reportKindLabel()}-${todayStr()}.txt`, filters: [{ name: 'Text', extensions: ['txt'] }] })
    if (!path) return
    await invoke('task_write_report_file', { path, content: exportPreview.value })
    message.success('报表已保存')
  } catch (e) {
    logger.error('保存报表失败:', e)
    message.error('保存失败：' + (e?.message || e))
  }
}
const copyReport = async () => {
  try { await navigator.clipboard.writeText(exportPreview.value); message.success('已复制周报') }
  catch { message.error('复制失败') }
}

// ── 按字段筛选导出 ──
const showFieldExport = ref(false)
// 只整体替换、从不原地改：用 shallowRef 免掉对几百个任务块（含大段正文）的深层代理，
// 筛选与生成预览时每次读字段都不必穿过 Proxy。
const fxData = shallowRef([])
const fxText = ref('')
const fxOneLine = ref(false)
const fxFilter = reactive({
  category: 'all', taskType: 'all', status: 'all', severity: 'all', priority: 'all',
  createFrom: null, createTo: null, completedFrom: null, completedTo: null, keyword: '',
})
const fxCategoryOptions = [
  { label: '全部分类', value: 'all' }, { label: '工作', value: 'work' }, { label: '生活', value: 'life' }, { label: '学习', value: 'study' },
]
const fxTypeOptions = [
  { label: '全部类型', value: 'all' }, { label: 'Bug', value: 'bug' }, { label: '任务', value: 'task' }, { label: '其他', value: 'other' },
]
const fxStatusOptions = [
  { label: '全部状态', value: 'all' }, { label: '已完成', value: 'done' }, { label: '未完成', value: 'undone' },
  { label: '待办 TODO', value: 'st:TODO' }, { label: '进行中 DOING', value: 'st:DOING' }, { label: '下一步 NEXT', value: 'st:NEXT' },
  { label: '等待 WAITING', value: 'st:WAITING' }, { label: '暂停 PAUSED', value: 'st:PAUSED' }, { label: '搁置 HOLD', value: 'st:HOLD' },
  { label: '已取消 CANCELLED', value: 'st:CANCELLED' }, { label: '已完成 DONE', value: 'st:DONE' },
]
const fxSeverityOptions = [
  { label: '全部', value: 'all' }, { label: 'critical', value: 'critical' }, { label: 'major', value: 'major' },
  { label: 'normal', value: 'normal' }, { label: 'minor', value: 'minor' }, { label: 'trivial', value: 'trivial' },
]
const fxPriorityOptions = [
  { label: '全部', value: 'all' }, { label: 'A', value: 'A' }, { label: 'B', value: 'B' }, { label: 'C', value: 'C' }, { label: '无', value: 'none' },
]
// 字段注册表：任务块 DSL 的全部默认字段（顺序与后端 KNOWN_FIELDS / 模板一致）。
// 同时驱动「导出字段」勾选表与「字段条件」筛选器，新增默认字段只需改这一处。
// type: enum（有固定取值，下拉选择）/ date（中文或 ISO 日期，按时间比较）/ text（其余，按文本匹配）
const enumOptionsOf = (pairs) => pairs.map(([value, label]) => ({ value, label }))
const FIELD_DEFS = [
  { key: 'manageType', label: '分类', type: 'enum', options: enumOptionsOf([['work', '工作'], ['life', '生活'], ['study', '学习']]) },
  { key: 'internalId', label: '内部 ID', type: 'text' },
  { key: 'id', label: 'ID', type: 'text' },
  { key: 'parentId', label: '父任务', type: 'text' },
  { key: 'taskType', label: '类型', type: 'enum', options: enumOptionsOf([['bug', 'Bug'], ['task', '任务'], ['other', '其他']]) },
  { key: 'kind', label: '形态', type: 'enum', options: enumOptionsOf([['task', '任务'], ['habit', '习惯'], ['goal', '目标'], ['review', '复盘']]) },
  { key: 'deadline', label: '截止', type: 'date' },
  { key: 'focus', label: '置顶', type: 'date' },
  { key: 'repeat', label: '重复', type: 'text' },
  { key: 'spent', label: '投入(分钟)', type: 'text' },
  { key: 'checkins', label: '打卡', type: 'text' },
  { key: 'createTime', label: '创建时间', type: 'date' },
  { key: 'completedTime', label: '完成时间', type: 'date' },
  { key: 'origin', label: '来源', type: 'enum', options: enumOptionsOf([['customer', 'customer'], ['qa', 'qa'], ['dev', 'dev'], ['other', 'other']]) },
  { key: 'severity', label: '严重度', type: 'enum', options: enumOptionsOf(['critical', 'major', 'normal', 'minor', 'trivial'].map((v) => [v, v])) },
  { key: 'priority', label: '优先级', type: 'enum', options: enumOptionsOf([['A', 'A'], ['B', 'B'], ['C', 'C']]) },
  { key: 'status', label: '状态', type: 'enum', options: enumOptionsOf(Object.entries(STATUS_ZH).map(([code, zh]) => [code, `${zh} ${code}`])) },
  { key: 'progress', label: '进度', type: 'text' },
  { key: 'fixStatus', label: '修复状态', type: 'enum', options: enumOptionsOf(['debug', 'test', 'review', 'approval', 'fixed', 'other'].map((v) => [v, v])) },
  { key: 'description', label: '描述', type: 'text' },
  { key: 'rootCause', label: '根本原因', type: 'text' },
  { key: 'conditionOfOccurrence', label: '复现步骤', type: 'text' },
  { key: 'fixedDetails', label: '修复详情', type: 'text' },
  { key: 'testingSuggestions', label: '测试建议', type: 'text' },
  { key: 'affectedRelease', label: '影响版本', type: 'text' },
  { key: 'reviewLink', label: 'Review', type: 'text' },
  { key: 'extraImpacts', label: 'Extra Impact', type: 'text' },
  { key: 'content', label: '详情', type: 'text' },
  { key: 'notes', label: '笔记', type: 'text' },
]
const fxFields = ref(['id', 'taskType', 'status', 'description', 'createTime', 'completedTime'])
// 自定义字段：汇总当前数据里出现过的所有自定义字段名，动态并入字段列表（筛选与导出共用）。
const customFieldKeys = computed(() => {
  const names = new Set()
  for (const b of fxData.value) {
    for (const cf of (b.customFields || [])) {
      if (cf && cf.name) names.add(cf.name)
    }
  }
  return [...names].map((n) => ({ key: 'cf:' + n, label: n, type: 'text' }))
})
const allExportFields = computed(() => FIELD_DEFS.concat(customFieldKeys.value))
const fieldDef = (key) => allExportFields.value.find((f) => f.key === key)
const fxFieldOptions = computed(() => allExportFields.value.map((f) => ({ label: f.label, value: f.key })))

// 字段条件：任意默认/自定义字段 + 比较操作符。日期类字段用 cnDateToIso 归一化后比较，
// 因此 "2026年06月20日" 与 "2026-6-20" 视作同一天。
const OPS_BY_TYPE = {
  enum: [['eq', '等于'], ['ne', '不等于'], ['in', '属于其中之一'], ['empty', '为空'], ['nempty', '不为空']],
  date: [['gte', '不早于'], ['lte', '不晚于'], ['range', '区间'], ['empty', '为空'], ['nempty', '不为空']],
  text: [['contains', '包含'], ['ncontains', '不包含'], ['eq', '等于'], ['ne', '不等于'], ['empty', '为空'], ['nempty', '不为空']],
}
const fxConds = ref([])
let condUid = 0
const condType = (c) => fieldDef(c?.field)?.type || 'text'
const condEnumOptions = (c) => fieldDef(c?.field)?.options || []
const condOpOptions = (c) => OPS_BY_TYPE[condType(c)].map(([value, label]) => ({ value, label }))
const condNoValue = (c) => c?.op === 'empty' || c?.op === 'nempty'
const condMulti = (c) => !!condEnumOptions(c).length && c?.op === 'in'
const condEnumSingle = (c) => !!condEnumOptions(c).length && !condNoValue(c) && !condMulti(c)
const condIsDate = (c) => condType(c) === 'date' && !condNoValue(c)
const addCond = () => {
  fxConds.value.push({ uid: ++condUid, field: 'origin', op: 'eq', value: null, value2: null, values: [] })
  recomputeFx()
}
const removeCond = (i) => { fxConds.value.splice(i, 1); recomputeFx() }
const clearConds = () => { fxConds.value = []; recomputeFx() }
// 换字段后旧操作符可能不适用（如 text→date 的「包含」），重置为该项类型的首个操作符并清值。
// key 由事件显式传入，不依赖 v-model 是否已写回 c.field。
const onCondFieldChange = (c, key) => {
  const type = fieldDef(key || c.field)?.type || 'text'
  c.op = OPS_BY_TYPE[type][0][0]
  c.value = null; c.value2 = null; c.values = []
  recomputeFx()
}
// 筛选用的原始值：枚举字段用英文规范值（与 options 的 value 对齐），日期字段用原文（比较时再归一化）。
const condRawValue = (b, key) => {
  if (key.startsWith('cf:')) {
    const name = key.slice(3)
    const cf = (b.customFields || []).find((x) => x && x.name === name)
    return cf ? String(cf.value ?? '') : ''
  }
  if (key === 'extraImpacts') return (Array.isArray(b.extraImpacts) ? b.extraImpacts : []).join('\n')
  if (key === 'notes') {
    const n = b.notes
    if (!n || n.kind === 'none') return ''
    return String(n.value || '')
  }
  if (key === 'status') return String(b.status || '').toUpperCase()
  return String(b[key] ?? '')
}
const matchCond = (b, c) => {
  if (!c || !c.field || !c.op) return true
  const s = condRawValue(b, c.field)
  const t = s.trim()
  switch (c.op) {
    case 'empty': return !t
    case 'nempty': return !!t
    case 'eq': return t === String(c.value ?? '').trim()
    case 'ne': return t !== String(c.value ?? '').trim()
    case 'in': return (c.values || []).includes(t)
    case 'contains': return t.toLowerCase().includes(String(c.value ?? '').toLowerCase())
    case 'ncontains': return !t.toLowerCase().includes(String(c.value ?? '').toLowerCase())
    case 'gte': { const iso = cnDateToIso(s); return !!iso && !!c.value && iso >= c.value }
    case 'lte': { const iso = cnDateToIso(s); return !!iso && !!c.value && iso <= c.value }
    case 'range': { const iso = cnDateToIso(s); return !!iso && (!c.value || iso >= c.value) && (!c.value2 || iso <= c.value2) }
    default: return true
  }
}

const fxMatched = computed(() => fxData.value.filter((b) => {
  if (fxFilter.category !== 'all' && (b.manageType || '') !== fxFilter.category) return false
  if (fxFilter.taskType !== 'all' && (b.taskType || '') !== fxFilter.taskType) return false
  const st = String(b.status || '').toUpperCase()
  if (fxFilter.status === 'done' && st !== 'DONE') return false
  if (fxFilter.status === 'undone' && st === 'DONE') return false
  if (fxFilter.status.startsWith('st:') && st !== fxFilter.status.slice(3)) return false
  if (fxFilter.severity !== 'all' && !String(b.severity || '').toLowerCase().includes(fxFilter.severity)) return false
  if (fxFilter.priority !== 'all') {
    if (fxFilter.priority === 'none') { if (b.priority) return false }
    else if ((b.priority || '') !== fxFilter.priority) return false
  }
  const cIso = cnDateToIso(b.createTime)
  if (fxFilter.createFrom && (!cIso || cIso < fxFilter.createFrom)) return false
  if (fxFilter.createTo && (!cIso || cIso > fxFilter.createTo)) return false
  const dIso = cnDateToIso(b.completedTime)
  if (fxFilter.completedFrom && (!dIso || dIso < fxFilter.completedFrom)) return false
  if (fxFilter.completedTo && (!dIso || dIso > fxFilter.completedTo)) return false
  if (fxFilter.keyword) {
    const k = fxFilter.keyword.toLowerCase()
    if (!String(b.description || '').toLowerCase().includes(k) && !String(b.id || '').toLowerCase().includes(k)) return false
  }
  for (const c of fxConds.value) if (!matchCond(b, c)) return false
  return true
}))
const fxCount = (t) => fxMatched.value.filter((b) => (b.taskType || '') === t).length
const fxDoneCount = computed(() => fxMatched.value.filter((b) => String(b.status || '').toUpperCase() === 'DONE').length)

const fxFieldValue = (b, key) => {
  if (key === 'taskType') return typeLabel(b.taskType)
  if (key === 'kind') return kindLabel(b.kind)
  if (key === 'status') return statusZh(String(b.status || '').toUpperCase())
  if (key === 'manageType') return catLabel(b.manageType)
  if (key === 'checkins') return (Array.isArray(b.checkins) ? b.checkins : []).join(', ')
  if (key === 'spent') return String(b.spent || 0)
  if (key === 'extraImpacts') return (Array.isArray(b.extraImpacts) ? b.extraImpacts : []).join('\n\n')
  if (key === 'notes') {
    const n = b.notes
    if (!n || n.kind === 'none') return ''
    return n.kind === 'file' ? `（外链）${n.value || ''}` : String(n.value || '')
  }
  return condRawValue(b, key)
}
const buildFxText = () => {
  const fields = allExportFields.value.filter((f) => fxFields.value.includes(f.key))
  const entries = fxMatched.value.map((b) => {
    if (fxOneLine.value) return fields.map((f) => `${f.label}: ${fxFieldValue(b, f.key).replace(/\n/g, ' ')}`).join(' | ')
    return fields.map((f) => {
      const v = fxFieldValue(b, f.key)
      return v.includes('\n') ? `${f.label}:\n${v}` : `${f.label}: ${v}`
    }).join('\n')
  })
  fxText.value = entries.join(fxOneLine.value ? '\n' : '\n----------------------------------------\n')
}
const recomputeFx = () => {
  if (fxTimer) clearTimeout(fxTimer)
  fxTimer = setTimeout(() => { fxTimer = null; buildFxText() }, 120)
}
let fxTimer = null
const fxSelectAll = (all) => { fxFields.value = all ? allExportFields.value.map((f) => f.key) : []; buildFxText() }
// 打开面板：后端在一个后台任务里完成「增量重扫 + 查询」，前端只等一次往返，
// 也不再额外发一次全量 task_scan_and_index。
let fxLoading = false
const openFieldExport = async () => {
  showFieldExport.value = true
  if (fxLoading) return
  fxLoading = true
  if (fxTimer) { clearTimeout(fxTimer); fxTimer = null }
  try {
    fxData.value = (await invoke('task_query_fresh', { query: {} })) || []
    buildFxText()
  } catch (e) {
    logger.error('加载任务失败:', e); fxData.value = []
  } finally {
    fxLoading = false
  }
}
const fxSaveFile = async () => {
  if (!fxText.value) return
  try {
    const path = await saveDialog({ defaultPath: `任务导出-${todayStr()}.txt`, filters: [{ name: 'Text', extensions: ['txt'] }] })
    if (!path) return
    await invoke('task_write_report_file', { path, content: fxText.value })
    message.success('已导出到文件')
  } catch (e) { logger.error('导出失败:', e); message.error('导出失败：' + (e?.message || e)) }
}
const fxCopy = async () => {
  try { await navigator.clipboard.writeText(fxText.value); message.success('已复制导出内容') }
  catch { message.error('复制失败') }
}

// ── 统计 ──
const doneCount = computed(() => items.value.filter((i) => i.done).length)
const yearDone = computed(() => items.value.filter((i) => i.done && i.completedIso?.startsWith(String(heatmapYear.value))).length)
const doingCount = computed(() => items.value.filter((i) => ['DOING', 'NEXT'].includes(String(i.status).toUpperCase())).length)
const bugCount = computed(() => items.value.filter((i) => i.taskType === 'bug').length)
// Bug 统计仅在工作/全部下显示（生活·学习无 bug 概念）
const showBugStat = computed(() => category.value !== 'life' && category.value !== 'study')
const completionRate = computed(() => items.value.length ? Math.round((doneCount.value / items.value.length) * 100) : 0)

// ── 日历（按截止日期 ISO） ──
const calRef = ref(null)
const calendarEvents = computed(() => {
  const evs = []
  for (const it of items.value) {
    if (it.deadlineIso) {
      evs.push({ id: it.id + ':d', title: '⏰ ' + it.title, start: it.deadlineIso, color: it.done ? '#18a058' : '#d03050', extendedProps: { item: it } })
    } else if (it.createTimeIso) {
      // 无截止日期时，用创建时间作为日历标记，保证任务仍可见
      evs.push({ id: it.id + ':c', title: it.title, start: it.createTimeIso, color: '#2080f0', extendedProps: { item: it } })
    }
  }
  return evs
})
const calendarOptions = computed(() => ({
  plugins: [dayGridPlugin, timeGridPlugin, listPlugin, interactionPlugin],
  initialView: 'dayGridMonth',
  headerToolbar: { left: 'prev,next today', center: 'title', right: 'dayGridMonth,timeGridWeek,timeGridDay,listYear' },
  buttonText: { today: '今天', month: '月', week: '周', day: '日', list: '年' },
  locale: 'zh-cn',
  firstDay: 1,
  height: '100%',
  events: calendarEvents.value,
  eventMouseEnter: (info) => {
    const it = info.event.extendedProps?.item
    if (!it) return
    const r = info.el.getBoundingClientRect()
    hover.value = { visible: true, x: r.left + r.width / 2, y: r.top, item: it }
  },
  eventMouseLeave: () => { hover.value.visible = false },
}))

// 日历悬浮详情卡片
const hover = ref({ visible: false, x: 0, y: 0, item: null })

// 视图切换：FullCalendar 在 display:none 下量不到尺寸，切回来要重新测量。
watch(view, async (v) => {
  await nextTick()
  if (v === 'plan' && planMode.value === 'calendar') {
    try { calRef.value?.getApi()?.updateSize() } catch (e) { logger.warn('日历重测量失败', e) }
  }
  try { window.dispatchEvent(new Event('resize')) } catch {}
})
watch(planMode, async (m) => {
  await nextTick()
  if (m === 'calendar') {
    try { calRef.value?.getApi()?.updateSize() } catch (e) { logger.warn('日历重测量失败', e) }
    try { window.dispatchEvent(new Event('resize')) } catch {}
  }
})
watch(heatmapYear, () => { selectedDay.value = '' })
watch([view, planMode], () => { selectedId.value = '' })

// 任务数据变化时，把事件推给已挂载的日历。
watch(calendarEvents, async (evs) => {
  await nextTick()
  const api = calRef.value?.getApi?.()
  if (api) {
    try { api.removeAllEvents(); api.addEventSource(evs) } catch (e) { logger.warn('日历事件更新失败', e) }
  }
})

const isTypingTarget = (e) => {
  const t = e?.target
  if (!t) return false
  return t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable
}
const onKeydown = (e) => {
  if (e.key === 'Escape') {
    if (detailVisible.value) {
      detailVisible.value = false
      e.preventDefault()
    }
    return
  }
  if (detailVisible.value) return
  if (isTypingTarget(e) || e.metaKey || e.ctrlKey || e.altKey) return
  const onButton = e.target?.closest?.('button, a, [role="button"]')
  if (onButton && (e.key === 'Enter' || e.key === ' ' || e.code === 'Space')) return
  if (e.key === 'j' || e.key === 'J') {
    void moveSelection(1)
    e.preventDefault()
  } else if (e.key === 'k' || e.key === 'K') {
    void moveSelection(-1)
    e.preventDefault()
  } else if (e.key === 'Enter') {
    if (selectedVisibleItem.value) {
      openDetail(selectedVisibleItem.value)
      e.preventDefault()
    }
  } else if (e.key === ' ' || e.code === 'Space') {
    if (selectedVisibleItem.value) {
      void toggleDone(selectedVisibleItem.value)
      e.preventDefault()
    }
  } else if (e.key === 'n' || e.key === 'N') {
    view.value = 'today'
    void focusCapture()
    e.preventDefault()
  } else if (e.key === '/') {
    searchInputRef.value?.focus?.()
    e.preventDefault()
  } else if (e.key === '1') view.value = 'today'
  else if (e.key === '2') view.value = 'plan'
  else if (e.key === '3') view.value = 'year'
}
const onWindowFocus = () => { void reload() }

let unlisten = null
let unlistenReminder = null
onMounted(async () => {
  applyTheme()
  await loadReminderConfig()
  // 首次打开先增量重扫：任务窗口关着时编辑器可能已经改过 .org，索引不一定是新的。
  await reload(true)
  try {
    await invoke('task_start_watching')
  } catch (e) {
    logger.warn('启动任务文件监听失败', e)
  }
  try {
    unlisten = await listen('guaxi:task-files-changed', () => reload())
  } catch (e) {
    logger.warn('监听任务文件变更失败', e)
  }
  try {
    unlistenReminder = await listen('guaxi:task-reminder', (ev) => {
      const body = ev?.payload?.body || '有任务到期'
      message.warning(body)
      void reload()
    })
  } catch (e) {
    logger.warn('监听任务提醒失败', e)
  }
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('focus', onWindowFocus)
})

onUnmounted(() => {
  try { unlisten?.() } catch {}
  try { unlistenReminder?.() } catch {}
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('focus', onWindowFocus)
  if (searchTimer) clearTimeout(searchTimer)
  if (previewTimer) clearTimeout(previewTimer)
  if (fxTimer) clearTimeout(fxTimer)
})
</script>

<style scoped>
.task-manager-page {
  /* 独立窗口根：用 100vh 而非 100% —— 与本项目其它独立窗口一致。
     #app 与 naive 的 config/message provider 之间的包裹层会破坏 height:100% 的百分比链，
     导致整页按内容撑高、看板列无法约束高度而不滚动。 */
  display: flex; flex-direction: column; height: 100vh; max-height: 100vh;
  background: var(--guaxi-app-bg); color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  font-size: 14px;
}
/* 让 naive-ui / FullCalendar 等子组件统一继承应用字体 */
.task-manager-page :deep(.fc),
.task-manager-page :deep(.n-button),
.task-manager-page :deep(.n-tabs),
.task-manager-page :deep(.n-input),
.task-manager-page :deep(.n-drawer),
.task-manager-page :deep(.n-card) {
  font-family: inherit;
}
.tm-toolbar {
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
  padding: 8px 14px; border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg); flex: 0 0 auto;
}
.tm-toolbar-left { display: flex; align-items: center; gap: 12px; min-width: 0; }
.tm-logo { color: var(--n-color-primary, #2080f0); }
.tm-title { font-weight: 600; font-size: 15px; white-space: nowrap; }
.tm-cat-tabs { width: 280px; }
.tm-toolbar-right { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
.tm-search { width: 180px; }
.tm-body {
  flex: 1 1 auto; min-height: 0; overflow: hidden; position: relative;
  display: flex; flex-direction: column;
}
.tm-loading-overlay {
  position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;
  background: color-mix(in srgb, var(--guaxi-app-bg) 60%, transparent); z-index: 20;
}

/* 看板 */
.tm-kanban {
  display: grid; grid-template-columns: repeat(4, 1fr);
  /* 单行高度绑定到栅格高度（minmax(0,1fr)），列才能被约束高度并触发列内滚动，
     否则默认 auto 行会随卡片增多而撑高，导致不滚动只增高。 */
  grid-template-rows: minmax(0, 1fr);
  gap: 12px;
  flex: 1 1 auto; min-height: 0;
  padding: 14px; box-sizing: border-box; overflow: hidden;
}
.tm-column {
  display: flex; flex-direction: column; min-height: 0;
  background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border);
  border-radius: 10px; overflow: hidden;
}
.tm-column-head {
  display: flex; align-items: center; gap: 8px; padding: 10px 12px;
  border-bottom: 2px solid; font-weight: 600; flex: 0 0 auto; font-size: 15px;
}
.tm-col-dot { width: 9px; height: 9px; border-radius: 50%; }
.tm-col-name { flex: 1; }
.tm-col-count { background: var(--guaxi-header-bg); }
.tm-cards { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 8px; }
.tm-empty-col { padding: 16px; text-align: center; color: var(--guaxi-text-secondary); font-size: 13px; }
.tm-card {
  background: var(--guaxi-app-bg); border: 1px solid var(--guaxi-border); border-radius: 8px;
  padding: 10px; cursor: pointer; transition: box-shadow .15s, transform .05s;
}
.tm-card:hover { box-shadow: 0 3px 10px rgba(0,0,0,.12); }
.tm-card.selected { border-color: var(--tm-accent); box-shadow: 0 0 0 2px var(--tm-accent-soft); }
.tm-card:active { transform: scale(.99); }
.tm-card.overdue { border-color: #d03050; }
.tm-card-top { display: flex; align-items: flex-start; gap: 6px; }
.tm-card-title { font-size: 15px; line-height: 1.5; word-break: break-word; }
.tm-pri { font-size: 11px; font-weight: 700; padding: 1px 6px; border-radius: 4px; color: #fff; flex: 0 0 auto; }
.pri-A { background: #d03050; } .pri-B { background: #f0a020; } .pri-C { background: #909399; }
.tm-card-meta { display: flex; align-items: center; gap: 8px; margin-top: 9px; flex-wrap: wrap; font-size: 13px; color: var(--guaxi-text-secondary); }
.tm-date { display: inline-flex; align-items: center; gap: 2px; }
.tm-date.deadline { color: #d03050; }

/* 卡片内联：状态芯片 + 截止日期，透明原生控件覆盖承接点击 */
.tm-status-wrap { position: relative; display: inline-flex; }
.tm-status-chip { font-size: 11px; padding: 1px 7px; border-radius: 8px; background: var(--guaxi-header-bg); color: var(--guaxi-text-secondary); white-space: nowrap; }
.tm-status-chip.st-DONE { color: #18a058; background: rgba(24, 160, 88, 0.16); }
.tm-status-chip.st-DOING,
.tm-status-chip.st-NEXT { color: var(--tm-accent); background: var(--tm-accent-soft); }
.tm-status-chip.st-PAUSED,
.tm-status-chip.st-WAITING,
.tm-status-chip.st-HOLD { color: #b07a12; background: rgba(240, 160, 32, 0.14); }
.tm-status-chip.st-CANCELLED { color: #8a8f99; background: rgba(144, 147, 153, 0.16); }
.tm-status-overlay {
  position: absolute; inset: 0; width: 100%; height: 100%;
  margin: 0; padding: 0; border: none; background: transparent;
  opacity: 0; cursor: pointer; appearance: none; -webkit-appearance: none;
}
.tm-date-wrap { position: relative; display: inline-flex; align-items: center; gap: 2px; }
.tm-date-chip { display: inline-flex; align-items: center; gap: 2px; font-size: 11px; color: #d03050; white-space: nowrap; }
.tm-date-wrap.empty .tm-date-chip { color: var(--guaxi-text-secondary); opacity: 0.75; }
.tm-date-overlay {
  position: absolute; inset: 0; width: 100%; height: 100%;
  margin: 0; padding: 0; border: none; background: transparent;
  opacity: 0; cursor: pointer;
}
.tm-date-clear {
  position: relative; z-index: 2; border: none; background: transparent;
  color: var(--guaxi-text-secondary); cursor: pointer; font: inherit;
  font-size: 12px; line-height: 1; padding: 0 2px;
}
.tm-date-clear:hover { color: #d03050; }
:global([data-guaxi-theme="dark"]) .tm-date-overlay,
:global([data-guaxi-theme="dark"]) .tm-status-overlay { color-scheme: dark; }
.tm-type { font-size: 11px; padding: 0 6px; border-radius: 8px; color: #fff; }
.tm-type.tt-bug { background: #d03050; } .tm-type.tt-task { background: #2080f0; } .tm-type.tt-other { background: #8a8f99; }
.tm-did { font-size: 11px; color: var(--guaxi-text-secondary); opacity: 0.85; }
/* 源自邮件的任务：给一个可点的来源标记，点它回到原邮件。 */
.tm-mail-origin {
  font-size: 11px; padding: 1px 7px; border-radius: 9px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border) 80%, transparent);
  background: transparent; color: var(--guaxi-text-secondary);
  cursor: pointer; font-family: inherit; line-height: 1.6;
}
.tm-mail-origin:hover { color: #2080f0; border-color: #2080f0; }

/* 日历 */
.tm-calendar { flex: 1 1 auto; min-height: 0; padding: 12px; box-sizing: border-box; }
.tm-calendar :deep(.fc) {
  height: 100%;
  /* 统一用应用主题变量驱动 FullCalendar，修复暗色下亮色边框/列表项 */
  --fc-border-color: var(--guaxi-border);
  --fc-page-bg-color: var(--guaxi-app-bg);
  --fc-neutral-bg-color: var(--guaxi-panel-bg);
  --fc-neutral-text-color: var(--guaxi-text-secondary);
  --fc-today-bg-color: rgba(32, 128, 240, 0.10);
  --fc-list-event-hover-bg-color: var(--guaxi-panel-bg);
  --fc-highlight-color: rgba(32, 128, 240, 0.12);
  color: var(--guaxi-text-primary);
}
.tm-calendar :deep(.fc .fc-toolbar-title) { font-size: 16px; }
/* 上一月/下一月箭头：不依赖 FullCalendar 自带的 fcicons 字体。
   那个字体是以 data: URI 内嵌在运行时注入的 CSS 里的，而 release 版本的 Tauri CSP
   只有 default-src 'self'（没有 font-src），data: 字体会被拦掉，按钮就变成两个方块豆腐。
   CSP 已经补上 font-src data:，这里再用 SVG mask 把箭头画出来做双保险：
   用 currentColor 上色，亮/暗主题都跟着走，也不再受字体加载影响。 */
.tm-calendar :deep(.fc .fc-icon-chevron-left::before),
.tm-calendar :deep(.fc .fc-icon-chevron-right::before),
.tm-calendar :deep(.fc .fc-icon-chevrons-left::before),
.tm-calendar :deep(.fc .fc-icon-chevrons-right::before) {
  content: '';
  display: block;
  width: 1em;
  height: 1em;
  background-color: currentColor;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: 68% 68%;
  mask-size: 68% 68%;
}
.tm-calendar :deep(.fc .fc-icon-chevron-left::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M15 5 8 12l7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M15 5 8 12l7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevron-right::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M9 5l7 7-7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M9 5l7 7-7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevrons-left::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M18 5l-7 7 7 7M11 5l-7 7 7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M18 5l-7 7 7 7M11 5l-7 7 7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevrons-right::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M6 5l7 7-7 7M13 5l7 7-7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M6 5l7 7-7 7M13 5l7 7-7 7'/></svg>");
}
/* 周/日视图：弱化半小时虚线分隔，避免刺眼 */
.tm-calendar :deep(.fc .fc-timegrid-slot-minor) { border-top-style: dotted; border-top-color: color-mix(in srgb, var(--guaxi-border) 45%, transparent); }
.tm-calendar :deep(.fc .fc-timegrid-slot) { border-color: color-mix(in srgb, var(--guaxi-border) 60%, transparent); }
.tm-calendar :deep(.fc .fc-timegrid-now-indicator-line) { border-color: #d03050; }
/* 列表(年)视图：暗色下文字/表头颜色 */
.tm-calendar :deep(.fc .fc-list-day-cushion) { background: var(--guaxi-panel-bg); }
.tm-calendar :deep(.fc .fc-list-day-text),
.tm-calendar :deep(.fc .fc-list-day-side-text),
.tm-calendar :deep(.fc .fc-list-event-title),
.tm-calendar :deep(.fc .fc-list-event-time) { color: var(--guaxi-text-primary); }
.tm-calendar :deep(.fc .fc-list-empty) { background: var(--guaxi-app-bg); color: var(--guaxi-text-secondary); }
.tm-calendar :deep(.fc .fc-col-header-cell-cushion),
.tm-calendar :deep(.fc .fc-daygrid-day-number) { color: var(--guaxi-text-primary); }
.tm-calendar :deep(.fc .fc-day-other .fc-daygrid-day-number) { color: var(--guaxi-text-secondary); opacity: 0.6; }

/* 日历悬浮详情卡片 */
.tm-cal-tip {
  position: fixed; z-index: 9999; transform: translate(-50%, calc(-100% - 10px));
  min-width: 220px; max-width: 320px; padding: 10px 12px; border-radius: 10px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, 'Microsoft YaHei', monospace;
  font-size: 13px; pointer-events: none; box-shadow: 0 8px 28px rgba(0,0,0,0.28);
  border: 1px solid; line-height: 1.6;
}
.tm-cal-tip.light { background: #ffffff; color: #1f2328; border-color: #e3e6eb; }
.tm-cal-tip.dark { background: #282c34; color: #abb2bf; border-color: #3e4451; }
.tm-cal-tip .tip-head { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; flex-wrap: wrap; }
.tm-cal-tip .tip-title { font-weight: 700; font-size: 14px; flex: 1; min-width: 0; }
.tm-cal-tip .tip-type { font-size: 11px; padding: 1px 7px; border-radius: 9px; color: #fff; }
.tm-cal-tip .tt-bug { background: #d03050; } .tm-cal-tip .tt-task { background: #2080f0; } .tm-cal-tip .tt-other { background: #8a8f99; }
.tm-cal-tip .tip-pri { font-size: 11px; font-weight: 700; padding: 1px 6px; border-radius: 4px; color: #fff; }
.tm-cal-tip .pri-A { background: #d03050; } .tm-cal-tip .pri-B { background: #f0a020; } .tm-cal-tip .pri-C { background: #909399; }
.tm-cal-tip .tip-row { display: flex; gap: 8px; }
.tm-cal-tip .tip-k { color: var(--guaxi-text-secondary, #8b94a3); flex: 0 0 44px; }
.tm-cal-tip .tip-v { word-break: break-word; }

/* 统计 */
.tm-stats { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 16px; box-sizing: border-box; }
.tm-stat-cards { display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; margin-bottom: 16px; }
.tm-stat-card {
  background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 10px;
  padding: 16px; text-align: center;
}
.tm-stat-num { font-size: 28px; font-weight: 700; }
.tm-stat-label { font-size: 13px; color: var(--guaxi-text-secondary); margin-top: 4px; }
.tm-charts { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.tm-chart-box { background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 10px; height: 300px; padding: 8px; }
.tm-chart-wide { grid-column: 1 / -1; }
.tm-export-form { margin-bottom: 10px; }
.tm-export-preview-wrap { display: flex; flex-direction: column; height: calc(100% - 60px); min-height: 360px; }
.tm-export-preview-label { font-size: 12px; color: var(--guaxi-text-secondary); margin-bottom: 6px; }
.tm-export-textarea {
  flex: 1 1 auto; width: 100%; box-sizing: border-box; resize: vertical; min-height: 340px;
  background: var(--guaxi-app-bg); color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 12px 14px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  font-size: 13.5px; line-height: 1.7; white-space: pre; overflow: auto;
}
.tm-export-textarea:focus { outline: none; border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.18); }
.tm-edit-info { margin-top: 4px; }

/* 统计顶栏 + 按字段筛选导出 */
.tm-stats-bar { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
.tm-stats-title { font-weight: 700; font-size: 15px; }
.tm-spacer2 { flex: 1; }
.tm-fx { display: flex; flex-direction: column; gap: 12px; height: 100%; }
.tm-fx-filters { display: flex; flex-direction: column; gap: 8px; }
.tm-fx-row { display: flex; flex-wrap: wrap; gap: 10px; }
.tm-fx-row label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--guaxi-text-secondary); min-width: 120px; }
.tm-fx-row label.grow { flex: 1; min-width: 180px; }
.tm-fx-stat { background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 8px 12px; font-size: 13px; }
.tm-fx-stat b { color: #2080f0; font-size: 15px; }
.tm-fx-fields { border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 10px 12px; }
.tm-fx-fields-head { display: flex; align-items: center; font-size: 12px; color: var(--guaxi-text-secondary); margin-bottom: 8px; }
.tm-fx-fields-head a { color: #2080f0; cursor: pointer; }
.tm-fx-fields-count { margin-left: 8px; opacity: 0.75; }

/* 字段条件（任意默认/自定义字段 + 操作符） */
.tm-fx-conds { border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 10px 12px; }
.tm-fx-cond { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.tm-fx-cond .fx-cond-field { flex: 0 0 150px; }
.tm-fx-cond .fx-cond-op { flex: 0 0 110px; }
.tm-fx-cond .fx-cond-val { flex: 1 1 auto; min-width: 120px; }
.tm-fx-cond .fx-cond-date { flex: 0 0 150px; }
.tm-fx-hint { font-size: 12px; color: var(--guaxi-text-secondary); opacity: 0.85; line-height: 1.6; }
.tm-fx-chk { display: inline-flex; align-items: center; gap: 4px; font-size: 13px; margin: 3px 12px 3px 0; cursor: pointer; }
.tm-fx-opts { font-size: 13px; }
.tm-fx-preview-wrap { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 220px; }

/* ── 三视图骨架 ── */
.task-manager-page { --tm-accent: #3f6ea8; --tm-accent-soft: rgba(63, 110, 168, 0.14); }
.tm-sync { font-size: 11px; color: var(--guaxi-text-secondary); white-space: nowrap; opacity: 0.8; }
.tm-reminder-pop { width: 240px; display: flex; flex-direction: column; gap: 10px; padding: 4px 2px; }
.tm-reminder-row { display: flex; align-items: center; justify-content: space-between; gap: 10px; font-size: 13px; }
.tm-reminder-hint { font-size: 11px; line-height: 1.6; color: var(--guaxi-text-secondary); }

/* ── 今日 ── */
.tm-today {
  flex: 1 1 auto; min-height: 0; overflow-y: auto;
  display: grid; grid-template-columns: minmax(0, 820px) 300px;
  justify-content: center; align-content: start; gap: 24px;
  padding: 22px 24px 40px; box-sizing: border-box;
}
.tm-today-main { min-width: 0; }
.tm-today-side { display: flex; flex-direction: column; gap: 14px; position: sticky; top: 0; align-self: start; }
.tm-today-head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
.tm-today-date { display: flex; align-items: baseline; gap: 12px; }
.tm-today-day { font-size: 52px; line-height: 1; font-weight: 700; letter-spacing: -0.03em; color: var(--guaxi-text-primary); }
.tm-today-meta { display: flex; flex-direction: column; gap: 2px; font-size: 12px; color: var(--guaxi-text-secondary); }
.tm-today-meta b { font-size: 15px; color: var(--guaxi-text-primary); font-weight: 600; }
.tm-today-counts { display: flex; align-items: center; gap: 14px; font-size: 12px; color: var(--guaxi-text-secondary); }
.tm-today-counts b { font-size: 17px; color: var(--guaxi-text-primary); font-weight: 700; margin-right: 2px; }
.tm-today-counts .danger b { color: #d03050; }

.tm-capture {
  display: flex; align-items: center; gap: 8px;
  border: 1px solid var(--guaxi-border); border-radius: 12px;
  background: var(--guaxi-panel-bg); padding: 7px 10px 7px 12px;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.tm-capture:focus-within { border-color: var(--tm-accent); box-shadow: 0 0 0 3px var(--tm-accent-soft); }
.tm-capture-plus { color: var(--guaxi-text-secondary); font-size: 18px; line-height: 1; }
.tm-capture-input { flex: 1; min-width: 0; border: none; outline: none; background: transparent; color: var(--guaxi-text-primary); font: inherit; font-size: 15px; padding: 4px 0; }
.tm-capture-input::placeholder { color: var(--guaxi-text-secondary); opacity: 0.75; }
.tm-capture-sel { border: 1px solid var(--guaxi-border); border-radius: 8px; background: transparent; color: var(--guaxi-text-secondary); font: inherit; font-size: 12px; padding: 3px 6px; }
.tm-capture-btn { border: none; border-radius: 8px; background: var(--tm-accent); color: #fff; font: inherit; font-size: 13px; padding: 6px 14px; cursor: pointer; }
.tm-capture-btn:disabled { opacity: 0.45; cursor: default; }
.tm-capture-hint { margin: 6px 2px 0; font-size: 11px; color: var(--guaxi-text-secondary); }

.tm-block { margin-top: 22px; }
.tm-block-head { display: flex; align-items: baseline; gap: 8px; font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: var(--guaxi-text-secondary); padding-bottom: 7px; border-bottom: 1px solid var(--guaxi-border); }
.tm-block-head .danger { color: #d03050; }
.tm-block-sub { font-size: 11px; font-weight: 400; opacity: 0.75; }
.tm-hint { padding: 12px 2px; font-size: 12px; color: var(--guaxi-text-secondary); }
.tm-row { display: flex; align-items: center; gap: 10px; padding: 9px 6px; border-bottom: 1px dashed color-mix(in srgb, var(--guaxi-border) 55%, transparent); cursor: pointer; border-radius: 6px; }
.tm-row:hover { background: color-mix(in srgb, var(--guaxi-panel-bg) 70%, transparent); }
.tm-row.selected { background: var(--tm-accent-soft); box-shadow: inset 2px 0 0 var(--tm-accent); }
.tm-row.overdue .tm-row-title { color: #d03050; }
.tm-row-focus { padding-left: 2px; }
.tm-focus-no { width: 18px; text-align: center; font-weight: 700; color: var(--tm-accent); font-size: 13px; }
.tm-check {
  width: 20px; height: 20px; flex: 0 0 20px; border-radius: 50%;
  border: 1.5px solid var(--guaxi-border); background: transparent; color: transparent;
  cursor: pointer; font-size: 11px; line-height: 1; padding: 0;
}
.tm-check:hover { border-color: var(--tm-accent); color: var(--tm-accent); }
.tm-check.on { background: var(--tm-accent); border-color: var(--tm-accent); color: #fff; }
.tm-row-main { flex: 1; min-width: 0; }
.tm-row-title { font-size: 14px; line-height: 1.55; word-break: break-word; }
.tm-row-meta { margin-top: 3px; font-size: 11px; color: var(--guaxi-text-secondary); }
.tm-row-act { border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font: inherit; font-size: 15px; padding: 2px 8px; border-radius: 6px; opacity: 0; }
.tm-row:hover .tm-row-act { opacity: 1; }
.tm-row-act:hover { background: var(--guaxi-header-bg); color: var(--tm-accent); }

.tm-habits { display: flex; flex-wrap: wrap; gap: 8px; padding-top: 10px; }
.tm-habit {
  display: inline-flex; align-items: center; gap: 8px;
  border: 1px solid var(--guaxi-border); border-radius: 10px;
  background: var(--guaxi-panel-bg); color: var(--guaxi-text-primary);
  font: inherit; font-size: 13px; padding: 7px 11px; cursor: pointer;
}
.tm-habit:hover { border-color: var(--tm-accent); }
.tm-habit.done { border-color: color-mix(in srgb, var(--tm-accent) 55%, transparent); background: var(--tm-accent-soft); }
.tm-habit-mark { color: var(--guaxi-text-secondary); }
.tm-habit.done .tm-habit-mark { color: var(--tm-accent); }
.tm-habit-streak { font-size: 11px; color: var(--guaxi-text-secondary); }

.tm-review {
  margin-top: 10px; padding: 14px 16px; border-left: 3px solid var(--tm-accent);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 75%, transparent);
  border-radius: 0 10px 10px 0; cursor: pointer;
}
.tm-review-text {
  font-family: Georgia, 'Songti SC', 'STSong', 'SimSun', serif;
  font-size: 15px; line-height: 1.9; color: var(--guaxi-text-primary);
  white-space: pre-wrap; word-break: break-word;
}
.tm-review-edit { display: inline-block; margin-top: 8px; font-size: 11px; color: var(--tm-accent); }
.tm-review-start {
  margin-top: 10px; width: 100%; text-align: left; cursor: pointer;
  border: 1px dashed var(--guaxi-border); border-radius: 10px; background: transparent;
  color: var(--guaxi-text-secondary); font: inherit; font-size: 13px; padding: 14px 16px;
}
.tm-review-start:hover { border-color: var(--tm-accent); color: var(--tm-accent); }

.tm-side-card { border: 1px solid var(--guaxi-border); border-radius: 12px; background: var(--guaxi-panel-bg); padding: 14px; }
.tm-side-title { font-size: 12px; font-weight: 700; letter-spacing: 0.05em; color: var(--guaxi-text-secondary); margin-bottom: 10px; }
.tm-side-stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.tm-side-stats div { display: flex; flex-direction: column; gap: 2px; }
.tm-side-stats b { font-size: 20px; font-weight: 700; }
.tm-side-stats span { font-size: 11px; color: var(--guaxi-text-secondary); }
.tm-recent { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; padding: 6px 0; cursor: pointer; }
.tm-recent-title { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tm-recent-date { font-size: 11px; color: var(--guaxi-text-secondary); flex: 0 0 auto; }
.tm-side-reminder { font-size: 14px; margin-bottom: 4px; }
.tm-side-hint { font-size: 11px; line-height: 1.6; color: var(--guaxi-text-secondary); }

/* ── 计划 ── */
.tm-plan { flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column; }
.tm-plan-bar { display: flex; align-items: center; gap: 10px; padding: 10px 14px 0; flex-wrap: wrap; }
.tm-plan-capture {
  flex: 0 1 340px; min-width: 180px;
  border: 1px solid var(--guaxi-border); border-radius: 8px;
  background: var(--guaxi-panel-bg); color: var(--guaxi-text-primary);
  font: inherit; font-size: 12px; padding: 5px 10px; outline: none;
}
.tm-plan-capture:focus { border-color: var(--tm-accent); box-shadow: 0 0 0 2px var(--tm-accent-soft); }
.tm-plan-capture::placeholder { color: var(--guaxi-text-secondary); }
.tm-plan-hint { font-size: 11px; color: var(--guaxi-text-secondary); }
.tm-col-add { border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font: inherit; font-size: 16px; line-height: 1; padding: 0 4px; border-radius: 6px; }
.tm-col-add:hover { background: var(--guaxi-header-bg); color: var(--tm-accent); }
.tm-col-more { border: 1px solid var(--guaxi-border); background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font: inherit; font-size: 10px; padding: 1px 6px; border-radius: 7px; }
.tm-col-more:hover { border-color: var(--tm-accent); color: var(--tm-accent); }
.tm-card-acts { display: inline-flex; gap: 2px; margin-left: auto; opacity: 0; }
.tm-card:hover .tm-card-acts { opacity: 1; }
.tm-card-acts button { border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font: inherit; font-size: 12px; padding: 1px 5px; border-radius: 5px; }
.tm-card-acts button:hover { background: var(--guaxi-header-bg); color: var(--tm-accent); }
.tm-card-rename { flex: 1; min-width: 0; font: inherit; font-size: 15px; line-height: 1.5; color: var(--guaxi-text-primary); background: var(--guaxi-panel-bg); border: 1px solid var(--tm-accent); border-radius: 6px; padding: 1px 6px; outline: none; }
.tm-cat { font-size: 11px; padding: 0 6px; border-radius: 8px; border: 1px solid var(--guaxi-border); }
.tm-cat.cat-work { color: #8b6bd0; border-color: rgba(139, 107, 208, 0.4); }
.tm-cat.cat-life { color: #2f9e78; border-color: rgba(47, 158, 120, 0.4); }
.tm-cat.cat-study { color: #3f6ea8; border-color: rgba(63, 110, 168, 0.4); }
.tm-kind { font-size: 11px; padding: 0 6px; border-radius: 8px; background: var(--guaxi-header-bg); color: var(--guaxi-text-secondary); }

.tm-week { flex: 1 1 auto; min-height: 0; display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; padding: 14px; box-sizing: border-box; overflow: hidden; }
.tm-week-lane { display: flex; flex-direction: column; min-height: 0; border: 1px solid var(--guaxi-border); border-radius: 10px; background: var(--guaxi-panel-bg); overflow: hidden; }
.tm-week-head { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; padding: 10px 12px; font-size: 14px; font-weight: 700; border-bottom: 2px solid var(--guaxi-border); flex: 0 0 auto; }
.tm-week-head.overdue { color: #d03050; border-bottom-color: #d03050; }
.tm-week-head.this { color: var(--tm-accent); border-bottom-color: var(--tm-accent); }
.tm-week-body { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 8px; }

.tm-goals { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 18px; }
.tm-goals-head { display: flex; align-items: center; gap: 10px; font-size: 14px; font-weight: 700; margin-bottom: 14px; }
.tm-goal { border: 1px solid var(--guaxi-border); border-radius: 12px; background: var(--guaxi-panel-bg); padding: 14px 16px; margin-bottom: 12px; cursor: pointer; }
.tm-goal:hover { border-color: color-mix(in srgb, var(--tm-accent) 45%, var(--guaxi-border)); }
.tm-goal-head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
.tm-goal-title { font-size: 15px; font-weight: 700; }
.tm-goal-pct { font-size: 18px; font-weight: 700; color: var(--tm-accent); }
.tm-goal-bar { height: 4px; border-radius: 4px; background: var(--guaxi-header-bg); margin: 10px 0 8px; overflow: hidden; }
.tm-goal-bar span { display: block; height: 100%; background: var(--tm-accent); border-radius: 4px; transition: width 0.25s ease; }
.tm-goal-meta { font-size: 11px; color: var(--guaxi-text-secondary); }
.tm-goal-children { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
.tm-goal-child { font-size: 11px; padding: 2px 8px; border-radius: 8px; background: var(--guaxi-header-bg); color: var(--guaxi-text-secondary); }
.tm-goal-child.done { text-decoration: line-through; opacity: 0.6; }
.tm-goal-mini { padding: 7px 0; cursor: pointer; }
.tm-goal-mini-head { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; font-size: 12px; }

/* ── 年轮 ── */
.tm-year { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 16px 18px 32px; box-sizing: border-box; }
.tm-year-head { display: flex; align-items: baseline; gap: 10px; margin-bottom: 14px; }
.tm-year-sub { font-size: 12px; color: var(--guaxi-text-secondary); display: inline-flex; align-items: center; gap: 4px; }
.tm-year-sub b { color: var(--guaxi-text-primary); font-size: 13px; }
.tm-year-nav { border: 1px solid var(--guaxi-border); background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font: inherit; font-size: 13px; line-height: 1; padding: 1px 6px; border-radius: 6px; }
.tm-year-nav:hover:not(:disabled) { border-color: var(--tm-accent); color: var(--tm-accent); }
.tm-year-nav:disabled { opacity: 0.35; cursor: default; }
.tm-year-grid { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 16px; align-items: start; }
.tm-year-main { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
.tm-year-side { display: flex; flex-direction: column; gap: 14px; }
.tm-year-card { border: 1px solid var(--guaxi-border); border-radius: 12px; background: var(--guaxi-panel-bg); padding: 16px; }
.tm-year-stats { display: grid; grid-template-columns: repeat(5, 1fr); gap: 10px; }
.tm-year-stats .tm-stat-card { padding: 12px; }
.tm-year-stats .tm-stat-num { font-size: 22px; }
.tm-day-detail { margin-top: 14px; border-top: 1px dashed var(--guaxi-border); padding-top: 12px; }
.tm-day-head { display: flex; align-items: baseline; gap: 10px; font-size: 13px; font-weight: 700; margin-bottom: 6px; }
.tm-muted { font-size: 11px; color: var(--guaxi-text-secondary); font-weight: 400; }
.tm-day-row { display: flex; align-items: center; gap: 8px; font-size: 12px; padding: 5px 0; cursor: pointer; }
.tm-day-row.habit { color: var(--guaxi-text-secondary); }
.tm-day-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--tm-accent); flex: 0 0 5px; }
.tm-cat-bars { display: flex; flex-direction: column; gap: 10px; margin-top: 6px; }
.tm-cat-bar { display: grid; grid-template-columns: 46px 1fr 64px; align-items: center; gap: 10px; font-size: 12px; }
.tm-cat-bar-label { color: var(--guaxi-text-secondary); }
.tm-cat-bar-track { height: 6px; border-radius: 6px; background: var(--guaxi-header-bg); overflow: hidden; }
.tm-cat-bar-track i { display: block; height: 100%; background: var(--tm-accent); border-radius: 6px; }
.tm-cat-bar-num { text-align: right; color: var(--guaxi-text-secondary); font-size: 11px; }
.tm-habit-row { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; padding: 6px 0; cursor: pointer; font-size: 12px; }
.tm-habit-row .tm-habit-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tm-review-mini { display: flex; gap: 8px; padding: 6px 0; cursor: pointer; font-size: 12px; }
.tm-review-date { flex: 0 0 auto; color: var(--guaxi-text-secondary); font-size: 11px; }
.tm-review-excerpt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

@media (max-width: 1100px) {
  .tm-today { grid-template-columns: minmax(0, 1fr); padding: 18px; }
  .tm-today-side { position: static; flex-direction: row; flex-wrap: wrap; }
  .tm-today-side .tm-side-card { flex: 1 1 220px; }
  .tm-year-grid { grid-template-columns: minmax(0, 1fr); }
  .tm-year-stats { grid-template-columns: repeat(3, 1fr); }
  .tm-week { grid-template-columns: repeat(2, 1fr); }
}
</style>
