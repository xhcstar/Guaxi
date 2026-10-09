<template>
  <div class="rh" :class="dense ? 'is-dense' : ''">
    <!-- 左侧图标导航栏 -->
    <nav class="rh-rail">
      <div class="rh-rail-top">
        <n-tooltip v-for="w in workspaces" :key="w.id" placement="right" :delay="200">
          <template #trigger>
            <button class="rh-rail-btn" :class="{ active: workspace === w.id }" @click="workspace = w.id">
              <Icon :icon="w.icon" width="20" height="20" />
              <span v-if="w.id === 'downloads' && activeCount" class="rh-rail-badge">{{ activeCount }}</span>
            </button>
          </template>
          {{ w.label }}
        </n-tooltip>
      </div>
    </nav>

    <!-- 右侧主区 -->
    <div class="rh-main">
      <!-- ===== 发现工作区 ===== -->
      <section v-show="workspace === 'discover'" class="rh-ws">
        <header class="rh-command">
          <div class="rh-command-bar">
            <Icon :icon="ic.search" width="20" height="20" class="rh-command-ic" />
            <input
              ref="searchInput"
              v-model="keyword"
              class="rh-command-input"
              :placeholder="rotatingHint"
              @keyup.enter="doSearch"
              @focus="showSuggest = true"
              @input="showSuggest = true"
            />
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-command-go" :disabled="searching" @click="doSearch">
                <Icon :icon="searching ? ic.loader : ic.arrowRight" width="18" height="18" :class="searching ? 'spin' : ''" />
              </button>
            </template>开始搜索 (Enter)</n-tooltip>

            <!-- 建议 / 历史 -->
            <div v-if="showSuggest" class="rh-suggest" @mousedown.prevent>
              <div class="rh-suggest-sec" v-if="recent.length">
                <span class="rh-suggest-h"><Icon :icon="ic.clock" width="12" height="12" />最近</span>
                <button v-for="q in recent" :key="'r'+q" class="rh-suggest-item" @click="applySuggest(q)">{{ q }}</button>
              </div>
              <div class="rh-suggest-sec">
                <span class="rh-suggest-h"><Icon :icon="ic.sparkles" width="12" height="12" />检索语法</span>
                <button v-for="t in templates" :key="'t'+t.q" class="rh-suggest-item" @click="applySuggest(t.q)">
                  <code>{{ t.q }}</code><span class="rh-suggest-desc">{{ t.desc }}</span>
                </button>
              </div>
            </div>
          </div>

          <!-- 类型 chips -->
          <div class="rh-chips">
            <button v-for="k in kindChips" :key="k.id" class="rh-chip" :class="{ on: kindSel.includes(k.id) || (k.id==='all' && !kindSel.length) }" @click="toggleKind(k.id)">
              <Icon v-if="k.icon" :icon="k.icon" width="14" height="14" />{{ k.label }}
            </button>
            <span class="rh-chips-sep" />
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-chip ghost" :class="{ on: useLlm }" @click="useLlm = !useLlm">
                <Icon :icon="ic.sparkles" width="14" height="14" />智能扩展
              </button>
            </template>用 LLM 扩展同义词/中英/相关术语，提升召回（需已配置模型）</n-tooltip>
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-chip ghost" :class="{ on: multilingual }" @click="multilingual = !multilingual">
                <Icon :icon="ic.globe" width="14" height="14" />多语检索
              </button>
            </template>默认已含中英双语召回；开启后再译日/德/法/俄/西并行检索，召回更广（可能引入他国语言结果）</n-tooltip>
          </div>
        </header>
        <!-- PLACEHOLDER_DISCOVER_BODY -->
        <div class="rh-disc-body">
          <div v-if="!results.length && !searching" class="rh-empty-hero">
            <Icon :icon="ic.radar" width="46" height="46" />
            <p class="rh-empty-title">全网嗅探书籍 · 论文 · 视频 · 音频 · 图片</p>
            <p class="rh-empty-sub">输入关键词开始，或试试这些</p>
            <div class="rh-empty-examples">
              <button v-for="e in examples" :key="e" @click="applySuggest(e); doSearch()">{{ e }}</button>
            </div>
          </div>

          <template v-else>
            <!-- 结果工具条：结果内筛选 + 分面 + 视图 + 批量 -->
            <div class="rh-toolbar">
              <div class="rh-filter-inline">
                <Icon :icon="ic.filter" width="15" height="15" />
                <input v-model="resultQuery" class="rh-mini-input" placeholder="在结果中筛选…" />
              </div>
              <n-select v-if="facetSources.length > 1" v-model:value="facetSource" size="small" class="rh-facet"
                :options="facetSourceOptions" />
              <n-select v-model:value="sortBy" size="small" class="rh-facet" :options="sortOptions" />
              <span class="rh-toolbar-spacer" />
              <span class="rh-count">{{ filteredResults.length }} / {{ results.length }}</span>
              <n-tooltip :delay="200"><template #trigger>
                <button class="rh-icon-btn" :class="{ on: selected.length }" :disabled="!selected.length" @click="enqueueBatch">
                  <Icon :icon="ic.download" width="16" height="16" /><span v-if="selected.length" class="rh-mini-badge">{{ selected.length }}</span>
                </button>
              </template>批量下载选中</n-tooltip>
              <div class="rh-view-toggle">
                <button :class="{ on: resultView==='list' }" @click="resultView='list'"><Icon :icon="ic.list" width="15" height="15" /></button>
                <button :class="{ on: resultView==='grid' }" @click="resultView='grid'"><Icon :icon="ic.grid" width="15" height="15" /></button>
              </div>
            </div>

            <div v-if="searching" class="rh-searching"><Icon :icon="ic.loader" width="18" height="18" class="spin" /> 正在多引擎嗅探…</div>
            <template v-else>
              <!-- 类型分栏：按资源类型分组展示，避免混在一起 -->
              <div v-if="resultKinds.length > 1" class="rh-kind-tabs">
                <button class="rh-kind-tab" :class="{ on: resultKindTab==='all' }" @click="resultKindTab='all'">
                  全部 <span class="rh-kt-count">{{ results.length }}</span>
                </button>
                <button v-for="k in resultKinds" :key="k.id" class="rh-kind-tab" :class="['k-'+k.id, { on: resultKindTab===k.id }]" @click="resultKindTab=k.id">
                  <Icon v-if="KIND[k.id]?.icon" :icon="KIND[k.id].icon" width="13" height="13" />
                  {{ KIND[k.id]?.label || k.id }} <span class="rh-kt-count">{{ k.count }}</span>
                </button>
              </div>
              <n-virtual-list v-if="resultView==='list' && filteredResults.length" class="rh-scroll" :items="filteredResults" :item-size="60" key-field="id">
                <template #default="{ item }">
                  <RhItem :item="item" view="list" :kind-meta="KIND" selectable :selected="selected.includes(item.id)"
                    :actions="resultActions" @toggle="toggleSel" @act="onResultAct" />
                </template>
              </n-virtual-list>
              <n-scrollbar v-else class="rh-scroll">
                <div v-if="filteredResults.length" class="rh-grid">
                  <RhItem v-for="item in filteredResults" :key="item.id" :item="item" view="grid" :kind-meta="KIND"
                    :thumb="thumbFor(item)" selectable :selected="selected.includes(item.id)" :actions="resultActions" @toggle="toggleSel" @act="onResultAct" />
                </div>
                <div v-else class="rh-empty-mini">没有符合当前筛选的结果</div>
              </n-scrollbar>
            </template>
          </template>
        </div>
      </section>
      <!-- PLACEHOLDER_WORKSPACES -->

      <!-- ===== 资源库工作区 ===== -->
      <section v-show="workspace === 'library'" class="rh-ws">
        <header class="rh-ws-head">
          <h2 class="rh-ws-title"><Icon :icon="ic.library" width="18" height="18" />资源库</h2>
          <div class="rh-lib-search">
            <Icon :icon="ic.search" width="15" height="15" />
            <input v-model="libQuery" class="rh-mini-input" placeholder="在资源库中搜索标题…" />
          </div>
          <n-select v-model:value="libKind" size="small" class="rh-facet" :options="libKindOptions" />
          <n-select v-if="allTags.length" v-model:value="libTag" size="small" class="rh-facet"
            :options="libTagOptions" placeholder="标签" clearable />
          <span class="rh-toolbar-spacer" />
          <span class="rh-count">{{ filteredLib.length }} 项</span>
          <template v-if="libSelected.length">
            <span class="rh-count">已选 {{ libSelected.length }}</span>
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-icon-btn" @click="bulkTag"><Icon :icon="ic.sparkles" width="16" height="16" /></button>
            </template>给选中项打标签</n-tooltip>
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-icon-btn danger" @click="bulkDelete"><Icon :icon="ic.trash" width="16" height="16" /></button>
            </template>删除选中项</n-tooltip>
            <n-tooltip :delay="200"><template #trigger>
              <button class="rh-icon-btn" @click="libSelected = []"><Icon :icon="ic.x" width="16" height="16" /></button>
            </template>清除选择</n-tooltip>
          </template>
          <n-tooltip :delay="200"><template #trigger>
            <button class="rh-icon-btn" @click="loadLibrary"><Icon :icon="ic.refresh" width="16" height="16" /></button>
          </template>刷新</n-tooltip>
          <n-tooltip :delay="200"><template #trigger>
            <button class="rh-icon-btn" @click="exportLibrary"><Icon :icon="ic.save" width="16" height="16" /></button>
          </template>导出资源库(备份)</n-tooltip>
          <n-tooltip :delay="200"><template #trigger>
            <button class="rh-icon-btn" @click="importLibrary"><Icon :icon="ic.upload" width="16" height="16" /></button>
          </template>导入资源库</n-tooltip>
          <div class="rh-view-toggle">
            <button :class="{ on: libView==='list' }" @click="libView='list'"><Icon :icon="ic.list" width="15" height="15" /></button>
            <button :class="{ on: libView==='grid' }" @click="libView='grid'"><Icon :icon="ic.grid" width="15" height="15" /></button>
          </div>
        </header>
        <div class="rh-lib-body">
          <div v-if="!filteredLib.length" class="rh-scroll rh-empty-mini">
            {{ libItems.length ? '没有符合筛选的资源' : '下载完成的资源会出现在这里' }}
          </div>
          <n-virtual-list v-else-if="libView==='list'" class="rh-scroll" :items="filteredLib" :item-size="58" key-field="id">
            <template #default="{ item }">
              <RhItem :item="item" view="list" :kind-meta="KIND" clickable selectable :selected="libSelected.includes(item.id)"
                :active="preview && preview.id===item.id" :actions="libActions"
                @pick="preview = item" @act="onLibAct" @toggle="toggleLibSel" />
            </template>
          </n-virtual-list>
          <n-scrollbar v-else class="rh-scroll">
            <div class="rh-grid">
              <RhItem v-for="r in filteredLib" :key="r.id" :item="r" view="grid" :kind-meta="KIND" clickable selectable
                :thumb="thumbFor(r)" :selected="libSelected.includes(r.id)" :active="preview && preview.id===r.id" :actions="libActions"
                @pick="preview = r" @act="onLibAct" @toggle="toggleLibSel" />
            </div>
          </n-scrollbar>
          <!-- 预览 / 详情面板 -->
          <aside v-if="preview" class="rh-preview">
            <div class="rh-preview-head">
              <span class="rh-kbadge lg" :class="'k-'+preview.kind"><Icon :icon="KIND[preview.kind]?.icon || ic.file" width="16" height="16" /></span>
              <button class="rh-icon-btn sm" @click="preview = null"><Icon :icon="ic.x" width="15" height="15" /></button>
            </div>
            <div class="rh-preview-title">{{ preview.title }}</div>
            <div v-if="previewSrc" class="rh-preview-media">
              <img v-if="preview.kind==='image'" :src="previewSrc" alt="" />
              <audio v-else-if="preview.kind==='audio'" :src="previewSrc" controls />
              <video v-else-if="preview.kind==='video'" :src="previewSrc" controls preload="metadata" />
            </div>
            <dl class="rh-preview-meta">
              <div><dt>类型</dt><dd>{{ KIND[preview.kind]?.label || preview.kind }}</dd></div>
              <div><dt>来源</dt><dd>{{ preview.source || '本地' }}</dd></div>
              <div v-if="preview.size_hint"><dt>大小</dt><dd>{{ fmtSize(preview.size_hint) }}</dd></div>
              <div v-if="preview.local_path"><dt>路径</dt><dd class="rh-path">{{ preview.local_path }}</dd></div>
            </dl>
            <div class="rh-preview-tags">
              <span class="rh-block-label">标签</span>
              <n-dynamic-tags :value="tagsOf(preview)" @update:value="v => saveTags(preview, v)" />
            </div>
            <div class="rh-preview-actions">
              <n-button size="small" type="primary" @click="openPath(preview.local_path)"><template #icon><Icon :icon="ic.open" /></template>打开</n-button>
              <n-button size="small" @click="reveal(preview.local_path)"><template #icon><Icon :icon="ic.folder" /></template>定位</n-button>
              <n-button size="small" @click="sharePath(preview.local_path)"><template #icon><Icon :icon="ic.share" /></template>分享</n-button>
            </div>
          </aside>
        </div>
      </section>
      <!-- PLACEHOLDER_DOWNLOADS -->

      <!-- ===== 下载工作区 ===== -->
      <section v-show="workspace === 'downloads'" class="rh-ws">
        <header class="rh-ws-head">
          <h2 class="rh-ws-title"><Icon :icon="ic.download" width="18" height="18" />下载队列</h2>
          <span class="rh-toolbar-spacer" />
          <span class="rh-count">{{ taskList.length }} 个任务</span>
          <n-tooltip :delay="200"><template #trigger>
            <button class="rh-icon-btn" @click="refreshTasks"><Icon :icon="ic.refresh" width="16" height="16" /></button>
          </template>刷新</n-tooltip>
        </header>
        <n-scrollbar class="rh-scroll">
          <div v-if="!taskList.length" class="rh-empty-mini">暂无下载任务。在「发现」里搜索并下载资源。</div>
          <div class="rh-task" v-for="t in taskList" :key="t.id" :class="'k-'+t.kind">
            <span class="rh-kbadge" :class="'k-'+t.kind"><Icon :icon="KIND[t.kind]?.icon || ic.file" width="12" height="12" /></span>
            <div class="rh-task-main">
              <div class="rh-task-title" :title="t.title">{{ t.title }}</div>
              <n-progress type="line" :percentage="pct(t)" :height="6" :show-indicator="false"
                :status="t.status==='error' ? 'error' : (t.status==='completed' ? 'success' : 'default')" />
              <div class="rh-task-meta">
                <span>{{ statusLabel(t.status) }}<template v-if="t.status==='active'"> · {{ fmtSpeed(t.speed) }} · {{ pct(t) }}%</template><template v-if="t.error"> · {{ t.error }}</template></span>
              </div>
            </div>
            <div class="rh-task-actions">
              <n-tooltip v-if="t.status==='active'" :delay="200"><template #trigger>
                <button class="rh-icon-btn sm" @click="pause(t.id)"><Icon :icon="ic.pause" width="15" height="15" /></button>
              </template>暂停</n-tooltip>
              <n-tooltip v-else-if="t.status==='paused'||t.status==='error'" :delay="200"><template #trigger>
                <button class="rh-icon-btn sm" @click="resume(t.id)"><Icon :icon="ic.play" width="15" height="15" /></button>
              </template>继续</n-tooltip>
              <n-tooltip v-if="t.status==='completed'" :delay="200"><template #trigger>
                <button class="rh-icon-btn sm primary" @click="openPath(t.dest)"><Icon :icon="ic.open" width="15" height="15" /></button>
              </template>打开</n-tooltip>
              <n-tooltip :delay="200"><template #trigger>
                <button class="rh-icon-btn sm danger" @click="cancel(t.id)"><Icon :icon="ic.x" width="15" height="15" /></button>
              </template>{{ t.status==='completed' ? '移除' : '取消' }}</n-tooltip>
            </div>
          </div>
        </n-scrollbar>
      </section>
      <!-- PLACEHOLDER_SETTINGS -->

      <!-- ===== 设置工作区 ===== -->
      <section v-show="workspace === 'settings'" class="rh-ws">
        <header class="rh-ws-head">
          <h2 class="rh-ws-title"><Icon :icon="ic.settings" width="18" height="18" />设置</h2>
        </header>
        <n-scrollbar class="rh-scroll">
          <div class="rh-settings">
            <div class="rh-set-card">
              <div class="rh-set-title"><Icon :icon="ic.shield" width="16" height="16" />代理</div>
              <p class="rh-set-hint">境外搜索引擎与富集站在中国大陆需代理才可达。设置后境外源自动走代理，境内源直连。</p>
              <n-radio-group v-model:value="proxy.mode" @update:value="saveProxy">
                <n-space>
                  <n-radio value="none">不使用</n-radio>
                  <n-radio value="system">跟随系统</n-radio>
                  <n-radio value="custom">自定义</n-radio>
                </n-space>
              </n-radio-group>
              <n-input v-if="proxy.mode==='custom'" v-model:value="proxy.url" class="rh-set-input"
                placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080" @blur="saveProxy" />
              <n-checkbox v-model:checked="proxy.proxy_foreign_only" @update:checked="saveProxy">仅境外域名走代理（境内直连）</n-checkbox>
              <div class="rh-block-label">直连白名单（域名后缀，每行一个）</div>
              <n-input v-model:value="bypassText" type="textarea" :rows="3" placeholder=".cn&#10;bilibili.com" @blur="saveProxy" />
            </div>

            <div class="rh-set-card">
              <div class="rh-set-title"><Icon :icon="ic.folder" width="16" height="16" />下载目录</div>
              <p class="rh-set-hint">所有下载的文件将保存到这里。可直接粘贴/输入路径，或点「选择」浏览。磁力任务的目录在下次启动后生效。</p>
              <div class="rh-dir-row">
                <n-input v-model:value="downloadDir" placeholder="默认目录（留空恢复默认）" @change="saveDownloadDir" @blur="saveDownloadDir" />
                <n-button size="small" @click="pickDownloadDir"><template #icon><Icon :icon="ic.folder" /></template>选择</n-button>
                <n-button size="small" quaternary @click="resetDownloadDir">恢复默认</n-button>
              </div>
              <div class="rh-block-label">全局限速（KB/s，0 = 不限速）</div>
              <div class="rh-dir-row">
                <n-input-number v-model:value="speedLimit" :min="0" :step="128" style="width:180px" @update:value="saveSpeedLimit" />
                <span class="rh-set-hint" style="margin:0">对多线程 HTTP 下载生效</span>
              </div>
            </div>

            <div class="rh-set-card">
              <div class="rh-set-title"><Icon :icon="ic.globe" width="16" height="16" />搜索引擎</div>
              <p class="rh-set-hint">勾选参与嗅探的引擎。带地球标记的是境外引擎，会走代理。</p>
              <div class="rh-eng-grid">
                <label v-for="e in engineMetas" :key="e.id" class="rh-eng" :class="{ on: engineOn.includes(e.id) }">
                  <n-checkbox :checked="engineOn.includes(e.id)" @update:checked="v => toggleEngine(e.id, v)" />
                  <span class="rh-eng-label">{{ e.label }}</span>
                  <n-tooltip v-if="e.foreign" :delay="200"><template #trigger>
                    <Icon :icon="ic.globe" width="13" height="13" class="rh-eng-foreign" />
                  </template>境外引擎 · 走代理</n-tooltip>
                </label>
              </div>
              <div class="rh-block-label">来源</div>
              <n-space>
                <n-checkbox v-model:checked="webDiscovery">全网发现 + 嗅探</n-checkbox>
                <n-checkbox v-model:checked="seedSites">富集站(arXiv/OpenAlex)</n-checkbox>
              </n-space>
            </div>

            <div class="rh-set-card">
              <div class="rh-set-title"><Icon :icon="ic.shield" width="16" height="16" />高质量源 API Key</div>
              <p class="rh-set-hint">可选。填写后自动启用对应源，留空则跳过。Key 仅存本地。</p>
              <div class="rh-key-grid">
                <label class="rh-key-row">
                  <span class="rh-key-label">CORE（论文全文）</span>
                  <n-input v-model:value="apiKeys.core" type="password" show-password-on="click"
                    placeholder="core.ac.uk API Key" @blur="saveApiKeys" size="small" />
                </label>
                <label class="rh-key-row">
                  <span class="rh-key-label">Pexels（图片/视频）</span>
                  <n-input v-model:value="apiKeys.pexels" type="password" show-password-on="click"
                    placeholder="pexels.com API Key" @blur="saveApiKeys" size="small" />
                </label>
                <label class="rh-key-row">
                  <span class="rh-key-label">Jamendo（音乐）</span>
                  <n-input v-model:value="apiKeys.jamendo" type="password" show-password-on="click"
                    placeholder="jamendo client_id" @blur="saveApiKeys" size="small" />
                </label>
                <label class="rh-key-row">
                  <span class="rh-key-label">Unpaywall 邮箱</span>
                  <n-input v-model:value="apiKeys.unpaywall_email"
                    placeholder="你的邮箱（Unpaywall 要求，用于开放获取论文）" @blur="saveApiKeys" size="small" />
                </label>
                <label class="rh-key-row">
                  <span class="rh-key-label">Unsplash（图片）</span>
                  <n-input v-model:value="apiKeys.unsplash" type="password" show-password-on="click"
                    placeholder="unsplash.com Access Key" @blur="saveApiKeys" size="small" />
                </label>
                <label class="rh-key-row">
                  <span class="rh-key-label">Pixabay（图片/视频）</span>
                  <n-input v-model:value="apiKeys.pixabay" type="password" show-password-on="click"
                    placeholder="pixabay.com API Key" @blur="saveApiKeys" size="small" />
                </label>
              </div>
              <div class="rh-block-label">SearXNG 元搜索实例</div>
              <p class="rh-set-hint">填写自建或公共 SearXNG 地址（需支持 JSON API），大幅增强全网发现稳定性。</p>
              <n-input v-model:value="searxngUrl" placeholder="例如 https://searx.example.com"
                @blur="saveSearxng" size="small" />
            </div>
          </div>
        </n-scrollbar>
      </section>
    </div>

    <n-modal v-model:show="showBulkTag" preset="card" title="批量打标签" style="width:420px">
      <n-space vertical>
        <p class="rh-set-hint">给选中的 {{ libSelected.length }} 项追加标签（逗号或空格分隔）。</p>
        <n-input v-model:value="bulkTagText" placeholder="例如：机器学习, 收藏" @keyup.enter="applyBulkTag" />
        <n-button type="primary" @click="applyBulkTag">应用</n-button>
      </n-space>
    </n-modal>
  </div>
