<template>
  <div class="rt-root">
    <n-tabs v-model:value="activeTab" type="line" class="rt-tabs" pane-class="rt-pane" display-directive="show:lazy" animated>
      <n-tab-pane name="log">
        <template #tab><span class="tw-tab"><Icon :icon="fileSearchIcon" width="16" height="16" />日志分析</span></template>
        <div class="rt-log">
          <div class="rt-toolbar">
            <n-button type="primary" size="small" :loading="importing" @click="onImport">
              <template #icon><Icon :icon="importIcon" width="15" height="15" /></template>
              导入日志文件
            </n-button>
            <n-select
              v-if="meta"
              v-model:value="selectedProfileId"
              :options="profileSelectOptions"
              size="small"
              style="width:250px"
              @update:value="onSelectProfile"
            />
            <n-button v-if="meta" size="small" @click="showRuleEditor = true">
              <template #icon><Icon :icon="settingsIcon" width="14" height="14" /></template>
              解析规则
            </n-button>
            <span v-if="meta" class="rt-meta">
              <span class="rt-chip"><Icon :icon="fileIcon" width="13" height="13" />{{ shortPath }}</span>
              <span class="rt-chip">{{ formatBytes(meta.byte_size) }}</span>
              <span class="rt-chip">{{ meta.line_count.toLocaleString() }} 行</span>
              <span class="rt-chip">{{ meta.frame_count.toLocaleString() }} 快照</span>
            </span>
            <span v-if="error" class="rt-error">{{ error }}</span>
          </div>

          <div v-if="meta" class="rt-inner">
            <n-tabs v-model:value="innerTab" type="segment" size="small" class="rt-inner-tabs" pane-class="rt-inner-pane">
              <!-- 原文 + 时间线 -->
              <n-tab-pane name="raw" tab="原文">
                <div class="rt-body">
                  <div class="rt-timeline" :style="{ width: timelineWidth + 'px' }">
                    <div class="rt-timeline-title">
                      <span>快照时间线</span>
                      <span class="rt-fontctl">
                        <button class="rt-fbtn" @click="setLogFontSize(logFontSize - 1)"><Icon :icon="minusIcon" width="12" height="12" /></button>
                        <span class="rt-fsz">{{ logFontSize }}</span>
                        <button class="rt-fbtn" @click="setLogFontSize(logFontSize + 1)"><Icon :icon="plusIcon" width="12" height="12" /></button>
                      </span>
                    </div>
                    <div class="rt-timeline-list">
                      <div
                        v-for="f in frames"
                        :key="f.index"
                        class="rt-frame"
                        :class="{ active: f.index === activeFrame }"
                        @click="jumpToFrame(f)"
                      >
                        <span class="rt-frame-idx">#{{ f.index }}</span>
                        <span class="rt-frame-ts">{{ f.timestamp_raw || '(无时间戳)' }}</span>
                      </div>
                    </div>
                  </div>
                  <div class="rt-tl-resize" @mousedown="onTimelineResizeDown"></div>
                  <div ref="viewportRef" class="rt-viewport" :style="{ fontSize: logFontSize + 'px', lineHeight: LINE_HEIGHT + 'px' }" @scroll="onScroll">
                    <div class="rt-spacer" :style="{ height: totalHeight + 'px' }">
                      <div
                        v-for="row in visibleRows"
                        :key="row.index"
                        class="rt-line"
                        :style="{ top: row.index * LINE_HEIGHT + 'px', height: LINE_HEIGHT + 'px' }"
                      >
                        <span class="rt-lineno">{{ row.index + 1 }}</span>
                        <span class="rt-linetext">{{ row.text }}</span>
                      </div>
                    </div>
                  </div>
                </div>
              </n-tab-pane>

              <!-- 泄漏排名 -->
              <n-tab-pane name="leak" tab="泄漏排名">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <n-button size="small" type="primary" :loading="leakLoading" @click="runLeak">
                      计算泄漏排名
                    </n-button>
                    <n-button size="small" :disabled="!leakRows.length" @click="sendLeak">→ 编辑器</n-button>
                    <span class="rt-hint">按“持续上涨”给指标打分，最可疑的排最前。点行看曲线。</span>
                  </div>
                  <div class="rt-split">
                    <div class="rt-table-wrap" :style="{ flex: '0 0 ' + splitPct }">
                      <table class="rt-table">
                        <thead>
                          <tr><th>指标</th><th>首值</th><th>末值</th><th>增量</th><th>斜率</th><th>单调%</th><th>样本</th></tr>
                        </thead>
                        <tbody>
                          <tr
                            v-for="r in leakRows"
                            :key="r.key"
                            :class="{ active: r.key === selectedKey }"
                            @click="selectMetric(r.key)"
                          >
                            <td class="rt-key" :title="r.key">{{ r.key }}</td>
                            <td>{{ fmtNum(r.first) }}</td>
                            <td>{{ fmtNum(r.last) }}</td>
                            <td class="rt-delta">+{{ fmtNum(r.delta) }}</td>
                            <td>{{ r.slope.toFixed(2) }}</td>
                            <td>{{ (r.monotonic_ratio * 100).toFixed(0) }}</td>
                            <td>{{ r.samples }}</td>
                          </tr>
                          <tr v-if="!leakRows.length"><td colspan="7" class="rt-empty-cell">尚无结果，点“计算泄漏排名”。</td></tr>
                        </tbody>
                      </table>
                    </div>
                    <div class="rt-split-resize" @mousedown="onSplitResizeDown"></div>
                    <div class="rt-chart-wrap">
                      <div class="rt-chart-head" v-if="selectedKey">
                        <span :title="selectedKey">{{ selectedKey }}</span>
                        <n-button size="tiny" @click="pinMetric(selectedKey)">加入关注</n-button>
                        <n-button size="tiny" @click="sendSeriesCsv">→ 编辑器</n-button>
                        <n-button size="tiny" quaternary @click="chartFullscreen = true" title="全屏"><Icon :icon="expandIcon" width="14" height="14" /></n-button>
                      </div>
                      <VChart v-if="chartOption" class="rt-chart" :option="chartOption" autoresize />
                      <div v-else class="rt-chart-empty">选择一个指标查看时间序列</div>
                    </div>
                  </div>
                </div>
              </n-tab-pane>

              <!-- 指标浏览：点选即出时序 -->
              <n-tab-pane name="browse" tab="指标浏览">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <span class="rt-hint">帧</span>
                    <n-input-number v-model:value="browseFrame" size="small" :min="0" :max="meta.frame_count - 1" style="width:120px" />
                    <n-button size="small" :loading="browseLoading" @click="loadFrameMetrics">加载该帧指标</n-button>
                    <n-input v-model:value="browseKeyword" size="small" placeholder="搜索指标关键字" clearable style="width:240px" />
                    <span class="rt-hint">{{ filteredMetrics.length }} / {{ browseMetrics.length }}</span>
                  </div>
                  <div class="rt-split">
                    <div class="rt-table-wrap" :style="{ flex: '0 0 ' + splitPct }">
                      <table class="rt-table">
                        <thead><tr><th>指标</th><th>当前值</th></tr></thead>
                        <tbody>
                          <tr
                            v-for="m in filteredMetrics.slice(0, 500)"
                            :key="m.key"
                            :class="{ active: m.key === selectedKey }"
                            @click="selectMetric(m.key)"
                          >
                            <td class="rt-key" :title="m.key">{{ m.key }}</td>
                            <td>{{ fmtNum(m.value) }}</td>
                          </tr>
                          <tr v-if="!browseMetrics.length"><td colspan="2" class="rt-empty-cell">先“加载该帧指标”。</td></tr>
                        </tbody>
                      </table>
                    </div>
                    <div class="rt-split-resize" @mousedown="onSplitResizeDown"></div>
                    <div class="rt-chart-wrap">
                      <div class="rt-chart-head" v-if="selectedKey">
                        <span :title="selectedKey">{{ selectedKey }}</span>
                        <n-button size="tiny" @click="pinMetric(selectedKey)">加入关注</n-button>
                        <n-button size="tiny" @click="sendSeriesCsv">→ 编辑器</n-button>
                        <n-button size="tiny" quaternary @click="chartFullscreen = true" title="全屏"><Icon :icon="expandIcon" width="14" height="14" /></n-button>
                      </div>
                      <VChart v-if="chartOption" class="rt-chart" :option="chartOption" autoresize />
                      <div v-else class="rt-chart-empty">点左侧指标查看时间序列</div>
                    </div>
                  </div>
                </div>
              </n-tab-pane>

              <!-- 快照对比 -->
              <n-tab-pane name="diff" tab="快照对比">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <span class="rt-hint">A</span>
                    <n-input-number v-model:value="diffA" size="small" :min="0" :max="meta.frame_count - 1" style="width:110px" />
                    <span class="rt-hint">B</span>
                    <n-input-number v-model:value="diffB" size="small" :min="0" :max="meta.frame_count - 1" style="width:110px" />
                    <n-button size="small" :loading="diffLoading" @click="runDiff">对比</n-button>
                    <span class="rt-hint">变化最大的指标排在前面。</span>
                  </div>
                  <div class="rt-table-wrap full">
                    <table class="rt-table">
                      <thead><tr><th>指标</th><th>A</th><th>B</th><th>变化</th><th>变化%</th></tr></thead>
                      <tbody>
                        <tr v-for="d in diffRows" :key="d.key">
                          <td class="rt-key" :title="d.key">{{ d.key }}</td>
                          <td>{{ fmtNum(d.a) }}</td>
                          <td>{{ fmtNum(d.b) }}</td>
                          <td :class="d.delta >= 0 ? 'rt-delta' : 'rt-delta-neg'">{{ d.delta >= 0 ? '+' : '' }}{{ fmtNum(d.delta) }}</td>
                          <td>{{ isFinite(d.pct) ? d.pct.toFixed(1) + '%' : '—' }}</td>
                        </tr>
                        <tr v-if="!diffRows.length"><td colspan="5" class="rt-empty-cell">选择两帧后点“对比”。</td></tr>
                      </tbody>
                    </table>
                  </div>
                </div>
              </n-tab-pane>

              <!-- 关注指标 / Profile -->
              <n-tab-pane name="profile" tab="关注指标">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <n-button size="small" :disabled="!pinnedKeys.length" @click="saveProfile">保存 Profile</n-button>
                    <n-button size="small" @click="loadProfile">加载 Profile</n-button>
                    <span class="rt-hint">{{ pinnedKeys.length }} 个关注指标。加载 Profile 后可在别的 dump 上复用。</span>
                  </div>
                  <div class="rt-table-wrap full">
                    <table class="rt-table">
                      <thead><tr><th>关注指标</th><th></th></tr></thead>
                      <tbody>
                        <tr v-for="k in pinnedKeys" :key="k">
                          <td class="rt-key" :title="k" @click="selectMetric(k)" style="cursor:pointer">{{ k }}</td>
                          <td><n-button size="tiny" text @click="unpinMetric(k)">移除</n-button></td>
                        </tr>
                        <tr v-if="!pinnedKeys.length"><td colspan="2" class="rt-empty-cell">在泄漏排名/指标浏览里“加入关注”。</td></tr>
                      </tbody>
                    </table>
                  </div>
                  <div class="rt-chart-wrap" style="height:300px">
                    <VChart v-if="chartOption" class="rt-chart" :option="chartOption" autoresize />
                  </div>
                </div>
              </n-tab-pane>

              <n-tab-pane name="search" tab="检索/聚类">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <n-input v-model:value="searchQuery" size="small" placeholder="检索关键字或正则" clearable style="width:280px" @keyup.enter="runSearch" />
                    <n-checkbox v-model:checked="searchRegex" size="small">正则</n-checkbox>
                    <n-checkbox v-model:checked="searchIgnoreCase" size="small">忽略大小写</n-checkbox>
                    <n-button size="small" type="primary" :loading="searchLoading" @click="runSearch">检索</n-button>
                    <n-button size="small" :loading="tplLoading" @click="runCluster">模板聚类</n-button>
                    <n-button size="small" :loading="fmtLoading" @click="runDetectFormat">识别格式</n-button>
                    <span v-if="formatReport" class="rt-hint">
                      猜测: {{ formatReport.guess }} (JSON {{ (formatReport.json_ratio*100).toFixed(0) }}% / syslog {{ (formatReport.syslog_ratio*100).toFixed(0) }}% / 时间戳 {{ (formatReport.timestamp_ratio*100).toFixed(0) }}%)
                    </span>
                  </div>
                  <div class="rt-table-wrap full" v-if="templates.length">
                    <table class="rt-table">
                      <thead><tr><th>次数</th><th>模板</th><th>样例</th></tr></thead>
                      <tbody>
                        <tr v-for="t in templates" :key="t.template">
                          <td>{{ t.count.toLocaleString() }}</td>
                          <td class="rt-key" :title="t.template">{{ t.template }}</td>
                          <td class="rt-key" :title="t.sample">{{ t.sample }}</td>
                        </tr>
                      </tbody>
                    </table>
                  </div>
                  <div class="rt-table-wrap full" v-else>
                    <table class="rt-table">
                      <thead><tr><th>行</th><th>快照</th><th>内容</th></tr></thead>
                      <tbody>
                        <tr v-for="h in searchHits" :key="h.line" @click="gotoLine(h.line)">
                          <td>{{ h.line + 1 }}</td>
                          <td>{{ h.frame != null ? '#' + h.frame : '—' }}</td>
                          <td class="rt-key" :title="h.text">{{ h.text }}</td>
                        </tr>
                        <tr v-if="!searchHits.length"><td colspan="3" class="rt-empty-cell">输入关键字检索，或点“模板聚类”把海量日志归并成模板。</td></tr>
                      </tbody>
                    </table>
                  </div>
                </div>
              </n-tab-pane>

              <n-tab-pane name="extract" tab="抽取/聚合">
                <div class="rt-analysis">
                  <div class="rt-actions">
                    <n-input v-model:value="dsl" size="small" placeholder="管道查询: filter ERROR | parse cost=(?P<ms>\d+)ms | stats avg(ms) by mod" style="flex:1;min-width:320px" @keyup.enter="runQuery" />
                    <n-button size="small" type="primary" :loading="aggLoading || extractLoading || searchLoading" @click="runQuery">运行</n-button>
                  </div>
                  <div class="rt-actions">
                    <n-input v-model:value="extractPattern" size="small" placeholder="抽取正则(命名组)，如 cost=(?P<ms>\d+)ms" style="width:340px" />
                    <n-button size="small" type="primary" :loading="extractLoading" @click="runExtract">抽取字段</n-button>
                    <n-button size="small" :disabled="!extractRows.length" @click="sendExtract">→ 编辑器</n-button>
                    <span class="rt-hint">{{ extractRows.length }} 行</span>
                  </div>
                  <div class="rt-actions">
                    <n-input v-model:value="aggGroup" size="small" placeholder="分组组名(命名组)" style="width:150px" />
                    <n-input v-model:value="aggValue" size="small" placeholder="数值组名(命名组)" style="width:150px" />
                    <n-select v-model:value="aggFunc" size="small" :options="aggFuncOptions" style="width:110px" />
                    <n-button size="small" :loading="aggLoading" @click="runAggregate">聚合</n-button>
                    <span class="rt-hint">用上面的抽取正则做 parse，按分组组聚合数值组</span>
                  </div>
                  <div class="rt-table-wrap full">
                    <table class="rt-table">
                      <thead><tr><th v-for="c in extractColumns" :key="c">{{ c }}</th></tr></thead>
                      <tbody>
                        <tr v-for="(r, ri) in extractRows.slice(0, 500)" :key="ri">
                          <td v-for="(v, ci) in r" :key="ci" class="rt-key" :title="v">{{ v }}</td>
                        </tr>
                        <tr v-if="!extractRows.length"><td :colspan="Math.max(1, extractColumns.length)" class="rt-empty-cell">用命名组正则抽取字段，或按分组聚合。</td></tr>
                      </tbody>
                    </table>
                  </div>
                </div>
              </n-tab-pane>
            </n-tabs>
          </div>

          <div v-else class="rt-empty">
            导入一份日志文件（如 monitor.out0）开始分析。大文件按需加载，百万行也不卡。
          </div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="terminal">
        <template #tab><span class="tw-tab"><Icon :icon="terminalIcon" width="16" height="16" />实时终端</span></template>
        <RemoteSshTerminal @open-in-analysis="onReplay" />
      </n-tab-pane>
      <n-tab-pane name="gdb">
        <template #tab><span class="tw-tab"><Icon :icon="bugIcon" width="16" height="16" />GDB 调试</span></template>
        <RemoteGdbDebugger />
      </n-tab-pane>
      <n-tab-pane name="toolbox">
        <template #tab><span class="tw-tab"><Icon :icon="wrenchIcon" width="16" height="16" />工具箱</span></template>
        <RemoteToolbox @import-file="onReplay" />
      </n-tab-pane>
      <n-tab-pane name="network">
        <template #tab><span class="tw-tab"><Icon :icon="networkIcon" width="16" height="16" />网络</span></template>
        <RemoteNetworkToolbox />
      </n-tab-pane>
    </n-tabs>

    <n-modal v-model:show="showRuleEditor" :mask-closable="false">
      <div class="rt-editor">
        <div class="rt-editor-head">
          <span>解析规则编辑器</span>
          <Icon :icon="xIcon" width="18" height="18" class="rt-editor-close" @click="showRuleEditor = false" />
        </div>
        <div class="rt-editor-body">
          <div class="rt-ed-row">
            <label>名称</label>
            <n-input v-model:value="editProfile.name" size="small" placeholder="规则名称" />
          </div>
          <div class="rt-ed-row">
            <label>模式</label>
            <n-radio-group v-model:value="editProfile.mode" size="small">
              <n-radio-button value="snapshot">快照分帧 (****时间戳****)</n-radio-button>
              <n-radio-button value="line">逐行 (syslog/kernel/任意)</n-radio-button>
            </n-radio-group>
          </div>
          <template v-if="editProfile.mode === 'line'">
            <div class="rt-ed-row">
              <label>时间戳正则</label>
              <n-input v-model:value="editProfile.timestamp_regex" size="small" placeholder="命名组 ts，如 (?P<ts>\d{4}-\d{2}-\d{2}[ T][\d:]+)" />
            </div>
            <div class="rt-ed-row">
              <label>时间格式</label>
              <n-input v-model:value="tsFormatsText" size="small" placeholder="逗号分隔，如 %Y-%m-%d %H:%M:%S" />
            </div>
          </template>
          <div class="rt-ed-rules">
            <div class="rt-ed-rules-head">
              <span>抽取规则（命名组名 = 指标名）</span>
              <n-button size="tiny" @click="addRule"><template #icon><Icon :icon="plusIcon" width="13" height="13" /></template>加规则</n-button>
            </div>
            <div v-for="(r, i) in editProfile.rules" :key="i" class="rt-ed-rule">
              <n-input v-model:value="r.name" size="small" placeholder="分组前缀(可空)" style="width:130px" />
              <n-input v-model:value="r.pattern" size="small" placeholder="正则，如 MemFree:\s+(?P<MemFree>\d+)" style="flex:1" />
              <n-button size="tiny" text @click="editProfile.rules.splice(i, 1)"><Icon :icon="trashIcon" width="14" height="14" /></n-button>
            </div>
            <div v-if="!editProfile.rules.length" class="rt-hint">加一条规则：正则里用 (?P&lt;名字&gt;\d+) 命名组捕获数值，名字就是图表里的指标名。</div>
          </div>
          <span v-if="editError" class="rt-error">{{ editError }}</span>
        </div>
        <div class="rt-editor-foot">
          <n-button size="small" @click="loadParseProfile">导入规则</n-button>
          <n-button size="small" @click="saveParseProfile">导出规则</n-button>
          <n-button size="small" type="primary" :loading="reparsing" @click="applyReparse">应用并重解析</n-button>
        </div>
      </div>
    </n-modal>

    <n-modal v-model:show="chartFullscreen">
      <div class="rt-chart-full">
        <div class="rt-chart-full-head">
          <span :title="selectedKey">{{ selectedKey || '时间序列' }}</span>
          <Icon :icon="xIcon" width="20" height="20" class="rt-editor-close" @click="chartFullscreen = false" />
        </div>
        <VChart v-if="chartOption" class="rt-chart-full-body" :option="chartOption" autoresize />
        <div v-else class="rt-chart-empty">无数据</div>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { ref, computed, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, DataZoomComponent, TitleComponent } from 'echarts/components'
