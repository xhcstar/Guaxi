<template>
  <n-message-provider>
    <div class="lang-learn-page" :class="{ 'is-immersive': isListeningImmersiveReady }">
      <header v-if="!isListeningImmersiveReady" class="ll-topbar">
        <div class="ll-brand">
          <div class="ll-brand-kicker">GUAXI · LANGUAGE</div>
          <h1 class="ll-brand-title serif">语言学习中心</h1>
        </div>
        <nav class="ll-mainnav">
          <button class="ll-navbtn" :class="{ active: activeTab === 'manager' }" @click="activeTab = 'manager'">书架</button>
          <button class="ll-navbtn" :class="{ active: activeTab === 'review' }" @click="goToReview">复习训练</button>
        </nav>
        <div class="ll-topright">
          <div class="ll-langpair" title="学习语言 → 母语（决定转写与翻译目标）">
            <n-select v-model:value="learningLang" size="small" :options="learningLangOptions" style="width: 116px" />
            <span class="ll-langarrow">→</span>
            <n-select v-model:value="nativeLang" size="small" :options="nativeLangOptions" style="width: 116px" />
          </div>
          <n-input
            v-if="activeTab === 'manager'"
            v-model:value="keyword"
            clearable
            size="small"
            class="ll-search"
            placeholder="搜索词条 / 译文 / 笔记"
            @keydown.enter="refreshAll"
          />
          <div v-if="syncQueueVisualRunning" class="ll-syncpill">
            <span class="pulse-dot"></span>
            后台处理 {{ Math.min(syncQueueDone, syncQueueTotal) }}/{{ syncQueueTotal }}
          </div>
          <n-dropdown trigger="click" :options="moreMenuOptions" @select="onMoreMenuSelect">
            <n-button quaternary circle size="small" title="更多">
              <Icon :icon="moreIcon" width="16" height="16" />
            </n-button>
          </n-dropdown>
        </div>
      </header>

      <section v-if="isListeningImmersiveReady" class="ll-studio">
        <header class="studio-head">
          <button class="studio-back" title="返回听力书架" @click="exitListeningImmersive">
            <Icon :icon="chevronLeftIcon" width="18" height="18" />
            <span>书架</span>
          </button>
          <div class="studio-titleblock">
            <div class="eyebrow">{{ activeAudio?.provider || 'LISTENING' }} · {{ activeAudio?.topic || '未标注主题' }}</div>
            <h1 class="studio-title serif">{{ activeAudio?.title || '听力学习' }}</h1>
          </div>
          <div class="studio-headmeta">
            <span class="studio-duration">{{ fmtDuration(activeAudio?.duration_sec) }}</span>
            <span class="sync-chip" :class="`tone-${activeAudioSyncState.tone}`">
              <span v-if="activeAudioSyncState.tone === 'running'" class="pulse-dot"></span>
              {{ activeAudioSyncState.label }}
            </span>
          </div>
        </header>

        <div class="studio-grid">
          <aside class="studio-player">
            <div class="player-sheet">
              <video
                v-if="activeMediaIsVideo"
                ref="activeAudioPlayerRef"
                class="listening-player listening-video-player"
                controls
                playsinline
                :src="activeAudioPlayableSrc || undefined"
                @loadedmetadata="onAudioLoadedMetadata"
                @canplay="handleListeningMediaCanPlay('active', $event)"
                @play="handleListeningMediaPlay('active', $event)"
                @timeupdate="handleAudioTimeUpdate"
                @error="handleListeningMediaError('active', $event)"
              >
                <track
                  v-if="activeMediaSubtitleVttSrc"
                  kind="subtitles"
                  srclang="zh"
                  label="自动字幕"
                  :src="activeMediaSubtitleVttSrc"
                  default
                />
              </video>
              <audio
                v-else
                ref="activeAudioPlayerRef"
                class="listening-player"
                controls
                :src="activeAudioPlayableSrc || undefined"
                @loadedmetadata="onAudioLoadedMetadata"
                @canplay="handleListeningMediaCanPlay('active', $event)"
                @play="handleListeningMediaPlay('active', $event)"
                @timeupdate="handleAudioTimeUpdate"
                @error="handleListeningMediaError('active', $event)"
              ></audio>
            </div>

            <div class="player-row">
              <span class="section-label">播放速度</span>
              <n-select
                v-model:value="playbackRate"
                size="small"
                style="width: 112px"
                :options="playbackRateOptions"
                @update:value="applyAudioPlaybackRate"
              />
              <span class="player-hint">{{ activeAudioTranscriptTimeline.length }} 段文稿</span>
            </div>

            <div class="sync-sheet" v-if="activeAudioSyncState.tone !== 'ready'">
              <div class="sync-sheet-head">
                <span class="section-label">{{ activeAudioSyncState.tone === 'error' ? (activeAudioSyncState.label === '音频缺失' ? '音频缺失' : '同步失败') : activeAudioSyncState.tone === 'partial' ? '译文待补齐' : '后台同步' }}</span>
                <span class="sync-percent">{{ Math.round(activeAudioSyncState.progress || 0) }}%</span>
              </div>
              <n-progress
                type="line"
                :percentage="Math.round(activeAudioSyncState.progress || 0)"
                :status="activeAudioSyncState.tone === 'error' ? 'error' : 'info'"
                :show-indicator="false"
                :height="4"
              />
              <div class="sync-message">{{ activeAudioSyncState.message || '正在准备音频' }}</div>
              <n-button v-if="activeAudioSyncState.tone === 'error' && activeAudioSyncState.label !== '音频缺失'" size="tiny" @click="fetchTranscriptForActiveAudio">重试同步</n-button>
              <n-button v-else-if="activeAudioSyncState.tone === 'partial'" size="tiny" @click="retranslateActiveAudio">补齐译文</n-button>
            </div>

            <div class="studio-verdict">
              <span class="section-label">听完这一条，感觉如何？</span>
              <div class="verdict-actions">
                <button class="verdict-btn known" @click="applyReviewResult('known')">听懂了</button>
                <button class="verdict-btn fuzzy" @click="applyReviewResult('fuzzy')">半懂</button>
                <button class="verdict-btn unknown" @click="applyReviewResult('unknown')">听不懂</button>
              </div>
            </div>

            <div class="studio-facts">
              <div class="fact">
                <span class="section-label">来源 / 主题</span>
                <span>{{ activeAudio?.provider || '—' }} / {{ activeAudio?.topic || '—' }}</span>
              </div>
              <div class="fact">
                <span class="section-label">创建时间</span>
                <span>{{ fmtDate(activeAudio?.created_at) }}</span>
              </div>
              <div class="fact">
                <span class="section-label">译文覆盖</span>
                <span>{{ formatCoverageLabel(activeAudio?.translationCoverage || { translated: 0, total: 0, ratio: 0 }) }}</span>
              </div>
            </div>

            <div class="studio-actions">
              <n-button size="small" @click="editAudio(activeAudio)">编辑资源</n-button>
              <n-button size="small" @click="retranslateActiveAudio">重译</n-button>
              <n-button size="small" @click="fetchTranscriptForActiveAudio">重新同步</n-button>
            </div>
          </aside>

          <section class="studio-transcript">
            <header class="transcript-head">
              <div>
                <div class="eyebrow">TRANSCRIPT</div>
                <div class="transcript-sub">点击任意一行跳转播放</div>
              </div>
              <div class="transcript-tools">
                <div class="seg-group">
                  <button
                    v-for="mode in transcriptDisplayModes"
                    :key="mode.value"
                    class="seg-btn"
                    :class="{ active: transcriptDisplayMode === mode.value }"
                    @click="transcriptDisplayMode = mode.value"
                  >{{ mode.label }}</button>
                </div>
                <button
                  v-if="activeMediaIsVideo"
                  class="seg-btn subtitle-toggle"
                  :title="`视频字幕：${subtitleDisplayModeLabel}`"
                  @click="cycleSubtitleDisplayMode"
                >视频字幕</button>
                <div class="font-stepper">
                  <button @click="decreaseTranscriptFontScale">A-</button>
                  <button @click="increaseTranscriptFontScale">A+</button>
                </div>
              </div>
            </header>

            <div
              v-if="activeAudioTranscriptTimeline.length"
              ref="immersiveTranscriptListRef"
              class="transcript-list"
              :style="transcriptListStyle"
              @scroll="handleActiveTranscriptScroll"
            >
              <div
                v-for="item in visibleActiveAudioTranscriptTimeline"
                :key="`immersive-tr-${item.idx}`"
                class="transcript-line"
                :class="{ active: item.idx === currentTranscriptLineIndex, hovered: item.idx === hoveredTranscriptIndex }"
                :data-transcript-idx="item.idx"
                @click="seekActiveAudioToTranscript(item.idx)"
                @mouseenter="hoveredTranscriptIndex = item.idx"
                @mouseleave="hoveredTranscriptIndex = -1"
              >
                <span class="transcript-index">{{ String(item.idx + 1).padStart(2, '0') }}</span>
                <div class="transcript-body">
                  <div v-if="transcriptDisplayMode !== 'target'" class="transcript-source">{{ item.line.text }}</div>
                  <div v-if="transcriptDisplayMode !== 'source'" class="transcript-target">{{ resolveTranscriptTargetText(item.line) }}</div>
                </div>
              </div>
            </div>

            <div v-else class="transcript-empty">
              <div class="skeleton-stack">
                <span v-for="n in 7" :key="`sk-${n}`" :style="{ width: `${92 - (n % 3) * 11}%` }"></span>
              </div>
              <div class="transcript-empty-copy">
                <div class="eyebrow">WAITING FOR SYNC</div>
                <h3 class="serif">文稿还在路上</h3>
                <p>{{ activeAudioSyncState.message || '音频会先转写，再逐段翻译；你可以一边播放一边等。' }}</p>
                <div class="empty-progress" v-if="activeAudioSyncState.tone === 'running'">
                  <n-progress type="line" :percentage="Math.round(activeAudioSyncState.progress || 0)" :show-indicator="false" :height="4" />
                  <span>{{ Math.round(activeAudioSyncState.progress || 0) }}%</span>
                </div>
                <n-button v-else size="small" type="primary" @click="fetchTranscriptForActiveAudio">开始转写</n-button>
              </div>
            </div>
          </section>
        </div>
      </section>

      <main v-else-if="activeTab === 'review'" class="ll-review">
        <header class="review-head">
          <button class="studio-back" @click="activeTab = 'manager'">
            <Icon :icon="chevronLeftIcon" width="18" height="18" />
            <span>书架</span>
          </button>
          <div class="review-titleblock">
            <div class="eyebrow">REVIEW DESK</div>
            <h1 class="serif">复习训练</h1>
          </div>
          <div class="review-counter">{{ reviewRows.length ? `${reviewIndex + 1} / ${reviewRows.length}` : '0 / 0' }}</div>
        </header>

        <div class="review-grid">
          <aside class="review-side">
            <div class="review-stat">
              <span class="section-label">总学习项</span>
              <strong>{{ totalLearningItemCount }}</strong>
            </div>
            <div class="review-stat">
              <span class="section-label">当前掌握度</span>
              <strong>{{ currentReviewMasteryLabel }}</strong>
            </div>
            <div class="review-stat">
              <span class="section-label">今日到期</span>
              <strong>{{ dueReviewCount }}</strong>
              <span class="muted">新词 {{ newReviewCount }} 条</span>
            </div>
            <div class="review-stat">
              <span class="section-label">掌握占比</span>
              <strong>{{ masteryPercent }}%</strong>
              <div class="rail-meter"><span :style="{ width: `${masteryPercent}%` }"></span></div>
            </div>
            <div class="review-keys">
              <div><kbd>Space</kbd> 显示答案</div>
              <div><kbd>1</kbd> 认识 · <kbd>2</kbd> 模糊 · <kbd>3</kbd> 不认识</div>
              <div><kbd>←</kbd><kbd>→</kbd> 切换卡片</div>
            </div>
          </aside>

          <section class="review-main">
            <div class="review-typebar">
              <button
                v-for="opt in reviewTypeOptions"
                :key="opt.value"
                class="review-type-btn"
                :class="{ active: reviewType === opt.value, empty: !reviewTypeCounts[opt.value] }"
                @click="reviewType = opt.value"
              >
                <span>{{ opt.label }}</span>
                <span class="review-type-count">{{ reviewTypeCounts[opt.value] || 0 }}</span>
              </button>
            </div>

            <section v-if="currentReviewItem" class="review-card2">
              <header class="review-card-head">
                <span class="type-chip">{{ rowTypeLabel(currentReviewItem.rowType) }}</span>
                <div class="review-card-tools">
                  <button class="seg-btn" :class="{ active: reviewDictationMode }" @click="toggleReviewDictation">听写</button>
                </div>
              </header>

            <div class="review-front">
              <template v-if="currentReviewItem.rowType === 'word'">
                <template v-if="reviewDictationMode">
                  <div class="review-phonetic">{{ currentReviewItem.phonetic || '听发音，写出单词' }}</div>
                  <n-button size="small" @click="playWordPronunciation(currentReviewItem.title, 'us')">播放发音</n-button>
                  <div class="review-prompt">听发音，写出单词</div>
                </template>
                <template v-else>
                  <div class="review-word serif">{{ currentReviewItem.title }}</div>
                  <div class="review-phonetic">{{ currentReviewItem.phonetic || '暂无音标' }}</div>
                  <div class="mastery-dots large">{{ masteryDots(getMasteryLevel('record', currentReviewItem.rawId)) }}</div>
                  <div class="review-prompt">先回忆释义，再显示答案</div>
                </template>
              </template>
              <template v-else-if="currentReviewItem.rowType === 'listening'">
                <div class="review-listening">
                  <audio
                    ref="reviewAudioPlayerRef"
                    class="review-listening-player"
                    controls
                    :src="reviewAudioPlayableSrc || undefined"
                    @timeupdate="handleReviewAudioTimeUpdate"
                    @loadedmetadata="handleReviewAudioLoadedMetadata"
                    @canplay="handleListeningMediaCanPlay('review', $event)"
                    @play="handleListeningMediaPlay('review', $event)"
                    @error="handleListeningMediaError('review', $event)"
                  ></audio>
                  <div class="review-prompt">{{ reviewDictationMode ? '听音频，写下你听到的句子' : '先听，再显示原文与译文' }}</div>
                </div>
              </template>
              <template v-else>
                <blockquote v-if="!reviewDictationMode" class="review-text">{{ currentReviewItem.sourceText }}</blockquote>
                <div class="review-prompt">{{ reviewDictationMode ? '先回忆原文，写出你记得的内容' : '试着口头复述，再显示译文' }}</div>
              </template>
            </div>

            <div v-if="reviewDictationMode" class="dictation-box">
              <n-input
                v-model:value="reviewDictationInput"
                type="textarea"
                :autosize="{ minRows: 2, maxRows: 4 }"
                placeholder="写下你听到或记住的内容；Ctrl+Enter 对照原文"
                @keydown.ctrl.enter.prevent="checkDictation"
              />
              <div class="dictation-actions">
                <n-button size="small" @click="checkDictation">对照原文</n-button>
                <span v-if="reviewDictationResult" class="dictation-result" :class="{ ok: reviewDictationResult.ok }">
                  匹配 {{ reviewDictationResult.score }}% · {{ reviewDictationResult.ok ? '基本正确' : '有差异' }}
                </span>
              </div>
            </div>

            <transition name="reveal-answer">
              <div v-if="reviewShowAnswer" class="review-answer">
                <div class="answer-rule"></div>
                <template v-if="currentReviewItem.rowType === 'word'">
                  <div class="answer-meta">
                    <span>音标 {{ currentReviewItem.phonetic || '—' }}</span>
                    <span>词性 {{ currentReviewItem.partOfSpeech || '—' }}</span>
                  </div>
                  <div class="answer-text">{{ currentReviewItem.combinedTranslation || currentReviewItem.translatedText || '—' }}</div>
                  <div v-if="currentReviewItem.examplePairs?.length" class="answer-examples">
                    <div class="section-label">例句</div>
                    <div v-for="(x, i) in currentReviewItem.examplePairs" :key="`rv-ex-${i}`" class="example-line">
                      <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                      <div class="example-target" v-if="x.target">{{ x.target }}</div>
                    </div>
                  </div>
                </template>
                <template v-else-if="currentReviewItem.rowType === 'listening'">
                  <div class="answer-head">
                    <span class="section-label">原文与译文</span>
                    <div class="seg-group small">
                      <button
                        v-for="mode in transcriptDisplayModes"
                        :key="`rv-mode-${mode.value}`"
                        class="seg-btn"
                        :class="{ active: transcriptDisplayMode === mode.value }"
                        @click="transcriptDisplayMode = mode.value"
                      >{{ mode.label }}</button>
                    </div>
                  </div>
                  <div v-if="currentReviewListeningTranscriptTimeline.length" ref="reviewTranscriptListRef" class="transcript-list review-transcript-list" :style="transcriptListStyle" @scroll="handleReviewTranscriptScroll">
                    <div
                      v-for="item in visibleReviewListeningTranscriptTimeline"
                      :key="`rv-tr-${item.idx}`"
                      class="transcript-line"
                      :class="{ active: item.idx === currentReviewTranscriptIndex, hovered: item.idx === hoveredReviewTranscriptIndex }"
                      :data-transcript-idx="item.idx"
                      @mouseenter="hoveredReviewTranscriptIndex = item.idx"
                      @mouseleave="hoveredReviewTranscriptIndex = -1"
                    >
                      <span class="transcript-index">{{ String(item.idx + 1).padStart(2, '0') }}</span>
                      <div class="transcript-body">
                        <div v-if="transcriptDisplayMode !== 'target'" class="transcript-source">{{ item.line.text }}</div>
                        <div v-if="transcriptDisplayMode !== 'source'" class="transcript-target">{{ resolveTranscriptTargetText(item.line) }}</div>
                      </div>
                    </div>
                  </div>
                  <div v-else class="answer-text">同步数据尚未就绪，稍后再试。</div>
                </template>
                <template v-else>
                  <div class="answer-text">{{ currentReviewItem.summary || currentReviewItem.translatedText || '—' }}</div>
                </template>
              </div>
            </transition>

            <footer class="review-actions">
              <n-button size="small" @click="reviewShowAnswer = !reviewShowAnswer">{{ reviewShowAnswer ? '隐藏答案' : '显示答案' }}</n-button>
              <div class="review-grade">
                <n-button size="small" type="success" @click="gradeAndNext('known')">认识</n-button>
                <n-button size="small" type="warning" @click="gradeAndNext('fuzzy')">模糊</n-button>
                <n-button size="small" type="error" @click="gradeAndNext('unknown')">不认识</n-button>
              </div>
              <div class="review-nav">
                <n-button size="small" quaternary @click="gotoPrevReview">上一个</n-button>
                <n-button size="small" quaternary @click="gotoNextReview">下一个</n-button>
              </div>
            </footer>
            </section>

            <div v-else class="review-empty">
              <n-empty :description="reviewEmptyDescription" />
              <div class="review-empty-actions">
                <n-button v-if="reviewType !== 'mixed' && reviewTypeCounts.mixed" size="small" @click="reviewType = 'mixed'">
                  查看全部分类（{{ reviewTypeCounts.mixed }}）
                </n-button>
                <n-button size="small" type="primary" @click="resetReviewFilter">重置筛选并开始</n-button>
              </div>
            </div>
          </section>

          <aside class="review-filters">
            <div class="eyebrow">FILTER</div>
            <div class="filter-switch">
              <n-switch v-model:value="reviewFilter.dueOnly" size="small" />
              <span>只复习到期</span>
            </div>
            <div class="filter-switch">
              <n-switch v-model:value="reviewFilter.includeNew" size="small" :disabled="!reviewFilter.dueOnly" />
              <span>包含新词</span>
            </div>
            <label class="filter-field">
              <span>添加时间</span>
              <n-select v-model:value="reviewFilter.createdPeriod" :options="createdPeriodOptions" size="small" />
            </label>
            <label class="filter-field">
              <span>上次复习</span>
              <n-select v-model:value="reviewFilter.lastReviewPeriod" :options="lastReviewPeriodOptions" size="small" />
            </label>
            <label class="filter-field">
              <span>掌握程度</span>
              <n-select v-model:value="reviewFilter.masteryBand" :options="masteryBandOptions" size="small" />
            </label>
            <label class="filter-field">
              <span>排序方式</span>
              <n-select v-model:value="reviewFilter.orderBy" :options="reviewOrderOptions" size="small" />
            </label>
            <div class="review-filter-actions">
              <n-button size="small" type="primary" @click="startReviewFromSelection">应用并开始</n-button>
              <n-button size="small" @click="resetReviewFilter">重置</n-button>
              <n-button size="small" quaternary @click="shuffleReviewRows">洗牌</n-button>
            </div>
          </aside>
        </div>
      </main>

      <main v-else class="ll-workspace" :class="{ 'inspector-hidden': rightCollapsed }">
        <aside class="ll-rail">
          <section class="rail-today">
            <div class="eyebrow">TODAY</div>
            <div class="rail-today-line">
              <strong class="serif">{{ totalLearningItemCount }}</strong>
              <span>条学习记录</span>
            </div>
            <div class="rail-meter"><span :style="{ width: `${masteryPercent}%` }"></span></div>
            <div class="rail-today-meta">掌握 {{ masteryPercent }}% · 待复习 {{ dueReviewCount + newReviewCount }} · 听力 {{ listeningPendingCount }} 条待处理</div>
          </section>

          <section class="rail-group">
            <div class="eyebrow">SHELVES</div>
            <button
              v-for="it in leftShelfItems"
              :key="it.key"
              class="rail-shelf"
              :class="{ active: selectedShelf === it.key }"
              @click="selectedShelf = it.key"
            >
              <span class="rail-glyph">{{ shelfGlyph(it.key) }}</span>
              <span class="rail-shelf-label">{{ it.label }}</span>
              <span class="rail-count">{{ it.count }}</span>
            </button>
          </section>

          <section class="rail-group">
            <div class="eyebrow">ACTIONS</div>
            <button class="rail-action" @click="openCreateDialog">
              <Icon :icon="plusIcon" width="15" height="15" />
              <span>新建学习项</span>
            </button>
            <button class="rail-action" @click="openAudioManager">
              <Icon :icon="settingsIcon" width="15" height="15" />
              <span>采集 / 导入听力</span>
            </button>
            <button class="rail-action" @click="openDictionaryManager">
              <Icon :icon="languagesIcon" width="15" height="15" />
              <span>离线词典</span>
            </button>
            <button class="rail-action" @click="openRandomCheckBoard">
              <Icon :icon="shuffleIcon" width="15" height="15" />
              <span>随机抽查</span>
            </button>
            <button class="rail-action" @click="startDueReview">
              <Icon :icon="reviewIcon" width="15" height="15" />
              <span>开始今日复习</span>
            </button>
          </section>

          <div class="rail-foot">
            <span class="rail-dot"></span>
            本地数据库 · 离线可用
          </div>
        </aside>

        <section class="ll-ledger">
          <header class="ledger-head">
            <div class="ledger-titleblock">
              <h2 class="serif">{{ selectedShelfLabel }}</h2>
              <span class="ledger-subline">{{ ledgerSubline }}</span>
            </div>
            <div class="ledger-tools">
              <n-button v-if="selectedShelf === 'my_listening'" type="primary" size="small" @click="openCollectSourceDialog">
                <template #icon><Icon :icon="sparkleIcon" width="15" height="15" /></template>
                采集听力
              </n-button>
              <n-button v-else type="primary" size="small" @click="openCreateDialog">
                <template #icon><Icon :icon="plusIcon" width="15" height="15" /></template>
                新建词条
              </n-button>
              <n-dropdown trigger="click" :options="ledgerMenuOptions" @select="onLedgerMenuSelect">
                <n-button quaternary circle size="small" title="更多操作">
                  <Icon :icon="moreIcon" width="16" height="16" />
                </n-button>
              </n-dropdown>
              <n-button quaternary circle size="small" :title="rightCollapsed ? '展开检视器' : '收起检视器'" @click="rightCollapsed = !rightCollapsed">
                <Icon :icon="rightCollapsed ? chevronLeftIcon : chevronRightIcon" width="16" height="16" />
              </n-button>
            </div>
          </header>

          <div class="ledger-filterbar">
            <div class="filter-chips">
              <template v-if="selectedShelf === 'my_listening'">
                <button
                  v-for="opt in listeningStatusOptions"
                  :key="opt.value"
                  class="filter-chip"
                  :class="{ active: listeningStatusFilter === opt.value }"
                  @click="listeningStatusFilter = opt.value"
                >{{ opt.label }}</button>
              </template>
              <template v-else>
                <button class="filter-chip" :class="{ active: !keyword }" @click="keyword = ''">全部</button>
                <span class="filter-hint">{{ keyword ? `搜索「${keyword}」` : '按添加时间倒序' }}</span>
              </template>
            </div>
            <div class="filter-selects">
              <template v-if="selectedShelf === 'my_listening'">
                <n-select v-model:value="listeningKindFilter" :options="listeningKindOptions" size="small" style="width: 108px" />
                <n-select v-model:value="listeningProviderFilter" :options="listeningProviderOptions" size="small" style="width: 128px" />
                <n-select v-model:value="listeningSort" :options="listeningSortOptions" size="small" style="width: 112px" />
              </template>
              <n-button quaternary circle size="small" title="随机抽查当前列表" @click="openRandomCheckBoard">
                <Icon :icon="shuffleIcon" width="16" height="16" />
              </n-button>
            </div>
          </div>

          <div v-if="libraryLoading" class="ledger-loading">
            <n-spin size="small" />
            <span>正在翻开索引…</span>
          </div>

          <template v-else-if="managerMode === 'random'">
            <div class="random-board">
              <div class="random-board-head">
                <span class="type-chip">随机抽查</span>
                <span class="random-topic">{{ selectedShelfLabel }}</span>
                <span class="random-deck-index">{{ randomDecks.length ? `${randomDeckIndex + 1} / ${randomDecks.length}` : '0 / 0' }}</span>
                <div class="random-board-actions">
                  <n-button quaternary circle size="small" title="上一组（←）" @click="gotoPrevRandomDeck">
                    <Icon :icon="chevronLeftIcon" width="16" height="16" />
                  </n-button>
                  <n-button quaternary circle size="small" title="下一组（→）" @click="gotoNextRandomDeck">
                    <Icon :icon="chevronRightIcon" width="16" height="16" />
                  </n-button>
                  <n-button quaternary circle size="small" title="翻牌（空格）" @click="toggleCurrentDeckFlip">
                    <Icon :icon="reviewIcon" width="16" height="16" />
                  </n-button>
                  <n-button quaternary circle size="small" title="抽新卡（回车）" @click="drawRandomCards">
                    <Icon :icon="shuffleIcon" width="16" height="16" />
                  </n-button>
                  <n-button quaternary circle size="small" title="返回列表" @click="managerMode = 'list'">
                    <Icon :icon="listIcon" width="16" height="16" />
                  </n-button>
                </div>
              </div>

              <div class="random-cards-grid">
                <div class="random-board-card" v-for="card in currentRandomCards" :key="card.id" @click="toggleRandomCardFlip(card)">
                  <div class="random-card-flip" :class="{ flipped: isCardFlipped(card) }">
                    <div class="random-card-face front" :class="{ 'is-listening': card.rowType === 'listening' }">
                      <div class="random-card-type">{{ rowTypeLabel(card.rowType) }}</div>
                      <template v-if="card.rowType === 'word'">
                        <div class="random-card-word serif">{{ card.title }}</div>
                        <div class="random-card-phonetic">{{ card.phonetic || '—' }}</div>
                        <div class="random-card-text">{{ card.sourceText || '—' }}</div>
                      </template>
                      <template v-else-if="card.rowType === 'listening'">
                        <div class="random-card-word serif">{{ card.title }}</div>
                        <div class="random-card-text clamped">{{ card.summary || '—' }}</div>
                        <div class="random-card-audio-row">
                          <button class="random-audio-action" :class="randomCardAudioActionState(card)" :title="randomCardAudioActionTitle(card)" @click.stop="playRandomCardAudio(card)">
                            <Icon :icon="randomCardAudioActionIcon(card)" width="22" height="22" />
                          </button>
                        </div>
                      </template>
                      <template v-else>
                        <div class="random-card-text">{{ card.sourceText || '—' }}</div>
                      </template>
                    </div>

                    <div class="random-card-face back" :class="{ 'is-listening': card.rowType === 'listening' }">
                      <template v-if="card.rowType === 'word'">
                        <div class="random-card-head-row">
                          <div class="random-card-word serif">{{ card.title }}</div>
                          <span class="mastery-dots">{{ masteryDots(card.masteryLevel) }}</span>
                        </div>
                        <div class="random-card-meta-row">
                          <span v-if="card.phoneticUk || card.phonetic" class="phonetic-pill">英 {{ card.phoneticUk || card.phonetic }}</span>
                          <span v-if="card.phoneticUs || card.phonetic" class="phonetic-pill">美 {{ card.phoneticUs || card.phonetic }}</span>
                          <span v-if="card.partOfSpeech" class="type-chip">{{ card.partOfSpeech }}</span>
                        </div>
                        <div class="random-card-section">
                          <div class="section-label">释义</div>
                          <div class="random-card-content">{{ cardWordMeanings(card).join('；') || '—' }}</div>
                        </div>
                        <div class="random-card-section" v-if="cardWordPairs(card.payload?.synonyms, card.payload?.synonyms_translations).length">
                          <div class="section-label">近义词</div>
                          <div class="chip-wrap">
                            <span v-for="(x, i) in cardWordPairs(card.payload?.synonyms, card.payload?.synonyms_translations)" :key="`rd-s-${card.id}-${i}`" class="word-chip">{{ x.word }}<em v-if="x.translation"> · {{ x.translation }}</em></span>
                          </div>
                        </div>
                        <div class="random-card-section" v-if="cardWordPairs(card.payload?.antonyms, card.payload?.antonyms_translations).length">
                          <div class="section-label">反义词</div>
                          <div class="chip-wrap">
                            <span v-for="(x, i) in cardWordPairs(card.payload?.antonyms, card.payload?.antonyms_translations)" :key="`rd-a-${card.id}-${i}`" class="word-chip warn">{{ x.word }}<em v-if="x.translation"> · {{ x.translation }}</em></span>
                          </div>
                        </div>
                        <div class="random-card-section" v-if="card.examplePairs?.length">
                          <div class="section-label">例句</div>
                          <div class="example-line" v-for="(x, i) in card.examplePairs.slice(0, 3)" :key="`rd-e-${card.id}-${i}`">
                            <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                            <div class="example-target" v-if="x.target">{{ x.target }}</div>
                          </div>
                        </div>
                      </template>
                      <template v-else-if="card.rowType === 'listening'">
                        <div class="random-card-head-row">
                          <div class="random-card-word serif">{{ card.title }}</div>
                        </div>
                        <div class="random-card-section grow">
                          <div class="section-label">原文 / 摘要</div>
                          <div
                            v-if="randomCardTimelineFor(card).length"
                            class="random-card-transcript scrollable"
                            tabindex="0"
                            @keydown.stop="handleListeningCardScrollKeydown"
                          >
                            <div
                              v-for="(line, idx) in randomCardTimelineFor(card)"
                              :key="`rt-${card.id}-${idx}`"
                              class="random-transcript-line"
                              :class="{ active: isRandomTranscriptLineActive(card, idx) }"
                            >{{ line.text }}</div>
                          </div>
                          <div
                            v-else
                            class="random-card-content scrollable"
                            tabindex="0"
                            @keydown.stop="handleListeningCardScrollKeydown"
                          >{{ card.sourceText || card.summary || '—' }}</div>
                        </div>
                        <div class="random-card-audio-row">
                          <button class="random-audio-action" :class="randomCardAudioActionState(card)" :title="randomCardAudioActionTitle(card)" @click.stop="playRandomCardAudio(card)">
                            <Icon :icon="randomCardAudioActionIcon(card)" width="22" height="22" />
                          </button>
                        </div>
                      </template>
                      <template v-else>
                        <div class="random-card-section">
                          <div class="section-label">原文</div>
                          <div class="random-card-content">{{ card.sourceText || '—' }}</div>
                        </div>
                        <div class="random-card-section">
                          <div class="section-label">译文</div>
                          <div class="random-card-content">{{ card.translatedText || card.summary || '—' }}</div>
                        </div>
                      </template>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </template>

          <template v-else-if="selectedShelf === 'my_listening'">
            <div class="ledger-table listening-table">
              <div class="ledger-thead">
                <span class="col-check">
                  <n-checkbox :checked="allPageChecked" :indeterminate="somePageChecked" @update:checked="togglePageChecked" />
                </span>
                <span class="col-idx">#</span>
                <span class="col-main">标题 / 主题</span>
                <span class="col-duration">时长</span>
                <span class="col-status">同步</span>
                <span class="col-coverage">译文</span>
                <span class="col-open"></span>
              </div>
              <div
                v-for="(row, idx) in pagedListeningRows"
                :key="row.id"
                class="ledger-row listening-row"
                :class="{ active: activeRow?.id === row.id, selected: isRowChecked(row.id), error: syncStateForRow(row).tone === 'error' }"
                @click="selectListeningRow(row)"
                @dblclick="openListeningImmersive(row)"
              >
                <span class="col-check" @click.stop>
                  <n-checkbox :checked="isRowChecked(row.id)" @update:checked="(checked) => toggleRowChecked(row.id, checked)" />
                </span>
                <span class="col-idx">{{ String((listeningPage - 1) * listeningPageSize + idx + 1).padStart(2, '0') }}</span>
                <div class="col-main">
                  <div class="row-title serif">{{ row.title || '未命名听力' }}</div>
                  <div class="row-sub">
                    <span>{{ normalizeTopicForDisplay(row.payload?.topic) }}</span>
                    <span class="dot-sep">·</span>
                    <span>{{ row.payload?.provider || 'local' }}</span>
                  </div>
                </div>
                <span class="col-duration">{{ fmtDuration(row.payload?.duration_sec) }}</span>
                <span class="col-status">
                  <span class="sync-chip" :class="`tone-${syncStateForRow(row).tone}`" :title="syncStateForRow(row).message">
                    <span v-if="syncStateForRow(row).tone === 'running'" class="pulse-dot"></span>
                    {{ syncStateForRow(row).label }}
                  </span>
                  <span v-if="syncStateForRow(row).tone === 'running'" class="row-progress">
                    <span :style="{ width: `${syncStateForRow(row).progress}%` }"></span>
                  </span>
                </span>
                <span class="col-coverage">
                  <span class="coverage-track"><span :style="{ width: `${coveragePercent(row)}%` }"></span></span>
                  <em>{{ coveragePercent(row) }}%</em>
                </span>
                <span class="col-open" @click.stop="openListeningImmersive(row)">
                  <Icon :icon="playIcon" width="15" height="15" />
                </span>
              </div>
            </div>

            <div v-if="!pagedListeningRows.length" class="ledger-empty">
              <div class="empty-card">
                <div class="empty-mark">听</div>
                <h3 class="serif">这一排放着听力</h3>
                <p>按主题搜索公开音频，预览时长与来源后，勾选导入。导入后会在后台转写、翻译。</p>
                <n-button type="primary" size="small" @click="openCollectSourceDialog">去采集</n-button>
              </div>
            </div>

            <footer class="table-footer" v-if="filteredListeningRows.length">
              <n-pagination
                size="small"
                :page="listeningPage"
                :page-size="listeningPageSize"
                :item-count="filteredListeningRows.length"
                show-size-picker
                :page-sizes="[20, 50, 100, 200]"
                @update:page="onListeningPageChange"
                @update:page-size="onListeningPageSizeChange"
              />
            </footer>
          </template>

          <template v-else>
            <div class="ledger-table record-table">
              <div class="ledger-thead">
                <span class="col-check">
                  <n-checkbox :checked="allPageChecked" :indeterminate="somePageChecked" @update:checked="togglePageChecked" />
                </span>
                <span class="col-idx">#</span>
                <span class="col-type">类型</span>
                <span class="col-main">词目 / 释义</span>
                <span class="col-phon">音标</span>
                <span class="col-mastery">掌握</span>
                <span class="col-date">添加</span>
              </div>
              <div
                v-for="(row, idx) in filteredRows"
                :key="row.id"
                class="ledger-row record-row"
                :class="{ active: activeRow?.id === row.id, selected: isRowChecked(row.id) }"
                @click="activeRow = row"
                @dblclick="row.sourceType === 'record' ? openEditDialog(row) : null"
              >
                <span class="col-check" @click.stop>
                  <n-checkbox :checked="isRowChecked(row.id)" @update:checked="(checked) => toggleRowChecked(row.id, checked)" />
                </span>
                <span class="col-idx">{{ String((recordPage - 1) * recordPageSize + idx + 1).padStart(2, '0') }}</span>
                <span class="col-type"><span class="type-sigil">{{ typeGlyph(row.rowType) }}</span></span>
                <div class="col-main">
                  <div class="row-title serif">{{ row.title || row.sourceText }}</div>
                  <div class="row-translation">{{ row.combinedTranslation || row.translatedText || row.summary || '—' }}</div>
                </div>
                <span class="col-phon">{{ row.phonetic || row.phoneticUs || '—' }}</span>
                <span class="col-mastery"><span class="mastery-dots">{{ masteryDots(row.masteryLevel) }}</span></span>
                <span class="col-date">{{ fmtDate(row.createdAt) }}</span>
              </div>
            </div>

            <div v-if="!filteredRows.length" class="ledger-empty">
              <div class="empty-card">
                <div class="empty-mark">词</div>
                <h3 class="serif">还没有收进这一格</h3>
                <p>在阅读器里划词、翻译，或手动新建一条；这里会自动按单词、例句和段落归档。</p>
                <n-button type="primary" size="small" @click="openCreateDialog">新建一条</n-button>
              </div>
            </div>

            <footer class="table-footer" v-if="recordCounts.records">
              <n-pagination
                size="small"
                :page="recordPage"
                :page-size="recordPageSize"
                :item-count="recordCounts.records"
                show-size-picker
                :page-sizes="[50, 100, 200, 500]"
                @update:page="onRecordPageChange"
                @update:page-size="onRecordPageSizeChange"
              />
            </footer>
          </template>

          <div class="selection-bar" v-if="checkedRowKeys.length">
            <span class="selection-count">已选 {{ checkedRowKeys.length }} 项</span>
            <button class="selection-action" @click="startReviewFromSelection">开始复习</button>
            <button v-if="selectedShelf === 'my_listening'" class="selection-action" @click="batchRetrySync">重新同步</button>
            <button class="selection-action danger" @click="deleteSelectedRows">删除</button>
            <button class="selection-action muted" @click="checkedRowKeys = []">取消选择</button>
          </div>
        </section>

        <aside v-show="!rightCollapsed" class="ll-inspector">
          <header class="inspector-head">
            <span class="eyebrow">INSPECTOR</span>
            <button class="icon-ghost" title="收起" @click="rightCollapsed = true">
              <Icon :icon="chevronRightIcon" width="16" height="16" />
            </button>
          </header>

          <template v-if="activeWordRecord">
            <div class="word-hero">
              <div class="word-overline">{{ activeWordRecord.part_of_speech || 'WORD' }}</div>
              <h2 class="word-headword serif">{{ activeWordRecord.word || activeWordRecord.source_text }}</h2>
              <div class="word-phonetics">
                <span v-if="activeWordPhoneticUk" class="phonetic-pill">
                  英 {{ activeWordPhoneticUk }}
                  <button class="phonetic-play" title="英式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'uk')">
                    <Icon :icon="volumeUkIcon" width="13" height="13" />
                  </button>
                </span>
                <span v-if="activeWordPhoneticUs" class="phonetic-pill">
                  美 {{ activeWordPhoneticUs }}
                  <button class="phonetic-play" title="美式发音" @click="playWordPronunciation(activeWordRecord.word || activeWordRecord.source_text, 'us')">
                    <Icon :icon="volumeUsIcon" width="13" height="13" />
                  </button>
                </span>
              </div>
              <div class="word-mastery">
                <span class="mastery-dots">{{ masteryDots(getMasteryLevel('record', activeWordRecord.id)) }}</span>
                <span class="muted">掌握度</span>
                <span v-if="activeWordOfflinePack" class="offline-badge">离线 · {{ activeWordOfflinePack }}</span>
              </div>
            </div>

            <section class="inspector-section">
              <div class="section-label">释义</div>
              <ol class="meaning-list">
                <li v-for="(meaning, idx) in activeWordMeaningList" :key="`wm-${idx}`">{{ meaning }}</li>
                <li v-if="!activeWordMeaningList.length" class="muted">暂无释义</li>
              </ol>
            </section>

            <section class="inspector-section" v-if="activeWordPhrasePairs.length">
              <div class="section-label">常用短语</div>
              <div class="example-line" v-for="(x, i) in activeWordPhrasePairs" :key="`wd-p-${i}`">
                <div class="example-src">{{ x.source }}</div>
                <div class="example-target" v-if="x.target">{{ x.target }}</div>
              </div>
            </section>

            <section class="inspector-section" v-if="activeWordSynonyms.length || activeWordAntonyms.length || activeWordAssociates.length">
              <div class="section-label">词族</div>
              <div class="relation-group" v-if="activeWordSynonymPairs.length">
                <span class="relation-label">近义</span>
                <div class="chip-wrap">
                  <span v-for="(x, i) in activeWordSynonymPairs" :key="`wd-s-${i}`" class="word-chip">{{ x.word }}<em v-if="x.translation"> · {{ x.translation }}</em></span>
                </div>
              </div>
              <div class="relation-group" v-if="activeWordAntonymPairs.length">
                <span class="relation-label">反义</span>
                <div class="chip-wrap">
                  <span v-for="(x, i) in activeWordAntonymPairs" :key="`wd-a-${i}`" class="word-chip warn">{{ x.word }}<em v-if="x.translation"> · {{ x.translation }}</em></span>
                </div>
              </div>
              <div class="relation-group" v-if="activeWordAssociatePairs.length">
                <span class="relation-label">联想</span>
                <div class="chip-wrap">
                  <span v-for="(x, i) in activeWordAssociatePairs" :key="`wd-r-${i}`" class="word-chip">{{ x.word }}<em v-if="x.translation"> · {{ x.translation }}</em></span>
                </div>
              </div>
            </section>

            <section class="inspector-section" v-if="activeWordExamplePairs.length">
              <div class="section-label">例句</div>
              <div class="example-line" v-for="(x, i) in activeWordExamplePairs.slice(0, 8)" :key="`wd-e-${i}`">
                <div class="example-src">{{ i + 1 }}. {{ x.source }}</div>
                <div class="example-target" v-if="x.target">{{ x.target }}</div>
              </div>
            </section>

            <div class="inspector-actions">
              <n-button size="small" type="primary" @click="refreshWordMeta(activeWordRecord)">刷新拓展</n-button>
              <n-button size="small" @click="openDictionaryManager">离线词典</n-button>
              <n-button size="small" @click="openEditDialog(activeRow)">编辑</n-button>
              <n-button size="small" @click="applyReviewResult('known')">标记掌握</n-button>
              <n-button size="small" quaternary @click="deleteSelectedRows">删除</n-button>
            </div>
          </template>

          <template v-else-if="activeSentenceRecord">
            <div class="entry-hero">
              <div class="eyebrow">{{ rowTypeLabel(activeSentenceRecord.record_type) }}</div>
              <blockquote class="entry-source serif">{{ activeSentenceRecord.source_text }}</blockquote>
              <div class="entry-translation">{{ activeSentenceRecord.translated_text || '—' }}</div>
              <div class="entry-meta">
                <span>{{ fmtDate(activeSentenceRecord.created_at) }}</span>
                <span v-if="activeSentenceRecord.engine">{{ activeSentenceRecord.engine }}</span>
              </div>
            </div>
            <div class="inspector-actions">
              <n-button size="small" type="primary" @click="startReviewFromSelection">拿它复习</n-button>
              <n-button size="small" @click="openEditDialog(activeRow)">编辑</n-button>
              <n-button size="small" quaternary @click="deleteSelectedRows">删除</n-button>
            </div>
          </template>

          <template v-else-if="activeAudio">
            <div class="audio-hero">
              <div class="eyebrow">LISTENING · {{ activeAudio.provider || 'LOCAL' }}</div>
              <h2 class="audio-title serif">{{ activeAudio.title }}</h2>
              <div class="audio-meta">
                <span>{{ activeAudio.topic || '未标注主题' }}</span>
                <span class="dot-sep">·</span>
                <span>{{ fmtDuration(activeAudio.duration_sec) }}</span>
                <span class="dot-sep">·</span>
                <span>{{ fmtDate(activeAudio.created_at) }}</span>
              </div>
            </div>

            <section class="inspector-section sync-inspector">
              <div class="sync-inspector-head">
                <div class="section-label">同步状态</div>
                <span class="sync-chip" :class="`tone-${activeAudioSyncState.tone}`">
                  <span v-if="activeAudioSyncState.tone === 'running'" class="pulse-dot"></span>
                  {{ activeAudioSyncState.label }}
                </span>
              </div>
              <n-progress
                type="line"
                :percentage="Math.round(activeAudioSyncState.progress || 0)"
                :status="activeAudioSyncState.tone === 'error' ? 'error' : activeAudioSyncState.tone === 'ready' ? 'success' : 'info'"
                :show-indicator="false"
                :height="4"
              />
              <div class="sync-message">{{ activeAudioSyncState.message || '等待后台处理' }}</div>
              <div class="coverage-line">
                <span>译文 {{ formatCoverageLabel(activeAudio.translationCoverage) }}</span>
                <span class="coverage-track"><span :style="{ width: `${coveragePercent({ translationCoverage: activeAudio.translationCoverage })}%` }"></span></span>
              </div>
            </section>

            <section class="inspector-section" v-if="activeAudioPreviewLines.length">
              <div class="section-label">文稿预览</div>
              <div class="preview-transcript">
                <div v-for="line in activeAudioPreviewLines" :key="`ap-${line.idx}`" class="preview-transcript-line">
                  <div class="transcript-source">{{ line.text }}</div>
                  <div class="transcript-target">{{ resolveTranscriptTargetText(line) }}</div>
                </div>
              </div>
            </section>

            <section class="inspector-section" v-if="activeAudio.notes">
              <div class="section-label">备注</div>
              <div class="detail-content">{{ activeAudio.notes }}</div>
            </section>

            <div class="inspector-actions">
              <n-button size="small" type="primary" @click="enterListeningStudy">进入听力学习</n-button>
              <n-button size="small" @click="retranslateActiveAudio">重译</n-button>
              <n-button size="small" @click="fetchTranscriptForActiveAudio">重新同步</n-button>
              <n-button size="small" @click="editAudio(activeAudio)">编辑</n-button>
              <n-button size="small" quaternary @click="removeAudio(activeAudio)">删除</n-button>
            </div>
          </template>

          <div v-else class="inspector-empty">
            <div class="empty-mark">↘</div>
            <h3 class="serif">选中一条</h3>
            <p>词条会在这里展开释义、词族和例句；听力会显示同步进度与文稿预览。</p>
          </div>
        </aside>
      </main>

      <n-modal v-model:show="audioManagerVisible" preset="card" style="width: 720px" :title="audioForm.id ? '编辑听力资源' : '新增听力资源'">
        <div class="dialog-lede">本地音频会保留原文件；在线音频会在后台下载并转写。</div>
        <n-form :model="audioForm" label-placement="left" label-width="76">
          <n-form-item label="标题">
            <n-input v-model:value="audioForm.title" placeholder="例如：BBC 6 Minute English - Sleep" />
          </n-form-item>
          <div class="form-grid">
            <n-form-item label="类型">
              <n-select v-model:value="audioForm.source_kind" :options="audioKindOptions" />
            </n-form-item>
            <n-form-item label="主题">
              <n-input v-model:value="audioForm.topic" placeholder="新闻 / 科技 / 旅行…" />
            </n-form-item>
          </div>
          <n-form-item label="地址">
            <n-input v-model:value="audioForm.source_uri" placeholder="本地路径或在线 URL" />
          </n-form-item>
          <div class="form-grid">
            <n-form-item label="时长">
              <n-input-number v-model:value="audioForm.duration_sec" :min="0" placeholder="秒" />
            </n-form-item>
            <n-form-item label="来源">
              <n-input v-model:value="audioForm.provider" placeholder="archive / wikimedia / local" />
            </n-form-item>
          </div>
          <n-form-item label="转写">
            <n-input v-model:value="audioForm.transcript" type="textarea" :autosize="{ minRows: 2, maxRows: 5 }" placeholder="可留空，保存后自动转写" />
          </n-form-item>
          <n-form-item label="备注">
            <n-input v-model:value="audioForm.notes" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
          </n-form-item>
        </n-form>
        <template #action>
          <div class="dialog-actions">
            <n-button @click="importLocalAudio">导入本地音频</n-button>
            <n-button @click="openCollectSourceDialog">按主题采集</n-button>
            <span class="dialog-spacer"></span>
            <n-button @click="audioManagerVisible = false">取消</n-button>
            <n-button type="primary" @click="saveAudio">保存并转写</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="collectSourceDialogVisible" preset="card" style="width: 640px" title="按主题采集听力">
        <div class="collect-dialog">
          <div class="collect-topic-row">
            <n-input v-model:value="collectTopic" clearable placeholder="输入主题，例如：量子物理、咖啡文化、面试英语" @keydown.enter="confirmCollectListeningByTopic" />
            <n-select v-model:value="collectTopicCategory" :options="listeningTopicCategoryOptions" clearable placeholder="选分类" style="width: 132px" @update:value="onCollectTopicCategoryChange" />
          </div>
          <div class="collect-range">
            <div class="range-field">
              <span class="section-label">最小时长</span>
              <n-input-number v-model:value="collectMinMinutes" :min="0" :max="120" size="small" />
              <span class="range-unit">分钟</span>
            </div>
            <div class="range-field">
              <span class="section-label">最大时长</span>
              <n-input-number v-model:value="collectMaxMinutes" :min="1" :max="120" size="small" />
              <span class="range-unit">分钟</span>
            </div>
            <div class="range-field">
              <span class="section-label">目标条数</span>
              <n-input-number v-model:value="collectLimit" :min="1" :max="50" size="small" />
            </div>
          </div>
          <div class="collect-source-head">
            <span class="section-label">本次来源</span>
            <n-radio-group v-model:value="collectSourceMode" size="small">
              <n-radio-button value="default">全部启用</n-radio-button>
              <n-radio-button value="custom">自定义</n-radio-button>
            </n-radio-group>
            <span class="muted">{{ collectAvailableSourceOptions.length }} 个可用</span>
          </div>
          <n-select
            v-if="collectSourceMode === 'custom'"
            v-model:value="collectSourceIds"
            multiple
            clearable
            filterable
            :options="collectAvailableSourceOptions"
            placeholder="选择本次抓取来源"
          />
          <div v-if="!collectAvailableSourceOptions.length" class="source-hint">{{ collectSourceEmptyHint }}</div>
          <div v-else class="source-hint">时长未知的候选会保留并标注，导入前可自行勾选。</div>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="collectSourceDialogVisible = false">取消</n-button>
            <n-button type="primary" :loading="collectPreviewLoading" @click="confirmCollectListeningByTopic">搜索候选</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="collectPreviewVisible" preset="card" style="width: 880px" title="候选听力">
        <div class="preview-head">
          <n-checkbox :checked="collectPreviewAllSelected" @update:checked="togglePreviewAll">全选可导入项</n-checkbox>
          <span class="muted">已选 {{ collectPreviewSelected.length }} / {{ collectPreviewRows.length }}</span>
          <span class="preview-topic">{{ collectPreviewTopic }}</span>
        </div>
        <div class="preview-list">
          <div
            v-for="candidate in collectPreviewRows"
            :key="candidate.candidate_id"
            class="preview-row"
            :class="{ disabled: candidate.already_collected, picked: isPreviewSelected(candidate) }"
          >
            <n-checkbox
              :checked="isPreviewSelected(candidate)"
              :disabled="candidate.already_collected"
              @update:checked="(checked) => togglePreviewCandidate(candidate, checked)"
            />
            <div class="preview-main">
              <div class="preview-title">{{ candidate.title }}</div>
              <div class="preview-meta">
                <span>{{ candidate.provider }}</span>
                <span class="dot-sep">·</span>
                <span>{{ candidate.duration_sec ? fmtDuration(candidate.duration_sec) : '时长未知' }}</span>
                <span class="dot-sep">·</span>
                <span>相关度 {{ Math.round((candidate.relevance || 0) * 100) }}%</span>
              </div>
            </div>
            <span v-if="candidate.already_collected" class="preview-flag">已在库</span>
            <span v-else-if="candidate.has_transcript" class="preview-flag good">带原文</span>
          </div>
          <div v-if="!collectPreviewRows.length" class="preview-empty">没有找到符合时长与主题的候选，换个主题或放宽时长再试。</div>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="muted">导入后会依次进入转写队列</span>
            <span class="dialog-spacer"></span>
            <n-button @click="collectPreviewVisible = false">取消</n-button>
            <n-button type="primary" :loading="collectImporting" :disabled="!collectPreviewSelected.length" @click="importSelectedCandidates">
              导入 {{ collectPreviewSelected.length }} 条
            </n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="dictionaryManagerVisible" preset="card" style="width: 920px" title="离线词典">
        <div class="dict-panel">
          <div class="dict-pairline">
            <span class="type-chip">{{ learningLangLabel }} → {{ nativeLangLabel }}</span>
            <span class="muted">已安装 {{ dictPacks.length }} 个词典 · {{ dictTotalEntries }} 条词条</span>
            <span class="dialog-spacer"></span>
            <span v-if="dictRunning" class="sync-chip tone-running">
              <span class="pulse-dot"></span>
              后台安装中
            </span>
          </div>
          <div class="source-hint">目录资源来自 GitHub / MDBG；大陆直连失败时，请先在设置里启用系统或自定义代理。</div>

          <section class="dict-section">
            <div class="section-label">在线目录</div>
            <div class="dict-catalog">
              <article v-for="item in dictCatalog" :key="item.id" class="dict-card" :class="{ installed: item.installed }">
                <div class="dict-card-head">
                  <span class="dict-name serif">{{ item.name }}</span>
                  <span class="type-chip">{{ item.source_lang }} → {{ item.target_lang }}</span>
                  <span v-if="item.installed" class="preview-flag good">已安装 {{ item.installed_entry_count }} 条</span>
                </div>
                <p class="dict-desc">{{ item.description }}</p>
                <div class="dict-meta">{{ fmtBytes(item.size_bytes) }} · {{ item.license }} · v{{ item.version }}</div>
                <div v-if="dictJobFor(item.id) && !['done', 'error'].includes(dictJobFor(item.id).stage)" class="dict-progress">
                  <n-progress type="line" :percentage="Math.round(dictJobFor(item.id).progress || 0)" :show-indicator="false" :height="4" />
                  <span>{{ dictJobFor(item.id).message || '处理中' }}</span>
                </div>
                <div v-else-if="dictJobFor(item.id)?.stage === 'error'" class="dict-error">{{ dictJobFor(item.id).error }}</div>
                <div class="dict-actions">
                  <n-button
                    v-if="!item.installed"
                    size="small"
                    type="primary"
                    :disabled="Boolean(dictJobFor(item.id) && !['done', 'error'].includes(dictJobFor(item.id).stage))"
                    @click="installDict(item.id)"
                  >下载并建立索引</n-button>
                  <n-button v-else size="small" @click="installDict(item.id)">重新安装</n-button>
                  <n-button v-if="item.installed" size="small" @click="exportDictById(item.id)">导出</n-button>
                </div>
              </article>
            </div>
          </section>

          <section class="dict-section">
            <div class="section-label">已安装</div>
            <div v-if="!dictPacks.length" class="source-hint">
              还没有安装离线词典。安装 ECDICT 后，常见英文词条在断网时也能直接出释义、音标和词性。
            </div>
            <div v-else class="dict-pack-list">
              <div v-for="pack in dictPacks" :key="pack.id" class="dict-pack-row">
                <div class="dict-pack-main">
                  <div class="dict-pack-name">{{ pack.name }}</div>
                  <div class="dict-meta">{{ pack.source_lang }} → {{ pack.target_lang }} · {{ pack.entry_count }} 条 · {{ pack.license }}</div>
                </div>
                <n-button size="tiny" @click="exportDictById(pack.id)">导出</n-button>
                <button class="icon-ghost danger" title="删除词典" @click="removeDict(pack)">
                  <Icon :icon="trashIcon" width="15" height="15" />
                </button>
              </div>
            </div>
          </section>

          <section class="dict-section">
            <div class="section-label">导入词典包</div>
            <div class="dict-import-row">
              <n-input v-model:value="dictImportName" size="small" placeholder="包名称（可选）" style="flex: 1" />
              <n-select v-model:value="dictImportSource" size="small" :options="learningLangOptions" style="width: 112px" />
              <n-select v-model:value="dictImportTarget" size="small" :options="nativeLangOptions" style="width: 112px" />
              <n-button size="small" :loading="dictImporting" @click="importDictPackage">选择文件导入</n-button>
            </div>
            <div class="source-hint">支持本应用导出的 ZIP（manifest + entries.csv），也支持 term,phonetic,pos,meanings,examples,synonyms 表头的 CSV 或 JSON 数组。</div>
          </section>

          <section class="dict-section">
            <div class="section-label">本地查词测试</div>
            <div class="dict-lookup-row">
              <n-input v-model:value="dictLookupWord" size="small" placeholder="输入单词或中文词，回车查询" @keydown.enter="lookupDictWord" />
              <n-button size="small" type="primary" :loading="dictLookupLoading" @click="lookupDictWord">查询</n-button>
            </div>
            <div v-if="dictLookupRows.length" class="dict-lookup-list">
              <div v-for="(entry, idx) in dictLookupRows" :key="`dict-${idx}-${entry.term}`" class="dict-entry">
                <div class="dict-entry-head">
                  <span class="dict-term serif">{{ entry.term }}</span>
                  <span v-if="entry.phonetic" class="muted">{{ entry.phonetic }}</span>
                  <span v-if="entry.pos" class="type-chip">{{ entry.pos }}</span>
                  <span class="preview-flag">{{ entry.pack_name }}</span>
                </div>
                <div class="dict-entry-meanings">{{ entry.meanings.join('；') }}</div>
              </div>
            </div>
            <div v-else class="muted dict-lookup-empty">
              {{ dictLookupSearched
                ? `没有找到「${dictLookupWord.trim()}」的词条（当前语言对 ${learningLangLabel} → ${nativeLangLabel}）。`
                : '查询结果会显示在这里；没有对应语言对的词典时为空。' }}
            </div>
          </section>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="dictionaryManagerVisible = false">完成</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="asrManagerVisible" preset="card" style="width: 880px" title="语音识别模型">
        <div class="dict-panel">
          <div class="dict-pairline">
            <span class="type-chip">学习语言：{{ learningLangLabel }}</span>
            <span class="muted">当前模型：{{ currentAsrLabel }}</span>
            <span class="dialog-spacer"></span>
            <span v-if="asrRunning" class="sync-chip tone-running">
              <span class="pulse-dot"></span>
              后台安装中
            </span>
          </div>
          <div class="source-hint">
            英语优先使用随包内置的英文模型；中 / 日 / 韩优先 SenseVoice；其它语言使用 Whisper 多语种。安装后按学习语言自动路由。
          </div>

          <section class="dict-section">
            <div class="section-label">可下载模型</div>
            <div class="dict-catalog">
              <article v-for="item in asrCatalog" :key="item.id" class="dict-card" :class="{ installed: item.installed }">
                <div class="dict-card-head">
                  <span class="dict-name serif">{{ item.name }}</span>
                  <span class="type-chip">{{ item.languages }}</span>
                  <span v-if="item.installed" class="preview-flag good">已安装</span>
                </div>
                <p class="dict-desc">{{ item.description }}</p>
                <div class="dict-meta">{{ fmtBytes(item.size_bytes) }}</div>
                <div v-if="asrJobFor(item.id) && !['done', 'error'].includes(asrJobFor(item.id).stage)" class="dict-progress">
                  <n-progress type="line" :percentage="Math.round(asrJobFor(item.id).progress || 0)" :show-indicator="false" :height="4" />
                  <span>{{ asrJobFor(item.id).message || '处理中' }}</span>
                </div>
                <div v-else-if="asrJobFor(item.id)?.stage === 'error'" class="dict-error">{{ asrJobFor(item.id).error }}</div>
                <div class="dict-actions">
                  <n-button
                    size="small"
                    :type="item.installed ? 'default' : 'primary'"
                    :disabled="Boolean(asrJobFor(item.id) && !['done', 'error'].includes(asrJobFor(item.id).stage))"
                    @click="installAsr(item.id)"
                  >{{ item.installed ? '重新安装' : '下载模型' }}</n-button>
                  <n-button v-if="item.installed" size="small" @click="removeAsr(asrModelById(item.id))">删除</n-button>
                </div>
              </article>
            </div>
          </section>

          <section class="dict-section">
            <div class="section-label">已安装模型</div>
            <div class="dict-pack-list">
              <div v-for="model in asrModels" :key="model.id" class="dict-pack-row">
                <div class="dict-pack-main">
                  <div class="dict-pack-name">{{ model.name }}</div>
                  <div class="dict-meta">{{ model.languages }} · {{ fmtBytes(model.size_bytes) }}<span v-if="model.builtin"> · 随包内置</span></div>
                </div>
                <span v-if="model.builtin" class="preview-flag">内置</span>
                <button v-else class="icon-ghost danger" title="删除模型" @click="removeAsr(model)">
                  <Icon :icon="trashIcon" width="15" height="15" />
                </button>
              </div>
            </div>
          </section>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="asrManagerVisible = false">完成</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="webImportVisible" preset="card" style="width: 860px" title="从网页导入">
        <div class="dict-panel">
          <div class="source-hint">
            只抓取你当前能访问的公开页面，不代管账号、不绕过登录或付费墙。JS 渲染页面如果正文为空，可以直接把正文粘贴到下方。
          </div>
          <div class="dict-lookup-row">
            <n-input v-model:value="webImportUrl" placeholder="https://…" @keydown.enter="previewWebImport" />
            <n-button type="primary" :loading="webImportLoading" @click="previewWebImport">提取</n-button>
          </div>

          <template v-if="webImportPreview">
            <div class="web-preview-head">
              <div>
                <div class="eyebrow">{{ webImportPreview.site }}</div>
                <h3 class="serif">{{ webImportPreview.title || '未命名页面' }}</h3>
              </div>
              <span class="type-chip">{{ webImportPreview.word_count }} 词</span>
            </div>
            <n-form label-placement="left" label-width="72">
              <n-form-item label="标题">
                <n-input v-model:value="webImportTitle" />
              </n-form-item>
              <n-form-item label="音频">
                <n-select
                  v-model:value="webImportAudioUrl"
                  clearable
                  :options="webImportAudioOptions"
                  placeholder="没有识别到音频；也可以粘贴直链"
                />
              </n-form-item>
              <n-form-item label="正文">
                <n-input
                  v-model:value="webImportText"
                  type="textarea"
                  :autosize="{ minRows: 5, maxRows: 12 }"
                  placeholder="正文为空时，直接粘贴网页正文"
                />
              </n-form-item>
            </n-form>
            <div class="web-options">
              <n-checkbox v-model:checked="webImportOptions.listening" :disabled="!webImportAudioUrl">导入为听力</n-checkbox>
              <n-checkbox v-model:checked="webImportOptions.reading" :disabled="!webImportHasText">导入为阅读段落</n-checkbox>
              <n-checkbox v-model:checked="webImportOptions.translate">同时翻译成母语</n-checkbox>
            </div>
            <div v-if="webImportPreview.subtitle_text" class="source-hint">
              识别到字幕，导入听力时会优先把字幕作为初始文稿。
            </div>
          </template>
          <n-empty v-else description="输入网址后点「提取」；提取失败时也可以直接粘贴正文导入阅读" />
          <n-form v-if="!webImportPreview" label-placement="left" label-width="72">
            <n-form-item label="标题">
              <n-input v-model:value="webImportTitle" placeholder="可选" />
            </n-form-item>
            <n-form-item label="正文">
              <n-input
                v-model:value="webImportText"
                type="textarea"
                :autosize="{ minRows: 6, maxRows: 14 }"
                placeholder="直接粘贴网页正文；只导入你本来就能访问的内容"
              />
            </n-form-item>
          </n-form>
          <div v-if="!webImportPreview" class="web-options">
            <n-checkbox v-model:checked="webImportOptions.reading" :disabled="!webImportHasText">导入为阅读段落</n-checkbox>
            <n-checkbox v-model:checked="webImportOptions.translate">同时翻译成母语</n-checkbox>
          </div>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="webImportVisible = false">取消</n-button>
            <n-button
              type="primary"
              :loading="webImporting"
              :disabled="!webImportPreview && !webImportHasText"
              @click="importWebContent"
            >导入</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="sourceManagerVisible" preset="card" style="width: 900px" title="听力源管理器">
        <div class="dialog-lede">RSS 地址可以用 <code>{topic}</code> 占位符；时长上限用于过滤过长的节目。</div>
        <div class="source-manager-head">
          <n-input v-model:value="newSourceName" size="small" placeholder="来源名称" style="width: 158px" />
          <n-input v-model:value="newSourceProvider" size="small" placeholder="标识" style="width: 96px" />
          <n-input v-model:value="newSourceFeedUrl" size="small" placeholder="RSS 地址，可用 {topic}" style="flex: 1" />
          <n-select v-model:value="newSourceLanguage" size="small" style="width: 108px" :options="learningLangOptions" />
          <n-input-number v-model:value="newSourceMaxDurationMin" size="small" :min="1" :max="30" style="width: 86px" />
          <n-button size="small" type="primary" @click="addListeningSource">新增</n-button>
        </div>
        <div class="source-manager-list">
          <div class="source-item" v-for="src in listeningSources" :key="src.id">
            <n-switch v-model:value="src.enabled" size="small" @update:value="persistListeningSources" />
            <n-input v-model:value="src.name" size="small" style="width: 160px" @blur="persistListeningSources" />
            <n-input v-model:value="src.provider" size="small" style="width: 92px" @blur="persistListeningSources" />
            <n-input v-model:value="src.feed_url" size="small" style="flex: 1" @blur="persistListeningSources" />
            <n-select v-model:value="src.language_tag" size="small" style="width: 104px" :options="learningLangOptions" @update:value="persistListeningSources" />
            <n-input-number v-model:value="src.max_duration_min" size="small" :min="1" :max="30" style="width: 84px" @update:value="persistListeningSources" />
            <button class="icon-ghost danger" title="删除来源" @click="removeListeningSource(src.id)">
              <Icon :icon="trashIcon" width="15" height="15" />
            </button>
          </div>
        </div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button type="primary" @click="sourceManagerVisible = false">完成</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="recordEditVisible" preset="card" style="width: 620px" :title="recordEditMode === 'edit' ? '编辑学习记录' : '新增学习记录'">
        <n-form :model="recordForm" label-placement="left" label-width="84">
          <n-form-item label="记录类型">
            <n-select v-model:value="recordForm.record_type" :options="recordKindOptions.filter((x) => x.value !== 'all' && x.value !== 'listening')" />
          </n-form-item>
          <n-form-item label="原文">
            <n-input v-model:value="recordForm.source_text" type="textarea" :autosize="{ minRows: 3, maxRows: 8 }" />
          </n-form-item>
          <n-form-item label="译文">
            <n-input v-model:value="recordForm.translated_text" type="textarea" :autosize="{ minRows: 3, maxRows: 8 }" />
          </n-form-item>
          <div class="form-grid">
            <n-form-item label="来源语言">
              <n-input v-model:value="recordForm.source_lang" />
            </n-form-item>
            <n-form-item label="目标语言">
              <n-input v-model:value="recordForm.target_lang" />
            </n-form-item>
          </div>
          <n-form-item label="引擎">
            <n-input v-model:value="recordForm.engine" />
          </n-form-item>
        </n-form>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="recordEditVisible = false">取消</n-button>
            <n-button type="primary" @click="saveRecord">保存</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="confirmDialog.show" preset="card" style="width: 420px" :title="confirmDialog.title">
        <div class="confirm-body">{{ confirmDialog.message }}</div>
        <template #action>
          <div class="dialog-actions">
            <span class="dialog-spacer"></span>
            <n-button @click="confirmDialog.show = false">取消</n-button>
            <n-button type="error" @click="runConfirm">确认删除</n-button>
          </div>
        </template>
      </n-modal>
    </div>
  </n-message-provider>