</template>

<!-- PLACEHOLDER_SCRIPT -->

<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { createDiscreteApi, darkTheme } from 'naive-ui'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { createLogger } from '../../../utils/logger'
import { useSettingsStore } from '../../../stores/settingsStore'
import { Icon } from '@iconify/vue'
import RhItem from './RhItem.vue'
import searchIcon from '@iconify-icons/lucide/search'
import arrowRightIcon from '@iconify-icons/lucide/arrow-right'
import loaderIcon from '@iconify-icons/lucide/loader-2'
import clockIcon from '@iconify-icons/lucide/clock'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import filterIcon from '@iconify-icons/lucide/filter'
import listIcon from '@iconify-icons/lucide/list'
import gridIcon from '@iconify-icons/lucide/layout-grid'
import downloadIcon from '@iconify-icons/lucide/download'
import externalIcon from '@iconify-icons/lucide/external-link'
import fileIcon from '@iconify-icons/lucide/file'
import libraryIcon from '@iconify-icons/lucide/library'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import folderIcon from '@iconify-icons/lucide/folder-open'
import trashIcon from '@iconify-icons/lucide/trash-2'
import shareIcon from '@iconify-icons/lucide/share-2'
import xIcon from '@iconify-icons/lucide/x'
import pauseIcon from '@iconify-icons/lucide/pause'
import playIcon from '@iconify-icons/lucide/play'
import settingsIcon from '@iconify-icons/lucide/settings-2'
import shieldIcon from '@iconify-icons/lucide/shield'
import globeIcon from '@iconify-icons/lucide/globe'
import bookIcon from '@iconify-icons/lucide/book'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import videoIcon from '@iconify-icons/lucide/video'
import musicIcon from '@iconify-icons/lucide/music'
import imageIcon from '@iconify-icons/lucide/image'
import openIcon from '@iconify-icons/lucide/book-open'
import radarIcon from '@iconify-icons/lucide/radar'
import codeIcon from '@iconify-icons/lucide/code'
import databaseIcon from '@iconify-icons/lucide/database'
import uploadIcon from '@iconify-icons/lucide/upload'
import saveIcon from '@iconify-icons/lucide/hard-drive-download'

