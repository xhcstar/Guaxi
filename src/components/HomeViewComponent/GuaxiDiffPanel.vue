<template>
  <div class="guaxi-diff-panel">
    <div class="dp-head">
      <div class="dp-row dp-row-main">
        <n-tag size="small" round :bordered="false" class="dp-mode">{{ modeLabel }}</n-tag>

        <n-tooltip trigger="hover" placement="bottom">
          <template #trigger>
            <n-radio-group :value="kindValue" size="small" class="dp-kind"
              :disabled="busy" @update:value="onKindChange">
              <n-radio-button value="file">文件</n-radio-button>
              <n-radio-button value="dir">目录</n-radio-button>
            </n-radio-group>
          </template>
          {{ busy ? '对比进行中，等这一轮结束再切换' : '对比任意两个文件，或者任意两个目录' }}
        </n-tooltip>

        <div class="dp-slot-wrap">
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <button type="button" class="dp-slot" :class="{ 'is-empty': !leftText }"
                @click="pickSide('left')">
                <Icon :icon="isDirMode ? folderIcon : fileIcon" width="13" height="13" />
                <span class="dp-slot-text">{{ leftText || '选择左侧…' }}</span>
              </button>
            </template>
            {{ store.left.path || `点击选择左侧${isDirMode ? '目录' : '文件'}` }}
          </n-tooltip>
        </div>

        <n-tooltip trigger="hover" placement="bottom">
          <template #trigger>
            <n-button text size="tiny" @click="onSwap">
              <Icon :icon="swapIcon" width="15" height="15" />
            </n-button>
          </template>
          交换左右两侧
        </n-tooltip>

        <div class="dp-slot-wrap">
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <button type="button" class="dp-slot" :class="{ 'is-empty': !rightText }"
                @click="pickSide('right')">
                <Icon :icon="isDirMode ? folderIcon : fileIcon" width="13" height="13" />
                <span class="dp-slot-text">{{ rightText || '选择右侧…' }}</span>
              </button>
            </template>
            {{ store.right.path || `点击选择右侧${isDirMode ? '目录' : '文件'}` }}
          </n-tooltip>
        </div>

        <div class="dp-actions">
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <span class="dp-edit-wrap">
                <n-radio-group :value="editMode ? 'edit' : 'read'" size="small" class="dp-edit-toggle"
                  :disabled="busy || editLoading || (!canEditDiff && !editMode)"
                  @update:value="onEditModeChange">
                  <n-radio-button value="read" aria-label="只读对比">
                    <Icon :icon="readIcon" width="14" height="14" />
                  </n-radio-button>
                  <n-radio-button value="edit" aria-label="编辑内容">
                    <Icon :icon="editIcon" width="14" height="14" />
                  </n-radio-button>
                </n-radio-group>
              </span>
            </template>
            <template v-if="canEditDiff || editMode">
              只读：只看差异，不动文件<br />编辑：可直接改内容并写回原文件（编码与换行符按原样保留，Ctrl+S 保存）
            </template>
            <template v-else>{{ editDisabledReason }}</template>
          </n-tooltip>
          <span class="dp-sep" />
          <template v-if="editMode">
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-button size="tiny" quaternary class="dp-icon-btn"
                  :type="anyDirty ? 'primary' : 'default'" :loading="savingAny" @click="saveDirty">
                  <template #icon><Icon :icon="saveIcon" width="15" height="15" /></template>
                </n-button>
              </template>
              {{ saveHint }}
            </n-tooltip>
          </template>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button size="tiny" quaternary class="dp-icon-btn is-run"
                :disabled="!canRun || editMode" :loading="busy" @click="run">
                <template #icon>
                  <Icon :icon="hasResult ? rerunIcon : playIcon" width="15" height="15" />
                </template>
              </n-button>
            </template>
            {{ hasResult ? '按当前选项重新计算差异' : '按当前选项计算差异' }}
          </n-tooltip>
          <n-tooltip v-if="busy && isDirMode" trigger="hover" placement="bottom">
            <template #trigger>
              <n-button size="tiny" quaternary class="dp-icon-btn is-stop" @click="cancelDir">
                <template #icon><Icon :icon="stopIcon" width="14" height="14" /></template>
              </n-button>
            </template>
            停掉这一轮目录扫描
          </n-tooltip>

          <span class="dp-sep" />

          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button quaternary size="tiny" class="dp-icon-btn"
                :class="{ 'is-on': showOptions }" @click="showOptions = !showOptions">
                <template #icon><Icon :icon="settingsIcon" width="15" height="15" /></template>
              </n-button>
            </template>
            {{ showOptions ? '收起对比选项' : '展开对比选项' }}
          </n-tooltip>
          <n-tooltip trigger="hover" placement="bottom">
            <template #trigger>
              <n-button quaternary size="tiny" class="dp-icon-btn" @click="emit('close')">
                <template #icon><Icon :icon="closeIcon" width="15" height="15" /></template>
              </n-button>
            </template>
            关闭对比窗口
          </n-tooltip>
        </div>
      </div>
      <div v-if="showOptions" class="dp-row dp-row-options">
        <template v-if="!isDirMode">
          <span class="dp-group">
            <Icon :icon="algorithmIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">算法</span>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <span class="dp-select-wrap">
                  <n-select v-model:value="opts.algorithm" size="small" class="dp-algo-select"
                    :options="algorithmOptions" :render-label="renderAlgorithmLabel"
                    :render-option="renderAlgorithmOption"
                    :consistent-menu-width="false" />
                </span>
              </template>
              <div>给行配对用哪种算法，点右边的 ? 看三者差别</div>
              <div v-if="editMode" class="dp-tip-note">{{ EDIT_ONLY_HINT }}</div>
            </n-tooltip>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-button quaternary size="tiny" class="dp-icon-btn dp-help-btn"
                  @click="showAlgorithmHelp = true">
                  <template #icon><Icon :icon="helpIcon" width="14" height="14" /></template>
                </n-button>
              </template>
              三种算法怎么选
            </n-tooltip>
          </span>

          <span class="dp-group">
            <Icon :icon="contextIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">上下文</span>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-input-number v-model:value="opts.contextLines" size="tiny" :min="0" :max="200"
                  :show-button="false" style="width: 54px" />
              </template>
              <div>每个改动块前后多留几行，方便看清改动的来龙去脉</div>
              <div v-if="editMode" class="dp-tip-note">{{ EDIT_ONLY_HINT }}</div>
            </n-tooltip>
          </span>

          <span class="dp-group">
            <Icon :icon="ignoreIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">忽略</span>
            <n-tooltip v-for="f in IGNORE_FLAGS" :key="f.key" trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="opts[f.key]" size="small">
                  {{ f.label }}
                </n-checkbox>
              </template>
              <div>{{ f.hint }}</div>
              <div v-if="editMode && !f.live" class="dp-tip-note">{{ EDIT_ONLY_HINT }}</div>
            </n-tooltip>
          </span>

          <span class="dp-sep" />

          <span class="dp-group">
            <Icon :icon="viewIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">视图</span>
            <n-radio-group v-model:value="viewMode" size="small">
              <n-radio-button value="split" title="左右并排对照">并排</n-radio-button>
              <n-radio-button value="unified" title="上下统一成一条流">统一</n-radio-button>
            </n-radio-group>
          </span>
        </template>
        <template v-else>
          <span class="dp-group">
            <Icon :icon="speedIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">比较方式</span>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="dirOpts.quick" size="small">快速模式</n-checkbox>
              </template>
              只比大小和修改时间，不读内容，几十万文件也很快（可能漏掉大小相同但内容不同的文件）
            </n-tooltip>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="dirOpts.detectRenames" size="small">检测改名</n-checkbox>
              </template>
              把内容相同、只是换了名字的文件认成"改名"而不是一删一增
            </n-tooltip>
          </span>

          <span class="dp-group">
            <Icon :icon="scopeIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">范围</span>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="dirOpts.includeHidden" size="small">含隐藏文件</n-checkbox>
              </template>
              把 .git、.env 这类点开头的文件也算进来
            </n-tooltip>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="dirOpts.respectGitignore" size="small">遵循 .gitignore</n-checkbox>
              </template>
              按仓库里的 .gitignore 规则跳过文件（构建产物、依赖目录）
            </n-tooltip>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-checkbox v-model:checked="dirOpts.includeSame" size="small">显示相同项</n-checkbox>
              </template>
              内容完全相同的条目也列出来，方便确认两边到底一不一样
            </n-tooltip>
          </span>

          <span class="dp-group">
            <Icon :icon="filterIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">排除</span>
            <n-tooltip trigger="hover" placement="bottom">
              <template #trigger>
                <n-input v-model:value="excludeText" size="tiny" placeholder="node_modules,*.log"
                  style="width: 168px" />
              </template>
              逗号分隔的 glob，命中的路径直接跳过，例如 node_modules,*.log,dist/**
            </n-tooltip>
          </span>

          <span class="dp-sep" />

          <span class="dp-group">
            <Icon :icon="viewIcon" width="13" height="13" class="dp-group-icon" />
            <span class="dp-group-label">视图</span>
            <n-radio-group v-model:value="dirView" size="small">
              <n-radio-button value="tree" title="按目录逐级展开">树形</n-radio-button>
              <n-radio-button value="list" title="平铺所有差异条目">列表</n-radio-button>
            </n-radio-group>
          </span>
        </template>

      </div>
      <div class="dp-row dp-row-status">
        <template v-if="drillRel">
          <n-button text size="tiny" class="dp-back" @click="backToDir">
            <Icon :icon="backIcon" width="14" height="14" />
            返回目录差异
          </n-button>
          <span class="dp-status-rel" :title="drillRel">{{ drillRel }}</span>
          <span class="dp-sep" />
        </template>

        <template v-if="editMode">
          <span class="dp-status-text">
            编辑中 · 左 <b>{{ editMeta.left?.encoding }}</b> {{ eolLabel(editMeta.left?.lineEnding) }}
            · 右 <b>{{ editMeta.right?.encoding }}</b> {{ eolLabel(editMeta.right?.lineEnding) }}
          </span>
          <span v-if="anyDirty" class="dp-status-text is-warn">● 有未保存的改动 · Ctrl+S 保存</span>
          <span v-else class="dp-status-text">两侧都已保存</span>
          <span v-if="editNote" class="dp-status-text is-warn" :title="editNote">{{ editNote }}</span>
        </template>
        <template v-else-if="busy">
          <span class="dp-status-text">{{ progressText }}</span>
        </template>
        <template v-else-if="error">
          <span class="dp-status-text is-error" :title="error">{{ error }}</span>
        </template>
        <template v-else-if="isDirMode && dirSummary">
          <span class="dp-badge is-add">+{{ dirSummary.added }}</span>
          <span class="dp-badge is-del">−{{ dirSummary.removed }}</span>
          <span class="dp-badge is-mod">±{{ dirSummary.modified }}</span>
          <span v-if="dirSummary.renamed" class="dp-badge is-ren">↷{{ dirSummary.renamed }}</span>
          <span v-if="dirSummary.type_changed" class="dp-badge is-type">⇄{{ dirSummary.type_changed }}</span>
          <span class="dp-status-text">
            共 {{ dirTotal }} 条 · 扫描 {{ dirSummary.left_files }}/{{ dirSummary.right_files }} 个文件
            · 读取 {{ formatSize(dirSummary.compared_bytes) }}
            · {{ dirSummary.elapsed_ms }} ms
          </span>
          <span v-if="dirSummary.cancelled" class="dp-status-text is-warn">已中止，结果不完整</span>
          <span v-else-if="dirSummary.entries_truncated" class="dp-status-text is-warn">条目已截断</span>
        </template>
        <template v-else-if="fileResult">
          <span class="dp-badge is-add">+{{ fileResult.stats.added }}</span>
          <span class="dp-badge is-del">−{{ fileResult.stats.removed }}</span>
          <span class="dp-status-text">
            {{ fileResult.stats.hunks }} 处改动 ·
            {{ fileResult.left.line_count }} → {{ fileResult.right.line_count }} 行 ·
            {{ formatSize(fileResult.left.size) }} / {{ formatSize(fileResult.right.size) }} ·
            {{ fileResult.elapsed_ms }} ms
          </span>
          <span v-if="fileResult.message" class="dp-status-text is-warn" :title="fileResult.message">
            {{ fileResult.message }}
          </span>
          <span v-if="fileResult.hunks_truncated" class="dp-status-text is-warn">显示已截断</span>
        </template>
        <span v-else class="dp-status-text">{{ hintText }}</span>
      </div>
    </div>

    <div v-if="showDirAny && sessionId" class="dp-row dp-row-filter">
      <n-input v-model:value="dirQuery" size="tiny" clearable placeholder="过滤路径关键字"
        style="width: 190px" @update:value="onFilterChanged" />
      <n-checkbox-group v-model:value="statusFilter" @update:value="onFilterChanged">
        <n-checkbox v-for="s in statusChoices" :key="s.key" :value="s.key" size="small">
          {{ s.label }}
        </n-checkbox>
      </n-checkbox-group>
      <span class="dp-status-text">
        {{ showDirTree ? `展开 ${treeShown} 行` : `${dirEntries.length} / ${dirTotal}` }}
      </span>
    </div>

    <div v-if="!editMode" ref="bodyRef" class="dp-body" :class="{ 'is-panes': splitPanes }"
      tabindex="0" @scroll.passive="onScroll" @keydown="onBodyKeydown">
      <div v-if="busy && !rows.length" class="dp-placeholder">
        <n-spin size="small" />
        <span>{{ progressText }}</span>
      </div>
      <div v-else-if="error" class="dp-placeholder is-error">{{ error }}</div>
      <div v-else-if="!rows.length" class="dp-placeholder">{{ emptyText }}</div>

      <!-- 并排视图：左右各自横向滚动，纵向同步，中间那条竖线可以拖。
           不做成"一整块超宽内容"是因为一侧有长行就会把另一侧顶到屏幕外。 -->
      <div v-else-if="splitPanes" ref="panesRef" class="dp-panes"
        :class="{ 'is-dragging': splitDragging }">
        <div ref="leftPaneRef" class="dp-pane is-left" :style="{ width: `calc(${splitPercent}% - 3px)` }"
          @scroll.passive="onPaneScroll('left', $event)">
          <div class="dp-spacer" :style="{ height: `${totalHeight}px`, width: paneWidth('left') }">
            <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
              <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="rowClass(row)"
                @mousedown="onRowDown(row)">
                <span v-if="row.kind === 'hunk'" class="dp-hunk">{{ row.header }}</span>
                <template v-else>
                  <span class="dp-no">{{ row.left.no || '' }}</span>
                  <span class="dp-cell is-fluid" :class="row.left.cls">
                    <span v-for="(seg, i) in row.left.segs" :key="i"
                      :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                  </span>
                </template>
              </div>
            </div>
          </div>
        </div>
        <div class="dp-divider" title="拖动改变左右宽度 · 双击回到各占一半"
          @mousedown.prevent="startSplitDrag" @dblclick="resetSplit"></div>
        <div ref="rightPaneRef" class="dp-pane is-right"
          @scroll.passive="onPaneScroll('right', $event)">
          <div class="dp-spacer" :style="{ height: `${totalHeight}px`, width: paneWidth('right') }">
            <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
              <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="rowClass(row)"
                @mousedown="onRowDown(row)">
                <span v-if="row.kind === 'hunk'" class="dp-hunk"></span>
                <template v-else>
                  <span class="dp-no">{{ row.right.no || '' }}</span>
                  <span class="dp-cell is-fluid" :class="row.right.cls">
                    <span v-for="(seg, i) in row.right.segs" :key="i"
                      :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                  </span>
                </template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="dp-spacer" :style="{ height: `${totalHeight}px`, width: contentWidth }">
        <div class="dp-window" :style="{ transform: `translateY(${offsetY}px)` }">
          <template v-if="showDirTree">
            <div v-for="row in visibleRows" :key="row.key" class="dp-entry dp-tree-row"
              :class="[row.node ? `is-${row.node.status}` : 'is-tip',
                       { 'is-dir-entry': !!row.node?.is_dir }]"
              :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
              :title="row.node ? nodeTitle(row.node) : ''" @click="onTreeClick(row)">
              <template v-if="row.node">
                <span class="dp-twist" :class="{ 'is-open': row.expanded }">
                  <span v-if="row.loading" class="dp-twist-wait">·</span>
                  <Icon v-else-if="row.node.expandable" :icon="chevronIcon" width="13" height="13" />
                </span>
                <span class="dp-entry-mark">{{ statusMark(row.node.status) }}</span>
                <Icon :icon="row.node.is_dir ? folderIcon : fileIcon" width="13" height="13"
                  class="dp-entry-icon" />
                <span class="dp-entry-rel">{{ row.node.name }}</span>
                <span v-if="row.node.renamed_from" class="dp-entry-extra">← {{ row.node.renamed_from }}</span>
                <span v-if="row.node.is_dir && row.node.child_count" class="dp-entry-extra">
                  {{ rollupText(row.node) }}
                </span>
                <span class="dp-entry-size">{{ entrySizeText(row.node) }}</span>
              </template>
              <span v-else class="dp-entry-extra">{{ row.tip }}</span>
            </div>
          </template>
          <template v-else-if="showDirList">
            <div v-for="row in visibleRows" :key="row.key" class="dp-entry"
              :class="[`is-${row.entry.status}`, { 'is-dir-entry': row.entry.is_dir }]"
              :title="entryTitle(row.entry)" @click="openEntry(row.entry)">
              <span class="dp-entry-mark">{{ statusMark(row.entry.status) }}</span>
              <Icon :icon="row.entry.is_dir ? folderIcon : fileIcon" width="13" height="13" class="dp-entry-icon" />
              <span class="dp-entry-rel">{{ row.entry.rel }}</span>
              <span v-if="row.entry.renamed_from" class="dp-entry-extra">← {{ row.entry.renamed_from }}</span>
              <span v-if="row.entry.child_count" class="dp-entry-extra">含 {{ row.entry.child_count }} 项</span>
              <span class="dp-entry-size">{{ entrySizeText(row.entry) }}</span>
            </div>
          </template>
          <template v-else>
            <div v-for="row in paintedRows" :key="row.key" class="dp-line" :class="rowClass(row)"
              @mousedown="onRowDown(row)">
              <span v-if="row.kind === 'hunk'" class="dp-hunk">{{ row.header }}</span>
              <template v-else>
                <span class="dp-no">{{ row.left.no || '' }}</span>
                <span class="dp-no">{{ row.right.no || '' }}</span>
                <span class="dp-sign">{{ row.sign }}</span>
                <span class="dp-cell" :class="row.cell.cls" :style="cellStyle">
                  <span v-for="(seg, i) in row.cell.segs" :key="i" :class="{ 'dp-ink': seg.hit }">{{ seg.t }}</span>
                </span>
              </template>
            </div>
          </template>
        </div>
      </div>
    </div>

    <!-- 编辑模式：自绘只读视图整个换掉，交给 Monaco 差异编辑器（两侧都可改可存） -->
    <div v-if="editMode" class="dp-edit-body">
      <div ref="editHostRef" class="dp-edit-host"></div>
      <div v-if="editLoading" class="dp-placeholder is-overlay">
        <n-spin size="small" />
        <span>{{ editPhase === 'diff' ? '正在计算差异…' : '正在装载编辑器…' }}</span>
      </div>
    </div>

    <div v-if="dropActive" class="dp-drop-mask">
      <div class="dp-drop-half" :class="{ 'is-on': dropSide === 'left' }">
        <Icon :icon="dropIcon" width="20" height="20" />
        <span>放到左侧</span>
      </div>
      <div class="dp-drop-half" :class="{ 'is-on': dropSide === 'right' }">
        <Icon :icon="dropIcon" width="20" height="20" />
        <span>放到右侧</span>
      </div>
      <div class="dp-drop-tip">松手即对比 · 一次拖两个可直接填满两侧 · 文件和目录不能混</div>
    </div>

    <n-modal v-model:show="showAlgorithmHelp" preset="card" class="dp-help-card"
      title="三种差异算法怎么选" :bordered="false" style="width: 620px">
      <p class="dp-help-lead">
        三者都在算"左边哪些行要变成右边"，区别在怎么给行配对。文件不同，配出来的结果可读性差很多。
      </p>
      <div v-for="a in ALGORITHMS" :key="a.value" class="dp-help-item"
        :class="{ 'is-current': opts.algorithm === a.value }">
        <div class="dp-help-head">
          <span class="dp-help-name">{{ a.name }}</span>
          <span class="dp-help-brief">{{ a.brief }}</span>
          <n-tag v-if="opts.algorithm === a.value" size="tiny" round :bordered="false">当前使用</n-tag>
          <n-button v-else text size="tiny" @click="opts.algorithm = a.value">用这个</n-button>
        </div>
        <dl class="dp-help-body">
          <dt>原理</dt>
          <dd>{{ a.principle }}</dd>
          <dt>适合</dt>
          <dd>{{ a.fit }}</dd>
          <dt>代价</dt>
          <dd>{{ a.cost }}</dd>
        </dl>
      </div>
      <p class="dp-help-lead">拿不准就用 Patience（默认）。</p>
    </n-modal>
  </div>
</template>
<script setup>
/**
 * 差异对比面板：任意两个文件 / 两个目录的内容差异，和 Git 无关。
 *
 * 性能取舍（对应 Rust 侧 commands/diffcompare.rs）：
 * 1. 差异一律由 Rust 算完再回传 hunk，前端不做行级 diff，也不给 Monaco 建巨型 model；
 *    100 MB 文件在 Monaco 里必崩，这里只渲染视口内的那几十行。
 * 2. 行是等高的，走自绘虚拟滚动：40 万行结果和 40 行结果的渲染成本一样。
 * 3. 目录结果留在 Rust 侧的 LRU 会话里，前端按页取，过滤和搜索也在 Rust 侧做。
 * 4. 行内高亮区间由 Rust 按 UTF-16 code unit 给出，JS 可以直接 slice。
 */