</template>

<script setup>
import { computed, h, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import speechIcon from '@iconify-icons/lucide/messages-square'
import badgeInfoIcon from '@iconify-icons/lucide/badge-info'
import volumeUsIcon from '@iconify-icons/lucide/volume-2'
import volumeUkIcon from '@iconify-icons/lucide/volume-1'
import chevronLeftIcon from '@iconify-icons/lucide/chevron-left'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import moreIcon from '@iconify-icons/lucide/more-horizontal'
import settingsIcon from '@iconify-icons/lucide/settings-2'
import importIcon from '@iconify-icons/lucide/download'
import exportIcon from '@iconify-icons/lucide/upload'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import sparkleIcon from '@iconify-icons/lucide/sparkles'
import shuffleIcon from '@iconify-icons/lucide/shuffle'
import listIcon from '@iconify-icons/lucide/list'
import playIcon from '@iconify-icons/lucide/play'
import pauseIcon from '@iconify-icons/lucide/pause'
import languagesIcon from '@iconify-icons/lucide/languages'
import plusIcon from '@iconify-icons/lucide/plus'
import editIcon from '@iconify-icons/lucide/pencil'
import trashIcon from '@iconify-icons/lucide/trash-2'
import reviewIcon from '@iconify-icons/lucide/book-open-check'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NDropdown,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NMessageProvider,
  NModal,
  NPagination,
  NProgress,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  useMessage,
} from 'naive-ui'

