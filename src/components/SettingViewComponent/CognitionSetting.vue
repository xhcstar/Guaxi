<template>
  <div class="cog-set">
    <div class="cog-set-head">
      <div>
        <div class="cog-set-title">
          <n-icon :component="BrainCircuit20Regular" />认知层
        </div>
        <div class="cog-set-sub">
          感知外部输入 → 记忆与召回 → 主动提醒。关闭后采集仍会入库，只是不打分、不审议、不提醒。
        </div>
      </div>
      <n-switch :value="enabled" :loading="store.statusLoading" @update:value="toggleEnabled" />
    </div>

    <div v-if="!store.embeddingReady" class="cog-set-warn">
      <n-icon :component="Warning20Regular" />
      <span>
        本地语义服务（embedding）尚未就绪。召回、新颖度打分、矛盾检测都依赖它——
        首次使用会自动下载模型，请在「知识图谱」里先生成一次向量。
      </span>
    </div>

    <n-collapse :default-expanded-names="['sources', 'budget']">
      <n-collapse-item name="sources">
        <template #header>
          <span class="cog-sec"><n-icon :component="Mail20Regular" />感知来源</span>
        </template>
        <div class="cog-note">
          每一路输入都可以单独关。<b>剪贴板与屏幕默认关闭</b>——它们涉及你在别处的活动，
          必须你明确同意才会采集。已采集的内容可以在「大脑 → 收件箱」里逐条查看和删除。
        </div>
        <div class="cog-rows">
          <div v-for="s in store.sources" :key="s[0]" class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">
                <n-icon :component="sourceIcon(s[0])" />{{ sourceLabel(s[0]) }}
              </div>
              <div class="cog-row-hint">{{ sourceHint(s[0]) }}</div>
            </div>
            <n-switch
              size="small"
              :value="s[1]"
              @update:value="(v) => setSource(s[0], v)"
            />
          </div>
        </div>
        <div class="cog-stat-line">
          共 {{ st?.percept?.total ?? 0 }} 条感知，未处理 {{ st?.percept?.newCount ?? 0 }}，
          待打分 {{ st?.percept?.pendingScore ?? 0 }}；
          累计中和过 <b>{{ st?.percept?.neutralizedTotal ?? 0 }}</b> 处试图指挥 AI 的内容。
        </div>

        <div class="cog-sub">额度与清理</div>
        <div class="cog-note">
          收件箱的价值取决于它够不够短。<b>抓取类来源（整站抓取、RSS）受每日额度约束</b>，
          超出的内容直接不入库；当天没看的抓取内容会被定期清掉，
          被引证、被信念引用、被提醒引用过的条目不会清。手动录入与插件采集不受额度限制。
        </div>
        <div class="cog-rows">
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">每日自动采集上限</div>
              <div class="cog-row-hint">
                只算抓取与 RSS。今日已采 <b>{{ st?.percept?.todayAuto ?? 0 }}</b> 条。
                0 表示不限（不建议：一次整站抓取就能塞进几千条）。
              </div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('percept_daily_cap', 100)"
              :min="0"
              :max="5000"
              :step="50"
              @update:value="(v) => setNum('percept_daily_cap', v)"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">未读保留天数</div>
              <div class="cog-row-hint">
                未读的抓取内容超过这个天数就在启动时与每日维护时清除。
                0 表示只保留当天：下次启动即清空之前所有未读抓取内容。
              </div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('percept_unread_keep_days', 1)"
              :min="0"
              :max="90"
              @update:value="(v) => setNum('percept_unread_keep_days', v)"
            />
          </div>
        </div>
        <div class="cog-actions">
          <GuaxiIconButton
            :icon="Broom20Regular"
            label="立即清理未读抓取内容"
            hint="不按天数规则，现在就把没读过的抓取与 RSS 内容删掉。被引证、被信念或提醒引用过的不会删。"
            :loading="busy.purge"
            @click="purgeUnread"
          />
          <GuaxiIconButton
            quaternary
            :icon="Gauge20Regular"
            label="立即打分待处理条目"
            hint="给待处理的感知求向量、算新颖度与显著性。全程本地模型，不花 token。"
            :loading="busy.score"
            @click="scoreNow"
          />
        </div>
      </n-collapse-item>
      <n-collapse-item name="activities">
        <template #header>
          <span class="cog-sec"><n-icon :component="History20Regular" />活动总线</span>
        </template>
        <div class="cog-note">
          记录「你做过什么」：打开哪本书、读到哪一页、摘了什么、搜了什么、保存了哪个文件。
          只写本地 cognition.db，不参与打分、不打断你；默认保留 180 天。
          后续的记忆沉淀与学习状态都从这张表来。
        </div>
        <div class="cog-stat-line">
          共 <b>{{ activityStats?.total ?? 0 }}</b> 条，今天 <b>{{ activityStats?.today ?? 0 }}</b> 条；
          近 {{ activityStats?.windowDays ?? 7 }} 天：
          <template v-if="activityStats?.byApp?.length">
            <span v-for="(row, i) in activityStats.byApp" :key="row[0]">
              {{ APP_LABELS[row[0]] || row[0] }} {{ row[1] }}<i v-if="i < activityStats.byApp.length - 1"> · </i>
            </span>
          </template>
          <span v-else>还没有活动</span>
        </div>
        <div class="cog-actions">
          <GuaxiIconButton
            :icon="History20Regular"
            label="刷新最近活动"
            hint="读取最近 80 条行为轨迹与近 7 天的分布。只读，不会触发任何后台任务。"
            :loading="busy.activity"
            @click="loadActivity"
          />
        </div>
        <div v-if="store.activity.length" class="cog-activity">
          <div v-for="row in store.activity" :key="row.id" class="cog-activity-row">
            <span class="cog-activity-at">{{ shortTime(row.at) }}</span>
            <span class="cog-activity-app">{{ APP_LABELS[row.app] || row.app }}</span>
            <span class="cog-activity-kind">{{ KIND_LABELS[row.kind] || row.kind }}</span>
            <span class="cog-activity-target" :title="row.targetId || row.path || ''">{{ activityTarget(row) }}</span>
            <span v-if="row.progress != null" class="cog-activity-progress">{{ Math.round(row.progress * 100) }}%</span>
          </div>
        </div>
        <div v-else-if="!store.activityLoading" class="cog-row-hint">
          还没有记录。打开一本书、摘一段、搜一次，就会出现在这里。
        </div>
      </n-collapse-item>
      <n-collapse-item name="memory">
        <template #header>
          <span class="cog-sec"><n-icon :component="BrainCircuit20Regular" />长期记忆</span>
        </template>
        <div class="cog-note">
          巩固任务把活动、读完的书与摘录沉淀成记忆：事实、偏好、疑问、技能、目标。
          派生记忆可以在下面「忘记」，也可以直接编辑 <code>memory.md</code> 的手动块——
          手动块里的增删会在下次巩固时同步进来。<b>忘记会留墓碑</b>：同一句话不会被自动加回来。
        </div>
        <div class="cog-stat-line">
          活跃 <b>{{ store.memories.length }}</b> 条，其中手动 <b>{{ manualMemoryCount }}</b> 条；
          <span v-if="store.memoryReport">
            上次巩固新增 {{ store.memoryReport.created }}，合并 {{ store.memoryReport.merged }}，退休 {{ store.memoryReport.retired }}<span v-if="store.memoryReport.usedLlm">（含模型抽取）</span>。
          </span>
          <span v-else>还没有跑过巩固。</span>
        </div>
        <div class="memory-add">
          <n-select v-model:value="memoryKind" size="small" :options="MEMORY_KIND_OPTIONS" style="width: 104px" />
          <n-input
            v-model:value="memoryText"
            size="small"
            placeholder="记一条：偏好、事实、悬而未决的问题……"
            @keydown.enter.prevent="addMemory"
          />
          <n-button size="small" type="primary" :disabled="!memoryText.trim()" @click="addMemory">记住</n-button>
          <GuaxiIconButton
            quaternary
            :icon="ArrowSync20Regular"
            label="立即巩固"
            hint="同步 memory.md 手动块，抽取最近一天的活动、读完的书与摘录，合并进记忆并重建镜像。"
            :loading="store.memoryBusy"
            @click="consolidateMemory"
          />
          <GuaxiIconButton
            quaternary
            :icon="ArrowClockwise20Regular"
            label="刷新"
            hint="重新读取记忆列表与 memory.md 路径。"
            :loading="store.memoriesLoading"
            @click="loadMemories"
          />
        </div>
        <div v-if="store.memoryFilePath" class="cog-row-hint">
          镜像文件 <code>{{ store.memoryFilePath }}</code>
        </div>
        <div v-if="store.memories.length" class="memory-list">
          <div v-for="m in store.memories" :key="m.id" class="memory-row">
            <n-tag size="tiny" :bordered="false" :type="memoryTagType(m.kind)">{{ memoryKindLabel(m.kind) }}</n-tag>
            <span class="memory-text" :title="m.text">{{ m.text }}</span>
            <span class="memory-meta">
              {{ m.source === 'manual' ? '手动' : '自动' }}<template v-if="m.hits"> · {{ m.hits + 1 }} 次</template>
            </span>
            <n-button size="tiny" text type="error" @click="forgetMemory(m)">忘记</n-button>
          </div>
        </div>
        <div v-else-if="!store.memoriesLoading" class="cog-row-hint">
          还没有记忆。跑一次巩固，或者在输入框里手动记一条。
        </div>
      </n-collapse-item>
      <n-collapse-item name="tasks">
        <template #header>
          <span class="cog-sec"><n-icon :component="CalendarClock20Regular" />任务接入</span>
        </template>
        <div class="cog-note">
          大脑的「任务」页签与到期提醒只读这里打开的分类。<b>关掉一类，它就不会出现在大脑里，
          也不会再生成到期提醒</b>；任务管理本身不受影响。例如只留「生活 / 学习」，工作待办就留在
          大脑之外。
        </div>
        <div class="cog-rows">
          <div v-for="c in taskCategories?.categories || []" :key="c.key" class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">{{ c.label }}</div>
              <div class="cog-row-hint">{{ taskCategoryHint(c.key) }}</div>
            </div>
            <n-switch
              size="small"
              :value="c.enabled"
              @update:value="(v) => setTaskCategory(c.key, v)"
            />
          </div>
        </div>
        <div class="cog-row-hint">
          当前接入：{{ taskCategories?.allowed?.length
            ? taskCategories.allowed.map(taskCategoryLabel).join(' / ')
            : '无（全部关闭）' }}
        </div>
      </n-collapse-item>
      <n-collapse-item name="budget">
        <template #header>
          <span class="cog-sec"><n-icon :component="Alert20Regular" />打扰与预算</span>
        </template>
        <div class="cog-note">
          这一节决定系统能在多大程度上打扰你。<b>没有任何提醒会被丢掉</b>——
          够不上打断门槛的一律降级到看板与简报，你随时能看到全部。
        </div>
        <div class="cog-rows">
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">每天最多打断次数</div>
              <div class="cog-row-hint">系统通知与应用内卡片合计。看板、简报不计入。</div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('max_interrupts_per_day', 3)"
              :min="0"
              :max="20"
              @update:value="(v) => setNum('max_interrupts_per_day', v)"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">每小时最多打断次数</div>
              <div class="cog-row-hint">防止同一时段连发。</div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('max_interrupts_per_hour', 1)"
              :min="0"
              :max="10"
              @update:value="(v) => setNum('max_interrupts_per_hour', v)"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">新提醒语音播报</div>
              <div class="cog-row-hint">
                新提醒投递时用语音念一遍标题与要点（默认关闭）。只播报提醒，不朗读文章正文；
                大脑顶部的喇叭按钮可以随时手动播报今日内容。
              </div>
            </div>
            <n-switch
              size="small"
              :value="Boolean(st?.voiceBroadcast)"
              @update:value="setVoiceBroadcast"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">每日 token 预算</div>
              <div class="cog-row-hint">
                用尽后自动跳过需要模型的环节（矛盾判决、简报开场），纯结构类提醒不受影响。
                今日已用 <b>{{ st?.nudge?.tokensToday ?? 0 }}</b>。
              </div>
            </div>
            <n-input-number
              size="small"
              class="cog-num-wide"
              :value="num('daily_token_budget', 60000)"
              :min="0"
              :step="10000"
              @update:value="(v) => setNum('daily_token_budget', v)"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">审议间隔（秒）</div>
              <div class="cog-row-hint">多久扫一次新感知与图谱变化。默认 300。</div>
            </div>
            <n-input-number
              size="small"
              class="cog-num-wide"
              :value="num('deliberate_interval_secs', 300)"
              :min="60"
              :step="60"
              @update:value="(v) => setNum('deliberate_interval_secs', v)"
            />
          </div>
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">审议用的模型</div>
              <div class="cog-row-hint">
                默认 6（本地）。矛盾判决的质量对模型敏感，如果误报偏多可以换成远端模型。
              </div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('deliberate_modelid', 6)"
              :min="0"
              :max="11"
              @update:value="(v) => setNum('deliberate_modelid', v)"
            />
          </div>
        </div>
        <div class="cog-policy">
          <div class="cog-policy-line">
            当前投递阈值 <b>{{ (st?.nudge?.threshold ?? 0).toFixed(2) }}</b>
            ｜近 14 天采纳率 <b>{{ acceptPct }}</b>
            ｜策略 v{{ st?.nudge?.policyVersion ?? 0 }}
            （{{ st?.nudge?.policySamples ?? 0 }} 条反馈，留出准确率
            {{ (st?.nudge?.policyMetric ?? 0).toFixed(3) }}）
          </div>
          <div class="cog-row-hint">
            阈值按采纳率自适应：你越少采纳，门槛越高。策略是一个几十参数的逻辑回归，
            只有在留出集上不比上一版差才会落库。觉得提醒变差了就回滚，反馈数据会保留。
          </div>
          <div class="cog-actions">
            <GuaxiIconButton
              :icon="BrainCircuit20Regular"
              label="重训策略"
              hint="用你最近的采纳／忽略记录重训投递策略。只有在留出集上不比现在差才会落库。"
              :loading="busy.train"
              @click="trainPolicy"
            />
            <GuaxiIconButton
              quaternary
              :icon="ArrowUndo20Regular"
              label="回滚到基线"
              hint="丢掉学出来的策略，回到内置的保守权重。你的反馈数据会保留，可以再训。"
              @click="resetPolicy"
            />
          </div>
        </div>
      </n-collapse-item>
      <n-collapse-item name="tools">
        <template #header>
          <span class="cog-sec"><n-icon :component="ShieldKeyhole20Regular" />工具授权</span>
        </template>
        <div class="cog-note">
          Agent 与外部 MCP 客户端通过这套工具操作应用。等级授权在**服务端强制执行**，
          不是提示词约束。另有一条独立的闸：由外部不可信内容驱动的调用，无论授权如何都只能用只读工具。
        </div>
        <div class="cog-rows">
          <div v-for="t in tierRows" :key="t.key" class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">
                <n-icon :component="t.icon" />{{ t.label }}
              </div>
              <div class="cog-row-hint">{{ t.hint }}</div>
            </div>
            <n-switch
              size="small"
              :value="t.granted"
              :disabled="t.key === 'read'"
              @update:value="(v) => setTier(t.key, v)"
            />
          </div>
        </div>

        <div class="cog-tool-list">
          <div class="cog-sub">共 {{ store.tools.length }} 个工具，已启用 {{ enabledToolCount }} 个</div>
          <template v-for="group in toolGroups" :key="group.key">
            <div v-if="group.tools.length" class="cog-tool-group-head">
              {{ group.label }}
              <span class="cog-tool-group-count">{{ group.tools.filter((t) => t.enabled).length }}/{{ group.tools.length }}</span>
            </div>
            <div v-for="tool in group.tools" :key="tool.name" class="cog-tool">
              <span class="cog-tool-tier" :class="'tier-' + tool.tier">{{ tool.tier }}</span>
              <span class="cog-tool-name" :class="{ off: !tool.enabled }">{{ tool.name }}</span>
              <span
                v-if="tool.requiresConfirm"
                class="cog-tool-confirm"
                title="每次调用都会弹确认，不会一次授权就连续动手"
              >逐次确认</span>
              <span class="cog-tool-desc">{{ tool.description }}</span>
            </div>
          </template>
        </div>

        <div class="cog-actions">
          <GuaxiIconButton
            quaternary
            :icon="History20Regular"
            label="查看调用审计"
            hint="最近 80 次工具调用：谁调的、调了什么、花了多久、有没有失败。授权是服务端强制的，这里是它的账本。"
            :loading="busy.audit"
            @click="loadAudit"
          />
        </div>
        <div v-if="audit.length" class="cog-audit">
          <div v-for="(row, i) in audit" :key="i" class="cog-audit-row" :class="{ bad: !row.ok }">
            <span class="cog-audit-at">{{ shortTime(row.at) }}</span>
            <span class="cog-audit-caller">{{ row.caller }}</span>
            <span class="cog-audit-tool">{{ row.tool }}</span>
            <span class="cog-audit-ms">{{ row.ms }}ms</span>
            <span v-if="!row.ok" class="cog-audit-err" :title="row.error">{{ row.error }}</span>
          </div>
        </div>
      </n-collapse-item>
      <n-collapse-item name="mcp">
        <template #header>
          <span class="cog-sec"><n-icon :component="PlugConnected20Regular" />MCP 接入</span>
        </template>
        <div class="cog-note">
          内置 codex 引擎会自动挂载这套工具（无需配置）。要让外部客户端
          （Claude Code、Cursor 等）驱动 Guaxi，把下面的配置加进它们的 MCP 设置即可——
          Guaxi 于是变成一个可被驱动的知识后端。
        </div>
        <div v-if="mcp" class="cog-mcp">
          <div class="cog-row-hint">
            服务端点 <code>{{ mcp.endpoint }}</code>（仅本机回环，不监听局域网）
          </div>
          <div class="cog-row-hint">
            stdio 桥接
            <code>{{ mcp.bridgePath }}</code>
            <n-tag size="tiny" :type="mcp.bridgeExists ? 'success' : 'warning'" :bordered="false">
              {{ mcp.bridgeExists ? '已就绪' : '未找到（需构建 guaxi_mcp）' }}
            </n-tag>
          </div>
          <div class="cog-row-hint">
            工具 {{ mcp.toolsEnabled }} / {{ mcp.toolsTotal }} 可用
          </div>
          <pre class="cog-code">{{ mcp.clientConfig }}</pre>
          <div class="cog-actions">
            <GuaxiIconButton
              :icon="Copy20Regular"
              label="复制配置"
              hint="复制上面这段 JSON，粘进外部客户端的 MCP 配置里即可接上。"
              @click="copyMcp"
            />
            <GuaxiIconButton
              quaternary
              :icon="ArrowSync20Regular"
              label="刷新"
              hint="重新读取端点、桥接程序状态与可用工具数。"
              :loading="busy.mcp"
              @click="loadMcp"
            />
          </div>
        </div>
        <div v-else class="cog-actions">
          <GuaxiIconButton
            :icon="PlugConnected20Regular"
            label="读取接入信息"
            hint="生成本机端点与可直接粘贴的客户端配置。端点只监听回环地址，不对局域网开放。"
            :loading="busy.mcp"
            @click="loadMcp"
          />
        </div>
      </n-collapse-item>
      <n-collapse-item name="profile">
        <template #header>
          <span class="cog-sec"><n-icon :component="Person20Regular" />我的画像</span>
        </template>
        <div class="cog-note">
          <b>这个文件是真相。</b>系统只会往「关注」区追加它的观察，其余各节完全由你掌握。
          删掉一行，系统就不再据此判断。它决定了什么值得提醒你、以及回答时不必从头解释什么。
        </div>
        <div v-if="store.profile" class="cog-profile">
          <div class="cog-row-hint">
            文件位置 <code>{{ store.profile.path }}</code>
          </div>
          <n-input
            v-model:value="profileDraft"
            type="textarea"
            :autosize="{ minRows: 12, maxRows: 26 }"
            class="cog-textarea"
            placeholder="画像内容"
          />
          <div class="cog-actions">
            <GuaxiIconButton
              type="primary"
              :icon="Save20Regular"
              label="保存"
              hint="写回磁盘上的 profile.md 并立即生效。这个文件是真相，系统只会往「关注」区追加。"
              :loading="busy.profile"
              @click="saveProfile"
            />
            <GuaxiIconButton
              quaternary
              :icon="ArrowUndo20Regular"
              label="放弃修改"
              hint="把编辑框恢复成磁盘上的内容，丢掉这次还没保存的改动。"
              @click="resetProfileDraft"
            />
          </div>

          <div class="cog-sub">系统学到的兴趣（可直接删）</div>
          <div class="cog-interests">
            <n-tag
              v-for="i in store.profile.interests"
              :key="i.topic"
              size="small"
              closable
              :bordered="false"
              :type="i.source === 'manual' ? 'info' : 'default'"
              :title="`${i.source === 'manual' ? '你写在文件里的' : '系统从行为推出的'}｜权重 ${i.weight.toFixed(2)}｜命中 ${i.hits} 次${i.embedded ? '' : '｜尚未生成向量'}`"
              @close="removeInterest(i.topic)"
            >{{ i.topic }} <i class="cog-w">{{ i.weight.toFixed(2) }}</i></n-tag>
            <span v-if="!store.profile.interests.length" class="cog-row-hint">还没有兴趣项。</span>
          </div>
        </div>
        <div v-else class="cog-actions">
          <GuaxiIconButton
            :icon="Person20Regular"
            label="读取画像"
            hint="从磁盘读出 profile.md 并可直接编辑。它决定了什么值得提醒你、回答时不必从头解释什么。"
            :loading="store.profileLoading"
            @click="loadProfile"
          />
        </div>
      </n-collapse-item>
      <n-collapse-item name="beliefs">
        <template #header>
          <span class="cog-sec"><n-icon :component="Lightbulb20Regular" />知识主张与矛盾</span>
        </template>
        <div class="cog-note">
          从知识库里抽出<b>可被反驳的结论</b>（主张），再用新材料两两比对，找出互相冲突的地方。
          结果只进入提醒的证据链，不会改动你的笔记。从大脑首页移到这里，避免日常界面被高级维护占满。
        </div>
        <div class="cog-row-hint">
          活跃 <b>{{ st?.belief?.active ?? 0 }}</b> 条｜已被反驳
          <b>{{ st?.belief?.contradicted ?? 0 }}</b> 条
        </div>
        <div class="cog-mine">
          <n-select
            v-model:value="beliefKb"
            size="tiny"
            class="cog-select"
            :options="kbOptions"
            placeholder="选择知识库"
          />
          <GuaxiIconButton
            :icon="Lightbulb20Regular"
            label="抽取主张"
            hint="从选中的知识库抽取可被反驳的结论（消耗少量 token）。"
            :disabled="!beliefKb"
            :loading="busy.belief"
            @click="extractBeliefs"
          />
          <GuaxiIconButton
            :icon="Warning20Regular"
            label="检测矛盾"
            hint="两两比对主张，找出互相冲突的结论并标记。"
            :loading="busy.contradict"
            @click="detectContradictions"
          />
        </div>
        <div v-if="store.beliefs.length" class="cog-beliefs">
          <div v-for="b in store.beliefs" :key="b.id" class="cog-belief">
            <span class="cog-belief-st">{{ b.statement }}</span>
            <n-tag
              size="tiny"
              :bordered="false"
              :type="b.status === 'contradicted' ? 'warning' : (b.status === 'active' ? 'success' : 'default')"
            >
              {{ beliefStatusLabel(b.status) }}
            </n-tag>
            <GuaxiIconButton
              v-if="b.status !== 'archived'"
              icon-only
              quaternary
              :icon="Archive20Regular"
              label="归档"
              hint="不再参与矛盾检测，但保留记录。"
              @click="setBeliefStatus(b.id, 'archived')"
            />
            <GuaxiIconButton
              icon-only
              quaternary
              :icon="DismissCircle20Regular"
              label="删除"
              hint="彻底删除这条主张。"
              @click="removeBelief(b.id)"
            />
          </div>
        </div>
        <div v-else class="cog-actions">
          <GuaxiIconButton
            quaternary
            :icon="ArrowClockwise20Regular"
            label="加载主张"
            hint="读取已抽取的主张列表。"
            :loading="store.beliefsLoading"
            @click="loadBeliefs"
          />
        </div>
      </n-collapse-item>
      <n-collapse-item name="evolve">
        <template #header>
          <span class="cog-sec"><n-icon :component="Beaker20Regular" />自调与评测闩</span>
        </template>
        <div class="cog-note">
          召回权重会自己调，但<b>只有在冻结评测集上不退化才允许落库</b>，且每一版都可回滚。
          没有这道闩，"自我进化"和"静默退化"从外面看完全一样。
          评测集从你自己的知识库里挖：一个章节由模型生成一个只能靠它回答的问题，章节本身就是标准答案。
        </div>
        <div class="cog-row-hint">
          当前评测样本 <b>{{ st?.evalItems ?? 0 }}</b> 条
          <span v-if="(st?.evalItems ?? 0) < 20">（少于 20 条时调参结果不可信，先扩充）</span>
          ｜召回权重
          <code>{{ weightsText }}</code>
        </div>

        <div class="cog-mine">
          <n-select
            v-model:value="mineKb"
            size="tiny"
            class="cog-select"
            :options="kbOptions"
            placeholder="选择知识库"
          />
          <n-input-number v-model:value="mineN" size="tiny" class="cog-num" :min="5" :max="200" />
          <GuaxiIconButton
            :icon="DatabaseSearch20Regular"
            label="挖掘评测样本"
            hint="让模型给选中知识库的章节各出一个只能靠它回答的问题，章节本身就是标准答案。少于 20 条时调参结果不可信。"
            :disabled="!mineKb"
            :loading="store.evalBusy"
            @click="mine"
          />
        </div>

        <div class="cog-actions">
          <GuaxiIconButton
            :icon="Beaker20Regular"
            label="跑一次评测"
            hint="用当前召回权重在评测集上算 hit@1 / hit@5 / MRR。只测量，不改任何设置。"
            :loading="store.evalBusy"
            @click="runEval"
          />
          <GuaxiIconButton
            type="primary"
            :icon="ArrowTrendingLines20Regular"
            label="自动调参"
            hint="搜索更好的召回权重组合。只有在评测集上不退化才允许落库，否则原样保留。"
            :loading="store.evalBusy"
            @click="tune"
          />
          <GuaxiIconButton
            quaternary
            :icon="ArrowUndo20Regular"
            label="回滚权重"
            hint="把召回权重恢复成内置缺省。评测历史会保留。"
            @click="rollbackWeights"
          />
        </div>

        <div v-if="store.evalRuns.length" class="cog-evals">
          <div v-for="r in store.evalRuns" :key="r.id" class="cog-eval" :class="{ ok: r.accepted }">
            <span class="cog-eval-at">{{ shortTime(r.at) }}</span>
            <span class="cog-eval-tag">{{ r.accepted ? '已采纳' : '未采纳' }}</span>
            <span class="cog-eval-m">MRR {{ r.mrr.toFixed(4) }}</span>
            <span class="cog-eval-m">hit@1 {{ r.hitAt1.toFixed(3) }}</span>
            <span class="cog-eval-m">hit@5 {{ r.hitAt5.toFixed(3) }}</span>
            <span class="cog-eval-note">{{ r.note }}</span>
          </div>
        </div>
        <div v-else class="cog-actions">
          <GuaxiIconButton
            quaternary
            :icon="History20Regular"
            label="读取评测历史"
            hint="每次评测与调参的结果都会留档：采纳与否、三个指标、原因。退化了能看出来是哪一版。"
            @click="store.refreshEvalRuns()"
          />
        </div>

        <div class="cog-sub">引证排序器</div>
        <div class="cog-note">
          用「引证是否被答案用上 / 被点开」训练一个轻量逻辑回归，在 RRF 融合之后做小幅重排。
          同样要过留出集：AUC 必须 ≥ 0.55 且比按融合分排序高 0.02 才会生效；否则保持原顺序。
          排序器只影响引证顺序，不改变召回哪一些候选。
        </div>
        <div class="cog-row-hint">
          <template v-if="store.rankerStatus?.trained">
            已训练 v{{ store.rankerStatus.version }}，AUC {{ store.rankerStatus.metric.toFixed(3) }}，
            样本 {{ store.rankerStatus.samples }}，可用标签 {{ store.rankerStatus.labeled }}
            （正例 {{ store.rankerStatus.positives }}）｜{{ shortTime(store.rankerStatus.trainedAt) }}
          </template>
          <template v-else>
            未训练（召回按纯 RRF）｜可用标签 {{ store.rankerStatus?.labeled ?? 0 }}
            （正例 {{ store.rankerStatus?.positives ?? 0 }}）
          </template>
        </div>
        <div class="cog-actions">
          <GuaxiIconButton
            :icon="ArrowTrendingLines20Regular"
            label="训练排序器"
            hint="用现有 used/clicked 标签训练一次。样本不足或 AUC 没赢过基线时只会给报告，不会落库。"
            :loading="store.rankerBusy"
            @click="trainRanker"
          />
          <GuaxiIconButton
            quaternary
            :icon="ArrowUndo20Regular"
            label="回滚排序器"
            hint="删除已生效的排序器，召回回到纯 RRF。标签与引证记录保留，可以再训。"
            @click="resetRanker"
          />
          <GuaxiIconButton
            quaternary
            :icon="History20Regular"
            label="刷新状态"
            hint="读取排序器版本、AUC 与可用标签数。"
            @click="store.loadRankerStatus()"
          />
        </div>
      </n-collapse-item>
      <n-collapse-item name="maint">
        <template #header>
          <span class="cog-sec"><n-icon :component="Database20Regular" />存储与维护</span>
        </template>
        <div class="cog-row-hint">
          数据库 <code>{{ st?.dbPath }}</code>（schema v{{ st?.schemaVersion }}）
        </div>
        <div class="cog-row-hint">
          数据目录 <code>{{ st?.dataDir }}</code>（画像与简报都是 markdown，可 git、可迁移）
        </div>
        <div class="cog-stat-line">
          记忆：情节 {{ st?.episode?.total ?? 0 }} 段（{{ st?.episode?.withRecall ?? 0 }} 段带召回），
          引证 {{ st?.episode?.citationTotal ?? 0 }} 条，其中被用上
          {{ st?.episode?.citationUsed ?? 0 }} 条、被点开 {{ st?.episode?.citationClicked ?? 0 }} 条；
          信念 {{ st?.belief?.active ?? 0 }} 条活跃，
          已被反驳 {{ st?.belief?.contradicted ?? 0 }} 条。
        </div>
        <div class="cog-rows">
          <div class="cog-row">
            <div class="cog-row-main">
              <div class="cog-row-label">情节保留天数</div>
              <div class="cog-row-hint">超期的轨迹连同其步骤与引证一起清理。</div>
            </div>
            <n-input-number
              size="small"
              class="cog-num"
              :value="num('episode_keep_days', 180)"
              :min="7"
              :max="3650"
              @update:value="(v) => setNum('episode_keep_days', v)"
            />
          </div>
        </div>
        <div class="cog-actions">
          <GuaxiIconButton
            :icon="Wrench20Regular"
            label="立即维护"
            hint="现在就做每天那一轮：兴趣衰减、裁剪过期轨迹、过期提醒。不动你的笔记。"
            :loading="busy.maint"
            @click="runMaint"
          />
          <GuaxiIconButton
            quaternary
            :icon="ArrowClockwise20Regular"
            label="按文件重建画像派生层"
            hint="以 profile.md 为唯一真相重算派生数据。手工删掉的兴趣不会被重新加回来。"
            @click="rederive"
          />
        </div>
      </n-collapse-item>
    </n-collapse>
  </div>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import {
  Alert20Regular,
  Archive20Regular,
  ArrowClockwise20Regular,
  ArrowSync20Regular,
  ArrowTrendingLines20Regular,
  ArrowUndo20Regular,
  Beaker20Regular,
  BrainCircuit20Regular,
  Broom20Regular,
  CalendarClock20Regular,
  Copy20Regular,
  Cursor20Regular,
  Database20Regular,
  DatabaseSearch20Regular,
  DismissCircle20Regular,
  Edit20Regular,
  Eye20Regular,
  Flash20Regular,
  Gauge20Regular,
  Globe20Regular,
  History20Regular,
  Lightbulb20Regular,
  Mail20Regular,
  Person20Regular,
  PlugConnected20Regular,
  Save20Regular,
  ShieldKeyhole20Regular,
  Warning20Regular,
  Wrench20Regular,
} from '@vicons/fluent'
import GuaxiIconButton from '../GuaxiIconButton.vue'
import { useCognitionStore, PERCEPT_SOURCE_LABELS } from '../../stores/cognitionStore'
import { sourceIcon } from '../../utils/cognitionIcons'
import { createLogger } from '../../utils/logger'

