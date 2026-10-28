<template>
  <div class="pdf-viewer-container">
    <!-- 顶部控制栏 -->
    <div class="pdf-controls guaxi-reader-controls" :style="{ '--pdf-progress': `${progressPercent}%` }">
      <div class="page-info">
        <span class="page-info-main">第 {{ currentPage }} 页 / 共 {{ totalPages }} 页（{{ progressPercent }}%）</span>
        <span v-if="completedReadingMs" class="page-info-time">已完成：{{ formatDuration(completedReadingMs) }}</span>
      </div>

      <div class="page-navigation">
        <button @click="prevPage" :disabled="currentPage <= 1" title="上一页">
          <n-icon :component="ArrowBackOutline" />
        </button>

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage">

        <button @click="nextPage" :disabled="currentPage >= totalPages" title="下一页">
          <n-icon :component="ArrowForwardOutline" />
        </button>
      </div>

      <div class="right-controls">
        <!-- 旧版渲染核心(PDF.js)专用控件 -->
        <template v-if="!useNewPdfViewer">
          <button class="rotate-left" @click="rotate(-90)" title="向左旋转90度">
            <Icon :icon="RotateCcwIcon" width="16" height="16" />
            <span class="btn-text">左旋</span>
          </button>
          <button class="rotate-right" @click="rotate(90)" title="向右旋转90度">
            <Icon :icon="RotateCwIcon" width="16" height="16" />
            <span class="btn-text">右旋</span>
          </button>
          <button class="fit-width" @click="fitToWidth" title="将页面调整为适合宽度">
            <n-icon :component="ExpandOutline" />
            <span class="btn-text">适合宽度</span>
          </button>
          <button class="zoom-out" @click="zoomOut" :disabled="zoomLevel <= 0.5" title="缩小">
            <n-icon :component="RemoveOutline" />
          </button>
          <span class="zoom-level">{{ Math.round(zoomLevel * 100) }}%</span>
          <button class="zoom-in" @click="zoomIn" :disabled="zoomLevel >= 2" title="放大">
            <n-icon :component="AddOutline" />
          </button>
        </template>

        <!-- 新版 pdfium 阅读器：工具栏外置到此处，统一驱动 GuaxiPdfViewer -->
        <template v-else>
          <div class="ctl-group">
            <button title="撤销高亮 (Ctrl+Z)" :disabled="!canUndoHighlight" @click="undoHighlight">
              <n-icon :component="ArrowUndoOutline" />
            </button>
            <button title="重做高亮 (Ctrl+Y)" :disabled="!canRedoHighlight" @click="redoHighlight">
              <n-icon :component="ArrowRedoOutline" />
            </button>
          </div>
          <span class="ctl-sep" />
          <div class="ctl-group">
            <button
              title="上一阅读位置 (Alt+←)"
              :disabled="!viewerCanReadBack"
              @click="viewerReadBack"
            >
              <n-icon :component="ChevronBackOutline" />
            </button>
            <button
              title="下一阅读位置 (Alt+→)"
              :disabled="!viewerCanReadForward"
              @click="viewerReadForward"
            >
              <n-icon :component="ChevronForwardOutline" />
            </button>
          </div>
          <span class="ctl-sep" />
          <div class="ctl-group">
            <button class="rotate-left" @click="viewerRotateLeft" title="向左旋转90度 (Ctrl+[)">
              <Icon :icon="RotateCcwIcon" width="16" height="16" />
            </button>
            <button class="rotate-right" @click="viewerRotateRight" title="向右旋转90度 (Ctrl+])">
              <Icon :icon="RotateCwIcon" width="16" height="16" />
            </button>
            <button class="fit-width" @click="viewerFitWidth" title="适合宽度 (Ctrl+0)">
              <n-icon :component="ExpandOutline" />
            </button>
          </div>
          <span class="ctl-sep" />
          <div class="ctl-group ctl-group--zoom">
            <button class="zoom-out" @click="viewerZoomOut" title="缩小 (Ctrl+-)">
              <n-icon :component="RemoveOutline" />
            </button>
            <span class="zoom-level">{{ viewerZoomPercent }}%</span>
            <button class="zoom-in" @click="viewerZoomIn" title="放大 (Ctrl++)">
              <n-icon :component="AddOutline" />
            </button>
          </div>
          <span class="ctl-sep" />
          <button class="pdf-search-toggle" @click="openPdfSearch" title="搜索 (Ctrl+F)">
            <n-icon :component="SearchOutline" />
          </button>
        </template>

        <span class="ctl-sep" />

        <n-dropdown trigger="click" :options="viewModeOptions" @select="handleViewModeSelect">
          <n-button class="layout-switch" text title="切换布局">
            <template #icon>
              <n-icon :component="OptionsOutline" />
            </template>
          </n-button>
        </n-dropdown>

        <n-button class="layout-switch" text
          :title="isBookmarkedCurrent ? `取消书签（第 ${currentPage} 页）` : `添加书签（第 ${currentPage} 页）`"
          @click="toggleBookmarkCurrentPage">
          <template #icon>
            <n-icon :component="BookmarksOutline" />
          </template>
        </n-button>

        <n-dropdown trigger="click" :options="bookmarkOptions" @select="handleBookmarkSelect">
          <n-button class="layout-switch" text title="书签列表">
            <template #icon>
              <n-icon :component="LibraryOutline" />
            </template>
          </n-button>
        </n-dropdown>

        <span class="ctl-sep" />

        <!-- 显示/隐藏笔记按钮 -->
        <button class="toggle-notes" :class="{ active: showNotes }" @click="toggleNotesPanel"
          :title="showNotes ? '隐藏笔记' : '显示笔记'">
          <n-icon :component="DocumentTextOutline" />
        </button>
        <button class="toggle-assistant" :class="{ active: showAssistant }" @click="toggleAssistantPanel"
          :title="showAssistant ? '隐藏助手' : '显示助手'">
          <n-icon :component="ChatbubbleEllipsesOutline" />
        </button>

        <button class="toggle-translate" @click="toggleTranslate" :title="showTranslation ? '隐藏译文' : '显示译文'"
          :class="{ 'translating': isTranslating }">
          <n-icon :component="LanguageOutline" />
        </button>

        <!-- 逐句朗读：读的是右侧转换后的正文，点开即从断点连续朗读 -->
        <button class="toggle-readaloud" :class="{ 'reading-aloud': readAloudPlaying }" @click="toggleReadAloud"
          :title="readAloudTitle">
          <n-icon :component="readAloudPlaying ? PauseOutline : PlayOutline" />
        </button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="pdf-main-content">
      <div v-if="pdfSearchVisible || mdSearchVisible" class="inpage-search-bar"
        :class="pdfSearchVisible ? 'inpage-search-bar-pdf' : 'inpage-search-bar-md'" @mousedown.stop @click.stop
        @dblclick.stop>
        <input v-if="pdfSearchVisible" ref="pdfSearchInput" class="inpage-search-input" v-model="pdfSearchQuery"
          placeholder="PDF搜索" @input="handlePdfSearchInput" @keydown.enter.exact.prevent="gotoNextPdfSearch"
          @keydown.shift.enter.prevent="gotoPrevPdfSearch" @keydown.esc.prevent="closePdfSearch" />
        <input v-else ref="mdSearchInput" class="inpage-search-input" v-model="mdSearchQuery" placeholder="Markdown搜索"
          @input="applyMarkdownSearch" @keydown.enter.exact.prevent="gotoNextMarkdownSearch"
          @keydown.shift.enter.prevent="gotoPrevMarkdownSearch" @keydown.esc.prevent="closeMarkdownSearch" />
        <span class="inpage-search-count">{{ pdfSearchVisible ? pdfSearchCountText : mdSearchCountText }}</span>
        <button v-if="pdfSearchVisible" class="inpage-search-btn" type="button" title="跳转"
          @click="jumpToActivePdfSearchResult">
          <n-icon :component="SearchOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="上一个"
          @click="pdfSearchVisible ? gotoPrevPdfSearch() : gotoPrevMarkdownSearch()">
          <n-icon :component="ArrowBackOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="下一个"
          @click="pdfSearchVisible ? gotoNextPdfSearch() : gotoNextMarkdownSearch()">
          <n-icon :component="ArrowForwardOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="关闭"
          @click="pdfSearchVisible ? closePdfSearch() : closeMarkdownSearch()">
          <n-icon :component="CloseOutline" />
        </button>
      </div>

      <div class="pdf-left-wrapper" v-show="viewMode !== 'full-right' || showOutline || pdfSearchVisible" :style="{ width: leftPanelWidth }" @mouseenter="lastReaderRegion = 'pdf'">
        <!--
          左侧内容改成标签侧栏：大纲 / 书签 / 笔记共用一列，靠顶部标签切换。

          原先只有「大纲」一列（点工具栏的大纲按钮才出现），书签和笔记都散在工具栏的下拉
          和右侧浮动面板里 —— 同一类「翻找书里某处」的动作被拆到三个位置。现在与电子书
          阅读器、独立 PDF 阅读器的侧栏是一致的：一列、几个标签、内容随标签换。
        -->
        <ReaderSidePanel
          v-if="showOutline"
          :tabs="sideTabs"
          :active="sideTab"
          :width="outlineWidth"
          @update:active="sideTab = $event"
          @close="showOutline = false"
          @resize-start="startOutlineResize"
        >
          <template #outline>
            <div @click.stop><GuaxiOutline /></div>
          </template>
          <template #bookmarks>
            <div class="rsp-list" @click.stop>
              <div v-if="!bookmarks.length" class="rsp-empty">还没有书签。用工具栏的书签按钮添加当前页。</div>
              <button
                v-for="p in sortedBookmarks"
                :key="`bm-${p}`"
                class="rsp-item"
                :class="{ 'is-on': p === currentPage }"
                @click="goToPage(p)"
              >
                <span class="rsp-item__title">第 {{ p }} 页</span>
              </button>
            </div>
          </template>
          <template #notes>
            <div class="rsp-list" @click.stop>
              <div v-if="highlightsLoading && !allHighlights.length" class="rsp-empty">正在读取高亮…</div>
              <div v-else-if="!allHighlights.length" class="rsp-empty">还没有高亮。在正文里选中文字，点浮条上的荧光笔即可添加。</div>
              <button
                v-for="h in allHighlights"
                :key="h.annotId"
                class="rsp-item"
                :class="{ 'is-on': selectedHighlight?.annotId === h.annotId }"
                :title="[h.text, h.note].filter(Boolean).join('\n')"
                @click="jumpToHighlight(h)"
              >
                <span class="rsp-item__title">{{ h.text || h.note || '（空高亮）' }}</span>
                <span class="rsp-item__sub">
                  第 {{ h.page }} 页<span v-if="h.text && h.note"> · {{ h.note }}</span>
                </span>
              </button>
            </div>
          </template>
        </ReaderSidePanel>

        <!-- 新版统一 pdfium 阅读器（GuaxiPdfViewer，连续滚动 + 虚拟化 + 字级文本层 + 写回式高亮）-->
        <div v-if="useNewPdfViewer" class="pdf-render-panel pdf-render-panel--new guaxi-reader-scrollbar" v-show="viewMode !== 'full-right'">
          <GuaxiPdfViewer
            ref="pdfViewer"
            :path="props.filePath"
            :theme="viewerTheme"
            :features="newViewerFeatures"
            fit-mode="width"
            :selected-highlight-id="selectedHighlight?.annotId || ''"
            @ready="onViewerReady"
            @page-change="onViewerPageChange"
            @outline="onViewerOutline"
            @highlight-click="onViewerHighlightClick"
            @selection="onViewerSelection"
          />
          <!--
            底部常驻的标注工具条已移除：高亮与配色改由划词浮条接管（见文件顶部的 selection-toolbar）。
            原先它固定在阅读区左下角、与选区无关，读者选中文字后还得把鼠标挪过去点一下，
            「选中 → 高亮」被拆成两个位置的两步操作；而它那套颜色入口也只有一个小色块，
            不如浮条里直接平铺五个预设色直观。
          -->
        </div>

        <!-- 旧版渲染核心（PDF.js）；灰度回退用，验证通过后移除 -->
        <div v-else class="pdf-render-panel guaxi-reader-scrollbar" v-show="viewMode !== 'full-right'" ref="pdfPanel" @wheel="handleWheelZoom" @mousedown="startDrag"
          @mousemove="handleDrag" @mouseup="handlePdfPanelMouseUp" @mouseleave="endDrag" @mouseenter="setScrollTargetFromEvent"
          :style="{ cursor: isDragging ? 'grabbing' : 'text' }">

          <div class="pdf-image-container">
            <div class="pdf-page-container" ref="pdfPageContainer" v-show="!!currentPageData"
              :class="{ 'pdf-editor-active': annotationMode !== 'none' }" @click="handlePdfPageClick">
              <canvas ref="pdfCanvas" class="rendered-pdf-canvas"></canvas>
              <div ref="pdfHighlightLayer" class="pdf-highlight-layer"></div>
              <div ref="pdfAnnotationLayer" class="pdf-annotation-layer annotationLayer"></div>
              <div ref="pdfEditorLayer" class="pdf-editor-layer annotationEditorLayer"></div>
              <div ref="pdfTextLayer" class="pdf-text-layer textLayer"></div>
            </div>

            <div v-if="!currentPageData" class="pdf-loading-message">
              正在加载PDF页面...
            </div>
          </div>

          <div class="pdf-annotation-toolbar-wrap" @mousedown.stop @click.stop @dblclick.stop>
            <div class="pdf-annotation-toolbar">
              <button class="pdf-annotation-tool-btn" type="button" title="高亮（选中文本后点击；再次点击可取消已有高亮）" @click="addPdfHighlightFromSelection">
                <n-icon :component="ColorWandOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" :disabled="!pdfActiveAnnotationId" title="取消高亮（删除当前高亮）" @click="deleteActivePdfHighlight">
                <n-icon :component="TrashOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" title="添加标注（高亮+写标注）" @click="addPdfAnnotationFromSelection">
                <n-icon :component="ChatboxEllipsesOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" title="保存到原PDF" @click="savePdfAnnotationsToOriginal">
                <n-icon :component="SaveOutline" />
              </button>
            </div>
          </div>
        </div>

        <div v-if="pdfAnnotationPanelVisible" class="pdf-annotations-panel" @mousedown.stop @click.stop @dblclick.stop>
          <div class="pdf-annotations-header">
            <div class="pdf-annotations-title">PDF 标注</div>
            <button class="pdf-annotations-close" type="button" title="关闭" @click="exitPdfAnnotationMode">
              <n-icon :component="CloseOutline" />
            </button>
          </div>

          <div class="pdf-annotations-list guaxi-reader-scrollbar">
            <div v-if="!pdfAnnotations.length" class="pdf-annotations-empty">暂无标注</div>
            <div v-for="it in pdfAnnotations" :key="it.id" class="pdf-annotation-item"
              :class="{ 'pdf-annotation-item-active': it.id === pdfActiveAnnotationId }" @click="activatePdfAnnotation(it.id)">
              <div class="pdf-annotation-item-main">
                <div class="pdf-annotation-item-meta">第 {{ it.page }} 页</div>
                <div class="pdf-annotation-item-text">{{ it.selectionText }}</div>

                <div v-if="it.note && !it.editing" class="pdf-annotation-item-note">{{ it.note }}</div>

                <div v-if="it.editing" class="pdf-annotation-item-editor" @click.stop>
                  <textarea v-model="it.note" class="pdf-annotation-note-input" placeholder="输入标注内容..."></textarea>
                  <button class="pdf-annotation-note-save" type="button" title="保存标注" @click.stop="savePdfAnnotationNote(it.id)">
                    <n-icon :component="SaveOutline" />
                  </button>
                </div>
              </div>
              <button class="pdf-annotation-item-delete" type="button" title="删除" @click.stop="deletePdfAnnotation(it.id)">
                <n-icon :component="TrashOutline" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 可拖动的中间线 -->
      <div class="resize-handle" v-show="viewMode === 'split'" @mousedown="startResize"></div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel guaxi-reader-scrollbar" ref="markdownPanel" v-show="viewMode !== 'full-left' || mdSearchVisible" @click="handleMarkdownClickAlt" @dblclick="handleMarkdownClick" @mousemove="handleMarkdownHover"
        @mouseleave="handleMarkdownLeave" @mouseenter="onMarkdownEnter"
        @mouseup="handleSelectionMouseUp">

        <div class="markdown-content" ref="markdownContainer">
          <template v-for="(line, index) in markdownLines" :key="index">
            <div class="markdown-line">
              <div class="original-content" v-html="line.html"></div>
              <div v-if="showTranslation" class="translated-content" :class="{
                'skipped-translation': line.skipTranslation,
                'error-translation': line.translationError
              }">
                <template v-if="line.skipTranslation">
                  <div v-html="line.html"></div>
                  <span class="translation-skipped-marker">(内容未翻译)</span>
                </template>
                <template v-else-if="line.translationError">
                  <div class="translation-error-message">
                    {{ line.translationError }}
                  </div>
                  <!-- <div v-html="line.html" class="original-fallback"></div> -->
                </template>
                <template v-else>
                  <div v-html="line.translation"></div>
                </template>
              </div>
            </div>
          </template>
        </div>

        <n-modal v-model:show="assistantTranslateDialogVisible" preset="card" title="用AI翻译" style="width: 420px">
          <div style="display: flex; flex-direction: column; gap: 12px;">
            <n-text depth="3">请选择目标语言后发送到智能助手翻译。</n-text>
            <n-select v-model:value="assistantTranslateTargetLang" size="small" :options="assistantTranslateLangOptions"
              placeholder="请选择目标语言" />
            <div style="display: flex; justify-content: flex-end; gap: 8px;">
              <n-button size="small" @click="assistantTranslateDialogVisible = false">取消</n-button>
              <n-button size="small" type="primary" @click="sendSelectionAsAiTranslate">发送并翻译</n-button>
            </div>
          </div>
        </n-modal>
      </div>
    </div>

    <div class="assistant-panel" :class="{ 'assistant-panel-visible': showAssistant }">
      <div class="assistant-header">
        <h3>智能助手</h3>
        <button @click="toggleAssistantPanel" class="close-btn">
          <n-icon :component="CloseOutline" size="18" />
        </button>
      </div>
      <div class="assistant-content">
        <GuaxiAgentAssistant />
      </div>
    </div>

    <!-- 笔记面板 -->
    <div class="notes-panel" :class="{ 'notes-panel-visible': showNotes }">
      <div class="notes-header">
        <h3>我的笔记</h3>
        <button @click="toggleNotesPanel" class="close-btn">
          <n-icon :component="CloseOutline" size="18" />
        </button>
      </div>
      <div class="notes-content guaxi-reader-scrollbar" ref="notesContentEl" @mouseenter="setScrollTargetFromEvent">
        <!--
          高亮摘录排在手写笔记卡前面：对 PDF 来说，「我标了哪些话」比「我写了什么卡片」
          更常用。此前右侧笔记面板只渲染笔记卡，新加的高亮哪儿都不出现，
          读者只能去左侧侧栏（且要先从布局菜单打开）才看得见。
        -->
        <div v-if="useNewPdfViewer" class="notes-highlights">
          <div class="notes-highlights__head">
            <span>高亮</span>
            <span class="notes-highlights__count">{{ allHighlights.length }}</span>
          </div>
          <div v-if="!allHighlights.length" class="notes-highlights__empty">
            在正文里选中文字，点浮条上的荧光笔即可添加高亮。
          </div>
          <button
            v-for="h in allHighlights"
            :key="`nh-${h.annotId}`"
            type="button"
            class="notes-highlight"
            :class="{ 'is-on': selectedHighlight?.annotId === h.annotId }"
            :title="[h.text, h.note].filter(Boolean).join('\n')"
            @click="jumpToHighlight(h)"
          >
            <span class="notes-highlight__swatch" :style="{ background: rgbaArrToCss(h.color) }"></span>
            <span class="notes-highlight__text">{{ h.text || h.note || '（空高亮）' }}</span>
            <span class="notes-highlight__page">第 {{ h.page }} 页</span>
          </button>
        </div>
        <n-card v-for="(card, index) in noteCards" :key="card.id" class="note-card"
          :class="{ 'note-card--flashing': flashingNoteIndex === index }" size="small" :title="card.title"
          :bordered="true" :segmented="{
            content: true,
            footer: 'soft'
          }" header-style="padding: 12px 16px;" content-style="padding: 0;">
          <template #header-extra>
            <div class="card-actions" v-if="activeCardIndex === index">
              <n-button size="tiny" type="primary" @click.stop="saveCardEdit(index)" title="保存">
                <n-icon :component="SaveOutline" size="12" />
              </n-button>
              <n-button size="tiny" text @click.stop="exitEditMode(index)" title="退出编辑">
                <n-icon :component="ExitOutline" size="12" />
              </n-button>
              <n-button size="tiny" text @click.stop="deleteCard(index)" title="删除">
                <n-icon :component="TrashOutline" size="12" />
              </n-button>
            </div>
          </template>

          <!-- 根据笔记类型使用不同的渲染方式 -->
          <div v-if="card.type !== 'composite' && activeCardIndex !== index" class="note-content"
            :contenteditable="false" v-html="noteHtmls[index]" @click="enterEditMode(index)"></div>

          <div v-else-if="card.type !== 'composite' && activeCardIndex === index" class="note-content">
            <textarea class="composite-note-editor" @input="markEditorDirty(index)" @keydown.meta.s.prevent="saveCardEdit(index)"
              @keydown.ctrl.s.prevent="saveCardEdit(index)" :ref="el => setContentEditableRef(index, el)"></textarea>
          </div>

          <!-- 添加复合笔记的专用渲染 -->
          <div v-else-if="card.type === 'composite' && activeCardIndex !== index" class="composite-note-content"
            @click="enterEditMode(index)">
            <div v-for="(segment, segIndex) in compositeSegments[index]" :key="segIndex" class="note-segment"
              :class="getSegmentClass(segment)">
              <div v-html="segment.content"></div>
            </div>
          </div>

          <div v-else class="note-content">
            <textarea class="composite-note-editor" @input="markEditorDirty(index)" @keydown.meta.s.prevent="saveCardEdit(index)"
              @keydown.ctrl.s.prevent="saveCardEdit(index)" :ref="el => setContentEditableRef(index, el)"></textarea>
          </div>

          <template #footer>
            <div class="note-card-footer">
              <n-text depth="3" style="font-size: 12px;">
                {{ formatTime(card.timestamp) }}
              </n-text>
              <!-- 摘录的"回到原文"：跳到锚点所在页并把那段文字框出来。
                   旧笔记、以及从别处建的卡没有锚点，就不显示这个入口。 -->
              <button v-if="cardAnchor(card)" class="note-anchor-btn" type="button"
                :title="`回到原文（第 ${cardAnchor(card).page} 页）`" @click.stop="jumpToNoteAnchor(card)">
                <n-icon :component="LocateOutline" size="13" />
                <span>第 {{ cardAnchor(card).page }} 页</span>
              </button>
            </div>
          </template>
        </n-card>

        <div class="add-card-container">
          <n-button type="primary" dashed block @click="addNewCard">
            <template #icon>
              <n-icon :component="AddOutline" />
            </template>
            添加新卡片
          </n-button>
        </div>
      </div>
    </div>

    <!-- 划词工具栏 / 翻译面板：提到顶层（position:fixed），避免随 markdown 面板在"仅左侧"布局
         被 v-show 隐藏而失效——这样在任意布局下选词都能弹出。 -->
    <div v-if="selectionToolbarVisible" ref="selectionToolbarEl" class="selection-toolbar"
      :style="{ top: `${selectionToolbarTop}px`, left: `${selectionToolbarLeft}px` }"
      @mousedown.stop.prevent @mouseup.stop @click.stop @dblclick.stop>
      <div class="selection-toolbar-actions">
        <button class="selection-tool-btn" type="button" title="翻译" :disabled="selectionBusy || !selectionText"
          @click="translateSelection">
          <n-icon :component="LanguageOutline" />
        </button>
        <button class="selection-tool-btn" type="button" title="查询" :disabled="selectionBusy || !selectionText"
          @click="querySelection">
          <n-icon :component="SearchOutline" />
        </button>
        <!--
          高亮：直接在选定文字上落笔，颜色取自下面的调色盘。

          这两颗按钮原先挂在阅读区左下角常驻的「标注工具条」上，读者得先把视线
          挪到角落、点一下、再回到正文。放到划词浮条里之后，「选中 → 高亮」是一个连续动作，
          和独立 PDF 阅读器（ReaderPane）的交互也统一了。
        -->
        <button class="selection-tool-btn" type="button" title="高亮所选文本（写回 PDF）"
          :disabled="!selectionText" @click="addHighlightViaViewer">
          <!--
            高亮用它自己的荧光笔图标，不再复用 ColorWandOutline。
            后者已经是「用AI翻译」的图标 —— 两颗紧挨着的按钮长同一个样子，
            读者只能靠悬停提示区分，等于没有图标。
          -->
          <Icon :icon="HighlighterOutline" width="15" height="15" :style="{ color: highlightColorCss }" />
        </button>
        <button class="selection-tool-btn selection-tool-btn--swatch" type="button"
          :class="{ active: highlightPaletteOpen }"
          :title="`高亮颜色（当前 ${highlightColorCss}）`"
          :aria-expanded="highlightPaletteOpen"
          @click.stop="highlightPaletteOpen = !highlightPaletteOpen">
          <span class="selection-tool-swatch" :style="{ background: highlightColorCss }"></span>
        </button>
        <template v-if="highlightPaletteOpen">
          <button
            v-for="(c, i) in highlightPresets"
            :key="`hlc-${i}`"
            class="selection-tool-btn selection-tool-btn--preset"
            :class="{ active: rgbaArrToCss(c) === highlightColorCss }"
            type="button"
            :title="`高亮颜色 ${i + 1}`"
            @click.stop="highlightColor = [...c]"
          >
            <span class="selection-tool-swatch" :style="{ background: rgbaArrToCss(c) }"></span>
          </button>
        </template>
        <button class="selection-tool-btn" type="button" title="问AI" :disabled="selectionBusy || !selectionText"
          @click="askSelectionWithAssistant">
          <n-icon :component="ChatbubbleEllipsesOutline" />
        </button>
        <button class="selection-tool-btn" type="button" title="用AI翻译" :disabled="selectionBusy || !selectionText"
          @click="openAssistantTranslateDialog">
          <n-icon :component="ColorWandOutline" />
        </button>
        <button class="selection-tool-btn" type="button" title="插入笔记" :disabled="selectionBusy || !selectionInsertText"
          @click="insertSelectionResult">
          <n-icon :component="AddOutline" />
        </button>
        <!-- 朗读所选文本：播放中再点即停 -->
        <button class="selection-tool-btn" type="button"
          :title="selectionReadAloudBusy ? '停止朗读' : '朗读所选文本'"
          :disabled="(!selectionText || selectionBusy) && !selectionReadAloudBusy"
          @click="toggleSelectionReadAloud(normalizeSelectionTextForLanguageTasks(selectionText))">
          <n-icon :component="selectionReadAloudBusy ? StopCircleOutline : VolumeHighOutline" />
        </button>
      </div>
    </div>

    <div v-if="selectionTranslatePanelVisible" class="selection-translate-layer">
      <div class="selection-translate-layer-inner">
        <GuaxiSelectionTranslatePanel :pos="selectionTranslatePanelPos" :raw="selectionTranslateRaw"
          :result="selectionTranslateResult" :error="selectionTranslateError" :busy="selectionTranslateBusy"
          :pinned="selectionTranslatePinned" :items="selectionTranslateItems" :from-lang="selectionTranslateFrom"
          :to-lang="selectionTranslateTo" :lang-options="selectionTranslateLangOptions" :word-meta="selectionTranslateWordMeta"
          @update:from-lang="(v) => (selectionTranslateFrom = String(v || 'auto'))"
          @update:to-lang="(v) => (selectionTranslateTo = String(v || 'zh'))" @swap-langs="swapSelectionTranslateLangs"
          @translate="translateSelectionText" @copy="copySelectionTranslateResult" @toggle-pin="toggleSelectionTranslatePinned"
          @update:pos="handleSelectionTranslatePanelUpdatePos" @close="closeSelectionTranslatePanel" />
      </div>
    </div>

    <!-- 添加笔记的提示气泡 -->
    <transition name="fade">
      <div v-if="showNoteBubble" class="note-added-bubble">
        <div class="bubble-content">
          <span>已添加到笔记</span>
          <i class="icon-check"></i>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { Icon } from '@iconify/vue'
