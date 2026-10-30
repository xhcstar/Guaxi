<template>
  <!-- embedded：独立写信窗口里没有「弹层」这回事——窗口本身就是容器。
       inline：回复直接长在阅读区顶部，不遮住列表，也不再需要 backdrop。 -->
  <div
    :class="
      inline ? 'mail-composer-inline' : embedded ? 'mail-composer-window' : 'mail-modal-backdrop'
    "
    @click.self="inline || embedded ? null : $emit('close')"
  >
    <div
      class="mail-modal mail-composer"
      :class="{
        'is-maximized': maximized,
        'is-embedded': embedded,
        'is-inline': inline,
      }"
      role="dialog"
      aria-label="写邮件"
    >
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">{{ title }}</h2>
        <select
          class="mail-select"
          :value="draft.accountId"
          title="发件账户"
          @change="$emit('update', { accountId: $event.target.value })"
        >
          <option v-for="account in accounts" :key="account.id" :value="account.id">
            {{ account.email }}
          </option>
        </select>
        <button
          v-if="!embedded && !inline"
          class="mail-icon-btn"
          :title="maximized ? '还原窗口' : '最大化'"
          :aria-label="maximized ? '还原窗口' : '最大化'"
          @click="maximized = !maximized"
        >
          <n-icon :component="maximized ? ContractOutline : ExpandOutline" />
        </button>

        <!-- 发送类操作放顶部右侧：工具栏要留给格式按钮，否则两行都挤在一起。 -->
        <div class="mail-compose-actions">
          <button
            class="mail-btn"
            :disabled="sending"
            title="保存到草稿箱"
            @click="$emit('save-draft')"
          >
            <MailIcon name="note" :size="14" />
            保存草稿
          </button>
          <button
            class="mail-btn"
            :class="{ 'is-on': scheduleOpen }"
            :disabled="sending || !canSend"
            title="选择一个将来的时间发送"
            @click="toggleSchedule"
          >
            <MailIcon name="clock" :size="14" />
            定时发送
          </button>
          <button
            v-if="undoSeconds > 0"
            class="mail-btn"
            :disabled="sending || !canSend"
            :title="`${undoSeconds} 秒内可以撤回`"
            @click="send(true)"
          >
            延迟 {{ undoSeconds }} 秒发送
          </button>
          <!-- 发送是主操作：只有它是实心主按钮；延迟发送退成次要按钮。 -->
          <button
            class="mail-btn is-primary mail-send-btn"
            :disabled="sending || !canSend"
            @click="send(false)"
          >
            {{ sending ? '发送中…' : '发送' }}
          </button>
          <!-- 放弃：紧挨着发送，允许整封作废；关之前问一句要不要留草稿。 -->
          <button
            ref="discardBtnRef"
            class="mail-icon-btn mail-discard-btn"
            :class="{ 'is-on': discardOpen }"
            title="放弃这次编辑"
            aria-label="放弃编辑"
            :disabled="sending"
            @click="toggleDiscardPanel"
          >
            <MailIcon name="trash" :size="15" />
          </button>
          <!-- fixed 定位：以前挂在操作条里（absolute），会被页头的层叠关系压住，
               背景像透明的一样，指针还能穿透去触发下面图标的提示。 -->
          <div
            v-if="discardOpen"
            class="mail-discard-panel"
            :style="{ left: `${discardPos.left}px`, top: `${discardPos.top}px` }"
            @mousedown.stop
          >
            <p class="mail-discard-title">放弃这次编辑？</p>
            <button class="mail-btn" @click="discardWithDraft">存为草稿并关闭</button>
            <button class="mail-btn is-danger" @click="discardForget">直接放弃</button>
            <button class="mail-btn" @click="discardOpen = false">取消</button>
          </div>
        </div>

        <!-- 独立窗口用系统标题栏的关闭按钮，这里再放一个只是重复。 -->
        <button v-if="!embedded" class="mail-modal-close" title="关闭" @click="$emit('close')">
          ×
        </button>
      </div>

      <div class="mail-compose-fields">
        <AddressInput
          :model-value="draft.to"
          label="收件人"
          placeholder="输入姓名或邮箱，回车确认"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { to: $event })"
        >
          <template #actions>
            <button class="mail-fmt-btn" title="抄送" @click="$emit('update', { showCc: !draft.showCc })">
              Cc
            </button>
            <button class="mail-fmt-btn" title="密送" @click="$emit('update', { showBcc: !draft.showBcc })">
              Bcc
            </button>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'to' }"
              @click="openPicker('to')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>

        <AddressInput
          v-if="draft.showCc"
          :model-value="draft.cc"
          label="抄送"
          placeholder="抄送地址"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { cc: $event })"
        >
          <template #actions>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择抄送人"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'cc' }"
              @click="openPicker('cc')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>
        <AddressInput
          v-if="draft.showBcc"
          :model-value="draft.bcc"
          label="密送"
          placeholder="密送地址"
          :suggest="suggestAddress"
          :groups="contacts.groups"
          @update:model-value="$emit('update', { bcc: $event })"
        >
          <template #actions>
            <button
              class="mail-fmt-btn"
              title="从通讯录选择密送人"
              :class="{ 'is-on': pickerOpen && pickerTarget === 'bcc' }"
              @click="openPicker('bcc')"
            >
              通讯录
            </button>
          </template>
        </AddressInput>
        <div class="mail-compose-line">
          <span class="key">主题</span>
          <input
            :value="draft.subject"
            placeholder="邮件主题"
            @input="$emit('update', { subject: $event.target.value })"
          />
        </div>
      </div>

      <AddressPicker
        v-if="pickerOpen"
        :exclude="[...draft.to, ...draft.cc, ...draft.bcc]"
        @pick="onPickAddress"
        @pick-group="onPickGroup"
        @close="pickerOpen = false"
      />

      <div class="mail-compose-toolbar">
        <template v-if="!sourceMode">
          <select
            class="mail-select mail-fmt-select mail-fmt-font"
            :class="{ 'is-native': nativeFontsUnavailable }"
            title="字体（含本机已安装的全部字体）"
            aria-label="字体"
            :style="{ fontFamily: currentFontFamily || 'inherit' }"
            @mousedown="rememberSelection"
            @change="onFontFamily($event)"
          >
            <option value="">字体</option>
            <optgroup label="常用">
              <option v-for="font in FONT_FAMILIES" :key="font.value" :value="font.value">
                {{ font.label }}
              </option>
            </optgroup>
            <optgroup v-if="systemFonts.length" label="本机字体">
              <option v-for="name in systemFonts" :key="name" :value="quoteFamily(name)">
                {{ name }}
              </option>
            </optgroup>
          </select>
          <select
            class="mail-select mail-fmt-select"
            title="字号"
            aria-label="字号"
            :style="{ fontFamily: currentFontFamily || 'inherit' }"
            @mousedown="rememberSelection"
            @change="onFontSize($event)"
          >
            <option value="">字号</option>
            <option v-for="size in FONT_SIZES" :key="size" :value="size">{{ size }} px</option>
          </select>
          <span class="mail-fmt-sep"></span>

          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.bold }"
            title="加粗 (Ctrl+B)"
            @mousedown.prevent="format('bold')"
          >
            <b>B</b>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.italic }"
            title="斜体 (Ctrl+I)"
            @mousedown.prevent="format('italic')"
          >
            <i>I</i>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.underline }"
            title="下划线 (Ctrl+U)"
            @mousedown.prevent="format('underline')"
          >
            <u>U</u>
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.strike }"
            title="删除线"
            @mousedown.prevent="format('strikeThrough')"
          >
            <span class="mail-fmt-strike">S</span>
          </button>

          <!-- 文字颜色 / 突出显示：与 Outlook 一样弹一排标准色，而不是系统取色器。
               直接点主按钮用最近一次的颜色，点箭头才展开色板。 -->
          <div class="mail-color-control" @mousedown.stop>
            <button
              class="mail-fmt-btn mail-color-main"
              title="应用最近用过的文字颜色"
              aria-label="文字颜色"
              @mousedown.prevent="rememberSelection(); applyTextColor(lastTextColor)"
            >
              <span class="mail-color-letter">A</span>
              <span class="mail-color-bar" :style="{ background: lastTextColor }"></span>
            </button>
            <button
              class="mail-color-toggle"
              title="选择文字颜色"
              aria-label="选择文字颜色"
              :class="{ 'is-on': openPalette === 'text' }"
              @mousedown.prevent="togglePalette('text')"
            >
              ▾
            </button>
            <div v-if="openPalette === 'text'" class="mail-palette" @mousedown.prevent>
              <div v-if="recentTextColors.length" class="mail-palette-block">
                <div class="mail-palette-label">最近使用</div>
                <div class="mail-palette-row">
                  <button
                    v-for="color in recentTextColors"
                    :key="'rt-' + color"
                    class="mail-swatch"
                    :style="{ background: color }"
                    :title="color"
                    @click="applyTextColor(color)"
                  ></button>
                </div>
              </div>
              <div class="mail-palette-block">
                <div class="mail-palette-label">标准色</div>
                <div v-for="(row, index) in STANDARD_COLORS" :key="'sr-' + index" class="mail-palette-row">
                  <button
                    v-for="color in row"
                    :key="color"
                    class="mail-swatch"
                    :style="{ background: color }"
                    :title="color"
                    @click="applyTextColor(color)"
                  ></button>
                </div>
              </div>
              <div class="mail-palette-actions">
                <button class="mail-btn" @click="clearTextColor">自动（清除颜色）</button>
              </div>
            </div>
          </div>

          <div class="mail-color-control" @mousedown.stop>
            <button
              class="mail-fmt-btn mail-color-main"
              title="应用最近用过的突出显示颜色"
              aria-label="突出显示"
              @mousedown.prevent="rememberSelection(); applyHiliteColor(lastHiliteColor)"
            >
              <span class="mail-hilite-letter" :style="{ background: lastHiliteColor }">A</span>
              <span class="mail-color-bar" :style="{ background: lastHiliteColor }"></span>
            </button>
            <button
              class="mail-color-toggle"
              title="选择突出显示颜色"
              aria-label="选择突出显示颜色"
              :class="{ 'is-on': openPalette === 'hilite' }"
              @mousedown.prevent="togglePalette('hilite')"
            >
              ▾
            </button>
            <div v-if="openPalette === 'hilite'" class="mail-palette" @mousedown.prevent>
              <div v-if="recentHiliteColors.length" class="mail-palette-block">
                <div class="mail-palette-label">最近使用</div>
                <div class="mail-palette-row">
                  <button
                    v-for="color in recentHiliteColors"
                    :key="'rh-' + color"
                    class="mail-swatch"
                    :style="{ background: color }"
                    :title="color"
                    @click="applyHiliteColor(color)"
                  ></button>
                </div>
              </div>
              <div class="mail-palette-block">
                <div class="mail-palette-label">标准色</div>
                <div v-for="(row, index) in HIGHLIGHT_COLORS" :key="'hr-' + index" class="mail-palette-row">
                  <button
                    v-for="color in row"
                    :key="color"
                    class="mail-swatch"
                    :style="{ background: color }"
                    :title="color"
                    @click="applyHiliteColor(color)"
                  ></button>
                </div>
              </div>
              <div class="mail-palette-actions">
                <button class="mail-btn" @click="clearHilite">无颜色</button>
              </div>
            </div>
          </div>

          <span class="mail-fmt-sep"></span>
          <button
            v-for="align in ALIGNMENTS"
            :key="align.command"
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.align === align.key }"
            :title="align.title"
            :aria-label="align.title"
            @mousedown.prevent="format(align.command)"
          >
            <MailIcon :name="align.icon" :size="15" />
          </button>

          <span class="mail-fmt-sep"></span>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.unordered }"
            title="无序列表"
            aria-label="无序列表"
            @mousedown.prevent="format('insertUnorderedList')"
          >
            <MailIcon name="listBullet" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.ordered }"
            title="有序列表"
            aria-label="有序列表"
            @mousedown.prevent="format('insertOrderedList')"
          >
            <MailIcon name="listOrdered" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="减少缩进"
            aria-label="减少缩进"
            @mousedown.prevent="format('outdent')"
          >
            <MailIcon name="indentDecrease" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="增加缩进"
            aria-label="增加缩进"
            @mousedown.prevent="format('indent')"
          >
            <MailIcon name="indentIncrease" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatState.quote }"
            title="引用"
            aria-label="引用"
            @mousedown.prevent="format('formatBlock', 'blockquote')"
          >
            <MailIcon name="quote" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="分隔线"
            aria-label="分隔线"
            @mousedown.prevent="format('insertHorizontalRule')"
          >
            <MailIcon name="divider" :size="15" />
          </button>

          <span class="mail-fmt-sep"></span>
          <button
            class="mail-fmt-btn"
            title="插入链接"
            aria-label="插入链接"
            @mousedown.prevent="insertLink"
          >
            <MailIcon name="link" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="插入代码块"
            aria-label="插入代码块"
            @mousedown.prevent="insertCode"
          >
            <MailIcon name="code" :size="15" />
          </button>
          <!-- 表格：插入的是真 <table>（带表现属性），不是 Markdown 管道语法——
               后者在 Outlook / 企业邮箱里会原样显示成一堆竖线。 -->
          <div class="mail-color-control" @mousedown.stop>
            <button
              class="mail-fmt-btn"
              title="插入表格"
              aria-label="插入表格"
              :class="{ 'is-on': openPalette === 'table' }"
              @mousedown.prevent="rememberSelection(); togglePalette('table')"
            >
              <MailIcon name="table" :size="15" />
            </button>
            <div v-if="openPalette === 'table'" class="mail-palette is-grid" @mousedown.prevent>
              <div class="mail-palette-label">插入表格：{{ tableRows }} 行 × {{ tableCols }} 列</div>
              <div class="mail-table-grid">
                <button
                  v-for="cell in 5 * 5"
                  :key="'tg-' + cell"
                  class="mail-table-cell"
                  :class="{
                    'is-on':
                      (cell - 1) % 5 < tableCols && Math.floor((cell - 1) / 5) < tableRows,
                  }"
                  :title="`${Math.floor((cell - 1) / 5) + 1} 行 × ${((cell - 1) % 5) + 1} 列`"
                  @mouseenter="hoverTable(Math.floor((cell - 1) / 5) + 1, ((cell - 1) % 5) + 1)"
                  @click="insertTable(Math.floor((cell - 1) / 5) + 1, ((cell - 1) % 5) + 1)"
                ></button>
              </div>
              <div class="mail-hint" style="margin: 6px 0 0">表头会带底色，收件端都能正常显示。</div>
            </div>
          </div>
          <!-- 查找替换：编辑器里的 Ctrl+F 会打开系统对话框，这里自己做一个
               不离开正文、能连续替换的面板。 -->
          <button
            class="mail-fmt-btn"
            title="查找与替换 (Ctrl+F)"
            aria-label="查找与替换"
            :class="{ 'is-on': findOpen }"
            @mousedown.prevent="toggleFind"
          >
            <MailIcon name="search" :size="15" />
          </button>
          <!-- Markdown：把写好的 Markdown 一次性转成邮件可显示的 HTML。 -->
          <button
            class="mail-fmt-btn"
            title="把正文里的 Markdown 转成富文本（表格会转成真表格）"
            aria-label="把正文里的 Markdown 转成富文本"
            @mousedown.prevent="convertMarkdown"
          >
            <span class="mail-md-mark">M↓</span>
          </button>
          <!-- 格式刷：先把光标处的格式复制下来，再刷到别的文字上（Outlook 的习惯）。 -->
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': formatPainter }"
            :title="formatPainter ? '格式已复制：选中要套用的文字（Esc 取消）' : '格式刷：复制当前文字的格式'"
            aria-label="格式刷"
            @mousedown.prevent="toggleFormatPainter"
          >
            <MailIcon name="formatPainter" :size="15" />
          </button>
          <span v-if="formatPainter" class="mail-fmt-hint">选一段文字刷上去</span>
          <button
            class="mail-fmt-btn"
            title="清除格式"
            aria-label="清除格式"
            @mousedown.prevent="format('removeFormat')"
          >
            <MailIcon name="clearFormat" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="撤销 (Ctrl+Z)"
            aria-label="撤销"
            @mousedown.prevent="format('undo')"
          >
            <MailIcon name="undo" :size="15" />
          </button>
          <button
            class="mail-fmt-btn"
            title="重做 (Ctrl+Y)"
            aria-label="重做"
            @mousedown.prevent="format('redo')"
          >
            <MailIcon name="redo" :size="15" />
          </button>
          <span class="mail-fmt-sep"></span>
        </template>

        <button
          class="mail-fmt-btn"
          :class="{ 'is-on': sourceMode }"
          title="切换 HTML 源码"
          aria-label="切换 HTML 源码"
          @click="toggleSource"
        >
          <MailIcon name="codeFile" :size="15" />
        </button>
        <button
          class="mail-fmt-btn"
          title="添加附件"
          aria-label="添加附件"
          @click="pickAttachments"
        >
          <MailIcon name="paperclip" :size="15" />
        </button>
        <button
          class="mail-fmt-btn"
          title="从笔记库选择文件作为附件"
          aria-label="从笔记库选择文件作为附件"
          @click="pickFromNotes"
        >
          <MailIcon name="book" :size="15" />
        </button>
        <button
          class="mail-fmt-btn"
          title="插入图片（直接显示在正文里，不当作附件）"
          aria-label="插入图片"
          @click="pickInlineImages"
        >
          <MailIcon name="image" :size="15" />
        </button>

        <!-- 模板库：一个图标弹出可选清单（名称 + 主题），点一条直接套用。
            以前是个 <select>，选项长了就被截断，也看不出模板讲什么。 -->
        <div v-if="templates.length" class="mail-color-control" @mousedown.stop>
          <button
            class="mail-fmt-btn"
            :class="{ 'is-on': openPalette === 'template' }"
            title="从模板库插入"
            aria-label="从模板库插入"
            @mousedown.prevent="togglePalette('template')"
          >
            <MailIcon name="book" :size="15" />
          </button>
          <div v-if="openPalette === 'template'" class="mail-palette is-templates" @mousedown.prevent>
            <div class="mail-palette-label">从模板库插入（覆盖主题与正文）</div>
            <button
              v-for="t in templates"
              :key="t.id"
              class="mail-template-pick"
              :title="t.subject || t.name"
              @click="applyTemplateNow(t)"
            >
              <span class="mail-template-pick-name">{{ t.name || '（未命名）' }}</span>
              <span class="mail-template-pick-subject">{{ t.subject || '（无主题）' }}</span>
            </button>
          </div>
        </div>

        <button
          v-if="needsPeerCert"
          class="mail-fmt-btn"
          :class="{ 'is-on': peerCertReady }"
          :title="peerCertReady ? '收件人证书已导入' : '加密发送需要收件人的 S/MIME 证书'"
          @click="importPeerCert"
        >
          {{ peerCertReady ? '✓ 收件人证书' : '⚠ 导入收件人证书' }}
        </button>
        <select
          class="mail-select"
          style="max-width: 150px"
          :value="draft.cryptoMode || ''"
          title="端到端加密 / 签名"
          @change="$emit('update', { cryptoMode: $event.target.value })"
        >
          <option value="">不加密</option>
          <option value="smime_sign">S/MIME 签名</option>
          <option value="smime_encrypt">S/MIME 加密</option>
          <option value="smime_sign_encrypt">S/MIME 签名并加密</option>
          <option value="pgp_sign">PGP 签名</option>
          <option value="pgp_encrypt">PGP 加密</option>
        </select>
      </div>

      <!-- 定时发送：预设几个常用时间，也可以选具体日期时间。 -->
      <div v-if="scheduleOpen" class="mail-schedule-panel">
        <div class="mail-schedule-presets">
          <span class="mail-hint">定时发送：</span>
          <button class="mail-chip" @click="scheduleAfter(1)">1 小时后</button>
          <button class="mail-chip" @click="scheduleAfter(3)">3 小时后</button>
          <button class="mail-chip" @click="scheduleTomorrowMorning()">明天 9:00</button>
        </div>
        <div class="mail-schedule-custom">
          <input
            v-model="scheduleInput"
            class="mail-input"
            type="datetime-local"
            :min="scheduleMin"
            aria-label="定时发送时间"
          />
          <button class="mail-btn is-primary" :disabled="!scheduleInput" @click="scheduleCustom">
            安排发送
          </button>
          <button class="mail-btn" @click="scheduleOpen = false">取消</button>
        </div>
      </div>

      <div class="mail-compose-body">
        <textarea
          v-if="sourceMode"
          v-show="!monacoAvailable"
          class="mail-compose-source"
          :value="htmlSource"
          spellcheck="false"
          @input="htmlSource = $event.target.value"
        ></textarea>
        <!-- 源码模式用 Monaco：语法高亮、括号匹配、查找替换，写复杂 HTML 比 textarea 稳。
             Monaco 是 MB 级依赖，切到源码模式才懒加载；加载失败时退回上面的 textarea。 -->
        <div
          v-if="sourceMode"
          v-show="monacoAvailable"
          ref="monacoSourceEl"
          class="mail-compose-source-editor"
        ></div>
        <div
          v-else
          ref="editorRef"
          class="mail-compose-editor"
          contenteditable="true"
          data-placeholder="写点什么…"
          @contextmenu="onEditorContextMenu"
          @pointerdown="onEditorPointerDown"
          @mouseup="onEditorMouseUp"
          @click="onEditorClick"
          @input="onEditorInput"
          @paste="onPaste"
          @dragstart.prevent
          @keydown.ctrl.b.prevent="format('bold')"
          @keydown.ctrl.i.prevent="format('italic')"
          @keydown.ctrl.u.prevent="format('underline')"
          @keydown.ctrl.enter.prevent="send(false)"
        ></div>
    <!-- 查找替换面板：贴在正文右上角，不遮挡正在编辑的内容。 -->
    <div v-if="findOpen" class="mail-find-panel" @mousedown.stop @keydown.esc.stop="closeFind">
      <div class="mail-find-row">
        <input
          ref="findInput"
          v-model="findText"
          class="mail-input"
          placeholder="查找…"
          @input="runFind"
          @keydown.enter.prevent="findNext"
        />
        <span class="mail-find-count">{{ findStatus }}</span>
      </div>
      <div class="mail-find-row">
        <input
          v-model="replaceText"
          class="mail-input"
          placeholder="替换为…"
          @keydown.enter.prevent="replaceOne"
        />
      </div>
      <div class="mail-find-actions">
        <button class="mail-btn" :disabled="!findText" @click="findNext">下一个</button>
        <button class="mail-btn" :disabled="!findText" @click="replaceOne">替换</button>
        <button class="mail-btn is-primary" :disabled="!findText" @click="replaceAll">
          全部替换
        </button>
        <button class="mail-btn" @click="closeFind">关闭</button>
      </div>
    </div>
    <!-- 浮条与右键菜单留在邮件根节点内部：主题变量（--mail-*）挂在那个根节点上，
         传送到 body 之后 var() 全部落空，菜单就成了「透明的」，字也看不清。
         两者都是 fixed 定位，只要没有带 transform 的祖先就不会被编辑区裁掉。 -->
    <!-- 选中图片的控制框：四个角都能拖，直观地改大小。框本身不吃鼠标事件，
         只有四个角上的把手吃——否则就没法拖动图片换位置了。
         外层是「编辑区可视范围」的裁剪层：fixed 定位的控制框一旦滚出编辑区就会
         叠到工具栏和标题上（真实反馈），套一层 overflow:hidden 就再也出不去。 -->
    <div
      v-if="imageFrame"
      class="mail-image-frame-layer"
      :style="{
        left: `${imageFrame.layer.left}px`,
        top: `${imageFrame.layer.top}px`,
        width: `${imageFrame.layer.width}px`,
        height: `${imageFrame.layer.height}px`,
      }"
    >
      <div
        class="mail-image-frame"
        :style="{
          left: `${imageFrame.left}px`,
          top: `${imageFrame.top}px`,
          width: `${imageFrame.width}px`,
          height: `${imageFrame.height}px`,
        }"
      >
        <span class="mail-image-handle is-tl" @mousedown.prevent.stop="onFrameMouseDown"></span>
        <span class="mail-image-handle is-tr" @mousedown.prevent.stop="onFrameMouseDown"></span>
        <span class="mail-image-handle is-bl" @mousedown.prevent.stop="onFrameMouseDown"></span>
        <span class="mail-image-handle is-br" @mousedown.prevent.stop="onFrameMouseDown"></span>
        <!-- 拖动把手：按住它就能把图片挪到别的段落前后，不用正好按在图片上。 -->
        <span class="mail-image-move" title="按住拖动，移动图片位置" @pointerdown.prevent.stop="startFrameMove">⠿</span>
      </div>
    </div>

    <!-- 拖动图片 / 表格时的落点线：只画一条线，松手才真正移动，
         否则被拖的那块会边拖边挤版式，指针永远追不上。 -->
    <div
      v-if="dropIndicator"
      class="mail-drop-indicator"
      :style="dropIndicatorStyle"
    ></div>

    <!-- 表格左上角的拖动把手（Outlook 的位置）：按住上下左右都能挪。 -->
    <button
      v-if="tableHandle"
      class="mail-table-move"
      :style="{ left: `${tableHandle.left}px`, top: `${tableHandle.top}px` }"
      title="按住拖动，移动表格位置"
      aria-label="拖动表格"
      @pointerdown.prevent.stop="startTableHandleDrag"
    >⠿</button>

    <!-- 选中图片 / 光标停在表格里时，贴着目标上沿浮出的一条小工具：只放最常用的
         几项，其余收进右键菜单（Outlook 就是这个分工）。fixed 定位挂在视口上，
         不占版面、也不会被编辑区的滚动容器裁掉。 -->
    <div
      v-if="editBar"
      ref="editBarRef"
      class="mail-edit-bar"
      :style="{ left: `${editBar.x}px`, top: `${editBar.y}px` }"
      @mousedown.prevent.stop
    >
      <template v-if="editBar.target === 'image'">
        <span class="mail-edit-bar-label">图片</span>
        <label class="mail-edit-bar-slider" title="拖动调整图片宽度">
          <input
            v-model.number="imageWidthPercent"
            type="range"
            min="10"
            max="100"
            step="5"
            @input="applyImageSlider"
            @change="commitImageWidth"
          />
          <span>{{ imageWidthPercent }}%</span>
        </label>
        <button
          v-for="align in IMAGE_ALIGNMENTS"
          :key="'bia-' + align.key"
          class="mail-edit-bar-btn"
          :class="{ 'is-on': imageAlign === align.key }"
          :title="align.title"
          :aria-label="align.title"
          @click="alignImage(align.key)"
        >
          <MailIcon :name="align.icon" :size="15" />
        </button>
        <span class="mail-edit-bar-sep"></span>
        <!-- 位置：直接按住把手拖到任意段落之间；也可以用上下移按钮一格一格挪。 -->
        <button
          class="mail-edit-bar-btn is-grip"
          title="按住拖动，移动图片位置"
          aria-label="拖动图片"
          @pointerdown.prevent.stop="startBarDrag('image', $event)"
        >⠿</button>
        <button class="mail-edit-bar-btn" title="上移一位（与上一段交换位置）" aria-label="上移" @click="moveImageStep(-1)">
          <MailIcon name="moveUp" :size="15" />
        </button>
        <button class="mail-edit-bar-btn" title="下移一位（与下一段交换位置）" aria-label="下移" @click="moveImageStep(1)">
          <MailIcon name="moveDown" :size="15" />
        </button>
      </template>
      <template v-else>
        <span class="mail-edit-bar-label">表格 {{ tableTools?.rows }}×{{ tableTools?.cols }}</span>
        <button
          v-for="item in TABLE_ROW_ACTIONS"
          :key="'btr-' + item.action"
          class="mail-edit-bar-btn"
          :title="item.title"
          :aria-label="item.label"
          @click="tableBarAction(item.action)"
        >
          <MailIcon :name="item.icon" :size="15" />
        </button>
        <span class="mail-edit-bar-sep"></span>
        <!-- 表格同样可以整块拖动：把手以外的地方还要用来选中单元格文字。 -->
        <button
          class="mail-edit-bar-btn is-grip"
          title="按住拖动，移动表格位置"
          aria-label="拖动表格"
          @pointerdown.prevent.stop="startBarDrag('table', $event)"
        >⠿</button>
        <button class="mail-edit-bar-btn" title="上移一位（与上一段交换位置）" aria-label="上移" @click="moveTableStep(-1)">
          <MailIcon name="moveUp" :size="15" />
        </button>
        <button class="mail-edit-bar-btn" title="下移一位（与下一段交换位置）" aria-label="下移" @click="moveTableStep(1)">
          <MailIcon name="moveDown" :size="15" />
        </button>
      </template>
      <span class="mail-edit-bar-sep"></span>
      <button class="mail-edit-bar-btn" title="更多格式（样式 / 宽度 / 单元格）" aria-label="更多" @click="openEditMenu">
        <MailIcon name="more" :size="15" />
      </button>
      <button
        class="mail-edit-bar-btn is-danger"
        :title="editBar.target === 'image' ? '从正文里删除这张图片' : '删除整个表格'"
        aria-label="删除"
        @click="removeEditTarget"
      >
        <MailIcon name="trash" :size="15" />
      </button>
    </div>

    <!-- 编辑器右键菜单：Outlook 的习惯是「所有操作都在这张菜单里」，所以浮动条放不下的
         全在这里。菜单用 fixed 挂在视口上：编辑区是 overflow 滚动容器，画在里面的菜单
         要么被裁掉、要么得自己开滚动条——两栏排版让整张菜单一屏放得下，两个问题都没有。
         表格分支左栏管行列与表格尺寸，右栏管样式与单元格格式。 -->
    <div
      v-if="editorMenu"
      ref="editorMenuRef"
      class="mail-editor-menu"
      :class="{ 'is-columns': editorMenu.table }"
      :style="{ left: `${editorMenu.x}px`, top: `${editorMenu.y}px` }"
      @mousedown.stop
      @click.stop
    >
      <template v-if="editorMenu.table">
        <div class="mail-editor-menu-col">
          <div class="mail-editor-menu-head">
            表格 {{ editorMenu.rows }} × {{ editorMenu.cols }}（第 {{ editorMenu.row + 1 }} 行，第
            {{ editorMenu.col + 1 }} 列）
          </div>
          <button class="mail-editor-menu-item" @click="tableAction('rowAbove')">在上方插入行</button>
          <button class="mail-editor-menu-item" @click="tableAction('rowBelow')">在下方插入行</button>
          <button class="mail-editor-menu-item" @click="tableAction('rowDelete')">删除这一行</button>
          <button class="mail-editor-menu-item" @click="tableAction('colLeft')">在左侧插入列</button>
          <button class="mail-editor-menu-item" @click="tableAction('colRight')">在右侧插入列</button>
          <button class="mail-editor-menu-item" @click="tableAction('colDelete')">删除这一列</button>
          <div class="mail-editor-menu-sep"></div>
          <div class="mail-editor-menu-head">表格宽度与位置</div>
          <div class="mail-editor-menu-row-actions">
            <!-- 宽度与样式都是一次选一个的枚举：收进下拉，不铺一条 chip 阵。 -->
            <select
              class="mail-editor-menu-select"
              title="表格宽度"
              aria-label="表格宽度"
              @change="onTableWidth"
            >
              <option value="">宽度</option>
              <option v-for="preset in TABLE_WIDTHS" :key="'mw-' + preset.value" :value="preset.value">
                {{ preset.label }}
              </option>
            </select>
            <button
              v-for="align in TABLE_ALIGNMENTS"
              :key="'ma-' + align.key"
              class="mail-fmt-btn"
              :title="align.title"
              :aria-label="align.title"
              @click="tableAction('tableAlign', align.key)"
            >
              <MailIcon :name="align.icon" :size="15" />
            </button>
          </div>
        </div>
        <div class="mail-editor-menu-col">
          <div class="mail-editor-menu-head">表格样式</div>
          <!-- 样式是单选枚举，收进下拉；四个 text item 平铺太占地方。 -->
          <select
            class="mail-editor-menu-select is-block"
            title="表格样式"
            aria-label="表格样式"
            @change="onTableStyle"
          >
            <option value="">选择样式…</option>
            <option v-for="style in TABLE_STYLES" :key="style.key" :value="style.key">
              {{ style.label }}
            </option>
          </select>
          <div class="mail-editor-menu-sep"></div>
          <div class="mail-editor-menu-head">单元格格式</div>
          <div class="mail-editor-menu-row-actions">
            <button
              v-for="align in CELL_ALIGNMENTS"
              :key="'mca-' + align.action"
              class="mail-fmt-btn"
              :title="align.title"
              :aria-label="align.title"
              @click="tableAction(align.action)"
            >
              <MailIcon :name="align.icon" :size="15" />
            </button>
            <button class="mail-fmt-btn" title="单元格加粗" aria-label="单元格加粗" @click="tableAction('cellBold')">
              <b>B</b>
            </button>
            <!-- 字号收进下拉：五个 chip 单独铺一行既占地方又和工具栏的字号框重复。 -->
            <select
              class="mail-editor-menu-select"
              title="单元格字号"
              aria-label="单元格字号"
              @change="onCellFontSize"
            >
              <option value="">字号</option>
              <option v-for="size in CELL_FONT_SIZES" :key="'fs-' + size" :value="size">
                {{ size }} px
              </option>
            </select>
          </div>
          <!-- 填充 / 文字颜色：点开是常用色网格（浅色打底 + 常见饱和色都有），
               想要别的再走系统取色器。关着的时候只占两个按钮。 -->
          <div class="mail-editor-menu-row-actions">
            <button
              class="mail-editor-menu-color"
              :class="{ 'is-on': colorPanel === 'fill' }"
              title="单元格填充色"
              @click="toggleColorPanel('fill')"
            >
              <span class="mail-editor-menu-color-dot" :style="{ background: DEFAULT_CELL_FILL }"></span>
              填充
            </button>
            <button
              class="mail-fmt-btn"
              title="清除填充"
              aria-label="清除填充"
              @click="tableAction('cellFill', '')"
            >
              <MailIcon name="clearFormat" :size="14" />
            </button>
            <button
              class="mail-editor-menu-color"
              :class="{ 'is-on': colorPanel === 'text' }"
              title="单元格文字颜色"
              @click="toggleColorPanel('text')"
            >
              <span class="mail-editor-menu-color-dot" :style="{ background: DEFAULT_TEXT_COLOR }"></span>
              文字
            </button>
          </div>
          <div v-if="colorPanel" class="mail-editor-menu-colors">
            <button
              v-for="color in colorPanel === 'fill' ? FILL_COLOR_LIST : TEXT_COLOR_LIST"
              :key="color"
              class="mail-editor-menu-swatch"
              :style="{ background: color }"
              :title="color"
              @click="
                colorPanel === 'fill' ? tableAction('cellFill', color) : tableAction('cellColor', color)
              "
            ></button>
            <label class="mail-editor-menu-custom" title="自定义颜色">
              <input
                type="color"
                :value="colorPanel === 'fill' ? DEFAULT_CELL_FILL : DEFAULT_TEXT_COLOR"
                @change="colorPanel === 'fill' ? onCellFillColor($event) : onCellTextColor($event)"
              />
              自定义
            </label>
          </div>
          <div class="mail-editor-menu-sep"></div>
          <button class="mail-editor-menu-item" @click="tableAction('tableDelete')">删除整个表格</button>
        </div>
      </template>
      <template v-else-if="editorMenu.image">
        <div class="mail-editor-menu-head">图片</div>
        <div class="mail-editor-menu-row-actions">
          <button
            v-for="preset in IMAGE_WIDTHS"
            :key="'miw-' + preset"
            class="mail-editor-menu-chip"
            :class="{ 'is-on': imageWidthPercent === preset }"
            :title="`图片宽度 ${preset}%`"
            @click="imageMenuAction('width', preset)"
          >
            {{ preset }}%
          </button>
        </div>
        <div class="mail-editor-menu-row-actions">
          <button
            v-for="align in IMAGE_ALIGNMENTS"
            :key="'mia-' + align.key"
            class="mail-fmt-btn"
            :class="{ 'is-on': imageAlign === align.key }"
            :title="align.title"
            :aria-label="align.title"
            @click="imageMenuAction('align', align.key)"
          >
            <MailIcon :name="align.icon" :size="15" />
          </button>
        </div>
        <div class="mail-editor-menu-sep"></div>
        <button class="mail-editor-menu-item" @click="imageMenuAction('remove')">
          从正文里删除这张图片
        </button>
      </template>
      <template v-else>
        <button class="mail-editor-menu-item" @click="editorCommand('cut')">剪切</button>
        <button class="mail-editor-menu-item" @click="editorCommand('copy')">复制</button>
        <button class="mail-editor-menu-item" @click="editorCommand('paste')">粘贴</button>
        <div class="mail-editor-menu-sep"></div>
        <button class="mail-editor-menu-item" @click="editorCommand('selectAll')">全选</button>
      </template>
    </div>

      </div>

      <div v-if="draft.attachments.length || (draft.inlineImages || []).length" class="mail-attach-list">
        <button
          v-for="att in draft.attachments"
          :key="att.path"
          class="mail-attachment"
          :class="{ 'is-danger': isDangerous(att.name) }"
          :title="warningFor(att.name) || `移除 ${att.path}`"
          @click="removeAttachment(att.path)"
        >
          <span>{{ isDangerous(att.name) ? '⚠' : '📄' }}</span>
          <span class="mail-attachment-name">{{ att.name }}</span>
          <span v-if="sizeHint(att.path)" class="mail-attachment-size">{{ sizeHint(att.path) }}</span>
          <span class="mail-attachment-size">×</span>
        </button>
        <button
          v-for="img in draft.inlineImages || []"
          :key="img.contentId"
          class="mail-attachment is-inline"
          :title="`移除图片 ${img.name || img.path}`"
          @click="removeInlineImage(img.contentId)"
        >
          <span>🖼</span>
          <span class="mail-attachment-name">{{ img.name || '插入的图片' }}</span>
          <span class="mail-attachment-size">×</span>
        </button>
      </div>
    </div>

    <!-- 划词翻译：与阅读区 / 电子书同一个面板。 -->
    <GuaxiSelectionTranslatePanel
      v-if="selectionTranslate.visible.value"
      :pos="selectionTranslate.pos.value"
      :raw="selectionTranslate.raw.value"
      :result="selectionTranslate.result.value"
      :error="selectionTranslate.error.value"
      :busy="selectionTranslate.busy.value"
      :pinned="selectionTranslate.pinned.value"
      :items="selectionTranslate.items.value"
      :from-lang="selectionTranslate.fromLang.value"
      :to-lang="selectionTranslate.toLang.value"
      :lang-options="selectionTranslate.langOptions"
      :word-meta="selectionTranslate.wordMeta.value"
      @translate="selectionTranslate.onTranslate()"
      @copy="selectionTranslate.onCopy()"
      @swap-langs="selectionTranslate.onSwapLangs()"
      @toggle-pin="selectionTranslate.onTogglePin()"
      @update:pos="(v) => { selectionTranslate.pos.value = v }"
      @update:from-lang="(v) => { selectionTranslate.fromLang.value = v }"
      @update:to-lang="(v) => { selectionTranslate.toLang.value = v }"
      @close="selectionTranslate.close()"
    />
  </div>