const logger = createLogger('CognitionSetting')
const store = useCognitionStore()
const message = useMessage()

const st = computed(() => store.status)
const enabled = computed(() => store.enabled)
const busy = ref({
  train: false,
  audit: false,
  mcp: false,
  profile: false,
  maint: false,
  purge: false,
  score: false,
  belief: false,
  contradict: false,
  activity: false,
})
const audit = ref([])
const mcp = ref(null)
const profileDraft = ref('')
const mineKb = ref(null)
const mineN = ref(40)
const beliefKb = ref(null)
const kbOptions = ref([])

const acceptPct = computed(() => {
  const r = st.value?.nudge?.acceptRate14d
  return r == null ? '—' : `${Math.round(r * 100)}%`
})

const weightsText = computed(() => {
  const w = st.value?.recallWeights || {}
  const order = ['semantic', 'graph', 'percept', 'episodic', 'memory']
  return order.map((k) => `${k} ${(w[k] ?? 0).toFixed(2)}`).join('  ')
})

const enabledToolCount = computed(() => store.tools.filter((t) => t.enabled).length)

/**
 * 工具按等级分组。
 *
 * 40 条平铺看不出结构——用户关心的是「这类能力开了几个、哪些每次都会问我」，
 * 不是逐条读描述。分组之后每个开关下面就是它管的那些工具。
 */
