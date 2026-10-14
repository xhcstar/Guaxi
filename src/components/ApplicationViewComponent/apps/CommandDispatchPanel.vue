<template>
  <div class="cd-root">
    <!-- ============ 顶栏：目标设备 + 配置来源 ============ -->
    <div class="cd-bar">
      <span class="cd-lbl">目标设备</span>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="选择 SSH 档案（或用 YAML 里的 global）" clearable style="width:230px" />
      </template>优先用库内 SSH 档案；留空则用配置里 global 段的 ip/username/password</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-input v-model:value="enablePassword" type="password" size="small" placeholder="enable 密码(可选)" show-password-on="click" style="width:160px" />
      </template>设备 CLI 进特权模式 `enable` 的口令。只有勾了档案才能存进 keychain</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" :disabled="!profileId || !enablePassword" @click="saveEnable"><Icon :icon="keyIcon" width="15" height="15" /></button>
      </template>把 enable 密码存入系统 keychain（对应所选档案）</n-tooltip>
      <span class="cd-sep"></span>
      <span class="cd-lbl">会话</span>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="sessionMode" size="small" :options="sessionModeOptions" style="width:130px" />
      </template>目标类型：自动探测 / 设备 CLI（enable + config terminal）/ Linux shell（编译机）。选错会导致"等待提示符超时"</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-checkbox v-model:checked="probeRc" size="small">退出码</n-checkbox>
      </template>shell 模式下给命令追加 `; echo RC=$?` 取真实退出码（比关键词猜测准）；命令若是交互式的请关掉</n-tooltip>
      <span class="cd-sep"></span>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="strategyId" size="small" :options="strategyOptions" placeholder="策略库" clearable style="width:170px" /></template>载入之前保存的策略（一组命令序列）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" :disabled="!strategyId" @click="loadStrategy"><Icon :icon="downloadIcon" width="15" height="15" /></button>
      </template>把选中的策略载入任务表</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib primary" :disabled="!tasks.length" @click="openSaveStrategy"><Icon :icon="saveIcon" width="15" height="15" /></button>
      </template>把当前任务表存成命名策略（凭据不入库）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib danger" :disabled="!strategyId" @click="deleteStrategy"><Icon :icon="trashIcon" width="15" height="15" /></button>
      </template>删除选中策略</n-tooltip>
      <span class="cd-sep"></span>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" @click="importYaml"><Icon :icon="folderIcon" width="15" height="15" /></button>
      </template>导入现有 config.yaml（global/targets/tasks 全兼容）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="scenarioId" size="small" :options="scenarioOptions" placeholder="内置场景模板" style="width:200px" />
      </template>选一个内置高级流程模板，填参数后一键生成整套任务</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib primary" :disabled="!scenarioId" @click="openScenario"><Icon :icon="wandIcon" width="15" height="15" /></button>
      </template>配置该场景的参数并生成任务</n-tooltip>
      <span v-if="notice" class="cd-notice">{{ notice }}</span>
      <span v-if="error" class="cd-err">{{ error }}</span>
    </div>

    <div class="cd-body">
      <!-- ============ 左：可视化任务表 ============ -->
      <div class="cd-left" :style="{ width: leftWidth + 'px' }">
        <div class="cd-head">
          <Icon :icon="listIcon" width="13" height="13" class="cd-head-ico" />
          <span>任务表</span>
          <span class="cd-n">{{ tasks.length }} 条</span>
          <n-radio-group v-model:value="editMode" size="tiny">
            <n-radio-button value="gui">图形</n-radio-button>
            <n-radio-button value="text">文本</n-radio-button>
          </n-radio-group>
          <template v-if="editMode === 'gui'">
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" @click="addTask"><Icon :icon="plusIcon" width="14" height="14" /></button>
            </template>新增一条任务</n-tooltip>
          </template>
          <template v-else>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib primary" @click="loadYaml"><Icon :icon="uploadIcon" width="15" height="15" /></button>
            </template>把文本解析回任务表（会覆盖当前任务表）</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" @click="exportYaml"><Icon :icon="saveIcon" width="15" height="15" /></button>
            </template>导出 config.yaml 到文件</n-tooltip>
          </template>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="openConfigExternally"><Icon :icon="externalIcon" width="15" height="15" /></button>
          </template>用系统默认程序（你的编辑器）打开这份 YAML。没有对应文件时会先存到受管目录，改完回来点上传图标重新载入</n-tooltip>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="revealConfig"><Icon :icon="revealIcon" width="15" height="15" /></button>
          </template>在系统文件管理器中显示这份 YAML 文件</n-tooltip>
        </div>
        <div class="cd-scroll" :class="{ 'cd-fill': editMode === 'text' }">
          <!-- 文本模式：Monaco 编辑 YAML（懒加载，切进这个模式才加载 monaco） -->
          <template v-if="editMode === 'text'">
            <div ref="yamlEditorEl" class="cd-yaml-monaco"></div>
            <div class="cd-hint">改完点上面的上传图标解析回任务表。字段与参考工具的 config.yaml 完全一致。{{ yamlEditorReady ? '' : '（编辑器加载中…）' }}</div>
          </template>

          <template v-else>
            <div class="cd-targets">
              <n-tooltip trigger="hover"><template #trigger>
                <n-input :value="hostnamesText" size="small" placeholder="目标主机名，逗号分隔" style="width:100%" @update:value="setHostnames" />
              </template>targets.hostnames：SSL 主机名等逻辑目标，逗号分隔</n-tooltip>
              <n-tooltip trigger="hover"><template #trigger>
                <n-input :value="domainsText" size="small" placeholder="域名，逗号分隔（可空 = 只做 host 级）" style="width:100%" @update:value="setDomains" />
              </template>targets.domains：域名列表；可留空（只跑 host 级任务）</n-tooltip>
            </div>
            <table class="cd-table">
              <thead><tr><th>id</th><th>命令</th><th>依赖</th><th>条件</th><th>PEM / index</th><th></th></tr></thead>
              <tbody>
                <tr v-for="(t, i) in tasks" :key="i">
                  <td><n-input v-model:value="t.id" size="small" placeholder="id" style="width:110px" @update:value="syncYaml" /></td>
                  <td class="cd-cmdcell">
                    <n-input
                      v-model:value="t.cmd"
                      type="textarea"
                      size="small"
                      :autosize="{ minRows: 1, maxRows: 6 }"
                      placeholder="命令（支持 {hostname}/{domain}/{index}）"
                      @update:value="syncYaml"
                    />
                    <span v-if="t.description" class="cd-desc">{{ t.description }}</span>
                  </td>
                  <td><n-input :value="(t.depends_on || []).join(',')" size="small" placeholder="依赖 id" style="width:140px" @update:value="v => setDeps(t, v)" /></td>
                  <td class="cd-cond">
                    <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_no_domain" size="small" @update:checked="v => { t.only_if_no_domain = v; if (v) t.only_if_domain = false; syncYaml() }">host</n-checkbox></template>只在 host 级（无域名）执行</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_domain" size="small" @update:checked="v => { t.only_if_domain = v; if (v) t.only_if_no_domain = false; syncYaml() }">sni</n-checkbox></template>只在关联了域名时执行（SNI 流程）</n-tooltip>
                  </td>
                  <td class="cd-pemcell">
                    <div class="cd-row">
                      <n-input v-model:value="t.pem_file" size="small" placeholder="PEM 文件（可空）" style="flex:1" @update:value="syncYaml" />
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" @click="openPemPicker(t)"><Icon :icon="fileIcon" width="15" height="15" /></button>
                      </template>选择要导入的证书/私钥文件：可从证书页的 CA 库 / 终端证书库直接取，也可选任意本地文件</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" :disabled="!t.pem_file" @click="openFileExternally(t.pem_file)"><Icon :icon="externalIcon" width="15" height="15" /></button>
                      </template>用系统默认程序打开这个证书/私钥文件</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" :disabled="!t.pem_file" @click="revealFile(t.pem_file)"><Icon :icon="revealIcon" width="15" height="15" /></button>
                      </template>在系统文件管理器中显示这个文件</n-tooltip>
                    </div>
                    <n-input v-model:value="t.index" size="small" placeholder="index" style="width:80px;margin-top:4px" @update:value="syncYaml" />
                  </td>
                  <td class="cd-ops">
                    <n-tooltip trigger="hover"><template #trigger>
                      <button class="tb-ib" @click="openCatalogFor(t)"><Icon :icon="bookIcon" width="15" height="15" /></button>
                    </template>从命令目录给这条选个命令模板</n-tooltip>
                    <n-tooltip trigger="hover"><template #trigger>
                      <button class="tb-ib danger" @click="removeTask(i)"><Icon :icon="trashIcon" width="15" height="15" /></button>
                    </template>删除该任务</n-tooltip>
                  </td>
                </tr>
                <tr v-if="!tasks.length"><td colspan="6" class="cd-hint">还没有任务。可「导入 config.yaml」、从「内置场景模板」生成，或点左上角 + 手动加一条。</td></tr>
              </tbody>
            </table>
          </template>
        </div>
      </div>

      <div class="cd-vresize" @mousedown="onLeftResize"></div>

      <!-- ============ 右：执行 ============ -->
      <div class="cd-right">
        <div class="cd-head">
          <Icon :icon="playIcon" width="13" height="13" class="cd-head-ico" />
          <span>执行</span>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" :disabled="running" @click="doPlan"><Icon :icon="listIcon" width="14" height="14" /></button>
          </template>预览：展开 targets 并做依赖排序，列出将要逐条执行的命令（不下发）</n-tooltip>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib primary" :disabled="running || !tasks.length" @click="doRun"><Icon :icon="playIcon" width="15" height="15" /></button>
          </template>开始下发（失败即停，除非配置里 continue_on_error）</n-tooltip>
          <n-tooltip v-if="running" trigger="hover"><template #trigger>
            <button class="tb-ib danger" @click="doCancel"><Icon :icon="stopIcon" width="15" height="15" /></button>
          </template>取消本次下发</n-tooltip>
          <span class="cd-stat" v-if="runStat">{{ runStat }}</span>
          <n-tooltip v-if="runLogPath" trigger="hover"><template #trigger>
            <button class="tb-ib" @click="openFileExternally(runLogPath)"><Icon :icon="externalIcon" width="15" height="15" /></button>
          </template>用系统默认程序打开本次运行的 JSONL 日志</n-tooltip>
          <n-tooltip v-if="runLogPath" trigger="hover"><template #trigger>
            <button class="tb-ib" @click="revealFile(runLogPath)"><Icon :icon="revealIcon" width="15" height="15" /></button>
          </template>在系统文件管理器中显示本次运行的日志文件</n-tooltip>
          <span v-if="notice" class="cd-notice">{{ notice }}</span>
          <span v-if="error" class="cd-err">{{ error }}</span>
        </div>
        <div class="cd-scroll">
          <table class="cd-table" v-if="steps.length">
            <thead><tr><th>#</th><th>状态</th><th>命令</th><th>说明</th></tr></thead>
            <tbody>
              <tr v-for="(s, i) in steps" :key="s.key" :class="'cd-' + s.status">
                <td>{{ i + 1 }}</td>
                <td class="cd-st">{{ statusLabel(s.status) }}</td>
                <td class="cd-cmd" :title="s.cmd">{{ s.cmd }}</td>
                <td class="cd-reason" :title="s.reason">{{ s.reason }}</td>
              </tr>
            </tbody>
          </table>
          <div v-else class="cd-hint">点右侧「预览」看展开后的执行计划，或直接「运行」。</div>
          <div class="cd-warns" v-if="warnings.length">
            <div v-for="(w, i) in warnings" :key="i">⚠ {{ w }}</div>
          </div>
          <div class="cd-out" v-if="lastOutput">
            <div class="cd-out-head">最近一次命令输出</div>
            <pre class="cd-pre">{{ lastOutput }}</pre>
          </div>
        </div>
      </div>
    </div>

    <!-- 场景参数弹窗 -->
    <n-modal v-model:show="showScenario" :mask-closable="false" style="width:680px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="wandIcon" width="16" height="16" class="cd-head-ico" />
          <span>{{ currentScenarioName }}</span>
          <button class="tb-ib" @click="showScenario = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b">
          <div class="cd-hint">{{ scenarioSummary }}</div>
          <div v-for="f in scenarioParams" :key="f.name" class="cd-field">
            <label>{{ f.name }}<span v-if="f.required" class="cd-req">*</span></label>
            <n-input v-model:value="scenarioValues[f.name]" size="small" :placeholder="f.example" />
            <span class="cd-doc">{{ f.doc }}</span>
          </div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showScenario = false">取消</n-button>
          <n-button size="small" type="primary" @click="renderScenario">生成任务</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 策略保存弹窗 -->
    <n-modal v-model:show="showSaveStrategy" :mask-closable="false" style="width:520px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="saveIcon" width="16" height="16" class="cd-head-ico" />
          <span>保存为策略</span>
          <button class="tb-ib" @click="showSaveStrategy = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b">
          <div class="cd-field">
            <label>策略名 *</label>
            <n-input v-model:value="strategyName" size="small" placeholder="如 ssl-链-ECC-生产" />
          </div>
          <div class="cd-field">
            <label>说明</label>
            <n-input v-model:value="strategyDesc" size="small" placeholder="这条策略是干什么的（可空）" />
          </div>
          <div class="cd-hint">保存 targets + 全部任务（含映射后的命令与 PEM 路径）。凭据（ip/密码/enable）不会存进策略。</div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showSaveStrategy = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveStrategy">保存</n-button>
        </div>
      </div>
    </n-modal>

    <!-- PEM 文件选择弹窗 -->
    <n-modal v-model:show="showPem" :mask-closable="false" style="width:760px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="fileIcon" width="16" height="16" class="cd-head-ico" />
          <span>选择要导入的文件（PEM）</span>
          <button class="tb-ib" @click="showPem = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b">
          <div class="cd-row" style="margin-bottom:10px">
            <n-button size="small" @click="pickLocalPem"><Icon :icon="folderIcon" width="14" height="14" />从任意位置选文件</n-button>
            <span class="cd-hint">选中后会落到受管目录并填进该任务的 PEM 路径</span>
          </div>
          <div class="cd-sub">证书页 · CA 库</div>
          <table class="cd-table" v-if="caList.length">
            <tbody>
              <tr v-for="c in caList" :key="c.id">
                <td>{{ c.name }}</td>
                <td class="cd-res">{{ c.subject_cn }}</td>
                <td class="cd-ops">
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="pickCaPem(c,'cert')"><Icon :icon="certIcon" width="15" height="15" /></button></template>用该 CA 的证书</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="pickCaPem(c,'key')"><Icon :icon="key2Icon" width="15" height="15" /></button></template>用该 CA 的私钥</n-tooltip>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-else class="cd-hint">CA 库为空（可在工具箱 → 证书/密钥 里生成或导入）。</div>

          <div class="cd-sub">证书页 · 终端证书库（client / server）</div>
          <table class="cd-table" v-if="leafList.length">
            <tbody>
              <tr v-for="l in leafList" :key="l.id">
                <td>{{ l.name }}</td>
                <td class="cd-res">{{ l.ca_name }}</td>
                <td class="cd-ops">
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="pickLeafPem(l,'cert')"><Icon :icon="certIcon" width="15" height="15" /></button></template>用该证书</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="pickLeafPem(l,'key')"><Icon :icon="key2Icon" width="15" height="15" /></button></template>用该私钥</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger><button class="tb-ib" @click="pickLeafPem(l,'chain')"><Icon :icon="listIcon" width="15" height="15" /></button></template>用整条证书链</n-tooltip>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-else class="cd-hint">终端证书库为空（可在证书页签发 server/client 证书）。</div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showPem = false">关闭</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 命令目录弹窗 -->
    <n-modal v-model:show="showCatalog" :mask-closable="false" style="width:860px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="bookIcon" width="16" height="16" class="cd-head-ico" />
          <span>命令目录（SSL / SLB）</span>
          <n-input v-model:value="catalogQuery" size="small" placeholder="搜索命令或说明" clearable style="width:220px" />
          <button class="tb-ib" @click="showCatalog = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b cd-cat">
          <div class="cd-cat-list">
            <template v-for="g in catalogGroups" :key="g.group">
              <div class="cd-cat-g">{{ g.group }}</div>
              <div v-for="c in g.items" :key="c.id" class="cd-cat-item" :class="{ active: picked && picked.id === c.id }" @click="pick(c)">
                <span class="cd-cat-name">{{ c.name }}</span>
                <span class="cd-cat-syn">{{ c.syntax }}</span>
              </div>
            </template>
          </div>
          <div class="cd-cat-detail" v-if="picked">
            <div class="cd-cat-title">{{ picked.name }}
              <span v-if="picked.pem" class="cd-tag">需要 PEM</span>
            </div>
            <div class="cd-cat-sum">{{ picked.summary }}</div>
            <div class="cd-cat-syntax">{{ picked.syntax }}</div>
            <div v-for="(pp, i) in picked.params" :key="i" class="cd-field">
              <label :class="{ 'cd-req-l': pp.required }">{{ pp.name }}</label>
              <n-input v-model:value="pickedValues[pp.name]" size="small" :placeholder="pp.example" />
              <span class="cd-doc">{{ pp.doc }}</span>
            </div>
            <div class="cd-row">
              <n-button size="small" @click="insertCatalog(false)">追加到任务表</n-button>
              <n-button size="small" type="primary" @click="insertCatalog(true)">替换选中任务</n-button>
            </div>
            <div class="cd-hint">生成的命令会把填的参数带进去；占位符 {{ '{hostname}' }} / {{ '{domain}' }} / {{ '{index}' }} 保留给 targets 展开时替换。</div>
          </div>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { useSettingsStore } from '../../../stores/settingsStore'