</template>

<script setup>
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { NIcon } from 'naive-ui'
import { ContractOutline, ExpandOutline } from '@vicons/ionicons5'
import { open } from '@tauri-apps/plugin-dialog'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { useContactsStore, expandGroupMembers, groupRecipientToken, parseGroupToken } from '../../../../stores/contactsStore'
import AddressInput from './AddressInput.vue'
import AddressPicker from './AddressPicker.vue'
import MailIcon from './MailIcon.vue'
import {
  FILL_COLOR_LIST,
  HIGHLIGHT_COLORS,
  STANDARD_COLORS,
  TEXT_COLOR_LIST,
  pushRecent,
} from './palette'
import { buildTableHtml, markdownToEmailHtml } from './composeTable'
import { flattenQuoteCards } from './quoteCards'
import { applyBodyThemeToElements } from './colors'
import { useMailSelectionTranslate } from './useMailSelectionTranslate'
import {
  TABLE_STYLES,
  cellPosition,
  collectRows,
  createCellHtml,
  insertionRowAfterInsert,
} from './tableActions'
import { createLogger } from '../../../../utils/logger'

// 划词翻译面板是写信组件里唯一依赖 naive-ui 的子树：只在真正划词时加载。
const GuaxiSelectionTranslatePanel = defineAsyncComponent(
  () => import('../../../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'),
)

