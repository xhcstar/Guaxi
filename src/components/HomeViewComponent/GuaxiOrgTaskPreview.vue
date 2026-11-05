<template>
  <div class="org-task-preview" :class="theme === 'dark' ? 'theme-dark' : 'theme-light'" @click="onContentLinkClick">
    <div class="otp-toolbar">
      <button v-if="canBack" class="otp-btn icon" title="返回" @click="$emit('back')">‹</button>
      <span class="otp-title">{{ titleLabel || '任务预览' }}</span>
      <span class="otp-count">{{ displayBlocks.length }}{{ filterView !== 'all' ? '/' + blocks.length : '' }} 项</span>
      <button
        class="otp-btn icon view-filter"
        :class="{ on: filterView !== 'all' }"
        :title="filterViewTitle"
        @click="cycleFilterView"
      >◉ {{ filterViewLabel }}</button>
      <span class="otp-spacer" />
      <button
        class="otp-btn icon"
        :title="activeBlock ? '上移选中卡片' : '请先点击选中一个卡片'"
        :disabled="!canMoveUp"
        @click="moveActive(-1)"
      >↑</button>
      <button
        class="otp-btn icon"
        :title="activeBlock ? '下移选中卡片' : '请先点击选中一个卡片'"
        :disabled="!canMoveDown"
        @click="moveActive(1)"
      >↓</button>
      <button class="otp-btn icon" title="批量选择" :class="{ on: batchMode }" @click="toggleBatchMode">☑</button>
      <button class="otp-btn icon" title="新建 Bug" @click="$emit('create-block', 'bug')">＋B</button>
      <button class="otp-btn icon" title="新建任务" @click="$emit('create-block', 'task')">＋T</button>
    </div>

    <!-- 批量操作栏 -->
    <div v-if="batchMode" class="otp-batchbar">
      <label class="otp-batch-all"><input type="checkbox" :checked="allSelected" @change="toggleSelectAll" /> 全选</label>
      <span class="otp-batch-count">已选 {{ selectedBlocks.length }}/{{ displayBlocks.length }}</span>
      <span class="otp-spacer" />
      <button class="otp-btn sm" :disabled="!selectedBlocks.length" title="批量上移" @click="batchMove(-1)">↑ 上移</button>
      <button class="otp-btn sm" :disabled="!selectedBlocks.length" title="批量下移" @click="batchMove(1)">↓ 下移</button>
      <button class="otp-btn sm danger" :disabled="!selectedBlocks.length" title="批量删除" @click="batchDelete">🗑 删除</button>
      <span class="otp-batch-sep" />
      <select v-model="batchCreateType" class="otp-sel" title="新建类型">
        <option v-for="t in typeOptions" :key="t" :value="t">{{ typeLabel(t) }}</option>
      </select>
      <input v-model="batchCreateCount" type="number" min="1" max="20" class="otp-batch-num" title="数量" />
      <button class="otp-btn sm" title="批量新建（追加到文档末尾）" @click="batchCreate">＋ 批量新建</button>
    </div>

    <!-- 搜索栏 -->
    <div class="otp-searchbar">
      <select v-model="searchField" class="otp-sel" title="搜索字段">
        <option value="all">全部字段</option>
        <option value="description">描述</option>
        <option value="id">ID</option>
        <option value="rootCause">根本原因</option>
        <option value="conditionOfOccurrence">复现步骤</option>
        <option value="fixedDetails">修复详情</option>
        <option value="testingSuggestions">测试建议</option>
        <option value="content">详情</option>
        <option value="notes">笔记</option>
        <option value="status">状态</option>
        <option value="fixStatus">修复状态</option>
        <option value="severity">严重度</option>
        <option value="origin">来源</option>
        <option value="createTime">创建时间</option>
        <option value="completedTime">完成时间</option>
        <option value="affectedRelease">影响版本</option>
      </select>
      <input v-model="search" class="otp-search" placeholder="搜索…" @keydown.enter.prevent="gotoMatch(1)" />
      <span class="otp-search-count">{{ occurrences.length ? (matchIdx + 1) + '/' + occurrences.length : '0/0' }}</span>
      <button class="otp-btn icon sm" title="上一个" :disabled="!occurrences.length" @click="gotoMatch(-1)">↑</button>
      <button class="otp-btn icon sm" title="下一个" :disabled="!occurrences.length" @click="gotoMatch(1)">↓</button>
    </div>

    <div class="otp-main">
      <div class="otp-body" ref="bodyRef" @scroll.passive="emit('scroll')">
        <div v-if="diagnostics.length" class="otp-diags">
          <div class="otp-diags-head">⚠ 语法检查（{{ errorCount }} 错误 / {{ warnCount }} 警告）</div>
          <div v-for="(d, di) in diagnostics" :key="di" class="otp-diag" :class="d.severity" @click="$emit('goto-line', d.line)">
            <span class="otp-diag-line">行 {{ d.line }}</span><span class="otp-diag-msg">{{ d.message }}</span>
          </div>
        </div>

        <div v-if="!blocks.length" class="otp-empty">
          暂无任务块。点击工具栏 <b>＋B / ＋T</b> 新建，或在编辑器输入 <code>##</code> 选择模板。
        </div>

        <div
          v-for="(b, idx) in displayBlocks"
          :key="b.internalId || b.blockStartLine || idx"
          class="otp-card"
          :class="{ done: b.done, active: b.blockStartLine === activeBlockLine, batch: batchMode, selected: isSelected(b) }"
          :data-block-line="b.blockStartLine"
          :ref="(el) => registerCardRef(b.blockStartLine, el)"
          @click="onCardClick(b, $event)"
        >
          <label v-if="batchMode" class="otp-card-check" @click.stop>
            <input type="checkbox" :checked="isSelected(b)" @change="toggleSelect(b)" />
          </label>
          <!-- ============ 读视图 ============ -->
          <template v-if="editingLine !== b.blockStartLine">
            <!-- 上方插入 -->
            <div class="otp-ins top">
              <span class="otp-ins-label">↑ 在上方插入</span>
              <button class="otp-ins-btn bug" title="在上方新建 Bug" @click="emit('insert-block', { taskType: 'bug', where: 'above', block: b })">＋Bug</button>
              <button class="otp-ins-btn task" title="在上方新建任务" @click="emit('insert-block', { taskType: 'task', where: 'above', block: b })">＋任务</button>
            </div>

            <div class="otp-card-head">
              <span class="otp-type" :class="'type-' + (b.taskType || 'task')">{{ typeLabel(b.taskType) }}</span>
              <!-- 优先级：点击弹原生下拉快速设置（无/A/B/C），透明 select 覆盖芯片承接点击 -->
              <span class="otp-pri-wrap">
                <span class="otp-pri" :class="b.priority ? 'pri-' + b.priority : 'pri-none'">{{ b.priority ? '#' + b.priority : '优先级' }}</span>
                <select class="otp-quick-overlay" :value="b.priority || ''" title="快速改优先级" @click.stop @change="(e) => quickApply(b, { priority: e.target.value })">
                  <option value="">无优先级</option>
                  <option value="A">A</option>
                  <option value="B">B</option>
                  <option value="C">C</option>
                </select>
              </span>
              <!-- 标题：单击定位到源码行，双击就地改描述 -->
              <input
                v-if="titleEditLine === b.blockStartLine"
                class="otp-title-input"
                :ref="(el) => registerTitleInput(b.blockStartLine, el)"
                v-model="titleDraft"
                @keydown.enter.prevent="commitTitleEdit(b)"
                @keydown.esc.prevent="cancelTitleEdit"
                @blur="commitTitleEdit(b)"
                @click.stop
              />
              <span v-else class="otp-card-title" title="单击定位 · 双击改标题" @click="$emit('goto-line', b.blockStartLine)" @dblclick.stop="startTitleEdit(b)" v-html="hl(b.description || b.id || '(未命名)')" />
              <button class="otp-mini xs" :class="{ ok: flashed === 'desc:' + b.blockStartLine }" title="复制描述" @click.stop="copyDesc(b)">{{ flashed === 'desc:' + b.blockStartLine ? '✓' : '❏' }}</button>
              <!-- 状态：点击弹原生下拉快速切换 -->
              <span class="otp-status-wrap">
                <span class="otp-status" :class="'st-' + (b.status || 'TODO')" v-html="hl(statusLabel(b.status))" />
                <select class="otp-quick-overlay" :value="b.status || 'TODO'" title="快速改状态" @click.stop @change="(e) => quickApply(b, { status: e.target.value })">
                  <option v-for="s in statusKeys" :key="s" :value="s">{{ statusLabel(s) }}</option>
                </select>
              </span>
              <span class="otp-actions">
                <button class="otp-mini" title="大窗预览整条" @click.stop="openCardZoom(b)">⛶</button>
                <button class="otp-mini" title="编辑" @click="startEdit(b)">✎</button>
                <button class="otp-mini" :class="{ ok: flashed === 'copy:' + b.blockStartLine }" title="复制 Bug 信息" @click="copyCard(b)">{{ flashed === 'copy:' + b.blockStartLine ? '✓' : '⧉' }}</button>
                <button class="otp-mini" :class="{ ok: flashed === 'export:' + b.blockStartLine }" title="导出提交信息" @click="exportCard(b)">{{ flashed === 'export:' + b.blockStartLine ? '✓' : '⇪' }}</button>
                <button class="otp-mini danger" title="删除" @click="$emit('delete-block', b)">🗑</button>
              </span>
            </div>

            <div class="otp-props">
              <div v-if="b.id" class="otp-prop"><span class="k">ID</span><span class="v" v-html="hl(b.id)" /></div>
              <div v-if="b.parentId" class="otp-prop"><span class="k">父任务</span><span class="v" v-html="hl(b.parentId)" /></div>
              <div v-if="b.severity" class="otp-prop"><span class="k">严重度</span><span class="v" v-html="hl(b.severity)" /></div>
              <div v-if="b.origin" class="otp-prop"><span class="k">来源</span><span class="v" v-html="hl(b.origin)" /></div>
              <div v-if="b.fixStatus" class="otp-prop"><span class="k">修复</span><span class="v" v-html="hl(b.fixStatus)" /></div>
              <div v-if="b.progress" class="otp-prop"><span class="k">进度</span><span class="v" v-html="hl(b.progress)" /></div>
              <div v-if="b.deadline" class="otp-prop"><span class="k">截止</span><span class="v" v-html="hl(b.deadline)" /></div>
              <div v-if="b.createTime" class="otp-prop"><span class="k">创建</span><span class="v" v-html="hl(b.createTime)" /></div>
              <div v-if="b.completedTime" class="otp-prop done-t"><span class="k">完成</span><span class="v" v-html="hl(b.completedTime)" /></div>
              <div v-if="b.affectedRelease" class="otp-prop"><span class="k">影响版本</span><span class="v" v-html="hl(b.affectedRelease)" /></div>
            </div>

            <!-- 可折叠分区 -->
            <section v-for="sec in sectionsOf(b)" :key="sec.key" class="otp-sec" :data-sec-key="sec.key">
              <div class="otp-sec-head" @click="toggleSec(b.blockStartLine, sec.key, sec.defaultCollapsed)">
                <span class="otp-sec-caret">{{ isCollapsed(b.blockStartLine, sec.key, sec.defaultCollapsed) ? '▸' : '▾' }}</span>
                <h4>{{ sec.title }}</h4>
                <span class="otp-sec-actions">
                  <button class="otp-mini xs" :class="{ ok: flashed === 'sec:' + b.blockStartLine + ':' + sec.key }" title="复制本节" @click.stop="copySection(sec, b.blockStartLine)">{{ flashed === 'sec:' + b.blockStartLine + ':' + sec.key ? '✓' : '⧉' }}</button>
                </span>
              </div>
              <div v-show="!isCollapsed(b.blockStartLine, sec.key, sec.defaultCollapsed)" class="otp-sec-body">
                <GuaxiOrgMarkdown
                  v-if="sec.type === 'text' || sec.type === 'pre' || sec.type === 'md'"
                  class="otp-md"
                  :content="sec.value"
                  :base-path="documentPath"
                  :theme="theme"
                  :query="search"
                />
                <a v-else-if="sec.type === 'link'" class="otp-link" :href="sec.value" target="_blank" rel="noreferrer">{{ sec.value }}</a>
                <template v-else-if="sec.type === 'notes'">
                  <GuaxiOrgMarkdown
                    v-if="sec.value.kind === 'inline'"
                    class="otp-md"
                    :content="sec.value.value"
                    :base-path="documentPath"
                    :theme="theme"
                    :query="search"
                  />
                  <a v-else class="otp-notes-file" href="#" @click.prevent="$emit('open-notes', sec.value.value)">📄 <span v-html="hl(sec.value.value)" />（点击打开）</a>
                </template>
                <div v-else-if="sec.type === 'extra'" class="otp-extra-list">
                  <GuaxiOrgMarkdown
                    v-for="(ei, i) in sec.value"
                    :key="i"
                    class="otp-md otp-extra-item"
                    :content="ei"
                    :base-path="documentPath"
                    :theme="theme"
                    :query="search"
                  />
                </div>
              </div>
            </section>

            <!-- 下方插入 -->
            <div class="otp-ins bottom">
              <span class="otp-ins-label">↓ 在下方插入</span>
              <button class="otp-ins-btn bug" title="在下方新建 Bug" @click="emit('insert-block', { taskType: 'bug', where: 'below', block: b })">＋Bug</button>
              <button class="otp-ins-btn task" title="在下方新建任务" @click="emit('insert-block', { taskType: 'task', where: 'below', block: b })">＋任务</button>
            </div>
          </template>

          <!-- ============ 编辑视图 ============ -->
          <template v-else>
            <div class="otp-edit-head">
              <strong>编辑任务</strong>
              <span class="otp-spacer" />
              <button class="otp-btn primary" @click="saveEdit">保存</button>
              <button class="otp-btn" @click="cancelEdit">取消</button>
            </div>
            <div class="otp-form">
              <label>类型<select v-model="draft.taskType"><option v-for="t in typeOptions" :key="t" :value="t">{{ typeLabel(t) }}</option></select></label>
              <label>状态<select v-model="draft.status"><option v-for="s in statusKeys" :key="s" :value="s">{{ statusLabel(s) }}</option></select></label>
              <label>优先级<select v-model="draft.priority"><option value="">未指定</option><option value="A">A</option><option value="B">B</option><option value="C">C</option></select></label>
              <label>严重度<select v-model="draft.severity"><option v-for="s in severityOptions" :key="s" :value="s">{{ s }}</option></select></label>
              <label>来源<select v-model="draft.origin"><option v-for="o in originOptions" :key="o" :value="o">{{ o }}</option></select></label>
              <label>修复状态<select v-model="draft.fixStatus"><option v-for="o in fixOptions" :key="o" :value="o">{{ o }}</option></select></label>
              <label>进度<input v-model="draft.progress" placeholder="50%" /></label>
              <label>ID<input v-model="draft.id" placeholder="bug/task id" /></label>
              <label>父任务<input v-model="draft.parentId" /></label>
              <label>截止
                <div class="otp-date-field">
                  <input v-model="draft.deadline" placeholder="2026年06月20日" />
                  <span class="otp-date-icon" aria-hidden="true">📅</span>
                  <input
                    type="date"
                    class="otp-date-overlay"
                    title="日历选择截止日期"
                    :value="toDateInput(draft.deadline)"
                    @change="onPickDeadline"
                  />
                </div>
              </label>
              <label>创建时间
                <div class="otp-date-field">
                  <input v-model="draft.createTime" placeholder="2026年06月20日 14:30:00" />
                  <span class="otp-date-icon" aria-hidden="true">📅</span>
                  <input
                    type="datetime-local"
                    step="1"
                    class="otp-date-overlay"
                    title="日历选择创建时间"
                    :value="toDateTimeInput(draft.createTime)"
                    @change="onPickCreateTime"
                  />
                </div>
              </label>
              <label>完成时间
                <div class="otp-date-field">
                  <input v-model="draft.completedTime" placeholder="留空则由软件自动计算" />
                  <span class="otp-date-icon" aria-hidden="true">📅</span>
                  <input
                    type="datetime-local"
                    step="1"
                    class="otp-date-overlay"
                    title="日历选择完成时间（留空则自动计算）"
                    :value="toDateTimeInput(draft.completedTime)"
                    @change="onPickCompletedTime"
                  />
                </div>
              </label>
              <label>影响版本<input v-model="draft.affectedRelease" /></label>
              <label class="full">描述<input :ref="setDescInput" v-model="draft.description" /></label>
              <label class="full">
                <span class="otp-field-head">根本原因<button type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('根本原因', draft.rootCause, (v) => (draft.rootCause = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-model="draft.rootCause"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':rootCause'"
                  :min-height="110"
                />
              </label>
              <label class="full">
                <span class="otp-field-head">复现步骤<button type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('复现步骤', draft.conditionOfOccurrence, (v) => (draft.conditionOfOccurrence = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-model="draft.conditionOfOccurrence"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':condition'"
                  :min-height="120"
                />
              </label>
              <label class="full">
                <span class="otp-field-head">修复详情<button type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('修复详情', draft.fixedDetails, (v) => (draft.fixedDetails = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-model="draft.fixedDetails"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':fixed'"
                  :min-height="120"
                />
              </label>
              <label class="full">
                <span class="otp-field-head">测试建议<button type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('测试建议', draft.testingSuggestions, (v) => (draft.testingSuggestions = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-model="draft.testingSuggestions"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':testing'"
                  :min-height="110"
                />
              </label>
              <label class="full">
                <span class="otp-field-head">详情 (CONTENT)<button type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('详情 (CONTENT)', draft.content, (v) => (draft.content = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-model="draft.content"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':content'"
                  :min-height="150"
                  :max-height="440"
                />
              </label>
              <label class="full">
                <span class="otp-field-head">笔记 (NOTES)<button v-if="draft.notes.kind !== 'file'" type="button" class="otp-mini xs otp-zoom" title="放大编辑" @click="openBigEdit('笔记 (NOTES)', draft.notes.value, (v) => (draft.notes.value = v))">⛶</button></span>
                <GuaxiFieldMarkdownEditor
                  v-if="draft.notes.kind !== 'file'"
                  v-model="draft.notes.value"
                  :theme="theme"
                  :document-path="documentPath"
                  :field-key="editingLine + ':notes'"
                  :min-height="150"
                  :max-height="440"
                />
                <input v-else v-model="draft.notes.value" />
              </label>
              <label class="full">Review 链接<input v-model="draft.reviewLink" /></label>
              <div class="full otp-ei-edit">
                <div class="otp-ei-head"><span>Extra Impact</span><button class="otp-mini" @click="draft.extraImpacts.push('')">＋</button></div>
                <div v-for="(ei, i) in draft.extraImpacts" :key="i" class="otp-ei-row">
                  <GuaxiFieldMarkdownEditor
                    v-model="draft.extraImpacts[i]"
                    :theme="theme"
                    :document-path="documentPath"
                    :field-key="editingLine + ':extra:' + i"
                    :min-height="96"
                  />
                  <div class="otp-ei-row-actions">
                    <button type="button" class="otp-mini" title="上移" :disabled="i === 0" @click="moveExtra(i, -1)">↑</button>
                    <button type="button" class="otp-mini" title="下移" :disabled="i === draft.extraImpacts.length - 1" @click="moveExtra(i, 1)">↓</button>
                    <button type="button" class="otp-mini" title="放大编辑" @click="openBigEdit('Extra Impact', draft.extraImpacts[i], (v) => (draft.extraImpacts[i] = v))">⛶</button>
                    <button type="button" class="otp-mini" title="删除" @click="draft.extraImpacts.splice(i, 1)">✕</button>
                  </div>
                </div>
              </div>
              <!-- 自定义字段：名(大写字母+空格) + 值；原样往返写回 .org -->
              <div class="full otp-cf-edit">
                <div class="otp-ei-head"><span>自定义字段</span><button class="otp-mini" title="新增自定义字段" @click="addCustomField">＋</button></div>
                <div v-if="!draft.customFields.length" class="otp-cf-hint">暂无自定义字段。字段名仅限大写字母与空格（如 <code>WEBUI BUG ID</code>），保存后原样写入 .org。</div>
                <div v-for="(cf, i) in draft.customFields" :key="i" class="otp-cf-row">
                  <input v-model="cf.name" class="otp-cf-name" placeholder="字段名(大写)" @blur="normalizeCfName(i)" />
                  <GuaxiFieldMarkdownEditor
                    v-model="cf.value"
                    class="otp-cf-val"
                    :theme="theme"
                    :document-path="documentPath"
                    :field-key="editingLine + ':cf:' + i"
                    :min-height="96"
                  />
                  <div class="otp-ei-row-actions">
                    <button type="button" class="otp-mini" title="放大编辑" @click="openBigEdit(cf.name || '自定义字段', cf.value, (v) => (cf.value = v))">⛶</button>
                    <button type="button" class="otp-mini" title="删除" @click="removeCustomField(i)">✕</button>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>

    <!-- 大窗预览浮层：把整条任务按阅读态展开到接近整窗大小。
         卡片列宽度有限，长正文（日志、表格）挤在里面很难读；这里复用同一套分区渲染，
         只换更大的排版尺度，不引入第二套内容渲染逻辑。 -->
    <div v-if="cardZoomBlock" class="otp-bigedit-mask" @mousedown.self="closeCardZoom">
      <div class="otp-bigedit otp-cardzoom">
        <div class="otp-bigedit-head">
          <strong>大窗预览</strong>
          <span class="otp-type" :class="'type-' + (cardZoomBlock.taskType || 'task')">{{ typeLabel(cardZoomBlock.taskType) }}</span>
          <span class="otp-cardzoom-title">{{ cardZoomBlock.description || cardZoomBlock.id || '(未命名)' }}</span>
          <span class="otp-spacer" />
          <button class="otp-btn" @click="$emit('goto-line', cardZoomBlock.blockStartLine)">定位源码</button>
          <button class="otp-btn" @click="startEditFromZoom">编辑</button>
          <button class="otp-btn" @click="closeCardZoom">关闭 (Esc)</button>
        </div>
        <div class="otp-cardzoom-body">
          <div class="otp-props">
            <div v-if="cardZoomBlock.id" class="otp-prop"><span class="k">ID</span><span class="v" v-html="hl(cardZoomBlock.id)" /></div>
            <div v-if="cardZoomBlock.parentId" class="otp-prop"><span class="k">父任务</span><span class="v" v-html="hl(cardZoomBlock.parentId)" /></div>
            <div class="otp-prop"><span class="k">状态</span><span class="v" v-html="hl(statusLabel(cardZoomBlock.status))" /></div>
            <div v-if="cardZoomBlock.priority" class="otp-prop"><span class="k">优先级</span><span class="v" v-html="hl('#' + cardZoomBlock.priority)" /></div>
            <div v-if="cardZoomBlock.severity" class="otp-prop"><span class="k">严重度</span><span class="v" v-html="hl(cardZoomBlock.severity)" /></div>
            <div v-if="cardZoomBlock.origin" class="otp-prop"><span class="k">来源</span><span class="v" v-html="hl(cardZoomBlock.origin)" /></div>
            <div v-if="cardZoomBlock.fixStatus" class="otp-prop"><span class="k">修复</span><span class="v" v-html="hl(cardZoomBlock.fixStatus)" /></div>
            <div v-if="cardZoomBlock.progress" class="otp-prop"><span class="k">进度</span><span class="v" v-html="hl(cardZoomBlock.progress)" /></div>
            <div v-if="cardZoomBlock.deadline" class="otp-prop"><span class="k">截止</span><span class="v" v-html="hl(cardZoomBlock.deadline)" /></div>
            <div v-if="cardZoomBlock.createTime" class="otp-prop"><span class="k">创建</span><span class="v" v-html="hl(cardZoomBlock.createTime)" /></div>
            <div v-if="cardZoomBlock.completedTime" class="otp-prop"><span class="k">完成</span><span class="v" v-html="hl(cardZoomBlock.completedTime)" /></div>
            <div v-if="cardZoomBlock.affectedRelease" class="otp-prop"><span class="k">影响版本</span><span class="v" v-html="hl(cardZoomBlock.affectedRelease)" /></div>
          </div>
          <section v-for="sec in sectionsOf(cardZoomBlock)" :key="sec.key" class="otp-sec">
            <div class="otp-sec-head static">
              <h4>{{ sec.title }}</h4>
            </div>
            <div class="otp-sec-body">
              <GuaxiOrgMarkdown
                v-if="sec.type === 'text' || sec.type === 'pre' || sec.type === 'md'"
                class="otp-md"
                :content="sec.value"
                :base-path="documentPath"
                :theme="theme"
                :query="search"
                immediate
              />
              <a v-else-if="sec.type === 'link'" class="otp-link" :href="sec.value" target="_blank" rel="noreferrer">{{ sec.value }}</a>
              <template v-else-if="sec.type === 'notes'">
                <GuaxiOrgMarkdown
                  v-if="sec.value.kind === 'inline'"
                  class="otp-md"
                  :content="sec.value.value"
                  :base-path="documentPath"
                  :theme="theme"
                  :query="search"
                  immediate
                />
                <a v-else class="otp-notes-file" href="#" @click.prevent="$emit('open-notes', sec.value.value)">📄 <span v-html="hl(sec.value.value)" />（点击打开）</a>
              </template>
              <div v-else-if="sec.type === 'extra'" class="otp-extra-list">
                <GuaxiOrgMarkdown
                  v-for="(ei, i) in sec.value"
                  :key="i"
                  class="otp-md otp-extra-item"
                  :content="ei"
                  :base-path="documentPath"
                  :theme="theme"
                  :query="search"
                  immediate
                />
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>

    <!-- 放大编辑浮层：把可编辑文本区展开到接近整窗大小，便于大段文本编辑 -->
    <div v-if="bigEdit.open" class="otp-bigedit-mask" @mousedown.self="cancelBigEdit">
      <div class="otp-bigedit">
        <div class="otp-bigedit-head">
          <strong>放大编辑 · {{ bigEdit.title }}</strong>
          <span class="otp-bigedit-views">
            <button class="otp-btn sm" :class="{ on: bigEdit.view === 'edit' }" @click="bigEdit.view = 'edit'">编辑</button>
            <button class="otp-btn sm" :class="{ on: bigEdit.view === 'split' }" @click="bigEdit.view = 'split'">并排</button>
            <button class="otp-btn sm" :class="{ on: bigEdit.view === 'preview' }" @click="bigEdit.view = 'preview'">预览</button>
          </span>
          <span class="otp-spacer" />
          <button class="otp-btn primary" @click="saveBigEdit">保存 (Ctrl+S)</button>
          <button class="otp-btn" @click="cancelBigEdit">取消</button>
        </div>
        <div class="otp-bigedit-body" :class="'view-' + bigEdit.view">
          <GuaxiFieldMarkdownEditor
            v-show="bigEdit.view !== 'preview'"
            ref="bigEditRef"
            v-model="bigEdit.text"
            class="otp-bigedit-area"
            :theme="theme"
            :document-path="documentPath"
            field-key="big-edit"
            :fill="true"
            :immediate="true"
          />
          <GuaxiOrgMarkdown
            v-show="bigEdit.view !== 'edit'"
            class="otp-md otp-bigedit-preview"
            :content="bigEdit.text"
            :base-path="documentPath"
            :theme="theme"
            immediate
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, nextTick, watch, onMounted, onBeforeUnmount } from 'vue'
import { parseTaskBlocks, validateTaskBlocks, serializeBlock, STATUS_LABELS } from '../../utils/guaxiTaskParser'
import { createLogger } from '../../utils/logger'
import GuaxiOrgMarkdown from './GuaxiOrgMarkdown.vue'
import GuaxiFieldMarkdownEditor from './GuaxiFieldMarkdownEditor.vue'

const logger = createLogger('GuaxiOrgTaskPreview')

const props = defineProps({
  content: { type: String, default: '' },
  category: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  titleLabel: { type: String, default: '任务预览' },
  activeBlockLine: { type: Number, default: 0 },
  canBack: { type: Boolean, default: false },
  /** 当前 .org 文件路径：本地图片与相对链接按它解析。 */
  documentPath: { type: String, default: '' },
})
const emit = defineEmits(['rebuild', 'open-notes', 'open-link', 'goto-line', 'apply-block', 'copy-block', 'export-block', 'back', 'create-block', 'delete-block', 'insert-block', 'move-block', 'batch-delete', 'batch-move', 'batch-create', 'scroll'])

// 字段正文里的链接一律不在 WebView 里导航：URL 交系统默认浏览器、本地路径按类型分流，
// 这些判定都在宿主 GuaxiEditor 里，与编辑 / 预览 / 探索者共用同一条链路。
// 这里只做拦截与上报原始 href（用 getAttribute 保住相对路径，a.href 会被解析成绝对 URL）。
const onContentLinkClick = (e) => {
  const anchor = e?.target?.closest?.('a')
  if (!anchor) return
  const href = String(anchor.getAttribute('href') || '').trim()
  // 空 href 与纯锚点没有可跳的目标；NOTES 的文件链接走 href="#" + open-notes 那条专门的路。
  if (!href || href === '#') return
  e.preventDefault()
  e.stopPropagation()
  emit('open-link', { href })
}

const blocks = computed(() => parseTaskBlocks(props.content, props.category))
// ── 视图过滤器：仅影响显示，不修改源文件或移动逻辑 ──
const filterView = ref('all') // 'all' | 'incomplete' | 'completed'
const filterViewLabel = computed(() => ({ all: '全部', incomplete: '未完成', completed: '已完成' }[filterView.value]))
const filterViewTitle = computed(() => `当前视图：${filterViewLabel.value}（点击切换）`)
const cycleFilterView = () => {
  filterView.value = filterView.value === 'all' ? 'incomplete' : filterView.value === 'incomplete' ? 'completed' : 'all'
}
// displayBlocks 用于卡片和大纲渲染；moveActive / 搜索 / 源文件操作仍使用 blocks。
const displayBlocks = computed(() => {
  if (filterView.value === 'all') return blocks.value
  if (filterView.value === 'completed') return blocks.value.filter((b) => b.done)
  return blocks.value.filter((b) => !b.done)
})
const diagnostics = computed(() => validateTaskBlocks(props.content, props.category))
const errorCount = computed(() => diagnostics.value.filter((d) => d.severity === 'error').length)
const warnCount = computed(() => diagnostics.value.filter((d) => d.severity === 'warning').length)

// ── 批量操作（多选 → 批量删除/上移下移/新建）──
// 选中集合按 blockStartLine 跟踪；任一批量操作后清空（源文件行号已变，避免悬挂选择）。
const batchMode = ref(false)
const selected = ref([])
const batchCreateType = ref('bug')
const batchCreateCount = ref(1)
const isSelected = (b) => selected.value.includes(b.blockStartLine)
const toggleSelect = (b) => {
  const i = selected.value.indexOf(b.blockStartLine)
  if (i >= 0) selected.value.splice(i, 1)
  else selected.value.push(b.blockStartLine)
}
const selectedBlocks = computed(() => displayBlocks.value.filter((b) => selected.value.includes(b.blockStartLine)))
const allSelected = computed(() => displayBlocks.value.length > 0 && selectedBlocks.value.length === displayBlocks.value.length)
const toggleSelectAll = () => {
  selected.value = allSelected.value ? [] : displayBlocks.value.map((b) => b.blockStartLine)
}
const toggleBatchMode = () => {
  batchMode.value = !batchMode.value
  if (!batchMode.value) selected.value = []
}
const batchDelete = () => {
  if (!selectedBlocks.value.length) return
  emit('batch-delete', selectedBlocks.value.slice())
  selected.value = []
}
const batchMove = (direction) => {
  if (!selectedBlocks.value.length) return
  emit('batch-move', { blocks: selectedBlocks.value.slice(), direction })
  selected.value = []
}
const batchCreate = () => {
  const count = Math.max(1, Math.min(20, parseInt(batchCreateCount.value, 10) || 1))
  emit('batch-create', { taskType: batchCreateType.value, count })
}

// ── 卡片上下移动 ──
// 选中态复用 activeBlockLine（点击卡片标题会 emit goto-line → 父组件移动光标 → 回传 activeBlockLine）。
const activeIndex = computed(() => blocks.value.findIndex((b) => b.blockStartLine === props.activeBlockLine))
const activeBlock = computed(() => (activeIndex.value >= 0 ? blocks.value[activeIndex.value] : null))
const canMoveUp = computed(() => activeIndex.value > 0)
const canMoveDown = computed(() => activeIndex.value >= 0 && activeIndex.value < blocks.value.length - 1)
const moveActive = (direction) => {
  if (!activeBlock.value) return
  if (direction < 0 && !canMoveUp.value) return
  if (direction > 0 && !canMoveDown.value) return
  emit('move-block', { block: activeBlock.value, direction: direction > 0 ? 1 : -1 })
}
// 点击卡片正文空白区域即选中该卡片（移动光标到块首行 → 父组件回传 activeBlockLine 高亮）。
// 跳过按钮/输入/链接等交互元素，避免拦截它们自身的点击；编辑态不触发。
const onCardClick = (b, ev) => {
  if (editingLine.value === b.blockStartLine) return
  const t = ev?.target
  if (t && typeof t.closest === 'function'
    && t.closest('button, input, textarea, select, a, .monaco-editor, .guaxi-field-md-editor, .otp-mini, .otp-ins-btn, .otp-sec-head, mark')) {
    return
  }
  // 批量模式：点击卡片即切换选中，不移动光标。
  if (batchMode.value) { toggleSelect(b); return }
  emit('goto-line', b.blockStartLine)
}

const statusKeys = ['TODO', 'NEXT', 'DOING', 'WAITING', 'PAUSED', 'HOLD', 'CANCELLED', 'DONE']
const severityOptions = ['critical', 'major', 'normal', 'minor', 'trivial']
const originOptions = ['dev', 'QA', 'customer', 'other']
const fixOptions = ['debug', 'test', 'review', 'approval', 'fixed', 'wontfix', 'doc', 'enhancement', 'invalid', 'remind', 'other']
const typeOptions = computed(() => (props.category === 'life' || props.category === 'study') ? ['task', 'other'] : ['bug', 'task', 'other'])

const typeLabel = (t) => ({ bug: 'Bug', task: '任务', other: '其他' }[t] || (t || '任务'))
const statusLabel = (s) => STATUS_LABELS[s] || s || '待办'
// 字段正文统一交给 GuaxiOrgMarkdown（编辑器同源渲染管线）；
// 搜索命中由该组件在渲染完成后按文本节点高亮，不再在父组件里拼 HTML。

// ── 分区（含顺序：Review 倒数第二、Extra Impact 最后且默认折叠）──
const sectionsOf = (b) => {
  const out = []
  if (b.rootCause) out.push({ key: 'rootCause', title: '根本原因', type: 'text', value: b.rootCause })
  if (b.conditionOfOccurrence) out.push({ key: 'condition', title: '复现步骤', type: 'pre', value: b.conditionOfOccurrence })
  if (b.fixedDetails) out.push({ key: 'fixed', title: '修复详情', type: 'pre', value: b.fixedDetails })
  if (b.testingSuggestions) out.push({ key: 'testing', title: '测试建议', type: 'pre', value: b.testingSuggestions })
  if (b.content) out.push({ key: 'content', title: '详情 (CONTENT)', type: 'md', value: b.content })
  if (b.notes && b.notes.kind !== 'none') out.push({ key: 'notes', title: '笔记 (NOTES)', type: 'notes', value: b.notes })
  if (b.reviewLink) out.push({ key: 'review', title: 'Review', type: 'link', value: b.reviewLink })
  // 自定义字段：按出现顺序渲染为文本分区（key 带前缀避免与内置分区冲突）。
  for (const cf of (b.customFields || [])) {
    if (cf && cf.name) out.push({ key: 'cf:' + cf.name, title: cf.name, type: 'text', value: cf.value })
  }
  if (b.extraImpacts && b.extraImpacts.length) out.push({ key: 'extra', title: 'Extra Impact', type: 'extra', value: b.extraImpacts, defaultCollapsed: true })
  return out
}
// ── 折叠状态持久化：localStorage + 监听 activeBlockLine 保存滚动位置 ──
const SEC_STATE_KEY = 'guaxi_org_section_collapse_state'
const SCROLL_POS_KEY = 'guaxi_org_scroll_position'
const loadSecState = () => {
  try {
    const raw = localStorage.getItem(SEC_STATE_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}
const saveSecState = () => {
  try {
    localStorage.setItem(SEC_STATE_KEY, JSON.stringify(secState))
  } catch {}
}
const secState = reactive(loadSecState())
const secKey = (line, key) => `${line}:${key}`
const isCollapsed = (line, key, dft) => {
  const k = secKey(line, key)
  return k in secState ? secState[k] : !!dft
}
const toggleSec = (line, key, dft) => {
  const k = secKey(line, key)
  secState[k] = !(k in secState ? secState[k] : !!dft)
  saveSecState()
}
const copySection = async (sec, line) => {
  let text = ''
  if (sec.type === 'extra') text = (sec.value || []).join('\n\n')
  else if (sec.type === 'notes') text = sec.value?.value || ''
  else text = String(sec.value || '')
  try { await navigator.clipboard.writeText(text); doFlash(`sec:${line}:${sec.key}`) } catch {}
}
// 复制成功的短暂反馈（icon → ✓）
const flashed = ref('')
let flashTimer = null
const doFlash = (key) => {
  flashed.value = key
  if (flashTimer) clearTimeout(flashTimer)
  flashTimer = setTimeout(() => { flashed.value = '' }, 1200)
}
const copyCard = (b) => { emit('copy-block', b); doFlash(`copy:${b.blockStartLine}`) }
const exportCard = (b) => { emit('export-block', b); doFlash(`export:${b.blockStartLine}`) }
const copyDesc = async (b) => {
  try { await navigator.clipboard.writeText(b.description || ''); doFlash(`desc:${b.blockStartLine}`) } catch {}
}

// ── 读视图内联快捷编辑：状态/优先级/标题就地改，直接回写源文件（复用 apply-block）──
// 以块当前完整字段为基，仅覆盖补丁字段，序列化后回写；done 跟随状态重算。
const quickApply = (b, patch) => {
  if (!b) return
  const next = JSON.parse(JSON.stringify(b))
  Object.assign(next, patch)
  if ('status' in patch) next.done = next.status === 'DONE' || next.status === 'CANCELLED'
  emit('apply-block', { startLine: b.blockStartLine, endLine: b.blockEndLine, text: serializeBlock(next) })
}
// 标题双击就地改描述
const titleEditLine = ref(0)
const titleDraft = ref('')
const titleInputRefs = new Map()
const registerTitleInput = (line, el) => { if (el) titleInputRefs.set(line, el) }
const startTitleEdit = (b) => {
  titleEditLine.value = b.blockStartLine
  titleDraft.value = b.description || ''
  nextTick(() => titleInputRefs.get(b.blockStartLine)?.focus?.())
}
const cancelTitleEdit = () => { titleEditLine.value = 0 }
const commitTitleEdit = (b) => {
  if (titleEditLine.value !== b.blockStartLine) return
  const v = titleDraft.value.trim()
  titleEditLine.value = 0
  if (v !== (b.description || '')) quickApply(b, { description: v })
}

// ── 搜索（高亮 + 上一个/下一个，按字段）──
const search = ref('')
const searchField = ref('all')
const matchFields = ['description', 'id', 'rootCause', 'conditionOfOccurrence', 'fixedDetails', 'testingSuggestions', 'content', 'status', 'fixStatus', 'severity', 'origin', 'createTime', 'completedTime', 'affectedRelease']
const blockMatches = (b) => {
  const q = search.value.trim().toLowerCase()
  if (!q) return false
  const fields = searchField.value === 'all' ? matchFields.concat('notes') : [searchField.value]
  return fields.some((f) => {
    if (f === 'notes') return String(b.notes?.value || '').toLowerCase().includes(q)
    // 状态字段：同时搜索原始值和中文字段名
    if (f === 'status') return (b.status || '').toLowerCase().includes(q) || (statusLabel(b.status) || '').toLowerCase().includes(q)
    if (f === 'severity') return (b.severity || '').toLowerCase().includes(q)
    if (f === 'fixStatus') return (b.fixStatus || '').toLowerCase().includes(q)
    return String(b[f] || '').toLowerCase().includes(q)
  })
}
// 命中按「关键字出现次数」计数与导航：一个块里出现多次就有多个停靠点。
// 顺序必须与卡片 DOM 中 <mark> 的先后一致（标题 → 状态 → 属性 → 各分区），
// 这样第 N 次命中可以直接落到卡片内第 N 个 <mark> 上。
const countOccurrences = (text, needle) => {
  if (!text || !needle) return 0
  const hay = String(text).toLowerCase()
  const ned = needle.toLowerCase()
  let count = 0
  let from = 0
  while (from <= hay.length) {
    const i = hay.indexOf(ned, from)
    if (i < 0) break
    count += 1
    from = i + ned.length
  }
  return count
}
const occurrenceKeysOfBlock = (b, q) => {
  // 返回每次命中所属的分区 key（'' 表示卡片头部/属性区），下标即块内序号。
  const keys = []
  const push = (text, secKey = '') => {
    for (let i = 0, n = countOccurrences(text, q); i < n; i += 1) keys.push(secKey)
  }
  push(b.description || b.id || '')
  push(statusLabel(b.status))
  for (const f of ['id', 'parentId', 'severity', 'origin', 'fixStatus', 'progress', 'deadline', 'createTime', 'completedTime', 'affectedRelease']) {
    push(b[f])
  }
  for (const sec of sectionsOf(b)) {
    // Review 是纯文本链接，不做高亮，不计入停靠点。
    if (sec.type === 'link') continue
    if (sec.type === 'extra') {
      for (const item of sec.value || []) push(item, sec.key)
    } else if (sec.type === 'notes') {
      push(sec.value?.value || '', sec.key)
    } else {
      push(sec.value, sec.key)
    }
  }
  return keys
}
const occurrences = computed(() => {
  const q = search.value.trim()
  if (!q) return []
  const out = []
  for (const b of displayBlocks.value) {
    if (!blockMatches(b)) continue
    // 正在编辑的卡片显示的是表单而不是高亮文本，没有可停靠的 mark。
    if (b.blockStartLine === editingLine.value) continue
    const keys = occurrenceKeysOfBlock(b, q)
    keys.forEach((secKey, ordinal) => out.push({ blockStartLine: b.blockStartLine, ordinal, secKey }))
  }
  return out
})
const matchIdx = ref(0)
let lastJumpLine = 0
let currentMark = null
const clearCurrentMark = () => {
  if (currentMark?.isConnected) currentMark.classList.remove('otp-hit-current')
  currentMark = null
}
watch(occurrences, () => {
  if (matchIdx.value >= occurrences.value.length) matchIdx.value = 0
})
watch(search, () => {
  matchIdx.value = 0
  lastJumpLine = 0
  clearCurrentMark()
})
// Markdown 渲染与高亮都是异步的：目标可能刚进视口还没渲染完，等对应标记出现再定位。
// 原文兜底（.guaxi-org-md__raw）里的 mark 会在渲染完成后消失，不能当作停靠目标。
const marksOfCard = (card) => Array.from(card.querySelectorAll('mark.otp-hit'))
  .filter((m) => !m.closest('.guaxi-org-md__raw'))
const openCollapsedSection = (card, secKey) => {
  if (!secKey) return false
  const sec = Array.from(card.querySelectorAll('.otp-sec')).find((el) => el.dataset.secKey === secKey)
  const body = sec?.querySelector('.otp-sec-body')
  if (!body || (body.offsetParent !== null && body.getClientRects().length > 0)) return false
  sec.querySelector('.otp-sec-head')?.click()
  return true
}
const revealOccurrence = async (occ) => {
  let card = cardRefs.get(occ.blockStartLine)
  if (!card) return
  card.scrollIntoView({ behavior: 'smooth', block: 'center' })
  for (let attempt = 0; attempt < 25; attempt += 1) {
    card = cardRefs.get(occ.blockStartLine)
    if (!card) return
    openCollapsedSection(card, occ.secKey)
    const marks = marksOfCard(card)
    if (marks.length > occ.ordinal) {
      clearCurrentMark()
      currentMark = marks[occ.ordinal]
      currentMark.classList.add('otp-hit-current')
      currentMark.scrollIntoView({ behavior: 'smooth', block: 'center' })
      return
    }
    await new Promise((resolve) => setTimeout(resolve, 80))
  }
}
const gotoMatch = (dir) => {
  const total = occurrences.value.length
  if (!total) return
  matchIdx.value = (matchIdx.value + dir + total) % total
  const occ = occurrences.value[matchIdx.value]
  // 同一块内的多个命中不重复挪编辑器光标，避免光标在源码里来回跳。
  if (occ.blockStartLine !== lastJumpLine) {
    lastJumpLine = occ.blockStartLine
    emit('goto-line', occ.blockStartLine)
  }
  void revealOccurrence(occ)
}
// 高亮工具：转义 + <mark> 包裹匹配（用于 v-html 字段）
const escapeHtml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const hl = (text) => {
  const raw = String(text ?? '')
  const q = search.value.trim()
  if (!q) return escapeHtml(raw)
  const esc = escapeHtml(raw)
  const qe = escapeHtml(q).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return esc.replace(new RegExp(qe, 'gi'), (m) => `<mark class="otp-hit">${m}</mark>`)
}

// ── 大窗预览：把整条任务按阅读态展开 ──
// 只记块首行号，块对象每次从 blocks 现取：源文件被编辑后 props.content 会重建 blocks，
// 留住旧对象就会在大窗里显示已经过期的内容。
const cardZoomLine = ref(0)
const cardZoomBlock = computed(() => blocks.value.find((b) => b.blockStartLine === cardZoomLine.value) || null)
const openCardZoom = (b) => {
  if (!b) return
  cardZoomLine.value = b.blockStartLine
}
const closeCardZoom = () => { cardZoomLine.value = 0 }
// 从大窗里点「编辑」：关掉浮层，回到卡片原位进入编辑态。
const startEditFromZoom = () => {
  const target = cardZoomBlock.value
  closeCardZoom()
  if (target) startEdit(target)
}

// ── 放大编辑浮层 ──
// 任意可编辑文本区（根本原因/复现步骤/修复详情/测试建议/详情/笔记/Extra Impact）都能放大到接近整窗，
// 通过 apply 回调把内容写回对应 draft 字段，做到与原位编辑等价。
const bigEdit = reactive({ open: false, title: '', text: '', apply: null, view: 'edit' })
const bigEditRef = ref(null)
const openBigEdit = async (title, value, apply) => {
  bigEdit.title = title
  bigEdit.text = String(value ?? '')
  bigEdit.apply = apply
  bigEdit.view = 'edit'
  bigEdit.open = true
  await nextTick()
  bigEditRef.value?.focusEditor?.({ cursor: 'end' })
}
const saveBigEdit = () => {
  if (typeof bigEdit.apply === 'function') bigEdit.apply(bigEdit.text)
  bigEdit.open = false
  bigEdit.apply = null
}
const cancelBigEdit = () => {
  bigEdit.open = false
  bigEdit.apply = null
}

// ── 快捷键：Ctrl/Cmd+S 保存，Esc 关闭放大浮层 ──
// 处于编辑态（含放大浮层）时拦截浏览器默认保存行为，与点击「保存」图标等价。
const onPreviewKeydown = (e) => {
  const isSave = (e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'S')
  if (isSave) {
    if (bigEdit.open) { e.preventDefault(); saveBigEdit(); return }
    if (editingLine.value) { e.preventDefault(); saveEdit() }
    return
  }
  if (e.key !== 'Escape') return
  // 焦点在 Monaco 里时，Esc 的第一含义是「关掉补全框/悬浮提示」。
  // 在这里顺手取消编辑会吃掉用户刚写的字段内容，所以让给编辑器自己处理。
  if (e.target?.closest?.('.monaco-editor')) return
  if (bigEdit.open) { e.preventDefault(); cancelBigEdit(); return }
  if (cardZoomBlock.value) { e.preventDefault(); closeCardZoom(); return }
  if (editingLine.value) { e.preventDefault(); cancelEdit() }
}
onMounted(() => { window.addEventListener('keydown', onPreviewKeydown, true) })
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onPreviewKeydown, true)
})

// ── 编辑模式 ──
const editingLine = ref(0)
const draft = reactive({})
const descInputRef = ref(null)
// 函数式 ref：编辑表单位于卡片 v-for 内，用函数 ref 规避 v-for 下静态 ref 变数组的歧义。
const setDescInput = (el) => { if (el) descInputRef.value = el }
let editSnapshot = ''
const startEdit = (b) => {
  Object.assign(draft, JSON.parse(JSON.stringify(b)))
  if (!Array.isArray(draft.extraImpacts)) draft.extraImpacts = []
  if (!Array.isArray(draft.customFields)) draft.customFields = []
  if (!draft.notes || !draft.notes.kind) draft.notes = { kind: 'inline', value: '' }
  editingLine.value = b.blockStartLine
  // 脏检查基线：进入编辑态时的序列化快照；并自动聚焦描述输入框。
  editSnapshot = serializeBlock(JSON.parse(JSON.stringify(draft)))
  nextTick(() => descInputRef.value?.focus?.())
}
const isDraftDirty = () => {
  try { return serializeBlock(JSON.parse(JSON.stringify(draft))) !== editSnapshot } catch { return true }
}
const cancelEdit = () => {
  if (isDraftDirty() && !window.confirm('有未保存的修改，确定放弃吗？')) return
  editingLine.value = 0
}
const saveEdit = () => {
  draft.done = draft.status === 'DONE' || draft.status === 'CANCELLED'
  // 完成时间：非完成态不保留完成时间（与后端 backfill 一致）；
  // 完成态且用户留空 → 保持为空，交由后端在重扫描时自动计算当前时间。
  if (!draft.done) draft.completedTime = ''
  emit('apply-block', { startLine: draft.blockStartLine, endLine: draft.blockEndLine, text: serializeBlock(draft) })
  editingLine.value = 0
}
// Extra Impact 行上移/下移（人性化重排）
const moveExtra = (i, dir) => {
  const arr = draft.extraImpacts
  const j = i + dir
  if (!Array.isArray(arr) || j < 0 || j >= arr.length) return
  const tmp = arr[i]; arr[i] = arr[j]; arr[j] = tmp
}
// ── 自定义字段编辑 ──
// 字段名必须是大写字母与空格（解析器识别规则），失焦时规范化；不与已知字段/NOTES 冲突。
const addCustomField = () => { draft.customFields.push({ name: '', value: '' }) }
const removeCustomField = (i) => { draft.customFields.splice(i, 1) }
const normalizeCfName = (i) => {
  const cf = draft.customFields[i]
  if (!cf) return
  cf.name = String(cf.name || '').toUpperCase().replace(/[^A-Z ]+/g, '').replace(/\s+/g, ' ').trim()
}

// ── 日历选择截止/创建时间 ──
// 设计：保留原文本输入（兼容手输与粘贴的多种格式），同时提供原生日历控件。
// 选择后统一写回与全局一致的中文格式（截止：2026年06月20日；创建：2026年06月20日 14:30:00）。
const pad2 = (n) => String(n).padStart(2, '0')
// 从已有字段值解析出年月日时分秒，兼容 ISO(2026-06-20)、斜杠(2026/6/20)、中文(2026年06月20日)及可选时间
const parseDateParts = (str) => {
  const s = String(str || '')
  const dm = s.match(/(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/)
  if (!dm) return null
  const tm = s.match(/(\d{1,2}):(\d{2})(?::(\d{2}))?/)
  return {
    y: +dm[1], mo: +dm[2], d: +dm[3],
    hh: tm ? +tm[1] : 0, mi: tm ? +tm[2] : 0, ss: tm ? +(tm[3] || 0) : 0,
  }
}
// 供原生 <input type="date"> 回显：yyyy-MM-dd
const toDateInput = (str) => {
  const p = parseDateParts(str)
  return p ? `${p.y}-${pad2(p.mo)}-${pad2(p.d)}` : ''
}
// 供原生 <input type="datetime-local" step="1"> 回显：yyyy-MM-ddTHH:mm:ss
const toDateTimeInput = (str) => {
  const p = parseDateParts(str)
  return p ? `${p.y}-${pad2(p.mo)}-${pad2(p.d)}T${pad2(p.hh)}:${pad2(p.mi)}:${pad2(p.ss)}` : ''
}
const onPickDeadline = (e) => {
  const m = String(e?.target?.value || '').match(/(\d{4})-(\d{2})-(\d{2})/)
  if (m) draft.deadline = `${m[1]}年${m[2]}月${m[3]}日`
}
const onPickCreateTime = (e) => {
  const m = String(e?.target?.value || '').match(/(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?/)
  if (m) draft.createTime = `${m[1]}年${m[2]}月${m[3]}日 ${m[4]}:${m[5]}:${m[6] || '00'}`
}
// 完成时间：用户选择即写入；留空则由后端 backfill 在完成态时自动计算。
const onPickCompletedTime = (e) => {
  const m = String(e?.target?.value || '').match(/(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?/)
  if (m) draft.completedTime = `${m[1]}年${m[2]}月${m[3]}日 ${m[4]}:${m[5]}:${m[6] || '00'}`
}

// ── 光标同步：滚动到激活块 ──
const bodyRef = ref(null)
const cardRefs = new Map()
const registerCardRef = (line, el) => { if (el) cardRefs.set(line, el) }

// 设计说明：
// 1. .org 任务预览的真实滚动容器是 .otp-body，而不是组件根节点。
// 2. 父组件在切换 tab 前后只需要一个稳定的“阅读比例”，因此这里暴露 scrollRatio/restoreScroll，
//    避免父组件依赖内部 DOM 结构，也不会影响现有的 goto-line、搜索和卡片编辑交互。
const clampScrollRatio = (value) => {
  const n = Number(value)
  if (!Number.isFinite(n)) return 0
  return Math.min(1, Math.max(0, n))
}
// 设计说明：返回 null 表示“此刻量不到”，而不是 0。
// 容器被 v-show 隐藏、或被 KeepAlive 移出文档时 scrollTop 恒为 0；宿主正是拿这个比例按文件
// 记住阅读位置，把量不到的 0 当成真实读数，就会把正确位置覆盖成文档开头。
const scrollRatio = () => {
  const el = bodyRef.value
  if (!el || !el.isConnected || el.clientHeight <= 0) return null
  const scrollableHeight = Math.max(0, el.scrollHeight - el.clientHeight)
  return scrollableHeight > 0 ? clampScrollRatio(el.scrollTop / scrollableHeight) : 0
}
const restoreScroll = (ratio) => {
  const el = bodyRef.value
  if (!el) {
    logger.debug('[PreviewReadPosition] skip org restore because body is not ready')
    return false
  }
  const safeRatio = clampScrollRatio(ratio)
  const scrollableHeight = Math.max(0, el.scrollHeight - el.clientHeight)
  el.scrollTop = safeRatio * scrollableHeight
  logger.debug('[PreviewReadPosition] restored org body scroll', {
    ratio: +safeRatio.toFixed(4),
    scrollTop: Math.round(el.scrollTop || 0),
    scrollHeight: el.scrollHeight,
    clientHeight: el.clientHeight,
  })
  return true
}
defineExpose({ scrollRatio, restoreScroll })

watch(() => props.activeBlockLine, async (line) => {
  if (!line) return
  await nextTick()
  const el = cardRefs.get(line)
  if (el && bodyRef.value) el.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
})
</script>

<script>
export default { name: 'GuaxiOrgTaskPreview' }
</script>

<style scoped>
.org-task-preview {
  /* 设计说明：
   * 1. 任务预览与主编辑器展示的是同一份 .org 文档内容，阅读态和编辑态应使用完全一致的字体栈。
   * 2. 原生 button/select/input/textarea 可能使用浏览器系统 UI 字体，不能只依赖根容器的普通继承。
   * 3. 使用组件级变量统一管理，后续调整主编辑器字体时可以直接同步这一处。
   */
  --otp-editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  /* 正文尺度。任务条目里真正要读的是字段内容（日志、命令、步骤），不是标题，
     所以正文基准单独成变量：改这一处，段落、代码块、列表、属性值一起跟着变。
     界面控件（工具栏、芯片）不跟着放，否则整块面板会显得臃肿。 */
  --otp-content-font-size: 16.5px;
  --otp-content-line-height: 1.85;
  display: flex; flex-direction: column; height: 100%; min-width: 0;
  font-family: var(--otp-editor-font-family);
  font-size: var(--otp-content-font-size);
}

/* Bug/Task 的所有条目、操作控件和编辑区统一继承主编辑器字体。
 * `button` 与表单元素需要显式指定，避免浏览器 user-agent 样式重新切回系统字体。
 */
.org-task-preview :where(
  button,
  input,
  select,
  option,
  textarea,
  label,
  pre,
  code
) {
  font-family: var(--otp-editor-font-family) !important;
}

/* 条目正文的 Markdown 产物位于 v-html 内容中，需要通过 :deep 明确覆盖其字体。 */
.org-task-preview :deep(.otp-md),
.org-task-preview :deep(.otp-md *),
.org-task-preview :deep(.otp-bigedit-preview),
.org-task-preview :deep(.otp-bigedit-preview *) {
  font-family: var(--otp-editor-font-family) !important;
}

.theme-light { --otp-bg:#f7f8fa; --otp-card:#ffffff; --otp-border:#e3e6eb; --otp-text:#1f2328; --otp-sub:#57606a; --otp-chip:#eef1f5; --otp-hit:#ffd54a; --otp-hit-fg:#1f2328; }
.theme-dark { --otp-bg:#21252b; --otp-card:#282c34; --otp-border:#3e4451; --otp-text:#abb2bf; --otp-sub:#8b94a3; --otp-chip:#2f343d; --otp-hit:#f0b429; --otp-hit-fg:#1f2328; }
.org-task-preview { background: var(--otp-bg); color: var(--otp-text); }

.otp-toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--otp-border); flex: 0 0 auto; }
.otp-title { font-weight: 700; font-size: 17px; }
.otp-count { font-size: 13px; color: var(--otp-sub); }
.otp-spacer { flex: 1; }
.otp-btn { background: transparent; border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 7px; cursor: pointer; padding: 3px 10px; font-size: 14px; }
.otp-btn.icon { padding: 3px 9px; font-weight: 700; }
.otp-btn.sm { padding: 2px 8px; }
.otp-btn.on { background: #2080f0; color: #fff; border-color: #2080f0; }
.otp-btn.primary { background: #2080f0; color: #fff; border-color: #2080f0; }
.otp-btn:hover:not(:disabled) { opacity: 0.85; }
.otp-btn:disabled { opacity: 0.4; cursor: default; }

.otp-searchbar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--otp-border); flex: 0 0 auto; }
.otp-sel, .otp-search { background: var(--otp-card); border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 6px; padding: 4px 8px; font-size: 13px; font-family: inherit; }
.otp-search { flex: 1; }
.otp-search-count { font-size: 13px; color: var(--otp-sub); min-width: 42px; text-align: center; }

/* 批量操作栏 */
.otp-batchbar { display: flex; align-items: center; gap: 8px; padding: 6px 12px; border-bottom: 1px solid var(--otp-border); flex: 0 0 auto; background: var(--otp-chip); }
.otp-batch-all { display: inline-flex; align-items: center; gap: 5px; font-size: 14px; cursor: pointer; }
.otp-batch-count { font-size: 13px; color: var(--otp-sub); }
.otp-batch-sep { width: 1px; align-self: stretch; background: var(--otp-border); margin: 2px 4px; }
.otp-batch-num { width: 52px; background: var(--otp-card); border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 6px; padding: 3px 6px; font-size: 13px; font-family: inherit; }
.otp-btn.danger { border-color: #d03050; color: #d03050; }
.otp-btn.danger:hover:not(:disabled) { background: rgba(208,48,80,0.12); }

.otp-main { flex: 1 1 auto; min-height: 0; display: flex; }

.otp-body { flex: 1 1 auto; min-width: 0; overflow-y: auto; padding: 12px; display: flex; flex-direction: column; gap: 12px; }
.otp-empty { color: var(--otp-sub); padding: 24px; text-align: center; }
.otp-empty code, .otp-empty b { background: var(--otp-chip); padding: 1px 5px; border-radius: 4px; }

.otp-diags { border: 1px solid var(--otp-border); border-radius: 8px; overflow: hidden; }
.otp-diags-head { padding: 6px 10px; background: var(--otp-chip); font-size: 13px; font-weight: 700; }
.otp-diag { display: flex; gap: 8px; padding: 5px 10px; font-size: 13px; cursor: pointer; border-top: 1px solid var(--otp-border); }
.otp-diag:hover { background: var(--otp-chip); }
.otp-diag.error { color: #e45649; } .otp-diag.warning { color: #c18401; }
.otp-diag-line { flex: 0 0 auto; opacity: 0.8; }

.otp-card { position: relative; background: var(--otp-card); border: 1px solid var(--otp-border); border-radius: 12px; padding: 14px 16px; box-shadow: 0 2px 8px rgba(0,0,0,0.07); }
.otp-card.done { opacity: 0.75; }
.otp-card.active { border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.25); }
.otp-card.batch { padding-left: 42px; cursor: pointer; }
.otp-card.selected { border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.3); }
.otp-card-check { position: absolute; left: 13px; top: 15px; z-index: 2; display: inline-flex; }
.otp-card-check input { width: 16px; height: 16px; cursor: pointer; }
.otp-ins { display: flex; align-items: center; gap: 6px; opacity: 0; transition: opacity .15s; margin: 2px 0; }
.otp-ins.bottom { margin-top: 10px; justify-content: flex-start; }
.otp-card:hover .otp-ins { opacity: 1; }
.otp-ins-label { font-size: 12px; color: var(--otp-sub); }
.otp-ins-btn { font-size: 12px; padding: 1px 8px; border-radius: 10px; border: 1px dashed var(--otp-border); background: transparent; color: var(--otp-sub); cursor: pointer; }
.otp-ins-btn.bug:hover { border-color: #d03050; color: #d03050; }
.otp-ins-btn.task:hover { border-color: #2080f0; color: #2080f0; }
.otp-card-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.otp-card-title { font-size: 18px; font-weight: 700; flex: 1; min-width: 120px; word-break: break-word; cursor: pointer; line-height: 1.5; }
.otp-card-title:hover { color: #2080f0; }
.otp-type { font-size: 13px; padding: 2px 9px; border-radius: 10px; color: #fff; font-weight: 600; }
.type-bug { background: #d03050; } .type-task { background: #2080f0; } .type-other { background: #8a8f99; }
.otp-pri { font-size: 13px; font-weight: 700; padding: 2px 8px; border-radius: 5px; color: #fff; }
.pri-A { background: #d03050; } .pri-B { background: #f0a020; } .pri-C { background: #909399; }
.otp-status { font-size: 14px; padding: 3px 11px; border-radius: 10px; background: var(--otp-chip); color: var(--otp-text); font-weight: 600; }
.st-DONE { background: rgba(24,160,88,0.18); color: #18a058; }
.st-DOING, .st-NEXT { background: rgba(32,128,240,0.18); color: #2080f0; }
.st-PAUSED, .st-WAITING, .st-HOLD { background: rgba(240,160,32,0.18); color: #f0a020; }
.st-CANCELLED { background: rgba(144,147,153,0.18); color: #909399; }
.otp-actions { display: inline-flex; gap: 4px; }

/* 读视图内联快捷编辑：状态/优先级芯片上覆盖透明原生 select 承接点击弹下拉 */
.otp-pri-wrap, .otp-status-wrap { position: relative; display: inline-flex; }
.otp-quick-overlay {
  position: absolute; inset: 0; width: 100%; height: 100%;
  margin: 0; padding: 0; border: none; background: transparent;
  opacity: 0; cursor: pointer; appearance: none; -webkit-appearance: none;
}
.theme-light .otp-quick-overlay { color-scheme: light; }
.theme-dark .otp-quick-overlay { color-scheme: dark; }
.otp-pri.pri-none { background: transparent; color: var(--otp-sub); border: 1px dashed var(--otp-border); }
.otp-pri-wrap:hover .otp-pri.pri-none { color: #2080f0; border-color: #2080f0; }
.otp-title-input {
  flex: 1; min-width: 120px; font-size: 18px; font-weight: 700; line-height: 1.5;
  background: var(--otp-bg); border: 1px solid #2080f0; border-radius: 6px; padding: 2px 8px;
  color: var(--otp-text); font-family: inherit;
}
.otp-title-input:focus { outline: none; box-shadow: 0 0 0 2px rgba(32,128,240,0.18); }
.otp-mini { background: var(--otp-chip); border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 6px; cursor: pointer; width: 28px; height: 28px; line-height: 1; font-size: 15px; }
.otp-mini.xs { width: 22px; height: 22px; font-size: 13px; }
.otp-mini:hover { background: var(--otp-border); }
.otp-mini.ok { background: rgba(24,160,88,0.2); color: #18a058; border-color: #18a058; }
.otp-mini.danger:hover { background: rgba(208,48,80,0.18); color: #d03050; border-color: #d03050; }

.otp-props { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 12px; }
.otp-prop { display: inline-flex; align-items: center; gap: 5px; background: var(--otp-chip); border-radius: 6px; padding: 3px 9px; font-size: 14.5px; }
.otp-prop .k { color: var(--otp-sub); }
.otp-prop.done-t .v { color: #18a058; }

.otp-sec { margin-top: 14px; }
.otp-sec-head { display: flex; align-items: center; gap: 6px; cursor: pointer; user-select: none; padding: 3px 0; border-bottom: 1px solid var(--otp-border); }
.otp-sec-caret { color: var(--otp-sub); font-size: 14px; width: 15px; }
.otp-sec-head h4 { margin: 0; font-size: calc(var(--otp-content-font-size) + 1px); font-weight: 700; flex: 1; letter-spacing: 0.01em; }
.otp-sec-actions { opacity: 0; transition: opacity .15s; }
.otp-sec-head:hover .otp-sec-actions { opacity: 1; }
.otp-sec-body { margin-top: 7px; }
.otp-text { font-size: var(--otp-content-font-size); line-height: var(--otp-content-line-height); white-space: pre-wrap; word-break: break-word; }
.otp-pre { margin: 0 0 6px; font-size: var(--otp-content-font-size); line-height: var(--otp-content-line-height); white-space: pre-wrap; word-break: break-word; background: var(--otp-chip); border-radius: 8px; padding: 10px 12px; }
.otp-extra-list { display: flex; flex-direction: column; gap: 8px; }
.otp-link, .otp-notes-file { color: #2080f0; word-break: break-all; font-size: var(--otp-content-font-size); }
:deep(mark.otp-hit) { background: var(--otp-hit); color: var(--otp-hit-fg); border-radius: 3px; padding: 0 1px; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.06); }
/* 当前停靠的命中：与其余命中区分开，避免"跳过去了但看不出跳到了哪" */
:deep(mark.otp-hit-current) { background: #ff9632; color: #1f2328; border-radius: 3px; padding: 0 1px; box-shadow: 0 0 0 2px rgba(255, 150, 50, 0.45); }

/* 编辑表单：大字段全宽 + 高文本域 */
.otp-edit-head { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; font-size: 16px; }
.otp-form { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 12px; }
.otp-form label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; color: var(--otp-sub); font-weight: 600; }
.otp-form .full { grid-column: 1 / -1; }
.otp-form input, .otp-form select, .otp-form textarea {
  background: var(--otp-bg); border: 1px solid var(--otp-border); color: var(--otp-text);
  border-radius: 8px; padding: 8px 10px; font-size: 15px; font-family: inherit; resize: vertical; line-height: 1.65;
}
.otp-form textarea.ta-md { min-height: 110px; }
.otp-form textarea.ta-lg { min-height: 170px; }
.otp-form textarea.ta-xl { min-height: 240px; }
/* Monaco 字段编辑器由自身控制高度，这里只保证它在网格/弹性行里占满可用宽度。 */
.otp-form :deep(.guaxi-field-md-editor) { width: 100%; }
.otp-ei-row :deep(.guaxi-field-md-editor) { flex: 1 1 auto; min-width: 0; }
.otp-extra-item { margin: 0; }
.otp-form input:focus, .otp-form select:focus, .otp-form textarea:focus { outline: none; border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.18); }
.otp-ei-edit { display: flex; flex-direction: column; gap: 6px; }
.otp-ei-head { display: flex; align-items: center; justify-content: space-between; font-size: 14px; color: var(--otp-sub); font-weight: 600; }
.otp-ei-row { display: flex; gap: 6px; }
.otp-ei-row textarea { flex: 1; }
.otp-ei-row-actions { display: flex; flex-direction: column; gap: 6px; }
/* 自定义字段编辑 */
.otp-cf-edit { display: flex; flex-direction: column; gap: 6px; }
.otp-cf-hint { font-size: 13px; color: var(--otp-sub); }
.otp-cf-hint code { background: var(--otp-chip); padding: 1px 5px; border-radius: 4px; }
.otp-cf-row { display: flex; gap: 6px; align-items: flex-start; }
.otp-cf-name { flex: 0 0 160px; text-transform: uppercase; }
.otp-cf-val { flex: 1; }

/* 字段标题行：标题文字 + 右侧「放大编辑」图标 */
.otp-field-head { display: inline-flex; align-items: center; gap: 6px; }
.otp-zoom { line-height: 1; }

/* 放大编辑浮层：全屏沉浸式编辑（无边框/圆角/留白，内容贴满视口） */
.otp-bigedit-mask { position: fixed; inset: 0; z-index: 3000; background: var(--otp-bg); display: flex; }
.otp-bigedit { width: 100%; height: 100%; background: var(--otp-card); color: var(--otp-text); display: flex; flex-direction: column; padding: 0; box-shadow: none; }
.otp-bigedit-head { display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-bottom: 1px solid var(--otp-border); font-size: 16px; flex: 0 0 auto; }
.otp-bigedit-views { display: inline-flex; gap: 4px; }
.otp-bigedit-body { flex: 1 1 auto; min-height: 0; display: flex; gap: 0; }
.otp-bigedit-body.view-split .otp-bigedit-area,
.otp-bigedit-body.view-split .otp-bigedit-preview { flex: 1 1 50%; width: 50%; }
/* 内部 Monaco 字段编辑器直接铺满浮层，去掉它自带的边框与圆角，避免看起来还有"框" */
.otp-bigedit-body .otp-bigedit-area { flex: 1 1 auto; width: 100%; border: none; border-radius: 0; background: var(--otp-bg); }
.otp-bigedit-preview { flex: 1 1 auto; min-width: 0; overflow: auto; background: var(--otp-bg); padding: 12px 18px; font-size: 15px; line-height: 1.75; border: none; border-radius: 0; }
.otp-bigedit-body.view-split .otp-bigedit-preview { border-left: 1px solid var(--otp-border); }

/* 截止/创建时间：单输入框 + 右侧日历图标。
 * 原生日期控件以透明层叠加在图标上方，真实点击落在“日历指示器”上即可弹出系统日历，
 * 无需依赖 showPicker()（WebView2 下更稳）。 */
.otp-date-field { position: relative; }
.otp-date-field > input:not(.otp-date-overlay) { width: 100%; box-sizing: border-box; padding-right: 36px; }
.otp-date-field .otp-date-icon {
  position: absolute; right: 6px; top: 50%; transform: translateY(-50%);
  width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center;
  font-size: 15px; line-height: 1; pointer-events: none; opacity: 0.85;
}
/* 透明原生控件覆盖在图标处，承接点击 */
.otp-date-field .otp-date-overlay {
  position: absolute; right: 4px; top: 50%; transform: translateY(-50%);
  width: 28px; height: 28px; padding: 0; margin: 0; border: none; background: transparent;
  opacity: 0; cursor: pointer;
}
.theme-light .otp-date-field .otp-date-overlay { color-scheme: light; }
.theme-dark .otp-date-field .otp-date-overlay { color-scheme: dark; }
/* 让整块透明控件都成为可点击的日历指示器热区 */
.otp-date-field .otp-date-overlay::-webkit-calendar-picker-indicator {
  position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; padding: 0;
  opacity: 0; cursor: pointer;
}

/* ── 条目正文的 Markdown 产物 ──
 * 这些节点来自 v-html，scoped 样式够不到，必须用 :deep。
 * 只覆盖阅读需要的间距与字重，颜色一律跟随主题变量，避免暗色下出现黑字。 */
.otp-md, .otp-bigedit-preview {
  font-size: var(--otp-content-font-size);
  line-height: var(--otp-content-line-height);
  word-break: break-word;
}
.otp-md :deep(p), .otp-bigedit-preview :deep(p) { margin: 6px 0; }
.otp-md :deep(p:first-child), .otp-bigedit-preview :deep(p:first-child) { margin-top: 0; }
.otp-md :deep(p:last-child), .otp-bigedit-preview :deep(p:last-child) { margin-bottom: 0; }

.otp-md :deep(h1), .otp-md :deep(h2), .otp-md :deep(h3),
.otp-md :deep(h4), .otp-md :deep(h5), .otp-md :deep(h6),
.otp-bigedit-preview :deep(h1), .otp-bigedit-preview :deep(h2), .otp-bigedit-preview :deep(h3),
.otp-bigedit-preview :deep(h4), .otp-bigedit-preview :deep(h5), .otp-bigedit-preview :deep(h6) {
  margin: 14px 0 6px; line-height: 1.45; font-weight: 700;
}
.otp-md :deep(h1), .otp-bigedit-preview :deep(h1) { font-size: 1.42em; }
.otp-md :deep(h2), .otp-bigedit-preview :deep(h2) { font-size: 1.28em; }
.otp-md :deep(h3), .otp-bigedit-preview :deep(h3) { font-size: 1.16em; }
.otp-md :deep(h4), .otp-md :deep(h5), .otp-md :deep(h6),
.otp-bigedit-preview :deep(h4), .otp-bigedit-preview :deep(h5), .otp-bigedit-preview :deep(h6) { font-size: 1.04em; }
.otp-md :deep(h1:first-child), .otp-md :deep(h2:first-child), .otp-md :deep(h3:first-child),
.otp-bigedit-preview :deep(h1:first-child),
.otp-bigedit-preview :deep(h2:first-child),
.otp-bigedit-preview :deep(h3:first-child) { margin-top: 0; }

/* 列表：默认的左内边距在窄卡片里太吃宽度，收到刚好容纳标记。
 * 嵌套层级按 ul 深度自己叠加，不依赖浏览器默认值。 */
.otp-md :deep(ul), .otp-md :deep(ol),
.otp-bigedit-preview :deep(ul), .otp-bigedit-preview :deep(ol) { margin: 6px 0; padding-left: 1.55em; }
.otp-md :deep(li), .otp-bigedit-preview :deep(li) { margin: 3px 0; }
.otp-md :deep(li > p), .otp-bigedit-preview :deep(li > p) { margin: 3px 0; }
.otp-md :deep(ul ul), .otp-md :deep(ol ol), .otp-md :deep(ul ol), .otp-md :deep(ol ul),
.otp-bigedit-preview :deep(ul ul), .otp-bigedit-preview :deep(ol ol),
.otp-bigedit-preview :deep(ul ol), .otp-bigedit-preview :deep(ol ul) { margin: 2px 0; }

.otp-md :deep(blockquote), .otp-bigedit-preview :deep(blockquote) {
  margin: 8px 0; padding: 2px 0 2px 12px;
  border-left: 3px solid var(--otp-border); color: var(--otp-sub);
}

.otp-md :deep(pre), .otp-bigedit-preview :deep(pre) {
  background: var(--otp-chip); border-radius: 8px; padding: 10px 12px;
  overflow-x: auto; font-size: 1em; line-height: 1.65; margin: 8px 0;
}
.otp-md :deep(code), .otp-bigedit-preview :deep(code) { font-size: 0.97em; }
.otp-md :deep(:not(pre) > code), .otp-bigedit-preview :deep(:not(pre) > code) {
  background: var(--otp-chip); border-radius: 4px; padding: 1px 5px;
}
.otp-md :deep(hr), .otp-bigedit-preview :deep(hr) { border: none; border-top: 1px solid var(--otp-border); margin: 12px 0; }
.otp-md :deep(a), .otp-bigedit-preview :deep(a) { color: #2080f0; word-break: break-all; }
.otp-md :deep(img), .otp-bigedit-preview :deep(img) { max-width: 100%; border-radius: 6px; display: block; margin: 8px 0; }

/* 表格：任务条目里的对照表常常肩并肩挤在一起，给单元格留出可读的内边距并允许横向滚动。 */
.otp-md :deep(table), .otp-bigedit-preview :deep(table) {
  border-collapse: collapse; margin: 8px 0; display: block; overflow-x: auto; max-width: 100%;
}
.otp-md :deep(th), .otp-md :deep(td),
.otp-bigedit-preview :deep(th), .otp-bigedit-preview :deep(td) {
  border: 1px solid var(--otp-border); padding: 5px 10px; text-align: left;
}
.otp-md :deep(th), .otp-bigedit-preview :deep(th) { background: var(--otp-chip); font-weight: 700; }

/* ── 大窗预览 ── */
.otp-cardzoom { width: 94vw; height: 90vh; }
.otp-cardzoom-title {
  font-size: 18px; font-weight: 700; min-width: 0;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.otp-cardzoom-body {
  flex: 1 1 auto; min-height: 0; overflow-y: auto;
  padding: 4px 2px 12px; font-size: 16px; line-height: 1.8;
}
/* 大窗里的分区标题只是标签，不参与折叠，所以去掉手型与悬停效果。 */
.otp-sec-head.static { cursor: default; }
.otp-cardzoom-body .otp-sec { margin-top: 18px; }
.otp-cardzoom-body .otp-text { font-size: var(--otp-content-font-size); line-height: var(--otp-content-line-height); }
.otp-cardzoom-body .otp-pre { font-size: var(--otp-content-font-size); line-height: var(--otp-content-line-height); }
.otp-cardzoom-body .otp-md,
.otp-cardzoom-body .otp-md :deep(p),
.otp-cardzoom-body .otp-md :deep(li) { font-size: var(--otp-content-font-size); line-height: var(--otp-content-line-height); }

.otp-body::-webkit-scrollbar, .otp-bigedit-area::-webkit-scrollbar,
.otp-cardzoom-body::-webkit-scrollbar { width: 9px; }
.otp-body::-webkit-scrollbar-thumb, .otp-bigedit-area::-webkit-scrollbar-thumb,
.otp-cardzoom-body::-webkit-scrollbar-thumb { background: rgba(140,140,150,0.38); border-radius: 10px; border: 2.5px solid transparent; background-clip: padding-box; }
</style>
