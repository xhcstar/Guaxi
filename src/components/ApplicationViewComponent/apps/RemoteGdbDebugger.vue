<template>
  <div class="gdb-root">
    <div class="gdb-bar">
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="transport" size="small" :options="transportOptions" :disabled="!!sessionId" style="width:120px" />
      </template>调试通道。SSH 走库内连接档案；串口 / Telnet 用「实时终端」里已建立的会话——调试网络相关程序、只能从串口控制台操作时用这两种</n-tooltip>
      <template v-if="transport === 'ssh'">
        <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="选择 SSH 档案" style="width:180px" />
      </template>
      <template v-else>
        <n-select v-model:value="consoleSessionId" size="small" :options="consoleSessionOptions" :placeholder="transport === 'serial' ? '选择已连接的串口会话' : '选择已连接的 Telnet 会话'" style="width:230px" />
        <n-tooltip trigger="hover"><template #trigger>
          <button class="gdb-ib" @click="reloadConsoleSessions"><Icon :icon="refreshIcon" width="14" height="14" /></button>
        </template>刷新可用的串口 / Telnet 会话（先在「实时终端」里连上）</n-tooltip>
      </template>
      <n-radio-group v-model:value="mode" size="small" :disabled="!!sessionId">
        <n-radio-button value="launch">启动程序</n-radio-button>
        <n-radio-button value="attach">附加进程</n-radio-button>
      </n-radio-group>
      <n-input v-model:value="program" size="small" :placeholder="mode === 'attach' ? '可执行文件(可空，仅符号)' : '可执行文件路径(可空=仅连gdb)'" style="width:230px" />
      <n-input-number v-if="mode === 'attach'" v-model:value="attachPid" size="small" :min="1" :show-button="false" placeholder="PID" style="width:110px" />
      <n-select v-if="mode === 'attach' && transport === 'ssh'" v-model:value="escalate" size="small" :options="escalateOptions" style="width:110px" title="提权方式" />
      <n-input v-if="mode === 'attach' && transport === 'ssh' && escalate === 'su'" v-model:value="escalatePassword" type="password" size="small" placeholder="root 密码" show-password-on="click" style="width:130px" />
      <n-input v-else-if="mode !== 'attach'" v-model:value="argsText" size="small" placeholder="参数(空格分隔)" style="width:150px" />
      <n-tooltip trigger="hover"><template #trigger>
        <button class="gdb-ib primary lg gdb-launch" :disabled="!!sessionId || starting" @click="start">
          <Icon :icon="starting ? refreshIcon : playCircleIcon" width="17" height="17" :class="{ 'gdb-spin': starting }" />
        </button>
      </template>{{ sessionId ? '会话进行中，先“结束”再重新启动' : (mode === 'attach' ? '启动 gdb 并附加到上面的 PID' : '启动远端 gdb -i=mi') }}</n-tooltip>
      <n-tooltip v-if="sessionId" trigger="hover"><template #trigger>
        <button class="gdb-ib lg" @click="stop"><Icon :icon="stopIcon" width="17" height="17" /></button>
      </template>结束当前调试会话</n-tooltip>
      <span class="gdb-status">{{ statusText }}</span>
      <span v-if="busy" class="gdb-busy" :title="busy.cmd">
        <Icon :icon="refreshIcon" width="13" height="13" class="gdb-spin" />
        {{ gdbReady ? '等待对端响应' : '正在建立（su 认证 / gdb 启动）' }} {{ busyElapsed }}s
        <span v-if="gdbReady && busyElapsed >= WAIT_HINT_SECS" class="gdb-busy-slow">（较慢：对端可能在加载符号或链路卡顿，可继续等或点“中断”）</span>
      </span>
      <span v-else-if="notice" class="gdb-notice">{{ notice }}</span>
      <span v-if="copiedTag" class="gdb-notice">{{ copiedTag }}</span>
      <span v-if="error" class="gdb-error">{{ error }}</span>
    </div>

    <div class="gdb-palette" v-if="sessionId">
      <n-tooltip v-for="c in palette" :key="c.label" trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg primary" @click="paletteRun(c)">
            <Icon :icon="c.icon" width="17" height="17" />
          </button>
        </template>
        {{ c.label }}
      </n-tooltip>
      <span class="gdb-pal-sep"></span>
      <n-tooltip trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg" @click="refreshAll"><Icon :icon="refreshIcon" width="17" height="17" /></button>
        </template>
        全量刷新（线程/调用栈/局部变量/寄存器/Watch/源码）
      </n-tooltip>
      <n-tooltip trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg" @click="stop"><Icon :icon="stopIcon" width="17" height="17" /></button>
        </template>
        结束调试会话
      </n-tooltip>
      <n-tooltip trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg" @click="leftCollapsed = !leftCollapsed">
            <Icon :icon="panelLeftIcon" width="17" height="17" :class="{ 'gdb-muted': leftCollapsed }" />
          </button>
        </template>
        {{ leftCollapsed ? '显示左栏（断点/调用栈/变量）' : '隐藏左栏，源码区更宽' }}
      </n-tooltip>
      <n-tooltip trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg" @click="rightCollapsed = !rightCollapsed">
            <Icon :icon="panelRightIcon" width="17" height="17" :class="{ 'gdb-muted': rightCollapsed }" />
          </button>
        </template>
        {{ rightCollapsed ? '显示右栏（线程/寄存器/内存）' : '隐藏右栏，源码区更宽' }}
      </n-tooltip>
      <n-tooltip trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg" @click="copyPanel(consoleText, '控制台')"><Icon :icon="copyIcon" width="17" height="17" /></button>
        </template>
        复制全部控制台输出
      </n-tooltip>
      <div class="gdb-fontctl" title="控制台/源码字号">
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize - 1)"><Icon :icon="minusIcon" width="13" height="13" /></button>
        <span class="gdb-fsz">{{ gdbFontSize }}</span>
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize + 1)"><Icon :icon="plusIcon" width="13" height="13" /></button>
      </div>
    </div>

    <div class="gdb-body" v-if="sessionId">
      <!-- ===== 左栏：断点 / 调用栈 / 局部变量 / 变量树 ===== -->
      <div class="gdb-side" v-show="!leftCollapsed" :style="{ width: gdbSideWidth + 'px' }">
        <!-- 断点 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('breaks')">
            <Icon :icon="panelCollapsed.breaks ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="bugIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">断点</span>
            <span class="gdb-panel-n">{{ breaks.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshBreaks"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新断点列表</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!breaks.length" @click.stop="copyPanel(breaksText, '断点')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制全部断点</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.breaks" :style="{ height: panelH.breaks + 'px' }">
            <div class="gdb-actions">
              <n-input v-model:value="bpInput" size="tiny" placeholder="main / file.c:42 / func" style="flex:1" @keyup.enter="addBreak" />
              <n-tooltip trigger="hover"><template #trigger>
                <button class="gdb-ib primary" @click="addBreak"><Icon :icon="plusIcon" width="14" height="14" /></button>
              </template>在输入位置加断点</n-tooltip>
            </div>
            <div class="gdb-scroll">
              <table class="gdb-table">
                <thead><tr><th>#</th><th>位置</th><th>命中</th><th></th></tr></thead>
                <tbody>
                  <tr v-for="b in breaks" :key="b.number">
                    <td>{{ b.number }}</td>
                    <td class="gdb-ellip" :title="b.where">{{ b.where }}</td>
                    <td>{{ b.times }}</td>
                    <td><n-tooltip trigger="hover"><template #trigger>
                      <button class="gdb-ib danger" @click="delBreak(b.number)"><Icon :icon="trashIcon" width="13" height="13" /></button>
                    </template>删除断点 {{ b.number }}</n-tooltip></td>
                  </tr>
                  <tr v-if="!breaks.length"><td colspan="4" class="gdb-hint">加断点或点右上角刷新。</td></tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.breaks" @mousedown="panelResizeDown('breaks', $event)"></div>
        </section>

        <!-- 调用栈 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('callstack')">
            <Icon :icon="panelCollapsed.callstack ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="layersIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">调用栈</span>
            <span class="gdb-panel-n">{{ callStack.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshStack"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新调用栈</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!callStack.length" @click.stop="copyPanel(stackText, '调用栈')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制调用栈</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!callStack.length" @click.stop="sendStack"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把调用栈发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.callstack" :style="{ height: panelH.callstack + 'px' }">
            <div class="gdb-scroll">
              <div
                v-for="f in callStack"
                :key="f.level"
                class="gdb-sframe"
                :class="{ active: String(f.level) === String(curFrame) }"
                @click="selectFrame(f.level)"
                @dblclick.stop="copyRow(`#${f.level} ${f.func} @ ${f.fullname || f.file}${f.line ? ':' + f.line : ''}`)"
                :title="(f.fullname || f.file) + (f.line ? ':' + f.line : '') + '（双击复制此行）'"
              >
                <span class="gdb-sframe-lv">#{{ f.level }}</span>
                <span class="gdb-sframe-func">{{ f.func }}</span>
                <span class="gdb-sframe-loc">{{ shortFile(f.file) }}<template v-if="f.line">:{{ f.line }}</template></span>
              </div>
              <div v-if="!callStack.length" class="gdb-hint">停在断点时自动加载，或点右上角刷新。</div>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.callstack" @mousedown="panelResizeDown('callstack', $event)"></div>
        </section>

        <!-- 局部变量 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('locals')">
            <Icon :icon="panelCollapsed.locals ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="variableIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">局部变量</span>
            <span class="gdb-panel-n">{{ locals.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshLocals"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新当前帧局部变量</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!locals.length" @click.stop="copyPanel(localsText, '局部变量')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制局部变量</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!locals.length" @click.stop="sendLocals"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把局部变量发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.locals" :style="{ height: panelH.locals + 'px' }">
            <div class="gdb-scroll">
              <table class="gdb-table">
                <tbody>
                  <tr v-for="l in locals" :key="l.name" @dblclick="copyRow(`${l.name} = ${l.value}`)" title="双击复制该变量">
                    <td class="gdb-lname">{{ l.name }}</td>
                    <td class="gdb-val">{{ l.value }}</td>
                  </tr>
                  <tr v-if="!locals.length"><td colspan="2" class="gdb-hint">停在断点时显示当前帧局部变量。</td></tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.locals" @mousedown="panelResizeDown('locals', $event)"></div>
        </section>

        <!-- 变量树 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('vars')">
            <Icon :icon="panelCollapsed.vars ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="expandTreeIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">变量树</span>
            <span class="gdb-panel-n">{{ varRoots.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!varRoots.length" @click.stop="copyPanel(varTreeText, '变量树')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制变量树</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!varRoots.length" @click.stop="sendVarTree"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把变量树发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.vars" :style="{ height: panelH.vars + 'px' }">
            <div class="gdb-actions">
              <n-input v-model:value="exprInput" size="tiny" placeholder="*ptr / mystruct / arr" style="flex:1" @keyup.enter="addVar" />
              <n-tooltip trigger="hover"><template #trigger>
                <button class="gdb-ib primary" @click="addVar"><Icon :icon="wandIcon" width="14" height="14" /></button>
              </template>展开表达式为可折叠树</n-tooltip>
            </div>
            <div class="gdb-actions">
              <n-tooltip trigger="hover"><template #trigger>
                <n-select v-model:value="varFormat" size="tiny" :options="varFormatOptions" style="width:110px" @update:value="applyVarFormat" />
              </template>变量显示进制（自然/十六进制/十进制/二进制）</n-tooltip>
            </div>
            <div class="gdb-scroll">
              <div class="gdb-tree">
                <GdbVarNode v-for="n in varRoots" :key="n.name" :node="n" :depth="0" @expand="expandVar" />
              </div>
              <div v-if="!varRoots.length" class="gdb-hint">输入表达式，把复杂结构体展开成可折叠树。</div>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.vars" @mousedown="panelResizeDown('vars', $event)"></div>
        </section>
      </div>

      <div class="gdb-sideresize" v-show="!leftCollapsed" @mousedown="onGdbResizeDown"></div>

      <!-- ===== 中间：源码 + 控制台 ===== -->
      <div class="gdb-center">
        <div class="gdb-source" :style="{ height: sourceHeight + 'px' }">
          <div class="gdb-source-bar">
            <n-radio-group v-model:value="sourceMode" size="tiny">
              <n-radio-button value="local">本机源码</n-radio-button>
              <n-radio-button value="remote">编译机源码</n-radio-button>
            </n-radio-group>
            <n-tooltip v-if="sourceMode === 'remote'" trigger="hover"><template #trigger>
              <n-select v-model:value="sourceProfileId" size="tiny" :options="profileOptions" placeholder="SSH 档案(编译机)" style="width:170px" />
            </template>源码放在哪台 Linux 编译机上：选一个 SSH 档案，之后自动用 SSH 去该机器读源码</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <n-select v-model:value="sourceRule" size="tiny" :options="sourceRuleOptions" style="width:170px" />
            </template>源码定位规则。构建方式不同，调试信息里的路径与源码真实位置的关系也不同：
• 自动：依次试「原样 → 逐段砍前缀 → 常见构建根(/build,/src,/work…)替换 → 按文件名搜」，绝大多数情况都能命中；
• 原地构建：产物就在工程目录里，用「原样」或「自动」；
• chroot / docker 构建：容器内构建根（如 /build）会被换成你的源码根，用「自动」；
• 按文件名搜索：只知道文件名时用（要求文件名在工程内唯一）</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <n-input v-model:value="sourceRoot" size="tiny" :placeholder="sourceMode === 'local' ? '源码工程根目录（点右侧选）' : '编译机上的工程根目录（点右侧浏览）'" style="flex:1;min-width:200px" />
            </template>源码工程根：本机就选目录；编译机就浏览远端目录。选好后下面的「源码根」就是你告诉我的唯一信息，其余路径映射我来处理</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click="pickRoot"><Icon :icon="folderIcon" width="14" height="14" /></button>
            </template>{{ sourceMode === 'local' ? '选择本机源码根目录' : '浏览编译机上的目录' }}</n-tooltip>
            <n-tooltip v-if="sourceRule === 'prefix'" trigger="hover"><template #trigger>
              <n-input v-model:value="srcFromPrefix" size="tiny" placeholder="构建根前缀，如 /build" style="width:150px" />
            </template>调试信息里的构建根前缀（如 /build）；会被替换成上面的源码根</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib primary" @click="refreshSource"><Icon :icon="crosshairIcon" width="14" height="14" /></button>
            </template>按当前帧 file:line 定位源码并高亮</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :class="{ primary: followFrame }" @click="followFrame = !followFrame"><Icon :icon="followIcon" width="14" height="14" /></button>
            </template>{{ followFrame ? '跟随调用栈：停止/切帧/单步时自动跳到对应源码行（已开启）' : '跟随调用栈：停止/切帧/单步时自动跳到对应源码行（已关闭）' }}</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!sourceLines.length" @click="copyPanel(sourceText, '源码')"><Icon :icon="copyIcon" width="14" height="14" /></button>
            </template>复制源码</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!sourceLines.length" @click="sendSource"><Icon :icon="sendIcon" width="14" height="14" /></button>
            </template>把当前源码发到主界面编辑器</n-tooltip>
            <span class="gdb-src-loc" :title="sourceFile">{{ shortFile(sourceFile) }}<template v-if="sourceCurLine">:{{ sourceCurLine }}</template></span>
          </div>
          <div class="gdb-source-body" ref="sourceBodyRef" :style="{ fontSize: gdbFontSize + 'px' }">
            <div
              v-for="ln in sourceLines"
              :key="ln.n"
              class="gdb-srcline"
              :class="{ cur: ln.n === sourceCurLine, bp: hasBreakpointAt(ln.n) }"
              :ref="el => { if (ln.n === sourceCurLine) curLineEl = el }"
              @click="onSourceLineClick(ln)"
              @dblclick="toggleSourceBreakpoint(ln)"
              :title="`第 ${ln.n} 行 · 单击定位 / 双击切换断点`"
            >
              <span class="gdb-srcno" :title="`第 ${ln.n} 行（双击复制该行）`"
                @dblclick.stop="copyRow(String(ln.n).padStart(5) + '  ' + ln.text)">{{ ln.n }}</span>
              <span class="gdb-srcbp">{{ hasBreakpointAt(ln.n) ? '●' : '' }}</span>
              <span class="gdb-srctext">{{ ln.text }}</span>
            </div>
            <div v-if="!sourceLines.length" class="gdb-src-empty">
              <div class="gdb-hint">{{ sourceHint }}</div>
              <pre v-if="locateInfo" class="gdb-mem gdb-src-tried">{{ locateInfo }}</pre>
            </div>
          </div>
        </div>
        <div class="gdb-vresize" @mousedown="onSourceResizeDown"></div>
        <div class="gdb-console" ref="consoleRef" :style="{ fontSize: gdbFontSize + 'px' }" @dblclick="copyPanel(consoleText, '控制台')" title="双击复制全部控制台输出">
          <pre v-for="(l, i) in consoleLines" :key="i" :class="'gdb-l-' + l.kind" @dblclick.stop="copyRow(l.text)" :title="'双击复制该行'">{{ l.text }}</pre>
        </div>
        <div class="gdb-input">
          <n-input v-model:value="cmdInput" size="small" placeholder="输入 MI(-exec-next) 或 CLI(bt) 命令回车执行" @keyup.enter="submitCmd" />
          <n-tooltip trigger="hover"><template #trigger>
            <button class="gdb-ib primary lg" @click="submitCmd"><Icon :icon="playCircleIcon" width="16" height="16" /></button>
          </template>执行命令</n-tooltip>
        </div>
      </div>

      <!-- ===== 右栏：线程 / Watch / 寄存器 / 内存 / 宏 ===== -->
      <div class="gdb-sideresize" v-show="!rightCollapsed" @mousedown="onGdbRightResizeDown"></div>
      <div class="gdb-side gdb-side-right" v-show="!rightCollapsed" :style="{ width: gdbRightWidth + 'px' }">
        <!-- 线程 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('threads')">
            <Icon :icon="panelCollapsed.threads ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="listIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">线程</span>
            <span class="gdb-panel-n">{{ threads.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshThreads"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新线程列表</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!threads.length" @click.stop="copyPanel(threadsText, '线程')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制线程列表</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.threads" :style="{ height: panelH.threads + 'px' }">
            <div class="gdb-scroll">
              <div v-for="t in threads" :key="t.id" class="gdb-thread" :class="{ active: t.id === currentThread }" @click="selectThread(t.id)" @dblclick.stop="copyRow(threadText(t))" title="双击复制该线程">
                <div class="gdb-thread-h">线程 {{ t.id }} <span v-if="t.name">({{ t.name }})</span> — {{ t.state }}</div>
                <div class="gdb-frame" v-if="t.frame">#{{ t.frame.level }} {{ t.frame.func }} @ {{ t.frame.file }}:{{ t.frame.line }}</div>
              </div>
              <div v-if="!threads.length" class="gdb-hint">点右上角刷新，或在停下时自动加载。</div>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.threads" @mousedown="panelResizeDown('threads', $event)"></div>
        </section>

        <!-- Watch -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('watch')">
            <Icon :icon="panelCollapsed.watch ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="eyeIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">Watch</span>
            <span class="gdb-panel-n">{{ watches.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshWatch"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新并高亮变化</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!watches.length" @click.stop="copyPanel(watchText, 'Watch')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制全部 Watch</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!watches.length" @click.stop="sendWatch"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把 Watch 发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.watch" :style="{ height: panelH.watch + 'px' }">
            <div class="gdb-actions">
              <n-input v-model:value="watchInput" size="tiny" placeholder="钉住表达式" style="flex:1" @keyup.enter="addWatch" />
              <n-tooltip trigger="hover"><template #trigger>
                <button class="gdb-ib primary" @click="addWatch"><Icon :icon="plusIcon" width="14" height="14" /></button>
              </template>添加 Watch</n-tooltip>
            </div>
            <div class="gdb-scroll">
              <table class="gdb-table">
                <tbody>
                  <tr v-for="w in watches" :key="w.expr" :class="{ changed: w.changed }" @dblclick="copyRow(`${w.expr} = ${w.value}`)" title="双击复制该 Watch">
                    <td class="gdb-ellip" :title="w.expr">{{ w.expr }}</td>
                    <td class="gdb-val">{{ w.value }}</td>
                    <td><n-tooltip trigger="hover"><template #trigger>
                      <button class="gdb-ib danger" @click="removeWatch(w.expr)"><Icon :icon="trashIcon" width="13" height="13" /></button>
                    </template>移除该 Watch</n-tooltip></td>
                  </tr>
                  <tr v-if="!watches.length"><td colspan="3" class="gdb-hint">停在断点时自动刷新并高亮变化。</td></tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.watch" @mousedown="panelResizeDown('watch', $event)"></div>
        </section>

        <!-- 寄存器 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('regs')">
            <Icon :icon="panelCollapsed.regs ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="cpuIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">寄存器</span>
            <span class="gdb-panel-n">{{ registers.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="refreshRegisters"><Icon :icon="refreshIcon" width="13" height="13" /></button>
            </template>刷新寄存器</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!registers.length" @click.stop="copyPanel(registersText, '寄存器')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制寄存器</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!registers.length" @click.stop="sendRegisters"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把寄存器发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.regs" :style="{ height: panelH.regs + 'px' }">
            <div class="gdb-scroll">
              <table class="gdb-table gdb-regs">
                <thead><tr><th>寄存器</th><th>HEX</th><th>十进制</th></tr></thead>
                <tbody>
                  <tr v-for="r in registers" :key="r.name" @dblclick="copyRow(`${r.name} = ${r.hex} (${r.dec})`)" title="双击复制该寄存器">
                    <td class="gdb-rname">{{ r.name }}</td>
                    <td class="gdb-val">{{ r.hex }}</td>
                    <td class="gdb-val">{{ r.dec }}</td>
                  </tr>
                  <tr v-if="!registers.length"><td colspan="3" class="gdb-hint">停下后点右上角刷新。</td></tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.regs" @mousedown="panelResizeDown('regs', $event)"></div>
        </section>

        <!-- 内存 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('mem')">
            <Icon :icon="panelCollapsed.mem ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="memoryIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">内存</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!memText" @click.stop="copyPanel(memOutText, '内存')"><Icon :icon="copyIcon" width="13" height="13" /></button>
            </template>复制内存视图</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" :disabled="!memText" @click.stop="sendMem"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把内存视图发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.mem" :style="{ height: panelH.mem + 'px' }">
            <div class="gdb-actions">
              <n-input v-model:value="memAddr" size="tiny" placeholder="&buf 或 0x..." style="flex:1" @keyup.enter="readMem" />
              <n-input-number v-model:value="memCount" size="tiny" :min="1" :max="4096" :show-button="false" style="width:70px" />
              <n-tooltip trigger="hover"><template #trigger>
                <button class="gdb-ib primary" @click="readMem"><Icon :icon="memoryIcon" width="14" height="14" /></button>
              </template>读取内存（hex + ASCII）</n-tooltip>
            </div>
            <div class="gdb-scroll">
              <pre class="gdb-mem" @dblclick="copyRow(memText)" title="双击复制全部内存内容">{{ memText || '按十六进制 + ASCII 展示内存块。' }}</pre>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.mem" @mousedown="panelResizeDown('mem', $event)"></div>
        </section>

        <!-- 宏 -->
        <section class="gdb-panel">
          <header class="gdb-panel-h" @click="togglePanel('macros')">
            <Icon :icon="panelCollapsed.macros ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="wandIcon" width="13" height="13" class="gdb-panel-ico" />
            <span class="gdb-panel-t">宏</span>
            <span class="gdb-panel-n">{{ macros.length }}</span>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="saveMacros"><Icon :icon="downloadIcon" width="13" height="13" /></button>
            </template>导出宏到文件</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib" @click.stop="loadMacros"><Icon :icon="uploadIcon" width="13" height="13" /></button>
            </template>从文件导入宏</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.macros" :style="{ height: panelH.macros + 'px' }">
            <div class="gdb-actions">
              <n-input v-model:value="macroName" size="tiny" placeholder="宏名" style="width:80px" />
              <n-input v-model:value="macroCmds" size="tiny" placeholder="命令;命令" style="flex:1" />
              <n-tooltip trigger="hover"><template #trigger>
                <button class="gdb-ib primary" @click="addMacro"><Icon :icon="saveIcon" width="14" height="14" /></button>
              </template>保存为宏</n-tooltip>
            </div>
            <div class="gdb-scroll">
              <table class="gdb-table">
                <tbody>
                  <tr v-for="m in macros" :key="m.name">
                    <td class="gdb-ellip" :title="m.name">{{ m.name }}</td>
                    <td>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="gdb-ib primary" @click="runMacro(m)"><Icon :icon="playCircleIcon" width="14" height="14" /></button>
                      </template>运行该宏</n-tooltip>
                      <n-tooltip trigger="hover"><template #trigger>
                        <button class="gdb-ib danger" @click="removeMacro(m.name)"><Icon :icon="trashIcon" width="13" height="13" /></button>
                      </template>删除该宏</n-tooltip>
                    </td>
                  </tr>
                  <tr v-if="!macros.length"><td colspan="2" class="gdb-hint">把常用命令序列存成一键宏。</td></tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="gdb-panel-rz" v-show="!panelCollapsed.macros" @mousedown="panelResizeDown('macros', $event)"></div>
        </section>
      </div>
    </div>
    <div v-else class="gdb-empty">
      <template v-if="consoleLines.length">
        <div class="gdb-dead-note">会话已结束。以下是 gdb 输出（含失败原因），排查后可重新启动：</div>
        <div class="gdb-console gdb-console-dead" :style="{ fontSize: gdbFontSize + 'px' }">
          <pre v-for="(l, i) in consoleLines" :key="i" :class="'gdb-l-' + l.kind">{{ l.text }}</pre>
        </div>
        <n-tooltip trigger="hover"><template #trigger>
          <button class="gdb-ib lg" @click="consoleLines = []"><Icon :icon="trashIcon" width="16" height="16" /></button>
        </template>清空控制台输出</n-tooltip>
      </template>
      <template v-else>选择 SSH 档案与可执行文件，启动远端 gdb -i=mi 开始调试。</template>
    </div>

    <!-- 编译机目录浏览（选远端源码根） -->
    <n-modal v-model:show="showRemotePick" :mask-closable="false" style="width:640px;max-width:94vw">
      <div class="gdb-pick">
        <div class="gdb-pick-h">
          <Icon :icon="folderIcon" width="16" height="16" class="gdb-panel-ico" />
          <span>浏览编译机上的目录</span>
          <n-select v-model:value="sourceProfileId" size="small" :options="profileOptions" placeholder="SSH 档案" style="width:190px" @update:value="browseRemote(remotePath || '~')" />
          <button class="gdb-ib" @click="showRemotePick = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="gdb-pick-bar">
          <n-tooltip trigger="hover"><template #trigger>
            <button class="gdb-ib" @click="remoteUp"><Icon :icon="arrowUpIcon" width="15" height="15" /></button>
          </template>上级目录</n-tooltip>
          <n-input v-model:value="remotePath" size="small" style="flex:1" @keyup.enter="browseRemote(remotePath)" />
          <n-tooltip trigger="hover"><template #trigger>
            <button class="gdb-ib" @click="browseRemote(remotePath)"><Icon :icon="refreshIcon" width="15" height="15" /></button>
          </template>刷新</n-tooltip>
        </div>
        <div class="gdb-pick-list">
          <div v-if="remoteLoading" class="gdb-hint">加载中…</div>
          <div v-for="d in remoteDirs" :key="d" class="gdb-pick-item" @click="remoteEnter(d)">
            <Icon :icon="folderIcon" width="14" height="14" class="gdb-pick-ico" />{{ d }}
          </div>
          <div v-if="!remoteLoading && !remoteDirs.length" class="gdb-hint">（没有子目录）</div>
        </div>
        <div class="gdb-pick-f">
          <span class="gdb-hint">{{ remotePath }}</span>
          <n-button size="small" @click="showRemotePick = false">取消</n-button>
          <n-button size="small" type="primary" @click="remoteChoose">用当前目录作为源码根</n-button>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { createLogger } from '../../../utils/logger'
import { useSshProfiles } from '../../../composables/useSshProfiles'
import { readUiState, persistUiState, oneOf, within } from '../../../utils/uiState'
import { Icon } from '@iconify/vue'
import playIcon from '@iconify-icons/lucide/play'
import fastForwardIcon from '@iconify-icons/lucide/fast-forward'
import skipForwardIcon from '@iconify-icons/lucide/skip-forward'
import arrowDownIcon from '@iconify-icons/lucide/corner-down-right'
import arrowUpIcon from '@iconify-icons/lucide/corner-up-right'
import pauseIcon from '@iconify-icons/lucide/pause'
import layersIcon from '@iconify-icons/lucide/layers'
import variableIcon from '@iconify-icons/lucide/variable'
import cpuIcon from '@iconify-icons/lucide/cpu'
import minusIcon from '@iconify-icons/lucide/minus'
import plusIcon from '@iconify-icons/lucide/plus'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import trashIcon from '@iconify-icons/lucide/trash-2'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import sendIcon from '@iconify-icons/lucide/external-link'
import folderIcon from '@iconify-icons/lucide/folder-open'
import crosshairIcon from '@iconify-icons/lucide/crosshair'
import followIcon from '@iconify-icons/lucide/locate-fixed'
import panelLeftIcon from '@iconify-icons/lucide/panel-left'
import panelRightIcon from '@iconify-icons/lucide/panel-right'
import saveIcon from '@iconify-icons/lucide/save'
import copyIcon from '@iconify-icons/lucide/copy'
import downloadIcon from '@iconify-icons/lucide/download'
import uploadIcon from '@iconify-icons/lucide/upload'
import stopIcon from '@iconify-icons/lucide/square'
import playCircleIcon from '@iconify-icons/lucide/play-circle'
import memoryIcon from '@iconify-icons/lucide/memory-stick'
import expandTreeIcon from '@iconify-icons/lucide/list-tree'
import bugIcon from '@iconify-icons/lucide/bug'
import listIcon from '@iconify-icons/lucide/list'
import eyeIcon from '@iconify-icons/lucide/eye'
import wandIcon from '@iconify-icons/lucide/wand-2'

const logger = createLogger('RemoteGdbDebugger')

import GdbVarNode from './GdbVarNode.vue'

const { profiles } = useSshProfiles(logger)
const profileOptions = computed(() =>
  profiles.value.map(p => ({ label: `${p.name || p.host} (${p.user}@${p.host})`, value: p.id })))
// 面板选择记在本地：下次开窗接着用（各窗口同源共享一份，不含任何凭据）。
const UI = {
  profile: 'guaxi_gdb_profile',
  mode: 'guaxi_gdb_mode',
  escalate: 'guaxi_gdb_escalate',
  program: 'guaxi_gdb_program',
  fontSize: 'guaxi_gdb_font_size_v2',
  sideW: 'guaxi_gdb_side_width',
  rightW: 'guaxi_gdb_right_width',
  srcMode: 'guaxi_gdb_source_mode',
  srcRule: 'guaxi_gdb_source_rule',
  srcRoot: 'guaxi_gdb_source_root',
  srcProfile: 'guaxi_gdb_source_profile',
  srcFrom: 'guaxi_gdb_source_prefix',
  srcHeight: 'guaxi_gdb_source_height',
}

const profileId = ref(readUiState(UI.profile, null))
/// 调试通道：ssh | serial | telnet（串口/Telnet 复用「实时终端」里已建立的会话）。
const transport = ref(readUiState('guaxi_gdb_transport', 'ssh', oneOf('ssh', 'serial', 'telnet')))
const transportOptions = [
  { label: 'SSH', value: 'ssh' },
  { label: '串口', value: 'serial' },
  { label: 'Telnet', value: 'telnet' },
]
const consoleSessionId = ref('')
const consoleSessions = ref([])
const consoleSessionOptions = computed(() =>
  consoleSessions.value
    .filter(s => s.kind === transport.value)
    .map(s => ({ label: s.label || s.id, value: s.id })),
)
async function reloadConsoleSessions() {
  try { consoleSessions.value = await invoke('console_list_sessions') } catch (_) { consoleSessions.value = [] }
  const ids = consoleSessionOptionIds()
  if (ids.length && !ids.includes(consoleSessionId.value)) consoleSessionId.value = ids[0]
}
function consoleSessionOptionIds() { return consoleSessionOptions.value.map(o => o.value) }
const mode = ref(readUiState(UI.mode, 'launch', oneOf('launch', 'attach')))   // 'launch' | 'attach'
const attachPid = ref(null)
const escalate = ref(readUiState(UI.escalate, 'none', oneOf('none', 'sudo', 'su')))   // 'none' | 'sudo' | 'su'
const escalatePassword = ref('')
const escalateOptions = [
  { label: '不提权', value: 'none' },
  { label: 'sudo', value: 'sudo' },
  { label: 'su root', value: 'su' },
]
const program = ref(readUiState(UI.program, ''))
const argsText = ref('')
const starting = ref(false)
const error = ref('')
const sessionId = ref('')
const statusText = ref('未连接')
/// 目标是否正在运行。运行中时 gdb 会把栈/变量类命令排队直到停下，
/// 期间命令不回复 → 前端干等超时。所以这些查询在运行中直接跳过并提示。
const isRunning = ref(false)

// ---- 命令等待状态（对端慢时给用户看"在等什么、等了多久"，而不是甩个错误）----
/// 单条命令的等待上限。足够覆盖加载大符号表、慢链路；到点只停止等待、不报错。
const WAIT_MAX_MS = 600_000
/// 超过这个秒数就在界面上提示"对端响应偏慢"。
const WAIT_HINT_SECS = 6
/// 正在等待应答的命令（可能有并发，展示最早的一条）。
const inflight = ref([])
const busyElapsed = ref(0)
const busy = computed(() => inflight.value[0] || null)
/// gdb 的 MI 是否就绪（su 认证 + 启动完成）。由 ready 事件 + 等待期间的状态轮询共同维护，
/// 用来把"还在建立"与"已就绪但对端慢"区分开显示。
const gdbReady = ref(false)
let ticker = null
function ensureTicker() {
  if (ticker) return
  ticker = setInterval(() => {
    const b = busy.value
    busyElapsed.value = b ? Math.floor((Date.now() - b.started) / 1000) : 0
  }, 1000)
}
function stopTickerIfIdle() {
  if (ticker && !inflight.value.length) {
    clearInterval(ticker)
    ticker = null
    busyElapsed.value = 0
  }
}
// 提示（非错误）：用于"会话已结束""等待超时"这类需要告知但不算失败的情况。
const notice = ref('')
let noticeTimer = null
function note(msg) {
  notice.value = msg
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => { notice.value = '' }, 15000)
}
function clearNote() { notice.value = ''; if (noticeTimer) { clearTimeout(noticeTimer); noticeTimer = null } }
const tab = ref('console')

const consoleLines = ref([])
const consoleRef = ref(null)
const cmdInput = ref('')
const exprInput = ref('')
const varRoots = ref([])
const varFormat = ref('natural')
const varFormatOptions = [
  { label: '自然', value: 'natural' },
  { label: '十六进制', value: 'hexadecimal' },
  { label: '十进制', value: 'decimal' },
  { label: '二进制', value: 'binary' },
]
const watchInput = ref('')
const watches = ref([])
const threads = ref([])
const currentThread = ref(null)
const bpInput = ref('')
const breaks = ref([])
const memAddr = ref('')
const memCount = ref(64)
const memText = ref('')
const macroName = ref('')
const macroCmds = ref('')
const macros = ref([])

// 调用栈 / 局部变量 / 寄存器（结构化面板）
const callStack = ref([])
const curFrame = ref(0)
const locals = ref([])
const registers = ref([])

// 跨机源码对照
const sourceMode = ref(readUiState(UI.srcMode, 'local', oneOf('local', 'remote')))  // 'local' | 'remote'
const srcFromPrefix = ref(readUiState(UI.srcFrom, ''))
const sourceLines = ref([])
const sourceCurLine = ref(0)
const followFrame = ref(readUiState('guaxi_gdb_follow_frame', true))
function hasBreakpointAt(n) {
  const file = sourceFile.value
  if (!file) return false
  return breaks.value.some(b => b.line === String(n) && b.file && (
    b.file === file || file.endsWith('/' + b.file) || b.file.endsWith('/' + shortFile(file))
  ))
}
const sourceFile = ref('')
const sourceHint = ref('停在断点时自动对照当前行；也可点“对照当前行”。')
const sourceBodyRef = ref(null)
let curLineEl = null
const sourceHeight = ref(readUiState(UI.srcHeight, 220, within(80, 640)))

const palette = [
  { label: '运行 / 重新运行 (run)', cmd: '-exec-run', icon: playIcon },
  { label: '继续 (continue)', cmd: '-exec-continue', icon: fastForwardIcon },
  { label: '下一步 (next，不进入函数)', cmd: '-exec-next', icon: skipForwardIcon },
  { label: '步入 (step，进入函数)', cmd: '-exec-step', icon: arrowDownIcon },
  { label: '步出 (finish，执行到返回)', cmd: '-exec-finish', icon: arrowUpIcon },
  { label: '中断 (interrupt，暂停目标)', cmd: '-exec-interrupt', icon: pauseIcon },
]
const gdbFontSize = ref(readUiState(UI.fontSize, 16, within(9, 32)))
function setGdbFontSize(v) { gdbFontSize.value = Math.min(28, Math.max(9, v)) }
const gdbSideWidth = ref(readUiState(UI.sideW, 300, within(200, 600)))
const gdbRightWidth = ref(readUiState(UI.rightW, 300, within(200, 600)))
const leftCollapsed = ref(readUiState('guaxi_gdb_left_collapsed', false))
const rightCollapsed = ref(readUiState('guaxi_gdb_right_collapsed', false))
function onGdbResizeDown(e) {
  const startX = e.clientX; const startW = gdbSideWidth.value
  let resizing = true
  const move = ev => { if (resizing) gdbSideWidth.value = Math.min(600, Math.max(200, startW + ev.clientX - startX)) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}
function onGdbRightResizeDown(e) {
  const startX = e.clientX; const startW = gdbRightWidth.value
  let resizing = true
  const move = ev => { if (resizing) gdbRightWidth.value = Math.min(600, Math.max(200, startW - (ev.clientX - startX))) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

// 侧栏分块：标题固定、内容各自滚动、块高可上下拖拽。
const panelCollapsed = reactive({})
const panelH = reactive({ breaks: 150, callstack: 150, locals: 150, vars: 200, threads: 140, watch: 150, regs: 200, mem: 160, macros: 150 })
function togglePanel(k) { panelCollapsed[k] = !panelCollapsed[k] }
function panelResizeDown(k, e) {
  e.preventDefault()
  const startY = e.clientY; const startH = panelH[k] || 150
  let resizing = true
  const move = ev => { if (resizing) panelH[k] = Math.min(800, Math.max(60, startH + ev.clientY - startY)) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

function miQuote(s) {
  return '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"'
}
// __BODY2__
let unlisteners = []
onMounted(async () => {
  reloadConsoleSessions()
  unlisteners.push(await listen('gdb-mi-output', ev => {
    if (ev.payload?.session_id === sessionId.value) appendConsole(ev.payload.kind, ev.payload.text)
  }))
  unlisteners.push(await listen('gdb-mi-async', ev => {
    if (ev.payload?.session_id === sessionId.value) onAsync(ev.payload)
  }))
  unlisteners.push(await listen('gdb-mi-exit', ev => {
    if (ev.payload?.session_id === sessionId.value) {
      statusText.value = '已退出'
      appendConsole('log', '[gdb 进程已退出]')
      sessionId.value = ''
      isRunning.value = false
      gdbReady.value = false
      inflight.value = []
      stopTickerIfIdle()
      // 保留 consoleLines 供用户查看失败原因（空状态区会继续显示）。
    }
  }))
})
onBeforeUnmount(() => {
  unlisteners.forEach(u => u())
  // 清掉等待计时器与提示定时器，避免组件卸载后还在跑。
  if (ticker) { clearInterval(ticker); ticker = null }
  if (noticeTimer) { clearTimeout(noticeTimer); noticeTimer = null }
  inflight.value = []
  if (sessionId.value) invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {})
})

function appendConsole(kind, text) {
  consoleLines.value.push({ kind, text })
  if (consoleLines.value.length > 5000) consoleLines.value.splice(0, consoleLines.value.length - 5000)
  nextTick(() => { const el = consoleRef.value; if (el) el.scrollTop = el.scrollHeight })
}

function onAsync(p) {
  if (p.class === 'ready') {
    // gdb 的 MI 就绪（su 认证 + gdb 启动完成）。之前排队的命令此时才开始下发。
    gdbReady.value = true
    if (/建立|启动/.test(statusText.value)) statusText.value = 'gdb 已就绪'
    return
  }
  if (p.class === 'notready') {
    // 后端在就绪超时后主动告知，并把排队的命令以明确错误回掉。
    gdbReady.value = false
    error.value = p.payload?.msg || 'gdb 未在预期时间内就绪'
    return
  }
  if (p.class === 'stopped') {
    isRunning.value = false
    statusText.value = '已停下: ' + (p.payload?.reason || '')
    curFrame.value = 0
    // 顺序刷新（不要并发）：gdb 是单线程处理 MI 的，一次发一堆只会互相排队；
    // 任一超时也会连累其它命令，表现为"命令失败: gdb 命令超时"。
    refreshOnStop()
  } else if (p.class === 'running') {
    isRunning.value = true
    statusText.value = '运行中（栈/变量需停下后才能查）'
  }
}

/// 停住后的定向刷新：线程 → 调用栈 → 局部变量 → 源码。逐个 await，避免并发排队。
async function refreshOnStop() {
  await refreshThreads()
  await refreshStack()
  await refreshLocals()
  if (followFrame.value) await refreshSource()
  if (watches.value.length) await refreshWatch()
}

async function start() {
  if (transport.value === 'ssh') {
    if (!profileId.value) { error.value = '请选择 SSH 档案'; return }
  } else if (!consoleSessionId.value) {
    error.value = transport.value === 'serial' ? '请先在「实时终端」连上串口，再在这里选择该会话' : '请先在「实时终端」连上 Telnet，再在这里选择该会话'
    return
  }
  if (mode.value === 'attach' && !attachPid.value) { error.value = '附加模式需要填 PID'; return }
  if (mode.value === 'attach' && transport.value === 'ssh' && escalate.value === 'su' && !escalatePassword.value) { error.value = 'su 提权需要填 root 密码'; return }
  starting.value = true; error.value = ''
  clearNote()
  inflight.value = []
  gdbReady.value = false
  consoleLines.value = []
  try {
    const args = argsText.value.trim() ? argsText.value.trim().split(/\s+/) : []
    sessionId.value = await invoke('gdb_mi_start', {
      profileId: transport.value === 'ssh' ? profileId.value : null,
      program: program.value || null,
      args,
      attachPid: mode.value === 'attach' ? Number(attachPid.value) : null,
      transport: transport.value,
      serialSessionId: transport.value === 'serial' ? consoleSessionId.value : null,
      telnetSessionId: transport.value === 'telnet' ? consoleSessionId.value : null,
      escalate: mode.value === 'attach' && transport.value === 'ssh' ? escalate.value : 'none',
      escalatePassword: mode.value === 'attach' && transport.value === 'ssh' && escalate.value === 'su' ? escalatePassword.value : null,
    })
    // 会话已建立，但 gdb 可能还在 su 认证 / 启动中（后端会在就绪后回发 ready 事件，
    // 期间提交的命令会排队而不是丢失）。所以这里先如实显示"正在建立"。
    statusText.value = mode.value === 'attach' ? '正在建立（su 认证 / gdb 启动）…' : '正在建立（gdb 启动中）…'
    appendConsole('log', mode.value === 'attach' ? `[gdb 已启动，正在附加 PID ${attachPid.value}]` : '[gdb 已启动]')
    if (mode.value === 'attach') {
      // 用 MI 命令显式附加：能拿到结构化结果与 *stopped 事件；attach 停住后要手动“继续”才会跑到断点。
      const r = await runCmd(`-target-attach ${Number(attachPid.value)}`)
      if (!r) {
        // runCmd 返回 null = 命令失败/会话已断（gdb 退出）。错误已由 runCmd 写入 error；
        // 若因提权失败/ptrace 受限导致 gdb 退出，控制台会显示其 stderr（见下方空状态区）。
        if (!sessionId.value) statusText.value = '附加失败：gdb 进程已退出（见下方输出）'
        else statusText.value = '附加未返回结果'
      } else if (r.class === 'error') {
        const msg = r?.payload?.msg || ''
        if (/ptrace|Operation not permitted/i.test(msg)) {
          error.value = '附加失败(ptrace 受限): ' + msg + ' —— 选 sudo 或 su root 提权重试；或目标机 root 执行 `echo 0 > /proc/sys/kernel/yama/ptrace_scope`；或与目标进程同一用户。'
        } else if (/not found|command not found/i.test(msg)) {
          error.value = '附加失败: ' + msg + ' —— 目标机无 sudo，请改选 "su root" 并填 root 密码。'
        } else if (/tty|password|authentication|su:/i.test(msg)) {
          error.value = '附加失败(提权): ' + msg + ' —— 检查 root 密码；su 已在 PTY 上喂密码。'
        } else {
          error.value = '附加失败: ' + (msg || '可能无权限或 PID 不存在')
        }
      } else {
        isRunning.value = false
        statusText.value = `已附加 PID ${attachPid.value}（已停住，加断点后点"继续"）`
        await refreshThreads(); await refreshStack(); await refreshLocals(); await refreshSource()
      }
    }
  } catch (e) { error.value = '启动失败: ' + (e?.message || e) } finally { starting.value = false }
}

async function stop() {
  if (sessionId.value) {
    await invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {})
    sessionId.value = ''
    statusText.value = '已结束'
    isRunning.value = false
    gdbReady.value = false
    inflight.value = []
    stopTickerIfIdle()
    clearNote()
  }
}

async function runCmd(cmd, opts = {}) {
  if (!sessionId.value) return null
  appendConsole('cmd', '» ' + cmd)
  const session = sessionId.value
  let token
  try {
    token = await invoke('gdb_mi_submit', { sessionId: session, command: cmd })
  } catch (e) {
    const m = e?.message || String(e)
    // 会话已经没了不是"命令出错"：清掉本地会话状态并提示，用户直接重新发起即可。
    // （否则会留下一个指向死会话的界面，之后每条命令都重复报同一个错。）
    if (/会话已关闭|会话不存在|已结束/.test(m)) {
      if (sessionId.value === session) {
        sessionId.value = ''
        isRunning.value = false
        gdbReady.value = false
      }
      note(`会话已结束（${cmd} 未发出）。请重新“启动 gdb”。`)
    } else {
      error.value = '命令提交失败: ' + m
    }
    return null
  }
  const started = Date.now()
  const limit = opts.timeoutMs ?? WAIT_MAX_MS
  const entry = { cmd, token, started }
  inflight.value = [...inflight.value, entry]
  ensureTicker()
  let tick = 0
  try {
    for (;;) {
      let r
      try {
        r = await invoke('gdb_mi_await', { sessionId: session, token, waitMs: 800 })
      } catch (e) {
        // 会话结束/断开不是"命令失败"，要区分开说，而不是甩一个错误给用户。
        const m = e?.message || String(e)
        if (/已结束|已断开|不存在/.test(m)) note(`会话已结束，命令「${cmd}」未收到应答。`)
        else note(`等待应答失败：${m}`)
        return null
      }
      if (r) return r
      if (sessionId.value !== session) return null   // 等待期间会话被关闭/重开
      // 每 ~4s 同步一次会话状态：这样提示能区分"还在 su 认证/gdb 启动"和"已就绪但对端慢"，
      // 也顺带兜住 ready 事件万一漏收的情况。
      if (tick++ % 5 === 0) {
        try {
          const st = await invoke('gdb_mi_state', { sessionId: session })
          gdbReady.value = !!st?.ready
          if (!st?.alive) { note(`会话已结束，命令「${cmd}」未收到应答。`); return null }
        } catch (_) { /* 状态查询失败不影响等待本身 */ }
      }
      const waited = Date.now() - started
      if (waited > limit) {
        note(`命令「${cmd}」等待超过 ${Math.round(waited / 1000)} 秒仍未返回，已停止等待。可点“中断”暂停目标，或结束会话重来。`)
        return null
      }
    }
  } finally {
    inflight.value = inflight.value.filter(x => x !== entry)
    stopTickerIfIdle()
  }
}

async function submitCmd() { const c = cmdInput.value.trim(); if (!c) return; cmdInput.value = ''; await runCmd(c) }

/// 执行类命令（run/continue/next/step/finish/interrupt）专用包装：
/// 已经在跑时不再重复发 continue（gdb 会回 "Cannot execute this command while the target is running"，
/// 而那条错误又会被我们当成"命令超时"式的失败显示）；发出后乐观标记运行中，
/// 真正的 *running / *stopped 事件到达时会纠正。
async function execCmd(cmd) {
  const isResume = /^-exec-(run|continue|next|step|finish)$/.test(cmd)
  if (isResume && isRunning.value) { statusText.value = '目标已在运行中（点“中断”可暂停）'; return null }
  const r = await runCmd(cmd)
  if (isResume) isRunning.value = true
  if (/^-exec-interrupt$/.test(cmd)) isRunning.value = true
  return r
}
function paletteRun(c) {
  if (c.fn) return c.fn()
  return /^-exec-/.test(c.cmd) ? execCmd(c.cmd) : runCmd(c.cmd)
}
// __BODY3__
async function addVar() {
  const expr = exprInput.value.trim(); if (!expr || !sessionId.value) return
  const r = await runCmd(`-var-create - * ${expr}`)
  const p = r?.payload
  if (p && p.name) {
    if (varFormat.value !== 'natural') {
      await runCmd(`-var-set-format ${p.name} ${varFormat.value}`)
      const er = await runCmd(`-var-evaluate-expression ${p.name}`)
      if (er?.payload?.value != null) p.value = er.payload.value
    }
    varRoots.value.push({ name: p.name, expr, value: p.value || '', type: p.type || '', numchild: parseInt(p.numchild || '0', 10), children: [], expanded: false })
  }
}
// 变量树显示格式切换：对所有已创建的 var 对象（含子节点）设格式并重取值，便于按 hex/dec/bin 看复杂结构。
async function applyVarFormat() {
  const fmt = varFormat.value
  const walk = async nodes => {
    for (const n of nodes) {
      await runCmd(`-var-set-format ${n.name} ${fmt}`)
      const er = await runCmd(`-var-evaluate-expression ${n.name}`)
      if (er?.payload?.value != null) n.value = er.payload.value
      if (n.children.length) await walk(n.children)
    }
  }
  await walk(varRoots.value)
}
async function expandVar(node) {
  if (node.expanded) { node.expanded = false; return }
  if (!node.children.length && node.numchild > 0) {
    const r = await runCmd(`-var-list-children --all-values ${node.name}`)
    const kids = r?.payload?.children || []
    node.children = kids.map(c => ({ name: c.name, expr: c.exp || '', value: c.value || '', type: c.type || '', numchild: parseInt(c.numchild || '0', 10), children: [], expanded: false }))
  }
  node.expanded = true
}

async function addWatch() { const e = watchInput.value.trim(); if (!e) return; watchInput.value = ''; watches.value.push({ expr: e, value: '', changed: false }); await refreshWatch() }
function removeWatch(e) { watches.value = watches.value.filter(w => w.expr !== e) }
async function refreshWatch() {
  if (!watches.value.length || blockedWhileRunning('Watch 表达式')) return
  for (const w of watches.value) {
    const r = await runCmd(`-data-evaluate-expression ${miQuote(w.expr)}`)
    const v = r?.payload?.value ?? ''
    w.changed = (w.value !== '' && w.value !== v)
    w.value = v
  }
}

async function refreshThreads() {
  const r = await runCmd('-thread-info'); const p = r?.payload
  if (p?.['current-thread-id']) currentThread.value = parseInt(p['current-thread-id'], 10)
  threads.value = (p?.threads || []).map(t => ({
    id: parseInt(t.id, 10), name: t['target-id'] || t.name || '', state: t.state || '',
    frame: t.frame ? { level: t.frame.level, func: t.frame.func, file: t.frame.file || t.frame.fullname || '', line: t.frame.line || '' } : null,
  }))
}
async function selectThread(id) {
  if (!sessionId.value || blockedWhileRunning('切换线程')) return
  currentThread.value = id
  await runCmd(`-thread-select ${id}`)
  await refreshWatch()
}

async function refreshBreaks() {
  const r = await runCmd('-break-list')
  const body = r?.payload?.BreakpointTable?.body || []
  breaks.value = body.map(b => ({
    number: b.number,
    file: b.file || '',
    line: b.line || '',
    where: b.func ? `${b.func} @ ${b.file || ''}:${b.line || ''}` : (b['original-location'] || b.addr || ''),
    times: b.times || '0',
  }))
}
async function addBreak() { const loc = bpInput.value.trim(); if (!loc) return; bpInput.value = ''; await runCmd(`-break-insert ${loc}`); await refreshBreaks() }
async function delBreak(n) { await runCmd(`-break-delete ${n}`); await refreshBreaks() }

async function readMem() {
  if (!memAddr.value) return
  const r = await runCmd(`-data-read-memory-bytes ${memAddr.value} ${memCount.value}`)
  const mem = r?.payload?.memory || []
  // 把各块的连续 hex 串格式化成经典 hexdump：地址 | 16 字节 hex | ASCII。
  const rows = []
  for (const m of mem) {
    const base = parseInt(m.begin || m.addr || '0', 16)
    const hex = (m.contents || '').replace(/[^0-9a-fA-F]/g, '')
    const bytes = []
    for (let i = 0; i + 1 < hex.length; i += 2) bytes.push(parseInt(hex.slice(i, i + 2), 16))
    for (let off = 0; off < bytes.length; off += 16) {
      const chunk = bytes.slice(off, off + 16)
      const addr = (base + off).toString(16).padStart(16, '0')
      const hexCells = chunk.map(b => b.toString(16).padStart(2, '0'))
      while (hexCells.length < 16) hexCells.push('  ')
      const hexStr = hexCells.slice(0, 8).join(' ') + '  ' + hexCells.slice(8).join(' ')
      const ascii = chunk.map(b => (b >= 32 && b < 127) ? String.fromCharCode(b) : '.').join('')
      rows.push(`0x${addr}  ${hexStr}  |${ascii}|`)
    }
  }
  memText.value = rows.join('\n') || '(无数据)'
}

// ---- 调用栈 / 局部变量 / 寄存器 结构化面板 ----
function shortFile(f) {
  if (!f) return ''
  const parts = String(f).replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || f
}
/// 运行中禁止下发栈/变量类查询：gdb 会把它们排队到停下才回复，期间前端只会干等超时。
/// 返回 true 表示"已拦截，调用方应直接 return"。
function blockedWhileRunning(what) {
  if (!isRunning.value) return false
  note(`目标运行中：${what}需停下后才可查。点“中断”暂停目标，或等断点命中。`)
  return true
}

async function refreshStack() {
  if (!sessionId.value || blockedWhileRunning('调用栈')) return
  const r = await runCmd('-stack-list-frames')
  const frames = r?.payload?.stack || []
  callStack.value = frames.map(f => ({
    level: f.level, func: f.func || '??',
    file: f.file || f.fullname || '', fullname: f.fullname || f.file || '', line: f.line || '', addr: f.addr || '',
  }))
}
async function selectFrame(level) {
  if (!sessionId.value) return
  curFrame.value = level
  await runCmd(`-stack-select-frame ${level}`)
  await refreshLocals(); await refreshSource(); await refreshWatch()
}
/// 点击源码行：查看该行是否可下断点，并在该行设置断点（VS Code 式点击跳转/下断点）。
/// 左键单击 = 跳到该行；如已在断点则提示；双击 = 在该行 toggle 断点。
function onSourceLineClick(ln) {
  if (!ln) return
  sourceCurLine.value = ln.n
}
async function toggleSourceBreakpoint(ln) {
  if (!sessionId.value || !ln) return
  const file = sourceFile.value
  if (!file) return
  const loc = `${file}:${ln.n}`
  const existing = breaks.value.find(b => b.file && b.line === String(ln.n) && (b.file === file || file.endsWith('/' + b.file) || b.file.endsWith('/' + shortFile(file))))
  if (existing) {
    await runCmd(`-break-delete ${existing.number}`)
    await refreshBreaks()
    note(`已删除断点 ${loc}`)
    return
  }
  const r = await runCmd(`-break-insert ${miQuote(loc)}`)
  await refreshBreaks()
  if (r?.class === 'error') note(`下断点失败：${r?.payload?.msg || loc}`)
  else note(`已在 ${loc} 下断点`)
}
async function refreshLocals() {
  if (!sessionId.value || blockedWhileRunning('局部变量')) return
  const r = await runCmd('-stack-list-variables --all-values')
  const vars = r?.payload?.variables || []
  locals.value = vars.map(v => ({ name: v.name, value: v.value != null ? String(v.value) : '' }))
}
async function refreshRegisters() {
  if (!sessionId.value || blockedWhileRunning('寄存器')) return
  const nr = await runCmd('-data-list-register-names')
  const names = nr?.payload?.['register-names'] || []
  const vr = await runCmd('-data-list-register-values x')
  const vals = vr?.payload?.['register-values'] || []
  const hexByNum = {}
  for (const rv of vals) hexByNum[rv.number] = rv.value
  const out = []
  names.forEach((nm, i) => {
    if (!nm) return
    const hex = hexByNum[String(i)] || hexByNum[i]
    if (hex == null) return
    let dec = ''
    try { dec = BigInt(hex).toString(10) } catch (_) { dec = '' }
    out.push({ name: nm, hex, dec })
  })
  registers.value = out
}

/// 全量刷新：线程 / 调用栈 / 局部变量 / 寄存器 / Watch / 源码。
/// 逐个 await（不并发），否则多个命令在 gdb 侧排队，任一卡住会连带全部超时。
async function refreshAll() {
  if (!sessionId.value) return
  if (blockedWhileRunning('调试信息')) return
  await refreshThreads(); await refreshStack(); await refreshLocals()
  await refreshRegisters(); await refreshWatch(); await refreshSource()
}

// ---- 跨机源码对照 ----
//
// 用户只需要给一个"源码工程根"（本机目录，或编译机上的目录），
// 路径映射（尤其 chroot/docker 构建里调试信息写的是 /build/... 这种情况）交给后端按规则解。
const sourceRule = ref(readUiState(UI.srcRule, 'auto', oneOf('auto', 'direct', 'prefix', 'search')))
const sourceRoot = ref(readUiState(UI.srcRoot, ''))
const sourceProfileId = ref(readUiState(UI.srcProfile, null))
const sourceRuleOptions = [
  { label: '自动（推荐）', value: 'auto' },
  { label: '按原样', value: 'direct' },
  { label: '前缀替换', value: 'prefix' },
  { label: '按文件名搜索', value: 'search' },
]
const locateInfo = ref('')     // 定位结果/失败诊断
// 远端目录浏览
const showRemotePick = ref(false)
const remotePath = ref('')
const remoteDirs = ref([])
const remoteLoading = ref(false)

// 选择变化即落盘（布局类如宽度/字号/高度也一样，拖一次就记住）。
// 放在所有被持久化的 ref 声明**之后**，否则会命中 TDZ。
for (const [key, src] of [
  [UI.profile, profileId], [UI.mode, mode], [UI.escalate, escalate], [UI.program, program],
  ['guaxi_gdb_transport', transport],
  ['guaxi_gdb_follow_frame', followFrame],
  ['guaxi_gdb_left_collapsed', leftCollapsed],
  ['guaxi_gdb_right_collapsed', rightCollapsed],
  [UI.fontSize, gdbFontSize], [UI.sideW, gdbSideWidth], [UI.rightW, gdbRightWidth],
  [UI.srcMode, sourceMode], [UI.srcRule, sourceRule], [UI.srcRoot, sourceRoot],
  [UI.srcProfile, sourceProfileId], [UI.srcFrom, srcFromPrefix], [UI.srcHeight, sourceHeight],
]) {
  watch(src, (v) => persistUiState(key, v))
}

watch(transport, (v) => {
  if (v !== 'ssh') reloadConsoleSessions()
})
watch(profiles, (list) => {
  if (!list.length) return
  const ids = new Set(list.map((p) => p.id))
  if (profileId.value && !ids.has(profileId.value)) profileId.value = null
  if (sourceProfileId.value && !ids.has(sourceProfileId.value)) sourceProfileId.value = null
})

function pickRoot() {
  if (sourceMode.value === 'local') return pickLocalRoot()
  showRemotePick.value = true
  if (!sourceProfileId.value) sourceProfileId.value = profileId.value
  browseRemote(remotePath.value || '~')
}
async function pickLocalRoot() {
  try {
    const sel = await open({ directory: true, multiple: false, title: '选择源码工程根目录' })
    if (sel) { sourceRoot.value = typeof sel === 'string' ? sel : sel?.path; note('源码根已设为 ' + sourceRoot.value) }
  } catch (_) {}
}
async function browseRemote(path) {
  if (!sourceProfileId.value) { error.value = '请先选一个 SSH 档案（编译机）'; return }
  remoteLoading.value = true
  try {
    remoteDirs.value = await invoke('ssh_list_dirs', { profileId: sourceProfileId.value, path: path || '~' })
    remotePath.value = path || '~'
  } catch (e) { error.value = '列远端目录失败: ' + (e?.message || e) } finally { remoteLoading.value = false }
}
function remoteEnter(name) {
  const base = remotePath.value.replace(/\/+$/, '')
  browseRemote(`${base}/${name}`)
}
function remoteUp() {
  const p = remotePath.value.replace(/\/+$/, '')
  const i = p.lastIndexOf('/')
  browseRemote(i > 0 ? p.slice(0, i) : '/')
}
function remoteChoose() {
  sourceRoot.value = remotePath.value
  showRemotePick.value = false
  note('源码根（编译机）已设为 ' + sourceRoot.value)
}

async function refreshSource() {
  if (!sessionId.value || blockedWhileRunning('当前帧源码')) return
  const r = await runCmd('-stack-info-frame')
  const fr = r?.payload?.frame
  if (!fr) { sourceHint.value = '当前无栈帧（先运行到断点）。'; return }
  const full = fr.fullname || fr.file || ''
  const line = parseInt(fr.line || '0', 10)
  sourceFile.value = full
  sourceCurLine.value = line
  if (!full) { sourceLines.value = []; sourceHint.value = '当前帧无源码信息。'; locateInfo.value = ''; return }
  if (!sourceRoot.value.trim()) {
    sourceLines.value = []
    sourceHint.value = '请先选「源码工程根」（本机目录，或切到「编译机源码」浏览远端目录）。'
    return
  }
  try {
    const loc = await invoke('cmd_source_locate', {
      req: {
        debugPath: full,
        root: sourceRoot.value,
        rule: sourceRule.value,
        toPrefix: sourceRoot.value,
        profileId: sourceMode.value === 'remote' ? sourceProfileId.value : null,
      },
    })
    if (!loc.found) {
      sourceLines.value = []
      sourceHint.value = loc.hint || '未找到源码。'
      locateInfo.value = loc.tried && loc.tried.length
        ? '尝试过:\n' + loc.tried.slice(0, 8).join('\n') + (loc.tried.length > 8 ? '\n…' : '')
        : ''
      return
    }
    const text = await invoke('cmd_source_read', {
      path: loc.path,
      profileId: sourceMode.value === 'remote' ? sourceProfileId.value : null,
      maxBytes: 1048576,
    })
    sourceLines.value = text.split(/\r?\n/).map((t, i) => ({ n: i + 1, text: t }))
    sourceHint.value = ''
    locateInfo.value = `定位: ${loc.path}（${loc.rule}）`
    // 顺带把映射告诉 gdb，使它内部的 list/断点源码定位也一致。
    if (sourceMode.value === 'remote' && sourceRule.value === 'prefix' && srcFromPrefix.value.trim()) {
      await runCmd(`-interpreter-exec console ${miQuote(`set substitute-path ${srcFromPrefix.value.trim()} ${sourceRoot.value.trim()}`)}`)
    }
    await nextTick()
    if (curLineEl && curLineEl.scrollIntoView) curLineEl.scrollIntoView({ block: 'center' })
  } catch (e) {
    sourceLines.value = []
    sourceHint.value = `读取源码失败（${shortFile(full)}）：${e?.message || e}`
  }
}
let sourceResizing = false
function onSourceResizeDown(e) {
  sourceResizing = true
  const startY = e.clientY; const startH = sourceHeight.value
  const move = ev => { if (sourceResizing) sourceHeight.value = Math.min(640, Math.max(80, startH + ev.clientY - startY)) }
  const up = () => { sourceResizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

function addMacro() {
  const n = macroName.value.trim(); if (!n || !macroCmds.value.trim()) return
  const cmds = macroCmds.value.split(';').map(s => s.trim()).filter(Boolean)
  const i = macros.value.findIndex(m => m.name === n)
  if (i >= 0) macros.value[i] = { name: n, commands: cmds }; else macros.value.push({ name: n, commands: cmds })
  macroName.value = ''; macroCmds.value = ''
}
function removeMacro(n) { macros.value = macros.value.filter(m => m.name !== n) }
async function runMacro(m) { for (const c of m.commands) await runCmd(c) }

// L：发送到主界面编辑器（复用 template_insert_event 通道）
async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送到编辑器失败: ' + (e?.message || e) }
}

// ---- 复制：每个面板都能整块复制，单行/单元格也能直接复制 ----
/// 统一写剪贴板：优先 Tauri clipboard 插件，退回 navigator.clipboard。
async function writeClipboard(text) {
  const s = String(text ?? '')
  if (!s) return
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(s)
  } catch (_) {
    try { await navigator.clipboard.writeText(s) }
    catch (e) { error.value = '复制失败: ' + (e?.message || e); return }
  }
  copiedTag.value = `已复制 ${s.split('\n').length > 1 ? s.split('\n').length + ' 行' : '1 项'}`
  if (copyTimer) clearTimeout(copyTimer)
  copyTimer = setTimeout(() => { copiedTag.value = '' }, 1800)
}
const copiedTag = ref('')
let copyTimer = null

/// 各面板的纯文本表示（同时供"复制"与"发到编辑器"使用）。
function stackText() {
  return callStack.value
    .map(f => `#${f.level} ${f.func} @ ${f.fullname || f.file}${f.line ? ':' + f.line : ''}`)
    .join('\n')
}
function localsText() {
  return locals.value.map(l => `${l.name} = ${l.value}`).join('\n')
}
function varTreeText() {
  const out = []
  for (const n of varRoots.value) flattenVar(n, 0, out)
  return out.join('\n')
}
function watchText() {
  return watches.value.map(w => `${w.expr} = ${w.value}`).join('\n')
}
function registersText() {
  return registers.value.map(r => `${r.name} = ${r.hex} (${r.dec})`).join('\n')
}
function threadsText() {
  return threads.value.map(threadText).join('\n')
}
function threadText(t) {
  const head = `线程 ${t.id}${t.name ? ` (${t.name})` : ''} — ${t.state}`
  return t.frame ? `${head}\n  #${t.frame.level} ${t.frame.func} @ ${t.frame.file}:${t.frame.line}` : head
}
function breaksText() {
  return breaks.value.map(b => `#${b.number} ${b.where}（命中 ${b.times}）`).join('\n')
}
function sourceText() {
  return sourceLines.value.map(l => String(l.n).padStart(5) + '  ' + l.text).join('\n')
}
function consoleText() {
  return consoleLines.value.map(l => l.text).join('\n')
}
function memOutText() { return memText.value || '' }

/// 面板复制：传一个返回文本的函数即可。
async function copyPanel(getText, name) {
  const text = getText()
  if (!text) { note(`${name}为空，没有可复制的内容`); return }
  await writeClipboard(text)
}
/// 行/单元格复制：附加一个标记，避免误判。
async function copyRow(text) { await writeClipboard(text) }
function flattenVar(node, depth, out) {
  out.push('  '.repeat(depth) + `${node.expr || node.name}${node.type ? ': ' + node.type : ''} = ${node.value}`)
  if (node.expanded) for (const c of node.children) flattenVar(c, depth + 1, out)
}
function sendVarTree() {
  const text = varTreeText()
  if (text) sendToEditor('```\n' + text + '\n```\n')
}
function sendWatch() {
  const text = watchText()
  if (text) sendToEditor('```\n' + text + '\n```\n')
}
function sendStack() {
  const text = stackText()
  if (text) sendToEditor('```\n' + text + '\n```\n')
}
function sendLocals() {
  const text = localsText()
  if (text) sendToEditor('```\n' + text + '\n```\n')
}
function sendRegisters() {
  const text = registersText()
  if (text) sendToEditor('```\n' + text + '\n```\n')
}
function sendMem() {
  if (!memText.value) return
  sendToEditor('```\n' + memText.value + '\n```\n')
}
function sendSource() {
  const text = sourceText()
  if (!text) return
  sendToEditor(`\`\`\`${shortFile(sourceFile.value)}\n${text}\n\`\`\`\n`)
}
async function saveMacros() {
  try {
    const path = await save({ title: '导出 gdb 宏', defaultPath: 'gdb-macros.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path) return
    await invoke('gdb_save_macros', { path, macros: macros.value })
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
async function loadMacros() {
  try {
    const sel = await open({ multiple: false, directory: false, filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!sel) return
    const path = typeof sel === 'string' ? sel : sel?.path
    macros.value = await invoke('gdb_load_macros', { path })
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
</script>

<style scoped>
.gdb-root { height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--guaxi-content-bg); }
.gdb-bar { display: flex; gap: 8px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; }
.gdb-status { font-size: 15px; color: var(--guaxi-text-secondary, #6b7280); }
.gdb-error { font-size: 15px; color: #e5484d; }
.gdb-notice { font-size: 15px; color: #f5a524; }
.gdb-busy { display: inline-flex; align-items: center; gap: 5px; font-size: 15px; color: var(--tw-accent); }
.gdb-busy-slow { color: #f5a524; }
.gdb-palette { display: flex; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; }
.gdb-body { flex: 1 1 auto; min-height: 0; display: flex; }
.gdb-tabs { height: 100%; width: 100%; display: flex; flex-direction: column; }
.gdb-tabs :deep(.gdb-pane) { height: 100%; padding: 8px 12px; min-height: 0; overflow: auto; }
.gdb-console { height: calc(100% - 44px); overflow: auto; background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 15px; padding: 8px; border-radius: 6px; }
.gdb-console pre { margin: 0; white-space: pre-wrap; word-break: break-all; }
.gdb-l-log { color: #9aa0a6; }
.gdb-l-target { color: #79c0ff; }
.gdb-l-cmd { color: #7ee787; }
.gdb-input { display: flex; gap: 8px; margin-top: 8px; }
.gdb-actions { display: flex; gap: 8px; align-items: center; margin-bottom: 8px; flex-wrap: wrap; }
.gdb-hint { color: var(--guaxi-text-secondary, #9ca3af); font-size: 15px; padding: 12px; }
.gdb-tree { font-family: var(--guaxi-mono-font, monospace); font-size: 15px; }.gdb-table { width: 100%; border-collapse: collapse; font-size: 15px; }
.gdb-table th, .gdb-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); }
.gdb-val { font-family: var(--guaxi-mono-font, monospace); white-space: pre-wrap; word-break: break-all; }
.gdb-table tr.changed { background: rgba(255,196,0,0.16); }
.gdb-threads { font-size: 15px; }
.gdb-thread { padding: 6px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); cursor: pointer; }.gdb-thread.active { background: var(--guaxi-active-bg, rgba(59,130,246,0.12)); }
.gdb-thread-h { font-weight: 500; }
.gdb-frame { color: var(--guaxi-text-secondary, #6b7280); font-family: var(--guaxi-mono-font, monospace); }
.gdb-mem { background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 15px; padding: 8px; border-radius: 6px; white-space: pre-wrap; word-break: break-all; margin: 0; }
.gdb-empty { flex: 1 1 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--guaxi-text-secondary, #6b7280); font-size: 16px; padding: 12px; min-height: 0; }
.gdb-dead-note { flex: 0 0 auto; color: #e5484d; align-self: flex-start; }

/* 编译机目录浏览弹窗 */
.gdb-pick { display: flex; flex-direction: column; max-height: 80vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; font-family: var(--tw-ui-font); }
.gdb-pick-h { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.gdb-pick-bar { display: flex; align-items: center; gap: 6px; padding: 8px 12px; border-bottom: 1px solid var(--guaxi-border); }
.gdb-pick-list { flex: 1 1 auto; min-height: 200px; max-height: 52vh; overflow: auto; padding: 6px 8px; }
.gdb-pick-item { display: flex; align-items: center; gap: 6px; padding: 6px 8px; border-radius: 6px; cursor: pointer; font-size: 16px; font-family: var(--gdb-mono); }
.gdb-pick-item:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.gdb-pick-ico { color: #e0a53f; }
.gdb-pick-f { display: flex; align-items: center; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--guaxi-border); }
.gdb-pick-f .gdb-hint { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gdb-src-empty { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
.gdb-src-tried { max-height: 200px; overflow: auto; margin: 0; font-size: 15px; }
.gdb-console-dead { flex: 1 1 auto; width: 100%; min-height: 0; overflow: auto; }

/* ---- 美化层 ---- */
.gdb-root { font-family: var(--tw-ui-font); }
.gdb-bar { gap: 8px; padding: 10px 14px; background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); }
.gdb-palette { gap: 5px; padding: 8px 14px; align-items: center; background: color-mix(in srgb, var(--guaxi-panel-bg) 35%, transparent); }
.gdb-palette :deep(.n-button) { border-radius: var(--tw-radius-sm); }
.gdb-console { background: var(--tw-code-bg) !important; color: var(--tw-code-fg) !important; font-family: var(--tw-mono-font) !important; border-radius: var(--tw-radius-sm); line-height: 1.55; }
.gdb-mem { background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); border-radius: var(--tw-radius-sm); }
.gdb-l-cmd { color: var(--tw-accent); font-weight: 600; }
.gdb-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.gdb-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); }
.gdb-tree, .gdb-val, .gdb-frame { font-family: var(--tw-mono-font); }
.gdb-table th { text-transform: uppercase; letter-spacing: .04em; font-size: 14px; color: var(--guaxi-text-secondary); }
.gdb-thread.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.gdb-fontctl { display: inline-flex; align-items: center; gap: 4px; margin-left: auto; padding: 2px 4px; border: 1px solid var(--guaxi-border); border-radius: 999px; }
.gdb-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.gdb-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.gdb-fsz { min-width: 20px; text-align: center; font-size: 15px; font-family: var(--tw-mono-font); color: var(--tw-accent); }

/* 面板填满高度 + 亮色适配 */
.gdb-tabs :deep(.gdb-pane) { height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--guaxi-content-bg); }
.gdb-console { flex: 1 1 auto; height: auto !important; min-height: 0; }

/* 两列布局：左侧调试面板(accordion) + 中间控制台 */
.gdb-body { flex: 1 1 auto; min-height: 0; display: flex; }
.gdb-sideresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; transition: opacity .15s, background .15s; }
.gdb-sideresize:hover { opacity: 1; background: var(--tw-accent); }
.gdb-center { flex: 1 1 auto; min-width: 0; min-height: 0; display: flex; flex-direction: column; padding: 8px; }
.gdb-center .gdb-console { flex: 1 1 auto; min-height: 0; }
.gdb-actions { flex-wrap: wrap; gap: 6px; margin-bottom: 6px; }

/* 1c：调试显示区统一用与终端(xterm)一致的等宽字体栈，避免落到非等宽的编辑器字体。 */
.gdb-root { --gdb-mono: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, 'Courier New', monospace; }
.gdb-console, .gdb-mem, .gdb-tree, .gdb-val, .gdb-frame, .gdb-source-body { font-family: var(--gdb-mono) !important; }

/* ===== 侧栏分块：标题固定、内容独立滚动、块高可拖拽 ===== */
.gdb-side { flex: 0 0 auto; min-height: 0; display: flex; flex-direction: column; overflow-y: auto; overflow-x: hidden; border-right: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 35%, transparent); }
.gdb-side.gdb-side-right { border-left: 1px solid var(--guaxi-border); }
.gdb-panel { flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.08)); }
.gdb-panel-h { flex: 0 0 auto; display: flex; align-items: center; gap: 5px; padding: 6px 8px; font-size: 15px; font-weight: 600; cursor: pointer; user-select: none; color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); }
.gdb-panel-h:hover { color: var(--tw-accent); }
.gdb-panel-ico { color: var(--tw-accent); }
.gdb-panel-t { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gdb-panel-n { flex: 0 0 auto; font-size: 13.5px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); font-family: var(--gdb-mono); }
.gdb-panel-b { flex: 0 0 auto; min-height: 0; display: flex; flex-direction: column; padding: 6px 8px 4px; overflow: hidden; }
/* 只有面板内容滚动，标题固定 */
.gdb-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; }
.gdb-panel-rz { flex: 0 0 auto; height: 5px; cursor: row-resize; background: var(--guaxi-border); opacity: .35; transition: opacity .15s, background .15s; }
.gdb-panel-rz:hover { opacity: 1; background: var(--tw-accent); }

/* 图标按钮（带 tooltip） */
.gdb-ib { display: inline-flex; align-items: center; justify-content: center; width: 24px; height: 24px; flex: 0 0 auto; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; transition: background .12s, color .12s; }
.gdb-ib:hover:not(:disabled) { background: var(--tw-accent-soft); color: var(--tw-accent); }
.gdb-ib.primary { color: var(--tw-accent); }
.gdb-ib.danger:hover:not(:disabled) { background: rgba(229,72,77,0.14); color: #e5484d; }
.gdb-ib:disabled { opacity: .35; cursor: default; }
.gdb-ib.lg { width: 30px; height: 30px; }
.gdb-launch { border: 1px solid color-mix(in srgb, var(--tw-accent) 55%, transparent); background: var(--tw-accent-soft); }
.gdb-launch:hover:not(:disabled) { background: color-mix(in srgb, var(--tw-accent) 28%, transparent); }
.gdb-spin { animation: gdb-spin 1s linear infinite; }
@keyframes gdb-spin { to { transform: rotate(360deg); } }
.gdb-pal-sep { width: 1px; height: 18px; background: var(--guaxi-border); margin: 0 4px; }
.gdb-ellip { max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* 调用栈 / 局部变量 / 寄存器面板 */
.gdb-stack { font-size: 15px; }
.gdb-sframe { display: flex; gap: 6px; align-items: baseline; padding: 4px 6px; cursor: pointer; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); white-space: nowrap; }
.gdb-sframe:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.gdb-sframe.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.gdb-sframe-lv { color: var(--guaxi-text-secondary); font-family: var(--gdb-mono); flex: 0 0 auto; }
.gdb-sframe-func { color: var(--tw-accent); overflow: hidden; text-overflow: ellipsis; }
.gdb-sframe-loc { color: var(--guaxi-text-secondary); font-family: var(--gdb-mono); margin-left: auto; flex: 0 0 auto; }
.gdb-lname, .gdb-rname { color: #79c0ff; font-family: var(--gdb-mono); }
.gdb-regs .gdb-val { white-space: nowrap; }

/* 源码对照面板（居中列顶部，可上下拖拽） */
.gdb-source { flex: 0 0 auto; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); overflow: hidden; background: var(--tw-code-bg); }
.gdb-source-bar { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 5px 8px; border-bottom: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); flex: 0 0 auto; }
.gdb-arrow { color: var(--guaxi-text-secondary); }
.gdb-src-loc { font-size: 14px; color: var(--tw-accent); font-family: var(--gdb-mono); margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 220px; }
.gdb-source-body { flex: 1 1 auto; min-height: 0; overflow: auto; background: var(--tw-code-bg); color: var(--tw-code-fg); padding: 4px 0; }
.gdb-srcline { display: flex; white-space: pre; line-height: 1.5; cursor: text; }
.gdb-srcline:hover { background: rgba(127,127,127,0.10); }
.gdb-srcline.cur { background: rgba(255,196,0,0.16); box-shadow: inset 3px 0 0 #f5a524; }
.gdb-srcline.bp .gdb-srcbp { color: #e5484d; }
.gdb-srcbp { flex: 0 0 auto; width: 14px; text-align: center; user-select: none; font-size: .9em; }
.gdb-muted { opacity: .45; }
.gdb-srcno { flex: 0 0 auto; width: 52px; text-align: right; padding-right: 12px; color: var(--guaxi-text-secondary); opacity: .7; user-select: none; cursor: copy; }
.gdb-srcno:hover { opacity: 1; color: var(--tw-accent); }
.gdb-srctext { flex: 1 1 auto; }
.gdb-src-empty { padding: 12px; }
.gdb-vresize { flex: 0 0 auto; height: 5px; cursor: row-resize; background: var(--guaxi-border); opacity: .5; margin: 4px 0; border-radius: 3px; transition: opacity .15s, background .15s; }
.gdb-vresize:hover { opacity: 1; background: var(--tw-accent); }
</style>
