<template>
  <div class="gdb-root">
    <div class="gdb-bar">
      <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="选择 SSH 档案" style="width:180px" />
      <n-radio-group v-model:value="mode" size="small" :disabled="!!sessionId">
        <n-radio-button value="launch">启动程序</n-radio-button>
        <n-radio-button value="attach">附加进程</n-radio-button>
      </n-radio-group>
      <n-input v-model:value="program" size="small" :placeholder="mode === 'attach' ? '可执行文件(可空，仅符号)' : '可执行文件路径(可空=仅连gdb)'" style="width:230px" />
      <n-input-number v-if="mode === 'attach'" v-model:value="attachPid" size="small" :min="1" :show-button="false" placeholder="PID" style="width:110px" />
      <n-select v-if="mode === 'attach'" v-model:value="escalate" size="small" :options="escalateOptions" style="width:110px" title="提权方式" />
      <n-input v-if="mode === 'attach' && escalate === 'su'" v-model:value="escalatePassword" type="password" size="small" placeholder="root 密码" show-password-on="click" style="width:130px" />
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
      <span v-if="error" class="gdb-error">{{ error }}</span>
    </div>

    <div class="gdb-palette" v-if="sessionId">
      <n-tooltip v-for="c in palette" :key="c.label" trigger="hover">
        <template #trigger>
          <button class="gdb-ib lg primary" @click="c.fn ? c.fn() : runCmd(c.cmd)">
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
      <div class="gdb-fontctl" title="控制台/源码字号">
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize - 1)"><Icon :icon="minusIcon" width="13" height="13" /></button>
        <span class="gdb-fsz">{{ gdbFontSize }}</span>
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize + 1)"><Icon :icon="plusIcon" width="13" height="13" /></button>
      </div>
    </div>

    <div class="gdb-body" v-if="sessionId">
      <!-- ===== 左栏：断点 / 调用栈 / 局部变量 / 变量树 ===== -->
      <div class="gdb-side" :style="{ width: gdbSideWidth + 'px' }">
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
                :title="(f.fullname || f.file) + (f.line ? ':' + f.line : '')"
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
              <button class="gdb-ib" :disabled="!locals.length" @click.stop="sendLocals"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把局部变量发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.locals" :style="{ height: panelH.locals + 'px' }">
            <div class="gdb-scroll">
              <table class="gdb-table">
                <tbody>
                  <tr v-for="l in locals" :key="l.name">
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

      <div class="gdb-sideresize" @mousedown="onGdbResizeDown"></div>

      <!-- ===== 中间：源码 + 控制台 ===== -->
      <div class="gdb-center">
        <div class="gdb-source" :style="{ height: sourceHeight + 'px' }">
          <div class="gdb-source-bar">
            <n-radio-group v-model:value="sourceMode" size="tiny">
              <n-radio-button value="local">本地源码</n-radio-button>
              <n-radio-button value="remote">远端源码</n-radio-button>
            </n-radio-group>
            <n-input v-model:value="srcFromPrefix" size="tiny" placeholder="编译路径前缀 (可空)" style="width:140px" />
            <span class="gdb-arrow">→</span>
            <n-input v-model:value="srcToPrefix" size="tiny" :placeholder="sourceMode === 'local' ? '本地源码根' : '远端路径前缀(可空)'" style="width:150px" />
            <n-tooltip v-if="sourceMode === 'local'" trigger="hover"><template #trigger>
              <button class="gdb-ib" @click="pickLocalRoot"><Icon :icon="folderIcon" width="14" height="14" /></button>
            </template>选择本地源码根目录</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <button class="gdb-ib primary" @click="refreshSource"><Icon :icon="crosshairIcon" width="14" height="14" /></button>
            </template>按当前帧 file:line 对照源码并高亮</n-tooltip>
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
              :class="{ cur: ln.n === sourceCurLine }"
              :ref="el => { if (ln.n === sourceCurLine) curLineEl = el }"
            >
              <span class="gdb-srcno">{{ ln.n }}</span>
              <span class="gdb-srctext">{{ ln.text }}</span>
            </div>
            <div v-if="!sourceLines.length" class="gdb-hint gdb-src-empty">{{ sourceHint }}</div>
          </div>
        </div>
        <div class="gdb-vresize" @mousedown="onSourceResizeDown"></div>
        <div class="gdb-console" ref="consoleRef" :style="{ fontSize: gdbFontSize + 'px' }">
          <pre v-for="(l, i) in consoleLines" :key="i" :class="'gdb-l-' + l.kind">{{ l.text }}</pre>
        </div>
        <div class="gdb-input">
          <n-input v-model:value="cmdInput" size="small" placeholder="输入 MI(-exec-next) 或 CLI(bt) 命令回车执行" @keyup.enter="submitCmd" />
          <n-tooltip trigger="hover"><template #trigger>
            <button class="gdb-ib primary lg" @click="submitCmd"><Icon :icon="playCircleIcon" width="16" height="16" /></button>
          </template>执行命令</n-tooltip>
        </div>
      </div>

      <!-- ===== 右栏：线程 / Watch / 寄存器 / 内存 / 宏 ===== -->
      <div class="gdb-sideresize" @mousedown="onGdbRightResizeDown"></div>
      <div class="gdb-side gdb-side-right" :style="{ width: gdbRightWidth + 'px' }">
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
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.threads" :style="{ height: panelH.threads + 'px' }">
            <div class="gdb-scroll">
              <div v-for="t in threads" :key="t.id" class="gdb-thread" :class="{ active: t.id === currentThread }" @click="selectThread(t.id)">
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
                  <tr v-for="w in watches" :key="w.expr" :class="{ changed: w.changed }">
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
              <button class="gdb-ib" :disabled="!registers.length" @click.stop="sendRegisters"><Icon :icon="sendIcon" width="13" height="13" /></button>
            </template>把寄存器发到主界面编辑器</n-tooltip>
          </header>
          <div class="gdb-panel-b" v-show="!panelCollapsed.regs" :style="{ height: panelH.regs + 'px' }">
            <div class="gdb-scroll">
              <table class="gdb-table gdb-regs">
                <thead><tr><th>寄存器</th><th>HEX</th><th>十进制</th></tr></thead>
                <tbody>
                  <tr v-for="r in registers" :key="r.name">
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
              <pre class="gdb-mem">{{ memText || '按十六进制 + ASCII 展示内存块。' }}</pre>
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
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { createLogger } from '../../../utils/logger'
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
import saveIcon from '@iconify-icons/lucide/save'
import downloadIcon from '@iconify-icons/lucide/download'
import uploadIcon from '@iconify-icons/lucide/upload'
import stopIcon from '@iconify-icons/lucide/square'
import playCircleIcon from '@iconify-icons/lucide/circle-play'
import memoryIcon from '@iconify-icons/lucide/memory-stick'
import expandTreeIcon from '@iconify-icons/lucide/list-tree'
import bugIcon from '@iconify-icons/lucide/bug'
import listIcon from '@iconify-icons/lucide/list'
import eyeIcon from '@iconify-icons/lucide/eye'
import wandIcon from '@iconify-icons/lucide/wand-2'