const logger = createLogger('ResourceHunterApp')
const settingsStore = useSettingsStore()
// 弹窗/消息脱离了 n-config-provider，需显式带上主题，否则暗色模式下文字看不清
const discreteConfig = computed(() => ({
  theme: settingsStore.appearanceTheme === 'dark' ? darkTheme : null,
}))
const { message, dialog } = createDiscreteApi(['message', 'dialog'], {
  configProviderProps: discreteConfig,
})

const ic = {
  search: searchIcon, arrowRight: arrowRightIcon, loader: loaderIcon, clock: clockIcon,
  sparkles: sparklesIcon, filter: filterIcon, list: listIcon, grid: gridIcon, download: downloadIcon,
  external: externalIcon, file: fileIcon, library: libraryIcon, refresh: refreshIcon, folder: folderIcon,
  trash: trashIcon, share: shareIcon, x: xIcon, pause: pauseIcon, play: playIcon, settings: settingsIcon,
  shield: shieldIcon, globe: globeIcon, open: openIcon, radar: radarIcon,
  upload: uploadIcon, save: saveIcon,
}
const KIND = {
  paper: { label: '论文', icon: fileTextIcon },
  book: { label: '书籍', icon: bookIcon },
  video: { label: '视频', icon: videoIcon },
  audio: { label: '音频', icon: musicIcon },
  image: { label: '图片', icon: imageIcon },
  code: { label: '代码', icon: codeIcon },
  dataset: { label: '数据集', icon: databaseIcon },
  other: { label: '其他', icon: fileIcon },
}
const resultActions = [
  { key: 'open', icon: externalIcon, tip: '浏览器打开' },
  { key: 'download', icon: downloadIcon, tip: '下载', cls: 'primary' },
]
const libActions = [
  { key: 'open', icon: openIcon, tip: '打开', cls: 'primary' },
  { key: 'reveal', icon: folderIcon, tip: '在文件夹中显示' },
  { key: 'del', icon: trashIcon, tip: '删除', cls: 'danger' },
]
// PLACEHOLDER_SCRIPT_B