import { getGuaxiMonacoTheme } from '../../../monaco/guaxiMonacoThemes'
import { Icon } from '@iconify/vue'
import keyIcon from '@iconify-icons/lucide/key-round'
import folderIcon from '@iconify-icons/lucide/folder-open'
import wandIcon from '@iconify-icons/lucide/wand-2'
import helpIcon from '@iconify-icons/lucide/help-circle'
import listIcon from '@iconify-icons/lucide/list'
import plusIcon from '@iconify-icons/lucide/plus'
import fileIcon from '@iconify-icons/lucide/file-code'
import bookIcon from '@iconify-icons/lucide/book-open'
import trashIcon from '@iconify-icons/lucide/trash-2'
import playIcon from '@iconify-icons/lucide/play'
import stopIcon from '@iconify-icons/lucide/square'
import uploadIcon from '@iconify-icons/lucide/upload'
import saveIcon from '@iconify-icons/lucide/save'
import downloadIcon from '@iconify-icons/lucide/download'
import xIcon from '@iconify-icons/lucide/x'
import scrollTextIcon from '@iconify-icons/lucide/scroll-text'
import keyRoundIcon from '@iconify-icons/lucide/key-round'
import externalIcon from '@iconify-icons/lucide/external-link'

// 证书图标沿用与证书页一致的语义（证书=卷轴、私钥=钥匙）。
const certIcon = scrollTextIcon
const key2Icon = keyRoundIcon
// 「在文件管理器中显示」沿用打开目录的图标。
const revealIcon = folderIcon