const toolGroups = computed(() => {
  const order = ['read', 'write', 'network', 'exec', 'gui']
  const byTier = new Map(order.map((k) => [k, []]))
  for (const tool of store.tools) {
    const list = byTier.get(tool.tier)
    if (list) list.push(tool)
  }
  return order.map((k) => ({
    key: k,
    label: { read: '只读', write: '写入', network: '出网', exec: '重活', gui: '操控本机' }[k] || k,
    tools: byTier.get(k) || [],
  }))
})

const TIER_HINTS = {
  read: '只读：召回、图谱查询、收件箱、任务、画像。始终开启——关了大脑就瞎了。',
  write: '写入：新建笔记、加标签、建关联、改状态、改画像。会真的改你的文件。',
  network: '出网：搜索、加入抓取队列。抓回的内容进收件箱，不会直接进知识库。',
  exec: '重活：重建索引、生成向量、抽取信念、生成简报。会拉起外部进程、耗时较长。',
  gui: '操控本机：截屏、枚举/激活窗口、移动鼠标、打字、读写剪贴板。'
    + '开启后 AI 会真的接管你的鼠标和键盘——每一次动作仍然要你逐条确认，'
    + '「操作电脑」模式还需要模型支持图像输入。',
}
const TIER_ICONS = {
  read: Eye20Regular,
  write: Edit20Regular,
  network: Globe20Regular,
  exec: Flash20Regular,
  // 操控本机用鼠标指针，和「执行重活」的闪电区分开。
  gui: Cursor20Regular,
}
const tierRows = computed(() =>
  (st.value?.tiers || []).map(([key, granted]) => ({
    key,
    granted,
    icon: TIER_ICONS[key] || ShieldKeyhole20Regular,
    label: { read: '只读', write: '写入', network: '出网', exec: '重活', gui: '操控本机' }[key] || key,
    hint: TIER_HINTS[key] || '',
  })),
)