import HighlighterOutline from '@iconify-icons/lucide/highlighter'
import RotateCcwIcon from '@iconify-icons/lucide/rotate-ccw'
import RotateCwIcon from '@iconify-icons/lucide/rotate-cw'
import { ref, computed, watchEffect, onMounted, onBeforeUnmount, watch, nextTick, h } from 'vue'
import { markReaderScrollbarActive } from './readerScrollbar'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import { openUrl } from '@tauri-apps/plugin-opener';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { maskPythonCellBlocksForMarkdown } from '../HomeViewComponent/utils/guaxiPythonCellSyntax'
import { NCard, NButton, NIcon, NText, NModal, NSelect, useMessage, NDropdown, NPopover, NColorPicker } from 'naive-ui'
import usePdfRenderer from './usePdfRenderer'
import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import GuaxiOutline from '../HomeViewComponent/GuaxiOutline.vue'
import ReaderSidePanel from './ReaderSidePanel.vue'
import GuaxiAgentAssistant from '../HomeViewComponent/GuaxiAgentAssistant.vue'
import GuaxiSelectionTranslatePanel from '../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'
import {
  ArrowBackOutline,
  ArrowForwardOutline,
  ExpandOutline,
  TrashOutline,
  AddOutline,
  RemoveOutline,
  DocumentTextOutline,
  DownloadOutline,
  CloseOutline,
  LanguageOutline,
  SaveOutline,
  ExitOutline,
  OptionsOutline,
  BookmarksOutline,
  BookmarkOutline,
  LibraryOutline,
  SearchOutline,
  ColorWandOutline,
  LocateOutline,
  ChatboxEllipsesOutline,
  ChatbubbleEllipsesOutline,
  ArrowUndoOutline,
  ArrowRedoOutline,
  ChevronBackOutline,
  ChevronForwardOutline,
  PauseOutline,
  PlayOutline,
  StopCircleOutline,
  VolumeHighOutline
} from '@vicons/ionicons5'
import { useReaderReadAloud } from '../../composables/useReaderReadAloud'
import { useSelectionReadAloud } from '../../composables/useSelectionReadAloud'

const logger = createLogger('GuaxiConvertPDFReader')
// 1. 组件属性定义
const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['pdf'].includes(value)
  },
  fileName: {
    type: String,
    required: true,
  },
  library: {
    type: String,
    required: true
  },
  importFileId: {
    type: Number,
    default: null,
  }
})

// 2. 初始化状态和工具
const message = useMessage()
const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const outlineStore = useOutlineStore()
const rightSiderStore = useRightSiderStore()
const md = new MarkdownIt()

const resolveMarkdownTheme = () => {
  try {
    const root = document?.documentElement
    const isDark =
      !!root?.classList?.contains?.('dark') ||
      !!root?.classList?.contains?.('dark-mode')
    return isDark ? 'dark' : 'default'
  } catch {
    return 'default'
  }
}

const isEditableElement = (el) => {
  if (!el) return false
  const t = el.tagName ? el.tagName.toLowerCase() : ''
  if (t === 'input' || t === 'textarea' || t === 'select') return true
  return Boolean(el.isContentEditable)
}

const lastScrollTarget = ref(null)

// 鼠标最近所在的阅读区域（'pdf' | 'md'），用于 Ctrl+F 决定打开哪个搜索条。
// 默认 'pdf'（本就是 PDF 阅读器）。不复用 lastScrollTarget，因其只在滚动时更新、悬停 PDF 不会变。
const lastReaderRegion = ref('pdf')
const onMarkdownEnter = (e) => {
  lastReaderRegion.value = 'md'
  setScrollTargetFromEvent(e)
}

const setScrollTargetFromEvent = (e) => {
  const t = e?.currentTarget
  if (t && t instanceof HTMLElement) {
    lastScrollTarget.value = t
  }
}

const isScrollableElement = (el) => {
  if (!el) return false
  const hasV = el.scrollHeight > el.clientHeight
  const hasH = el.scrollWidth > el.clientWidth
  return hasV || hasH
}

const handleAnyScroll = (e) => {
  const t = e.target
  if (t === document) {
    const se = document.scrollingElement
    if (se) {
      markReaderScrollbarActive(se)
      lastScrollTarget.value = se
    }
    return
  }
  if (t && t instanceof HTMLElement && isScrollableElement(t)) {
    if (lastScrollTarget.value !== t) {
      logger.debug('[ReaderScrollbar] active scroll target changed', {
        className: t.className,
        scrollHeight: t.scrollHeight,
        clientHeight: t.clientHeight
      })
    }
    markReaderScrollbarActive(t)
    lastScrollTarget.value = t
  }
}

const handleReaderShortcuts = (e) => {
  if (isEditableElement(e.target)) return

  if (
    !e.altKey &&
    (e.key === 'ArrowUp' || e.key === 'ArrowDown' || e.key === 'ArrowLeft' || e.key === 'ArrowRight')
  ) {
    const target = lastScrollTarget.value
    if (target && isScrollableElement(target)) {
      const step = e.shiftKey ? 160 : 40
      const top = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0
      const left = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0
      if (top !== 0 || left !== 0) {
        e.preventDefault()
        target.scrollBy({ top, left, behavior: 'auto' })
        return
      }
    }
  }

  const ctrl = e.ctrlKey || e.metaKey

  if (e.key === 'PageUp') {
    e.preventDefault()
    prevPage()
    return
  }

  if (e.key === 'PageDown') {
    e.preventDefault()
    nextPage()
    return
  }

  if (ctrl && e.key === '0') {
    e.preventDefault()
    fitToWidth()
    return
  }

  if (ctrl && (e.key === '=' || e.key === '+')) {
    e.preventDefault()
    zoomIn()
    return
  }

  if (ctrl && e.key === '-') {
    e.preventDefault()
    zoomOut()
    return
  }

  // 新版 viewer 专属：撤销/重做高亮、旋转、平移、阅读位置前后。
  if (useNewPdfViewer.value) {
    // 撤销 Ctrl+Z；重做 Ctrl+Y 或 Ctrl+Shift+Z
    if (ctrl && !e.shiftKey && (e.key === 'z' || e.key === 'Z')) {
      e.preventDefault(); undoHighlight(); return
    }
    if (ctrl && ((e.key === 'y' || e.key === 'Y') || (e.shiftKey && (e.key === 'z' || e.key === 'Z')))) {
      e.preventDefault(); redoHighlight(); return
    }
    // 删除选中的高亮。和上面的撤销是一对，所以挨着放。
    if (!ctrl && !e.altKey && (e.key === 'Backspace' || e.key === 'Delete') && selectedHighlight.value) {
      e.preventDefault(); deleteSelectedHighlight(); return
    }
    if (e.key === 'Escape' && selectedHighlight.value) {
      e.preventDefault(); clearHighlightSelection(); return
    }
    // 旋转 Ctrl+[ / Ctrl+]
    if (ctrl && e.key === '[') { e.preventDefault(); viewerRotateLeft(); return }
    if (ctrl && e.key === ']') { e.preventDefault(); viewerRotateRight(); return }
    // 平移：空格临时平移由 GuaxiPdfViewer 自己在 window 上接管（它要按可见性判断，
    // 并且需要 keyup 收尾），这里不再抢占空格。
    // 阅读位置 Alt+← / Alt+→ 也由 viewer 统一处理（useReadingHistory 在 window 捕获阶段接管），
    // 宿主不再维护第二份历史栈。
  }

  if (ctrl && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault()
    toggleNotesPanel()
    return
  }

  if (ctrl && (e.key === 'f' || e.key === 'F')) {
    e.preventDefault()
    // 光标在哪个阅读区域，就打开哪个区域的搜索条。
    if (lastReaderRegion.value === 'md') {
      openMarkdownSearch()
    } else {
      openPdfSearch()
    }
    return
  }

  if (ctrl && (e.key === 't' || e.key === 'T')) {
    e.preventDefault()
    toggleTranslate()
    return
  }

  if (ctrl && e.key === '1') {
    e.preventDefault()
    handleViewModeSelect('split')
    return
  }

  if (ctrl && e.key === '2') {
    e.preventDefault()
    handleViewModeSelect('full-left')
    return
  }

  if (ctrl && e.key === '3') {
    e.preventDefault()
    handleViewModeSelect('full-right')
    return
  }

  if (ctrl && e.key === '4') {
    e.preventDefault()
    handleViewModeSelect('outline')
  }
}

onMounted(() => {
  window.addEventListener('scroll', handleAnyScroll, true)
  window.addEventListener('keydown', handleReaderShortcuts, true)
  window.addEventListener('mousedown', handleGlobalMouseDown, true)

  nextTick(() => {
    if (lastScrollTarget.value) return
    const defaultTarget =
      document.querySelector('.markdown-panel') ||
      document.querySelector('.notes-content') ||
      document.scrollingElement
    if (defaultTarget && defaultTarget instanceof HTMLElement) {
      lastScrollTarget.value = defaultTarget
    }
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('scroll', handleAnyScroll, true)
  window.removeEventListener('keydown', handleReaderShortcuts, true)
  window.removeEventListener('mousedown', handleGlobalMouseDown, true)
})

const markdownContainer = ref(null);
// 逐句朗读要的滚动容器：正文在 markdownContainer 里，真正滚动的是外层 .markdown-panel。
// 另外四个阅读器（Word/Epub/Markdown/Text）都有这个 ref，只有本组件当初漏了，
// 于是在 useReaderReadAloud 的 getContext 里抛 ReferenceError —— 朗读一点就报错。
const markdownPanel = ref(null);
// 笔记面板的滚动容器，以及"刚从 PDF 高亮跳过来"的那张卡的下标——
// 只滚动落位的话，用户不一定认得出是哪张，所以再闪一下。
const notesContentEl = ref(null);
const flashingNoteIndex = ref(-1);
let noteFlashTimer = null;
const noteHtmls = ref([]);
const compositeSegments = ref([]);
let notesRenderToken = 0;

const selectionToolbarVisible = ref(false)
const selectionToolbarTop = ref(0)
const selectionToolbarLeft = ref(0)
const selectionToolbarEl = ref(null)
// 划词浮动条的外框尺寸，与 .selection-toolbar 的 max-content 对应（6 个 34px 按钮 + 5×6px 间距 + 12px 内边距 + 2px 边框）。
// 只用于首帧兜底：真实宽度在 nextTick 里按 offsetWidth 实测修正。
const SELECTION_TOOLBAR_WIDTH = 248
const SELECTION_TOOLBAR_HEIGHT = 42
const selectionText = ref('')
// 最近一次 PDF 选区在文档里的位置（页码 + 页内矩形 pt）。必须在"选中的那一刻"取：
// 离开那一步 DOM 选区就没了，矩形再也取不到。之后「插入笔记」建卡时拿它当来源锚点。
const lastPdfSelectionAnchor = ref(null)
const selectionBusy = ref(false)
const selectionInsertText = ref('')
// 划词朗读：一小段文本走一次性合成，与逐句朗读共用引擎但**不同实例**，互不阻塞。
// 浮动条一关就停止（否则工具栏没了、声音还在响，用户没有停止入口）。
const { busy: selectionReadAloudBusy, toggle: toggleSelectionReadAloud } = useSelectionReadAloud({
  visible: selectionToolbarVisible,
  message,
})
const showAssistant = ref(false)

const selectionTranslatePanelVisible = ref(false)
const selectionTranslatePinned = ref(false)
const selectionTranslatePanelPos = ref({ left: 8, top: 8 })

const selectionTranslateFrom = ref('auto')
const selectionTranslateTo = ref('zh')

const selectionTranslateLangOptions = [
  { label: '自动检测', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' }
]
const assistantTranslateDialogVisible = ref(false)
const assistantTranslateTargetLang = ref('en')
const assistantTranslateSourceText = ref('')
const assistantTranslateLangOptions = selectionTranslateLangOptions.filter((x) => x.value !== 'auto')

const selectionTranslateBusy = ref(false)
const selectionTranslateRaw = ref('')
const selectionTranslateResult = ref('')
const selectionTranslateError = ref('')
const selectionTranslateItems = ref([])
const selectionTranslateWordMeta = ref(null)

const selectionTranslateMultiAvailable = ref(true)
let selectionTranslateMultiNotFoundWarned = false

let selectionTranslateScheduleTimer = null
const scheduleSelectionTranslate = () => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  clearTimeout(selectionTranslateScheduleTimer)
  selectionTranslateScheduleTimer = setTimeout(() => {
    translateSelectionText(raw)
  }, 250)
}

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return
  scheduleSelectionTranslate()
})

const selectionPreviewHtml = computed(() => {
  const v = selectionInsertText.value.trim()
  return v ? md.render(v) : ''
})

const handleSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return
  const left = Number(pos.left)
  const top = Number(pos.top)
  if (!Number.isFinite(left) || !Number.isFinite(top)) return

  const cur = selectionTranslatePanelPos.value
  if (cur && Number(cur.left) === left && Number(cur.top) === top) return

  selectionTranslatePanelPos.value = { left, top }
}

const toggleSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value
  logger.info('[SelectionTranslate][PDFReader] pin:', { pinned: selectionTranslatePinned.value })
}

const swapSelectionTranslateLangs = () => {
  const tmp = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = tmp
  logger.info('[SelectionTranslate][PDFReader] swap langs', { from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateSelectionText(selectionTranslateRaw.value)
  }
}

const closeSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false
  selectionTranslatePinned.value = false
}

const copySelectionTranslateResult = async (payload) => {
  const t = typeof payload === 'string' ? payload : payload?.text
  const text = String(t || selectionTranslateResult.value || '')
  if (!text) return

  try {
    await navigator.clipboard.writeText(text)
    message.success('已复制译文', { duration: 1200 })
  } catch {
    message.warning('复制失败：无剪贴板权限')
  }
}

const pdfSearchVisible = ref(false)
const pdfSearchQuery = ref('')
const pdfSearchMatches = ref([])
const pdfSearchActiveIndex = ref(-1)
const pdfSearchInput = ref(null)

const pdfFullSearchBusy = ref(false)
const pdfFullSearchResults = ref([])
const pdfFullSearchActiveIndex = ref(-1)
let pdfFullSearchTimer = 0

const mdSearchVisible = ref(false)
const mdSearchQuery = ref('')
const mdSearchMatches = ref([])
const mdSearchActiveIndex = ref(-1)
const mdSearchInput = ref(null)

const pdfSearchCountText = computed(() => {
  if (useNewPdfViewer.value) {
    void viewerStateTick.value
    const st = pdfViewer.value?.state
    const total = st?.searchHits?.value?.length || 0
    if (!total) return '0/0'
    return `${(st?.searchActiveIdx?.value || 0) + 1}/${total}`
  }
  if (pdfFullSearchBusy.value) return '搜索中…'

  const totalResults = pdfFullSearchResults.value.length
  if (totalResults) {
    const i = Math.max(0, pdfFullSearchActiveIndex.value)
    return `${Math.min(totalResults, i + 1)}/${totalResults}`
  }

  const total = pdfSearchMatches.value.length
  if (!total) return '0/0'
  return `${Math.max(0, pdfSearchActiveIndex.value) + 1}/${total}`
})

const mdSearchCountText = computed(() => {
  const total = mdSearchMatches.value.length
  if (!total) return '0/0'
  return `${Math.max(0, mdSearchActiveIndex.value) + 1}/${total}`
})

// 3. PDF渲染相关
const {
  renderMode,
  loadPage,
  renderContent,
  textLayer,
  isLoading,
  error,
  currentPageData,
  getOutlineFlat,
  searchText,
  ensureAnnotationEditorUi,
  setAnnotationEditorMode,
  saveDocumentBase64,
  getAnnotationEditorIdsForPage,
  removeAnnotationEditorById,
  highlightSelectionAsAnnotation
} = usePdfRenderer(props);

// === 新版统一 pdfium 阅读器（GuaxiPdfViewer）灰度开关与接线 ===
// useNewPdfViewer = true 时，PDF 渲染核心走 GuaxiPdfViewer（连续滚动/虚拟化/字级文本层/写回式高亮）；
// 旧的 PDF.js 路径保留为 v-else 回退，QA 通过后再删除。
const pdfViewer = ref(null)
const useNewPdfViewer = ref(true)
const viewerTheme = computed(() =>
  (typeof document !== 'undefined' && document.documentElement.classList.contains('dark')) ? 'dark' : 'light'
)
const newViewerFeatures = {
  outline: true,
  search: true,
  annotations: true,
  reverseJump: false,
  export: false,
  rebuild: false,
  // 阅读位置：工具栏按钮由本组件外置的工具栏负责，Alt+← / Alt+→ 由 viewer 统一接管。
  readingHistory: true,
  // 工具栏外置：隐藏 viewer 自带工具栏，改由本组件顶部公用工具栏驱动。
  toolbar: false,
}

// viewer 缩放/旋转状态：直接读取 viewer 暴露的内部 ref（响应式），供工具栏显示/高亮。
// viewerStateTick 在每次操作后自增，强制这些 computed 重新求值（兼容键盘路径直接调 viewer）。
const viewerStateTick = ref(0)
const bindViewerState = () => { viewerStateTick.value++ }
const viewerZoomPercent = computed(() => {
  void viewerStateTick.value
  return Math.round((pdfViewer.value?.state?.zoom?.value || 1) * 100)
})
// 阅读位置历史由 viewer 自己记账：这里只读它的 canBack / canForward 控制置灰。
// 直接读 ref（不加 tick）—— history 是 view-state 之外的独立状态，栈变化时 computed 会自己重算。
const viewerCanReadBack = computed(() => !!pdfViewer.value?.state?.canReadBack?.value)
const viewerCanReadForward = computed(() => !!pdfViewer.value?.state?.canReadForward?.value)
// 顶部工具栏操作：全部转发给 viewer 暴露的方法，并刷新镜像。
const viewerZoomIn = () => { pdfViewer.value?.zoomIn(); bindViewerState() }
const viewerZoomOut = () => { pdfViewer.value?.zoomOut(); bindViewerState() }
const viewerFitWidth = () => { pdfViewer.value?.fitWidth(); bindViewerState() }
const viewerRotateLeft = () => { pdfViewer.value?.rotateLeft(); bindViewerState() }
const viewerRotateRight = () => { pdfViewer.value?.rotateRight(); bindViewerState() }
const viewerReadBack = () => { pdfViewer.value?.goReadingBack?.() }
const viewerReadForward = () => { pdfViewer.value?.goReadingForward?.() }

const onViewerReady = ({ totalPages: tp } = {}) => {
  if (tp) totalPages.value = tp
  bindViewerState()
  // 文档就绪后再列高亮：pdf_list_highlights 要求该 PDF 已在本进程内打开。
  void refreshAllHighlights()
  if (pendingRestorePage.value != null && tp) {
    const p = Math.max(1, Math.min(Number(pendingRestorePage.value) || 1, tp))
    pendingRestorePage.value = null
    if (p !== 1) nextTick(() => pdfViewer.value?.goToPage(p))
  }
}
const onViewerPageChange = (n) => {
  const p = Number(n) || 1
  if (p === currentPage.value) return
  currentPage.value = p
  jumpPage.value = p
  scheduleLoadMarkdownPage(p)
}

// 连续滚动时 onViewerPageChange 是逐页触发的：不防抖的话，快速划过 20 页就要跑 20 次
// load_markdown_content + 20 次 markdown 转换与重渲染，右侧面板一路白闪。
// 只有这条滚动路径防抖；离散的翻页/跳转/大纲定位仍直接调用 loadMarkdownPage 立即加载。
const MD_LOAD_DEBOUNCE_MS = 140
let mdLoadTimer = null
const scheduleLoadMarkdownPage = (pageNum) => {
  if (mdLoadTimer) clearTimeout(mdLoadTimer)
  mdLoadTimer = setTimeout(() => {
    mdLoadTimer = null
    loadMarkdownPage(pageNum)
  }, MD_LOAD_DEBOUNCE_MS)
}

// ─────────── 阅读位置历史 ───────────
// 已迁进 GuaxiPdfViewer：位置、滚动几何、Alt+← / Alt+→ 与置灰状态都由 viewer 自己维护，
// 宿主再存一份只会和它各走一路（此前栈几乎永远是空的，按钮与快捷键都没反应）。
// 本组件只保留工具栏按钮的转发（见 viewerReadBack / viewerReadForward）。
const onViewerOutline = (list) => {
  // 后端 pdf_outline 返回 {title, page, level}，而 GuaxiOutline 的 n-tree 用 text 作标签、
  // 用 line 做跳转键。这里做字段映射，否则大纲有树形和图标却没有文字。
  const mapped = (Array.isArray(list) ? list : []).map((it) => ({
    level: Number(it.level) || 1,
    text: it.title ?? it.text ?? '',
    line: Number(it.page ?? it.line) || 1,
  }))
  outlineStore.setOutline(mapped)
  outlineStore.setJumpToLine((p) => {
    const n = Number(p)
    if (Number.isFinite(n)) goToPage(n)
  })
}
// 新版 viewer 内选中 PDF 文本 -> 复用划词工具栏（翻译/查询/问AI/AI翻译/插入笔记）。
// 旧路径靠 handleSelectionMouseUp(限定容器为旧 PDF.js 文本层)；新 viewer 的文本层在子组件内，
// 故改由 viewer 的 selection 事件驱动，直接用实时 DOM 选区计算位置。
const onViewerSelection = ({ text } = {}) => {
  nextTick(() => {
    const selection = window.getSelection()
    const t = (selection ? selection.toString().trim() : '') || String(text || '').trim()
    if (!t || !selection || selection.rangeCount === 0) { closeSelectionToolbar(); return }
    const rect = selection.getRangeAt(0).getBoundingClientRect()
    if (!rect || (rect.width === 0 && rect.height === 0)) { closeSelectionToolbar(); return }
    selectionText.value = t
    // 顺手记下这次选区在 PDF 里的位置，供随后「插入笔记」当来源锚点用。
    const selInfo = pdfViewer.value?.getSelectionRects?.()
    lastPdfSelectionAnchor.value =
      selInfo?.page && Array.isArray(selInfo.rects) && selInfo.rects.length
        ? { page: selInfo.page, rects: selInfo.rects }
        : null
    const pad = 8
    const toolHeight = SELECTION_TOOLBAR_HEIGHT
    let top = rect.top - toolHeight - 8
    if (top < pad) top = rect.bottom + 8
    const anchorLeft = rect.left
    const clampLeft = (width) => Math.round(
      Math.max(pad, Math.min(anchorLeft, window.innerWidth - width - pad))
    )
    selectionToolbarTop.value = Math.round(top)
    selectionToolbarLeft.value = clampLeft(SELECTION_TOOLBAR_WIDTH)
    selectionToolbarVisible.value = true
    // 元素是刚创建的，再下一帧按实测宽度把右边界夹取修准（常量只兜首帧）
    nextTick(() => {
      const el = selectionToolbarEl.value
      if (el) selectionToolbarLeft.value = clampLeft(el.offsetWidth)
    })
  })
}

// 高亮的选中态。宿主持有它，是因为选中之后还要联动右侧笔记面板（滚到对应摘录卡）。
const selectedHighlight = ref(null)