// ============ 状态 ============
const profiles = ref([])
const profileOptions = computed(() => profiles.value.map(p => ({ label: `${p.name || p.host}`, value: p.id })))
const profileId = ref(null)
const enablePassword = ref('')

const cfg = ref({ global: {}, targets: { hostnames: [], domains: [] }, tasks: [], continue_on_error: false })
const yamlText = ref('')
/// 编辑方式：图形任务表 / 文本 YAML。
const editMode = ref('gui')
const baseDir = ref('')

/// 会话类型与退出码探针（选错会话类型会"等待提示符超时"）。
const sessionMode = ref('auto')
const sessionModeOptions = [
  { label: '自动探测', value: 'auto' },
  { label: '设备 CLI', value: 'device' },
  { label: 'Linux shell', value: 'shell' },
]
const probeRc = ref(true)

/// 策略库。
const strategies = ref([])
const strategyId = ref(null)
const showSaveStrategy = ref(false)
const strategyName = ref('')
const strategyDesc = ref('')
const strategyOptions = computed(() => strategies.value.map(s => ({ label: `${s.name} (${s.task_count})`, value: s.id })))

/// PEM 选择器（可从证书页的 CA 库 / 终端证书库取，也可选任意本地文件）。
const showPem = ref(false)
const pemTarget = ref(null)
const caList = ref([])
const leafList = ref([])