const workspaces = [
  { id: 'discover', label: '发现', icon: searchIcon },
  { id: 'library', label: '资源库', icon: libraryIcon },
  { id: 'downloads', label: '下载', icon: downloadIcon },
  { id: 'settings', label: '设置', icon: settingsIcon },
]
const kindChips = [
  { id: 'all', label: '全部', icon: null },
  { id: 'paper', label: '论文', icon: fileTextIcon },
  { id: 'book', label: '书籍', icon: bookIcon },
  { id: 'video', label: '视频', icon: videoIcon },
  { id: 'audio', label: '音频', icon: musicIcon },
  { id: 'image', label: '图片', icon: imageIcon },
  { id: 'code', label: '代码', icon: codeIcon },
  { id: 'dataset', label: '数据集', icon: databaseIcon },
]
const examples = ['深度学习 filetype:pdf', 'Rust 编程 epub', '宋代海上贸易', 'lofi hip hop', '星空 4k 壁纸']
const templates = [
  { q: 'filetype:pdf', desc: '限定文件类型' },
  { q: 'site:arxiv.org', desc: '限定站点' },
  { q: 'intitle:', desc: '标题包含' },
]
const HINTS = ['搜书籍、论文、视频、音频、图片…', '试试「作者名 + filetype:pdf」', '输入关键词，全网嗅探可下载文件']

const workspace = ref('discover')
const dense = ref(false)
// 发现
const keyword = ref('')
const kindSel = ref([])
const searching = ref(false)
const results = ref([])
const selected = ref([])
const resultView = ref('list')
const resultQuery = ref('')
const facetSource = ref('all')
const sortBy = ref('relevance')
const resultKindTab = ref('all')
const showSuggest = ref(false)
const searchInput = ref(null)
const useLlm = ref(false)
const multilingual = ref(false)
const recent = ref([])
const hintIdx = ref(0)
let hintTimer = null
// 引擎/来源/代理
const engineMetas = ref([])
const engineOn = ref([])
const webDiscovery = ref(true)
const seedSites = ref(true)
const proxy = ref({ mode: 'system', url: '', bypass: [], proxy_foreign_only: false })
const bypassText = ref('')
const downloadDir = ref('')
const speedLimit = ref(0)
// 第三方 API Key（可选，填了对应源才启用）与 SearXNG 元搜索实例
const apiKeys = ref({ core: '', pexels: '', jamendo: '', unpaywall_email: '', unsplash: '', pixabay: '' })
const searxngUrl = ref('')
// 下载
const tasks = ref({})
// 资源库
const libItems = ref([])
const libQuery = ref('')
const libKind = ref('all')
const libTag = ref(null)
const libView = ref('grid')
const libSelected = ref([])
const showBulkTag = ref(false)
const bulkTagText = ref('')
const allTags = ref([])
const preview = ref(null)
let unlisten = null
// PLACEHOLDER_SCRIPT_C

const rotatingHint = computed(() => HINTS[hintIdx.value % HINTS.length])
const suggestions = computed(() => (recent.value.length ? 1 : 0) + templates.length)

const taskList = computed(() =>
  Object.values(tasks.value).sort((a, b) => (b.created_at || 0) - (a.created_at || 0)),
)
const activeCount = computed(() =>
  taskList.value.filter((t) => t.status === 'active' || t.status === 'queued').length,
)

const facetSources = computed(() => [...new Set(results.value.map((r) => r.source))])
const facetSourceOptions = computed(() => [
  { label: '全部来源', value: 'all' },
  ...facetSources.value.map((s) => ({ label: s, value: s })),
])
const sortOptions = [
  { label: '相关度', value: 'relevance' },
  { label: '标题', value: 'title' },
]
const KIND_ORDER = ['paper', 'book', 'video', 'audio', 'image', 'code', 'other']
const resultKinds = computed(() => {
  const counts = {}
  for (const r of results.value) counts[r.kind] = (counts[r.kind] || 0) + 1
  return KIND_ORDER.filter((k) => counts[k]).map((k) => ({ id: k, count: counts[k] }))
})
const filteredResults = computed(() => {
  let list = results.value
  if (resultKindTab.value !== 'all') list = list.filter((r) => r.kind === resultKindTab.value)
  const q = resultQuery.value.trim().toLowerCase()
  if (q) list = list.filter((r) => r.title.toLowerCase().includes(q))
  if (facetSource.value !== 'all') list = list.filter((r) => r.source === facetSource.value)
  const arr = [...list]
  if (sortBy.value === 'title') arr.sort((a, b) => a.title.localeCompare(b.title))
  else arr.sort((a, b) => b.relevance - a.relevance)
  return arr
})
// 图片/视频类目默认用网格视图，其余用列表
watch(resultKindTab, (k) => {
  if (k === 'image' || k === 'video') resultView.value = 'grid'
  else if (k !== 'all') resultView.value = 'list'
})