// ─────────── 全书高亮列表（侧栏「笔记」标签）───────────
//
// viewer 的页模型只按可见页懒加载高亮，拿不到全书清单；而「笔记」标签要回答的是
// 「这本书里我标记了哪些地方」。所以这里单独问后端要全量：pdf_list_highlights 一次扫完
// 所有页的批注，页数再多也只有一次 IPC。新增/删除/撤销后都要重拉，列表才不会落后于文件。
const allHighlights = ref([])
const highlightsLoading = ref(false)
// 列表落后于文件的标记。高亮增删后只置脏；侧栏没开就等它开时再拉，
// 免得每划一条高亮就把整本 PDF 的批注扫一遍。
let highlightsStale = true
// 刷新期间又有新请求（连续加几条高亮）：记一笔，本轮结束后再拉一次，别把后来的改动吞掉。
let highlightRefreshQueued = false
// 换文档时旧请求可能晚到：用 token 丢弃不属于当前文件的结果。
let highlightLoadToken = 0
const normalizeHighlight = (h) => ({
  annotId: String(h?.annot_id ?? h?.annotId ?? ''),
  page: Math.max(1, Number(h?.page) || 1),
  text: String(h?.text || ''),
  note: String(h?.note || ''),
  color: h?.color,
  rects: Array.isArray(h?.rects) ? h.rects.map((r) => ({ ...r })) : [],
})
const refreshAllHighlights = async () => {
  if (!useNewPdfViewer.value || !props.filePath) return
  if (highlightsLoading.value) { highlightRefreshQueued = true; return }
  highlightsLoading.value = true
  const token = ++highlightLoadToken
  try {
    const list = await invoke('pdf_list_highlights', {
      path: String(props.filePath).split('?')[0],
    })
    if (token !== highlightLoadToken) return
    allHighlights.value = (Array.isArray(list) ? list : [])
      .map(normalizeHighlight)
      .filter((h) => h.annotId)
      .sort((a, b) => a.page - b.page)
    highlightsStale = false
  } catch (e) {
    logger.warn('加载高亮列表失败', { error: String(e?.message || e) })
  } finally {
    if (token === highlightLoadToken) highlightsLoading.value = false
    if (highlightRefreshQueued) {
      highlightRefreshQueued = false
      void refreshAllHighlights()
    }
  }
}
const jumpToHighlight = (h) => {
  if (!h?.annotId) return
  selectedHighlight.value = { ...h }
  const p = Number(h.page) || 1
  gotoReadingPage(p)
}

/** 高亮增删后调用：任一面板开着就立刻重拉，都关着则等打开时再拉。 */
const markHighlightsDirty = () => {
  highlightsStale = true
  if (showOutline.value || showNotes.value) void refreshAllHighlights()
}

const clearHighlightSelection = () => {
  if (selectedHighlight.value) selectedHighlight.value = null
}

// 单击 = 选中，Alt+点击 = 删除。
// 以前单击直接删：没有确认，而光标和悬停描边都像"选中"——等于把一个破坏性操作
// 藏在一个看起来无害的手势里，误删只能靠 Ctrl+Z 捞回来。
const onViewerHighlightClick = async ({ annotId, page, note, color, rects, altKey, text } = {}) => {
  if (!annotId) return
  const pg = Number(page) || currentPage.value
  // 删除前已由 viewer 带回 rects/color/note/text，用于撤销时重建。
  const snapshot = {
    page: pg,
    annotId,
    note: note || '',
    color,
    rects: Array.isArray(rects) ? rects : [],
    text: text || '',
  }

  if (!altKey) {
    selectedHighlight.value = snapshot
    scrollNotesToHighlight(snapshot)
    logger.info('[高亮] 选中', { page: pg, annotId })
    return
  }

  const ok = await deleteHighlightBySnapshot(snapshot)
  if (ok) message.info('已删除高亮（Ctrl+Z 可撤销）')
}

// 删除一条高亮并记入撤销栈：Alt+点击和 Backspace 共用这一条路径。
const deleteHighlightBySnapshot = async (snapshot) => {
  if (!snapshot?.annotId) return false
  const ok = await pdfViewer.value?.deleteHighlight(snapshot.page, snapshot.annotId)
  if (!ok) return false
  pushHighlightHistory({ type: 'delete', ...snapshot })
  if (selectedHighlight.value?.annotId === snapshot.annotId) selectedHighlight.value = null
  markHighlightsDirty()
  return true
}

// 删除当前选中的高亮（Backspace / Delete），和 Ctrl+Z 撤销是一对。
const deleteSelectedHighlight = async () => {
  const sel = selectedHighlight.value
  if (!sel) return false
  const ok = await deleteHighlightBySnapshot({ ...sel })
  if (ok) message.info('已删除高亮（Ctrl+Z 可撤销）')
  return ok
}

// ─────────── 高亮撤销/重做历史 ───────────
const highlightUndoStack = ref([])
const highlightRedoStack = ref([])
const canUndoHighlight = computed(() => highlightUndoStack.value.length > 0)
const canRedoHighlight = computed(() => highlightRedoStack.value.length > 0)
const pushHighlightHistory = (entry) => {
  highlightUndoStack.value.push(entry)
  if (highlightUndoStack.value.length > 200) highlightUndoStack.value.shift()
  highlightRedoStack.value = []
}
const undoHighlight = async () => {
  const entry = highlightUndoStack.value.pop()
  if (!entry) { message.info('没有可撤销的高亮操作'); return }
  // 撤销/重做会让 annot_id 变化（重建的高亮由后端重新分配），旧的选中项已经指向不存在的对象。
  clearHighlightSelection()
  if (entry.type === 'add') {
    // 撤销新增 -> 删除
    const ok = await pdfViewer.value?.deleteHighlight(entry.page, entry.annotId)
    if (ok) { highlightRedoStack.value.push(entry); message.info('已撤销高亮') }
    else highlightUndoStack.value.push(entry)
  } else if (entry.type === 'delete') {
    // 撤销删除 -> 重建
    const r = await pdfViewer.value?.reAddHighlight({
      page: entry.page, rects: entry.rects, color: entry.color, note: entry.note, text: entry.text,
    })
    if (r) { highlightRedoStack.value.push({ ...entry, annotId: r.annotId }); message.info('已恢复高亮') }
    else highlightUndoStack.value.push(entry)
  }
  markHighlightsDirty()
}
const redoHighlight = async () => {
  const entry = highlightRedoStack.value.pop()
  if (!entry) { message.info('没有可重做的高亮操作'); return }
  clearHighlightSelection()
  if (entry.type === 'add') {
    const r = await pdfViewer.value?.reAddHighlight({
      page: entry.page, rects: entry.rects, color: entry.color, note: entry.note, text: entry.text,
    })
    if (r) { highlightUndoStack.value.push({ ...entry, annotId: r.annotId }); message.info('已重做高亮') }
    else highlightRedoStack.value.push(entry)
  } else if (entry.type === 'delete') {
    const ok = await pdfViewer.value?.deleteHighlight(entry.page, entry.annotId)
    if (ok) { highlightUndoStack.value.push(entry); message.info('已重做删除') }
    else highlightRedoStack.value.push(entry)
  }
  markHighlightsDirty()
}
// 高亮颜色（RGBA 0-255）+ 预设色板。乐观渲染下选词点高亮即时出现。
const clamp255 = (n) => Math.max(0, Math.min(255, Math.round(Number(n) || 0)))
const highlightColor = ref([255, 235, 59, 110])
/** 划词浮条上的调色盘是否展开：默认收起，高亮图标直接用当前颜色落笔。 */
const highlightPaletteOpen = ref(false)
const highlightPresets = [
  [255, 235, 59, 110],   // 黄
  [76, 217, 100, 110],   // 绿
  [64, 196, 255, 110],   // 蓝
  [255, 145, 180, 110],  // 粉
  [255, 159, 67, 110],   // 橙
]
const rgbaArrToCss = (c) => {
  const [r, g, b, a] = Array.isArray(c) ? c : highlightColor.value
  return `rgba(${clamp255(r)}, ${clamp255(g)}, ${clamp255(b)}, ${(clamp255(a) / 255).toFixed(3)})`
}
const cssToRgbaArr = (str) => {
  const m = String(str || '').match(/rgba?\(([^)]+)\)/i)
  if (m) {
    const p = m[1].split(',').map((s) => s.trim())
    const a = p[3] != null ? clamp255(parseFloat(p[3]) * 255) : 110
    return [clamp255(p[0]), clamp255(p[1]), clamp255(p[2]), a]
  }
  const hex = String(str || '').replace('#', '')
  if (hex.length >= 6) {
    return [
      parseInt(hex.slice(0, 2), 16),
      parseInt(hex.slice(2, 4), 16),
      parseInt(hex.slice(4, 6), 16),
      hex.length >= 8 ? parseInt(hex.slice(6, 8), 16) : 110,
    ]
  }
  return [255, 235, 59, 110]
}
const highlightColorCss = computed(() => rgbaArrToCss(highlightColor.value))

const addHighlightViaViewer = async () => {
  const r = await pdfViewer.value?.addHighlightFromSelection({ color: highlightColor.value })
  highlightPaletteOpen.value = false
  if (r) {
    const snapshot = {
      page: r.page,
      annotId: r.annotId,
      rects: Array.isArray(r.rects) ? r.rects.map((x) => ({ ...x })) : [],
      color: [...highlightColor.value],
      note: '',
      text: r.text || '',
    }
    pushHighlightHistory({ type: 'add', ...snapshot })
    // 新加的高亮立即进入侧栏「笔记」标签，并保持选中：
    // 读者点完荧光笔，列表里就能看到刚划的那一段，而不是要重开文档才出现。
    selectedHighlight.value = snapshot
    markHighlightsDirty()
    message.success('已高亮')
  } else message.info('请先在页面中选择文本')
}

// 4. 视图状态
const zoomLevel = ref(1);
const rotation = ref(0);
const pdfPanel = ref(null);
const pdfCanvas = ref(null);
const pdfAnnotationLayer = ref(null);
const pdfTextLayer = ref(null);
const isResizing = ref(false);
const startX = ref(0);
const startLeftWidth = ref(0);
const leftPanelWidth = ref('50%');
// 大纲列宽：可拖拽调整。原来写死 260px，且那条"分隔线"只是 border-right，没有任何拖拽处理。
const outlineWidth = ref(260);
const isOutlineResizing = ref(false);
let outlineStartX = 0;
let outlineStartWidth = 0;

const startOutlineResize = (e) => {
  isOutlineResizing.value = true;
  outlineStartX = e.clientX;
  outlineStartWidth = outlineWidth.value;
  document.addEventListener('mousemove', handleOutlineResizeMove);
  document.addEventListener('mouseup', stopOutlineResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

const handleOutlineResizeMove = (e) => {
  if (!isOutlineResizing.value) return;
  const container = document.querySelector('.pdf-main-content');
  // 上限同时受窗口宽度约束，避免把正文挤没
  const maxWidth = Math.min(560, (container?.offsetWidth || window.innerWidth) * 0.6);
  outlineWidth.value = Math.max(160, Math.min(outlineStartWidth + e.clientX - outlineStartX, maxWidth));
};

const stopOutlineResize = () => {
  isOutlineResizing.value = false;
  document.removeEventListener('mousemove', handleOutlineResizeMove);
  document.removeEventListener('mouseup', stopOutlineResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const pdfOutlineLoadedFor = ref('')

const readerStorageKey = computed(() => {
  const baseId = props.importFileId != null && props.importFileId !== 0
    ? `id:${props.importFileId}`
    : `path:${props.filePath || props.fileName || ''}`;
  const safeId = String(baseId).replace(/\\/g, '/');
  return `guaxi_reader_state_v1:${props.library}:${props.fileType}:${safeId}`;
});

const pendingRestorePage = ref(null);
// 朗读断点（第几段）。与页码一起存进阅读状态，重开文档即从这一句继续。
const speechIndex = ref(-1);
const isHydrated = ref(false);

const bookmarks = ref([]);
const totalReadingMs = ref(0);
const completedAt = ref(null);
const completedReadingMs = ref(null);
const sessionStartedAt = ref(0);

const progressPercent = computed(() => {
  if (!totalPages.value) return 0;
  return Math.min(100, Math.max(0, Math.round((currentPage.value / totalPages.value) * 1000) / 10));
});

const formatDuration = (ms) => {
  const v = Number(ms);
  if (!Number.isFinite(v) || v <= 0) return '0s';
  const totalSeconds = Math.floor(v / 1000);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
};

const loadReaderState = () => {
  const normalizePdfAnnotations = (v) => {
    if (!Array.isArray(v)) return [];
    return v
      .map((it) => {
        const id = typeof it?.id === 'string' ? it.id : '';
        const page = Math.max(1, Number(it?.page) || 1);
        const selectionText = typeof it?.selectionText === 'string' ? it.selectionText : '';
        const note = typeof it?.note === 'string' ? it.note : '';

        const ax = Number(it?.anchor?.x);
        const ay = Number(it?.anchor?.y);
        const anchor = Number.isFinite(ax) && Number.isFinite(ay)
          ? { x: Math.max(0, Math.min(1, ax)), y: Math.max(0, Math.min(1, ay)) }
          : null;

        const color = typeof it?.color === 'string' && it.color.trim()
          ? it.color.trim()
          : 'rgba(64, 196, 255, 0.32)';

        const rects = Array.isArray(it?.rects)
          ? it.rects
            .map((r) => {
              const x = Number(r?.x);
              const y = Number(r?.y);
              const w = Number(r?.w);
              const h = Number(r?.h);
              if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(w) || !Number.isFinite(h)) return null;
              return {
                x: Math.max(0, Math.min(1, x)),
                y: Math.max(0, Math.min(1, y)),
                w: Math.max(0, Math.min(1, w)),
                h: Math.max(0, Math.min(1, h)),
              };
            })
            .filter(Boolean)
            .filter((r) => r.w > 0.002 && r.h > 0.002)
            .slice(0, 200)
          : [];

        const editorId = typeof it?.editorId === 'string' ? it.editorId : '';
        const noteEditorId = typeof it?.noteEditorId === 'string' ? it.noteEditorId : '';

        if (!id || !selectionText) return null;
        return {
          id,
          page,
          selectionText: selectionText.slice(0, 800),
          note: note.slice(0, 2000),
          anchor,
          rects,
          color,
          editorId,
          noteEditorId,
          editing: false,
        };
      })
      .filter(Boolean);
  };

  try {
    const raw = localStorage.getItem(readerStorageKey.value);
    if (!raw) {
      return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, pdfAnnotations: [], speechIndex: -1 };
    }
    const parsed = JSON.parse(raw);
    return {
      lastPage: Math.max(1, Number(parsed.lastPage) || 1),
      bookmarks: Array.isArray(parsed.bookmarks)
        ? parsed.bookmarks
          .map((n) => Number(n))
          .filter((n) => Number.isFinite(n) && n > 0)
        : [],
      totalReadingMs: Math.max(0, Number(parsed.totalReadingMs) || 0),
      completedAt: parsed.completedAt || null,
      completedReadingMs: parsed.completedReadingMs == null ? null : Math.max(0, Number(parsed.completedReadingMs) || 0),
      pdfAnnotations: normalizePdfAnnotations(parsed.pdfAnnotations),
      // 0 是合法段号，不能用 `||` 兜底（会被当成假值丢掉）。
      speechIndex: Number.isFinite(Number(parsed.speechIndex)) ? Math.max(-1, Math.trunc(Number(parsed.speechIndex))) : -1,
    };
  } catch {
    return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, pdfAnnotations: [] };
  }
};

const persistReaderState = () => {
  try {
    localStorage.setItem(
      readerStorageKey.value,
      JSON.stringify({
        v: 1,
        lastPage: currentPage.value,
        bookmarks: [...new Set(bookmarks.value)].sort((a, b) => a - b),
        totalReadingMs: totalReadingMs.value,
        completedAt: completedAt.value,
        completedReadingMs: completedReadingMs.value,
        pdfAnnotations: Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [],
        speechIndex: speechIndex.value,
      })
    );
  } catch {
  }
};

const startReadingSession = () => {
  sessionStartedAt.value = Date.now();
};

const endReadingSession = () => {
  if (!sessionStartedAt.value) return;
  const elapsed = Math.max(0, Date.now() - sessionStartedAt.value);
  totalReadingMs.value += elapsed;
  sessionStartedAt.value = 0;
};

const markCompletedIfNeeded = () => {
  if (completedAt.value) return;
  if (!totalPages.value) return;
  if (currentPage.value < totalPages.value) return;
  completedAt.value = new Date().toISOString();
  const elapsed = sessionStartedAt.value ? Math.max(0, Date.now() - sessionStartedAt.value) : 0;
  completedReadingMs.value = totalReadingMs.value + elapsed;
};

const isBookmarkedCurrent = computed(() => bookmarks.value.includes(currentPage.value));

const toggleBookmarkCurrentPage = () => {
  const p = currentPage.value;
  if (!p) return;
  const i = bookmarks.value.indexOf(p);
  if (i >= 0) {
    bookmarks.value.splice(i, 1);
  } else {
    bookmarks.value.push(p);
  }
  persistReaderState();
};

const renderBookmarkOptionIcon = (iconComp) => () =>
  h(NIcon, null, { default: () => h(iconComp) });

const bookmarkOptions = computed(() => {
  const opts = [
    {
      label: isBookmarkedCurrent.value
        ? `取消书签（第 ${currentPage.value} 页）`
        : `添加书签（第 ${currentPage.value} 页）`,
      key: 'toggle-current',
      icon: renderBookmarkOptionIcon(BookmarksOutline)
    }
  ];

  const sorted = [...new Set(bookmarks.value)].sort((a, b) => a - b);
  if (sorted.length) {
    opts.push({
      label: '书签列表',
      key: 'bookmarks-title',
      disabled: true,
      icon: renderBookmarkOptionIcon(BookmarksOutline)
    });
    sorted.forEach((p) => {
      opts.push({
        label: `跳转到第 ${p} 页`,
        key: `page:${p}`,
        icon: renderBookmarkOptionIcon(BookmarkOutline)
      });
    });
    opts.push({ label: '清空书签', key: 'clear', icon: renderBookmarkOptionIcon(TrashOutline) });
  }

  return opts;
});

const handleBookmarkSelect = (key) => {
  if (key === 'toggle-current') return toggleBookmarkCurrentPage();
  if (key === 'clear') {
    bookmarks.value = [];
    persistReaderState();
    return;
  }
  if (typeof key === 'string' && key.startsWith('page:')) {
    const p = Number(key.slice('page:'.length));
    if (Number.isFinite(p)) goToPage(p);
  }
};

const elements = ref([])
const viewMode = ref('split')
const previousLeftWidth = ref(leftPanelWidth.value)
const showOutline = ref(false)

/**
 * 左侧标签侧栏：大纲 / 书签 / 笔记。
 *
 * 三者都是「在书里找一处并跳过去」，所以共用一列、靠标签切换；
 * 数字角标让读者不打开也知道有多少条。
 */
const sideTab = ref('outline')
const sortedBookmarks = computed(() => [...new Set(bookmarks.value)].sort((a, b) => a - b))
const sideTabs = computed(() => [
  { key: 'outline', label: '大纲' },
  { key: 'bookmarks', label: '书签', count: sortedBookmarks.value.length },
  { key: 'notes', label: '笔记', count: allHighlights.value.length },
])

// 侧栏打开、或切到「笔记」标签时，如果列表落后于文件就补一次。
watch([showOutline, sideTab], ([open, tab]) => {
  if (!open) return
  if (tab === 'notes' || highlightsStale) void refreshAllHighlights()
})

const viewModeOptions = [
  { label: '左右分栏', key: 'split' },
  { label: '仅左侧', key: 'full-left' },
  { label: '仅右侧', key: 'full-right' },
  { label: '大纲', key: 'outline' }
]

const handleViewModeSelect = (key) => {
  if (key === 'outline') {
    showOutline.value = !showOutline.value
  } else {
    setViewMode(key)
  }
}

// 5. Markdown相关
const markdownContent = ref('')
const markdownLines = ref([]);
const markdownFilePath = ref('')

function generateOutlineFromMarkdown(content) {
  if (!content) {
    outlineStore.setOutline([]);
    return;
  }
  // Python Cell 使用 Markdown 注释哨兵存储，生成 Reader 大纲前先屏蔽源码区，避免 Python `#` 注释被当作标题。
  const outlineSource = maskPythonCellBlocksForMarkdown(content);
  const tokens = md.parse(outlineSource, {});
  const result = [];
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1));
      const nextToken = tokens[idx + 1];
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: result.length
        });
      }
    }
  });
  outlineStore.setOutline(result);
}