import VChart from 'vue-echarts'
import RemoteSshTerminal from './RemoteSshTerminal.vue'
import RemoteGdbDebugger from './RemoteGdbDebugger.vue'
import RemoteToolbox from './RemoteToolbox.vue'
import RemoteNetworkToolbox from './RemoteNetworkToolbox.vue'
import { Icon } from '@iconify/vue'
import fileSearchIcon from '@iconify-icons/lucide/file-search'
import terminalIcon from '@iconify-icons/lucide/terminal'
import bugIcon from '@iconify-icons/lucide/bug'
import wrenchIcon from '@iconify-icons/lucide/wrench'
import networkIcon from '@iconify-icons/lucide/network'
import importIcon from '@iconify-icons/lucide/import'
import fileIcon from '@iconify-icons/lucide/file'
import settingsIcon from '@iconify-icons/lucide/settings-2'
import plusIcon from '@iconify-icons/lucide/plus'
import minusIcon from '@iconify-icons/lucide/minus'
import expandIcon from '@iconify-icons/lucide/maximize-2'
import trashIcon from '@iconify-icons/lucide/trash-2'
import xIcon from '@iconify-icons/lucide/x'
import { createLogger } from '../../../utils/logger'

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent, DataZoomComponent, TitleComponent])

const logger = createLogger('RemoteTerminalApp')