const catalog = ref([])
const catalogQuery = ref('')
const showCatalog = ref(false)
const picked = ref(null)
const pickedValues = ref({})
const catalogTarget = ref(-1)   // 替换哪条任务；-1 = 追加

const scenarios = ref([])
const scenarioId = ref(null)
const showScenario = ref(false)
const scenarioValues = ref({})
const scenarioTarget = ref(null)

const steps = ref([])
const warnings = ref([])
const running = ref(false)
const runId = ref('')
const runStat = ref('')
const lastOutput = ref('')
/// 本次运行的 JSONL 日志路径（后端返回，供外部打开/定位）。
const runLogPath = ref('')
/// 当前这份 YAML 对应的文件路径（导入过 config.yaml 就是它；否则首次"外部打开"时落受管目录）。
const configPath = ref('')
let unlisteners = []

const leftWidth = ref(560)
const notice = ref(''); let noticeTimer = null
const error = ref('')
function note(m) { notice.value = m; if (noticeTimer) clearTimeout(noticeTimer); noticeTimer = setTimeout(() => { notice.value = '' }, 12000) }

const tasks = computed(() => cfg.value.tasks || [])
const hostnamesText = computed(() => (cfg.value.targets?.hostnames || []).join(','))
const domainsText = computed(() => (cfg.value.targets?.domains || []).join(','))
function setHostnames(v) { cfg.value.targets.hostnames = v.split(',').map(s => s.trim()).filter(Boolean); syncYaml() }
// 域名列表只放真实域名即可：host 级任务靠 only_if_no_domain 强制 domain=""，不依赖这里有没有空项。
function setDomains(v) { cfg.value.targets.domains = v.split(',').map(s => s.trim()).filter(Boolean); syncYaml() }
function setDeps(t, v) { t.depends_on = String(v).split(',').map(s => s.trim()).filter(Boolean); syncYaml() }

// ============ Monaco（YAML 文本模式）============
//
// 加载策略：**懒加载**。只有切到「文本」模式才 dynamic import monaco 与 yaml 语法贡献，
// 不打开这个模式就完全不加载（monaco 是 MB 级依赖）。加载是异步的，中途可能组件已卸载
// 或用户又切回图形模式，所以用 disposed / 模式判断做闸门，避免"已销毁还去 create"。
const settingsStore = useSettingsStore()
const yamlEditorEl = ref(null)
const yamlEditorReady = ref(false)
let monacoNs = null            // import('monaco-editor') 的结果
let yamlContributionLoaded = false
let yamlEditor = null          // monaco editor 实例
let yamlModel = null           // 独立的 model（dispose 时一并释放）
let yamlChangeSub = null       // onDidChangeModelContent 的 disposable
let creating = false
let disposed = false
/// 程序化写入 model 时置位，避免与 watch 互相触发成环。
let pushingToModel = false

async function loadMonaco() {
  if (monacoNs) return monacoNs
  const tasks = [import('monaco-editor')]
  if (!yamlContributionLoaded) {
    // YAML 是 basic-language（Monarch 分词），无需独立 worker，但要显式加载它的贡献点才有高亮。
    tasks.push(
      import('monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution').then(() => {
        yamlContributionLoaded = true
      }),
    )
  }
  const [m] = await Promise.all(tasks)
  monacoNs = m
  return m
}

