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
        <button class="cd-strategy-btn" @click="showStrategyLibrary = true">
          <Icon :icon="bookIcon" width="16" height="16" />
          <span class="cd-strategy-label" :title="strategyLabel">{{ strategyLabel }}</span>
        </button>
      </template>打开策略库：可搜索、滚动查看全部策略，点击条目载入；悬停可看完整说明</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib primary" :disabled="!tasks.length" @click="openSaveStrategy"><Icon :icon="saveIcon" width="16" height="16" /></button>
      </template>把当前任务表存成命名策略（凭据不入库）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib danger" :disabled="!strategyId || isBuiltinStrategy" @click="deleteStrategy"><Icon :icon="trashIcon" width="16" height="16" /></button>
      </template>{{ isBuiltinStrategy ? '内置策略不可删除' : '删除选中策略' }}</n-tooltip>
      <span class="cd-sep"></span>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" @click="importYaml"><Icon :icon="folderIcon" width="16" height="16" /></button>
      </template>导入现有 config.yaml（global/targets/tasks 全兼容）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="scenarioId" size="small" :options="scenarioOptions" placeholder="内置场景模板" style="width:200px" />
      </template>选一个内置高级流程模板，填参数后一键生成整套任务</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib primary" :disabled="!scenarioId" @click="openScenario"><Icon :icon="wandIcon" width="16" height="16" /></button>
      </template>配置该场景的参数并生成任务</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" :class="{ primary: rightPaneOpen }" @click="rightPaneOpen = !rightPaneOpen"><Icon :icon="panelRightIcon" width="16" height="16" /></button>
      </template>{{ rightPaneOpen ? '隐藏执行面板' : '显示执行面板（预览/下发时会自动打开）' }}</n-tooltip>
      <span v-if="notice" class="cd-notice">{{ notice }}</span>
      <span v-if="error" class="cd-err">{{ error }}</span>
    </div>

    <div class="cd-body">
      <!-- ============ 左：任务编辑区（默认占满） ============ -->
      <div class="cd-left">
        <div class="cd-head">
          <Icon :icon="listIcon" width="15" height="15" class="cd-head-ico" />
          <span>任务表</span>
          <span class="cd-n">{{ tasks.length }} 条</span>
          <n-radio-group :value="editMode" size="tiny" @update:value="switchMode">
            <n-radio-button value="gui">图形</n-radio-button>
            <n-radio-button value="text">文本</n-radio-button>
          </n-radio-group>
          <template v-if="editMode === 'gui'">
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" @click="addTask"><Icon :icon="plusIcon" width="16" height="16" /></button>
            </template>新增一条任务</n-tooltip>
          </template>
          <template v-else>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib primary" @click="loadYaml"><Icon :icon="uploadIcon" width="16" height="16" /></button>
            </template>把文本解析回任务表（切回图形/保存/预览时也会自动解析）</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" @click="exportYaml"><Icon :icon="saveIcon" width="16" height="16" /></button>
            </template>导出 config.yaml 到文件</n-tooltip>
            <span class="cd-sep"></span>
            <span class="cd-fs-lbl">字号</span>
            <button class="tb-ib" title="缩小字号" @click="setYamlFontSize(yamlFontSize - 1)"><Icon :icon="minusIcon" width="16" height="16" /></button>
            <span class="cd-fs">{{ yamlFontSize }}</span>
            <button class="tb-ib" title="放大字号" @click="setYamlFontSize(yamlFontSize + 1)"><Icon :icon="plusIcon" width="16" height="16" /></button>
          </template>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="openConfigExternally"><Icon :icon="externalIcon" width="16" height="16" /></button>
          </template>用系统默认程序（你的编辑器）打开这份 YAML。没有对应文件时会先存到受管目录，改完回来点上传图标重新载入</n-tooltip>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="revealConfig"><Icon :icon="revealIcon" width="16" height="16" /></button>
          </template>在系统文件管理器中显示这份 YAML 文件</n-tooltip>
          <span class="cd-head-spacer"></span>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" :class="{ primary: rightPaneOpen }" @click="rightPaneOpen = !rightPaneOpen"><Icon :icon="panelRightIcon" width="16" height="16" /></button>
          </template>{{ rightPaneOpen ? '隐藏执行面板' : '显示执行面板' }}</n-tooltip>
        </div>
        <div class="cd-scroll" :class="{ 'cd-fill': editMode === 'text' }">
          <!-- 文本模式：Monaco 编辑 YAML（懒加载，切进这个模式才加载 monaco） -->
          <template v-if="editMode === 'text'">
            <div ref="yamlEditorEl" class="cd-yaml-monaco"></div>
          </template>

          <template v-else>
            <div class="cd-targets">
              <label class="cd-tgt">
                <span class="cd-tgt-lbl">主机名</span>
                <n-tooltip trigger="hover"><template #trigger>
                  <n-input :value="hostnamesText" size="small" placeholder="逗号分隔，如 vhost,rhost" @update:value="setHostnames" />
                </template>targets.hostnames：SSL 主机名等逻辑目标，逗号分隔</n-tooltip>
              </label>
              <label class="cd-tgt">
                <span class="cd-tgt-lbl">域名</span>
                <n-tooltip trigger="hover"><template #trigger>
                  <n-input :value="domainsText" size="small" placeholder="可空 = 只做 host 级" @update:value="setDomains" />
                </template>targets.domains：域名列表；可留空（只跑 host 级任务）</n-tooltip>
              </label>
              <span class="cd-tgt-info">共 {{ tasks.length }} 条任务 · 可拖列边界调整宽度，双击单元格标题可自动适配</span>
            </div>
            <div class="cd-tbl-wrap">
              <table class="cd-table cd-task-table">
                <colgroup>
                  <col v-for="(c, ci) in taskColumns" :key="c.key" :style="{ width: colWidths[ci] + 'px' }" />
                </colgroup>
                <thead>
                  <tr>
                    <th v-for="(c, ci) in taskColumns" :key="c.key" :title="c.hint">
                      <span class="cd-th-lbl">{{ c.label }}</span>
                      <span
                        v-if="ci < taskColumns.length - 1"
                        class="cd-col-resizer"
                        title="拖动调整列宽"
                        @mousedown.prevent="onColResize(ci, $event)"
                        @dblclick.stop="autoFitColumn(ci)"
                      ></span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(t, i) in tasks" :key="i">
                    <td>
                      <n-input v-model:value="t.id" size="small" placeholder="任务 id" :title="t.id" @update:value="syncYaml" />
                    </td>
                    <td class="cd-cmdcell" @dblclick.self="openTaskEditor(t)">
                      <n-input
                        v-model:value="t.cmd"
                        type="textarea"
                        size="small"
                        :autosize="{ minRows: 2, maxRows: 6 }"
                        placeholder="命令（支持 {hostname}/{domain}/{index}）"
                        :title="t.cmd"
                        @update:value="syncYaml"
                      />
                      <div class="cd-cell-foot">
                        <n-tooltip v-if="t.description || t.verify_cmd || (t.answers && t.answers.length)" trigger="hover">
                          <template #trigger>
                            <span class="cd-badge" @click="openTaskEditor(t)">
                              <Icon :icon="infoIcon" width="13" height="13" />
                              <span v-if="t.verify_cmd">校验</span>
                              <span v-if="t.answers && t.answers.length">交互 {{ t.answers.length }}</span>
                              <span v-if="t.description && !t.verify_cmd && !(t.answers && t.answers.length)">说明</span>
                            </span>
                          </template>
                          <div class="cd-tip">
                            <div v-if="t.description">{{ t.description }}</div>
                            <div v-if="t.verify_cmd">校验：{{ t.verify_cmd }}</div>
                            <div v-if="t.expect && t.expect.length">期望：{{ t.expect.join('、') }}</div>
                            <div v-if="t.answers && t.answers.length">交互：{{ t.answers.map(a => `“${a.when}” → ${a.reply}`).join('；') }}</div>
                          </div>
                        </n-tooltip>
                        <span class="cd-head-spacer"></span>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="openTaskEditor(t)"><Icon :icon="expandIcon" width="15" height="15" /></button>
                        </template>打开大编辑器（完整命令 / 说明 / 交互应答 / 校验）</n-tooltip>
                      </div>
                    </td>
                    <td>
                      <n-input :value="(t.depends_on || []).join(',')" size="small" placeholder="依赖 id" :title="(t.depends_on || []).join(', ')" @update:value="v => setDeps(t, v)" />
                    </td>
                    <td class="cd-cond">
                      <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_no_domain" size="small" @update:checked="v => { t.only_if_no_domain = v; if (v) t.only_if_domain = false; syncYaml() }">host</n-checkbox></template>只在 host 级（无域名）执行</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_domain" size="small" @update:checked="v => { t.only_if_domain = v; if (v) t.only_if_no_domain = false; syncYaml() }">sni</n-checkbox></template>只在关联了域名时执行（SNI 流程）</n-tooltip>
                    </td>
                    <td class="cd-pemcell">
                      <div class="cd-row">
                        <n-tooltip trigger="hover"><template #trigger>
                          <n-input v-model:value="t.pem_file" size="small" placeholder="PEM 文件（可空）" style="flex:1;min-width:0" :title="t.pem_file" @update:value="syncYaml" />
                        </template>{{ t.pem_file || '未选择文件' }}</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" @click="openPemPicker(t)"><Icon :icon="fileIcon" width="16" height="16" /></button>
                        </template>选择要导入的证书/私钥文件：可从证书页的 CA 库 / 终端证书库直接取，也可选任意本地文件</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" :disabled="!t.pem_file" @click="openFileExternally(t.pem_file)"><Icon :icon="externalIcon" width="16" height="16" /></button>
                        </template>用系统默认程序打开这个证书/私钥文件</n-tooltip>
                        <n-tooltip trigger="hover"><template #trigger>
                          <button class="tb-ib" :disabled="!t.pem_file" @click="revealFile(t.pem_file)"><Icon :icon="revealIcon" width="16" height="16" /></button>
                        </template>在系统文件管理器中显示这个文件</n-tooltip>
                      </div>
                      <div class="cd-row cd-index-row">
                        <span class="cd-mini-lbl">索引导入证书/私钥使用的证书索引</span>
                        <n-input-number v-model:value="t.index" size="small" :min="1" :max="3" :show-button="false" placeholder="index" style="width:88px" @update:value="syncYaml" />
                      </div>
                    </td>
                    <td class="cd-verify-cell">
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" :class="{ primary: !!t.verify_cmd }" @click="openVerify(t)"><Icon :icon="shieldIcon" width="16" height="16" /></button>
                      </template>{{ t.verify_cmd ? '已配置执行后校验，点击修改' : '配置执行后校验：跑一条校验命令，并检查输出是否包含期望关键字' }}</n-tooltip>
                    </td>
                    <td class="cd-ops">
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" @click="openTaskEditor(t)"><Icon :icon="expandIcon" width="16" height="16" /></button>
                      </template>打开大编辑器</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib" @click="openCatalogFor(t)"><Icon :icon="bookIcon" width="16" height="16" /></button>
                      </template>从命令目录给这条选个命令模板</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="tb-ib danger" @click="removeTask(i)"><Icon :icon="trashIcon" width="16" height="16" /></button>
                      </template>删除该任务</n-tooltip>
                    </td>
                  </tr>
                  <tr v-if="!tasks.length"><td colspan="7" class="cd-hint">还没有任务。可「导入 config.yaml」、从「内置场景模板」生成，或点左上角 + 手动加一条。</td></tr>
                </tbody>
              </table>
            </div>
          </template>
        </div>
      </div>

      <template v-if="rightPaneOpen">
        <div class="cd-vresize" @mousedown="onRightResize"></div>

        <!-- ============ 右：执行（可收起） ============ -->
        <div class="cd-right" :style="{ width: rightWidth + 'px' }">
          <div class="cd-head">
            <Icon :icon="playIcon" width="15" height="15" class="cd-head-ico" />
            <span>执行</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" :disabled="running" @click="doPlan"><Icon :icon="listIcon" width="16" height="16" /></button>
            </template>预览：展开 targets 并做依赖排序，列出将要逐条执行的命令（不下发）</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib primary" :disabled="running || !tasks.length" @click="doRun"><Icon :icon="playIcon" width="16" height="16" /></button>
            </template>开始下发（失败即停，除非配置里 continue_on_error）</n-tooltip>
            <n-tooltip v-if="running" trigger="hover"><template #trigger>
              <button class="tb-ib danger" @click="doCancel"><Icon :icon="stopIcon" width="16" height="16" /></button>
            </template>取消本次下发</n-tooltip>
            <span class="cd-stat" v-if="runStat">{{ runStat }}</span>
            <n-tooltip v-if="runLogPath" trigger="hover"><template #trigger>
              <button class="tb-ib" @click="openFileExternally(runLogPath)"><Icon :icon="externalIcon" width="16" height="16" /></button>
            </template>用系统默认程序打开本次运行的 JSONL 日志</n-tooltip>
            <n-tooltip v-if="runLogPath" trigger="hover"><template #trigger>
              <button class="tb-ib" @click="revealFile(runLogPath)"><Icon :icon="revealIcon" width="16" height="16" /></button>
            </template>在系统文件管理器中显示本次运行的日志文件</n-tooltip>
            <span class="cd-head-spacer"></span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="tb-ib" @click="rightPaneOpen = false"><Icon :icon="xIcon" width="16" height="16" /></button>
            </template>收起执行面板</n-tooltip>
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
      </template>
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

    <!-- 任务大编辑器 -->
    <n-modal v-model:show="showTaskEditor" :mask-closable="false" style="width:900px;max-width:95vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="expandIcon" width="16" height="16" class="cd-head-ico" />
          <span>编辑任务</span>
          <span class="cd-tag">{{ editorTask?.id || '新任务' }}</span>
          <span class="cd-head-spacer"></span>
          <button class="tb-ib" @click="showTaskEditor = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b" v-if="editorTask">
          <div class="cd-edit-grid">
            <label class="cd-field2">
              <span>任务 ID</span>
              <n-input v-model:value="editorTask.id" size="small" placeholder="如 import_key" />
            </label>
            <label class="cd-field2">
              <span>证书索引 index</span>
              <n-input v-model:value="editorTask.index" size="small" placeholder="1 / 2 / 3" />
            </label>
            <label class="cd-field2 cd-span2">
              <span>依赖（逗号分隔的任务 id）</span>
              <n-input :value="(editorTask.depends_on || []).join(', ')" size="small" placeholder="如 sm2_sign_key_host" @update:value="setEditorDeps" />
            </label>
          </div>

          <label class="cd-field2">
            <span>命令（支持 {hostname} / {domain} / {index}）</span>
            <n-input v-model:value="editorTask.cmd" type="textarea" size="small" :autosize="{ minRows: 3, maxRows: 10 }" class="cd-mono-input" />
          </label>

          <label class="cd-field2">
            <span>说明（只显示在悬停提示里）</span>
            <n-input v-model:value="editorTask.description" size="small" placeholder="这条任务是做什么的" />
          </label>

          <div class="cd-edit-grid">
            <label class="cd-field2">
              <span>PEM 文件</span>
              <n-input v-model:value="editorTask.pem_file" size="small" placeholder="可空" />
            </label>
            <div class="cd-field2">
              <span>条件</span>
              <div class="cd-row">
                <n-checkbox v-model:checked="editorTask.only_if_no_domain">只 host 级</n-checkbox>
                <n-checkbox v-model:checked="editorTask.only_if_domain">只 SNI 域名级</n-checkbox>
              </div>
            </div>
          </div>

          <div class="cd-sub">执行后校验</div>
          <div class="cd-edit-grid">
            <label class="cd-field2">
              <span>校验命令</span>
              <n-input v-model:value="editorTask.verify_cmd" size="small" placeholder="如 show ssl settings {hostname}" />
            </label>
            <label class="cd-field2">
              <span>期望关键字（逗号分隔）</span>
              <n-input :value="(editorTask.expect || []).join(',')" size="small" @update:value="v => editorTask.expect = v.split(',').map(s => s.trim()).filter(Boolean)" />
            </label>
          </div>

          <div class="cd-sub">
            交互应答
            <n-button size="tiny" style="margin-left:8px" @click="addAnswer(editorTask)">增加一条</n-button>
          </div>
          <div class="cd-hint">设备出现提示时自动发送对应内容，例如激活命令问 “[YES/(NO)]” 时答 YES；备份命令先问文件名、再问密码，可加两条按顺序匹配。</div>
          <div v-for="(a, ai) in (editorTask.answers || [])" :key="ai" class="cd-answer-row">
            <n-input v-model:value="a.when" size="small" placeholder="提示包含的文字，如 [YES/(NO)]" style="flex:2" />
            <span class="cd-arrow">→</span>
            <n-input v-model:value="a.reply" size="small" placeholder="自动发送的内容" style="flex:1" />
            <n-checkbox v-model:checked="a.repeat" size="small">重复</n-checkbox>
            <button class="tb-ib danger" @click="editorTask.answers.splice(ai, 1)"><Icon :icon="trashIcon" width="16" height="16" /></button>
          </div>
          <div v-if="!(editorTask.answers || []).length" class="cd-hint">未配置交互应答。激活/恢复等命令通常需要一条 YES。</div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showTaskEditor = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveTaskEditor">保存</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 策略库弹窗 -->
    <n-modal v-model:show="showStrategyLibrary" :mask-closable="true" style="width:760px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="bookIcon" width="16" height="16" class="cd-head-ico" />
          <span>策略库</span>
          <n-input v-model:value="strategyQuery" size="small" placeholder="搜索名称 / 说明 / 主机 / 域名" clearable style="width:280px" />
          <span class="cd-head-spacer"></span>
          <button class="tb-ib" @click="showStrategyLibrary = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b cd-strat-list">
          <div
            v-for="s in filteredStrategies"
            :key="s.id"
            class="cd-strat-row"
            :class="{ active: s.id === strategyId }"
            @click="loadStrategyById(s.id)"
          >
            <div class="cd-strat-head">
              <span class="cd-strat-name" :title="s.name">{{ s.name }}</span>
              <span v-if="s.builtin" class="cd-tag">内置</span>
              <span class="cd-strat-count">{{ s.task_count }} 条</span>
              <span class="cd-head-spacer"></span>
              <n-tooltip v-if="!s.builtin" trigger="hover"><template #trigger>
                <button class="tb-ib danger" @click.stop="deleteStrategyById(s.id)"><Icon :icon="trashIcon" width="16" height="16" /></button>
              </template>删除该策略</n-tooltip>
            </div>
            <div class="cd-strat-meta" :title="`主机: ${(s.hostnames || []).join(', ') || '—'}\n域名: ${(s.domains || []).join(', ') || '—'}`">
              主机 {{ (s.hostnames || []).join(', ') || '—' }} · 域名 {{ (s.domains || []).join(', ') || '—' }}
            </div>
            <div v-if="s.description" class="cd-strat-desc" :title="s.description">{{ s.description }}</div>
          </div>
          <div v-if="!filteredStrategies.length" class="cd-hint">没有匹配的策略。</div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" :disabled="!tasks.length" @click="openSaveStrategyFromLibrary">保存当前任务表</n-button>
          <n-button size="small" @click="showStrategyLibrary = false">关闭</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 策略保存弹窗 -->
    <n-modal v-model:show="showSaveStrategy" :mask-closable="false" style="width:520px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="saveIcon" width="16" height="16" class="cd-head-ico" />
          <span>{{ isBuiltinStrategy ? '另存为策略' : '保存为策略' }}</span>
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
          <div class="cd-hint">保存 targets + 全部任务（含映射后的命令与 PEM 路径）。凭据（ip/密码/enable）不会存进策略。{{ isBuiltinStrategy ? '当前选中的是内置策略，保存会新建一份用户策略。' : '' }}</div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showSaveStrategy = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveStrategy">保存</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 任务校验弹窗 -->
    <n-modal v-model:show="showVerify" :mask-closable="false" style="width:620px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="shieldIcon" width="16" height="16" class="cd-head-ico" />
          <span>执行后校验</span>
          <button class="tb-ib" @click="showVerify = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b">
          <div class="cd-hint">主命令成功后，自动再执行一条校验命令；期望关键字会不区分大小写地检查校验命令的输出。留空表示不做额外校验。</div>
          <div class="cd-field">
            <label>校验命令</label>
            <n-input v-model:value="verifyCmd" size="small" placeholder="如 show ssl host {hostname}" />
            <span class="cd-doc">支持 {hostname}/{domain}/{index} 占位符；设备 CLI 与 Linux shell 都可用。</span>
          </div>
          <div class="cd-field">
            <label>常用校验</label>
            <n-select :options="verifyPresets" size="small" placeholder="选一条预置校验命令（会填入上框）" @update:value="v => verifyCmd = v" />
            <span class="cd-doc">SSL 主机状态用 show ssl host；全局配置用 ssl settings / ssl globals（或带 show 的版本）。</span>
          </div>
          <div class="cd-field">
            <label>期望关键字</label>
            <n-input v-model:value="verifyExpect" type="textarea" size="small" :autosize="{ minRows: 2, maxRows: 5 }" placeholder="每行一个，或用逗号分隔；全部命中才算校验通过" />
            <span class="cd-doc">例如 activate 后校验 `show ssl host` 输出里出现 {hostname} 与 active/activated；关键字也支持占位符。</span>
          </div>

          <div class="cd-sub">
            交互应答
            <n-button size="tiny" style="margin-left:8px" @click="addVerifyAnswer">增加一条</n-button>
          </div>
          <div class="cd-hint">
            设备弹出提示时自动发送对应内容。实测：激活问 “[YES/(NO)]” → 答 YES；恢复问 “Type YES to proceed” → 答 YES；备份先问文件名、再问密码，可加两条按顺序匹配。未配置时也会自动应答常见的 YES/NO 确认。
          </div>
          <div v-for="(a, ai) in verifyAnswers" :key="ai" class="cd-answer-row">
            <n-input v-model:value="a.when" size="small" placeholder="提示包含的文字，如 [YES/(NO)]" style="flex:2" />
            <span class="cd-arrow">→</span>
            <n-input v-model:value="a.reply" size="small" placeholder="自动发送的内容" style="flex:1" />
            <n-checkbox v-model:checked="a.repeat" size="small">重复</n-checkbox>
            <button class="tb-ib danger" @click="verifyAnswers.splice(ai, 1)"><Icon :icon="trashIcon" width="16" height="16" /></button>
          </div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="clearVerify">清除校验</n-button>
          <n-button size="small" @click="showVerify = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveVerify">保存</n-button>
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
          <div class="cd-pick-head">
            <n-input v-model:value="pemQuery" size="small" placeholder="搜索名称 / 主题 / 算法" clearable style="width:260px" />
            <n-button size="small" @click="pickLocalPem"><Icon :icon="folderIcon" width="14" height="14" />从任意位置选文件</n-button>
            <span class="cd-head-spacer"></span>
            <span class="cd-hint">选中后会落到受管目录并填进该任务的 PEM 路径</span>
          </div>

          <div class="cd-sub">CA 库 <span class="cd-count">{{ filteredCas.length }}</span></div>
          <div class="cd-pick-list" v-if="filteredCas.length">
            <div v-for="c in filteredCas" :key="c.id" class="cd-pick-row">
              <div class="cd-pick-main">
                <span class="cd-pick-name" :title="c.name">{{ c.name }}</span>
                <span class="cd-pick-sub" :title="c.subject_cn">{{ c.subject_cn }}</span>
              </div>
              <div class="cd-pick-ops">
                <n-button size="tiny" @click="pickCaPem(c,'cert')"><Icon :icon="certIcon" width="14" height="14" />证书</n-button>
                <n-button size="tiny" @click="pickCaPem(c,'key')"><Icon :icon="key2Icon" width="14" height="14" />私钥</n-button>
              </div>
            </div>
          </div>
          <div v-else class="cd-hint">CA 库为空（可在工具箱 → 证书/密钥 里生成或导入）。</div>

          <div class="cd-sub">终端证书库 <span class="cd-count">{{ filteredLeaves.length }}</span></div>
          <div class="cd-pick-list" v-if="filteredLeaves.length">
            <div v-for="l in filteredLeaves" :key="l.id" class="cd-pick-row">
              <div class="cd-pick-main">
                <span class="cd-pick-name" :title="l.name">{{ l.name }}</span>
                <span class="cd-pick-sub" :title="l.ca_name">{{ l.ca_name }}</span>
              </div>
              <div class="cd-pick-ops">
                <n-button size="tiny" @click="pickLeafPem(l,'cert')">证书</n-button>
                <n-button size="tiny" @click="pickLeafPem(l,'key')">私钥</n-button>
                <n-button size="tiny" @click="pickLeafPem(l,'chain')">证书链</n-button>
              </div>
            </div>
          </div>
          <div v-else class="cd-hint">终端证书库为空（可在证书页签发 server/client 证书）。</div>

          <div class="cd-sub">受管资产库（私钥 / CSR） <span class="cd-count">{{ filteredAssets.length }}</span></div>
          <div class="cd-pick-list" v-if="filteredAssets.length">
            <div v-for="a in filteredAssets" :key="a.id" class="cd-pick-row">
              <div class="cd-pick-main">
                <span class="cd-pick-name" :title="a.name">{{ a.name }}</span>
                <span class="cd-pick-sub">{{ pemAssetKindLabel[a.kind] || a.kind }} · {{ a.algo || '—' }}</span>
              </div>
              <div class="cd-pick-ops">
                <n-button v-if="a.kind === 'key'" size="tiny" @click="pickAssetPem(a,'key')">私钥</n-button>
                <n-button v-if="a.kind === 'key'" size="tiny" @click="pickAssetPem(a,'pub')">公钥</n-button>
                <n-button v-if="a.kind === 'csr'" size="tiny" @click="pickAssetPem(a,'csr')">CSR</n-button>
              </div>
            </div>
          </div>
          <div v-else class="cd-hint">受管资产库为空（可在证书页把独立密钥/CSR 存入库）。</div>
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
import { createLogger } from '../../../utils/logger'
import { useSshProfiles } from '../../../composables/useSshProfiles'
import { readUiState, persistUiState, oneOf, within } from '../../../utils/uiState'
import { getGuaxiMonacoTheme, defineGuaxiMonacoThemes } from '../../../monaco/guaxiMonacoThemes'
import { resolveSharedEditorDisplayOptions } from '../../../monaco/editorFeatures'
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
import xIcon from '@iconify-icons/lucide/x'
import scrollTextIcon from '@iconify-icons/lucide/scroll-text'
import keyRoundIcon from '@iconify-icons/lucide/key-round'
import externalIcon from '@iconify-icons/lucide/external-link'
import panelRightIcon from '@iconify-icons/lucide/panel-right'
import shieldIcon from '@iconify-icons/lucide/shield-check'
import minusIcon from '@iconify-icons/lucide/minus'
import infoIcon from '@iconify-icons/lucide/info'
import expandIcon from '@iconify-icons/lucide/maximize-2'