const libKindOptions = [
  { label: '全部类型', value: 'all' },
  ...['paper', 'book', 'video', 'audio', 'image', 'code', 'dataset', 'other'].map((k) => ({ label: KIND[k].label, value: k })),
]
const libTagOptions = computed(() => allTags.value.map((t) => ({ label: t, value: t })))
function tagsOf(r) {
  return (r.tags || '').split(',').map((s) => s.trim()).filter(Boolean)
}
const filteredLib = computed(() => {
  let list = libItems.value
  if (libKind.value !== 'all') list = list.filter((r) => r.kind === libKind.value)
  if (libTag.value) list = list.filter((r) => tagsOf(r).includes(libTag.value))
  return list
})
const previewSrc = computed(() => {
  const p = preview.value
  if (!p || !p.local_path) return ''
  if (!['image', 'audio', 'video'].includes(p.kind)) return ''
  try { return convertFileSrc(p.local_path) } catch (e) { return '' }
})

const KIND_LABEL = { queued: '排队', active: '下载中', paused: '已暂停', completed: '完成', error: '出错' }
const statusLabel = (s) => KIND_LABEL[s] || s
function pct(t) {
  if (!t.total_size) return t.status === 'completed' ? 100 : 0
  return Math.min(100, Math.round((t.downloaded / t.total_size) * 100))
}
function fmtSpeed(b) {
  if (!b) return '—'
  if (b > 1048576) return (b / 1048576).toFixed(1) + ' MB/s'
  if (b > 1024) return (b / 1024).toFixed(0) + ' KB/s'
  return b + ' B/s'
}
function fmtSize(b) {
  if (!b) return ''
  if (b > 1073741824) return (b / 1073741824).toFixed(1) + ' GB'
  if (b > 1048576) return (b / 1048576).toFixed(1) + ' MB'
  if (b > 1024) return (b / 1024).toFixed(0) + ' KB'
  return b + ' B'
}
// PLACEHOLDER_SCRIPT_D

function toggleKind(id) {
  if (id === 'all') { kindSel.value = []; return }
  kindSel.value = kindSel.value.includes(id)
    ? kindSel.value.filter((k) => k !== id)
    : [...kindSel.value, id]
}
function toggleSel(id, v) {
  if (v) selected.value = [...selected.value, id]
  else selected.value = selected.value.filter((x) => x !== id)
}

const thumbs = ref({})
const thumbInflight = new Set()
function thumbFor(item) {
  if (item.kind !== 'image' && item.kind !== 'video') return ''
  if (item.local_path) {
    try { return convertFileSrc(item.local_path) } catch (e) { return '' }
  }
  const url = item.thumbnail || item.download_url
  if (!url) return ''
  if (thumbs.value[item.id]) return thumbs.value[item.id]
  if (!thumbInflight.has(item.id)) {
    thumbInflight.add(item.id)
    invoke('rh_fetch_thumb', { url })
      .then((src) => { if (src) thumbs.value = { ...thumbs.value, [item.id]: src } })
      .catch(() => {})
  }
  return ''
}
function applySuggest(q) {
  keyword.value = q
  showSuggest.value = false
}
function pushRecent(q) {
  const list = [q, ...recent.value.filter((x) => x !== q)].slice(0, 6)
  recent.value = list
  try { localStorage.setItem('rh_recent', JSON.stringify(list)) } catch (e) {}
}

async function doSearch() {
  const kw = keyword.value.trim()
  if (!kw) return
  showSuggest.value = false
  searching.value = true
  results.value = []
  selected.value = []
  resultKindTab.value = 'all'
  pushRecent(kw)
  try {
    const items = await invoke('rh_search', {
      query: {
        keyword: kw,
        kinds: kindSel.value,
        engines: engineOn.value,
        web_discovery: webDiscovery.value,
        seed_sites: seedSites.value,
        limit: 60,
        use_llm: useLlm.value,
        multilingual: multilingual.value,
      },
    })
    results.value = items || []
    if (!results.value.length) message.info('没有找到结果。检查代理设置，或换关键词/来源。')
  } catch (e) {
    logger.error('搜索失败', e)
    message.error('搜索失败: ' + e)
  } finally {
    searching.value = false
  }
}

async function enqueue(item) {
  try {
    await invoke('rh_enqueue', { item })
    message.success('已加入下载队列')
  } catch (e) {
    message.error('加入下载失败: ' + e)
  }
}
async function enqueueBatch() {
  const items = filteredResults.value.filter((r) => selected.value.includes(r.id))
  if (!items.length) return
  try {
    const ids = await invoke('rh_enqueue_batch', { items })
    message.success(`已加入 ${ids.length} 个下载任务`)
    selected.value = []
    workspace.value = 'downloads'
  } catch (e) {
    message.error('批量下载失败: ' + e)
  }
}
function onResultAct(key, item) {
  if (key === 'open') openOnline(item)
  else if (key === 'download') enqueue(item)
}
function onLibAct(key, item) {
  if (key === 'open') openPath(item.local_path)
  else if (key === 'reveal') reveal(item.local_path)
  else if (key === 'del') delResource(item)
}
// PLACEHOLDER_SCRIPT_E

async function refreshTasks() {
  try {
    const list = await invoke('rh_list_tasks')
    const map = {}
    for (const t of list || []) map[t.id] = t
    tasks.value = map
  } catch (e) { logger.error(e) }
}
async function pause(id) { try { await invoke('rh_pause', { id }) } catch (e) { message.error('' + e) } }
async function resume(id) { try { await invoke('rh_resume', { id }); await refreshTasks() } catch (e) { message.error('' + e) } }
async function cancel(id) {
  try { await invoke('rh_cancel', { id }); const m = { ...tasks.value }; delete m[id]; tasks.value = m } catch (e) { message.error('' + e) }
}
async function openOnline(item) {
  const url = item.source_url || item.download_url
  if (url) { try { await invoke('rh_open_path', { path: url }) } catch (e) { message.error('打开失败: ' + e) } }
}
async function openPath(path) {
  if (path) { try { await invoke('rh_open_path', { path }) } catch (e) { message.error('打开失败: ' + e) } }
}
async function reveal(path) {
  if (path) { try { await invoke('rh_reveal', { path }) } catch (e) { message.error('定位失败: ' + e) } }
}
async function sharePath(path) {
  if (!path) return
  try { await navigator.clipboard.writeText(path); message.success('已复制文件路径') }
  catch (e) { reveal(path) }
}

async function loadLibrary() {
  try {
    libItems.value = (await invoke('rh_list_resources', { kind: null })) || []
    allTags.value = (await invoke('rh_all_tags')) || []
  } catch (e) { logger.error(e) }
}

let libSearchTimer = null
watch(libQuery, () => {
  clearTimeout(libSearchTimer)
  libSearchTimer = setTimeout(async () => {
    const q = libQuery.value.trim()
    if (!q) { await loadLibrary(); return }
    try {
      libItems.value = (await invoke('rh_library_search', { query: q, kind: null })) || []
    } catch (e) { logger.error('资源库检索失败', e) }
  }, 250)
})
function delResource(r) {
  dialog.warning({
    title: '删除资源',
    content: `确定从资源库移除「${r.title}」？可选择是否同时删除本地文件。`,
    positiveText: '删除记录',
    negativeText: '删除记录和文件',
    onPositiveClick: async () => { await doDelete(r.id, false) },
    onNegativeClick: async () => { await doDelete(r.id, true) },
  })
}
async function doDelete(id, removeFile) {
  try {
    await invoke('rh_delete_resource', { id, removeFile })
    libItems.value = libItems.value.filter((x) => x.id !== id)
    if (preview.value && preview.value.id === id) preview.value = null
    message.success('已删除')
  } catch (e) { message.error('删除失败: ' + e) }
}
async function saveTags(r, tags) {
  try {
    await invoke('rh_update_tags', { id: r.id, tags })
    r.tags = tags.join(',')
    allTags.value = (await invoke('rh_all_tags')) || []
  } catch (e) { message.error('保存标签失败: ' + e) }
}