async function ensureYamlEditor() {
  if (yamlEditor || creating || disposed) return
  creating = true
  try {
    const m = await loadMonaco()
    // 加载是异步的：回来时可能已卸载、或已切回图形模式、或容器还没渲染出来。
    if (disposed || editMode.value !== 'text' || !yamlEditorEl.value) return
    yamlModel = m.editor.createModel(yamlText.value || '', 'yaml')
    yamlEditor = m.editor.create(yamlEditorEl.value, {
      model: yamlModel,
      theme: getGuaxiMonacoTheme(settingsStore.appearanceTheme),
      automaticLayout: true,     // 容器尺寸变化（拖分隔条、切标签）自动重排
      fontSize: 13,
      lineNumbers: 'on',
      tabSize: 2,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      wordWrap: 'on',
      renderWhitespace: 'selection',
      fixedOverflowWidgets: true,
      scrollbar: { alwaysConsumeMouseWheel: false },
    })
    yamlChangeSub = yamlEditor.onDidChangeModelContent(() => {
      if (pushingToModel || !yamlModel) return
      yamlText.value = yamlModel.getValue()
    })
    yamlEditorReady.value = true
    await nextTick()
    yamlEditor.layout()
  } catch (e) {
    error.value = '加载 YAML 编辑器失败: ' + (e?.message || e)
  } finally {
    creating = false
  }
}

function disposeYamlEditor() {
  yamlChangeSub?.dispose?.()
  yamlChangeSub = null
  // 先销毁 editor 再销毁 model（顺序反了 monaco 会告警/残留）。
  try { yamlEditor?.dispose() } catch (_) {}
  try { yamlModel?.dispose() } catch (_) {}
  yamlEditor = null
  yamlModel = null
  yamlEditorReady.value = false
}

// 切模式：进文本模式才创建，离开就立刻释放，不把 monaco 实例挂在后台。
watch(editMode, async (v) => {
  if (v === 'text') { await nextTick(); ensureYamlEditor() } else { disposeYamlEditor() }
})

// 外部改写了 yamlText（场景生成/导入/策略载入/图形模式编辑）：同步进编辑器。
// 用 setValue 会清掉撤销栈，但那几件事本来就是"换掉整份配置"，语义上正确。
watch(yamlText, (v) => {
  if (!yamlModel || pushingToModel) return
  if (yamlModel.getValue() !== v) {
    pushingToModel = true
    try { yamlModel.setValue(v || '') } finally { pushingToModel = false }
  }
})

// 明暗主题跟随应用外观（setTheme 是全局的，与仓库里其它 monaco 用法一致）。
watch(() => settingsStore.appearanceTheme, (t) => {
  if (yamlEditor && monacoNs) monacoNs.editor.setTheme(getGuaxiMonacoTheme(t))
})

// ============ YAML 双向同步（YAML 只由后端解析，前端不引 YAML 库） ============
let syncing = false
async function syncYaml() {
  if (syncing) return
  syncing = true
  try { yamlText.value = await invoke('cmd_dispatch_to_yaml', { config: cfg.value }) }
  catch (e) { error.value = '生成 YAML 失败: ' + (e?.message || e) }
  finally { syncing = false }
}
async function loadYaml() {
  try {
    const c = await invoke('cmd_dispatch_parse', { configYaml: yamlText.value })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    note('已从 YAML 载入任务表')
  } catch (e) { error.value = 'YAML 解析失败: ' + (e?.message || e) }
}

// ============ 策略库 ============
async function loadStrategies() {
  try { strategies.value = await invoke('cmd_strategy_list') } catch (_) {}
}
async function loadStrategy() {
  if (!strategyId.value) return
  try {
    yamlText.value = await invoke('cmd_strategy_load', { id: strategyId.value })
    await loadYaml()
    const s = strategies.value.find(x => x.id === strategyId.value)
    note(`已载入策略「${s?.name || ''}」（${tasks.value.length} 条任务）`)
  } catch (e) { error.value = '载入策略失败: ' + (e?.message || e) }
}
function openSaveStrategy() {
  const s = strategies.value.find(x => x.id === strategyId.value)
  strategyName.value = s?.name || ''
  strategyDesc.value = s?.description || ''
  showSaveStrategy.value = true
}
async function saveStrategy() {
  if (!strategyName.value.trim()) { error.value = '策略名不能为空'; return }
  try {
    await syncYaml()
    const r = await invoke('cmd_strategy_save', {
      id: strategyId.value || null,
      name: strategyName.value,
      description: strategyDesc.value || null,
      configYaml: yamlText.value,
    })
    await loadStrategies()
    strategyId.value = r.id
    showSaveStrategy.value = false
    note(`策略「${r.name}」已保存（${r.task_count} 条任务）`)
  } catch (e) { error.value = '保存策略失败: ' + (e?.message || e) }
}
async function deleteStrategy() {
  if (!strategyId.value) return
  const s = strategies.value.find(x => x.id === strategyId.value)
  if (!window.confirm(`删除策略「${s?.name || ''}」？`)) return
  try { await invoke('cmd_strategy_delete', { id: strategyId.value }); strategyId.value = null; await loadStrategies(); note('策略已删除') }
  catch (e) { error.value = '删除策略失败: ' + (e?.message || e) }
}

// ============ PEM 文件选择 ============
async function openPemPicker(t) {
  pemTarget.value = t
  showPem.value = true
  try { caList.value = await invoke('crypto_ca_list') } catch (_) {}
  try { leafList.value = await invoke('crypto_leaf_list') } catch (_) {}
}
async function stagePem(name, content) {
  try {
    const p = await invoke('cmd_dispatch_stage_pem', { name, content })
    if (pemTarget.value) { pemTarget.value.pem_file = p; syncYaml() }
    showPem.value = false
    note('已选用: ' + p)
  } catch (e) { error.value = '落盘失败: ' + (e?.message || e) }
}
async function pickCaPem(c, which) {
  try {
    const r = await invoke('crypto_ca_export', { id: c.id })
    await stagePem(`ca-${c.name}-${which}`, which === 'key' ? r.key_pem : r.cert_pem)
  } catch (e) { error.value = '取 CA 内容失败: ' + (e?.message || e) }
}
async function pickLeafPem(l, which) {
  try {
    const r = await invoke('crypto_leaf_export', { id: l.id })
    const content = which === 'key' ? r.key_pem : which === 'chain' ? r.chain_pem : r.cert_pem
    await stagePem(`leaf-${l.name}-${which}`, content)
  } catch (e) { error.value = '取证书内容失败: ' + (e?.message || e) }
}
async function pickLocalPem() {
  const sel = await open({ multiple: false, directory: false, title: '选择证书/私钥文件' })
  if (!sel) return
  const p = typeof sel === 'string' ? sel : sel?.path
  if (pemTarget.value) { pemTarget.value.pem_file = p; syncYaml() }
  showPem.value = false
  note('已选用: ' + p)
}