import { computed, h, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open as openDialog, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import {
  NButton, NCheckbox, NCheckboxGroup, NInput, NInputNumber, NModal,
  NRadioButton, NRadioGroup, NSelect, NSpin, NTag, NTooltip, useMessage,
} from 'naive-ui'
import { Icon } from '@iconify/vue'
import fileIcon from '@iconify-icons/lucide/file-text'
import folderIcon from '@iconify-icons/lucide/folder'
import swapIcon from '@iconify-icons/lucide/arrow-left-right'
import settingsIcon from '@iconify-icons/lucide/sliders-horizontal'
import closeIcon from '@iconify-icons/lucide/x'
import backIcon from '@iconify-icons/lucide/corner-up-left'
import chevronIcon from '@iconify-icons/lucide/chevron-right'
import dropIcon from '@iconify-icons/lucide/download'
import saveIcon from '@iconify-icons/lucide/save'
import readIcon from '@iconify-icons/lucide/eye'
import editIcon from '@iconify-icons/lucide/pencil'
import helpIcon from '@iconify-icons/lucide/help-circle'
import playIcon from '@iconify-icons/lucide/play'
import rerunIcon from '@iconify-icons/lucide/refresh-cw'
import stopIcon from '@iconify-icons/lucide/square'
import algorithmIcon from '@iconify-icons/lucide/git-compare'
import contextIcon from '@iconify-icons/lucide/align-left'
import ignoreIcon from '@iconify-icons/lucide/eraser'
import viewIcon from '@iconify-icons/lucide/split-square-horizontal'
import speedIcon from '@iconify-icons/lucide/zap'
import scopeIcon from '@iconify-icons/lucide/folder-tree'
import filterIcon from '@iconify-icons/lucide/filter'
import { createLogger } from '../../utils/logger'
import { useDiffCompareStore } from '../../stores/diffCompareStore'
import { useSettingsStore } from '../../stores/settingsStore'
import {
  GUAXI_MONACO_SEMANTIC_TOKEN_COLORS,
  GUAXI_MONACO_THEME_RULES,
  getLanguageForFile,
} from '../../monaco/guaxiMonacoThemes'

const logger = createLogger('GuaxiDiffPanel')
const message = useMessage()
const store = useDiffCompareStore()
const settingsStore = useSettingsStore()

const props = defineProps({
  visible: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

/** 行高，必须和样式里的 --dp-row-h 一致：虚拟滚动全靠它算偏移。 */
const ROW_H = 24
const OVERSCAN = 14
const DIR_PAGE = 500
const CHAR_FALLBACK = 7.2
/** 左右分栏比例、目录视图形态都记在本地，下次开窗接着用（各窗口同源，共享一份）。 */
const SPLIT_KEY = 'guaxi_diff_split_ratio'
const DIR_VIEW_KEY = 'guaxi_diff_dir_view'
const SPLIT_MIN = 0.12
const SPLIT_MAX = 0.88

const readSplitRatio = () => {
  try {
    const v = Number(localStorage.getItem(SPLIT_KEY))
    if (Number.isFinite(v) && v >= SPLIT_MIN && v <= SPLIT_MAX) return v
  } catch {
    // 读不到就一半一半
  }
  return 0.5
}
const readDirView = () => {
  try {
    return localStorage.getItem(DIR_VIEW_KEY) === 'list' ? 'list' : 'tree'
  } catch {
    return 'tree'
  }
}

const showOptions = ref(false)
const viewMode = ref('split')
/** 'tree' | 'list' —— 目录结果按层展开，还是平铺所有差异条目 */
const dirView = ref(readDirView())
const busy = ref(false)
const error = ref('')

const fileResult = ref(null)
const rows = ref([])
const maxCols = ref(80)
/** 并排视图两侧各自的最宽列数：一侧的长行不该撑开另一侧。 */
const maxColsLeft = ref(40)
const maxColsRight = ref(40)

const sessionId = ref('')
const dirSummary = ref(null)
const dirEntries = ref([])
const dirTotal = ref(0)
const dirHasMore = ref(false)
const dirLoadingPage = ref(false)
const dirQuery = ref('')
const statusFilter = ref([])
const progress = reactive({ phase: '', scanned: 0, compared: 0, elapsed: 0 })

/** 从目录结果点进某一条时记住相对路径，返回时不用重扫。 */
const drillRel = ref('')

const opts = reactive({
  algorithm: 'patience',
  contextLines: 3,
  ignoreWhitespace: false,
  ignoreTrailingWhitespace: false,
  ignoreCase: false,
  ignoreBlankLines: false,
})

const dirOpts = reactive({
  quick: false,
  detectRenames: false,
  includeHidden: false,
  respectGitignore: false,
  includeSame: false,
})
const excludeText = ref('')

/** 三种算法：给用户看的是"什么情况用哪个"，不是教科书定义。 */
const ALGORITHMS = [
  {
    name: 'Patience',
    value: 'patience',
    brief: '锚点对齐，代码文件最像人眼 —— 默认',
    menu: '锚点对齐，默认',
    principle:
      '先找出在两侧都只出现一次的"唯一行"当锚点，再在锚点之间递归求解。整块函数被插删时，锚点会稳稳落在函数边界上。',
    fit: '代码、配置这类有清晰结构的文本；改动是成块的增删；大文件也比 LCS 快得多。',
    cost: '重复行很多时（生成代码、日志、压缩后的 JS）几乎找不到唯一行，锚点稀疏，可能退化成慢且不理想的配对。',
  },
  {
    name: 'Myers',
    value: 'myers',
    brief: '增删最少，通用的最短编辑脚本',
    menu: '增删最少，最通用',
    principle:
      '经典的"最短编辑脚本"算法：在所有把左边变成右边的走法里，找增删行数最少的那条路。git 默认用它。',
    fit: '改动零散、想看到"数学上最小"的差异；文件不大、两侧相似度中等时结果干净。',
    cost: '高度相似或很大的文件，时间与内存开销明显上升；有时为了少一行，配对看起来反而不如 Patience 直观。',
  },
  {
    name: 'LCS',
    value: 'lcs',
    brief: '最长公共子序列，稳定可预期但最慢',
    menu: '最长公共子序列，最慢',
    principle:
      '直接求两侧的最长公共子序列，最大化按原有顺序保留下来的公共行骨架，结果是确定性的。',
    fit: '看重"公共结构完整保留"的场景，例如大段代码被搬来搬去、想确认到底哪些行真的变了。',
    cost: '复杂度最高（大致 O(n·m)），大文件会明显更慢、更吃内存，本面板对大输入会自行退回统计模式。',
  },
]

/**
 * 下拉菜单里的文案：算法名 + 一句白话，用户不用打开说明弹窗就能选。
 * 收起时只留算法名（长文案会把工具栏撑开，见 renderAlgorithmLabel）。
 */
const algorithmOptions = ALGORITHMS.map((a) => ({
  value: a.value,
  label: `${a.name} · ${a.menu}`,
  name: a.name,
}))

/** 收起状态只显示算法名。naive 会把 render-label 同时用在菜单项上，所以菜单项另走 render-option。 */
const renderAlgorithmLabel = (option) => h('span', { class: 'dp-algo-name' }, option?.name || '')

/**
 * 展开后的菜单项：算法名 + 一句白话，选之前就能比一比。
 * 换了 render-option 就没有 naive 自带的勾选图标，选中项仍有底色，够用。
 */
const renderAlgorithmOption = ({ option }) => h('div', { class: 'dp-algo-opt' }, [
  h('span', { class: 'dp-algo-opt__name' }, option.name),
  h('span', { class: 'dp-algo-opt__hint' }, option.menu),
])

/**
 * 编辑模式下的红绿由 Monaco 自己算，这些选项里只有"忽略空白"能同步过去（对应它的
 * ignoreTrimWhitespace），其余的只落在只读对比的结果上。选项不再锁死，而是在提示里补一句，
 * 免得改完看不到变化以为控件坏了。
 */
const EDIT_ONLY_HINT = '编辑模式下红绿由编辑器实时算，这一项只影响只读结果'

/** 四个忽略开关：文案说清"忽略的是什么"。 */
const IGNORE_FLAGS = [
  // live：这一项能同步给编辑器（Monaco 的 ignoreTrimWhitespace），提示里就不用补上面那句
  { key: 'ignoreWhitespace', label: '忽略空白', live: true, hint: '行内任意位置的空白差异都不算改动（相当于 git diff -w）' },
  { key: 'ignoreTrailingWhitespace', label: '忽略行尾空白', hint: '只忽略行末多出来的空格和制表符，行中间的空白照常比较' },
  { key: 'ignoreCase', label: '忽略大小写', hint: '大小写不同不算改动，适合大小写风格不统一的代码库' },
  { key: 'ignoreBlankLines', label: '忽略空行', hint: '单纯多一行或少一行空白行不算改动' },
]

const statusChoices = [
  { key: 'modified', label: '已改' },
  { key: 'added', label: '新增' },
  { key: 'removed', label: '删除' },
  { key: 'renamed', label: '改名' },
  { key: 'type_changed', label: '类型变' },
  { key: 'same', label: '相同' },
]
const isDirMode = computed(() => store.mode === 'dir')
/** 目录结果页（还没点进具体文件）：两种视图形态共用这个前提。 */
const showDirAny = computed(() => isDirMode.value && !drillRel.value)
const showDirTree = computed(() => showDirAny.value && dirView.value === 'tree')
const showDirList = computed(() => showDirAny.value && dirView.value === 'list')
/** 行级差异 + 并排：走左右两个独立滚动区，其余情况还是单个滚动区。 */
const splitPanes = computed(() => !showDirAny.value && viewMode.value === 'split')
const leftText = computed(() => store.leftLabel)
const rightText = computed(() => store.rightLabel)
const modeLabel = computed(() => {
  if (isDirMode.value) return '目录对比'
  if (store.mode === 'text') return '文本对比'
  return store.mode === 'buffer' ? '未保存改动' : '文件对比'
})
/** 文件 / 目录切换器的选中值：text、buffer 模式下两个都不选中，点一下就切到对应的路径对比。 */
const kindValue = computed(() => {
  if (store.mode === 'dir') return 'dir'
  return store.mode === 'file' ? 'file' : null
})
/** 屏幕上已经有结果了：动作按钮这时候该说"重算"而不是"开始"。 */
const hasResult = computed(() => !!fileResult.value || !!sessionId.value)
const canRun = computed(() => {
  if (busy.value) return false
  if (store.mode === 'text') return store.left.text != null && store.right.text != null
  if (store.mode === 'buffer') return !!store.left.path && store.right.text != null
  return !!(store.left.path && store.right.path)
})
const hintText = computed(() => {
  const what = isDirMode.value ? '目录' : '文件'
  return `拖两个${what}进来，或点上面的方框各选一个，然后点"对比"`
})
const emptyText = computed(() => {
  if (!store.left.path && !store.right.path && store.mode !== 'text') return hintText.value
  if (showDirAny.value && sessionId.value) return '没有符合条件的差异'
  if (!fileResult.value && !sessionId.value && !busy.value) return '两侧都填好了，点"对比"开始'
  if (fileResult.value?.identical) return fileResult.value.message || '两侧内容完全相同'
  if (fileResult.value?.mode === 'binary') return fileResult.value.message || '二进制文件，不做行级对比'
  if (fileResult.value?.mode === 'too_large') return fileResult.value.message || '文件超过大小上限，只对比元信息'
  if (fileResult.value?.mode === 'too_divergent') return fileResult.value.message || '差异过大，只给统计'
  return '暂无差异内容'
})
const progressText = computed(() => {
  if (!isDirMode.value) return '正在计算差异…'
  const phase = { scan: '扫描目录', compare: '比对内容', rename: '匹配改名' }[progress.phase] || '准备中'
  return `${phase}：已扫描 ${progress.scanned} 项 · 已比对 ${progress.compared} 个文件 · ${progress.elapsed} ms`
})

// ───── 虚拟滚动 ─────
const bodyRef = ref(null)
const scrollTop = ref(0)
const viewportH = ref(360)
const charW = ref(CHAR_FALLBACK)

const totalHeight = computed(() => rows.value.length * ROW_H)
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_H) - OVERSCAN))
const endIndex = computed(() => Math.min(
  rows.value.length,
  Math.ceil((scrollTop.value + viewportH.value) / ROW_H) + OVERSCAN,
))
const visibleRows = computed(() => rows.value.slice(startIndex.value, endIndex.value))
const offsetY = computed(() => startIndex.value * ROW_H)