function toggleLibSel(id, v) {
  if (v) libSelected.value = [...libSelected.value, id]
  else libSelected.value = libSelected.value.filter((x) => x !== id)
}
function bulkDelete() {
  const n = libSelected.value.length
  if (!n) return
  dialog.warning({
    title: '批量删除',
    content: `确定从资源库移除选中的 ${n} 项？（仅删记录，不删本地文件）`,
    positiveText: '删除',
    negativeText: '取消',
    onPositiveClick: async () => {
      const ids = [...libSelected.value]
      for (const id of ids) {
        try { await invoke('rh_delete_resource', { id, removeFile: false }) } catch (e) {}
      }
      libItems.value = libItems.value.filter((r) => !ids.includes(r.id))
      if (preview.value && ids.includes(preview.value.id)) preview.value = null
      libSelected.value = []
      message.success('已删除')
    },
  })
}
function bulkTag() {
  bulkTagText.value = ''
  showBulkTag.value = true
}
async function applyBulkTag() {
  const tags = bulkTagText.value.split(/[,，\s]+/).map((s) => s.trim()).filter(Boolean)
  showBulkTag.value = false
  if (!tags.length) return
  for (const id of libSelected.value) {
    const r = libItems.value.find((x) => x.id === id)
    if (!r) continue
    const merged = Array.from(new Set([...tagsOf(r), ...tags]))
    try { await invoke('rh_update_tags', { id, tags: merged }); r.tags = merged.join(',') } catch (e) {}
  }
  allTags.value = (await invoke('rh_all_tags')) || []
  message.success('已打标签')
}

async function loadEngines() {
  try {
    engineMetas.value = (await invoke('rh_list_engines')) || []
    engineOn.value = engineMetas.value.filter((e) => e.default_on).map((e) => e.id)
  } catch (e) { logger.error(e) }
}
function toggleEngine(id, v) {
  if (v) engineOn.value = [...engineOn.value, id]
  else engineOn.value = engineOn.value.filter((x) => x !== id)
}
async function loadProxy() {
  try {
    const cfg = await invoke('rh_get_proxy')
    if (cfg) { proxy.value = cfg; bypassText.value = (cfg.bypass || []).join('\n') }
  } catch (e) { logger.error(e) }
}
async function saveProxy() {
  proxy.value.bypass = bypassText.value.split('\n').map((s) => s.trim()).filter(Boolean)
  try { await invoke('rh_set_proxy', { config: proxy.value }) } catch (e) { message.error('保存失败: ' + e) }
}

async function loadDownloadDir() {
  try { downloadDir.value = await invoke('rh_get_download_dir') } catch (e) { logger.error(e) }
  try { speedLimit.value = await invoke('rh_get_speed_limit') } catch (e) {}
}
async function saveSpeedLimit() {
  try { await invoke('rh_set_speed_limit', { kbps: Number(speedLimit.value) || 0 }) } catch (e) { message.error('' + e) }
}
// 加载/保存第三方 API Key 与 SearXNG 实例地址
async function loadApiKeys() {
  try {
    const k = await invoke('rh_get_api_keys')
    if (k && typeof k === 'object') apiKeys.value = { ...apiKeys.value, ...k }
  } catch (e) { logger.error(e) }
  try { searxngUrl.value = (await invoke('rh_get_searxng')) || '' } catch (e) {}
}
async function saveApiKeys() {
  try { await invoke('rh_set_api_keys', { keys: { ...apiKeys.value } }) } catch (e) { message.error('保存失败: ' + e) }
}
async function saveSearxng() {
  try { await invoke('rh_set_searxng', { url: (searxngUrl.value || '').trim() }) } catch (e) { message.error('保存失败: ' + e) }
}
async function exportLibrary() {
  try {
    const path = await invoke('rh_export_library')
    message.success('已导出到: ' + path)
  } catch (e) { message.error('导出失败: ' + e) }
}
async function importLibrary() {
  try {
    const file = await openDialog({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }], title: '选择资源库清单' })
    if (typeof file === 'string' && file) {
      const n = await invoke('rh_import_library', { path: file })
      await loadLibrary()
      message.success(`已导入 ${n} 项`)
    }
  } catch (e) { message.error('导入失败: ' + e) }
}
async function pickDownloadDir() {
  try {
    const dir = await openDialog({ directory: true, multiple: false, title: '选择下载目录', defaultPath: downloadDir.value || undefined })
    if (typeof dir === 'string' && dir) {
      await invoke('rh_set_download_dir', { path: dir })
      downloadDir.value = dir
      message.success('下载目录已更新')
    }
  } catch (e) { message.error('设置目录失败: ' + e) }
}
// 手动输入/粘贴路径后保存（失焦或回车触发）
async function saveDownloadDir() {
  const p = (downloadDir.value || '').trim()
  try {
    await invoke('rh_set_download_dir', { path: p })
    if (p) message.success('下载目录已更新')
  } catch (e) {
    message.error('设置目录失败: ' + e)
    await loadDownloadDir()
  }
}
async function resetDownloadDir() {
  try {
    await invoke('rh_set_download_dir', { path: '' })
    await loadDownloadDir()
    message.success('已恢复默认下载目录')
  } catch (e) { message.error('' + e) }
}
// PLACEHOLDER_SCRIPT_F

onMounted(async () => {
  try { recent.value = JSON.parse(localStorage.getItem('rh_recent') || '[]') } catch (e) {}
  await Promise.all([loadEngines(), loadProxy(), loadDownloadDir(), loadApiKeys(), refreshTasks(), loadLibrary()])
  unlisten = await listen('resource_hunter://download_update', (evt) => {
    const p = evt.payload
    if (!p) return
    if (p.removed) { const m = { ...tasks.value }; delete m[p.id]; tasks.value = m; return }
    tasks.value = { ...tasks.value, [p.id]: p }
    if (p.status === 'completed') loadLibrary()
  })
  hintTimer = setInterval(() => { hintIdx.value++ }, 4000)
  document.addEventListener('click', onDocClick)
})
function onDocClick(e) {
  if (!e.target.closest('.rh-command-bar')) showSuggest.value = false
}
onUnmounted(() => {
  if (unlisten) unlisten()
  if (hintTimer) clearInterval(hintTimer)
  document.removeEventListener('click', onDocClick)
})
</script>

<!-- PLACEHOLDER_STYLE -->