function jumpToHeading(index) {
  if (!markdownContainer.value) return;
  const headings = markdownContainer.value.querySelectorAll('h1, h2, h3, h4, h5, h6');
  const target = headings[index];
  if (target) {
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

// 6. 笔记相关
const showNotes = ref(false)
// 右侧「我的笔记」打开时，如果高亮列表落后于文件就补一次。
watch(showNotes, (open) => {
  if (open && highlightsStale) void refreshAllHighlights()
})
const notesContent = ref('')
const showNoteBubble = ref(false)
const isFitting = false
const noteCards = ref([])
const activeCardIndex = ref(-1)
const isNotesReady = ref(false)
const notesFilePath = ref('')

// 7. 拖拽和动画相关
const isDragging = ref(false)
const startDragPos = ref({ x: 0, y: 0 })
const startScrollPos = ref({ left: 0, top: 0 })
const velocity = ref({ x: 0, y: 0 })
const lastDragTime = ref(0)
const animationFrameId = ref(null)
const isDecelerating = ref(false)
let dragGlobalListenersActive = false

// 8. 翻译相关
const showTranslation = ref(false)
const isTranslating = ref(false)

// 9. Alt键收集器状态
const altKeyCollector = ref({
  isActive: false,
  currentCard: null,
  timer: null
})

// 10. 工具函数
async function processImages(content, basePath = '') {
  const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
  const replacements = [];

  for (const match of content.matchAll(imgRegex)) {
    const [full, alt, src] = match;
    if (!src.match(/^(http|\/|data:image)/i)) {
      try {
        const fullPath = await join(basePath, src);
        const safeSrc = await convertFileSrc(fullPath);
        replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
      } catch (error) {
        logger.warn(`路径转换失败: ${src}`, error);
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

// 渲染笔记内容为HTML
const renderNoteContent = async (markdown) => {
  if (!markdown) return '';
  try {
    const imageDir = await getImagesDirectory();
    const processedContent = await processImages(markdown, imageDir);
    return await renderMarkdown(processedContent, {
      preserveOriginal: false,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      buttons: false,
      theme: resolveMarkdownTheme()
    });
  } catch (e) {
    return `<div class="render-error">${e.message}</div>`;
  }
};

async function convertPdfRenderMarkdown(content, options = {}) {
  const processedContent = await processImages(content, options.libraryBasePath);
  try {
    const theme = resolveMarkdownTheme()
    let html = await renderMarkdown(processedContent, {
      preserveOriginal: true,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme
    });

    nextTick(() => {
      if (markdownContainer.value) {
        addCopyButtons(markdownContainer.value, { theme });
        addMediaViewerButtons(markdownContainer.value, { theme });
      }
    });

    // 创建临时DOM元素来解析HTML
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = html;

    // 递归遍历DOM节点并拆分
    const splitByNodes = (node) => {
      const result = [];

      // 处理文本节点
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent.trim();
        if (text) {
          result.push({
            raw: text,
            html: text,
            // 新增：默认不跳过翻译
            skipTranslation: false
          });
        }
        return result;
      }

      // 处理元素节点
      if (node.nodeType === Node.ELEMENT_NODE) {
        // 特殊处理这些元素，保持其完整性
        const keepWholeTags = ['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
          'img', 'table', 'pre', 'code', 'div.katex-display',
          'span.katex-inline', 'div.mermaid', 'svg.mermaid',
          // 添加MathJax相关标签
          'mjx-container', 'mjx-math', 'mjx-assistive-mml',
          // 修改代码高亮相关标签 - 移除无效选择器
          'div.hljs', 'code.hljs',
          // 添加所有hljs开头的类名元素
          '[class^="hljs-"]', '[class*=" hljs-"]'];

        if (keepWholeTags.some(tag => node.matches(tag))) {
          const outerHTML = node.outerHTML;

          // 新增：判断是否需要跳过翻译
          const skipTranslation = shouldSkipTranslation(node);

          result.push({
            raw: outerHTML,
            html: outerHTML,
            // 新增：标记是否需要跳过翻译
            skipTranslation
          });
          return result;
        }

        // 递归处理子节点
        for (const child of node.childNodes) {
          result.push(...splitByNodes(child));
        }
        return result;
      }

      return result;
    };

    // 从临时div的子节点开始处理
    const lines = [];
    for (const child of tempDiv.childNodes) {
      lines.push(...splitByNodes(child));
    }

    return lines;
  } catch (error) {
    logger.error('[ERROR] 渲染失败:', error);
    return [{
      raw: content,
      html: `<div class="render-error">${error.message}</div>`,
      // 新增：错误内容也不翻译
      skipTranslation: true
    }];
  }
}

// 新增：判断节点是否需要跳过翻译
function shouldSkipTranslation(node) {
  // 首先检查 node 是否有效
  if (!node || typeof node.matches !== 'function') {
    return false;
  }

  try {
    // 1. 图片元素
    if (node.tagName === 'IMG') {
      return true;
    }

    // 2. 数学公式相关元素
    if (node.classList && (
      node.classList.contains('katex-display') ||
      node.classList.contains('katex-inline') ||
      node.tagName === 'MJX-CONTAINER' ||
      node.tagName === 'MJX-MATH'
    )) {
      return true;
    }

    // 3. 图表相关元素
    if (node.classList && (
      node.classList.contains('mermaid') ||
      (node.tagName === 'SVG' && node.classList.contains('mermaid'))
    )) {
      return true;
    }

    // 4. 代码块
    if (['PRE', 'CODE'].includes(node.tagName) ||
      (node.classList && (
        node.classList.contains('hljs') ||
        Array.from(node.classList).some(cls => cls.startsWith('hljs-'))
      ))) {
      return true;
    }

    // 5. 表格
    if (node.tagName === 'TABLE') {
      return true;
    }

    return false;
  } catch (error) {
    logger.error('Error checking node:', error);
    return false;
  }
}

// 添加安全编码函数
function safeEncode(content) {
  if (!content) return '';

  // 首先确保是字符串
  content = String(content);

  // 替换保留字符
  return encodeURIComponent(content)
    .replace(/'/g, "%27")     // 单引号
    .replace(/"/g, "%22")     // 双引号
    .replace(/</g, "%3C")     // 小于号
    .replace(/>/g, "%3E")     // 大于号
    .replace(/&/g, "%26")     // &符号
    .replace(/\//g, "%2F")    // 斜杠
    .replace(/\\/g, "%5C")    // 反斜杠
    .replace(/\$/g, "%24")    // $符号（数学公式重要字符）
    .replace(/{/g, "%7B")     // 左大括号
    .replace(/}/g, "%7D")     // 右大括号
    .replace(/\^/g, "%5E")    // 脱字符
    .replace(/_/g, "%5F");    // 下划线
}

// 11. 笔记相关函数
function determineNoteType(target) {
  logger.info('determineNoteType called with target:', target);

  // 1. 优先检查数学公式
  if (target.closest('mjx-container, mjx-math, .math-inline, .math-block')) {
    return 'math';
  }

  // 2. 检查Mermaid图表
  const mermaidContainer = target.closest('.mermaid, [data-processed="true"]');
  if (mermaidContainer) {
    // 确认是Mermaid而非普通代码块
    if (mermaidContainer.querySelector('svg') || mermaidContainer.classList.contains('mermaid')) {
      return 'diagram';
    }
  }

  // 3. 检查代码块
  const pre = target.closest('pre');
  if (pre) {
    // 排除图表
    if (!pre.classList.contains('mermaid')) {
      // 检测标准代码结构或高亮类
      if (pre.querySelector('code') ||
        [...pre.classList].some(c => ['hovered', 'hljs', 'code'].includes(c))) {
        return 'code';
      }
    }
  }

  // 4. 检查图片
  const el = target instanceof Element ? target : target?.parentElement
  if (el && (el.tagName === 'IMG' || el.closest('img') || el.querySelector('img'))) {
    return 'image';
  }

  // 默认文本类型
  return 'text';
}

async function processNoteImage(imgElement) {
  try {
    // 解码URL并提取实际路径
    const rawSrc = imgElement.src;
    // logger.info('原始图片路径：', rawSrc);
    let filePath = '';

    if (rawSrc.startsWith('http://asset.localhost/')) {
      filePath = decodeURIComponent(rawSrc.replace('http://asset.localhost/', ''));
      // logger.info('处理本地图片路径', rawSrc, filePath);
    } else if (rawSrc.startsWith('file://')) {
      filePath = decodeURIComponent(new URL(rawSrc).pathname);
      // logger.info('处理file协议图片路径', rawSrc, filePath);
    } else {
      // logger.warn('非本地图片，无需处理:', rawSrc);
      return rawSrc; // 非本地图片直接返回原URL
    }

    const osPlatform = await platform();
    // 标准化Windows路径
    if (osPlatform === 'win32') {
      filePath = filePath.replace(/^\/([a-zA-Z]:)/, '$1');
    }

    // 准备目标路径
    const imageDir = await getImagesDirectory();
    const ext = filePath.split('.').pop().toLowerCase();
    const validExts = ['jpg', 'jpeg', 'png', 'gif', 'webp'];
    const finalExt = validExts.includes(ext) ? ext : 'png';
    const fileName = `note-img-${Date.now()}.${finalExt}`;

    // logger.info('准备复制图片:', {
    //   srcPath: filePath,
    //   destDir: imageDir,
    //   fileName: fileName
    // });

    // 调用后端复制图片
    const relativePath = await invoke('copy_image_file', {
      srcPath: filePath,
      destDir: imageDir,
      fileName: fileName
    });

    return `../images/${relativePath}`;
  } catch (error) {
    logger.error('图片处理失败:', error);
    return imgElement.src; // 失败时返回原URL
  }
}

// 在handleMarkdownClick函数中添加获取原始Markdown的逻辑
// ───────────────────────── 逐句朗读 ─────────────────────────
// 朗读对象是**已转换的 markdown 面板**，不是 PDF 文字层：PDF 的文字层跨行跨页，
// 断句质量不可控、高亮也只能一格一格跳。markdown 是重排过的连续正文，读起来才对。
// 高亮、滚动跟随、点击定位的接线收在 useReaderReadAloud 里，五个阅读器共用。
const readAloud = useReaderReadAloud({
  // 正文与滚动容器都"用时现取"：mount 时内容可能还没渲染出来
  getContext: () => ({
    root: markdownContainer.value,
    scrollRoot: markdownPanel.value,
    win: window,
  }),
  getIdentity: () => ({ library: props.library || '', fileName: props.fileName || '' }),
  speechIndex,
  onIndexChange: () => persistReaderState(),
  message,
  ownerLabel: 'GuaxiConvertPDFReader',
})

const readAloudPlaying = readAloud.isPlaying
const readAloudBuffering = readAloud.isBuffering
const readAloudTitle = readAloud.title
const toggleReadAloud = () => readAloud.toggle()
const seekReadAloudAtPoint = (event) => readAloud.seekAtPoint(event)

onBeforeUnmount(() => {
  readAloud.clearHighlight()
})

const handleMarkdownClickAlt = async (e) => {
  // 朗读进行中：点正文任意位置就从那一句开始读（"点哪读哪"，不用记快捷键）
  if ((readAloudPlaying.value || readAloudBuffering.value) && seekReadAloudAtPoint(e)) {
    return
  }
  if (!e?.altKey) return
  return handleMarkdownClick(e)
}

const handleMarkdownClick = async (e) => {
  // 1. 阻止默认行为（防止Alt+Click触发浏览器默认动作）
  e.preventDefault();

  // 2. 获取关键状态
  const isAltPressed = e.altKey;
  const rawTarget = e.target;
  const hoveredElement = document.querySelector('.markdown-content .hovered');
  const target = hoveredElement && rawTarget instanceof Node && hoveredElement.contains(rawTarget)
    ? hoveredElement
    : rawTarget;
  const selection = window.getSelection();
  const selectedText = selection ? selection.toString().trim() : '';

  // 3. Alt键特殊处理（不再提前return）
  if (isAltPressed && !altKeyCollector.value.isActive) {
    startAltKeyCollection();
    // 继续执行后续逻辑，不return
  }

  // 4. 清除之前选中的元素
  document.querySelectorAll('.markdown-content .selected').forEach(el => {
    el.classList.remove('selected');
  });

  // 5. 元素类型判断
  let selectedElement = null;
  let message = '';
  let noteContent = '';
  let originalMarkdown = ''; // 新增：存储原始Markdown

  // 处理翻译内容（如果点击在翻译区域内）
  const translatedElement = target.closest('.translated-content');
  if (translatedElement) {
    // 获取最内层的div（实际包含翻译内容的元素）
    const contentDiv = translatedElement.querySelector('div');
    if (contentDiv) {
      selectedElement = translatedElement;
      noteContent = contentDiv.innerHTML; // 获取翻译内容的HTML
      message = '已选中译文';
      
      // 如果需要纯文本内容，可以使用:
      noteContent = contentDiv.textContent;
      originalMarkdown = noteContent;
    }
  }
  // 图片处理（现在支持Alt+Click）
  else if (target instanceof Element && (target.tagName === 'IMG' || target.closest('img') || target.querySelector('img'))) {
    const imgElement = target.tagName === 'IMG'
      ? target
      : (target.closest('img') || target.querySelector('img'));

    selectedElement = imgElement;
    try {
      const processedSrc = await processNoteImage(imgElement);
      message = '已选中图片';
      noteContent = `![图片](${processedSrc})`;
      originalMarkdown = noteContent; // 图片的原始Markdown就是noteContent
    } catch (error) {
      logger.error('图片处理失败:', error);
      message = '图片处理失败，使用原始链接';
      noteContent = `![图片](${imgElement.src})`;
      originalMarkdown = noteContent;
    }
  }
  // 代码块
  else if (target.closest('pre, code, .code-block')) {
    const codeBlock = target.closest('pre, code, .code-block');
    selectedElement = codeBlock;

    // 克隆代码块节点以避免修改原始DOM
    const clonedBlock = codeBlock.cloneNode(true);

    const btn = clonedBlock.querySelector('.copy-button');
    if (btn) btn.remove();

    // 获取处理后的代码内容
    const codeContent = clonedBlock.textContent || '';
    message = `已选中代码块: ${codeContent.substring(0, 30)}${codeContent.length > 30 ? '...' : ''}`;
    logger.info('选中的代码块:', codeBlock);

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = codeBlock.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`\n${codeContent}\n\`\`\``;
    logger.info('代码块原始Markdown:', originalMarkdown);

    originalMarkdown = decodeURIComponent(originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 数学公式
  else if (target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered')) {
    // 获取最接近的数学元素（可能是容器或SVG）
    const mathElement = target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered');
    selectedElement = mathElement;

    // 调试信息
    // logger.info('点击的数学元素:', mathElement);

    // 查找原始LaTeX内容（按优先级）
    let original = mathElement.getAttribute('data-md-original');
    if (!original && mathElement.closest('[data-md-original]')) {
      original = mathElement.closest('[data-md-original]').getAttribute('data-md-original');
    }
    if (!original && mathElement.querySelector('[data-md-original]')) {
      original = mathElement.querySelector('[data-md-original]').getAttribute('data-md-original');
    }

    // 安全解码原始内容
    let decodedOriginal = original;
    try {
      if (decodedOriginal) {
        decodedOriginal = decodeURIComponent(decodedOriginal);
      } else {
        logger.warn('未找到data-md-original属性，尝试从元素内容重建');
        const isBlock = mathElement.classList.contains('math-block') ||
          mathElement.closest('.math-block') ||
          mathElement.tagName === 'MJX-BLOCK';
        const content = mathElement.textContent || '';
        decodedOriginal = isBlock ? `$$${content}$$` : `$${content}$`;
      }
    } catch (err) {
      logger.warn('解码公式失败:', err);
      decodedOriginal = original || '无法获取公式内容';
    }

    // 构建笔记数据
    noteContent = decodedOriginal;
    originalMarkdown = decodedOriginal;

    // 确定是否为块级公式
    const isBlockFormula = decodedOriginal.startsWith('$$') &&
      decodedOriginal.endsWith('$$') &&
      decodedOriginal.length > 4;

    message = `已选中${isBlockFormula ? '块级' : '行内'}公式: ${decodedOriginal.substring(0, 30)}${decodedOriginal.length > 30 ? '...' : ''}`;

    // logger.info('数学公式处理结果:', {
    // noteContent,
    // originalMarkdown,
    // isBlockFormula
    // });
  }
  // mermaid图表
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;

    // 解码原始Markdown
    originalMarkdown = decodeURIComponent(originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 文本选择
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      selectedText;

    // 解码原始Markdown
    originalMarkdown = decodeURIComponent(originalMarkdown);
    logger.info('原始Markdown:', originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 段落点击
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;

      // 获取原始Markdown（从data-md-original属性）
      originalMarkdown = paragraph.getAttribute('data-md-original') ||
        textContent;

      logger.info('段落原始Markdown:', originalMarkdown);

      // 解码原始Markdown
      originalMarkdown = decodeURIComponent(originalMarkdown);

      noteContent = originalMarkdown;
    }
  }

  // 6. 应用选中效果和笔记
  if (selectedElement) {
    const hasContent = (originalMarkdown || noteContent || '').trim();
    if (!hasContent) {
      showSelectionMessage('未选中有效内容');
      return;
    }

    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const noteType = determineNoteType(target);
    const noteData = {
      type: noteType,
      content: noteContent, // 使用原始Markdown作为内容
      originalMarkdown: originalMarkdown, // 新增：存储原始Markdown
      // 右侧 markdown 面板显示的正是当前页的正文，所以"当前页"就是这个摘录的来源位置。
      anchor: { page: Number(currentPage.value) || 1 }
    };

    logger.info('确定的笔记类型:', noteType);
    logger.info('笔记数据:', noteData);

    // 7. 区分复合笔记和新建笔记
    if (altKeyCollector.value.isActive) {
      appendToCurrentNote(noteData);
    } else {
      // 新建笔记
      await addToNotes(noteData);
    }
  }

  // 8. Alt键释放检测
  if (!isAltPressed && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

// 开始Alt键收集模式
const startAltKeyCollection = () => {
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  altKeyCollector.value.isActive = true;

  // 创建新的复合笔记卡片
  const newCard = {
    id: Date.now().toString(),
    type: 'composite',
    title: '复合笔记 (0项)',
    content: '',
    items: [], // 初始化空数组
    timestamp: new Date().toISOString()
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = -1;
  altKeyCollector.value.currentCard = newCard;

  // 设置30秒后自动结束收集
  altKeyCollector.value.timer = setTimeout(() => {
    if (altKeyCollector.value.isActive) endAltKeyCollection();
  }, 30000);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
  });

  message.info('已进入多选模式, 松开Alt键结束收集', { duration: 3000 });
};

// 结束Alt键收集模式
const endAltKeyCollection = () => {
  if (!altKeyCollector.value.isActive) return;

  // 清除定时器
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  // 如果有收集内容，则添加到笔记
  if (altKeyCollector.value.currentCard &&
    (altKeyCollector.value.currentCard.content || altKeyCollector.value.currentCard.originalMarkdown)) {
    // 确保卡片已经存在于noteCards中（因为我们在start时已经添加）
    const existingIndex = noteCards.value.findIndex(card => card.id === altKeyCollector.value.currentCard.id);
    if (existingIndex === -1) {
      noteCards.value.push(altKeyCollector.value.currentCard);
    }
    activeCardIndex.value = -1;
    updateBackendNotes('add', altKeyCollector.value.currentCard);

    nextTick(() => {
      const cards = document.querySelectorAll('.n-card');
      if (cards.length > 0) {
        cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
      }
    });
  }

  // 重置状态
  altKeyCollector.value.isActive = false;
  altKeyCollector.value.currentCard = null;

  message.success('多选模式已结束', { duration: 2000 });
};

/**
 * 解析复合笔记内容 - 直接从items数组中获取内容
 * @param {Array} items 笔记条目数组
 * @returns {Promise<Array>} 解析后的笔记片段数组
 */
async function parseCompositeContent(items) {
  if (!items || !Array.isArray(items)) {
    logger.warn('无效的items参数:', items);
    return [];
  }

  try {
    // 并行处理所有items
    const segments = await Promise.all(
      items.map(async (item, index) => {
        // 使用item中的内容，优先使用original_markdown
        const content = item.original_markdown || item.originalMarkdown || item.content || '';

        // 渲染Markdown为HTML
        const html = await renderNoteContent(content);

        return {
          id: item.id || `${Date.now()}-${index}`,
          type: item.type || 'text', // 默认类型为text
          content: html,
          originalMarkdown: content,
          timestamp: item.timestamp || new Date().toISOString()
        };
      })
    );

    return segments;
  } catch (error) {
    logger.error('解析复合笔记失败:', error);
    return [{
      type: 'error',
      content: `<div class="parse-error">解析失败: ${error.message}</div>`,
      originalMarkdown: '解析失败',
      timestamp: new Date().toISOString()
    }];
  }
}

// 获取片段样式类
const getSegmentClass = (segment) => {
  switch (segment.type) {
    case 'code': return 'code-segment';
    case 'image': return 'image-segment';
    case 'diagram': return 'diagram-segment';
    case 'math': return 'math-segment';
    default: return 'text-segment';
  }
};

const appendToCurrentNote = (element) => {
  if (!altKeyCollector.value.isActive || !altKeyCollector.value.currentCard) return;

  const currentCard = altKeyCollector.value.currentCard;

  // 确保items数组存在
  if (!currentCard.items) {
    currentCard.items = [];
  }

  // 创建新条目
  const newItem = {
    id: Date.now().toString(),
    type: element.type,
    content: element.content || element.originalMarkdown,
    originalMarkdown: element.originalMarkdown || element.content,
    original_markdown: element.originalMarkdown || element.content,
    timestamp: new Date().toISOString(),
    // 复合笔记的每条各记各的来源位置：长按 Alt 连续收集时，几条很可能来自不同页。
    anchor: element.anchor || null
  };

  // 添加到items数组
  currentCard.items.push(newItem);

  // 更新卡片内容（将所有条目内容合并，带编号）
  currentCard.content = currentCard.items.map((item, index) => {
    return `${index + 1}. ${item.originalMarkdown || item.content}`;
  }).join('\n\n');

  // 更新卡片类型和标题
  currentCard.type = 'composite';
  currentCard.title = `复合笔记 (${currentCard.items.length}项)`;
  currentCard.timestamp = new Date().toISOString();

  // 触发响应式更新
  noteCards.value = [...noteCards.value];

  // 显示追加成功的消息
  message.success(`已添加第 ${currentCard.items.length} 条内容`, { duration: 1500 });
};

// 15. UI辅助函数
const showSelectionMessage = (message) => {
  // 移除旧的消息
  const oldMessage = document.querySelector('.selection-message');
  if (oldMessage) oldMessage.remove();

  const messageEl = document.createElement('div');
  messageEl.className = 'selection-message';
  messageEl.textContent = message;
  document.body.appendChild(messageEl);

  setTimeout(() => {
    messageEl.classList.add('fade-out');
    setTimeout(() => messageEl.remove(), 500);
  }, 1500);
}

const closeSelectionToolbar = () => {
  selectionToolbarVisible.value = false
  selectionText.value = ''
  selectionInsertText.value = ''
  highlightPaletteOpen.value = false
}

/**
 * 在浮条外按下鼠标：收起浮条并清掉文字选区。
 *
 * 只置 visible=false 是不够的：选手在 PDF 画布的灰色区域或页边按下时，浏览器不会清掉
 * 已有的文字选区，紧接着 mouseup 又会读到同一段文字把浮条浮回来。
 * 高亮矩形的选中态另有处理（见 handleGlobalMouseDown），这里只管文字选区。
 */
const clearSelectionOnOutsideDown = () => {
  window.getSelection?.()?.removeAllRanges()
  closeSelectionToolbar()
}

const normalizeSelectionTextForLanguageTasks = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n。！？.!?;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

const isWordSelection = (text) => {
  const t = String(text || '').trim()
  if (!t) return false
  if (t.length > 48) return false
  if (/\s/.test(t)) return false
  return /^[\p{L}\p{N}'-]+$/u.test(t)
}

const insertIntoActiveNoteEditor = (text) => {
  const idx = activeCardIndex.value
  if (idx < 0) return false

  const el = contentEditables.value[idx]
  if (!el) return false

  const payload = `\n\n${text}\n\n`

  if (el instanceof HTMLTextAreaElement) {
    el.focus()
    const start = typeof el.selectionStart === 'number' ? el.selectionStart : el.value.length
    const end = typeof el.selectionEnd === 'number' ? el.selectionEnd : el.value.length
    el.value = el.value.slice(0, start) + payload + el.value.slice(end)
    const cursor = start + payload.length
    el.setSelectionRange(cursor, cursor)
    return true
  }

  if (el instanceof HTMLElement && el.isContentEditable) {
    el.focus()
    const selection = window.getSelection()
    if (!selection) return false

    let range = selection.rangeCount ? selection.getRangeAt(0) : null
    if (!range || !el.contains(range.commonAncestorContainer)) {
      range = document.createRange()
      range.selectNodeContents(el)
      range.collapse(false)
      selection.removeAllRanges()
      selection.addRange(range)
    }

    range.deleteContents()
    const node = document.createTextNode(payload)
    range.insertNode(node)
    range.setStartAfter(node)
    range.collapse(true)
    selection.removeAllRanges()
    selection.addRange(range)
    return true
  }

  return false
}

const openSelectionTranslatePanel = async () => {
  const text = selectionText.value.trim()
  if (!text) return

  const pw = window.innerWidth
  const ph = window.innerHeight

  const pad = 8
  const panelW = 360
  const panelH = 260
  const maxLeft = Math.max(pad, pw - panelW - pad)
  const maxTop = Math.max(pad, ph - panelH - pad)

  const baseLeft = Number(selectionToolbarLeft.value)
  const baseTop = Number(selectionToolbarTop.value)

  const nextLeft = Number.isFinite(baseLeft) ? baseLeft : pad
  const nextTop = Number.isFinite(baseTop) ? baseTop + 44 : 52

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, nextLeft), maxLeft),
    top: Math.min(Math.max(pad, nextTop), maxTop)
  }

  selectionTranslatePanelVisible.value = true

  await translateSelectionText(text)
}

const translateSelectionText = async (overrideText) => {
  const text = String((overrideText ?? selectionText.value) || '').trim()
  if (!text) return

  if (selectionTranslateBusy.value) return

  selectionBusy.value = true
  selectionTranslateBusy.value = true
  selectionTranslateRaw.value = text
  selectionTranslateResult.value = ''
  selectionTranslateError.value = ''
  selectionTranslateItems.value = []
  selectionTranslateWordMeta.value = null

  logger.info('[SelectionTranslate][PDFReader] start', { len: text.length, from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  try {
    // 词条模块（音标/短语/例句）和译文互不依赖，这里先并发发出去。
    // 原来是「译文 → 基础词条 → 详细词条」串行三跳，慢的时候要等七八秒。
    const needWordMeta = isWordSelection(text)
    const wordBasicPromise = needWordMeta
      ? invoke('language_learning_enrich_word_basic', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value
      }).catch((err) => {
        logger.warn('[SelectionTranslate][PDFReader] enrich basic failed:', err)
        return null
      })
      : Promise.resolve(null)
    const wordDetailsPromise = needWordMeta
      ? invoke('language_learning_enrich_word_details', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value
      }).catch((err) => {
        logger.warn('[SelectionTranslate][PDFReader] enrich details failed:', err)
        return null
      })
      : Promise.resolve(null)

    let hasUsableMultiResult = false
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value
        })

        const items = Array.isArray(response?.items) ? response.items : []
        selectionTranslateItems.value = items.map((it) => ({
          engine: String(it?.engine || ''),
          success: Boolean(it?.success),
          text: String(it?.text || ''),
          error: it?.error ? String(it.error) : ''
        }))

        const primary = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        if (primary) {
          selectionTranslateResult.value = String(primary.text || '')
          selectionTranslateError.value = ''
          hasUsableMultiResult = true
          logger.info('[SelectionTranslate][PDFReader] multi success', { engines: selectionTranslateItems.value.length })
        } else {
          const err = String(response?.error || '翻译结果为空')
          throw new Error(err)
        }
      } catch (err) {
        const msg = String(err?.message || err || '')
        if (msg.includes('selection_translate_free_multi') && msg.toLowerCase().includes('not found')) {
          selectionTranslateMultiAvailable.value = false
          if (!selectionTranslateMultiNotFoundWarned) {
            selectionTranslateMultiNotFoundWarned = true
            logger.warn('[SelectionTranslate][PDFReader] multi command not found, disabled multi path. Please restart tauri backend.')
          }
        } else {
          logger.warn('[SelectionTranslate][PDFReader] multi failed, fallback to single:', err)
        }
      }
    }

    if (!hasUsableMultiResult) {
      const response = await invoke('translate_text_free', {
        texts: [text],
        from: selectionTranslateFrom.value,
        to: selectionTranslateTo.value
      })

      const translated = (response?.translated_texts?.[0] || '').toString()

      if (!translated.trim() || translated.includes('[Translation Failed]')) {
        const err = String(response?.error || '翻译结果为空')
        throw new Error(err)
      }

      if (!selectionTranslateResult.value) {
        selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
        selectionTranslateResult.value = translated
        selectionTranslateError.value = ''
        logger.info('[SelectionTranslate][PDFReader] single success', { len: translated.length })
      }
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (needWordMeta) {
      try {
        const [basic, details] = await Promise.all([wordBasicPromise, wordDetailsPromise])
        const base = {
          word: String(basic?.word || text).trim(),
          phonetic: String(basic?.phonetic || '').trim(),
          phonetic_uk: String(basic?.phonetic_uk || '').trim(),
          phonetic_us: String(basic?.phonetic_us || '').trim(),
          part_of_speech: String(basic?.part_of_speech || '').trim(),
          meanings: Array.isArray(basic?.meanings) ? basic.meanings : [],
          synonyms: [],
          antonyms: [],
          associates: [],
          synonyms_translations: [],
          antonyms_translations: [],
          associates_translations: [],
          phrases: [],
          phrase_translations: [],
          examples: Array.isArray(basic?.examples) ? basic.examples : [],
          example_translations: [],
        }
        selectionTranslateWordMeta.value = details
          ? {
            ...base,
            synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
            antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
            associates: Array.isArray(details?.associates) ? details.associates : [],
            synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
            antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
            associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
            phrases: Array.isArray(details?.phrases) ? details.phrases : [],
            phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
            examples: Array.isArray(details?.examples) && details.examples.length ? details.examples : base.examples,
            example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
          }
          : base
      } catch (err) {
        logger.warn('[SelectionTranslate][PDFReader] enrich word failed:', err)
      }
    }

    if (finalTranslated) {
      selectionInsertText.value = `**${text}**\n\n${finalTranslated}`
      try {
        const primaryItem = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        await invoke('language_learning_record_translation', {
          input: {
            source_text: text,
            translated_text: finalTranslated,
            source_lang: selectionTranslateFrom.value,
            target_lang: selectionTranslateTo.value,
            engine: String(primaryItem?.engine || 'Free'),
            record_type: isWordSelection(text) ? 'word' : undefined,
            word_meta: selectionTranslateWordMeta.value || undefined,
          }
        })
      } catch (err) {
        logger.warn('[SelectionTranslate][PDFReader] record save failed:', err)
      }
    }
  } catch (err) {
    const msg = String(err?.message || err || '翻译失败')
    selectionTranslateError.value = msg
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: msg }]
    selectionTranslateResult.value = ''
    selectionInsertText.value = ''
    logger.error('[SelectionTranslate][PDFReader] failed:', err)
    message.error(`翻译失败: ${msg}`)
  } finally {
    selectionTranslateBusy.value = false
    selectionBusy.value = false
  }
}

const translateSelection = async () => {
  selectionToolbarVisible.value = false
  await openSelectionTranslatePanel()
}

const querySelection = async () => {
  const text = selectionText.value.trim()
  if (!text) return

  const url = `https://www.bing.com/search?q=${encodeURIComponent(text)}`
  // 同时写入可插入笔记的链接，并在系统默认浏览器中打开搜索
  //（此前只生成链接、点了没反应，现用 plugin-opener 真正打开浏览器）。
  selectionInsertText.value = `[查询：${text}](${url})`
  try {
    await openUrl(url)
  } catch (e) {
    message.warning('打开浏览器失败')
  }
}

const getTranslateLangLabel = (value) => {
  const item = selectionTranslateLangOptions.find((x) => x.value === value)
  return item ? String(item.label || '') : '目标语言'
}

const sendSelectionToAssistant = (overrideText, options = {}) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? selectionText.value)
  if (!text) {
    message.warning('请先框选要发送到助手的文本')
    return false
  }
  const autoSend = options.autoSend !== false
  const disableWebSearch = !!options.disableWebSearch
  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  rightSiderStore.appendAssistantDraft(text, { autoSend, disableWebSearch })
  showAssistant.value = true
  selectionToolbarVisible.value = false
  message.success(options.successMessage || (autoSend ? '已发送到助手并开始提问' : '已插入到助手输入框'))
  return true
}

const askSelectionWithAssistant = () => {
  sendSelectionToAssistant(undefined, { autoSend: true, successMessage: '已发送到助手并开始提问' })
}

const openAssistantTranslateDialog = () => {
  const text = normalizeSelectionTextForLanguageTasks(selectionText.value)
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  assistantTranslateSourceText.value = text
  assistantTranslateDialogVisible.value = true
  selectionToolbarVisible.value = false
}

const sendSelectionAsAiTranslate = () => {
  const text = normalizeSelectionTextForLanguageTasks(assistantTranslateSourceText.value)
  if (!text) {
    assistantTranslateDialogVisible.value = false
    message.warning('请先框选要翻译的文本')
    return
  }
  const lang = String(assistantTranslateTargetLang.value || 'en')
  const label = getTranslateLangLabel(lang)
  const prompt = `请将以下内容翻译成${label}，保持原意并保留术语、代码与格式，只输出译文：\n\n${text}`
  const ok = sendSelectionToAssistant(prompt, {
    autoSend: true,
    disableWebSearch: true,
    successMessage: `已发送到助手并翻译为${label}`,
  })
  if (ok) {
    assistantTranslateDialogVisible.value = false
  }
}

const insertSelectionResult = async () => {
  const text = selectionInsertText.value.trim()
  if (!text) return

  // 带上这次选区的来源位置，笔记卡之后就能"回到原文"。
  // 没有实时选区信息时退化成当前页——至少能跳回那一页。
  const anchor = lastPdfSelectionAnchor.value || { page: Number(currentPage.value) || 1 }
  const ok = insertIntoActiveNoteEditor(text)
  if (!ok) {
    await addToNotes({ type: 'text', content: text, originalMarkdown: text, anchor })
  }

  message.success('已插入笔记', { duration: 1200 })
  closeSelectionToolbar()
}

const handleSelectionMouseUp = () => {
  const selection = window.getSelection()
  const text = selection ? selection.toString().trim() : ''
  if (!text) {
    closeSelectionToolbar()
    return
  }

  const containers = [markdownContainer.value, pdfTextLayer.value, pdfPageContainer.value].filter((x) => x && x instanceof HTMLElement)
  if (!selection || selection.rangeCount === 0 || !containers.length) {
    closeSelectionToolbar()
    return
  }

  const range = selection.getRangeAt(0)
  const common = range.commonAncestorContainer
  const commonEl = common instanceof Element ? common : common.parentElement
  const container = commonEl ? containers.find((x) => x.contains(commonEl)) : null
  if (!container) {
    closeSelectionToolbar()
    return
  }

  const rect = range.getBoundingClientRect()
  if (!rect || (rect.width === 0 && rect.height === 0)) {
    closeSelectionToolbar()
    return
  }

  selectionText.value = text

  const pad = 8
  const toolHeight = SELECTION_TOOLBAR_HEIGHT

  let top = rect.top - toolHeight - 8
  if (top < pad) top = rect.bottom + 8

  const left = rect.left
  selectionToolbarTop.value = Math.round(top)
  selectionToolbarLeft.value = Math.round(
    Math.max(pad, Math.min(left, window.innerWidth - SELECTION_TOOLBAR_WIDTH - pad))
  )
  selectionToolbarVisible.value = true
  // 元素是刚创建的，下一帧按实测宽度把右边界夹取修准（常量只兜首帧）
  nextTick(() => {
    const el = selectionToolbarEl.value
    if (!el) return
    selectionToolbarLeft.value = Math.round(
      Math.max(pad, Math.min(left, window.innerWidth - el.offsetWidth - pad))
    )
  })
}

const handleGlobalMouseDown = (e) => {
  const t = e?.target
  if (!(t instanceof Element)) {
    clearSelectionOnOutsideDown()
    return
  }

  if (t.closest('.selection-toolbar')) return
  if (t.closest('.pot-selection-translate-panel')) return
  // 点在高亮上时先别清选中：那一下正要把它选上（click 在 mousedown 之后才到）。
  if (!t.closest('.gpv__hl-rect')) clearHighlightSelection()
  clearSelectionOnOutsideDown()
}

const updateActivePdfSearchMatch = (nextIndex) => {
  const total = pdfSearchMatches.value.length
  if (!total) return

  const normalized = ((nextIndex % total) + total) % total

  const prev = pdfSearchMatches.value[pdfSearchActiveIndex.value]
  if (prev) prev.classList.remove('inpage-match-active')

  pdfSearchActiveIndex.value = normalized

  const el = pdfSearchMatches.value[normalized]
  if (el) {
    el.classList.add('inpage-match-active')
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' })
  }
}

const gotoNextPdfSearchMatch = () => {
  if (!pdfSearchMatches.value.length) return
  updateActivePdfSearchMatch(pdfSearchActiveIndex.value + 1)
}

const gotoPrevPdfSearchMatch = () => {
  if (!pdfSearchMatches.value.length) return
  updateActivePdfSearchMatch(pdfSearchActiveIndex.value - 1)
}

const updateActivePdfFullSearchResult = (nextIndex) => {
  const total = pdfFullSearchResults.value.length
  if (!total) {
    pdfFullSearchActiveIndex.value = -1
    return
  }
  const normalized = ((nextIndex % total) + total) % total
  pdfFullSearchActiveIndex.value = normalized
}

const jumpToActivePdfSearchResult = () => {
  if (!pdfFullSearchResults.value.length) return

  const i = pdfFullSearchActiveIndex.value >= 0 ? pdfFullSearchActiveIndex.value : 0
  const hit = pdfFullSearchResults.value[i]
  if (!hit?.page) return

  goToPage(hit.page)
}

const gotoNextPdfSearch = () => {
  if (useNewPdfViewer.value) { pdfViewer.value?.nextHit?.(); return }
  if (pdfFullSearchResults.value.length) {
    updateActivePdfFullSearchResult(pdfFullSearchActiveIndex.value + 1)
    jumpToActivePdfSearchResult()
    return
  }
  gotoNextPdfSearchMatch()
}

const gotoPrevPdfSearch = () => {
  if (useNewPdfViewer.value) { pdfViewer.value?.prevHit?.(); return }
  if (pdfFullSearchResults.value.length) {
    updateActivePdfFullSearchResult(pdfFullSearchActiveIndex.value - 1)
    jumpToActivePdfSearchResult()
    return
  }
  gotoPrevPdfSearchMatch()
}

const updateActiveMarkdownSearchMatch = (nextIndex) => {
  const total = mdSearchMatches.value.length
  if (!total) return

  const normalized = ((nextIndex % total) + total) % total

  const prev = mdSearchMatches.value[mdSearchActiveIndex.value]
  if (prev) prev.classList.remove('inpage-match-active')

  mdSearchActiveIndex.value = normalized

  const el = mdSearchMatches.value[normalized]
  if (el) {
    el.classList.add('inpage-match-active')
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' })
  }
}

const gotoNextMarkdownSearch = () => {
  if (!mdSearchMatches.value.length) return
  updateActiveMarkdownSearchMatch(mdSearchActiveIndex.value + 1)
}

const gotoPrevMarkdownSearch = () => {
  if (!mdSearchMatches.value.length) return
  updateActiveMarkdownSearchMatch(mdSearchActiveIndex.value - 1)
}

const clearPdfSearchHighlights = () => {
  const layer = pdfTextLayer.value
  if (!layer) return

  const marks = layer.querySelectorAll('span.inpage-match, span.inpage-match-active')
  marks.forEach((el) => {
    el.classList.remove('inpage-match')
    el.classList.remove('inpage-match-active')
  })
}

const applyPdfSearch = () => {
  clearPdfSearchHighlights()

  pdfSearchMatches.value = []
  pdfSearchActiveIndex.value = -1

  const query = pdfSearchQuery.value.trim()
  if (!query) return

  const layer = pdfTextLayer.value
  if (!layer) return

  const qLower = query.toLowerCase()
  const spans = Array.from(layer.querySelectorAll('span'))

  spans.forEach((span) => {
    const t = String(span.textContent || '').trim()
    if (!t) return
    if (t.toLowerCase().includes(qLower)) {
      span.classList.add('inpage-match')
      pdfSearchMatches.value.push(span)
    }
  })

  if (pdfSearchMatches.value.length) {
    nextTick(() => updateActivePdfSearchMatch(0))
  }
}

const clearMarkdownSearchHighlights = () => {
  const container = markdownContainer.value
  if (!container) return

  const marks = container.querySelectorAll('span.inpage-match, span.inpage-match-active')
  marks.forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  })
}