// 「在文件管理器中显示」沿用打开目录的图标。
const revealIcon = folderIcon
const certIcon = scrollTextIcon
const key2Icon = keyRoundIcon

// ============ 状态 ============
const logger = createLogger('CommandDispatchPanel')
const { profiles } = useSshProfiles(logger)
const profileOptions = computed(() => profiles.value.map(p => ({ label: `${p.name || p.host}`, value: p.id })))
// 面板选择记在本地：下次开窗接着用（各窗口同源共享一份；**enable 密码不在这里**，它在 keychain）。
const UI = {
  profile: 'guaxi_cmd_dispatch_profile',
  sessionMode: 'guaxi_cmd_dispatch_session_mode',
  probeRc: 'guaxi_cmd_dispatch_probe_rc',
  editMode: 'guaxi_cmd_dispatch_edit_mode',
  rightOpen: 'guaxi_cmd_dispatch_right_open',
  rightWidth: 'guaxi_cmd_dispatch_right_width',
  fontSize: 'guaxi_cmd_dispatch_yaml_font_size',
  strategy: 'guaxi_cmd_dispatch_strategy',
}

const profileId = ref(readUiState(UI.profile, null))
const enablePassword = ref('')

const cfg = ref({ global: {}, targets: { hostnames: [], domains: [] }, tasks: [], continue_on_error: false })
const yamlText = ref('')
/// 编辑方式：图形任务表 / 文本 YAML。
const editMode = ref(readUiState(UI.editMode, 'gui', oneOf('gui', 'text')))
const baseDir = ref('')