const logFontSize = ref(14)
const LINE_HEIGHT = computed(() => logFontSize.value + 7)
function setLogFontSize(v) { logFontSize.value = Math.min(28, Math.max(9, v)) }
const PAGE = 500
const OVERSCAN = 20

const activeTab = ref('log')
const innerTab = ref('raw')
const importing = ref(false)
const error = ref('')
const meta = ref(null)
const frames = ref([])
const sessionId = ref('')

// 原文虚拟滚动
const viewportRef = ref(null)
const scrollTop = ref(0)
const viewportHeight = ref(600)
const activeFrame = ref(-1)
const timelineWidth = ref(300)
const splitRatio = ref(0.55)
const splitPct = computed(() => (splitRatio.value * 100).toFixed(1) + '%')
const chartFullscreen = ref(false)
function onSplitResizeDown(e) {
  const container = e.currentTarget.parentElement
  if (!container) return
  const rect = container.getBoundingClientRect()
  let resizing = true
  const move = ev => {
    if (!resizing) return
    const r = (ev.clientX - rect.left) / rect.width
    splitRatio.value = Math.min(0.85, Math.max(0.2, r))
  }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move)
  document.addEventListener('mouseup', up)
}
function onTimelineResizeDown(e) {
  const startX = e.clientX
  const startW = timelineWidth.value
  let resizing = true
  const move = ev => { if (resizing) timelineWidth.value = Math.min(520, Math.max(200, startW + ev.clientX - startX)) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move)
  document.addEventListener('mouseup', up)
}
const pageCache = new Map()
const pendingPages = new Set()