const applyMarkdownSearch = () => {
  clearMarkdownSearchHighlights()

  mdSearchMatches.value = []
  mdSearchActiveIndex.value = -1

  const query = mdSearchQuery.value.trim()
  if (!query) return

  const container = markdownContainer.value
  if (!container) return

  const qLower = query.toLowerCase()

  const walker = document.createTreeWalker(
    container,
    NodeFilter.SHOW_TEXT,
    {
      acceptNode(node) {
        if (!node?.textContent || !node.textContent.trim()) return NodeFilter.FILTER_REJECT
        const parent = node.parentElement
        if (!parent) return NodeFilter.FILTER_REJECT
        if (parent.closest('script, style')) return NodeFilter.FILTER_REJECT
        return NodeFilter.FILTER_ACCEPT
      }
    }
  )

  const textNodes = []
  while (walker.nextNode()) textNodes.push(walker.currentNode)

  textNodes.forEach((textNode) => {
    const text = textNode.textContent || ''
    const lower = text.toLowerCase()

    let idx = lower.indexOf(qLower)
    if (idx === -1) return

    const frag = document.createDocumentFragment()
    let last = 0

    while (idx !== -1) {
      if (idx > last) frag.appendChild(document.createTextNode(text.slice(last, idx)))

      const span = document.createElement('span')
      span.className = 'inpage-match'
      span.textContent = text.slice(idx, idx + query.length)
      frag.appendChild(span)
      mdSearchMatches.value.push(span)

      last = idx + query.length
      idx = lower.indexOf(qLower, last)
    }

    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)))

    const parent = textNode.parentNode
    if (parent) parent.replaceChild(frag, textNode)
  })

  if (mdSearchMatches.value.length) {
    nextTick(() => updateActiveMarkdownSearchMatch(0))
  }
}

const invokeWithTimeout = (p, ms) => {
  let t = 0
  const timeout = new Promise((_, reject) => {
    t = window.setTimeout(() => reject(new Error('timeout')), ms)
  })
  return Promise.race([p, timeout]).finally(() => {
    if (t) {
      clearTimeout(t)
    }
  })
}

const searchFullPdfDocument = () => {
  if (pdfFullSearchTimer) {
    clearTimeout(pdfFullSearchTimer)
    pdfFullSearchTimer = 0
  }

  const query = pdfSearchQuery.value.trim()
  if (!query) {
    pdfFullSearchBusy.value = false
    pdfFullSearchResults.value = []
    pdfFullSearchActiveIndex.value = -1
    return
  }

  pdfFullSearchTimer = window.setTimeout(async () => {
    try {
      pdfFullSearchBusy.value = true
      pdfFullSearchResults.value = []
      pdfFullSearchActiveIndex.value = -1

      const hits = typeof searchText === 'function' ? await searchText(query) : []
      pdfFullSearchResults.value = Array.isArray(hits) ? hits : []

      const currentIdx = pdfFullSearchResults.value.findIndex((m) => m?.page === currentPage.value)
      pdfFullSearchActiveIndex.value = currentIdx >= 0
        ? currentIdx
        : (pdfFullSearchResults.value.length ? 0 : -1)
    } catch {
      pdfFullSearchResults.value = []
      pdfFullSearchActiveIndex.value = -1
    } finally {
      pdfFullSearchBusy.value = false
    }
  }, 250)
}

let newViewerSearchTimer = 0
const handlePdfSearchInput = () => {
  if (useNewPdfViewer.value) {
    // 新版 viewer：把宿主搜索框输入转发给 viewer 的全文搜索（防抖）。
    pdfViewer.value?.setSearchQuery(pdfSearchQuery.value || '')
    if (newViewerSearchTimer) clearTimeout(newViewerSearchTimer)
    newViewerSearchTimer = setTimeout(() => {
      newViewerSearchTimer = 0
      pdfViewer.value?.runSearch?.()
    }, 250)
    return
  }
  applyPdfSearch()
  searchFullPdfDocument()
}

const openPdfSearch = async () => {
  closeMarkdownSearch()
  pdfSearchVisible.value = true
  await nextTick()
  const input = pdfSearchInput.value
  if (input && input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
  handlePdfSearchInput()
}

const closePdfSearch = () => {
  pdfSearchVisible.value = false
  pdfSearchQuery.value = ''

  if (useNewPdfViewer.value) {
    if (newViewerSearchTimer) { clearTimeout(newViewerSearchTimer); newViewerSearchTimer = 0 }
    pdfViewer.value?.clearSearch?.()
  }

  if (pdfFullSearchTimer) {
    clearTimeout(pdfFullSearchTimer)
    pdfFullSearchTimer = 0
  }

  pdfFullSearchBusy.value = false
  pdfFullSearchResults.value = []
  pdfFullSearchActiveIndex.value = -1

  clearPdfSearchHighlights()
  pdfSearchMatches.value = []
  pdfSearchActiveIndex.value = -1
}

const openMarkdownSearch = async () => {
  closePdfSearch()
  mdSearchVisible.value = true
  await nextTick()
  const input = mdSearchInput.value
  if (input && input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
  applyMarkdownSearch()
}

const closeMarkdownSearch = () => {
  mdSearchVisible.value = false
  mdSearchQuery.value = ''
  clearMarkdownSearchHighlights()
  mdSearchMatches.value = []
  mdSearchActiveIndex.value = -1
}

const handleMarkdownHover = (e) => {
  // 清除之前的悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });

  const element = e.target.closest(
    'p, h1, h2, h3, h4, h5, h6, li, blockquote, ' +
    'pre, code, .code-block, ' +
    '.katex-display, .katex-inline, ' +
    '.mermaid, svg'
  );

  if (element) {
    element.classList.add('hovered');
  }
}

const handleMarkdownLeave = () => {
  // 清除所有悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

// 16. 路径处理
const updateMarkdownPath = async () => {
  const baseFileName = props.fileName.replace('.pdf', '')
  markdownFilePath.value = await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'markdown',
    baseFileName,
    `${baseFileName}.md`
  )
}

const getNotesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'notes'
  );
};

const getImagesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'images'
  );
};

// 17. 数据加载
const isPdfDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_pdfjs') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const pdfDlog = (...args) => {
  if (!isPdfDebug()) return
  logger.info('[PDFJS][Reader]', ...args)
}

const isPdfHighlightDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_pdfhl') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const pdfHlog = (...args) => {
  if (!isPdfHighlightDebug()) return
  logger.info('[PDFHL][Reader]', ...args)
}

const pdfPageContainer = ref(null)
const pdfEditorLayer = ref(null)
const pdfHighlightLayer = ref(null)

const pdfAnnotations = ref([])
const pdfActiveAnnotationId = ref(null)
const pdfAnnotationPanelVisible = ref(false)

const clearPdfHighlightLayer = () => {
  const el = pdfHighlightLayer.value
  if (el && el instanceof HTMLElement) el.innerHTML = ''
}

const normalizeClientRectsToPage = (domRects, pageRect) => {
  const pr = pageRect && pageRect.width > 0 && pageRect.height > 0
    ? pageRect
    : null
  if (!pr) return []

  const w = pr.width || 1
  const h = pr.height || 1

  return (Array.isArray(domRects) ? domRects : [])
    .map((r) => {
      if (!r) return null
      const x = (Number(r.left) - pr.left) / w
      const y = (Number(r.top) - pr.top) / h
      const rw = Number(r.width) / w
      const rh = Number(r.height) / h
      if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(rw) || !Number.isFinite(rh)) return null
      return {
        x: Math.max(0, Math.min(1, x)),
        y: Math.max(0, Math.min(1, y)),
        w: Math.max(0, Math.min(1, rw)),
        h: Math.max(0, Math.min(1, rh)),
      }
    })
    .filter(Boolean)
    .filter((r) => r.w > 0.002 && r.h > 0.002)
    .slice(0, 200)
}

const findPdfJsEditorElement = (editorId) => {
  const root = pdfEditorLayer.value
  if (!(root instanceof HTMLElement)) return null
  const id = String(editorId || '').trim()
  if (!id) return null

  const sel = []
  sel.push(`#${typeof CSS !== 'undefined' && CSS.escape ? CSS.escape(id) : id}`)
  sel.push(`[data-editor-id="${id}"]`)
  sel.push(`[data-id="${id}"]`)
  sel.push(`[data-annotation-id="${id}"]`)
  sel.push(`[data-element-id="${id}"]`)

  for (const s of sel) {
    try {
      const el = root.querySelector(s)
      if (el && el instanceof HTMLElement) return el
    } catch {
    }
  }

  try {
    const all = Array.from(root.querySelectorAll('*'))
    for (let i = 0; i < all.length; i += 1) {
      const el = all[i]
      if (!(el instanceof HTMLElement)) continue
      if (el.id === id) return el
      const v = el.getAttribute('data-editor-id') || el.getAttribute('data-id') || el.getAttribute('data-annotation-id') || el.getAttribute('data-element-id')
      if (v === id) return el
    }
  } catch {
  }

  return null
}

const getHighlightRectsFromPdfJsEditor = (editorId) => {
  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) return []

  const editorEl = findPdfJsEditorElement(editorId)
  if (!(editorEl instanceof HTMLElement)) {
    if (isPdfHighlightDebug()) {
      pdfHlog('pdfjsEditorNotFound', { editorId: String(editorId || '') })
    }
    return []
  }

  const pageRect = pageEl.getBoundingClientRect()
  const rects = normalizeClientRectsToPage(Array.from(editorEl.getClientRects()), pageRect)

  if (isPdfHighlightDebug()) {
    const er = editorEl.getBoundingClientRect()
    pdfHlog('pdfjsEditorRects', {
      editorId: String(editorId || ''),
      editorTag: editorEl.tagName,
      editorClass: typeof editorEl.className === 'string' ? editorEl.className : '',
      editorRect: { left: Math.round(er.left), top: Math.round(er.top), width: Math.round(er.width), height: Math.round(er.height) },
      rectsPreview: rects.slice(0, 10).map((r) => ({
        x: Number((Number(r.x) || 0).toFixed(6)),
        y: Number((Number(r.y) || 0).toFixed(6)),
        w: Number((Number(r.w) || 0).toFixed(6)),
        h: Number((Number(r.h) || 0).toFixed(6)),
      })),
    })
  }

  return rects
}

const pdfEditorRectHydrated = new Set()

const handlePdfPageClick = (e) => {
  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) return

  const clientX = Number(e?.clientX)
  const clientY = Number(e?.clientY)
  if (!Number.isFinite(clientX) || !Number.isFinite(clientY)) return

  const pageRect = pageEl.getBoundingClientRect()
  if (!pageRect.width || !pageRect.height) return

  const x = (clientX - pageRect.left) / pageRect.width
  const y = (clientY - pageRect.top) / pageRect.height

  const tolX = 3 / pageRect.width
  const tolY = 3 / pageRect.height

  const pageNum = Number(currentPage.value) || 1
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []

  const hit = list.find((it) => {
    if (Number(it?.page) !== pageNum) return false
    const rects = Array.isArray(it?.rects) ? it.rects : []
    for (let i = 0; i < rects.length; i += 1) {
      const r = rects[i]
      const rx = Number(r?.x)
      const ry = Number(r?.y)
      const rw = Number(r?.w)
      const rh = Number(r?.h)
      if (!Number.isFinite(rx) || !Number.isFinite(ry) || !Number.isFinite(rw) || !Number.isFinite(rh)) continue
      if (x >= rx - tolX && x <= rx + rw + tolX && y >= ry - tolY && y <= ry + rh + tolY) return true
    }
    return false
  })

  if (!hit?.id) return

  pdfActiveAnnotationId.value = hit.id
  pdfAnnotationPanelVisible.value = true
  nextTick(() => {
    renderPdfActiveHighlightForCurrentPage()
  })

  try {
    e.preventDefault()
    e.stopPropagation()
  } catch {
  }
}

const renderPdfActiveHighlightForCurrentPage = () => {
  clearPdfHighlightLayer()
  const layerEl = pdfHighlightLayer.value
  const pageEl = pdfPageContainer.value
  if (!(layerEl instanceof HTMLElement) || !(pageEl instanceof HTMLElement)) return

  const pageNum = Number(currentPage.value) || 1
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const onPage = list.filter((x) => Number(x?.page) === pageNum)
  if (!onPage.length) return

  const pr = pageEl.getBoundingClientRect()
  const w = pr.width
  const h = pr.height
  if (!w || !h) return

  const activeId = pdfActiveAnnotationId.value
  let mutated = false

  if (isPdfHighlightDebug()) {
    pdfHlog('renderHighlights', {
      page: pageNum,
      count: onPage.length,
      activeId: typeof activeId === 'string' ? activeId : String(activeId || ''),
      pageClient: { w: pageEl.clientWidth, h: pageEl.clientHeight },
      pageRect: { w: Number(pr.width.toFixed(3)), h: Number(pr.height.toFixed(3)), left: Number(pr.left.toFixed(3)), top: Number(pr.top.toFixed(3)) },
    })
  }

  onPage.forEach((it) => {
    if (!it) return

    let rects = Array.isArray(it.rects) ? it.rects : []

    const eid = typeof it.editorId === 'string' ? it.editorId : ''
    if (eid && !pdfEditorRectHydrated.has(eid)) {
      const fromPdfJs = getHighlightRectsFromPdfJsEditor(eid)
      if (fromPdfJs.length) {
        it.rects = fromPdfJs
        rects = fromPdfJs
        pdfEditorRectHydrated.add(eid)
        mutated = true
      }
    }

    const color = typeof it.color === 'string' ? it.color : 'rgba(64, 196, 255, 0.32)'
    const isActive = activeId && it.id === activeId

    rects.forEach((r) => {
      const x = (Number(r?.x) || 0) * w
      const y = (Number(r?.y) || 0) * h
      const rw = (Number(r?.w) || 0) * w
      const rh = (Number(r?.h) || 0) * h
      if (rw <= 0 || rh <= 0) return

      const div = document.createElement('div')
      div.className = 'pdf-highlight-rect'
      div.style.position = 'absolute'
      div.style.borderRadius = '3px'
      div.style.left = `${Number(x.toFixed(2))}px`
      div.style.top = `${Number(y.toFixed(2))}px`
      div.style.width = `${Number(rw.toFixed(2))}px`
      div.style.height = `${Number(rh.toFixed(2))}px`
      div.style.background = color
      if (isActive) {
        div.style.boxShadow = '0 0 0 1px rgba(64, 196, 255, 0.55)'
      }
      layerEl.appendChild(div)
    })
  })

  if (mutated) {
    pdfAnnotations.value = [...list]
    persistReaderState()
  }
}

const exitPdfAnnotationMode = () => {
  pdfAnnotationPanelVisible.value = false
  pdfActiveAnnotationId.value = null
  clearPdfHighlightLayer()
}