const message = useMessage()
const activeTab = ref('manager')
const keyword = ref('')
const learningLangStorageKey = 'guaxi_language_learning_target_lang'
const listeningSourceStorageKey = 'guaxi_language_listening_sources_v1'
// 学习语言同时决定听力源的语言筛选，默认值若与已配置来源不符，开箱状态下永远抓不到资源
const pickLearningLangFromStoredSources = () => {
  try {
    const rows = JSON.parse(window.localStorage.getItem(listeningSourceStorageKey) || '[]')
    if (!Array.isArray(rows)) return ''
    const counts = new Map()
    rows.filter((x) => x && x.enabled !== false).forEach((x) => {
      const tag = String(x?.language_tag || '').trim().toLowerCase()
      if (!tag || tag === 'all') return
      counts.set(tag, (counts.get(tag) || 0) + 1)
    })
    const top = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]
    return top?.[0] || ''
  } catch {
    return ''
  }
}
const learningLang = ref(
  window.localStorage.getItem(learningLangStorageKey) || pickLearningLangFromStoredSources() || 'en'
)
const nativeLangStorageKey = 'guaxi_language_learning_native_lang'
const nativeLang = ref(window.localStorage.getItem(nativeLangStorageKey) || 'zh')
const selectedShelf = ref('all')
const records = ref([])
const audioResources = ref([])
const activeRow = ref(null)
const managerMode = ref('list')
const leftCollapsed = ref(false)
const rightCollapsed = ref(false)
const checkedRowKeys = ref([])
const listeningKindFilter = ref('all')
const listeningProviderFilter = ref('all')
const listeningStatusFilter = ref('all')
const listeningSort = ref('recent')
const collectTopicCategory = ref('')
const collectTopic = ref('')
const collectLimit = ref(15)
const collectMinMinutes = ref(2)
const collectMaxMinutes = ref(15)
const collectListeningLoading = ref(false)
const collectPreviewVisible = ref(false)
const collectPreviewRows = ref([])
const collectPreviewSelected = ref([])
const collectPreviewTopic = ref('')
const collectPreviewLoading = ref(false)
const collectImporting = ref(false)
const libraryLoading = ref(false)
const syncJobs = ref({})
const dictionaryManagerVisible = ref(false)
const dictPacks = ref([])
const dictCatalog = ref([])
const dictJobs = ref({})
const dictLookupWord = ref('')
const dictLookupRows = ref([])
const dictLookupSearched = ref(false)
const dictLookupLoading = ref(false)
const dictImportName = ref('')
const dictImportSource = ref(learningLang.value)
const dictImportTarget = ref(nativeLang.value)
const dictImporting = ref(false)
const activeWordOfflinePack = ref('')
let dictPollTimer = 0
const asrManagerVisible = ref(false)
const asrModels = ref([])
const asrCatalog = ref([])
const asrJobs = ref({})
let asrPollTimer = 0
const webImportVisible = ref(false)
const webImportUrl = ref('')
const webImportPreview = ref(null)
const webImportTitle = ref('')
const webImportText = ref('')
const webImportAudioUrl = ref('')
const webImportSubtitleUrl = ref('')
const webImportLoading = ref(false)
const webImporting = ref(false)
const webImportOptions = reactive({ listening: true, reading: true, translate: true })
const exportingLearningData = ref(false)
const importingLearningData = ref(false)
const listeningImmersiveMode = ref(false)
const playbackRate = ref(1)
const recordPage = ref(1)
const recordPageSize = ref(100)
const listeningPage = ref(1)
const listeningPageSize = ref(100)
const recordCounts = reactive({ records: 0, audio_resources: 0 })
const reviewType = ref('mixed')
const reviewRows = ref([])
const reviewIndex = ref(0)
const reviewShowAnswer = ref(false)
const reviewDictationMode = ref(false)
const reviewDictationInput = ref('')
const reviewDictationResult = ref(null)
let reviewRecordsSnapshot = null
const masteryStore = ref({ record: {}, audio: {} })
const shelfCounts = reactive({ all: 0, my_vocab: 0, my_sentences: 0, my_paragraphs: 0, my_listening: 0 })
const audioManagerVisible = ref(false)
const randomDecks = ref([])
const randomDeckIndex = ref(0)
const randomFlippedMap = ref({})
const randomCardAudioPlayingId = ref('')
const randomCardAudioPaused = ref(false)
const randomCardTimeline = ref([])
const randomCardTimelineCardId = ref('')
const randomCardActiveLineIndex = ref(-1)
const currentTranscriptLineIndex = ref(0)
const hoveredTranscriptIndex = ref(-1)
const activeAudioPlayerRef = ref(null)
const activeAudioDurationSec = ref(0)
const activeAudioSyncTimeline = ref([])
const activeAudioPlayableSrc = ref('')
const activeAudioSyncGenerating = ref(false)
const activeTimelineQuickTranslating = ref(false)
const syncQueueRunning = ref(false)
const syncQueueVisualRunning = ref(false)
const syncQueueTotal = ref(0)
const syncQueueDone = ref(0)
const currentSyncingAudioId = ref('')
const reviewAudioPlayerRef = ref(null)
const reviewAudioDurationSec = ref(0)
const currentReviewTranscriptIndex = ref(0)
const hoveredReviewTranscriptIndex = ref(-1)
const reviewAudioPlayableSrc = ref('')
const reviewAudioSyncTimeline = ref([])
const reviewTimelineQuickTranslating = ref(false)
const quickTimelineTranslateCooldownMs = 12000
const quickTimelineTranslateLastRun = new Map()
const immersiveTranscriptListRef = ref(null)
const reviewTranscriptListRef = ref(null)
const transcriptScreenPageSize = 36
const transcriptScreenPreloadSize = 28
const activeTranscriptVisibleCount = ref(transcriptScreenPageSize)
const reviewTranscriptVisibleCount = ref(transcriptScreenPageSize)
const transcriptDisplayMode = ref('both')
const subtitleDisplayMode = ref('bilingual')
const transcriptFontScale = ref(1.16)
const syncingAudioIdSet = new Set()
const syncQueuedIdSet = new Set()
const syncQueuedTasks = []
const syncRequeueCooldownMs = 45000
const syncLastEnqueueAtMap = new Map()
let syncQueueHideTimer = 0
let syncQueueRunStartAt = 0
let pendingSyncSweepTimer = 0
const pendingSyncSweepRunning = ref(false)
const audioTranslationCoverageMap = ref({})
let audioTranslationCoverageRefreshSerial = 0
const sourceManagerVisible = ref(false)
let randomCardAudioPlayer = null
const listeningSources = ref([])
const newSourceName = ref('')
const newSourceProvider = ref('')
const newSourceFeedUrl = ref('')
const newSourceLanguage = ref('en')
const newSourceMaxDurationMin = ref(30)
const collectSourceDialogVisible = ref(false)
const collectSourceMode = ref('default')
const collectSourceIds = ref([])
const confirmDialog = reactive({ show: false, title: '', message: '', action: null })
let syncPollTimer = 0
let syncCompleteRefreshTimer = 0

const reviewFilter = reactive({
  createdPeriod: 'all',
  lastReviewPeriod: 'all',
  masteryBand: 'all',
  orderBy: 'created_desc',
  dueOnly: true,
  includeNew: true,
})

const recordEditVisible = ref(false)
const recordEditMode = ref('create')
const recordForm = reactive({
  id: '',
  source_text: '',
  translated_text: '',
  record_type: 'sentence',
  source_lang: 'auto',
  target_lang: nativeLang.value,
  engine: 'Manual',
})

const recordKindOptions = [
  { label: '全部', value: 'all' },
  { label: '单词', value: 'word' },
  { label: '例句', value: 'sentence' },
  { label: '段落', value: 'paragraph' },
  { label: '听力', value: 'listening' },
]

const learningLangOptions = [
  { label: '学习中文', value: 'zh' },
  { label: '学习英语', value: 'en' },
  { label: '学习日语', value: 'ja' },
  { label: '学习韩语', value: 'ko' },
  { label: '学习法语', value: 'fr' },
  { label: '学习德语', value: 'de' },
  { label: '学习西语', value: 'es' },
  { label: '学习俄语', value: 'ru' },
]

const nativeLangOptions = [
  { label: '母语中文', value: 'zh' },
  { label: '母语英语', value: 'en' },
  { label: '母语日语', value: 'ja' },
  { label: '母语韩语', value: 'ko' },
  { label: '母语法语', value: 'fr' },
  { label: '母语德语', value: 'de' },
  { label: '母语西语', value: 'es' },
  { label: '母语俄语', value: 'ru' },
]

const audioKindOptions = [
  { label: '在线音频', value: 'remote' },
  { label: '本地文件', value: 'local' },
]

const listeningProviderOptions = [
  { label: '全部来源', value: 'all' },
  { label: 'Archive', value: 'archive' },
  { label: 'Wikimedia', value: 'wikimedia' },
  { label: '本地导入', value: 'local' },
]
const listeningKindOptions = [
  { label: '全部音频', value: 'all' },
  { label: '在线音频', value: 'remote' },
  { label: '本地文件', value: 'local' },
]
const listeningStatusOptions = [
  { label: '全部', value: 'all' },
  { label: '待转写', value: 'pending' },
  { label: '处理中', value: 'running' },
  { label: '待翻译', value: 'partial' },
  { label: '已就绪', value: 'ready' },
  { label: '失败', value: 'error' },
]
const listeningSortOptions = [
  { label: '最近添加', value: 'recent' },
  { label: '时长最长', value: 'duration' },
  { label: '标题', value: 'title' },
]
const listeningTopicCategoryOptions = [
  { label: '新闻', value: 'news' },
  { label: '科技', value: 'tech' },
  { label: '科学', value: 'science' },
  { label: '数学', value: 'math' },
  { label: '物理', value: 'physics' },
  { label: '历史', value: 'history' },
  { label: '地理', value: 'geography' },
  { label: '文化', value: 'culture' },
  { label: '宗教', value: 'religion' },
  { label: '生活', value: 'life' },
  { label: '交通', value: 'transport' },
  { label: '旅游', value: 'travel' },
  { label: '音乐', value: 'music' },
  { label: '电影', value: 'movie' },
]
const resolveTopicCategoryLabel = (value) => {
  const key = String(value || '').trim()
  if (!key) return ''
  const hit = listeningTopicCategoryOptions.find((x) => x.value === key)
  return String(hit?.label || '').trim()
}

const playbackRateOptions = [
  { label: '0.75x', value: 0.75 },
  { label: '1.0x', value: 1 },
  { label: '1.25x', value: 1.25 },
  { label: '1.5x', value: 1.5 },
  { label: '1.75x', value: 1.75 },
  { label: '2.0x', value: 2 },
]

const transcriptDisplayModes = [
  { label: '原文+译文', value: 'both' },
  { label: '仅原文', value: 'source' },
  { label: '仅译文', value: 'target' },
]
const subtitleDisplayModes = [
  { label: '原文字幕', value: 'source' },
  { label: '译文字幕', value: 'target' },
  { label: '双语字幕', value: 'bilingual' },
]

const defaultListeningSources = () => ([
  { id: 'src_ted_default', name: 'TED Audio', provider: 'ted', feed_url: 'https://feeds.feedburner.com/TEDTalks_audio', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_bbc_default', name: 'BBC Learning', provider: 'bbc', feed_url: 'https://podcasts.files.bbci.co.uk/p02pc9tn.rss', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_cnn_default', name: 'CNN 5 Things', provider: 'cnn', feed_url: 'https://rss.art19.com/cnn-5-things', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_npr_upfirst_default', name: 'NPR Up First', provider: 'npr', feed_url: 'https://feeds.npr.org/510318/podcast.xml', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_npr_shortwave_default', name: 'NPR Short Wave', provider: 'npr_science', feed_url: 'https://feeds.npr.org/510351/podcast.xml', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_npr_planetmoney_default', name: 'NPR Planet Money', provider: 'npr_economy', feed_url: 'https://feeds.npr.org/510289/podcast.xml', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_abc_science_default', name: 'ABC Science Show', provider: 'abc_science', feed_url: 'https://www.abc.net.au/feeds/2889546/podcast.xml', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_bbc_global_news_default', name: 'BBC Global News', provider: 'bbc_news', feed_url: 'https://podcasts.files.bbci.co.uk/p02nq0gn.rss', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_in_our_time_default', name: 'BBC In Our Time', provider: 'bbc_culture', feed_url: 'https://podcasts.files.bbci.co.uk/b006qykl.rss', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_nasa_hwhap_default', name: 'NASA Podcast', provider: 'nasa', feed_url: 'https://www.nasa.gov/podcasts/houston-we-have-a-podcast/feed/', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_vergecast_default', name: 'The Vergecast', provider: 'tech_verge', feed_url: 'https://feeds.megaphone.fm/vergecast', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_science_vs_default', name: 'Science Vs', provider: 'science_vs', feed_url: 'https://feeds.megaphone.fm/sciencevs', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_this_american_life_default', name: 'This American Life', provider: 'culture_life', feed_url: 'https://feed.thisamericanlife.org/talpodcast', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_99pi_default', name: '99% Invisible', provider: 'design_history', feed_url: 'https://feeds.99percentinvisible.org/99percentinvisible', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_song_exploder_default', name: 'Song Exploder', provider: 'music_song', feed_url: 'https://feed.songexploder.net/SongExploder', language_tag: 'en', max_duration_min: 30, enabled: true },
  { id: 'src_amateur_traveler_default', name: 'Amateur Traveler', provider: 'travel', feed_url: 'https://amateurtraveler.com/feed/podcast/', language_tag: 'en', max_duration_min: 30, enabled: true },
])

const normalizeListeningSource = (src, fallbackId = '') => ({
  id: String(src?.id || fallbackId || `src_${Date.now()}_${Math.random().toString(16).slice(2, 7)}`),
  name: String(src?.name || '').trim(),
  provider: String(src?.provider || '').trim().toLowerCase(),
  feed_url: String(src?.feed_url || '').trim(),
  language_tag: String(src?.language_tag || 'en').trim().toLowerCase(),
  max_duration_min: Math.min(30, Math.max(1, Number(src?.max_duration_min || 30))),
  enabled: Boolean(src?.enabled),
})
const collectAvailableSourceOptions = computed(() => {
  const lang = String(learningLang.value || '').toLowerCase()
  return listeningSources.value
    .filter((x) => x.enabled && x.feed_url)
    .filter((x) => !x.language_tag || x.language_tag === 'all' || x.language_tag === lang)
    .map((x) => ({
      label: `${x.name} (${x.provider || 'source'})`,
      value: x.id,
    }))
})
const collectAvailableSourcePayload = computed(() => {
  const lang = String(learningLang.value || '').toLowerCase()
  return listeningSources.value
    .filter((x) => x.enabled && x.feed_url)
    .filter((x) => !x.language_tag || x.language_tag === 'all' || x.language_tag === lang)
    .map((x) => ({
      id: x.id,
      provider: x.provider,
      name: x.name,
      feed_url: x.feed_url,
      language_tag: x.language_tag,
      max_duration_sec: Math.round(Math.min(30, Math.max(1, Number(x.max_duration_min || 30))) * 60),
    }))
})
const resolveLearningLangLabel = (value) => {
  const key = String(value || '').trim().toLowerCase()
  if (!key) return ''
  const hit = learningLangOptions.find((x) => x.value === key)
  return String(hit?.label || key)
}
const learningLangLabel = computed(() => resolveLearningLangLabel(learningLang.value).replace(/^学习/, ''))
const nativeLangLabel = computed(() => resolveLearningLangLabel(nativeLang.value).replace(/^学习/, ''))
const collectSourceEmptyHint = computed(() => {
  const enabled = listeningSources.value.filter((x) => x.enabled && x.feed_url)
  if (!enabled.length) return '当前没有已启用的听力源，请先在听力源管理器启用来源。'
  const lang = String(learningLang.value || '').trim().toLowerCase()
  const otherTags = [...new Set(enabled
    .map((x) => String(x.language_tag || '').trim().toLowerCase())
    .filter((t) => t && t !== 'all' && t !== lang))]
  const otherLabels = otherTags.map((t) => resolveLearningLangLabel(t)).filter(Boolean)
  const otherText = otherLabels.length ? `，现有来源为「${otherLabels.join('、')}」` : ''
  return `已启用 ${enabled.length} 个听力源，但语言都与当前学习语言「${learningLangLabel.value}」不匹配${otherText}。请在听力源管理器修改来源语言，或切换顶部学习语言。`
})
const mergeListeningSourcesWithDefaults = (currentList) => {
  const current = (Array.isArray(currentList) ? currentList : [])
    .map((x, idx) => normalizeListeningSource(x, `src_${idx}`))
    .filter((x) => x.name && x.feed_url)
  const defaults = defaultListeningSources().map((x, idx) => normalizeListeningSource(x, `src_default_${idx}`))
  const byKey = new Map()
  current.forEach((x) => {
    const key = `${x.provider}|${x.feed_url}`.toLowerCase()
    if (!byKey.has(key)) byKey.set(key, x)
  })
  defaults.forEach((x) => {
    const key = `${x.provider}|${x.feed_url}`.toLowerCase()
    if (!byKey.has(key)) byKey.set(key, x)
  })
  return Array.from(byKey.values())
}

const loadListeningSources = () => {
  try {
    const raw = window.localStorage.getItem(listeningSourceStorageKey)
    if (!raw) {
      listeningSources.value = defaultListeningSources()
      return
    }
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed) || !parsed.length) {
      listeningSources.value = defaultListeningSources()
      return
    }
    listeningSources.value = mergeListeningSourcesWithDefaults(parsed)
    try {
      window.localStorage.setItem(listeningSourceStorageKey, JSON.stringify(listeningSources.value))
    } catch {}
    if (!listeningSources.value.length) listeningSources.value = defaultListeningSources()
  } catch {
    listeningSources.value = defaultListeningSources()
  }
}

const persistListeningSources = () => {
  listeningSources.value = listeningSources.value.map((x, idx) => normalizeListeningSource(x, `src_${idx}`)).filter((x) => x.name && x.feed_url)
  try {
    window.localStorage.setItem(listeningSourceStorageKey, JSON.stringify(listeningSources.value))
  } catch {}
}

const addListeningSource = () => {
  const name = String(newSourceName.value || '').trim()
  const provider = String(newSourceProvider.value || '').trim().toLowerCase()
  const feedUrl = String(newSourceFeedUrl.value || '').trim()
  if (!name || !provider || !feedUrl) {
    message.warning('请填写来源名称、来源标识和RSS地址')
    return
  }
  listeningSources.value = [
    ...listeningSources.value,
    normalizeListeningSource({
      id: `src_${Date.now()}_${Math.random().toString(16).slice(2, 7)}`,
      name,
      provider,
      feed_url: feedUrl,
      language_tag: newSourceLanguage.value,
      max_duration_min: newSourceMaxDurationMin.value,
      enabled: true,
    }),
  ]
  persistListeningSources()
  newSourceName.value = ''
  newSourceProvider.value = ''
  newSourceFeedUrl.value = ''
}

const removeListeningSource = (id) => {
  listeningSources.value = listeningSources.value.filter((x) => x.id !== id)
  persistListeningSources()
}
const onCollectTopicCategoryChange = (value) => {
  const label = resolveTopicCategoryLabel(value)
  if (!label) return
  const current = String(collectTopic.value || '').trim()
  if (!current) {
    collectTopic.value = label
  }
}

const createdPeriodOptions = [
  { label: '全部时间', value: 'all' },
  { label: '最近3天', value: '3d' },
  { label: '最近7天', value: '7d' },
  { label: '最近30天', value: '30d' },
]

const lastReviewPeriodOptions = [
  { label: '全部', value: 'all' },
  { label: '未复习过', value: 'never' },
  { label: '最近7天复习', value: '7d' },
  { label: '最近30天复习', value: '30d' },
]

const masteryBandOptions = [
  { label: '全部', value: 'all' },
  { label: '未掌握(0-1)', value: 'low' },
  { label: '模糊(2-3)', value: 'mid' },
  { label: '已掌握(4-5)', value: 'high' },
]

const reviewOrderOptions = [
  { label: '添加时间新到旧', value: 'created_desc' },
  { label: '添加时间旧到新', value: 'created_asc' },
  { label: '上次复习最久优先', value: 'review_oldest' },
]

const audioForm = reactive({
  id: '',
  title: '',
  source_kind: 'remote',
  source_uri: '',
  provider: '',
  topic: '',
  language_tag: learningLang.value,
  duration_sec: 0,
  transcript: '',
  sync_json_uri: '',
  notes: '',
})

const masteryKey = 'guaxi_language_learning_mastery_v1'

const toNum = (v) => {
  const n = Number(v)
  return Number.isFinite(n) ? n : 0
}

const loadMasteryStore = () => {
  try {
    const raw = window.localStorage.getItem(masteryKey) || ''
    if (!raw) return
    const parsed = JSON.parse(raw)
    masteryStore.value = {
      record: parsed?.record && typeof parsed.record === 'object' ? parsed.record : {},
      audio: parsed?.audio && typeof parsed.audio === 'object' ? parsed.audio : {},
    }
  } catch {
    masteryStore.value = { record: {}, audio: {} }
  }
}

const saveMasteryStore = () => {
  try {
    window.localStorage.setItem(masteryKey, JSON.stringify(masteryStore.value))
  } catch {}
}

const getMasteryEntry = (sourceType, rawId) => {
  const box = masteryStore.value[sourceType] || {}
  const key = String(rawId || '')
  const row = box[key]
  if (!row || typeof row !== 'object') {
    return {
      level: 0,
      reviewed: 0,
      mastered: 0,
      lastResult: '',
      lastReviewedAt: '',
      dueAt: '',
      intervalDays: 0,
      ease: 2.5,
      reps: 0,
      lapses: 0,
    }
  }
  return {
    level: toNum(row.level),
    reviewed: toNum(row.reviewed),
    mastered: toNum(row.mastered),
    lastResult: String(row.lastResult || ''),
    lastReviewedAt: String(row.lastReviewedAt || ''),
    dueAt: String(row.dueAt || ''),
    intervalDays: toNum(row.intervalDays),
    ease: toNum(row.ease) || 2.5,
    reps: toNum(row.reps),
    lapses: toNum(row.lapses),
  }
}

const upsertMasteryEntry = (sourceType, rawId, result) => {
  const key = String(rawId || '')
  if (!key) return
  const current = getMasteryEntry(sourceType, key)
  let level = current.level
  if (result === 'known') level += 1
  if (result === 'unknown') level -= 1
  level = Math.min(5, Math.max(0, level))

  // SM-2 lite：认识=5、模糊=3、不认识=1。
  // 模糊和不认识都算这次没通过，但不认识会额外记一次 lapse；到期时间用小数天，
  // 这样「10 分钟后再看」也能落到同一条复习队列里。
  const quality = result === 'known' ? 5 : result === 'fuzzy' ? 3 : 1
  let ease = Math.max(1.3, current.ease + (0.1 - (5 - quality) * (0.08 + (5 - quality) * 0.02)))
  let reps = current.reps
  let lapses = current.lapses
  let interval = current.intervalDays
  if (quality < 3) {
    reps = 0
    lapses += 1
    interval = 10 / 1440
  } else {
    reps += 1
    if (reps === 1) interval = 1
    else if (reps === 2) interval = 3
    else interval = Math.max(1, Math.round(Math.max(1, interval) * ease))
  }
  const dueAt = new Date(Date.now() + Math.max(0.0069, interval) * 86400000).toISOString()

  const next = {
    level,
    reviewed: current.reviewed + 1,
    mastered: current.mastered + (result === 'known' ? 1 : 0),
    lastResult: result,
    lastReviewedAt: new Date().toISOString(),
    dueAt,
    intervalDays: interval,
    ease,
    reps,
    lapses,
  }
  masteryStore.value = {
    ...masteryStore.value,
    [sourceType]: {
      ...(masteryStore.value[sourceType] || {}),
      [key]: next,
    },
  }
  saveMasteryStore()
}

const masteryStars = (level) => {
  const n = Math.min(5, Math.max(0, toNum(level)))
  return `${'★'.repeat(n)}${'☆'.repeat(5 - n)}`
}

const masteryDots = (level) => {
  const n = Math.min(5, Math.max(0, Math.round(toNum(level))))
  return `${'●'.repeat(n)}${'○'.repeat(5 - n)}`
}

const shelfGlyph = (key) => {
  if (key === 'my_vocab') return '词'
  if (key === 'my_sentences') return '句'
  if (key === 'my_paragraphs') return '段'
  if (key === 'my_listening') return '听'
  return '全'
}

const typeGlyph = (t) => {
  if (t === 'word') return '词'
  if (t === 'paragraph') return '段'
  if (t === 'listening') return '听'
  return '句'
}

const fmtDate = (raw) => {
  if (!raw) return '—'
  const ts = new Date(raw)
  if (Number.isNaN(ts.getTime())) return String(raw).slice(0, 10) || '—'
  return new Intl.DateTimeFormat('zh-CN', { year: '2-digit', month: '2-digit', day: '2-digit' }).format(ts)
}

const askConfirm = (title, message, action) => {
  confirmDialog.title = title
  confirmDialog.message = message
  confirmDialog.action = action
  confirmDialog.show = true
}

const runConfirm = async () => {
  const action = confirmDialog.action
  confirmDialog.show = false
  confirmDialog.action = null
  if (typeof action === 'function') await action()
}

const rowTypeLabel = (t) => {
  if (t === 'word') return '单词'
  if (t === 'paragraph') return '段落'
  if (t === 'listening') return '听力'
  return '例句'
}

const normalizeText = (v) => String(v || '').trim().toLowerCase()
const normalizeTopicForDisplay = (v) => {
  const raw = String(v || '').trim()
  if (!raw) return '未标注'
  const compact = raw.replace(/\s+/g, ' ').trim()
  const tokens = compact.split(' ').filter(Boolean)
  if (tokens.length >= 4 && tokens.length % 2 === 0) {
    const half = tokens.length / 2
    const a = tokens.slice(0, half).join(' ')
    const b = tokens.slice(half).join(' ')
    if (a === b) return a
  }
  return compact
}
const splitSimpleTranslations = (text) => {
  const src = String(text || '').trim()
  if (!src) return []
  const parts = src.split(/[;；,，、/|｜]/g).map((x) => String(x || '').trim()).filter(Boolean)
  const out = []
  parts.forEach((p) => {
    if (p.length > 16) return
    if (!out.some((x) => x === p)) out.push(p)
  })
  return out.slice(0, 5)
}

const allRows = computed(() => {
  const recRows = (Array.isArray(records.value) ? records.value : []).map((r) => {
    const m = getMasteryEntry('record', r.id)
    return {
      id: `record:${r.id}`,
      rawId: r.id,
      sourceType: 'record',
      rowType: String(r.record_type || 'sentence'),
      title: r.is_word ? String(r.word || r.source_text || '') : String(r.source_text || '').slice(0, 36),
      sourceText: String(r.source_text || ''),
      translatedText: String(r.translated_text || ''),
      summary: String(r.translated_text || ''),
      phonetic: String(r.phonetic || ''),
      phoneticUk: String(r.phonetic_uk || ''),
      phoneticUs: String(r.phonetic_us || ''),
      partOfSpeech: String(r.part_of_speech || ''),
      combinedTranslation: normalizeList(Array.isArray(r.meanings) && r.meanings.length ? r.meanings : splitSimpleTranslations(r.translated_text)).join('；'),
      note: String((r.examples && r.examples[0]) || ''),
      createdAt: String(r.created_at || ''),
      examples: Array.isArray(r.examples) ? r.examples : [],
      examplePairs: (() => {
        const sources = Array.isArray(r.examples) ? r.examples : []
        const targets = Array.isArray(r.example_translations) ? r.example_translations : []
        const list = sources.map((source, idx) => ({
          source: String(source || '').trim(),
          target: String(targets[idx] || '').trim(),
        })).filter((x) => x.source)
        if (list.length) return list
        const fallback = String(((Array.isArray(r.phrases) ? r.phrases : [])[0]) || '').trim() || `This is a ${String(r.word || r.source_text || '').trim()}.`
        const fallbackTarget = String(((Array.isArray(r.phrase_translations) ? r.phrase_translations : [])[0]) || '').trim()
          || normalizeList(Array.isArray(r.meanings) ? r.meanings : splitSimpleTranslations(r.translated_text)).slice(0, 1).join('')
        return [{ source: fallback, target: fallbackTarget }]
      })(),
      phrasePairs: (Array.isArray(r.phrases) ? r.phrases : []).map((source, idx) => ({
        source: String(source || '').trim(),
        target: String((Array.isArray(r.phrase_translations) ? r.phrase_translations : [])[idx] || '').trim(),
      })),
      payload: r,
      masteryLevel: m.level,
      reviewed: m.reviewed,
      mastered: m.mastered,
      lastReviewedAt: m.lastReviewedAt,
    }
  })
  const audioRows = (Array.isArray(audioResources.value) ? audioResources.value : []).map((r) => {
    const m = getMasteryEntry('audio', r.id)
    return {
      id: `audio:${r.id}`,
      rawId: r.id,
      sourceType: 'audio',
      rowType: 'listening',
      title: String(r.title || ''),
      sourceText: String(r.transcript || r.notes || r.source_uri || ''),
      translatedText: String(r.notes || ''),
      summary: String(r.transcript || r.notes || ''),
      phonetic: '',
      note: String(r.source_uri || ''),
      translationCoverage: audioTranslationCoverageMap.value[String(r.id || '')] || { translated: 0, total: 0, ratio: 0 },
      createdAt: String(r.created_at || ''),
      examples: [],
      payload: r,
      masteryLevel: m.level,
      reviewed: m.reviewed,
      mastered: m.mastered,
      lastReviewedAt: m.lastReviewedAt,
    }
  })
  return [...recRows, ...audioRows]
})

const matchesKeyword = (row) => {
  const kw = normalizeText(keyword.value)
  if (!kw) return true
  return [row.title, row.sourceText, row.summary, row.note].some((x) => normalizeText(x).includes(kw))
}

const shelfFilteredRows = computed(() => {
  const rows = allRows.value
  if (selectedShelf.value === 'all') return rows
  if (selectedShelf.value === 'my_vocab') return rows.filter((r) => r.rowType === 'word')
  if (selectedShelf.value === 'my_sentences') return rows.filter((r) => r.rowType === 'sentence')
  if (selectedShelf.value === 'my_paragraphs') return rows.filter((r) => r.rowType === 'paragraph')
  if (selectedShelf.value === 'my_listening') return rows.filter((r) => r.rowType === 'listening')
  return rows
})

const filteredRows = computed(() => {
  return shelfFilteredRows.value.filter(matchesKeyword)
})

const leftShelfItems = computed(() => {
  return [
    { key: 'all', label: '全部分类', count: shelfCounts.all },
    { key: 'my_vocab', label: '我的单词本', count: shelfCounts.my_vocab },
    { key: 'my_sentences', label: '我摘录的例句', count: shelfCounts.my_sentences },
    { key: 'my_paragraphs', label: '我摘录的段落', count: shelfCounts.my_paragraphs },
    { key: 'my_listening', label: '我的听力', count: shelfCounts.my_listening },
  ]
})

const listeningRows = computed(() => {
  let rows = shelfFilteredRows.value.filter(matchesKeyword).filter((r) => r.rowType === 'listening')
  if (listeningKindFilter.value !== 'all') {
    rows = rows.filter((r) => String(r.payload?.source_kind || '').toLowerCase() === listeningKindFilter.value)
  }
  if (listeningProviderFilter.value !== 'all') {
    rows = rows.filter((r) => String(r.payload?.provider || '').toLowerCase() === listeningProviderFilter.value)
  }
  return rows
})

const syncStageLabel = (stage) => {
  const map = {
    queued: '排队中',
    prepare: '准备音频',
    download: '获取音频',
    transcribe: '转写中',
    translate: '翻译中',
    timeline: '整理时间轴',
    save: '写入数据',
    done: '已就绪',
    error: '失败',
  }
  return map[String(stage || '')] || '处理中'
}

const syncStateForRow = (row) => {
  const id = String(row?.rawId || '').trim()
  const job = id ? syncJobs.value[id] : null
  if (job && job.stage && job.stage !== 'done' && job.stage !== 'error') {
    return {
      tone: 'running',
      label: syncStageLabel(job.stage),
      progress: Math.max(0, Math.min(100, Number(job.progress || 0))),
      message: String(job.message || ''),
    }
  }
  if (job?.stage === 'error') {
    return {
      tone: 'error',
      label: '失败',
      progress: 0,
      message: String(job.error || job.message || '转写失败'),
    }
  }
  if (row?.rowType === 'listening' && row.payload?.audio_available === false) {
    return {
      tone: 'error',
      label: '音频缺失',
      progress: 0,
      message: '本地音频文件已不存在，请重新导入该条目',
    }
  }
  const coverage = row?.translationCoverage || { translated: 0, total: 0, ratio: 0 }
  const hasSync = Boolean(String(row?.payload?.sync_json_uri || '').trim())
  if (coverage.total > 0) {
    if (coverage.ratio >= 0.98 && String(row?.payload?.transcript || '').trim()) {
      return { tone: 'ready', label: '已就绪', progress: 100, message: '文稿与译文完整' }
    }
    return {
      tone: 'partial',
      label: coverage.ratio > 0 ? `待翻译 ${Math.round(Number(coverage.ratio || 0) * 100)}%` : '待翻译',
      progress: Math.round(Number(coverage.ratio || 0) * 100),
      message: '已有文稿，译文尚未补齐',
    }
  }
  if (hasSync) {
    return { tone: 'partial', label: '待翻译', progress: 0, message: '同步数据存在但缺少译文' }
  }
  return { tone: 'pending', label: '待转写', progress: 0, message: '尚未生成文稿' }
}