// 分析
const leakLoading = ref(false)
const leakRows = ref([])
const browseFrame = ref(0)
const browseKeyword = ref('')
const browseMetrics = ref([])
const browseLoading = ref(false)
const diffA = ref(0)
const diffB = ref(0)
const diffRows = ref([])
const diffLoading = ref(false)
const selectedKey = ref('')
const seriesPoints = ref([])
const pinnedKeys = ref([])

// E1/D1/D2/D3/E3 检索与杂乱日志
const searchQuery = ref('')
const searchRegex = ref(false)
const searchIgnoreCase = ref(true)
const searchHits = ref([])
const searchLoading = ref(false)
const templates = ref([])
const tplLoading = ref(false)
const formatReport = ref(null)
const fmtLoading = ref(false)
const extractPattern = ref('')
const extractColumns = ref([])
const extractRows = ref([])
const extractLoading = ref(false)
const aggGroup = ref('')
const aggValue = ref('')
const aggFunc = ref('count')
const aggLoading = ref(false)
const dsl = ref('')

// 解析 Profile / 自定义规则
const builtinProfiles = ref([])
const selectedProfileId = ref('snapshot')
const showRuleEditor = ref(false)
const reparsing = ref(false)
const editError = ref('')
const editProfile = ref({ id: '', name: '', builtin: false, mode: 'snapshot', timestamp_regex: '', timestamp_formats: [], rules: [] })
const tsFormatsText = ref('')
const profileSelectOptions = computed(() =>
  builtinProfiles.value.map(p => ({ label: p.name, value: p.id })))