const getPdfSelectionInfo = () => {
  const layer = pdfTextLayer.value
  const pageEl = pdfPageContainer.value
  if (!(layer instanceof HTMLElement) || !(pageEl instanceof HTMLElement)) return null

  const sel = window.getSelection()
  const text = sel?.toString()?.trim() || ''
  if (!text) return null

  const range = sel && sel.rangeCount ? sel.getRangeAt(0) : null
  if (!range) return null

  const inLayer = layer.contains(range.startContainer) && layer.contains(range.endContainer)
  if (!inLayer) return null

  const pageRect = pageEl.getBoundingClientRect()
  const layerRect = layer.getBoundingClientRect()
  const basisRect = layerRect && layerRect.width > 0 && layerRect.height > 0
    ? layerRect
    : pageRect

  const bw = basisRect.width || 1
  const bh = basisRect.height || 1

  const br = range.getBoundingClientRect()
  const rect = br && br.width > 0 && br.height > 0
    ? br
    : (range.getClientRects()?.[0] || null)
  if (!rect) return null

  const ax = (rect.left + rect.width / 2 - pageRect.left) / (pageRect.width || 1)
  const ay = (rect.top + rect.height / 2 - pageRect.top) / (pageRect.height || 1)

  const rawClientRects = Array.from(range.getClientRects())

  const rects = rawClientRects
    .map((r) => ({
      x: (r.left - basisRect.left) / bw,
      y: (r.top - basisRect.top) / bh,
      w: r.width / bw,
      h: r.height / bh,
    }))
    .filter((r) => Number.isFinite(r.x) && Number.isFinite(r.y) && Number.isFinite(r.w) && Number.isFinite(r.h))
    .map((r) => ({
      x: Math.max(0, Math.min(1, r.x)),
      y: Math.max(0, Math.min(1, r.y)),
      w: Math.max(0, Math.min(1, r.w)),
      h: Math.max(0, Math.min(1, r.h)),
    }))
    .filter((r) => r.w > 0.002 && r.h > 0.002)
    .slice(0, 200)

  if (isPdfHighlightDebug()) {
    const sn = (n) => {
      if (!n) return null
      if (n instanceof Element) {
        return {
          node: n.tagName,
          cls: typeof n.className === 'string' ? n.className : '',
          text: String(n.textContent || '').trim().slice(0, 32),
        }
      }
      return { node: n.nodeType, text: String(n.textContent || '').trim().slice(0, 32) }
    }

    const toRect = (r) => ({
      left: Math.round(r.left),
      top: Math.round(r.top),
      width: Math.round(r.width),
      height: Math.round(r.height),
    })

    const raw = rawClientRects.slice(0, 24).map((r) => ({
      ...toRect(r),
      relLeft: Number((((r.left - pageRect.left) / (pageRect.width || 1)) || 0).toFixed(6)),
      relTop: Number((((r.top - pageRect.top) / (pageRect.height || 1)) || 0).toFixed(6)),
    }))

    const norm = rects.slice(0, 24).map((r) => ({
      x: Number((Number(r.x) || 0).toFixed(6)),
      y: Number((Number(r.y) || 0).toFixed(6)),
      w: Number((Number(r.w) || 0).toFixed(6)),
      h: Number((Number(r.h) || 0).toFixed(6)),
    }))

    const stats = (() => {
      const xs = rects.map((r) => Number(r.x)).filter((v) => Number.isFinite(v))
      const ys = rects.map((r) => Number(r.y)).filter((v) => Number.isFinite(v))
      if (!xs.length || !ys.length) return null
      return {
        minX: Number(Math.min(...xs).toFixed(6)),
        maxX: Number(Math.max(...xs).toFixed(6)),
        minY: Number(Math.min(...ys).toFixed(6)),
        maxY: Number(Math.max(...ys).toFixed(6)),
        count: rects.length,
      }
    })()

    let spanRects = []
    try {
      const spans = Array.from(layer.querySelectorAll('span'))
      for (let i = 0; i < spans.length && spanRects.length < 14; i += 1) {
        const s = spans[i]
        let hit = false
        try {
          hit = range.intersectsNode(s)
        } catch {
          hit = false
        }
        if (!hit) continue
        const rr = s.getBoundingClientRect()
        spanRects.push({
          text: String(s.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 24),
          ...toRect(rr),
          relLeft: Number((((rr.left - pageRect.left) / (pageRect.width || 1)) || 0).toFixed(6)),
        })
      }
    } catch {
      spanRects = []
    }

    pdfHlog('selectionInfo', {
      textLen: text.length,
      textPreview: text.slice(0, 80),
      pageRect: toRect(pageRect),
      layerRect: toRect(layerRect),
      basisRect: toRect(basisRect),
      boundRect: toRect(rect),
      start: sn(range.startContainer instanceof Element ? range.startContainer : range.startContainer?.parentElement),
      end: sn(range.endContainer instanceof Element ? range.endContainer : range.endContainer?.parentElement),
      rawClientRects: raw,
      normalizedRects: norm,
      stats,
      selectedSpanRects: spanRects,
    })
  }

  return {
    text,
    range: range.cloneRange(),
    rect,
    rects,
    anchor: {
      x: Math.max(0, Math.min(1, ax)),
      y: Math.max(0, Math.min(1, ay)),
    },
  }
}

const applyPdfJsHighlightForCurrentSelection = async (providedInfo) => {
  const info = providedInfo || getPdfSelectionInfo()
  if (!info) return null

  let editorId = null

  try {
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(info.range)

    editorId = await highlightSelectionAsAnnotation({ color: '#40C4FF' })

    await nextTick()
    await new Promise((r) => setTimeout(r, 0))
  } finally {
    try {
      window.getSelection()?.removeAllRanges()
    } catch {
    }
  }

  return { info, editorId }
}

const deleteActivePdfHighlight = async () => {
  const id = typeof pdfActiveAnnotationId.value === 'string' ? pdfActiveAnnotationId.value : ''
  if (!id) return
  await deletePdfAnnotation(id)
}

const rectOverlapRatio = (a, b) => {
  const ax = Number(a?.x)
  const ay = Number(a?.y)
  const aw = Number(a?.w)
  const ah = Number(a?.h)
  const bx = Number(b?.x)
  const by = Number(b?.y)
  const bw = Number(b?.w)
  const bh = Number(b?.h)
  if (!Number.isFinite(ax) || !Number.isFinite(ay) || !Number.isFinite(aw) || !Number.isFinite(ah)) return 0
  if (!Number.isFinite(bx) || !Number.isFinite(by) || !Number.isFinite(bw) || !Number.isFinite(bh)) return 0
  if (aw <= 0 || ah <= 0 || bw <= 0 || bh <= 0) return 0

  const x1 = Math.max(ax, bx)
  const y1 = Math.max(ay, by)
  const x2 = Math.min(ax + aw, bx + bw)
  const y2 = Math.min(ay + ah, by + bh)
  const iw = Math.max(0, x2 - x1)
  const ih = Math.max(0, y2 - y1)
  const inter = iw * ih
  if (inter <= 0) return 0

  const aArea = aw * ah
  const bArea = bw * bh
  const denom = Math.max(1e-9, Math.min(aArea, bArea))
  return inter / denom
}

const selectionOverlapsAnnotation = (selRects, annRects) => {
  const s = Array.isArray(selRects) ? selRects : []
  const a = Array.isArray(annRects) ? annRects : []
  if (!s.length || !a.length) return false

  for (let i = 0; i < s.length; i += 1) {
    for (let j = 0; j < a.length; j += 1) {
      if (rectOverlapRatio(s[i], a[j]) >= 0.28) return true
    }
  }
  return false
}

const addPdfHighlightFromSelection = async () => {
  const info = getPdfSelectionInfo()
  if (!info) return

  const page = Number(currentPage.value) || 1
  const selRects = Array.isArray(info.rects) ? info.rects : []
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []

  const hit = list.find((x) => Number(x?.page) === page && selectionOverlapsAnnotation(selRects, x?.rects))
  if (hit?.id) {
    await deletePdfAnnotation(hit.id)
    return
  }

  const id = `pdfhl_${Date.now().toString(16)}_${Math.random().toString(16).slice(2)}`
  const item = {
    id,
    page,
    selectionText: String(info.text || '').slice(0, 800),
    note: '',
    anchor: info.anchor || null,
    rects: Array.isArray(info.rects) ? info.rects : [],
    color: 'rgba(64, 196, 255, 0.32)',
    editorId: '',
    noteEditorId: '',
    editing: false,
  }

  pdfAnnotations.value = [item, ...list]
  pdfActiveAnnotationId.value = id
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()

  const r = await applyPdfJsHighlightForCurrentSelection(info)
  if (r?.editorId) {
    item.editorId = r.editorId

    const fromPdfJs = getHighlightRectsFromPdfJsEditor(r.editorId)
    if (fromPdfJs.length) {
      item.rects = fromPdfJs
      await nextTick()
      renderPdfActiveHighlightForCurrentPage()
    }

    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }
}

const addPdfAnnotationFromSelection = async () => {
  const info = getPdfSelectionInfo()
  pdfAnnotationPanelVisible.value = true
  if (!info) return

  const id = `pdfnote_${Date.now().toString(16)}_${Math.random().toString(16).slice(2)}`
  const next = {
    id,
    page: Number(currentPage.value) || 1,
    selectionText: String(info.text || '').slice(0, 800),
    note: '',
    anchor: info.anchor || null,
    rects: Array.isArray(info.rects) ? info.rects : [],
    color: 'rgba(64, 196, 255, 0.32)',
    editorId: '',
    noteEditorId: '',
    editing: true,
  }

  pdfAnnotations.value = [
    next,
    ...((Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []).map((x) => ({ ...x, editing: false }))),
  ]

  pdfActiveAnnotationId.value = id
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()

  const r = await applyPdfJsHighlightForCurrentSelection(info)
  if (r?.editorId) {
    next.editorId = r.editorId

    const fromPdfJs = getHighlightRectsFromPdfJsEditor(r.editorId)
    if (fromPdfJs.length) {
      next.rects = fromPdfJs
      await nextTick()
      renderPdfActiveHighlightForCurrentPage()
    }

    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }
}

const activatePdfAnnotation = async (id) => {
  const it = (Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []).find((x) => x?.id === id)
  if (!it) return
  pdfActiveAnnotationId.value = id
  pdfAnnotationPanelVisible.value = true

  const targetPage = Number(it.page) || 1
  if (targetPage !== Number(currentPage.value)) {
    await loadPdfPage(targetPage)
  }

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()
}

const dispatchFreeTextCreateAtClientPoint = async ({ clientX, clientY, text }) => {
  const editor = pdfEditorLayer.value
  if (!(editor instanceof HTMLElement)) return false

  await setPdfAnnotationMode('freetext')
  await nextTick()

  const common = { bubbles: true, cancelable: true, clientX, clientY, button: 0 }
  try {
    editor.dispatchEvent(new MouseEvent('mousedown', common))
    editor.dispatchEvent(new MouseEvent('mouseup', common))
    editor.dispatchEvent(new MouseEvent('click', common))
    editor.dispatchEvent(new MouseEvent('dblclick', common))
  } catch {
  }

  await nextTick()
  await new Promise((r) => setTimeout(r, 0))

  const textareas = Array.from(editor.querySelectorAll('textarea'))
  const editables = Array.from(editor.querySelectorAll('[contenteditable="true"]'))
  const ta = textareas.length ? textareas[textareas.length - 1] : null
  const ed = !ta && editables.length ? editables[editables.length - 1] : null

  try {
    if (ta && ta instanceof HTMLTextAreaElement) {
      ta.focus()
      ta.value = String(text || '')
      ta.dispatchEvent(new Event('input', { bubbles: true }))
      ta.blur()
    } else if (ed && ed instanceof HTMLElement) {
      ed.focus()
      ed.textContent = String(text || '')
      ed.dispatchEvent(new Event('input', { bubbles: true }))
      ed.blur()
    }
  } catch {
  }

  await setPdfAnnotationMode('none')
  return true
}

const savePdfAnnotationNote = async (id) => {
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const idx = list.findIndex((x) => x?.id === id)
  if (idx < 0) return

  const it = list[idx]
  const noteText = String(it?.note || '').trim()
  it.editing = false
  pdfAnnotations.value = [...list]
  persistReaderState()

  if (!noteText) {
    if (it?.noteEditorId) {
      await removeAnnotationEditorById(it.noteEditorId)
      it.noteEditorId = ''
      pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
      persistReaderState()
    }
    await savePdfAnnotationsToOriginal()
    return
  }

  const targetPage = Number(it.page) || 1
  if (targetPage !== Number(currentPage.value)) {
    await loadPdfPage(targetPage)
  }

  await nextTick()

  if (it?.noteEditorId) {
    await removeAnnotationEditorById(it.noteEditorId)
    it.noteEditorId = ''
  }

  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) {
    await savePdfAnnotationsToOriginal()
    return
  }

  const pageIndex = Math.max(0, (Number(targetPage) || 1) - 1)
  const beforeIds = getAnnotationEditorIdsForPage(pageIndex)

  const r = pageEl.getBoundingClientRect()
  const ax = Number(it?.anchor?.x)
  const ay = Number(it?.anchor?.y)
  const cx = r.left + (Number.isFinite(ax) ? Math.max(0, Math.min(1, ax)) : 0.5) * r.width
  const cy = r.top + (Number.isFinite(ay) ? Math.max(0, Math.min(1, ay)) : 0.5) * r.height

  await dispatchFreeTextCreateAtClientPoint({ clientX: cx, clientY: cy, text: noteText })

  await nextTick()
  await new Promise((r) => setTimeout(r, 0))

  const afterIds = getAnnotationEditorIdsForPage(pageIndex)
  const created = afterIds.filter((x) => !beforeIds.includes(x))
  const newId = created.length ? created[created.length - 1] : ''
  if (newId) {
    it.noteEditorId = newId
    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }

  await savePdfAnnotationsToOriginal()
}

const deletePdfAnnotation = async (id) => {
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const it = list.find((x) => x?.id === id)

  if (it?.editorId) {
    await removeAnnotationEditorById(it.editorId)
  }
  if (it?.noteEditorId) {
    await removeAnnotationEditorById(it.noteEditorId)
  }

  pdfAnnotations.value = list.filter((x) => x?.id !== id)
  if (pdfActiveAnnotationId.value === id) {
    pdfActiveAnnotationId.value = null
  }
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()
}

const annotationMode = ref('none')

const setPdfAnnotationMode = async (mode) => {
  annotationMode.value = String(mode || 'none')
  await setAnnotationEditorMode(annotationMode.value)
}

const togglePdfHighlight = async () => {
  const next = annotationMode.value === 'highlight' ? 'none' : 'highlight'
  await setPdfAnnotationMode(next)
}

const togglePdfFreeText = async () => {
  const next = annotationMode.value === 'freetext' ? 'none' : 'freetext'
  await setPdfAnnotationMode(next)
}

const savePdfAnnotationsToOriginal = async () => {
  try {
    const b64 = await saveDocumentBase64()
    await invoke('write_file_bytes_base64', { path: String(props.filePath || ''), base64: b64 })
    message.success('PDF标注已保存到原文件', { duration: 1200 })
  } catch (e) {
    const msg = typeof e === 'object' ? e.message || e.toString() : String(e)
    message.error(`保存PDF标注失败: ${msg}`)
  }
}

const loadPdfPage = async (pageNum) => {
  if (useNewPdfViewer.value) {
    // 新版渲染核心由 GuaxiPdfViewer 负责，这里只需驱动其跳页（首次加载由 :path 触发）。
    pdfViewer.value?.goToPage(pageNum)
    return
  }
  try {
    await ensureAnnotationEditorUi({ containerEl: pdfPanel.value, viewerEl: pdfPageContainer.value })
    await setPdfAnnotationMode(annotationMode.value)

    const data = await loadPage(pageNum, {
      scale: zoomLevel.value,
      rotation: rotation.value,
      canvasEl: pdfCanvas.value,
      textLayerEl: pdfTextLayer.value,
      annotationLayerEl: pdfAnnotationLayer.value,
      editorLayerEl: pdfEditorLayer.value,
      onGoToPage: (p) => goToPage(p)
    });

    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;

    await nextTick()
    renderPdfActiveHighlightForCurrentPage()

    if (pdfSearchVisible.value && pdfSearchQuery.value.trim()) {
      nextTick(() => applyPdfSearch())
    }

    const fp = String(props.filePath || '')
    if (fp && pdfOutlineLoadedFor.value !== fp && typeof getOutlineFlat === 'function') {
      pdfOutlineLoadedFor.value = fp
      try {
        const outline = await getOutlineFlat()
        outlineStore.setOutline(Array.isArray(outline) ? outline : [])
        outlineStore.setJumpToLine((p) => {
          const n = Number(p)
          if (Number.isFinite(n)) goToPage(n)
        })
      } catch {
        outlineStore.setOutline([])
      }
    }

    if (isPdfDebug()) {
      const spanCount = pdfTextLayer.value ? pdfTextLayer.value.querySelectorAll('span').length : -1
      pdfDlog('pageLoaded', {
        page: currentPage.value,
        total: totalPages.value,
        pageW: Number(data?.page_width) || 0,
        pageH: Number(data?.page_height) || 0,
        spanCount
      })
    }
  } catch (err) {
    logger.error('页面加载失败:', err);
    message.error(`加载失败: ${err.message}`);
  }
};

// 加载Markdown页面
// 加载令牌：连续滚动时可能有多次加载并发在途，慢的那次若后返回，会把右侧盖回旧页的内容。
// 每个 await 之后比对一次，过期请求直接丢掉结果。
let mdLoadToken = 0;
const loadMarkdownPage = async (pageNum) => {
  const token = ++mdLoadToken;
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath();
      if (token !== mdLoadToken) return;
    }

    const response = await invoke('load_markdown_content', {
      filepath: markdownFilePath.value,
      page: pageNum - 1
    });
    if (token !== mdLoadToken) return;

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    // 获取markdown文件所在目录
    const mdDir = await dirname(markdownFilePath.value);
    markdownContent.value = response.content;
    // logger.info("markdownContent.value:", markdownContent.value);
    const lines = await convertPdfRenderMarkdown(markdownContent.value, { libraryBasePath: mdDir });
    if (token !== mdLoadToken) return;
    markdownLines.value = lines;
    logger.info("markdownLines.value:", markdownLines.value);

    await nextTick();
    requestAnimationFrame(() => {
      const container = markdownContainer.value;
      if (container) {
        const theme = resolveMarkdownTheme();
        addCopyButtons(container, { theme });
        addMediaViewerButtons(container, { theme });
        logger.info('[ConvertPDFReader] media buttons refreshed:', {
          images: container.querySelectorAll('img').length,
          mermaid: container.querySelectorAll('.mermaid, svg').length,
        });
      }
    });

    if (mdSearchVisible.value && mdSearchQuery.value.trim()) {
      applyMarkdownSearch();
    }

  } catch (error) {
    logger.error('加载Markdown失败:', error);
    markdownLines.value = [{
      raw: '',
      html: `<div class="error">加载错误: ${error.message}</div>`
    }];
  }
};

// 生成唯一文件名（浏览器兼容方案）
function generateUniqueSvgName(content, type = 'mermaid') {
  // 简单哈希函数（替代Node.js的crypto）
  const simpleHash = (str) => {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return Math.abs(hash).toString(16).substring(0, 8);
  };

  // 使用内容哈希和时间戳
  const contentHash = simpleHash(content);
  const timestamp = Date.now();

  return `${type}-${contentHash}-${timestamp}.svg`;
}

async function processRenderedSvg(content, type = 'mermaid') {
  try {
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = content;

    const svgElement = tempDiv.querySelector('svg');
    if (!svgElement) return content;

    // 克隆并清理SVG
    const svgClone = svgElement.cloneNode(true);
    svgClone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    const svgContent = svgClone.outerHTML;

    // 生成唯一文件名（浏览器兼容）
    const fileName = generateUniqueSvgName(svgContent, type);
    const imageDir = await getImagesDirectory();

    // 调用Tauri后端保存
    const { relative_path } = await invoke('save_svg_file', {
      content: svgContent,
      fileName: fileName,
      imageDir: imageDir
    });
    const processedImagesPath = await processImages(`![${type}图表](../${relative_path})`, imageDir);
    logger.info(`${type}图表已保存到: ${processedImagesPath}`);
    return processedImagesPath;

  } catch (error) {
    logger.error(`处理${type} SVG失败:`, error);
    // 失败时返回原始内容，但添加错误标记
    return `<!-- SVG处理失败 -->\n${content}`;
  }
}

// 从渲染后的HTML代码块中提取原始代码
function extractCodeFromRendered(htmlContent) {
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = htmlContent;

  // 尝试从不同结构的代码块中提取
  const codeElement = tempDiv.querySelector('pre code') ||
    tempDiv.querySelector('.hljs code') ||
    tempDiv.querySelector('code');

  if (codeElement) {
    return codeElement.textContent;
  }

  // 如果无法提取，尝试从原始内容中获取
  const match = htmlContent.match(/<code[^>]*>([\s\S]*?)<\/code>/);
  return match ? match[1] : htmlContent;
}

// 处理代码块内容
function processCodeContent(content, htmlContent) {
  // 提取原始代码
  const rawCode = extractCodeFromRendered(htmlContent || content);

  // 检测语言类型
  let language = '';
  const langMatch = content.match(/^```(\w+)/) ||
    (htmlContent || '').match(/language-(\w+)/);
  if (langMatch) language = langMatch[1];

  // 生成标准Markdown代码块
  return `\`\`\`${language}
${rawCode.trim()}
\`\`\``;
}

// 确保路径初始化的专用函数
const ensureNotesPath = async () => {
  if (!notesFilePath.value) {
    const notesDir = await getNotesDirectory()
    notesFilePath.value = await join(notesDir, `${props.fileName.replace('.pdf', '')}.json`)
  }
  return notesFilePath.value
}

// 时间格式化
const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

// 笔记卡的来源锚点：取第一条带锚点的条目。复合笔记可能跨多页，这里回到第一处。
// 锚点形状是 { page, rects? }——rects 是页内 pt 矩形，没有它也能只跳页。
const cardAnchor = (card) => {
  const items = Array.isArray(card?.items) ? card.items : []
  const hit = items.find((it) => it?.anchor && Number(it.anchor.page) > 0)
  return hit ? hit.anchor : null
}

const jumpToNoteAnchor = (card) => {
  const anchor = cardAnchor(card)
  if (!anchor) return
  const page = Number(anchor.page) || 1
  logger.info('[笔记] 回到原文', { page, rects: Array.isArray(anchor.rects) ? anchor.rects.length : 0 })
  if (useNewPdfViewer.value) {
    pdfViewer.value?.flashAnchor?.(anchor)
  } else {
    goToPage(page)
  }
}

// ─────────── 高亮 → 笔记卡 ───────────
// 高亮存在 .pdf 里、笔记卡存在 JSON 里，两边没有互相引用，所以只能按位置认：
// 同一页、且锚点矩形与高亮矩形有重叠 → 就是同一段文字既被高亮又被摘录。
// 只读匹配、不改数据；匹配不到就什么都不做（旧笔记根本没有锚点）。
const rectsOverlap = (a, b) => {
  if (!a || !b) return false
  return (
    Math.min(a.x + a.w, b.x + b.w) > Math.max(a.x, b.x) &&
    Math.min(a.y + a.h, b.y + b.h) > Math.max(a.y, b.y)
  )
}

const flashNoteCard = (index) => {
  flashingNoteIndex.value = index
  nextTick(() => {
    const el = notesContentEl.value?.querySelectorAll('.note-card')?.[index]
    el?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  })
  if (noteFlashTimer) clearTimeout(noteFlashTimer)
  noteFlashTimer = setTimeout(() => { flashingNoteIndex.value = -1 }, 1600)
}

const scrollNotesToHighlight = (hl) => {
  const rects = Array.isArray(hl?.rects) ? hl.rects : []
  // 笔记面板没开的时候不自动弹出来——那是个 600px 的浮层，不该被一次点击拽出来
  if (!rects.length || !showNotes.value) return
  const idx = noteCards.value.findIndex((card) =>
    (Array.isArray(card?.items) ? card.items : []).some((it) => {
      const a = it?.anchor
      if (!a || Number(a.page) !== Number(hl.page) || !Array.isArray(a.rects)) return false
      return a.rects.some((r1) => rects.some((r2) => rectsOverlap(r1, r2)))
    })
  )
  if (idx < 0) {
    logger.debug('[高亮] 未找到对应的笔记卡', { page: hl.page })
    return
  }
  flashNoteCard(idx)
}

// 添加新卡片
const addNewCard = () => {
  const now = Date.now();
  const placeholder = '点击编辑内容...';
  const newCard = {
    id: now.toString(),
    type: 'single',
    title: '新笔记',
    timestamp: new Date().toISOString(),
    items: [{
      id: `${now}-0`,
      type: 'text',
      content: placeholder,
      originalMarkdown: placeholder,
      original_markdown: placeholder,
      timestamp: new Date().toISOString()
    }]
  }

  noteCards.value.push(newCard)
  updateBackendNotes('add', newCard)
}


const deleteCard = async (index) => {
  if (index >= 0 && index < noteCards.value.length) {
    const cardId = noteCards.value[index].id;
    noteCards.value.splice(index, 1);

    await updateBackendNotes('delete', { id: cardId });

    // 调整选中状态
    if (activeCardIndex.value === index) {
      activeCardIndex.value = -1;
    } else if (activeCardIndex.value > index) {
      activeCardIndex.value--;
    }
  }
};

const addToNotes = async (element) => {
  // 如果处于Alt键收集模式，则追加到当前卡片
  if (altKeyCollector.value.isActive) {
    appendToCurrentNote(element);
    return;
  }

  const safeContent = (element?.originalMarkdown || element?.content || '').trim();
  if (!safeContent) {
    showSelectionMessage('未选中有效内容');
    return;
  }

  const normalizedElement = {
    ...element,
    content: element?.content || safeContent,
    originalMarkdown: element?.originalMarkdown || safeContent
  };

  // 创建新卡片（单条笔记）
  const newCard = {
    id: Date.now().toString(),
    type: 'single',  // 单条笔记类型
    title: getCardTitle(normalizedElement.type),
    timestamp: new Date().toISOString(),
    // 单条笔记的items数组只包含一个元素
    items: [{
      id: `${Date.now()}-0`,
      type: normalizedElement.type,
      content: normalizedElement.content,
      originalMarkdown: normalizedElement.originalMarkdown,
      original_markdown: normalizedElement.originalMarkdown,
      timestamp: new Date().toISOString(),
      anchor: normalizedElement.anchor || null
    }]
  };

  noteCards.value.push(newCard);
  await updateBackendNotes('add', newCard);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) {
      cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
    }
  });
};

// 获取卡片标题
const getCardTitle = (type) => {
  const titles = {
    text: '文本摘录',
    image: '图片摘录',
    code: '代码摘录',
    math: '公式摘录',
    diagram: '图表摘录'
  }
  return titles[type] || '笔记'
}

// 加载现有笔记
const loadExistingNotes = async () => {
  try {
    const response = await invoke('load_notes_file', {
      filePath: notesFilePath.value
    });

    // 确保返回的是解析后的JSON数据
    const notesData = typeof response === 'string' ? JSON.parse(response) : response;

    if (notesData?.cards?.length) {
      noteCards.value = notesData.cards.map(card => {
        // 兼容旧数据格式
        const items = card.items || [{
          id: card.id || Date.now().toString(),
          type: card.type === 'composite' ? 'text' : card.type,
          content: card.content,
          originalMarkdown: card.originalMarkdown || card.content,
          timestamp: card.timestamp || new Date().toISOString()
        }];

        return {
          ...card,
          id: card.id || Date.now().toString(),
          type: items.length > 1 ? 'composite' : 'single', // 根据条目数量确定类型
          items,
          timestamp: card.timestamp || new Date().toISOString()
        };
      });
    }
  } catch (error) {
    logger.error('加载笔记失败:', error);
    noteCards.value = [];
  }
};

const handleMissingNote = async (action, cardData) => {
  try {
    // 1. 尝试重新加载数据
    await loadExistingNotes();

    // 2. 检查笔记是否存在
    const exists = noteCards.value.some(card => card.id === cardData?.id);

    if (!exists) {
      logger.warn('笔记不存在，自动转为新增操作', cardData);

      // 3. 确保数据完整
      const completeCard = {
        ...cardData,
        items: Array.isArray(cardData.items) ? cardData.items : [],
        timestamp: cardData.timestamp || new Date().toISOString()
      };

      await updateBackendNotes('add', completeCard);
    }
  } catch (error) {
    logger.error('处理缺失笔记失败:', error);
    message.error('自动恢复失败，请手动添加笔记');
  }
};