async function importYaml() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false, title: '选择 config.yaml', filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }] })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    yamlText.value = await readTextFile(path)
    // PEM 相对路径以该配置文件所在目录为基准；同时记住文件本身，供「用外部编辑器打开」。
    baseDir.value = String(path).replace(/\\/g, '/').split('/').slice(0, -1).join('/')
    configPath.value = path
    const c = await invoke('cmd_dispatch_parse', { configYaml: yamlText.value })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    if (cfg.value.global?.ip && !profileId.value) note(`配置里带了 global.ip=${cfg.value.global.ip}，未选档案时将用它直连`)
    note(`已导入 ${String(path).replace(/\\/g, '/').split('/').pop()}`)
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
async function exportYaml() {
  try {
    await syncYaml()
    const p = await save({ defaultPath: 'config.yaml', filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }] })
    if (p) { await invoke('write_file_content', { path: p, content: yamlText.value }); configPath.value = p; note('已导出: ' + p) }
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}

// ============ 用外部编辑器打开 / 在文件管理器中显示 ============
//
// 相对路径按"导入 config.yaml 的目录"解析（与执行时的 PEM 解析规则一致），
// 否则把相对路径交给系统也打不开。绝对路径（含盘符 / UNC / POSIX）直接用。
function resolvePath(p) {
  const s = String(p || '').trim()
  if (!s) return ''
  if (/^[A-Za-z]:[\\/]/.test(s) || s.startsWith('/') || s.startsWith('\\\\')) return s
  if (baseDir.value) return baseDir.value.replace(/[\\/]+$/, '') + '/' + s.replace(/^\.\//, '')
  return ''
}
async function openFileExternally(p) {
  const abs = resolvePath(p)
  if (!abs) {
    error.value = `无法确定该文件的绝对路径（${p}）。相对路径需要先「导入 config.yaml」以确定基准目录，或直接填绝对路径。`
    return
  }
  try { await invoke('cmd_open_in_os', { path: abs }) } catch (e) { error.value = '用外部程序打开失败: ' + (e?.message || e) }
}
async function revealFile(p) {
  const abs = resolvePath(p)
  if (!abs) { error.value = `无法定位：${p} 不是可解析的绝对路径（相对路径请先导入 config.yaml）。`; return }
  try { await invoke('cmd_reveal_in_os', { path: abs }) } catch (e) { error.value = '在文件管理器中显示失败: ' + (e?.message || e) }
}
/// 保证当前 YAML 有一个磁盘文件：已导入/导出过就用它，否则落到受管目录（固定文件名）。
async function ensureConfigFile() {
  if (configPath.value) return configPath.value
  await syncYaml()
  configPath.value = await invoke('cmd_dispatch_stage_config', { content: yamlText.value })
  return configPath.value
}
async function openConfigExternally() {
  error.value = ''
  try {
    const p = await ensureConfigFile()
    await invoke('cmd_open_in_os', { path: p })
    note('已用外部程序打开（' + p + '）；改完回来点上传图标重新载入')
  } catch (e) { error.value = '打开配置失败: ' + (e?.message || e) }
}
async function revealConfig() {
  error.value = ''
  try { await invoke('cmd_reveal_in_os', { path: await ensureConfigFile() }) }
  catch (e) { error.value = '定位配置失败: ' + (e?.message || e) }
}

// ============ 任务表 ============
function addTask() {
  cfg.value.tasks.push({ id: 'task' + (tasks.value.length + 1), cmd: '', pem_file: '', index: '', depends_on: [], only_if_domain: false, only_if_no_domain: false })
  syncYaml()
}
function removeTask(i) { cfg.value.tasks.splice(i, 1); syncYaml() }

// ============ 命令目录 ============
const catalogGroups = computed(() => {
  const q = catalogQuery.value.trim().toLowerCase()
  const hit = catalog.value.filter(c => !q || c.name.toLowerCase().includes(q) || c.summary.toLowerCase().includes(q) || c.group.toLowerCase().includes(q))
  const m = new Map()
  for (const c of hit) { if (!m.has(c.group)) m.set(c.group, []); m.get(c.group).push(c) }
  return Array.from(m.entries()).map(([group, items]) => ({ group, items }))
})
function openCatalogFor(t) {
  catalogTarget.value = cfg.value.tasks.indexOf(t)
  showCatalog.value = true
  if (t.cmd) { /* 保留当前内容，用户选完再替换 */ }
}
function pick(c) {
  picked.value = c
  pickedValues.value = {}
  for (const pp of c.params) if (pp.example && !/^\d+$/.test(pp.example)) pickedValues.value[pp.name] = ''
}
function buildFromCatalog(c, vals) {
  // 用模板 + 用户填的参数拼出命令；占位符保留。
  let cmd = c.cmd_template
  for (const [k, v] of Object.entries(vals)) {
    if (!v) continue
    const re = new RegExp('\\{' + k + '\\}', 'g')
    cmd = cmd.replace(re, v)
  }
  return cmd
}
function insertCatalog(replace) {
  const c = picked.value
  if (!c) return
  const cmd = buildFromCatalog(c, pickedValues.value)
  const deps = []
  if (replace && catalogTarget.value >= 0) {
    const t = cfg.value.tasks[catalogTarget.value]
    t.id = c.id
    t.cmd = cmd
    if (c.pem) t.pem_file = t.pem_file || './cfgfiles/'
    if (c.id.includes('activate')) t.index = t.index || '1'
    syncYaml()
  } else {
    cfg.value.tasks.push({ id: c.id, cmd, pem_file: c.pem ? './cfgfiles/' : '', index: '', depends_on: deps, only_if_domain: false, only_if_no_domain: false })
    syncYaml()
  }
  showCatalog.value = false
}

// ============ 场景模板 ============
const scenarioOptions = computed(() => scenarios.value.map(s => ({ label: s.name, value: s.id })))
const currentScenario = computed(() => scenarios.value.find(s => s.id === scenarioId.value) || null)
const currentScenarioName = computed(() => currentScenario.value?.name || '场景参数')
const scenarioSummary = computed(() => currentScenario.value?.summary || '')
const scenarioParams = computed(() => currentScenario.value?.params || [])
function openScenario() {
  scenarioValues.value = {}
  for (const pp of scenarioParams.value) scenarioValues.value[pp.name] = pp.example && !/^\d+$/.test(pp.example) && !pp.example.startsWith('http') ? pp.example : (pp.example || '')
  showScenario.value = true
}
async function renderScenario() {
  try {
    const y = await invoke('cmd_scenario_render', {
      id: scenarioId.value,
      opts: {
        ip: cfg.value.global?.ip || null,
        port: cfg.value.global?.port || 22,
        username: cfg.value.global?.username || null,
        password: cfg.value.global?.password || null,
        enablePassword: enablePassword.value || null,
        params: scenarioValues.value,
      },
    })
    yamlText.value = y
    const c = await invoke('cmd_dispatch_parse', { configYaml: y })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    showScenario.value = false
    note(`已按场景「${currentScenarioName.value}」生成 ${tasks.value.length} 条任务`)
  } catch (e) { error.value = '生成场景失败: ' + (e?.message || e) }
}

// ============ 执行 ============
function statusLabel(s) {
  return { pending: '待执行', running: '执行中', ok: '成功', 'ok-idem': '幂等', fail: '失败', skipped: '跳过' }[s] || s
}
function resetSteps(planSteps) {
  steps.value = planSteps.map(t => ({ key: t.task_key || t.taskKey, cmd: t.cmd, status: 'pending', reason: '' }))
}
async function doPlan() {
  error.value = ''
  try {
    await syncYaml()
    const plan = await invoke('cmd_dispatch_plan', { configYaml: yamlText.value })
    resetSteps(plan.steps)
    warnings.value = plan.warnings || []
    runStat.value = `共 ${plan.task_count} 条（${plan.target_count} 个目标）`
  } catch (e) { error.value = '预览失败: ' + (e?.message || e) }
}
async function doRun() {
  error.value = ''
  await syncYaml()
  try {
    const plan = await invoke('cmd_dispatch_plan', { configYaml: yamlText.value })
    resetSteps(plan.steps)
    warnings.value = plan.warnings || []
  } catch (e) { error.value = '配置有问题: ' + (e?.message || e); return }
  runId.value = 'run-' + Date.now()
  lastOutput.value = ''
  runStat.value = `执行中 0/${steps.value.length}`
  running.value = true
  try {
    // base_dir 让 PEM 相对路径以导入的 config.yaml 所在目录为基准。
    const r = await invoke('cmd_dispatch_run', {
      runId: runId.value,
      profileId: profileId.value || null,
      configYaml: yamlText.value,
      opts: {
        baseDir: baseDir.value || null,
        continueOnError: null,
        mode: sessionMode.value,
        probeRc: probeRc.value,
      },
    })
    runLogPath.value = r.log_path || ''
    runStat.value = `执行中 0/${r.plan.task_count}`
  } catch (e) { error.value = '下发失败: ' + (e?.message || e); running.value = false }
}
async function doCancel() {
  if (runId.value) { await invoke('cmd_dispatch_cancel', { runId: runId.value }).catch(() => {}); note('已请求取消') }
}
function onTask(ev) {
  const p = ev.payload || {}
  if (p.runId !== runId.value) return
  const i = p.index
  if (steps.value[i]) {
    steps.value[i].status = p.status
    if (p.reason) steps.value[i].reason = p.reason
    if (!steps.value[i].cmd && p.cmd) steps.value[i].cmd = p.cmd
  }
  if (p.output) lastOutput.value = p.output
  const done = steps.value.filter(s => s.status !== 'pending' && s.status !== 'running').length
  runStat.value = `执行中 ${done}/${steps.value.length}`
}
function onDone(ev) {
  const p = ev.payload || {}
  if (p.runId !== runId.value) return
  running.value = false
  if (p.error) { error.value = '连接/执行失败: ' + p.error; runStat.value = ''; return }
  runStat.value = `${p.ok ? '全部完成' : '结束'}：成功 ${p.done - p.failed}/${p.total}${p.failed ? `，失败 ${p.failed}` : ''}${p.cancelled ? '（已取消）' : ''}`
}

// ============ 生命周期 ============
async function saveEnable() {
  if (!profileId.value || !enablePassword.value) return
  try { await invoke('ssh_set_enable_secret', { profileId: profileId.value, secret: enablePassword.value }); note('enable 密码已存入 keychain') }
  catch (e) { error.value = '保存失败: ' + (e?.message || e) }
}
function onLeftResize(e) {
  const startX = e.clientX; const startW = leftWidth.value
  let on = true
  const mv = ev => { if (on) leftWidth.value = Math.min(1000, Math.max(380, startW + ev.clientX - startX)) }
  const up = () => { on = false; document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up)
}
onMounted(async () => {
  try { profiles.value = await invoke('ssh_list_profiles') } catch (_) {}
  try { catalog.value = await invoke('cmd_catalog_list') } catch (_) {}
  try { scenarios.value = await invoke('cmd_scenario_list') } catch (_) {}
  unlisteners.push(await listen('cmd-dispatch-task', onTask))
  unlisteners.push(await listen('cmd-dispatch-done', onDone))
  await syncYaml()
  loadStrategies()
})
onBeforeUnmount(() => {
  unlisteners.forEach(u => u())
  if (noticeTimer) clearTimeout(noticeTimer)
  // 先置 disposed 再释放：正在 await 的 ensureYamlEditor 回来后会看到它并直接退出，
  // 不会在组件销毁后再去 create（那会留下永远无人释放的 editor/model）。
  disposed = true
  disposeYamlEditor()
})
</script>

<style scoped>
.cd-root { height: 100%; display: flex; flex-direction: column; min-height: 0; font-family: var(--tw-ui-font); }
.cd-bar { display: flex; align-items: center; gap: 8px; padding: 8px 14px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); }
.cd-lbl { font-size: 12px; color: var(--guaxi-text-secondary); }
.cd-sep { width: 1px; height: 18px; background: var(--guaxi-border); margin: 0 4px; }
.cd-notice { font-size: 12px; color: #f5a524; }
.cd-err { font-size: 12px; color: #e5484d; }
.cd-stat { font-size: 12px; color: var(--tw-accent); font-family: var(--tw-mono-font); }

.cd-body { flex: 1 1 auto; min-height: 0; display: flex; }
.cd-left { flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; border-right: 1px solid var(--guaxi-border); }
.cd-right { flex: 1 1 auto; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
.cd-vresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; }
.cd-vresize:hover { opacity: 1; background: var(--tw-accent); }

.cd-head { flex: 0 0 auto; display: flex; align-items: center; gap: 6px; padding: 6px 10px; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.08)); color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); }
.cd-head-ico { color: var(--tw-accent); }
.cd-n { font-size: 10px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.cd-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 8px 10px; }