const logger = createLogger('RemoteGdbDebugger')

import GdbVarNode from './GdbVarNode.vue'

const profiles = ref([])
const profileOptions = computed(() =>
  profiles.value.map(p => ({ label: `${p.name || p.host} (${p.user}@${p.host})`, value: p.id })))
const profileId = ref(null)
const mode = ref('launch')       // 'launch' | 'attach'
const attachPid = ref(null)
const escalate = ref('none')     // 'none' | 'sudo' | 'su'
const escalatePassword = ref('')
const escalateOptions = [
  { label: '不提权', value: 'none' },
  { label: 'sudo', value: 'sudo' },
  { label: 'su root', value: 'su' },
]
const program = ref('')
const argsText = ref('')
const starting = ref(false)
const error = ref('')
const sessionId = ref('')
const statusText = ref('未连接')
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
const sourceMode = ref('local')  // 'local' | 'remote'
const srcFromPrefix = ref('')
const srcToPrefix = ref('')
const sourceLines = ref([])
const sourceCurLine = ref(0)
const sourceFile = ref('')
const sourceHint = ref('停在断点时自动对照当前行；也可点“对照当前行”。')
const sourceBodyRef = ref(null)
let curLineEl = null
const sourceHeight = ref(220)

const palette = [
  { label: '运行 / 重新运行 (run)', cmd: '-exec-run', icon: playIcon },
  { label: '继续 (continue)', cmd: '-exec-continue', icon: fastForwardIcon },
  { label: '下一步 (next，不进入函数)', cmd: '-exec-next', icon: skipForwardIcon },
  { label: '步入 (step，进入函数)', cmd: '-exec-step', icon: arrowDownIcon },
  { label: '步出 (finish，执行到返回)', cmd: '-exec-finish', icon: arrowUpIcon },
  { label: '中断 (interrupt，暂停目标)', cmd: '-exec-interrupt', icon: pauseIcon },
]
const gdbFontSize = ref(14)
function setGdbFontSize(v) { gdbFontSize.value = Math.min(28, Math.max(9, v)) }
const gdbSideWidth = ref(300)
const gdbRightWidth = ref(300)
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
  try { profiles.value = await invoke('ssh_list_profiles') } catch (e) { logger.error('list profiles', e) }
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
      // 保留 consoleLines 供用户查看失败原因（空状态区会继续显示）。
    }
  }))
})
onBeforeUnmount(() => {
  unlisteners.forEach(u => u())
  if (sessionId.value) invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {})
})