// 修改updateBackendNotes函数，确保原始Markdown被保存到文件
const updateBackendNotes = async (action, cardData) => {
  try {
    // 1. 验证必要字段
    if (!cardData?.id) {
      throw new Error('无效的笔记ID');
    }

    // 2. 确保items数组存在
    const items = Array.isArray(cardData.items) ? cardData.items : [];

    // 3. 构建与后端匹配的数据结构
        const payload = {
      action,
      data: {
        id: cardData.id,
        type: cardData.type || 'single',
        title: cardData.title || '',
        items: items.map(item => ({
          id: item.id || `${cardData.id}-${Date.now()}`,
          type: item.type || 'text',
          content: item.content || '',
          original_markdown: item.original_markdown || item.originalMarkdown || item.content || '',
          timestamp: item.timestamp || new Date().toISOString(),
          // 来源锚点必须显式带上：后端 update_notes_file 是按结构体反序列化再落盘的，
          // 这里不带过去，锚点在保存那一刻就没了（不会报错，只是静默丢失）。
          ...(item.anchor ? { anchor: item.anchor } : {})
        })),
        timestamp: cardData.timestamp || new Date().toISOString()
      },
      filePath: await ensureNotesPath(), // 确保路径已初始化
      importFileId: Number(props.importFileId) || 0,
      importFileName: props.fileName || '',
    };

    // 4. 调试日志
    logger.debug('发送更新请求:', {
      action,
      cardId: cardData.id,
      payload: JSON.parse(JSON.stringify(payload)) // 深度拷贝
    });

    await invoke('update_notes_file', payload);
    message.success('笔记已保存', { duration: 1500 });

  } catch (error) {
    // 5. 更健壮的错误处理
    const errorMessage = typeof error === 'object'
      ? error.message || error.toString()
      : String(error);

    logger.error('更新笔记失败:', {
      error: errorMessage,
      cardData: JSON.parse(JSON.stringify(cardData || {}))
    });

    // 6. 安全地检查错误信息
    const normalizedError = errorMessage.toLowerCase();
    if (normalizedError.includes('未找到') || normalizedError.includes('not found')) {
      await handleMissingNote(action, cardData);
    } else {
      message.error(`保存失败: ${errorMessage}`, { duration: 3000 });
    }
  }
};