const SOURCE_HINTS = {
  crawl: '整站抓取与文章深解析的产物。',
  rss: 'RSS / Atom 订阅源。',
  capture: '浏览器插件、手机采集、OCR 工具送进来的内容。',
  file: '本地知识库目录里新增或改动的文件。',
  clipboard: '你复制的内容。信噪比最高，但也最敏感——默认关闭。',
  screen: '定时对屏幕做 OCR。最具侵入性，默认关闭，建议配合应用白名单使用。',
  manual: '你手动粘贴或通过工具录入的内容。',
  note: '阅读器里加进笔记的摘录。以你自己的资料身份参与打分与召回，不占收件箱。',
  library: '资料库里新导入 / 新转换的文档。日常维护增量扫描，不占收件箱。',
}
const sourceLabel = (k) => PERCEPT_SOURCE_LABELS[k] || k
const sourceHint = (k) => SOURCE_HINTS[k] || ''

/** 可调项统一从 cognition_status 回读，避免输入框与真实值不一致。 */
const num = (key, fallback) => {
  const s = st.value
  if (!s) return fallback
  const direct = {
    deliberate_interval_secs: s.deliberateIntervalSecs,
    deliberate_modelid: s.deliberateModelid,
    max_interrupts_per_day: s.maxInterruptsPerDay,
    max_interrupts_per_hour: s.maxInterruptsPerHour,
    episode_keep_days: s.episodeKeepDays,
    daily_token_budget: s.nudge?.tokenBudget,
    percept_daily_cap: s.percept?.dailyCap,
    percept_unread_keep_days: s.percept?.unreadKeepDays,
  }
  const v = direct[key]
  return v == null ? fallback : v
}