// ───── 当前行 ─────
/**
 * 只读视图是自绘的虚拟列表，没有 Monaco 那种光标，"当前行"只能自己记。
 * 记的是行 key 而不是下标：换视图、改选项、重算差异都是整份换掉行数组，按下标会挪到别的行上去。
 */
const currentKey = ref('')

/** hunk 头只是一条分隔线，不该停下来：从 from 出发朝 dir 找下一个能当当前行的行。 */
const findRowIndex = (list, from, dir) => {
  for (let i = from; i >= 0 && i < list.length; i += dir) {
    if (list[i].kind !== 'hunk') return i
  }
  return -1
}

/** 朝 dir 找；顶到头了就往回找最近的一条，翻页键才不会卡在最后一行上不动。 */
const settleRowIndex = (list, from, dir) => {
  const at = Math.min(list.length - 1, Math.max(0, from))
  const hit = findRowIndex(list, at, dir)
  return hit >= 0 ? hit : findRowIndex(list, at, -dir)
}

/** 当前行的下标。只在按键时找一次，所以不额外维护索引表，省得每次重建行都要跟着重建。 */
const currentRowIndex = () => {
  const list = rows.value
  const key = currentKey.value
  if (!key) return -1
  for (let i = 0; i < list.length; i += 1) {
    if (list[i].key === key) return i
  }
  return -1
}

/** 行走到视口外就把它带进来，上下各留两行余量，跟编辑器一样。 */
const revealRow = (index) => {
  const el = splitPanes.value ? leftPaneRef.value : bodyRef.value
  if (!el) return
  const viewH = el.clientHeight || viewportH.value
  const top = index * ROW_H
  const margin = ROW_H * 2
  if (top < el.scrollTop + margin) el.scrollTop = Math.max(0, top - margin)
  else if (top + ROW_H + margin > el.scrollTop + viewH) el.scrollTop = top + ROW_H + margin - viewH
}

const gotoRow = (index) => {
  if (index < 0 || index >= rows.value.length) return
  currentKey.value = rows.value[index].key
  revealRow(index)
}

/**
 * 行数组重建后把当前行接住：key 还在就留在原处，key 没了（换了视图形态、重算了差异）
 * 就回到第一行 —— 编辑器里光标总是在的，不该因为重算就整块消失。
 */
watch(rows, (list) => {
  if (!list.length) {
    currentKey.value = ''
    return
  }
  if (currentKey.value && list.some((r) => r.key === currentKey.value)) return
  const first = findRowIndex(list, 0, 1)
  currentKey.value = first < 0 ? '' : list[first].key
})

/** 键盘导航：上下键走一行，翻页键走一屏，Home/End 到首尾。 */
const onBodyKeydown = (ev) => {
  const list = rows.value
  if (showDirAny.value || !list.length) return
  const at = currentRowIndex()
  const page = Math.max(1, Math.floor((viewportH.value || ROW_H) / ROW_H) - 1)
  const start = at < 0 ? startIndex.value : at
  const end = at < 0 ? endIndex.value : at
  let next = -1
  if (ev.key === 'ArrowDown') next = settleRowIndex(list, start + 1, 1)
  else if (ev.key === 'ArrowUp') next = settleRowIndex(list, end - 1, -1)
  else if (ev.key === 'PageDown') next = settleRowIndex(list, start + page, 1)
  else if (ev.key === 'PageUp') next = settleRowIndex(list, end - page, -1)
  else if (ev.key === 'Home') next = settleRowIndex(list, 0, 1)
  else if (ev.key === 'End') next = settleRowIndex(list, list.length - 1, -1)
  else return
  ev.preventDefault()
  if (next >= 0) gotoRow(next)
}

/** 鼠标按在哪一行哪一行就是当前行：跟编辑器里点一下会移光标是一回事。 */
const onRowDown = (row) => {
  if (row.kind === 'hunk') return
  currentKey.value = row.key
}

const colWidth = computed(() => Math.max(260, Math.ceil(maxCols.value * charW.value) + 16))
const cellStyle = computed(() => ({ width: `${colWidth.value}px` }))
const contentWidth = computed(() => {
  if (showDirAny.value) return '100%'
  return `${colWidth.value + 128}px`
})
/**
 * 并排视图里每一侧内容的宽度。
 * `.dp-spacer` 带 `min-width:100%`，所以短文件不会缩成一条窄柱；
 * 算出来的宽度只在真的比可视区宽时才生效，也就是该出横向滚动条的时候。
 */
const paneWidth = (side) => {
  const cols = side === 'right' ? maxColsRight.value : maxColsLeft.value
  return `${52 + Math.ceil(cols * charW.value) + 24}px`
}
// 等宽字体下 CJK 占两格：列宽按"显示宽度"算，两侧才对得齐。
// 超长行（压缩后的 js）没必要精确量，扫到 2000 列就停手。
const displayWidth = (s) => {
  const n = Math.min(s.length, 2000)
  let w = 0
  for (let i = 0; i < n; i += 1) {
    const c = s.charCodeAt(i)
    w += c > 0x2e7f && c < 0xff61 ? 2 : 1
  }
  return w
}

const measureCharWidth = () => {
  try {
    const el = bodyRef.value
    if (!el) return
    const cs = window.getComputedStyle(el)
    const ctx = document.createElement('canvas').getContext('2d')
    if (!ctx) return
    ctx.font = `${cs.fontSize} ${cs.fontFamily}`
    const w = ctx.measureText('M'.repeat(64)).width / 64
    if (w > 3 && w < 32) charW.value = w
  } catch {
    // 量不到就用兜底值，只影响横向滚动余量
  }
}

const EMPTY_CELL = Object.freeze({ no: null, text: '', inline: null, cls: 'is-blank' })

// ───── 并排视图的左右分栏 ─────
const panesRef = ref(null)
const leftPaneRef = ref(null)
const rightPaneRef = ref(null)
const splitRatio = ref(readSplitRatio())
const splitDragging = ref(false)
const splitPercent = computed(() => (splitRatio.value * 100).toFixed(3))

const persistSplit = () => {
  try {
    localStorage.setItem(SPLIT_KEY, splitRatio.value.toFixed(4))
  } catch {
    // 存不下只是下次回到一半一半
  }
}

let splitDrag = null
const onSplitDragMove = (ev) => {
  if (!splitDrag) return
  const next = splitDrag.ratio + (ev.clientX - splitDrag.x) / splitDrag.w
  splitRatio.value = Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, next))
}
const endSplitDrag = () => {
  if (!splitDrag) return
  splitDrag = null
  splitDragging.value = false
  window.removeEventListener('mousemove', onSplitDragMove)
  window.removeEventListener('mouseup', endSplitDrag)
  persistSplit()
}
/** 分隔线按住就拖。监听挂在 window 上，鼠标甩出分隔线也不会中断。 */
const startSplitDrag = (ev) => {
  const host = panesRef.value
  if (!host) return
  splitDrag = { x: ev.clientX, ratio: splitRatio.value, w: host.clientWidth || 1 }
  splitDragging.value = true
  window.addEventListener('mousemove', onSplitDragMove)
  window.addEventListener('mouseup', endSplitDrag)
}
const resetSplit = () => {
  splitRatio.value = 0.5
  persistSplit()
}

/**
 * 两侧纵向滚动同步，横向各滚各的（长行只在自己这侧滚，另一侧不跟着跑偏）。
 * 互相赋值会再触发一次 scroll，用开关挡掉，下一帧放开。
 */
let syncingPanes = false
const onPaneScroll = (which, ev) => {
  const el = ev?.currentTarget
  if (!el) return
  scrollTop.value = el.scrollTop
  if (syncingPanes) return
  const other = which === 'left' ? rightPaneRef.value : leftPaneRef.value
  if (!other || Math.abs(other.scrollTop - el.scrollTop) < 1) return
  syncingPanes = true
  other.scrollTop = el.scrollTop
  requestAnimationFrame(() => {
    syncingPanes = false
  })
}

const resetScroll = () => {
  scrollTop.value = 0
  nextTick(() => {
    if (bodyRef.value) bodyRef.value.scrollTop = 0
    for (const el of [leftPaneRef.value, rightPaneRef.value]) {
      if (!el) continue
      el.scrollTop = 0
      el.scrollLeft = 0
    }
  })
}

/** 视口高度取当前真正在滚的那个容器：并排时是左面板，其余是 body。 */
const syncViewport = () => {
  const el = splitPanes.value ? leftPaneRef.value || bodyRef.value : bodyRef.value
  if (el) viewportH.value = el.clientHeight || 360
}

const onScroll = (ev) => {
  const el = ev.currentTarget
  scrollTop.value = el.scrollTop
  if (!showDirList.value || !dirHasMore.value || dirLoadingPage.value) return
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 640) void loadNextPage()
}
// ───── 行模型 ─────
// 一个 hunk 里的行序固定是 ctx* del* add* ctx*，并排视图按 run 把 del/add 一一配对。
const buildFileRows = (result) => {
  const out = []
  let widest = 60
  // 并排视图两侧各算一份，只有统一视图才用合并后的 widest。
  let widestL = 20
  let widestR = 20
  const track = (t) => {
    const w = displayWidth(t)
    if (w > widest) widest = w
  }
  const trackL = (t) => {
    const w = displayWidth(t)
    if (w > widestL) widestL = w
    if (w > widest) widest = w
  }
  const trackR = (t) => {
    const w = displayWidth(t)
    if (w > widestR) widestR = w
    if (w > widest) widest = w
  }
  const split = viewMode.value === 'split'
  let seq = 0
  for (const hunk of result?.hunks || []) {
    out.push({ key: `h${hunk.index}`, kind: 'hunk', cls: 'is-hunk', header: hunk.header })
    const lines = hunk.lines || []
    if (!split) {
      for (const ln of lines) {
        track(ln.text)
        out.push({
          key: `u${seq++}`,
          kind: 'uni',
          cls: `is-${ln.tag}`,
          sign: ln.tag === 'del' ? '−' : ln.tag === 'add' ? '+' : ' ',
          left: { no: ln.left_no },
          right: { no: ln.right_no },
          cell: { no: null, text: ln.text, inline: ln.inline, cls: `is-${ln.tag}` },
        })
      }
      continue
    }
    let i = 0
    while (i < lines.length) {
      if (lines[i].tag === 'ctx') {
        const ln = lines[i]
        trackL(ln.text)
        trackR(ln.text)
        out.push({
          key: `p${seq++}`,
          kind: 'pair',
          cls: 'is-ctx',
          left: { no: ln.left_no, text: ln.text, inline: null, cls: '' },
          right: { no: ln.right_no, text: ln.text, inline: null, cls: '' },
        })
        i += 1
        continue
      }
      const dels = []
      while (i < lines.length && lines[i].tag === 'del') dels.push(lines[i++])
      const adds = []
      while (i < lines.length && lines[i].tag === 'add') adds.push(lines[i++])
      if (!dels.length && !adds.length) {
        i += 1
        continue
      }
      for (let k = 0; k < Math.max(dels.length, adds.length); k += 1) {
        const d = dels[k]
        const a = adds[k]
        if (d) trackL(d.text)
        if (a) trackR(a.text)
        out.push({
          key: `p${seq++}`,
          kind: 'pair',
          cls: 'is-change',
          left: d ? { no: d.left_no, text: d.text, inline: d.inline, cls: 'is-del' } : EMPTY_CELL,
          right: a ? { no: a.right_no, text: a.text, inline: a.inline, cls: 'is-add' } : EMPTY_CELL,
        })
      }
    }
  }
  maxCols.value = Math.min(widest, 2000)
  maxColsLeft.value = Math.min(widestL, 2000)
  maxColsRight.value = Math.min(widestR, 2000)
  return out
}
// 行内高亮切段只对视口内的行做：Rust 给的区间已经是 UTF-16 下标，直接 slice。
const paint = (cell) => {
  const text = cell?.text || ''
  const ranges = cell?.inline
  if (!ranges || !ranges.length) {
    return { no: cell?.no, cls: cell?.cls || '', segs: [{ t: text, hit: false }] }
  }
  const segs = []
  let pos = 0
  for (const r of ranges) {
    const a = Math.max(pos, Math.min(Number(r[0]) || 0, text.length))
    const b = Math.max(a, Math.min(Number(r[1]) || 0, text.length))
    if (a > pos) segs.push({ t: text.slice(pos, a), hit: false })
    if (b > a) segs.push({ t: text.slice(a, b), hit: true })
    pos = b
  }
  if (pos < text.length) segs.push({ t: text.slice(pos), hit: false })
  return { no: cell?.no, cls: cell?.cls || '', segs }
}

const paintedRows = computed(() => {
  if (showDirAny.value) return []
  return visibleRows.value.map((r) => {
    if (r.kind === 'hunk') return r
    if (r.kind === 'uni') return { ...r, cell: paint(r.cell) }
    return { ...r, left: paint(r.left), right: paint(r.right) }
  })
})

/** 行的 class 在模板里现拼：当前行是逐行比对出来的，不混进 paintedRows，
 *  否则点一下就要把视口里那几十行的行内高亮整段重算。 */
const rowClass = (row) => [row.cls, { 'is-current': !!row.key && row.key === currentKey.value }]

const rebuildDirRows = () => {
  rows.value = dirEntries.value.map((e, i) => ({
    key: `${i}:${e.status}:${e.rel}`,
    kind: 'entry',
    entry: e,
  }))
}

/** 目录结果的行由当前视图形态决定：树形按层要，列表按页要。 */
const refreshDirRows = async () => {
  if (dirView.value === 'tree') {
    if (!treeChildren.has('')) await loadTreeLevel('')
    rebuildTreeRows()
  } else {
    rebuildDirRows()
  }
}