const filteredListeningRows = computed(() => {
  let rows = [...listeningRows.value]
  if (listeningStatusFilter.value !== 'all') {
    rows = rows.filter((row) => syncStateForRow(row).tone === listeningStatusFilter.value)
  }
  if (listeningSort.value === 'duration') {
    rows.sort((a, b) => Number(b.payload?.duration_sec || 0) - Number(a.payload?.duration_sec || 0))
  } else if (listeningSort.value === 'title') {
    rows.sort((a, b) => String(a.title || '').localeCompare(String(b.title || ''), 'zh-CN'))
  } else {
    rows.sort((a, b) => new Date(b.createdAt || 0).getTime() - new Date(a.createdAt || 0).getTime())
  }
  return rows
})

const listeningPendingCount = computed(() => {
  return allRows.value
    .filter((row) => row.rowType === 'listening')
    .filter((row) => {
      const tone = syncStateForRow(row).tone
      return tone === 'pending' || tone === 'partial' || tone === 'error'
    })
    .length
})

const coveragePercent = (row) => {
  const ratio = Number(row?.translationCoverage?.ratio || 0)
  return Math.max(0, Math.min(100, Math.round(ratio * 100)))
}

const ledgerSubline = computed(() => {
  if (selectedShelf.value === 'my_listening') {
    const running = listeningRows.value.filter((row) => syncStateForRow(row).tone === 'running').length
    return `${listeningRows.value.length} 条 · 处理中 ${running} · 待处理 ${listeningPendingCount.value}`
  }
  return `${recordCounts.records} 条${keyword.value ? ' · 搜索结果' : ''}`
})

const activeAudioSyncState = computed(() => {
  const row = allRows.value.find((item) => item.id === activeRow.value?.id)
  if (!row) {
    return { tone: 'pending', label: '等待开始', progress: 0, message: '' }
  }
  return syncStateForRow(row)
})

const activeAudioPreviewLines = computed(() => {
  const timeline = activeAudioTranscriptTimeline.value
  return Array.isArray(timeline) ? timeline.slice(0, 4).map((line, idx) => ({ ...line, idx })) : []
})

const activeWordMeaningList = computed(() => {
  return String(activeWordCombinedTranslation.value || '')
    .split(/[；;]/)
    .map((x) => x.trim())
    .filter(Boolean)
})

const pageRows = computed(() => {
  return selectedShelf.value === 'my_listening' ? filteredListeningRows.value.slice(
    (Math.max(1, listeningPage.value) - 1) * listeningPageSize.value,
    Math.max(1, listeningPage.value) * listeningPageSize.value
  ) : filteredRows.value
})

const isRowChecked = (id) => checkedRowKeys.value.includes(id)

const toggleRowChecked = (id, checked) => {
  const set = new Set(checkedRowKeys.value)
  if (checked) set.add(id)
  else set.delete(id)
  checkedRowKeys.value = [...set]
}

const allPageChecked = computed(() => {
  const rows = pageRows.value
  return rows.length > 0 && rows.every((row) => isRowChecked(row.id))
})

const somePageChecked = computed(() => {
  return pageRows.value.some((row) => isRowChecked(row.id)) && !allPageChecked.value
})

const togglePageChecked = (checked) => {
  const ids = pageRows.value.map((row) => row.id)
  const set = new Set(checkedRowKeys.value)
  ids.forEach((id) => {
    if (checked) set.add(id)
    else set.delete(id)
  })
  checkedRowKeys.value = [...set]
}

const pagedListeningRows = computed(() => {
  const pageSize = Math.max(1, Number(listeningPageSize.value || 100))
  const page = Math.max(1, Number(listeningPage.value || 1))
  const offset = (page - 1) * pageSize
  return filteredListeningRows.value.slice(offset, offset + pageSize)
})

const isListeningImmersive = computed(() => {
  return selectedShelf.value === 'my_listening' && listeningImmersiveMode.value
})

const isListeningImmersiveReady = computed(() => {
  return isListeningImmersive.value && Boolean(activeAudio.value)
})