function appendConsole(kind, text) {
  consoleLines.value.push({ kind, text })
  if (consoleLines.value.length > 5000) consoleLines.value.splice(0, consoleLines.value.length - 5000)
  nextTick(() => { const el = consoleRef.value; if (el) el.scrollTop = el.scrollHeight })
}

function onAsync(p) {
  if (p.class === 'stopped') {
    statusText.value = '已停下: ' + (p.payload?.reason || '')
    curFrame.value = 0
    refreshThreads(); refreshWatch(); refreshStack(); refreshLocals(); refreshSource()
  } else if (p.class === 'running') {
    statusText.value = '运行中'
  }
}

async function start() {
  if (!profileId.value) { error.value = '请选择 SSH 档案'; return }
  if (mode.value === 'attach' && !attachPid.value) { error.value = '附加模式需要填 PID'; return }
  if (mode.value === 'attach' && escalate.value === 'su' && !escalatePassword.value) { error.value = 'su 提权需要填 root 密码'; return }
  starting.value = true; error.value = ''
  consoleLines.value = []
  try {
    const args = argsText.value.trim() ? argsText.value.trim().split(/\s+/) : []
    sessionId.value = await invoke('gdb_mi_start', {
      profileId: profileId.value,
      program: program.value || null,
      args,
      attachPid: mode.value === 'attach' ? Number(attachPid.value) : null,
      escalate: mode.value === 'attach' ? escalate.value : 'none',
      escalatePassword: mode.value === 'attach' && escalate.value === 'su' ? escalatePassword.value : null,
    })
    statusText.value = '已连接'
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
        statusText.value = `已附加 PID ${attachPid.value}（已停住，加断点后点"继续"）`
        await refreshThreads(); await refreshStack(); await refreshLocals(); await refreshSource()
      }
    }
  } catch (e) { error.value = '启动失败: ' + (e?.message || e) } finally { starting.value = false }
}

async function stop() {
  if (sessionId.value) { await invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {}); sessionId.value = ''; statusText.value = '已结束' }
}

async function runCmd(cmd) {
  if (!sessionId.value) return null
  appendConsole('cmd', '» ' + cmd)
  try { return await invoke('gdb_mi_command', { sessionId: sessionId.value, command: cmd }) }
  catch (e) { error.value = '命令失败: ' + (e?.message || e); return null }
}

async function submitCmd() { const c = cmdInput.value.trim(); if (!c) return; cmdInput.value = ''; await runCmd(c) }
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
function selectThread(id) { currentThread.value = id; runCmd(`-thread-select ${id}`).then(() => refreshWatch()) }