async function setNum(key, value) {
  if (value == null) return
  try {
    await store.setSetting(key, value)
  } catch (e) {
    message.error(String(e))
  }
}

async function setVoiceBroadcast(value) {
  try {
    await store.setSetting('voice_broadcast', value ? '1' : '0')
  } catch (e) {
    message.error(String(e))
  }
}

async function toggleEnabled(v) {
  try {
    await store.setEnabled(v)
    message.success(v ? '认知层已开启' : '认知层已关闭')
  } catch (e) {
    message.error(String(e))
  }
}

async function setSource(kind, v) {
  try {
    await store.setSourceEnabled(kind, v)
    if (v && (kind === 'clipboard' || kind === 'screen')) {
      message.info('已开启。采集到的内容都会出现在「大脑 → 收件箱」里，可随时逐条删除。')
    }
  } catch (e) {
    message.error(String(e))
  }
}

/**
 * 手动清一次未读抓取内容。
 *
 * 这里刻意传 0 而不是设置里的保留天数：用户点这个按钮的意思是"现在就把没看的清掉"，
 * 而不是"按天数规则再跑一遍"。定期清理仍然按 percept_unread_keep_days 走。
 */
async function purgeUnread() {
  busy.value.purge = true
  try {
    const n = await store.purgePercepts(0)
    message.success(n ? `已清理 ${n} 条未读抓取内容` : '没有可清理的内容')
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.purge = false
  }
}