const logger = createLogger('MailComposer')

const props = defineProps({
  draft: { type: Object, required: true },
  accounts: { type: Array, default: () => [] },
  settings: { type: Object, default: () => ({}) },
  themeMode: { type: String, default: 'dark' },
  undoSeconds: { type: Number, default: 0 },
  /** 模板清单（store.templates）。空数组时下拉不显示。 */
  templates: { type: Array, default: () => [] },
  /** 附件检查结果：[{ path, name, size, level, detail }]（store.checkAttachments）。 */
  attachmentChecks: { type: Array, default: () => [] },
  /** 独立窗口模式：不画弹层外壳、不显示最大化与内部关闭按钮。 */
  embedded: { type: Boolean, default: false },
  /** 阅读区内联模式：卡片长在正文上方，不铺满整个应用。 */
  inline: { type: Boolean, default: false },
  /**
   * 写进服务器草稿箱（编辑器自己的 15 秒自动保存走这里）。
   *
   * 必须是函数 prop：编辑器要把「刚才那份草稿的 uid」记下来，下一次保存先删旧的
   * 再写新的——否则每 15 秒就在草稿箱里多一封同名草稿，用户看到一堆
   * 只带引用原文的「空壳」，会以为自己的正文没被保存。
   */
  saveMailboxDraft: { type: Function, default: null },
})
const emit = defineEmits([
  'update',
  'send',
  'schedule',
  'save-draft',
  'close',
  /** 用户明确选择「直接放弃」：不留草稿，父级不要走保存流程。 */
  'discard',
  'apply-template',
  'check-attachments',
])

const editorRef = ref(null)
const maximized = ref(false)
const pickerOpen = ref(false)
/** 通讯录选择器当前给哪个字段加人：收件人 / 抄送 / 密送。 */
const pickerTarget = ref('to')
const contacts = useContactsStore()
const sourceMode = ref(false)
const htmlSource = ref(props.draft.html || '')
const monacoSourceEl = ref(null)
const monacoAvailable = ref(false)
const sending = ref(false)
const scheduleOpen = ref(false)
const scheduleInput = ref('')
const scheduleMin = computed(() => toLocalInput(Date.now() + 60_000))
let lastHtml = props.draft.html || ''
let monacoNs = null
let sourceEditor = null

/** 工具栏字体 / 字号 / 对齐选项。字体只列系统里大概率存在的，避免选了却回退。 */
const FONT_FAMILIES = [
  { label: '系统默认', value: '' },
  { label: '微软雅黑', value: "'Microsoft YaHei', 'PingFang SC', sans-serif" },
  { label: '宋体', value: "SimSun, 'Songti SC', serif" },
  { label: '黑体', value: "SimHei, 'Heiti SC', sans-serif" },
  { label: '楷体', value: "KaiTi, 'Kaiti SC', serif" },
  { label: 'Arial', value: 'Arial, Helvetica, sans-serif' },
  { label: 'Calibri', value: 'Calibri, Candara, Segoe UI, sans-serif' },
  { label: 'Georgia', value: "Georgia, 'Times New Roman', serif" },
  { label: 'Times New Roman', value: "'Times New Roman', Times, serif" },
  { label: '等宽', value: "Consolas, 'Courier New', monospace" },
]
const FONT_SIZES = [12, 13, 14, 15, 16, 18, 20, 24, 28, 32]

/**
 * 本机已安装的字体家族。
 *
 * WebView 里没有系统字体枚举 API（`queryLocalFonts` 在 WebView2 里拿不到权限），
 * 所以走后端 `list_system_fonts`（fontdb 读系统字体库，设置页的阅读字体也用它）。
 * 字体名可能带空格或中文，写进 `execCommand('fontName')` 与内联样式时都要加引号。
 */
const systemFonts = ref([])
/** 一次运行内字体列表不会变，失败也不重试（避免每次打开写信窗口都扫一遍字体库）。 */
let systemFontsLoaded = false

const nativeFontsUnavailable = computed(() => systemFontsLoaded && systemFonts.value.length === 0)