const fmtDuration = (secRaw) => {
  const sec = Math.max(0, Number(secRaw || 0))
  if (!Number.isFinite(sec) || sec <= 0) return '—'
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${m}:${String(s).padStart(2, '0')}`
}

const sanitizeMediaUri = (raw) => {
  const text = String(raw || '').trim()
  if (!text) return ''
  const unwrapped = text
    .replace(/^[`'"\s]+/, '')
    .replace(/[`'"\s]+$/, '')
    .replace(/^<+/, '')
    .replace(/>+$/, '')
    .trim()
  return unwrapped
}

const normalizeLocalFilePath = (rawPath) => {
  const cleaned = sanitizeMediaUri(rawPath)
  if (!cleaned) return ''
  if (/^[a-zA-Z]:[\\/]/.test(cleaned)) return cleaned
  if (cleaned.startsWith('/')) {
    const hit = cleaned.match(/^\/([a-zA-Z]:\/.*)$/)
    if (hit?.[1]) return hit[1]
  }
  return cleaned
}

const inferMediaKindByPath = (rawPath) => {
  const path = String(rawPath || '').toLowerCase()
  if (/\.(mp4|webm|mov|mkv|m4v)(\?|#|$)/.test(path)) return 'video'
  return 'audio'
}

const bytesFromBase64 = (base64Text) => {
  const raw = atob(String(base64Text || ''))
  const out = new Uint8Array(raw.length)
  for (let i = 0; i < raw.length; i += 1) {
    out[i] = raw.charCodeAt(i)
  }
  return out
}

const handleWindowResizeObserverWarning = (evt) => {
  const msg = String(evt?.message || '')
  if (!msg.includes('ResizeObserver loop completed with undelivered notifications')) return
  if (typeof evt.preventDefault === 'function') {
    evt.preventDefault()
  }
  if (typeof evt.stopImmediatePropagation === 'function') {
    evt.stopImmediatePropagation()
  }
}

const resolvePlayableMediaSrc = async (raw) => {
  const cleaned = sanitizeMediaUri(raw)
  if (!cleaned) return ''
  if (/^(https?:|data:|blob:|asset:)/i.test(cleaned)) {
    console.info('[LangLearning] media:direct-source', {
      protocol: cleaned.split(':')[0],
      sourceKind: inferMediaKindByPath(cleaned),
    })
    return cleaned
  }
  const localPath = normalizeLocalFilePath(
    cleaned.startsWith('file://')
      ? decodeURIComponent(cleaned.replace(/^file:\/\//i, ''))
      : cleaned
  )
  if (/^[a-zA-Z]:[\\/]/.test(localPath) || localPath.startsWith('/')) {
    // 本地音频交给 asset 协议直接读盘：整文件 base64 过 IPC 会为每集播客
    // 凭空造出一份等长字符串、在主线程逐字符解码，还要常驻一个 Blob。
    const assetUrl = convertFileSrc(localPath)
    console.info('[LangLearning] media:local-asset-url', {
      localPath,
      sourceKind: inferMediaKindByPath(localPath),
      protocol: String(assetUrl || '').split(':')[0],
    })
    return assetUrl
  }
  return cleaned
}

const refreshActiveAudioPlayableSrc = async () => {
  const sourceUri = activeAudio.value?.source_uri || ''
  // 音频文件已丢失时不要再交给 asset 协议去读盘，否则 webview 只会报 500。
  if (activeAudio.value?.audio_available === false) {
    activeAudioPlayableSrc.value = ''
    return
  }
  activeAudioPlayableSrc.value = await resolvePlayableMediaSrc(sourceUri)
  console.info('[LangLearning] activeAudio:playable-src-refreshed', {
    audioId: activeAudio.value?.id || '',
    sourceProtocol: sanitizeMediaUri(sourceUri).split(':')[0],
    playableProtocol: String(activeAudioPlayableSrc.value || '').split(':')[0],
    hasPlayableSrc: Boolean(activeAudioPlayableSrc.value),
  })
}

const refreshReviewAudioPlayableSrc = async () => {
  const sourceUri = currentReviewItem.value?.payload?.source_uri || ''
  if (currentReviewItem.value?.payload?.audio_available === false) {
    reviewAudioPlayableSrc.value = ''
    return
  }
  reviewAudioPlayableSrc.value = await resolvePlayableMediaSrc(sourceUri)
  console.info('[LangLearning] reviewAudio:playable-src-refreshed', {
    audioId: currentReviewItem.value?.payload?.id || '',
    sourceProtocol: sanitizeMediaUri(sourceUri).split(':')[0],
    playableProtocol: String(reviewAudioPlayableSrc.value || '').split(':')[0],
    hasPlayableSrc: Boolean(reviewAudioPlayableSrc.value),
  })
}

const parseClockTimestampToSec = (raw) => {
  const src = String(raw || '').trim().replace(',', '.')
  if (!src) return null
  const parts = src.split(':').map((x) => Number(String(x || '').trim()))
  if (parts.some((x) => !Number.isFinite(x))) return null
  if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2]
  if (parts.length === 2) return parts[0] * 60 + parts[1]
  if (parts.length === 1) return parts[0]
  return null
}

const resolveSegmentTranslatedText = (segment) => {
  const raw = segment?.translated_text
    ?? segment?.translation
    ?? segment?.translatedText
    ?? segment?.translated
    ?? segment?.target_text
    ?? segment?.targetText
    ?? segment?.target
    ?? ''
  return String(raw || '').trim()
}

const normalizeSyncTimeline = (payload) => {
  const rows = []
  const customSegments = Array.isArray(payload?.segments) ? payload.segments : []
  customSegments.forEach((x, idx) => {
    const start = Number(x?.start_sec ?? x?.start ?? 0)
    const end = Number(x?.end_sec ?? x?.end ?? (start + 1.2))
    const text = String(x?.text || '').trim()
    const translated = resolveSegmentTranslatedText(x)
    if (!text) return
    if (!Number.isFinite(start) || !Number.isFinite(end)) return
    rows.push({ text, translated_text: translated, start: Math.max(0, start), end: Math.max(start + 0.3, end), index: idx })
  })
  if (!rows.length) {
    const whisperSegments = Array.isArray(payload?.transcription) ? payload.transcription : []
    whisperSegments.forEach((x, idx) => {
      const text = String(x?.text || '').trim()
      const translated = resolveSegmentTranslatedText(x)
      if (!text) return
      const tsStart = parseClockTimestampToSec(x?.timestamps?.from)
      const tsEnd = parseClockTimestampToSec(x?.timestamps?.to)
      const offStart = Number(x?.offsets?.from)
      const offEnd = Number(x?.offsets?.to)
      const start = Number.isFinite(tsStart) ? Number(tsStart) : (Number.isFinite(offStart) ? offStart / 1000 : 0)
      const end = Number.isFinite(tsEnd) ? Number(tsEnd) : (Number.isFinite(offEnd) ? offEnd / 1000 : (start + 1.2))
      if (!Number.isFinite(start) || !Number.isFinite(end)) return
      rows.push({ text, translated_text: translated, start: Math.max(0, start), end: Math.max(start + 0.3, end), index: idx })
    })
  }
  rows.sort((a, b) => {
    const startDiff = Number(a.start || 0) - Number(b.start || 0)
    if (startDiff !== 0) return startDiff
    const indexDiff = Number(a.index || 0) - Number(b.index || 0)
    if (indexDiff !== 0) return indexDiff
    return Number(a.end || 0) - Number(b.end || 0)
  })
  return rows
}

const loadSyncTimelineByJsonPath = async (jsonPath) => {
  const rawPath = sanitizeMediaUri(jsonPath)
  const isFileUri = /^file:\/\//i.test(rawPath)
  let decodedPath = rawPath
  if (isFileUri) {
    try {
      decodedPath = decodeURIComponent(rawPath.replace(/^file:\/\//i, ''))
    } catch {
      decodedPath = rawPath.replace(/^file:\/\//i, '')
    }
  } else if (/%[0-9a-f]{2}/i.test(rawPath)) {
    try {
      decodedPath = decodeURIComponent(rawPath)
    } catch {}
  }
  const path = normalizeLocalFilePath(
    decodedPath
  )
  if (!path || /^https?:\/\//i.test(path)) return []
  const candidates = [...new Set([
    path,
    path.replace(/\\/g, '/'),
    path.replace(/\//g, '\\'),
    /^[a-zA-Z]:[\\/]/.test(path) ? `/${path.replace(/\\/g, '/')}` : '',
  ].filter(Boolean))]
  const parseTimelineText = (text, loadedPath, mode) => {
    const payload = JSON.parse(String(text || '').trim())
    console.info('[LangLearning] syncJson:loaded', {
      rawPath,
      path: loadedPath,
      mode,
      segmentCount: Array.isArray(payload?.segments) ? payload.segments.length : 0,
      transcriptionCount: Array.isArray(payload?.transcription) ? payload.transcription.length : 0,
    })
    return normalizeSyncTimeline(payload)
  }
  try {
    for (const candidate of candidates) {
      try {
        const text = String(await readTextFileByPath(candidate) || '').trim()
        if (!text) continue
        return parseTimelineText(text, candidate, 'fs')
      } catch {}
      try {
        const payload = await invoke('language_learning_read_local_media_base64', { path: candidate })
        const base64Text = String(payload?.base64 || '').trim()
        if (!base64Text) continue
        const bytes = bytesFromBase64(base64Text)
        if (!bytes.length) continue
        const text = new TextDecoder('utf-8').decode(bytes).trim()
        if (!text) continue
        return parseTimelineText(text, candidate, 'backend-base64')
      } catch {}
    }
    console.warn('[LangLearning] syncJson:load-failed', {
      rawPath,
      path,
      candidates,
      error: 'all candidates failed',
    })
    return []
  } catch (err) {
    console.warn('[LangLearning] syncJson:load-failed', {
      rawPath,
      path,
      candidates,
      error: String(err?.message || err || ''),
    })
    return []
  }
}

const listeningColumns = [
  {
    type: 'selection',
    multiple: true,
    width: 44,
    fixed: 'left',
  },
  {
    title: '主题',
    key: 'topic',
    width: 140,
    render: (row) => {
      const topic = normalizeTopicForDisplay(row.payload?.topic)
      return h('div', { class: 'topic-cell', title: topic }, topic)
    },
  },
  { title: '标题', key: 'title', minWidth: 240, ellipsis: { tooltip: true } },
  {
    title: '简要描述',
    key: 'summary',
    minWidth: 260,
    ellipsis: { tooltip: true },
    render: (row) => String(row.summary || '').replace(/\s+/g, ' ').slice(0, 96) || '—',
  },
  {
    title: '分类',
    key: 'source_kind',
    width: 90,
    render: (row) => h(NTag, { size: 'small', type: 'warning' }, { default: () => String(row.payload?.source_kind || 'listening') }),
  },
  {
    title: '译文可用率',
    key: 'translationCoverage',
    width: 118,
    render: (row) => {
      const coverage = row.translationCoverage || { translated: 0, total: 0, ratio: 0 }
      return h(
        NTag,
        { size: 'small', type: resolveCoverageType(coverage) },
        { default: () => formatCoverageLabel(coverage) }
      )
    },
  },
  {
    title: '掌握度',
    key: 'masteryLevel',
    width: 100,
    render: (row) => h('span', { class: 'mastery-stars' }, masteryStars(row.masteryLevel)),
  },
  {
    title: '时长',
    key: 'duration',
    width: 88,
    render: (row) => fmtDuration(row.payload?.duration_sec),
  },
  { title: '创建时间', key: 'createdAt', width: 184, ellipsis: { tooltip: true } },
]

const openListeningImmersive = (row) => {
  if (!row || row.sourceType !== 'audio') return
  const hit = allRows.value.find((x) => x.id === row.id && x.sourceType === 'audio')
  console.info('[LangLearning] openListeningImmersive:click', {
    rowId: row?.id || '',
    rowType: row?.rowType || '',
    rowSourceType: row?.sourceType || '',
    hasPayload: Boolean(row?.payload),
    hasSourceUri: Boolean(row?.payload?.source_uri),
    hit: Boolean(hit),
    listeningRows: listeningRows.value.length,
  })
  if (!hit && !sanitizeMediaUri(row?.payload?.source_uri || '')) {
    message.warning('该听力条目暂不可用，请刷新后重试')
    console.warn('[LangLearning] openListeningImmersive:invalid-row', {
      rowId: row?.id || '',
      hasSourceUri: Boolean(row?.payload?.source_uri),
    })
    return
  }
  activeTab.value = 'manager'
  selectedShelf.value = 'my_listening'
  managerMode.value = 'list'
  activeRow.value = hit || row
  rightCollapsed.value = false
  listeningImmersiveMode.value = true
  console.info('[LangLearning] openListeningImmersive:state-updated', {
    activeTab: activeTab.value,
    selectedShelf: selectedShelf.value,
    managerMode: managerMode.value,
    activeRow: activeRow.value?.id || '',
    listeningImmersiveMode: listeningImmersiveMode.value,
  })
}

const selectListeningRow = (row) => {
  if (!row || row.sourceType !== 'audio') return
  const hit = allRows.value.find((x) => x.id === row.id && x.sourceType === 'audio')
  activeRow.value = hit || row
  selectedShelf.value = 'my_listening'
  managerMode.value = 'list'
  rightCollapsed.value = false
}

const enterListeningStudy = () => {
  if (!activeRow.value || activeRow.value.sourceType !== 'audio') {
    message.warning('请先选择一个听力条目')
    return
  }
  // 先进入再转写：播放入口不等后台任务，watch(activeAudio) 会在进入后触发同步。
  openListeningImmersive(activeRow.value)
  refreshSyncJobs()
}

const exitListeningImmersive = () => {
  listeningImmersiveMode.value = false
}

const loadAllRecordsForReview = async () => {
  if (reviewRecordsSnapshot) return
  reviewRecordsSnapshot = records.value
  try {
    // 复习是全局范围，记录按全部类型加载，避免停在某个书架时只载入局部数据。
    const kind = 'all'
    const all = []
    let offset = 0
    while (offset < 10000) {
      const page = await invoke('language_learning_list_records', {
        keyword: keyword.value,
        kind,
        learningLang: learningLang.value,
        limit: 1000,
        offset,
      })
      const rows = Array.isArray(page) ? page : []
      all.push(...rows)
      if (rows.length < 1000) break
      offset += 1000
    }
    records.value = all
    rebuildReviewRows()
  } catch (err) {
    console.warn('[LangLearning] review-load-all-failed', err)
  }
}

const restoreManagerRecords = async () => {
  if (!reviewRecordsSnapshot) return
  records.value = reviewRecordsSnapshot
  reviewRecordsSnapshot = null
  await refreshRecords()
  if (!filteredRows.value.some((row) => row.id === activeRow.value?.id)) {
    activeRow.value = filteredRows.value[0] || null
  }
}

const goToReview = async () => {
  await loadAllRecordsForReview()
  activeTab.value = 'review'
  rebuildReviewRows()
}

const startDueReview = async () => {
  reviewFilter.dueOnly = true
  reviewFilter.includeNew = true
  await goToReview()
}

const hasRunningSyncJobs = computed(() => {
  return Object.values(syncJobs.value).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))
})

const scheduleSyncCompletionRefresh = () => {
  if (syncCompleteRefreshTimer) window.clearTimeout(syncCompleteRefreshTimer)
  syncCompleteRefreshTimer = window.setTimeout(async () => {
    syncCompleteRefreshTimer = 0
    await refreshAudio()
    const audioId = String(activeAudio.value?.id || '').trim()
    if (!audioId) return
    const row = allRows.value.find((item) => item.id === `audio:${audioId}`)
    const timeline = await loadSyncTimelineByJsonPath(row?.payload?.sync_json_uri || '')
    if (Array.isArray(timeline) && timeline.length) {
      activeAudioSyncTimeline.value = timeline
      audioTranslationCoverageMap.value = {
        ...audioTranslationCoverageMap.value,
        [audioId]: resolveTranslationCoverageFromTimeline(timeline),
      }
    }
  }, 450)
}

let lastSyncStageById = new Map()
const refreshSyncJobs = async () => {
  try {
    const list = await invoke('language_learning_get_sync_jobs')
    const rows = Array.isArray(list) ? list : []
    const map = {}
    let completed = false
    rows.forEach((job) => {
      const id = String(job?.audio_id || '').trim()
      if (!id) return
      map[id] = job
      const prevStage = lastSyncStageById.get(id)
      if (prevStage !== job.stage && (job.stage === 'done' || job.stage === 'error')) {
        completed = true
      }
      lastSyncStageById.set(id, job.stage)
    })
    syncJobs.value = map
    if (completed) scheduleSyncCompletionRefresh()
  } catch {}
}

const ensureSyncPolling = () => {
  if (syncPollTimer) return
  syncPollTimer = window.setInterval(async () => {
    await refreshSyncJobs()
    if (!hasRunningSyncJobs.value) {
      window.clearInterval(syncPollTimer)
      syncPollTimer = 0
    }
  }, 1100)
}

const retranslateActiveAudio = async () => {
  const id = String(activeAudio.value?.id || '').trim()
  if (!id) return
  syncJobs.value = {
    ...syncJobs.value,
    [id]: { audio_id: id, stage: 'translate', progress: 8, message: '按当前母语重译', error: '' },
  }
  ensureSyncPolling()
  try {
    await invoke('language_learning_retranslate_audio', {
      audioId: id,
      learningLang: learningLang.value,
      nativeLang: nativeLang.value,
    })
    message.success('已按当前母语重新翻译')
    await refreshAudio()
    const row = allRows.value.find((item) => item.id === `audio:${id}`)
    const timeline = await loadSyncTimelineByJsonPath(row?.payload?.sync_json_uri || '')
    if (timeline.length) activeAudioSyncTimeline.value = timeline
    await refreshSyncJobs()
  } catch (err) {
    message.error(`重译失败: ${String(err?.message || err || '')}`)
    await refreshSyncJobs()
  }
}

const batchRetrySync = async () => {
  const ids = allRows.value
    .filter((row) => checkedRowKeys.value.includes(row.id) && row.sourceType === 'audio')
    .map((row) => String(row.rawId || ''))
    .filter(Boolean)
  if (!ids.length) {
    message.warning('请先选择听力条目')
    return
  }
  for (const id of ids) {
    try {
      await invoke('language_learning_generate_whisper_sync_json_async', {
        audioId: id,
        learningLang: learningLang.value,
        nativeLang: nativeLang.value,
        force: true,
      })
    } catch (err) {
      message.error(`提交重新转写失败: ${String(err?.message || err || '')}`)
    }
  }
  ensureSyncPolling()
  message.info(`已提交 ${ids.length} 条重新转写`)
}

const moreMenuOptions = [
  { label: '离线词典', key: 'dictionary' },
  { label: '语音模型', key: 'asr' },
  { label: '从网页导入', key: 'webimport' },
  { label: '导入学习数据', key: 'import' },
  { label: '导出学习数据', key: 'export' },
  { label: '听力源管理器', key: 'sources' },
  { label: '随机抽查', key: 'random' },
]

const onMoreMenuSelect = (key) => {
  if (key === 'dictionary') openDictionaryManager()
  else if (key === 'asr') openAsrManager()
  else if (key === 'webimport') openWebImport()
  else if (key === 'import') importLearningData()
  else if (key === 'export') exportLearningData()
  else if (key === 'sources') sourceManagerVisible.value = true
  else if (key === 'random') openRandomCheckBoard()
}

const fmtBytes = (raw) => {
  const bytes = Number(raw || 0)
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const dictJobFor = (id) => dictJobs.value[String(id || '')] || null

const dictRunning = computed(() => {
  return Object.values(dictJobs.value).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))
})

const dictTotalEntries = computed(() => {
  return dictPacks.value.reduce((sum, pack) => sum + Number(pack?.entry_count || 0), 0)
})

const refreshDictData = async () => {
  try {
    const [packs, catalog] = await Promise.all([
      invoke('language_learning_dict_list'),
      invoke('language_learning_dict_catalog'),
    ])
    dictPacks.value = Array.isArray(packs) ? packs : []
    dictCatalog.value = Array.isArray(catalog) ? catalog : []
    await refreshActiveWordOffline()
  } catch (err) {
    console.warn('[LangLearning] dict-list-failed', err)
  }
}

const refreshDictJobs = async () => {
  try {
    const jobs = await invoke('language_learning_dict_jobs')
    const map = {}
    ;(Array.isArray(jobs) ? jobs : []).forEach((job) => {
      if (job?.id) map[job.id] = job
    })
    dictJobs.value = map
    if (!Object.values(map).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))) {
      if (dictPollTimer) {
        window.clearInterval(dictPollTimer)
        dictPollTimer = 0
      }
      await refreshDictData()
    }
  } catch {}
}

const ensureDictPolling = () => {
  if (dictPollTimer) return
  dictPollTimer = window.setInterval(refreshDictJobs, 1000)
}

const openDictionaryManager = async () => {
  dictionaryManagerVisible.value = true
  if (!String(dictImportName.value || '').trim()) {
    dictImportSource.value = learningLang.value
    dictImportTarget.value = nativeLang.value
  }
  await refreshDictData()
  await refreshDictJobs()
  if (dictRunning.value) ensureDictPolling()
}

const installDict = async (id) => {
  try {
    await invoke('language_learning_dict_install', { id: String(id) })
    message.info('已开始后台下载并建立索引')
    ensureDictPolling()
  } catch (err) {
    message.error(`启动词典下载失败: ${String(err?.message || err || '')}`)
  }
}

const removeDict = (pack) => {
  if (!pack) return
  askConfirm('删除离线词典', `将删除「${pack.name}」及其本地索引，之后查词会回退到在线词典。`, async () => {
    try {
      await invoke('language_learning_dict_remove', { id: pack.id })
      message.success('离线词典已删除')
      await refreshDictData()
      activeWordOfflinePack.value = ''
    } catch (err) {
      message.error(`删除词典失败: ${String(err?.message || err || '')}`)
    }
  })
}

const exportDictById = async (id) => {
  const pack = dictPacks.value.find((item) => item.id === id)
  if (!pack) return
  try {
    const path = await save({
      defaultPath: `${pack.id}-dict.zip`,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!path) return
    await invoke('language_learning_dict_export', { id: pack.id, outputPath: String(path) })
    message.success(`已导出 ${pack.name}`)
  } catch (err) {
    message.error(`导出词典失败: ${String(err?.message || err || '')}`)
  }
}

const importDictPackage = async () => {
  if (dictImporting.value) return
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: '词典包', extensions: ['zip', 'csv', 'json'] }],
    })
    if (!path || Array.isArray(path)) return
    dictImporting.value = true
    const imported = await invoke('language_learning_dict_import', {
      path: String(path),
      name: String(dictImportName.value || '').trim(),
      sourceLang: dictImportSource.value,
      targetLang: dictImportTarget.value,
    })
    message.success(`已导入 ${imported?.name || '词典'}（${imported?.entry_count || 0} 条）`)
    dictImportName.value = ''
    await refreshDictData()
    await lookupDictWord()
  } catch (err) {
    message.error(`导入词典失败: ${String(err?.message || err || '')}`)
  } finally {
    dictImporting.value = false
  }
}

const lookupDictWord = async () => {
  const word = String(dictLookupWord.value || '').trim()
  if (!word) {
    dictLookupRows.value = []
    dictLookupSearched.value = false
    return
  }
  dictLookupLoading.value = true
  try {
    const isCjk = /[\u4e00-\u9fff]/.test(word)
    const target = isCjk ? 'en' : nativeLang.value
    const rows = await invoke('language_learning_dict_lookup', {
      word,
      sourceLang: 'auto',
      targetLang: target,
      limit: 6,
    })
    dictLookupRows.value = Array.isArray(rows) ? rows : []
    dictLookupSearched.value = true
  } catch (err) {
    message.error(`离线查词失败: ${String(err?.message || err || '')}`)
  } finally {
    dictLookupLoading.value = false
  }
}

const refreshActiveWordOffline = async () => {
  const word = String(activeWordRecord.value?.word || activeWordRecord.value?.source_text || '').trim()
  if (!word) {
    activeWordOfflinePack.value = ''
    return
  }
  try {
    const rows = await invoke('language_learning_dict_lookup', {
      word,
      sourceLang: 'auto',
      targetLang: nativeLang.value,
      limit: 1,
    })
    activeWordOfflinePack.value = String(rows?.[0]?.pack_name || '')
  } catch {
    activeWordOfflinePack.value = ''
  }
}

const asrJobFor = (id) => asrJobs.value[String(id || '')] || null

const asrRunning = computed(() => {
  return Object.values(asrJobs.value).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))
})

const webImportHasText = computed(() => String(webImportText.value || '').trim().length > 0)

const asrModelById = (id) => asrModels.value.find((model) => model.id === id) || null

const asrSupports = (model, lang) => {
  const name = `${model?.name || ''} ${model?.id || ''}`.toLowerCase()
  if (name.includes('sense')) return ['zh', 'en', 'ja', 'ko', 'yue'].includes(lang)
  if (name.includes('.en')) return lang === 'en'
  return true
}

const asrPreference = (model, lang) => {
  const name = `${model?.name || ''} ${model?.id || ''}`.toLowerCase()
  let score = 0
  if (lang === 'en' && name.includes('.en')) score += 40
  if (['zh', 'ja', 'ko'].includes(lang) && name.includes('sense')) score += 30
  if (name.includes('base')) score += 10
  if (name.includes('tiny')) score += 5
  if (model?.builtin) score += 8
  return score
}

const currentAsrModel = computed(() => {
  const lang = String(learningLang.value || 'en')
  return [...asrModels.value]
    .filter((model) => asrSupports(model, lang))
    .sort((a, b) => asrPreference(b, lang) - asrPreference(a, lang))[0] || null
})

const currentAsrLabel = computed(() => currentAsrModel.value?.name || '未安装多语种模型')

const refreshAsrData = async () => {
  try {
    const [models, catalog] = await Promise.all([
      invoke('language_learning_asr_models'),
      invoke('language_learning_asr_catalog'),
    ])
    asrModels.value = Array.isArray(models) ? models : []
    asrCatalog.value = Array.isArray(catalog) ? catalog : []
  } catch (err) {
    console.warn('[LangLearning] asr-list-failed', err)
  }
}

const refreshAsrJobs = async () => {
  try {
    const jobs = await invoke('language_learning_asr_jobs')
    const map = {}
    ;(Array.isArray(jobs) ? jobs : []).forEach((job) => {
      if (job?.id) map[job.id] = job
    })
    asrJobs.value = map
    if (!Object.values(map).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))) {
      if (asrPollTimer) {
        window.clearInterval(asrPollTimer)
        asrPollTimer = 0
      }
      await refreshAsrData()
    }
  } catch {}
}

const ensureAsrPolling = () => {
  if (asrPollTimer) return
  asrPollTimer = window.setInterval(refreshAsrJobs, 1000)
}

const openAsrManager = async () => {
  asrManagerVisible.value = true
  await refreshAsrData()
  await refreshAsrJobs()
  if (Object.values(asrJobs.value).some((job) => job && job.stage && !['done', 'error'].includes(job.stage))) {
    ensureAsrPolling()
  }
}

const installAsr = async (id) => {
  try {
    await invoke('language_learning_asr_install', { id: String(id) })
    message.info('已开始后台下载语音模型')
    ensureAsrPolling()
  } catch (err) {
    message.error(`启动模型下载失败: ${String(err?.message || err || '')}`)
  }
}

const removeAsr = (model) => {
  if (!model || model.builtin) return
  askConfirm('删除语音模型', `将删除「${model.name}」，之后该语言会回退到其他可用模型。`, async () => {
    try {
      await invoke('language_learning_asr_remove', { id: model.id })
      message.success('语音模型已删除')
      await refreshAsrData()
    } catch (err) {
      message.error(`删除模型失败: ${String(err?.message || err || '')}`)
    }
  })
}

const webImportAudioOptions = computed(() => {
  return (webImportPreview.value?.audio_urls || []).map((url, idx) => ({
    label: idx === 0 ? `音频 ${idx + 1}（默认）` : `音频 ${idx + 1}`,
    value: url,
  }))
})

const openWebImport = () => {
  webImportVisible.value = true
  webImportUrl.value = ''
  webImportPreview.value = null
  webImportTitle.value = ''
  webImportText.value = ''
  webImportAudioUrl.value = ''
  webImportSubtitleUrl.value = ''
  webImportOptions.listening = false
  webImportOptions.reading = false
  webImportOptions.translate = true
}

const previewWebImport = async () => {
  const url = String(webImportUrl.value || '').trim()
  if (!url) {
    message.warning('请先输入网页地址')
    return
  }
  webImportLoading.value = true
  try {
    const preview = await invoke('language_learning_preview_web_import', { url })
    webImportPreview.value = preview
    webImportTitle.value = preview?.title || ''
    webImportText.value = preview?.text || preview?.subtitle_text || ''
    webImportAudioUrl.value = preview?.audio_urls?.[0] || ''
    webImportSubtitleUrl.value = preview?.subtitle_urls?.[0] || ''
    webImportOptions.listening = Boolean(webImportAudioUrl.value)
    webImportOptions.reading = Boolean(String(webImportText.value).trim())
    if (!webImportAudioUrl.value && !String(webImportText.value).trim()) {
      message.warning('没有识别到音频或正文；JS 渲染页面可以手动粘贴正文后再导入')
    }
  } catch (err) {
    message.error(`网页提取失败: ${String(err?.message || err || '')}`)
  } finally {
    webImportLoading.value = false
  }
}

const importWebContent = async () => {
  if (webImporting.value) return
  if (!webImportOptions.listening && !webImportOptions.reading) {
    message.warning('请至少选择导入听力或阅读')
    return
  }
  if (webImportOptions.listening && !webImportAudioUrl.value) {
    message.warning('没有可导入的音频地址')
    return
  }
  if (webImportOptions.reading && !String(webImportText.value || '').trim()) {
    message.warning('正文为空，请先粘贴正文')
    return
  }
  webImporting.value = true
  try {
    const result = await invoke('language_learning_import_web_content', {
      input: {
        url: String(webImportUrl.value || '').trim(),
        title: String(webImportTitle.value || '').trim(),
        text: String(webImportText.value || '').trim(),
        subtitle_text: String(webImportPreview.value?.subtitle_text || '').trim(),
        audio_url: String(webImportAudioUrl.value || '').trim(),
        learning_lang: learningLang.value,
        native_lang: nativeLang.value,
        import_listening: Boolean(webImportOptions.listening),
        import_reading: Boolean(webImportOptions.reading),
        translate: Boolean(webImportOptions.translate),
      },
    })
    const audio = result?.audio
    const record = result?.record
    const parts = []
    if (audio) parts.push('听力')
    if (record) parts.push('阅读段落')
    message.success(`已从网页导入${parts.length ? `：${parts.join(' + ')}` : ''}`)
    if (record && String(record.translated_text || '').includes('待翻译')) {
      message.warning('阅读段落已导入，但译文未生成，可在条目里继续编辑')
    }
    webImportVisible.value = false
    if (audio?.id) triggerSyncGenerationForAudioId(audio.id, false)
    await refreshAll()
    if (audio?.id) {
      const hit = allRows.value.find((row) => row.id === `audio:${audio.id}`)
      if (hit) activeRow.value = hit
      selectedShelf.value = 'my_listening'
    }
  } catch (err) {
    message.error(`网页导入失败: ${String(err?.message || err || '')}`)
  } finally {
    webImporting.value = false
  }
}

const ledgerMenuOptions = computed(() => {
  if (selectedShelf.value === 'my_listening') {
    return [
      { label: '编辑当前听力', key: 'editAudio', disabled: !activeAudio.value },
      { label: '重新同步选中项', key: 'retry', disabled: !checkedRowKeys.value.length },
      { label: '删除选中项', key: 'delete', disabled: !checkedRowKeys.value.length && !activeRow.value },
      { label: '听力源管理器', key: 'sources' },
    ]
  }
  return [
    { label: '编辑当前词条', key: 'editRecord', disabled: !activeRow.value || activeRow.value.sourceType !== 'record' },
    { label: '开始复习', key: 'review' },
    { label: '删除选中项', key: 'delete', disabled: !checkedRowKeys.value.length && !activeRow.value },
    { label: '导出当前分类', key: 'export' },
  ]
})

const onLedgerMenuSelect = (key) => {
  if (key === 'editAudio') editAudio(activeAudio.value)
  else if (key === 'retry') batchRetrySync()
  else if (key === 'delete') deleteSelectedRows()
  else if (key === 'sources') sourceManagerVisible.value = true
  else if (key === 'editRecord') openEditDialog(activeRow.value)
  else if (key === 'review') startReviewFromSelection()
  else if (key === 'export') exportLearningData()
}

const listeningRowProps = (row) => ({
  onClick: () => {
    selectListeningRow(row)
  },
})

const onListeningCheckedRowKeysChange = (keys) => {
  checkedRowKeys.value = Array.isArray(keys) ? keys : []
  console.info('[LangLearning] listening-table:checked-keys', {
    count: checkedRowKeys.value.length,
  })
}

const managerColumns = [
  {
    type: 'selection',
    multiple: true,
    width: 44,
  },
  {
    title: '类型',
    key: 'rowType',
    width: 86,
    render: (row) => h(NTag, { size: 'small', type: row.rowType === 'word' ? 'success' : row.rowType === 'listening' ? 'warning' : 'info' }, { default: () => rowTypeLabel(row.rowType) }),
  },
  { title: '词目/标题', key: 'title', width: 180, ellipsis: { tooltip: true } },
  { title: '音标', key: 'phonetic', width: 120, ellipsis: { tooltip: true } },
  {
    title: '等级',
    key: 'masteryLevel',
    width: 96,
    render: (row) => h('span', { class: 'mastery-stars' }, masteryStars(row.masteryLevel)),
  },
  { title: '简明解释', key: 'summary', ellipsis: { tooltip: true } },
  { title: '我的笔记', key: 'note', width: 220, ellipsis: { tooltip: true } },
  { title: '添加时间', key: 'createdAt', width: 188, ellipsis: { tooltip: true } },
]

const managerRowProps = (row) => ({
  onClick: () => {
    activeRow.value = row
  },
})

const onManagerCheckedRowKeysChange = (keys) => {
  checkedRowKeys.value = Array.isArray(keys) ? keys : []
}

const activeWordRecord = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'record' || r.rowType !== 'word') return null
  return r.payload
})

const activeSentenceRecord = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'record' || r.rowType === 'word') return null
  return r.payload
})

const activeAudio = computed(() => {
  const r = activeRow.value
  if (!r || r.sourceType !== 'audio') return null
  const id = String(r.rawId || r.payload?.id || '').trim()
  const coverage = r.translationCoverage || audioTranslationCoverageMap.value[id] || { translated: 0, total: 0, ratio: 0 }
  return {
    ...(r.payload || {}),
    translationCoverage: coverage,
  }
})
const activeMediaIsVideo = computed(() => {
  const sourcePath = String(activeAudio.value?.source_uri || '').trim()
  const playableSrc = String(activeAudioPlayableSrc.value || '').trim()
  return inferMediaKindByPath(sourcePath) === 'video' || inferMediaKindByPath(playableSrc) === 'video'
})

const normalizeList = (arr) => {
  const list = Array.isArray(arr) ? arr : []
  const out = []
  list.forEach((x) => {
    const t = String(x || '').trim()
    if (!t) return
    if (!out.some((y) => y.toLowerCase() === t.toLowerCase())) out.push(t)
  })
  return out
}

const activeWordCombinedTranslation = computed(() => {
  const row = activeWordRecord.value
  if (!row) return ''
  const meanings = Array.isArray(row.meanings) && row.meanings.length ? row.meanings : splitSimpleTranslations(row.translated_text)
  return normalizeList(meanings).slice(0, 5).join('；')
})

const activeWordPhoneticUk = computed(() => String(activeWordRecord.value?.phonetic_uk || activeWordRecord.value?.phonetic || '').trim())
const activeWordPhoneticUs = computed(() => String(activeWordRecord.value?.phonetic_us || activeWordRecord.value?.phonetic || '').trim())

const activeWordSynonyms = computed(() => normalizeList(activeWordRecord.value?.synonyms))
const activeWordAntonyms = computed(() => normalizeList(activeWordRecord.value?.antonyms))
const activeWordAssociates = computed(() => normalizeList(activeWordRecord.value?.associates))
const activeWordSynonymPairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.synonyms_translations) ? row.synonyms_translations : []
  return activeWordSynonyms.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordAntonymPairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.antonyms_translations) ? row.antonyms_translations : []
  return activeWordAntonyms.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordAssociatePairs = computed(() => {
  const row = activeWordRecord.value
  const trans = Array.isArray(row?.associates_translations) ? row.associates_translations : []
  return activeWordAssociates.value.map((word, idx) => ({ word, translation: String(trans[idx] || '').trim() }))
})
const activeWordPhrasePairs = computed(() => {
  const row = activeWordRecord.value
  if (!row) return []
  const sources = normalizeList(row.phrases)
  const targets = Array.isArray(row.phrase_translations) ? row.phrase_translations.map((x) => String(x || '').trim()) : []
  return sources.slice(0, 12).map((source, idx) => ({ source, target: String(targets[idx] || '').trim() }))
})

const activeWordExamplePairs = computed(() => {
  const row = activeWordRecord.value
  if (!row) return []
  const examples = normalizeList(row.examples)
  const targets = Array.isArray(row.example_translations)
    ? row.example_translations.map((x) => String(x || '').trim())
    : []
  const pairs = examples.map((source, idx) => ({
    source,
    target: String(targets[idx] || '').trim(),
  }))
  if (pairs.length) return pairs
  const fallbackSource = String(activeWordPhrasePairs.value[0]?.source || '').trim()
    || `This is a ${String(row.word || row.source_text || '').trim()}.`
  const fallbackTarget = String(activeWordPhrasePairs.value[0]?.target || '').trim() || activeWordCombinedTranslation.value
  return [{ source: fallbackSource, target: String(fallbackTarget || '').trim() }]
})

const getMasteryLevel = (sourceType, rawId) => getMasteryEntry(sourceType, rawId).level

const applyReviewResult = (result) => {
  if (!activeRow.value) return
  upsertMasteryEntry(activeRow.value.sourceType, activeRow.value.rawId, result)
  if (activeRow.value) {
    activeRow.value = allRows.value.find((r) => r.id === activeRow.value.id) || activeRow.value
  }
}

// 本地音频已丢失的听力条目放不出声，不该进入复习/抽查卡组。
const isReviewableRow = (row) => row?.rowType !== 'listening' || row.payload?.audio_available !== false

const reviewRowsBase = computed(() => {
  // 复习范围跟左侧“总学习项”一致，不受当前书架筛选影响；
  // 否则停在空书架（如例句 0 条）时点“开始今日复习”会直接无内容可复习。
  return allRows.value.filter(matchesKeyword).filter(isReviewableRow).filter((row) => {
    const now = Date.now()
    const createdTs = row.createdAt ? new Date(row.createdAt).getTime() : 0
    const lastReviewedTs = row.lastReviewedAt ? new Date(row.lastReviewedAt).getTime() : 0
    if (reviewFilter.createdPeriod !== 'all' && createdTs > 0) {
      const day = reviewFilter.createdPeriod === '3d' ? 3 : reviewFilter.createdPeriod === '7d' ? 7 : 30
      if (now - createdTs > day * 24 * 3600 * 1000) return false
    }
    if (reviewFilter.lastReviewPeriod === 'never' && lastReviewedTs > 0) return false
    if (reviewFilter.lastReviewPeriod === '7d' && (!(lastReviewedTs > 0) || now - lastReviewedTs > 7 * 24 * 3600 * 1000)) return false
    if (reviewFilter.lastReviewPeriod === '30d' && (!(lastReviewedTs > 0) || now - lastReviewedTs > 30 * 24 * 3600 * 1000)) return false
    if (reviewFilter.masteryBand === 'low' && !(row.masteryLevel <= 1)) return false
    if (reviewFilter.masteryBand === 'mid' && !(row.masteryLevel >= 2 && row.masteryLevel <= 3)) return false
    if (reviewFilter.masteryBand === 'high' && !(row.masteryLevel >= 4)) return false
    if (reviewFilter.dueOnly) {
      const mastery = getMasteryEntry(row.sourceType, row.rawId)
      if (!mastery.dueAt) {
        if (!reviewFilter.includeNew) return false
      } else {
        const due = new Date(mastery.dueAt).getTime()
        if (Number.isFinite(due) && due > now) return false
      }
    }
    return true
  })
})

// 类型栏常驻显示，空类型也保留标签，避免 0 条时无处切换。
const reviewTypeOptions = [
  { label: '混合', value: 'mixed' },
  { label: '单词', value: 'word' },
  { label: '例句', value: 'sentence' },
  { label: '段落', value: 'paragraph' },
  { label: '听力', value: 'listening' },
]

const reviewTypeCounts = computed(() => {
  const counts = { mixed: reviewRowsBase.value.length, word: 0, sentence: 0, paragraph: 0, listening: 0 }
  reviewRowsBase.value.forEach((row) => {
    if (row.rowType in counts) counts[row.rowType] += 1
  })
  return counts
})

const reviewRowsByType = computed(() => {
  if (reviewType.value === 'mixed') return reviewRowsBase.value
  return reviewRowsBase.value.filter((r) => r.rowType === reviewType.value)
})

const reviewEmptyDescription = computed(() => {
  if (!reviewRowsBase.value.length) return '当前筛选下暂无可复习内容，可重置筛选后再试'
  const typeLabel = reviewTypeOptions.find((opt) => opt.value === reviewType.value)?.label || '当前类型'
  return `「${typeLabel}」在筛选范围内没有内容，可切换到其他类型`
})

const currentReviewItem = computed(() => {
  if (!reviewRows.value.length) return null
  const idx = Math.max(0, Math.min(reviewRows.value.length - 1, reviewIndex.value))
  return reviewRows.value[idx] || null
})

const rebuildReviewRows = () => {
  const rows = [...reviewRowsByType.value]
  rows.sort((a, b) => {
    if (reviewFilter.orderBy === 'created_asc') return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    if (reviewFilter.orderBy === 'review_oldest') {
      const aTs = a.lastReviewedAt ? new Date(a.lastReviewedAt).getTime() : 0
      const bTs = b.lastReviewedAt ? new Date(b.lastReviewedAt).getTime() : 0
      return aTs - bTs
    }
    return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
  })
  reviewRows.value = rows
  reviewIndex.value = 0
  reviewShowAnswer.value = false
  resetDictation()
}

const shuffleReviewRows = () => {
  const next = [...reviewRowsByType.value]
  for (let i = next.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1))
    const t = next[i]
    next[i] = next[j]
    next[j] = t
  }
  reviewRows.value = next
  reviewIndex.value = 0
  reviewShowAnswer.value = false
}

const resetDictation = () => {
  reviewDictationInput.value = ''
  reviewDictationResult.value = null
}

const gotoNextReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value + 1) % reviewRows.value.length
  reviewShowAnswer.value = false
  resetDictation()
}

const gotoPrevReview = () => {
  if (!reviewRows.value.length) return
  reviewIndex.value = (reviewIndex.value - 1 + reviewRows.value.length) % reviewRows.value.length
  reviewShowAnswer.value = false
  resetDictation()
}

const gradeAndNext = (result) => {
  const cur = currentReviewItem.value
  if (!cur) return
  upsertMasteryEntry(cur.sourceType, cur.rawId, result)
  resetDictation()
  if (reviewFilter.dueOnly) {
    // 评分后到期时间已经推到未来，直接从队列里移除这一张。
    rebuildReviewRows()
  } else {
    gotoNextReview()
  }
}

const startReviewFromSelection = async () => {
  const alreadyInReview = activeTab.value === 'review'
  await goToReview()
  if (!alreadyInReview) return
  if (reviewRows.value.length) {
    message.info(`已应用筛选：共 ${reviewRows.value.length} 张卡片`)
  } else {
    message.warning('当前筛选下没有可复习的内容')
  }
}

const toggleReviewDictation = () => {
  reviewDictationMode.value = !reviewDictationMode.value
  resetDictation()
  reviewShowAnswer.value = false
}

const normalizeDictationText = (raw) => {
  return String(raw || '')
    .toLowerCase()
    .replace(/[\s\u3000]+/g, ' ')
    .replace(/[.,!?;:'"“”‘’()[\]{}<>《》，。！？；：、]/g, '')
    .trim()
}

const levenshteinDistance = (a, b) => {
  const left = Array.from(a).slice(0, 800)
  const right = Array.from(b).slice(0, 800)
  if (!left.length) return right.length
  if (!right.length) return left.length
  let prev = new Array(right.length + 1).fill(0).map((_, i) => i)
  for (let i = 1; i <= left.length; i += 1) {
    const next = [i]
    for (let j = 1; j <= right.length; j += 1) {
      const cost = left[i - 1] === right[j - 1] ? 0 : 1
      next[j] = Math.min(prev[j] + 1, next[j - 1] + 1, prev[j - 1] + cost)
    }
    prev = next
  }
  return prev[right.length]
}

const dictationExpectedText = computed(() => {
  const row = currentReviewItem.value
  if (!row) return ''
  if (row.rowType === 'word') return row.title
  if (row.rowType === 'listening') {
    const timeline = currentReviewListeningTranscriptTimeline.value
    const text = timeline.map((line) => String(line?.text || '').trim()).filter(Boolean).join(' ')
    return text || row.sourceText
  }
  return row.sourceText
})

const checkDictation = () => {
  const input = normalizeDictationText(reviewDictationInput.value)
  if (!input) {
    message.warning('先写下你听到或记住的内容')
    return
  }
  const expected = normalizeDictationText(dictationExpectedText.value)
  const distance = levenshteinDistance(input, expected)
  const maxLen = Math.max(Array.from(input).length, Array.from(expected).length, 1)
  const score = Math.max(0, Math.min(100, Math.round((1 - distance / maxLen) * 100)))
  reviewDictationResult.value = {
    score,
    ok: score >= 90,
    expected,
  }
  reviewShowAnswer.value = true
}

const totalLearningItemCount = computed(() => allRows.value.length)

const masteryAggregate = computed(() => {
  let mastered = 0
  allRows.value.forEach((r) => {
    const m = getMasteryEntry(r.sourceType, r.rawId)
    if (m.level >= 4) mastered += 1
  })
  return { mastered }
})

const currentReviewMasteryLabel = computed(() => {
  const cur = currentReviewItem.value
  if (!cur) return '未开始'
  const m = getMasteryEntry(cur.sourceType, cur.rawId)
  if (m.lastResult === 'known') return '认识'
  if (m.lastResult === 'fuzzy') return '模糊'
  if (m.lastResult === 'unknown') return '不认识'
  return '未评估'
})
const masteryPercent = computed(() => {
  const total = totalLearningItemCount.value
  const mastered = masteryAggregate.value.mastered
  if (total <= 0) return 0
  return Math.min(100, Math.max(0, Math.round((mastered / total) * 100)))
})

const dueReviewCount = computed(() => {
  const now = Date.now()
  return allRows.value.filter((row) => {
    const mastery = getMasteryEntry(row.sourceType, row.rawId)
    if (!mastery.dueAt) return false
    const due = new Date(mastery.dueAt).getTime()
    return Number.isFinite(due) && due <= now
  }).length
})

const newReviewCount = computed(() => {
  return allRows.value.filter((row) => !getMasteryEntry(row.sourceType, row.rawId).dueAt).length
})

const resolveQueryKind = () => {
  if (selectedShelf.value === 'all') return 'all'
  if (selectedShelf.value === 'my_vocab') return 'word'
  if (selectedShelf.value === 'my_sentences') return 'sentence'
  if (selectedShelf.value === 'my_paragraphs') return 'paragraph'
  return 'all'
}

const refreshShelfCounts = async () => {
  try {
    const [wordC, sentenceC, paragraphC, allRecordC, listeningC] = await Promise.all([
      invoke('language_learning_get_counts', { keyword: '', kind: 'word', learningLang: learningLang.value }),
      invoke('language_learning_get_counts', { keyword: '', kind: 'sentence', learningLang: learningLang.value }),
      invoke('language_learning_get_counts', { keyword: '', kind: 'paragraph', learningLang: learningLang.value }),
      invoke('language_learning_get_counts', { keyword: '', kind: 'all', learningLang: learningLang.value }),
      invoke('language_learning_get_counts', { keyword: '', kind: 'all', learningLang: learningLang.value }),
    ])
    shelfCounts.my_vocab = Number(wordC?.records || 0)
    shelfCounts.my_sentences = Number(sentenceC?.records || 0)
    shelfCounts.my_paragraphs = Number(paragraphC?.records || 0)
    shelfCounts.my_listening = Number(listeningC?.audio_resources || 0)
    shelfCounts.all = Number(allRecordC?.records || 0) + shelfCounts.my_listening
  } catch {}
}

const refreshRecords = async () => {
  try {
    const kind = resolveQueryKind()
    const offset = (recordPage.value - 1) * recordPageSize.value
    const data = await invoke('language_learning_list_records', {
      keyword: keyword.value,
      kind,
      learningLang: learningLang.value,
      limit: recordPageSize.value,
      offset,
    })
    records.value = Array.isArray(data) ? data : []
    const counts = await invoke('language_learning_get_counts', {
      keyword: keyword.value,
      kind,
      learningLang: learningLang.value,
    })
    recordCounts.records = Number(counts?.records || 0)
    recordCounts.audio_resources = Number(counts?.audio_resources || 0)
  } catch (err) {
    message.error(`读取学习记录失败: ${String(err?.message || err || '')}`)
  }
}

const refreshAudio = async () => {
  try {
    const data = await invoke('language_learning_list_audio_resources', { keyword: keyword.value, learningLang: learningLang.value })
    audioResources.value = Array.isArray(data) ? data : []
    await refreshAudioTranslationCoverage(audioResources.value)
    if (!syncQueueRunning.value) {
      schedulePendingSyncSweep(220)
    }
  } catch (err) {
    message.error(`读取听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const schedulePendingSyncSweep = (delayMs = 220) => {
  if (pendingSyncSweepTimer) {
    window.clearTimeout(pendingSyncSweepTimer)
    pendingSyncSweepTimer = 0
  }
  pendingSyncSweepTimer = window.setTimeout(async () => {
    if (pendingSyncSweepRunning.value) return
    pendingSyncSweepRunning.value = true
    try {
      await resumePendingSyncTasks()
    } finally {
      pendingSyncSweepRunning.value = false
    }
  }, Math.max(0, Number(delayMs || 0)))
}

const resolveTranslationCoverageFromTimeline = (timeline) => {
  if (!Array.isArray(timeline) || !timeline.length) {
    return { translated: 0, total: 0, ratio: 0 }
  }
  let translated = 0
  const total = timeline.length
  timeline.forEach((line) => {
    const source = String(line?.text || '').trim()
    const target = resolveSegmentTranslatedText(line)
    if (target && (!source || target !== source)) {
      translated += 1
    }
  })
  return {
    translated,
    total,
    ratio: total > 0 ? translated / total : 0,
  }
}

const formatCoverageLabel = (coverage) => {
  const translated = Number(coverage?.translated || 0)
  const total = Number(coverage?.total || 0)
  if (total <= 0) return '0/0'
  return `${translated}/${total}`
}

const resolveCoverageType = (coverage) => {
  const total = Number(coverage?.total || 0)
  const ratio = Number(coverage?.ratio || 0)
  if (total <= 0) return 'default'
  if (ratio >= 0.98) return 'success'
  if (ratio >= 0.6) return 'warning'
  return 'error'
}

const refreshAudioTranslationCoverage = async (rows) => {
  const list = Array.isArray(rows) ? rows : []
  const serial = audioTranslationCoverageRefreshSerial + 1
  audioTranslationCoverageRefreshSerial = serial
  const entries = await Promise.all(list.map(async (row) => {
    const id = String(row?.id || '').trim()
    if (!id) return ['', { translated: 0, total: 0, ratio: 0 }]
    // 本地音频已丢失的条目无法再补同步，每次刷新都去读不存在的 JSON 只会刷日志。
    if (row?.audio_available === false) {
      return [id, { translated: 0, total: 0, ratio: 0 }]
    }
    const syncPath = sanitizeMediaUri(row?.sync_json_uri || '')
    let timeline = []
    if (syncPath) {
      timeline = await loadSyncTimelineByJsonPath(syncPath)
    }
    if (!timeline.length) {
      timeline = buildTranscriptTimeline(String(row?.transcript || ''), Number(row?.duration_sec || 0))
    }
    const stat = resolveTranslationCoverageFromTimeline(timeline)
    // 还没生成过同步 JSON 的条目 coverage 必然是 0——那是「还没同步」，不是异常。
    // 只有「同步 JSON 就在那儿却一条译文都没有」才值得报警；否则整张听力列表每刷新
    // 一次就刷十几行 warn，真正的问题反而被淹掉。
    if (syncPath && stat.total > 0 && stat.ratio <= 0) {
      console.warn('[LangLearning] coverage:zero', {
        id,
        title: String(row?.title || ''),
        syncPath,
        transcriptLength: String(row?.transcript || '').trim().length,
        timelineSize: timeline.length,
        translated: stat.translated,
        total: stat.total,
      })
    }
    return [id, stat]
  }))
  if (serial !== audioTranslationCoverageRefreshSerial) return
  const next = {}
  entries.forEach(([id, stat]) => {
    if (id) next[id] = stat
  })
  audioTranslationCoverageMap.value = next
  const summary = entries.reduce((acc, [, stat]) => {
    const total = Number(stat?.total || 0)
    const translated = Number(stat?.translated || 0)
    acc.totalSegments += total
    acc.translatedSegments += translated
    if (total > 0 && translated > 0) acc.nonZeroRows += 1
    return acc
  }, { totalSegments: 0, translatedSegments: 0, nonZeroRows: 0 })
  console.info('[LangLearning] coverage:refresh', {
    rowCount: entries.length,
    nonZeroRows: summary.nonZeroRows,
    translatedSegments: summary.translatedSegments,
    totalSegments: summary.totalSegments,
  })
}

const runSyncQueueWorker = async () => {
  if (syncQueueRunning.value) return
  syncQueueRunning.value = true
  console.info('[LangLearning] syncQueue:worker-start', {
    queued: syncQueuedTasks.length,
    total: syncQueueTotal.value,
    done: syncQueueDone.value,
  })
  while (syncQueuedTasks.length) {
    const task = syncQueuedTasks.shift()
    const id = String(task?.id || '').trim()
    const taskLearningLang = String(task?.learningLang || learningLang.value || '').trim() || 'zh'
    const taskNativeLang = String(task?.nativeLang || nativeLang.value || 'zh').trim() || 'zh'
    const queueKey = `${taskLearningLang}:${id}`
    if (!id) continue
    currentSyncingAudioId.value = id
    if (activeAudio.value?.id === id) {
      activeAudioSyncGenerating.value = true
    }
    const startedAt = Date.now()
    console.info('[LangLearning] syncQueue:task-start', {
      audioId: id,
      learningLang: taskLearningLang,
      queueLeft: syncQueuedTasks.length,
    })
    try {
      const refreshed = await invoke('language_learning_backfill_sync_translation', {
        audioId: id,
        learningLang: taskLearningLang,
        nativeLang: taskNativeLang,
      })
      if (refreshed?.id) {
        const idx = audioResources.value.findIndex((x) => String(x?.id || '') === String(refreshed.id))
        if (idx >= 0) {
          audioResources.value[idx] = refreshed
        }
      }
      const syncPath = String(refreshed?.sync_json_uri || '').trim()
      if (syncPath) {
        const timeline = await loadSyncTimelineByJsonPath(syncPath)
        if (Array.isArray(timeline) && timeline.length) {
          audioTranslationCoverageMap.value = {
            ...audioTranslationCoverageMap.value,
            [id]: resolveTranslationCoverageFromTimeline(timeline),
          }
          if (activeAudio.value?.id === id) {
            activeAudioSyncTimeline.value = timeline
          }
        }
      }
      console.info('[LangLearning] syncQueue:task-success', {
        audioId: id,
        learningLang: taskLearningLang,
        costMs: Date.now() - startedAt,
      })
    } catch (err) {
      const errorText = String(err?.message || err || '')
      if (errorText.includes('听力条目不存在')) {
        console.info('[LangLearning] syncQueue:skip-missing-audio', { audioId: id, learningLang: taskLearningLang })
      } else {
        console.error('[LangLearning] 同步转写失败', { audioId: id, learningLang: taskLearningLang, error: err })
        const title = String(audioResources.value.find((x) => String(x?.id || '') === id)?.title || id)
        message.error(`听力转写失败: ${title}：${errorText || '请检查语音模型与翻译配置'}`)
      }
    }
    syncQueueDone.value += 1
    syncQueuedIdSet.delete(queueKey)
    syncingAudioIdSet.delete(queueKey)
    if (activeAudio.value?.id === id) {
      activeAudioSyncGenerating.value = false
    }
  }
  syncQueueRunning.value = false
  currentSyncingAudioId.value = ''
  await refreshAudio()
  if (activeAudio.value?.id) {
    const refreshed = allRows.value.find((x) => x.id === `audio:${activeAudio.value.id}`)
    if (refreshed) activeRow.value = refreshed
  }
  console.info('[LangLearning] syncQueue:worker-idle', {
    total: syncQueueTotal.value,
    done: syncQueueDone.value,
  })
}

const triggerSyncGenerationForAudioId = (audioId, showHint = false, lang = '') => {
  const id = String(audioId || '').trim()
  const taskLearningLang = String(lang || learningLang.value || '').trim() || 'zh'
  const queueKey = `${taskLearningLang}:${id}`
  const now = Date.now()
  const lastEnqueueAt = Number(syncLastEnqueueAtMap.get(queueKey) || 0)
  if (lastEnqueueAt > 0 && now - lastEnqueueAt < syncRequeueCooldownMs) return false
  if (!id) return false
  if (syncQueuedIdSet.has(queueKey) || syncingAudioIdSet.has(queueKey)) return true
  syncLastEnqueueAtMap.set(queueKey, now)
  syncQueuedIdSet.add(queueKey)
  syncingAudioIdSet.add(queueKey)
  syncQueuedTasks.push({ id, learningLang: taskLearningLang, nativeLang: nativeLang.value })
  syncQueueTotal.value += 1
  console.info('[LangLearning] syncQueue:enqueue', {
    audioId: id,
    learningLang: taskLearningLang,
    queueLength: syncQueuedTasks.length,
    running: syncQueueRunning.value,
    total: syncQueueTotal.value,
    done: syncQueueDone.value,
  })
  if (activeAudio.value?.id === id) {
    activeAudioSyncGenerating.value = true
  }
  if (showHint) {
    message.info('已加入后台转换队列，将依次转写并翻译')
  }
  ensureSyncPolling()
  runSyncQueueWorker()
  return true
}

const dropSyncTasksForAudioIds = (audioIds) => {
  const idSet = new Set((Array.isArray(audioIds) ? audioIds : []).map((x) => String(x || '').trim()).filter(Boolean))
  if (!idSet.size) return
  for (let i = syncQueuedTasks.length - 1; i >= 0; i -= 1) {
    const taskId = String(syncQueuedTasks[i]?.id || '').trim()
    if (idSet.has(taskId)) {
      syncQueuedTasks.splice(i, 1)
    }
  }
  Array.from(syncQueuedIdSet).forEach((key) => {
    const id = String(key).split(':').slice(-1)[0] || ''
    if (idSet.has(id)) syncQueuedIdSet.delete(key)
  })
  Array.from(syncingAudioIdSet).forEach((key) => {
    const id = String(key).split(':').slice(-1)[0] || ''
    if (idSet.has(id)) syncingAudioIdSet.delete(key)
  })
  idSet.forEach((id) => {
    if (currentSyncingAudioId.value === id) {
      activeAudioSyncGenerating.value = false
    }
  })
}

const resumePendingSyncTasks = async () => {
  try {
    const ids = await invoke('language_learning_list_pending_sync_audio_ids', {
      learningLang: learningLang.value,
    })
    const list = Array.isArray(ids) ? ids : []
    list.forEach((id) => {
      // 已经失败的任务不自动重试，避免每个刷新周期都重新跑一遍长音频；
      // 用户点「重新同步」会显式 force 重跑。
      if (syncJobs.value[String(id)]?.stage === 'error') return
      triggerSyncGenerationForAudioId(id, false, learningLang.value)
    })
  } catch {}
}

const refreshAll = async () => {
  libraryLoading.value = true
  try {
    await refreshRecords()
    await refreshAudio()
    await refreshShelfCounts()
    checkedRowKeys.value = []
    if (!activeRow.value && filteredRows.value.length) {
      activeRow.value = filteredRows.value[0]
    }
  } finally {
    libraryLoading.value = false
  }
}

const onRecordPageChange = async (nextPage) => {
  recordPage.value = Number(nextPage || 1)
  await refreshRecords()
}

const onRecordPageSizeChange = async (size) => {
  recordPageSize.value = Number(size || 100)
  recordPage.value = 1
  await refreshRecords()
}

const onListeningPageChange = (nextPage) => {
  listeningPage.value = Math.max(1, Number(nextPage || 1))
}

const onListeningPageSizeChange = (size) => {
  listeningPageSize.value = Math.max(1, Number(size || 100))
  listeningPage.value = 1
}

const refreshWordMeta = async (row) => {
  if (!row) return
  try {
    const payload = await invoke('language_learning_enrich_word', {
      word: row.word || row.source_text,
      sourceLang: row.source_lang || 'auto',
      targetLang: nativeLang.value || row.target_lang || 'zh',
    })
    await invoke('language_learning_record_translation', {
      input: {
        source_text: row.source_text,
        translated_text: row.translated_text,
        source_lang: row.source_lang,
        target_lang: row.target_lang,
        engine: row.engine,
        record_type: 'word',
        word_meta: {
          word: payload?.word || row.word || row.source_text,
          phonetic: payload?.phonetic || '',
          phonetic_uk: payload?.phonetic_uk || '',
          phonetic_us: payload?.phonetic_us || '',
          part_of_speech: payload?.part_of_speech || '',
          meanings: payload?.meanings || [],
          synonyms: payload?.synonyms || [],
          antonyms: payload?.antonyms || [],
          associates: payload?.associates || [],
          synonyms_translations: payload?.synonyms_translations || [],
          antonyms_translations: payload?.antonyms_translations || [],
          associates_translations: payload?.associates_translations || [],
          phrases: payload?.phrases || [],
          phrase_translations: payload?.phrase_translations || [],
          examples: payload?.examples || [],
          example_translations: payload?.example_translations || [],
        },
      },
    })
    const source = String(payload?.source || '')
    if (source.startsWith('offline:')) {
      activeWordOfflinePack.value = source.slice('offline:'.length)
      message.success(`已从离线词典更新：${activeWordOfflinePack.value}`)
    } else {
      message.success('词汇拓展已更新')
    }
    await refreshRecords()
    const hit = allRows.value.find((x) => x.sourceType === 'record' && x.rawId === row.id)
    if (hit) activeRow.value = hit
  } catch (err) {
    message.error(`刷新词汇拓展失败: ${String(err?.message || err || '')}`)
  }
}

const resetAudioForm = () => {
  audioForm.id = ''
  audioForm.title = ''
  audioForm.source_kind = 'remote'
  audioForm.source_uri = ''
  audioForm.provider = ''
  audioForm.topic = ''
  audioForm.language_tag = learningLang.value
  audioForm.duration_sec = 0
  audioForm.transcript = ''
  audioForm.sync_json_uri = ''
  audioForm.notes = ''
}

const editAudio = (row) => {
  if (!row) return
  audioForm.id = row.id || ''
  audioForm.title = row.title || ''
  audioForm.source_kind = row.source_kind || 'remote'
  audioForm.source_uri = sanitizeMediaUri(row.source_uri || '')
  audioForm.provider = row.provider || ''
  audioForm.topic = row.topic || ''
  audioForm.language_tag = row.language_tag || learningLang.value
  audioForm.duration_sec = Number(row.duration_sec || 0)
  audioForm.transcript = row.transcript || ''
  audioForm.sync_json_uri = row.sync_json_uri || ''
  audioForm.notes = row.notes || ''
  audioManagerVisible.value = true
}

const saveAudio = async () => {
  try {
    if (!String(audioForm.language_tag || '').trim()) {
      audioForm.language_tag = learningLang.value
    }
    if (!String(audioForm.provider || '').trim() && audioForm.source_kind === 'local') {
      audioForm.provider = 'local'
    }
    const saved = await invoke('language_learning_upsert_audio_resource', {
      input: { ...audioForm },
      learningLang: learningLang.value,
    })
    message.success('听力资源已保存')
    if (saved?.id) {
      triggerSyncGenerationForAudioId(saved.id, false)
    }
    resetAudioForm()
    await refreshAudio()
  } catch (err) {
    message.error(`保存听力资源失败: ${String(err?.message || err || '')}`)
  }
}

const removeAudio = (row) => {
  if (!row) return
  askConfirm('删除听力资源', `将删除「${row.title || '未命名听力'}」及其本地缓存文件。`, async () => {
    try {
      await invoke('language_learning_delete_audio_resource', { id: row.id, learningLang: learningLang.value })
      message.success('听力资源已删除')
      if (activeRow.value?.sourceType === 'audio' && activeRow.value?.rawId === row.id) {
        activeRow.value = null
      }
      await refreshAudio()
    } catch (err) {
      message.error(`删除听力资源失败: ${String(err?.message || err || '')}`)
    }
  })
}

const openCreateDialog = () => {
  recordEditMode.value = 'create'
  recordForm.id = ''
  recordForm.source_text = ''
  recordForm.translated_text = ''
  recordForm.record_type = selectedShelf.value === 'my_paragraphs' ? 'paragraph' : 'sentence'
  recordForm.source_lang = 'auto'
  recordForm.target_lang = nativeLang.value
  recordForm.engine = 'Manual'
  recordEditVisible.value = true
}

const openEditDialog = (row) => {
  if (!row || row.sourceType !== 'record') return
  recordEditMode.value = 'edit'
  recordForm.id = String(row.rawId || '')
  recordForm.source_text = String(row.payload?.source_text || '')
  recordForm.translated_text = String(row.payload?.translated_text || '')
  recordForm.record_type = String(row.payload?.record_type || 'sentence')
  recordForm.source_lang = String(row.payload?.source_lang || 'auto')
  recordForm.target_lang = String(row.payload?.target_lang || nativeLang.value)
  recordForm.engine = String(row.payload?.engine || 'Manual')
  recordEditVisible.value = true
}

const saveRecord = async () => {
  try {
    if (!String(recordForm.source_text || '').trim() || !String(recordForm.translated_text || '').trim()) {
      message.warning('请填写原文和译文')
      return
    }
    if (recordEditMode.value === 'edit' && recordForm.id) {
      await invoke('language_learning_update_record', {
        input: { ...recordForm },
        learningLang: learningLang.value,
      })
    } else {
      await invoke('language_learning_record_translation', {
        input: {
          source_text: recordForm.source_text,
          translated_text: recordForm.translated_text,
          source_lang: recordForm.source_lang,
          target_lang: recordForm.target_lang,
          engine: recordForm.engine,
          record_type: recordForm.record_type,
        },
        learningLang: learningLang.value,
      })
    }
    recordEditVisible.value = false
    message.success('记录已保存')
    await refreshAll()
  } catch (err) {
    message.error(`保存记录失败: ${String(err?.message || err || '')}`)
  }
}

const deleteSelectedRows = () => {
  const selected = allRows.value.filter((r) => checkedRowKeys.value.includes(r.id))
  const fallback = activeRow.value ? [activeRow.value] : []
  const rows = selected.length ? selected : fallback
  if (!rows.length) {
    message.warning('请先选择要删除的项目')
    return
  }
  const label = rows.length === 1 ? `「${rows[0].title || '未命名'}」` : `${rows.length} 项学习记录`
  askConfirm('删除学习记录', `将永久删除 ${label}，听力对应的本地音频缓存也会一并清理。此操作不可撤销。`, async () => {
    try {
      const recordIds = rows.filter((r) => r.sourceType === 'record').map((r) => r.rawId)
      const audioIds = rows.filter((r) => r.sourceType === 'audio').map((r) => r.rawId)
      dropSyncTasksForAudioIds(audioIds)
      if (recordIds.length) {
        await invoke('language_learning_delete_records', {
          ids: recordIds,
          learningLang: learningLang.value,
        })
      }
      for (const id of audioIds) {
        await invoke('language_learning_delete_audio_resource', { id, learningLang: learningLang.value })
      }
      message.success(`已删除 ${rows.length} 项`)
      activeRow.value = null
      await refreshAll()
      rebuildReviewRows()
    } catch (err) {
      message.error(`删除失败: ${String(err?.message || err || '')}`)
    }
  })
}

const openCollectSourceDialog = () => {
  audioManagerVisible.value = false
  if (!String(collectTopic.value || '').trim() && String(audioForm.topic || '').trim()) {
    collectTopic.value = audioForm.topic
  }
  const ids = collectAvailableSourcePayload.value.map((x) => x.id)
  collectSourceIds.value = ids
  collectSourceMode.value = 'default'
  collectSourceDialogVisible.value = true
}

const confirmCollectListeningByTopic = async () => {
  const available = collectAvailableSourcePayload.value
  if (!available.length) {
    message.warning(collectSourceEmptyHint.value)
    return
  }
  let selected = available
  if (collectSourceMode.value === 'custom') {
    const selectedIdSet = new Set((Array.isArray(collectSourceIds.value) ? collectSourceIds.value : []).map((x) => String(x)))
    selected = available.filter((x) => selectedIdSet.has(String(x.id)))
    if (!selected.length) {
      message.warning('请至少选择一个抓取来源')
      return
    }
  }
  const baseTopic = String(collectTopic.value || audioForm.topic || '').trim()
  const categoryLabel = resolveTopicCategoryLabel(collectTopicCategory.value)
  const topic = (baseTopic || categoryLabel).trim()
  if (!topic) {
    message.warning('请先输入听力主题或选择分类')
    return
  }
  if (Number(collectMinMinutes.value || 0) > Number(collectMaxMinutes.value || 0)) {
    message.warning('最小时长不能大于最大时长')
    return
  }
  collectSourceDialogVisible.value = false
  await previewCollectListeningByTopic(topic, selected.map(({ id, ...rest }) => rest))
}

const previewCollectListeningByTopic = async (topic, selectedSources) => {
  collectPreviewLoading.value = true
  collectListeningLoading.value = true
  try {
    const rows = await invoke('language_learning_preview_audio_by_topic', {
      input: {
        topic,
        learning_lang: learningLang.value,
        limit: Number(collectLimit.value || 15),
        min_duration_sec: Math.round(Number(collectMinMinutes.value || 0) * 60),
        max_duration_sec: Math.round(Number(collectMaxMinutes.value || 15) * 60),
        sources: selectedSources,
      },
    })
    const list = Array.isArray(rows) ? rows : []
    collectPreviewRows.value = list
    collectPreviewSelected.value = list
      .filter((item) => !item.already_collected)
      .map((item) => item.candidate_id)
    collectPreviewTopic.value = topic
    collectPreviewVisible.value = true
    audioForm.topic = topic
    if (!list.length) message.warning('没有找到符合时长与主题的候选，试试放宽时长')
  } catch (err) {
    message.error(`搜索候选失败: ${String(err?.message || err || '')}`)
  } finally {
    collectPreviewLoading.value = false
    collectListeningLoading.value = false
  }
}

const isPreviewSelected = (candidate) => collectPreviewSelected.value.includes(candidate?.candidate_id)

const togglePreviewCandidate = (candidate, checked) => {
  const id = String(candidate?.candidate_id || '')
  if (!id) return
  const set = new Set(collectPreviewSelected.value)
  if (checked) set.add(id)
  else set.delete(id)
  collectPreviewSelected.value = [...set]
}

const collectPreviewAllSelected = computed(() => {
  const importable = collectPreviewRows.value.filter((item) => !item.already_collected)
  return importable.length > 0 && importable.every((item) => isPreviewSelected(item))
})

const togglePreviewAll = (checked) => {
  collectPreviewSelected.value = checked
    ? collectPreviewRows.value.filter((item) => !item.already_collected).map((item) => item.candidate_id)
    : []
}

const importSelectedCandidates = async () => {
  const candidates = collectPreviewRows.value.filter((item) => isPreviewSelected(item))
  if (!candidates.length) return
  collectImporting.value = true
  try {
    const saved = await invoke('language_learning_import_audio_candidates', {
      input: {
        learning_lang: learningLang.value,
        topic: collectPreviewTopic.value,
        candidates,
      },
    })
    const rows = Array.isArray(saved) ? saved : []
    message.success(`已导入 ${rows.length} 条听力，正在转入后台转写`)
    collectPreviewVisible.value = false
    rows.forEach((item) => {
      if (item?.id) triggerSyncGenerationForAudioId(item.id, false)
    })
    await refreshAudio()
    await refreshShelfCounts()
    const first = rows[0]
    if (first?.id) {
      const hit = allRows.value.find((item) => item.id === `audio:${first.id}`)
      if (hit) activeRow.value = hit
    }
  } catch (err) {
    message.error(`导入候选失败: ${String(err?.message || err || '')}`)
  } finally {
    collectImporting.value = false
  }
}

const importLocalAudio = async () => {
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Audio', extensions: ['mp3', 'm4a', 'wav', 'flac', 'ogg'] }],
    })
    if (!path || Array.isArray(path)) return
    audioForm.source_kind = 'local'
    audioForm.source_uri = String(path)
    audioForm.provider = 'local'
    audioForm.language_tag = learningLang.value
    if (!audioForm.title.trim()) {
      const normalized = String(path).replace(/\\/g, '/')
      audioForm.title = normalized.split('/').pop() || '本地音频'
    }
    audioForm.transcript = ''
    audioForm.sync_json_uri = ''
    message.success('已选择本地音频，保存后将自动在后台生成同步数据')
  } catch (err) {
    message.error(`选择文件失败: ${String(err?.message || err || '')}`)
  }
}