/// 会话类型与退出码探针（选错会话类型会"等待提示符超时"）。
const sessionMode = ref(readUiState(UI.sessionMode, 'auto', oneOf('auto', 'device', 'shell')))
const sessionModeOptions = [
  { label: '自动探测', value: 'auto' },
  { label: '设备 CLI', value: 'device' },
  { label: 'Linux shell', value: 'shell' },
]
const probeRc = ref(readUiState(UI.probeRc, true))

/// 策略库。
const strategies = ref([])
const strategyId = ref(readUiState(UI.strategy, null))
const showSaveStrategy = ref(false)
const strategyName = ref('')
const strategyDesc = ref('')
const isBuiltinStrategy = computed(() => !!strategies.value.find(s => s.id === strategyId.value)?.builtin)
const showStrategyLibrary = ref(false)
const strategyQuery = ref('')
const currentStrategy = computed(() => strategies.value.find(s => s.id === strategyId.value) || null)
const strategyLabel = computed(() => currentStrategy.value
  ? `${currentStrategy.value.builtin ? '内置 · ' : ''}${currentStrategy.value.name} (${currentStrategy.value.task_count})`
  : '策略库')
const filteredStrategies = computed(() => {
  const q = strategyQuery.value.trim().toLowerCase()
  if (!q) return strategies.value
  return strategies.value.filter(s =>
    [s.name, s.description, ...(s.hostnames || []), ...(s.domains || [])]
      .filter(Boolean).some(v => String(v).toLowerCase().includes(q)))
})
function loadStrategyById(id) {
  strategyId.value = id
  showStrategyLibrary.value = false
  loadStrategy()
}
function openSaveStrategyFromLibrary() {
  showStrategyLibrary.value = false
  openSaveStrategy()
}
async function deleteStrategyById(id) {
  const s = strategies.value.find(x => x.id === id)
  if (!s || s.builtin) return
  if (!window.confirm(`删除策略「${s.name}」？`)) return
  try {
    await invoke('cmd_strategy_delete', { id })
    if (strategyId.value === id) strategyId.value = null
    await loadStrategies()
    note('策略已删除')
  } catch (e) { error.value = '删除策略失败: ' + (e?.message || e) }
}