<style scoped>
.rh {
  display: flex;
  height: 100vh;
  background: var(--guaxi-content-bg, #fff);
  color: var(--guaxi-text-primary, #1f2328);
  font-size: 14px;
  --rh-accent: #2563eb;
  --k-paper: #4f6bed; --k-book: #8b5cf6; --k-video: #ef4444;
  --k-audio: #f59e0b; --k-image: #10b981; --k-other: #6b7280; --k-code: #0ea5e9;
  --k-dataset: #14b8a6;
}
/* 左侧图标导航 */
.rh-rail {
  width: 56px; flex-shrink: 0;
  background: var(--guaxi-panel-bg, #f4f6f8);
  border-right: 1px solid var(--guaxi-border, #d0d7de);
  display: flex; flex-direction: column; align-items: center;
  padding-top: 10px; gap: 6px;
}
.rh-rail-btn {
  position: relative;
  width: 40px; height: 40px; border-radius: 10px;
  display: flex; align-items: center; justify-content: center;
  border: none; background: transparent; cursor: pointer;
  color: var(--guaxi-text-secondary, #57606a);
  transition: background .15s, color .15s;
}
.rh-rail-btn:hover { background: var(--guaxi-border, #e3e6ea); color: var(--guaxi-text-primary); }
.rh-rail-btn.active { background: color-mix(in srgb, var(--rh-accent) 18%, transparent); color: var(--rh-accent); }
.rh-rail-badge {
  position: absolute; top: 2px; right: 2px; min-width: 16px; height: 16px;
  padding: 0 4px; border-radius: 8px; background: var(--rh-accent); color: #fff;
  font-size: 10px; line-height: 16px; text-align: center;
}
.rh-main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.rh-ws { flex: 1; min-height: 0; display: flex; flex-direction: column; }

/* 命令/搜索栏（发现区的主角） */
.rh-command { padding: 22px 24px 12px; border-bottom: 1px solid var(--guaxi-border, #eee); }
.rh-command-bar {
  position: relative; max-width: 720px; margin: 0 auto;
  display: flex; align-items: center; gap: 10px;
  background: var(--guaxi-app-bg, #fff);
  border: 1.5px solid var(--guaxi-border, #d0d7de); border-radius: 12px;
  padding: 10px 12px; box-shadow: 0 1px 3px rgba(0,0,0,.05);
}
.rh-command-bar:focus-within { border-color: var(--rh-accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--rh-accent) 15%, transparent); }
.rh-command-ic { color: var(--guaxi-text-secondary); flex-shrink: 0; }
.rh-command-input {
  flex: 1; border: none; outline: none; background: transparent;
  font-size: 18px; color: var(--guaxi-text-primary); font-family: inherit;
}
.rh-command-go {
  flex-shrink: 0; width: 36px; height: 36px; border-radius: 9px; border: none;
  background: var(--rh-accent); color: #fff; cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.rh-command-go:disabled { opacity: .6; }
.rh-chips { display: flex; gap: 8px; justify-content: center; margin-top: 12px; flex-wrap: wrap; }
.rh-chip {
  display: inline-flex; align-items: center; gap: 5px;
  padding: 5px 12px; border-radius: 999px; cursor: pointer; font-size: 13px;
  border: 1px solid var(--guaxi-border, #d0d7de); background: transparent;
  color: var(--guaxi-text-secondary);
}
.rh-chip.on { background: var(--rh-accent); border-color: var(--rh-accent); color: #fff; }
.rh-chip.ghost { border-style: dashed; }
.rh-chip.ghost.on { background: color-mix(in srgb, var(--rh-accent) 16%, transparent); border-color: var(--rh-accent); border-style: solid; color: var(--rh-accent); }
.rh-chips-sep { width: 1px; height: 18px; background: var(--guaxi-border, #d0d7de); margin: 0 2px; }
/* PLACEHOLDER_STYLE2 */
/* 建议下拉 */
.rh-suggest {
  position: absolute; top: calc(100% + 6px); left: 0; right: 0; z-index: 20;
  background: var(--guaxi-app-bg, #fff); border: 1px solid var(--guaxi-border, #d0d7de);
  border-radius: 10px; box-shadow: 0 8px 24px rgba(0,0,0,.14); padding: 8px; max-height: 340px; overflow-y: auto;
}
.rh-suggest-sec { padding: 4px 0; }
.rh-suggest-h { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--guaxi-text-secondary); padding: 4px 8px; }
.rh-suggest-item {
  display: flex; align-items: center; gap: 8px; width: 100%; text-align: left;
  padding: 7px 8px; border: none; background: transparent; border-radius: 7px; cursor: pointer;
  color: var(--guaxi-text-primary); font-size: 13px;
}
.rh-suggest-item:hover { background: var(--guaxi-border, #eef); }
.rh-suggest-item code { font-family: var(--guaxi-editor-font-family, monospace); color: var(--rh-accent); }
.rh-suggest-desc { color: var(--guaxi-text-secondary); font-size: 12px; }
/* 空状态 hero */
.rh-empty-hero { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: var(--guaxi-text-secondary); gap: 8px; }
.rh-empty-hero > svg { opacity: .5; }
.rh-empty-title { font-size: 16px; color: var(--guaxi-text-primary); margin: 6px 0 0; }
.rh-empty-sub { font-size: 13px; margin: 0; }
.rh-empty-examples { display: flex; gap: 8px; flex-wrap: wrap; justify-content: center; margin-top: 8px; max-width: 560px; }
.rh-empty-examples button {
  padding: 5px 12px; border-radius: 999px; border: 1px solid var(--guaxi-border, #d0d7de);
  background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font-size: 13px;
}
.rh-empty-examples button:hover { border-color: var(--rh-accent); color: var(--rh-accent); }
.rh-empty-mini { text-align: center; color: var(--guaxi-text-secondary); font-size: 13px; padding: 40px 0; }
.rh-searching { display: flex; align-items: center; gap: 8px; justify-content: center; color: var(--guaxi-text-secondary); padding: 16px; font-size: 13px; }
/* 工具条 */
.rh-disc-body, .rh-lib-body { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.rh-lib-body { flex-direction: row; }
.rh-toolbar, .rh-ws-head {
  display: flex; align-items: center; gap: 10px; padding: 10px 20px;
  border-bottom: 1px solid var(--guaxi-border, #eee);
}
.rh-ws-title { display: flex; align-items: center; gap: 8px; font-size: 16px; font-weight: 600; margin: 0; }
.rh-toolbar-spacer { flex: 1; }
.rh-count { font-size: 12px; color: var(--guaxi-text-secondary); }
.rh-filter-inline, .rh-lib-search {
  display: flex; align-items: center; gap: 6px; padding: 5px 10px; border-radius: 8px;
  border: 1px solid var(--guaxi-border, #d0d7de); background: var(--guaxi-app-bg, #fff); min-width: 200px;
}
.rh-lib-search { min-width: 240px; }
.rh-mini-input { border: none; outline: none; background: transparent; font-size: 13px; color: var(--guaxi-text-primary); font-family: inherit; flex: 1; }
.rh-facet { width: 130px; }
/* 结果类型分栏 */
.rh-kind-tabs { display: flex; align-items: center; gap: 6px; padding: 8px 20px; border-bottom: 1px solid var(--guaxi-border, #eee); flex-wrap: wrap; }
.rh-kind-tab {
  display: inline-flex; align-items: center; gap: 5px; padding: 4px 11px; border-radius: 999px; cursor: pointer;
  font-size: 12.5px; border: 1px solid var(--guaxi-border, #d0d7de); background: transparent;
  color: var(--guaxi-text-secondary); transition: background .12s, color .12s, border-color .12s;
}
.rh-kind-tab:hover { border-color: var(--k-color, var(--rh-accent)); color: var(--guaxi-text-primary); }
.rh-kind-tab.on { background: var(--k-color, var(--rh-accent)); border-color: var(--k-color, var(--rh-accent)); color: #fff; }
.rh-kind-tab.k-paper { --k-color: var(--k-paper); } .rh-kind-tab.k-book { --k-color: var(--k-book); }
.rh-kind-tab.k-video { --k-color: var(--k-video); } .rh-kind-tab.k-audio { --k-color: var(--k-audio); }
.rh-kind-tab.k-image { --k-color: var(--k-image); } .rh-kind-tab.k-code { --k-color: var(--k-code); }
.rh-kind-tab.k-other { --k-color: var(--k-other); }
.rh-kind-tab.k-dataset { --k-color: var(--k-dataset); }
.rh-kt-count { font-size: 11px; opacity: .8; font-variant-numeric: tabular-nums; }
/* PLACEHOLDER_STYLE3 */
.rh-icon-btn {
  position: relative; width: 32px; height: 30px; border-radius: 8px; border: 1px solid transparent;
  background: transparent; color: var(--guaxi-text-secondary); cursor: pointer;
  display: inline-flex; align-items: center; justify-content: center;
}
.rh-icon-btn:hover { background: var(--guaxi-border, #eef); color: var(--guaxi-text-primary); }
.rh-icon-btn.sm { width: 28px; height: 26px; }
.rh-icon-btn.primary { color: var(--rh-accent); }
.rh-icon-btn.primary:hover { background: color-mix(in srgb, var(--rh-accent) 14%, transparent); }
.rh-icon-btn.danger:hover { background: color-mix(in srgb, var(--k-video) 14%, transparent); color: var(--k-video); }
.rh-icon-btn.on { color: var(--rh-accent); }
.rh-icon-btn:disabled { opacity: .4; cursor: default; }
.rh-mini-badge, .rh-mini-badge { position: absolute; top: -3px; right: -3px; background: var(--rh-accent); color: #fff; font-size: 9px; min-width: 14px; height: 14px; border-radius: 7px; line-height: 14px; text-align: center; padding: 0 3px; }
.rh-view-toggle { display: inline-flex; border: 1px solid var(--guaxi-border, #d0d7de); border-radius: 8px; overflow: hidden; }
.rh-view-toggle button { width: 30px; height: 28px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; display: flex; align-items: center; justify-content: center; }
.rh-view-toggle button.on { background: var(--rh-accent); color: #fff; }
.rh-scroll { flex: 1; min-height: 0; }
/* 结果 / 资源库条目 */
.rh-rows { display: flex; flex-direction: column; }
.rh-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; padding: 14px 20px; }
.rh-item {
  display: flex; align-items: center; gap: 10px; border-left: 3px solid var(--k-color, transparent);
}
.rh-item.list { padding: 10px 20px; border-bottom: 1px solid var(--guaxi-border, #eee); }
.rh-item.grid { flex-direction: column; align-items: stretch; padding: 12px; border: 1px solid var(--guaxi-border, #e6e9ee); border-radius: 10px; background: var(--guaxi-panel-bg, #fafbfc); }
.rh-item.k-paper { --k-color: var(--k-paper); } .rh-item.k-book { --k-color: var(--k-book); }
.rh-item.k-video { --k-color: var(--k-video); } .rh-item.k-audio { --k-color: var(--k-audio); }
.rh-item.k-image { --k-color: var(--k-image); } .rh-item.k-other { --k-color: var(--k-other); }
.rh-item.k-code { --k-color: var(--k-code); }
.rh-item.k-dataset { --k-color: var(--k-dataset); }
.rh-item.active { background: color-mix(in srgb, var(--rh-accent) 8%, transparent); }
.rh-item-check { flex-shrink: 0; }
.rh-item-body { flex: 1; min-width: 0; cursor: default; }
.rh-item-title { display: flex; align-items: center; gap: 7px; font-size: 14px; line-height: 1.4; color: var(--guaxi-text-primary); }
.rh-item.list .rh-item-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rh-item.grid .rh-item-name { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.rh-item-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 4px; }
.rh-item-fmt { font-weight: 600; letter-spacing: .3px; }
.rh-item-tag { padding: 0 6px; border-radius: 4px; background: var(--guaxi-border, #eef); font-size: 11px; }
.rh-item-rel { margin-left: auto; }
.rh-item.grid .rh-item-rel { margin-left: 0; }
.rh-item-actions { display: flex; gap: 4px; flex-shrink: 0; }
.rh-item.grid .rh-item-actions { justify-content: flex-end; margin-top: 8px; }
.rh-kbadge { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: 6px; color: #fff; flex-shrink: 0; background: var(--k-other); }
.rh-kbadge.lg { width: 30px; height: 30px; border-radius: 8px; }
.rh-kbadge.k-paper { background: var(--k-paper); } .rh-kbadge.k-book { background: var(--k-book); }
.rh-kbadge.k-video { background: var(--k-video); } .rh-kbadge.k-audio { background: var(--k-audio); }
.rh-kbadge.k-image { background: var(--k-image); } .rh-kbadge.k-other { background: var(--k-other); }
.rh-kbadge.k-code { background: var(--k-code); }
.rh-kbadge.k-dataset { background: var(--k-dataset); }
/* PLACEHOLDER_STYLE4 */
/* 下载任务行 */
.rh-task { display: flex; align-items: center; gap: 12px; padding: 12px 20px; border-bottom: 1px solid var(--guaxi-border, #eee); border-left: 3px solid var(--k-color, transparent); }
.rh-task.k-paper { --k-color: var(--k-paper); } .rh-task.k-book { --k-color: var(--k-book); }
.rh-task.k-video { --k-color: var(--k-video); } .rh-task.k-audio { --k-color: var(--k-audio); }
.rh-task.k-image { --k-color: var(--k-image); } .rh-task.k-other { --k-color: var(--k-other); }
.rh-task.k-code { --k-color: var(--k-code); }
.rh-task.k-dataset { --k-color: var(--k-dataset); }
.rh-task-main { flex: 1; min-width: 0; }
.rh-task-title { font-size: 13px; color: var(--guaxi-text-primary); margin-bottom: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rh-task-meta { font-size: 12px; color: var(--guaxi-text-secondary); margin-top: 5px; }
.rh-task-actions { display: flex; gap: 4px; flex-shrink: 0; }
/* 预览面板 */
.rh-preview { width: 300px; flex-shrink: 0; border-left: 1px solid var(--guaxi-border, #d0d7de); padding: 14px; display: flex; flex-direction: column; gap: 12px; overflow-y: auto; background: var(--guaxi-panel-bg, #fafbfc); }
.rh-preview-head { display: flex; align-items: center; justify-content: space-between; }
.rh-preview-title { font-size: 15px; font-weight: 600; color: var(--guaxi-text-primary); line-height: 1.4; }
.rh-preview-media { border-radius: 8px; overflow: hidden; background: var(--guaxi-border, #eee); }
.rh-preview-media img, .rh-preview-media video { display: block; width: 100%; max-height: 240px; object-fit: contain; background: #000; }
.rh-preview-media audio { display: block; width: 100%; }
.rh-preview-meta { margin: 0; display: flex; flex-direction: column; gap: 8px; }
.rh-preview-meta > div { display: flex; gap: 8px; font-size: 12px; }
.rh-preview-meta dt { color: var(--guaxi-text-secondary); width: 40px; flex-shrink: 0; margin: 0; }
.rh-preview-meta dd { margin: 0; color: var(--guaxi-text-primary); word-break: break-all; }
.rh-path { font-size: 11px; }
.rh-block-label { font-size: 12px; color: var(--guaxi-text-secondary); }
.rh-preview-tags { display: flex; flex-direction: column; gap: 6px; }
.rh-preview-actions { display: flex; gap: 6px; flex-wrap: wrap; margin-top: auto; }
/* 设置 */
.rh-settings { max-width: 720px; margin: 0 auto; padding: 20px; display: flex; flex-direction: column; gap: 18px; }
.rh-set-card { border: 1px solid var(--guaxi-border, #e6e9ee); border-radius: 12px; padding: 16px; display: flex; flex-direction: column; gap: 12px; background: var(--guaxi-panel-bg, #fafbfc); }
.rh-set-title { display: flex; align-items: center; gap: 8px; font-size: 15px; font-weight: 600; }
.rh-set-hint { margin: 0; font-size: 12px; color: var(--guaxi-text-secondary); line-height: 1.5; }
.rh-set-input { max-width: 420px; }
.rh-eng-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 8px; }
.rh-eng { display: flex; align-items: center; gap: 6px; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--guaxi-border, #e6e9ee); cursor: pointer; }
.rh-eng.on { border-color: var(--rh-accent); background: color-mix(in srgb, var(--rh-accent) 8%, transparent); }
.rh-eng-label { font-size: 13px; }
.rh-eng-foreign { color: var(--guaxi-text-secondary); }
.rh-key-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 10px; }
.rh-key-row { display: flex; flex-direction: column; gap: 4px; }
.rh-key-label { font-size: 12px; color: var(--guaxi-text-secondary); }
.rh-dir-row { display: flex; gap: 8px; align-items: center; }
.rh-dir-row .n-input { flex: 1; }
/* 杂项 */
.spin { animation: rh-spin 1s linear infinite; }
@keyframes rh-spin { to { transform: rotate(360deg); } }
</style>