const openAudioManager = () => {
  audioManagerVisible.value = true
}

const resetReviewFilter = () => {
  reviewFilter.createdPeriod = 'all'
  reviewFilter.lastReviewPeriod = 'all'
  reviewFilter.masteryBand = 'all'
  reviewFilter.orderBy = 'created_desc'
  reviewFilter.dueOnly = true
  reviewFilter.includeNew = true
  reviewType.value = 'mixed'
  rebuildReviewRows()
}

const selectedShelfLabel = computed(() => {
  const hit = leftShelfItems.value.find((x) => x.key === selectedShelf.value)
  return hit?.label || '当前分类'
})

const transcriptDisplayModeLabel = computed(() => {
  const hit = transcriptDisplayModes.find((x) => x.value === transcriptDisplayMode.value)
  return hit?.label || '原文+译文'
})
const subtitleDisplayModeLabel = computed(() => {
  const hit = subtitleDisplayModes.find((x) => x.value === subtitleDisplayMode.value)
  return hit ? hit.label : '双语字幕'
})

const cycleTranscriptDisplayMode = () => {
  const idx = transcriptDisplayModes.findIndex((x) => x.value === transcriptDisplayMode.value)
  const next = idx < 0 ? 0 : (idx + 1) % transcriptDisplayModes.length
  transcriptDisplayMode.value = transcriptDisplayModes[next].value
}
const cycleSubtitleDisplayMode = () => {
  const idx = subtitleDisplayModes.findIndex((x) => x.value === subtitleDisplayMode.value)
  const next = idx < 0 ? 0 : (idx + 1) % subtitleDisplayModes.length
  subtitleDisplayMode.value = subtitleDisplayModes[next].value
}
const increaseTranscriptFontScale = () => {
  transcriptFontScale.value = Math.min(1.42, Number((transcriptFontScale.value + 0.08).toFixed(2)))
}
const decreaseTranscriptFontScale = () => {
  transcriptFontScale.value = Math.max(0.96, Number((transcriptFontScale.value - 0.08).toFixed(2)))
}

const currentRandomCards = computed(() => {
  const deck = randomDecks.value[randomDeckIndex.value]
  return Array.isArray(deck) ? deck : []
})

const chunkByThree = (list) => {
  const out = []
  for (let i = 0; i < list.length; i += 3) {
    out.push(list.slice(i, i + 3))
  }
  return out
}

const drawRandomCards = () => {
  stopRandomCardAudio()
  const candidates = filteredRows.value.filter(isReviewableRow)
  if (!candidates.length) {
    randomDecks.value = []
    randomDeckIndex.value = 0
    randomFlippedMap.value = {}
    return
  }
  const pool = [...candidates]
  for (let i = pool.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1))
    const t = pool[i]
    pool[i] = pool[j]
    pool[j] = t
  }
  randomDecks.value = chunkByThree(pool)
  randomDeckIndex.value = 0
  randomFlippedMap.value = {}
}

const gotoPrevRandomDeck = () => {
  if (!randomDecks.value.length) return
  stopRandomCardAudio()
  randomDeckIndex.value = (randomDeckIndex.value - 1 + randomDecks.value.length) % randomDecks.value.length
}

const gotoNextRandomDeck = () => {
  if (!randomDecks.value.length) return
  stopRandomCardAudio()
  randomDeckIndex.value = (randomDeckIndex.value + 1) % randomDecks.value.length
}

const isCardFlipped = (card) => Boolean(randomFlippedMap.value[String(card.id || '')])

const toggleRandomCardFlip = (card) => {
  const key = String(card?.id || '')
  if (!key) return
  randomFlippedMap.value = {
    ...randomFlippedMap.value,
    [key]: !randomFlippedMap.value[key],
  }
}

const toggleCurrentDeckFlip = () => {
  const cards = currentRandomCards.value
  if (!cards.length) return
  const allFlipped = cards.every((x) => isCardFlipped(x))
  const next = { ...randomFlippedMap.value }
  cards.forEach((card) => {
    next[String(card.id || '')] = !allFlipped
  })
  randomFlippedMap.value = next
}

const openRandomCheckBoard = () => {
  activeTab.value = 'manager'
  managerMode.value = 'random'
  drawRandomCards()
  if (!currentRandomCards.value.length) {
    message.warning('当前分类暂无可抽查内容')
  }
}

const randomCardTimelineFor = (card) => {
  const cardId = String(card?.id || '')
  return cardId && cardId === randomCardTimelineCardId.value ? randomCardTimeline.value : []
}

const isRandomTranscriptLineActive = (card, idx) => {
  const cardId = String(card?.id || '')
  return cardId === randomCardTimelineCardId.value && Number(idx) === randomCardActiveLineIndex.value
}

const syncRandomCardTranscriptHighlight = (player) => {
  const timeline = randomCardTimeline.value
  if (!timeline.length) return
  const idx = resolveTranscriptHighlightIndex(timeline, Number(player?.currentTime || 0), -0.03)
  if (idx === randomCardActiveLineIndex.value) return
  randomCardActiveLineIndex.value = idx
  nextTick(() => {
    const el = document.querySelector('.random-transcript-line.active')
    if (el && typeof el.scrollIntoView === 'function') {
      el.scrollIntoView({ block: 'nearest' })
    }
  })
}

const loadRandomCardTimelineFor = async (card) => {
  const cardId = String(card?.id || '')
  const payload = card?.payload || {}
  let timeline = []
  const syncPath = sanitizeMediaUri(payload.sync_json_uri || '')
  if (syncPath) {
    timeline = await loadSyncTimelineByJsonPath(syncPath)
  }
  if (!timeline.length) {
    timeline = buildTranscriptTimeline(String(payload.transcript || ''), Number(payload.duration_sec || 0))
  }
  timeline = timeline.filter((line) => String(line?.text || '').trim())
  // 加载期间可能已经切到别的卡片/停止了播放。
  if (randomCardTimelineCardId.value !== cardId) return
  randomCardTimeline.value = timeline
  if (randomCardAudioPlayer && randomCardAudioPlayingId.value === cardId) {
    syncRandomCardTranscriptHighlight(randomCardAudioPlayer)
  }
}

const playRandomCardAudio = (card) => {
  if (String(card?.rowType || '') !== 'listening') return
  if (card?.payload?.audio_available === false) return
  const cardId = String(card?.id || '')
  if (!cardId || !sanitizeMediaUri(card?.payload?.source_uri || '')) return
  if (randomCardAudioPlayer && randomCardAudioPlayingId.value === cardId) {
    if (!randomCardAudioPlayer.paused) {
      randomCardAudioPlayer.pause()
      randomCardAudioPaused.value = true
      return
    }
    randomCardAudioPlayer.currentTime = 0
    randomCardActiveLineIndex.value = -1
    randomCardAudioPaused.value = false
    randomCardAudioPlayer.play().catch(() => {
      stopRandomCardAudio()
    })
    return
  }
  try {
    resolvePlayableMediaSrc(String(card.payload.source_uri || ''))
      .then((src) => {
        if (!src) return
        stopRandomCardAudio()
        const player = new Audio(src)
        randomCardAudioPlayer = player
        randomCardAudioPlayingId.value = cardId
        randomCardAudioPaused.value = false
        randomCardTimelineCardId.value = cardId
        randomCardTimeline.value = []
        randomCardActiveLineIndex.value = -1
        loadRandomCardTimelineFor(card)
        player.ontimeupdate = () => {
          syncRandomCardTranscriptHighlight(player)
        }
        player.onended = () => {
          stopRandomCardAudio()
        }
        player.onerror = () => {
          stopRandomCardAudio()
        }
        player.play().catch(() => {
          stopRandomCardAudio()
        })
      })
      .catch(() => {})
  } catch {}
}

const stopRandomCardAudio = () => {
  if (randomCardAudioPlayer) {
    randomCardAudioPlayer.ontimeupdate = null
    randomCardAudioPlayer.onended = null
    randomCardAudioPlayer.onerror = null
    randomCardAudioPlayer.pause()
    randomCardAudioPlayer = null
  }
  randomCardAudioPlayingId.value = ''
  randomCardAudioPaused.value = false
  randomCardTimelineCardId.value = ''
  randomCardTimeline.value = []
  randomCardActiveLineIndex.value = -1
}

const isRandomCardAudioActive = (card) => String(card?.id || '') === randomCardAudioPlayingId.value

const isRandomCardAudioPlaying = (card) => isRandomCardAudioActive(card) && !randomCardAudioPaused.value

const isRandomCardAudioPaused = (card) => isRandomCardAudioActive(card) && randomCardAudioPaused.value

const randomCardAudioActionState = (card) => {
  if (isRandomCardAudioPlaying(card)) return 'playing'
  if (isRandomCardAudioPaused(card)) return 'paused'
  return 'idle'
}

const randomCardAudioActionIcon = (card) => {
  if (isRandomCardAudioPlaying(card)) return pauseIcon
  if (isRandomCardAudioPaused(card)) return refreshIcon
  return playIcon
}

const randomCardAudioActionTitle = (card) => {
  if (isRandomCardAudioPlaying(card)) return '暂停听力'
  if (isRandomCardAudioPaused(card)) return '重新播放听力'
  return '播放听力'
}

const handleListeningCardScrollKeydown = (evt) => {
  const target = evt?.currentTarget
  if (!(target instanceof HTMLElement)) return
  let handled = true
  if (evt.key === 'ArrowDown') {
    target.scrollBy({ top: 44, behavior: 'smooth' })
  } else if (evt.key === 'ArrowUp') {
    target.scrollBy({ top: -44, behavior: 'smooth' })
  } else if (evt.key === 'PageDown') {
    target.scrollBy({ top: Math.max(120, Math.round(target.clientHeight * 0.82)), behavior: 'smooth' })
  } else if (evt.key === 'PageUp') {
    target.scrollBy({ top: -Math.max(120, Math.round(target.clientHeight * 0.82)), behavior: 'smooth' })
  } else if (evt.key === 'Home') {
    target.scrollTo({ top: 0, behavior: 'smooth' })
  } else if (evt.key === 'End') {
    target.scrollTo({ top: target.scrollHeight, behavior: 'smooth' })
  } else {
    handled = false
  }
  if (handled) {
    evt.preventDefault()
  }
}

const cardWordPairs = (words, trans) => {
  const src = normalizeList(words)
  const target = Array.isArray(trans) ? trans : []
  return src.map((word, idx) => ({ word, translation: String(target[idx] || '').trim() }))
}

const cardWordMeanings = (card) => {
  const meanings = Array.isArray(card?.payload?.meanings) && card.payload.meanings.length
    ? card.payload.meanings
    : splitSimpleTranslations(card?.translatedText || '')
  return normalizeList(meanings)
}

const handleReviewKeydown = (evt) => {
  if (activeTab.value !== 'review') return
  if (confirmDialog.show || collectPreviewVisible.value || collectSourceDialogVisible.value || audioManagerVisible.value || recordEditVisible.value || sourceManagerVisible.value) return
  const target = evt.target
  const tag = String(target?.tagName || '').toUpperCase()
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target?.isContentEditable) return
  if (evt.code === 'Space') {
    evt.preventDefault()
    reviewShowAnswer.value = !reviewShowAnswer.value
    return
  }
  if (evt.key === '1') {
    evt.preventDefault()
    gradeAndNext('known')
    return
  }
  if (evt.key === '2') {
    evt.preventDefault()
    gradeAndNext('fuzzy')
    return
  }
  if (evt.key === '3') {
    evt.preventDefault()
    gradeAndNext('unknown')
    return
  }
  if (evt.key === 'ArrowRight') {
    evt.preventDefault()
    gotoNextReview()
    return
  }
  if (evt.key === 'ArrowLeft') {
    evt.preventDefault()
    gotoPrevReview()
  }
}

const handleRandomBoardKeydown = (evt) => {
  if (managerMode.value !== 'random' || activeTab.value !== 'manager') return
  if (evt.key === 'ArrowLeft') {
    evt.preventDefault()
    gotoPrevRandomDeck()
    return
  }
  if (evt.key === 'ArrowRight') {
    evt.preventDefault()
    gotoNextRandomDeck()
    return
  }
  if (evt.key === ' ' || evt.code === 'Space') {
    evt.preventDefault()
    toggleCurrentDeckFlip()
    return
  }
  if (evt.key === 'Enter') {
    evt.preventDefault()
    drawRandomCards()
  }
}

const parseTimestampToSec = (raw) => {
  const src = String(raw || '').trim()
  const hit = src.match(/(\d{1,2}:)?\d{1,2}:\d{2}(?:[.,]\d{1,3})?/)
  const t = String(hit?.[0] || '').trim().replace(',', '.')
  if (!t) return null
  const parts = t.split(':').map((x) => Number(String(x).trim()))
  if (parts.some((x) => !Number.isFinite(x))) return null
  if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2]
  if (parts.length === 2) return parts[0] * 60 + parts[1]
  if (parts.length === 1) return parts[0]
  return null
}

const parseTimedTranscript = (rawText) => {
  const text = String(rawText || '').trim()
  if (!text) return []
  const out = []
  const lines = text.split(/\r?\n/)
  const lrcPattern = /\[(\d{1,2}):(\d{2})(?:[.:](\d{1,3}))?\]/g
  const cuePattern = /^(\d{1,2}:)?\d{1,2}:\d{2}(?:[.,]\d{1,3})?\s*-->\s*(\d{1,2}:)?\d{1,2}:\d{2}(?:[.,]\d{1,3})?(?:\s+.*)?$/
  let i = 0
  while (i < lines.length) {
    const line = String(lines[i] || '').trim()
    if (!line) {
      i += 1
      continue
    }
    if (/^WEBVTT$/i.test(line) || /^NOTE(?:\s|$)/i.test(line)) {
      i += 1
      continue
    }
    if (/^\d+$/.test(line)) {
      i += 1
      continue
    }
    const cueMatch = cuePattern.test(line)
    if (cueMatch) {
      const [left, right] = line.split('-->').map((x) => String(x || '').trim())
      const start = parseTimestampToSec(left)
      const end = parseTimestampToSec(right)
      i += 1
      const textParts = []
      while (i < lines.length && String(lines[i] || '').trim()) {
        textParts.push(String(lines[i] || '').trim())
        i += 1
      }
      const cueText = textParts.join(' ').trim()
      if (cueText && Number.isFinite(start) && Number.isFinite(end)) {
        out.push({ text: cueText, start: Number(start), end: Number(end) })
      }
      continue
    }
    let lrcMatched = false
    const tags = [...line.matchAll(lrcPattern)]
    if (tags.length) {
      const plain = line.replace(lrcPattern, '').trim()
      if (plain) {
        tags.forEach((m) => {
          const mm = Number(m[1] || 0)
          const ss = Number(m[2] || 0)
          const ms = Number(String(m[3] || '0').padEnd(3, '0').slice(0, 3))
          const start = mm * 60 + ss + ms / 1000
          out.push({ text: plain, start, end: null })
        })
        lrcMatched = true
      }
    }
    if (!lrcMatched) {
      const parts = line
        .split(/(?<=[。！？!?])|(?<=[.])\s+/g)
        .map((x) => String(x || '').trim())
        .filter(Boolean)
      parts.forEach((x) => out.push({ text: x, start: null, end: null }))
    }
    i += 1
  }
  return out
}

const buildTranscriptTimeline = (rawText, durationSec) => {
  const parsed = parseTimedTranscript(rawText)
  if (!parsed.length) return []
  if (parsed.some((x) => Number.isFinite(x.start) || Number.isFinite(x.end))) {
    const sorted = [...parsed].sort((a, b) => (Number(a.start ?? Number.MAX_SAFE_INTEGER) - Number(b.start ?? Number.MAX_SAFE_INTEGER)))
    for (let i = 0; i < sorted.length; i += 1) {
      if (!Number.isFinite(sorted[i].start)) {
        sorted[i].start = i === 0 ? 0 : Number(sorted[i - 1].end ?? sorted[i - 1].start ?? 0)
      }
      if (!Number.isFinite(sorted[i].end)) {
        const nextStart = i < sorted.length - 1 ? Number(sorted[i + 1].start ?? NaN) : NaN
        sorted[i].end = Number.isFinite(nextStart) ? nextStart : Number(sorted[i].start) + 2
      }
      if (Number(sorted[i].end) <= Number(sorted[i].start)) {
        sorted[i].end = Number(sorted[i].start) + 1.6
      }
    }
    return sorted
  }
  const duration = Number(durationSec || 0)
  const safeDuration = Number.isFinite(duration) && duration > 0 ? duration : parsed.length * 2.5
  const totalWeight = parsed.reduce((sum, x) => sum + Math.max(8, String(x.text || '').length), 0)
  let cursor = 0
  return parsed.map((x, idx) => {
    const weight = Math.max(8, String(x.text || '').length)
    const seg = (weight / totalWeight) * safeDuration
    const start = cursor
    const end = idx === parsed.length - 1 ? safeDuration : cursor + Math.max(0.6, seg)
    cursor = end
    const source = String(x.text || '').trim()
    return { text: source, translated_text: '', start, end }
  })
}