async function loadBuiltinProfiles() {
  try { builtinProfiles.value = await invoke('log_builtin_profiles') } catch (e) { logger.error('builtin profiles', e) }
}
function onSelectProfile(id) {
  const p = builtinProfiles.value.find(x => x.id === id)
  if (!p) return
  editProfile.value = JSON.parse(JSON.stringify(p))
  tsFormatsText.value = (p.timestamp_formats || []).join(', ')
  applyReparse()
}
function addRule() { editProfile.value.rules.push({ name: '', pattern: '' }) }
async function applyReparse() {
  if (!sessionId.value) return
  reparsing.value = true; editError.value = ''
  try {
    const profile = {
      ...editProfile.value,
      timestamp_regex: editProfile.value.timestamp_regex || null,
      timestamp_formats: tsFormatsText.value.split(',').map(s => s.trim()).filter(Boolean),
    }
    const r = await invoke('log_reparse', { sessionId: sessionId.value, profile })
    // 重解析后刷新帧与清空分析缓存
    frames.value = await invoke('log_list_frames', { sessionId: sessionId.value })
    leakRows.value = []; browseMetrics.value = []; diffRows.value = []; selectedKey.value = ''; seriesPoints.value = []
    logger.info('reparse done', r)
  } catch (e) { editError.value = '重解析失败: ' + (e?.message || e) } finally { reparsing.value = false }
}
async function saveParseProfile() {
  try {
    const path = await save({ title: '导出解析规则', defaultPath: 'log-parse-profile.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path) return
    const profile = { ...editProfile.value, timestamp_regex: editProfile.value.timestamp_regex || null, timestamp_formats: tsFormatsText.value.split(',').map(s => s.trim()).filter(Boolean) }
    await invoke('log_save_parse_profile', { path, profile })
  } catch (e) { editError.value = '导出失败: ' + (e?.message || e) }
}
async function loadParseProfile() {
  try {
    const sel = await open({ multiple: false, directory: false, filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!sel) return
    const path = typeof sel === 'string' ? sel : sel?.path
    const p = await invoke('log_load_parse_profile', { path })
    editProfile.value = { ...p, timestamp_regex: p.timestamp_regex || '' }
    tsFormatsText.value = (p.timestamp_formats || []).join(', ')
  } catch (e) { editError.value = '导入失败: ' + (e?.message || e) }
}
const aggFuncOptions = [
  { label: '计数', value: 'count' },
  { label: '求和', value: 'sum' },
  { label: '平均', value: 'avg' },
  { label: '最小', value: 'min' },
  { label: '最大', value: 'max' },
]

const totalHeight = computed(() => (meta.value ? meta.value.line_count * LINE_HEIGHT.value : 0))

const visibleRange = computed(() => {
  if (!meta.value) return { start: 0, end: 0 }
  const total = meta.value.line_count
  const first = Math.max(0, Math.floor(scrollTop.value / LINE_HEIGHT.value) - OVERSCAN)
  const visible = Math.ceil(viewportHeight.value / LINE_HEIGHT.value) + OVERSCAN * 2
  return { start: first, end: Math.min(total, first + visible) }
})

const visibleRows = computed(() => {
  const { start, end } = visibleRange.value
  const rows = []
  for (let i = start; i < end; i++) rows.push({ index: i, text: lineAt(i) })
  return rows
})

function lineAt(index) {
  const page = Math.floor(index / PAGE)
  const arr = pageCache.get(page)
  return arr ? (arr[index - page * PAGE] ?? '') : ''
}

async function ensurePages(start, end) {
  if (!sessionId.value) return
  const firstPage = Math.floor(start / PAGE)
  const lastPage = Math.floor((end - 1) / PAGE)
  for (let p = firstPage; p <= lastPage; p++) {
    if (p < 0 || pageCache.has(p) || pendingPages.has(p)) continue
    pendingPages.add(p)
    try {
      const res = await invoke('log_read_lines', { sessionId: sessionId.value, start: p * PAGE, count: PAGE })
      pageCache.set(p, res.lines)
    } catch (e) {
      logger.error('读取行失败', e)
    } finally {
      pendingPages.delete(p)
    }
  }
}

let rafPending = false
function onScroll() {
  if (rafPending) return
  rafPending = true
  requestAnimationFrame(() => {
    rafPending = false
    const el = viewportRef.value
    if (!el) return
    scrollTop.value = el.scrollTop
    viewportHeight.value = el.clientHeight
    const { start, end } = visibleRange.value
    ensurePages(start, end)
  })
}

async function onImport() {
  error.value = ''
  let selected
  try {
    selected = await open({ multiple: false, directory: false, title: '选择日志文件' })
  } catch (e) {
    error.value = '打开文件对话框失败'
    logger.error('open dialog failed', e)
    return
  }
  if (!selected) return
  const path = typeof selected === 'string' ? selected : selected?.path
  if (!path) return
  await doImport(path)
}

async function doImport(path) {
  importing.value = true
  error.value = ''
  try {
    if (sessionId.value) await invoke('log_close_session', { sessionId: sessionId.value }).catch(() => {})
    resetAll()
    const res = await invoke('log_import_file', { path })
    meta.value = res
    frames.value = res.frames
    sessionId.value = res.session_id
    diffB.value = Math.max(0, res.frame_count - 1)
    selectedProfileId.value = 'snapshot'
    if (!builtinProfiles.value.length) await loadBuiltinProfiles()
    const snap = builtinProfiles.value.find(p => p.id === 'snapshot')
    if (snap) { editProfile.value = JSON.parse(JSON.stringify(snap)); tsFormatsText.value = '' }
    logger.info('日志导入完成', { lines: res.line_count, frames: res.frame_count, bytes: res.byte_size })
    await nextTick()
    const el = viewportRef.value
    if (el) viewportHeight.value = el.clientHeight
    await ensurePages(0, Math.ceil(viewportHeight.value / LINE_HEIGHT.value) + OVERSCAN * 2)
  } catch (e) {
    error.value = '导入失败: ' + (e?.message || e)
    logger.error('导入失败', e)
  } finally {
    importing.value = false
  }
}

// 会话录制回放：把 SSH 录制文件喂进同一分析管道（B4）。
async function onReplay(path) {
  activeTab.value = 'log'
  innerTab.value = 'raw'
  await doImport(path)
}

function resetAll() {
  pageCache.clear()
  pendingPages.clear()
  scrollTop.value = 0
  activeFrame.value = -1
  leakRows.value = []
  browseMetrics.value = []
  diffRows.value = []
  selectedKey.value = ''
  seriesPoints.value = []
  searchHits.value = []
  templates.value = []
  formatReport.value = null
  extractColumns.value = []
  extractRows.value = []
  if (viewportRef.value) viewportRef.value.scrollTop = 0
}

function jumpToFrame(f) {
  activeFrame.value = f.index
  const el = viewportRef.value
  if (el) el.scrollTop = f.line_start * LINE_HEIGHT.value
}

async function runLeak() {
  if (!sessionId.value) return
  leakLoading.value = true
  try {
    leakRows.value = await invoke('log_leak_ranking', { sessionId: sessionId.value, minSamples: 5, limit: 200 })
  } catch (e) {
    error.value = '泄漏排名失败: ' + (e?.message || e)
    logger.error('leak ranking failed', e)
  } finally {
    leakLoading.value = false
  }
}

async function loadFrameMetrics() {
  if (!sessionId.value) return
  browseLoading.value = true
  try {
    browseMetrics.value = await invoke('log_frame_metrics', { sessionId: sessionId.value, frameIndex: browseFrame.value })
  } catch (e) {
    error.value = '加载帧指标失败: ' + (e?.message || e)
    logger.error('frame metrics failed', e)
  } finally {
    browseLoading.value = false
  }
}

const filteredMetrics = computed(() => {
  const kw = browseKeyword.value.trim().toLowerCase()
  if (!kw) return browseMetrics.value
  return browseMetrics.value.filter(m => m.key.toLowerCase().includes(kw))
})

async function selectMetric(key) {
  selectedKey.value = key
  try {
    seriesPoints.value = await invoke('log_metric_series', { sessionId: sessionId.value, key })
  } catch (e) {
    logger.error('取时间序列失败', e)
    seriesPoints.value = []
  }
}

const chartOption = computed(() => {
  const pts = seriesPoints.value
  if (!pts.length) return null
  const useTs = pts.some(p => p.timestamp_ms != null)
  const data = pts.map(p => [useTs && p.timestamp_ms != null ? p.timestamp_ms : p.frame, p.value])
  return {
    animation: false,
    grid: { left: 60, right: 20, top: 20, bottom: 50 },
    tooltip: { trigger: 'axis' },
    xAxis: { type: useTs ? 'time' : 'value', name: useTs ? '时间' : '帧' },
    yAxis: { type: 'value', scale: true },
    dataZoom: [{ type: 'inside' }, { type: 'slider', height: 18, bottom: 12 }],
    series: [{ type: 'line', data, showSymbol: false, sampling: 'lttb', large: true, lineStyle: { width: 1.2 } }],
  }
})

async function runDiff() {
  if (!sessionId.value) return
  diffLoading.value = true
  try {
    diffRows.value = await invoke('log_snapshot_diff', {
      sessionId: sessionId.value, frameA: diffA.value, frameB: diffB.value, limit: 200,
    })
  } catch (e) {
    error.value = '快照对比失败: ' + (e?.message || e)
    logger.error('snapshot diff failed', e)
  } finally {
    diffLoading.value = false
  }
}

function pinMetric(key) {
  if (key && !pinnedKeys.value.includes(key)) pinnedKeys.value.push(key)
}
function unpinMetric(key) {
  pinnedKeys.value = pinnedKeys.value.filter(k => k !== key)
}

async function saveProfile() {
  try {
    const path = await save({ title: '保存指标 Profile', defaultPath: 'metrics-profile.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path) return
    await invoke('log_save_profile', { path, name: shortPath.value || 'profile', keys: pinnedKeys.value })
    logger.info('profile saved', { path, count: pinnedKeys.value.length })
  } catch (e) {
    error.value = '保存 Profile 失败: ' + (e?.message || e)
  }
}

async function loadProfile() {
  try {
    const selected = await open({ multiple: false, directory: false, title: '加载指标 Profile', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!selected) return
    const path = typeof selected === 'string' ? selected : selected?.path
    const profile = await invoke('log_load_profile', { path })
    pinnedKeys.value = profile.keys || []
    logger.info('profile loaded', { count: pinnedKeys.value.length })
  } catch (e) {
    error.value = '加载 Profile 失败: ' + (e?.message || e)
  }
}

// L：发送到主界面编辑器（复用现有 template_insert_event 跨窗口通道）
async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送到编辑器失败: ' + (e?.message || e) }
}
function tableToMarkdown(columns, rows) {
  if (!columns.length) return ''
  const head = '| ' + columns.join(' | ') + ' |'
  const sep = '| ' + columns.map(() => '---').join(' | ') + ' |'
  const body = rows.map(r => '| ' + r.map(c => String(c).replace(/\|/g, '\\|')).join(' | ') + ' |').join('\n')
  return `${head}\n${sep}\n${body}\n`
}
function sendLeak() {
  const cols = ['指标', '首值', '末值', '增量', '斜率', '单调%', '样本']
  const rows = leakRows.value.map(r => [r.key, fmtNum(r.first), fmtNum(r.last), fmtNum(r.delta), r.slope.toFixed(2), (r.monotonic_ratio * 100).toFixed(0), r.samples])
  sendToEditor('## 泄漏排名\n\n' + tableToMarkdown(cols, rows))
}
function sendExtract() {
  sendToEditor(tableToMarkdown(extractColumns.value, extractRows.value.slice(0, 500)))
}
function sendSeriesCsv() {
  if (!seriesPoints.value.length) return
  const csv = 'frame,timestamp_ms,value\n' +
    seriesPoints.value.map(p => `${p.frame},${p.timestamp_ms ?? ''},${p.value}`).join('\n')
  sendToEditor('```csv\n' + `# ${selectedKey.value}\n` + csv + '\n```\n')
}

function fmtNum(n) {
  if (n == null || !isFinite(n)) return '—'
  if (Math.abs(n) >= 1000) return Math.round(n).toLocaleString()
  return String(n)
}

function formatBytes(n) {
  if (n < 1024) return n + ' B'
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB'
  if (n < 1024 * 1024 * 1024) return (n / 1024 / 1024).toFixed(1) + ' MB'
  return (n / 1024 / 1024 / 1024).toFixed(2) + ' GB'
}

const shortPath = computed(() => {
  if (!meta.value?.path) return ''
  const parts = meta.value.path.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || meta.value.path
})

async function runSearch() {
  if (!sessionId.value || !searchQuery.value) return
  searchLoading.value = true
  templates.value = []
  try {
    searchHits.value = await invoke('log_search', {
      sessionId: sessionId.value, query: searchQuery.value,
      isRegex: searchRegex.value, ignoreCase: searchIgnoreCase.value, limit: 1000,
    })
  } catch (e) { error.value = '检索失败: ' + (e?.message || e) } finally { searchLoading.value = false }
}

function gotoLine(line) {
  innerTab.value = 'raw'
  activeFrame.value = -1
  nextTick(() => { const el = viewportRef.value; if (el) el.scrollTop = line * LINE_HEIGHT.value })
}

async function runCluster() {
  if (!sessionId.value) return
  tplLoading.value = true
  searchHits.value = []
  try {
    templates.value = await invoke('log_cluster_templates', { sessionId: sessionId.value, limit: 200 })
  } catch (e) { error.value = '聚类失败: ' + (e?.message || e) } finally { tplLoading.value = false }
}

async function runDetectFormat() {
  if (!sessionId.value) return
  fmtLoading.value = true
  try {
    formatReport.value = await invoke('log_detect_format', { sessionId: sessionId.value, sample: 500 })
  } catch (e) { error.value = '识别失败: ' + (e?.message || e) } finally { fmtLoading.value = false }
}

async function runExtract() {
  if (!sessionId.value || !extractPattern.value) return
  extractLoading.value = true
  try {
    const r = await invoke('log_extract_regex', { sessionId: sessionId.value, pattern: extractPattern.value, limit: 5000 })
    extractColumns.value = r.columns
    extractRows.value = r.rows.map(x => x.values)
  } catch (e) { error.value = '抽取失败: ' + (e?.message || e) } finally { extractLoading.value = false }
}

async function runAggregate() {
  if (!sessionId.value || !extractPattern.value || !aggGroup.value || !aggValue.value) {
    error.value = '聚合需要：抽取正则 + 分组组名 + 数值组名'
    return
  }
  aggLoading.value = true
  try {
    const r = await invoke('log_aggregate', {
      sessionId: sessionId.value, parsePattern: extractPattern.value,
      groupField: aggGroup.value, valueField: aggValue.value,
      agg: aggFunc.value, filter: searchQuery.value || null,
    })
    const lines = (r.csv || '').trim().split(/\r?\n/).filter(Boolean)
    if (!lines.length) { extractColumns.value = []; extractRows.value = [] }
    else { extractColumns.value = lines[0].split(','); extractRows.value = lines.slice(1).map(l => l.split(',')) }
  } catch (e) { error.value = '聚合失败: ' + (e?.message || e) } finally { aggLoading.value = false }
}

// E2 管道 DSL：filter <re> | parse <re> | stats <agg>(<val>) by <group>
async function runQuery() {
  const text = dsl.value.trim()
  if (!text) return
  let hasParse = false, hasStats = false
  for (const rawStage of text.split('|')) {
    const stage = rawStage.trim()
    if (stage.startsWith('filter ')) searchQuery.value = stage.slice(7).trim()
    else if (stage.startsWith('parse ')) { extractPattern.value = stage.slice(6).trim(); hasParse = true }
    else if (stage.startsWith('stats ')) {
      const m = stage.slice(6).trim().match(/^(\w+)\s*\(\s*(\w+)\s*\)\s+by\s+(\w+)$/)
      if (m) { aggFunc.value = m[1] === 'mean' ? 'avg' : m[1]; aggValue.value = m[2]; aggGroup.value = m[3]; hasStats = true }
      else { error.value = 'stats 语法应为 agg(value) by group'; return }
    }
  }
  if (hasStats) await runAggregate()
  else if (hasParse) await runExtract()
  else if (searchQuery.value) { innerTab.value = 'search'; await runSearch() }
}

onBeforeUnmount(() => {
  if (sessionId.value) invoke('log_close_session', { sessionId: sessionId.value }).catch(() => {})
})</script>

<style scoped>
.rt-root { height: 100%; display: flex; flex-direction: column; background: var(--guaxi-content-bg); --tw-ui-font: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif; --tw-mono-font: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace); --tw-accent: #0ca6c0; --tw-accent-2: #1fbfd6; --tw-accent-soft: rgba(12,166,192,0.12); --tw-radius: 10px; --tw-radius-sm: 7px; --tw-code-bg: #0f1720; --tw-code-fg: #d6dee8; --tw-elev: 0 1px 2px rgba(2,12,20,0.12), 0 6px 20px rgba(2,12,20,0.10); font-family: var(--tw-ui-font); font-size: 13px; color: var(--guaxi-text-primary); }
:root[data-guaxi-theme='dark'] .rt-root { --tw-accent: #3dd6e8; --tw-accent-2: #22b8cf; --tw-accent-soft: rgba(61,214,232,0.14); --tw-code-bg: #0b1118; --tw-code-fg: #d8e0eb; }
.rt-tabs { height: 100%; display: flex; flex-direction: column; }
.rt-tabs :deep(.rt-pane) { height: 100%; padding: 0; }
.rt-log { height: 100%; display: flex; flex-direction: column; min-height: 0; }
.rt-toolbar { display: flex; align-items: center; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--guaxi-border); flex: 0 0 auto; }
.rt-meta { font-size: 12px; color: var(--guaxi-text-secondary, #6b7280); }
.rt-error { font-size: 12px; color: #e5484d; }
.rt-inner { flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column; }
.rt-inner-tabs { height: 100%; display: flex; flex-direction: column; }
.rt-inner-tabs :deep(.rt-inner-pane) { height: 100%; padding: 0; min-height: 0; }

.rt-body { height: 100%; display: flex; min-height: 0; }
.rt-timeline { width: 300px; flex: 0 0 auto; border-right: 1px solid var(--guaxi-border); display: flex; flex-direction: column; min-height: 0; }
.rt-timeline-title { padding: 8px 12px; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border); }
.rt-timeline-list { flex: 1 1 auto; overflow-y: auto; min-height: 0; }
.rt-frame { padding: 6px 12px; font-size: 12px; cursor: pointer; display: flex; gap: 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.04)); }
.rt-frame:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.rt-frame.active { background: var(--guaxi-active-bg, rgba(59,130,246,0.12)); }
.rt-frame-idx { color: var(--guaxi-text-secondary, #9ca3af); flex: 0 0 auto; }
.rt-frame-ts { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.rt-viewport { flex: 1 1 auto; overflow: auto; position: relative; font-family: var(--tw-mono-font); background: var(--guaxi-content-bg); color: var(--guaxi-text-primary); border-radius: 0 0 var(--tw-radius) 0; }
.rt-spacer { position: relative; width: 100%; }
.rt-line { position: absolute; left: 0; right: 0; height: 18px; display: flex; white-space: pre; }
.rt-lineno { flex: 0 0 auto; width: 64px; text-align: right; padding-right: 12px; color: var(--guaxi-code-lineno, #6b7280); user-select: none; }
.rt-linetext { flex: 1 1 auto; overflow: hidden; }

.rt-analysis { height: 100%; display: flex; flex-direction: column; min-height: 0; padding: 10px 12px; gap: 10px; box-sizing: border-box; }
.rt-actions { display: flex; align-items: center; gap: 10px; flex: 0 0 auto; flex-wrap: wrap; }
.rt-hint { font-size: 12px; color: var(--guaxi-text-secondary, #6b7280); }
.rt-split { flex: 1 1 auto; min-height: 0; display: flex; gap: 12px; }
.rt-table-wrap { flex: 1 1 55%; min-width: 0; overflow: auto; border: 1px solid var(--guaxi-border); border-radius: 6px; }
.rt-table-wrap.full { flex: 1 1 auto; }
.rt-chart-wrap { flex: 1 1 45%; min-width: 0; display: flex; flex-direction: column; border: 1px solid var(--guaxi-border); border-radius: 6px; padding: 8px; }
.rt-chart-head { font-size: 12px; display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 6px; }
.rt-chart-head span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rt-chart { flex: 1 1 auto; min-height: 200px; width: 100%; }
.rt-chart-empty { flex: 1 1 auto; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary, #6b7280); font-size: 12px; }

.rt-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.rt-table th, .rt-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); white-space: nowrap; }
.rt-table th { position: sticky; top: 0; background: var(--guaxi-panel-bg, #f5f5f5); z-index: 1; }
.rt-table tbody tr { cursor: pointer; }
.rt-table tbody tr:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.rt-table tbody tr.active { background: var(--guaxi-active-bg, rgba(59,130,246,0.12)); }
.rt-key { max-width: 420px; overflow: hidden; text-overflow: ellipsis; }
.rt-delta { color: #d9480f; }
.rt-delta-neg { color: #2b8a3e; }
.rt-empty-cell { color: var(--guaxi-text-secondary, #9ca3af); text-align: center; padding: 20px; }

.rt-empty { flex: 1 1 auto; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary, #6b7280); font-size: 13px; padding: 40px; text-align: center; }

/* ---- 美化层 ---- */
.tw-tab { display: inline-flex; align-items: center; gap: 7px; font-weight: 500; }
.rt-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); padding: 12px 18px; transition: color .18s ease; }
.rt-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.rt-tabs :deep(.n-tabs-bar) { background: var(--tw-accent) !important; height: 2.5px; border-radius: 2px; }
.rt-tabs :deep(.n-tabs-nav) { padding: 0 6px; }
.rt-toolbar { padding: 10px 14px; gap: 10px; background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); }
.rt-meta { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
.rt-chip { display: inline-flex; align-items: center; gap: 5px; font-size: 11.5px; padding: 3px 9px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); font-family: var(--tw-mono-font); }
.rt-timeline { width: 300px; }
.rt-timeline-title { text-transform: uppercase; letter-spacing: 0.06em; font-size: 11px; color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); display: flex; align-items: center; justify-content: space-between; }
.rt-fontctl { display: inline-flex; align-items: center; gap: 3px; }
.rt-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 18px; height: 18px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.rt-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.rt-fsz { min-width: 16px; text-align: center; font-size: 11px; color: var(--tw-accent); }
.rt-line { color: var(--guaxi-text-primary); }
.rt-lineno { color: var(--guaxi-text-secondary); opacity: .7; }
.rt-split-resize { flex: 0 0 auto; width: 6px; cursor: col-resize; background: var(--guaxi-border); opacity: .4; border-radius: 3px; transition: opacity .15s, background .15s; }
.rt-split-resize:hover { opacity: 1; background: var(--tw-accent); }
.rt-chart-full { width: 90vw; height: 84vh; display: flex; flex-direction: column; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; font-family: var(--tw-ui-font); }
.rt-chart-full-head { display: flex; align-items: center; justify-content: space-between; padding: 12px 18px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; font-family: var(--tw-mono-font); }
.rt-chart-full-body { flex: 1 1 auto; min-height: 0; width: 100%; }
.rt-frame { border-bottom-color: color-mix(in srgb, var(--guaxi-border) 45%, transparent); transition: background .12s ease; }
.rt-frame.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.rt-frame.active .rt-frame-idx { color: var(--tw-accent); }
.rt-inner-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); font-weight: 500; }
.rt-inner-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.rt-table-wrap, .rt-chart-wrap { border-radius: var(--tw-radius-sm); border-color: color-mix(in srgb, var(--guaxi-border) 70%, transparent); }
.rt-table th { text-transform: uppercase; letter-spacing: 0.04em; font-size: 11px; color: var(--guaxi-text-secondary); }
.rt-table tbody tr.active { background: var(--tw-accent-soft); }
.rt-key { font-family: var(--tw-mono-font); }
.rt-lineno { font-family: var(--tw-mono-font); }
.rt-tl-resize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .45; transition: opacity .15s, background .15s; }
.rt-tl-resize:hover { opacity: 1; background: var(--tw-accent); }
.rt-editor { width: 680px; max-width: 92vw; max-height: 86vh; display: flex; flex-direction: column; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); font-family: var(--tw-ui-font); overflow: hidden; }
.rt-editor-head { display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.rt-editor-close { cursor: pointer; color: var(--guaxi-text-secondary); }
.rt-editor-close:hover { color: var(--tw-accent); }
.rt-editor-body { padding: 16px 18px; overflow: auto; display: flex; flex-direction: column; gap: 12px; }
.rt-ed-row { display: flex; align-items: center; gap: 10px; }
.rt-ed-row > label { flex: 0 0 76px; font-size: 12px; color: var(--guaxi-text-secondary); }
.rt-ed-row > :deep(.n-input), .rt-ed-row > :deep(.n-radio-group) { flex: 1 1 auto; }
.rt-ed-rules { border: 1px solid color-mix(in srgb, var(--guaxi-border) 70%, transparent); border-radius: var(--tw-radius-sm); padding: 10px 12px; }
.rt-ed-rules-head { display: flex; align-items: center; justify-content: space-between; font-size: 12px; font-weight: 600; margin-bottom: 8px; color: var(--tw-accent); }
.rt-ed-rule { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.rt-ed-rule :deep(.n-input__input-el) { font-family: var(--tw-mono-font); }
.rt-editor-foot { display: flex; justify-content: flex-end; gap: 8px; padding: 12px 18px; border-top: 1px solid var(--guaxi-border); }
</style>