async function scoreNow() {
  busy.value.score = true
  try {
    const n = await store.scorePercepts(48)
    message.success(n ? `已打分 ${n} 条` : '没有待打分的条目')
  } catch (e) {
    message.warning(`打分暂不可用：${e}`)
  } finally {
    busy.value.score = false
  }
}

async function setTier(key, v) {
  try {
    await store.setTier(key, v)
    if (v && key !== 'read') {
      message.info('已授权。所有调用都会记入审计，可在下方查看。')
    }
  } catch (e) {
    message.error(String(e))
  }
}

async function loadAudit() {
  busy.value.audit = true
  try {
    audit.value = await store.toolAudit(80)
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.audit = false
  }
}

// ── 活动总线（只读调试视图）
const activityStats = computed(() => store.activityStats)
const APP_LABELS = {
  repo: '资料库',
  reader: '阅读',
  editor: '编辑',
  search: '搜索',
  mail: '邮件',
  task: '任务',
  git: 'Git',
}
const KIND_LABELS = {
  open: '打开',
  read: '阅读',
  edit: '保存',
  note: '摘录',
  highlight: '高亮',
  search: '搜索',
  convert: '转换',
  quiz: '自测',
}

/** 列表里显示「书名 / 文件名 / 查询串」，原始路径放 title。 */
const activityTarget = (row) => {
  let meta = null
  try {
    meta = row?.meta ? JSON.parse(row.meta) : null
  } catch {
    meta = null
  }
  const name = meta?.fileName || meta?.importFileName || ''
  if (name) return String(name)
  if (row?.kind === 'search') return String(row.targetId || '')
  const raw = String(row?.targetId || row?.path || '')
  if (!raw) return ''
  const parts = raw.replace(/\\/g, '/').split('/').filter(Boolean)
  return parts.length ? parts[parts.length - 1] : raw
}

async function loadActivity() {
  busy.value.activity = true
  try {
    await Promise.all([store.loadActivity({ limit: 80 }), store.refreshActivityStats(7)])
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.activity = false
  }
}

// ── 长期记忆
const MEMORY_KIND_OPTIONS = [
  { label: '事实', value: 'fact' },
  { label: '偏好', value: 'preference' },
  { label: '疑问', value: 'question' },
  { label: '技能', value: 'skill' },
  { label: '目标', value: 'goal' },
]
const memoryKind = ref('fact')
const memoryText = ref('')
const manualMemoryCount = computed(
  () => store.memories.filter((m) => m.source === 'manual').length,
)
const memoryKindLabel = (kind) =>
  MEMORY_KIND_OPTIONS.find((o) => o.value === kind)?.label || kind
const memoryTagType = (kind) =>
  ({
    preference: 'info',
    question: 'warning',
    skill: 'success',
    goal: 'error',
  })[kind] || 'default'

async function loadMemories() {
  await Promise.all([store.loadMemories(), store.loadMemoryFilePath()])
}

async function addMemory() {
  const text = memoryText.value.trim()
  if (!text) return
  try {
    await store.addMemory(memoryKind.value, text)
    memoryText.value = ''
    message.success('已记住')
  } catch (e) {
    message.error(String(e))
  }
}

async function forgetMemory(memory) {
  try {
    await store.forgetMemory(memory.id)
    message.success('已忘记')
  } catch (e) {
    message.error(String(e))
  }
}

async function consolidateMemory() {
  try {
    const r = await store.consolidateMemory()
    message.success(`巩固完成：新增 ${r.created}，合并 ${r.merged}，退休 ${r.retired}`)
  } catch (e) {
    message.error(String(e))
  }
}

// ── 引证排序器
async function trainRanker() {
  try {
    const r = await store.trainRanker()
    if (r.accepted) {
      message.success(
        `排序器 v${r.version} 已生效：AUC ${r.auc.toFixed(3)}（基线 ${r.aucBase.toFixed(3)}）`,
      )
    } else {
      message.info(`未过门禁：${r.note}`)
    }
  } catch (e) {
    message.error(String(e))
  }
}

async function resetRanker() {
  try {
    await store.resetRanker()
    message.success('已回滚：召回按纯 RRF')
  } catch (e) {
    message.error(String(e))
  }
}

// ── 任务接入
const taskCategories = computed(() => store.taskCategories)
const TASK_CATEGORY_HINTS = {
  work: '工作待办。默认接入；不想让工作进入大脑就关掉它。',
  life: '生活待办。',
  study: '学习待办。',
}
const taskCategoryHint = (key) => TASK_CATEGORY_HINTS[key] || ''
const taskCategoryLabel = (key) =>
  store.taskCategories?.categories?.find((c) => c.key === key)?.label || key