const activeAudioTranscriptTimeline = computed(() => {
  if (activeAudioSyncTimeline.value.length) return activeAudioSyncTimeline.value
  return buildTranscriptTimeline(activeAudio.value?.transcript || '', activeAudioDurationSec.value || activeAudio.value?.duration_sec || 0)
})
const formatVttTimelineTime = (secRaw) => {
  const sec = Math.max(0, Number(secRaw || 0))
  const h = Math.floor(sec / 3600)
  const m = Math.floor((sec % 3600) / 60)
  const s = Math.floor(sec % 60)
  const ms = Math.round((sec - Math.floor(sec)) * 1000)
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}.${String(ms).padStart(3, '0')}`
}
const escapeVttText = (text) => String(text || '').replace(/[\r\n]+/g, ' ').trim()
const resolveSubtitleCueText = (line) => {
  const source = escapeVttText(line?.text)
  const target = escapeVttText(resolveTranscriptTargetText(line))
  if (subtitleDisplayMode.value === 'source') return source
  if (subtitleDisplayMode.value === 'target') return target || source
  return target && target !== '（译文生成中）' ? `${source}\n${target}` : source
}
const transcriptListStyle = computed(() => ({
  '--transcript-font-scale': String(transcriptFontScale.value),
}))
const activeMediaSubtitleVttSrc = computed(() => {
  if (!activeMediaIsVideo.value) return ''
  const timeline = activeAudioTranscriptTimeline.value
  if (!Array.isArray(timeline) || !timeline.length) return ''
  const lines = ['WEBVTT', '']
  timeline.forEach((line, idx) => {
    const cueText = resolveSubtitleCueText(line)
    if (!cueText) return
    const start = Number.isFinite(Number(line?.start)) ? Number(line.start) : idx * 2
    const endRaw = Number.isFinite(Number(line?.end)) ? Number(line.end) : start + 2
    const end = endRaw > start ? endRaw : start + 1.2
    lines.push(`${formatVttTimelineTime(start)} --> ${formatVttTimelineTime(end)}`)
    lines.push(cueText)
    lines.push('')
  })
  if (lines.length <= 2) return ''
  return `data:text/vtt;charset=utf-8,${encodeURIComponent(lines.join('\n'))}`
})
const visibleActiveAudioTranscriptTimeline = computed(() => {
  const timeline = activeAudioTranscriptTimeline.value
  const minCount = Math.max(activeTranscriptVisibleCount.value, currentTranscriptLineIndex.value + 8)
  return timeline.slice(0, minCount).map((line, idx) => ({ line, idx }))
})
const isAudioSyncQueued = (audioId) => {
  const id = String(audioId || '').trim()
  if (!id) return false
  return syncQueuedTasks.some((task) => String(task?.id || '').trim() === id)
}
const activeTranscriptTranslating = computed(() => {
  const id = String(activeAudio.value?.id || '').trim()
  if (!id) return activeTimelineQuickTranslating.value
  if (!timelineHasMissingTranslation(activeAudioTranscriptTimeline.value)) return false
  if (currentSyncingAudioId.value === id) return true
  if (isAudioSyncQueued(id)) return true
  return activeAudioSyncGenerating.value || activeTimelineQuickTranslating.value
})

const resolveTranscriptTargetText = (line) => {
  const translated = String(line?.translated_text || line?.translation || '').trim()
  if (translated) return translated
  return '（译文生成中）'
}

const isSentenceTail = (text) => /[.!?。！？…]["')\]]*$/.test(String(text || '').trim())

const buildSentenceGroupsFromTimeline = (timeline) => {
  const list = Array.isArray(timeline) ? timeline : []
  const groups = []
  let current = []
  let currentLen = 0
  list.forEach((line, idx) => {
    const source = String(line?.text || '').trim()
    if (!source) return
    current.push(idx)
    currentLen += source.length
    if (isSentenceTail(source) || currentLen >= 220 || current.length >= 6) {
      groups.push(current)
      current = []
      currentLen = 0
    }
  })
  if (current.length) groups.push(current)
  return groups
}

const splitTranslationByWeights = (translated, weights) => {
  const text = String(translated || '').trim()
  const ws = Array.isArray(weights) ? weights : []
  if (!ws.length) return []
  if (ws.length === 1) return [text]
  const chars = [...text]
  if (!chars.length) return ws.map(() => '')
  const totalChars = chars.length
  const totalWeight = Math.max(1, ws.reduce((sum, n) => sum + Math.max(1, Number(n || 1)), 0))
  const out = []
  let cursor = 0
  let usedWeight = 0
  ws.forEach((w, idx) => {
    if (idx === ws.length - 1) {
      out.push(chars.slice(cursor).join('').trim())
      return
    }
    usedWeight += Math.max(1, Number(w || 1))
    let end = Math.round((usedWeight / totalWeight) * totalChars)
    if (end <= cursor) end = Math.min(totalChars, cursor + 1)
    out.push(chars.slice(cursor, end).join('').trim())
    cursor = end
  })
  return out
}

const timelineHasMissingTranslation = (timeline) =>
  Array.isArray(timeline) && timeline.some((x) => {
    const translated = String(x?.translated_text || x?.translation || '').trim()
    if (!translated) return true
    const source = String(x?.text || '').trim()
    return source && translated === source
  })

const quickTranslateTimelineToZh = async (timeline, stateRef) => {
  if (!Array.isArray(timeline) || !timeline.length || stateRef.value) return timeline
  if (String(nativeLang.value) === String(learningLang.value)) return timeline
  const groups = buildSentenceGroupsFromTimeline(timeline)
  const requestGroups = groups
    .map((indices) => {
      const items = indices.map((idx) => timeline[idx]).filter(Boolean)
      const hasMissing = items.some((line) => {
        const translated = String(line?.translated_text || line?.translation || '').trim()
        const source = String(line?.text || '').trim()
        return source && (!translated || translated === source)
      })
      if (!hasMissing) return null
      const sourceText = items.map((line) => String(line?.text || '').trim()).filter(Boolean).join(' ')
      const weights = items.map((line) => Math.max(1, String(line?.text || '').trim().length))
      if (!sourceText) return null
      return { indices, sourceText, weights }
    })
    .filter(Boolean)
  if (!requestGroups.length) return timeline
  console.info('[LangLearning] quickTranslate:start', {
    timelineSize: timeline.length,
    missingCount: requestGroups.length,
  })
  try {
    stateRef.value = true
    const result = await invoke('translate_text_free', {
      texts: requestGroups.map((x) => x.sourceText),
      from: 'auto',
      to: nativeLang.value || 'zh',
    })
    const translated = Array.isArray(result?.translated_texts) ? result.translated_texts : []
    const patched = [...timeline]
    requestGroups.forEach((group, gidx) => {
      const translatedSentence = String(translated[gidx] || '').trim()
      if (!translatedSentence || translatedSentence.includes('[Translation Failed]')) return
      const parts = splitTranslationByWeights(translatedSentence, group.weights)
      group.indices.forEach((idx, localIdx) => {
        const line = patched[idx]
        if (!line) return
        const next = String(parts[localIdx] || translatedSentence).trim()
        if (!next) return
        patched[idx] = { ...line, translated_text: next }
      })
    })
    const stillMissing = patched.filter((line) => {
      const translatedText = String(line?.translated_text || line?.translation || '').trim()
      const sourceText = String(line?.text || '').trim()
      return sourceText && (!translatedText || translatedText === sourceText)
    }).length
    console.info('[LangLearning] quickTranslate:done', {
      requested: requestGroups.length,
      received: translated.length,
      stillMissing,
    })
    return patched
  } catch (error) {
    console.error('[LangLearning] quickTranslate:error', {
      message: String(error?.message || error || ''),
      missingCount: requestGroups.length,
    })
    return timeline
  } finally {
    stateRef.value = false
  }
}

const extendActiveTranscriptWindow = (step = transcriptScreenPreloadSize) => {
  const total = activeAudioTranscriptTimeline.value.length
  if (activeTranscriptVisibleCount.value >= total) return
  activeTranscriptVisibleCount.value = Math.min(total, activeTranscriptVisibleCount.value + Math.max(8, Number(step || 0)))
}

const extendReviewTranscriptWindow = (step = transcriptScreenPreloadSize) => {
  const total = currentReviewListeningTranscriptTimeline.value.length
  if (reviewTranscriptVisibleCount.value >= total) return
  reviewTranscriptVisibleCount.value = Math.min(total, reviewTranscriptVisibleCount.value + Math.max(8, Number(step || 0)))
}

const handleActiveTranscriptScroll = (evt) => {
  const el = evt?.target
  if (!el) return
  const nearBottom = Number(el.scrollHeight || 0) - (Number(el.scrollTop || 0) + Number(el.clientHeight || 0))
  if (nearBottom > 260) return
  extendActiveTranscriptWindow()
}

const handleReviewTranscriptScroll = (evt) => {
  const el = evt?.target
  if (!el) return
  const nearBottom = Number(el.scrollHeight || 0) - (Number(el.scrollTop || 0) + Number(el.clientHeight || 0))
  if (nearBottom > 260) return
  extendReviewTranscriptWindow()
}

const shouldRunQuickTimelineTranslate = (audioId) => {
  const id = String(audioId || '').trim()
  if (!id) return false
  const now = Date.now()
  const last = Number(quickTimelineTranslateLastRun.get(id) || 0)
  if (now - last < quickTimelineTranslateCooldownMs) return false
  quickTimelineTranslateLastRun.set(id, now)
  return true
}

const keepActiveTranscriptLineVisible = (containerRef, activeIndex) => {
  const container = containerRef?.value
  if (!container || activeIndex < 0) return
  const activeLine = container.querySelector(`.transcript-line[data-transcript-idx="${Number(activeIndex)}"]`)
  if (!activeLine) return
  const lineTop = Number(activeLine.offsetTop || 0)
  const lineBottom = lineTop + Number(activeLine.offsetHeight || 0)
  const viewportTop = Number(container.scrollTop || 0)
  const viewportBottom = viewportTop + Number(container.clientHeight || 0)
  if (lineTop >= viewportTop && lineBottom <= viewportBottom) return
  const targetTop = Math.max(0, lineTop - Math.max(24, Math.floor(Number(container.clientHeight || 0) * 0.35)))
  container.scrollTo({ top: targetTop, behavior: 'smooth' })
}

const currentReviewListeningTranscriptTimeline = computed(() => {
  const row = currentReviewItem.value
  if (!row || row.rowType !== 'listening' || row.sourceType !== 'audio') return []
  if (reviewAudioSyncTimeline.value.length) return reviewAudioSyncTimeline.value
  return buildTranscriptTimeline(row.payload?.transcript || '', reviewAudioDurationSec.value || row.payload?.duration_sec || 0)
})
const visibleReviewListeningTranscriptTimeline = computed(() => {
  const timeline = currentReviewListeningTranscriptTimeline.value
  const minCount = Math.max(reviewTranscriptVisibleCount.value, currentReviewTranscriptIndex.value + 8)
  return timeline.slice(0, minCount).map((line, idx) => ({ line, idx }))
})
const reviewTranscriptTranslating = computed(() => {
  const row = currentReviewItem.value
  const id = String(row?.payload?.id || '').trim()
  if (!id) return reviewTimelineQuickTranslating.value
  if (!timelineHasMissingTranslation(currentReviewListeningTranscriptTimeline.value)) return false
  if (currentSyncingAudioId.value === id) return true
  if (isAudioSyncQueued(id)) return true
  return reviewTimelineQuickTranslating.value
})

const seekActiveAudioToTranscript = (idx) => {
  const timeline = activeAudioTranscriptTimeline.value
  const item = timeline[Number(idx)]
  const player = activeAudioPlayerRef.value
  if (!item || !player || !Number.isFinite(Number(item.start))) return
  player.currentTime = Math.max(0, Number(item.start || 0))
  player.play().catch(() => {})
}

const resolveTranscriptHighlightIndex = (timeline, currentSec, leadSec = -0.03) => {
  if (!Array.isArray(timeline) || !timeline.length) return 0
  const t = Math.max(0, Number(currentSec || 0) + Number(leadSec || 0))
  const activeIndices = []
  for (let i = 0; i < timeline.length; i += 1) {
    const start = Number(timeline[i]?.start || 0)
    const end = Number(timeline[i]?.end || Number.MAX_SAFE_INTEGER)
    if (t >= start && t < end) {
      activeIndices.push(i)
    }
  }
  if (activeIndices.length) {
    return activeIndices.reduce((picked, idx) => {
      const pickedStart = Number(timeline[picked]?.start || 0)
      const idxStart = Number(timeline[idx]?.start || 0)
      if (idxStart > pickedStart) return idx
      if (idxStart < pickedStart) return picked
      return idx > picked ? idx : picked
    }, activeIndices[0])
  }
  let fallback = -1
  for (let i = 0; i < timeline.length; i += 1) {
    const start = Number(timeline[i]?.start || 0)
    if (!Number.isFinite(start) || t < start) break
    fallback = i
  }
  if (fallback >= 0) return fallback
  return 0
}

const handleAudioTimeUpdate = (evt) => {
  const player = evt?.target
  if (!player || !Number.isFinite(player.currentTime)) return
  const timeline = activeAudioTranscriptTimeline.value
  if (!timeline.length) {
    currentTranscriptLineIndex.value = 0
    return
  }
  currentTranscriptLineIndex.value = resolveTranscriptHighlightIndex(timeline, Number(player.currentTime || 0), -0.03)
}

const getListeningMediaDebugState = (player) => ({
  duration: Number(player?.duration || 0),
  currentTime: Number(player?.currentTime || 0),
  networkState: player?.networkState || 0,
  readyState: player?.readyState || 0,
  currentSrcProtocol: String(player?.currentSrc || '').split(':')[0],
})

const getListeningMediaContext = (scope) => {
  if (scope === 'review') {
    return {
      scope,
      audioId: currentReviewItem.value?.payload?.id || '',
      sourceProtocol: sanitizeMediaUri(currentReviewItem.value?.payload?.source_uri || '').split(':')[0],
      playableProtocol: String(reviewAudioPlayableSrc.value || '').split(':')[0],
    }
  }
  return {
    scope,
    audioId: activeAudio.value?.id || '',
    sourceProtocol: sanitizeMediaUri(activeAudio.value?.source_uri || '').split(':')[0],
    playableProtocol: String(activeAudioPlayableSrc.value || '').split(':')[0],
  }
}

const handleListeningMediaCanPlay = (scope, evt) => {
  console.info('[LangLearning] media:canplay', {
    ...getListeningMediaContext(scope),
    ...getListeningMediaDebugState(evt?.target),
  })
}

const handleListeningMediaPlay = (scope, evt) => {
  console.info('[LangLearning] media:play', {
    ...getListeningMediaContext(scope),
    ...getListeningMediaDebugState(evt?.target),
  })
}

const handleListeningMediaError = (scope, evt) => {
  const player = evt?.target
  const mediaError = player?.error
  console.error('[LangLearning] media:error', {
    ...getListeningMediaContext(scope),
    ...getListeningMediaDebugState(player),
    mediaErrorCode: mediaError?.code || 0,
    mediaErrorMessage: mediaError?.message || '',
  })
}

const onAudioLoadedMetadata = (evt) => {
  const player = evt?.target
  if (player && Number.isFinite(player.duration) && player.duration > 0) {
    activeAudioDurationSec.value = Number(player.duration)
  }
  console.info('[LangLearning] media:loadedmetadata', {
    ...getListeningMediaContext('active'),
    ...getListeningMediaDebugState(player),
  })
  applyAudioPlaybackRate()
}

const handleReviewAudioLoadedMetadata = (evt) => {
  const player = evt?.target
  if (player && Number.isFinite(player.duration) && player.duration > 0) {
    reviewAudioDurationSec.value = Number(player.duration)
  }
  console.info('[LangLearning] media:loadedmetadata', {
    ...getListeningMediaContext('review'),
    ...getListeningMediaDebugState(player),
  })
}

const handleReviewAudioTimeUpdate = (evt) => {
  const player = evt?.target
  if (!player || !Number.isFinite(player.currentTime)) return
  const timeline = currentReviewListeningTranscriptTimeline.value
  if (!timeline.length) {
    currentReviewTranscriptIndex.value = 0
    return
  }
  currentReviewTranscriptIndex.value = resolveTranscriptHighlightIndex(timeline, Number(player.currentTime || 0), -0.03)
}

const applyAudioPlaybackRate = () => {
  const player = activeAudioPlayerRef.value
  if (!player || typeof player.playbackRate !== 'number') return
  const nextRate = Number(playbackRate.value || 1)
  if (!Number.isFinite(nextRate) || nextRate <= 0) return
  player.playbackRate = nextRate
}

const fetchTranscriptForActiveAudio = async () => {
  const row = activeAudio.value
  if (!row?.id) return
  const id = String(row.id)
  syncJobs.value = {
    ...syncJobs.value,
    [id]: { audio_id: id, stage: 'queued', progress: 0, message: '重新转写', error: '' },
  }
  ensureSyncPolling()
  try {
    await invoke('language_learning_generate_whisper_sync_json_async', {
      audioId: id,
      learningLang: learningLang.value,
      nativeLang: nativeLang.value,
      force: true,
    })
    message.info('已提交后台重新转写')
  } catch (err) {
    message.error(`提交同步失败: ${String(err?.message || err || '')}`)
  }
}

let learningPronounceAudio = null
const playWordPronunciation = (rawWord, accent = 'us') => {
  const text = String(rawWord || '').trim()
  if (!text) return
  const type = accent === 'uk' ? '1' : '0'
  const url = `https://dict.youdao.com/dictvoice?audio=${encodeURIComponent(text)}&type=${type}`
  try {
    if (learningPronounceAudio) {
      learningPronounceAudio.pause()
      learningPronounceAudio = null
    }
    learningPronounceAudio = new Audio(url)
    learningPronounceAudio.play().catch(() => {})
  } catch {}
}

const readTextFileByPath = async (path) => {
  const { readFile } = await import('@tauri-apps/plugin-fs')
  const bytes = await readFile(path)
  const decoder = new TextDecoder('utf-8')
  return decoder.decode(bytes)
}

const cleanedTopicLangSet = new Set()
const resolveLearningScope = () => {
  const key = String(selectedShelf.value || 'all').trim()
  return key || 'all'
}
const scopeNeedsAssetPackage = (scope) => scope === 'all' || scope === 'my_listening'
const cleanupDuplicateListeningTopics = async () => {
  const lang = String(learningLang.value || 'zh')
  if (cleanedTopicLangSet.has(lang)) return
  try {
    await invoke('language_learning_cleanup_duplicate_topics', { learningLang: lang })
    cleanedTopicLangSet.add(lang)
  } catch {}
}

const exportLearningData = async () => {
  if (exportingLearningData.value) return
  exportingLearningData.value = true
  try {
    const scope = resolveLearningScope()
    const isAssetPackage = scopeNeedsAssetPackage(scope)
    const ext = isAssetPackage ? 'zip' : 'json'
    const defaultName = `language-learning-export-${scope}-${Date.now()}.${ext}`
    const path = await save({
      defaultPath: defaultName,
      filters: [{ name: isAssetPackage ? 'ZIP' : 'JSON', extensions: [ext] }],
    })
    if (!path) return
    const result = await invoke('language_learning_export_scope_package', {
      learningLang: learningLang.value,
      scope,
      outputPath: String(path),
      mastery: masteryStore.value,
    })
    message.success(`学习数据已导出（${scope}）`)
    console.info('[LangLearning] export-scope-package', result)
  } catch (err) {
    message.error(`导出失败: ${String(err?.message || err || '')}`)
  } finally {
    exportingLearningData.value = false
  }
}

const importLearningData = async () => {
  if (importingLearningData.value) return
  importingLearningData.value = true
  try {
    const scope = resolveLearningScope()
    const path = await open({
      multiple: false,
      filters: [
        { name: 'Package', extensions: ['zip', 'json'] },
      ],
    })
    if (!path || Array.isArray(path)) return
    const rowsBefore = allRows.value.length
    const result = await invoke('language_learning_import_scope_package', {
      learningLang: learningLang.value,
      scope,
      inputPath: String(path),
    })
    if (result?.mastery && typeof result.mastery === 'object') {
      masteryStore.value = {
        record: result.mastery.record && typeof result.mastery.record === 'object' ? result.mastery.record : masteryStore.value.record,
        audio: result.mastery.audio && typeof result.mastery.audio === 'object' ? result.mastery.audio : masteryStore.value.audio,
      }
      saveMasteryStore()
    }
    await cleanupDuplicateListeningTopics()
    await refreshAll()
    const rowsAfter = allRows.value.length
    message.success(`导入完成（${scope}），新增或更新 ${Math.max(0, rowsAfter - rowsBefore)} 项`)
    console.info('[LangLearning] import-scope-package', result)
  } catch (err) {
    message.error(`导入失败: ${String(err?.message || err || '')}`)
  } finally {
    importingLearningData.value = false
  }
}

watch(listeningKindFilter, () => {
  listeningPage.value = 1
  if (selectedShelf.value !== 'my_listening') return
  if (!listeningRows.value.length) {
    activeRow.value = null
    return
  }
  if (!activeRow.value || !listeningRows.value.some((x) => x.id === activeRow.value.id)) {
    activeRow.value = listeningRows.value[0]
  }
})

watch(listeningProviderFilter, () => {
  listeningPage.value = 1
  if (selectedShelf.value !== 'my_listening') return
  if (!listeningRows.value.length) {
    activeRow.value = null
    return
  }
  if (!activeRow.value || !listeningRows.value.some((x) => x.id === activeRow.value.id)) {
    activeRow.value = listeningRows.value[0]
  }
})

watch(selectedShelf, async () => {
  listeningPage.value = 1
  if (selectedShelf.value !== 'my_listening') {
    listeningImmersiveMode.value = false
  }
  recordPage.value = 1
  await refreshRecords()
  if (managerMode.value === 'random') {
    drawRandomCards()
  }
  const targetRows = selectedShelf.value === 'my_listening' ? listeningRows.value : filteredRows.value
  if (!targetRows.length) {
    activeRow.value = null
    return
  }
  if (!activeRow.value || !targetRows.some((x) => x.id === activeRow.value.id)) {
    activeRow.value = targetRows[0]
  }
})

watch(keyword, async () => {
  listeningPage.value = 1
  recordPage.value = 1
  await refreshAll()
  if (managerMode.value === 'random') {
    drawRandomCards()
  }
})

watch(learningLang, async (v) => {
  try {
    window.localStorage.setItem(learningLangStorageKey, String(v || 'zh'))
  } catch {}
  activeRow.value = null
  await cleanupDuplicateListeningTopics()
  await refreshAll()
  await resumePendingSyncTasks()
  rebuildReviewRows()
})

watch(nativeLang, (value, previous) => {
  try {
    window.localStorage.setItem(nativeLangStorageKey, String(value || 'zh'))
  } catch {}
  if (previous && value !== previous) {
    message.info('母语已切换；已有译文不会自动改写，可在听力条目上点「重译」按新母语生成')
  }
})

watch([listeningStatusFilter, listeningSort], () => {
  listeningPage.value = 1
  if (selectedShelf.value !== 'my_listening') return
  if (!filteredListeningRows.value.length) {
    activeRow.value = null
    return
  }
  if (!activeRow.value || !filteredListeningRows.value.some((x) => x.id === activeRow.value.id)) {
    activeRow.value = filteredListeningRows.value[0]
  }
})

watch(playbackRate, () => {
  applyAudioPlaybackRate()
})

watch(syncQueueRunning, (running) => {
  if (syncQueueHideTimer) {
    window.clearTimeout(syncQueueHideTimer)
    syncQueueHideTimer = 0
  }
  if (running) {
    syncQueueRunStartAt = Date.now()
    syncQueueVisualRunning.value = true
    return
  }
  const elapsed = Date.now() - syncQueueRunStartAt
  const waitMs = Math.max(0, 1800 - elapsed)
  syncQueueHideTimer = window.setTimeout(() => {
    syncQueueVisualRunning.value = false
    if (syncQueueDone.value >= syncQueueTotal.value) {
      syncQueueDone.value = 0
      syncQueueTotal.value = 0
    }
    syncQueueHideTimer = 0
  }, waitMs)
}, { immediate: true })

watch(activeAudio, async () => {
  currentTranscriptLineIndex.value = 0
  activeTranscriptVisibleCount.value = transcriptScreenPageSize
  activeAudioDurationSec.value = Number(activeAudio.value?.duration_sec || 0)
  const audioUnavailable = activeAudio.value?.audio_available === false
  activeAudioSyncTimeline.value = audioUnavailable ? [] : await loadSyncTimelineByJsonPath(activeAudio.value?.sync_json_uri || '')
  if (!activeAudioSyncTimeline.value.length) {
    activeAudioSyncTimeline.value = buildTranscriptTimeline(activeAudio.value?.transcript || '', activeAudioDurationSec.value || 0)
  }
  if (activeAudio.value?.id && activeAudioSyncTimeline.value.length && timelineHasMissingTranslation(activeAudioSyncTimeline.value)) {
    console.info('[LangLearning] activeAudio:translation-missing', {
      audioId: activeAudio.value.id,
      timelineSize: activeAudioSyncTimeline.value.length,
      pending: activeAudioSyncTimeline.value.filter((x) => {
        const translated = String(x?.translated_text || x?.translation || '').trim()
        const source = String(x?.text || '').trim()
        return source && (!translated || translated === source)
      }).length,
    })
    activeAudioSyncGenerating.value = !audioUnavailable && triggerSyncGenerationForAudioId(activeAudio.value.id, false, learningLang.value)
    if (!audioUnavailable && shouldRunQuickTimelineTranslate(activeAudio.value.id)) {
      const patchedTimeline = await quickTranslateTimelineToZh(activeAudioSyncTimeline.value, activeTimelineQuickTranslating)
      if (Array.isArray(patchedTimeline) && patchedTimeline.length) {
        activeAudioSyncTimeline.value = patchedTimeline
        audioTranslationCoverageMap.value = {
          ...audioTranslationCoverageMap.value,
          [String(activeAudio.value.id || '')]: resolveTranslationCoverageFromTimeline(patchedTimeline),
        }
      }
    }
  }
  if (activeAudioSyncTimeline.value.length) {
    activeAudioSyncGenerating.value = false
  }
  await refreshActiveAudioPlayableSrc()
  setTimeout(() => {
    applyAudioPlaybackRate()
  }, 0)
  if (activeAudio.value?.id && !audioUnavailable && !activeAudio.value?.sync_json_uri && !activeAudioSyncTimeline.value.length) {
    triggerSyncGenerationForAudioId(activeAudio.value.id, false, learningLang.value)
  }
  console.info('[LangLearning] activeAudio:changed', {
    hasAudio: Boolean(activeAudio.value),
    audioId: activeAudio.value?.id || '',
    sourceUri: activeAudio.value?.source_uri || '',
    immersive: listeningImmersiveMode.value,
    ready: isListeningImmersiveReady.value,
  })
})

watch(currentReviewItem, async (row) => {
  if (row?.sourceType === 'audio') {
    const id = String(row?.payload?.id || '').trim()
    if (id && !audioResources.value.some((x) => String(x?.id || '').trim() === id)) {
      reviewAudioSyncTimeline.value = []
      return
    }
  }
  currentReviewTranscriptIndex.value = 0
  reviewTranscriptVisibleCount.value = transcriptScreenPageSize
  reviewAudioDurationSec.value = Number(row?.payload?.duration_sec || 0)
  const audioUnavailable = row?.payload?.audio_available === false
  reviewAudioSyncTimeline.value = audioUnavailable ? [] : await loadSyncTimelineByJsonPath(row?.payload?.sync_json_uri || '')
  if (!reviewAudioSyncTimeline.value.length) {
    reviewAudioSyncTimeline.value = buildTranscriptTimeline(row?.payload?.transcript || '', reviewAudioDurationSec.value || 0)
  }
  if (row?.payload?.id && reviewAudioSyncTimeline.value.length && timelineHasMissingTranslation(reviewAudioSyncTimeline.value)) {
    console.info('[LangLearning] reviewAudio:translation-missing', {
      audioId: row.payload.id,
      timelineSize: reviewAudioSyncTimeline.value.length,
      pending: reviewAudioSyncTimeline.value.filter((x) => {
        const translated = String(x?.translated_text || x?.translation || '').trim()
        const source = String(x?.text || '').trim()
        return source && (!translated || translated === source)
      }).length,
    })
    if (!audioUnavailable) triggerSyncGenerationForAudioId(row.payload.id, false, learningLang.value)
    if (!audioUnavailable && shouldRunQuickTimelineTranslate(row.payload.id)) {
      const patchedTimeline = await quickTranslateTimelineToZh(reviewAudioSyncTimeline.value, reviewTimelineQuickTranslating)
      if (Array.isArray(patchedTimeline) && patchedTimeline.length) {
        reviewAudioSyncTimeline.value = patchedTimeline
        audioTranslationCoverageMap.value = {
          ...audioTranslationCoverageMap.value,
          [String(row.payload.id || '')]: resolveTranslationCoverageFromTimeline(patchedTimeline),
        }
      }
    }
  }
  await refreshReviewAudioPlayableSrc()
})

watch([activeAudioSyncTimeline, () => activeAudio.value?.id], ([timeline, id]) => {
  const audioId = String(id || '').trim()
  if (!audioId) return
  audioTranslationCoverageMap.value = {
    ...audioTranslationCoverageMap.value,
    [audioId]: resolveTranslationCoverageFromTimeline(timeline),
  }
}, { deep: true })

watch([currentTranscriptLineIndex, activeAudioTranscriptTimeline], async ([idx, timeline]) => {
  if (!Array.isArray(timeline) || !timeline.length) return
  if (Number(idx || 0) + 6 >= activeTranscriptVisibleCount.value) {
    extendActiveTranscriptWindow(transcriptScreenPreloadSize)
  }
  await nextTick()
  keepActiveTranscriptLineVisible(immersiveTranscriptListRef, Number(idx || 0))
}, { flush: 'post' })

watch([currentReviewTranscriptIndex, currentReviewListeningTranscriptTimeline], async ([idx, timeline]) => {
  if (!Array.isArray(timeline) || !timeline.length) return
  if (Number(idx || 0) + 6 >= reviewTranscriptVisibleCount.value) {
    extendReviewTranscriptWindow(transcriptScreenPreloadSize)
  }
  await nextTick()
  keepActiveTranscriptLineVisible(reviewTranscriptListRef, Number(idx || 0))
}, { flush: 'post' })

watch(activeRow, (row) => {
  console.info('[LangLearning] activeRow:changed', {
    rowId: row?.id || '',
    rowType: row?.rowType || '',
    sourceType: row?.sourceType || '',
    selectedShelf: selectedShelf.value,
    immersive: listeningImmersiveMode.value,
  })
  if (!row || row.sourceType !== 'audio') {
    if (listeningImmersiveMode.value) listeningImmersiveMode.value = false
  }
  refreshActiveWordOffline()
})

watch([isListeningImmersive, activeAudio], ([immersive, audio]) => {
  if (!immersive) return
  if (audio) return
  console.warn('[LangLearning] immersive:auto-exit-no-audio', {
    selectedShelf: selectedShelf.value,
    activeRow: activeRow.value?.id || '',
  })
  listeningImmersiveMode.value = false
  if (selectedShelf.value === 'my_listening' && listeningRows.value.length) {
    if (!activeRow.value || activeRow.value.sourceType !== 'audio') {
      activeRow.value = listeningRows.value[0]
    }
  }
})

watch([reviewType, reviewRowsBase, () => reviewFilter.createdPeriod, () => reviewFilter.lastReviewPeriod, () => reviewFilter.masteryBand, () => reviewFilter.orderBy, () => reviewFilter.dueOnly, () => reviewFilter.includeNew], () => {
  rebuildReviewRows()
})

watch(filteredRows, () => {
  if (managerMode.value === 'random') {
    drawRandomCards()
  }
})

watch(activeTab, (tab) => {
  if (tab !== 'manager') {
    stopRandomCardAudio()
  }
  if (tab === 'review') {
    rebuildReviewRows()
  } else {
    restoreManagerRecords()
  }
})

watch(managerMode, (mode) => {
  if (mode !== 'random') {
    stopRandomCardAudio()
  }
})

onMounted(async () => {
  window.addEventListener('error', handleWindowResizeObserverWarning, true)
  window.addEventListener('keydown', handleRandomBoardKeydown)
  window.addEventListener('keydown', handleReviewKeydown)
  loadMasteryStore()
  loadListeningSources()
  persistListeningSources()
  await cleanupDuplicateListeningTopics()
  await refreshAll()
  await resumePendingSyncTasks()
  await refreshDictData()
  await refreshSyncJobs()
  if (hasRunningSyncJobs.value) ensureSyncPolling()
  if (filteredRows.value.length) {
    activeRow.value = filteredRows.value[0]
  }
  rebuildReviewRows()
})

onBeforeUnmount(() => {
  window.removeEventListener('error', handleWindowResizeObserverWarning, true)
  window.removeEventListener('keydown', handleRandomBoardKeydown)
  window.removeEventListener('keydown', handleReviewKeydown)
  if (learningPronounceAudio) {
    learningPronounceAudio.pause()
    learningPronounceAudio = null
  }
  stopRandomCardAudio()
  if (syncQueueHideTimer) {
    window.clearTimeout(syncQueueHideTimer)
    syncQueueHideTimer = 0
  }
  if (pendingSyncSweepTimer) {
    window.clearTimeout(pendingSyncSweepTimer)
    pendingSyncSweepTimer = 0
  }
  if (syncPollTimer) {
    window.clearInterval(syncPollTimer)
    syncPollTimer = 0
  }
  if (dictPollTimer) {
    window.clearInterval(dictPollTimer)
    dictPollTimer = 0
  }
  if (asrPollTimer) {
    window.clearInterval(asrPollTimer)
    asrPollTimer = 0
  }
  if (syncCompleteRefreshTimer) {
    window.clearTimeout(syncCompleteRefreshTimer)
    syncCompleteRefreshTimer = 0
  }
})
</script>

<style scoped>
.lang-learn-page {
  --ll-serif: 'Iowan Old Style', 'Palatino Linotype', 'Book Antiqua', Georgia, 'Songti SC', 'SimSun', serif;
  --ll-bg: var(--guaxi-app-bg);
  --ll-panel: var(--guaxi-panel-bg);
  --ll-border: var(--guaxi-border);
  --ll-border-soft: color-mix(in srgb, var(--guaxi-border) 62%, transparent);
  --ll-text: var(--guaxi-text-primary);
  --ll-muted: var(--guaxi-text-secondary);
  --ll-accent: var(--primary-color, #61afef);
  --ll-amber: var(--warning-color, #e5c07b);
  --ll-green: var(--success-color, #98c379);
  --ll-red: var(--error-color, #e06c75);
  --ll-paper: color-mix(in srgb, var(--guaxi-panel-bg) 97%, #c8a96a 3%);
  --ll-accent-wash: color-mix(in srgb, var(--ll-accent) 9%, transparent);
  --ll-accent-line: color-mix(in srgb, var(--ll-accent) 42%, var(--ll-border));
  --ll-shadow: 0 16px 40px rgba(0, 0, 0, 0.18);
  height: 100vh;
  box-sizing: border-box;
  background: var(--ll-bg);
  color: var(--ll-text);
  display: flex;
  flex-direction: column;
  font-family: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei');
  overflow: hidden;
}

.lang-learn-page *,
.lang-learn-page *::before,
.lang-learn-page *::after {
  box-sizing: border-box;
}

.lang-learn-page *::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.lang-learn-page *::-webkit-scrollbar-thumb {
  background: var(--guaxi-scrollbar-thumb, rgba(127, 127, 127, 0.32));
  border-radius: 8px;
}

.lang-learn-page *::-webkit-scrollbar-track {
  background: transparent;
}

.serif {
  font-family: var(--ll-serif);
  font-weight: 600;
  letter-spacing: -0.01em;
}

.eyebrow,
.section-label {
  font-size: 10px;
  line-height: 1.4;
  letter-spacing: 0.15em;
  text-transform: uppercase;
  color: var(--ll-muted);
}

.section-label {
  letter-spacing: 0.08em;
  font-size: 10.5px;
}

.muted {
  color: var(--ll-muted);
  font-size: 11px;
}

.dot-sep {
  color: var(--ll-muted);
  opacity: 0.55;
  margin: 0 5px;
}

/* ---------- top bar ---------- */
.ll-topbar {
  flex: 0 0 66px;
  height: 66px;
  display: flex;
  align-items: center;
  gap: 26px;
  padding: 0 22px;
  border-bottom: 1px solid var(--ll-border);
  background: color-mix(in srgb, var(--ll-panel) 86%, transparent);
  backdrop-filter: blur(8px);
  z-index: 20;
}

.ll-brand {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 190px;
}

.ll-brand-kicker {
  font-size: 9px;
  letter-spacing: 0.22em;
  color: var(--ll-muted);
}

.ll-brand-title {
  margin: 0;
  font-size: 21px;
  line-height: 1.18;
  color: var(--ll-text);
}

.ll-mainnav {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 100%;
}

.ll-navbtn {
  position: relative;
  height: 100%;
  padding: 0 14px;
  background: none;
  border: 0;
  color: var(--ll-muted);
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
  transition: color 0.16s ease;
}

.ll-navbtn::after {
  content: '';
  position: absolute;
  left: 14px;
  right: 14px;
  bottom: -1px;
  height: 2px;
  background: transparent;
  transition: background 0.16s ease;
}

.ll-navbtn:hover {
  color: var(--ll-text);
}

.ll-navbtn.active {
  color: var(--ll-text);
}

.ll-navbtn.active::after {
  background: var(--ll-accent);
}

.ll-topright {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.ll-langpair {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 3px 6px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 8px;
  background: color-mix(in srgb, var(--ll-panel) 92%, transparent);
}

.ll-langarrow {
  color: var(--ll-muted);
  font-size: 11px;
}

.ll-search {
  width: clamp(180px, 26vw, 320px);
}

.ll-syncpill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 26px;
  padding: 0 10px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--ll-amber) 38%, var(--ll-border));
  background: color-mix(in srgb, var(--ll-amber) 10%, transparent);
  color: color-mix(in srgb, var(--ll-amber) 72%, var(--ll-text));
  font-size: 11px;
  white-space: nowrap;
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 0 0 currentColor;
  animation: llPulse 1.4s infinite;
  flex: 0 0 auto;
}

@keyframes llPulse {
  0% { box-shadow: 0 0 0 0 color-mix(in srgb, currentColor 55%, transparent); opacity: 1; }
  70% { box-shadow: 0 0 0 6px transparent; opacity: 0.55; }
  100% { box-shadow: 0 0 0 0 transparent; opacity: 1; }
}

/* ---------- workspace ---------- */
.ll-workspace {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 228px minmax(0, 1fr) 412px;
  transition: grid-template-columns 0.18s ease;
}

.ll-workspace.inspector-hidden {
  grid-template-columns: 228px minmax(0, 1fr) 0;
}

.ll-rail {
  min-height: 0;
  overflow: auto;
  padding: 20px 14px 16px;
  border-right: 1px solid var(--ll-border);
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.rail-today {
  padding: 14px 14px 13px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 10px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--ll-accent) 5%, transparent), transparent 70%),
    var(--ll-panel);
}

.rail-today-line {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 6px 0 10px;
}

.rail-today-line strong {
  font-size: 34px;
  line-height: 1;
  font-weight: 600;
  color: var(--ll-text);
}

.rail-today-line span {
  font-size: 11px;
  color: var(--ll-muted);
}

.rail-meter {
  height: 3px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--ll-border) 70%, transparent);
  overflow: hidden;
}

.rail-meter span {
  display: block;
  height: 100%;
  background: linear-gradient(90deg, var(--ll-accent), var(--ll-green));
  transition: width 0.3s ease;
}

.rail-today-meta {
  margin-top: 9px;
  font-size: 10.5px;
  color: var(--ll-muted);
  line-height: 1.5;
}

.rail-group {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.rail-group > .eyebrow {
  margin: 0 0 7px 8px;
}

.rail-shelf {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  min-height: 36px;
  padding: 5px 9px 5px 7px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--ll-text);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
}

.rail-shelf:hover {
  background: var(--ll-accent-wash);
}

.rail-shelf.active {
  background: color-mix(in srgb, var(--ll-accent) 11%, transparent);
  border-color: color-mix(in srgb, var(--ll-accent) 30%, transparent);
}

.rail-glyph {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex: 0 0 24px;
  border: 1px solid var(--ll-border);
  border-radius: 6px;
  font-family: var(--ll-serif);
  font-size: 12px;
  color: var(--ll-muted);
  background: color-mix(in srgb, var(--ll-panel) 80%, transparent);
}

.rail-shelf.active .rail-glyph {
  border-color: var(--ll-accent-line);
  color: var(--ll-accent);
}

.rail-shelf-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rail-count {
  margin-left: auto;
  font-size: 10.5px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
}

.rail-action {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  min-height: 32px;
  padding: 4px 9px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: color 0.15s ease, background 0.15s ease;
}

.rail-action:hover {
  color: var(--ll-text);
  background: var(--ll-accent-wash);
}

.rail-foot {
  margin-top: auto;
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 8px;
  color: var(--ll-muted);
  font-size: 10px;
  letter-spacing: 0.04em;
}

.rail-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--ll-green);
}

/* ---------- ledger ---------- */
.ll-ledger {
  position: relative;
  min-width: 0;
  min-height: 0;
  padding: 14px 28px 14px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.ledger-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  padding-bottom: 10px;
}

.ledger-titleblock {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
  overflow: hidden;
}

.ledger-titleblock h2 {
  margin: 0;
  font-size: 20px;
  line-height: 1.2;
  font-weight: 600;
}