/// 任务级校验（主命令成功后跑校验命令 + 核对期望关键字）。
const showVerify = ref(false)
const verifyTarget = ref(null)
const verifyCmd = ref('')
const verifyExpect = ref('')
const verifyAnswers = ref([])

/// 任务大编辑器（小表格里放不下的完整编辑）。
const showTaskEditor = ref(false)
const editorTask = ref(null)
let editorIndex = -1
function cloneTask(t) {
  return JSON.parse(JSON.stringify({
    id: t.id || '', cmd: t.cmd || '', pem_file: t.pem_file || '', index: t.index || '',
    depends_on: t.depends_on || [], only_if_domain: !!t.only_if_domain, only_if_no_domain: !!t.only_if_no_domain,
    description: t.description || '', verify_cmd: t.verify_cmd || '', expect: t.expect || [], answers: t.answers || [],
  }))
}
function openTaskEditor(t) {
  editorIndex = cfg.value.tasks.indexOf(t)
  editorTask.value = cloneTask(t)
  if (!editorTask.value.answers) editorTask.value.answers = []
  showTaskEditor.value = true
}
function setEditorDeps(v) {
  if (!editorTask.value) return
  editorTask.value.depends_on = String(v).split(',').map(s => s.trim()).filter(Boolean)
}
function addAnswer(t) {
  if (!t.answers) t.answers = []
  t.answers.push({ when: '', reply: 'YES', repeat: false })
}
function saveTaskEditor() {
  if (editorIndex < 0 || !cfg.value.tasks[editorIndex] || !editorTask.value) return
  const cleaned = cloneTask(editorTask.value)
  cleaned.answers = (cleaned.answers || []).filter(a => String(a.when || '').trim())
  cleaned.expect = (cleaned.expect || []).filter(Boolean)
  if (!cleaned.verify_cmd) cleaned.verify_cmd = ''
  if (!cleaned.description) cleaned.description = null
  cfg.value.tasks[editorIndex] = cleaned
  showTaskEditor.value = false
  syncYaml()
}