.cd-targets { display: flex; gap: 6px; margin-bottom: 8px; }
.cd-table { width: 100%; border-collapse: collapse; font-size: 13px; }
.cd-table th, .cd-table td { text-align: left; padding: 8px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); vertical-align: top; }
.cd-table th { font-size: 12px; text-transform: uppercase; letter-spacing: .04em; color: var(--guaxi-text-secondary); position: sticky; top: 0; background: var(--guaxi-panel-bg); z-index: 1; }
.cd-cond { white-space: nowrap; }
.cd-ops { white-space: nowrap; }
/* 命令可能很长：给足宽度，并用多行输入让整条命令都看得见 */
.cd-cmdcell { min-width: 380px; }
.cd-cmdcell :deep(.n-input__textarea-el), .cd-cmdcell :deep(.n-input__input-el) { font-family: var(--tw-mono-font); font-size: 13px; line-height: 1.5; }
.cd-pemcell { min-width: 260px; }
.cd-desc { display: block; margin-top: 3px; font-size: 11.5px; color: var(--guaxi-text-secondary); }
.cd-res { font-family: var(--tw-mono-font); font-size: 12px; }
.cd-sub { margin: 12px 0 4px; font-size: 12.5px; font-weight: 600; color: var(--tw-accent); }
.cd-yaml-monaco { flex: 1 1 auto; min-height: 0; width: 100%; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); overflow: hidden; }
/* 文本模式下让滚动交给 Monaco 自己，外层不再滚 */
.cd-scroll.cd-fill { display: flex; flex-direction: column; overflow: hidden; gap: 6px; }
.cd-cmd { font-family: var(--tw-mono-font); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-reason { font-size: 11px; color: var(--guaxi-text-secondary); max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-st { white-space: nowrap; }
.cd-ok td { color: #30a46c; }
.cd-ok-idem td { color: #3b82f6; }
.cd-fail td { color: #e5484d; }
.cd-running td { color: var(--tw-accent); }
.cd-skipped td { color: var(--guaxi-text-secondary); opacity: .7; }
.cd-hint { font-size: 12px; color: var(--guaxi-text-secondary); padding: 8px 2px; }
.cd-warns { margin-top: 8px; font-size: 12px; color: #f5a524; }
.cd-yaml { margin-top: 8px; }
.cd-row { display: flex; gap: 6px; margin-top: 6px; }
.cd-out { margin-top: 10px; }
.cd-out-head { font-size: 12px; font-weight: 600; color: var(--tw-accent); margin-bottom: 4px; }
.cd-pre { margin: 0; padding: 8px; border-radius: var(--tw-radius-sm); background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); font-size: 12px; white-space: pre-wrap; word-break: break-all; max-height: 320px; overflow: auto; }

.cd-modal { display: flex; flex-direction: column; max-height: 84vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; }
.cd-modal-h { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.cd-modal-b { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 12px 14px; }
.cd-modal-f { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--guaxi-border); }
.cd-field { display: grid; grid-template-columns: 130px 1fr; gap: 4px 10px; align-items: center; margin-bottom: 8px; }
.cd-field label { font-size: 12px; color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); }
.cd-field .cd-doc { grid-column: 2; font-size: 11px; color: var(--guaxi-text-secondary); }
.cd-req { color: #e5484d; margin-left: 2px; }
.cd-req-l::after { content: ' *'; color: #e5484d; }

.cd-cat { display: flex; gap: 10px; min-height: 420px; }
.cd-cat-list { flex: 0 0 320px; overflow: auto; border-right: 1px solid var(--guaxi-border); padding-right: 8px; }
.cd-cat-g { font-size: 11px; font-weight: 700; color: var(--tw-accent); margin: 8px 0 4px; }
.cd-cat-item { padding: 5px 8px; border-radius: 6px; cursor: pointer; }
.cd-cat-item:hover { background: var(--tw-accent-soft); }
.cd-cat-item.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.cd-cat-name { display: block; font-size: 12px; font-family: var(--tw-mono-font); }
.cd-cat-syn { display: block; font-size: 11px; color: var(--guaxi-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-cat-detail { flex: 1 1 auto; min-width: 0; overflow: auto; }
.cd-cat-title { font-size: 13px; font-weight: 600; margin-bottom: 6px; }
.cd-tag { font-size: 10px; padding: 1px 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); margin-left: 6px; }
.cd-cat-sum { font-size: 12px; color: var(--guaxi-text-primary); margin-bottom: 8px; line-height: 1.5; }
.cd-cat-syntax { font-family: var(--tw-mono-font); font-size: 12px; padding: 6px 8px; border-radius: 6px; background: var(--tw-code-bg); color: var(--tw-code-fg); margin-bottom: 10px; word-break: break-all; }
</style>