.ledger-subline {
  color: var(--ll-muted);
  font-size: 11.5px;
  white-space: nowrap;
}

.ledger-tools {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 7px;
}

.ledger-filterbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 0 0 10px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.filter-chips {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
  min-width: 0;
}

.filter-chip {
  height: 25px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.filter-chip:hover {
  color: var(--ll-text);
  border-color: var(--ll-border-soft);
}

.filter-chip.active {
  color: var(--ll-accent);
  border-color: color-mix(in srgb, var(--ll-accent) 32%, transparent);
  background: var(--ll-accent-wash);
}

.filter-hint {
  margin-left: 6px;
  font-size: 10.5px;
  color: var(--ll-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.filter-selects {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.ledger-loading {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--ll-muted);
  font-size: 12px;
}

.ledger-table {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  margin: 0 -10px;
  padding: 0 10px;
}

.ledger-thead,
.ledger-row {
  display: grid;
  align-items: center;
  gap: 10px;
}

.record-table .ledger-thead,
.record-table .ledger-row {
  grid-template-columns: 26px 34px 42px minmax(180px, 1fr) 140px 92px 92px;
}

.listening-table .ledger-thead,
.listening-table .ledger-row {
  grid-template-columns: 26px 34px minmax(200px, 1fr) 74px 132px 108px 30px;
}

.ledger-thead {
  position: sticky;
  top: 0;
  z-index: 3;
  height: 32px;
  background: var(--ll-bg);
  border-bottom: 1px solid var(--ll-border);
  color: var(--ll-muted);
  font-size: 9.5px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.ledger-row {
  min-height: 58px;
  padding: 7px 0;
  border-bottom: 1px solid var(--ll-border-soft);
  cursor: pointer;
  transition: background 0.13s ease, box-shadow 0.13s ease;
}

.ledger-row:hover {
  background: color-mix(in srgb, var(--ll-accent) 5%, transparent);
}

.ledger-row.active {
  background: color-mix(in srgb, var(--ll-accent) 8%, transparent);
  box-shadow: inset 2px 0 0 var(--ll-accent);
}

.ledger-row.selected {
  background: color-mix(in srgb, var(--ll-accent) 12%, transparent);
}

.ledger-row.error {
  box-shadow: inset 2px 0 0 var(--ll-red);
}

.col-check {
  display: flex;
  align-items: center;
  justify-content: center;
}

.ledger-row .col-check :deep(.n-checkbox) {
  opacity: 0;
  transition: opacity 0.12s ease;
}

.ledger-row:hover .col-check :deep(.n-checkbox),
.ledger-row.selected .col-check :deep(.n-checkbox) {
  opacity: 1;
}

.col-idx {
  font-size: 10px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.04em;
}

.col-main {
  min-width: 0;
}

.row-title {
  font-size: 15.5px;
  line-height: 1.35;
  color: var(--ll-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-translation,
.row-sub {
  margin-top: 2px;
  font-size: 11.5px;
  line-height: 1.4;
  color: var(--ll-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-sub {
  display: flex;
  align-items: center;
  gap: 2px;
}

.type-sigil {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 1px solid var(--ll-border);
  border-radius: 6px;
  font-family: var(--ll-serif);
  font-size: 11px;
  color: var(--ll-muted);
}

.col-phon {
  font-size: 11px;
  color: var(--ll-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mastery-dots {
  font-size: 9px;
  letter-spacing: 2px;
  color: var(--ll-amber);
  white-space: nowrap;
}

.mastery-dots.large {
  font-size: 12px;
  letter-spacing: 3px;
}

.col-date,
.col-duration {
  font-size: 10.5px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.col-status {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.row-progress {
  height: 2px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--ll-border) 70%, transparent);
  overflow: hidden;
}

.row-progress span {
  display: block;
  height: 100%;
  background: var(--ll-accent);
  transition: width 0.4s ease;
}

.col-coverage {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
}

.coverage-track {
  display: block;
  width: 56px;
  height: 3px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--ll-border) 70%, transparent);
  overflow: hidden;
}

.coverage-track > span {
  display: block;
  height: 100%;
  background: linear-gradient(90deg, var(--ll-accent), var(--ll-green));
}

.col-open {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  color: var(--ll-muted);
  transition: all 0.15s ease;
}

.ledger-row:hover .col-open {
  color: var(--ll-accent);
  background: var(--ll-accent-wash);
}

.sync-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  width: fit-content;
  max-width: 100%;
  height: 21px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--ll-border);
  font-size: 10px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sync-chip.tone-pending {
  color: var(--ll-muted);
  border-color: var(--ll-border);
}

.sync-chip.tone-running {
  color: color-mix(in srgb, var(--ll-accent) 84%, var(--ll-text));
  border-color: color-mix(in srgb, var(--ll-accent) 38%, var(--ll-border));
  background: var(--ll-accent-wash);
}

.sync-chip.tone-ready {
  color: var(--ll-green);
  border-color: color-mix(in srgb, var(--ll-green) 34%, var(--ll-border));
  background: color-mix(in srgb, var(--ll-green) 8%, transparent);
}

.sync-chip.tone-partial {
  color: color-mix(in srgb, var(--ll-amber) 82%, var(--ll-text));
  border-color: color-mix(in srgb, var(--ll-amber) 34%, var(--ll-border));
  background: color-mix(in srgb, var(--ll-amber) 8%, transparent);
}

.sync-chip.tone-error {
  color: var(--ll-red);
  border-color: color-mix(in srgb, var(--ll-red) 38%, var(--ll-border));
  background: color-mix(in srgb, var(--ll-red) 8%, transparent);
}

.table-footer {
  flex: 0 0 auto;
  display: flex;
  justify-content: flex-end;
  padding-top: 10px;
}

/* ---------- empty / selection ---------- */
.ledger-empty {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 30px;
}

.empty-card {
  width: min(380px, 90%);
  padding: 28px;
  border: 1px dashed color-mix(in srgb, var(--ll-border) 80%, transparent);
  border-radius: 12px;
  text-align: center;
  background:
    repeating-linear-gradient(0deg, transparent, transparent 27px, color-mix(in srgb, var(--ll-border) 22%, transparent) 28px),
    var(--ll-panel);
}

.empty-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  margin-bottom: 12px;
  border: 1px solid var(--ll-border);
  border-radius: 50%;
  font-family: var(--ll-serif);
  font-size: 18px;
  color: var(--ll-muted);
}

.empty-card h3 {
  margin: 0 0 7px;
  font-size: 20px;
}

.empty-card p {
  margin: 0 0 16px;
  color: var(--ll-muted);
  font-size: 11.5px;
  line-height: 1.7;
}

.selection-bar {
  position: absolute;
  left: 50%;
  bottom: 18px;
  transform: translateX(-50%);
  z-index: 12;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 9px 7px 14px;
  border: 1px solid var(--ll-accent-line);
  border-radius: 999px;
  background: color-mix(in srgb, var(--ll-panel) 94%, transparent);
  box-shadow: var(--ll-shadow);
  backdrop-filter: blur(10px);
  animation: selectionUp 0.18s ease;
}

@keyframes selectionUp {
  from { opacity: 0; transform: translate(-50%, 8px); }
  to { opacity: 1; transform: translate(-50%, 0); }
}

.selection-count {
  font-size: 11px;
  color: var(--ll-muted);
  margin-right: 4px;
  white-space: nowrap;
}

.selection-action {
  height: 26px;
  padding: 0 11px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  background: transparent;
  color: var(--ll-text);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}

.selection-action:hover {
  border-color: var(--ll-accent-line);
  color: var(--ll-accent);
}

.selection-action.danger:hover {
  border-color: color-mix(in srgb, var(--ll-red) 50%, var(--ll-border));
  color: var(--ll-red);
}

.selection-action.muted {
  color: var(--ll-muted);
}

/* ---------- inspector ---------- */
.ll-inspector {
  min-height: 0;
  overflow: auto;
  border-left: 1px solid var(--ll-border);
  padding: 18px 22px 26px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--ll-amber) 3%, transparent), transparent 46%),
    var(--ll-paper);
}

.inspector-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.icon-ghost {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--ll-muted);
  cursor: pointer;
}

.icon-ghost:hover {
  color: var(--ll-text);
  background: var(--ll-accent-wash);
}

.icon-ghost.danger:hover {
  color: var(--ll-red);
}

.word-hero {
  padding: 20px 0 16px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.word-overline {
  font-size: 9.5px;
  letter-spacing: 0.16em;
  color: var(--ll-muted);
  text-transform: uppercase;
}

.word-headword {
  margin: 6px 0 10px;
  font-size: 36px;
  line-height: 1.06;
  font-weight: 600;
  word-break: break-word;
}

.word-phonetics {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.phonetic-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 23px;
  padding: 0 8px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  font-size: 10.5px;
  color: var(--ll-muted);
  background: color-mix(in srgb, var(--ll-panel) 70%, transparent);
}

.phonetic-play {
  display: inline-flex;
  align-items: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--ll-accent);
  cursor: pointer;
}

.word-mastery {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.inspector-section {
  padding: 15px 0;
  border-bottom: 1px solid var(--ll-border-soft);
}

.inspector-section > .section-label {
  margin-bottom: 9px;
}

.meaning-list {
  margin: 0;
  padding: 0;
  list-style: none;
  counter-reset: meaning;
}

.meaning-list li {
  position: relative;
  padding: 3px 0 3px 24px;
  font-size: 12.5px;
  line-height: 1.65;
  counter-increment: meaning;
}

.meaning-list li::before {
  content: counter(meaning, decimal-leading-zero);
  position: absolute;
  left: 0;
  top: 5px;
  font-size: 9px;
  color: var(--ll-accent);
  letter-spacing: 0.04em;
}

.example-line {
  padding: 6px 0 6px 10px;
  border-left: 1px solid var(--ll-border-soft);
  margin-bottom: 5px;
}

.example-src {
  font-family: var(--ll-serif);
  font-size: 13px;
  line-height: 1.5;
  color: var(--ll-text);
}

.example-target {
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.55;
  color: var(--ll-muted);
}

.relation-group {
  display: flex;
  gap: 8px;
  padding: 4px 0;
}

.relation-label {
  flex: 0 0 30px;
  padding-top: 3px;
  font-size: 10px;
  color: var(--ll-muted);
}

.chip-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.word-chip {
  display: inline-flex;
  align-items: baseline;
  padding: 2px 7px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 5px;
  font-size: 10.5px;
  color: var(--ll-text);
  background: color-mix(in srgb, var(--ll-panel) 72%, transparent);
}

.word-chip em {
  font-style: normal;
  color: var(--ll-muted);
}

.word-chip.warn {
  border-color: color-mix(in srgb, var(--ll-amber) 28%, var(--ll-border));
}

.inspector-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  padding-top: 16px;
}

.entry-hero {
  padding: 20px 0 14px;
}

.entry-source {
  margin: 10px 0 14px;
  padding: 0 0 0 16px;
  border-left: 2px solid var(--ll-accent);
  font-size: 20px;
  line-height: 1.5;
  font-weight: 500;
}

.entry-translation {
  font-size: 13px;
  line-height: 1.75;
  color: var(--ll-muted);
  padding-left: 18px;
}

.entry-meta {
  display: flex;
  gap: 12px;
  margin-top: 16px;
  padding-left: 18px;
  font-size: 10px;
  color: var(--ll-muted);
}

.audio-hero {
  padding: 20px 0 14px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.audio-title {
  margin: 7px 0 8px;
  font-size: 24px;
  line-height: 1.25;
}

.audio-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  font-size: 10.5px;
  color: var(--ll-muted);
}

.sync-inspector-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.sync-message {
  margin-top: 6px;
  font-size: 10.5px;
  color: var(--ll-muted);
  line-height: 1.5;
}

.coverage-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  font-size: 10.5px;
  color: var(--ll-muted);
}

.preview-transcript {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.preview-transcript-line {
  padding-left: 10px;
  border-left: 1px solid var(--ll-border-soft);
}

.preview-transcript-line .transcript-source {
  font-family: var(--ll-serif);
  font-size: 13px;
  line-height: 1.45;
}

.preview-transcript-line .transcript-target {
  font-size: 11px;
  line-height: 1.5;
  color: var(--ll-muted);
  margin-top: 1px;
}

.detail-content {
  font-size: 12px;
  line-height: 1.7;
  color: var(--ll-muted);
  white-space: pre-wrap;
}

.inspector-empty {
  padding: 60px 20px;
  text-align: center;
}

.inspector-empty h3 {
  margin: 8px 0 7px;
  font-size: 20px;
}

.inspector-empty p {
  margin: 0;
  color: var(--ll-muted);
  font-size: 11.5px;
  line-height: 1.7;
}

/* ---------- immersive studio ---------- */
.is-immersive {
  background: var(--ll-bg);
}

.ll-studio {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.studio-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 14px 22px;
  border-bottom: 1px solid var(--ll-border);
  background: color-mix(in srgb, var(--ll-panel) 82%, transparent);
}

.studio-back {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 30px;
  padding: 0 9px 0 5px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.studio-back:hover {
  color: var(--ll-accent);
  border-color: var(--ll-accent-line);
}

.studio-titleblock {
  min-width: 0;
}

.studio-titleblock .eyebrow {
  margin-bottom: 3px;
}

.studio-title {
  margin: 0;
  font-size: 24px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.studio-headmeta {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.studio-duration {
  font-size: 11px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
}

.studio-grid {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(340px, 42%) minmax(0, 1fr);
}

.studio-player {
  min-height: 0;
  overflow: auto;
  padding: 22px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  border-right: 1px solid var(--ll-border);
}

.player-sheet {
  position: relative;
  border: 1px solid var(--ll-border);
  border-radius: 12px;
  padding: 14px;
  background:
    radial-gradient(120% 120% at 20% 0%, color-mix(in srgb, var(--ll-accent) 9%, transparent), transparent 60%),
    color-mix(in srgb, var(--ll-panel) 94%, #000 6%);
  box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 5%, transparent);
}

.listening-player {
  width: 100%;
  display: block;
}

.listening-video-player {
  max-height: 300px;
  border-radius: 7px;
  background: #000;
}

.player-row {
  display: flex;
  align-items: center;
  gap: 9px;
}

.player-hint {
  margin-left: auto;
  font-size: 10px;
  color: var(--ll-muted);
}

.sync-sheet {
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--ll-accent) 24%, var(--ll-border));
  border-radius: 9px;
  background: color-mix(in srgb, var(--ll-accent) 5%, transparent);
}

.sync-sheet-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 7px;
}

.sync-percent {
  font-size: 10px;
  color: var(--ll-accent);
  font-variant-numeric: tabular-nums;
}

.studio-verdict {
  padding: 12px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 9px;
  background: var(--ll-panel);
}

.verdict-actions {
  display: flex;
  gap: 6px;
  margin-top: 9px;
}

.verdict-btn {
  flex: 1;
  height: 31px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 7px;
  background: transparent;
  color: var(--ll-text);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.verdict-btn:hover {
  background: var(--ll-accent-wash);
  border-color: var(--ll-accent-line);
}

.verdict-btn.known:hover { color: var(--ll-green); border-color: color-mix(in srgb, var(--ll-green) 46%, var(--ll-border)); }
.verdict-btn.fuzzy:hover { color: var(--ll-amber); border-color: color-mix(in srgb, var(--ll-amber) 46%, var(--ll-border)); }
.verdict-btn.unknown:hover { color: var(--ll-red); border-color: color-mix(in srgb, var(--ll-red) 46%, var(--ll-border)); }

.studio-facts {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.fact {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 11px;
  color: var(--ll-text);
}

.studio-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: auto;
}

.studio-transcript {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 16px 22px 8px;
}

.transcript-head {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.transcript-sub {
  margin-top: 3px;
  font-size: 10.5px;
  color: var(--ll-muted);
}

.transcript-tools {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.seg-group {
  display: inline-flex;
  padding: 2px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 7px;
  background: color-mix(in srgb, var(--ll-panel) 80%, transparent);
}

.seg-group.small .seg-btn {
  height: 20px;
  padding: 0 7px;
  font-size: 9.5px;
}

.seg-btn {
  height: 23px;
  padding: 0 9px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 10.5px;
  cursor: pointer;
  transition: all 0.14s ease;
  white-space: nowrap;
}

.seg-btn:hover {
  color: var(--ll-text);
}

.seg-btn.active {
  background: color-mix(in srgb, var(--ll-accent) 14%, var(--ll-panel));
  color: var(--ll-accent);
}

.subtitle-toggle {
  border: 1px solid var(--ll-border-soft);
  border-radius: 6px;
}

.font-stepper {
  display: inline-flex;
  border: 1px solid var(--ll-border-soft);
  border-radius: 6px;
  overflow: hidden;
}

.font-stepper button {
  width: 28px;
  height: 25px;
  border: 0;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 10px;
  cursor: pointer;
}

.font-stepper button + button {
  border-left: 1px solid var(--ll-border-soft);
}

.font-stepper button:hover {
  color: var(--ll-accent);
  background: var(--ll-accent-wash);
}

.transcript-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 10px 6px 40px 0;
  scroll-behavior: smooth;
}

.transcript-line {
  display: flex;
  gap: 12px;
  padding: 9px 12px 9px 10px;
  border-left: 2px solid transparent;
  border-radius: 0 7px 7px 0;
  cursor: pointer;
  transition: background 0.13s ease, border-color 0.13s ease;
}

.transcript-line:hover,
.transcript-line.hovered {
  background: color-mix(in srgb, var(--ll-accent) 5%, transparent);
}

.transcript-line.active {
  background: color-mix(in srgb, var(--ll-accent) 9%, transparent);
  border-left-color: var(--ll-accent);
}

.transcript-index {
  flex: 0 0 20px;
  padding-top: 3px;
  font-size: 9px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
}

.transcript-body {
  min-width: 0;
}

.transcript-source {
  font-family: var(--ll-serif);
  font-size: calc(15px * var(--transcript-font-scale, 1.1));
  line-height: 1.55;
  color: var(--ll-text);
}

.transcript-line.active .transcript-source {
  color: color-mix(in srgb, var(--ll-accent) 40%, var(--ll-text));
}

.transcript-target {
  margin-top: 2px;
  font-size: calc(12px * var(--transcript-font-scale, 1.1));
  line-height: 1.55;
  color: var(--ll-muted);
}

.transcript-empty {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  align-items: center;
  gap: 30px;
  padding: 20px 8% 40px;
}

.skeleton-stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
  opacity: 0.6;
}

.skeleton-stack span {
  display: block;
  height: 13px;
  border-radius: 4px;
  background: linear-gradient(90deg,
    color-mix(in srgb, var(--ll-border) 45%, transparent) 25%,
    color-mix(in srgb, var(--ll-border) 80%, transparent) 37%,
    color-mix(in srgb, var(--ll-border) 45%, transparent) 63%);
  background-size: 400% 100%;
  animation: llShimmer 1.6s ease infinite;
}

@keyframes llShimmer {
  0% { background-position: 100% 50%; }
  100% { background-position: 0 50%; }
}

.transcript-empty-copy h3 {
  margin: 8px 0 8px;
  font-size: 26px;
}

.transcript-empty-copy p {
  margin: 0 0 16px;
  max-width: 360px;
  color: var(--ll-muted);
  font-size: 12px;
  line-height: 1.7;
}

.empty-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 300px;
  font-size: 10px;
  color: var(--ll-accent);
}

.empty-progress :deep(.n-progress) {
  flex: 1;
}

/* ---------- review ---------- */
.ll-review {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 18px 24px 22px;
}

.review-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 18px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--ll-border);
}

.review-titleblock h1 {
  margin: 2px 0 0;
  font-size: 28px;
  line-height: 1.1;
}

.review-counter {
  margin-left: auto;
  font-size: 11px;
  color: var(--ll-muted);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.08em;
}

.review-grid {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 190px minmax(0, 1fr) 250px;
  gap: 18px;
  padding-top: 18px;
}

.review-main {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  min-height: 0;
}

.review-typebar {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.review-type-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 11px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  background: transparent;
  color: var(--ll-muted);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.review-type-btn:hover {
  color: var(--ll-text);
  border-color: var(--ll-border);
}

.review-type-btn.active {
  color: var(--ll-accent);
  border-color: color-mix(in srgb, var(--ll-accent) 32%, transparent);
  background: var(--ll-accent-wash);
}

.review-type-btn.empty {
  opacity: 0.55;
}

.review-type-count {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  opacity: 0.8;
}

.review-side {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-right: 16px;
  border-right: 1px solid var(--ll-border-soft);
}

.review-stat {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.review-stat strong {
  font-family: var(--ll-serif);
  font-size: 22px;
  font-weight: 600;
}

.review-stat .rail-meter {
  margin-top: 4px;
}

.review-keys {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding-top: 14px;
  border-top: 1px solid var(--ll-border-soft);
  font-size: 10px;
  color: var(--ll-muted);
  line-height: 1.6;
}

kbd {
  display: inline-block;
  min-width: 18px;
  padding: 1px 4px;
  border: 1px solid var(--ll-border);
  border-bottom-width: 2px;
  border-radius: 4px;
  background: var(--ll-panel);
  font: inherit;
  font-size: 9px;
  text-align: center;
}

.review-card2 {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 18px 22px;
  border: 1px solid var(--ll-border);
  border-radius: 12px;
  background:
    repeating-linear-gradient(0deg, transparent, transparent 31px, color-mix(in srgb, var(--ll-border) 14%, transparent) 32px),
    var(--ll-panel);
  overflow: auto;
}

.review-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.review-card-tools {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-switch {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 10.5px;
  color: var(--ll-muted);
  cursor: pointer;
}

.dictation-box {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 0 4px;
  border-top: 1px solid var(--ll-border-soft);
}

.dictation-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dictation-result {
  font-size: 10.5px;
  color: var(--ll-amber);
}

.dictation-result.ok {
  color: var(--ll-green);
}

.type-chip {
  display: inline-flex;
  align-items: center;
  height: 21px;
  padding: 0 8px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  font-size: 9.5px;
  letter-spacing: 0.06em;
  color: var(--ll-muted);
}

.review-front {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 34px 12px;
  text-align: center;
}

.review-word {
  font-size: clamp(38px, 5.2vw, 64px);
  line-height: 1.05;
  word-break: break-word;
}

.review-phonetic {
  margin-top: 12px;
  font-size: 12px;
  color: var(--ll-muted);
}

.review-prompt {
  margin-top: 22px;
  font-size: 10px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--ll-muted) 76%, transparent);
}

.review-text {
  max-width: 680px;
  margin: 0;
  padding: 0 0 0 18px;
  border-left: 2px solid var(--ll-accent);
  text-align: left;
  font-family: var(--ll-serif);
  font-size: clamp(20px, 2.6vw, 30px);
  line-height: 1.5;
}

.review-listening {
  width: min(560px, 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.review-listening-player {
  width: 100%;
}

.review-answer {
  flex: 0 0 auto;
  max-height: 46%;
  overflow: auto;
  padding: 4px 2px 12px;
}

.answer-rule {
  height: 1px;
  margin-bottom: 14px;
  background: linear-gradient(90deg, var(--ll-accent), transparent 70%);
}

.answer-meta {
  display: flex;
  gap: 16px;
  font-size: 10.5px;
  color: var(--ll-muted);
}

.answer-text {
  margin-top: 8px;
  font-size: 13.5px;
  line-height: 1.75;
}

.answer-examples {
  margin-top: 14px;
}

.answer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.review-transcript-list {
  max-height: 280px;
  padding-right: 6px;
}

.review-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--ll-border-soft);
}

.review-grade {
  display: flex;
  gap: 5px;
}

.review-nav {
  margin-left: auto;
  display: flex;
  gap: 3px;
}

.review-filters {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding-left: 16px;
  border-left: 1px solid var(--ll-border-soft);
}

.filter-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 10.5px;
  color: var(--ll-muted);
}

.review-filter-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
}

.reveal-answer-enter-active,
.reveal-answer-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.reveal-answer-enter-from,
.reveal-answer-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.review-empty {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
}

.review-empty-actions {
  display: flex;
  gap: 8px;
}

/* ---------- random board ---------- */
.random-board {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 4px;
  overflow: auto;
}

.random-board-head {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--ll-muted);
  font-size: 11px;
}

.random-topic {
  color: var(--ll-text);
}

.random-deck-index {
  padding: 2px 8px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 999px;
  font-size: 10px;
}

.random-board-actions {
  margin-left: auto;
  display: flex;
  gap: 5px;
}

.random-cards-grid {
  flex: 1 1 auto;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  grid-auto-rows: minmax(0, 1fr);
  gap: 16px;
  min-height: 0;
}

.random-board-card {
  display: flex;
  min-height: 480px;
  perspective: 1400px;
  animation: cardPop 0.2s ease;
}

@keyframes cardPop {
  from { opacity: 0; transform: translateY(5px); }
  to { opacity: 1; transform: translateY(0); }
}

.random-card-flip {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 480px;
  transform-style: preserve-3d;
  transition: transform 0.38s cubic-bezier(0.2, 0.7, 0.2, 1);
}

.random-card-flip.flipped {
  transform: rotateY(180deg);
}

.random-card-face {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px;
  border: 1px solid var(--ll-border);
  border-radius: 12px;
  backface-visibility: hidden;
  background:
    linear-gradient(160deg, color-mix(in srgb, var(--ll-accent) 5%, transparent), transparent 55%),
    var(--ll-panel);
  overflow: auto;
}

.random-card-face.is-listening {
  background:
    linear-gradient(160deg, color-mix(in srgb, var(--ll-amber) 5%, transparent), transparent 55%),
    var(--ll-panel);
}

.random-card-face.back {
  transform: rotateY(180deg);
}

.random-card-type {
  font-size: 10px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--ll-muted);
}

.random-card-word {
  font-size: 30px;
  line-height: 1.15;
  word-break: break-word;
}

.random-card-phonetic {
  font-size: 12.5px;
  color: var(--ll-muted);
}

.random-card-text {
  font-size: 14px;
  line-height: 1.7;
  color: color-mix(in srgb, var(--ll-text) 84%, transparent);
  white-space: pre-wrap;
  word-break: break-word;
}

.random-card-text.clamped {
  display: -webkit-box;
  -webkit-line-clamp: 7;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.random-card-head-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.random-card-meta-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
}

.random-card-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.random-card-section.grow {
  flex: 1 1 auto;
  min-height: 0;
}

.random-card-content {
  font-size: 14px;
  line-height: 1.75;
  color: color-mix(in srgb, var(--ll-text) 84%, transparent);
  white-space: pre-wrap;
  word-break: break-word;
}

.random-card-content.scrollable {
  max-height: 180px;
  overflow: auto;
  padding-right: 4px;
}

.random-card-section.grow .random-card-content.scrollable,
.random-card-section.grow .random-card-transcript {
  flex: 1 1 auto;
  min-height: 0;
  max-height: none;
}

.random-card-transcript {
  overflow: auto;
  padding-right: 4px;
  scroll-behavior: smooth;
}

.random-transcript-line {
  padding: 5px 9px;
  border-radius: 6px;
  font-size: 14.5px;
  line-height: 1.75;
  color: color-mix(in srgb, var(--ll-text) 78%, transparent);
  transition: color 0.15s ease, background-color 0.15s ease;
}

.random-transcript-line.active {
  color: var(--ll-text);
  background: var(--ll-accent-wash);
  box-shadow: inset 2px 0 0 var(--ll-accent);
}

.random-card-audio-row {
  margin-top: auto;
  display: flex;
  justify-content: center;
  padding-top: 8px;
}

.random-audio-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--ll-accent-line);
  border-radius: 50%;
  background: var(--ll-accent-wash);
  color: var(--ll-accent);
  cursor: pointer;
  transition: transform 0.15s ease;
}

.random-audio-action:hover {
  transform: scale(1.05);
}

.random-audio-action.playing {
  color: var(--ll-green);
  border-color: color-mix(in srgb, var(--ll-green) 50%, var(--ll-border));
}

/* ---------- dialogs ---------- */
/* Naive 卡片底部 action 自带一档底色，和弹窗主体不一致，会在底部露出色带。 */
:global(body .n-card > .n-card__action) {
  background-color: transparent;
}

.dialog-lede {
  margin-bottom: 14px;
  color: var(--ll-muted);
  font-size: 11px;
  line-height: 1.6;
}

.dialog-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.dialog-spacer {
  flex: 1;
}

.confirm-body {
  font-size: 12px;
  line-height: 1.7;
  color: var(--ll-muted);
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.collect-dialog {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.collect-topic-row {
  display: flex;
  gap: 8px;
}

.collect-range {
  display: flex;
  align-items: flex-end;
  gap: 14px;
}

.range-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.range-field :deep(.n-input-number) {
  width: 112px;
}

.range-unit {
  font-size: 10px;
  color: var(--ll-muted);
}

.collect-source-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-top: 4px;
  border-top: 1px solid var(--ll-border-soft);
}

.source-hint {
  padding: 9px 11px;
  border-radius: 7px;
  border: 1px dashed var(--ll-border);
  color: var(--ll-muted);
  font-size: 10.5px;
  line-height: 1.6;
}

.preview-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.preview-topic {
  margin-left: auto;
  padding: 2px 9px;
  border: 1px solid var(--ll-accent-line);
  border-radius: 999px;
  color: var(--ll-accent);
  font-size: 10px;
}

.preview-list {
  max-height: 54vh;
  overflow: auto;
  display: flex;
  flex-direction: column;
}

.preview-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 4px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.preview-row.picked {
  background: color-mix(in srgb, var(--ll-accent) 5%, transparent);
}

.preview-row.disabled {
  opacity: 0.5;
}

.preview-main {
  min-width: 0;
  flex: 1;
}

.preview-title {
  font-family: var(--ll-serif);
  font-size: 14px;
  line-height: 1.4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.preview-meta {
  margin-top: 2px;
  font-size: 10px;
  color: var(--ll-muted);
}

.preview-flag {
  padding: 2px 7px;
  border: 1px solid var(--ll-border);
  border-radius: 999px;
  font-size: 9.5px;
  color: var(--ll-muted);
  white-space: nowrap;
}

.preview-flag.good {
  color: var(--ll-green);
  border-color: color-mix(in srgb, var(--ll-green) 32%, var(--ll-border));
}

.preview-empty {
  padding: 40px 10px;
  text-align: center;
  color: var(--ll-muted);
  font-size: 11.5px;
}

.source-manager-head {
  display: flex;
  align-items: center;
  gap: 7px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--ll-border-soft);
}

.source-manager-list {
  max-height: 48vh;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 10px;
}

.source-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 4px;
  border-bottom: 1px solid var(--ll-border-soft);
}

/* ---------- offline dictionary ---------- */
.offline-badge {
  display: inline-flex;
  align-items: center;
  height: 20px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--ll-green) 34%, var(--ll-border));
  border-radius: 999px;
  background: color-mix(in srgb, var(--ll-green) 8%, transparent);
  color: var(--ll-green);
  font-size: 9.5px;
  white-space: nowrap;
}

.dict-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 70vh;
  overflow: auto;
  padding-right: 4px;
}

.dict-pairline {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.dict-section {
  padding-top: 13px;
  border-top: 1px solid var(--ll-border-soft);
}

.dict-section > .section-label {
  margin-bottom: 9px;
}

.dict-catalog {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.dict-card {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 12px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 9px;
  background: var(--ll-panel);
}

.dict-card.installed {
  border-color: color-mix(in srgb, var(--ll-green) 30%, var(--ll-border));
}

.dict-card-head {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
}

.dict-name {
  font-size: 16px;
  line-height: 1.3;
}

.dict-desc {
  margin: 0;
  color: var(--ll-muted);
  font-size: 10.5px;
  line-height: 1.6;
}

.dict-meta {
  color: var(--ll-muted);
  font-size: 9.5px;
}

.dict-progress {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--ll-accent);
  font-size: 9.5px;
}

.dict-error {
  color: var(--ll-red);
  font-size: 10px;
  line-height: 1.5;
}

.dict-actions {
  display: flex;
  gap: 6px;
  margin-top: auto;
  padding-top: 3px;
}

.dict-pack-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.dict-pack-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 0;
  border-bottom: 1px solid var(--ll-border-soft);
}

.dict-pack-main {
  flex: 1;
  min-width: 0;
}

.dict-pack-name {
  font-size: 12.5px;
  line-height: 1.4;
}

.dict-import-row {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.dict-lookup-row {
  display: flex;
  gap: 7px;
}

.dict-lookup-list {
  display: flex;
  flex-direction: column;
  gap: 7px;
  max-height: 220px;
  overflow: auto;
  margin-top: 10px;
}

.dict-entry {
  padding: 8px 10px;
  border: 1px solid var(--ll-border-soft);
  border-radius: 8px;
  background: var(--ll-panel);
}

.dict-entry-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.dict-term {
  font-size: 18px;
  line-height: 1.2;
}

.dict-entry-meanings {
  margin-top: 4px;
  font-size: 11.5px;
  line-height: 1.6;
}

.dict-lookup-empty {
  margin-top: 8px;
}

.web-preview-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 0 10px;
}

.web-preview-head h3 {
  margin: 4px 0 0;
  font-size: 20px;
  line-height: 1.3;
}

.web-options {
  display: flex;
  align-items: center;
  gap: 18px;
  flex-wrap: wrap;
  padding: 4px 0 10px;
  font-size: 11px;
}

@media (max-width: 1080px) {
  .dict-catalog {
    grid-template-columns: 1fr;
  }
}

/* ---------- responsive ---------- */
@media (max-width: 1240px) {
  .ll-workspace {
    grid-template-columns: 200px minmax(0, 1fr) 360px;
  }
  .ll-workspace.inspector-hidden {
    grid-template-columns: 200px minmax(0, 1fr) 0;
  }
  .ll-search {
    width: 190px;
  }
  .review-grid {
    grid-template-columns: 170px minmax(0, 1fr) 220px;
  }
}

@media (max-width: 1080px) {
  .ll-brand {
    min-width: 150px;
  }
  .ll-brand-title {
    font-size: 18px;
  }
  .ll-workspace {
    grid-template-columns: 188px minmax(0, 1fr) 0;
  }
  .ll-inspector {
    display: none;
  }
  .record-table .ledger-thead,
  .record-table .ledger-row {
    grid-template-columns: 26px 34px 42px minmax(160px, 1fr) 120px 84px;
  }
  .record-table .col-date {
    display: none;
  }
}
</style>