/// 可调列宽（记住上次宽度）。
const taskColumns = [
  { key: 'id', label: '任务 ID', hint: '任务的逻辑 id，depends_on 引用它', w: 150 },
  { key: 'cmd', label: '命令', hint: '支持 {hostname} / {domain} / {index} 占位符；双击打开大编辑器', w: 380 },
  { key: 'deps', label: '依赖', hint: '必须在此任务之前执行的 id，逗号分隔', w: 150 },
  { key: 'cond', label: '执行条件', hint: 'host = 只在 host 级执行；sni = 只在对每个域名执行', w: 130 },
  { key: 'pem', label: 'PEM 文件 / 索引', hint: '要导入或使用的证书/私钥文件，以及设备上的证书索引', w: 300 },
  { key: 'verify', label: '校验', hint: '执行后自动跑校验命令并核对期望关键字', w: 70 },
  { key: 'ops', label: '操作', hint: '编辑、从命令目录插入、删除', w: 110 },
]
const colWidths = ref(readUiState(
  'guaxi_cmd_dispatch_col_widths',
  taskColumns.map(c => c.w),
  (v) => Array.isArray(v) && v.length === taskColumns.length && v.every(n => Number.isFinite(n)),
))
watch(colWidths, (v) => persistUiState('guaxi_cmd_dispatch_col_widths', v), { deep: true })
function onColResize(ci, e) {
  const startX = e.clientX
  const startW = colWidths.value[ci]
  const move = (ev) => {
    const next = [...colWidths.value]
    next[ci] = Math.max(60, Math.min(900, startW + ev.clientX - startX))
    colWidths.value = next
  }
  const up = () => {
    document.removeEventListener('mousemove', move)
    document.removeEventListener('mouseup', up)
  }
  document.addEventListener('mousemove', move)
  document.addEventListener('mouseup', up)
}
function autoFitColumn(ci) {
  const longest = tasks.value.reduce((m, t) => {
    const v = [t.id, t.cmd, (t.depends_on || []).join(','), t.pem_file, String(t.index || '')]
    const idx = { id: 0, cmd: 1, deps: 2, pem: 3 }[taskColumns[ci].key]
    if (idx == null) return m
    return Math.max(m, String(v[idx] || '').split('\n').reduce((n, line) => Math.max(n, line.length), 0))
  }, 0)
  const next = [...colWidths.value]
  next[ci] = Math.max(80, Math.min(900, longest * 9 + 90))
  colWidths.value = next
}
const verifyPresets = [
  { label: 'show ssl host', value: 'show ssl host' },
  { label: 'show ssl settings {hostname}', value: 'show ssl settings {hostname}' },
  { label: 'show ssl globals', value: 'show ssl globals' },
  { label: 'ssl settings', value: 'ssl settings' },
  { label: 'ssl globals', value: 'ssl globals' },
]

/// PEM 选择器（可从证书页的 CA 库 / 终端证书库取，也可选任意本地文件）。
const showPem = ref(false)
const pemTarget = ref(null)
const caList = ref([])
const leafList = ref([])
const pemAssets = ref([])
const pemQuery = ref('')
const pemAssetKindLabel = { key: '私钥', csr: 'CSR', crl: 'CRL' }
function pemHit(...vals) {
  const q = pemQuery.value.trim().toLowerCase()
  if (!q) return true
  return vals.filter(Boolean).some(v => String(v).toLowerCase().includes(q))
}
const filteredCas = computed(() => caList.value.filter(c => pemHit(c.name, c.subject_cn)))
const filteredLeaves = computed(() => leafList.value.filter(l => pemHit(l.name, l.ca_name, l.subject_cn)))
const filteredAssets = computed(() => pemAssets.value.filter(a => pemHit(a.name, a.kind, a.algo)))

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