/** CSS 里 `Microsoft YaHei` 这类带空格的名字必须加引号，否则被当成多个候选。 */
function quoteFamily(name) {
  const clean = String(name || '').trim()
  if (!clean) return ''
  if (/^['"]/.test(clean)) return clean
  return /[\s,]/.test(clean) || /[^\x00-\x7F]/.test(clean) ? `'${clean.replace(/'/g, "\\'")}'` : clean
}

/** 光标所在位置实际用的字体（工具栏下拉回显）。 */
const currentFontFamily = computed(() => {
  const family = formatState.fontFamily
  if (!family) return ''
  return family.split(',')[0].trim().replace(/^["']|["']$/g, '')
})

async function loadSystemFonts() {
  if (systemFontsLoaded) return
  systemFontsLoaded = true
  try {
    const families = await invoke('list_system_fonts')
    // 后端按「等宽优先」排过序；写信场景更常用正文字体，这里按名字重排一次。
    systemFonts.value = (families || [])
      .map((item) => item.name)
      .filter(Boolean)
      .sort((a, b) => a.localeCompare(b, 'zh-CN'))
  } catch {
    systemFonts.value = []
  }
}
const ALIGNMENTS = [
  { command: 'justifyLeft', key: 'left', icon: 'alignLeft', title: '左对齐' },
  { command: 'justifyCenter', key: 'center', icon: 'alignCenter', title: '居中' },
  { command: 'justifyRight', key: 'right', icon: 'alignRight', title: '右对齐' },
  { command: 'justifyFull', key: 'full', icon: 'alignJustify', title: '两端对齐' },
]
const formatState = reactive({
  bold: false,
  italic: false,
  underline: false,
  strike: false,
  unordered: false,
  ordered: false,
  quote: false,
  align: '',
  fontFamily: '',
})
const lastTextColor = ref('#1a5fd0')
const lastHiliteColor = ref('#fff3a3')

/** 当前展开的面板：'text' | 'hilite' | 'table' | ''。 */
const openPalette = ref('')
const recentTextColors = ref([])
const recentHiliteColors = ref([])
/** 表格选择器的当前行列（鼠标悬停时跟着变）。 */
const tableRows = ref(3)
const tableCols = ref(3)

function togglePalette(name) {
  openPalette.value = openPalette.value === name ? '' : name
  if (openPalette.value === 'text') rememberSelection()
  if (openPalette.value === 'hilite') rememberSelection()
}

function closePalette() {
  openPalette.value = ''
}

function onPaletteEscape(event) {
  if (event.key !== 'Escape') return
  if (openPalette.value) {
    event.stopPropagation()
    closePalette()
    return
  }
  // 格式刷的「待刷」状态也按 Esc 取消，免得变成一个退不掉的隐形模式。
  if (formatPainter.value) {
    event.stopPropagation()
    formatPainter.value = null
  }
}

/** 点空白 / 按 Esc 收起色板。 */
function onPaletteOutside(event) {
  if (!openPalette.value) return
  if (event.target?.closest?.('.mail-color-control')) return
  closePalette()
  // 右键菜单也一起收：两条浮层同时开着会互相压住。
  closeEditorMenu()
}

function hoverTable(rows, cols) {
  tableRows.value = rows
  tableCols.value = cols
}

/** 应用文字颜色：保留光标位置，颜色记进「最近使用」。 */
function applyTextColor(color) {
  closePalette()
  if (!color) return
  lastTextColor.value = color
  recentTextColors.value = pushRecent(recentTextColors.value, color)
  restoreSelection()
  format('foreColor', color)
}

/** 工具栏控件抢焦点前的选区：下拉 / 取色器关闭后要能接着改刚才选中的文字。 */
let savedRange = null
function rememberSelection() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return
  if (!editorRef.value?.contains(selection.anchorNode)) return
  savedRange = selection.getRangeAt(0).cloneRange()
}

function restoreSelection() {
  const editor = editorRef.value
  if (!editor) return null
  editor.focus()
  const selection = window.getSelection()
  if (!selection) return null
  if (savedRange) {
    try {
      selection.removeAllRanges()
      selection.addRange(savedRange)
    } catch {
      // 之前的选区节点可能已经被删除（粘贴图片 / 清除格式）：退回光标当前的位置。
      savedRange = null
    }
  }
  return selection
}

/** 光标/选区变化时刷新工具栏的按下状态。 */
function refreshFormatState() {
  const editor = editorRef.value
  const selection = window.getSelection()
  // 表格工具行跟着光标走：光标进出表格时它要跟着出现 / 收起。
  refreshTableTools()
  if (!editor || !selection?.rangeCount || !editor.contains(selection.anchorNode)) return
  // 选区一变就记下来：否则先加粗 A、再选中 B 点斜体时，会拿旧的 A 去应用格式。
  savedRange = selection.getRangeAt(0).cloneRange()
  try {
    formatState.bold = document.queryCommandState('bold')
    formatState.italic = document.queryCommandState('italic')
    formatState.underline = document.queryCommandState('underline')
    formatState.strike = document.queryCommandState('strikeThrough')
    formatState.unordered = document.queryCommandState('insertUnorderedList')
    formatState.ordered = document.queryCommandState('insertOrderedList')
    formatState.align =
      ['justifyCenter', 'justifyRight', 'justifyFull', 'justifyLeft'].find((command) =>
        document.queryCommandState(command),
      )?.replace('justify', '').toLowerCase() || ''
    formatState.quote = String(document.queryCommandValue('formatBlock') || '')
      .toLowerCase()
      .includes('blockquote')
    // 字体下拉要回显「选区现在用的字体」：execCommand 查不到时退回计算样式。
    const queried = String(document.queryCommandValue('fontName') || '').trim()
    if (queried) {
      formatState.fontFamily = queried
    } else {
      const anchor = selection.anchorNode?.nodeType === 1
        ? selection.anchorNode
        : selection.anchorNode?.parentElement
      formatState.fontFamily = anchor ? getComputedStyle(anchor).fontFamily || '' : ''
    }
  } catch {
    /* WebView 对部分命令的查询会抛错：保持上一次状态即可 */
  }
}

const title = computed(
  () =>
    ({
      new: '新邮件',
      reply: '回复',
      replyAll: '全部回复',
      forward: '转发',
    })[props.draft.mode] || '新邮件',
)

const canSend = computed(
  () => props.draft.to.length || props.draft.cc.length || props.draft.bcc.length,
)

onMounted(async () => {
  // 通讯组只在地址栏放芯片，成员人数与发送确认要读 contacts.groups；
  // 这里顺手初始化一次，没打开过通讯录页时也能算出真实人数。
  contacts.init().catch(() => {})
  // 字体列表是「打开写信窗口才需要」的重操作（要扫系统字体库），发出去不等待。
  loadSystemFonts()
  document.addEventListener('mousedown', onPaletteOutside)
  document.addEventListener('mousedown', onTranslateOutside)
  document.addEventListener('keydown', onPaletteEscape)
  document.addEventListener('selectionchange', refreshFormatState)
  if (!editorRef.value) return
  // 直接把已有 HTML 灌进 contenteditable：用 innerHTML 而不是 v-html，
  // 避免 Vue 与浏览器的编辑结果互相覆盖（contenteditable 里的 DOM 是浏览器在管）。
  editorRef.value.innerHTML = props.draft.html || ''
  // 旧草稿 / 转发来的正文里可能还留着一层套一层的引用卡片：统一压平成
  // 「按时间顺序的卡片列表」（阅读区早就是这么做的）。以前只在 buildQuote 里压，
  // 从草稿恢复进独立写信窗口时还是层层嵌套（真实反馈）。
  flattenQuoteCards(editorRef.value)
  lastHtml = editorRef.value.innerHTML
  initialMailboxSignature = mailboxSignature(lastHtml)
  resolveCidImages()
  scheduleQuoteTheme()
  await nextTick()
  editorRef.value.focus()
})

// 从「源码模式」切回所见即所得时，把编辑区的 DOM 同步成最新的源码。
watch(sourceMode, async (on) => {
  if (!on) {
    disposeSourceEditor()
    monacoAvailable.value = false
    await nextTick()
    if (editorRef.value) {
      editorRef.value.innerHTML = htmlSource.value
      lastHtml = htmlSource.value
      resolveCidImages()
      scheduleQuoteTheme()
    }
  } else {
    htmlSource.value = serializeEditorHtml() || props.draft.html || ''
    await nextTick()
    await ensureSourceEditor()
  }
})

// 源码编辑器跟随应用明暗主题。
watch(
  () => props.themeMode,
  (mode) => {
    monacoNs?.editor.setTheme(mode === 'light' ? 'vs' : 'vs-dark')
    sourceEditor?.layout()
    // 引用历史的配色也要跟着换：切到亮色时把映射结果还原成邮件原值。
    scheduleQuoteTheme()
  },
)

/** 懒加载 Monaco：只有切到源码模式才加载（MB 级依赖）。 */
async function ensureSourceEditor() {
  if (!sourceMode.value) return
  if (sourceEditor) {
    sourceEditor.layout()
    return
  }
  if (!monacoNs) {
    try {
      monacoNs = await import('monaco-editor')
    } catch (error) {
      logger.warn('Monaco 加载失败，源码模式回退到 textarea', error)
      return
    }
  }
  if (!sourceMode.value) return
  monacoAvailable.value = true
  await nextTick()
  if (!monacoSourceEl.value || sourceEditor) return
  try {
    sourceEditor = monacoNs.editor.create(monacoSourceEl.value, {
      value: htmlSource.value,
      language: 'html',
      theme: props.themeMode === 'light' ? 'vs' : 'vs-dark',
      automaticLayout: true,
      minimap: { enabled: false },
      wordWrap: 'on',
      fontSize: 12.5,
      lineNumbers: 'on',
      scrollBeyondLastLine: false,
      padding: { top: 12, bottom: 12 },
      tabSize: 2,
    })
    sourceEditor.onDidChangeModelContent(() => {
      if (!sourceEditor) return
      const value = sourceEditor.getValue()
      if (value !== htmlSource.value) htmlSource.value = value
    })
  } catch (error) {
    logger.warn('Monaco 初始化失败，源码模式回退到 textarea', error)
    monacoAvailable.value = false
    sourceEditor = null
  }
}

function disposeSourceEditor() {
  sourceEditor?.dispose()
  sourceEditor = null
}

// ---------------- 查找与替换 ----------------

const findOpen = ref(false)
const findText = ref('')
const replaceText = ref('')
const findInput = ref(null)
/** `当前 / 总数`，给用户一个「还剩几处」的直观反馈。 */
const findStatus = ref('')
/** 匹配到的文本节点区间（从后往前替换，避免偏移失效）。 */
let findMatches = []

function toggleFind() {
  if (findOpen.value) {
    closeFind()
    return
  }
  findOpen.value = true
  // 带上编辑器里选中的词，省得用户再打一遍
  const selected = window.getSelection()?.toString?.() || ''
  if (selected && selected.length < 80) findText.value = selected
  nextTick(() => {
    findInput.value?.focus()
    findInput.value?.select()
    runFind()
  })
}

function closeFind() {
  findOpen.value = false
  findStatus.value = ''
  findMatches = []
  clearFindHighlight()
  editorRef.value?.focus()
}

/** 在编辑器的文本节点里找出全部匹配（不跨节点，够用且不会破坏 DOM）。 */
function collectMatches() {
  const editor = editorRef.value
  const needle = findText.value
  findMatches = []
  if (!editor || !needle) return
  const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT)
  let node = walker.nextNode()
  while (node) {
    const hay = node.nodeValue || ''
    let index = hay.indexOf(needle)
    while (index >= 0) {
      findMatches.push({ node, start: index, end: index + needle.length })
      index = hay.indexOf(needle, index + needle.length)
    }
    node = walker.nextNode()
  }
}

function runFind() {
  collectMatches()
  findStatus.value = findText.value
    ? findMatches.length
      ? `${findMatches.length} 处`
      : '无匹配'
    : ''
}

/** 跳到下一处并把选区落在它上面。 */
function findNext() {
  if (!findText.value) return
  collectMatches()
  if (!findMatches.length) {
    findStatus.value = '无匹配'
    return
  }
  const first = findMatches[0]
  const range = document.createRange()
  range.setStart(first.node, first.start)
  range.setEnd(first.node, first.end)
  const selection = window.getSelection()
  selection.removeAllRanges()
  selection.addRange(range)
  // 滚到可见位置
  const rect = range.getBoundingClientRect()
  const editor = editorRef.value
  if (editor) {
    const box = editor.getBoundingClientRect()
    if (rect.bottom > box.bottom - 40 || rect.top < box.top + 40) {
      editor.scrollIntoView({ block: 'nearest' })
    }
  }
}

/** 替换当前这一处（即选区处的匹配），然后自动跳到下一处。 */
function replaceOne() {
  if (!findText.value) return
  const editor = editorRef.value
  if (!editor) return
  const selection = window.getSelection()
  if (selection && selection.rangeCount && selection.toString() === findText.value) {
    document.execCommand('insertText', false, replaceText.value)
    onEditorInput()
  }
  runFind()
  findNext()
}

/**
 * 全部替换。
 *
 * 从后往前替换每个匹配：这样前面节点的偏移不会被后面的替换打乱。
 * 用的是「文本节点直接赋值」而不是 execCommand——批量替换时 execCommand
 * 会破坏编辑器里已有的选区与撤销栈。
 */
function replaceAll() {
  if (!findText.value) return
  collectMatches()
  if (!findMatches.length) {
    findStatus.value = '无匹配'
    return
  }
  const count = findMatches.length
  for (let i = findMatches.length - 1; i >= 0; i -= 1) {
    const { node, start, end } = findMatches[i]
    const hay = node.nodeValue || ''
    node.nodeValue = hay.slice(0, start) + replaceText.value + hay.slice(end)
  }
  onEditorInput()
  findStatus.value = `已替换 ${count} 处`
  findMatches = []
}

function clearFindHighlight() {
  const selection = window.getSelection()
  if (selection) selection.removeAllRanges()
}

// ---------------- 划词翻译 ----------------

/**
 * 划词翻译的容器是「正文编辑区」：在工具栏 / 地址栏上拖选不该触发译文。
 */
const selectionTranslate = useMailSelectionTranslate(() => editorRef.value)

/**
 * 鼠标松开时只把事件交给共享的划词逻辑：触发条件是「Alt + 划词」
 * （见 useMailSelectionTranslate）。普通拖选不弹面板，否则每松一次鼠标就译一次。
 */
// ---------------- 格式刷 ----------------

/**
 * 格式刷：先把光标处的格式「复制」下来，再去刷另一段文字。一次刷完自动取消，
 * Esc 也能取消——和 Outlook 一致，不做一个会一直粘着的隐形模式。
 */
const formatPainter = ref(null)

function toggleFormatPainter() {
  if (formatPainter.value) {
    formatPainter.value = null
    return
  }
  formatPainter.value = captureFormat() || null
}

/** 从光标所在元素上读出要复制的格式（行内样式 + 段落对齐）。 */
function captureFormat() {
  const selection = window.getSelection()
  const node = selection?.anchorNode
  const el = node ? (node.nodeType === 1 ? node : node.parentElement) : null
  const editor = editorRef.value
  if (!el || !editor?.contains(el)) return null
  const style = window.getComputedStyle(el)
  const background = style.backgroundColor
  return {
    fontFamily: style.fontFamily,
    fontSize: style.fontSize,
    fontWeight: style.fontWeight,
    fontStyle: style.fontStyle,
    textDecoration: style.textDecorationLine === 'none' ? '' : style.textDecorationLine,
    color: style.color,
    // 透明背景别刷成一层白，否则「正常文字」被刷出底色。
    backgroundColor: /rgba\(0, 0, 0, 0\)|transparent/i.test(background) ? '' : background,
    textAlign: style.textAlign,
  }
}

/** 把复制的格式套到当前选区上。返回 true 表示这次确实刷了。 */
function paintFormat() {
  const paint = formatPainter.value
  const selection = window.getSelection()
  if (!paint || !selection || !selection.rangeCount) return false
  const range = selection.getRangeAt(0)
  if (range.collapsed) return false
  const span = document.createElement('span')
  for (const [key, value] of Object.entries(paint)) {
    if (!value || key === 'textAlign') continue
    span.style[key] = value
  }
  span.appendChild(range.extractContents())
  range.insertNode(span)
  // 对齐是段落级的，行内 span 表达不了，落到最近的块级祖先上。
  if (paint.textAlign) {
    const block = span.parentElement?.closest?.('p, div, li, td, th, h1, h2, h3, blockquote')
    if (block) block.style.textAlign = paint.textAlign
  }
  const next = document.createRange()
  next.selectNodeContents(span)
  selection.removeAllRanges()
  selection.addRange(next)
  savedRange = next.cloneRange()
  formatPainter.value = null
  onEditorInput()
  refreshFormatState()
  return true
}

function onEditorMouseUp(event) {
  // 格式刷等着刷：这一下松开鼠标就是「套用」。
  if (formatPainter.value && paintFormat()) return
  selectionTranslate.handleMouseUp(event)
}

/** 点正文以外的地方收起译文（固定时保留，用户要它留着对照看）。 */
function onTranslateOutside(event) {
  selectionTranslate.handleDocumentMouseDown(event)
  // 放弃菜单：点面板和按钮以外的地方收起。
  if (discardOpen.value && !event.target?.closest?.('.mail-discard-panel, .mail-discard-btn')) {
    discardOpen.value = false
  }
}

// ---------------- 放弃这次编辑 ----------------

/** 放弃按钮展开的小面板：只问「要不要留一份草稿」。 */
const discardOpen = ref(false)
const discardBtnRef = ref(null)
/** 面板的视口坐标（fixed 定位，见模板里的注释）。 */
const discardPos = ref({ left: 0, top: 0 })
/** 用户已经选了「直接放弃」：之后的自动暂存一律不再写。 */
let discarded = false

function toggleDiscardPanel() {
  if (discardOpen.value) {
    discardOpen.value = false
    return
  }
  const el = discardBtnRef.value
  if (el) {
    const box = el.getBoundingClientRect()
    discardPos.value = {
      left: Math.max(8, Math.round(box.right - 200)),
      top: Math.round(box.bottom + 6),
    }
  }
  discardOpen.value = true
}

/** 留草稿再关：交给父级的关闭流程去写草稿箱。 */
function discardWithDraft() {
  discardOpen.value = false
  emit('close')
}

/** 什么都不留：把本地暂存也删掉，免得下次启动又被「恢复」出来。 */
async function discardForget() {
  discardOpen.value = false
  // 先立旗：这之后的自动保存 / 卸载暂存都不许再写，否则刚删掉的草稿会在
  // 关窗口那一刻又被补回去（自动保存是 15 秒一跳，正好撞上就白删了）。
  discarded = true
  stopAutosave()
  try {
    await invoke('mail_delete_stashed_draft', { id: draftId.value })
  } catch (error) {
    logger.warn('删除本地暂存失败', error)
  }
  emit('discard')
}

// ---------------- 图片 / 表格的可见工具 ----------------
//
// 「插进去就动不了」是最常见的一条反馈：图片只能删、表格只能靠右键（很多人不知道
// 右键）。这里把两类操作做成工具栏下方的第二行——点了图片、或光标停在表格里就出现。

/** 正文里被点选的图片；工具条的宽度 / 对齐 / 删除都作用于它。 */
const selectedImage = ref(null)
const imageWidthPercent = ref(100)
/** 图片当前的对齐方式（工具条按钮的按下态）。DOM 上写的样式不会自动触发重算。 */
const imageAlign = ref('left')
const IMAGE_WIDTHS = [25, 50, 75, 100]
const IMAGE_ALIGNMENTS = [
  { key: 'left', icon: 'alignLeft', title: '图片靠左' },
  { key: 'center', icon: 'alignCenter', title: '图片居中' },
  { key: 'right', icon: 'alignRight', title: '图片靠右' },
]

function onEditorClick(event) {
  const img = event.target?.closest?.('img')
  if (img && editorRef.value?.contains(img)) {
    selectImage(img)
    return
  }
  selectedImage.value = null
}

/** 选中正文里的一张图片，并把工具条上的宽度 / 对齐读数同步成它当前的样子。 */
function selectImage(img) {
  selectedImage.value = img
  imageWidthPercent.value = imageWidthPercentOf(img)
  imageAlign.value = alignOf(img)
}

/** 图片当前的对齐：左右 auto 外边距的三种组合，其余按靠左处理。 */
function alignOf(img) {
  if (img.style.marginLeft === 'auto' && img.style.marginRight === 'auto') return 'center'
  if (img.style.marginLeft === 'auto') return 'right'
  return 'left'
}

/** 图片当前宽度（百分比）：优先读内联宽度，其次按编辑区宽度折算。 */
function imageWidthPercentOf(img) {
  const declared = String(img.style?.width || img.getAttribute('width') || '')
  const percent = declared.match(/^([\d.]+)%$/)
  if (percent) return Math.max(10, Math.min(100, Math.round(Number(percent[1]))))
  const box = editorRef.value?.clientWidth || 0
  const width = img.getBoundingClientRect?.().width || 0
  if (box && width) {
    return Math.max(10, Math.min(100, Math.round((width / box) * 100)))
  }
  return 100
}

/**
 * 宽度写成百分比：收件端窗口再宽再窄，版式都不会被撑破。
 *
 * `commit = false` 用于拖动滑块的过程：只改屏幕上的宽度，松手（change）时才
 * 同步回草稿——拖一次会触发几十个 input，每个都序列化整篇正文会明显卡顿。
 */
function setImageWidth(percent, { commit = true } = {}) {
  const img = selectedImage.value
  if (!img) return
  const value = Math.max(10, Math.min(100, Number(percent) || 100))
  img.style.width = `${value}%`
  img.style.height = 'auto'
  img.removeAttribute('width')
  img.removeAttribute('height')
  imageWidthPercent.value = value
  if (commit) onEditorInput()
}

function applyImageSlider() {
  if (selectedImage.value) setImageWidth(imageWidthPercent.value, { commit: false })
}

function commitImageWidth() {
  if (selectedImage.value) onEditorInput()
}

/** 图片的「位置」：块级图片用左右 auto 外边距实现，表现属性之外客户端也认。 */
function alignImage(where) {
  const img = selectedImage.value
  if (!img) return
  img.style.display = 'block'
  img.style.marginLeft = where === 'right' || where === 'center' ? 'auto' : '0'
  img.style.marginRight = where === 'left' || where === 'center' ? 'auto' : '0'
  imageAlign.value = where
  onEditorInput()
}

/** 删图：正文里的 <img> 与「内联图片」登记要一起摘掉，否则会发出一个没人引用的附件。 */
function removeSelectedImage() {
  const img = selectedImage.value
  if (!img) return
  const src = img.getAttribute('src') || ''
  const cid = img.dataset?.cid || (/^cid:/i.test(src) ? src.slice(4) : '')
  img.remove()
  selectedImage.value = null
  if (cid) removeInlineImage(cid)
  else onEditorInput()
}

/** 光标所在的表格；不在表格里就是 null（工具行随之收起）。 */
const tableTools = ref(null)

// 浮动条只放最常用的几项，用图标 + 悬浮提示（文字标签留给右键菜单）。
const TABLE_ROW_ACTIONS = [
  { action: 'rowAbove', icon: 'rowAbove', label: '上方插入行', title: '在当前行上方插入一行' },
  { action: 'rowBelow', icon: 'rowBelow', label: '下方插入行', title: '在当前行下方插入一行' },
  { action: 'rowDelete', icon: 'rowDelete', label: '删除行', title: '删除光标所在这一行' },
  { action: 'colLeft', icon: 'colLeft', label: '左侧插入列', title: '在当前列左侧插入一列' },
  { action: 'colRight', icon: 'colRight', label: '右侧插入列', title: '在当前列右侧插入一列' },
  { action: 'colDelete', icon: 'colDelete', label: '删除列', title: '删除光标所在这一列' },
]
const TABLE_WIDTHS = [
  { value: '100', label: '整行', title: '表格占满正文宽度' },
  { value: '75', label: '75%', title: '表格宽度 75%' },
  { value: '50', label: '50%', title: '表格宽度 50%' },
  { value: 'auto', label: '按内容', title: '表格宽度由内容决定' },
]
const TABLE_ALIGNMENTS = [
  { key: 'left', icon: 'alignLeft', title: '表格靠左' },
  { key: 'center', icon: 'alignCenter', title: '表格居中' },
  { key: 'right', icon: 'alignRight', title: '表格靠右' },
]
const CELL_ALIGNMENTS = [
  { key: 'left', action: 'cellAlignLeft', icon: 'alignLeft', title: '单元格内容靠左' },
  { key: 'center', action: 'cellAlignCenter', icon: 'alignCenter', title: '单元格内容居中' },
  { key: 'right', action: 'cellAlignRight', icon: 'alignRight', title: '单元格内容靠右' },
]

function refreshTableTools() {
  const editor = editorRef.value
  const selection = window.getSelection()
  if (!editor || !selection?.rangeCount) {
    tableTools.value = null
    return
  }
  const anchor = selection.anchorNode
  const node = anchor?.nodeType === 1 ? anchor : anchor?.parentElement
  if (!node || !editor.contains(node)) {
    tableTools.value = null
    return
  }
  const cell = node.closest?.('td, th')
  const table = cell?.closest?.('table')
  if (!table || !editor.contains(table)) {
    tableTools.value = null
    return
  }
  const pos = cellPosition(table, cell)
  tableTools.value = pos ? { table: true, cell, tableEl: table, ...pos } : null
}

/** 工具行里的表格动作：作用于「光标当前所在的表格」，不用先右键。 */
function tableBarAction(action, payload) {
  if (!tableTools.value) return
  tableAction(action, payload, tableTools.value)
  nextTick(refreshTableTools)
}

// ---------------- 图片 / 表格的浮动工具条 ----------------

/**
 * 贴着选中目标浮出的小工具条：`{ target, anchor, x, y }`。
 *
 * `anchor` 是目标元素当时的视口矩形（拆成普通数字存：DOMRect 塞进 ref 会被套上
 * 响应式代理，取 `.width` 会以代理为 this 触发原型 getter，直接 Illegal invocation）。
 * 存矩形而不是存元素，是为了滚动时能直接重算位置。
 */
const editBar = ref(null)
const editBarRef = ref(null)

/** 当前要挂浮条的对象：先图片（点选优先），否则光标所在的表格。都不在就返回 null。 */
function editTargetSpot() {
  const editor = editorRef.value
  if (!editor || sourceMode.value) return null
  const image = selectedImage.value
  if (image && editor.contains(image)) {
    return { target: 'image', anchor: viewportRect(image) }
  }
  const table = tableTools.value?.tableEl
  if (table && editor.contains(table)) {
    return { target: 'table', anchor: viewportRect(table) }
  }
  return null
}

function viewportRect(el) {
  const box = el.getBoundingClientRect()
  return {
    left: box.left,
    top: box.top,
    width: box.width,
    height: box.height,
    bottom: box.bottom,
  }
}

function refreshEditBar() {
  const spot = editTargetSpot()
  refreshImageFrame()
  refreshTableHandle()
  if (!spot) {
    editBar.value = null
    return
  }
  // 光标每动一下都会重算表格状态（selectionchange 很密），目标和位置都没变就别重排，
  // 否则浮条会在每次按键时重新量一遍、看着像在抖。
  const previous = editBar.value
  if (previous && previous.target === spot.target && sameAnchor(previous.anchor, spot.anchor)) return
  // 位置要等浮条渲染出来量了尺寸才知道，先按上沿占位，nextTick 里再校正。
  editBar.value = { ...spot, x: Math.round(spot.anchor.left), y: Math.round(spot.anchor.top) }
  nextTick(placeEditBar)
}

function sameAnchor(a, b) {
  return (
    !!a &&
    !!b &&
    a.left === b.left &&
    a.top === b.top &&
    a.width === b.width &&
    a.height === b.height
  )
}

/** 浮条默认贴在目标上方居中；上方放不下就翻到下方，左右都夹进视口，绝不溢出屏幕。 */
function placeEditBar() {
  const el = editBarRef.value
  const bar = editBar.value
  if (!el || !bar) return
  const box = el.getBoundingClientRect()
  const anchor = bar.anchor
  const x = clamp(bar.anchor.left + anchor.width / 2 - box.width / 2, 8, window.innerWidth - box.width - 8)
  let y = anchor.top - box.height - 6
  if (y < 8) y = Math.min(anchor.bottom + 6, window.innerHeight - box.height - 8)
  editBar.value = { ...bar, x: Math.round(clamp(x, 8, window.innerWidth - box.width - 8)), y: Math.round(Math.max(8, y)) }
}

function clamp(value, low, high) {
  if (high < low) return low
  return Math.max(low, Math.min(value, high))
}

/**
 * 表格左上角的拖动把手。
 *
 * Outlook 把「挪表格」的把手钉在表格左上角，用户的肌肉记忆就在那儿；放在浮条里
 * 既不好找、也离表格太远。这里跟着表格矩形走，滚出可视区就收起。
 */
function refreshTableHandle() {
  const editor = editorRef.value
  const table = tableTools.value?.tableEl
  if (!editor || !table || !editor.contains(table) || sourceMode.value || blockDrag?.active) {
    tableHandle.value = null
    return
  }
  const rect = table.getBoundingClientRect()
  // 可视范围取滚动容器：编辑区本身的高度是整篇正文，拿它算会让把手跑到工具栏上。
  const scroller = dragScroller() || editor
  const view = scroller.getBoundingClientRect()
  if (rect.bottom <= view.top + 4 || rect.top >= view.bottom - 4) {
    tableHandle.value = null
    return
  }
  const left = Math.round(Math.max(view.left + 2, rect.left - 18))
  const top = Math.round(Math.max(view.top + 2, rect.top - 18))
  const previous = tableHandle.value
  if (previous && previous.left === left && previous.top === top) return
  tableHandle.value = { left, top }
}

/** 浮条上的「更多」：把整张右键菜单开到目标下面（同一个菜单，不另做一份）。 */
function openEditMenu() {
  const bar = editBar.value
  if (!bar) return
  const spot = { x: Math.round(bar.anchor.left), y: Math.round(bar.anchor.bottom + 4) }
  if (bar.target === 'image') {
    editorMenu.value = { ...spot, image: true }
  } else {
    const tools = tableTools.value
    if (!tools) return
    editorMenu.value = { ...spot, ...tools }
  }
  nextTick(clampEditorMenu)
  armEditorMenuClose()
}

/** 浮条上的「删除」：图片连着内联图片登记一起摘，表格整张删掉。 */
function removeEditTarget() {
  if (editBar.value?.target === 'image') removeSelectedImage()
  else tableBarAction('tableDelete')
  editBar.value = null
}

watch([selectedImage, tableTools, sourceMode], refreshEditBar)

// ---------------- 图片：拖边框改大小、拖动换位置 ----------------

/**
 * 套在选中图片外面的控制框（视口坐标）。图片是块级的，四个角都能拖：
 * 拖右边/右下角连动的都是宽度百分比，高度自动——发出去在别的客户端也不会变形。
 */
const imageFrame = ref(null)
/** 正在拖框改宽度：`{ startX, startWidth, boxWidth }`。 */
let resizing = null
/**
 * 正在拖动（或准备拖动）的整块：`{ kind, el, fromParent, pointerId, startX, startY, grabX, grabY, active }`。
 *
 * 图片与表格共用同一套拖动：按下时把指针捕获到被拖的块上，之后即使指针离开
 * 元素本身也仍然收得到 pointermove。以前把监听挂在 window 的 mousemove 上，
 * webview 里一旦手势被浏览器原生拖拽接管就彻底收不到，表现就是「拖了没反应」。
 */
let blockDrag = null
/** 松手后插到哪个顶层块前面；null 表示追加到末尾。 */
let dropRef = null
/** 指针还停在被拖块自己的段落里：纵向不动，只改水平位置。 */
let dropKeep = false
let dropValid = false
/** 拖动过程中画在落点上的那条线（视口坐标）。 */
const dropIndicator = ref(null)
const dropIndicatorStyle = computed(() =>
  dropIndicator.value
    ? {
        left: `${dropIndicator.value.left}px`,
        top: `${dropIndicator.value.y}px`,
        width: `${dropIndicator.value.width}px`,
      }
    : null,
)
/** 表格左上角的拖动把手（Outlook 同款位置）：`{ left, top }`。 */
const tableHandle = ref(null)

/** 跟着指针走的浮层（被拖块的副本）：只在拖动时存在。 */
let ghostEl = null
let ghostContentEl = null
let lastPointer = { x: 0, y: 0 }
let autoScrollFrame = null
const AUTO_SCROLL_EDGE = 56

/** 离被拖块最近的「正文直接子元素」：图片常被浏览器包在段落里，位置要按这层算。 */
function topLevelAncestor(el) {
  const editor = editorRef.value
  if (!editor || !el) return null
  let node = el
  while (node && node.parentElement !== editor) node = node.parentElement
  return node && node.parentElement === editor ? node : null
}

/**
 * 拖动时该滚谁。
 *
 * 弹层模式里滚的是 `.mail-compose-body`；阅读区里的内联回复卡片没有固定高度，
 * 真正滚动的是外层阅读窗格，所以这里往上找第一个「确实能滚」的祖先。
 */
function dragScroller() {
  const editor = editorRef.value
  if (!editor) return null
  let node = editor.parentElement
  while (node && node !== document.body && node !== document.documentElement) {
    const style = getComputedStyle(node)
    if (/(auto|scroll)/.test(style.overflowY) && node.scrollHeight > node.clientHeight + 1) {
      return node
    }
    node = node.parentElement
  }
  return document.scrollingElement || null
}


function refreshImageFrame() {
  const img = selectedImage.value
  const editor = editorRef.value
  if (!img || !editor?.contains(img) || sourceMode.value) {
    imageFrame.value = null
    return
  }
  const imageBox = img.getBoundingClientRect()
  // 可见区域是「滚动容器」而不是编辑区本身：编辑区的高度等于全文高度，
  // 拿它的矩形做裁剪的话，滚动时框照样会冒到工具栏上。
  const scroller = editor.closest?.('.mail-compose-body') || editor
  const viewBox = scroller.getBoundingClientRect()
  // 图片整个滚出可视区就不再画框：裁剪层之外也没什么可看的了。
  if (imageBox.bottom <= viewBox.top || imageBox.top >= viewBox.bottom) {
    imageFrame.value = null
    return
  }
  imageFrame.value = {
    // 裁剪层 = 滚动容器的可视矩形；框的坐标相对它算，滚出边界就被裁掉。
    layer: {
      left: viewBox.left,
      top: viewBox.top,
      width: viewBox.width,
      height: viewBox.height,
    },
    left: imageBox.left - viewBox.left,
    top: imageBox.top - viewBox.top,
    width: imageBox.width,
    height: imageBox.height,
  }
}

function onFrameMouseDown(event) {
  const img = selectedImage.value
  if (!img) return
  event.preventDefault()
  event.stopPropagation()
  resizing = {
    startX: event.clientX,
    startWidth: img.getBoundingClientRect().width,
    boxWidth: editorRef.value?.clientWidth || 1,
  }
  window.addEventListener('mousemove', onFrameMouseMove)
  window.addEventListener('mouseup', onFrameMouseUp, { once: true })
}

function onFrameMouseMove(event) {
  if (!resizing || !selectedImage.value) return
  const width = Math.max(24, resizing.startWidth + (event.clientX - resizing.startX))
  setImageWidth(Math.round((width / resizing.boxWidth) * 100), { commit: false })
  refreshImageFrame()
}

function onFrameMouseUp() {
  window.removeEventListener('mousemove', onFrameMouseMove)
  resizing = null
  commitImageWidth()
  refreshImageFrame()
}

/** 在正文里按下指针：图片上是「拖动换位置」，其余位置交给浏览器处理文字。 */
function onEditorPointerDown(event) {
  if (event.button !== 0) return
  const img = event.target?.closest?.('img')
  if (!img || !editorRef.value?.contains(img)) return
  // 拦掉默认行为（选文字 / 浏览器原生图片拖拽）。不拦的话这次手势会被浏览器
  // 接管，后续的 pointermove 就回不到我们手里。
  event.preventDefault()
  // 按下即选中：pointerdown 被拦掉后，兼容的 click 事件不一定还会派发，
  // 不能指望 onEditorClick 来点亮控制框。
  selectImage(img)
  beginBlockDrag('image', img, event)
}

/** 从控制框上的把手开始拖动：不依赖「正好按在图片上」。 */
function startFrameMove(event) {
  const img = selectedImage.value
  if (img) beginBlockDrag('image', img, event)
}

/** 浮条上的把手：图片 / 表格都用它开始拖动。 */
function startBarDrag(kind, event) {
  const el = kind === 'image' ? selectedImage.value : tableTools.value?.tableEl
  if (el) beginBlockDrag(kind, el, event)
}

/** 表格左上角的把手：按住它拖，就是 Outlook 里那个「挪表格」的手势。 */
function startTableHandleDrag(event) {
  const table = tableTools.value?.tableEl
  if (table) beginBlockDrag('table', table, event)
}

function beginBlockDrag(kind, el, event) {
  const editor = editorRef.value
  if (!el || !editor?.contains(el)) return
  event.preventDefault?.()
  const rect = el.getBoundingClientRect()
  const editorStyle = getComputedStyle(editor)
  const padLeft = parseFloat(editorStyle.paddingLeft) || 0
  const padRight = parseFloat(editorStyle.paddingRight) || 0
  const editorRect = editor.getBoundingClientRect()
  blockDrag = {
    kind,
    el,
    fromParent: el.parentElement,
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    startLeft: rect.left,
    startWidth: rect.width,
    // 正文可用的水平范围（缓存一次，拖动中每帧都要用）。
    available: Math.max(1, editorRect.width - padLeft - padRight),
    contentLeft: editorRect.left + padLeft,
    // 抓取点在块内的位置：浮层与松手后的水平位置都按它对齐，块不会「跳」到指针下。
    grabX: event.clientX - rect.left,
    grabY: event.clientY - rect.top,
    active: false,
  }
  // 指针捕获：拖出元素范围（拖到段落之间、正文空白处）也持续收到事件。
  try {
    el.setPointerCapture?.(event.pointerId)
  } catch {
    /* 不支持捕获的环境退回 window 上的监听 */
  }
  window.addEventListener('pointermove', onBlockDragMove)
  window.addEventListener('pointerup', onBlockDragEnd)
  window.addEventListener('pointercancel', onBlockDragCancel)
}

function onBlockDragMove(event) {
  if (!blockDrag) return
  lastPointer = { x: event.clientX, y: event.clientY }
  if (!blockDrag.active) {
    const moved = Math.hypot(event.clientX - blockDrag.startX, event.clientY - blockDrag.startY)
    if (moved < 4) return
    activateBlockDrag()
  }
  event.preventDefault()
  moveGhost()
  updateDropTarget(event.clientY)
}

function activateBlockDrag() {
  if (!blockDrag) return
  blockDrag.active = true
  document.body.classList.add('mail-block-dragging')
  // 原位留一个半透明的「占位」：用户能看出它原本在哪。
  blockDrag.el.setAttribute('data-drag-source', '')
  tableHandle.value = null
  createGhost()
  if (!autoScrollFrame) autoScrollFrame = requestAnimationFrame(runAutoScroll)
}

/** 克隆一份跟着指针走的浮层：表格 / 图片在拖动过程中就有实时反馈，不再是「松手才动」。 */
function createGhost() {
  const drag = blockDrag
  if (!drag) return
  const rect = drag.el.getBoundingClientRect()
  const wrap = document.createElement('div')
  wrap.className = 'mail-block-ghost'
  // 里面套一层编辑区类名：表格的格线、图片的块级排版都复用编译区同一套样式。
  const inner = document.createElement('div')
  inner.className = 'mail-compose-editor'
  inner.style.cssText =
    'padding:0;min-height:0;flex:0 0 auto;background:transparent;color:inherit;line-height:inherit'
  const clone = drag.el.cloneNode(true)
  clone.removeAttribute('data-drag-source')
  clone.style.margin = '0'
  clone.style.width = `${Math.round(rect.width)}px`
  clone.style.maxWidth = '100%'
  if (clone.tagName === 'IMG') clone.style.height = 'auto'
  inner.appendChild(clone)
  wrap.appendChild(inner)
  wrap.style.width = `${Math.round(rect.width)}px`
  ;(editorRef.value?.closest?.('.mail-composer') || document.body).appendChild(wrap)
  ghostEl = wrap
  ghostContentEl = clone
  moveGhost()
}

function moveGhost() {
  if (!ghostEl || !blockDrag) return
  const left = Math.round(lastPointer.x - blockDrag.grabX)
  ghostEl.style.left = `${left}px`
  ghostEl.style.top = `${Math.round(lastPointer.y - blockDrag.grabY)}px`
  // 整行宽的块往右拖时会收窄：浮层同步收，落点看起来才和结果一致。
  if (blockDrag.startWidth >= blockDrag.available - 1) {
    const offset = clamp(left - blockDrag.contentLeft, 0, blockDrag.available * 0.7)
    const width = Math.round(blockDrag.available - offset)
    ghostEl.style.width = `${width}px`
    if (ghostContentEl) ghostContentEl.style.width = `${width}px`
  }
}

function removeGhost() {
  ghostEl?.remove()
  ghostEl = null
  ghostContentEl = null
}

/**
 * 拖到正文上下边缘时自动滚动。
 *
 * 以前只能把内容拖到当前这一屏里，想挪到上面 / 下面几段得先松手、滚动、再拖一次。
 * 指针停在边缘就一直滚，并且每滚一帧都重算一次落点（内容和指针的相对位置变了）。
 */
function runAutoScroll() {
  autoScrollFrame = null
  if (!blockDrag?.active) return
  const scroller = dragScroller()
  if (scroller) {
    const box = scroller.getBoundingClientRect()
    let delta = 0
    if (lastPointer.y < box.top + AUTO_SCROLL_EDGE) {
      delta = -Math.max(4, Math.round((box.top + AUTO_SCROLL_EDGE - lastPointer.y) / 3))
    } else if (lastPointer.y > box.bottom - AUTO_SCROLL_EDGE) {
      delta = Math.max(4, Math.round((lastPointer.y - (box.bottom - AUTO_SCROLL_EDGE)) / 3))
    }
    if (delta) {
      const before = scroller.scrollTop
      scroller.scrollTop = before + delta
      if (scroller.scrollTop !== before) updateDropTarget(lastPointer.y)
    }
  }
  autoScrollFrame = requestAnimationFrame(runAutoScroll)
}

/**
 * 松手后块插到哪：
 * - 指针还在块自己所在的那一段里 → 纵向不动（只调左右位置），否则图片会被从
 *   所在段落里硬拽出来，稍微一晃就换个段落，很难用；
 * - 指针已经离开这一段 → 找第一个「中线在指针下方」的顶层块，插到它前面。
 */
function updateDropTarget(clientY) {
  const editor = editorRef.value
  const target = blockDrag?.el
  if (!editor || !target) return
  const box = editor.getBoundingClientRect()
  dropValid = clientY >= box.top - 80 && clientY <= box.bottom + 80
  if (!dropValid) {
    dropRef = null
    dropKeep = false
    dropIndicator.value = null
    return
  }
  const anchor = topLevelAncestor(target)
  dropKeep = false
  if (anchor) {
    const rect = anchor.getBoundingClientRect()
    // 上下各放宽一点：拖动把手本身就长在块的边角上，按在把手上时指针并不在块内。
    dropKeep = clientY >= rect.top - 24 && clientY <= rect.bottom + 24
  }
  if (dropKeep) {
    dropRef = anchor
    dropIndicator.value = null
    return
  }
  let ref = null
  for (const child of editor.children) {
    const rect = child.getBoundingClientRect()
    if (clientY < rect.top + rect.height / 2) {
      ref = child
      break
    }
  }
  const last = editor.lastElementChild
  let y = ref ? ref.getBoundingClientRect().top : last ? last.getBoundingClientRect().bottom : box.top + 8
  // 落点滚到可视区外时，线贴在滚动容器的边缘，不画到工具栏上。
  const scroller = dragScroller()
  if (scroller) {
    const view = scroller.getBoundingClientRect()
    y = clamp(y, view.top + 2, view.bottom - 2)
  }
  dropRef = ref
  dropIndicator.value = {
    left: Math.round(box.left),
    width: Math.round(box.width),
    y: Math.round(y),
  }
}

function onBlockDragEnd() {
  const drag = blockDrag
  if (!drag) return
  const active = drag.active
  const ref = dropRef
  const keep = dropKeep
  const valid = dropValid
  const pointer = lastPointer
  finishBlockDrag()
  if (!active || !valid) return
  const editor = editorRef.value
  if (!editor?.contains(drag.el)) return
  if (!keep) {
    if (ref && ref !== drag.el) ref.insertAdjacentElement('beforebegin', drag.el)
    else if (!ref) editor.appendChild(drag.el)
  }
  // 左右：把块的左缘对到浮层左缘，写成 margin-left（收件端同样有效）。
  applyHorizontalDrop(drag.el, drag, pointer.x)
  cleanupEmptyWrapper(drag.fromParent)
  // 换完位置重新选中它：控制框 / 浮条跟着走，而不是停在旧地方。
  if (drag.kind === 'image') selectImage(drag.el)
  onEditorInput()
  refreshImageFrame()
  refreshEditBar()
  if (drag.kind === 'table') nextTick(refreshTableTools)
  refreshFormatState()
}

function onBlockDragCancel() {
  finishBlockDrag()
}

function finishBlockDrag() {
  window.removeEventListener('pointermove', onBlockDragMove)
  window.removeEventListener('pointerup', onBlockDragEnd)
  window.removeEventListener('pointercancel', onBlockDragCancel)
  if (blockDrag) {
    try {
      blockDrag.el.releasePointerCapture?.(blockDrag.pointerId)
    } catch {
      /* 捕获可能已经随 pointerup 自动释放 */
    }
    blockDrag.el.removeAttribute('data-drag-source')
  }
  if (autoScrollFrame) {
    cancelAnimationFrame(autoScrollFrame)
    autoScrollFrame = null
  }
  removeGhost()
  blockDrag = null
  dropRef = null
  dropKeep = false
  dropValid = false
  dropIndicator.value = null
  document.body.classList.remove('mail-block-dragging')
}

/**
 * 水平位置：把块的左缘对上松手时的位置。
 *
 * 邮件正文没有「自由画布」，能表达的水平位移只有缩进（margin-left）。块比正文窄
 * 时左右挪就是改缩进；整行宽的块没有空位，往右拖时把右缘钉住、宽度按百分比收窄
 * ——否则表格会比正文还宽，收件端直接横向溢出。
 *
 * 只是上下拖、手指抖了几像素的时候不动外边距：否则居中 / 靠右的图片会被顺手
 * 改成靠左（真实反馈里最难解释的一类「我没让它动」）。
 */
function applyHorizontalDrop(el, drag, pointerX) {
  if (!editorRef.value) return
  const available = drag.available
  const contentLeft = drag.contentLeft
  const startWidth = Math.min(drag.startWidth, available)
  const fullWidth = startWidth >= available - 1
  // 整行宽的块允许收窄到 30%（再窄就不像一张表了）；本来就窄的只挪不缩。
  const maxOffset = fullWidth
    ? Math.max(0, available - Math.max(96, available * 0.3))
    : Math.max(0, available - startWidth)
  const offset = clamp(pointerX - drag.grabX - contentLeft, 0, maxOffset)
  const startOffset = clamp(drag.startLeft - contentLeft, 0, maxOffset)
  if (Math.abs(offset - startOffset) < 16) return
  if (fullWidth && offset > 0) {
    // 百分比宽度收件端也认；px 宽度在窄窗口里会把正文顶横。
    el.style.width = `${Math.round(((available - offset) / available) * 100)}%`
  }
  el.style.marginLeft = offset ? `${Math.round(offset)}px` : ''
  el.style.marginRight = '0'
  if (/^TABLE$/i.test(el.tagName)) el.setAttribute('align', 'left')
}

/** 图片被从段落里拖出来后，别在正文里留下一个空段落。 */
function cleanupEmptyWrapper(parent) {
  const editor = editorRef.value
  if (!parent || !editor || parent === editor || !parent.isConnected) return
  if (parent.querySelector?.('img, table, hr, video, audio, iframe')) return
  if ((parent.textContent || '').trim()) return
  parent.remove()
}

/** 整块上移 / 下移一格：拖拽之外的确定性入口（键盘 / 触控板也顺手）。 */
function moveBlockStep(target, delta) {
  const editor = editorRef.value
  const anchor = topLevelAncestor(target)
  if (!editor || !target || !anchor) return false
  const fromParent = target.parentElement
  if (delta > 0) {
    const next = anchor.nextElementSibling
    if (next) next.insertAdjacentElement('afterend', target)
    // 已经在最后一个块里：至少把它从所在段落里挪到段落之后。
    else if (anchor !== target) anchor.insertAdjacentElement('afterend', target)
    else return false
  } else {
    const prev = anchor.previousElementSibling
    if (prev) prev.insertAdjacentElement('beforebegin', target)
    else if (anchor !== target) anchor.insertAdjacentElement('beforebegin', target)
    else return false
  }
  cleanupEmptyWrapper(fromParent)
  return true
}

function moveImageStep(delta) {
  if (!moveBlockStep(selectedImage.value, delta)) return
  refreshImageFrame()
  refreshEditBar()
  onEditorInput()
}

function moveTableStep(delta) {
  if (!moveBlockStep(tableTools.value?.tableEl, delta)) return
  refreshEditBar()
  nextTick(refreshTableTools)
  onEditorInput()
}

// 浮条要跟着目标走：正文滚动、窗口改尺寸都得重算位置（一帧最多算一次）。
let barReflowQueued = false
function onEditBarReflow() {
  // 以前只在浮条可见时重算：只选中图片（没浮条）时滚动，控制框会停在原地、
  // 甚至叠到工具栏上。控制框自己也要跟着滚。
  if (barReflowQueued) return
  barReflowQueued = true
  requestAnimationFrame(() => {
    barReflowQueued = false
    if (editBar.value) refreshEditBar()
    else refreshImageFrame()
  })
}
onMounted(() => {
  document.addEventListener('scroll', onEditBarReflow, true)
  window.addEventListener('resize', onEditBarReflow)
})
onBeforeUnmount(() => {
  document.removeEventListener('scroll', onEditBarReflow, true)
  window.removeEventListener('resize', onEditBarReflow)
  finishBlockDrag()
})

// ---------------- 编辑器右键菜单 ----------------

/** 单元格可选的填充色 / 文字色 / 字号（右键菜单里的快捷项）。 */
const CELL_FONT_SIZES = [11, 12, 13, 14, 16]
/** 取色器的初始值：菜单里只放一个色块，具体颜色由系统取色器给。 */
const DEFAULT_CELL_FILL = '#ddebf7'
const DEFAULT_TEXT_COLOR = '#44546a'

/**
 * 单元格的填充 / 文字颜色与工具栏共用同一份色板（见 palette.js），
 * 以前两处各有一套，同一个红色在两处对不上。
 */
/** 展开的是哪一组颜色：'' / 'fill' / 'text'。 */
const colorPanel = ref('')

function toggleColorPanel(kind) {
  colorPanel.value = colorPanel.value === kind ? '' : kind
}

/** 当前打开着的右键菜单：`{ x, y, table, image, cell, tableEl, row, col, rows, cols }`。 */
const editorMenu = ref(null)
const editorMenuRef = ref(null)

/**
 * 右键菜单的定位。
 *
 * 菜单是 fixed 定位，坐标直接用视口坐标（= 右键处的 clientX/Y），所以不再需要
 * 把编辑区的滚动量换算进去——以前它是 `.mail-compose-body` 里的绝对定位元素，
 * 一旦菜单比编辑区剩余空间高就得开滚动条，或者被容器裁掉半截。
 * 现在改成渲染完量一次真实尺寸，再夹进视口：放不下就翻到光标另一侧。
 */
function clampEditorMenu() {
  const el = editorMenuRef.value
  const menu = editorMenu.value
  if (!el || !menu) return
  const box = el.getBoundingClientRect()
  const x = clamp(menu.x, 8, Math.max(8, window.innerWidth - box.width - 8))
  let y = menu.y
  if (y + box.height > window.innerHeight - 8) {
    // 下面放不下就往上翻；连上面也放不下，就贴着视口底边（此时两栏排版已经保证放得下）。
    y = Math.max(8, window.innerHeight - box.height - 8)
  }
  if (x === menu.x && Math.round(y) === Math.round(menu.y)) return
  editorMenu.value = { ...menu, x: Math.round(x), y: Math.round(y) }
}

function armEditorMenuClose() {
  setTimeout(() => document.addEventListener('mousedown', closeEditorMenu, { once: true }), 0)
}

function onEditorContextMenu(event) {
  const editor = editorRef.value
  if (!editor) return
  // 表格与图片的右键由我们接管（行列 / 宽度操作）；其余位置也接管，保证菜单跟着光标。
  event.preventDefault()
  const spot = { x: event.clientX, y: event.clientY }
  colorPanel.value = ''
  const cell = event.target?.closest?.('td, th')
  const table = event.target?.closest?.('table')
  const img = event.target?.closest?.('img')
  if (table && cell && editor.contains(table) && editor.contains(cell)) {
    // 记住光标在表格里的位置：后续操作用它定位，而不是依赖「当前选区」——
    // 右键之后浏览器可能把选区挪到别处。
    const pos = cellPosition(table, cell)
    tableTools.value = pos ? { table: true, cell, tableEl: table, ...pos } : null
    editorMenu.value = { ...spot, table: true, cell, tableEl: table, ...pos }
  } else if (img && editor.contains(img)) {
    selectImage(img)
    editorMenu.value = { ...spot, image: true }
  } else {
    editorMenu.value = { ...spot, table: false }
  }
  nextTick(clampEditorMenu)
  armEditorMenuClose()
}

function closeEditorMenu() {
  editorMenu.value = null
  colorPanel.value = ''
  document.removeEventListener('mousedown', closeEditorMenu)
}

/** 图片右键菜单里的动作：与浮动条共用同一套实现，做完收起菜单。 */
function imageMenuAction(kind, payload) {
  if (kind === 'width') setImageWidth(payload)
  else if (kind === 'align') alignImage(payload)
  else removeSelectedImage()
  closeEditorMenu()
}

/** 单元格字号下拉：选中即生效（tableAction 自己会收起菜单）。 */
function onCellFontSize(event) {
  const size = Number(event?.target?.value) || 0
  if (size) tableAction('cellFontSize', size)
}

function onTableStyle(event) {
  const key = event?.target?.value
  if (key) tableAction('style', key)
}

function onTableWidth(event) {
  const value = event?.target?.value
  if (value) tableAction('tableWidth', value)
}

function onCellFillColor(event) {
  const color = event?.target?.value
  if (color) tableAction('cellFill', color)
}

function onCellTextColor(event) {
  const color = event?.target?.value
  if (color) tableAction('cellColor', color)
}

/** 普通编辑命令（剪切 / 复制 / 粘贴 / 全选）。 */
function editorCommand(command) {
  closeEditorMenu()
  editorRef.value?.focus()
  try {
    document.execCommand(command, false, null)
  } catch (error) {
    logger.warn(`编辑器命令 ${command} 失败`, error)
  }
}

/**
 * 表格操作。所有改动都直接操作 DOM 再同步回草稿——
 * `execCommand` 没有行列增删能力，只能自己拆/拼节点。
 *
 * `context` 由右键菜单或工具栏给出：`{ tableEl, cell, row, col }`。
 * 两处共用同一套实现，避免「右键能做的事工具栏做不了」这种分裂。
 */
function tableAction(action, payload, context = null) {
  const menu = context || editorMenu.value
  closeEditorMenu()
  if (!menu?.tableEl) return
  const table = menu.tableEl
  const rows = collectRows(table)

  const rebuildRow = (source, tag) => {
    const row = document.createElement('tr')
    const cols = Math.max(...rows.map((r) => r.cells.length), 1)
    for (let i = 0; i < cols; i += 1) {
      row.insertAdjacentHTML('beforeend', createCellHtml(tag))
    }
    // 复制被点击那一行的对齐方式，新行看起来才和周围一致
    const template = source?.cells?.[0]
    if (template?.style?.textAlign) {
      for (const cell of row.children) cell.style.textAlign = template.style.textAlign
    }
    return row
  }

  switch (action) {
    case 'rowAbove':
    case 'rowBelow': {
      const anchor = table.rows[menu.row]
      if (!anchor) return
      const targetIndex = insertionRowAfterInsert(rows, menu.row, action === 'rowAbove' ? 'above' : 'below')
      const tag = rows[menu.row]?.isHeader ? 'td' : 'td'
      const newRow = rebuildRow(anchor, tag)
      const reference = table.rows[targetIndex] || null
      anchor.parentNode.insertBefore(newRow, action === 'rowAbove' && targetIndex === menu.row ? anchor : reference)
      break
    }
    case 'rowDelete': {
      const row = table.rows[menu.row]
      // 最后一行不允许删：留下一张空表格比删掉整个表格更让人困惑
      if (!row || table.rows.length <= 1) return
      row.remove()
      break
    }
    case 'colLeft':
    case 'colRight': {
      const insertAt = action === 'colLeft' ? menu.col : menu.col + 1
      for (const row of [...table.rows]) {
        const cells = [...row.children].filter((el) => /^(TD|TH)$/i.test(el.tagName))
        const tag = cells[0]?.tagName?.toLowerCase() === 'th' ? 'th' : 'td'
        const cell = document.createElement(tag)
        cell.innerHTML = '<br>'
        const reference = cells[insertAt] || null
        row.insertBefore(cell, reference)
      }
      break
    }
    case 'colDelete': {
      // 最后一列不允许删，理由同上
      if ((rows[menu.col]?.cells?.length || 0) <= 1 || (rows[0]?.cells?.length || 0) <= 1) return
      for (const row of [...table.rows]) {
        const cells = [...row.children].filter((el) => /^(TD|TH)$/i.test(el.tagName))
        cells[menu.col]?.remove()
      }
      break
    }
    case 'style': {
      const preset = TABLE_STYLES.find((item) => item.key === payload)
      if (!preset) return
      applyTableStyle(table, preset)
      break
    }
    case 'tableWidth': {
      // 自动布局的表格在收件端最稳：宽度按百分比给，别写死像素。
      table.style.width = payload === 'auto' ? 'auto' : `${Number(payload) || 100}%`
      table.removeAttribute('width')
      break
    }
    case 'tableAlign': {
      // 居中 / 靠右靠外边距推开；align 属性是给老客户端（Outlook）的兜底。
      table.style.marginLeft = payload === 'right' || payload === 'center' ? 'auto' : '0'
      table.style.marginRight = payload === 'left' || payload === 'center' ? 'auto' : '0'
      table.setAttribute('align', payload)
      break
    }
    case 'cellAlignLeft':
    case 'cellAlignCenter':
    case 'cellAlignRight': {
      const value = action.replace('cellAlign', '').toLowerCase()
      const cell = menu.cell
      if (cell) cell.style.textAlign = value
      break
    }
    case 'cellFill': {
      const cell = menu.cell
      if (!cell) return
      if (payload) {
        // bgcolor 是 Outlook 唯一稳定支持的底色写法；style 只是给别的客户端兜底。
        cell.setAttribute('bgcolor', payload)
        cell.style.backgroundColor = payload
      } else {
        cell.removeAttribute('bgcolor')
        cell.style.backgroundColor = ''
      }
      break
    }
    case 'cellColor': {
      const cell = menu.cell
      if (!cell) return
      // 写成 font 标签包一层，比 style 在更多老客户端里生效。
      cell.style.color = payload
      break
    }
    case 'cellFontSize': {
      const cell = menu.cell
      if (!cell) return
      cell.style.fontSize = `${payload}px`
      break
    }
    case 'cellBold': {
      const cell = menu.cell
      if (!cell) return
      cell.style.fontWeight = cell.style.fontWeight === 'bold' ? '' : 'bold'
      break
    }
    case 'tableDelete': {
      table.remove()
      break
    }
    default:
      return
  }
  onEditorInput()
}

/**
 * 套用表格样式。
 *
 * 只用**邮件客户端普遍认的写法**：边框走 `style`（各客户端都支持），
 * 表头底色走 `bgcolor` 表现属性（Outlook 的 Word 引擎只认这个）。
 */
function applyTableStyle(table, preset) {
  const border = preset.borderStyle || 'none'
  table.setAttribute('border', border === 'none' ? '0' : '1')
  table.setAttribute('cellpadding', '6')
  table.setAttribute('cellspacing', '0')
  table.style.borderCollapse = 'collapse'
  for (const cell of table.querySelectorAll('td, th')) {
    cell.style.border = border
  }
  // 表头底色
  for (const th of table.querySelectorAll('th')) {
    if (preset.headerBg) th.setAttribute('bgcolor', preset.headerBg)
    else th.removeAttribute('bgcolor')
    th.style.color = preset.key === 'header' ? '#ffffff' : ''
  }
  const headRow = table.querySelector('thead tr') || table.rows[0]
  if (headRow) {
    if (preset.headerBg) headRow.setAttribute('bgcolor', preset.headerBg)
    else headRow.removeAttribute('bgcolor')
  }
  // 隔行变色：偶数行加一个很浅的底色（写 bgcolor，客户端才认）
  for (const [index, row] of [...table.rows].entries()) {
    if (preset.zebra && index > 0 && index % 2 === 1) row.setAttribute('bgcolor', '#f9f9f9')
    else if (preset.zebra) row.removeAttribute('bgcolor')
  }
}

function onEditorInput() {
  const html = serializeEditorHtml()
  if (html === lastHtml) return
  lastHtml = html
  emit('update', { html, text: htmlToText(html) })
}

/**
 * 引用历史「按当前主题」重上色。
 *
 * 引用原文是发件人按浅色纸面写的（白色卡片 + 深色文字），直接塞进深色编辑器里
 * 就是一大块白底浅字——这就是新窗口里的引用看起来和阅读区完全不一样的原因。
 * 阅读区靠 colors.js 逐元素映射解决同一件事，这里对引用卡片做同样的映射；
 * 序列化草稿时再还原成原值，收件端看到的仍会是他发来的那份配色。
 */
function quoteThemeTargets(root) {
  if (!root?.querySelectorAll) return []
  const cards = [...root.querySelectorAll('[data-guaxi-quote-card]')]
  const out = []
  for (const card of cards) out.push(card, ...card.querySelectorAll('*'))
  return out
}

let quoteThemeQueued = false

/**
 * 合并这一轮的多处调用（输入、粘贴、切主题都可能同时触发），
 * 每帧最多跑一次——映射要逐元素读计算样式，没有必要每次按键都跑两遍。
 */
function scheduleQuoteTheme() {
  if (quoteThemeQueued) return
  quoteThemeQueued = true
  requestAnimationFrame(() => {
    quoteThemeQueued = false
    const targets = quoteThemeTargets(editorRef.value)
    if (!targets.length) return
    applyBodyThemeToElements(targets, props.themeMode === 'light' ? 'light' : 'dark')
  })
}

/**
 * 编辑器里的 `cid:` 图片没法直接显示：用 asset URL 映射到本地临时文件，
 * 并把原始 cid 记在 `data-cid` 上；序列化时再换回 `cid:`（发信/存草稿要用）。
 */
function resolveCidImages() {  if (!editorRef.value) return
  const images = props.draft.inlineImages || []
  // 没有 inlineImages 也要跑一遍：老草稿的正文里可能还留着 cid:，
  // 早退的话那张图会一直停在加载失败的状态。
  const map = new Map(
    images.map((img) => [String(img.contentId || '').replace(/^<|>$/g, ''), img.path]),
  )
  editorRef.value.querySelectorAll('img').forEach((img) => {
    const src = img.getAttribute('src') || ''
    if (!/^cid:/i.test(src)) return
    const cid = decodeURIComponent(src.slice(4)).replace(/^<|>$/g, '')
    img.dataset.cid = cid
    const path = map.get(cid)
    if (path) {
      img.src = convertFileSrc(path)
      return
    }
    // 找不到落盘文件（草稿被清理 / 附件被删 / 父级还没同步）：不要让它停在 cid: 上，
    // 浏览器会一直尝试加载并显示成一块空白。换成 1×1 透明图占位，
    // data-cid 保留着，序列化成草稿时仍然是正确的内容。
    img.src = MISSING_INLINE_PIXEL
    img.classList.add('is-missing-inline')
  })
}

/** 找不到本地文件的 inline 图占位：1×1 透明 GIF。 */
const MISSING_INLINE_PIXEL =
  'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'

/** 生成用于草稿/发送的 HTML：把显示用的 asset URL 换回 `cid:`。 */
function serializeEditorHtml() {
  if (!editorRef.value) return ''
  // 用 <template> 装副本：模板内容是惰性的，往里写 `cid:` 不会触发任何加载。
  // 以前直接克隆成游离节点，每 30 秒自动保存都会真的去 GET 一遍 cid:，
  // 控制台刷满 ERR_UNKNOWN_URL_SCHEME。
  const host = document.createElement('template')
  host.innerHTML = editorRef.value.innerHTML
  const clone = host.content
  // 拖动中的临时标记（半透明的占位样式）不能写进草稿 / 发出去。
  clone.querySelectorAll('[data-drag-source]').forEach((el) => el.removeAttribute('data-drag-source'))
  // 引用卡片的配色映射只为本机编辑区服务（见 scheduleQuoteTheme）：
  // 交给草稿 / 发送之前先还原成邮件原值，不能把深色主题的映射结果发出去。
  const themed = quoteThemeTargets(clone)
  if (themed.length) applyBodyThemeToElements(themed, 'light')
  clone.querySelectorAll('img[data-cid]').forEach((img) => {
    img.setAttribute('src', `cid:${img.dataset.cid}`)
    img.removeAttribute('data-cid')
  })
  return host.innerHTML
}

function format(command, value = null) {
  if (sourceMode.value) return
  restoreSelection()
  try {
    // styleWithCSS 让 execCommand 输出 <span style>，而不是 <font> / <b>：
    // 邮件客户端对行内样式支持最好，也能和主题 / 粘贴清洗保持一致。
    document.execCommand('styleWithCSS', false, true)
    document.execCommand(command, false, value)
  } catch (error) {
    logger.warn('格式命令失败', command, error)
  }
  savedRange = currentRange()
  onEditorInput()
  refreshFormatState()
}

/** 去掉突出显示。 */
function clearHilite() {
  closePalette()
  restoreSelection()
  try {
    document.execCommand('styleWithCSS', false, true)
    document.execCommand('hiliteColor', false, 'transparent')
  } catch (error) {
    logger.warn('清除高亮失败', error)
  }
  onEditorInput()
}

function onFontFamily(event) {
  const family = event.target.value
  // 不清空下拉：字体是「当前状态」而不是一次性动作，清空后用户看不到自己选了什么。
  if (family) format('fontName', family)
  formatState.fontFamily = family
}

function onFontSize(event) {
  const size = Number(event.target.value)
  event.target.value = ''
  if (size) applyFontSize(size)
}

/** execCommand('fontSize') 只支持 1-7 档；这里用 span 包选区，能精确到 px。 */
function applyFontSize(size) {
  if (sourceMode.value) return
  const selection = restoreSelection()
  if (!selection || !selection.rangeCount) return
  const range = selection.getRangeAt(0)
  if (range.collapsed) return
  const span = document.createElement('span')
  span.style.fontSize = `${size}px`
  span.appendChild(range.extractContents())
  range.insertNode(span)
  const next = document.createRange()
  next.selectNodeContents(span)
  selection.removeAllRanges()
  selection.addRange(next)
  savedRange = next.cloneRange()
  onEditorInput()
  refreshFormatState()
}

/** 清除文字颜色（回到正文默认色）。 */
function clearTextColor() {
  closePalette()
  restoreSelection()
  format('removeFormat')
}

/** 插入表格：直接写成真 <table>，任何客户端都能显示。 */
function insertTable(rows = tableRows.value, cols = tableCols.value) {
  closePalette()
  const editor = editorRef.value
  const tableCount = editor ? editor.querySelectorAll('table').length : 0
  insertHtmlAt(currentRange() || null, buildTableHtml({ rows, cols }))
  onEditorInput()
  // 插完把光标放进新表的第一个单元格（并选中占位文字）：
  // 接着就能直接打字，表格工具行也会立刻出现——不用先自己点进表格。
  const tables = editor ? editor.querySelectorAll('table') : []
  const table = tables[Math.min(tableCount, tables.length - 1)]
  const cell = table?.querySelector('th, td')
  if (!cell) return
  const selection = window.getSelection()
  const range = document.createRange()
  range.selectNodeContents(cell)
  selection?.removeAllRanges()
  selection?.addRange(range)
  editorRef.value?.focus()
}

/**
 * 把正文里的 Markdown 转成富文本。
 *
 * 只在「源码模式」或正文本身像 Markdown 时才做——用户直接用工具栏排版时，
 * 正文已经是 HTML 了，再跑一遍转换只会破坏排版。
 */
function convertMarkdown() {
  closePalette()
  const editor = editorRef.value
  if (!editor) return
  const source = editor.innerText || ''
  if (!source.trim()) return
  const looksLikeMarkdown =
    /(^|\n)\s*(#{1,6}\s|[-*+]\s|\d+[.)]\s|>|```)/.test(source) ||
    /\|.*\|/.test(source) ||
    /\*\*[^*]+\*\*/.test(source)
  if (!looksLikeMarkdown) {
    window.alert('正文里没有检测到 Markdown 语法（标题、列表、粗体、表格等）。')
    return
  }
  const html = markdownToEmailHtml(source)
  editor.innerHTML = html
  onEditorInput()
}

/** 应用突出显示颜色。 */
function applyHiliteColor(color) {
  closePalette()
  if (!color) return
  lastHiliteColor.value = color
  recentHiliteColors.value = pushRecent(recentHiliteColors.value, color)
  restoreSelection()
  try {
    document.execCommand('styleWithCSS', false, true)
    // Chromium/WebView2 支持 hiliteColor；个别环境只认 backColor。
    if (!document.execCommand('hiliteColor', false, color)) {
      document.execCommand('backColor', false, color)
    }
  } catch (error) {
    logger.warn('设置高亮失败', error)
  }
  savedRange = currentRange()
  onEditorInput()
  refreshFormatState()
}

function insertLink() {
  rememberSelection()
  const url = window.prompt('链接地址', 'https://')
  if (!url) return
  format('createLink', url)
}

function insertCode() {
  rememberSelection()
  const code = window.prompt('代码内容')
  if (code == null) return
  const selection = restoreSelection()
  if (!selection || !selection.rangeCount) return
  const pre = document.createElement('pre')
  pre.textContent = code
  const range = selection.getRangeAt(0)
  range.deleteContents()
  range.insertNode(pre)
  const next = document.createRange()
  next.setStartAfter(pre)
  next.collapse(true)
  selection.removeAllRanges()
  selection.addRange(next)
  savedRange = next.cloneRange()
  onEditorInput()
  refreshFormatState()
}

function toggleSource() {
  sourceMode.value = !sourceMode.value
}

/** 粘贴时只保留纯文本与基本行内格式：外部网页的样式会毁掉整封邮件的排版。 */
function onPaste(event) {
  // 剪贴板里的图片：优先作为内联图（截图贴进正文是最常见的用法）。
  // 有些环境只填 clipboardData.files、有些只填 items，两种都取。
  const imageItems = Array.from(event.clipboardData?.items || []).filter(
    (item) => item.kind === 'file' && String(item.type || '').startsWith('image/'),
  )
  const files = imageItems.length
    ? imageItems.map((item) => item.getAsFile()).filter(Boolean)
    : Array.from(event.clipboardData?.files || []).filter((file) =>
        String(file.type || '').startsWith('image/'),
      )
  if (files.length) {
    event.preventDefault()
    // 异步读文件期间编辑器可能失焦：先记住光标位置，读完再插回去。
    const range = currentRange()
    Promise.all(files.map(readFileAsDataUrl)).then(async (urls) => {
      const added = []
      const parts = []
      for (let index = 0; index < urls.length; index += 1) {
        const url = urls[index]
        if (!url) continue
        const inline = await materializePastedImage(url, index).catch((error) => {
          logger.warn('粘贴图片落盘失败，保留 data URL', error)
          return null
        })
        if (inline) {
          added.push(inline)
          // 直接写解析好的本机地址，不要再先塞 `cid:` 再等下一帧替换：
          // 父级的 inlineImages 更新是异步的，那一刻常常还查不到映射，
          // 于是 cid: 留在正文里变成一张「永远加载不出来」的图——既刷控制台
          // （ERR_UNKNOWN_URL_SCHEME），又在深色正文里占出一大块空白。
          // data-cid 仍然保留，序列化成草稿时会换回 cid。
          parts.push(
            `<img data-guaxi-img="1" src="${convertFileSrc(inline.path)}" data-cid="${inline.contentId}" alt="粘贴的图片" />`,
          )
        } else {
          parts.push(`<img data-guaxi-img="1" src="${url}" alt="粘贴的图片" />`)
        }
      }
      if (added.length) {
        emit('update', { inlineImages: [...(props.draft.inlineImages || []), ...added] })
      }
      insertHtmlAt(range, parts.join(''))
      onEditorInput()
      nextTick(() => {
        resolveCidImages()
        // 图片异步落盘 / 换 src 之后选区可能又被撑开，这里统一收回末尾。
        collapseSelectionToEnd()
      })
    })
    return
  }
  const html = event.clipboardData?.getData('text/html')
  if (html) {
    event.preventDefault()
    const cleaned = sanitizePastedHtml(html)
    document.execCommand('insertHTML', false, cleaned)
    // 有些浏览器会把刚插入的表格/图片整块选中，留下一条蓝色高亮。
    collapseSelectionToEnd()
    onEditorInput()
    return
  }
  const text = event.clipboardData?.getData('text/plain')
  if (text != null) {
    event.preventDefault()
    document.execCommand('insertText', false, text)
    collapseSelectionToEnd()
    onEditorInput()
  }
}

function currentRange() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return null
  return selection.getRangeAt(0).cloneRange()
}

function collapseSelectionToEnd() {
  const selection = window.getSelection()
  if (!selection || !selection.rangeCount) return
  const range = selection.getRangeAt(0)
  range.collapse(false)
  selection.removeAllRanges()
  selection.addRange(range)
}

function readFileAsDataUrl(file) {
  return new Promise((resolve) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => resolve('')
    reader.readAsDataURL(file)
  })
}

/**
 * 粘贴的图片先落到系统临时目录，再以内联附件（cid）插入正文：
 * 直接发 data URL，多数邮箱（Gmail/Outlook）会把图片当不安全内容剥掉。
 */
async function materializePastedImage(dataUrl, index) {
  const match = dataUrl.match(/^data:image\/([a-zA-Z0-9.+-]+);base64,([\s\S]+)$/)
  if (!match) return null
  const extension =
    match[1].toLowerCase() === 'jpeg' ? 'jpg' : match[1].replace(/[^a-z0-9]/gi, '') || 'png'
  const [{ writeFile, mkdir }, { appDataDir, join }] = await Promise.all([
    import('@tauri-apps/plugin-fs'),
    import('@tauri-apps/api/path'),
  ])
  // 放应用数据目录而不是系统临时目录：草稿可能留几天，TEMP 被清理会让图片变破图。
  const dir = await join(await appDataDir(), 'mail-inline')
  await mkdir(dir, { recursive: true }).catch(() => {})
  const contentId = `guaxi-${Date.now().toString(36)}-${index}-${Math.random().toString(36).slice(2, 8)}@guaxi`
  const name = `粘贴图片-${Date.now()}-${index}.${extension}`
  const path = await join(dir, name)
  const binary = atob(match[2].replace(/\s/g, ''))
  const bytes = Uint8Array.from(binary, (ch) => ch.charCodeAt(0))
  await writeFile(path, bytes)
  return { contentId, path, name }
}

/**
 * 把 HTML 插到记下的光标处。
 *
 * 末尾的 `<br>` 是必需的：图片是块级元素，后面没有换行时光标只能停在图片本身，
 * 接着打字会落进图片节点里。但 `<br>` 之后还要把选区折叠到编辑区的最末，
 * 否则 WebView 会把「刚插入的整段」保持在选中态——用户看到的就是图片下面
 * 那条蓝色高亮线（选中区域的边界）。
 */
function insertHtmlAt(range, html) {
  if (!html) return
  const selection = window.getSelection()
  if (range && selection) {
    selection.removeAllRanges()
    selection.addRange(range)
  }
  document.execCommand('insertHTML', false, `${html}<br>`)
  collapseSelectionToEnd()
  // 再折叠一次：execCommand 之后 WebView 有时会把插入内容重新选上（异步的），
  // 下一帧再清一次才能稳定落成「光标在末尾、没有选区」。
  nextTick(() => {
    collapseSelectionToEnd()
    editorRef.value?.focus()
  })
}

/** 只保留白色名单里的标签；class/id/style 全部丢弃。 */
function sanitizePastedHtml(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const allowed = new Set([
    'B', 'STRONG', 'I', 'EM', 'U', 'S', 'P', 'BR', 'DIV', 'SPAN', 'A', 'UL', 'OL', 'LI',
    'BLOCKQUOTE', 'PRE', 'CODE', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'TABLE', 'THEAD',
    'TBODY', 'TR', 'TD', 'TH', 'HR', 'IMG',
  ])
  const walk = (node) => {
    for (const child of [...node.childNodes]) {
      if (child.nodeType === 1) {
        if (!allowed.has(child.tagName)) {
          // 不认识的容器：把子节点提上来，保留文字。
          while (child.firstChild) node.insertBefore(child.firstChild, child)
          node.removeChild(child)
          continue
        }
        // 从别的邮件里复制来的内嵌图（src="cid:..."）：图片数据带不过来，
        // 留着只会变成一张永远加载失败的坏图——控制台刷 ERR_UNKNOWN_URL_SCHEME，
        // 深色正文里还占出一大块空白。直接丢掉这个 <img>。
        if (child.tagName === 'IMG' && /^\s*cid:/i.test(child.getAttribute('src') || '')) {
          node.removeChild(child)
          continue
        }
        for (const attr of [...child.attributes]) {
          const name = attr.name.toLowerCase()
          if (name === 'href' && /^(https?:|mailto:|cid:)/i.test(attr.value)) continue
          if (child.tagName === 'IMG' && name === 'src' && /^(https?:|data:image\/|cid:)/i.test(attr.value)) {
            continue
          }
          child.removeAttribute(attr.name)
        }
        walk(child)
      }
    }
  }
  walk(doc.body)
  return doc.body.innerHTML
}

/** 地址栏补全：联系人与历史往来地址统一走 contacts store。 */
async function suggestAddress(prefix) {
  return contacts.suggest(prefix)
}

/** 通讯录选择器：同一个面板可以给收件人、抄送或密送加人。 */
function openPicker(field) {
  if (pickerOpen.value && pickerTarget.value === field) {
    pickerOpen.value = false
    return
  }
  pickerTarget.value = field
  pickerOpen.value = true
}

function appendToTarget(addresses) {
  const key = pickerTarget.value
  const current = [...(props.draft[key] || [])]
  for (const address of addresses) {
    const value = String(address || '').trim()
    if (value && !current.some((item) => sameAddress(item, value))) current.push(value)
  }
  emit('update', { [key]: current })
}

function onPickAddress(address) {
  appendToTarget([address])
}

function onPickGroup(groupId) {
  const group = (contacts.groups || []).find((g) => g.id === groupId)
  appendToTarget([groupRecipientToken(group || { id: groupId, name: '通讯组' })])
  pickerOpen.value = false
}

function sameAddress(a, b) {
  const norm = (value) => {
    const match = String(value || '').match(/<([^>]+)>/)
    return (match ? match[1] : value).trim().toLowerCase()
  }
  return norm(a) === norm(b)
}

/**
 * 收件人合计：通讯组按展开后的唯一地址计数，跨收件人/抄送/密送去重。
 * 大组发送前用它给一次确认，避免用户不知道这一封会发给上百人。
 */
function recipientSummary() {
  const seen = new Set()
  let groups = 0
  for (const list of [props.draft.to, props.draft.cc, props.draft.bcc]) {
    for (const token of list || []) {
      const group = parseGroupToken(token)
      if (group) {
        groups += 1
        for (const member of expandGroupMembers(contacts.groups, group.id)) {
          seen.add(member.email.toLowerCase())
        }
        continue
      }
      const match = String(token || '').match(/<([^>]+)>/)
      const email = (match ? match[1] : token).trim().toLowerCase()
      if (email) seen.add(email)
    }
  }
  return { groups, total: seen.size }
}

async function pickAttachments() {
  try {
    const files = await open({ multiple: true, title: '选择附件' })
    if (!files) return
    addPaths(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('选择附件失败', error)
  }
}

/** 从笔记库挑文件作为附件：直接把笔记库目录当起始位置，省得用户自己找。 */
async function pickFromNotes() {
  try {
    const root = await invoke('get_libraries_root').catch(() => '')
    const files = await open({
      multiple: true,
      title: '从笔记库选择文件',
      defaultPath: root || undefined,
    })
    if (!files) return
    addPaths(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('从笔记库选择附件失败', error)
  }
}

/** 把图片作为 `multipart/related` 的内联资源插进正文。 */
async function pickInlineImages() {
  try {
    const files = await open({
      multiple: true,
      title: '选择要插入正文的图片',
      filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'] }],
    })
    if (!files) return
    addInlineImages(Array.isArray(files) ? files : [files])
  } catch (error) {
    logger.warn('选择内联图片失败', error)
  }
}

/**
 * 登记内联图片并把 `<img src="cid:...">` 插到光标处。
 * Content-ID 用随机串而不是文件名：同名图片在多封邮件里复用同一 cid
 * 会让收信端把缓存串起来（换了图却还显示旧图）。
 */
function addInlineImages(paths) {
  if (sourceMode.value) {
    window.alert('请先切回富文本模式再插入图片')
    return
  }
  const next = [...(props.draft.inlineImages || [])]
  for (const path of paths) {
    if (!path) continue
    const name = path.split(/[\\/]/).pop()
    const contentId = `guaxi-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}@guaxi`
    next.push({ contentId, path, name })
    document.execCommand(
      'insertHTML',
      false,
      `<img data-guaxi-img="1" src="cid:${contentId}" alt="${escapeHtml(name)}" />`,
    )
  }
  emit('update', { inlineImages: next })
  nextTick(() => {
    resolveCidImages()
    // 刚插进去的图直接进入「选中」状态：图片工具行随之出现，宽度与位置马上可调。
    const images = editorRef.value?.querySelectorAll('img')
    const last = images?.[images.length - 1]
    if (!last) return
    selectedImage.value = last
    imageWidthPercent.value = imageWidthPercentOf(last)
    imageAlign.value = alignOf(last)
  })
  onEditorInput()
}

function removeInlineImage(contentId) {
  const next = (props.draft.inlineImages || []).filter((img) => img.contentId !== contentId)
  emit('update', { inlineImages: next })
  // 正文里对应的 img 一并删掉，否则收信端会看到一个破图。
  if (editorRef.value) {
    editorRef.value.querySelectorAll('img').forEach((img) => {
      if (img.dataset.cid === contentId || img.getAttribute('src') === `cid:${contentId}`) {
        img.remove()
      }
    })
    onEditorInput()
  }
}

/** 套用模板：只覆盖主题与正文，保留收件人。点模板条目本身就是确认。 */
function applyTemplateNow(template) {
  if (!template) return
  closePalette()
  emit('apply-template', template)
  if (editorRef.value && template.html) {
    editorRef.value.innerHTML = template.html
    lastHtml = template.html
    scheduleQuoteTheme()
    onEditorInput()
  }
}

// ── S/MIME 收件人证书 ──
//
// 加密发送必须有收件人的证书；没有就在工具栏上给一个明显的入口，
// 而不是等到点「发送」才报「无法加密」。
const needsPeerCert = computed(() => String(props.draft.cryptoMode || '').startsWith('smime_encrypt') || String(props.draft.cryptoMode || '').startsWith('smime_sign_encrypt'))
const peerCertReady = ref(false)
const peerAddress = computed(() => {
  const first = props.draft.to?.[0] || ''
  const match = String(first).match(/<([^>]+)>/)
  return (match ? match[1] : first).trim().toLowerCase()
})

watch(
  () => [needsPeerCert.value, peerAddress.value],
  async ([need, address]) => {
    if (!need || !address) {
      peerCertReady.value = false
      return
    }
    peerCertReady.value = await invoke('mail_smime_peer_status', { address }).catch(() => false)
  },
)

async function importPeerCert() {
  const address = peerAddress.value
  if (!address) {
    window.alert('请先填写收件人')
    return
  }
  const { open } = await import('@tauri-apps/plugin-dialog')
  const files = await open({
    multiple: false,
    title: `选择 ${address} 的 S/MIME 证书`,
    filters: [{ name: '证书', extensions: ['pem', 'crt', 'cer'] }],
  })
  if (!files) return
  try {
    const { readTextFile } = await import('@tauri-apps/plugin-fs')
    const certPem = await readTextFile(files)
    await invoke('mail_smime_save_peer', { address, certPem })
    peerCertReady.value = true
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

/** 附件危险类型 / 超限提示。 */
function checkOf(path) {
  return props.attachmentChecks.find((c) => c.path === path) || null
}

function isDangerous(name) {
  const ext = String(name || '').split('.').pop().toLowerCase()
  return [
    'exe', 'scr', 'com', 'bat', 'cmd', 'pif', 'msi', 'vbs', 'vbe', 'js', 'jse',
    'wsf', 'wsh', 'ps1', 'psm1', 'hta', 'cpl', 'reg', 'lnk', 'jar',
  ].includes(ext)
}

function warningFor(path) {
  const check = checkOf(path)
  if (!check) return ''
  return check.level === 'ok' ? '' : check.detail
}

function sizeHint(path) {
  const check = checkOf(path)
  if (!check || !check.size) return ''
  const mb = check.size / 1048576
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(check.size / 1024))} KB`
}

function addPaths(list) {
  const next = [...props.draft.attachments]
  for (const path of list) {
    if (!path) continue
    if (next.some((a) => a.path === path)) continue
    const name = path.replace(/\\/g, '/').split('/').pop()
    next.push({ path, name })
  }
  emit('update', { attachments: next })
  emit('check-attachments', next.map((a) => a.path))
}

/** 把一段文本作为正文追加（用于「把邮件/笔记内容发出去」这类场景）。 */
function appendBodyText(text, asQuote = false) {
  const safe = String(text || '').trim()
  if (!safe) return
  const html = safe
    .split(/\n{2,}/)
    .map((p) => `<p>${escapeHtml(p).replace(/\n/g, '<br>')}</p>`)
    .join('')
  const wrapped = asQuote ? `<blockquote>${html}</blockquote>` : html
  const nextHtml = `${props.draft.html || ''}${wrapped}`
  emit('update', { html: nextHtml, text: `${props.draft.text || ''}\n\n${safe}` })
  if (!sourceMode.value && editorRef.value) {
    editorRef.value.innerHTML = nextHtml
  }
}

function escapeHtml(text) {
  return String(text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

// ── 自动暂存 ──
//
// 每隔一段时间把当前草稿写进本地（不发送、不碰服务器）。目的很具体：
// 写完一大段话，点关闭或切账户后内容不能凭空消失——邮件客户端里最容易让人恼火的事
// 就是"写了半小时的邮件没了"。
const draftId = ref(props.draft.id || `draft-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`)
let autosaveTimer = null
const lastSavedAt = ref(0)

/** 草稿箱里那一份的 uid / folder：自动保存复用同一份，不会每次新建。 */
const mailboxDraft = reactive({ uid: null, folder: '' })

/**
 * 立即保存一次。
 *
 * 两条通道一起写：
 * 1. `mail_stash_draft`：本地快速暂存（崩溃后能恢复，不依赖网络）；
 * 2. `mail_save_draft`：真正写进草稿箱（用户在别的客户端也能看到）。
 *
 * 为什么两条都要：暂存是"本机能恢复"，草稿箱是"用户看得见"。只做前者，
 * 崩溃重启后用户找不到那封写了一半的信；只做后者，断网时会一直失败。
 *
 * 草稿箱里的那一份是**替换**而不是追加：记住上一份的 uid，写新的成功后再删旧的。
 * 以前每次心跳都 append 一封新的，草稿箱里很快堆出一串同名草稿——最新一份
 * 还来不及带上正文，用户打开看到的就是「只有引用的历史邮件」。
 */
let stashInflight = false
/** 上一份写进草稿箱的内容签名：内容没变就不再删一封、写一封。 */
let lastMailboxSignature = ''
/** 打开编辑器时的内容签名：用户一个字都没动过就不该在草稿箱里造一封「空壳」。 */
let initialMailboxSignature = ''
/**
 * 写草稿箱的最小间隔。
 *
 * 本地暂存每 15 秒一次（几乎零成本），写服务器草稿箱要登录 IMAP + APPEND + 同步，
 * 删旧稿还要再来一次——每 15 秒跑两轮会把弱网环境拖垮。30 秒一次既够「意外中断
 * 不丢」，也不会让后台一直占着连接。
 */
const MAILBOX_AUTOSAVE_MS = 30_000
let lastMailboxAt = 0

function mailboxSignature(html) {
  return [
    html || '',
    props.draft.subject || '',
    (props.draft.to || []).join(','),
    (props.draft.cc || []).join(','),
    (props.draft.bcc || []).join(','),
  ].join('\u0000')
}

async function stashNow({ toMailbox = true } = {}) {
  // 用户已经点了「直接放弃」：连本地暂存都不该再写回去。
  if (discarded) return null
  // 不判断有没有内容：只要这封还在编辑，就留一份可以继续写的草稿。
  if (stashInflight) return null
  const html = editorRef.value && !sourceMode.value ? serializeEditorHtml() : htmlSource.value
  const payload = { ...props.draft, id: draftId.value, html }
  stashInflight = true
  try {
    try {
      await invoke('mail_stash_draft', { draft: payload })
      lastSavedAt.value = Date.now()
    } catch (error) {
      logger.warn('自动暂存失败', error)
    }
    // 放弃和这一轮自动保存撞在一起：刚写进去的本地暂存要立刻撤掉。
    if (discarded) {
      await invoke('mail_delete_stashed_draft', { id: draftId.value }).catch(() => {})
      return draftId.value
    }

    if (!toMailbox || !props.saveMailboxDraft) return draftId.value
    const signature = mailboxSignature(html)
    // 没动过、或者和上一次写的完全一样：都不必再碰草稿箱。
    if (signature === lastMailboxSignature || signature === initialMailboxSignature) {
      return draftId.value
    }
    if (Date.now() - lastMailboxAt < MAILBOX_AUTOSAVE_MS) return draftId.value
    try {
      const previous = mailboxDraft.uid ? { uid: mailboxDraft.uid, folder: mailboxDraft.folder } : null
      const saved = await props.saveMailboxDraft({
        accountId: props.draft.accountId,
        to: props.draft.to,
        cc: props.draft.cc,
        bcc: props.draft.bcc,
        subject: props.draft.subject,
        text: props.draft.text,
        html: props.draft.htmlMode ? html : null,
        inReplyTo: props.draft.inReplyTo,
        references: props.draft.references,
        attachments: (props.draft.attachments || []).map((a) => a.path),
        inlineImages: (props.draft.inlineImages || []).map((img) => ({
          contentId: img.contentId,
          path: img.path,
          filename: img.name || '',
        })),
        cryptoMode: props.draft.cryptoMode || '',
      })
      if (saved?.uid) {
        mailboxDraft.uid = saved.uid
        mailboxDraft.folder = saved.folder || mailboxDraft.folder
      }
      // 同上：这一轮写进草稿箱的同时用户点了放弃，这份也要立刻删掉，
      // 否则「放弃了，草稿箱里却还多出一封」。
      if (discarded) {
        if (saved?.uid && saved?.folder) {
          await invoke('mail_discard_draft', {
            accountId: props.draft.accountId,
            folder: saved.folder,
            uid: saved.uid,
          }).catch(() => {})
        }
        return draftId.value
      }
      // 新草稿写成功之后才删旧的那份：中途失败也不会把内容弄丢。
      if (previous?.uid && previous.folder && previous.uid !== mailboxDraft.uid) {
        await invoke('mail_discard_draft', {
          accountId: props.draft.accountId,
          folder: previous.folder,
          uid: previous.uid,
        }).catch(() => {})
      }
      // 关窗 / 发送路径也要知道这份自动草稿，才能在落最终草稿时把它替换掉。
      if (props.draft) {
        props.draft.mailboxDraft = mailboxDraft.uid
          ? { uid: mailboxDraft.uid, folder: mailboxDraft.folder }
          : null
      }
      lastMailboxSignature = signature
      lastMailboxAt = Date.now()
      lastSavedAt.value = Date.now()
    } catch (error) {
      logger.warn('自动保存草稿箱失败', error)
    }
  } finally {
    stashInflight = false
  }
  return draftId.value
}

/** 父组件写进草稿箱后回传 uid，下一轮自动保存先删旧的再写。 */
function rememberMailboxDraft(uid, folder) {
  mailboxDraft.uid = uid || null
  mailboxDraft.folder = folder || ''
}

function startAutosave() {
  stopAutosave()
  // 15 秒一次：用户要求"千万不能因为意外丢失"，而写草稿箱的代价只是一次本地 append。
  autosaveTimer = setInterval(() => {
    stashNow()
  }, 15_000)
}
function stopAutosave() {
  if (autosaveTimer) {
    clearInterval(autosaveTimer)
    autosaveTimer = null
  }
}

onMounted(startAutosave)

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onPaletteOutside)
  document.removeEventListener('mousedown', onTranslateOutside)
  document.removeEventListener('keydown', onPaletteEscape)
  document.removeEventListener('selectionchange', refreshFormatState)
  disposeSourceEditor()
  stopAutosave()
  // 关闭时立刻存一次本地暂存（不写草稿箱）：真正写进草稿箱由父级在
  // closeCompose / closeWindow 里做，这里再写一次只会在草稿箱里多出一份重复草稿。
  // 「直接放弃」时不能再存：那份暂存刚刚才被删掉。
  if (!discarded) stashNow({ toMailbox: false })
})

defineExpose({ appendBodyText, addPaths, stashNow, rememberMailboxDraft })

function removeAttachment(path) {
  const next = props.draft.attachments.filter((a) => a.path !== path)
  emit('update', { attachments: next })
  emit('check-attachments', next.map((a) => a.path))
}

async function send(useOutbox) {
  if (sending.value) return
  if (!confirmBulkRecipients()) return
  syncEditorToDraft()
  sending.value = true
  try {
    await nextTick()
    emit('send', { useOutbox })
  } finally {
    sending.value = false
  }
}

/** 大通讯组：地址栏只有一个组名，用户看不出真实人数；发送前报一次总数。 */
function confirmBulkRecipients() {
  const summary = recipientSummary()
  if (summary.total <= 50) return true
  const groupHint = summary.groups ? `（其中 ${summary.groups} 个通讯组）` : ''
  return window.confirm(
    `这封邮件将发给 ${summary.total} 位收件人${groupHint}。\n` +
      '部分邮件服务器对单封收件人数有上限（如 Gmail 100 人），超出时整封会被拒收。仍要发送吗？',
  )
}

/** 把编辑区最新内容并入草稿（用户可能还没触发 input 事件）。 */
function syncEditorToDraft() {
  if (!sourceMode.value && editorRef.value) {
    const html = serializeEditorHtml()
    emit('update', { html, text: htmlToText(html) })
  } else {
    emit('update', { html: htmlSource.value, text: htmlToText(htmlSource.value) })
  }
}

function toLocalInput(ms) {
  const date = new Date(ms)
  const pad = (value) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

function toggleSchedule() {
  if (!canSend.value) return
  if (!scheduleOpen.value) {
    scheduleInput.value = toLocalInput(Date.now() + 60 * 60 * 1000)
  }
  scheduleOpen.value = !scheduleOpen.value
}

function scheduleAfter(hours) {
  scheduleAt(Date.now() + hours * 60 * 60 * 1000)
}

function scheduleTomorrowMorning() {
  const date = new Date()
  date.setDate(date.getDate() + 1)
  date.setHours(9, 0, 0, 0)
  scheduleAt(date.getTime())
}

function scheduleCustom() {
  const ms = new Date(scheduleInput.value).getTime()
  if (!Number.isFinite(ms)) {
    window.alert('请选择有效的发送时间')
    return
  }
  scheduleAt(ms)
}

/** 定时发送：内容校验、大组确认、编辑区同步都和立即发送一致，只是投递时间不同。 */
async function scheduleAt(ms) {
  if (sending.value) return
  if (!canSend.value) return
  if (ms <= Date.now() + 30 * 1000) {
    window.alert('定时发送时间需要晚于当前时间至少 1 分钟')
    return
  }
  if (!confirmBulkRecipients()) return
  syncEditorToDraft()
  sending.value = true
  try {
    await nextTick()
    emit('schedule', ms)
    scheduleOpen.value = false
  } finally {
    sending.value = false
  }
}

function setList(key, value) {
  emit('update', { [key]: splitAddresses(value) })
}

function joinList(list) {
  return (list || []).join(', ')
}

/** 按逗号/分号切分，保留 `Name <addr>` 形态。 */
function splitAddresses(value) {
  return String(value || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
}

/** 富文本转纯文本（发给不支持 HTML 的客户端 / 保存搜索索引用的那一份）。 */
function htmlToText(html) {
  const doc = new DOMParser().parseFromString(String(html || ''), 'text/html')
  doc.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
  doc.querySelectorAll('p,div,li,tr,h1,h2,h3,h4,h5,h6,blockquote,pre').forEach((el) => {
    el.append('\n')
  })
  return (doc.body.textContent || '').replace(/\n{3,}/g, '\n\n').trim()
}
</script>