async function refreshBreaks() {
  const r = await runCmd('-break-list')
  const body = r?.payload?.BreakpointTable?.body || []
  breaks.value = body.map(b => ({ number: b.number, where: b.func ? `${b.func} @ ${b.file || ''}:${b.line || ''}` : (b['original-location'] || b.addr || ''), times: b.times || '0' }))
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
async function refreshStack() {
  if (!sessionId.value) return
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
async function refreshLocals() {
  if (!sessionId.value) return
  const r = await runCmd('-stack-list-variables --all-values')
  const vars = r?.payload?.variables || []
  locals.value = vars.map(v => ({ name: v.name, value: v.value != null ? String(v.value) : '' }))
}
async function refreshRegisters() {
  if (!sessionId.value) return
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
async function refreshAll() {
  if (!sessionId.value) return
  await refreshThreads(); await refreshStack(); await refreshLocals()
  await refreshRegisters(); await refreshWatch(); await refreshSource()
}

// ---- 跨机源码对照（本地 readTextFile / 远端 ssh_read_text_file） ----
function mapSourcePath(fullpath) {
  const from = srcFromPrefix.value.trim()
  const to = srcToPrefix.value.trim()
  let p = fullpath
  if (from && p.startsWith(from)) p = to.replace(/[\\/]+$/, '') + p.slice(from.length)
  else if (!from && to) {
    // 无前缀时：把编译路径的文件名拼到目标根下（最朴素映射）。
    p = to.replace(/[\\/]+$/, '') + '/' + shortFile(p)
  }
  return p
}
async function pickLocalRoot() {
  try {
    const sel = await open({ directory: true, multiple: false, title: '选择本地源码根目录' })
    if (sel) srcToPrefix.value = typeof sel === 'string' ? sel : sel?.path
  } catch (_) {}
}
async function refreshSource() {
  if (!sessionId.value) return
  const r = await runCmd('-stack-info-frame')
  const fr = r?.payload?.frame
  if (!fr) { sourceHint.value = '当前无栈帧（先运行到断点）。'; return }
  const full = fr.fullname || fr.file || ''
  const line = parseInt(fr.line || '0', 10)
  sourceFile.value = full
  sourceCurLine.value = line
  if (!full) { sourceLines.value = []; sourceHint.value = '当前帧无源码信息。'; return }
  const target = mapSourcePath(full)
  try {
    let text = ''
    if (sourceMode.value === 'local') {
      text = await readTextFile(target)
    } else {
      // 远端：同时把映射告诉 gdb，使其内部 list/断点源码定位也一致。
      if (srcFromPrefix.value.trim() && srcToPrefix.value.trim()) {
        await runCmd(`-interpreter-exec console ${miQuote(`set substitute-path ${srcFromPrefix.value.trim()} ${srcToPrefix.value.trim()}`)}`)
      }
      text = await invoke('ssh_read_text_file', { profileId: profileId.value, path: target, maxBytes: 1048576 })
    }
    sourceLines.value = text.split(/\r?\n/).map((t, i) => ({ n: i + 1, text: t }))
    await nextTick()
    if (curLineEl && curLineEl.scrollIntoView) curLineEl.scrollIntoView({ block: 'center' })
  } catch (e) {
    sourceLines.value = []
    sourceHint.value = `读取源码失败（${shortFile(target)}）：${e?.message || e}`
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
function flattenVar(node, depth, out) {
  out.push('  '.repeat(depth) + `${node.expr || node.name}${node.type ? ': ' + node.type : ''} = ${node.value}`)
  if (node.expanded) for (const c of node.children) flattenVar(c, depth + 1, out)
}
function sendVarTree() {
  const out = []
  for (const n of varRoots.value) flattenVar(n, 0, out)
  if (out.length) sendToEditor('```\n' + out.join('\n') + '\n```\n')
}
function sendWatch() {
  if (!watches.value.length) return
  const text = watches.value.map(w => `${w.expr} = ${w.value}`).join('\n')
  sendToEditor('```\n' + text + '\n```\n')
}
function sendStack() {
  if (!callStack.value.length) return
  const text = callStack.value
    .map(f => `#${f.level} ${f.func} @ ${f.fullname || f.file}${f.line ? ':' + f.line : ''}`)
    .join('\n')
  sendToEditor('```\n' + text + '\n```\n')
}
function sendLocals() {
  if (!locals.value.length) return
  const text = locals.value.map(l => `${l.name} = ${l.value}`).join('\n')
  sendToEditor('```\n' + text + '\n```\n')
}
function sendRegisters() {
  if (!registers.value.length) return
  const text = registers.value.map(r => `${r.name} = ${r.hex} (${r.dec})`).join('\n')
  sendToEditor('```\n' + text + '\n```\n')
}
function sendMem() {
  if (!memText.value) return
  sendToEditor('```\n' + memText.value + '\n```\n')
}
function sendSource() {
  if (!sourceLines.value.length) return
  const body = sourceLines.value.map(l => String(l.n).padStart(5) + '  ' + l.text).join('\n')
  sendToEditor(`\`\`\`${shortFile(sourceFile.value)}\n${body}\n\`\`\`\n`)
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
.gdb-status { font-size: 12px; color: var(--guaxi-text-secondary, #6b7280); }
.gdb-error { font-size: 12px; color: #e5484d; }
.gdb-palette { display: flex; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; }
.gdb-body { flex: 1 1 auto; min-height: 0; display: flex; }
.gdb-tabs { height: 100%; width: 100%; display: flex; flex-direction: column; }
.gdb-tabs :deep(.gdb-pane) { height: 100%; padding: 8px 12px; min-height: 0; overflow: auto; }
.gdb-console { height: calc(100% - 44px); overflow: auto; background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; }
.gdb-console pre { margin: 0; white-space: pre-wrap; word-break: break-all; }
.gdb-l-log { color: #9aa0a6; }
.gdb-l-target { color: #79c0ff; }
.gdb-l-cmd { color: #7ee787; }
.gdb-input { display: flex; gap: 8px; margin-top: 8px; }
.gdb-actions { display: flex; gap: 8px; align-items: center; margin-bottom: 8px; flex-wrap: wrap; }
.gdb-hint { color: var(--guaxi-text-secondary, #9ca3af); font-size: 12px; padding: 12px; }
.gdb-tree { font-family: var(--guaxi-mono-font, monospace); font-size: 12px; }.gdb-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.gdb-table th, .gdb-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); }
.gdb-val { font-family: var(--guaxi-mono-font, monospace); white-space: pre-wrap; word-break: break-all; }
.gdb-table tr.changed { background: rgba(255,196,0,0.16); }
.gdb-threads { font-size: 12px; }
.gdb-thread { padding: 6px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); cursor: pointer; }.gdb-thread.active { background: var(--guaxi-active-bg, rgba(59,130,246,0.12)); }
.gdb-thread-h { font-weight: 500; }
.gdb-frame { color: var(--guaxi-text-secondary, #6b7280); font-family: var(--guaxi-mono-font, monospace); }
.gdb-mem { background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; white-space: pre-wrap; word-break: break-all; margin: 0; }
.gdb-empty { flex: 1 1 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--guaxi-text-secondary, #6b7280); font-size: 13px; padding: 12px; min-height: 0; }
.gdb-dead-note { flex: 0 0 auto; color: #e5484d; align-self: flex-start; }
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
.gdb-table th { text-transform: uppercase; letter-spacing: .04em; font-size: 11px; color: var(--guaxi-text-secondary); }
.gdb-thread.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.gdb-fontctl { display: inline-flex; align-items: center; gap: 4px; margin-left: auto; padding: 2px 4px; border: 1px solid var(--guaxi-border); border-radius: 999px; }
.gdb-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.gdb-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.gdb-fsz { min-width: 20px; text-align: center; font-size: 12px; font-family: var(--tw-mono-font); color: var(--tw-accent); }

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
.gdb-panel-h { flex: 0 0 auto; display: flex; align-items: center; gap: 5px; padding: 6px 8px; font-size: 12px; font-weight: 600; cursor: pointer; user-select: none; color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); }
.gdb-panel-h:hover { color: var(--tw-accent); }
.gdb-panel-ico { color: var(--tw-accent); }
.gdb-panel-t { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gdb-panel-n { flex: 0 0 auto; font-size: 10px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); font-family: var(--gdb-mono); }
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
.gdb-stack { font-size: 12px; }
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
.gdb-src-loc { font-size: 11px; color: var(--tw-accent); font-family: var(--gdb-mono); margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 220px; }
.gdb-source-body { flex: 1 1 auto; min-height: 0; overflow: auto; background: var(--tw-code-bg); color: var(--tw-code-fg); padding: 4px 0; }
.gdb-srcline { display: flex; white-space: pre; line-height: 1.5; }
.gdb-srcline.cur { background: rgba(255,196,0,0.16); box-shadow: inset 3px 0 0 #f5a524; }
.gdb-srcno { flex: 0 0 auto; width: 52px; text-align: right; padding-right: 12px; color: var(--guaxi-text-secondary); opacity: .7; user-select: none; }
.gdb-srctext { flex: 1 1 auto; }
.gdb-src-empty { padding: 12px; }
.gdb-vresize { flex: 0 0 auto; height: 5px; cursor: row-resize; background: var(--guaxi-border); opacity: .5; margin: 4px 0; border-radius: 3px; transition: opacity .15s, background .15s; }
.gdb-vresize:hover { opacity: 1; background: var(--tw-accent); }
</style>