/// 执行面板默认收起，编辑区占满；预览/下发时自动打开。
const rightPaneOpen = ref(readUiState(UI.rightOpen, false))
const rightWidth = ref(readUiState(UI.rightWidth, 520, within(360, 1200)))
const yamlFontSize = ref(readUiState(UI.fontSize, 18, within(12, 32)))
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
let themesDefined = false
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
  // 关键：`guaxi-dark` / `guaxi-light` 必须**在本窗口的 monaco 实例上**注册。
  // 主题名是全局注册表，而每个窗口是独立的 monaco 实例（本面板在独立应用窗口里，
  // 主窗口的 GitPanel/编辑器注册的用不上）。没注册时 create({theme:'guaxi-dark'})
  // 会静默退回默认 `vs`——表现就是"暗色模式下编辑器还是亮的"。
  if (!themesDefined) {
    defineGuaxiMonacoThemes(m)
    themesDefined = true
  }
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
    const shared = resolveSharedEditorDisplayOptions()
    yamlModel = m.editor.createModel(yamlText.value || '', 'yaml')
    yamlEditor = m.editor.create(yamlEditorEl.value, {
      model: yamlModel,
      theme: getGuaxiMonacoTheme(settingsStore.appearanceTheme),
      automaticLayout: true,     // 容器尺寸变化（拖分隔条、切标签）自动重排
      fontSize: yamlFontSize.value,
      fontFamily: shared.fontFamily,
      lineHeight: shared.lineHeight,
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

// 切模式：先完成同步/解析，再真正切换视图。
// 这样解析失败时可以原地留在文本模式，编辑器 DOM 不会被卸载。
let switchingMode = false
/// 图形模式是否改过任务表：没改就保留文本模式里的原始 YAML（注释/排版不丢）。
let cfgDirty = false
async function switchMode(v) {
  if (switchingMode || v === editMode.value) return
  switchingMode = true
  try {
    if (v === 'text') {
      if (cfgDirty) await serializeCfg()
      editMode.value = 'text'
      await nextTick()
      ensureYamlEditor()
    } else {
      await parseYamlIntoConfig()
      disposeYamlEditor()
      editMode.value = 'gui'
    }
  } catch (e) {
    error.value = (v === 'text' ? '生成 YAML 失败: ' : 'YAML 解析失败，已留在文本模式: ') + (e?.message || e)
  } finally {
    switchingMode = false
  }
}

function setYamlFontSize(v) {
  yamlFontSize.value = Math.min(32, Math.max(12, Math.round(v)))
  yamlEditor?.updateOptions({ fontSize: yamlFontSize.value })
  nextTick(() => yamlEditor?.layout())
}

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
/// 把当前任务表序列化成 YAML（不关心当前编辑模式）。
async function serializeCfg() {
  yamlText.value = await invoke('cmd_dispatch_to_yaml', { config: cfg.value })
  return yamlText.value
}
/// 图形模式改动的回写。文本模式下绝不覆盖编辑器内容。
async function syncYaml() {
  if (editMode.value === 'text') return yamlText.value
  cfgDirty = true
  try { return await serializeCfg() }
  catch (e) { error.value = '生成 YAML 失败: ' + (e?.message || e) }
}
/// 解析文本并应用到任务表（图形模式/预览/运行/保存前调用）。
async function parseYamlIntoConfig() {
  const c = await invoke('cmd_dispatch_parse', { configYaml: yamlText.value })
  cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
  cfgDirty = false
  return c
}
async function loadYaml() {
  try {
    await parseYamlIntoConfig()
    note('已从 YAML 载入任务表')
  } catch (e) { error.value = 'YAML 解析失败: ' + (e?.message || e) }
}
/// 保证 cfg 与当前编辑器内容一致：
/// 文本模式解析 YAML；图形模式序列化任务表。返回将要用于预览/运行的 YAML。
async function ensureCfgFromCurrentMode() {
  if (editMode.value === 'text') {
    await parseYamlIntoConfig()
    return yamlText.value
  }
  await serializeCfg()
  return yamlText.value
}

// ============ 策略库 ============
async function loadStrategies() {
  try { strategies.value = await invoke('cmd_strategy_list') } catch (_) {}
}
async function loadStrategy() {
  if (!strategyId.value) return
  try {
    yamlText.value = await invoke('cmd_strategy_load', { id: strategyId.value })
    await parseYamlIntoConfig()
    const s = strategies.value.find(x => x.id === strategyId.value)
    note(`已载入${s?.builtin ? '内置' : ''}策略「${s?.name || ''}」（${tasks.value.length} 条任务）`)
  } catch (e) { error.value = '载入策略失败: ' + (e?.message || e) }
}
function openSaveStrategy() {
  const s = strategies.value.find(x => x.id === strategyId.value)
  strategyName.value = s ? (s.builtin ? `${s.name} 副本` : s.name) : ''
  strategyDesc.value = s?.description || ''
  showSaveStrategy.value = true
}
async function saveStrategy() {
  if (!strategyName.value.trim()) { error.value = '策略名不能为空'; return }
  try {
    const y = await ensureCfgFromCurrentMode()
    const r = await invoke('cmd_strategy_save', {
      // 内置策略不可覆盖：保存时按新建处理。
      id: isBuiltinStrategy.value ? null : (strategyId.value || null),
      name: strategyName.value,
      description: strategyDesc.value || null,
      configYaml: y,
    })
    await loadStrategies()
    strategyId.value = r.id
    showSaveStrategy.value = false
    note(`策略「${r.name}」已保存（${r.task_count} 条任务）`)
  } catch (e) { error.value = '保存策略失败: ' + (e?.message || e) }
}
async function deleteStrategy() {
  if (strategyId.value) await deleteStrategyById(strategyId.value)
}

// ============ PEM 文件选择 ============
async function openPemPicker(t) {
  pemTarget.value = t
  showPem.value = true
  try { caList.value = await invoke('crypto_ca_list') } catch (_) {}
  try { leafList.value = await invoke('crypto_leaf_list') } catch (_) {}
  try { pemAssets.value = await invoke('crypto_asset_list', { kind: null }) } catch (_) {}
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
  const sel = await open({
    multiple: false,
    directory: false,
    title: '选择证书/私钥/CSR 文件',
    filters: [{ name: '证书 / 私钥', extensions: ['pem', 'crt', 'cer', 'key', 'pub', 'csr'] }],
  })
  if (!sel) return
  const p = typeof sel === 'string' ? sel : sel?.path
  if (pemTarget.value) { pemTarget.value.pem_file = p; syncYaml() }
  showPem.value = false
  note('已选用: ' + p)
}
async function pickAssetPem(a, which) {
  try {
    const r = await invoke('crypto_asset_get', { id: a.id })
    const content = which === 'pub' ? r.pem2 : r.pem
    if (!String(content || '').trim()) { error.value = '该资产没有可用内容'; return }
    await stagePem(`asset-${a.name}-${which}`, content)
  } catch (e) { error.value = '取资产内容失败: ' + (e?.message || e) }
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
    cfgDirty = false
    if (cfg.value.global?.ip && !profileId.value) note(`配置里带了 global.ip=${cfg.value.global.ip}，未选档案时将用它直连`)
    note(`已导入 ${String(path).replace(/\\/g, '/').split('/').pop()}`)
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
async function exportYaml() {
  try {
    const y = await ensureCfgFromCurrentMode()
    const p = await save({ defaultPath: 'config.yaml', filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }] })
    if (p) { await invoke('write_file_content', { path: p, content: y }); configPath.value = p; note('已导出: ' + p) }
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
  const y = await ensureCfgFromCurrentMode()
  configPath.value = await invoke('cmd_dispatch_stage_config', { content: y })
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
  cfg.value.tasks.push({ id: 'task' + (tasks.value.length + 1), cmd: '', pem_file: '', index: '', depends_on: [], only_if_domain: false, only_if_no_domain: false, verify_cmd: '', expect: [], answers: [] })
  syncYaml()
}
function removeTask(i) { cfg.value.tasks.splice(i, 1); syncYaml() }

// ============ 任务级校验 / 交互应答 ============
function openVerify(t) {
  verifyTarget.value = t
  verifyCmd.value = t.verify_cmd || ''
  verifyExpect.value = (t.expect || []).join('\n')
  verifyAnswers.value = JSON.parse(JSON.stringify(t.answers || []))
  showVerify.value = true
}
function saveVerify() {
  const t = verifyTarget.value
  if (!t) return
  t.verify_cmd = verifyCmd.value.trim()
  t.expect = verifyExpect.value
    .split(/[,\n]/)
    .map(s => s.trim())
    .filter(Boolean)
  t.answers = verifyAnswers.value.filter(a => String(a.when || '').trim())
  showVerify.value = false
  syncYaml()
  note(t.verify_cmd || t.answers.length ? '已保存校验 / 交互设置' : '已清除校验 / 交互设置')
}
function clearVerify() {
  const t = verifyTarget.value
  if (t) { t.verify_cmd = ''; t.expect = []; t.answers = []; syncYaml() }
  verifyCmd.value = ''
  verifyExpect.value = ''
  verifyAnswers.value = []
  showVerify.value = false
}
function addVerifyAnswer() {
  verifyAnswers.value.push({ when: '', reply: 'YES', repeat: false })
}

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
    cfg.value.tasks.push({ id: c.id, cmd, pem_file: c.pem ? './cfgfiles/' : '', index: '', depends_on: deps, only_if_domain: false, only_if_no_domain: false, verify_cmd: '', expect: [] })
    syncYaml()
  }
  showCatalog.value = false
  // 激活/备份/恢复只看主命令输出容易"报成功但没生效"：提醒去配校验。
  if (/(activate|backup|restore)/.test(c.id)) {
    note('已插入。激活/备份/恢复类命令建议点该行的盾牌图标配置「执行后校验」')
  }
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
    cfgDirty = false
    showScenario.value = false
    note(`已按场景「${currentScenarioName.value}」生成 ${tasks.value.length} 条任务`)
  } catch (e) { error.value = '生成场景失败: ' + (e?.message || e) }
}