async function setTaskCategory(key, enabled) {
  const allowed = new Set(store.taskCategories?.allowed || [])
  if (enabled) allowed.add(key)
  else allowed.delete(key)
  try {
    await store.setTaskCategories([...allowed])
  } catch (e) {
    message.error(String(e))
  }
}

async function loadMcp() {
  busy.value.mcp = true
  try {
    mcp.value = await store.mcpInfo()
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.mcp = false
  }
}

async function copyMcp() {
  if (!mcp.value) return
  try {
    await writeText(mcp.value.clientConfig)
    message.success('已复制')
  } catch (e) {
    logger.warn('copy failed', e)
    message.warning('复制失败，请手动选中复制')
  }
}

async function loadProfile() {
  await store.loadProfile()
  profileDraft.value = store.profile?.markdown || ''
}

function resetProfileDraft() {
  profileDraft.value = store.profile?.markdown || ''
}

async function saveProfile() {
  busy.value.profile = true
  try {
    await store.saveProfile(profileDraft.value)
    message.success('画像已保存，立即生效')
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.profile = false
  }
}

async function removeInterest(topic) {
  try {
    await store.removeInterest(topic)
  } catch (e) {
    message.error(String(e))
  }
}

async function trainPolicy() {
  busy.value.train = true
  try {
    const r = await store.trainPolicy()
    message[r.trained ? 'success' : 'info'](r.reason)
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.train = false
  }
}

async function resetPolicy() {
  try {
    await store.resetPolicy()
    message.success('已回滚到基线策略，反馈数据保留')
  } catch (e) {
    message.error(String(e))
  }
}

async function mine() {
  if (!mineKb.value) return
  try {
    const r = await store.mineEvalSet(mineKb.value, { targetN: mineN.value })
    message.success(`扫描 ${r.scanned} 个章节，新增 ${r.added} 条样本（失败 ${r.failures}）`)
  } catch (e) {
    message.error(String(e))
  }
}

async function runEval() {
  try {
    const r = await store.runEval()
    message.success(
      `${r.n} 条样本：hit@1 ${r.hitAt1.toFixed(3)}，hit@5 ${r.hitAt5.toFixed(3)}，MRR ${r.mrr.toFixed(4)}`,
    )
  } catch (e) {
    message.warning(String(e))
  }
}

async function tune() {
  try {
    const r = await store.tuneRecall()
    message[r.accepted ? 'success' : 'info'](r.note)
  } catch (e) {
    message.warning(String(e))
  }
}

async function rollbackWeights() {
  try {
    await store.rollbackRecallWeights()
    message.success('召回权重已回到内置缺省，评测历史保留')
  } catch (e) {
    message.error(String(e))
  }
}

async function runMaint() {
  busy.value.maint = true
  try {
    const r = await store.maintenance({
      decayInterests: true,
      pruneEpisodes: true,
      expireNudges: true,
      rederiveProfile: false,
    })
    message.success(
      `已完成：衰减 ${r.decayed ?? 0} 项兴趣，裁剪 ${r.pruned ?? 0} 段轨迹，过期 ${r.expired ?? 0} 条提醒`,
    )
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.maint = false
  }
}

async function rederive() {
  try {
    const r = await store.maintenance({
      decayInterests: false,
      pruneEpisodes: false,
      expireNudges: false,
      rederiveProfile: true,
    })
    await loadProfile()
    message.success(`已按文件重建，手工兴趣 ${r.interests ?? 0} 项`)
  } catch (e) {
    message.error(String(e))
  }
}

const shortTime = (iso) => {
  if (!iso) return ''
  const s = String(iso)
  return s.length > 19 ? s.slice(5, 19).replace('T', ' ') : s
}

watch(
  () => store.profile?.markdown,
  (v) => {
    if (v != null && !profileDraft.value) profileDraft.value = v
  },
)

const beliefStatusLabel = (s) =>
  ({ active: '活跃', contradicted: '已被反驳', archived: '已归档' }[s] || s || '')

async function loadBeliefs() {
  try {
    await store.refreshBeliefs({ kbName: beliefKb.value || '' })
  } catch (e) {
    message.error(String(e))
  }
}

async function extractBeliefs() {
  if (!beliefKb.value) {
    message.warning('请先选择知识库')
    return
  }
  busy.value.belief = true
  try {
    const r = await store.extractBeliefs(beliefKb.value)
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
    await store.refreshBeliefs({ kbName: beliefKb.value || '' })
  } catch (e) {
    message.error(String(e))
  } finally {
    busy.value.contradict = false
  }
}

async function setBeliefStatus(id, status) {
  try {
    await store.setBeliefStatus(id, status)
  } catch (e) {
    message.error(String(e))
  }
}

async function removeBelief(id) {
  try {
    await store.removeBelief(id)
  } catch (e) {
    message.error(String(e))
  }
}

onMounted(async () => {
  await store.refreshStatus()
  await Promise.all([
    store.refreshSources(),
    store.refreshTools(),
    loadMemories(),
    store.loadRankerStatus(),
    store.loadTaskCategories(),
  ])
  try {
    const summaries = await invoke('kg_kb_summaries')
    kbOptions.value = (summaries || [])
      .map((k) => k.knowledge_base_name)
      .filter(Boolean)
      .map((name) => ({ label: name, value: name }))
  } catch (e) {
    logger.debug('kg_kb_summaries failed', e)
    kbOptions.value = []
  }
  if (!beliefKb.value) beliefKb.value = kbOptions.value[0]?.value || null
  if (beliefKb.value) store.refreshBeliefs({ kbName: beliefKb.value }).catch(() => {})
})
</script>

<style scoped>
/* 排版基线：与主界面（编辑器 / 偏好设置）用同一套字体与字号。
   次要文字统一走 --guaxi-text-secondary，不用 opacity 调暗——
   这个变量在两套主题下都是校准过的可读灰，叠 opacity 会把字压到看不清。 */
.cog-set {
  padding: 14px 18px 36px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-primary);
}
.cog-set-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 12px;
}
.cog-set-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 17px;
  font-weight: 600;
}
.cog-set-title :deep(.n-icon) {
  font-size: 20px;
  color: var(--n-color-primary, #2dd4bf);
}
.cog-set-sub {
  font-size: 13px;
  line-height: 1.8;
  color: var(--guaxi-text-secondary);
  margin-top: 4px;
  max-width: 680px;
}
.cog-set-warn {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 13px;
  line-height: 1.8;
  padding: 9px 12px;
  margin-bottom: 12px;
  border-radius: 4px;
  border-left: 3px solid rgba(217, 138, 43, 0.85);
  background: rgba(217, 138, 43, 0.1);
}
.cog-set-warn :deep(.n-icon) {
  font-size: 18px;
  flex: 0 0 auto;
  margin-top: 2px;
  color: rgba(217, 138, 43, 1);
}

/* 折叠区标题：图标 + 文字。图标压在次要色，标题文字仍是主色。 */
.cog-sec {
  display: inline-flex;
  align-items: center;
  gap: 7px;
}
.cog-sec :deep(.n-icon) {
  font-size: 18px;
  color: var(--guaxi-text-secondary);
}
.cog-note {
  font-size: 13px;
  line-height: 1.9;
  color: var(--guaxi-text-secondary);
  margin-bottom: 12px;
  max-width: 760px;
}
.cog-sub {
  font-size: 14px;
  font-weight: 600;
  margin: 16px 0 8px;
  color: var(--guaxi-text-primary);
}

.cog-rows {
  display: flex;
  flex-direction: column;
}
.cog-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 9px 0;
  border-bottom: 1px solid var(--guaxi-border);
}
.cog-row:last-child {
  border-bottom: none;
}
.cog-row-main {
  flex: 1 1 auto;
  min-width: 0;
}
.cog-row-label {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 14px;
  line-height: 1.6;
}
.cog-row-label :deep(.n-icon) {
  font-size: 16px;
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
}
.cog-row-hint {
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  line-height: 1.8;
  margin-top: 3px;
}
.cog-row-hint code,
.cog-code {
  font-size: 12.5px;
  padding: 1px 5px;
  border-radius: 3px;
  background: rgba(128, 128, 128, 0.16);
  word-break: break-all;
}
.cog-num {
  width: 104px;
  flex: 0 0 auto;
}
.cog-num-wide {
  width: 140px;
  flex: 0 0 auto;
}
.cog-select {
  width: 190px;
}
/* 动作按钮现在是"图标 + 文字"，间距要比纯文字按钮松一点才不显得挤。 */
.cog-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 10px;
}
.cog-stat-line {
  font-size: 13px;
  line-height: 1.9;
  color: var(--guaxi-text-secondary);
  margin-top: 10px;
}