const formatSize = (n) => {
  const v = Number(n) || 0
  if (v < 1024) return `${v} B`
  if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KB`
  if (v < 1024 * 1024 * 1024) return `${(v / 1024 / 1024).toFixed(1)} MB`
  return `${(v / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const STATUS_MARK = { added: '+', removed: '−', modified: '±', renamed: '↷', type_changed: '⇄', same: '=' }
const statusMark = (s) => STATUS_MARK[s] || '·'
const STATUS_TEXT = {
  added: '仅右侧有', removed: '仅左侧有', modified: '内容不同',
  renamed: '改名', type_changed: '文件/目录类型不同', same: '内容相同',
  mixed: '里面有多种变化',
}
const entryTitle = (e) => {
  const base = `${STATUS_TEXT[e.status] || e.status} · ${e.rel}`
  if (e.is_dir) return e.child_count ? `${base}（整棵子树 ${e.child_count} 项）` : base
  return `${base}\n左 ${formatSize(e.left_size)} · 右 ${formatSize(e.right_size)}`
}

const entrySizeText = (e) => {
  if (e.is_dir) return ''
  if (e.status === 'added') return formatSize(e.right_size)
  if (e.status === 'removed') return formatSize(e.left_size)
  return `${formatSize(e.left_size)} → ${formatSize(e.right_size)}`
}

// ───── 目录树：一层一层地要，展开哪层算哪层 ─────
// 缓存故意用普通 Map / Set 而不是 reactive：行数组由 rebuildTreeRows 显式重建，
// 免得深层嵌套结构的响应式追踪拖慢几万条目录结果。
const treeChildren = new Map()
const treeExpanded = new Set()
const treeLoading = new Set()
const treeShown = ref(0)

const resetTree = () => {
  treeChildren.clear()
  treeExpanded.clear()
  treeLoading.clear()
  treeShown.value = 0
}

const nodeTitle = (n) => {
  const base = `${STATUS_TEXT[n.status] || n.status} · ${n.rel}`
  if (!n.is_dir) return `${base}\n左 ${formatSize(n.left_size)} · 右 ${formatSize(n.right_size)}`
  if (n.status === 'added' || n.status === 'removed') {
    return n.child_count ? `${base}（整棵子树 ${n.child_count} 项）` : base
  }
  return n.child_count ? `${base}（里面 ${n.child_count} 处差异）` : base
}

/** 目录节点右边那串小计。整棵子树只在一侧时后端不分类，退回条数。 */
const rollupText = (n) => {
  const parts = []
  if (n.added) parts.push(`+${n.added}`)
  if (n.removed) parts.push(`−${n.removed}`)
  if (n.modified) parts.push(`±${n.modified}`)
  if (n.renamed) parts.push(`↷${n.renamed}`)
  if (n.type_changed) parts.push(`⇄${n.type_changed}`)
  if (n.same) parts.push(`=${n.same}`)
  return parts.length ? parts.join(' ') : `${n.child_count} 项`
}

const rebuildTreeRows = () => {
  const out = []
  const walk = (parent, depth) => {
    const level = treeChildren.get(parent)
    if (!level) return
    const nodes = level.nodes || []
    for (const node of nodes) {
      const expanded = !!node.is_dir && treeExpanded.has(node.rel)
      out.push({
        key: `n:${node.rel}`,
        depth,
        node,
        expanded,
        loading: treeLoading.has(node.rel),
      })
      if (!expanded) continue
      const sub = treeChildren.get(node.rel)
      if (!sub) continue
      if (sub.nodes?.length) walk(node.rel, depth + 1)
      else if (!treeLoading.has(node.rel)) {
        out.push({ key: `z:${node.rel}`, depth: depth + 1, node: null, tip: '（这层没有内容）' })
      }
    }
    if (level.truncated) {
      out.push({
        key: `t:${parent}`,
        depth,
        node: null,
        tip: `这层还有 ${Math.max(0, Number(level.total || 0) - nodes.length)} 项没显示`,
      })
    }
  }
  walk('', 0)
  treeShown.value = out.length
  rows.value = out
}

/** 取一层。过滤条件跟着一起送后端，展开的层和列表视图看到的是同一套筛选。 */
const loadTreeLevel = async (parent) => {
  if (!sessionId.value) return
  treeLoading.add(parent)
  if (parent) rebuildTreeRows()
  try {
    const level = await invoke('diff_dirs_children', {
      sessionId: sessionId.value,
      parent: parent || null,
      statuses: statusFilter.value.length ? [...statusFilter.value] : null,
      query: dirQuery.value ? dirQuery.value : null,
    })
    treeChildren.set(parent, level)
  } catch (e) {
    treeExpanded.delete(parent)
    message.warning(String(e?.message || e || '展开目录失败'))
  } finally {
    treeLoading.delete(parent)
  }
}

const toggleTreeNode = async (node) => {
  if (!node?.is_dir || !node.expandable) return
  if (treeExpanded.has(node.rel)) {
    treeExpanded.delete(node.rel)
    rebuildTreeRows()
    return
  }
  treeExpanded.add(node.rel)
  if (!treeChildren.has(node.rel)) await loadTreeLevel(node.rel)
  rebuildTreeRows()
}

const onTreeClick = (row) => {
  const node = row?.node
  if (!node) return
  if (node.is_dir) {
    void toggleTreeNode(node)
    return
  }
  void openEntry(node)
}

// ───── 请求 ─────
const fileOptionsPayload = () => ({
  algorithm: opts.algorithm,
  contextLines: Number(opts.contextLines) || 0,
  ignoreWhitespace: opts.ignoreWhitespace,
  ignoreTrailingWhitespace: opts.ignoreTrailingWhitespace,
  ignoreCase: opts.ignoreCase,
  ignoreBlankLines: opts.ignoreBlankLines,
})

const applyFileResult = (res) => {
  fileResult.value = res
  rows.value = buildFileRows(res)
  resetScroll()
}

const runFileDiff = async (leftPath, rightPath) => {
  const res = await invoke('diff_files', {
    leftPath,
    rightPath,
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}

const runTextDiff = async () => {
  const res = await invoke('diff_texts', {
    leftText: store.left.text ?? '',
    rightText: store.right.text ?? '',
    leftLabel: store.left.label || '左侧',
    rightLabel: store.right.label || '右侧',
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}
const runBufferDiff = async () => {
  const res = await invoke('diff_buffer_vs_file', {
    path: store.left.path,
    bufferText: store.right.text ?? '',
    options: fileOptionsPayload(),
  })
  applyFileResult(res)
}

const closeSession = async () => {
  const sid = sessionId.value
  if (!sid) return
  sessionId.value = ''
  try {
    await invoke('diff_dirs_close', { sessionId: sid })
  } catch (e) {
    logger.debug('[Diff] close session failed', { error: String(e?.message || e || '') })
  }
}

const dirOptionsPayload = () => {
  const excludeGlobs = excludeText.value
    .split(/[,;\n]/)
    .map((s) => s.trim())
    .filter(Boolean)
  return {
    quick: dirOpts.quick,
    detectRenames: dirOpts.detectRenames,
    includeHidden: dirOpts.includeHidden,
    respectGitignore: dirOpts.respectGitignore,
    includeSame: dirOpts.includeSame,
    pageSize: DIR_PAGE,
    excludeGlobs: excludeGlobs.length ? excludeGlobs : null,
  }
}

const runDirDiff = async () => {
  await closeSession()
  const sid = `dir-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  sessionId.value = sid
  progress.phase = 'scan'
  progress.scanned = 0
  progress.compared = 0
  progress.elapsed = 0
  dirEntries.value = []
  rows.value = []
  const res = await invoke('diff_dirs', {
    leftRoot: store.left.path,
    rightRoot: store.right.path,
    sessionId: sid,
    options: dirOptionsPayload(),
  })
  dirSummary.value = res.summary
  dirEntries.value = res.entries || []
  dirTotal.value = Number(res.summary?.total ?? dirEntries.value.length)
  dirHasMore.value = !!res.has_more
  resetTree()
  await refreshDirRows()
  resetScroll()
}
const drillPaths = ref({ left: '', right: '' })

const run = async () => {
  if (busy.value || !canRun.value) return
  // 重新对比会把只读视图整个换掉，编辑内容先处理好（有未保存改动会在这里问一句）。
  if (!(await ensureEditSafe())) return
  error.value = ''
  busy.value = true
  // 这一趟一定会重算，之前记下的"选项变了"到此为止
  optsStale = false
  // 手动跑过一次之后，后面的交换、重跑就该自动生效了。
  store.autoRun = true
  try {
    if (store.mode === 'text') {
      await runTextDiff()
    } else if (store.mode === 'buffer') {
      await runBufferDiff()
    } else if (drillRel.value) {
      await runFileDiff(drillPaths.value.left, drillPaths.value.right)
    } else if (isDirMode.value) {
      await runDirDiff()
    } else {
      await runFileDiff(store.left.path, store.right.path)
    }
  } catch (e) {
    error.value = String(e?.message || e || '对比失败')
    rows.value = []
    if (!drillRel.value) fileResult.value = null
    logger.warn('[Diff] compare failed', { mode: store.mode, error: error.value })
  } finally {
    busy.value = false
  }
}

const loadPage = async (offset, append) => {
  if (!sessionId.value) return
  dirLoadingPage.value = true
  try {
    const page = await invoke('diff_dirs_page', {
      sessionId: sessionId.value,
      offset,
      limit: DIR_PAGE,
      statuses: statusFilter.value.length ? [...statusFilter.value] : null,
      query: dirQuery.value ? dirQuery.value : null,
    })
    const list = page.entries || []
    dirEntries.value = append ? dirEntries.value.concat(list) : list
    dirTotal.value = Number(page.total || 0)
    dirHasMore.value = !!page.has_more
    rebuildDirRows()
    if (!append) resetScroll()
  } catch (e) {
    message.warning(String(e?.message || e || '取分页失败'))
  } finally {
    dirLoadingPage.value = false
  }
}

const loadNextPage = () => loadPage(dirEntries.value.length, true)
let filterTimer = 0
const onFilterChanged = () => {
  window.clearTimeout(filterTimer)
  filterTimer = window.setTimeout(() => {
    if (showDirTree.value) {
      // 过滤条件变了，缓存的层级全作废，从根重新要一层。
      resetTree()
      void refreshDirRows().then(resetScroll)
      return
    }
    void loadPage(0, false)
  }, 180)
}

const cancelDir = async () => {
  if (!sessionId.value) return
  try {
    const stopped = await invoke('diff_dirs_cancel', { sessionId: sessionId.value })
    if (!stopped) message.info('这轮扫描已经结束了')
  } catch (e) {
    message.warning(String(e?.message || e || '中止失败'))
  }
}

const openEntry = async (entry) => {
  if (!entry || entry.is_dir || busy.value) return
  error.value = ''
  busy.value = true
  try {
    const leftRel = entry.renamed_from || entry.rel
    const [lp] = await invoke('diff_dirs_entry_paths', { sessionId: sessionId.value, rel: leftRel })
    const pair = await invoke('diff_dirs_entry_paths', { sessionId: sessionId.value, rel: entry.rel })
    drillPaths.value = { left: lp, right: pair[1] }
    drillRel.value = entry.rel
    await runFileDiff(lp, pair[1])
  } catch (e) {
    error.value = String(e?.message || e || '打开该项失败')
    rows.value = []
  } finally {
    busy.value = false
  }
}

const backToDir = () => {
  drillRel.value = ''
  drillPaths.value = { left: '', right: '' }
  fileResult.value = null
  error.value = ''
  // 树的展开状态留着，返回时还是刚才那棵展开的树。
  void refreshDirRows()
  resetScroll()
}

const onSwap = () => {
  store.swap()
  if (!store.left.path || !store.right.path) return
  backToDir()
}
const pickSide = async (side) => {
  if (store.mode === 'text') return
  const dir = isDirMode.value
  const current = side === 'left' ? store.left.path : store.right.path
  try {
    const picked = await openDialog({
      directory: dir,
      multiple: false,
      title: `选择${side === 'left' ? '左' : '右'}侧${dir ? '目录' : '文件'}`,
      defaultPath: current || store.left.path || undefined,
    })
    const next = Array.isArray(picked) ? picked[0] : picked
    if (!next) return
    if (store.mode === 'buffer') {
      // 缓冲区对比的两侧是同一个文件的两个版本，手动挑一侧就退回普通文件对比。
      drillRel.value = ''
      store.compareFiles(side === 'left' ? next : store.left.path, side === 'right' ? next : store.left.path)
      return
    }
    store.setSide(side, next)
    if (store.left.path && store.right.path) {
      drillRel.value = ''
      void run()
    }
  } catch (e) {
    message.warning(String(e?.message || e || '选择失败'))
  }
}

// ───── 任意文件 / 目录：类型切换与系统拖放 ─────
/**
 * 换一侧、换模式之前先把上一轮的结果和目录会话清干净。
 * 返回 false 表示用户在"未保存改动"那道确认上选了留下，调用方应当整个中止。
 */
const resetResults = async () => {
  if (!(await ensureEditSafe())) return false
  await closeSession()
  drillRel.value = ''
  drillPaths.value = { left: '', right: '' }
  fileResult.value = null
  dirSummary.value = null
  dirEntries.value = []
  dirTotal.value = 0
  dirHasMore.value = false
  rows.value = []
  error.value = ''
  resetTree()
  return true
}

/** 路径到底是文件还是目录得问一下磁盘，光看字符串猜不出来（也顺便查出不存在的）。 */
const probeKinds = async (paths) => {
  const map = new Map()
  const list = (paths || []).filter(Boolean)
  if (!list.length) return map
  try {
    const got = await invoke('diff_probe_paths', { paths: list })
    for (const item of got || []) map.set(item.path, item.kind)
  } catch (e) {
    logger.warn('[Diff] probe paths failed', { error: String(e?.message || e || '') })
  }
  return map
}

/** 手动切"文件 / 目录"：类型对不上的已填路径留不住，只能丢掉。 */
const onKindChange = async (kind) => {
  const next = kind === 'dir' ? 'dir' : 'file'
  if (store.mode === next || busy.value) return
  const paths = [store.left.path, store.right.path].filter(Boolean)
  let keep = false
  if (paths.length && store.isPathMode) {
    const kinds = await probeKinds(paths)
    keep = paths.every((p) => kinds.get(p) === next)
  }
  if (!(await resetResults())) return
  store.setMode(next, { keepSides: keep })
  if (canRun.value) void run()
}

/** 把一个路径填到某一侧，必要时连模式一起切过去；两侧类型必须一致。 */
const fillSide = async (side, path, kind) => {
  const otherSide = side === 'left' ? 'right' : 'left'
  const otherPath = (otherSide === 'left' ? store.left.path : store.right.path) || ''
  if (kindValue.value !== kind) {
    let keepOther = false
    if (otherPath && store.isPathMode) {
      const kinds = await probeKinds([otherPath])
      keepOther = kinds.get(otherPath) === kind
      if (!keepOther) message.info('两侧类型必须一致，已清掉另一侧')
    }
    if (!(await resetResults())) return
    store.setMode(kind, { keepSides: keepOther })
  } else if (!(await resetResults())) {
    return
  }
  store.setSide(side, path)
  if (canRun.value) void run()
}

const dropActive = ref(false)
const dropSide = ref('left')

// onDragDropEvent 给的是物理像素，换成 CSS 像素才能和 innerWidth 比。
const sideFromX = (physicalX) => {
  const ratio = window.devicePixelRatio || 1
  const x = Number(physicalX || 0) / ratio
  return x > (window.innerWidth || 1) / 2 ? 'right' : 'left'
}

/**
 * 系统拖放落地：拖一个就填光标那一侧，拖两个直接填满两侧。
 * 文件和目录不能混着比，混了就明说，不去猜用户想比哪种。
 */
const acceptDrop = async (paths, side) => {
  const list = (paths || []).map((p) => String(p || '')).filter(Boolean)
  if (!list.length) return
  if (list.length > 2) message.info('一次只取前两个路径')
  const picked = list.slice(0, 2)
  const kinds = await probeKinds(picked)
  if (picked.some((p) => (kinds.get(p) || 'missing') === 'missing')) {
    message.warning('这个路径读不到，可能已经不在了')
    return
  }
  if (picked.length === 2) {
    if (kinds.get(picked[0]) !== kinds.get(picked[1])) {
      message.warning('只能文件对文件、目录对目录地比较')
      return
    }
    if (kinds.get(picked[0]) === 'dir') store.compareDirs(picked[0], picked[1])
    else store.compareFiles(picked[0], picked[1])
    return
  }
  await fillSide(side, picked[0], kinds.get(picked[0]))
}

// ───── 编辑模式 ─────
// 只读视图是自绘的虚拟滚动，故意不碰 Monaco（100 MB 文件进 Monaco 必崩）。
// 编辑模式另起一个 Monaco 差异编辑器，只在文件不大时可用：两侧都能改，各自存回原文件。
// Monaco 体积不小，所以走动态 import —— 全程只看差异的用户不该为它买单。

/** 和 Rust 侧 MAX_EDIT_BYTES 一致。Rust 才是权威，这里只用来提前禁用按钮。 */
const EDIT_MAX_BYTES = 8 * 1024 * 1024

/**
 * Monaco 算差异的时间上限（毫秒）。它跑在主线程上，且首屏一次、每次停下来打字后还有一次，
 * 所以这个值直接等于"界面最多冻多久"。默认 5000 对动辄几 MB 的文件就是整整五秒，
 * 收到 1000：超时只是让配对粗一点，但换来的是能用的手感。
 */
const EDIT_DIFF_MAX_COMPUTATION_MS = 1000

const editMode = ref(false)
const editLoading = ref(false)
/** 'load' 装 Monaco 与两个 model，'diff' 已经在算差异了——遮罩上的文案跟着换。 */
const editPhase = ref('load')
const editHostRef = ref(null)
const showAlgorithmHelp = ref(false)
const editSaving = reactive({ left: false, right: false })
const dirty = reactive({ left: false, right: false })
const editMeta = reactive({ left: null, right: null })

let monacoMod = null
let diffEditor = null
let liveDiffSync = null
const editModels = { left: null, right: null }
const editBaselines = { left: 0, right: 0 }
let editChangeDisposables = []
/** 编辑期间存过盘：退出编辑模式时要重算一次只读差异，否则屏幕上还是旧结果。 */
let savedDuringEdit = false
/** 编辑期间改过对比选项：退出编辑模式时同样要重算，否则只读结果还是按旧选项算的。 */
let optsStale = false
let bypassCloseGuard = false
let closeGuardBusy = false
let unlistenClose = null

const anyDirty = computed(() => dirty.left || dirty.right)

/** 编辑针对的两个磁盘文件：钻取态用 drillPaths，普通文件对比用 store 的两侧。 */
const editPaths = computed(() => {
  if (drillRel.value) return { left: drillPaths.value.left, right: drillPaths.value.right }
  if (store.mode === 'file') return { left: store.left.path, right: store.right.path }
  return { left: '', right: '' }
})

/** 现在能不能进编辑模式；不能的话缺的是什么。 */
const editDisabledReason = computed(() => {
  if (store.mode === 'text') return '文本对比是两段未落盘的内容，没有文件可写回'
  if (store.mode === 'buffer') return '缓冲区对比有一侧是未保存内容，没有文件可写回'
  if (store.mode === 'dir' && !drillRel.value) return '目录对比请先点开具体文件，再进入编辑'
  const res = fileResult.value
  if (!res) return '先点"对比"算出差异，才能进入编辑'
  if (res.mode === 'binary') return '二进制文件只能只读对比'
  if (res.mode === 'too_large') return '文件超过大小上限，只能只读对比'
  if (res.mode === 'too_divergent') return '两侧差异过大，只能只读对比'
  const paths = editPaths.value
  if (!paths.left || !paths.right) return '两侧都要是磁盘上的文件才能编辑'
  if (!res.left.exists || !res.right.exists) return '有一侧的文件不存在'
  if (Math.max(res.left.size || 0, res.right.size || 0) > EDIT_MAX_BYTES) {
    return `文件超过 ${EDIT_MAX_BYTES / 1024 / 1024} MB，只支持只读对比`
  }
  return ''
})
const canEditDiff = computed(() => !editDisabledReason.value)

/** 两侧读盘时的提示（行尾不统一之类）并成一句话。 */
const editNote = computed(() =>
  [editMeta.left?.message, editMeta.right?.message].filter(Boolean).join('；'))

const eolLabel = (v) => ({ crlf: 'CRLF', lf: 'LF', none: '无换行' }[v] || v || '')

/** 动态装载 Monaco：全程只读的会话不会拉起这个 chunk。
 *  Worker 环境由 main.js 启动时统一装好（configureMonacoWorkers），这里不用再管。 */
const loadMonaco = async () => {
  if (!monacoMod) monacoMod = await import('monaco-editor')
  return monacoMod
}

/** Monaco 吃不下 CSS 变量里的 var() 链，先让浏览器把字体栈算成实际值。
 *  必须量 .dp-edit-body —— 只读的 .dp-body 在编辑模式下已经被 v-if 摘掉了，
 *  退到面板根元素只会拿到主界面字体，代码就换了副样子。 */
const editorHostStyle = () => {
  const el = editHostRef.value?.parentElement || document.querySelector('.guaxi-diff-panel')
  return el ? window.getComputedStyle(el) : null
}

const readHostVar = (name, fallback) => {
  const v = editorHostStyle()?.getPropertyValue(name)?.trim()
  return v || fallback
}

/**
 * 同 readHostVar，但只放行十六进制。
 * Monaco 的主题色只认 #RRGGBB(AA)：读到 rgb(...) 之类的写法它不报错，
 * 而是把底色、差异色一起画成纯红，所以宁可用这里的默认值。
 */
const readHostHex = (name, fallback) => {
  const v = readHostVar(name, fallback)
  return /^#[0-9a-f]{3,8}$/i.test(v) ? v : fallback
}

const hexToRgb = (hex) => {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(hex).trim())
  if (!m) return null
  const h = m[1].length === 3 ? m[1].split('').map((c) => c + c).join('') : m[1]
  const n = Number.parseInt(h, 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

/**
 * 把面板 CSS 里 color-mix(色 透明度, transparent) 的效果算成 #RRGGBBAA。
 *
 * 必须是十六进制：Monaco 的 Color.fromHex 只认 #RGB/#RGBA/#RRGGBB/#RRGGBBAA，
 * 喂 rgba() 进去不报错，而是静默兜成纯红——配色错了都看不出来是哪里错。
 */
const withAlpha = (color, alpha) => {
  const rgb = hexToRgb(color)
  if (!rgb) return color
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255).toString(16).padStart(2, '0')
  return `#${rgb.map((v) => v.toString(16).padStart(2, '0')).join('')}${a}`
}

/**
 * 差异配色。只读视图（.dp-cell.is-del/.is-add/.dp-ink）和编辑模式的主题必须是同一组色，
 * 不然同一个文件在两种模式里一个发青绿、一个发橄榄绿（Monaco 默认就是后者）。
 * 改色时两边一起改。
 */
const DIFF_HUES = { removed: '#f87171', inserted: '#4ade80' }

/** 当前行底色：只读视图的 .dp-line.is-current 和编辑模式的 editor.lineHighlightBackground 共用一份。 */
const CURRENT_LINE_ALPHA = 0.1

/**
 * 编辑模式专用主题。
 *
 * 不能直接用 guaxi-light/guaxi-dark：那是给 GitPanel 配的，底色是 #282c34，
 * 而只读视图的底色是 --guaxi-panel-bg（暗色 #21252b）—— 两个模式一对比就看得出接缝。
 * 行号、差异底色、行内高亮、滚动条、中缝同样按只读视图的 CSS 取值，让两边看起来是同一块东西。
 */
const defineDiffEditTheme = (monaco) => {
  const dark = settingsStore.appearanceTheme === 'dark'
  const name = dark ? 'guaxi-diffedit-dark' : 'guaxi-diffedit-light'
  const panelBg = readHostHex('--guaxi-panel-bg', dark ? '#21252b' : '#ffffff')
  const textPrimary = readHostHex('--guaxi-text-primary', dark ? '#d8e0eb' : '#1f2328')
  const textSecondary = readHostHex('--guaxi-text-secondary', dark ? '#9da5b4' : '#57606a')
  const border = readHostHex('--guaxi-border', dark ? '#3e4451' : '#d0d7de')
  const accent = readHostHex('--guaxi-accent', dark ? '#4c8bf5' : '#2f6feb')
  // .dp-no 是 76% 的文字次要色
  const lineNumber = withAlpha(textSecondary, 0.76)
  // 滚动条滑块和 GuaxiEditor 里那套 rgba(120,120,120,.45)/rgba(255,255,255,.28) 对齐
  const scrollThumb = dark ? ['#ffffff47', '#ffffff73', '#ffffff8c'] : ['#78787873', '#787878a6', '#787878cc']

  monaco.editor.defineTheme(name, {
    base: dark ? 'vs-dark' : 'vs',
    inherit: true,
    rules: dark ? GUAXI_MONACO_THEME_RULES.dark : GUAXI_MONACO_THEME_RULES.light,
    semanticHighlighting: true,
    semanticTokenColors: dark
      ? GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.dark
      : GUAXI_MONACO_SEMANTIC_TOKEN_COLORS.light,
    colors: {
      'editor.background': panelBg,
      'editor.foreground': textPrimary,
      'editorGutter.background': panelBg,
      // 当前行：底色取只读视图 .dp-line.is-current 的同一份（次要色 10%），
      // 左边那条强调色短线由样式里的 .current-line-margin 补。
      'editor.lineHighlightBackground': withAlpha(textSecondary, CURRENT_LINE_ALPHA),
      // 这个边框是四边一圈的框，不是我们要的左边一条，保持透明。
      'editor.lineHighlightBorder': '#00000000',
      'editorWidget.background': panelBg,
      'editorWidget.border': border,
      // 行号跟 .dp-no 一样：次要色的 76%，亮色主题下不再是 Monaco 默认的蓝
      'editorLineNumber.foreground': lineNumber,
      'editorLineNumber.activeForeground': withAlpha(textPrimary, 0.9),
      // 整行 16% / 行内 40%，对应 .dp-cell.is-del/.is-add 与 .dp-ink
      'diffEditor.removedLineBackground': withAlpha(DIFF_HUES.removed, 0.16),
      'diffEditor.insertedLineBackground': withAlpha(DIFF_HUES.inserted, 0.16),
      'diffEditor.removedTextBackground': withAlpha(DIFF_HUES.removed, 0.4),
      'diffEditor.insertedTextBackground': withAlpha(DIFF_HUES.inserted, 0.4),
      // 只读视图只给代码那一列上色，行号栏留白，这里也把 gutter 的底色抹平
      'diffEditorGutter.removedLineBackground': '#00000000',
      'diffEditorGutter.insertedLineBackground': '#00000000',
      // 另一侧空出来的位置，对应 .dp-cell.is-blank 的 8% 次要色
      'diffEditor.diagonalFill': withAlpha(textSecondary, 0.08),
      // 两侧之间的那道缝，对应 .dp-divider
      'diffEditor.border': border,
      'sash.hoverBorder': accent,
      'scrollbar.shadow': '#00000000',
      'scrollbarSlider.background': scrollThumb[0],
      'scrollbarSlider.hoverBackground': scrollThumb[1],
      'scrollbarSlider.activeBackground': scrollThumb[2],
    },
  })
  return name
}

/**
 * 编辑模式的实时差异。Monaco 0.52 那套重算有两处靠不住，都在这里补上：
 *
 * 1. 慢：重算挂在自己 ViewModel 里一个 200ms 的 RunOnceScheduler 上，停下来打字之后还要再等
 *    200ms 才动一步（实测 4000 行文件每次按键 227ms 才更新）。VS Code 靠 applyModifiedEdits
 *    把编辑增量套到旧结果上做到"边打边更新"，而这个函数在 monaco 0.52 里是空实现
 *    （diffEditorViewModel.js 里直接 return undefined），只能自己把重算提前。
 * 2. 抖：每重算一次，两侧对不齐时补的 view zone 和整屏差异装饰都会整批拆掉重建
 *    （实测 4000 行文件敲一个字符动 160 个 view zone 节点、300 多处 DOM 写入），
 *    看起来就是"整个编辑区在重绘"。
 *
 * 所以行内字符级高亮改成面板自己画（.dp-edit-ink-*，取值同只读视图的 .dp-ink），行级结构
 * （哪几行跟哪几行对应）没变就不去惊动 Monaco，真的多/少了一行才让它重算一次。
 * 实测：打字时 view zone 变更为 0、按键到行内高亮约 14ms；换行这类真正改结构的操作约 20ms。
 */
const EDIT_INK_CLASS = { add: 'dp-edit-ink-add', del: 'dp-edit-ink-del' }
/** 行内高亮只给"成对且行数相等"的改动块，每块上限 400 行——跟只读视图 diffcompare.rs 的规则一致。 */
const EDIT_INK_PAIR_LIMIT = 400
const EDIT_DIFF_RETRY_DELAY_MS = 60
const EDIT_DIFF_RETRY_LIMIT = 3

/**
 * worker 被回收、或者模型还没同步进 worker 时会拿不到结果（editorWorkerService 把 null
 * 抛成 'no diff result available'）。这种失败等一拍再问就有结果；不重试的话这一轮更新整段丢掉，
 * 红绿就停在上一次的位置不动，看起来就是"改了没反应"。
 */
const isRetryableDiffError = (e) => {
  const msg = String(e?.message || e || '')
  return msg.includes('no diff result available') || /cancel/i.test(msg)
}

const computeDiffWithRetry = async (provider, original, modified, options, token) => {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await provider.computeDiff(original, modified, options, token)
    } catch (e) {
      if (attempt >= EDIT_DIFF_RETRY_LIMIT || token?.isCancellationRequested || !isRetryableDiffError(e)) throw e
      // 编辑器已经拆掉（模型跟着销毁）就没必要再问一遍
      if (original?.isDisposed?.() || modified?.isDisposed?.()) throw e
      await new Promise((r) => window.setTimeout(r, EDIT_DIFF_RETRY_DELAY_MS * (attempt + 1)))
    }
  }
}

/**
 * 只比较行级区间，不看行内字符级差异：后者由面板自己画，不必惊动 Monaco 的差异视图。
 * 两份结果的区间一一对应就是"结构没变"——整行底色、两侧对齐都不会有变化。
 */
const sameLineStructure = (a, b) => {
  const before = a?.mappings
  const after = b?.mappings
  if (!before || !after || before.length !== after.length) return false
  for (let i = 0; i < before.length; i += 1) {
    const was = before[i]?.lineRangeMapping
    const now = after[i]?.lineRangeMapping
    if (!was || !now) return false
    if (was.original.startLineNumber !== now.original.startLineNumber) return false
    if (was.original.endLineNumberExclusive !== now.original.endLineNumberExclusive) return false
    if (was.modified.startLineNumber !== now.modified.startLineNumber) return false
    if (was.modified.endLineNumberExclusive !== now.modified.endLineNumberExclusive) return false
  }
  return true
}

/** 结构指纹：和 sameLineStructure 是同一套口径，一份给算出来的结果用，一份给 ViewModel 的状态用。 */
const lineStructureKey = (result) => (result?.changes || [])
  .map((c) => `${c.original.startLineNumber}-${c.original.endLineNumberExclusive}/${c.modified.startLineNumber}-${c.modified.endLineNumberExclusive}`)
  .join('|')

const createLiveDiffSync = (models, vm, base, setup) => {
  const listeners = new Set()
  let firstRun = true
  let lastKey = null
  let inflight = 0
  let pending = false
  let timer = 0
  let disposed = false
  let inkEnabled = setup?.ink !== false
  let restoreDiffSet = null
  const inkIds = { left: [], right: [] }

  /** 装成 Monaco 的 diffAlgorithm：计算转回它原来那个 provider，同一个 worker、同一份结果缓存。 */
  const provider = {
    onDidChange: (listener) => {
      listeners.add(listener)
      return { dispose: () => listeners.delete(listener) }
    },
    computeDiff: (original, modified, options, token) => computeDiffWithRetry(base, original, modified, options, token),
  }

  /**
   * 交给 worker 的选项必须和 ViewModel 自己拼的那份一样：provider 的结果缓存拿它做键，
   * 键一致时 Monaco 收到通知后那次重算就是命中缓存，几乎不花时间。
   */
  const readOptions = () => ({
    ignoreTrimWhitespace: vm._options.ignoreTrimWhitespace.get(),
    maxComputationTimeMs: vm._options.maxComputationTimeMs.get(),
    computeMoves: vm._options.showMoves.get(),
  })

  /**
   * 行内高亮装饰。规则照只读视图来：纯增/纯删的整行没有行内高亮（那边只有 16% 的整行底色），
   * 行数不等的改动块算不成配对、也不给高亮。空区间不画，不然会多出一条零宽的色块。
   */
  const inkDecorations = (result) => {
    const out = { left: [], right: [] }
    for (const change of result?.changes || []) {
      const before = change.original
      const after = change.modified
      if (before.isEmpty || after.isEmpty) continue
      if (before.lineCount !== after.lineCount || before.lineCount > EDIT_INK_PAIR_LIMIT) continue
      for (const inner of change.innerChanges || []) {
        if (!inner.originalRange.isEmpty?.()) {
          out.left.push({ range: inner.originalRange, options: { description: EDIT_INK_CLASS.del, inlineClassName: EDIT_INK_CLASS.del } })
        }
        if (!inner.modifiedRange.isEmpty?.()) {
          out.right.push({ range: inner.modifiedRange, options: { description: EDIT_INK_CLASS.add, inlineClassName: EDIT_INK_CLASS.add } })
        }
      }
    }
    return out
  }

  const clearInk = () => {
    for (const side of ['left', 'right']) {
      if (!models[side] || !inkIds[side].length) continue
      try {
        inkIds[side] = models[side].deltaDecorations(inkIds[side], [])
      } catch {
        // 模型已经跟着编辑器销毁了
      }
    }
  }

  const paintInk = (result) => {
    if (!inkEnabled) return
    const decos = inkDecorations(result)
    inkIds.left = models.left.deltaDecorations(inkIds.left, decos.left)
    inkIds.right = models.right.deltaDecorations(inkIds.right, decos.right)
  }

  const notify = () => {
    for (const listener of Array.from(listeners)) {
      try {
        listener()
      } catch {
        // Monaco 自己的订阅，出错也轮不到我们接管
      }
    }
  }

  /** 挪出当前事件循环再算：内容变更处理途中去动 Model 状态容易撞上 Monaco 自己的更新 */
  const schedule = () => {
    if (timer || disposed) return
    timer = window.setTimeout(() => {
      timer = 0
      run()
    }, 0)
  }

  const run = async () => {
    if (disposed) return
    inflight += 1
    try {
      // provider 只读 token.isCancellationRequested，这里不需要真的取消
      const token = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose: () => {} }) }
      const result = await computeDiffWithRetry(base, models.left, models.right, readOptions(), token)
      // 这一趟算的时候又敲过键：丢掉这个过期结果，等下一趟
      if (disposed || pending) return
      paintInk(result)
      const key = lineStructureKey(result)
      const changed = key !== lastKey
      lastKey = key
      // 首轮不通知：Monaco 建编辑器时已经按同一份内容算过一遍，结构是一样的
      if (changed && !firstRun) notify()
      firstRun = false
    } catch (e) {
      logger.warn('[Diff] live diff compute failed', { error: String(e?.message || e || '') })
    } finally {
      inflight -= 1
      if (pending && inflight === 0) {
        pending = false
        schedule()
      }
    }
  }

  return {
    provider,
    /** 内容变了：没有计算在飞就立刻算，有的话记下来等它回来 */
    contentChanged() {
      if (disposed) return
      if (inflight > 0) {
        pending = true
        return
      }
      schedule()
    },
    seed() {
      schedule()
    },
    /**
     * 并排视图才由面板自己画行内高亮。统一视图里"删掉的那几行"是画在 view zone 里、拿原文本
     * 重新渲染的，模型上的装饰落不到那段 DOM 上，所以那种视图把行内高亮还给 Monaco。
     */
    setInkEnabled(enabled) {
      if (disposed || inkEnabled === enabled) return
      inkEnabled = enabled
      if (!enabled) {
        clearInk()
        // 统一视图的行内标记读的是 Monaco 自己那份 diff 结果，而并排期间只改行内、不改行级结构
        // 的那些结果都被吞掉了：这里让它重算一次，切过去才不会是旧标记。
        notify()
        return
      }
      if (!inflight) schedule()
    },
    /**
     * 接管 ViewModel 的 diff 观察值：行级结构没变就把这次通知吞掉。
     *
     * Monaco 每 200ms 自己还会重算一次（原本的节奏），结果的行区间明明没变，却照样会让
     * view zone 和整屏装饰整批重建——就是编辑时看到的"整片区域在闪"。并排视图下行内高亮
     * 已经由 .dp-edit-ink-* 自己画了，所以这里吞掉不会有任何视觉损失。
     *
     * 只在自家画行内高亮（并排视图）时吞：统一视图的行内标记还是 Monaco 拿这份结果画的
     * （删掉的行在 view zone 里重新渲染，模型装饰落不进去），吞了就会停在旧标记上。
     */
    takeOverDiffValue() {
      const diffValue = vm._diff
      const originalSet = diffValue.set
      diffValue.set = function patchedSet(value, tx, change) {
        if (inkEnabled && change === undefined && sameLineStructure(diffValue.get(), value)) return
        return originalSet.call(diffValue, value, tx, change)
      }
      restoreDiffSet = () => {
        diffValue.set = originalSet
      }
    },
    dispose() {
      disposed = true
      if (timer) window.clearTimeout(timer)
      timer = 0
      restoreDiffSet?.()
      restoreDiffSet = null
      listeners.clear()
      clearInk()
    },
  }
}

/**
 * 把实时同步装进 diff 编辑器；返回 null 表示这次没装上，行为退回 Monaco 原本的 200ms 节奏。
 *
 * 只能从内部装配：Monaco 0.52 把 diffAlgorithm 按字符串枚举校验（editorOptions.js 的 stringSet
 * 对非字符串直接退回原值），自定义 provider 传不进去，所以这里改的是 DiffEditorOptions 内部
 * 那个 observable。写进去之后 Monaco 自己的响应式链路会把 diffAlgorithm 重新求值、重建 diff
 * provider，新 provider 的 computeDiff 再转回原来那个（同一个 worker、同一份缓存）。
 * Monaco 哪天改了内部结构，这里就是拿不到字段然后安静退回。
 */
const installLiveDiffSync = (editor, models, setup) => {
  try {
    const vm = editor?._diffModel?.get?.()
    const options = vm?._options?._options
    const base = vm?._diffProvider?.get?.()?.diffProvider
    if (typeof options?.get !== 'function' || typeof options?.set !== 'function') return null
    if (typeof base?.computeDiff !== 'function') return null
    const sync = createLiveDiffSync(models, vm, base, setup)
    options.set({ ...options.get(), diffAlgorithm: sync.provider }, undefined, { changedOptions: { diffAlgorithm: sync.provider } })
    sync.takeOverDiffValue()
    sync.seed()
    return sync
  } catch (e) {
    logger.warn('[Diff] live diff sync unavailable', { error: String(e?.message || e || '') })
    return null
  }
}

const disposeEditor = () => {
  for (const d of editChangeDisposables) {
    try { d.dispose() } catch { /* 已随编辑器一起销毁 */ }
  }
  editChangeDisposables = []
  if (liveDiffSync) {
    try { liveDiffSync.dispose() } catch { /* ignore */ }
    liveDiffSync = null
  }
  editHostRef.value?.classList.remove('is-live-ink')
  if (diffEditor) {
    try { diffEditor.setModel(null) } catch { /* ignore */ }
    try { diffEditor.dispose() } catch { /* ignore */ }
    diffEditor = null
  }
  for (const side of ['left', 'right']) {
    if (editModels[side]) {
      try { editModels[side].dispose() } catch { /* ignore */ }
      editModels[side] = null
    }
  }
  if (editHostRef.value) editHostRef.value.innerHTML = ''
}

/**
 * 等到浏览器真的画完一帧。连等两次 rAF 是有意的：第一帧交给布局/样式，第二帧交给合成，
 * 之后再去跑那段同步的差异计算，遮罩上的新文案才来得及上屏。
 */
const paintFrame = () => new Promise((resolve) => {
  requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
})

const mountEditor = async () => {
  const host = editHostRef.value
  if (!host) return
  const monaco = await loadMonaco()

  disposeEditor()
  const paths = editPaths.value
  const makeModel = (side) => monaco.editor.createModel(
    editMeta[side].text,
    getLanguageForFile(paths[side]),
    monaco.Uri.file(paths[side]),
  )
  editModels.left = makeModel('left')
  editModels.right = makeModel('right')

  const fontSize = Number.parseFloat(readHostVar('--dp-code-size', '16px')) || 16
  const fontFamily = editorHostStyle()?.fontFamily || 'Consolas, monospace'
  // 主题要在建编辑器之前定好，否则第一帧会先按默认主题画一遍再被换掉，看着闪一下。
  const themeName = defineDiffEditTheme(monaco)
  monaco.editor.setTheme(themeName)

  diffEditor = monaco.editor.createDiffEditor(host, {
    // 两侧都要能改：左侧是"原版"但用户同样要修它。
    originalEditable: true,
    readOnly: false,
    // 折叠加号、概览标尺这些只读视图没有的东西一律关掉，否则同一个文件在两种模式里像两份东西。
    renderSideBySide: viewMode.value === 'split',
    // Monaco 0.52 只有 legacy/advanced，Patience/Myers/LCS 是 Rust 侧只读对比的算法。
    diffAlgorithm: 'advanced',
    // 算差异是同步跑在主线程上的（首次装载一次，之后每敲完一段还有一次）。默认 5 秒意味着
    // 大文件能把界面冻满 5 秒，红绿就是在冻完之后才蹦出来的。收到 1 秒：超时只让配对变粗，
    // 不会算错，卡顿上限却降下来了。
    maxComputationTime: EDIT_DIFF_MAX_COMPUTATION_MS,
    // 面板窄于 900px 时 Monaco 会自作主张切成 unified，把工具栏里选的"并排"顶掉。
    useInlineViewWhenSpaceIsLimited: false,
    // 只读视图默认"空白差异也算差异"，这里必须跟着走，否则同一对文件两种模式结论不同。
    ignoreTrimWhitespace: opts.ignoreWhitespace,
    automaticLayout: true,
    // 行高/字号/字体都跟只读视图同一份来源，两边的行才对得齐。
    lineHeight: ROW_H,
    fontSize,
    fontFamily,
    fontLigatures: true,
    lineNumbersMinChars: 3,
    lineDecorationsWidth: 8,
    glyphMargin: false,
    folding: false,
    // 当前行高亮要留着：跟只读视图一样，光标落在哪一行就亮哪一行（配色见 defineDiffEditTheme）。
    // 取 all 而不是 line：行号栏那一截也要跟着亮，才和只读视图那边整行高亮的样子对得上。
    renderLineHighlight: 'all',
    renderIndicators: false,
    renderMarginRevertIcon: false,
    overviewRulerLanes: 0,
    minimap: { enabled: false },
    // 下面这些都只读视图没有：缩进参考线、括号着色、不可见字符高亮、超长行折行。
    // 不关掉的话同一个文件在两种模式里长得像两份东西。
    guides: { indentation: false, bracketPairs: false, highlightActiveBracketPair: false },
    bracketPairColorization: { enabled: false },
    unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false },
    renderWhitespace: 'none',
    wordWrap: 'off',
    scrollBeyondLastLine: false,
    enableSplitViewResizing: true,
    renderOverviewRuler: false,
    diffWordWrap: 'off',
    scrollbar: { useShadows: false, verticalScrollbarSize: 10, horizontalScrollbarSize: 10 },
    // 只读视图的行是紧贴着的，这里也把留白收掉
    padding: { top: 0, bottom: 0 },
  })

  // setModel 这一步才会真正算差异，而且是同步阻塞的。先让"正在计算差异…"这一帧上屏，
  // 用户看到的是有说明的等待，而不是界面莫名僵住。
  editPhase.value = 'diff'
  await paintFrame()
  diffEditor.setModel({ original: editModels.left, modified: editModels.right })

  // 首屏那次差异算完就接管：行内高亮和行级结构都改由面板自己盯着，不再等 Monaco 那 200ms。
  // 装上了才挂 is-live-ink：那条 CSS 会遮掉 Monaco 的 char-* 行内高亮，万一没装上得让它留着。
  const inkSideBySide = viewMode.value === 'split'
  liveDiffSync = installLiveDiffSync(diffEditor, editModels, { ink: inkSideBySide })
  if (liveDiffSync) editHostRef.value?.classList.toggle('is-live-ink', inkSideBySide)

  editBaselines.left = editModels.left.getAlternativeVersionId()
  editBaselines.right = editModels.right.getAlternativeVersionId()
  dirty.left = false
  dirty.right = false
  for (const side of ['left', 'right']) {
    const model = editModels[side]
    editChangeDisposables.push(model.onDidChangeContent(() => {
      dirty[side] = model.getAlternativeVersionId() !== editBaselines[side]
      liveDiffSync?.contentChanged()
    }))
  }
}

const enterEditMode = async () => {
  if (editMode.value || editLoading.value || !canEditDiff.value) return
  error.value = ''
  editPhase.value = 'load'
  editLoading.value = true
  try {
    const paths = editPaths.value
    const [leftInfo, rightInfo] = await Promise.all([
      invoke('diff_read_text', { path: paths.left }),
      invoke('diff_read_text', { path: paths.right }),
    ])
    if (!leftInfo.canEdit || !rightInfo.canEdit) {
      message.warning(leftInfo.message || rightInfo.message || '这一侧的文件不支持编辑')
      return
    }
    editMeta.left = leftInfo
    editMeta.right = rightInfo
    editMode.value = true
    await nextTick()
    await mountEditor()
  } catch (e) {
    error.value = String(e?.message || e || '进入编辑模式失败')
    logger.warn('[Diff] enter edit mode failed', { error: error.value })
  } finally {
    editLoading.value = false
  }
}

/**
 * 退出编辑模式。有未保存改动先问一句。
 * `refresh` 为真时把只读差异重算一遍 —— 用户点"只读"就是为了看刚存下去的结果。
 */
const exitEditMode = async ({ refresh = false } = {}) => {
  if (!editMode.value) return true
  if (anyDirty.value) {
    const ok = await confirmDialog('有未保存的改动，退出编辑模式会丢掉这些改动。确定退出吗？', {
      title: '未保存的改动',
      kind: 'warning',
    })
    if (!ok) return false
  }
  const wasSaved = savedDuringEdit
  disposeEditor()
  editMode.value = false
  editMeta.left = null
  editMeta.right = null
  dirty.left = false
  dirty.right = false
  savedDuringEdit = false
  // 只读视图的 DOM 被 v-if 换掉过，ref / ResizeObserver 得重新挂上。
  await nextTick()
  observeBody()
  syncViewport()
  measureCharWidth()
  // 存过盘、或者改过对比选项，只读视图都得重算；什么都没发生就别白跑一趟。
  if (refresh && (wasSaved || optsStale)) {
    optsStale = false
    const paths = editPaths.value
    if (paths.left && paths.right) {
      try {
        await runFileDiff(paths.left, paths.right)
      } catch (e) {
        logger.warn('[Diff] refresh after edit failed', { error: String(e?.message || e || '') })
      }
    }
  }
  return true
}

/** 任何会丢掉编辑内容的流程（切模式、重新对比、换文件）都先过这道闸。 */
const ensureEditSafe = async () => {
  if (!editMode.value) return true
  return exitEditMode()
}

const onEditModeChange = (v) => {
  if (v === 'edit') void enterEditMode()
  else void exitEditMode({ refresh: true })
}

const fileBaseName = (p) => String(p || '').split(/[\\/]/).filter(Boolean).pop() || String(p || '')

const saveSide = async (side, { force = false } = {}) => {
  if (!editMode.value || editSaving[side] || !dirty[side]) return
  const model = editModels[side]
  const meta = editMeta[side]
  const path = editPaths.value[side]
  if (!model || !meta || !path) return
  // 保存是异步的，期间用户可能继续打字：记住这次发出去的版本号，
  // 回来只把 baseline 推到这一版，之后的改动仍然算"未保存"。
  const sentVersion = model.getAlternativeVersionId()
  const sentText = model.getValue()
  editSaving[side] = true
  try {
    const res = await invoke('diff_write_text', {
      path,
      text: sentText,
      encoding: meta.encoding,
      lineEnding: meta.lineEnding,
      expectedMtimeMs: meta.mtimeMs,
      force,
    })
    meta.mtimeMs = res.mtimeMs
    meta.size = res.size
    editBaselines[side] = sentVersion
    dirty[side] = model.getAlternativeVersionId() !== sentVersion
    savedDuringEdit = true
    if (res.message) message.warning(res.message)
    else message.success(`已保存 ${fileBaseName(path)}`)
  } catch (e) {
    const text = String(e?.message || e || '保存失败')
    // Rust 用前缀区分"需要用户再确认一次"和真错误。
    if (!force && (text.startsWith('stale:') || text.startsWith('encode:'))) {
      const reason = text.slice(text.indexOf(':') + 1)
      const ok = await confirmDialog(`${reason}。\n\n仍要覆盖保存吗？`, {
        title: '保存确认',
        kind: 'warning',
      })
      if (ok) {
        editSaving[side] = false
        await saveSide(side, { force: true })
        return
      }
      return
    }
    message.error(text)
    logger.warn('[Diff] save failed', { side, error: text })
  } finally {
    editSaving[side] = false
  }
}

const savingAny = computed(() => editSaving.left || editSaving.right)

/** 保存按钮的说明：写清这次会存哪几侧；没得存时也说明白，免得看成按钮坏了。 */
const saveHint = computed(() => {
  if (savingAny.value) return '正在保存…'
  const sides = [dirty.left ? '左' : '', dirty.right ? '右' : ''].filter(Boolean)
  return sides.length ? `把${sides.join('、')}侧的改动写回原文件 · Ctrl+S` : '两侧都没有未保存的改动'
})

/** 工具栏只留一颗保存按钮：把当前脏的那几侧写回去。 */
const saveDirty = async () => {
  if (!editMode.value || savingAny.value) return
  const sides = ['left', 'right'].filter((side) => dirty[side])
  if (!sides.length) {
    message.info('两侧都没有未保存的改动')
    return
  }
  // 串行：两侧都可能弹"要不要覆盖"确认，一起发会同时叠出两个弹窗
  for (const side of sides) await saveSide(side)
}

/** Ctrl+S 存哪一侧：光标在哪侧就存哪侧，都没焦点就存唯一脏的那侧。 */
const focusedSide = () => {
  if (diffEditor) {
    try {
      if (diffEditor.getModifiedEditor().hasTextFocus()) return 'right'
      if (diffEditor.getOriginalEditor().hasTextFocus()) return 'left'
    } catch { /* 编辑器可能正在销毁 */ }
  }
  if (dirty.left !== dirty.right) return dirty.left ? 'left' : 'right'
  return null
}

const onEditKeydown = (ev) => {
  if (!editMode.value) return
  if (!(ev.ctrlKey || ev.metaKey) || String(ev.key).toLowerCase() !== 's') return
  ev.preventDefault()
  ev.stopPropagation()
  const side = focusedSide()
  if (side) {
    void saveSide(side)
    return
  }
  // 焦点不在编辑器上（刚点过工具栏之类）：那就是想全存，和工具栏那颗按钮走同一条路。
  if (anyDirty.value) void saveDirty()
}

/** 关窗也是离开编辑的一条路，同样要拦一下。
 *
 * 注意 Tauri 的 onCloseRequested 在 handler 没 preventDefault 时会自己调 window.destroy()：
 * 注册了这个监听之后，所有关闭路径（工具栏的 X 和系统标题栏的 X）最后都落到 destroy 上，
 * 所以 capabilities 里必须放行 core:window:allow-destroy，否则窗口会关不掉。 */
const setupCloseGuard = async () => {
  if (!store.host) return
  try {
    unlistenClose = await getCurrentWindow().onCloseRequested(async (ev) => {
      if (bypassCloseGuard || !editMode.value || !anyDirty.value) return
      // 确认框弹着的时候用户又点了 X：别叠第二个框。
      if (closeGuardBusy) {
        ev.preventDefault()
        return
      }
      ev.preventDefault()
      closeGuardBusy = true
      try {
        const ok = await confirmDialog('有未保存的改动，关闭窗口会丢掉这些改动。仍要关闭吗？', {
          title: '未保存的改动',
          kind: 'warning',
        })
        if (!ok) return
        bypassCloseGuard = true
        await getCurrentWindow().close()
      } finally {
        closeGuardBusy = false
      }
    })
  } catch (e) {
    logger.warn('[Diff] close guard failed', { error: String(e?.message || e || '') })
  }
}

// ───── 生命周期 ─────
let unlisten = null
let unlistenDrop = null
let ro = null
let optTimer = 0

const observeBody = () => {
  // 进出编辑模式会把 .dp-body 整个换掉，得先把旧观察者摘掉再挂新的。
  ro?.disconnect()
  ro = null
  if (!bodyRef.value || typeof ResizeObserver === 'undefined') return
  ro = new ResizeObserver(() => syncViewport())
  ro.observe(bodyRef.value)
}

/**
 * 接管窗口级的系统拖放。只在独立对比窗口里挂：嵌到别的窗口时不该抢宿主的文件拖入。
 */
const setupDragDrop = async () => {
  if (!store.host) return
  try {
    unlistenDrop = await getCurrentWindow().onDragDropEvent((ev) => {
      const p = ev?.payload || {}
      if (p.type === 'enter' || p.type === 'over') {
        dropActive.value = true
        dropSide.value = sideFromX(p.position?.x)
        return
      }
      dropActive.value = false
      if (p.type === 'drop') void acceptDrop(p.paths, sideFromX(p.position?.x))
    })
  } catch (e) {
    logger.warn('[Diff] listen drag drop failed', { error: String(e?.message || e || '') })
  }
}

onMounted(async () => {
  await nextTick()
  syncViewport()
  measureCharWidth()
  observeBody()
  try {
    unlisten = await listen('diff-dirs-progress', (ev) => {
      const p = ev?.payload
      if (!p || p.session_id !== sessionId.value) return
      progress.phase = p.phase
      progress.scanned = Number(p.scanned || 0)
      progress.compared = Number(p.compared || 0)
      progress.elapsed = Number(p.elapsed_ms || 0)
    })
  } catch (e) {
    logger.warn('[Diff] listen progress failed', { error: String(e?.message || e || '') })
  }
  await setupDragDrop()
  await setupCloseGuard()
  // 编辑模式期间的 Ctrl+S。挂在 window 上捕获阶段：Monaco 自己也吃 keydown，
  // 不抢在它前面的话会被它吞掉。
  window.addEventListener('keydown', onEditKeydown, true)
  // 预填路径的请求（Header 那个"带上次路径开窗"的入口）不自动跑，等用户点"对比"。
  if (store.autoRun && canRun.value && !rows.value.length) void run()
})
onBeforeUnmount(() => {
  window.clearTimeout(filterTimer)
  window.clearTimeout(optTimer)
  window.removeEventListener('keydown', onEditKeydown, true)
  endSplitDrag()
  disposeEditor()
  ro?.disconnect()
  ro = null
  if (typeof unlisten === 'function') unlisten()
  unlisten = null
  if (typeof unlistenDrop === 'function') unlistenDrop()
  unlistenDrop = null
  if (typeof unlistenClose === 'function') unlistenClose()
  unlistenClose = null
  void closeSession()
})

// 新请求进来先清场再跑：只填了一侧的请求（比如工具栏"顺手带上当前文件"）跑不起来，
// 不清的话会把上一轮的结果留在屏幕上，看着像是新对比的结果。
watch(() => store.requestSeq, async (seq) => {
  if (!seq) return
  if (!(await resetResults())) return
  if (store.autoRun) await run()
})

watch(viewMode, () => {
  // 编辑模式下这个开关直接落到 Monaco 的并排/统一上，不用重建结果。
  if (editMode.value) {
    const split = viewMode.value === 'split'
    diffEditor?.updateOptions({ renderSideBySide: split })
    // 行内高亮由面板自己画，只在并排视图成立；换视图时两边一起切。
    liveDiffSync?.setInkEnabled(split)
    editHostRef.value?.classList.toggle('is-live-ink', split && !!liveDiffSync)
    return
  }
  if (!fileResult.value) return
  rows.value = buildFileRows(fileResult.value)
  resetScroll()
})

// 并排/统一之间切换换了滚动容器，视口高度得按新容器重新量一次。
watch(splitPanes, () => {
  nextTick(() => {
    syncViewport()
    measureCharWidth()
  })
})

// 外观主题变了：只读视图靠 CSS 变量自动跟随，编辑模式的 Monaco 得手动重设主题，
// 否则切换主题后两个模式一个亮一个暗。
watch(() => settingsStore.appearanceTheme, () => {
  if (!editMode.value || !monacoMod) return
  monacoMod.editor.setTheme(defineDiffEditTheme(monacoMod))
})

// 树形 / 列表切换：两种视图的数据各自现取，缺哪份补哪份。
watch(dirView, async (v) => {
  try {
    localStorage.setItem(DIR_VIEW_KEY, v)
  } catch {
    // 记不住就下次还是默认树形
  }
  if (!sessionId.value || !showDirAny.value) return
  if (v === 'list' && !dirEntries.value.length) await loadPage(0, false)
  else await refreshDirRows()
  resetScroll()
})

/**
 * 把编辑模式用得上的选项同步给编辑器。只有"忽略空白"有对应项（Monaco 的 ignoreTrimWhitespace），
 * 其余几项是 Rust 侧只读对比的参数，编辑器里没有等价开关。改完让实时同步重算一次，红绿立刻跟上。
 */
const applyEditorOptions = () => {
  if (!editMode.value || !diffEditor) return
  try {
    diffEditor.updateOptions({ ignoreTrimWhitespace: opts.ignoreWhitespace })
  } catch { /* 编辑器可能正在销毁 */ }
  liveDiffSync?.contentChanged()
}

// 文件选项改了就重算（目录选项太贵，必须手动点"对比"）。
watch(
  () => [
    opts.algorithm, opts.contextLines, opts.ignoreWhitespace,
    opts.ignoreTrailingWhitespace, opts.ignoreCase, opts.ignoreBlankLines,
  ].join('|'),
  () => {
    if (!fileResult.value || busy.value) return
    // 编辑模式下不能走 run()：它会经过 ensureEditSafe 把人踢出编辑。这里只把能同步的同步过去，
    // 记一笔"选项变了"，退出编辑时再按新选项重算只读结果。
    if (editMode.value) {
      optsStale = true
      applyEditorOptions()
      return
    }
    window.clearTimeout(optTimer)
    optTimer = window.setTimeout(() => { void run() }, 260)
  },
)

watch(() => props.visible, (v) => {
  if (!v) return
  nextTick(() => {
    syncViewport()
    measureCharWidth()
    if (store.autoRun && canRun.value && !rows.value.length) void run()
  })
})
</script>
<style scoped>
.guaxi-diff-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  /* 拖放蒙层按面板定位 */
  position: relative;
  background: var(--guaxi-panel-bg, #1e1e1e);
  color: var(--guaxi-text-primary, #d4d4d4);
  /*
   * 字体跟主界面同一份：--guaxi-editor-font-family 定义在 App.vue 的 :root，
   * 每个窗口都拿得到。原来写的 --editor-font-family 只存在于主窗口编辑器容器内部，
   * 面板搬进独立窗口后取不到值，直接掉到浏览器兜底字体，中文尤其明显地和主界面不是一套。
   * 字号也一并调大：这页就是拿来读代码的，12.5px 太挤。行高必须跟 JS 里的 ROW_H 对齐。
   */
  --dp-code-font: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --dp-code-size: 16px;
  --dp-row-h: 24px;
}

.dp-head {
  flex: 0 0 auto;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 82%, transparent);
  background: var(--toolbar-bg, var(--guaxi-panel-bg, #252526));
}

.dp-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px;
  min-height: 30px;
  flex-wrap: wrap;
}

.dp-row-options,
.dp-row-status,
.dp-row-filter {
  border-top: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 60%, transparent);
  font-size: 12.5px;
}

/* 主行原来不定字号，文本会按继承下来的主界面字号画，比选项行、状态行都大一号。 */
.dp-row-main {
  font-size: 12.5px;
}

.dp-mode {
  flex: 0 0 auto;
  font-size: 11px;
}

.dp-kind {
  flex: 0 0 auto;
}

.dp-kind :deep(.n-radio-button) {
  font-size: 11px;
  padding: 0 8px;
}

/* 拖放蒙层：左右两半提示会落到哪一侧，光标在哪半就亮哪半 */
.dp-drop-mask {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: stretch;
  gap: 8px;
  padding: 8px;
  pointer-events: none;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #1e1e1e) 72%, transparent);
  backdrop-filter: blur(1px);
}

.dp-drop-half {
  flex: 1 1 50%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 2px dashed color-mix(in srgb, var(--guaxi-border, #3a3d41) 90%, transparent);
  border-radius: 8px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 13px;
}

.dp-drop-half.is-on {
  border-color: var(--guaxi-accent, #4c8bf5);
  color: var(--guaxi-accent, #4c8bf5);
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 12%, transparent);
}

.dp-drop-tip {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 14px;
  text-align: center;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #9da5b4);
}

/* 路径框的外层负责占位：n-tooltip 可能会在中间插一层触发器包裹元素，
   把 flex 尺寸放在自己这层，才不会被那层包裹打断。 */
.dp-slot-wrap {
  display: flex;
  flex: 1 1 160px;
  min-width: 0;
  max-width: 340px;
}

.dp-slot-wrap > * {
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
}

.dp-slot {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  width: 100%;
  padding: 3px 8px;
  border-radius: 5px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 84%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 92%, transparent);
  color: var(--guaxi-text-primary, #d4d4d4);
  font-size: 12px;
  cursor: pointer;
}

.dp-slot:hover {
  border-color: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 60%, transparent);
}

.dp-slot.is-empty {
  color: var(--guaxi-text-secondary, #9da5b4);
  border-style: dashed;
}

.dp-slot-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
  /* 路径用代码字体：和状态行里的 .dp-status-rel 一套，等宽下更容易比对两侧 */
  font-family: var(--dp-code-font);
}

.dp-actions {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.dp-actions :deep(.n-button.is-on) {
  color: var(--guaxi-accent, #4c8bf5);
}

/* 动作一律图标化：方形、等宽高、悬停才出底色，说明交给 tooltip。
   按钮内边距是 naive 用 --n-padding 下的，改这个变量比覆盖 .n-button 的规则稳。 */
.dp-icon-btn {
  --n-padding: 0 5px;
  height: 26px;
  min-width: 26px;
  border-radius: 6px;
  font-size: 12px;
}

.dp-icon-btn.is-run {
  color: var(--guaxi-accent, #4c8bf5);
}

.dp-icon-btn.is-stop {
  color: color-mix(in srgb, #f59e0b 84%, var(--guaxi-text-primary, #d4d4d4));
}

/* 选项按"组"排：一个图标 + 一个小标题 + 若干控件，和主界面工具栏的密度一致。 */
.dp-group {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  /* 极淡的底色把每组圈成一块，行里控件多起来才看得出边界 */
  padding: 3px 8px;
  border-radius: 7px;
  background: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 9%, transparent);
}

.dp-group-icon {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #9da5b4);
  opacity: 0.85;
}

.dp-group-label {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.02em;
}

/*
 * 选项行里的控件统一到 12px。naive 各控件默认字号不一样（单选框 14、输入框 12），
 * 而组标签只有 11px，混着排就会出现"选项比标题还大"。
 * 单选框的文字层类名是 .n-radio__label —— 写成 .n-radio-button__label 是不存在的，之前那条规则一直没生效。
 */
.dp-row-options :deep(.n-radio__label),
.dp-row-options :deep(.n-checkbox__label),
.dp-row-options :deep(.n-base-selection),
.dp-row-options :deep(.n-input__input-el) {
  font-size: 12px;
}

/* 提示里的第二行：说明这一项在另一套模式下不生效，比正文淡一档 */
.dp-tip-note {
  margin-top: 4px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11.5px;
}

/* 算法下拉：收起时只显示算法名，宽度让工具栏不再被三个单选按钮撑开 */
.dp-select-wrap {
  display: inline-flex;
  flex: 0 0 auto;
}

.dp-algo-select {
  width: 116px;
}

/*
 * 下拉的收起文案与菜单项都是 naive 渲染的（菜单还可能被 teleport 到 body，
 * 那样就落不到本组件的 scoped 作用域里），所以这两条用全局选择器，靠 dp-algo-* 圈定范围。
 * 字体变量给两层兜底：--dp-code-font 定义在面板上，teleport 出去就取不到了。
 */
:global(.dp-algo-name),
:global(.dp-algo-opt__name) {
  font-family: var(--dp-code-font, var(--guaxi-editor-font-family, monospace));
  font-size: 12px;
}

:global(.dp-algo-opt) {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

:global(.dp-algo-opt__hint) {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 11px;
}

.dp-help-btn {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-help-btn:hover {
  color: var(--guaxi-accent, #4c8bf5);
}

.dp-sep {
  width: 1px;
  height: 14px;
  background: color-mix(in srgb, var(--guaxi-border, #3a3d41) 80%, transparent);
}

.dp-status-text {
  color: var(--guaxi-text-secondary, #9da5b4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dp-status-text.is-error {
  color: color-mix(in srgb, #ef4444 84%, var(--guaxi-text-primary));
}

.dp-status-text.is-warn {
  color: color-mix(in srgb, #f59e0b 84%, var(--guaxi-text-primary));
}

.dp-status-rel {
  font-family: var(--dp-code-font);
  font-size: 13px;
  color: var(--guaxi-text-primary, #d4d4d4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 42%;
}

.dp-badge {
  font-family: var(--dp-code-font);
  font-size: 13px;
  padding: 0 5px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 70%, transparent);
}

.dp-badge.is-add { color: #4ade80; }
.dp-badge.is-del { color: #f87171; }
.dp-badge.is-mod { color: #60a5fa; }
.dp-badge.is-ren { color: #c084fc; }
.dp-badge.is-type { color: #fbbf24; }

.dp-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  position: relative;
  font-family: var(--dp-code-font);
  font-size: var(--dp-code-size);
  /* 上下键导航要能聚焦，但焦点框不能有：编辑器里也没有，当前行那条高亮就是焦点提示 */
  outline: none;
}

/* 并排视图：滚动交给两个面板自己，外层不再滚 */
.dp-body.is-panes {
  overflow: hidden;
}

.dp-panes {
  display: flex;
  align-items: stretch;
  height: 100%;
  min-height: 0;
}

.dp-panes.is-dragging {
  cursor: col-resize;
  user-select: none;
}

.dp-pane {
  min-width: 0;
  overflow: auto;
  position: relative;
}

/* 左侧宽度由拖动比例定死，右侧吃掉剩下的。
   min-width:0 + flex-basis:0 是关键：不然右侧会按内容宽度撑爆容器。 */
.dp-pane.is-left {
  flex: 0 0 auto;
}

.dp-pane.is-right {
  flex: 1 1 0;
}

/* 两侧纵向是同步的，左边那根纵向滚动条纯属重复；横向的留着，各侧自己滚 */
.dp-pane.is-left::-webkit-scrollbar:vertical {
  width: 0;
  height: 0;
}

.dp-divider {
  flex: 0 0 6px;
  cursor: col-resize;
  background: color-mix(in srgb, var(--guaxi-border, #3a3d41) 70%, transparent);
  transition: background 0.12s ease;
}

.dp-divider:hover,
.dp-panes.is-dragging .dp-divider {
  background: var(--guaxi-accent, #4c8bf5);
}

.dp-spacer {
  position: relative;
  min-width: 100%;
}

.dp-window {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  will-change: transform;
}

.dp-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 100%;
  min-height: 120px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12.5px;
  padding: 0 16px;
  text-align: center;
}

.dp-placeholder.is-error {
  color: color-mix(in srgb, #ef4444 84%, var(--guaxi-text-primary));
}

.dp-line {
  display: flex;
  align-items: stretch;
  height: var(--dp-row-h);
  line-height: var(--dp-row-h);
  white-space: pre;
}

.dp-line.is-hunk {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 12%, transparent);
  color: var(--guaxi-text-secondary, #9da5b4);
}

/* 当前行：底色跟编辑模式 Monaco 的 editor.lineHighlightBackground 是同一份（次要色 10%），
   左边再压一条强调色短线。点一下或者用上下键都会挪到这里，两个模式里的位置也是同一条。 */
.dp-line.is-current {
  background: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 10%, transparent);
  box-shadow: inset 2px 0 0 var(--guaxi-accent, #4c8bf5);
}

/* 悬停只做提示：这一行点了就会变成当前行。hunk 头和已经是当前行的行不用再提示。 */
.dp-line:not(.is-hunk):not(.is-current):hover {
  background: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 5%, transparent);
}

.dp-hunk {
  padding-left: 10px;
  font-size: 13px;
}

.dp-no {
  flex: 0 0 52px;
  width: 52px;
  padding-right: 8px;
  text-align: right;
  color: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 76%, transparent);
  font-size: 13px;
  user-select: none;
}

.dp-sign {
  flex: 0 0 16px;
  width: 16px;
  text-align: center;
  user-select: none;
}

.dp-cell {
  display: inline-block;
  flex: 0 0 auto;
  overflow: hidden;
  padding: 0 4px;
}

/* 并排视图里不给单元格定宽：横向由所在面板的滚动区兜着，
   一侧长行只让这一侧出滚动条，另一侧照旧顶格显示 */
.dp-cell.is-fluid {
  flex: 1 0 auto;
  overflow: visible;
}

.dp-cell.is-del {
  background: color-mix(in srgb, #f87171 16%, transparent);
}

.dp-cell.is-add {
  background: color-mix(in srgb, #4ade80 16%, transparent);
}

.dp-cell.is-blank {
  background: color-mix(in srgb, var(--guaxi-text-secondary, #9da5b4) 8%, transparent);
}

.dp-ink {
  border-radius: 2px;
  background: color-mix(in srgb, #fbbf24 34%, transparent);
}

.dp-cell.is-del .dp-ink {
  background: color-mix(in srgb, #f87171 40%, transparent);
}

.dp-cell.is-add .dp-ink {
  background: color-mix(in srgb, #4ade80 40%, transparent);
}

.dp-entry {
  display: flex;
  align-items: center;
  gap: 6px;
  height: var(--dp-row-h);
  line-height: var(--dp-row-h);
  padding: 0 10px;
  cursor: pointer;
  font-size: 13.5px;
}

.dp-entry:hover {
  background: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 14%, transparent);
}

.dp-entry.is-dir-entry {
  cursor: default;
}

/* 树里的目录是可以点开的，光标得给出可点的样子 */
.dp-tree-row.is-dir-entry {
  cursor: pointer;
}

.dp-tree-row.is-tip {
  cursor: default;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-tree-row.is-tip:hover {
  background: transparent;
}

.dp-twist {
  flex: 0 0 13px;
  width: 13px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #9da5b4);
  transition: transform 0.12s ease;
}

.dp-twist.is-open {
  transform: rotate(90deg);
}

.dp-twist-wait {
  font-weight: 700;
  opacity: 0.7;
}

.dp-entry.is-mixed .dp-entry-mark {
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-entry-mark {
  flex: 0 0 12px;
  text-align: center;
  font-weight: 600;
}

.dp-entry.is-added .dp-entry-mark { color: #4ade80; }
.dp-entry.is-removed .dp-entry-mark { color: #f87171; }
.dp-entry.is-modified .dp-entry-mark { color: #60a5fa; }
.dp-entry.is-renamed .dp-entry-mark { color: #c084fc; }
.dp-entry.is-type_changed .dp-entry-mark { color: #fbbf24; }
.dp-entry.is-same { color: var(--guaxi-text-secondary, #9da5b4); }

.dp-entry-icon {
  flex: 0 0 auto;
  opacity: 0.72;
}

.dp-entry-rel {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dp-entry-extra {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 28%;
}

.dp-entry-size {
  margin-left: auto;
  flex: 0 0 auto;
  padding-left: 10px;
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: 12px;
}

.dp-back {
  flex: 0 0 auto;
}

/* ───── 编辑模式 ───── */

/* 只读/编辑开关：和 .dp-kind 同一套观感，图标化之后并进主行最右边那组动作里 */
.dp-edit-wrap {
  display: inline-flex;
  flex: 0 0 auto;
}

/* 按钮里只剩一个 14px 图标，用 inline-flex 把它在 28px 高的分段按钮里摆正 */
.dp-edit-toggle :deep(.n-radio-button) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  padding: 0 7px;
}

/* svg 默认坐在基线上，按钮组那一行 line-height 是整段高度，会把它顶偏；转成块级再交给 flex 居中 */
.dp-edit-toggle :deep(.n-radio-button svg) {
  display: block;
}

/* Monaco 容器：撑满 .dp-body 原来的位置，横向纵向都由它自己滚 */
.dp-edit-body {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 必须和 .dp-body 声明同一套代码字体：Monaco 的 fontFamily / fontSize
     就是从这层量的（见 mountEditor），不声明就会退到主界面字体，两个模式一眼就能看出差别。 */
  font-family: var(--dp-code-font);
  font-size: var(--dp-code-size);
}

.dp-edit-host {
  flex: 1 1 auto;
  min-height: 0;
}

/* 行号跟只读视图的 .dp-no 对齐：同一字号、右对齐、同样留 8px。
   Monaco 没有"行号单独设字号"的选项，只能覆盖它的行号样式。 */
.dp-edit-host :deep(.monaco-editor .line-numbers) {
  font-size: 13px;
  padding-right: 8px;
  text-align: right;
}

/* 当前行左边那条强调色短线：位置和粗细对齐只读视图的 .dp-line.is-current。
   底色由主题的 editor.lineHighlightBackground 给，这里只补左边那一条。
   必须挂在 .current-line-margin（行号栏那层，起点就是编辑器最左边）上；
   代码区那层 .current-line 从行号栏之后开始，加在上面会变成代码前面多一条。 */
.dp-edit-host :deep(.monaco-editor .margin-view-overlays .current-line-margin) {
  border-left: 2px solid var(--guaxi-accent, #4c8bf5);
}

/* 行内高亮由面板自己画：取值就是只读视图的 .dp-ink —— 40% 叠在 16% 的整行底色上，圆角 2px。
   下面那条只遮 background-color，因为 Monaco 给"纯增/纯删"的整行还会额外叠一层整行的 40%
   （diffWholeLine*Decoration），只读视图里那种行只有 16% 的底色，两边一对比深了三倍；
   diff-range-empty 那根 3px 竖条走的是 border-left，留着当插入点标记。
   .is-live-ink 只在实时同步装上、且当前是并排视图时才挂：没装上就让 Monaco 原样显示，
   统一视图里"删掉的那几行"在 view zone 里渲染，模型装饰落不进去，也交回 Monaco。 */
.dp-edit-host.is-live-ink :deep(.monaco-editor .char-insert),
.dp-edit-host.is-live-ink :deep(.monaco-editor .char-delete) {
  background-color: transparent;
}

.dp-edit-host :deep(.dp-edit-ink-add) {
  background: color-mix(in srgb, #4ade80 40%, transparent);
  border-radius: 2px;
}

.dp-edit-host :deep(.dp-edit-ink-del) {
  background: color-mix(in srgb, #f87171 40%, transparent);
  border-radius: 2px;
}

.dp-placeholder.is-overlay {
  position: absolute;
  inset: 0;
  background: color-mix(in srgb, var(--guaxi-panel-bg, #1e1e1e) 88%, transparent);
}

/* 两侧之间那道缝对齐只读视图的 .dp-divider：默认的 4px 且只在悬停时显形，改成 6px 常显，
   鼠标按上去再变成强调色。宽高走 Monaco 自己的变量，比直接改 width 稳。 */
.dp-edit-host :deep(.monaco-sash.vertical) {
  --vscode-sash-size: 6px;
  --vscode-sash-hover-size: 6px;
}

.dp-edit-host :deep(.monaco-sash.vertical:before) {
  background: color-mix(in srgb, var(--guaxi-border, #3a3d41) 70%, transparent);
}

.dp-edit-host :deep(.monaco-sash.vertical.hover:before),
.dp-edit-host :deep(.monaco-sash.vertical.active:before) {
  background: var(--guaxi-accent, #4c8bf5);
}

/* ───── 算法说明 ───── */
/* 说明弹窗跟编辑区同一套字：同一族等宽字体、同一字号（--dp-code-font / --dp-code-size）。
   弹窗是盖在代码上的，字体不跟着走就像贴上去的另一套界面。层级只留标题比正文大一档。 */
.dp-help-card {
  /* 弹窗是 teleport 到 body 的，.guaxi-diff-panel 上那两条 --dp-code-* 传不进来，这里照抄一份。
     下面这两行必须和 .guaxi-diff-panel 里的声明保持一致，改一处就得改两处。 */
  --dp-code-font: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --dp-code-size: 16px;
}

.dp-help-card :deep(.n-card-header__main) {
  font-family: var(--dp-code-font);
  font-size: calc(var(--dp-code-size) + 1px);
  font-weight: 600;
}

.dp-help-card :deep(.n-card__content) {
  font-family: var(--dp-code-font);
  font-size: var(--dp-code-size);
  /* 字号跟上来以后弹窗高了一截：窗口被拉矮时让正文自己滚，别把标题顶出可视区 */
  max-height: calc(100vh - 200px);
  overflow: auto;
}

.dp-help-lead {
  margin: 0 0 12px;
  font-size: var(--dp-code-size);
  line-height: 1.75;
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-help-item {
  padding: 10px 12px;
  margin-bottom: 10px;
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #3a3d41) 70%, transparent);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #2d2d30) 60%, transparent);
}

.dp-help-item.is-current {
  border-color: color-mix(in srgb, var(--guaxi-accent, #4c8bf5) 60%, transparent);
}

.dp-help-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

/* 头里那两种控件也是同一族字：字号比正文小一档，别再混进别的字体族 */
.dp-help-head :deep(.n-tag),
.dp-help-head :deep(.n-button) {
  font-family: var(--dp-code-font);
  font-size: calc(var(--dp-code-size) - 3px);
}

.dp-help-name {
  font-family: var(--dp-code-font);
  font-size: var(--dp-code-size);
  font-weight: 600;
  color: var(--guaxi-text-primary, #d4d4d4);
}

.dp-help-brief {
  flex: 1 1 auto;
  min-width: 0;
  font-size: calc(var(--dp-code-size) - 0.5px);
  color: var(--guaxi-text-secondary, #9da5b4);
}

.dp-help-body {
  display: grid;
  grid-template-columns: 40px 1fr;
  gap: 4px 10px;
  margin: 0;
  font-size: var(--dp-code-size);
  line-height: 1.7;
}

.dp-help-body dt {
  color: var(--guaxi-text-secondary, #9da5b4);
  font-size: calc(var(--dp-code-size) - 1px);
}

.dp-help-body dd {
  margin: 0;
  color: var(--guaxi-text-primary, #d4d4d4);
}
</style>