// 导出笔记
const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空');
    return;
  }

  // 创建临时div来提取文本内容
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = notesContent.value;

  // 转换内容为Markdown格式
  let exportContent = '';

  // 处理每个笔记条目
  const items = tempDiv.querySelectorAll('.note-item');
  items.forEach(item => {
    const type = item.querySelector('.note-type')?.textContent || '笔记';
    const time = item.querySelector('.note-time')?.textContent || '';
    const content = item.querySelector('.note-content')?.textContent || '';

    exportContent += `### ${type} (${time})\n\n`;
    exportContent += `${content}\n\n---\n\n`;
  });

  const blob = new Blob([exportContent], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);

  const a = document.createElement('a');
  a.href = url;
  a.download = `PDF笔记_${props.fileName.replace('.pdf', '')}_${new Date().toISOString().slice(0, 10)}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

// 20. 拖拽和缩放功能
const startDrag = (e) => {
  try {
    if (isPdfDebug()) {
      pdfDlog('mousedown', {
        altKey: Boolean(e.altKey),
        button: e.button,
        target: e?.target?.tagName,
        inTextLayer: Boolean(e?.target instanceof Element && e.target.closest('.pdf-text-layer')),
      })
    }

    if (e.button !== 0 || !pdfPanel.value) return;
    if (!e.altKey) return;

    // 取消任何进行中的动画
    if (animationFrameId.value) {
      cancelAnimationFrame(animationFrameId.value);
      animationFrameId.value = null;
    }

    isDragging.value = true;
    isDecelerating.value = false;
    startDragPos.value = {
      x: e.clientX,
      y: e.clientY
    };
    startScrollPos.value = {
      left: pdfPanel.value.scrollLeft,
      top: pdfPanel.value.scrollTop
    };
    velocity.value = { x: 0, y: 0 };
    lastDragTime.value = performance.now();

    document.body.style.userSelect = 'none';
    pdfPanel.value.style.cursor = 'grabbing';

    if (!dragGlobalListenersActive) {
      dragGlobalListenersActive = true
      window.addEventListener('mouseup', endDrag, true)
      window.addEventListener('blur', endDrag, true)
    }

    e.preventDefault();
  } catch (error) {
    logger.error('Error in startDrag:', error);
    resetDragState();
  }
};

const handleDrag = (e) => {
  try {
    if (!isDragging.value || !pdfPanel.value) return;

    const now = performance.now();
    const deltaTime = now - lastDragTime.value;
    lastDragTime.value = now;

    // 计算移动距离
    const dx = e.clientX - startDragPos.value.x;
    const dy = e.clientY - startDragPos.value.y;

    // 更新滚动位置
    pdfPanel.value.scrollLeft = startScrollPos.value.left - dx;
    pdfPanel.value.scrollTop = startScrollPos.value.top - dy;

    // 计算速度（像素/毫秒）
    // 注意：scrollLeft/scrollTop 的更新方向与鼠标位移 dx/dy 相反（scroll = start - delta），
    // 若不取反，松手后惯性滚动会把页面往回拉。
    if (deltaTime > 0) {
      velocity.value = {
        x: -dx / deltaTime,
        y: -dy / deltaTime
      };
    }

    e.preventDefault();
  } catch (error) {
    logger.error('Error in handleDrag:', error);
    endDrag();
  }
};

const endDrag = () => {
  try {
    if (!isDragging.value) return;

    resetDragState();

    // 如果速度足够大，启动惯性滚动
    const speedThreshold = 0.1;
    if (Math.abs(velocity.value.x) > speedThreshold ||
      Math.abs(velocity.value.y) > speedThreshold) {
      isDecelerating.value = true;
      decelerate();
    }
  } catch (error) {
    logger.error('Error in endDrag:', error);
    resetDragState();
  }
};

const handlePdfPanelMouseUp = () => {
  endDrag()
  handleSelectionMouseUp()
}

const resetDragState = () => {
  isDragging.value = false;
  isDecelerating.value = false;
  document.body.style.userSelect = '';

  if (dragGlobalListenersActive) {
    dragGlobalListenersActive = false
    window.removeEventListener('mouseup', endDrag, true)
    window.removeEventListener('blur', endDrag, true)
  }

  if (pdfPanel.value) {
    pdfPanel.value.style.cursor = 'text';
  }
};

// 21. 惯性滚动处理
const decelerate = () => {
  if (!isDecelerating.value || !pdfPanel.value) return;

  const friction = 0.95; // 摩擦系数
  const minSpeed = 0.5; // 最小速度阈值

  // 应用摩擦减速
  velocity.value.x *= friction;
  velocity.value.y *= friction;

  // 更新滚动位置
  pdfPanel.value.scrollLeft += velocity.value.x * 16; // 16ms为一帧
  pdfPanel.value.scrollTop += velocity.value.y * 16;

  // 检查是否需要继续减速
  if (Math.abs(velocity.value.x) > minSpeed || Math.abs(velocity.value.y) > minSpeed) {
    animationFrameId.value = requestAnimationFrame(decelerate);
  } else {
    isDecelerating.value = false;
    velocity.value = { x: 0, y: 0 };
  }
};

// 22. 缩放控制
const handleWheelZoom = async (e) => {
  if (!e.ctrlKey || !pdfPanel.value) return

  e.preventDefault()

  const panel = pdfPanel.value
  const prevZoom = zoomLevel.value

  const rect = panel.getBoundingClientRect()
  const localX = e.clientX - rect.left
  const localY = e.clientY - rect.top

  const prevScrollLeft = panel.scrollLeft
  const prevScrollTop = panel.scrollTop

  const delta = e.deltaY < 0 ? 0.1 : -0.1
  const nextZoom = Math.max(0.5, Math.min(2, prevZoom + delta))

  if (nextZoom === prevZoom) return

  zoomLevel.value = nextZoom

  await loadPdfPage(currentPage.value)

  await nextTick()
  if (!pdfPanel.value) return

  const scale = nextZoom / prevZoom

  const targetScrollLeft = (prevScrollLeft + localX) * scale - localX
  const targetScrollTop = (prevScrollTop + localY) * scale - localY

  panel.scrollLeft = Math.max(0, targetScrollLeft)
  panel.scrollTop = Math.max(0, targetScrollTop)

  // 缩放和滚动位置稳定后，强制重绘当前页高亮，避免缩放过程中高亮短暂消失
  renderPdfActiveHighlightForCurrentPage()
}

// 23. 分页控制
const prevPage = () => {
  if (currentPage.value > 1) {
    const newPage = currentPage.value - 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    const newPage = currentPage.value + 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const goToPage = (page) => {
  const pageNum = typeof page === 'number' ? page : jumpPage.value;
  const targetPage = Math.max(1, Math.min(pageNum, totalPages.value));
  currentPage.value = targetPage;
  jumpPage.value = targetPage;
  loadPdfPage(targetPage);
  loadMarkdownPage(targetPage);
};

// 24. 面板控制
const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value;
  if (showNotes.value) {
    noteCards.value = [...noteCards.value];
  }
};

const toggleAssistantPanel = () => {
  showAssistant.value = !showAssistant.value
  if (showAssistant.value) {
    showNotes.value = false
    rightSiderStore.setPanel('assistant')
    rightSiderStore.open()
  }
}

// 25. 缩放功能
const zoomIn = async () => {
  if (useNewPdfViewer.value) { pdfViewer.value?.zoomIn?.(); bindViewerState(); return }
  zoomLevel.value = Math.min(zoomLevel.value + 0.1, 2)
  await loadPdfPage(currentPage.value)
};

const zoomOut = async () => {
  if (useNewPdfViewer.value) { pdfViewer.value?.zoomOut?.(); bindViewerState(); return }
  zoomLevel.value = Math.max(zoomLevel.value - 0.1, 0.5)
  await loadPdfPage(currentPage.value)
};

// 26. 面板大小调整
const startResize = (e) => {
  isResizing.value = true;
  startX.value = e.clientX;

  const leftEl = document.querySelector('.pdf-left-wrapper')
  startLeftWidth.value = leftEl
    ? parseInt(document.defaultView.getComputedStyle(leftEl).width, 10)
    : 0

  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

// 处理鼠标移动
const handleMouseMove = (e) => {
  if (!isResizing.value) return;

  const containerEl = document.querySelector('.pdf-main-content');
  if (!containerEl) return;

  // Split resize should be based on the available width for panels (exclude the resize handle itself)
  const handleEl = document.querySelector('.resize-handle');
  const handleWidth = handleEl ? handleEl.getBoundingClientRect().width : 0;

  const containerWidth = Math.max(1, containerEl.getBoundingClientRect().width - handleWidth);
  const dx = e.clientX - startX.value;
  let newLeftWidth = ((startLeftWidth.value + dx) * 100) / containerWidth;

  newLeftWidth = Math.max(30, Math.min(70, newLeftWidth));

  leftPanelWidth.value = `${newLeftWidth}%`;
  previousLeftWidth.value = leftPanelWidth.value;
};

// 停止拖动
const stopResize = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};

// 27. 旋转功能
const rotate = async (degrees) => {
  if (useNewPdfViewer.value) { message.info('该阅读器暂不支持旋转'); return }
  rotation.value = (rotation.value + degrees) % 360
  await loadPdfPage(currentPage.value)
  await fitToWidth()
}

// 28. 自适应宽度
const fitToWidth = async () => {
  if (useNewPdfViewer.value) {
    pdfViewer.value?.fitWidth?.()
    bindViewerState()
    return
  }
  const panel = pdfPanel.value
  if (!panel) return

  // Use actual available width to fit page (do not involve height constraints)
  const panelWidth = panel.getBoundingClientRect().width || panel.clientWidth
  if (!panelWidth) return

  const horizontalMargin = Math.min(20, panelWidth * 0.05)
  const safeWidth = Math.max(0, panelWidth - horizontalMargin)

  const base = await loadPage(currentPage.value, {
    scale: 1,
    rotation: rotation.value
  })

  const baseW = Number(base?.page_width) || 0
  if (!baseW) return

  const minScale = 0.5
  const scale = safeWidth / baseW
  zoomLevel.value = Math.max(minScale, Math.min(scale, 2))

  await loadPdfPage(currentPage.value)

  await nextTick()
  const w = Number(currentPageData.value?.page_width) || 0
  if (!w) return

  // Re-center using current clientWidth after render (accounts for scrollbar changes)
  panel.scrollLeft = Math.max(0, (w - panel.clientWidth) / 2)
  panel.scrollTop = 0

  // 自适应宽度完成后，再次按照当前缩放和尺寸重绘高亮
  renderPdfActiveHighlightForCurrentPage()
};

const setViewMode = (mode) => {
  if (mode === viewMode.value) return;

  if (mode === 'split') {
    leftPanelWidth.value = previousLeftWidth.value || '50%';
  } else if (mode === 'full-left') {
    previousLeftWidth.value = leftPanelWidth.value;
    leftPanelWidth.value = '100%';
  } else if (mode === 'full-right') {
    if (viewMode.value === 'full-left') {
      leftPanelWidth.value = previousLeftWidth.value || '50%';
    }
  }

  viewMode.value = mode;
};

// 29. 防抖函数
const debounce = (fn, delay) => {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
};

// 30. 翻译功能
// const toggleTranslate = async () => {
//   showTranslation.value = !showTranslation.value;

//   if (showTranslation.value) {
//     // 先标记所有需要跳过的行
//     markdownLines.value = markdownLines.value.map(line => {
//       if (!line.translation) {
//         line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
//         if (line.skipTranslation) {
//           line.translation = line.html; // 直接使用原始内容作为"翻译"
//         }
//       }
//       return line;
//     });

//     // 只翻译未标记为跳过的行
//     await translateMarkdown();
//   }
// };

// 翻译相关状态
const translateQueue = ref([]);
// const isTranslating = ref(false);
const lastTranslateTime = ref(0);
const translateBatchSize = 5; // 每次批量翻译5条
const translateInterval = 200; // 间隔200ms，确保不超过5次/秒

// 修改后的翻译函数
const translateMarkdown = async () => {
  if (isTranslating.value) return;

  // 收集需要翻译的行
  const linesToTranslate = markdownLines.value
    .filter(line => !line.skipTranslation && !line.translation)
    .map(line => {
      // 提取纯文本内容
      const tempDiv = document.createElement('div');
      tempDiv.innerHTML = line.html;
      const textNodes = [];
      const extractText = (node) => {
        if (shouldSkipTranslation(node)) return;

        if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
          textNodes.push(node.textContent.trim());
        } else if (node.childNodes) {
          for (const child of node.childNodes) {
            extractText(child);
          }
        }
      };
      extractText(tempDiv);

      return {
        line,
        text: textNodes.join(' '), // 合并所有文本节点
        html: line.html
      };
    })
    .filter(item => item.text.length > 0);

  // 添加到队列
  translateQueue.value.push(...linesToTranslate);

  // 开始处理队列
  processTranslateQueue();
};

// 处理翻译队列
const processTranslateQueue = async () => {
  if (isTranslating.value || translateQueue.value.length === 0) return;

  const now = Date.now();
  if (now - lastTranslateTime.value < translateInterval) {
    setTimeout(processTranslateQueue, translateInterval - (now - lastTranslateTime.value));
    return;
  }

  isTranslating.value = true;
  try {
    const currentBatch = translateQueue.value.splice(0, translateBatchSize);
    const texts = currentBatch.map(item => item.text);

    // 付费翻译接口
    // const response = await invoke('translate_text', {
    //   texts: texts,
    //   secretId: "1122334",
    //   secretKey: "5566778",
    // });

    // 免费翻译接口
    const response = await invoke('translate_text_free', {
      texts: texts,
    });

    // 调试日志
    logger.info('翻译响应:', response);

    // 修复点1: 检查响应结构是否匹配
    if (response && response.translated_texts?.length === texts.length) {
      currentBatch.forEach((item, index) => {
        // 修复点2: 确保使用正确的字段名
        const translatedText = response.translated_texts[index];
        if (translatedText) {
          item.line.translation = wrapTranslation(item.line.html, translatedText);
          item.line.translationError = null;
        } else {
          item.line.translationError = '翻译结果为空';
        }
      });
    } else {
      // 修复点3: 提供更详细的错误信息
      const errorMsg = response?.error ||
        (response?.translated_texts ? '返回结果数量不匹配' : '无效的响应格式');
      currentBatch.forEach(item => {
        item.line.translationError = `翻译失败: ${errorMsg}`;
      });
      logger.error('翻译错误:', response);
    }

    lastTranslateTime.value = Date.now();
  } catch (error) {
    // 修复点4: 更好的错误处理
    logger.error('翻译异常:', error);
    translateQueue.value.forEach(item => {
      item.line.translationError = `翻译异常: ${error.message || '未知错误'}`;
    });
  } finally {
    isTranslating.value = false;
    if (translateQueue.value.length > 0) {
      setTimeout(processTranslateQueue, translateInterval);
    }
  }
};

const wrapTranslation = (html, translatedText) => {
  if (!translatedText || translatedText.trim() === '') {
    return html; // 返回原始内容
  }

  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = html;

  const textNodes = [];
  const findTextNodes = (node) => {
    if (shouldSkipTranslation(node)) return;

    if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
      textNodes.push(node);
    } else if (node.childNodes) {
      for (const child of node.childNodes) {
        findTextNodes(child);
      }
    }
  };
  findTextNodes(tempDiv);

  let textPos = 0;
  for (const node of textNodes) {
    const originalText = node.textContent.trim();
    if (textPos < translatedText.length) {
      const endPos = Math.min(textPos + originalText.length, translatedText.length);
      node.textContent = translatedText.substring(textPos, endPos);
      textPos = endPos + 1;
    }
  }

  return tempDiv.innerHTML;
};

// 修改toggleTranslate函数
const toggleTranslate = async () => {
  showTranslation.value = !showTranslation.value;

  if (showTranslation.value) {
    // 先标记所有需要跳过的行
    markdownLines.value = markdownLines.value.map(line => {
      if (!line.translation) {
        line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
        if (line.skipTranslation) {
          line.translation = line.html; // 直接使用原始内容作为"翻译"
        }
      }
      return line;
    });

    // 只翻译未标记为跳过的行
    await translateMarkdown();
  }
};

// 1. 定义必要的 ref
const contentEditables = ref([]);
let autoSaveInterval = null;

const setContentEditableRef = (index, el) => {
  const idx = Number(index);
  if (!Number.isFinite(idx) || idx < 0) return;

  if (contentEditables.value[idx] === el) return;
  contentEditables.value = [...contentEditables.value];
  contentEditables.value[idx] = el || null;
};

const editorDirty = ref([]);

const setEditorDirty = (index, dirty) => {
  const idx = Number(index);
  if (!Number.isFinite(idx) || idx < 0) return;
  if (editorDirty.value[idx] === Boolean(dirty)) return;
  editorDirty.value = [...editorDirty.value];
  editorDirty.value[idx] = Boolean(dirty);
};

const markEditorDirty = (index) => setEditorDirty(index, true);
const resetEditorDirty = (index) => setEditorDirty(index, false);

// 2. 修改保存函数
const getSingleEditorText = (card) => {
  const item = card?.items?.[0] || {};
  return String(item.original_markdown || item.originalMarkdown || item.content || '');
};

const saveCardEdit = async (index) => {
  try {
    if (index < 0 || index >= noteCards.value.length) {
      return;
    }

    const editableDiv = contentEditables.value[index];
    if (!editableDiv) {
      return;
    }

    // 创建卡片的深拷贝
    const currentCard = JSON.parse(JSON.stringify(noteCards.value[index]));

    // 统一处理内容更新
    if (currentCard.type === 'composite') {
      let raw = editableDiv?.tagName === 'TEXTAREA' ? editableDiv.value : editableDiv.innerText;
      if (editableDiv?.tagName === 'TEXTAREA' && !editorDirty.value[index] && !String(raw || '').trim()) {
        raw = getCompositeEditorText(currentCard);
      }

      const items = parseCompositeEditorTextToItems(raw, currentCard.id);
      currentCard.items = items;
      currentCard.type = items.length > 1 ? 'composite' : 'single';
      if (currentCard.type === 'composite') {
        currentCard.title = `复合笔记 (${items.length}项)`;
      }
      currentCard.content = raw;
    } else {
      if (!Array.isArray(currentCard.items)) {
        currentCard.items = [];
      }

      let raw = editableDiv?.tagName === 'TEXTAREA' ? editableDiv.value : editableDiv.innerText;
      if (editableDiv?.tagName === 'TEXTAREA' && !editorDirty.value[index] && !String(raw || '').trim()) {
        raw = getSingleEditorText(currentCard);
      }

      if (currentCard.items[0]) {
        currentCard.items[0].content = raw;
        currentCard.items[0].original_markdown = raw;
        currentCard.items[0].originalMarkdown = raw;
      } else {
        currentCard.items[0] = {
          id: `${currentCard.id}-0`,
          type: 'text',
          content: raw,
          original_markdown: raw,
          originalMarkdown: raw,
          timestamp: new Date().toISOString()
        };
      }
    }

    currentCard.timestamp = new Date().toISOString();
    await updateBackendNotes('update', currentCard);

    noteCards.value.splice(index, 1, currentCard);
    resetEditorDirty(index);
    activeCardIndex.value = -1;

  } catch (error) {
    logger.error('保存编辑失败:', {
      error,
      index,
      card: noteCards.value[index]
    });
    message.error(`保存失败: ${error.message}`);
  }
};

// 4. 退出编辑模式
const getCompositeEditorText = (card) => {
  const items = Array.isArray(card?.items) ? card.items : [];
  return items
    .map(item => (item?.original_markdown || item?.originalMarkdown || item?.content || '').trim())
    .filter(Boolean)
    .join('\n\n---\n\n');
};

const parseCompositeEditorTextToItems = (text, cardId) => {
  const normalized = String(text || '').replace(/\r\n/g, '\n').trim();
  if (!normalized) return [];

  const parts = normalized
    .split(/\n\s*---\s*\n/g)
    .map(p => p.trim())
    .filter(Boolean);

  const now = Date.now();
  const ts = new Date().toISOString();

  return parts.map((part, idx) => ({
    id: `${cardId || now}-${idx}-${now}`,
    type: 'text',
    content: part,
    originalMarkdown: part,
    original_markdown: part,
    timestamp: ts
  }));
};

const exitEditMode = (index) => {
  const card = noteCards.value[index];
  const editableDiv = contentEditables.value[index];

  if (card?.type === 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    const before = getCompositeEditorText(card);
    if (editableDiv.value !== before) {
      if (!confirm('有未保存的修改，确定要取消吗？')) return;
    }
    resetEditorDirty(index);
    activeCardIndex.value = -1;
    return;
  }

  if (card?.type !== 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    const before = getSingleEditorText(card);
    if (editableDiv.value !== before) {
      if (!confirm('有未保存的修改，确定要取消吗？')) return;
    }
    resetEditorDirty(index);
    activeCardIndex.value = -1;
    return;
  }

  if (editableDiv && editableDiv.tagName !== 'TEXTAREA' && editableDiv.innerHTML !== noteCards.value[index].content) {
    if (!confirm('有未保存的修改，确定要取消吗？')) return;
  }
  activeCardIndex.value = -1;
};

const enterEditMode = async (index) => {
  activeCardIndex.value = index;
  await nextTick();
  const editableDiv = contentEditables.value[index];
  const card = noteCards.value[index];

  if (card?.type === 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    resetEditorDirty(index);
    if (!editableDiv.value || editableDiv.value.trim() === '') {
      editableDiv.value = getCompositeEditorText(card);
    }
    editableDiv.focus();
    editableDiv.setSelectionRange(editableDiv.value.length, editableDiv.value.length);
    return;
  }

  if (card?.type !== 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    resetEditorDirty(index);
    if (!editableDiv.value || editableDiv.value.trim() === '') {
      editableDiv.value = getSingleEditorText(card);
    }
    editableDiv.focus();
    editableDiv.setSelectionRange(editableDiv.value.length, editableDiv.value.length);
    return;
  }

  if (editableDiv) {
    if (!editableDiv.innerHTML || editableDiv.innerHTML.trim() === '') {
      editableDiv.innerHTML = noteHtmls.value[index] || '';
    }
    editableDiv.focus();
    const range = document.createRange();
    range.selectNodeContents(editableDiv);
    range.collapse(false);
    const selection = window.getSelection();
    if (selection) {
      selection.removeAllRanges();
      selection.addRange(range);
    }
  }
};

const debouncedSave = debounce(saveCardEdit, 500);

const handleKeyUp = (e) => {
  if (e.key === 'Alt' && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

// 31. 生命周期和事件监听
onMounted(async () => {
  logger.info('[PDFJS][Reader] mounted')

  document.addEventListener('selectionchange', () => {
    if (!isPdfDebug()) return
    try {
      const sel = window.getSelection()
      const text = sel?.toString() || ''
      if (text.trim()) {
        pdfDlog('selectionchange', {
          len: text.trim().length,
          sample: text.trim().slice(0, 80)
        })
      }
    } catch {
    }
  })

  await store.get_librariesRoot()
  await updateMarkdownPath()
  outlineStore.setJumpToLine((p) => {
    const n = Number(p)
    if (Number.isFinite(n)) goToPage(n)
  })

  const state = loadReaderState();
  bookmarks.value = [...new Set(state.bookmarks)].sort((a, b) => a - b);
  pdfAnnotations.value = Array.isArray(state.pdfAnnotations) ? state.pdfAnnotations : [];
  totalReadingMs.value = state.totalReadingMs;
  completedAt.value = state.completedAt;
  completedReadingMs.value = state.completedReadingMs;
  pendingRestorePage.value = state.lastPage;
  isHydrated.value = true;
  startReadingSession();

  // 并行加载但不包括fitToWidth
  await Promise.all([
    (async () => {
      await ensureNotesPath()
      await loadExistingNotes()
    })(),
    loadPdfPage(1),
    loadMarkdownPage(1)
  ])

  if (pendingRestorePage.value != null && totalPages.value) {
    const p = Math.max(1, Math.min(Number(pendingRestorePage.value) || 1, totalPages.value));
    pendingRestorePage.value = null;
    if (p !== 1) goToPage(p);
  }

  watch(currentPage, () => {
    if (!isHydrated.value) return;
    markCompletedIfNeeded();
    persistReaderState();
  });

  watch(bookmarks, () => {
    if (!isHydrated.value) return;
    persistReaderState();
  }, { deep: true });

  watch(pdfAnnotations, () => {
    if (!isHydrated.value) return;
    persistReaderState();
  }, { deep: true });

  // 添加全局键盘事件监听
  window.addEventListener('keyup', handleKeyUp);

  nextTick(() => {
    window.addEventListener('resize', debounce(() => {
      if (isNotesReady.value) fitToWidth()
    }, 300))

    fitToWidth()
  })

  autoSaveInterval = setInterval(() => {
    const idx = activeCardIndex.value;
    if (idx < 0 || idx >= noteCards.value.length) return;
    if (!contentEditables.value?.[idx]) return;
    saveCardEdit(idx);
  }, 30000); // 每30秒自动保存
})

onBeforeUnmount(() => {
  if (autoSaveInterval) {
    clearInterval(autoSaveInterval);
    autoSaveInterval = null;
  }

  endReadingSession();
  markCompletedIfNeeded();
  if (isHydrated.value) persistReaderState();

  window.removeEventListener('resize', handleResize);
  // 清理拖动事件监听
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  window.removeEventListener('keyup', handleKeyUp);
  if (mdLoadTimer) {
    clearTimeout(mdLoadTimer);
    mdLoadTimer = null;
  }
  if (noteFlashTimer) {
    clearTimeout(noteFlashTimer);
    noteFlashTimer = null;
  }
  outlineStore.setOutline([]);
});

const handleResize = () => {
  if (pdfPanel.value && currentPageData.value) {
    fitToWidth();
  }
};

// 32. 文件路径变化监听
watch(() => props.filePath, async (newPath) => {
  if (newPath) {
    await updateMarkdownPath();
    currentPage.value = 1;
    loadPdfPage(1);
    loadMarkdownPage(1);
    notesContent.value = '';
  }
});

// 33. 图片加载处理
const handleImageLoad = () => {
  nextTick(() => {
    fitToWidth();
  });
};

// 监听 noteCards，自动渲染所有笔记
watchEffect((onInvalidate) => {
  const token = ++notesRenderToken;
  const cards = noteCards.value.slice();
  let cancelled = false;
  onInvalidate(() => {
    cancelled = true;
  });

  (async () => {
    const htmls = await Promise.all(
      cards.map(async card => {
        if (card.type === 'single') {
          const item = card.items?.[0] || {};
          return await renderNoteContent(item.original_markdown || item.originalMarkdown || item.content || '');
        }
        return "";
      })
    );

    if (cancelled || token !== notesRenderToken) return;
    noteHtmls.value = htmls;

    const segments = await Promise.all(
      cards.map(card =>
        card.type === 'composite'
          ? parseCompositeContent(card.items)
          : []
      )
    );

    if (cancelled || token !== notesRenderToken) return;
    compositeSegments.value = segments;

    nextTick(() => {
      if (cancelled || token !== notesRenderToken) return;
      document.querySelectorAll('.notes-panel .mermaid').forEach(el => {
        try {
          if (!el.querySelector('svg')) {
            mermaid.run({ nodes: [el] });
          }
        } catch (e) {
        }
      });
    });
  })();
});
</script>

<style scoped>
/* 34. 主容器样式 */
.pdf-viewer-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

/* 35~42 的工具栏样式已抽到 readerChrome.css（.guaxi-reader-controls），六个阅读器共用一份；
   这里只保留本组件自己的按钮基础样式，摘录/批注/卡片那些按钮还依赖它。 */

/* 36. 按钮基础样式 */
button {
  padding: 5px 8px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

button:hover {
  background-color: var(--guaxi-header-bg);
  border-color: var(--guaxi-border);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

button:disabled {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

.composite-note-editor {
  box-sizing: border-box;
  width: 100%;
  max-width: 100%;
  min-height: 180px;
  padding: 14px 16px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  outline: none;
  resize: vertical;
  background-color: var(--guaxi-content-bg, var(--guaxi-panel-bg));
  color: var(--guaxi-text-primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 15px;
  line-height: 1.65;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
}

.composite-note-editor:focus {
  border-color: rgba(77, 171, 247, 0.9);
  box-shadow: 0 0 0 3px rgba(77, 171, 247, 0.18);
}

.dark .composite-note-editor {
  background-color: rgba(255, 255, 255, 0.04);
}

/* 43. 导出按钮样式 */
.export-notes {
  background-color: #20c997;
  color: white;
  border: 1px solid #12b886;
  transition: all 0.2s;
}

.export-notes:hover {
  background-color: #12b886;
  box-shadow: 0 2px 8px rgba(32, 201, 151, 0.3);
}

/* 44. 主内容区样式 */
.pdf-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  gap: 0;
  min-height: 0;
  /* 页内搜索条按 absolute 挂在这里（见 .inpage-search-bar），需要定位上下文。 */
  position: relative;
}

.pdf-left-wrapper {
  display: flex;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

/* 45. PDF渲染面板样式 */
.pdf-render-panel {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  position: relative;
  background-color: var(--guaxi-content-bg);
  overscroll-behavior: none;
  scroll-behavior: auto;
}

/* 设计说明：新版 PDF 的滚动由内部 GuaxiPdfViewer.gpv__body 承担。
   外层只负责承载覆盖式批注工具条，避免工具条进入文档流后在底部撑出一段空白滚动区。 */
.pdf-render-panel--new {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.pdf-render-panel--new :deep(.gpv) {
  flex: 1 1 auto;
  width: 100%;
  min-width: 0;
  min-height: 0;
}

/* 46. PDF页面容器样式 */
.pdf-image-container {
  width: 100%;
  min-height: 0;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding: 16px 0;
  background-color: var(--guaxi-content-bg);
  box-sizing: border-box;
}

.outline-panel-wrapper {
  /* 宽度由内联 flex-basis（outlineWidth）决定，可拖拽调整 */
  flex: 0 0 auto;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
  background-color: var(--guaxi-panel-bg);
}

/* 大纲与正文之间的拖拽把手，兼作分隔线（原来只有一条拖不动的 border-right） */
.outline-resize-handle {
  flex: 0 0 3px;
  width: 3px;
  height: 100%;
  background-color: var(--guaxi-border);
  cursor: col-resize;
  z-index: 10;
  transition: background-color 0.2s;
}

.outline-resize-handle:hover {
  background-color: #4acadb;
}

.pdf-page-container {
  position: relative;
  display: inline-block;
  background: #fff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.rendered-pdf-canvas {
  display: block;
  pointer-events: none;
}

.pdf-annotation-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 2;
  pointer-events: none;
}

.pdf-text-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  line-height: 1;
  user-select: text;
  z-index: 3;
}

.pdf-editor-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 4;
  pointer-events: none;
}

.pdf-page-container.pdf-editor-active .pdf-editor-layer {
  pointer-events: auto;
}

.pdf-highlight-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 2.3;
  pointer-events: none;
}

.pdf-highlight-rect {
  position: absolute;
  border-radius: 3px;
}

:deep(.pdf-annotation-layer a),
:deep(.pdf-annotation-layer input),
:deep(.pdf-annotation-layer textarea),
:deep(.pdf-annotation-layer select),
:deep(.pdf-annotation-layer button) {
  pointer-events: auto;
}

:deep(.pdf-text-layer span) {
  color: transparent;
  position: absolute;
  white-space: pre;
  transform-origin: 0% 0%;
  pointer-events: auto;
}

/* 48. 加载消息样式 */
.pdf-loading-message {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: var(--guaxi-text-secondary);
  font-size: 14px;
}

/* 49. 调整大小手柄样式 */
.resize-handle {
  width: 3px;
  height: 100%;
  background-color: var(--guaxi-border);
  cursor: col-resize;
  position: relative;
  z-index: 10;
  transition: background-color 0.2s;
}

.resize-handle:hover {
  background-color: #4acadb;
}

.resize-handle:active {
  background-color: #55ebd7;
}

/* 50. Markdown面板样式 */
.markdown-panel {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 5px;
  background-color: var(--guaxi-content-bg);
  display: flex;
}

.outline-panel-wrapper {
  /* 宽度由内联 flex-basis（outlineWidth）决定，可拖拽调整 */
  flex: 0 0 auto;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
}

/* 51. Markdown内容样式 */
.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  background-color: var(--guaxi-content-bg);
  user-select: text;
  padding: 10px;
  cursor: text;
  word-wrap: break-word;
  overflow-wrap: break-word;
  word-break: break-word;
}

.markdown-content :deep(a),
.markdown-content :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .markdown-content :deep(a),
.dark-mode .markdown-content :deep(a:visited),
.dark .markdown-content :deep(a),
.dark .markdown-content :deep(a:visited),
:root[data-guaxi-theme='dark'] .markdown-content :deep(a),
:root[data-guaxi-theme='dark'] .markdown-content :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.markdown-content :deep(a:hover) {
  opacity: 0.92;
}

/* 52. Markdown元素样式 */
.markdown-content :deep(p),
.markdown-content :deep(h1),
.markdown-content :deep(h2),
.markdown-content :deep(h3),
.markdown-content :deep(h4),
.markdown-content :deep(h5),
.markdown-content :deep(h6),
.markdown-content :deep(li),
.markdown-content :deep(blockquote) {
  position: relative;
  transition: all 0.2s ease;
  padding: 4px 8px;
  margin: 4px 0;
  border-radius: 4px;
}

/* 53. 图片样式 */
.markdown-content :deep(.markdown-image) {
  max-width: 100%;
  height: auto;
  margin: 10px 0;
  border-radius: 4px;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
  cursor: pointer;
  transition: transform 0.2s;
}

.markdown-content :deep(.markdown-image:hover) {
  transform: scale(1.02);
}

/* 54. 悬停 / 选中与页内命中的配色已抽到 readerChrome.css（六个阅读器共用一份）。 */

/* 55. 选择消息样式 */
.selection-message {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  background-color: rgba(0, 0, 0, 0.8);
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  z-index: 1001;
  opacity: 1;
  transition: opacity 0.5s ease;
  font-size: 14px;
  pointer-events: none;
}

.selection-message.fade-out {
  opacity: 0;
}

.selection-toolbar {
  position: fixed;
  z-index: 1100;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  padding: 6px;
  /* 按图标内容收缩：固定 320px 会在按钮右侧留一大段空条 */
  width: max-content;
  max-width: calc(100vw - 16px);
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  backdrop-filter: blur(8px);
  box-shadow: var(--guaxi-header-shadow);
  color: var(--guaxi-text-primary);
}

.selection-translate-layer {
  position: fixed;
  left: 0;
  top: 0;
  width: 100vw;
  height: 100vh;
  z-index: 1150;
  pointer-events: none;
}

.selection-translate-layer-inner {
  position: relative;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.selection-translate-layer-inner :deep(.pot-selection-translate-panel) {
  pointer-events: auto;
}

.selection-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.selection-toolbar-body {
  max-height: 220px;
  overflow: auto;
  padding: 8px 10px;
  background: var(--guaxi-header-bg);
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.selection-toolbar-body :deep(p) {
  margin: 0 0 6px;
}

.selection-toolbar-body :deep(p:last-child) {
  margin-bottom: 0;
}

.selection-toolbar-loading {
  opacity: 0.85;
}

.selection-tool-btn {
  width: 32px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.selection-tool-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

/* 划词浮条上的高亮配色：一颗当前色 + 展开后平铺的预设色。
   用圆形色点而不是图标，读者一眼就能认出「这是在挑颜色」。 */
.selection-tool-btn--swatch {
  padding: 0;
}

.selection-tool-btn--preset {
  width: 24px;
  padding: 0;
}

.selection-tool-swatch {
  display: block;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.25);
}

.selection-tool-btn.active {
  box-shadow: inset 0 0 0 2px var(--n-color-primary, #2f6fdf);
}

.pdf-annotation-toolbar-wrap {
  position: sticky;
  bottom: 14px;
  z-index: 900;
  display: flex;
  justify-content: center;
  pointer-events: none;
}

.pdf-annotation-toolbar-wrap--new {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 14px;
}

.pdf-annotation-toolbar {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 12px;
  background: rgba(20, 20, 20, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.12);
  backdrop-filter: blur(10px);
}

.pdf-annotations-panel {
  flex: 0 0 320px;
  width: 320px;
  min-width: 280px;
  height: 100%;
  border-left: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.pdf-annotations-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.pdf-annotations-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.pdf-annotations-close {
  width: 30px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotations-list {
  flex: 1 1 auto;
  overflow: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pdf-annotations-empty {
  padding: 10px 8px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.pdf-annotation-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  cursor: pointer;
}

.pdf-annotation-item-active {
  box-shadow: 0 0 0 2px rgba(255, 180, 80, 0.35);
  border-color: rgba(255, 180, 80, 0.65);
}

.pdf-annotation-item-meta {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.pdf-annotation-item-text {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  line-height: 1.45;
  word-break: break-word;
}

.pdf-annotation-item-delete {
  width: 30px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-item-delete:hover {
  color: var(--guaxi-text-primary);
  background: rgba(0, 0, 0, 0.04);
}

.pdf-annotation-item-main {
  flex: 1 1 auto;
  min-width: 0;
}

.pdf-annotation-item-note {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: rgba(64, 196, 255, 0.08);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.pdf-annotation-item-editor {
  margin-top: 8px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.pdf-annotation-note-input {
  flex: 1 1 auto;
  width: 100%;
  min-width: 0;
  min-height: 86px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.45;
  resize: vertical;
  outline: none;
  box-sizing: border-box;
}

.pdf-annotation-note-save {
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-tool-btn {
  width: 36px;
  height: 32px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.25);
  color: rgba(255, 255, 255, 0.9);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-tool-btn-active {
  background: rgba(77, 171, 247, 0.35);
  border-color: rgba(77, 171, 247, 0.65);
}

.pdf-annotation-tool-btn:hover {
  background: rgba(0, 0, 0, 0.35);
}

.pdf-annotation-tool-btn:active {
  transform: translateY(1px);
}

.pdf-hl-color-btn {
  padding: 0;
}

.pdf-hl-swatch {
  display: inline-block;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.5);
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.25) inset;
}

.pdf-hl-color-pop {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 208px;
}

.pdf-hl-presets {
  display: flex;
  gap: 8px;
}

.pdf-hl-preset {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  border: 1px solid rgba(0, 0, 0, 0.15);
  cursor: pointer;
  transition: transform 0.12s ease, box-shadow 0.12s ease;
}

.pdf-hl-preset:hover {
  transform: translateY(-1px);
}

.pdf-hl-preset.active {
  box-shadow: 0 0 0 2px var(--n-color-primary, #4dabf7);
}

/*
 * 页内搜索：吸附在各自正文区的上角，而不是浮在窗口 top:64px。
 * 分栏阅读时固定到视口的搜索条会压住另一栏的内容，也不跟随面板宽度变化。
 */
.inpage-search-bar {
  position: absolute;
  top: 10px;
  right: 12px;
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  backdrop-filter: blur(8px);
  box-shadow: var(--guaxi-header-shadow);
}

.inpage-search-bar-pdf {
  left: 12px;
  right: auto;
}

.inpage-search-bar-md {
  right: 12px;
  left: auto;
}

.inpage-search-input {
  width: 220px;
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  outline: none;
}

.inpage-search-count {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  min-width: 48px;
  text-align: center;
}

.inpage-search-btn {
  width: 32px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

/* 56. 笔记面板样式 */
.notes-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 100;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--guaxi-border);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
}

.notes-panel-visible {
  right: 0;
}

/* 57. 笔记头部样式 */
/* 笔记头部：原来是写死的饱和蓝色块 + 白字，暗色主题下就是面板顶上一整块突兀的亮色，
   和面板自身的主题色也对不上。改成跟随主题的安静表面——与面板同色，靠一条分隔线
   与内容区分开，标题用主题文字色。 */
.notes-header {
  padding: 12px 16px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

/* 关闭键也要跟着走主题。头部原来是白字，而按钮底色由全局 button 规则给
   （var(--guaxi-panel-bg)），叠在饱和色头部上就是"浅底白字"；暗色主题下同样对不上。 */
.notes-header .close-btn,
.assistant-header .close-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  box-shadow: none;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}

.notes-header .close-btn:hover,
.assistant-header .close-btn:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
  transform: none;
  box-shadow: none;
}

.notes-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

/* 58. 笔记内容区样式 */
.notes-content {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

/* 笔记面板里的「高亮」分组：一条一行，色块 + 原文 + 页码。 */
.notes-highlights {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px;
  border-radius: 8px;
  background: var(--guaxi-hover-bg, rgba(120, 120, 120, 0.06));
}

.notes-highlights__head {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 0 4px 4px;
  font-size: 12px;
  font-weight: 600;
  opacity: 0.8;
}

.notes-highlights__count {
  font-size: 11px;
  font-weight: 400;
  opacity: 0.7;
}

.notes-highlights__empty {
  padding: 6px 4px;
  font-size: 12px;
  line-height: 1.6;
  opacity: 0.6;
}

.notes-highlight {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

.notes-highlight:hover {
  background: var(--guaxi-hover-bg, rgba(120, 120, 120, 0.1));
}

.notes-highlight.is-on {
  background: var(--guaxi-selection-bg, rgba(0, 122, 204, 0.14));
}

.notes-highlight__swatch {
  flex: 0 0 auto;
  width: 10px;
  height: 10px;
  border-radius: 2px;
}

.notes-highlight__text {
  flex: 1 1 auto;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.notes-highlight__page {
  flex: 0 0 auto;
  font-size: 11px;
  opacity: 0.6;
  font-variant-numeric: tabular-nums;
}

.assistant-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 101;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--guaxi-border);
}

.assistant-panel-visible {
  right: 0;
}

.assistant-header {
  padding: 12px 16px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.assistant-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.assistant-content {
  flex: 1;
  min-height: 0;
  background-color: var(--guaxi-content-bg);
}

/* 60. 笔记内容样式 */
.note-content {
  padding: 12px 16px;
  min-height: 60px;
  outline: none;
}

.note-content :deep(a),
.note-content :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .note-content :deep(a),
.dark-mode .note-content :deep(a:visited),
.dark .note-content :deep(a),
.dark .note-content :deep(a:visited),
:root[data-guaxi-theme='dark'] .note-content :deep(a),
:root[data-guaxi-theme='dark'] .note-content :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.note-content :deep(a:hover) {
  opacity: 0.92;
}

/* 保存反馈动画 */
.note-content.saved-feedback {
  animation: savedPulse 1s;
}

@keyframes savedPulse {
  0% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0.4);
  }

  70% {
    box-shadow: 0 0 0 10px rgba(77, 171, 247, 0);
  }

  100% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0);
  }
}

/* 编辑状态更明显的视觉反馈 */
.note-content[contenteditable="true"] {
  /* outline: 2px solid #4dabf7; */
  padding: 12px;
  border-radius: 8px;
  background-color: rgba(3, 101, 167, 0.05);
}

/* 复合笔记的编辑状态 */
.composite-note-content[contenteditable="true"] {
  min-height: 100px;
  padding: 12px;
}

/* 59. 笔记卡片样式 */
.note-card {
  transition: all 0.2s var(--n-bezier);
  cursor: pointer;
}

.note-card:hover {
  box-shadow: 0 1px 2px -2px rgba(0, 0, 0, 0.08),
    0 3px 6px 0 rgba(0, 0, 0, 0.06),
    0 5px 12px 4px rgba(0, 0, 0, 0.04);
}

/* 卡片页脚：时间在左、"回到原文"在右。
   按钮做成安静的胶囊——它是"跳转"这个动作，不是当前状态，所以不抢颜色，
   只在悬停时用主题色把自己点亮。 */
.note-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.note-anchor-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 9px;
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  line-height: 18px;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s, border-color 0.15s;
}

.note-anchor-btn:hover {
  background: var(--guaxi-hover-bg);
  border-color: var(--n-color-primary, #4dabf7);
  color: var(--n-color-primary, #4dabf7);
  transform: none;
  box-shadow: none;
}

/* 从 PDF 高亮跳过来的那张卡：只滚动落位不够，用户认不出是哪张，所以再圈一下。
   用 color-mix 而不是直接写主题色，明暗两套主题下都不会过重。 */
.note-card.note-card--flashing {
  border-color: var(--n-color-primary, #4dabf7);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--n-color-primary, #4dabf7) 55%, transparent),
    0 4px 14px -4px color-mix(in srgb, var(--n-color-primary, #4dabf7) 40%, transparent);
}

/* 61. 添加卡片容器样式 */
.add-card-container {
  margin-top: 8px;
  margin-bottom: 16px;
}

/* 62. 关闭按钮样式 */
.close-btn {
  background: none;
  border: none;
  color: white;
  cursor: pointer;
  padding: 4px;
  border-radius: 50%;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.2);
  transform: rotate(90deg);
}

/* 63. 笔记气泡样式 */
.note-added-bubble {
  position: fixed;
  top: 70px;
  right: 20px;
  background-color: #27ae60;
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 10px;
}

/* 64. 淡入淡出动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* 65. 复合笔记样式 */
.composite-note-content {
  padding: 12px;
}

.note-segment {
  margin-bottom: 16px;
  padding-bottom: 16px;
  border-bottom: 1px dashed #e0e0e0;
}

/* 66. 不同类型笔记段落的样式 */
.text-segment {
  font-size: 14px;
  line-height: 1.6;
}

.code-segment {
  background: #f6f8fa;
  padding: 12px;
  border-radius: 6px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  overflow-x: auto;
}

.image-segment {
  text-align: center;
}

.image-segment img {
  max-width: 100%;
  border-radius: 4px;
}

.diagram-segment {
  background: white;
  padding: 16px;
  border-radius: 8px;
  overflow-x: auto;
}

.math-segment {
  padding: 8px;
  overflow-x: auto;
}

/* 卡片操作按钮容器 */
.card-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  /* 调整按钮间距 */
}

/* 按钮图标样式 */
.n-button--tiny .n-icon {
  margin: 0;
  /* 移除默认外边距 */
  display: flex;
  /* 确保图标居中 */
  align-items: center;
  justify-content: center;
}

/* 按钮悬停效果 */
.n-button--tiny:hover {
  transform: scale(1.1);
  transition: transform 0.2s ease;
}

/* 删除按钮特殊样式 */
.n-button--tiny.text-type[data-v-xxxxxx]:hover {
  color: #ff6b6b;
  background-color: rgba(255, 107, 107, 0.1);
}


/* 翻译/朗读按钮的状态样式、以及右栏的双语行（.markdown-line / .original-content /
   .translated-content / .skipped-translation / .error-translation）都已抽到
   readerChrome.css，六个阅读器共用一份 —— 之前每个阅读器各抄一份，改一处要记得改六处。 */

.translation-error-message {
  color: #ff6b6b;
  font-size: 0.9em;
  margin-bottom: 8px;
  font-weight: bold;
}

.original-fallback {
  opacity: 0.7;
  border-top: 1px dashed #ccc;
  padding-top: 8px;
  margin-top: 8px;
}

.translation-error {
  color: #ff6b6b;
  font-size: 0.9em;
  font-style: italic;
}

.translation-error-marker {
  display: inline-block;
  font-size: 0.8em;
  background-color: #ffebee;
  color: #ff6b6b;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}

.translating-indicator {
  display: inline-block;
  width: 1em;
  text-align: left;
}

.translating {
  position: relative;
  opacity: 0.7;
}

.translating::after {
  content: "";
  position: absolute;
  bottom: -2px;
  left: 0;
  right: 0;
  height: 2px;
  background-color: var(--n-color-primary, #4dabf7);
  animation: translating-progress 2s infinite;
}

@keyframes translating-progress {
  0% {
    transform: translateX(-100%);
  }

  100% {
    transform: translateX(100%);
  }
}

/* 导轨由 readerChrome.css 的 .skipped-translation 给（inset box-shadow）。
   这里不要再写 border-left——两者叠加会在译文块左边出现两条竖线。 */
.skipped-translation {
  opacity: 0.7;
  padding-left: 8px;
  margin-left: 4px;
  font-style: italic;
  color: var(--guaxi-text-secondary);
}

.translation-skipped-marker {
  display: inline-block;
  font-size: 0.8em;
  /* 原来写死浅灰底 + #999 字，暗色主题下是一块发亮的色片 */
  background-color: var(--guaxi-hover-bg);
  color: var(--guaxi-text-secondary);
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}
</style>

<style>
/* 71. 数学公式全局样式 */
/* .math-inline {
  display: inline;
  vertical-align: middle;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
} */

/* 72. Mermaid图表全局样式 */
.mermaid {
  background-color: #ffffff;
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  color: #1f2328;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

/* 73. 代码块全局样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #ffffff !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

.hljs code {
  font-family: inherit !important;
}

code:not(.hljs code) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
}

.hljs:hover::before {
  opacity: 1;
}

/* 74. 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

.dark .hljs::before {
  background: #444;
  color: #fff;
}

/* 75. 复制按钮样式 */
.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

/* 76. 不同类型笔记的样式 */
.text-note {
  white-space: pre-wrap;
}

.code-note pre {
  background-color: #f6f8fa;
  padding: 12px;
  border-radius: 4px;
  overflow-x: auto;
}

.math-note {
  overflow-x: auto;
}

.diagram-note {
  text-align: center;
}

/* 确保数学公式容器正确显示 */
.math-inline-container,
.math-block-container {
  display: inline-block;
}

mjx-container.math-inline {
  display: inline;
  vertical-align: middle;
}

mjx-container.math-block {
  display: block;
  margin: 1em 0;
  text-align: center;
}
</style>