.cog-policy {
  margin-top: 12px;
  padding: 11px 13px;
  border-radius: 5px;
  background: rgba(128, 128, 128, 0.08);
}
.cog-policy-line {
  font-size: 13px;
  line-height: 1.9;
}

.cog-tool-list {
  margin-top: 10px;
  max-height: 320px;
  overflow: auto;
  border: 1px solid var(--guaxi-border);
  border-radius: 5px;
  padding: 6px 9px;
}
.cog-tool {
  display: flex;
  align-items: baseline;
  gap: 9px;
  padding: 3px 0;
  font-size: 12.5px;
  line-height: 1.7;
}
.cog-tool-tier {
  flex: 0 0 auto;
  width: 56px;
  text-align: center;
  border-radius: 3px;
  font-size: 11.5px;
  padding: 1px 0;
  background: rgba(128, 128, 128, 0.18);
}
.tier-write {
  background: rgba(217, 138, 43, 0.22);
}
.tier-network {
  background: rgba(59, 130, 196, 0.22);
}
.tier-exec {
  background: rgba(224, 79, 95, 0.2);
}
.cog-tool-name {
  flex: 0 0 auto;
  font-family: var(--guaxi-editor-font-family);
  min-width: 200px;
}
.cog-tool-name.off {
  color: var(--guaxi-text-secondary);
  text-decoration: line-through;
}
.cog-tool-desc {
  color: var(--guaxi-text-secondary);
}

/* 等级分组标题：小一号、压暗，只做分隔，不和工具行抢注意力。 */
.cog-tool-group-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 14px 0 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-secondary);
  letter-spacing: 0.04em;
}

.cog-tool-group-count {
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

/* 「逐次确认」标记：只给写/操控类工具，说明它不会一次授权就连续动手。 */
.cog-tool-confirm {
  flex: 0 0 auto;
  font-size: 11px;
  padding: 0 5px;
  border-radius: 3px;
  color: #b47d1a;
  background: rgba(180, 125, 26, 0.12);
  white-space: nowrap;
}

.cog-audit,
.cog-evals {
  margin-top: 8px;
  max-height: 260px;
  overflow: auto;
  font-size: 12.5px;
  font-variant-numeric: tabular-nums;
}
.cog-audit-row,
.cog-eval {
  display: flex;
  gap: 10px;
  padding: 3px 0;
  border-bottom: 1px solid var(--guaxi-border);
}
.cog-audit-row.bad {
  color: #e04f5f;
}
.cog-audit-at,
.cog-eval-at {
  flex: 0 0 auto;
  width: 110px;
  color: var(--guaxi-text-secondary);
}
.cog-audit-caller {
  flex: 0 0 auto;
  width: 62px;
  color: var(--guaxi-text-secondary);
}
.cog-audit-tool {
  flex: 0 0 auto;
  width: 200px;
  font-family: var(--guaxi-editor-font-family);
}
.cog-audit-ms {
  flex: 0 0 auto;
  width: 60px;
  text-align: right;
  color: var(--guaxi-text-secondary);
}
.cog-audit-err {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cog-eval-tag {
  flex: 0 0 auto;
  width: 56px;
}

.cog-eval.ok .cog-eval-tag {
  color: #5b9e6a;
}

/* 活动总线调试视图：与工具审计同一套行样式。 */
.cog-activity {
  margin-top: 8px;
  max-height: 300px;
  overflow: auto;
  font-size: 12.5px;
  font-variant-numeric: tabular-nums;
}

.cog-activity-row {
  display: flex;
  gap: 10px;
  padding: 3px 0;
  border-bottom: 1px solid var(--guaxi-border);
}

.cog-activity-at {
  flex: 0 0 auto;
  width: 118px;
  color: var(--guaxi-text-secondary);
}

.cog-activity-app {
  flex: 0 0 auto;
  width: 52px;
  color: var(--guaxi-text-secondary);
}

.cog-activity-kind {
  flex: 0 0 auto;
  width: 42px;
  color: var(--guaxi-text-secondary);
}

.cog-activity-target {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cog-activity-progress {
  flex: 0 0 auto;
  width: 46px;
  text-align: right;
  color: var(--guaxi-text-secondary);
}

/* 长期记忆 */
.memory-add {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  margin: 10px 0 6px;
}

.memory-add :deep(.n-input) {
  flex: 1 1 260px;
  min-width: 200px;
}

.memory-list {
  margin-top: 8px;
  max-height: 320px;
  overflow: auto;
  font-size: 12.5px;
}

.memory-row {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 6px 0;
  border-bottom: 1px solid var(--guaxi-border);
}

.memory-text {
  flex: 1 1 auto;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  white-space: normal;
  line-height: 1.5;
}

.memory-meta {
  flex: 0 0 auto;
  width: 86px;
  text-align: right;
  color: var(--guaxi-text-secondary);
}
.cog-eval-m {
  flex: 0 0 auto;
  width: 92px;
}
.cog-eval-note {
  flex: 1 1 auto;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cog-mcp .cog-code {
  display: block;
  margin-top: 8px;
  padding: 9px 11px;
  white-space: pre;
  overflow: auto;
  max-height: 200px;
  line-height: 1.7;
}
.cog-textarea {
  margin-top: 8px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 13px;
}
.cog-interests {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.cog-w {
  font-style: normal;
  color: var(--guaxi-text-secondary);
  margin-left: 4px;
  font-size: 11.5px;
}
.cog-mine {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-top: 10px;
}

/* naive-ui 的折叠标题默认比正文小一档且更淡，
   在这一页会显得比它下面的说明文字还弱，所以拉回 15px/600 主色。 */
.cog-set :deep(.n-collapse-item__header-main) {
  font-size: 15px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}
.cog-set :deep(.n-input__input-el),
.cog-set :deep(.n-input__textarea-el),
.cog-set :deep(.n-base-selection-label),
.cog-set :deep(.n-button) {
  font-family: var(--guaxi-editor-font-family);
}

/* 知识主张：窄屏也能一行读完，操作按钮贴右 */
.cog-beliefs {
  margin-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.cog-belief {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
}
.cog-belief-st {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