// ============ 执行 ============
function statusLabel(s) {
  return { pending: '待执行', running: '执行中', ok: '成功', 'ok-idem': '幂等', fail: '失败', skipped: '跳过' }[s] || s
}
function resetSteps(planSteps) {
  steps.value = planSteps.map(t => ({
    key: t.task_key || t.taskKey,
    cmd: t.cmd,
    status: 'pending',
    reason: t.verify_cmd ? `校验: ${t.verify_cmd}` : '',
  }))
}
async function doPlan() {
  error.value = ''
  try {
    const y = await ensureCfgFromCurrentMode()
    const plan = await invoke('cmd_dispatch_plan', { configYaml: y })
    resetSteps(plan.steps)
    warnings.value = plan.warnings || []
    runStat.value = `共 ${plan.task_count} 条（${plan.target_count} 个目标）`
    rightPaneOpen.value = true
  } catch (e) { error.value = '预览失败: ' + (e?.message || e) }
}
async function doRun() {
  error.value = ''
  let y
  try { y = await ensureCfgFromCurrentMode() }
  catch (e) { error.value = '配置有问题: ' + (e?.message || e); return }
  rightPaneOpen.value = true
  try {
    const plan = await invoke('cmd_dispatch_plan', { configYaml: y })
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
      configYaml: y,
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
function onRightResize(e) {
  const startX = e.clientX; const startW = rightWidth.value
  let on = true
  const mv = ev => { if (on) rightWidth.value = Math.min(1200, Math.max(360, startW - (ev.clientX - startX))) }
  const up = () => { on = false; document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up)
}
onMounted(async () => {
  try { catalog.value = await invoke('cmd_catalog_list') } catch (_) {}
  try { scenarios.value = await invoke('cmd_scenario_list') } catch (_) {}
  unlisteners.push(await listen('cmd-dispatch-task', onTask))
  unlisteners.push(await listen('cmd-dispatch-done', onDone))
  await serializeCfg()
  await loadStrategies()
  // 上次用的策略还在就接着用；否则新开窗时自动载入第一个内置策略，
  // 保证策略库和任务表都有内容（内置策略始终存在、不可删除）。
  if (strategyId.value && strategies.value.some((s) => s.id === strategyId.value)) {
    await loadStrategy()
  } else if (!cfg.value.tasks.length && strategies.value.length) {
    strategyId.value = strategies.value[0].id
    await loadStrategy()
  }
  if (editMode.value === 'text') { await nextTick(); ensureYamlEditor() }
})
// 选择变化即落盘（右栏宽度/字号也一样）。
for (const [key, src] of [
  [UI.profile, profileId], [UI.sessionMode, sessionMode], [UI.probeRc, probeRc],
  [UI.editMode, editMode], [UI.rightOpen, rightPaneOpen], [UI.rightWidth, rightWidth],
  [UI.fontSize, yamlFontSize], [UI.strategy, strategyId],
]) {
  watch(src, (v) => persistUiState(key, v))
}

// 档案/策略可能已被删掉：列表到位后清掉失效的选中，避免下拉挂着不存在的项。
watch(profiles, (list) => {
  if (!list.length) return
  const ids = new Set(list.map((p) => p.id))
  if (profileId.value && !ids.has(profileId.value)) profileId.value = null
})
watch(strategies, (list) => {
  if (!list.length) return
  const ids = new Set(list.map((s) => s.id))
  if (strategyId.value && !ids.has(strategyId.value)) strategyId.value = null
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
.cd-lbl { font-size: 15px; color: var(--guaxi-text-secondary); }
.cd-sep { width: 1px; height: 18px; background: var(--guaxi-border); margin: 0 4px; }
.cd-notice { font-size: 15px; color: #f5a524; }
.cd-err { font-size: 15px; color: #e5484d; }
.cd-stat { font-size: 15px; color: var(--tw-accent); font-family: var(--tw-mono-font); }

.cd-body { flex: 1 1 auto; min-height: 0; display: flex; }
/* 编辑区占满剩余宽度；执行面板默认收起，打开时才占一栏。 */
.cd-left { flex: 1 1 auto; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
.cd-right { flex: 0 0 auto; min-width: 360px; display: flex; flex-direction: column; min-height: 0; border-left: 1px solid var(--guaxi-border); }
.cd-vresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; }
.cd-vresize:hover { opacity: 1; background: var(--tw-accent); }

.cd-head { flex: 0 0 auto; display: flex; align-items: center; gap: 7px; padding: 6px 10px; font-size: 15px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.08)); color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); flex-wrap: wrap; row-gap: 4px; }
.cd-head-ico { color: var(--tw-accent); }
.cd-head-spacer { flex: 1 1 auto; }
.cd-n { font-size: 13.5px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.cd-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 8px 10px; }

/* 图标按钮：本面板是独立组件，父级 RemoteToolbox 的 scoped 样式管不到这里。 */
.tb-ib { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; flex: 0 0 auto; padding: 0; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; transition: background .12s, color .12s; vertical-align: middle; }
.tb-ib svg { width: 16px !important; height: 16px !important; }
.tb-ib:hover:not(:disabled) { background: var(--tw-accent-soft); color: var(--tw-accent); }
.tb-ib:disabled { opacity: .35; cursor: not-allowed; }
.tb-ib.primary { color: var(--tw-accent); }
.tb-ib.danger:hover:not(:disabled) { color: #e5484d; background: color-mix(in srgb, #e5484d 12%, transparent); }
.cd-fs-lbl { font-size: 13.5px; color: var(--guaxi-text-secondary); }
.cd-fs { min-width: 22px; text-align: center; font-size: 14px; font-family: var(--tw-mono-font); color: var(--tw-accent); }
.cd-strategy-btn { display: inline-flex; align-items: center; gap: 6px; height: 28px; max-width: 260px; padding: 0 10px; border: 1px solid var(--guaxi-border); border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; font-size: 14px; }
.cd-strategy-btn:hover { border-color: var(--tw-accent); color: var(--tw-accent); }
.cd-strategy-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-strat-list { max-height: 60vh; overflow: auto; padding: 8px 10px; }
.cd-strat-row { padding: 10px 12px; margin-bottom: 8px; border: 1px solid var(--guaxi-border-weak, rgba(0,0,0,.08)); border-radius: 8px; cursor: pointer; }
.cd-strat-row:hover { background: var(--tw-accent-soft); border-color: var(--tw-accent); }
.cd-strat-row.active { box-shadow: inset 3px 0 0 var(--tw-accent); }
.cd-strat-head { display: flex; align-items: center; gap: 8px; }
.cd-strat-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 15px; font-weight: 600; }
.cd-strat-count { flex: 0 0 auto; font-size: 13px; color: var(--guaxi-text-secondary); }
.cd-strat-meta, .cd-strat-desc { margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px; color: var(--guaxi-text-secondary); }

.cd-targets { display: flex; gap: 12px; align-items: center; margin-bottom: 8px; flex-wrap: wrap; }
.cd-tgt { display: flex; align-items: center; gap: 6px; flex: 1 1 260px; min-width: 0; }
.cd-tgt-lbl { flex: 0 0 auto; font-size: 14px; color: var(--guaxi-text-secondary); }
.cd-tgt-info { flex: 0 0 auto; font-size: 13px; color: var(--guaxi-text-secondary); }
/* 横向可滚：列宽可拖，总宽超出时出现滚动条而不是挤压内容。 */
.cd-tbl-wrap { overflow: auto; min-height: 0; max-width: 100%; }
.cd-table { width: max-content; min-width: 100%; border-collapse: collapse; font-size: 16px; }
.cd-table th, .cd-table td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); vertical-align: top; }
.cd-table th { font-size: 14px; letter-spacing: .02em; color: var(--guaxi-text-secondary); position: sticky; top: 0; background: var(--guaxi-panel-bg); z-index: 2; white-space: nowrap; }
.cd-task-table { table-layout: fixed; }
.cd-table thead tr { position: relative; }
.cd-th-lbl { display: inline-block; padding-right: 6px; }
/* 列边界拖拽条：悬停变亮，双击自动适配。 */
.cd-col-resizer { position: absolute; top: 0; bottom: 0; right: -3px; width: 7px; cursor: col-resize; z-index: 3; }
.cd-col-resizer::after { content: ''; position: absolute; top: 4px; bottom: 4px; left: 3px; width: 1px; background: var(--guaxi-border); }
.cd-col-resizer:hover::after { background: var(--tw-accent); width: 2px; left: 2.5px; }
.cd-cond { white-space: nowrap; }
.cd-ops { white-space: nowrap; text-align: right; }
.cd-verify-cell { text-align: center; }
/* 命令多行输入：整条命令都能看全，且自动换行。 */
.cd-cmdcell { min-width: 0; }
.cd-cmdcell :deep(.n-input__textarea-el), .cd-cmdcell :deep(.n-input__input-el) { font-family: var(--tw-mono-font); font-size: 15px; line-height: 1.5; white-space: pre-wrap; word-break: break-word; }
.cd-cmdcell :deep(.n-input) { width: 100%; }
.cd-pemcell { min-width: 0; }
.cd-cell-foot { display: flex; align-items: center; gap: 4px; margin-top: 2px; min-height: 20px; }
.cd-badge { display: inline-flex; align-items: center; gap: 4px; padding: 0 7px; border-radius: 999px; font-size: 12.5px; color: var(--tw-accent); background: var(--tw-accent-soft); cursor: pointer; }
.cd-badge:hover { filter: brightness(1.15); }
.cd-tip { max-width: 380px; white-space: normal; line-height: 1.5; font-size: 13.5px; }
.cd-index-row { align-items: center; gap: 6px; }
.cd-mini-lbl { flex: 1 1 auto; min-width: 0; font-size: 12.5px; color: var(--guaxi-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-res { font-family: var(--tw-mono-font); font-size: 15px; }
.cd-sub { margin: 12px 0 4px; font-size: 15.5px; font-weight: 600; color: var(--tw-accent); }
.cd-edit-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 14px; margin-bottom: 10px; }
.cd-field2 { display: flex; flex-direction: column; gap: 4px; min-width: 0; margin-bottom: 10px; }
.cd-field2 > span { font-size: 13.5px; color: var(--guaxi-text-secondary); }
.cd-span2 { grid-column: span 2; }
.cd-mono-input :deep(.n-input__textarea-el) { font-family: var(--tw-mono-font); font-size: 14px; }
.cd-answer-row { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; }
.cd-arrow { color: var(--tw-accent); font-size: 14px; }
.cd-pick-head { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; flex-wrap: wrap; }
.cd-count { font-size: 13px; color: var(--guaxi-text-secondary); font-weight: 400; margin-left: 4px; }
.cd-pick-list { display: flex; flex-direction: column; gap: 4px; max-height: 220px; overflow: auto; padding-right: 4px; }
.cd-pick-row { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border: 1px solid var(--guaxi-border-weak, rgba(0,0,0,.08)); border-radius: 6px; }
.cd-pick-row:hover { border-color: var(--tw-accent); background: var(--tw-accent-soft); }
.cd-pick-main { display: flex; flex-direction: column; gap: 1px; min-width: 0; flex: 1 1 auto; }
.cd-pick-name { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-pick-sub { font-size: 12.5px; color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-pick-ops { display: flex; gap: 4px; flex: 0 0 auto; }
.cd-yaml-monaco { flex: 1 1 auto; min-height: 0; width: 100%; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); overflow: hidden; }
/* 文本模式下让滚动交给 Monaco 自己，外层不再滚，编辑器占满整高。 */
.cd-scroll.cd-fill { display: flex; flex-direction: column; overflow: hidden; padding: 0; gap: 0; }
.cd-scroll.cd-fill .cd-yaml-monaco { border: none; border-radius: 0; }
.cd-cmd { font-family: var(--tw-mono-font); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-reason { font-size: 14px; color: var(--guaxi-text-secondary); max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-st { white-space: nowrap; }
.cd-ok td { color: #30a46c; }
.cd-ok-idem td { color: #3b82f6; }
.cd-fail td { color: #e5484d; }
.cd-running td { color: var(--tw-accent); }
.cd-skipped td { color: var(--guaxi-text-secondary); opacity: .7; }
.cd-hint { font-size: 15px; color: var(--guaxi-text-secondary); padding: 8px 2px; }
.cd-warns { margin-top: 8px; font-size: 15px; color: #f5a524; }
.cd-yaml { margin-top: 8px; }
.cd-row { display: flex; gap: 6px; margin-top: 6px; }
.cd-out { margin-top: 10px; }
.cd-out-head { font-size: 15px; font-weight: 600; color: var(--tw-accent); margin-bottom: 4px; }
.cd-pre { margin: 0; padding: 8px; border-radius: var(--tw-radius-sm); background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); font-size: 15px; white-space: pre-wrap; word-break: break-all; max-height: 320px; overflow: auto; }

.cd-modal { display: flex; flex-direction: column; max-height: 84vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; }
.cd-modal-h { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.cd-modal-b { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 12px 14px; }
.cd-modal-f { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--guaxi-border); }
.cd-field { display: grid; grid-template-columns: 130px 1fr; gap: 4px 10px; align-items: center; margin-bottom: 8px; }
.cd-field label { font-size: 15px; color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); }
.cd-field .cd-doc { grid-column: 2; font-size: 14px; color: var(--guaxi-text-secondary); }
.cd-req { color: #e5484d; margin-left: 2px; }
.cd-req-l::after { content: ' *'; color: #e5484d; }

.cd-cat { display: flex; gap: 10px; min-height: 420px; }
.cd-cat-list { flex: 0 0 320px; overflow: auto; border-right: 1px solid var(--guaxi-border); padding-right: 8px; }
.cd-cat-g { font-size: 14px; font-weight: 700; color: var(--tw-accent); margin: 8px 0 4px; }
.cd-cat-item { padding: 5px 8px; border-radius: 6px; cursor: pointer; }
.cd-cat-item:hover { background: var(--tw-accent-soft); }
.cd-cat-item.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.cd-cat-name { display: block; font-size: 15px; font-family: var(--tw-mono-font); }
.cd-cat-syn { display: block; font-size: 14px; color: var(--guaxi-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-cat-detail { flex: 1 1 auto; min-width: 0; overflow: auto; }
.cd-cat-title { font-size: 16px; font-weight: 600; margin-bottom: 6px; }
.cd-tag { font-size: 13.5px; padding: 1px 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); margin-left: 6px; }
.cd-cat-sum { font-size: 15px; color: var(--guaxi-text-primary); margin-bottom: 8px; line-height: 1.5; }
.cd-cat-syntax { font-family: var(--tw-mono-font); font-size: 15px; padding: 6px 8px; border-radius: 6px; background: var(--tw-code-bg); color: var(--tw-code-fg); margin-bottom: 10px; word-break: break-all; }
</style>
